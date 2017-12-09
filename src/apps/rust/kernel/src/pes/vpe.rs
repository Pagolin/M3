use base::col::{String, ToString, Vec};
use base::cell::{Ref, RefCell, RefMut};
use base::cfg;
use base::dtu::{EpId, PEId, HEADER_COUNT, EP_COUNT, FIRST_FREE_EP};
use base::errors::{Code, Error};
use base::kif::{CapSel, PEDesc, Perm};
use base::rc::Rc;
use core::fmt;
use core::mem;
use thread;

use arch::kdtu;
use arch::loader::Loader;
use cap::{Capability, CapTable, KObject, SGateObject, RGateObject, MGateObject};
use pes::vpemng;
use platform;

pub type VPEId = usize;

bitflags! {
    pub struct VPEFlags : u32 {
        const BOOTMOD     = 0b00000001;
        const DAEMON      = 0b00000010;
        const IDLE        = 0b00000100;
        const INIT        = 0b00001000;
        const HASAPP      = 0b00010000;
        const MUXABLE     = 0b00100000; // TODO temporary
        const READY       = 0b01000000;
        const WAITING     = 0b10000000;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    RUNNING,
    SUSPENDED,
    DEAD
}

pub const INVALID_VPE: VPEId = 0xFFFF;

pub struct VPEDesc<'v> {
    pe: PEId,
    vpe: Option<&'v VPE>,
}

impl<'v> VPEDesc<'v> {
    pub fn new(pe: PEId, vpe: &'v VPE) -> VPEDesc<'v> {
        VPEDesc {
            pe: pe,
            vpe: Some(vpe),
        }
    }

    pub fn new_mem(pe: PEId) -> Self {
        VPEDesc {
            pe: pe,
            vpe: None,
        }
    }

    pub fn vpe(&self) -> Option<&VPE> {
        self.vpe
    }
    pub fn pe_id(&self) -> PEId {
        self.pe
    }
    pub fn vpe_id(&self) -> VPEId {
        match self.vpe {
            Some(v) => v.id(),
            None    => INVALID_VPE,
        }
    }
}

pub struct VPE {
    id: VPEId,
    pe: PEId,
    pid: i32,
    state: State,
    name: String,
    flags: VPEFlags,
    obj_caps: CapTable,
    eps_addr: usize,
    args: Vec<String>,
    req: Vec<String>,
    ep_caps: Vec<Option<CapSel>>,
    exit_code: Option<i32>,
    dtu_state: kdtu::State,
    rbufs_size: usize,
    headers: usize,
}

impl VPE {
    pub fn new(name: &str, id: VPEId, pe: PEId, flags: VPEFlags) -> Rc<RefCell<Self>> {
        let vpe = Rc::new(RefCell::new(VPE {
            id: id,
            pe: pe,
            pid: 0,
            state: State::DEAD,
            name: name.to_string(),
            flags: flags,
            obj_caps: CapTable::new(),
            eps_addr: 0,
            args: Vec::new(),
            req: Vec::new(),
            ep_caps: vec![None; EP_COUNT - FIRST_FREE_EP],
            exit_code: None,
            dtu_state: kdtu::State::new(),
            rbufs_size: 0,
            headers: 0,
        }));

        {
            let mut vpe_mut = vpe.borrow_mut();
            unsafe {
                vpe_mut.obj_caps.set_vpe(vpe.as_ptr());
            }

            // cap for own VPE
            vpe_mut.obj_caps_mut().insert(
                Capability::new(0, KObject::VPE(vpe.clone()))
            );
            // cap for own memory
            vpe_mut.obj_caps_mut().insert(
                Capability::new(1, KObject::MGate(MGateObject::new(
                    pe, id, 0, cfg::MEM_CAP_END, Perm::RWX
                )))
            );

            vpe_mut.init();
        }

        vpe
    }

    pub fn destroy(&mut self) {
        self.obj_caps.revoke_all();
    }

    #[cfg(target_os = "linux")]
    fn init(&mut self) {
    }

    #[cfg(target_os = "none")]
    fn init(&mut self) {
        use base::dtu;
        use base::cfg;
        use pes::vpemng;

        let rgate = RGateObject::new(cfg::SYSC_RBUF_ORD, cfg::SYSC_RBUF_ORD);

        // attach syscall receive endpoint
        {
            let mut rgate = rgate.borrow_mut();
            rgate.vpe = vpemng::KERNEL_VPE;
            rgate.order = cfg::SYSC_RBUF_ORD;
            rgate.msg_order = cfg::SYSC_RBUF_ORD;
            rgate.addr = platform::default_rcvbuf(self.pe_id());
            rgate.ep = Some(dtu::SYSC_SEP);
            self.config_rcv_ep(dtu::SYSC_REP, &mut rgate).unwrap();
        }

        // attach syscall endpoint
        let sgate = SGateObject::new(&rgate, self.id() as dtu::Label, cfg::SYSC_RBUF_SIZE as u64);
        self.config_snd_ep(dtu::SYSC_SEP, &sgate.borrow(), platform::kernel_pe()).unwrap();

        // attach upcall receive endpoint
        {
            let mut rgate = rgate.borrow_mut();
            rgate.order = cfg::UPCALL_RBUF_ORD;
            rgate.msg_order = cfg::UPCALL_RBUF_ORD;
            rgate.addr += cfg::SYSC_RBUF_SIZE;
            self.config_rcv_ep(dtu::UPCALL_REP, &mut rgate).unwrap();
        }

        // attach default receive endpoint
        {
            let mut rgate = rgate.borrow_mut();
            rgate.order = cfg::DEF_RBUF_ORD;
            rgate.msg_order = cfg::DEF_RBUF_ORD;
            rgate.addr += cfg::DEF_RBUF_SIZE;
            self.config_rcv_ep(dtu::DEF_REP, &mut rgate).unwrap();
        }

        self.rbufs_size = rgate.borrow().addr + (1 << rgate.borrow().order);
        self.rbufs_size -= platform::default_rcvbuf(self.pe_id());

        if !self.is_bootmod() {
            let loader = Loader::get();
            loader.init_app(self).unwrap();
        }
    }

    pub fn id(&self) -> VPEId {
        self.id
    }
    pub fn pe_id(&self) -> PEId {
        self.pe
    }
    pub fn desc(&self) -> VPEDesc {
        VPEDesc::new(self.pe, self)
    }
    pub fn pe_desc(&self) -> PEDesc {
        platform::pe_desc(self.pe_id())
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn args(&self) -> &Vec<String> {
        &self.args
    }
    pub fn requirements(&self) -> &Vec<String> {
        &self.req
    }

    pub fn obj_caps(&self) -> &CapTable {
        &self.obj_caps
    }
    pub fn obj_caps_mut(&mut self) -> &mut CapTable {
        &mut self.obj_caps
    }

    pub fn state(&self) -> State {
        self.state.clone()
    }
    pub fn set_state(&mut self, state: State) {
        self.state = state;
    }

    pub fn dtu_state(&mut self) -> &mut kdtu::State {
        &mut self.dtu_state
    }

    pub fn is_bootmod(&self) -> bool {
        !(self.flags & VPEFlags::BOOTMOD).is_empty()
    }
    pub fn is_daemon(&self) -> bool {
        !(self.flags & VPEFlags::DAEMON).is_empty()
    }
    pub fn make_daemon(&mut self) {
        self.flags |= VPEFlags::DAEMON;
    }
    pub fn add_arg(&mut self, arg: &str) {
        self.args.push(arg.to_string());
    }
    pub fn add_requirement(&mut self, req: &str) {
        self.req.push(req.to_string());
    }

    pub fn eps_addr(&self) -> usize {
        self.eps_addr
    }
    pub fn set_eps_addr(&mut self, eps: usize) {
        self.eps_addr = eps;
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }
    pub fn set_pid(&mut self, pid: i32) {
        self.pid = pid;
    }

    fn exit_event(&self) -> thread::Event {
        &self.exit_code as *const Option<i32> as thread::Event
    }

    pub fn start(&mut self, pid: i32) -> Result<(), Error> {
        self.flags |= VPEFlags::HASAPP;
        self.set_pid(pid);

        let loader = Loader::get();
        let pid = loader.load_app(self)?;
        self.set_pid(pid);
        Ok(())
    }

    pub fn wait(vpe: Rc<RefCell<VPE>>) -> Option<i32> {
        let event = vpe.borrow().exit_event();
        thread::ThreadManager::get().wait_for(event);
        mem::replace(&mut vpe.borrow_mut().exit_code, None)
    }

    pub fn stop(vpe: Rc<RefCell<VPE>>, exit_code: i32) {
        if vpe.borrow().flags.contains(VPEFlags::HASAPP) {
            vpe.borrow_mut().flags.remove(VPEFlags::HASAPP);
            vpe.borrow_mut().exit_code = Some(exit_code);

            thread::ThreadManager::get().notify(vpe.borrow().exit_event(), None);

            // if it's a boot module, there is nobody waiting for it; just remove it
            if vpe.borrow().is_bootmod() {
                let id = vpe.borrow().id();
                vpemng::get().remove(id);
            }
        }
    }

    pub fn ep_with_sel(&self, sel: CapSel) -> Option<EpId> {
        for ep in 0..EP_COUNT - FIRST_FREE_EP {
            match self.ep_caps[ep] {
                Some(s)     => if s == sel { return Some(ep + FIRST_FREE_EP) },
                None        => {},
            }
        }
        None
    }
    pub fn get_ep_sel(&self, ep: EpId) -> Option<CapSel> {
        self.ep_caps[ep - FIRST_FREE_EP].clone()
    }
    pub fn set_ep_sel(&mut self, ep: EpId, sel: Option<CapSel>) {
        self.ep_caps[ep - FIRST_FREE_EP] = sel;
    }

    pub fn config_snd_ep(&mut self, ep: EpId, obj: &Ref<SGateObject>, pe_id: PEId) -> Result<(), Error> {
        let rgate = obj.rgate.borrow();
        assert!(rgate.activated());

        klog!(EPS, "VPE{}:EP{} = {:?}", self.id(), ep, obj);

        self.dtu_state.config_send(
            ep, obj.label, pe_id, rgate.vpe, rgate.ep.unwrap(), rgate.msg_size(), obj.credits
        );
        self.update_ep(ep)
    }

    pub fn config_rcv_ep(&mut self, ep: EpId, obj: &mut RefMut<RGateObject>) -> Result<(), Error> {
        // it needs to be in the receive buffer space
        let addr = platform::default_rcvbuf(self.pe_id());
        let size = platform::rcvbufs_size(self.pe_id());

        // default_rcvbuf() == 0 means that we do not validate it
        if addr != 0 && (obj.addr < addr ||
                         obj.addr > addr + size ||
                         obj.addr + obj.size() > addr + size ||
                         obj.addr < addr + self.rbufs_size) {
            return Err(Error::new(Code::InvArgs));
        }

        // no free headers left?
        let msg_slots = 1 << (obj.order - obj.msg_order);
        if self.headers + msg_slots > HEADER_COUNT {
            return Err(Error::new(Code::OutOfMem));
        }

        // TODO really manage the header space and zero the headers first in case they are reused
        obj.header = self.headers;
        self.headers += msg_slots;

        klog!(EPS, "VPE{}:EP{} = {:?}", self.id(), ep, obj);

        self.dtu_state.config_recv(ep, obj.addr, obj.order, obj.msg_order, obj.header);
        self.update_ep(ep)?;

        thread::ThreadManager::get().notify(obj.get_event(), None);

        Ok(())
    }

    pub fn config_mem_ep(&mut self, ep: EpId, obj: &Ref<MGateObject>, off: usize) -> Result<(), Error> {
        if off >= obj.size || obj.addr + off < off {
            return Err(Error::new(Code::InvArgs));
        }

        klog!(EPS, "VPE{}:EP{} = {:?}", self.id(), ep, obj);

        // TODO
        self.dtu_state.config_mem(ep, obj.pe, obj.vpe, obj.addr + off, obj.size - off, obj.perms);
        self.update_ep(ep)
    }

    pub fn invalidate_ep(&mut self, ep: EpId, cmd: bool) -> Result<(), Error> {
        klog!(EPS, "VPE{}:EP{} = invalid", self.id(), ep);

        if cmd {
            if self.state == State::RUNNING {
                kdtu::KDTU::get().invalidate_ep_remote(&self.desc(), ep)?;
            }
            else {
                self.dtu_state.invalidate(ep, true)?;
            }
        }
        else {
            self.dtu_state.invalidate(ep, false)?;
            self.update_ep(ep)?;
        }
        Ok(())
    }

    fn update_ep(&mut self, ep: EpId) -> Result<(), Error> {
        if self.state == State::RUNNING {
            kdtu::KDTU::get().write_ep_remote(&self.desc(), ep, self.dtu_state.get_ep(ep))
        }
        else {
            Ok(())
        }
    }
}

impl fmt::Debug for VPE {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "VPE[id={}, pe={}, name={}, state={:?}]",
            self.id(), self.pe_id(), self.name(), self.state())
    }
}

/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores).
 *
 * M3 is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License version 2 as
 * published by the Free Software Foundation.
 *
 * M3 is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * General Public License version 2 for more details.
 */

use m3::cap::Selector;
use m3::cell::LazyStaticCell;
use m3::col::String;
use m3::com::{GateIStream, RecvGate};
use m3::errors::{Code, Error, VerboseError};
use m3::goff;
use m3::kif;
use m3::log;
use m3::pes::VPE;
use m3::reply_vmsg;
use m3::session::ResMngOperation;

use crate::childs::{self, Child, Id};
use crate::sendqueue;
use crate::subsys;

static RGATE: LazyStaticCell<RecvGate> = LazyStaticCell::default();

pub fn init(rgate: RecvGate) {
    RGATE.set(rgate);
}

pub fn rgate() -> &'static RecvGate {
    &RGATE
}

pub fn workloop<F, S>(mut func: F, mut spawn: S) -> Result<(), VerboseError>
where
    F: FnMut(),
    S: FnMut(&mut childs::OwnChild) -> Result<(), VerboseError>,
{
    let thmng = thread::ThreadManager::get();
    let upcall_rg = RecvGate::upcall();

    loop {
        if let Some(msg) = RGATE.fetch() {
            let is = GateIStream::new(msg, &RGATE);
            handle_request_async(is);
            subsys::start_delayed(&mut spawn)?;
        }

        if let Some(msg) = upcall_rg.fetch() {
            childs::get().handle_upcall_async(msg);
        }

        sendqueue::check_replies();

        func();

        if thmng.ready_count() > 0 {
            thmng.try_yield();
        }

        if childs::get().should_stop() {
            break;
        }

        VPE::sleep().ok();
    }

    if !thmng.cur().is_main() {
        thmng.stop();
        // just in case there is no ready thread
        m3::exit(0);
    }
    Ok(())
}

fn handle_request_async(mut is: GateIStream) {
    let op: Result<ResMngOperation, Error> = is.pop();
    let child = childs::get().child_by_id_mut(is.label() as Id).unwrap();

    let res = match op {
        Ok(ResMngOperation::REG_SERV) => reg_serv(&mut is, child),
        Ok(ResMngOperation::UNREG_SERV) => unreg_serv_async(&mut is, child),

        Ok(ResMngOperation::OPEN_SESS) => open_session_async(&mut is, child),
        Ok(ResMngOperation::CLOSE_SESS) => close_session_async(&mut is, child),

        Ok(ResMngOperation::ADD_CHILD) => add_child(&mut is, child),
        Ok(ResMngOperation::REM_CHILD) => rem_child_async(&mut is, child),

        Ok(ResMngOperation::ALLOC_MEM) => alloc_mem(&mut is, child),
        Ok(ResMngOperation::FREE_MEM) => free_mem(&mut is, child),

        Ok(ResMngOperation::ALLOC_PE) => match alloc_pe(&mut is, child) {
            // reply already done
            Ok(_) => return,
            Err(e) => Err(e),
        },
        Ok(ResMngOperation::FREE_PE) => free_pe(&mut is, child),

        Ok(ResMngOperation::USE_RGATE) => match use_rgate(&mut is, child) {
            // reply already done
            Ok(_) => return,
            Err(e) => Err(e),
        },
        Ok(ResMngOperation::USE_SGATE) => use_sgate(&mut is, child),

        Ok(ResMngOperation::USE_SEM) => use_sem(&mut is, child),

        _ => Err(Error::new(Code::InvArgs)),
    };

    match res {
        Err(e) => {
            log!(crate::LOG_DEF, "{}: {:?} failed: {}", child.name(), op, e);
            is.reply_error(e.code())
        },
        Ok(_) => is.reply_error(Code::None),
    }
    .ok(); // ignore errors; we might have removed the child in the meantime
}

fn reg_serv(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let sgate_sel: Selector = is.pop()?;
    let sessions: u32 = is.pop()?;
    let name: String = is.pop()?;

    child.reg_service(dst_sel, sgate_sel, name, sessions)
}

fn unreg_serv_async(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let notify: bool = is.pop()?;

    child.unreg_service_async(sel, notify)
}

fn open_session_async(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    child.open_session_async(dst_sel, &name)
}

fn close_session_async(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    child.close_session_async(sel)
}

fn add_child(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let vpe_sel: Selector = is.pop()?;
    let sgate_sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    child.add_child(vpe_sel, &RGATE, sgate_sel, name)
}

fn rem_child_async(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let vpe_sel: Selector = is.pop()?;

    child.rem_child_async(vpe_sel)
}

fn alloc_mem(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let addr: goff = is.pop()?;
    let size: goff = is.pop()?;
    let perms = kif::Perm::from_bits_truncate(is.pop::<u32>()?);

    if addr == !0 {
        child.alloc_mem(dst_sel, size, perms)
    }
    else {
        child.alloc_mem_at(dst_sel, addr, size, perms)
    }
}

fn free_mem(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    child.free_mem(sel)
}

fn alloc_pe(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let dst_sel: Selector = is.pop()?;
    let name: &str = is.pop()?;

    child
        .alloc_pe(dst_sel, &name)
        .and_then(|(id, desc)| reply_vmsg!(is, Code::None as u32, id, desc.value()))
}

fn free_pe(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;

    child.free_pe(sel)
}

fn use_rgate(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    child
        .use_rgate(&name, sel)
        .and_then(|(order, msg_order)| reply_vmsg!(is, Code::None as u32, order, msg_order))
}

fn use_sgate(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    child.use_sgate(&name, sel)
}

fn use_sem(is: &mut GateIStream, child: &mut dyn Child) -> Result<(), Error> {
    let sel: Selector = is.pop()?;
    let name: String = is.pop()?;

    child.use_sem(&name, sel)
}

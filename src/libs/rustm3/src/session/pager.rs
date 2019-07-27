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

use cap;
use com::{SendGate, RecvGate, RGateArgs};
use core::fmt;
use dtu::{EpId, FIRST_FREE_EP};
use errors::Error;
use goff;
use kif;
use session::ClientSession;
use vpe::VPE;

/// Represents a session at the pager.
///
/// The pager is responsible to resolve page faults and allows to create memory mappings.
pub struct Pager {
    sess: ClientSession,
    sep: EpId,
    rep: EpId,
    rbuf: usize,
    rgate: Option<RecvGate>,
    parent_sgate: SendGate,
    child_sgate: SendGate,
}

int_enum! {
    struct DelOp : u32 {
        const DATASPACE = 0x0;
        const MEMGATE   = 0x1;
    }
}

int_enum! {
    struct Operation : u32 {
        const PAGEFAULT = 0x0;
        const CLONE     = 0x1;
        const MAP_ANON  = 0x2;
        const UNMAP     = 0x3;
    }
}

impl Pager {
    /// Creates a new session for VPE `vpe` at the pager service with given name.
    pub fn new(vpe: &mut VPE, rbuf: usize, pager: &str) -> Result<Self, Error> {
        let sess = ClientSession::new(pager)?;
        Self::create(vpe, rbuf, sess)
    }

    /// Binds a new pager-session to given selectors.
    pub fn new_bind(sess_sel: cap::Selector, rgate: cap::Selector) -> Result<Self, Error> {
        let sess = ClientSession::new_bind(sess_sel);
        let sgate = SendGate::new_bind(sess.obtain_obj()?);
        Ok(Pager {
            sess,
            sep: 0,
            rep: 0,
            rbuf: 0,
            rgate: Some(RecvGate::new_bind(rgate, 6)),
            parent_sgate: sgate,
            child_sgate: SendGate::new_bind(kif::INVALID_SEL),
        })
    }

    /// Clones the session to be shared with given VPE.
    pub fn new_clone(&self, vpe: &mut VPE, rbuf: usize) -> Result<Self, Error> {
        let mut args = kif::syscalls::ExchangeArgs::default();
        // dummy arg to distinguish from the get_sgate operation
        args.count = 1;
        let sess = self.sess.obtain(1, &mut args)?;
        Self::create(vpe, rbuf, ClientSession::new_owned_bind(sess.start()))
    }

    fn create(vpe: &mut VPE, rbuf: usize, sess: ClientSession) -> Result<Self, Error> {
        let parent_sgate = SendGate::new_bind(sess.obtain_obj()?);
        let child_sgate = SendGate::new_bind(sess.obtain_obj()?);
        let sep = vpe.alloc_ep()?;
        let rep = vpe.alloc_ep()?;
        let rgate = if vpe.pe().has_mmu() {
            Some(RecvGate::new_with(RGateArgs::default().order(6).msg_order(6))?)
        }
        else {
            None
        };

        Ok(Pager {
            sess,
            sep,
            rep,
            rbuf,
            rgate,
            parent_sgate,
            child_sgate,
        })
    }

    /// Returns the sessions capability selector.
    pub fn sel(&self) -> cap::Selector {
        self.sess.sel()
    }

    /// Returns the endpoint used for sending page faults.
    pub fn sep(&self) -> EpId {
        self.sep
    }
    /// Returns the endpoint used for receiving page fault replies.
    pub fn rep(&self) -> EpId {
        self.rep
    }

    /// Returns the [`SendGate`] used by the parent to send requests to the pager.
    pub fn parent_sgate(&self) -> &SendGate {
        &self.parent_sgate
    }
    /// Returns the [`SendGate`] used by the child to send page faults to the pager.
    pub fn child_sgate(&self) -> &SendGate {
        &self.child_sgate
    }
    /// Returns the [`RecvGate`] used to receive page fault replies.
    pub fn rgate(&self) -> Option<&RecvGate> {
        self.rgate.as_ref()
    }

    /// Delegates the required capabilities from `vpe` to the server.
    pub fn delegate_caps(&mut self, vpe: &VPE) -> Result<(), Error> {
        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, vpe.sel(), 2);
        self.sess.delegate_crd(crd)
    }

    /// Deactivates the gates (necessary after stopping the VPE).
    // TODO actually, this was only necessary to use a VPE multiple times, right?
    pub fn deactivate(&mut self) {
        if let Some(ref mut rg) = self.rgate {
            rg.deactivate();
        }
    }
    /// Activates the gates (necessary before starting the VPE).
    pub fn activate(&mut self, first_ep: cap::Selector) -> Result<(), Error> {
        self.child_sgate.activate_for(first_ep + (self.sep - FIRST_FREE_EP) as cap::Selector)?;

        if let Some(ref mut rg) = self.rgate {
            assert!(self.rbuf != 0);
            rg.activate_for(first_ep, self.rep, self.rbuf as goff)
        }
        else {
            Ok(())
        }
    }

    /// Performs the clone-operation on server-side using copy-on-write.
    #[allow(clippy::should_implement_trait)]
    pub fn clone(&self) -> Result<(), Error> {
        send_recv_res!(
            &self.parent_sgate, RecvGate::def(),
            Operation::CLONE
        ).map(|_| ())
    }

    /// Sends a page fault for the virtual address `addr` for given access type to the server.
    pub fn pagefault(&self, addr: goff, access: u32) -> Result<(), Error> {
        send_recv_res!(
            &self.parent_sgate, RecvGate::def(),
            Operation::PAGEFAULT, addr, access
        ).map(|_| ())
    }

    /// Maps `len` bytes of anonymous memory to virtual address `addr` with permissions `prot`.
    pub fn map_anon(&self, addr: goff, len: usize, prot: kif::Perm) -> Result<goff, Error> {
        let mut reply = send_recv_res!(
            &self.parent_sgate, RecvGate::def(),
            Operation::MAP_ANON, addr, len, prot.bits(), 0
        )?;
        Ok(reply.pop())
    }

    /// Maps a dataspace of `len` bytes handled by given session to virtual address `addr` with
    /// permissions `prot`.
    pub fn map_ds(&self, addr: goff, len: usize, off: usize, prot: kif::Perm,
                  sess: &ClientSession) -> Result<goff, Error> {
        let mut args = kif::syscalls::ExchangeArgs {
            count: 5,
            vals: kif::syscalls::ExchangeUnion {
                i: [
                    u64::from(DelOp::DATASPACE.val),
                    addr as u64,
                    len as u64,
                    u64::from(prot.bits()),
                    off as u64,
                    0, 0, 0
                ]
            },
        };

        let crd = kif::CapRngDesc::new(kif::CapType::OBJECT, sess.sel(), 1);
        self.sess.delegate(crd, &mut args)?;
        unsafe {
            Ok(args.vals.i[0] as goff)
        }
    }

    /// Unaps the mapping at virtual address `addr`.
    pub fn unmap(&self, addr: goff) -> Result<(), Error> {
        send_recv_res!(
            &self.parent_sgate, RecvGate::def(),
            Operation::UNMAP, addr
        ).map(|_| ())
    }
}

impl fmt::Debug for Pager {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "Pager[sel: {}, rep: {}, sep: {}]",
            self.sel(), self.rep, self.parent_sgate.ep().unwrap())
    }
}

/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

use m3::boxed::Box;
use m3::com::{GateIStream, RecvGate};
use m3::errors::{Code, Error, VerboseError};
use m3::log;
use m3::reply_vmsg;
use m3::session::resmng;
use m3::tiles::OwnActivity;
use m3::vec::Vec;

use crate::childs::{ChildManager, Id, OwnChild};
use crate::resources::Resources;
use crate::sendqueue;
use crate::subsys::{self, ChildStarter};

pub struct Requests {
    rgate: RecvGate,
}

impl Requests {
    pub fn new(rgate: RecvGate) -> Self {
        Self { rgate }
    }

    pub fn recv_gate(&self) -> &RecvGate {
        &self.rgate
    }

    pub fn run_loop<F>(
        &self,
        childs: &mut ChildManager,
        delayed: &mut Vec<Box<OwnChild>>,
        res: &mut Resources,
        mut func: F,
        starter: &mut dyn ChildStarter,
    ) -> Result<(), VerboseError>
    where
        F: FnMut(&mut ChildManager, &mut Resources),
    {
        let upcall_rg = RecvGate::upcall();

        loop {
            {
                if let Ok(msg) = self.rgate.fetch() {
                    let is = GateIStream::new(msg, &self.rgate);
                    self.handle_request_async(childs, res, is);
                    subsys::start_delayed_async(childs, delayed, self, res, starter)?;
                }
            }

            if let Ok(msg) = upcall_rg.fetch() {
                childs.handle_upcall_async(self, res, msg);
            }

            sendqueue::check_replies(res);

            func(childs, res);

            if thread::ready_count() > 0 {
                thread::try_yield();
            }

            if childs.should_stop() {
                break;
            }

            OwnActivity::sleep().ok();
        }

        if !thread::cur().is_main() {
            thread::stop();
            // just in case there is no ready thread
            OwnActivity::exit(Ok(()));
        }
        Ok(())
    }

    fn handle_request_async(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        mut is: GateIStream<'_>,
    ) {
        let op: Result<resmng::Operation, Error> = is.pop();
        let id = is.label() as Id;

        let res = match op {
            Ok(resmng::Operation::REG_SERV) => self.reg_serv(childs, res, &mut is, id),
            Ok(resmng::Operation::UNREG_SERV) => self.unreg_serv(childs, res, &mut is, id),

            Ok(resmng::Operation::OPEN_SESS) => self.open_session_async(childs, res, &mut is, id),
            Ok(resmng::Operation::CLOSE_SESS) => self.close_session_async(childs, res, &mut is, id),

            Ok(resmng::Operation::ADD_CHILD) => self.add_child(childs, res, &mut is, id),
            Ok(resmng::Operation::REM_CHILD) => self.rem_child_async(childs, res, &mut is, id),

            Ok(resmng::Operation::ALLOC_MEM) => self.alloc_mem(childs, res, &mut is, id),
            Ok(resmng::Operation::FREE_MEM) => self.free_mem(childs, res, &mut is, id),

            Ok(resmng::Operation::ALLOC_TILE) => match self.alloc_tile(childs, res, &mut is, id) {
                // reply already done
                Ok(_) => return,
                Err(e) => Err(e),
            },
            Ok(resmng::Operation::FREE_TILE) => self.free_tile(childs, res, &mut is, id),

            Ok(resmng::Operation::USE_RGATE) => match self.use_rgate(childs, res, &mut is, id) {
                // reply already done
                Ok(_) => return,
                Err(e) => Err(e),
            },
            Ok(resmng::Operation::USE_SGATE) => self.use_sgate(childs, res, &mut is, id),

            Ok(resmng::Operation::USE_SEM) => self.use_sem(childs, res, &mut is, id),

            Ok(resmng::Operation::USE_MOD) => self.use_mod(childs, res, &mut is, id),

            Ok(resmng::Operation::GET_SERIAL) => self.get_serial(childs, res, &mut is, id),

            Ok(resmng::Operation::GET_INFO) => self.get_info(childs, res, &mut is, id),

            _ => Err(Error::new(Code::InvArgs)),
        };

        match res {
            Err(e) => {
                let child = childs.child_by_id_mut(id).unwrap();
                log!(crate::LOG_DEF, "{}: {:?} failed: {}", child.name(), op, e);
                is.reply_error(e.code())
            },
            Ok(_) => is.reply_error(Code::Success),
        }
        .ok(); // ignore errors; we might have removed the child in the meantime
    }

    fn reg_serv(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::RegServiceReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.reg_service(res, req.dst, req.sgate, req.name, req.sessions)
    }

    fn unreg_serv(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::FreeReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.unreg_service(res, req.sel)
    }

    fn open_session_async(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::OpenSessionReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.open_session_async(res, id, req.dst, &req.name)
    }

    fn close_session_async(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::FreeReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.close_session_async(res, id, req.sel)
    }

    fn add_child(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::AddChildReq = is.pop()?;

        childs.add_child(res, id, req.id, req.sel, &self.rgate, req.sgate, req.name)
    }

    fn rem_child_async(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::FreeReq = is.pop()?;

        childs.rem_child_async(self, res, id, req.sel)
    }

    fn alloc_mem(
        &self,
        childs: &mut ChildManager,
        _res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::AllocMemReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.alloc_mem(req.dst, req.size, req.perms)
    }

    fn free_mem(
        &self,
        childs: &mut ChildManager,
        _res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::FreeReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.free_mem(req.sel)
    }

    fn alloc_tile(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::AllocTileReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child
            .alloc_tile(res, req.dst, req.desc, req.inherit_pmp)
            .and_then(|(id, desc)| {
                reply_vmsg!(is, Code::Success, resmng::AllocTileReply { id, desc })
            })
    }

    fn free_tile(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::FreeReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.free_tile(res, req.sel)
    }

    fn use_rgate(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::UseReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child
            .use_rgate(res, &req.name, req.dst)
            .and_then(|(order, msg_order)| {
                reply_vmsg!(is, Code::Success, resmng::UseRGateReply {
                    order,
                    msg_order
                })
            })
    }

    fn use_sgate(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::UseReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.use_sgate(res, &req.name, req.dst)
    }

    fn use_sem(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::UseReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.use_sem(res, &req.name, req.dst)
    }

    fn use_mod(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::UseReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.use_mod(res, &req.name, req.dst)
    }

    fn get_serial(
        &self,
        childs: &mut ChildManager,
        _res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::GetSerialReq = is.pop()?;

        let child = childs.child_by_id_mut(id).unwrap();
        child.get_serial(req.dst)
    }

    fn get_info(
        &self,
        childs: &mut ChildManager,
        res: &mut Resources,
        is: &mut GateIStream<'_>,
        id: Id,
    ) -> Result<(), Error> {
        let req: resmng::GetInfoReq = is.pop()?;

        let idx = match req.idx {
            usize::MAX => None,
            n => Some(n),
        };

        childs
            .get_info(res, id, idx)
            .and_then(|info| reply_vmsg!(is, Code::Success, info))
    }
}

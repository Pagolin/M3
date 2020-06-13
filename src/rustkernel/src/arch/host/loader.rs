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

use base::cell::StaticCell;
use base::cfg;
use base::col::{String, ToString, Vec};
use base::tcu::PEId;
use base::errors::{Code, Error};
use base::kif;
use base::libc;
use base::rc::Rc;

use ktcu;
use pes::{pemng, State, VPEId, VPE};

pub fn init(build_dir: &str) {
    LOADER.set(Some(Loader::new(build_dir)));
}

static LOADER: StaticCell<Option<Loader>> = StaticCell::new(None);

pub struct Loader {
    build_dir: String,
}

impl Loader {
    fn new(build_dir: &str) -> Self {
        Loader {
            build_dir: build_dir.to_string(),
        }
    }

    pub fn get() -> &'static mut Loader {
        LOADER.get_mut().as_mut().unwrap()
    }

    pub fn init_memory(&mut self, _vpe: &Rc<VPE>) -> Result<(), Error> {
        Ok(())
    }

    pub fn start(&mut self, vpe: &Rc<VPE>) -> Result<i32, Error> {
        if let Some(pid) = vpe.pid() {
            vpe.set_state(State::RUNNING);
            Self::write_env_file(pid, vpe.id(), vpe.pe_id(), 0);
            return Ok(pid);
        }

        let pid = unsafe { libc::fork() };
        match pid {
            -1 => Err(Error::new(Code::OutOfMem)),
            0 => {
                let pid = unsafe { libc::getpid() };
                Self::write_env_file(pid, vpe.id(), vpe.pe_id(), vpe.first_sel());

                let mut arg = self.build_dir.clone();
                arg.push('/');
                arg.push_str(vpe.name());
                arg.push('\0');

                let mut argv: Vec<*const i8> = Vec::new();
                argv.push(arg.as_ptr() as *const i8);
                argv.push(0 as *const i8);

                klog!(VPES, "Loading mod '{}':", vpe.name());

                unsafe {
                    libc::execv(argv[0], argv.as_ptr());
                    // special error code to let the WorkLoop delete the VPE
                    libc::exit(255);
                }
            },
            pid => {
                vpe.set_state(State::RUNNING);
                Ok(pid)
            },
        }
    }

    pub fn finish_start(&self, vpe: &Rc<VPE>) -> Result<(), Error> {
        let pemux = pemng::get().pemux(vpe.pe_id());
        // update all EPs (e.g., to allow parents to activate EPs for their childs)
        // set base for all receive EPs (do it for all, but it's just unused for the other types)
        pemux.update_eps()
    }

    fn write_env_file(pid: i32, id: VPEId, pe: PEId, first_sel: kif::CapSel) {
        let path = format!("/tmp/m3/{}\0", pid);
        let data = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            "foo", // TODO SHM prefix
            pe,
            first_sel,
            kif::FIRST_FREE_SEL,
            id,
            ktcu::KSYS_EP,
            cfg::SYSC_RBUF_SIZE,
        );

        unsafe {
            let fd = libc::open(
                path.as_bytes().as_ptr() as *const i8,
                libc::O_WRONLY | libc::O_TRUNC | libc::O_CREAT,
                0o600,
            );
            assert!(fd != -1);
            libc::write(fd, data.as_ptr() as *const libc::c_void, data.len());
            libc::close(fd);
        }
    }
}

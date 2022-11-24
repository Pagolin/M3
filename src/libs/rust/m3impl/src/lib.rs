/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#![feature(core_intrinsics)]
#![feature(trace_macros)]
#![no_std]

#[macro_use]
pub mod io;
#[macro_use]
pub mod com;

/// Netstack related structures
pub mod net;

pub use base::{
    backtrace, borrow, boxed, build_vmsg, cell, cfg, col, cpu, elf, errors, format, function, goff,
    impl_boxitem, int_enum, kif, libc, llog, log, mem, quota, rc, serde, serialize, sync, tcu,
    time, tmif, util, vec,
};

pub mod cap;
pub mod compat;
pub mod crypto;
pub mod env;
pub mod server;
pub mod session;
pub mod syscalls;
#[macro_use]
pub mod test;
pub mod tiles;
pub mod vfs;

use base::errors::Error;

use core::ptr;

use crate::tiles::Activity;

extern "C" {
    fn __m3_init_libc(argc: i32, argv: *const *const u8, envp: *const *const u8);
}

extern "Rust" {
    fn main() -> Result<(), Error>;
}

pub fn deinit() {
    io::deinit();
    vfs::deinit();
}

#[no_mangle]
pub extern "C" fn env_run() {
    unsafe {
        __m3_init_libc(0, ptr::null(), ptr::null());
    }
    syscalls::init();
    com::pre_init();
    tiles::init();
    io::init();
    com::init();

    let res = if let Some(cl) = env::get().load_closure() {
        cl()
    }
    else {
        unsafe { main() }
    };

    Activity::own().exit(res);
}

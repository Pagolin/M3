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

use m3::com::{MGateArgs, MemGate, Perm};
use m3::errors::Code;
use m3::test;

pub fn run(t: &mut dyn test::WvTester) {
    wv_run_test!(t, create);
    wv_run_test!(t, create_readonly);
    wv_run_test!(t, create_writeonly);
    wv_run_test!(t, derive);
    wv_run_test!(t, read_write);
    wv_run_test!(t, read_write_object);
}

fn create() {
    wv_assert_err!(
        MemGate::new_with(MGateArgs::new(0x1000, Perm::R).sel(1)),
        Code::InvArgs
    );
}

fn create_readonly() {
    let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::R));
    let mut data = [0u8; 8];
    wv_assert_err!(mgate.write(&data, 0), Code::NoPerm);
    wv_assert_ok!(mgate.read(&mut data, 0));
}

fn create_writeonly() {
    let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::W));
    let mut data = [0u8; 8];
    wv_assert_err!(mgate.read(&mut data, 0), Code::NoPerm);
    wv_assert_ok!(mgate.write(&data, 0));
}

fn derive() {
    let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::RW));
    wv_assert_err!(mgate.derive(0x0, 0x2000, Perm::RW), Code::InvArgs);
    wv_assert_err!(mgate.derive(0x1000, 0x10, Perm::RW), Code::InvArgs);
    wv_assert_err!(mgate.derive(0x800, 0x1000, Perm::RW), Code::InvArgs);
    let dgate = wv_assert_ok!(mgate.derive(0x800, 0x800, Perm::R));
    let mut data = [0u8; 8];
    wv_assert_err!(dgate.write(&data, 0), Code::NoPerm);
    wv_assert_ok!(dgate.read(&mut data, 0));
}

fn read_write() {
    let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::RW));
    let refdata = [0u8, 1, 2, 3, 4, 5, 6, 7];
    let mut data = [0u8; 8];
    wv_assert_ok!(mgate.write(&refdata, 0));
    wv_assert_ok!(mgate.read(&mut data, 0));
    wv_assert_eq!(data, refdata);

    wv_assert_ok!(mgate.read(&mut data[0..4], 4));
    wv_assert_eq!(&data[0..4], &refdata[4..8]);
    wv_assert_eq!(&data[4..8], &refdata[4..8]);
}

fn read_write_object() {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Test {
        a: u32,
        b: u64,
        c: bool,
    }

    let mgate = wv_assert_ok!(MemGate::new(0x1000, Perm::RW));
    let refobj = Test {
        a: 0x1234,
        b: 0xF000_F000_AAAA_BBBB,
        c: true,
    };

    wv_assert_ok!(mgate.write_obj(&refobj, 0));
    let obj: Test = wv_assert_ok!(mgate.read_obj(0));

    wv_assert_eq!(refobj, obj);
}

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

use m3::cap::Selector;
use m3::com::{recv_msg, RecvGate, SGateArgs, SendGate};
use m3::env;
use m3::math;
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, ActivityArgs, ChildActivity, RunningActivity, Tile};
use m3::time::TimeDuration;

use m3::{send_vmsg, wv_assert_eq, wv_assert_ok, wv_run_test};

use m3::com::channel;
use m3::activity;
use m3::errors::Error;

pub fn run(t: &mut dyn WvTester) {
/*    wv_run_test!(t, run_stop);
    wv_run_test!(t, run_arguments);
    wv_run_test!(t, run_send_receive); */
    wv_run_test!(t, run_send_receive_chan);
    wv_run_test!(t, run_send_receive_iso);
/*    #[cfg(not(target_vendor = "host"))]
    wv_run_test!(t, exec_fail);
    wv_run_test!(t, exec_hello);
    wv_run_test!(t, exec_rust_hello); */
}

fn run_stop(_t: &mut dyn WvTester) {
    use m3::com::RGateArgs;
    use m3::vfs;

    let mut rg = wv_assert_ok!(RecvGate::new_with(
        RGateArgs::default().order(6).msg_order(6)
    ));
    wv_assert_ok!(rg.activate());

    let tile = wv_assert_ok!(Tile::get("clone|own"));

    let mut wait_time = TimeDuration::from_nanos(10000);
    for _ in 1..100 {
        let mut act = wv_assert_ok!(ChildActivity::new_with(
            tile.clone(),
            ActivityArgs::new("test")
        ));

        // pass sendgate to child
        let sg = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rg).credits(1)));
        wv_assert_ok!(act.delegate_obj(sg.sel()));

        // pass root fs to child
        act.add_mount("/", "/");

        let mut dst = act.data_sink();
        dst.push(sg.sel());

        let act = wv_assert_ok!(act.run(|| {
            let mut src = Activity::own().data_source();
            let sg_sel: Selector = src.pop().unwrap();

            // notify parent that we're running
            let sg = SendGate::new_bind(sg_sel);
            wv_assert_ok!(send_vmsg!(&sg, RecvGate::def(), 1));
            let mut _n = 0;
            loop {
                _n += 1;
                // just to execute more interesting instructions than arithmetic or jumps
                vfs::VFS::stat("/").ok();
            }
        }));

        // wait for child
        wv_assert_ok!(recv_msg(&rg));

        // wait a bit and stop activity
        wv_assert_ok!(Activity::own().sleep_for(wait_time));
        wv_assert_ok!(act.stop());

        // increase by one ns to attempt interrupts at many points in the instruction stream
        wait_time += TimeDuration::from_nanos(1);
    }
}

fn run_arguments(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone|own"));
    let act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("test")));

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        wv_assert_eq!(t, env::args().count(), 1);
        assert!(env::args().next().is_some());
        assert!(env::args().next().unwrap().ends_with("rustunittests"));
        0
    }));

    wv_assert_eq!(t, act.wait(), Ok(0));
}

fn run_send_receive(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone|own"));
    let mut act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("test")));

    let rgate = wv_assert_ok!(RecvGate::new(math::next_log2(256), math::next_log2(256)));
    let sgate = wv_assert_ok!(SendGate::new_with(SGateArgs::new(&rgate).credits(1)));

    wv_assert_ok!(act.delegate_obj(rgate.sel()));

    let mut dst = act.data_sink();
    dst.push(rgate.sel());

    let act = wv_assert_ok!(act.run(|| {
        let mut t = DefaultWvTester::default();
        let mut src = Activity::own().data_source();
        let rg_sel: Selector = src.pop().unwrap();

        let mut rgate = RecvGate::new_bind(rg_sel, math::next_log2(256), math::next_log2(256));
        wv_assert_ok!(rgate.activate());
        let mut res = wv_assert_ok!(recv_msg(&rgate));
        let i1 = wv_assert_ok!(res.pop::<u32>());
        let i2 = wv_assert_ok!(res.pop::<u32>());
        wv_assert_eq!(t, (i1, i2), (42, 23));
        (i1 + i2) as i32
    }));

    wv_assert_ok!(send_vmsg!(&sgate, RecvGate::def(), 42, 23));

    wv_assert_eq!(t, act.wait(), Ok(42 + 23));
}

/// This test case uses solely the [`channel`] abstraction.
/// The ['channel'] abstraction is aligned as much as possible with Rust's [`mpsc::channel] API. It
/// essentially replaces the abstraction of gates.
/// (It also shows the expanded code of the [`activity`] macro in the next test case.)
fn run_send_receive_chan(t: &mut dyn WvTester) {
    let res =
        wv_assert_ok!(
            (|| -> Result<i32, Error> {
                let (tx, rx) = channel::channel()?;
                let (res_tx, mut res_rx) = channel::channel()?;
          
                let future =
                    {
                        use m3::tiles::iso;
                        use m3::tiles::iso::Capable;
            
                        let mut act = iso::ChildActivity::new()?;
                        act.delegate_cap(&rx)?;
                        act.delegate_cap(&res_tx)?;
                        
                        let mut sink = act.new_sink();
                        iso::sink(&mut sink, &rx);
                        iso::sink(&mut sink, &res_tx);
            
                        act.act.run(|| {
                            let f = || -> Result<(), Error> {
                                let mut source = iso::OwnActivity::new();
                                let rx0:channel::Receiver = source.activate()?;
                                let res_tx0:channel::Sender = source.activate()?;
            
                                let i1 = rx0.recv::<u32>()?;
                                let res = (i1 + 5) as i32;
                                res_tx0.send(res)?;
                                Ok(())
                            };
                            f().map(|_| 0).unwrap() // currently necessary because of the API
                        })
                    }?;
          
                tx.activate()?;
                tx.send::<u32>(42)?;
                res_rx.activate()?; // latest for activating result channel
                future.wait()?;
            
                let res :i32 = res_rx.recv()?;
                Ok(res)
            })()
        );
    wv_assert_eq!(t, res, 42 + 5);
}

/// This test case also uses the [`activity`] abstraction which takes care of
/// child activity creation, capability delegation, passing channels to the child activity and
/// reloading the channels on the child activity.
/// Note that the syntax is absolutely valid Rust code:
/// ```
/// (|rx0: channel::Receiver, tx0: channel::Sender|   // definition of the anonymous function
/// {                                                 
///   /* activity code goes here */                     
/// })
/// (rx, tx)                                         // calling the anonymous function
/// ```
/// The code resembles a call to a closure which essentially defines what is being executed on the
/// activity.
/// The activity return type is [`Result<T, Error>`] where [`T`] is a type of your choosing.
/// Note that currently M3 does not support transferring errors from an activity to the root
/// activity.
fn run_send_receive_iso(t: &mut dyn WvTester) {
    let res =
        wv_assert_ok!(
            (|| -> Result<i32, Error> {
                let (tx, rx) = channel::channel()?;
                let (res_tx, mut res_rx) = channel::channel()?;
            
                let future =
                    activity!(
                        |rx0: channel::Receiver, res_tx0: channel::Sender| {
                            let i1 = rx0.recv::<u32>()?;
                            let res = (i1 + 5) as i32;
                            res_tx0.send(res)?;
                            Ok(())
                        }(rx, res_tx)
                    )?;
            
                tx.activate()?;
                tx.send::<u32>(42)?;
                res_rx.activate()?; // latest for activating result channel
                future.wait()?;
            
                let res: i32 = res_rx.recv()?;
                Ok(res)
            })()
        );
    wv_assert_eq!(t, res, 42 + 5);
}

#[cfg(not(target_vendor = "host"))]
fn exec_fail(_t: &mut dyn WvTester) {
    use m3::errors::Code;

    let tile = wv_assert_ok!(Tile::get("clone|own"));
    // file too small
    {
        let act = wv_assert_ok!(ChildActivity::new_with(
            tile.clone(),
            ActivityArgs::new("test")
        ));
        let act = act.exec(&["/testfile.txt"]);
        assert!(act.is_err() && act.err().unwrap().code() == Code::EndOfFile);
    }

    // not an ELF file
    {
        let act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("test")));
        let act = act.exec(&["/pat.bin"]);
        assert!(act.is_err() && act.err().unwrap().code() == Code::InvalidElf);
    }
}

fn exec_hello(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone|own"));
    let act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("test")));

    let act = wv_assert_ok!(act.exec(&["/bin/hello"]));
    wv_assert_eq!(t, act.wait(), Ok(0));
}

fn exec_rust_hello(t: &mut dyn WvTester) {
    let tile = wv_assert_ok!(Tile::get("clone|own"));
    let act = wv_assert_ok!(ChildActivity::new_with(tile, ActivityArgs::new("test")));

    let act = wv_assert_ok!(act.exec(&["/bin/rusthello"]));
    wv_assert_eq!(t, act.wait(), Ok(0));
}

/*
 * Copyright (C) 2023 Nils Asmussen, Barkhausen Institut
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

use m3::col::ToString;
use m3::errors::Code;
use m3::kif::{Perm, TileDesc, TileISA, TileType};
use m3::test::{DefaultWvTester, WvTester};
use m3::tiles::{Activity, Tile};
use m3::{wv_assert_eq, wv_assert_err, wv_assert_ok, wv_assert_some, wv_run_test};

use resmng::childs::Child;
use resmng::resources::Resources;

use crate::helper::{run_subsys, setup_resmng, TestStarter};

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, basics);
}

fn basics(t: &mut dyn WvTester) {
    run_subsys(
        t,
        "<app args=\"resmngtest\">
             <dom>
                 <app args=\"/bin/rusthello\" usermem=\"16K\">
                    <serv name=\"test\" />
                    <tiles type=\"core\" count=\"1\"/>
                 </app>
             </dom>
         </app>",
        |subsys| {
            subsys.add_tile(wv_assert_ok!(Tile::get("clone")));
        },
        || {
            let mut t = DefaultWvTester::default();

            let (reqs, mut childs, child_sub, mut res) = setup_resmng();

            let cid = childs.next_id();
            let delayed =
                wv_assert_ok!(child_sub.start(&mut childs, &reqs, &mut res, &mut TestStarter {}));
            wv_assert_eq!(t, delayed.len(), 0);
            wv_assert_eq!(t, childs.children(), 1);

            let child = wv_assert_some!(childs.child_by_id_mut(cid));

            services(&mut t, child, &mut res);
            memories(&mut t, child, &mut res);
            tiles(&mut t, child, &mut res);

            let sel = child.activity_sel();
            childs.kill_child_async(&reqs, &mut res, sel, Code::Success);

            wv_assert_eq!(t, childs.children(), 0);

            Ok(())
        },
    );
}

fn services(t: &mut dyn WvTester, child: &mut dyn Child, res: &mut Resources) {
    wv_assert_err!(
        t,
        child.reg_service(res, 123, 124, "other".to_string(), 16),
        Code::InvArgs
    );
    wv_assert_ok!(child.reg_service(res, 123, 124, "test".to_string(), 16));
    wv_assert_eq!(t, child.res().services().len(), 1);

    wv_assert_err!(t, child.unreg_service(res, 124), Code::InvArgs);
    wv_assert_ok!(child.unreg_service(res, 123));
    wv_assert_eq!(t, child.res().services().len(), 0);
}

fn memories(t: &mut dyn WvTester, child: &mut dyn Child, _res: &mut Resources) {
    wv_assert_eq!(t, child.res().memories().len(), 0);
    wv_assert_eq!(t, child.mem().quota(), 16 * 1024);

    wv_assert_err!(t, child.alloc_mem(123, 0x1000000, Perm::RW), Code::NoSpace);
    wv_assert_ok!(child.alloc_mem(123, 4 * 1024, Perm::RW));

    wv_assert_eq!(t, child.res().memories().len(), 1);
    wv_assert_eq!(t, child.mem().quota(), 12 * 1024);

    wv_assert_err!(t, child.free_mem(124), Code::InvArgs);
    wv_assert_ok!(child.free_mem(123));

    wv_assert_eq!(t, child.res().memories().len(), 0);
    wv_assert_eq!(t, child.mem().quota(), 16 * 1024);
}

fn tiles(t: &mut dyn WvTester, child: &mut dyn Child, res: &mut Resources) {
    wv_assert_eq!(t, child.res().tiles().len(), 0);

    wv_assert_err!(
        t,
        child.alloc_tile(
            res,
            123,
            TileDesc::new(TileType::MEM, TileISA::NONE, 0),
            false
        ),
        Code::InvArgs
    );
    wv_assert_ok!(child.alloc_tile(res, 123, Activity::own().tile_desc(), false));

    wv_assert_eq!(t, child.res().tiles().len(), 1);

    wv_assert_err!(t, child.free_tile(res, 124), Code::InvArgs);
    wv_assert_ok!(child.free_tile(res, 123));

    wv_assert_eq!(t, child.res().tiles().len(), 0);
}

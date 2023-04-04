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

use m3::errors::Code;
use m3::kif::{TileAttr, TileDesc, TileISA, TileType};
use m3::rc::Rc;
use m3::tcu::TileId;
use m3::test::WvTester;
use m3::tiles::Tile;
use m3::{wv_assert_eq, wv_assert_err, wv_assert_ok, wv_run_test};

use resmng::resources::tiles::TileManager;

fn create_tiles() -> TileManager {
    let mut mng = TileManager::default();
    mng.add(Rc::new(Tile::new_bind(
        TileId::new(0, 1),
        TileDesc::new(TileType::COMP, TileISA::RISCV, 0),
        64,
    )));
    mng.add(Rc::new(Tile::new_bind(
        TileId::new(0, 2),
        TileDesc::new_with_attr(TileType::COMP, TileISA::ARM, 32 * 1024, TileAttr::IMEM),
        65,
    )));
    mng.add(Rc::new(Tile::new_bind(
        TileId::new(0, 3),
        TileDesc::new(TileType::MEM, TileISA::NONE, 1024 * 1024),
        66,
    )));
    mng
}

pub fn run(t: &mut dyn WvTester) {
    wv_run_test!(t, find);
    wv_run_test!(t, usage);
}

fn find(t: &mut dyn WvTester) {
    let mng = create_tiles();

    let riscv = wv_assert_ok!(mng.find(TileDesc::new(TileType::COMP, TileISA::RISCV, 0)));
    wv_assert_eq!(t, riscv.tile_id(), TileId::new(0, 1));
    wv_assert_eq!(t, riscv.tile_obj().sel(), 64);
    let arm = wv_assert_ok!(mng.find(TileDesc::new_with_attr(
        TileType::COMP,
        TileISA::ARM,
        0,
        TileAttr::IMEM
    )));
    wv_assert_eq!(t, arm.tile_id(), TileId::new(0, 2));
    wv_assert_eq!(t, arm.tile_obj().sel(), 65);
    wv_assert_err!(
        t,
        mng.find(TileDesc::new(TileType::COMP, TileISA::X86, 0)),
        Code::NotFound
    );

    let base = TileDesc::new(TileType::COMP, TileISA::RISCV, 0);
    let riscv = wv_assert_ok!(mng.find_with_attr(base, "boom|core"));
    wv_assert_eq!(t, riscv.tile_id(), TileId::new(0, 1));
    let arm = wv_assert_ok!(mng.find_with_attr(base, "arm+imem"));
    wv_assert_eq!(t, arm.tile_id(), TileId::new(0, 2));
}

fn usage(t: &mut dyn WvTester) {
    let mng = create_tiles();

    wv_assert_ok!(mng.find(TileDesc::new(TileType::COMP, TileISA::RISCV, 0)));
    let riscv = wv_assert_ok!(mng.find(TileDesc::new(TileType::COMP, TileISA::RISCV, 0)));
    mng.add_user(&riscv);

    wv_assert_err!(
        t,
        mng.find(TileDesc::new(TileType::COMP, TileISA::RISCV, 0)),
        Code::NotFound
    );

    mng.add_user(&riscv);
    mng.remove_user(&riscv);

    wv_assert_err!(
        t,
        mng.find(TileDesc::new(TileType::COMP, TileISA::RISCV, 0)),
        Code::NotFound
    );

    mng.remove_user(&riscv);

    wv_assert_ok!(mng.find(TileDesc::new(TileType::COMP, TileISA::RISCV, 0)));
}

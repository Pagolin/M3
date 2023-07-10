/*
 * Copyright (C) 2021-2022 Nils Asmussen, Barkhausen Institut
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

#![no_std]

#[allow(unused_extern_crates)]
extern crate heap;

#[path = "../../vmtest/src/helper.rs"]
mod helper;
#[path = "../../vmtest/src/paging.rs"]
mod paging;

use base::log;
use base::mem::MsgBuf;
use base::tcu::{EpId, TileId, FIRST_USER_EP, TCU};
use base::util::math;

const LOG_DEF: bool = true;
const LOG_DETAIL: bool = false;
const LOG_TMCALLS: bool = false;

const OWN_ACT: u16 = 0xFFFF;

const DST_TILE: TileId = TileId::new(0, 0);
const DST_EP: EpId = FIRST_USER_EP;

const REP: EpId = FIRST_USER_EP;
const SEP: EpId = FIRST_USER_EP + 1;

const MSG_SIZE: usize = 64;
const CREDITS: u32 = 4;
const SENDS: usize = 100000;

static RBUF: [u64; CREDITS as usize * MSG_SIZE] = [0; CREDITS as usize * MSG_SIZE];

#[no_mangle]
pub extern "C" fn env_run() {
    helper::init("stdasender");

    let msg_size = math::next_log2(MSG_SIZE);
    helper::config_local_ep(SEP, |regs| {
        TCU::config_send(regs, OWN_ACT, 0x1234, DST_TILE, DST_EP, msg_size, CREDITS);
    });

    let buf_ord = math::next_log2(RBUF.len());
    let msg_ord = math::next_log2(MSG_SIZE);
    let (rbuf_virt, rbuf_phys) = helper::virt_to_phys(RBUF.as_ptr() as usize);
    helper::config_local_ep(REP, |regs| {
        TCU::config_recv(regs, OWN_ACT, rbuf_phys, buf_ord, msg_ord, None);
    });

    let mut msg = MsgBuf::new();
    msg.set::<u64>(0);

    log!(crate::LOG_DEF, "Hello World from sender!");
    log!(crate::LOG_DEF, "Starting sends!");

    // initial send; wait until receiver is ready
    while let Err(e) = TCU::send(SEP, &msg, 0x2222, REP) {
        log!(crate::LOG_DEF, "send failed: {}", e);
        // get credits back
        helper::config_local_ep(SEP, |regs| {
            TCU::config_send(regs, OWN_ACT, 0x1234, DST_TILE, DST_EP, 6, CREDITS);
        });
    }

    let mut sent = 1;
    let mut recv = 0;
    while recv < SENDS {
        // received reply?
        while let Some(m) = helper::fetch_msg(REP, rbuf_virt) {
            assert_eq!(m.header.label(), 0x2222);
            log!(crate::LOG_DETAIL, "got reply {}", m.as_words()[0]);

            // ack reply
            TCU::ack_msg(REP, TCU::msg_to_offset(rbuf_virt, m)).unwrap();
            recv += 1;
        }

        if sent < SENDS && TCU::credits(SEP).unwrap() > 0 {
            // send message
            TCU::send(SEP, &msg, 0x2222, REP).unwrap();
            msg.set(msg.get::<u64>() + 1);
            sent += 1;
        }
    }

    // wait for ever
    loop {}
}

/*
 * Copyright (C) 2022 Lisza Zeidler <lisza.zeidle@tu-dresden.de>
 * Economic rights: Barkhausen Institut
 *
 * This file is part of M3 (Microkernel-based SysteM for Heterogeneous Manycores
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

// I mightneed this later for converting &str to c_char* without using std
//#![feature(slice_internals)]


#![no_std]
#![allow(dead_code)]
#![allow(unused_imports)]
mod loop_lib;
mod driver;

// We need this to keep types opaque when interfacing with leveldb
// in the store but it must be defined in the crate root (here).
#[macro_use]
extern crate ffi_opaque;
// extern crate libc;

use crate::driver::*;
use loop_lib::init_components::{init_device, init_ip_stack, init_app, App, DeviceType};

// The rust API for m3 basically defines one logging macro we can use here 
// replacing debug!, info! and error!
// However they are trivially defined in M3/src/libs/rust/base/src/io/mod.rs so 
// I could probably augment them with the appropriate other definitions 
use m3::{log, vec, println};
use m3::col::{BTreeMap, Vec};
use m3::tiles::OwnActivity;
use m3::com::Semaphore;


use local_smoltcp::iface::{Interface, NeighborCache, SocketSet, InterfaceCall};
use local_smoltcp::phy::{Device, DeviceCapabilities};
use local_smoltcp::time::{Duration, Instant};
use local_smoltcp::{Either, Result};

// m3's log takes a log-level-bool parameter
const DEBUG: bool = true;


fn maybe_wait(call: Either<InterfaceCall, (Option<Duration>, bool)>) -> InterfaceCall {
    // We enter this method in every interaction between te device and the ip_stack
    // Only in case of waiting call (Either::Right) from the device, we should wait
    return if Either::is_left(&call) {
        // No waiting -> Call the Interface directly
        call.left_or_panic()
    } else {
        //phy_wait(fd, iface.poll_delay(timestamp, &sockets)).expect("wait error");
        /* Original waiting logic of phy_wait ... :
            1. If the device (file pointer) is ready/available
                -> poll
            2. If poll_delay return Some(advisory wait)
                -> wait until the device is ready BUT at most for the advisory waiting time
            3. If poll_delay returns None
                ->   wait until the device becomes available again
           Logic in the smoltcp_server/net loop:
            1. If device.needs_poll() -> continue polling
            2. If advised waiting time is Some(0) -> continue polling
            3. If advised waiting time is Some(t) -> wait for t
            4. If advised waiting time is None -> wait TimeDuration::MAX
           -> There seems to be additional event logic preventing the m3 loop from
               literally waiting for years so for the case where device does not need
               polling and there is no advised waiting time we need to pick a
               reasonable time to wait -> we take the one from the smoltcp loopback loop
        */
        let (advised_waiting_timeout, device_needs_poll) = call.right_or_panic();
        if !device_needs_poll {
            match advised_waiting_timeout {
                Some(t) => OwnActivity::sleep_for(t.as_m3_duration()).ok(),
                None => OwnActivity::sleep_for(m3::time::TimeDuration::from_millis(1)).ok(),
            };
        }
        InterfaceCall::InitPoll
    }
}


#[no_mangle]
pub fn main() {
    log!(DEBUG,
r#"
      ___           ___           ___           ___                    ___           ___
     /\  \         /\__\         /\__\         /\  \                  /\__\         /\__\
    /::\  \       /:/  /        /:/  /        /::\  \                /:/  /        /:/  /
   /:/\:\  \     /:/__/        /:/  /        /:/\:\  \              /:/__/        /:/  /
  /:/  \:\  \   /::\  \ ___   /:/  /  ___   /::\_\:\  \            /::\__\____   /:/__/  ___
 /:/__/ \:\__\ /:/\:\  /\__\ /:/__/  /\__\ /:/\:\ \:\__\          /:/\:::::\__\  |:|  | /\__\
 \:\  \ /:/  / \/__\:\/:/  / \:\  \ /:/  / \/__\:\/:/  /          \/_|:|~~|~     |:|  |/:/  /
  \:\  /:/  /       \::/  /   \:\  /:/  /       \::/  /              |:|  |      |:|__/:/  /
   \:\/:/  /        /:/  /     \:\/:/  /        /:/  /               |:|  |       \::::/__/
    \::/  /        /:/  /       \::/  /        /:/  /                |:|  |
     \/__/         \/__/         \/__/         \/__/                  \|__|
"#
    );
    log!(DEBUG, "Running smoltcp_server");

    // We need t omount the file system to be able to create a DB
    m3::vfs::VFS::mount("/", "m3fs", "m3fs").expect("Failed to mount root filesystem on smoltcp_server");

    let mut app = init_app();

    let (mut device, caps):(DeviceType, DeviceCapabilities) = init_device();

    let mut ip_stack:Interface<'_> = init_ip_stack(caps);
    let mut ip_stack_call = InterfaceCall::InitPoll;

    // To ensure the client sends requests only after the smoltcp_server started we need a semaphore from m3
    let mut semaphore_set = false;


    loop {
        // ToDo: Where to put the semaphore handling?
        if !semaphore_set {
            // The client is attached to the same semaphore and will only try to send
            // once the smoltcp_server listens
            let _sem_r = Semaphore::attach("net").unwrap().up();
            semaphore_set = true;
        }
        let device_or_app_call = ip_stack.process_call::<DeviceType>(ip_stack_call);
        if Either::is_left(&device_or_app_call){
            let call = device.process_call(device_or_app_call.left_or_panic());
            ip_stack_call = maybe_wait(call)
        } else {
            let (readiness_has_changed, message) = device_or_app_call.right_or_panic();
            let answers = app.process_message(Ok(readiness_has_changed), message);
            ip_stack_call = InterfaceCall::AnswerToSocket(answers);
        }
    }
}

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

// for offset_of with unstable_const feature
#![feature(const_ptr_offset_from)]
#![no_std]

mod loop_lib;
mod driver;

use crate::driver::*;
use loop_lib::store::{Store};

use core::str;
// The rust API for m3 basically defines one logging macro we can use here 
// replacing debug!, info! and error!
// However they are trivially defined in M3/src/libs/rust/base/src/io/mod.rs so 
// I could probably augment them with the appropriate other definitions 
use m3::{log, vec, println};
use m3::col::{BTreeMap, Vec};
use m3::tiles::Activity;
use m3::com::Semaphore;


use local_smoltcp::iface::{FragmentsCache, InterfaceBuilder, NeighborCache, SocketSet};
use local_smoltcp::phy::{Device, Medium};
use local_smoltcp::socket::{tcp};
use local_smoltcp::time::{Duration, Instant};
use local_smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};

// m3's log takes a log-level-bool parameter
const DEBUG: bool = true;

fn process_octets(octets: &mut [u8]) -> (usize, Vec<u8>) {
    let recvd_len = octets.len();
    let mut data = vec![];
    data.extend_from_slice(octets);
    (recvd_len, data)
}


#[no_mangle]
pub fn main() -> i32{
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
    log!(DEBUG, "Running smoltcp server");
    let mut store = Store::default();


    let tcp_rx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
    let tcp_tx_buffer = tcp::SocketBuffer::new(vec![0; 1024]);
    let tcp_socket = tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer);

    #[cfg(target_vendor = "gem5")]
    let mut device = E1000Device::new().unwrap();
    #[cfg(target_vendor = "hw")]
    let mut device = AXIEthDevice::new().unwrap();
    #[cfg(target_vendor = "host")]
    // The name parameter is used to identify the socket and is usually ser
    // via the app config e.g. in boot/rust-net-tests.xml
    let mut device = DevFifo::new("kvsocket");


    let neighbor_cache = NeighborCache::new(BTreeMap::new());
    let ethernet_addr = EthernetAddress([0x02, 0x00, 0x00, 0x00, 0x00, 0x01]);
    let ip_addrs = [
        IpCidr::new(IpAddress::v4(192, 168, 69, 2), 24)
    ];

    let medium = device.capabilities().medium;
    let mut builder = InterfaceBuilder::new(vec![]).ip_addrs(ip_addrs);


    let mut out_packet_buffer = [0u8; 1024];

    if medium == Medium::Ethernet {
        builder = builder.hardware_addr(ethernet_addr.into()).neighbor_cache(neighbor_cache);
    }
    let mut iface = builder.finalize(&mut device);

    let mut sockets = SocketSet::new(vec![]);
    let tcp_handle = sockets.add(tcp_socket);
    // As long as I haven't figured out a sensible way of ending the loop
    // I'll just count iterations
    let mut interim_break = 0;
    let mut semaphore_set = false;

    while interim_break < 10 {
        let timestamp = Instant::now();
        match iface.poll(timestamp, &mut device, & mut sockets) {
            Ok(_) => {}
            Err(e) => {
                log!(DEBUG, "poll error: {}", e);
            }
        }

        let socket = sockets.get_mut::<tcp::Socket>(tcp_handle);
        if !socket.is_open() {
            socket.listen(6969).unwrap();
        }

        if !semaphore_set {
            // The client is attached to the same semaphore and will only try to send once
            // once the server listens
            Semaphore::attach("net").unwrap().up();
        }


        if socket.may_recv() {
            let input = socket.recv(process_octets).unwrap();
            println!("Server: Packet got through");
            if socket.can_send() && !input.is_empty() {
                println!(
                    "Server Input: {:?}",
                    str::from_utf8(input.as_ref()).unwrap_or("(invalid utf8)")
                );
                let outbytes = store.handle_message(&input);
                println!("Server: got outbytes {:?}", str::from_utf8(&outbytes));
                socket.send_slice(&outbytes[..]).unwrap();

            } else if input.is_empty() {
                let outbytes = store.handle_message(b"OK");
                socket.send_slice(&outbytes[..]).unwrap();
                println!("Server: Input was empty, but sent OK");
            } else {
                println!("Server: Socket couldn't send");
            }
        } else if socket.may_send() {
            log!(DEBUG, "tcp:6969 close");
            socket.close();
        }
        //phy_wait(fd, iface.poll_delay(timestamp, &sockets)).expect("wait error");
        /* Original waiting logic of phy_wait ... :
            1. If the device (file pointer) is ready/available
                -> poll
            2. If poll_delay return Some(advisory wait)
                -> wait until the device is ready BUT at most for the advisory waiting time
            3. If poll_delay returns None
                ->   wait until the device becomes available again
           Logic in the server/net loop:
            1. If device.needs_poll() -> continue polling
            2. If advised waiting time is Some(0) -> continue polling
            3. If advised waiting time is Some(t) -> wait for t
            4. If advised waiting time is None -> wait TimeDuration::MAX
           -> There seems to be additional event logic preventing the m3 loop from
               literally waiting for years so for the case where device does not need
               polling and there is no advised waiting time we need to pick a
               reasonable time to wait -> we take the one from the smoltcp loopback loop
        */
        if device.needs_poll() {
            println!("Server: Device needs poll");
            continue
        } else {
            let advised_waiting_timeout = iface.poll_delay(timestamp, &sockets);
            println!("Server: Gonna wait a bit");
            match advised_waiting_timeout {
                None => Activity::own().sleep_for(m3::time::TimeDuration::from_millis(1)).ok(),
                Some(t) => Activity::own().sleep_for(t.as_m3_duration()).ok(),
            }
        };
        interim_break += 1;
    }
    println!("Server: Going to stop now");
    0
}

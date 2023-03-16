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

use driver::*;
use loop_lib::store::{Store};

use core::str;
// The rust API for m3 basically defines one logging macro we can use here 
// replacing debug!, info! and error!
// However they are trivially defined in M3/src/libs/rust/base/src/io/mod.rs so 
// I could probably augment them with the appropriate other definitions 
use m3::{log, vec};
use m3::col::{BTreeMap, Vec};


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
        IpCidr::new(IpAddress::v4(192, 168, 69, 1), 24)
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

    while interim_break < 100 {
        let timestamp = Instant::now();
        match iface.poll(timestamp, &mut device, & mut sockets) {
            Ok(_) => {}
            Err(e) => {
                log!(DEBUG, "poll error: {}", e);
            }
        }

        let socket = sockets.get_mut::<tcp::Socket>(tcp_handle);
        //log!(DEBUG, "State {:?}", socket.state());
        if !socket.is_open() {
            socket.listen(6969).unwrap();
        }

        if socket.may_recv() {
            let input = socket.recv(process_octets).unwrap();
            if socket.can_send() && !input.is_empty() {
                log!(DEBUG,
                    "Input {:?}",
                    str::from_utf8(input.as_ref()).unwrap_or("(invalid utf8)")
                );
                let outbytes = store.handle_message(&input);
                //log!(DEBUG, "Got outbytes {:?}", str::from_utf8(&outbytes));
                socket.send_slice(&outbytes[..]).unwrap();

            }
        } else if socket.may_send() {
            log!(DEBUG, "tcp:6969 close");
            socket.close();
        }
        //phy_wait(fd, iface.poll_delay(timestamp, &sockets)).expect("wait error");
        interim_break += 1;
    }
    0
}

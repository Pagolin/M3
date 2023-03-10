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

#![no_std]

mod loop_lib;
use loop_lib::store::{Store};

use core::str;
// The rust API for m3 basically defines one logging macro we can use here 
// replacing debug!, info! and error!
// However they are trivially defined in M3/src/libs/rust/base/src/io/mod.rs so 
// I could probably augment them with the appropriate other definitions 
use m3::{log, vec};
use m3::col::{BTreeMap, Vec};


use local_smoltcp::iface::{FragmentsCache, InterfaceBuilder, NeighborCache, SocketSet};
use local_smoltcp::phy::{Loopback, Medium};
use local_smoltcp::socket::{tcp};
use local_smoltcp::time::{Duration, Instant};
use local_smoltcp::wire::{EthernetAddress, IpAddress, IpCidr};

// m3's log takes a log-level-bool parameter
const DEBUG: bool = true;

fn process_octets(octets:&mut [u8]) -> (usize, Vec<u8>) {
    let recvd_len = octets.len();
    let mut data = vec![];
    data.extend_from_slice(octets);
    (recvd_len, data)
}

mod mock {
    use core::cell::Cell;
    use local_smoltcp::time::{Duration, Instant};

    #[derive(Debug)]
    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct Clock(Cell<Instant>);

    impl Clock {
        pub fn new() -> Clock {
            Clock(Cell::new(Instant::from_millis(0)))
        }

        pub fn advance(&self, duration: Duration) {
            self.0.set(self.0.get() + duration)
        }

        pub fn elapsed(&self) -> Instant {
            self.0.get()
        }
    }
}


#[no_mangle]
fn main() {
    let clock = mock::Clock::new();
    let mut store = Store::default();

    let mut device = Loopback::new(Medium::Ethernet);

    // we leave out the device setup with logging and timing features cause there's to much std
    // involved which we'd need to replace

    let mut neighbor_cache_entries = [None; 8];
    let mut neighbor_cache = NeighborCache::new(&mut neighbor_cache_entries[..]);

    let mut ip_addrs = [IpCidr::new(IpAddress::v4(127, 0, 0, 1), 8)];
    let mut iface = InterfaceBuilder::new(vec![])
        .hardware_addr(EthernetAddress::default().into())
        .neighbor_cache(neighbor_cache)
        .ip_addrs(ip_addrs)
        .finalize(&mut device);

    let server_socket = {
        // It is not strictly necessary to use a `static mut` and unsafe code here, but
        // on embedded systems that smoltcp targets it is far better to allocate the data
        // statically to verify that it fits into RAM rather than get undefined behavior
        // when stack overflows.
        static mut TCP_SERVER_RX_DATA: [u8; 1024] = [0; 1024];
        static mut TCP_SERVER_TX_DATA: [u8; 1024] = [0; 1024];
        let tcp_rx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_SERVER_RX_DATA[..] });
        let tcp_tx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_SERVER_TX_DATA[..] });
        tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
    };

    let client_socket = {
        static mut TCP_CLIENT_RX_DATA: [u8; 1024] = [0; 1024];
        static mut TCP_CLIENT_TX_DATA: [u8; 1024] = [0; 1024];
        let tcp_rx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_CLIENT_RX_DATA[..] });
        let tcp_tx_buffer = tcp::SocketBuffer::new(unsafe { &mut TCP_CLIENT_TX_DATA[..] });
        tcp::Socket::new(tcp_rx_buffer, tcp_tx_buffer)
    };

    let mut sockets: [_; 2] = Default::default();
    let mut sockets = SocketSet::new(&mut sockets[..]);
    let server_handle = sockets.add(server_socket);
    let client_handle = sockets.add(client_socket);

    let mut did_listen = false;
    let mut did_connect = false;
    let mut done = false;

    while !done && clock.elapsed() < Instant::from_millis(10_000) {
        match iface.poll(clock.elapsed(), &mut device, &mut sockets) {
            Ok(_) => {}
            Err(e) => {
                log!(DEBUG,"poll error");
            }
        }

        let mut socket = sockets.get_mut::<tcp::Socket>(server_handle);
        if !socket.is_active() && !socket.is_listening() {
            if !did_listen {
                log!(DEBUG, "server listening");
                socket.listen(1234).unwrap();
                did_listen = true;
            }
        }

        if socket.can_recv() {
            let input = socket.recv(process_octets).unwrap();
            log!(DEBUG,
                "got server input {:?}",
                input
            );
            if socket.can_send() {
                let outbytes = store.handle_message(&input);
                socket.send_slice(&outbytes[..]).unwrap();
            }
            socket.close();
            done = true;
        }

        let mut socket = sockets.get_mut::<tcp::Socket>(client_handle);
        let cx = iface.context();
        if !socket.is_open() {
            if !did_connect {
                log!(DEBUG, "connecting");
                socket
                    .connect(cx, (IpAddress::v4(127, 0, 0, 1), 1234), 65000)
                    .unwrap();
                did_connect = true;
            }
        }

        if socket.can_send() {
            log!(DEBUG, "sending");
            socket.send_slice(b"Client requests Nonsense").unwrap();
            socket.close();
        }

        match iface.poll_delay(clock.elapsed(), &sockets) {
            Some(Duration::ZERO) => log!(DEBUG, "resuming"),
            Some(delay) => {
                log!(DEBUG, "sleeping for {} ms", delay);
                clock.advance(delay)
            }
            None => clock.advance(Duration::from_millis(1)),
        }
    }

    if done {
        log!(DEBUG, "done")
    } else {
        log!(DEBUG, "this is taking too long, bailing out")
    }
}

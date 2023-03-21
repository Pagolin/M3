/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
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

#[macro_use]
extern crate m3;

use m3::{
    col::Vec,
    com::Semaphore,
    env,
    net::{
        Endpoint, IpAddr, Port, StreamSocket, StreamSocketArgs,
        TcpSocket, UdpSocket,
    },
    println,
    rc::Rc,
    session::NetworkManager,
    vfs::{BufReader, OpenFlags},
};


const VERBOSE: bool = true;

fn usage() {
    let name = env::args().next().unwrap();
    println!("Usage: {} tcp <ip> <port> ", name);
    println!("Usage: {} udp <port>", name);
    m3::exit(1);
}


fn tcp_client(nm: Rc<NetworkManager>, ip: IpAddr, port: Port) {

    let default_request = b"{\"Insert\":{\"key\":\"somekey\", \"value\":\"somevalue\"}}";
    // Connect to server
    let mut socket = TcpSocket::new(
        StreamSocketArgs::new(nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024),
    )
    .expect("Could not create TCP socket");

    // Wait for server to listen
    let mut sem = Semaphore::attach("net").unwrap().down().unwrap();

    socket
        .connect(Endpoint::new(ip, port))
        .unwrap_or_else(|_| panic!("{}", format!("Unable to connect to {}:{}", ip, port)));

    if VERBOSE {
        println!("Sending operation...");
    }

    socket.send(default_request).expect("send failed");

    if VERBOSE {
        println!("Receiving response...");
    }

    let mut response = vec![0u8; 2];
    socket
        .recv(&mut response)
        .expect("receive response failed");
    if VERBOSE {
        println!("Got response.");
    }
}

#[no_mangle]
pub fn main() -> i32 {
    let args: Vec<_> = env::args().collect();
    if args.len() < 2 {
        println!("To few args");
        usage();
    }

    let nm = NetworkManager::new("net").expect("Could not connect to network manager");


    let ip = args[2]
        .parse::<IpAddr>()
        .expect("Failed to parse IP address");
    let port = args[3].parse::<Port>().expect("Failed to parse port");
    tcp_client(nm, ip, port);
    return 0
}

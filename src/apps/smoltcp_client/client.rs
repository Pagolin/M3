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
    col::{String, Vec},
    com::Semaphore,
    env,
    errors::Error,
    net::{Endpoint, IpAddr, Port, Socket, StreamSocketArgs, TcpSocket},
    println,
    rc::Rc,
    session::NetworkManager,
    time::{Duration, TimeInstant},
    tmif::exit,
    vfs::{BufReader, OpenFlags},
};

use core::str;
mod importer;

//ToDO: If it's not broken, its highly breakable. COme up with a better way to aggregate
struct Stats {
    fails: Vec<u64>,
    inserts: Vec<u64>,
    delets: Vec<u64>,
    reads: Vec<u64>,
    scans: Vec<u64>,
    updates: Vec<u64>,
    times: Vec<u64>,
}

impl Stats {
    fn new() -> Self {
        Stats {
            fails: vec![],
            inserts: vec![],
            delets: vec![],
            reads: vec![],
            scans: vec![],
            updates: vec![],
            times: vec![],
        }
    }

    fn update(&mut self, values: &[u64], time: u64) {
        self.fails.push(values[0]);
        self.inserts.push(values[1]);
        self.delets.push(values[2]);
        self.reads.push(values[3]);
        self.scans.push(values[4]);
        self.updates.push(values[5]);
        self.times.push(time);
    }

    // Not sure if I can derive in m3
    fn print(&self) {
        println!(
            "Final Statistic:\n \
            Failures: {:?}\n \
            Insert Success: {:?}\n \
            Delete Success: {:?}\n \
            Read Success: {:?}\n \
            Scan Success: {:?}\n \
            Update Success: {:?}\n \
            Total Time: {:?}",
            self.fails,
            self.inserts,
            self.delets,
            self.reads,
            self.scans,
            self.updates,
            self.times
                .iter()
                .map(|i| TimeInstant::from_nanos(*i))
                .collect::<Vec<TimeInstant>>()
        );
    }
}

const VERBOSE: bool = false;
const ERROR_MSG: [u8; 5] = *b"ERROR";

fn usage() {
    let name = env::args().next().unwrap();
    println!("Usage: {} tcp <ip> <port> <workload> <repeats>", name);
    exit(1.into());
}

fn tcp_client(nm: Rc<NetworkManager>, ip: IpAddr, port: Port, wl: &str, repeats: u32) {
    // Mount fs to load binary data
    m3::vfs::VFS::mount("/", "m3fs", "m3fs")
        .expect("Failed to mount root filesystem on smoltcp_server");

    if VERBOSE {
        println!("Client: Started");
    }

    // Connect to smoltcp_server
    let mut socket = TcpSocket::new(
        StreamSocketArgs::new(nm)
            .send_buffer(64 * 1024)
            .recv_buffer(256 * 1024),
    )
    .expect("Could not create TCP socket");
    if VERBOSE {
        println!("Client: Got socket");
    }

    // Wait for smoltcp_server to listen
    Semaphore::attach("net").unwrap().down().unwrap();

    socket
        .connect(Endpoint::new(ip, port))
        .unwrap_or_else(|_| panic!("{}", format!("Unable to connect to {}:{}", ip, port)));

    let mut total_stats = Stats::new();

    for repeat in 0..repeats {
        // Load workload info for the benchmark
        let workload = m3::vfs::VFS::open(wl, OpenFlags::R).expect("Could not open file");
        let mut workload_buffer = BufReader::new(workload);
        let workload_header = importer::WorkloadHeader::load_from_file(&mut workload_buffer);
        let mut run_results = [0u64; 6];

        // Measure how long it takes to process all requests in the workload (except in case of error)
        let start_time = TimeInstant::now();

        for i in 0..workload_header.number_of_operations {
            let db_request = importer::Package::load_as_bytes(&mut workload_buffer);
            debug_assert!(importer::Package::from_bytes(&db_request).is_ok());
            if VERBOSE {
                println!("Sending operation...");
                println!("Operation has {} bytes", db_request.len());
            }
            // first byte encodes db operation. we need it for counting (successful) operations.
            let operation = db_request[0] as usize;
            // Send length info for next request
            socket
                .send(&db_request.len().to_be_bytes())
                .expect("send failed");
            // Send next request
            socket.send(&db_request).expect("send failed");

            if VERBOSE {
                println!("Receiving response...");
            }

            let mut resp_bytes = [0u8; 8];
            socket
                .recv(&mut resp_bytes)
                .expect("receive response header failed");
            let resp_len = u64::from_be_bytes(resp_bytes);

            if VERBOSE {
                println!("Expecting {} byte response.", resp_len);
            }

            let mut response = vec![0u8; resp_len as usize];
            let mut rem = resp_len as usize;
            while rem > 0 {
                let amount = socket
                    .recv(&mut response[resp_len as usize - rem..])
                    .expect("receive response failed");
                rem -= amount;
            }

            if VERBOSE {
                println!("Client: Got  {:?} response bytes", response.len());
            }

            if response == ERROR_MSG {
                // can't use log! with format strings
                println!("Client received a length encoding error and will end now. Operations remaining:{:?}", i);
                break;
            }

            match response.len() {
                0 => run_results[0] += 1,
                // operations start at 1 indexes at 0
                _ => run_results[operation] += 1,
            }
        }
        let time_taken = TimeInstant::now() - start_time;
        println!("Done repeat {:?}", repeat);
        // as_raw gives value as nanoseconds as u64 for Duration
        total_stats.update(&run_results, time_taken.as_raw());
    }

    total_stats.print();
    if VERBOSE {
        println!("Client: Will send end Message");
    }
    let end_msg = b"ENDNOW";
    socket.send(&end_msg.len().to_be_bytes()).unwrap();
    socket.send(end_msg).unwrap();
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let args: Vec<_> = env::args().collect();

    let nm = NetworkManager::new("net").expect("Could not connect to network manager");
    if args.len() != 6 {
        usage();
    }

    let ip = args[2]
        .parse::<IpAddr>()
        .expect("Failed to parse IP address");
    let port = args[3].parse::<Port>().expect("Failed to parse port");
    let repeats = args[5].parse::<u32>().expect("Failed to parse repeats");
    tcp_client(nm, ip, port, args[4], repeats);
    Ok(())
}

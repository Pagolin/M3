/*
 * Copyright (C) 2021 Nils Asmussen, Barkhausen Institut
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
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

use crate::errors::{Code, Error};
use crate::llog;
use crate::net::dataqueue::DataQueue;
use crate::net::{
    event, log_net, Endpoint, IpAddr, NetEvent, NetEventChannel, NetEventType, NetLogEvent, Port,
    Sd, SocketType, MTU,
};
use crate::rc::Rc;
use crate::vfs::{File, FileEvent};

mod dgram;
mod raw;
mod stream;
mod tcp;
mod udp;

pub use self::dgram::DGramSocket;
pub use self::raw::{RawSocket, RawSocketArgs};
pub use self::stream::StreamSocket;
pub use self::tcp::{StreamSocketArgs, TcpSocket};
pub use self::udp::{DgramSocketArgs, UdpSocket};

const EVENT_FETCH_BATCH_SIZE: u32 = 4;

pub struct SocketArgs {
    pub rbuf_slots: usize,
    pub rbuf_size: usize,
    pub sbuf_slots: usize,
    pub sbuf_size: usize,
}

impl Default for SocketArgs {
    fn default() -> Self {
        Self {
            rbuf_slots: 4,
            rbuf_size: 16 * 1024,
            sbuf_slots: 4,
            sbuf_size: 16 * 1024,
        }
    }
}

/// The states sockets can be in
#[derive(Eq, Debug, PartialEq, Clone, Copy)]
pub enum State {
    /// The socket is bound to a local address and port
    Bound,
    /// The socket is listening on a local address and port for remote connections
    Listening,
    /// The socket is currently connecting to a remote endpoint
    Connecting,
    /// The socket is connected to a remote endpoint
    Connected,
    /// The remote side has closed the connection
    RemoteClosed,
    /// The socket is currently being closed, initiated by our side
    Closing,
    /// The socket is closed (default state)
    Closed,
}

/// Trait for all sockets.
pub trait Socket: File {
    /// Returns the current state of the socket
    fn state(&self) -> State;

    /// Returns the local endpoint
    ///
    /// The local endpoint is only `Some` if the socket has been put into listen mode via
    /// [`listen`](StreamSocket::listen), has been bound via [`bind`](DGramSocket::bind), or was
    /// connected to a remote endpoint via [`connect`](Socket::connect).
    fn local_endpoint(&self) -> Option<Endpoint>;

    /// Returns the remote endpoint
    ///
    /// The remote endpoint is only `Some`, if the socket is currently connected (achieved either
    /// via [`connect`](Socket::connect) or [`accept`](StreamSocket::accept)). Otherwise, the remote
    /// endpoint is `None`.
    fn remote_endpoint(&self) -> Option<Endpoint>;

    /// Connects this socket to the given remote endpoint.
    ///
    /// For datagram sockets, this merely sets the endpoint to use for subsequent send calls and
    /// therefore does not involve the remote side in any way. If the socket has not been bound so
    /// far, bind(0) will be called to bind it to an unused ephemeral port.
    ///
    /// For stream sockets, a connection will be established to the given remote endpoint, involving
    /// the remote side.
    fn connect(&mut self, ep: Endpoint) -> Result<(), Error>;

    /// Returns whether data can currently be received from the socket
    ///
    /// Note that this function does not process events. To receive data, any receive function on
    /// this socket or [`FileWaiter::wait`](crate::vfs::FileWaiter::wait) has to be called.
    fn has_data(&self) -> bool;

    /// Receives data from the socket into the given buffer.
    ///
    /// Stream sockets need to be connected first (either via [`connect`](Socket::connect) or
    /// [`accept`](StreamSocket::accept)). Note that data can be received after the remote side has
    /// closed the socket (state [`RemoteClosed`](State::RemoteClosed)), but not if this side has
    /// been closed.
    ///
    /// Returns the number of received bytes.
    fn recv(&mut self, data: &mut [u8]) -> Result<usize, Error>;

    /// Sends the given data to this socket
    ///
    /// Datagram sockets send to the remote endpoint set at [`connect`](Socket::connect).
    ///
    /// Stream sockets need to be connected first (either via [`connect`](Socket::connect) or
    /// [`accept`](StreamSocket::accept)). Note that data can be received after the remote side has
    /// closed the socket (state [`RemoteClosed`](State::RemoteClosed)), but not if this side has
    /// been closed.
    ///
    /// Returns the number of sent bytes or an error. If an error occurs (e.g., remote side closed
    /// the socket) and some of the data has already been sent, the number of sent bytes is
    /// returned. Otherwise, the error is returned.
    fn send(&mut self, data: &[u8]) -> Result<usize, Error>;
}

/// Socket prototype that is shared between sockets.
pub(crate) struct BaseSocket {
    sd: Sd,
    ty: SocketType,
    blocking: bool,

    state: State,

    local_ep: Option<Endpoint>,
    remote_ep: Option<Endpoint>,

    channel: Rc<NetEventChannel>,
    recv_queue: DataQueue,
}

impl BaseSocket {
    pub fn new(sd: Sd, ty: SocketType, channel: Rc<NetEventChannel>) -> Self {
        Self {
            sd,
            ty,

            state: State::Closed,
            blocking: true,

            local_ep: None,
            remote_ep: None,

            channel,
            recv_queue: DataQueue::default(),
        }
    }

    pub fn sd(&self) -> Sd {
        self.sd
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn blocking(&self) -> bool {
        self.blocking
    }

    pub fn set_blocking(&mut self, blocking: bool) {
        self.blocking = blocking;
    }

    pub fn disconnect(&mut self) {
        self.local_ep = None;
        self.remote_ep = None;
        self.state = State::Closed;
    }

    pub fn has_data(&self) -> bool {
        self.recv_queue.has_data()
    }

    pub fn has_all_credits(&self) -> bool {
        self.channel.has_all_credits()
    }

    pub fn next_data<F, R>(&mut self, amount: usize, mut consume: F) -> Result<R, Error>
    where
        F: FnMut(&[u8], Endpoint) -> (usize, R),
    {
        loop {
            if let Some((sent, res)) = self.recv_queue.next_data(amount, &mut consume) {
                log_net(NetLogEvent::FetchData, self.sd, sent);
                return Ok(res);
            }

            if !self.blocking {
                self.process_events();
                return Err(Error::new(Code::WouldBlock));
            }

            self.wait_for_events(true)?;
        }
    }

    pub fn send(&self, data: &[u8], endpoint: Endpoint) -> Result<(), Error> {
        if data.len() > MTU {
            return Err(Error::new(Code::OutOfBounds));
        }

        let msg = self
            .channel
            .build_data_message(endpoint, data.len(), |buf| {
                buf.copy_from_slice(data);
            });

        loop {
            let res = self.channel.send_data(&msg);
            match res {
                Err(e) if e.code() != Code::NoCredits => break Err(e),
                Ok(_) => {
                    log_net(NetLogEvent::SentPacket, self.sd, msg.size as usize);
                    break Ok(());
                },
                _ => {},
            }

            if !self.blocking {
                self.fetch_replies();
                return Err(Error::new(Code::WouldBlock));
            }

            self.wait_for_credits();

            if self.state == State::Closed {
                return Err(Error::new(Code::SocketClosed));
            }
        }
    }

    pub fn has_events(&mut self, events: FileEvent) -> bool {
        self.fetch_replies();

        (events.contains(FileEvent::INPUT) && (self.process_events() || self.has_data()))
            || (events.contains(FileEvent::OUTPUT) && self.can_send())
    }

    pub fn tear_down(&self) {
        // make sure that all packets we sent are seen and handled by the server. thus, wait until
        // we have got all replies to our potentially in-flight packets, in which case we also have
        // received our credits back.
        loop {
            self.fetch_replies();
            if self.has_all_credits() {
                break;
            }

            log_net(NetLogEvent::StartedWaiting, self.sd, 0);
            self.channel.wait_for_credits();
            log_net(NetLogEvent::StoppedWaiting, self.sd, 0);
        }
    }

    fn fetch_replies(&self) {
        self.channel.fetch_replies();
    }

    fn can_send(&self) -> bool {
        self.channel.can_send().unwrap()
    }

    fn process_events(&mut self) -> bool {
        let mut res = false;
        for _ in 0..EVENT_FETCH_BATCH_SIZE {
            if let Some(event) = self.channel.fetch_event() {
                self.process_event(event);
                res = true;
            }
            else {
                break;
            }
        }
        res
    }

    fn wait_for_events(&mut self, ignore_remote_closes: bool) -> Result<(), Error> {
        while !self.process_events() {
            if !ignore_remote_closes && self.state == State::RemoteClosed {
                return Err(Error::new(Code::SocketClosed));
            }

            log_net(NetLogEvent::StartedWaiting, self.sd, 0);
            self.channel.wait_for_events();
            log_net(NetLogEvent::StoppedWaiting, self.sd, 0);
        }
        Ok(())
    }

    fn wait_for_credits(&self) {
        loop {
            self.fetch_replies();
            if self.can_send() {
                break;
            }

            log_net(NetLogEvent::StartedWaiting, self.sd, 0);
            self.channel.wait_for_credits();
            log_net(NetLogEvent::StoppedWaiting, self.sd, 0);
        }
    }

    fn process_event(&mut self, event: NetEvent) {
        match event.msg_type() {
            NetEventType::DATA => {
                if self.ty != SocketType::Stream
                    || (self.state != State::Closing && self.state != State::Closed)
                {
                    let _msg = event.msg::<event::DataMessage>();
                    log_net(NetLogEvent::RecvPacket, self.sd, _msg.size as usize);
                    llog!(
                        NET,
                        "socket {}: received data with {}b from {}",
                        self.sd,
                        _msg.size,
                        Endpoint::new(IpAddr(_msg.addr as u32), _msg.port as Port)
                    );
                    self.recv_queue.append(event, 0);
                }
            },

            NetEventType::CONNECTED => {
                let msg = event.msg::<event::ConnectedMessage>();
                let ep = Endpoint::new(IpAddr(msg.remote_addr as u32), msg.remote_port as Port);
                log_net(
                    NetLogEvent::RecvConnected,
                    self.sd,
                    msg.remote_port as usize,
                );
                llog!(NET, "socket {}: connected to {}", self.sd, ep);
                self.state = State::Connected;
                self.remote_ep = Some(ep);
            },

            NetEventType::CLOSED => {
                log_net(NetLogEvent::RecvClosed, self.sd, 0);
                llog!(NET, "socket {}: closed", self.sd);
                self.disconnect();
            },

            NetEventType::CLOSE_REQ => {
                log_net(NetLogEvent::RecvRemoteClosed, self.sd, 0);
                llog!(NET, "socket {}: remote side was closed", self.sd);
                self.state = State::RemoteClosed;
            },

            t => panic!("unexpected message type {}", t),
        }
    }
}

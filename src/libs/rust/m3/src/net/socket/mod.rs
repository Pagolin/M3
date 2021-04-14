/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
 * Copyright (C) 2017, Georg Kotheimer <georg.kotheimer@mailbox.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
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

use crate::cell::{Cell, RefCell};
use crate::errors::{Code, Error};
use crate::net::dataqueue::DataQueue;
use crate::net::{event, IpAddr, NetEvent, NetEventChannel, NetEventType, Port, Sd, SocketType};
use crate::rc::Rc;
use crate::session::NetworkManager;

mod raw;
mod tcp;
mod udp;

pub use self::raw::RawSocket;
pub use self::tcp::{StreamSocketArgs, TcpSocket};
pub use self::udp::{DgramSocketArgs, UdpSocket};

const EVENT_FETCH_BATCH_SIZE: u32 = 4;

pub(crate) struct SocketArgs {
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

/// Socket prototype that is shared between sockets.
pub(crate) struct Socket {
    sd: Sd,
    ty: SocketType,

    state: Cell<State>,
    blocking: Cell<bool>,

    local_addr: Cell<IpAddr>,
    local_port: Cell<Port>,
    remote_addr: Cell<IpAddr>,
    remote_port: Cell<Port>,

    channel: Rc<NetEventChannel>,
    recv_queue: RefCell<DataQueue>,
}

impl Socket {
    pub fn new(sd: Sd, ty: SocketType, channel: Rc<NetEventChannel>) -> Rc<Self> {
        Rc::new(Self {
            sd,
            ty,

            state: Cell::new(State::Closed),
            blocking: Cell::new(true),

            local_addr: Cell::new(IpAddr::new(0, 0, 0, 0)),
            local_port: Cell::new(0),

            remote_addr: Cell::new(IpAddr::new(0, 0, 0, 0)),
            remote_port: Cell::new(0),

            channel,
            recv_queue: RefCell::new(DataQueue::default()),
        })
    }

    pub fn sd(&self) -> Sd {
        self.sd
    }

    pub fn state(&self) -> State {
        self.state.get()
    }

    pub fn blocking(&self) -> bool {
        self.blocking.get()
    }

    pub fn set_blocking(&self, blocking: bool) {
        self.blocking.set(blocking);
    }

    pub fn set_local(&self, addr: IpAddr, port: Port, state: State) {
        self.local_addr.set(addr);
        self.local_port.set(port);
        self.state.set(state);
    }

    pub fn connect(
        &self,
        nm: &NetworkManager,
        remote_addr: IpAddr,
        remote_port: Port,
    ) -> Result<(), Error> {
        if self.state.get() == State::Connected {
            if !(self.remote_addr.get() == remote_addr && self.remote_port.get() == remote_port) {
                return Err(Error::new(Code::IsConnected));
            }
            return Ok(());
        }
        if self.state.get() == State::RemoteClosed {
            return Err(Error::new(Code::InvState));
        }

        if self.state.get() == State::Connecting {
            return Err(Error::new(Code::AlreadyInProgress));
        }

        let local_port = nm.connect(self.sd, remote_addr, remote_port)?;
        self.state.set(State::Connecting);
        self.remote_addr.set(remote_addr);
        self.remote_port.set(remote_port);
        self.local_port.set(local_port);

        if !self.blocking.get() {
            return Err(Error::new(Code::InProgress));
        }

        while self.state.get() == State::Connecting {
            self.wait_for_events();
        }

        if self.state.get() != State::Connected {
            Err(Error::new(Code::ConnectionFailed))
        }
        else {
            Ok(())
        }
    }

    pub fn accept(&self) -> Result<(IpAddr, Port), Error> {
        if self.state.get() == State::Connected {
            return Ok((self.remote_addr.get(), self.remote_port.get()));
        }
        if self.state.get() == State::Connecting {
            return Err(Error::new(Code::AlreadyInProgress));
        }
        if self.state.get() != State::Listening {
            return Err(Error::new(Code::InvState));
        }

        // TODO non-blocking

        self.state.set(State::Connecting);
        while self.state.get() == State::Connecting {
            self.wait_for_events();
        }

        if self.state.get() != State::Connected {
            Err(Error::new(Code::ConnectionFailed))
        }
        else {
            Ok((self.remote_addr.get(), self.remote_port.get()))
        }
    }

    pub fn has_data(&self) -> bool {
        self.recv_queue.borrow().has_data()
    }

    pub fn next_data<F, R>(&self, amount: usize, mut consume: F) -> Result<R, Error>
    where
        F: FnMut(&[u8], IpAddr, Port) -> R,
    {
        loop {
            if let Some(res) = self.recv_queue.borrow_mut().next_data(amount, &mut consume) {
                return Ok(res);
            }

            if !self.blocking.get() {
                self.process_events();
                return Err(Error::new(Code::WouldBlock));
            }

            self.wait_for_events();
        }
    }

    pub fn send(&self, data: &[u8], addr: IpAddr, port: Port) -> Result<(), Error> {
        loop {
            let res = self.channel.send_data(addr, port, data.len(), |buf| {
                buf.copy_from_slice(data);
            });
            match res {
                Err(e) if e.code() != Code::NoCredits => break Err(e),
                Ok(_) => break Ok(()),
                _ => {},
            }

            if !self.blocking.get() {
                self.fetch_replies();
                return Err(Error::new(Code::WouldBlock));
            }

            self.wait_for_credits();

            if self.state.get() == State::Closed {
                return Err(Error::new(Code::SocketClosed));
            }
        }
    }

    pub fn close(&self) -> Result<(), Error> {
        // ensure that we don't receive more data (which could block our event channel and thus
        // prevent us from receiving the closed event)
        self.state.set(State::Closing);
        self.recv_queue.borrow_mut().clear();

        // send the close request; this has to be blocking
        loop {
            match self.channel.send_event(event::CloseReqMessage::new()) {
                Err(e) if e.code() == Code::NoCredits => {},
                Err(e) => return Err(e),
                Ok(_) => break,
            }

            self.wait_for_credits();
        }

        // now wait for the response; can be non-blocking
        while self.state.get() != State::Closed {
            if !self.blocking.get() {
                // TODO error inprogress?
                return Err(Error::new(Code::WouldBlock));
            }

            self.wait_for_events();
        }
        Ok(())
    }

    pub fn abort(&self, nm: &NetworkManager, remove: bool) -> Result<(), Error> {
        nm.abort(self.sd, remove)?;
        self.recv_queue.borrow_mut().clear();
        self.state.set(State::Closed);
        Ok(())
    }

    pub fn fetch_replies(&self) {
        self.channel.fetch_replies();
    }

    pub fn can_send(&self) -> bool {
        self.channel.can_send().unwrap()
    }

    pub fn process_events(&self) -> bool {
        let mut res = false;
        for _ in 0..EVENT_FETCH_BATCH_SIZE {
            if let Some(event) = self.channel.receive_event() {
                self.process_event(event);
                res = true;
            }
            else {
                break;
            }
        }
        res
    }

    fn wait_for_events(&self) {
        while !self.process_events() {
            self.channel.wait_for_events();
        }
    }

    fn wait_for_credits(&self) {
        loop {
            self.fetch_replies();
            if self.can_send() {
                break;
            }
            self.channel.wait_for_credits();
        }
    }

    fn process_event(&self, event: NetEvent) {
        match event.msg_type() {
            NetEventType::DATA => {
                if self.ty != SocketType::Stream
                    || (self.state.get() != State::Closing && self.state.get() != State::Closed)
                {
                    self.recv_queue.borrow_mut().append(event);
                }
            },

            NetEventType::CONNECTED => {
                let msg = event.msg::<event::ConnectedMessage>();
                self.state.set(State::Connected);
                self.remote_addr.set(IpAddr(msg.remote_addr as u32));
                self.remote_port.set(msg.remote_port as Port);
            },

            NetEventType::CLOSED => {
                self.state.set(State::Closed);
            },

            NetEventType::CLOSE_REQ => {
                self.state.set(State::RemoteClosed);
            },

            t => panic!("unexpected message type {}", t),
        }
    }
}

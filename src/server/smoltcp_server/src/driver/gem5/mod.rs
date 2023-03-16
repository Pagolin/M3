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

use m3::cell::{RefCell, StaticRefCell};
use m3::errors::Error;
use m3::rc::Rc;
use m3::vec::Vec;


use local_smoltcp::time::{Instant as LocalInstant, Duration};
use local_smoltcp::device;
mod defines;
mod e1000;
mod eeprom;

/// Wrapper around the E1000 driver, implementing smols Device trait
pub struct E1000Device {
    dev: Rc<RefCell<e1000::E1000>>,
}

impl E1000Device {
    pub fn new() -> Result<Self, Error> {
        Ok(E1000Device {
            dev: Rc::new(RefCell::new(e1000::E1000::new()?)),
        })
    }

    pub fn needs_poll(&self) -> bool {
        self.dev.borrow().needs_poll()
    }
}

impl<'a> local_smoltcp::phy::Device<'a> for E1000Device {
    type RxToken = RxToken;
    type TxToken = TxToken;

    fn capabilities(&self) -> loacl_smoltcp::phy::DeviceCapabilities {
        let mut caps = local_smoltcp::phy::DeviceCapabilities::default();
        caps.max_transmission_unit = e1000::E1000::mtu();
        caps.checksum.ipv4 = local_smoltcp::phy::Checksum::None;
        caps.checksum.udp = local_smoltcp::phy::Checksum::None;
        caps.checksum.tcp = local_smoltcp::phy::Checksum::None;
        caps
    }

    fn receive(&'a mut self) -> Option<(Self::RxToken, Self::TxToken)> {
        match self.dev.borrow_mut().receive() {
            Ok(buffer) => {
                let rx = RxToken { buffer };
                let tx = TxToken {
                    device: self.dev.clone(),
                };
                Some((rx, tx))
            },
            Err(_) => None,
        }
    }

    fn transmit(&'a mut self) -> Option<Self::TxToken> {
        Some(TxToken {
            device: self.dev.clone(),
        })
    }
        fn needs_poll(&self, _max_duration: Option<Duration>) -> bool{
            self.needs_poll()
        }

}

pub struct RxToken {
    buffer: Vec<u8>,
}


impl local_smoltcp::phy::RxToken for RxToken {
    fn consume<R, F>(mut self, _timestamp: LocalInstant, f: F) -> local_smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> local_smoltcp::Result<R>,
    {
        f(&mut self.buffer[..])
    }
}

pub struct TxToken {
    device: Rc<RefCell<e1000::E1000>>,
}

// use a static and initialized buffer for all packets we send
static SEND_BUF: StaticRefCell<[u8; e1000::E1000::mtu()]> =
    StaticRefCell::new([0u8; e1000::E1000::mtu()]);


impl local_smoltcp::phy::TxToken for TxToken {
    fn consume<R, F>(self, _timestamp: LocalInstant, len: usize, f: F) -> local_smoltcp::Result<R>
    where
        F: FnOnce(&mut [u8]) -> local_smoltcp::Result<R>,
    {
        // fill buffer with "to be send" data
        assert!(len <= SEND_BUF.borrow().len());
        let res = f(&mut SEND_BUF.borrow_mut()[0..len])?;
        match self.device.borrow_mut().send(&SEND_BUF.borrow()[0..len]) {
            true => Ok(res),
            false => Err(local_smoltcp::Error::Exhausted),
        }
    }
}
/*
 * Copyright (C) 2021, Tendsin Mende <tendsin.mende@mailbox.tu-dresden.de>
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

/// Conditional include of the driver
#[cfg(target_vendor = "host")]
#[path = "host/mod.rs"]
mod inner;

#[cfg(target_vendor = "gem5")]
#[path = "gem5/mod.rs"]
mod inner;

#[cfg(target_vendor = "hw")]
#[path = "hw/mod.rs"]
mod inner;

pub use inner::*;

pub const LOG_ERR: bool = true;
pub const LOG_DEF: bool = true;
pub const LOG_SESS: bool = false;
pub const LOG_DATA: bool = false;
pub const LOG_PORTS: bool = false;
pub const LOG_NIC: bool = false;
pub const LOG_NIC_ERR: bool = true;
pub const LOG_NIC_DETAIL: bool = false;
pub const LOG_SMOLTCP: bool = false;
pub const LOG_DETAIL: bool = false;

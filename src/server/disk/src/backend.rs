/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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

use m3::com::MemGate;
use m3::errors::Error;

pub trait BlockDevice {
    fn partition_exists(&self, part: usize) -> bool;

    fn read(
        &mut self,
        part: usize,
        buf: &MemGate,
        buf_off: usize,
        disk_off: usize,
        bytes: usize,
    ) -> Result<(), Error>;

    fn write(
        &mut self,
        part: usize,
        buf: &MemGate,
        buf_off: usize,
        disk_off: usize,
        bytes: usize,
    ) -> Result<(), Error>;
}

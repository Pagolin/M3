/*
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

use core::fmt;

use base::mem::GlobAddr;

use crate::cap::{CapFlags, Selector};
use crate::cell::Ref;
use crate::col::Vec;
use crate::com::ep::EP;
use crate::com::gate::Gate;
use crate::errors::Error;
use crate::goff;
use crate::kif::INVALID_SEL;
use crate::mem::{self, MaybeUninit};
use crate::syscalls;
use crate::tcu;
use crate::tiles::Activity;

pub use crate::kif::Perm;

/// A memory gate (`MemGate`) has access to a contiguous memory region and allows RDMA-like memory
/// accesses via TCU.
pub struct MemGate {
    gate: Gate,
    resmng: bool,
}

/// The arguments for [`MemGate`] creations.
pub struct MGateArgs {
    size: usize,
    perm: Perm,
    sel: Selector,
}

impl MGateArgs {
    /// Creates a new `MGateArgs` object with default settings
    pub fn new(size: usize, perm: Perm) -> MGateArgs {
        MGateArgs {
            size,
            perm,
            sel: INVALID_SEL,
        }
    }

    /// Sets the capability selector that should be used for this [`MemGate`]. Otherwise and by
    /// default, [`Activity::own().alloc_sel`](crate::tiles::OwnActivity::alloc_sel) will be used to
    /// choose a free selector.
    pub fn sel(mut self, sel: Selector) -> Self {
        self.sel = sel;
        self
    }
}

impl MemGate {
    /// Creates a new `MemGate` that has access to a region of `size` bytes with permissions `perm`.
    pub fn new(size: usize, perm: Perm) -> Result<Self, Error> {
        Self::new_with(MGateArgs::new(size, perm))
    }

    /// Creates a new `MemGate` with given arguments.
    pub fn new_with(args: MGateArgs) -> Result<Self, Error> {
        let sel = if args.sel == INVALID_SEL {
            Activity::own().alloc_sel()
        }
        else {
            args.sel
        };

        Activity::own()
            .resmng()
            .unwrap()
            .alloc_mem(sel, args.size as goff, args.perm)?;
        Ok(MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            resmng: true,
        })
    }

    /// Creates a new `MemGate` for the region `off`..`off`+`size` in the address space of the given
    /// activity. The region must be physically contiguous and page aligned.
    pub fn new_foreign(act: Selector, off: goff, size: goff, perm: Perm) -> Result<Self, Error> {
        let sel = Activity::own().alloc_sel();
        syscalls::create_mgate(sel, act, off, size, perm)?;
        Ok(MemGate::new_owned_bind(sel))
    }

    /// Binds a new `MemGate` to the given selector.
    pub fn new_bind(sel: Selector) -> Self {
        MemGate {
            gate: Gate::new(sel, CapFlags::KEEP_CAP),
            resmng: false,
        }
    }

    /// Binds a new `MemGate` to the given selector and revokes the cap on drop.
    pub fn new_owned_bind(sel: Selector) -> Self {
        MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            resmng: false,
        }
    }

    /// Binds a new `MemGate` to the boot module with given name.
    pub fn new_bind_bootmod(name: &str) -> Result<Self, Error> {
        let sel = Activity::own().alloc_sel();
        Activity::own().resmng().unwrap().use_mod(sel, name)?;
        Ok(MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            resmng: false,
        })
    }

    /// Returns the selector of this gate
    pub fn sel(&self) -> Selector {
        self.gate.sel()
    }

    /// Returns the endpoint of the gate. If the gate is not activated, `None` is returned.
    pub fn ep(&self) -> Option<Ref<'_, EP>> {
        self.gate.ep()
    }

    /// Sets or unsets the endpoint.
    pub(crate) fn set_ep(&mut self, ep: Option<EP>) {
        self.gate.set_ep(ep);
    }

    /// Returns the memory region (global address and size) this MemGate references.
    pub fn region(&self) -> Result<(GlobAddr, goff), Error> {
        syscalls::mgate_region(self.sel())
    }

    /// Derives a new `MemGate` from `self` that has access to a subset of `self`'s the memory
    /// region and has a subset of `self`'s permissions. The subset of the memory region is defined
    /// by `offset` and `size` and the permissions by `perm`.
    ///
    /// Note that kernel makes sure that only owned permissions can be passed on to the derived
    /// `MemGate`.
    pub fn derive(&self, offset: goff, size: usize, perm: Perm) -> Result<Self, Error> {
        let sel = Activity::own().alloc_sel();
        self.derive_for(Activity::own().sel(), sel, offset, size, perm)
    }

    /// Like [`MemGate::derive`], but assigns the new `MemGate` to the given activity and uses given
    /// selector.
    pub fn derive_for(
        &self,
        act: Selector,
        sel: Selector,
        offset: goff,
        size: usize,
        perm: Perm,
    ) -> Result<Self, Error> {
        syscalls::derive_mem(act, sel, self.sel(), offset, size as goff, perm)?;
        Ok(MemGate {
            gate: Gate::new(sel, CapFlags::empty()),
            resmng: false,
        })
    }

    /// Uses the TCU read command to read from the memory region at offset `off` and stores the read
    /// data into a vector. The number of bytes to read is defined by the number of items and the
    /// size of `T`.
    #[allow(clippy::uninit_vec)]
    pub fn read_into_vec<T>(&self, items: usize, off: goff) -> Result<Vec<T>, Error> {
        let mut vec = Vec::<T>::with_capacity(items);
        // we deliberately use uninitialize memory here, because it's performance critical
        // safety: this is okay, because the TCU does not read from `vec`
        unsafe { vec.set_len(items) };
        self.read(&mut vec, off)?;
        Ok(vec)
    }

    /// Uses the TCU read command to read from the memory region at offset `off` and stores the read
    /// data into the slice `data`. The number of bytes to read is defined by `data`.
    pub fn read<T>(&self, data: &mut [T], off: goff) -> Result<(), Error> {
        self.read_bytes(
            data.as_mut_ptr() as *mut u8,
            data.len() * mem::size_of::<T>(),
            off,
        )
    }

    /// Reads `mem::size_of::<T>()` bytes via the TCU read command from the memory region at offset
    /// `off` and returns the data as an object of `T`.
    pub fn read_obj<T>(&self, off: goff) -> Result<T, Error> {
        #[allow(clippy::uninit_assumed_init)]
        // safety: will be initialized in read_bytes
        let mut obj: T = unsafe { MaybeUninit::uninit().assume_init() };
        self.read_bytes(&mut obj as *mut T as *mut u8, mem::size_of::<T>(), off)?;
        Ok(obj)
    }

    /// Reads `size` bytes via the TCU read command from the memory region at offset `off` and
    /// stores the read data into `data`.
    pub fn read_bytes(&self, data: *mut u8, size: usize, off: goff) -> Result<(), Error> {
        let ep = self.activate()?;
        tcu::TCU::read(ep, data, size, off)
    }

    /// Writes `data` with the TCU write command to the memory region at offset `off`.
    pub fn write<T>(&self, data: &[T], off: goff) -> Result<(), Error> {
        self.write_bytes(
            data.as_ptr() as *const u8,
            data.len() * mem::size_of::<T>(),
            off,
        )
    }

    /// Writes `obj` via the TCU write command to the memory region at offset `off`.
    pub fn write_obj<T>(&self, obj: &T, off: goff) -> Result<(), Error> {
        self.write_bytes(obj as *const T as *const u8, mem::size_of::<T>(), off)
    }

    /// Writes the `size` bytes at `data` via the TCU write command to the memory region at offset
    /// `off`.
    pub fn write_bytes(&self, data: *const u8, size: usize, off: goff) -> Result<(), Error> {
        let ep = self.activate()?;
        tcu::TCU::write(ep, data, size, off)
    }

    /// Activates the gate. Returns the chosen endpoint number.
    /// The endpoint can be delegated to other services (e.g. M3FS) to let them
    /// remotely configure it to point to memory in another tile.
    #[inline(always)]
    pub fn activate(&self) -> Result<tcu::EpId, Error> {
        self.gate.activate()
    }

    /// Deactivates this `MemGate` in case it was already activated
    pub fn deactivate(&mut self) {
        self.gate.release(true);
    }
}

impl Drop for MemGate {
    fn drop(&mut self) {
        if !self.gate.flags().contains(CapFlags::KEEP_CAP) && self.resmng {
            Activity::own().resmng().unwrap().free_mem(self.sel()).ok();
            self.gate.set_flags(CapFlags::KEEP_CAP);
        }
    }
}

impl fmt::Debug for MemGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "MemGate[sel: {}, ep: {:?}]",
            self.sel(),
            self.gate.epid()
        )
    }
}

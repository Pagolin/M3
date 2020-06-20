/*
 * Copyright (C) 2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#![feature(llvm_asm)]
#![no_std]

#[macro_use]
extern crate base;
#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate cfg_if;
#[cfg(feature = "heap")]
extern crate heap;

cfg_if! {
    if #[cfg(target_arch = "x86_64")] {
        #[path = "x86_64/mod.rs"]
        mod arch;
    }
    else if #[cfg(target_arch = "arm")] {
        #[path = "arm/mod.rs"]
        mod arch;
    }
    else if #[cfg(target_arch = "riscv64")] {
        #[path = "riscv/mod.rs"]
        mod arch;
    }
}

use base::cfg;
use base::errors::{Code, Error};
use base::goff;
use base::kif::{PageFlags, PTE};
use base::libc;
use base::math;
use base::mem::GlobAddr;
use base::tcu::TCU;
use base::util;
use core::fmt;

use arch::{LEVEL_BITS, LEVEL_CNT, LEVEL_MASK};

pub type VPEId = u64;

pub use arch::{
    build_pte, enable_paging, glob_to_phys, phys_to_glob, pte_to_phys, to_page_flags, MMUFlags,
    Phys, MMUPTE,
};

/// Logs mapping operations
pub const LOG_MAP: bool = false;
/// Logs detailed mapping operations
pub const LOG_MAP_DETAIL: bool = false;

pub type AllocFrameFunc = extern "C" fn(vpe: VPEId) -> Phys;
pub type XlatePtFunc = extern "C" fn(vpe: VPEId, phys: Phys) -> usize;

pub struct ExtAllocator {
    vpe: VPEId,
    alloc_frame: AllocFrameFunc,
    xlate_pt: XlatePtFunc,
}

impl ExtAllocator {
    pub fn new(vpe: VPEId, alloc_frame: AllocFrameFunc, xlate_pt: XlatePtFunc) -> Self {
        Self {
            vpe,
            alloc_frame,
            xlate_pt,
        }
    }
}

impl Allocator for ExtAllocator {
    fn allocate_pt(&mut self) -> MMUPTE {
        (self.alloc_frame)(self.vpe)
    }

    fn translate_pt(&self, phys: Phys) -> usize {
        (self.xlate_pt)(self.vpe, phys)
    }

    fn free_pt(&mut self, _phys: Phys) {
    }
}

#[no_mangle]
pub extern "C" fn init_aspace(
    id: VPEId,
    alloc_frame: AllocFrameFunc,
    xlate_pt: XlatePtFunc,
    root: goff,
) {
    let aspace = AddrSpace::new(
        id,
        GlobAddr::new(root),
        ExtAllocator::new(id, alloc_frame, xlate_pt),
        true,
    );
    aspace.init();
}

#[no_mangle]
pub extern "C" fn map_pages(
    id: VPEId,
    virt: usize,
    global: goff,
    pages: usize,
    perm: PTE,
    alloc_frame: AllocFrameFunc,
    xlate_pt: XlatePtFunc,
    root: goff,
) {
    let mut aspace = AddrSpace::new(
        id,
        GlobAddr::new(root),
        ExtAllocator::new(id, alloc_frame, xlate_pt),
        true,
    );
    let perm = PageFlags::from_bits_truncate(perm);
    aspace
        .map_pages(virt, GlobAddr::new(global), pages, perm)
        .unwrap();
}

#[no_mangle]
pub extern "C" fn get_addr_space() -> goff {
    arch::phys_to_glob(arch::get_root_pt())
}

#[no_mangle]
pub extern "C" fn set_addr_space(root: goff, alloc_frame: AllocFrameFunc, xlate_pt: XlatePtFunc) {
    let aspace = AddrSpace::new(
        0,
        GlobAddr::new(root),
        ExtAllocator::new(0, alloc_frame, xlate_pt),
        true,
    );
    aspace.switch_to();
}

#[no_mangle]
pub extern "C" fn translate(
    id: VPEId,
    root: goff,
    alloc_frame: AllocFrameFunc,
    xlate_pt: XlatePtFunc,
    virt: usize,
    perm: PTE,
) -> PTE {
    let aspace = AddrSpace::new(
        id,
        GlobAddr::new(root),
        ExtAllocator::new(id, alloc_frame, xlate_pt),
        true,
    );
    aspace.translate(virt, perm)
}

pub trait Allocator {
    /// Allocates a new page table and returns its physical addres
    fn allocate_pt(&mut self) -> Phys;

    /// Translates the given physical address of a page table to a virtual address
    fn translate_pt(&self, phys: Phys) -> usize;

    /// Frees the given page table
    fn free_pt(&mut self, phys: Phys);
}

pub struct AddrSpace<A: Allocator> {
    id: VPEId,
    root: Phys,
    alloc: A,
    is_temp: bool,
}

impl<A: Allocator> AddrSpace<A> {
    pub fn new(id: VPEId, root: GlobAddr, alloc: A, is_temp: bool) -> Self {
        AddrSpace {
            id,
            root: build_pte(
                arch::glob_to_phys(root.raw()),
                MMUFlags::empty(),
                LEVEL_CNT,
                false,
            ),
            alloc,
            is_temp,
        }
    }

    pub fn id(&self) -> VPEId {
        self.id
    }

    pub fn allocator(&self) -> &A {
        &self.alloc
    }

    pub fn allocator_mut(&mut self) -> &mut A {
        &mut self.alloc
    }

    pub fn switch_to(&self) {
        arch::set_root_pt(self.id, pte_to_phys(self.root));
    }

    pub fn init(&self) {
        let root_phys = pte_to_phys(self.root);
        let pt_virt = self.alloc.translate_pt(root_phys);
        Self::clear_pt(pt_virt);
    }

    pub fn translate(&self, virt: usize, perm: PTE) -> PTE {
        // otherwise, walk through all levels
        let perm = arch::to_mmu_perms(PageFlags::from_bits_truncate(perm));
        let mut pte = self.root;
        for lvl in (0..LEVEL_CNT).rev() {
            let pt_virt = self.alloc.translate_pt(pte_to_phys(pte));
            let idx = (virt >> (cfg::PAGE_BITS + lvl * LEVEL_BITS)) & LEVEL_MASK;
            let pte_addr = pt_virt + idx * util::size_of::<MMUPTE>();

            // safety: as above
            pte = unsafe { *(pte_addr as *const MMUPTE) };

            let pte_flags = MMUFlags::from_bits_truncate(pte);
            if pte_flags.is_leaf(lvl) || pte_flags.perms_missing(perm) {
                let res = phys_to_glob(pte_to_phys(pte));
                let flags = MMUFlags::from_bits_truncate(pte);
                return res | to_page_flags(lvl, flags).bits();
            }
        }
        unreachable!();
    }

    pub fn map_pages(
        &mut self,
        mut virt: usize,
        global: GlobAddr,
        mut pages: usize,
        perm: PageFlags,
    ) -> Result<(), Error> {
        log!(
            crate::LOG_MAP,
            "VPE{}: mapping 0x{:0>16x}..0x{:0>16x} to {:?}..{:?} with {:?}",
            self.id,
            virt,
            virt + pages * cfg::PAGE_SIZE - 1,
            global,
            global + (pages * cfg::PAGE_SIZE - 1) as goff,
            perm
        );

        let lvl = LEVEL_CNT - 1;
        let mut phys = arch::glob_to_phys(global.raw());
        let perm = arch::to_mmu_perms(perm);
        self.map_pages_rec(&mut virt, &mut phys, &mut pages, perm, self.root, lvl)
    }

    fn map_pages_rec(
        &mut self,
        virt: &mut usize,
        phys: &mut Phys,
        pages: &mut usize,
        perm: MMUFlags,
        pte: MMUPTE,
        level: usize,
    ) -> Result<(), Error> {
        // determine virtual address for page table
        let pt_virt = self.alloc.translate_pt(pte_to_phys(pte));

        // start at the corresponding index
        let idx = (*virt >> (cfg::PAGE_BITS + level * LEVEL_BITS)) & LEVEL_MASK;
        let mut pte_addr = pt_virt + idx * util::size_of::<MMUPTE>();

        while *pages > 0 {
            // reached end of page table?
            if pte_addr >= pt_virt + cfg::PAGE_SIZE {
                break;
            }

            // safety: as above
            let mut pte = unsafe { *(pte_addr as *const MMUPTE) };

            // can we use a large page?
            if level == 0
                || (level == 1
                    && math::is_aligned(*virt, cfg::LPAGE_SIZE)
                    && math::is_aligned(*phys, cfg::LPAGE_SIZE as MMUPTE)
                    && *pages * cfg::PAGE_SIZE >= cfg::LPAGE_SIZE)
            {
                let psize = if level == 1 {
                    cfg::LPAGE_SIZE
                }
                else {
                    cfg::PAGE_SIZE
                };

                let new_pte = build_pte(*phys, perm, level, true);

                // determine if we need to perform an TLB invalidate
                let old_flags = MMUFlags::from_bits_truncate(pte);
                let new_flags = MMUFlags::from_bits_truncate(new_pte);
                let invalidate = arch::needs_invalidate(new_flags, old_flags);
                if invalidate {
                    TCU::invalidate_page(self.id as u16, *virt);
                    arch::invalidate_page(self.id, *virt);
                }

                // safety: as above
                unsafe {
                    *(pte_addr as *mut MMUPTE) = new_pte
                };

                log!(
                    crate::LOG_MAP_DETAIL,
                    "VPE{}: lvl {} PTE for 0x{:0>16x}: 0x{:0>16x} (invalidate={})",
                    self.id,
                    level,
                    virt,
                    new_pte,
                    invalidate
                );

                *pages -= psize / cfg::PAGE_SIZE;
                *virt += psize;
                *phys += psize as MMUPTE;
            }
            else {
                // unmapping non-existing PTs is a noop
                if !(pte == 0 && perm.is_empty()) {
                    if pte == 0 {
                        pte = self.create_pt(*virt, pte_addr, level)?;
                    }

                    self.map_pages_rec(virt, phys, pages, perm, pte, level - 1)?;
                }
            }

            pte_addr += util::size_of::<MMUPTE>();
        }

        Ok(())
    }

    fn create_pt(&mut self, virt: usize, pte_addr: usize, level: usize) -> Result<MMUPTE, Error> {
        let frame = self.alloc.allocate_pt();
        if frame == 0 {
            return Err(Error::new(Code::NoSpace));
        }
        Self::clear_pt(self.alloc.translate_pt(frame));

        // insert PTE
        let pte = build_pte(frame, MMUFlags::empty(), level, false);
        // safety: as above
        unsafe {
            *(pte_addr as *mut MMUPTE) = pte
        };

        let pt_size = (1 << (LEVEL_BITS * level)) * cfg::PAGE_SIZE;
        let virt_base = virt as usize & !(pt_size - 1);
        log!(
            crate::LOG_MAP_DETAIL,
            "VPE{}: lvl {} PTE for 0x{:0>16x}: 0x{:0>16x}",
            self.id,
            level,
            virt_base,
            pte
        );

        Ok(pte)
    }

    fn clear_pt(pt_virt: usize) {
        unsafe {
            libc::memset(pt_virt as *mut _, 0, cfg::PAGE_SIZE)
        };
    }

    fn free_pts_rec(&mut self, pt: MMUPTE, level: usize) {
        let mut ptes = self.alloc.translate_pt(pte_to_phys(pt));
        for _ in 0..1 << LEVEL_BITS {
            // safety: as above
            let pte = unsafe { *(ptes as *const MMUPTE) };
            if pte != 0 {
                // refers the PTE to a PT?
                if !MMUFlags::from_bits_truncate(pte).is_leaf(level) {
                    // there are no PTEs refering to PTs at level 0
                    if level > 1 {
                        self.free_pts_rec(pte, level - 1);
                    }
                    self.alloc.free_pt(pte_to_phys(pte));
                }
            }

            ptes += util::size_of::<MMUPTE>();
        }
    }

    fn print_as_rec(
        &self,
        f: &mut fmt::Formatter<'_>,
        pt: MMUPTE,
        mut virt: usize,
        level: usize,
    ) -> Result<(), fmt::Error> {
        let mut ptes = self.alloc.translate_pt(pte_to_phys(pt));
        for _ in 0..1 << LEVEL_BITS {
            // safety: as above
            let pte = unsafe { *(ptes as *const MMUPTE) };
            if pte != 0 {
                let w = (LEVEL_CNT - level - 1) * 2;
                writeln!(f, "{:w$}0x{:0>16x}: 0x{:0>16x}", "", virt, pte, w = w)?;
                if !MMUFlags::from_bits_truncate(pte).is_leaf(level) {
                    self.print_as_rec(f, pte, virt, level - 1)?;
                }
            }

            virt += 1 << (level as usize * LEVEL_BITS + cfg::PAGE_BITS);
            ptes += util::size_of::<MMUPTE>();
        }
        Ok(())
    }
}

impl<A: Allocator> Drop for AddrSpace<A> {
    fn drop(&mut self) {
        if !self.is_temp && pte_to_phys(self.root) != 0 {
            self.free_pts_rec(self.root, LEVEL_CNT - 1);

            // invalidate entire TLB to allow us to reuse the VPE id
            arch::invalidate_tlb();
            TCU::invalidate_tlb();
        }
    }
}

impl<A: Allocator> fmt::Debug for AddrSpace<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        writeln!(f, "Address space @ 0x{:0>16x}:", pte_to_phys(self.root))?;
        self.print_as_rec(f, self.root, 0, LEVEL_CNT - 1)
    }
}

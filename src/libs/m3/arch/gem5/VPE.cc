/*
 * Copyright (C) 2015, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Common.h>
#include <base/util/Math.h>
#include <base/Config.h>
#include <base/Heap.h>

#include <m3/VPE.h>

namespace m3 {

extern "C" void *_text_start;
extern "C" void *_text_end;
extern "C" void *_data_start;
extern "C" void *_bss_end;

word_t VPE::get_sp() {
    word_t val;
    asm volatile (
          "mov %%rsp,%0;"
          : "=r" (val)
    );
    return val;
}

uintptr_t VPE::get_entry() {
    return (uintptr_t)&_text_start;
}

void VPE::copy_sections() {
    /* copy text */
    uintptr_t start_addr = Math::round_dn((uintptr_t)&_text_start, DTU_PKG_SIZE);
    uintptr_t end_addr = Math::round_up((uintptr_t)&_text_end, DTU_PKG_SIZE);
    _mem.write_sync((void*)start_addr, end_addr - start_addr, start_addr);

    /* copy data and heap */
    start_addr = Math::round_dn((uintptr_t)&_data_start, DTU_PKG_SIZE);
    end_addr = Math::round_up(Heap::end(), DTU_PKG_SIZE);
    _mem.write_sync((void*)start_addr, end_addr - start_addr, start_addr);

    /* copy end-area of heap */
    start_addr = Math::round_up((uintptr_t)&_bss_end + INIT_HEAP_SIZE, PAGE_SIZE) - DTU_PKG_SIZE;
    _mem.write_sync((void*)start_addr, DTU_PKG_SIZE, start_addr);

    /* copy stack */
    start_addr = get_sp();
    end_addr = STACK_TOP;
    _mem.write_sync((void*)start_addr, end_addr - start_addr, start_addr);
}

bool VPE::skip_section(ElfPh *) {
    return false;
}

}

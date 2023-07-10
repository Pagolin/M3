/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2020 Nils Asmussen, Barkhausen Institut
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

#include <base/Backtrace.h>
#include <base/CPU.h>
#include <base/Config.h>
#include <base/stream/Format.h>
#include <base/util/Math.h>

namespace m3 {

size_t Backtrace::collect(uintptr_t *addr, size_t max) {
    uintptr_t bp = CPU::base_pointer();

    uintptr_t base = Math::round_dn<uintptr_t>(bp, STACK_SIZE);
    uintptr_t end = Math::round_up<uintptr_t>(bp, STACK_SIZE);
    uintptr_t start = end - STACK_SIZE;

    size_t i = 0;
    for(; bp >= start && bp < end && i < max; ++i) {
        bp = base + (bp & (STACK_SIZE - 1));
        bp = CPU::backtrace_step(bp, addr + i);
    }
    return i;
}

void Backtrace::print(OStream &os) {
    uintptr_t addr[MAX_DEPTH];
    size_t cnt = collect(addr, MAX_DEPTH);

    println_to(os, "Backtrace:"_cf);
    for(size_t i = 0; i < cnt; ++i)
        println_to(os, " {:p}"_cf, addr[i]);
}

}

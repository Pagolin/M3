/*
 * Copyright (C) 2020-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/Init.h>

#include <m3/Exception.h>
#include <m3/Syscalls.h>
#include <m3/com/RecvBufs.h>
#include <m3/tiles/OwnActivity.h>

namespace m3 {

INIT_PRIO_RECVBUF RecvBufs RecvBufs::_inst;

RecvBuf *RecvBufs::alloc(size_t size) {
    bool vm = Activity::own().tile_desc().has_virtmem();
    // page align the receive buffers so that we can map them
    auto maybe_addr = _bufs.allocate(size, vm ? PAGE_SIZE : 1);
    if(!maybe_addr)
        vthrow(Errors::NO_SPACE, "Insufficient rbuf space for {}b"_cf, size);

    auto addr = maybe_addr.unwrap();
    std::unique_ptr<MemGate> mgate;
    if(vm) {
        // allocate memory
        size_t aligned_size = Math::round_up(size, static_cast<size_t>(PAGE_SIZE));
        mgate.reset(new MemGate(MemGate::create_global(aligned_size, MemGate::R)));

        // map receive buffer
        capsel_t dst = addr / PAGE_SIZE;
        capsel_t pages = aligned_size / PAGE_SIZE;
        try {
            Syscalls::create_map(dst, Activity::own().sel(), mgate->sel(), 0, pages, MemGate::R);
        }
        catch(...) {
            // undo allocation
            _bufs.free(addr, size);
            throw;
        }
    }

    return new RecvBuf(addr, size, mgate);
}

void RecvBufs::free(RecvBuf *rbuf) noexcept {
    _bufs.free(rbuf->addr(), rbuf->size());
    delete rbuf;
}

}

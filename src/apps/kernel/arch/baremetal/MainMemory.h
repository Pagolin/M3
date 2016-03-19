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

#pragma once

#include <base/Common.h>
#include <base/Config.h>
#include <base/Log.h>

#include "mem/MemoryMap.h"

namespace kernel {

class MainMemory {
    explicit MainMemory() : _size(DRAM_SIZE), _map(addr(), DRAM_SIZE) {
        LOG(DEF, "We have " << (DRAM_SIZE / 1024) << " KiB of main memory");
    }

public:
    static MainMemory &get() {
        return _inst;
    }

    uintptr_t base() const {
        return 0;
    }
    uintptr_t addr() const {
        return DRAM_OFFSET;
    }
    size_t size() const {
        return _size;
    }
    size_t epid() const {
        return 0;
    }
    MemoryMap &map() {
        return _map;
    }

private:
    size_t _size;
    MemoryMap _map;
    static MainMemory _inst;
};

}

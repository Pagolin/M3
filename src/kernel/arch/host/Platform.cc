/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/Config.h>
#include <base/log/Kernel.h>
#include <base/Init.h>

#include <sys/mman.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <vector>

#include "mem/MainMemory.h"
#include "DTU.h"
#include "Platform.h"

namespace kernel {

m3::PEDesc *Platform::_pes;
m3::BootInfo::Mod *Platform::_mods;
INIT_PRIO_USER(2) m3::BootInfo Platform::_info;
INIT_PRIO_USER(3) Platform::Init Platform::_init;

static MainMemory::Allocation binfomem;

Platform::Init::Init() {
    // no modules
    Platform::_info.mod_count = 0;
    Platform::_info.mod_size = 0;

    // init PEs
    Platform::_info.pe_count = PE_COUNT + 1;
    Platform::_pes = new m3::PEDesc[PE_COUNT + 1];
    for(int i = 0; i < PE_COUNT; ++i)
        Platform::_pes[i] = m3::PEDesc(m3::PEType::COMP_IMEM, m3::PEISA::X86, 1024 * 1024);
    Platform::_pes[PE_COUNT] = m3::PEDesc(m3::PEType::MEM, m3::PEISA::NONE, TOTAL_MEM_SIZE);

    // create memory
    uintptr_t base = reinterpret_cast<uintptr_t>(
        mmap(0, TOTAL_MEM_SIZE, PROT_READ | PROT_WRITE, MAP_ANON | MAP_PRIVATE, -1, 0));

    if(TOTAL_MEM_SIZE <= FS_MAX_SIZE + KERNEL_MEM)
        PANIC("Not enough DRAM");

    MainMemory &mem = MainMemory::get();
    mem.add(new MemoryModule(MemoryModule::OCCUPIED, 0, base, FS_MAX_SIZE));
    mem.add(new MemoryModule(MemoryModule::KERNEL, 0, base + FS_MAX_SIZE, KERNEL_MEM));
    size_t usize = TOTAL_MEM_SIZE - (FS_MAX_SIZE + KERNEL_MEM);
    mem.add(new MemoryModule(MemoryModule::USER, 0, base + FS_MAX_SIZE + KERNEL_MEM, usize));

    // set memories
    _info.mems[0] = m3::BootInfo::Mem(FS_MAX_SIZE, true);
    _info.mems[1] = m3::BootInfo::Mem(usize, false);
}

void Platform::add_modules(int argc, char **argv) {
    MainMemory &mem = MainMemory::get();

    std::vector<m3::BootInfo::Mod*> mods;
    size_t bmodsize = 0;
    for(int i = 0; i < argc; ++i) {
        if(strcmp(argv[i], "--") == 0)
            continue;

        m3::OStringStream args;
        int j = i + 1;
        args << basename(argv[i]);
        for(; j < argc; ++j) {
            if(strcmp(argv[j], "--") == 0)
                break;
            // ignore the pager
            if(strcmp(argv[j], "requires=pager") == 0)
                continue;
            args << " " << argv[j];
        }

        // ignore the pager
        if(strncmp(args.str(), "pager", 5) == 0) {
            i = j;
            continue;
        }

        m3::BootInfo::Mod *mod = reinterpret_cast<m3::BootInfo::Mod*>(
            malloc(sizeof(m3::BootInfo::Mod) + args.length() + 1));
        mod->namelen = args.length() + 1;
        strcpy(mod->name, args.str());

        bmodsize += sizeof(m3::BootInfo::Mod) + args.length() + 1;

        // copy boot module into memory
        {
            int fd = open(argv[i], O_RDONLY);
            if(fd < 0)
                PANIC("Opening '" << argv[i] << "' for reading failed");
            struct stat info;
            if(fstat(fd, &info) == -1)
                PANIC("Stat for '" << argv[i] << "' failed");

            MainMemory::Allocation alloc = mem.allocate(static_cast<size_t>(info.st_size), 1);
            if(!alloc)
                PANIC("Not enough memory for boot module '" << argv[i] << "'");
            ssize_t res = read(fd, reinterpret_cast<void*>(alloc.addr), alloc.size);
            if(res == -1)
                PANIC("Reading from '" << argv[i] << "' failed");
            close(fd);

            mod->addr = alloc.addr;
            mod->size = alloc.size;
        }

        mods.push_back(mod);
        i = j;
    }

    // set modules
    _info.mod_count = mods.size();
    _info.mod_size = bmodsize;

    // build kinfo page
    size_t bsize = sizeof(m3::BootInfo) + bmodsize + sizeof(m3::PEDesc) * PE_COUNT;
    binfomem = mem.allocate(bsize, 1);
    if(!binfomem)
        PANIC("Not enough memory for boot info");
    m3::BootInfo *binfo = reinterpret_cast<m3::BootInfo*>(binfomem.addr);
    memcpy(binfo, &_info, sizeof(_info));

    // add modules to info
    uintptr_t mod_addr = binfomem.addr + sizeof(_info);
    _mods = reinterpret_cast<m3::BootInfo::Mod*>(mod_addr);
    for(auto mod : mods) {
        size_t size = sizeof(*mod) + mod->namelen;
        memcpy(reinterpret_cast<void*>(mod_addr), &*mod, size);
        mod_addr += size;
    }

    // add PEs to info
    for(uint64_t i = 0; i < _info.pe_count; ++i) {
        memcpy(reinterpret_cast<void*>(mod_addr), Platform::_pes + i, sizeof(m3::PEDesc));
        mod_addr += sizeof(m3::PEDesc);
    }

    // free memory
    for(auto mod : mods)
        free(&*mod);
}

gaddr_t Platform::info_addr() {
    return m3::DTU::build_gaddr(binfomem.pe(), binfomem.addr);
}

peid_t Platform::kernel_pe() {
    return 0;
}
peid_t Platform::first_pe() {
    return 1;
}
peid_t Platform::last_pe() {
    return _info.pe_count - 1;
}

goff_t Platform::def_recvbuf(peid_t) {
    // unused
    return 0;
}

}

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

#include <base/Common.h>
#include <base/stream/Serial.h>
#include <base/Backtrace.h>
#include <base/Env.h>

#include <m3/com/RecvGate.h>
#include <m3/stream/Standard.h>
#include <m3/PEXCalls.h>
#include <m3/Syscalls.h>
#include <m3/pes/VPE.h>

namespace m3 {

class EnvUserBackend : public Gem5EnvBackend {
public:
    explicit EnvUserBackend() {
    }

    virtual void init() override {
        uint64_t *argv = reinterpret_cast<uint64_t*>(env()->argv);
        Serial::init(reinterpret_cast<char*>(argv[0]), env()->pe_id);
    }

    virtual void reinit() override {
        init();
        Syscalls::reinit();
        RecvGate::reinit();
        VPE::reset();
    }

    NORETURN void exit(UNUSED int code) override {
#if defined(__gem5__)
        PEXCalls::call1(Operation::EXIT, static_cast<word_t>(code));
#else
        asm volatile ("jr %0" : : "r"(PEMUX_CODE_START));
#endif
        UNREACHED;
    }
};

EXTERN_C void init_env(Env *e) {
    m3::Heap::init();
    std::set_terminate(Exception::terminate_handler);
    e->backend_addr = reinterpret_cast<uint64_t>(new EnvUserBackend());
}

}

/*
 * Copyright (C) 2015-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <base/util/String.h>
#include <base/Env.h>
#include <base/KIF.h>
#include <base/PEDesc.h>

#include <m3/com/SendGate.h>
#include <m3/com/GateStream.h>

namespace m3 {

class Env;
class RecvGate;

class Syscalls {
    friend class Env;

public:
    static Syscalls &get() {
        return _inst;
    }

private:
    explicit Syscalls()
        : _gate(ObjCap::INVALID, 0, &RecvGate::syscall(), DTU::SYSC_SEP) {
    }

public:
    Errors::Code create_srv(capsel_t dst, capsel_t vpe, capsel_t rgate, const String &name);
    Errors::Code create_sess(capsel_t dst, capsel_t srv, word_t ident);
    Errors::Code create_rgate(capsel_t dst, int order, int msgorder);
    Errors::Code create_sgate(capsel_t dst, capsel_t rgate, label_t label, word_t credits);
    Errors::Code create_vgroup(capsel_t dst);
    Errors::Code create_vpe(const KIF::CapRngDesc &dst, capsel_t sgate, const String &name, PEDesc &pe,
                            epid_t sep, epid_t rep, uint flags, capsel_t kmem, capsel_t group);
    Errors::Code create_map(capsel_t dst, capsel_t vpe, capsel_t mgate, capsel_t first,
                            capsel_t pages, int perms);

    Errors::Code activate(capsel_t ep, capsel_t gate, goff_t addr);
    Errors::Code vpe_ctrl(capsel_t vpe, KIF::Syscall::VPEOp op, xfer_t arg);
    Errors::Code vpe_wait(const capsel_t *vpes, size_t count, event_t event,
                          capsel_t *vpe, int *exitcode);
    Errors::Code derive_mem(capsel_t vpe, capsel_t dst, capsel_t src, goff_t offset,
                            size_t size, int perms);
    Errors::Code derive_kmem(capsel_t kmem, capsel_t dst, size_t quota);
    Errors::Code kmem_quota(capsel_t kmem, size_t &amount);

    Errors::Code delegate(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                          KIF::ExchangeArgs *args = nullptr);
    Errors::Code obtain(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                        KIF::ExchangeArgs *args = nullptr);
    Errors::Code exchange(capsel_t vpe, const KIF::CapRngDesc &own, capsel_t other, bool obtain);
    Errors::Code revoke(capsel_t vpe, const KIF::CapRngDesc &crd, bool own = true);

    Errors::Code forward_msg(capsel_t sgate, capsel_t rgate, const void *msg, size_t len,
                             label_t rlabel, event_t event);
    Errors::Code forward_mem(capsel_t mgate, void *data, size_t len, goff_t offset, uint flags,
                             event_t event);
    Errors::Code forward_reply(capsel_t rgate, const void *msg, size_t len, goff_t msgaddr,
                               event_t event);

    Errors::Code noop();

    void exit(int exitcode);

private:
    DTU::Message *send_receive(const void *msg, size_t size);
    Errors::Code send_receive_result(const void *msg, size_t size);
    Errors::Code exchange_sess(capsel_t vpe, capsel_t sess, const KIF::CapRngDesc &crd,
                               KIF::ExchangeArgs *args, bool obtain);

    SendGate _gate;
    static Syscalls _inst;
};

}

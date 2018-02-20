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

#include <m3/com/GateStream.h>
#include <m3/session/Pager.h>

namespace m3 {

Errors::Code Pager::pagefault(uintptr_t addr, uint access) {
    GateIStream reply = send_receive_vmsg(_gate, PAGEFAULT, addr, access);
    Errors::Code res;
    reply >> res;
    return res;
}

Errors::Code Pager::map_anon(uintptr_t *virt, size_t len, int prot, int flags) {
    GateIStream reply = send_receive_vmsg(_gate, MAP_ANON, *virt, len, prot, flags);
    Errors::Code res;
    reply >> res;
    if(res != Errors::NONE)
        return res;
    reply >> *virt;
    return Errors::NONE;
}

Errors::Code Pager::map_ds(uintptr_t *virt, size_t len, int prot, int flags, const Session &sess,
        int fd, size_t offset) {
    xfer_t args[6];
    args[0] = DelOp::DATASPACE;
    args[1] = *virt;
    args[2] = len;
    args[3] = static_cast<xfer_t>(prot | flags);
    args[4] = static_cast<xfer_t>(fd);
    args[5] = offset;
    size_t argcount = ARRAY_SIZE(args);
    delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sess.sel()), &argcount, args);
    if(Errors::last != Errors::NONE)
        return Errors::last;
    *virt = args[0];
    return Errors::NONE;
}

Errors::Code Pager::map_mem(uintptr_t *virt, MemGate &mem, size_t len, int prot) {
    xfer_t args[4];
    args[0] = DelOp::MEMGATE;
    args[1] = *virt;
    args[2] = len;
    args[3] = static_cast<xfer_t>(prot);
    size_t argcount = ARRAY_SIZE(args);
    delegate(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, mem.sel()), &argcount, args);
    if(Errors::last != Errors::NONE)
        return Errors::last;
    *virt = args[0];
    return Errors::NONE;
}

Errors::Code Pager::unmap(uintptr_t virt) {
    GateIStream reply = send_receive_vmsg(_gate, UNMAP, virt);
    Errors::Code res;
    reply >> res;
    return res;
}

Pager *Pager::create_clone(VPE &vpe) {
    KIF::CapRngDesc caps;
    {
        caps = obtain(1);
        if(Errors::last != Errors::NONE)
            return nullptr;
    }
    return new Pager(vpe, caps.start());
}

Errors::Code Pager::clone() {
    GateIStream reply = send_receive_vmsg(_gate, CLONE);
    Errors::Code res;
    reply >> res;
    return res;
}

}

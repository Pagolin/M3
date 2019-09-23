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

#include <base/log/Kernel.h>

#include "pes/PEMux.h"
#include "pes/VPEManager.h"
#include "DTU.h"
#include "Platform.h"
#include "SyscallHandler.h"

namespace kernel {

PEMux::PEMux(peid_t pe)
    : _caps(VPE::INVALID_ID),
      _vpes(),
      _pe(pe),
      _headers(0),
      _header_start(),
      _rbufs_size(),
      _mem_base(),
      _dtustate() {
#if defined(__gem5__)
    // configure send EP
    _dtustate.config_send(m3::DTU::KPEX_SEP, reinterpret_cast<label_t>(this),
                          Platform::kernel_pe(), SyscallHandler::pexep(),
                          KPEX_RBUF_SIZE, KPEX_RBUF_SIZE);

    // configure receive EP
    _dtustate.config_recv(m3::DTU::KPEX_REP, Platform::def_recvbuf(_pe),
                          KPEX_RBUF_ORDER, KPEX_RBUF_ORDER, 0);
#endif

    for(epid_t ep = m3::DTU::FIRST_FREE_EP; ep < EP_COUNT; ++ep) {
        capsel_t sel = m3::KIF::FIRST_EP_SEL + ep - m3::DTU::FIRST_FREE_EP;
        _caps.set(sel, new EPCapability(&_caps, sel, new EPObject(_pe, ep)));
    }

    _headers += 1;
    _header_start = _headers;
}

static void reply_result(const m3::DTU::Message *msg, m3::Errors::Code code) {
    m3::KIF::DefaultReply reply;
    reply.error = static_cast<xfer_t>(code);
    DTU::get().reply(SyscallHandler::pexep(), &reply, sizeof(reply), msg);
}

void PEMux::handle_call(const m3::DTU::Message *msg) {
    auto req = reinterpret_cast<const m3::KIF::DefaultRequest*>(msg->data);
    auto op = static_cast<m3::KIF::PEMux::Operation>(req->opcode);

    switch(op) {
        case m3::KIF::PEMux::ACTIVATE:
            pexcall_activate(msg);
            break;

        default:
            reply_result(msg, m3::Errors::INV_ARGS);
            break;
    }
}

void PEMux::pexcall_activate(const m3::DTU::Message *msg) {
    auto req = reinterpret_cast<const m3::KIF::PEMux::Activate*>(msg->data);

    KLOG(PEXC, "PEXCall[" << _pe << "] activate(vpe=" << req->vpe_sel
        << ", gate=" << req->gate_sel << ", ep=" << req->ep
        << ", addr=" << m3::fmt(req->addr, "p") << ")");

    auto vpecap = static_cast<VPECapability*>(_caps.get(req->vpe_sel, Capability::VIRTPE));
    if(vpecap == nullptr) {
        reply_result(msg, m3::Errors::INV_ARGS);
        return;
    }

    auto epcap = static_cast<EPCapability*>(_caps.get(req->ep, Capability::EP));
    if(epcap == nullptr) {
        reply_result(msg, m3::Errors::INV_ARGS);
        return;
    }

    m3::Errors::Code res = vpecap->obj->activate(epcap, req->gate_sel, req->addr);
    reply_result(msg, res);
}

size_t PEMux::allocate_headers(size_t num) {
    // TODO really manage the header space and zero the headers first in case they are reused
    if(_headers + num > m3::DTU::HEADER_COUNT)
        return m3::DTU::HEADER_COUNT;
    _headers += num;
    return _headers - num;
}

bool PEMux::invalidate_ep(epid_t ep, bool force) {
    KLOG(EPS, "PE" << _pe << ":EP" << ep << " = invalid");

    return DTU::get().inval_ep_remote(desc(), ep, force) == m3::Errors::NONE;
}

void PEMux::invalidate_eps() {
    // no update on the PE here, since we don't save the state anyway
    _dtustate.invalidate_eps(m3::DTU::FIRST_FREE_EP);
}

m3::Errors::Code PEMux::config_rcv_ep(epid_t ep, RGateObject &obj) {
    // it needs to be in the receive buffer space
    const goff_t addr = Platform::def_recvbuf(_pe);
    const size_t size = Platform::pe(_pe).has_virtmem() ? RECVBUF_SIZE : RECVBUF_SIZE_SPM;
    // def_recvbuf() == 0 means that we do not validate it
    if(addr && (obj.addr < addr || obj.addr > addr + size || obj.addr + obj.size() > addr + size))
        return m3::Errors::INV_ARGS;
    if(obj.addr < addr + _rbufs_size)
        return m3::Errors::INV_ARGS;

    // no free headers left?
    size_t msgSlots = 1UL << (obj.order - obj.msgorder);
    size_t off = allocate_headers(msgSlots);
    if(off == m3::DTU::HEADER_COUNT)
        return m3::Errors::OUT_OF_MEM;

    obj.header = off;
    KLOG(EPS, "PE" << _pe << ":EP" << ep << " = "
        "RGate[addr=#" << m3::fmt(obj.addr, "x")
        << ", order=" << obj.order
        << ", msgorder=" << obj.msgorder
        << ", header=" << obj.header
        << "]");

    dtustate().config_recv(ep, rbuf_base() + obj.addr, obj.order, obj.msgorder, obj.header);
    update_ep(ep);

    m3::ThreadManager::get().notify(reinterpret_cast<event_t>(&obj));
    return m3::Errors::NONE;
}

m3::Errors::Code PEMux::config_snd_ep(epid_t ep, SGateObject &obj) {
    assert(obj.rgate->addr != 0);
    if(obj.activated)
        return m3::Errors::EXISTS;

    KLOG(EPS, "PE" << _pe << ":EP" << ep << " = "
        "Send[pe=" << obj.rgate->pe
        << ", ep=" << obj.rgate->ep
        << ", label=#" << m3::fmt(obj.label, "x")
        << ", msgsize=" << obj.rgate->msgorder
        << ", crd=#" << m3::fmt(obj.credits, "x")
        << "]");

    obj.activated = true;
    dtustate().config_send(ep, obj.label, obj.rgate->pe, obj.rgate->ep,
                           1UL << obj.rgate->msgorder, obj.credits);
    update_ep(ep);
    return m3::Errors::NONE;
}

m3::Errors::Code PEMux::config_mem_ep(epid_t ep, const MGateObject &obj, goff_t off) {
    if(off >= obj.size || obj.addr + off < off)
        return m3::Errors::INV_ARGS;

    KLOG(EPS, "PE" << _pe << ":EP" << ep << " = "
        "Mem [vpe=" << obj.vpe
        << ", pe=" << obj.pe
        << ", addr=#" << m3::fmt(obj.addr + off, "x")
        << ", size=#" << m3::fmt(obj.size - off, "x")
        << ", perms=#" << m3::fmt(obj.perms, "x")
        << "]");

    // TODO
    dtustate().config_mem(ep, obj.pe, obj.addr + off, obj.size - off, obj.perms);
    update_ep(ep);
    return m3::Errors::NONE;
}

void PEMux::update_ep(epid_t ep) {
    DTU::get().write_ep_remote(desc(), ep, dtustate().get_ep(ep));
}

}

/**
 * Copyright (C) 2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universität Dresden (Germany)
 *
 * This file is part of M3 (Microkernel for Minimalist Manycores).
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
#include <base/log/Services.h>

#include <m3/com/MemGate.h>
#include <m3/com/RecvGate.h>
#include <m3/com/SendGate.h>
#include <m3/server/RequestHandler.h>
#include <m3/server/Server.h>
#include <m3/session/LoadGen.h>
#include <m3/session/ServerSession.h>

using namespace m3;

static char http_req[] =
    "GET /index.html HTTP/1.0\r\n"
    "Host: localhost\r\n"
    "User-Agent: ApacheBench/2.3\r\n"
    "Accept: */*\r\n"
    "\r\n";

class LoadGenSession : public m3::ServerSession {
public:
    explicit LoadGenSession(RecvGate *rgate, size_t crt, capsel_t srv_sel)
        : m3::ServerSession(crt, srv_sel),
          rem_req(),
          clisgate(SendGate::create(rgate, SendGateArgs().label(ptr_to_label(this)).credits(1))),
          sgate(),
          mgate() {
    }

    void send_request() {
        if(rem_req > 0) {
            mgate->write(http_req, sizeof(http_req), 0);
            auto msg = create_vmsg(sizeof(http_req));
            sgate->send(msg.finish(), ptr_to_label(this));
            rem_req--;
        }
    }

    uint rem_req;
    SendGate clisgate;
    std::unique_ptr<SendGate> sgate;
    std::unique_ptr<MemGate> mgate;
};

class ReqHandler;
typedef RequestHandler<ReqHandler, LoadGen::Operation, LoadGen::COUNT, LoadGenSession> base_class_t;

class ReqHandler : public base_class_t {
public:
    static constexpr size_t MSG_SIZE = 64;
    static constexpr size_t BUF_SIZE = Server<ReqHandler>::MAX_SESSIONS * MSG_SIZE;

    explicit ReqHandler(WorkLoop *wl)
        : base_class_t(),
          _rgate(RecvGate::create(nextlog2<BUF_SIZE>::val, nextlog2<MSG_SIZE>::val)) {
        add_operation(LoadGen::START, &ReqHandler::start);
        add_operation(LoadGen::RESPONSE, &ReqHandler::response);

        using std::placeholders::_1;
        _rgate.start(wl, std::bind(&ReqHandler::handle_message, this, _1));
    }

    virtual Errors::Code open(LoadGenSession **sess, size_t crt, capsel_t srv_sel,
                              const std::string_view &) override {
        *sess = new LoadGenSession(&_rgate, crt, srv_sel);
        return Errors::SUCCESS;
    }

    virtual Errors::Code obtain(LoadGenSession *sess, size_t, CapExchange &xchg) override {
        if(xchg.in_caps() != 1)
            return Errors::INV_ARGS;

        SLOG(LOADGEN, "{:#x}: mem::get_sgate()"_cf, (word_t)sess);

        xchg.out_caps(KIF::CapRngDesc(KIF::CapRngDesc::OBJ, sess->clisgate.sel()));
        return Errors::SUCCESS;
    }

    virtual Errors::Code delegate(LoadGenSession *sess, size_t, CapExchange &xchg) override {
        if(xchg.in_caps() != 2 || sess->sgate)
            return Errors::INV_ARGS;

        SLOG(LOADGEN, "{:#x}: mem::create_chan()"_cf, (word_t)sess);

        KIF::CapRngDesc crd(KIF::CapRngDesc::OBJ, Activity::own().alloc_sels(2), 2);

        sess->sgate.reset(new SendGate(SendGate::bind(crd.start() + 0, &_rgate)));
        sess->mgate.reset(new MemGate(MemGate::bind(crd.start() + 1)));

        xchg.out_caps(crd);
        return Errors::SUCCESS;
    }

    virtual Errors::Code close(LoadGenSession *sess, size_t) override {
        delete sess;
        return Errors::SUCCESS;
    }

    virtual void shutdown() override {
        _rgate.stop();
    }

    void start(GateIStream &is) {
        LoadGenSession *sess = is.label<LoadGenSession *>();
        uint count;
        is >> count;
        sess->rem_req = count;

        SLOG(LOADGEN, "{:#x}: mem::start(count={})"_cf, (word_t)sess, count);

        sess->send_request();
        reply_vmsg(is, Errors::SUCCESS);
    }

    void response(GateIStream &is) {
        LoadGenSession *sess = is.label<LoadGenSession *>();
        size_t amount;
        is >> amount;

        SLOG(LOADGEN, "{:#x}: mem::response(amount={})"_cf, (word_t)sess, amount);

        sess->send_request();
    }

private:
    RecvGate _rgate;
};

int main(int argc, char **argv) {
    WorkLoop wl;

    const char *name = argc > 1 ? argv[1] : "loadgen";
    Server<ReqHandler> srv(name, &wl, std::make_unique<ReqHandler>(&wl));

    wl.run();
    return 0;
}

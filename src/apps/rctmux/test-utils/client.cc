/**
* Copyright (C) 2015, Nils Asmussen <nils@os.inf.tu-dresden.de>
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
#include <base/util/Sync.h>
#include <base/util/Profile.h>
#include <base/Panic.h>

#include <m3/stream/Standard.h>
#include <m3/vfs/Executable.h>
#include <m3/vfs/VFS.h>
#include <m3/Syscalls.h>
#include <m3/VPE.h>

using namespace m3;

int main() {
    cout << "Mounting filesystem...\n";
    if(VFS::mount("/", new M3FS("m3fs")) < 0)
        PANIC("Cannot mount root fs");

    const char *args1[] = {"/bin/rctmux-util-service", "srv1"};
    const char *args2[] = {"/bin/rctmux-util-service", "srv2"};

    // start the first vpe
    VPE s1(args1[0], VPE::self().pe(), "pager", true);
    s1.mountspace(*VPE::self().mountspace());
    s1.obtain_mountspace();
    Executable exec1(ARRAY_SIZE(args1), args1);
    Errors::Code res1 = s1.exec(exec1);
    if(res1 != Errors::NO_ERROR)
        PANIC("Cannot execute " << args1[0] << ": " << Errors::to_string(res1));

    // start the second VPE
    VPE s2(args2[0], VPE::self().pe(), "pager", true);
    s2.mountspace(*VPE::self().mountspace());
    s2.obtain_mountspace();
    Executable exec2(ARRAY_SIZE(args2), args2);
    Errors::Code res2 = s2.exec(exec2);
    if(res2 != Errors::NO_ERROR)
        PANIC("Cannot execute " << args2[0] << ": " << Errors::to_string(res2));

    enum TestOp {
        TEST
    };

    cout << "Starting session creation...\n";

    Session *sess[2];
    SendGate *sgate[2];
    const char *name[2];

    for(int i = 0; i < 2; ++i) {
        name[i] = i == 0 ? "srv1" : "srv2";

        // the kernel does not block us atm until the service is available
        // so try to connect until it's available
        while(sess[i] == nullptr) {
            sess[i] = new Session(name[i]);
            if(sess[i]->is_connected())
                break;

            for(volatile int x = 0; x < 10000; ++x)
                ;
            delete sess[i];
            sess[i] = nullptr;
        }

        sgate[i] = new SendGate(SendGate::bind(sess[i]->obtain(1).start()));
    }

    cout << "Starting test...\n";

    for(int i = 0; i < 5; ++i) {
        size_t no = i % 2;
        int res;
        GateIStream reply = send_receive_vmsg(*sgate[no], TEST);
        reply >> res;
        cout << "Got " << res << " from " << name[no] << "\n";
    }

    cout << "Test finished.\n";

    for(int i = 0; i < 2; ++i) {
        delete sgate[i];
        delete sess[i];
    }

    return 0;
}

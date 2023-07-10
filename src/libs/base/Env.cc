/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2022 Nils Asmussen, Barkhausen Institut
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

#include <base/CPU.h>
#include <base/Common.h>
#include <base/Env.h>
#include <base/stream/Serial.h>

#include <exception>
#include <functional>
#include <stdlib.h>

typedef void (*constr_func)();

extern constr_func CTORS_BEGIN;
extern constr_func CTORS_END;

EXTERN_C void __m3_init_libc(int argc, char **argv, char **envp, int tls);
EXTERN_C void __m3_set_args(char **argv, char **envp);
EXTERN_C void __cxa_finalize(void *);
EXTERN_C void _init();

namespace m3 {

static char **rewrite_args(uint64_t *args, int count) {
    char **nargs = new char *[count + 1];
    for(int i = 0; i < count; ++i)
        nargs[i] = reinterpret_cast<char *>(args[i]);
    nargs[count] = nullptr;
    return nargs;
}

std::pair<int, char **> init(bool tls) {
    BootEnv *e = bootenv();

    int argc = static_cast<int>(e->argc);
    char **argv = reinterpret_cast<char **>(e->argv);
    char **envp = reinterpret_cast<char **>(e->envp);
    if(sizeof(char *) != sizeof(uint64_t)) {
        // ensure that the libc is initialized before the first malloc
        __m3_init_libc(0, nullptr, nullptr, tls);
        uint64_t *envp64 = reinterpret_cast<uint64_t *>(e->envp);
        int envcnt = 0;
        for(; envp64 && *envp64; envcnt++)
            envp64++;
        envp = rewrite_args(reinterpret_cast<uint64_t *>(e->envp), envcnt);
        argv = rewrite_args(reinterpret_cast<uint64_t *>(e->argv), argc);
        __m3_set_args(argv, envp);
    }
    else
        __m3_init_libc(argc, argv, envp, tls);

    // call constructors
    _init();
    for(constr_func *func = &CTORS_BEGIN; func < &CTORS_END; ++func)
        (*func)();

    return std::make_pair(argc, argv);
}

void deinit() {
    __cxa_finalize(nullptr);
}

}

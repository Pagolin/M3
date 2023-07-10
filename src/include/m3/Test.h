/*
 * Copyright (C) 2019 Nils Asmussen, Barkhausen Institut
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

#include <m3/stream/Standard.h>

extern int failed;

#define WVPERF(name, bench) \
    m3::println("! {}:{}  PERF \"{}\": {}"_cf, __FILE__, __LINE__, name, bench)

#define WVASSERT(val)                                                                           \
    ({                                                                                          \
        if(!(val)) {                                                                            \
            failed++;                                                                           \
            m3::println("! {}:{}  expected true, got {} (false) FAILED"_cf, __FILE__, __LINE__, \
                        #val);                                                                  \
        }                                                                                       \
    })

#define WVASSERTEQ(a, b)                                                                    \
    ({                                                                                      \
        auto _a = a;                                                                        \
        auto _b = b;                                                                        \
        if(_a != _b) {                                                                      \
            failed++;                                                                       \
            m3::println("! {}:{}  \"{}\" == \"{}\" FAILED"_cf, __FILE__, __LINE__, _a, _b); \
        }                                                                                   \
    })

#define WVASSERTSTREQ(a, b)                                                                 \
    ({                                                                                      \
        auto _a = (const char *)a;                                                          \
        auto _b = (const char *)b;                                                          \
        if((_a == nullptr) != (_b == nullptr) || (_a && strcmp(_a, _b) != 0)) {             \
            failed++;                                                                       \
            m3::println("! {}:{}  \"{}\" == \"{}\" FAILED"_cf, __FILE__, __LINE__, _a, _b); \
        }                                                                                   \
    })

static inline void WVASSERTERR(m3::Errors::Code err, std::function<void()> func) {
    using namespace m3;
    try {
        func();
        WVASSERT(false);
    }
    catch(const Exception &e) {
        WVASSERTEQ(e.code(), err);
    }
}

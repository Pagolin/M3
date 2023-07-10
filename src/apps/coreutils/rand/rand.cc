/*
 * Copyright (C) 2015-2018 Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
 *
 * Copyright (C) 2019-2021 Nils Asmussen, Barkhausen Institut
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
#include <base/stream/IStringStream.h>
#include <base/util/Random.h>

#include <m3/stream/Standard.h>

#include "loop.h"

using namespace m3;

alignas(64) static rand_type buffer[EL_COUNT];

int main(int argc, char **argv) {
    if(argc != 2)
        exitmsg("Usage: {} <count>"_cf, argv[0]);

    size_t count = IStringStream::read_from<size_t>(argv[1]);

    while(count > 0) {
        size_t amount = Math::min(count, ARRAY_SIZE(buffer));

        CPU::compute(amount * 8);
        // generate(buffer, amount);
        cout.write_all(buffer, amount * sizeof(rand_type));

        count -= amount;
    }
    return 0;
}

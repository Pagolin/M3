/*
 * Copyright (C) 2015-2016, Nils Asmussen <nils@os.inf.tu-dresden.de>
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

#include <m3/stream/Standard.h>
#include <stdio.h>

#include "libctest.h"

int failed;

int main() {
    RUN_SUITE(tdir);
    RUN_SUITE(tfile);

    if(failed > 0)
        m3::cout << "\033[1;31m" << failed << " tests failed\033[0;m\n";
    else
        m3::cout << "\033[1;32mAll tests successful!\033[0;m\n";
    return 0;
}

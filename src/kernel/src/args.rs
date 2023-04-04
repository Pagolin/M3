/*
 * Copyright (C) 2020-2021 Nils Asmussen, Barkhausen Institut
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

use base::cell::{LazyStaticRefCell, Ref};
use base::cfg;
use base::col::Vec;
use base::env;

#[derive(Default)]
pub struct Args {
    pub kmem: usize,
}

static ARGS: LazyStaticRefCell<Args> = LazyStaticRefCell::default();

pub fn get() -> Ref<'static, Args> {
    ARGS.borrow()
}

pub fn parse() {
    let mut args = Args {
        kmem: 64 * 1024 * 1024,
    };

    let mut i = 1;
    let argv: Vec<&str> = env::args().collect();
    while i < argv.len() {
        if argv[i] == "-m" {
            let size_str = argv.get(i + 1).unwrap_or_else(|| usage());
            let size = parse_size(size_str).unwrap_or_else(|| usage());
            if size <= cfg::FIXED_KMEM {
                usage();
            }
            args.kmem = size;
            i += 1;
        }
        i += 1;
    }

    ARGS.set(args);
}

fn usage() -> ! {
    panic!(
        "\nUsage: {} [-m <kmem>]
          -m: the kernel memory size (> FIXED_KMEM)",
        env::args().next().unwrap()
    );
}

fn parse_size(s: &str) -> Option<usize> {
    let mul = match s.chars().last() {
        Some(c) if c >= '0' && c <= '9' => 1,
        Some('k') | Some('K') => 1024,
        Some('m') | Some('M') => 1024 * 1024,
        Some('g') | Some('G') => 1024 * 1024 * 1024,
        _ => return None,
    };
    Some(match mul {
        1 => s.parse::<u64>().ok()? as usize,
        m => m * s[0..s.len() - 1].parse::<u64>().ok()? as usize,
    })
}

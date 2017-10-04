use core::intrinsics;
use core::slice;
use libc;

// TODO move to proper place
pub fn jmp_to(addr: u64) {
    unsafe {
        asm!(
            "jmp *$0"
            : : "r"(addr)
        );
    }
}

pub fn size_of<T>() -> usize {
    unsafe {
        intrinsics::size_of::<T>()
    }
}

pub fn size_of_val<T: ?Sized>(val: &T) -> usize {
    unsafe {
        intrinsics::size_of_val(val)
    }
}

pub fn cstr_to_str(s: *const u8) -> &'static str {
    unsafe {
        let len = libc::strlen(s);
        let sl = slice::from_raw_parts(s, len as usize + 1);
        intrinsics::transmute(&sl[..sl.len() - 1])
    }
}

fn _next_log2(size: usize, shift: i32) -> i32 {
    if size > (1 << shift) {
        shift + 1
    }
    else if shift == 0 {
        0
    }
    else {
        _next_log2(size, shift - 1)
    }
}

pub fn next_log2(size: usize) -> i32 {
    _next_log2(size, (size_of::<usize>() * 8 - 2) as i32)
}

// TODO make these generic
pub fn round_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

pub fn round_dn(value: usize, align: usize) -> usize {
    value & !(align - 1)
}

pub fn min<T: Ord>(a: T, b: T) -> T {
    if a > b {
        b
    }
    else {
        a
    }
}

pub fn max<T: Ord>(a: T, b: T) -> T {
    if a > b {
        b
    }
    else {
        a
    }
}

/*
 * Copyright (C) 2016-2018, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Economic rights: Technische Universitaet Dresden (Germany)
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

#pragma once

#include <base/Common.h>
#include <base/Config.h>

namespace m3 {

/* the stack frame for the interrupt-handler */
struct ExceptionState {
    /* general purpose registers */
    ulong r15;
    ulong r14;
    ulong r13;
    ulong r12;
    ulong r11;
    ulong r10;
    ulong r9;
    ulong r8;
    ulong rbp;
    ulong rsi;
    ulong rdi;
    ulong rdx;
    ulong rcx;
    ulong rbx;
    ulong rax;
    /* interrupt-number */
    ulong intrptNo;
    /* error-code (for exceptions); default = 0 */
    ulong errorCode;
    /* pushed by the CPU */
    ulong rip;
    ulong cs;
    ulong rflags;
    ulong rsp;
    ulong ss;
} PACKED;

class ISRBase {
    ISRBase() = delete;

protected:
    /* the descriptor table */
    struct DescTable {
        uint16_t size;      /* the size of the table -1 (size=0 is not allowed) */
        ulong offset;
    } PACKED;

    /* a descriptor */
    struct Desc {
        enum {
            SYS_TASK_GATE   = 0x05,
            SYS_TSS         = 0x09,
            SYS_INTR_GATE   = 0x0E,
            DATA_RO         = 0x10,
            DATA_RW         = 0x12,
            CODE_X          = 0x18,
            CODE_XR         = 0x1A,
        };

        enum {
            DPL_KERNEL      = 0x0,
            DPL_USER        = 0x3,
        };

        enum {
            BITS_32         = 0 << 5,
            BITS_64         = 1 << 5,
        };

        /**
         * size:        If 0 the selector defines 16 bit protected mode. If 1 it defines 32 bit
         *              protected mode. You can have both 16 bit and 32 bit selectors at once.
         */
        enum {
            SIZE_16         = 0 << 6,
            SIZE_32         = 1 << 6,
        };

        /**
         * granularity: If 0 the limit is in 1 B blocks (byte granularity), if 1 the limit is in
         *              4 KiB blocks (page granularity).
         */
        enum {
            GRANU_BYTES     = 0 << 7,
            GRANU_PAGES     = 1 << 7,
        };

        /* limit[0..15] */
        uint16_t limitLow;

        /* address[0..15] */
        uint16_t addrLow;

        /* address[16..23] */
        uint8_t addrMiddle;

        /*
         * present:     This must be 1 for all valid selectors.
         * dpl:         Contains the ring level, 0 = highest (kernel), 3 = lowest (user applications).
         * type:        segment type
         */
        uint8_t type : 5,
                dpl : 2,
                present : 1;

        /* address[24..31] and other fields, depending on the type of descriptor */
        uint16_t addrHigh;
    } PACKED;

    /* only on x86_64 */
    struct Desc64 : public Desc {
        uint32_t addrUpper;
        uint32_t : 32;
    };

    /* the Task State Segment */
    struct TSS {
        uint32_t : 32; /* reserved */
        uint64_t rsp0;
        uint32_t fields[11];
        uint16_t : 16; /* reserved */

        /* Contains a 16-bit offset from the base of the TSS to the I/O permission bit map
         * and interrupt redirection bitmap. When present, these maps are stored in the
         * TSS at higher addresses. The I/O map base address points to the beginning of the
         * I/O permission bit map and the end of the interrupt redirection bit map. */
        uint16_t ioMapOffset;
    } PACKED;

    typedef void (*entry_func)();

    /* we need 5 entries: null-entry, code+data for kernel/user, 2 for TSS (on x86_64) */
    static const size_t GDT_ENTRY_COUNT = 7;

public:
    static const size_t ISR_COUNT       = 66;

    static const size_t PEX_ISR         = 63;
    static const size_t DTU_ISR         = 64;

    /* segments numbers */
    enum {
        SEG_KCODE          = 1,
        SEG_KDATA          = 2,
        SEG_UCODE          = 3,
        SEG_UDATA          = 4,
        SEG_TSS            = 5,
    };

protected:
    static void load_idt(DescTable *tbl) {
        asm volatile ("lidt %0" : : "m"(*tbl));
    }
    static void get_idt(DescTable *tbl) {
        asm volatile ("sidt %0" : : "m"(*tbl));
    }
    static void load_tss(size_t gdtOffset) {
        asm volatile ("ltr %0" : : "m"(gdtOffset));
    }
    static void load_gdt(DescTable *gdt) {
        asm volatile ("lgdt (%0)" : : "r"(gdt));
    }

    static void set_desc(Desc *d,uintptr_t address,size_t limit,uint8_t granu,uint8_t type,uint8_t dpl);
    static void set_desc64(Desc *d,uintptr_t address,size_t limit,uint8_t granu,uint8_t type,uint8_t dpl);
    static void set_idt(size_t number,entry_func handler,uint8_t dpl);
    static void set_tss(Desc *gdt,TSS *tss,uintptr_t kstack);

    static Desc gdt[GDT_ENTRY_COUNT];
    static Desc64 idt[ISR_COUNT];
    static TSS tss;
    static Desc64 *idt_p;
};

}

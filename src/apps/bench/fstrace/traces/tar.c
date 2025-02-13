// This file has been automatically generated by strace2c.
// Do not edit it!

#include "../op_types.h"

trace_op_t trace_ops_tar[] = {
    /* #1 = 0x1 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 169674 } },
    /* #2 = 0x2 */ { .opcode = OPEN_OP, .args.open = { 3, "/tmp/test.tar", O_WRONLY|O_CREAT|O_TRUNC, 0666 } },
    /* #3 = 0x3 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 3862 } },
    /* #4 = 0x4 */ { .opcode = FSTAT_OP, .args.fstat = { 0, 3 } },
    /* #5 = 0x5 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 4866 } },
    /* #6 = 0x6 */ { .opcode = FSTATAT_OP, .args.fstatat = { 0, "/tardata/tar-16m" } },
    /* #7 = 0x7 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 70276 } },
    /* #8 = 0x8 */ { .opcode = OPEN_OP, .args.open = { 4, "/etc/passwd", O_RDONLY, 0 } },
    /* #9 = 0x9 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 22320 } },
    /* #10 = 0xa */ { .opcode = READ_OP, .args.read = { 340, 4, 4096, 1 } },
    /* #11 = 0xb */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 5197 } },
    /* #12 = 0xc */ { .opcode = CLOSE_OP, .args.close = { 0, 4 } },
    /* #13 = 0xd */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 23094 } },
    /* #14 = 0xe */ { .opcode = OPEN_OP, .args.open = { 4, "/etc/group", O_RDONLY, 0 } },
    /* #15 = 0xf */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 2923 } },
    /* #16 = 0x10 */ { .opcode = READ_OP, .args.read = { 307, 4, 4096, 1 } },
    /* #17 = 0x11 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 2113 } },
    /* #18 = 0x12 */ { .opcode = CLOSE_OP, .args.close = { 0, 4 } },
    /* #19 = 0x13 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 11426 } },
    /* #20 = 0x14 */ { .opcode = WRITE_OP, .args.write = { 512, 3, 512, 1 } },
    /* #21 = 0x15 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 4460 } },
    /* #22 = 0x16 */ { .opcode = OPEN_OP, .args.open = { 4, "/tardata/tar-16m", O_RDONLY|O_NONBLOCK|O_DIRECTORY|O_CLOEXEC, 0 } },
    /* #23 = 0x17 */ { .opcode = FSTAT_OP, .args.fstat = { 0, 4 } },
    /* #24 = 0x18 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 6759 } },
    /* #25 = 0x19 */ { .opcode = GETDENTS_OP, .args.getdents = { 272, 4, 9, 1024 } },
    /* #26 = 0x1a */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 23883 } },
    /* #27 = 0x1b */ { .opcode = FSTATAT_OP, .args.fstatat = { 0, "/tardata/tar-16m/4096.bin" } },
    /* #28 = 0x1c */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 2399 } },
    /* #29 = 0x1d */ { .opcode = OPEN_OP, .args.open = { 5, "/tardata/tar-16m/4096.bin", O_RDONLY, 0 } },
    /* #30 = 0x1e */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 24119 } },
    /* #31 = 0x1f */ { .opcode = WRITE_OP, .args.write = { 512, 3, 512, 1 } },
    /* #32 = 0x20 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 4343 } },
    /* #33 = 0x21 */ { .opcode = SENDFILE_OP, .args.sendfile = { 4194304, 3, 5, NULL, 4194304 } },
    /* #34 = 0x22 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 15657 } },
    /* #35 = 0x23 */ { .opcode = CLOSE_OP, .args.close = { 0, 5 } },
    /* #36 = 0x24 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 71488 } },
    /* #37 = 0x25 */ { .opcode = FSTATAT_OP, .args.fstatat = { 0, "/tardata/tar-16m/128.bin" } },
    /* #38 = 0x26 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 7825 } },
    /* #39 = 0x27 */ { .opcode = OPEN_OP, .args.open = { 5, "/tardata/tar-16m/128.bin", O_RDONLY, 0 } },
    /* #40 = 0x28 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 36120 } },
    /* #41 = 0x29 */ { .opcode = WRITE_OP, .args.write = { 512, 3, 512, 1 } },
    /* #42 = 0x2a */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 1832 } },
    /* #43 = 0x2b */ { .opcode = SENDFILE_OP, .args.sendfile = { 131072, 3, 5, NULL, 131072 } },
    /* #44 = 0x2c */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 3738 } },
    /* #45 = 0x2d */ { .opcode = CLOSE_OP, .args.close = { 0, 5 } },
    /* #46 = 0x2e */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 62922 } },
    /* #47 = 0x2f */ { .opcode = FSTATAT_OP, .args.fstatat = { 0, "/tardata/tar-16m/512.bin" } },
    /* #48 = 0x30 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 6623 } },
    /* #49 = 0x31 */ { .opcode = OPEN_OP, .args.open = { 5, "/tardata/tar-16m/512.bin", O_RDONLY, 0 } },
    /* #50 = 0x32 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 33646 } },
    /* #51 = 0x33 */ { .opcode = WRITE_OP, .args.write = { 512, 3, 512, 1 } },
    /* #52 = 0x34 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 1091 } },
    /* #53 = 0x35 */ { .opcode = SENDFILE_OP, .args.sendfile = { 524288, 3, 5, NULL, 524288 } },
    /* #54 = 0x36 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 4004 } },
    /* #55 = 0x37 */ { .opcode = CLOSE_OP, .args.close = { 0, 5 } },
    /* #56 = 0x38 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 66373 } },
    /* #57 = 0x39 */ { .opcode = FSTATAT_OP, .args.fstatat = { 0, "/tardata/tar-16m/1024.bin" } },
    /* #58 = 0x3a */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 6727 } },
    /* #59 = 0x3b */ { .opcode = OPEN_OP, .args.open = { 5, "/tardata/tar-16m/1024.bin", O_RDONLY, 0 } },
    /* #60 = 0x3c */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 32816 } },
    /* #61 = 0x3d */ { .opcode = WRITE_OP, .args.write = { 512, 3, 512, 1 } },
    /* #62 = 0x3e */ { .opcode = SENDFILE_OP, .args.sendfile = { 1048576, 3, 5, NULL, 1048576 } },
    /* #63 = 0x3f */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 4485 } },
    /* #64 = 0x40 */ { .opcode = CLOSE_OP, .args.close = { 0, 5 } },
    /* #65 = 0x41 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 66372 } },
    /* #66 = 0x42 */ { .opcode = FSTATAT_OP, .args.fstatat = { 0, "/tardata/tar-16m/8192.bin" } },
    /* #67 = 0x43 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 6932 } },
    /* #68 = 0x44 */ { .opcode = OPEN_OP, .args.open = { 5, "/tardata/tar-16m/8192.bin", O_RDONLY, 0 } },
    /* #69 = 0x45 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 32261 } },
    /* #70 = 0x46 */ { .opcode = WRITE_OP, .args.write = { 512, 3, 512, 1 } },
    /* #71 = 0x47 */ { .opcode = SENDFILE_OP, .args.sendfile = { 8388608, 3, 5, NULL, 8388608 } },
    /* #72 = 0x48 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 6928 } },
    /* #73 = 0x49 */ { .opcode = CLOSE_OP, .args.close = { 0, 5 } },
    /* #74 = 0x4a */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 70932 } },
    /* #75 = 0x4b */ { .opcode = FSTATAT_OP, .args.fstatat = { 0, "/tardata/tar-16m/256.bin" } },
    /* #76 = 0x4c */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 8187 } },
    /* #77 = 0x4d */ { .opcode = OPEN_OP, .args.open = { 5, "/tardata/tar-16m/256.bin", O_RDONLY, 0 } },
    /* #78 = 0x4e */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 33895 } },
    /* #79 = 0x4f */ { .opcode = WRITE_OP, .args.write = { 512, 3, 512, 1 } },
    /* #80 = 0x50 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 1697 } },
    /* #81 = 0x51 */ { .opcode = SENDFILE_OP, .args.sendfile = { 262144, 3, 5, NULL, 262144 } },
    /* #82 = 0x52 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 3999 } },
    /* #83 = 0x53 */ { .opcode = CLOSE_OP, .args.close = { 0, 5 } },
    /* #84 = 0x54 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 66161 } },
    /* #85 = 0x55 */ { .opcode = FSTATAT_OP, .args.fstatat = { 0, "/tardata/tar-16m/2048.bin" } },
    /* #86 = 0x56 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 6687 } },
    /* #87 = 0x57 */ { .opcode = OPEN_OP, .args.open = { 5, "/tardata/tar-16m/2048.bin", O_RDONLY, 0 } },
    /* #88 = 0x58 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 33656 } },
    /* #89 = 0x59 */ { .opcode = WRITE_OP, .args.write = { 512, 3, 512, 1 } },
    /* #90 = 0x5a */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 1032 } },
    /* #91 = 0x5b */ { .opcode = SENDFILE_OP, .args.sendfile = { 2097152, 3, 5, NULL, 2097152 } },
    /* #92 = 0x5c */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 4785 } },
    /* #93 = 0x5d */ { .opcode = CLOSE_OP, .args.close = { 0, 5 } },
    /* #94 = 0x5e */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 20985 } },
    /* #95 = 0x5f */ { .opcode = GETDENTS_OP, .args.getdents = { 0, 4, 0, 1024 } },
    /* #96 = 0x60 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 13490 } },
    /* #97 = 0x61 */ { .opcode = CLOSE_OP, .args.close = { 0, 4 } },
    /* #98 = 0x62 */ { .opcode = WAITUNTIL_OP, .args.waituntil = { 0, 12214 } },
    /* #99 = 0x63 */ { .opcode = WRITE_OP, .args.write = { 1024, 3, 1024, 1 } },
    /* #100 = 0x64 */ { .opcode = CLOSE_OP, .args.close = { 0, 3 } },
    { .opcode = INVALID_OP }
};

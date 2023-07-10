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

#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <fs/internal.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/dir.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

// undo stupid definition
#undef direct

#define DEBUG 0

#if DEBUG
#    define PRINT(fmt, ...) printf(fmt, ##__VA_ARGS__)
#else
#    define PRINT(...)
#endif

template<typename T>
static constexpr T round_up(T value, T align) {
    return (value + align - 1) & ~(align - 1);
}

enum {
    MAX_BLOCKS = 1024 * 1024,
    MAX_INODES = 4096,
};

m3::SuperBlock sb;
FILE *file;

static m3::inodeno_t next_ino = 0;
static m3::blockno_t last_block = 0;

static m3::Bitmap *block_bitmap;
static m3::Bitmap *inode_bitmap;

static uint blks_per_extent;
static bool use_rand;

static m3::blockno_t alloc_block(bool new_ext) {
    m3::blockno_t blk;
    if(sb.free_blocks == 0)
        errx(1, "Not enough blocks");

    // distribute most blocks randomly in memory, but put some directly after another
    if(!new_ext && last_block + 1 < sb.total_blocks && !block_bitmap->is_set(last_block + 1))
        blk = last_block + 1;
    else {
        blk = last_block + 1;
        while(block_bitmap->is_set(blk)) {
            if(use_rand) {
                size_t size = sb.total_blocks - sb.first_data_block();
                blk = (static_cast<size_t>(rand()) % size) + sb.first_data_block();
            }
            else
                blk++;
        }
    }

    PRINT("Allocated block %u\n", blk);

    last_block = blk;
    block_bitmap->set(blk);
    sb.free_blocks--;
    return blk;
}

static bool append_to_extent(m3::INode *ino, m3::Extent *extent, m3::blockno_t bno, bool new_ext) {
    if(extent->length == 0) {
        extent->start = bno;
        extent->length = 1;
        ino->extents++;
        return true;
    }
    if(!new_ext && bno == extent->start + extent->length) {
        extent->length++;
        return true;
    }
    return false;
}

static bool create_indir_block(m3::INode *ino, m3::blockno_t *indir, uint i, m3::blockno_t bno,
                               int level, uint div, bool new_ext) {
    m3::Extent *extents = new m3::Extent[sb.extents_per_block()];
    if(*indir == 0) {
        *indir = alloc_block(false);
        memset(extents, 0, sb.blocksize);
    }
    else
        read_from_block(extents, sb.blocksize, *indir);

    bool res;
    if(level == 0) {
        assert(i < sb.extents_per_block());
        res = append_to_extent(ino, extents + i, bno, new_ext);
    }
    else {
        res = create_indir_block(ino, &(extents[i / div].start), i % div, bno, level - 1,
                                 div / sb.extents_per_block(), new_ext);
        extents[i / div].length = 1;
    }

    write_to_block(extents, sb.blocksize, *indir);
    delete[] extents;
    return res;
}

static m3::blockno_t store_blockno(const char *path, m3::INode *ino, m3::blockno_t bno,
                                   bool new_ext) {
    uint i = ino->extents == 0 ? 0 : ino->extents - 1;
    // if the block number does not fit into the last extent, try the next one (this will always
    // be empty and thus we can use it)
    for(bool res = false; !res; i++) {
        if(i < m3::INODE_DIR_COUNT)
            res = append_to_extent(ino, ino->direct + i, bno, new_ext);
        else if(i < m3::INODE_DIR_COUNT + sb.extents_per_block()) {
            res = create_indir_block(ino, &ino->indirect, i - m3::INODE_DIR_COUNT, bno, 0, 1,
                                     new_ext);
        }
        else if(i < m3::INODE_DIR_COUNT + sb.extents_per_block() +
                        sb.extents_per_block() * sb.extents_per_block()) {
            res = create_indir_block(ino, &ino->dindirect,
                                     i - (m3::INODE_DIR_COUNT + sb.extents_per_block()), bno, 1,
                                     sb.extents_per_block(), new_ext);
        }
        else {
            errx(1, "File '%s' is too large. Max no. of extents is %u", path,
                 m3::INODE_DIR_COUNT + sb.extents_per_block() +
                     sb.extents_per_block() * sb.extents_per_block());
        }
    }
    ino->size += sb.blocksize;
    return bno;
}

static m3::DirEntry *write_dirent(m3::INode *dir, m3::DirEntry *prev, const char *path,
                                  const char *name, m3::inodeno_t inode, size_t &off,
                                  m3::blockno_t &block) {
    size_t len = strlen(name);
    // all entries should be 4-byte aligned
    size_t total = sizeof(m3::DirEntry) + round_up(len, static_cast<size_t>(4));
    if(off + total > sb.blocksize) {
        size_t namelen = round_up(static_cast<size_t>(prev->namelen), static_cast<size_t>(4));
        size_t prevlen = sizeof(m3::DirEntry) + namelen;
        prev->next += sb.blocksize - off;
        write_to_block(prev, prevlen, block, off - prevlen);

        bool new_ext = blks_per_extent > 0 && ((dir->size / sb.blocksize) % blks_per_extent) == 0;
        block = store_blockno(path, dir, alloc_block(new_ext), new_ext);
        off = 0;
    }

    m3::DirEntry *entry = (m3::DirEntry *)malloc(total);
    if(!entry)
        err(1, "malloc failed");
    entry->nodeno = inode;
    entry->namelen = len;
    entry->next = total;
    memcpy(entry->name, name, len);

    PRINT("Writing dir-entry %s/%s to %u+%zu\n", path, name, block, off);

    write_to_block(entry, total, block, off);
    off += total;
    return entry;
}

static m3::inodeno_t copy(const char *path, m3::inodeno_t parent, int level) {
    static char buffer[m3::MAX_BLOCK_SIZE];
    struct stat st;
    int fd = open(path, O_RDONLY);
    if(fd < 0)
        err(1, "open of '%s' failed", path);
    if(fstat(fd, &st) != 0)
        err(1, "stat of '%s' failed", path);
    if(level == 0 && !S_ISDIR(st.st_mode))
        errx(1, "'%s' is no directory", path);

    if(sb.free_inodes == 0)
        errx(1, "Not enough inodes");

    m3::INode ino;
    ino.devno = 0;
    ino.inode = next_ino++;
    // TODO don't copy the number of links
    ino.links = st.st_nlink;
    ino.mode = st.st_mode;
    ino.lastaccess = static_cast<m3::time_t>(st.st_atime);
    ino.lastmod = static_cast<m3::time_t>(st.st_mtime);
    ino.size = 0;
    for(int i = 0; i < m3::INODE_DIR_COUNT; ++i)
        ino.direct[i].start = ino.direct[i].length = 0;
    ino.indirect = 0;
    ino.dindirect = 0;
    ino.extents = 0;

    inode_bitmap->set(ino.inode);
    sb.free_inodes--;

    if(S_ISREG(ino.mode)) {
        ssize_t len;
        for(size_t i = 0; (len = read(fd, buffer, sb.blocksize)) > 0; i++) {
            bool new_ext = blks_per_extent > 0 && (i % blks_per_extent) == 0;
            m3::blockno_t bno = store_blockno(path, &ino, alloc_block(new_ext), new_ext);
            PRINT("Writing block %zu of %s to block %u\n", i, path, bno);
            write_to_block(buffer, static_cast<size_t>(len), bno);
        }
        ino.size = static_cast<uint64_t>(st.st_size);
    }
    else if(S_ISDIR(ino.mode)) {
        DIR *d = opendir(path);
        if(!d)
            err(1, "opendir of '%s' failed", path);

        struct dirent *e;
        size_t diroff = 0;
        m3::DirEntry *prev = nullptr, *newent = nullptr;
        m3::blockno_t block = alloc_block(false);
        ino.size = sb.blocksize;

        ino.extents = 1;
        ino.direct[0].start = block;
        ino.direct[0].length = 1;

        while((e = readdir(d))) {
            if(newent) {
                free(prev);
                prev = newent;
            }

            m3::inodeno_t inode;
            if(strcmp(e->d_name, ".") == 0)
                inode = ino.inode;
            else if(strcmp(e->d_name, "..") == 0)
                inode = parent;
            else {
                char *epath = new char[strlen(path) + strlen(e->d_name) + 2];
                sprintf(epath, "%s/%s", path, e->d_name);
                inode = copy(epath, ino.inode, level + 1);
                delete[] epath;
            }

            newent = write_dirent(&ino, prev, path, e->d_name, inode, diroff, block);
        }

        // set next of last entry to the end of the block
        size_t newentlen = newent->next;
        newent->next += sb.blocksize - diroff;
        write_to_block(newent, newentlen, block, diroff - newentlen);

        free(newent);
        free(prev);
        closedir(d);
    }
    else
        fprintf(stderr, "Warning: ignored file '%s' (no regular file or directory)\n", path);
    close(fd);

    // write inode
    write_to_block(&ino, sizeof(ino), sb.first_inode_block(), ino.inode * sizeof(m3::INode));
    return ino.inode;
}

int main(int argc, char **argv) {
    if(argc != 6 && argc != 7) {
        fprintf(stderr, "Usage: %s <fsimage> <path> <blocks> <inodes> <blksperext> [-rand]\n",
                argv[0]);
        fprintf(stderr, "  <fsimage> is the image to create\n");
        fprintf(stderr, "  <path> is the path of the host-directory to copy into the fs\n");
        fprintf(stderr, "  <blocks> is the number of blocks the fs image should have\n");
        fprintf(stderr, "  <inodes> is the number of inodes the fs image should have\n");
        fprintf(stderr, "  <blksperext> the max. number of blocks per extent (0 = unlimited)\n");
        fprintf(stderr, "  -rand: use random for the block allocation\n");
        return EXIT_FAILURE;
    }

    srand(static_cast<uint>(time(nullptr)));

    sb.blocksize = 4096;
    sb.total_blocks = strtoul(argv[3], nullptr, 0);
    sb.total_inodes = strtoul(argv[4], nullptr, 0);
    sb.free_blocks = sb.total_blocks;
    sb.free_inodes = sb.total_inodes;
    blks_per_extent = strtoul(argv[5], nullptr, 0);
    use_rand = argc == 7 && strcmp(argv[6], "-rand");
    last_block = sb.first_data_block() - 1;

    if(sb.total_blocks > MAX_BLOCKS)
        errx(1, "Too many blocks. Max is %d", MAX_BLOCKS);
    if(sb.total_inodes > MAX_INODES)
        errx(1, "Too many inodes. Max is %d", MAX_INODES);
    if(sb.first_data_block() > sb.free_blocks)
        errx(1, "Not enough blocks");

    block_bitmap = new m3::Bitmap(sb.total_blocks);
    inode_bitmap = new m3::Bitmap(sb.total_inodes);

    file = fopen(argv[1], "w+");
    if(!file)
        err(1, "Unable to open '%s' for writing", argv[1]);

    // first, init the fs-image with zeros
    ftruncate(fileno(file), static_cast<off_t>(sb.blocksize * sb.total_blocks));

    // mark superblock, inode and block bitmap and inode blocks as occupied
    for(m3::blockno_t i = 0; i < sb.first_data_block(); ++i)
        block_bitmap->set(i);
    sb.free_blocks -= sb.first_data_block();

    // copy content from given directory to fs
    copy(argv[2], 0, 0);

    sb.first_free_inode = first_free(*inode_bitmap, sb.total_inodes);
    sb.first_free_block = first_free(*block_bitmap, sb.total_blocks);

    PRINT("Writing superblock in block 0\n");
    sb.checksum = sb.get_checksum();
    write_to_block(&sb, sizeof(sb), 0);

    PRINT("Writing inode bitmap in blocks %u..%u\n", sb.first_inodebm_block(),
          sb.first_inodebm_block() + sb.inodebm_blocks());
    write_to_block(inode_bitmap->bytes(), (sb.total_inodes + 7) / 8, sb.first_inodebm_block());

    PRINT("Writing block bitmap in blocks %u..%u\n", sb.first_blockbm_block(),
          sb.first_blockbm_block() + sb.blockbm_blocks());
    write_to_block(block_bitmap->bytes(), (sb.total_blocks + 7) / 8, sb.first_blockbm_block());

    fclose(file);
    return 0;
}

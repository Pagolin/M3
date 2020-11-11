/*
 * Copyright (C) 2015-2020, Nils Asmussen <nils@os.inf.tu-dresden.de>
 * Copyright (C) 2019-2020, Tendsin Mende <tendsin@protonmail.com>
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

use crate::data::{DirEntryIterator, FileMode, INodeRef, InodeNo};
use crate::ops::{inodes, links};

use m3::errors::{Code, Error};

/// Returns the directory and filename part of the given path.
///
/// - split_path("/foo/bar.baz") == ("/foo", "bar.baz")
/// - split_path("/foo/bar/") == ("/foo", "bar");
/// - split_path("foo") == ("", "foo");
fn split_path<'a>(mut path: &'a str) -> (&'a str, &'a str) {
    // skip trailing slashes
    while path.ends_with('/') {
        path = &path[..path.len() - 1];
    }

    let last_slash = path.rfind('/');
    if let Some(s) = last_slash {
        (&path[..s], &path[s + 1..])
    }
    else {
        // the path is either empty or only contained slashes
        ("", path)
    }
}

fn find_entry(inode: &INodeRef, name: &str) -> Result<InodeNo, Error> {
    let mut indir = None;

    for ext_idx in 0..inode.extents {
        let ext = inodes::get_extent(inode, ext_idx as usize, &mut indir, false)?;
        for bno in ext.blocks() {
            let mut block = crate::hdl().metabuffer().get_block(bno, false)?;
            let entry_iter = DirEntryIterator::from_block(&mut block);
            while let Some(entry) = entry_iter.next() {
                if entry.name() == name {
                    return Ok(entry.nodeno);
                }
            }
        }
    }

    Err(Error::new(Code::NoSuchFile))
}

/// Searches for the given path, optionally creates a new file, and returns the inode number.
pub fn search(path: &str, create: bool) -> Result<InodeNo, Error> {
    let ino = do_search(path, create);
    log!(
        crate::LOG_DIRS,
        "dirs::search(path={}, create={}) -> {:?}",
        path,
        create,
        ino.as_ref().map_err(|e| e.code()),
    );
    ino
}

fn do_search(mut path: &str, create: bool) -> Result<InodeNo, Error> {
    // remove all leading /
    while path.starts_with('/') {
        path = &path[1..];
    }

    // root inode?
    if path.is_empty() {
        return Ok(0);
    }

    // start at root inode with search
    let mut ino = 0;

    let (filename, inode) = loop {
        // get directory inode
        let inode = inodes::get(ino)?;

        // find directory entry
        let next_end = path.find('/').unwrap_or(path.len());
        let filename = &path[..next_end];
        let next_ino = find_entry(&inode, filename);

        // walk to next path component start
        let mut end = &path[next_end..];
        while end.starts_with('/') {
            end = &end[1..];
        }

        if let Ok(nodeno) = next_ino {
            // if path is now empty, finish searching
            if end.is_empty() {
                return Ok(nodeno);
            }
            // continue with this directory
            ino = nodeno;
        }
        else {
            // cannot create new file if it's not the last path component
            if !end.is_empty() {
                return Err(Error::new(Code::NoSuchFile));
            }

            // not found, maybe we want to create it
            break (filename, inode);
        }

        // to next path component
        path = end;
    };

    if create {
        // create inode and put link into directory
        let new_inode = inodes::create(FileMode::FILE_DEF)?;
        if let Err(e) = links::create(&inode, filename, &new_inode) {
            crate::hdl().files().delete_file(new_inode.inode).ok();
            return Err(e);
        };
        return Ok(new_inode.inode);
    }

    Err(Error::new(Code::NoSuchFile))
}

/// Creates a new directory with given mode at given path
pub fn create(path: &str, mode: FileMode) -> Result<(), Error> {
    let res = do_create(path, mode);
    log!(
        crate::LOG_DIRS,
        "dirs::create(path={}, mode={:o}) -> {:?}",
        path,
        mode,
        res.as_ref().map_err(|e| e.code()),
    );
    res
}

fn do_create(path: &str, mode: FileMode) -> Result<(), Error> {
    let (dir, name) = split_path(path);

    // get parent directory
    let parent_ino = search(dir, false)?;

    // ensure that the entry doesn't exist
    if search(path, false).is_ok() {
        return Err(Error::new(Code::Exists));
    }

    let parinode = inodes::get(parent_ino)?;
    if let Ok(dirino) = inodes::create(FileMode::DIR_DEF | mode) {
        // create directory itself
        if let Err(e) = links::create(&parinode, name, &dirino) {
            crate::hdl().files().delete_file(dirino.inode).ok();
            return Err(e);
        }

        // create "." link
        if let Err(e) = links::create(&dirino, ".", &dirino) {
            links::remove(&parinode, name, false).unwrap();
            crate::hdl().files().delete_file(dirino.inode).ok();
            return Err(e);
        }

        // create ".." link
        if let Err(e) = links::create(&dirino, "..", &parinode) {
            links::remove(&dirino, ".", false).unwrap();
            links::remove(&parinode, name, false).unwrap();
            crate::hdl().files().delete_file(dirino.inode).ok();
            return Err(e);
        }

        Ok(())
    }
    else {
        Err(Error::new(Code::NoSpace))
    }
}

/// Removes the directory at given path if it is empty
pub fn remove(path: &str) -> Result<(), Error> {
    log!(crate::LOG_DIRS, "dirs::remove(path={})", path);

    let ino = search(path, false)?;

    // it has to be a directory
    let inode = inodes::get(ino)?;
    if !inode.mode.is_dir() {
        return Err(Error::new(Code::IsNoDir));
    }

    // check whether it's empty
    let mut indir = None;

    for ext_idx in 0..inode.extents {
        let ext = inodes::get_extent(&inode, ext_idx as usize, &mut indir, false)?;
        for bno in ext.blocks() {
            let mut block = crate::hdl().metabuffer().get_block(bno, false)?;
            let entry_iter = DirEntryIterator::from_block(&mut block);
            while let Some(entry) = entry_iter.next() {
                if entry.name() != "." && entry.name() != ".." {
                    return Err(Error::new(Code::DirNotEmpty));
                }
            }
        }
    }

    // hardlinks to directories are not possible, thus we always have 2 ( . and ..)
    assert!(inode.links == 2, "expected 2 links, found {}", inode.links);
    // ensure that the inode is removed
    inode.as_mut().links -= 1;

    // TODO if that fails, we have already reduced the link count!?
    unlink(path, true)
}

/// Creates a link at `new_path` to `old_path`
pub fn link(old_path: &str, new_path: &str) -> Result<(), Error> {
    log!(
        crate::LOG_DIRS,
        "dirs::link(old_path={}, new_path={})",
        old_path,
        new_path
    );

    let oldino = search(old_path, false)?;

    // it can't be a directory
    let old_inode = inodes::get(oldino)?;
    if old_inode.mode.is_dir() {
        return Err(Error::new(Code::IsDir));
    }

    let (dir, name) = split_path(new_path);

    let baseino = search(dir, false)?;
    let base_ino = inodes::get(baseino)?;
    links::create(&base_ino, name, &old_inode)
}

/// Removes the directory entry at given path
///
/// If `is_dir` is true, the link count at the parent inode is reduced.
pub fn unlink(path: &str, is_dir: bool) -> Result<(), Error> {
    log!(
        crate::LOG_DIRS,
        "dirs::unlink(path={}, is_dir={})",
        path,
        is_dir
    );

    let (dir, name) = split_path(path);
        let (base_slice, dir_slice) = get_base_dir(path);
        (&path[base_slice], &path[dir_slice])
    };

    let parino = search(dir, false)?;
    let parinode = inodes::get(parino)?;

    let res = links::remove(&parinode, name, !is_dir);
    if is_dir && res.is_ok() {
        // decrement link count for parent inode by one
        parinode.as_mut().links -= 1;
    }

    res
}

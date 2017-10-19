use cap::Selector;
use cell::RefCell;
use com::*;
use core::fmt;
use core::intrinsics;
use errors::Error;
use kif::INVALID_SEL;
use rc::{Rc, Weak};
use session::Session;
use vfs::{FileInfo, FileMode, FileSystem, OpenFlags, RegularFile, SeekMode};

pub type Fd = i32;
pub type ExtId = u16;

pub struct M3FS {
    self_weak: Weak<RefCell<M3FS>>,
    sess: Session,
    sgate: SendGate,
}

const MAX_LOCS: usize = 4;

pub struct LocList {
    lens: [usize; MAX_LOCS],
    count: usize,
    sel: Selector,
}

impl LocList {
    pub const MAX: usize = MAX_LOCS;

    pub fn new() -> Self {
        LocList {
            lens: unsafe { intrinsics::uninit() },
            count: 0,
            sel: INVALID_SEL,
        }
    }

    pub fn clear(&mut self) {
        self.count = 0;
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn total_length(&self) -> usize {
        let mut len = 0;
        for i in 0..self.count {
            len += self.lens[i];
        }
        len
    }

    pub fn get_len(&self, idx: ExtId) -> usize {
        if (idx as usize) < self.count {
            self.lens[idx as usize]
        }
        else {
            0
        }
    }

    pub fn get_sel(&self, idx: ExtId) -> Selector {
        self.sel + idx as Selector
    }

    pub fn set_sel(&mut self, sel: Selector) {
        self.sel = sel;
    }

    pub fn append(&mut self, len: usize) {
        assert!(self.count < MAX_LOCS);
        self.lens[self.count] = len;
        self.count += 1;
    }
}

impl fmt::Display for LocList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "LocList["));
        for i in 0..self.count {
            try!(write!(f, "{}", self.lens[i]));
            if i + 1 < self.count {
                try!(write!(f, ", "));
            }
        }
        write!(f, "]")
    }
}

int_enum! {
    struct Operation : u32 {
        const OPEN      = 0x0;
        const STAT      = 0x1;
        const FSTAT     = 0x2;
        const SEEK      = 0x3;
        const MKDIR     = 0x4;
        const RMDIR     = 0x5;
        const LINK      = 0x6;
        const UNLINK    = 0x7;
        const COMMIT    = 0x8;
        const CLOSE     = 0x9;
    }
}

bitflags! {
    pub struct LocFlags : u32 {
        const BYTE_OFF  = 0b01;
        const EXTEND    = 0b10;
    }
}

impl M3FS {
    fn create(sess: Session, sgate: SendGate) -> Rc<RefCell<Self>> {
        let inst = Rc::new(RefCell::new(M3FS {
            self_weak: Weak::new(),
            sess: sess,
            sgate: sgate,
        }));
        inst.borrow_mut().self_weak = Rc::downgrade(&inst);
        inst
    }

    pub fn new(name: &str) -> Result<Rc<RefCell<Self>>, Error> {
        let sess = try!(Session::new(name, 0));
        let sgate = SendGate::new_bind(try!(sess.obtain(1, &[], &mut [])).1.start());
        Ok(Self::create(sess, sgate))
    }

    pub fn new_bind(sess: Selector, sgate: Selector) -> Rc<RefCell<Self>> {
        Self::create(Session::new_bind(sess), SendGate::new_bind(sgate))
    }

    pub fn get_locs(&self, fd: Fd, ext: ExtId, locs: &mut LocList,
                    flags: LocFlags) -> Result<(usize, bool), Error> {
        let loc_count = if flags.contains(LocFlags::EXTEND) { 2 } else { MAX_LOCS };
        let sargs: [u64; 4] = [fd as u64, ext as u64, loc_count as u64, flags.bits as u64];
        let mut rargs = [0u64; 2 + MAX_LOCS];
        let (num, crd) = try!(self.sess.obtain(MAX_LOCS as u32, &sargs, &mut rargs));
        locs.set_sel(crd.start());
        for i in 2..num {
            locs.append(rargs[i] as usize);
        }
        Ok((rargs[1] as usize, rargs[0] == 1))
    }

    pub fn fstat(&self, fd: Fd) -> Result<FileInfo, Error> {
        let mut reply = try!(send_recv_res!(
            &self.sgate, RecvGate::def(),
            Operation::FSTAT, fd
        ));
        Ok(reply.pop())
    }

    pub fn seek(&self, fd: Fd, off: usize, mode: SeekMode, extent: ExtId, extoff: usize)
                -> Result<(ExtId, usize, usize), Error> {
        let mut reply = try!(send_recv_res!(
            &self.sgate, RecvGate::def(),
            Operation::SEEK, fd, off, mode, extent, extoff
        ));
        Ok((reply.pop(), reply.pop(), reply.pop()))
    }

    pub fn commit(&self, fd: Fd, extent: ExtId, off: usize) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            Operation::COMMIT, fd, extent, off
        ).map(|_| ())
    }

    pub fn close(&self, fd: Fd, extent: ExtId, off: usize) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            Operation::CLOSE, fd, extent, off
        ).map(|_| ())
    }
}

impl FileSystem<RegularFile> for M3FS {
    fn open(&self, path: &str, flags: OpenFlags) -> Result<RegularFile, Error> {
        let mut reply = try!(send_recv_res!(
            &self.sgate, RecvGate::def(),
            Operation::OPEN, path, flags.bits()
        ));
        let fd = reply.pop();
        Ok(RegularFile::new(self.self_weak.upgrade().unwrap(), fd, flags))
    }

    fn stat(&self, path: &str) -> Result<FileInfo, Error> {
        let mut reply = try!(send_recv_res!(
            &self.sgate, RecvGate::def(),
            Operation::STAT, path
        ));
        Ok(reply.pop())
    }

    fn mkdir(&self, path: &str, mode: FileMode) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            Operation::MKDIR, path, mode
        ).map(|_| ())
    }
    fn rmdir(&self, path: &str) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            Operation::RMDIR, path
        ).map(|_| ())
    }

    fn link(&self, old_path: &str, new_path: &str) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            Operation::LINK, old_path, new_path
        ).map(|_| ())
    }
    fn unlink(&self, path: &str) -> Result<(), Error> {
        send_recv_res!(
            &self.sgate, RecvGate::def(),
            Operation::UNLINK, path
        ).map(|_| ())
    }
}

pub mod tests {
    use super::*;
    use vfs::*;

    pub fn run(t: &mut ::test::Tester) {
        run_test!(t, meta_ops);
    }

    pub fn meta_ops() {
        let m3fs = M3FS::new("m3fs").expect("connect to m3fs failed");

        assert_ok!(m3fs.borrow_mut().mkdir("/example", 0o755));
        assert_err!(m3fs.borrow_mut().mkdir("/example", 0o755), Error::Exists);
        assert_err!(m3fs.borrow_mut().mkdir("/example/foo/bar", 0o755), Error::NoSuchFile);

        {
            let mut file = assert_ok!(m3fs.borrow_mut().open("/example/myfile",
                OpenFlags::W | OpenFlags::CREATE));
            assert_ok!(write!(file, "text\n"));
        }

        // TODO add that as soon as mounting is supported
        // {
        //     assert_int(m3fs.borrow_mut().mount("/fs/", "m3fs"), Errors::NONE);
        //     assert_int(m3fs.borrow_mut().link("/example/myfile", "/fs/foo"), Errors::XFS_LINK);
        //     m3fs.borrow_mut().unmount("/fs");
        // }

        assert_err!(m3fs.borrow_mut().rmdir("/example/foo/bar"), Error::NoSuchFile);
        assert_err!(m3fs.borrow_mut().rmdir("/example/myfile"), Error::IsNoDir);
        assert_err!(m3fs.borrow_mut().rmdir("/example"), Error::DirNotEmpty);

        assert_err!(m3fs.borrow_mut().link("/example", "/newpath"), Error::IsDir);
        assert_ok!(m3fs.borrow_mut().link("/example/myfile", "/newpath"));

        assert_err!(m3fs.borrow_mut().unlink("/example"), Error::IsDir);
        assert_err!(m3fs.borrow_mut().unlink("/example/foo"), Error::NoSuchFile);
        assert_ok!(m3fs.borrow_mut().unlink("/example/myfile"));

        assert_ok!(m3fs.borrow_mut().rmdir("/example"));

        assert_ok!(m3fs.borrow_mut().unlink("/newpath"));
    }
}

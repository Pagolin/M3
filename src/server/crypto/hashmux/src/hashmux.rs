/*
 * Copyright (C) 2021, Stephan Gerhold <stephan.gerhold@mailbox.tu-dresden.de>
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

#![no_std]

// Choose between using hardware accelerator vs software fallback using XKCP.
// Note that this is a compile-time choice currently, the XKCP increases the
// binary size of HashMux by about 200 KiB.
// FIXME: Enable hardware accelerator on "hw" target
#[cfg(target_vendor = "gem5")]
mod kecacc;

#[cfg(not(target_vendor = "gem5"))]
#[path = "kecacc-xkcp.rs"]
mod kecacc;

use crate::kecacc::{KecAcc, KecAccState};
use base::const_assert;
use core::cmp::min;
use core::sync::atomic;
use m3::cap::Selector;
use m3::cell::{LazyReadOnlyCell, LazyStaticRefCell, StaticRefCell};
use m3::col::VecDeque;
use m3::com::{GateIStream, MemGate, SGateArgs, SendGate};
use m3::crypto::{HashAlgorithm, HashType};
use m3::errors::{Code, Error};
use m3::kif::{CapRngDesc, CapType, INVALID_SEL};
use m3::log;
use m3::mem::{size_of, AlignedBuf, MsgBuf, MsgBufRef};
use m3::server::{
    server_loop, CapExchange, Handler, RequestHandler, Server, SessId, SessionContainer,
    DEF_MSG_SIZE,
};
use m3::session::{HashOp, ServerSession};
use m3::tcu::{Label, Message};
use m3::tiles::Activity;
use m3::time::{TimeDuration, TimeInstant};

const LOG_DEF: bool = false;
const LOG_ERRORS: bool = true;
const LOG_VERBOSE: bool = false;

/// Size of the two SRAMs used as temporary buffer for TCU transfers
/// and the accelerator.
const BUFFER_SIZE: usize = 8 * 1024; // 8 KiB

/// Maximum number of sessions the multiplexer allows.
const MAX_SESSIONS: usize = 32;

/// The default time slice if not specified in the session arguments.
const DEFAULT_TIME_SLICE: TimeDuration = TimeDuration::from_micros(100);

/// Wait until the accelerator is done before triggering a new read/write via
/// the TCU. Enabling this effectively disables the performance benefit of the
/// double buffering approach, which can be used for performance comparison.
const DISABLE_DOUBLE_BUFFER: bool = false;

/// Force saving/loading accelerator state even if the client is already loaded
/// in the accelerator. Mostly useful for benchmarking.
const FORCE_CONTEXT_SWITCH: bool = false;

/// Enables an optimization that checks for new messages while the accelerator
/// is busy. The optimization is most useful when there is often just one client
/// active at the same time and there are short time slices.
const OPTIMIZE_SCHEDULING: bool = true;

// Two memory regions used as temporary buffers (accessed by TCU and accelerator)
// FIXME: This should point to the dedicated SRAMs
static BUF1: StaticRefCell<AlignedBuf<BUFFER_SIZE>> = StaticRefCell::new(AlignedBuf::new_zeroed());
static BUF2: StaticRefCell<AlignedBuf<BUFFER_SIZE>> = StaticRefCell::new(AlignedBuf::new_zeroed());

// Memory region used to save/load states of the accelerator for context switches
static STATES: StaticRefCell<[KecAccState; MAX_SESSIONS]> =
    StaticRefCell::new([KecAccState::new(); MAX_SESSIONS]);

/// Amount of bytes that may be directly returned as part of the TCU reply.
/// Must also fit into [`MsgBuf::borrow_def()`].
const MAX_DIRECT_SIZE: usize = HashAlgorithm::MAX_OUTPUT_BYTES;
const_assert!(MAX_DIRECT_SIZE <= BUFFER_SIZE);

static RECV: LazyReadOnlyCell<HashMuxReceiver> = LazyReadOnlyCell::default();
static QUEUE: LazyStaticRefCell<VecDeque<SessId>> = LazyStaticRefCell::default();
static KECACC: KecAcc = KecAcc::new(0xF4200000);

#[derive(Debug)]
enum HashRequestType {
    /// Read bytes from the MemGate and input them into the hash.
    Input,
    /// Output bytes from the hash and write them into the MemGate.
    Output,
    /// Output bytes from the hash and write them into the MemGate. Pad first.
    OutputPad,
    /// Output a few bytes from the hash and send them directly in the reply.
    OutputDirect,
}

/// A delayed request from a client that will be processed with scheduling.
struct HashRequest {
    ty: HashRequestType,
    msg: &'static Message,
    len: usize,
    off: usize,
}

/// A session opened by a client, holds state and scheduling information.
struct HashSession {
    sess: ServerSession,
    sgate: SendGate,
    mgate: MemGate,
    algo: Option<&'static HashAlgorithm>,
    state_saved: bool,
    req: Option<HashRequest>,
    time_slice: TimeDuration,
    remaining_time: i64, // in nanoseconds
    output_bytes: usize,
}

/// Handles requests and holds all hash sessions.
struct HashHandler {
    sessions: SessionContainer<HashSession>,
    current: Option<SessId>,
}

/// Receives requests from kernel and clients through `RecvGate`s.
struct HashMuxReceiver {
    server: Server,
    reqhdl: RequestHandler,
}

/// Run func repeatedly with the two buffers swapped,
/// until an error occurs or func returns Ok(false).
fn loop_double_buffer<F>(mut func: F) -> Result<(), Error>
where
    F: FnMut(&mut [u8], &mut [u8]) -> Result<bool, Error>,
{
    let buf1 = &mut BUF1.borrow_mut()[..];
    let buf2 = &mut BUF2.borrow_mut()[..];
    while func(buf1, buf2)? && func(buf2, buf1)? {}
    Ok(())
}

/// Handles time accounting when working on a request by a client.
struct HashMuxTimer {
    /// The time slice configured for the client, copied from the session.
    slice: TimeDuration,
    /// The time in TCU nano seconds when the clients time expires.
    end: TimeInstant,
}

impl HashMuxTimer {
    fn start(sess: &HashSession) -> Self {
        HashMuxTimer {
            slice: sess.time_slice,
            // wrapping_add handles unsigned + signed addition here
            end: TimeInstant::from_nanos(
                TimeInstant::now()
                    .as_nanos()
                    .wrapping_add(sess.remaining_time as u64),
            ),
        }
    }

    /// Wait until the accelerator is really done and calculate remaining time.
    fn remaining_time(&self, mut t: TimeInstant) -> i64 {
        if KECACC.is_busy() {
            KECACC.poll_complete();
            t = TimeInstant::now();
        }

        // This might underflow (end < now()), resulting in a negative number after the cast
        self.end.as_nanos().wrapping_sub(t.as_nanos()) as i64
    }

    /// Check if the client has run out of time. If yes, return how much additional
    /// time the client has used (= a negative amount of remaining time).
    ///
    /// An additional optimization done here is to check if there are actually
    /// any other clients that have work ready. If no other client is ready,
    /// the current one can just keep working without interrupting the
    /// double buffering. This is slightly faster (although not that much
    /// with large enough time slices).
    ///
    /// This optimization would mainly allow reducing latency if prioritization
    /// is needed at some point. The time when the accelerator is busy could
    /// be used to check pending messages more frequently, without adjusting
    /// the time slice of low-priority clients.
    fn try_continue(&mut self) -> Result<(), i64> {
        let t = TimeInstant::now();
        if t >= self.end {
            // Time expired, look for other clients to work on
            if !OPTIMIZE_SCHEDULING || !QUEUE.borrow().is_empty() || RECV.get().has_messages() {
                return Err(self.remaining_time(t));
            }

            // No one else has work ready so just keep going
            self.end += self.slice;
        }
        Ok(())
    }

    /// Calculate the remaining time the client has if work completes early
    /// (because it is done or because of an error).
    fn finish(self) -> i64 {
        self.remaining_time(TimeInstant::now())
    }
}

impl HashRequest {
    /// Try to take up to one full buffer from the request and return the
    /// size of it (typically the buffer size unless there is not enough left).
    fn take_buffer_size(&mut self) -> usize {
        let n = min(self.len, BUFFER_SIZE);
        self.len -= n;
        n
    }

    /// Complete work on a buffer size obtained by [take_buffer_size()].
    fn complete_buffer(&mut self, n: usize) {
        self.off += n;
    }

    /// Reply with code to the original request.
    fn reply(self, code: Code) {
        let mut msg = MsgBuf::borrow_def();
        msg.set(code as u64);
        self.reply_msg(msg)
    }

    /// Reply with the specified message to the original request.
    fn reply_msg(self, msg: MsgBufRef<'_>) {
        let rgate = RECV.get().reqhdl.recv_gate();
        rgate
            .reply(&msg, self.msg)
            .or_else(|_| rgate.ack_msg(self.msg))
            .ok();
    }
}

impl HashSession {
    /// Work on an input request by reading memory from the MemGate and
    /// letting the accelerator absorb it.
    fn work_input(&mut self, mut req: HashRequest) -> bool {
        let mut timer = HashMuxTimer::start(self);

        let res = loop_double_buffer(|buf, _| {
            let n = req.take_buffer_size();
            if n == 0 {
                return Err(Error::new(Code::Success)); // Done
            }

            self.mgate.read_bytes(buf.as_mut_ptr(), n, req.off as u64)?;

            KECACC.start_absorb(&buf[..n]);
            if DISABLE_DOUBLE_BUFFER {
                KECACC.poll_complete();
            }
            req.complete_buffer(n);

            if let Err(remaining_time) = timer.try_continue() {
                self.remaining_time = remaining_time;
                return Ok(false);
            }
            Ok(true)
        });
        self.handle_result(res, req, timer)
    }

    /// Work on an output request by letting the accelerator squeeze it
    /// and then writing it to the MemGate.
    fn work_output(&mut self, mut req: HashRequest) -> bool {
        let mut timer = HashMuxTimer::start(self);

        // Apply padding once for the request if needed
        if matches!(req.ty, HashRequestType::OutputPad) {
            KECACC.start_pad();
            req.ty = HashRequestType::Output;
        }

        // Output is a bit more complicated than input because the MemGate is
        // always used synchronously (poll until done), while the accelerator
        // runs asynchronously (start, do other work, poll if not done yet).
        // Since the accelerator runs first for output, it needs to finish up
        // one buffer first before the double buffering can be started.
        // This also means that the last squeezed buffer always need to be
        // written back to the MemGate before returning from this function.

        let mut ln = req.take_buffer_size();
        KECACC.start_squeeze(&mut BUF1.borrow_mut()[..ln]);

        let res = loop_double_buffer(|lbuf, buf| {
            let n = req.take_buffer_size();
            if n == 0 {
                // Still need to write back the last buffer
                KECACC.poll_complete();
                self.mgate.write_bytes(lbuf.as_ptr(), ln, req.off as u64)?;
                return Err(Error::new(Code::Success)); // Done
            }

            // Start squeezing *new* buffer and write back the *last* buffer
            KECACC.start_squeeze(&mut buf[..n]);
            if DISABLE_DOUBLE_BUFFER {
                KECACC.poll_complete();
            }

            self.mgate.write_bytes(lbuf.as_ptr(), ln, req.off as u64)?;
            req.complete_buffer(ln);

            if let Err(remaining_time) = timer.try_continue() {
                // Still need to write back the last buffer - this might take a bit
                // so measure the time and subtract it from the remaining time.
                let t = TimeInstant::now();
                self.mgate.write_bytes(buf.as_ptr(), n, req.off as u64)?;
                req.complete_buffer(n);
                self.remaining_time = remaining_time - (TimeInstant::now() - t).as_nanos() as i64;
                return Ok(false);
            }

            ln = n;
            Ok(true)
        });
        self.handle_result(res, req, timer)
    }

    fn handle_result(
        &mut self,
        res: Result<(), Error>,
        req: HashRequest,
        timer: HashMuxTimer,
    ) -> bool {
        match res {
            Ok(_) => {
                // More work needed, restore request
                self.req = Some(req);

                log!(
                    LOG_VERBOSE,
                    "[{}] hash::work() pause, remaining time {}",
                    self.sess.ident(),
                    self.remaining_time,
                );
                false // not done
            },
            Err(e) => {
                req.reply(e.code());
                self.remaining_time = timer.finish();

                if e.code() == Code::Success {
                    log!(
                        LOG_DEF,
                        "[{}] hash::work() done, remaining time {}",
                        self.sess.ident(),
                        self.remaining_time,
                    )
                }
                else {
                    log!(
                        LOG_ERRORS,
                        "[{}] hash::work() failed with {:?}",
                        self.sess.ident(),
                        e.code(),
                    );
                }
                true // done
            },
        }
    }

    /// Work on a direct output request by letting the accelerator squeeze
    /// a few bytes and then sending them directly as reply.
    fn work_output_direct(&mut self, req: HashRequest) -> bool {
        let timer = HashMuxTimer::start(self);
        let buf = &mut BUF1.borrow_mut()[..req.len];
        let mut msg = MsgBuf::borrow_def();

        KECACC.start_pad();
        KECACC.start_squeeze(buf);
        KECACC.poll_complete();

        // Make sure the accelerator above is actually done and has written back
        // its result. Without this computing a SHA3-512 hash on x86_64 returns
        // the previous contents of the buffer instead of the generated hash.
        atomic::fence(atomic::Ordering::SeqCst);
        msg.set_from_slice(buf);

        req.reply_msg(msg);
        self.remaining_time = timer.finish();
        log!(
            LOG_DEF,
            "[{}] hash::work() done, remaining time {}",
            self.sess.ident(),
            self.remaining_time,
        );
        true // done
    }

    fn work(&mut self) -> bool {
        // Fill up time of client. Subtract time from time slice if client took too long last time
        if self.remaining_time < 0 {
            self.remaining_time += self.time_slice.as_nanos() as i64;
        }
        else {
            self.remaining_time = self.time_slice.as_nanos() as i64;
        }
        let req = self.req.take().unwrap();

        log!(
            LOG_VERBOSE,
            "[{}] hash::work() {:?} start len {} off {} remaining time {} queue {:?}",
            self.sess.ident(),
            req.ty,
            req.len,
            req.off,
            self.remaining_time,
            QUEUE.borrow(),
        );

        match req.ty {
            HashRequestType::Input => self.work_input(req),
            HashRequestType::Output | HashRequestType::OutputPad => self.work_output(req),
            HashRequestType::OutputDirect => self.work_output_direct(req),
        }
    }
}

impl HashSession {
    fn id(&self) -> SessId {
        self.sess.ident() as SessId
    }

    fn reset(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let ty: HashType = is.pop()?;
        let algo = HashAlgorithm::from_type(ty).ok_or_else(|| Error::new(Code::InvArgs))?;

        log!(
            LOG_DEF,
            "[{}] hash::reset() algo {}",
            self.sess.ident(),
            algo
        );
        assert!(self.req.is_none());
        self.algo = Some(algo);
        self.state_saved = false;
        self.output_bytes = 0;
        is.reply_error(Code::Success)
    }

    /// Queue a new request for the client and mark client as ready and perhaps
    /// even waiting if it has remaining time.
    fn queue_request(&mut self, req: HashRequest) -> Result<(), Error> {
        assert!(self.req.is_none());

        if req.len > 0 {
            self.req = Some(req);
            QUEUE.borrow_mut().push_back(self.id());
        }
        else {
            // This is weird but not strictly wrong, just return immediately
            req.reply(Code::Success);
        }
        Ok(())
    }

    fn input(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        log!(LOG_DEF, "[{}] hash::input()", self.sess.ident());

        // Disallow input after output for now since this is not part of the SHA-3 specification.
        // However, there is a separate paper about the "Duplex" construction:
        // https://keccak.team/files/SpongeDuplex.pdf, so this could be changed if needed.
        if self.output_bytes > 0 {
            return Err(Error::new(Code::InvState));
        }

        self.queue_request(HashRequest {
            ty: HashRequestType::Input,
            off: is.pop()?,
            len: is.pop()?,
            msg: is.take_msg(),
        })
    }

    fn output(&mut self, is: &mut GateIStream<'_>) -> Result<(), Error> {
        log!(LOG_DEF, "[{}] hash::output()", self.sess.ident());

        let algo = self.algo.ok_or_else(|| Error::new(Code::InvState))?;

        let req = if is.size() > size_of::<u64>() {
            HashRequest {
                ty: if self.output_bytes == 0 {
                    // On first output padding still needs to be applied to the input
                    HashRequestType::OutputPad
                }
                else {
                    HashRequestType::Output
                },
                off: is.pop()?,
                len: is.pop()?,
                msg: is.take_msg(),
            }
        }
        else {
            // No parameters, return the fixed size hash directly in the TCU reply
            if algo.output_bytes > MAX_DIRECT_SIZE {
                log!(
                    LOG_ERRORS,
                    "[{}] hash::output() cannot use direct output for {}",
                    self.sess.ident(),
                    algo.name,
                );
                return Err(Error::new(Code::InvArgs));
            }

            HashRequest {
                ty: HashRequestType::OutputDirect,
                len: algo.output_bytes,
                msg: is.take_msg(),
                off: 0,
            }
        };

        // Verify that the client does not consume more bytes than supported for the hash algorithm
        self.output_bytes = self.output_bytes.saturating_add(req.len);
        if self.output_bytes > algo.output_bytes {
            log!(
                LOG_ERRORS,
                "[{}] hash::output() attempting to output {} bytes while only {} are supported for {}",
                self.sess.ident(),
                self.output_bytes,
                algo.output_bytes,
                algo.name
            );

            req.reply(Code::InvArgs);
            return Ok(());
        }

        self.queue_request(req)
    }
}

impl HashHandler {
    fn handle(&mut self, op: HashOp, is: &mut GateIStream<'_>) -> Result<(), Error> {
        let sid = is.label() as SessId;
        let sess = self
            .sessions
            .get_mut(sid)
            .ok_or_else(|| Error::new(Code::InvArgs))?;

        match op {
            HashOp::RESET => sess.reset(is).map(|_| {
                // Clear current session if necessary to force re-initialization
                if self.current == Some(sid) {
                    self.current = None;
                }
            }),
            HashOp::INPUT => sess.input(is),
            HashOp::OUTPUT => sess.output(is),
            _ => Err(Error::new(Code::InvArgs)),
        }
    }

    /// Switch the current client in the accelerator if necessary
    /// by saving/loading the state.
    fn switch(&mut self, to: SessId) {
        let mut state = STATES.borrow_mut();

        if let Some(cur) = self.current {
            if !FORCE_CONTEXT_SWITCH && cur == to {
                // Already the current client
                return;
            }

            // Save state of current client
            KECACC.start_save(&mut state[cur]);
            self.sessions.get_mut(cur as SessId).unwrap().state_saved = true;
        }
        self.current = Some(to);

        // Restore state of new client or initialize it if necessary
        let sess = self.sessions.get(to).unwrap();
        if sess.state_saved {
            KECACC.start_load(&state[to]);
        }
        else {
            KECACC.start_init(sess.algo.unwrap().ty.val as u8);
        }
    }
}

impl Handler<HashSession> for HashHandler {
    fn sessions(&mut self) -> &mut SessionContainer<HashSession> {
        &mut self.sessions
    }

    fn open(
        &mut self,
        crt: usize,
        srv_sel: Selector,
        arg: &str,
    ) -> Result<(Selector, SessId), Error> {
        // Use the time slice specified in the arguments or fall back to the default one
        let time_slice = if !arg.is_empty() {
            TimeDuration::from_nanos(arg.parse::<u64>().map_err(|_| Error::new(Code::InvArgs))?)
        }
        else {
            DEFAULT_TIME_SLICE
        };

        self.sessions.add_next(crt, srv_sel, false, |sess| {
            let sid = sess.ident() as SessId;
            log!(LOG_DEF, "[{}] hash::open()", sid);
            assert!(sid < MAX_SESSIONS);

            let sel = Activity::own().alloc_sels(2);
            Ok(HashSession {
                sess,
                sgate: SendGate::new_with(
                    SGateArgs::new(RECV.get().reqhdl.recv_gate())
                        .sel(sel)
                        .credits(1)
                        .label(sid as Label),
                )?,
                mgate: MemGate::new_bind(INVALID_SEL),
                algo: None,
                state_saved: false,
                req: None,
                time_slice,
                remaining_time: 0,
                output_bytes: 0,
            })
        })
    }

    fn obtain(
        &mut self,
        _crt: usize,
        sid: SessId,
        xchg: &mut CapExchange<'_>,
    ) -> Result<(), Error> {
        log!(LOG_DEF, "[{}] hash::obtain()", sid);
        let hash = self
            .sessions
            .get_mut(sid)
            .ok_or_else(|| Error::new(Code::InvArgs))?;

        // FIXME: Let client obtain EP capability directly with first obtain() below
        // Right now this is not possible because mgate.activate() returns an EP
        // from the EpMng which does not allow binding it to a specified capability selector.
        if xchg.in_caps() == 1 {
            hash.mgate.activate()?;
            let ep_sel = hash.mgate.ep().unwrap().sel();
            xchg.out_caps(CapRngDesc::new(CapType::OBJECT, ep_sel, 1));
            return Ok(());
        }

        if xchg.in_caps() != 2 {
            return Err(Error::new(Code::InvArgs));
        }
        xchg.out_caps(CapRngDesc::new(CapType::OBJECT, hash.sgate.sel(), 1));
        Ok(())
    }

    fn close(&mut self, crt: usize, sid: SessId) {
        log!(LOG_DEF, "[{}] hash::close()", sid);

        let sess = self.sessions.remove(crt, sid);
        QUEUE.borrow_mut().retain(|&n| n != sid);
        if self.current == Some(sid) {
            self.current = None;
        }

        // Revoke EP capability that was delegated for memory accesses
        if let Some(ep) = sess.mgate.ep() {
            if let Err(e) =
                Activity::own().revoke(CapRngDesc::new(CapType::OBJECT, ep.sel(), 1), true)
            {
                log!(LOG_ERRORS, "[{}] Failed to revoke EP cap: {}", sid, e)
            }
        };
    }
}

impl HashMuxReceiver {
    fn has_messages(&self) -> bool {
        self.reqhdl.recv_gate().has_msgs().unwrap() || self.server.rgate().has_msgs().unwrap()
    }

    fn handle_messages(&self, hdl: &mut HashHandler) -> Result<(), Error> {
        // NOTE: Currently this only fetches a single message, perhaps handle()
        // should return if a message was handled so this could be put in a loop.
        self.server.handle_ctrl_chan(hdl)?;
        self.reqhdl.handle(|op, is| hdl.handle(op, is))
    }
}

#[no_mangle]
pub fn main() -> Result<(), Error> {
    let mut hdl = HashHandler {
        sessions: SessionContainer::new(MAX_SESSIONS),
        current: None,
    };
    let server = Server::new("hash", &mut hdl).expect("Unable to create service 'hash'");

    RECV.set(HashMuxReceiver {
        server,
        reqhdl: RequestHandler::new_with(MAX_SESSIONS, DEF_MSG_SIZE)
            .expect("Unable to create request handler"),
    });
    QUEUE.set(VecDeque::with_capacity(MAX_SESSIONS));

    server_loop(|| {
        RECV.get().handle_messages(&mut hdl)?;

        // The QUEUE is mutably borrowed to pop a session and again later while
        // working on a request. When this is placed directly into the while let
        // Rust only drops the borrow after the loop iteration for some reason.
        // Placing this in a separate function/closure seems to convince it to
        // drop it again immediately...
        let pop_queue = || QUEUE.borrow_mut().pop_front();

        // Keep working until no more work is available
        while let Some(sid) = pop_queue() {
            hdl.switch(sid);
            let done = hdl.sessions.get_mut(sid as SessId).unwrap().work();
            RECV.get().handle_messages(&mut hdl)?;
            if !done {
                QUEUE.borrow_mut().push_back(sid);
            }
        }
        Ok(())
    })
    .ok();

    Ok(())
}

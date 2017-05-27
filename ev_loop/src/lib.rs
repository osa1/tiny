// TODO:
//
// - Better error reporting

extern crate libc;

use std::collections::HashMap;
use std::collections::HashSet;
use std::ops::BitOr;

pub struct EvLoop<Ctx> {
    fds: HashMap<libc::c_int, Handler<Ctx>>,
}

enum Handler<Ctx> {
    Timer { cb: Box<FnMut(&mut EvLoopCtrl<Ctx>, &mut Ctx, u64) -> ()> },
    Signal { cb: Box<FnMut(&mut EvLoopCtrl<Ctx>, &mut Ctx) -> ()> },
    Fd { evs: FdEv, cb: Box<FnMut(FdEv, &mut EvLoopCtrl<Ctx>, &mut Ctx) -> ()> },
}

/// File descriptor events. Use bitwise or to combine.
pub struct FdEv(libc::c_short);

pub const READ_EV: FdEv = FdEv(libc::POLLIN);
pub const WRITE_EV: FdEv = FdEv(libc::POLLOUT);
pub const ERR_EV: FdEv = FdEv(libc::POLLERR);
pub const HUP_EV: FdEv = FdEv(libc::POLLHUP);

impl BitOr for FdEv {
    type Output = FdEv;

    fn bitor(self, rhs: FdEv) -> FdEv {
        FdEv(self.0 | rhs.0)
    }
}

pub struct TimerRef(libc::c_int);
pub struct SignalRef(libc::c_int);

extern "C" {
    fn timerfd_create(
        clockid: libc::c_int,
        flags: libc::c_int
        ) -> libc::c_int;

    fn timerfd_settime(
        fd: libc::c_int,
        flags: libc::c_int,
        new_value: *const itimerspec,
        old_value: *mut itimerspec
        ) -> libc::c_int;

    // fn timerfd_gettime(
    //     fd: libc::c_int,
    //     curr_value: *mut libc::itimerspec
    //     ) -> libc::c_int;
}

// Can't find itimerspec in libc
#[repr(C)]
pub struct itimerspec {
    it_interval: libc::timespec,
    it_value: libc::timespec,
}

pub struct EvLoopCtrl<'a, Ctx: 'a> {
    stop_ref: &'a mut bool,
    remove_self: &'a mut bool,
    new_fds: HashMap<libc::c_int, Handler<Ctx>>,
    removed_fds: HashSet<libc::c_int>,
}

impl<'a, Ctx> EvLoopCtrl<'a, Ctx> {
    pub fn stop(&mut self) {
        *self.stop_ref = true;
    }

    pub fn remove_self(&mut self) {
        *self.remove_self = true;
    }

    pub fn add_fd(&mut self, fd: libc::c_int, evs: FdEv, cb: Box<FnMut(FdEv, &mut EvLoopCtrl<Ctx>, &mut Ctx) -> ()>) {
        self.new_fds.insert(fd, Handler::Fd { evs: evs, cb: cb });
        self.removed_fds.remove(&fd);
    }

    pub fn remove_fd(&mut self, fd: libc::c_int) {
        self.removed_fds.insert(fd);
        self.new_fds.remove(&fd);
    }
}

fn mk_timespec(millis: i64) -> libc::timespec {
    let secs: i64 = millis / 1000;
    let nanos: i64 = (millis % 1000) * 1000000;
    libc::timespec { tv_sec: secs, tv_nsec: nanos }
}

impl<Ctx> EvLoop<Ctx> {
    pub fn new() -> EvLoop<Ctx> {
        EvLoop { fds: HashMap::new() }
    }

    /// Register a non-blocking socket. Use the same fd for unregister.
    pub fn add_fd(&mut self, fd: libc::c_int, evs: FdEv, cb: Box<FnMut(FdEv, &mut EvLoopCtrl<Ctx>, &mut Ctx) -> ()>) {
        self.fds.insert(fd, Handler::Fd { evs: evs, cb: cb });
    }

    pub fn remove_fd(&mut self, fd: libc::c_int) {
        self.fds.remove(&fd);
    }

    /// `timeout` and `period` in milliseconds. `timeout` must be non-zero for this to work. If
    /// `period` is non-zero, timer expires repeatedly after the initial timeout.
    pub fn add_timer(&mut self, timeout: i64, period: i64, cb: Box<FnMut(&mut EvLoopCtrl<Ctx>, &mut Ctx, u64) -> ()>) -> TimerRef {
        let fd = unsafe { timerfd_create(libc::CLOCK_MONOTONIC, libc::EFD_NONBLOCK) };
        assert!(fd != -1);

        let timeout_spec = mk_timespec(timeout);
        let period_spec = mk_timespec(period);
        let timerspec = itimerspec { it_interval: period_spec, it_value: timeout_spec };

        assert!(unsafe { timerfd_settime(fd, 0, &timerspec, std::ptr::null_mut()) } != -1);
        self.fds.insert(fd, Handler::Timer { cb: cb });
        TimerRef(fd)
    }

    pub fn remove_timer(&mut self, timer_ref: TimerRef) {
        assert!(unsafe { libc::close(timer_ref.0) } != -1);
        self.fds.remove(&timer_ref.0);
    }

    pub fn add_signal(&mut self, sigs: &libc::sigset_t, cb: Box<FnMut(&mut EvLoopCtrl<Ctx>, &mut Ctx) -> ()>) -> SignalRef {
        // Block the signals we handle using signalfd() so they don't cause signal handlers to run
        assert!(unsafe { libc::sigprocmask(libc::SIG_BLOCK, sigs as *const libc::sigset_t, std::ptr::null_mut()) } != -1);
        let fd = unsafe { libc::signalfd(-1, sigs, libc::EFD_NONBLOCK) };
        assert!(fd != -1);
        self.fds.insert(fd, Handler::Signal { cb: cb });
        SignalRef(fd)
    }

    pub fn remove_signal(&mut self, signal_ref: SignalRef) {
        self.fds.remove(&signal_ref.0);
    }

    pub fn run(&mut self, mut ctx: Ctx) -> Ctx {
        let mut stop = false;
        while !stop {
            let mut fds: Vec<libc::pollfd> = Vec::with_capacity(self.fds.len());

            for (fd, handler) in self.fds.iter() {
                match handler {
                    &Handler::Timer{ .. } | &Handler::Signal{ .. } => {
                        fds.push(libc::pollfd { fd: *fd, events: libc::POLLIN, revents: 0 });
                    }
                    &Handler::Fd{ ref evs, .. } => {
                        fds.push(libc::pollfd { fd: *fd, events: evs.0, revents: 0 });
                    }
                }
            }

            assert!(unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as u64, -1) } != -1);

            for pollfd in fds {
                if pollfd.revents != 0 {

                    let mut remove_fd = false;
                    {
                        let mut controller = EvLoopCtrl {
                            stop_ref: &mut stop,
                            remove_self: &mut remove_fd,
                            new_fds: HashMap::with_capacity(0),
                            removed_fds: HashSet::with_capacity(0),
                        };

                        match self.fds.get_mut(&pollfd.fd).unwrap() {
                            &mut Handler::Timer{ ref mut cb } => {
                                let mut ret: u64 = 0;
                                assert!(unsafe { libc::read(pollfd.fd, &mut ret as *mut u64 as *mut libc::c_void, 8) } != -1);
                                cb(&mut controller, &mut ctx, ret);
                            }
                            &mut Handler::Signal{ ref mut cb } => {
                                let mut siginfo: libc::signalfd_siginfo = unsafe { std::mem::zeroed() };
                                assert!(unsafe {
                                    libc::read(
                                        pollfd.fd,
                                        &mut siginfo as *mut libc::signalfd_siginfo as *mut libc::c_void,
                                        std::mem::size_of::<libc::signalfd_siginfo>())
                                } != -1);
                                cb(&mut controller, &mut ctx);
                            }
                            &mut Handler::Fd{ ref mut cb, .. } => {
                                cb(FdEv(pollfd.revents), &mut controller, &mut ctx);
                            }
                        }

                        for (fd, h) in controller.new_fds.into_iter() {
                            self.fds.insert(fd, h);
                        }

                        for fd in controller.removed_fds.into_iter() {
                            self.fds.remove(&fd);
                        }
                    }

                    if remove_fd {
                        self.fds.remove(&pollfd.fd);
                    }
                }
            }
        }
        ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn it_works() {
        let count: Rc<RefCell<i32>> = Rc::new(RefCell::new(0));

        let mut ev_loop = EvLoop::new();
        {
            let count_clone = count.clone();
            ev_loop.add_timer(100, 100, Box::new(move |ctrl, _, _| {
                *count_clone.borrow_mut() += 1;
                println!("bump");
                if *(*count_clone).borrow() == 10 {
                    ctrl.stop();
                }
            }));
        }
        ev_loop.run(());

        assert_eq!(*(*count).borrow(), 10);
    }

    #[test]
    fn it_works_2() {
        struct Ctx {
            cb1: u64,
            cb2: u64,
            cb3: bool,
        }

        let mut ev_loop: EvLoop<Ctx> = EvLoop::new();

        ev_loop.add_timer(100, 100, Box::new(move |ctrl, ctx, amt| {
            assert!(ctx.cb1 == 0);
            ctx.cb1 = amt;
            ctrl.remove_self();
        }));

        ev_loop.add_timer(100, 100, Box::new(move |ctrl, ctx, amt| {
            assert!(ctx.cb2 == 0);
            ctx.cb2 = amt;
            ctrl.remove_self();
        }));

        {
            let fd = unsafe { timerfd_create(libc::CLOCK_MONOTONIC, libc::EFD_NONBLOCK) };
            assert!(fd != -1);

            let timeout_spec = mk_timespec(300);
            let period_spec = mk_timespec(300);
            let timerspec = itimerspec { it_interval: period_spec, it_value: timeout_spec };

            assert!(unsafe { timerfd_settime(fd, 0, &timerspec, std::ptr::null_mut()) } != -1);
            ev_loop.add_fd(fd, READ_EV, Box::new(move |_, ctrl, ctx| {
                assert!(ctx.cb3 == false);
                ctx.cb3 = true;
                ctrl.remove_self();
                ctrl.stop();
            }));
        }

        let ctx = ev_loop.run(Ctx { cb1: 0, cb2: 0, cb3: false });

        assert_eq!(ctx.cb1, 1);
        assert_eq!(ctx.cb2, 1);
        assert!(ctx.cb3);
    }
}

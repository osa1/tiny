use mio::Poll;
use mio::Token;
use mio::unix::EventedFd;
use mio::Ready;
use mio::PollOpt;
use std::os::unix::io::RawFd;

pub fn register_for_r(poll: &Poll, fd: RawFd) {
    let _ = poll.register(
        &EventedFd(&fd),
        Token(fd as usize),
        Ready::readable(),
        PollOpt::level(),
    );
}

pub fn reregister_for_r(poll: &Poll, fd: RawFd) {
    let _ = poll.reregister(
        &EventedFd(&fd),
        Token(fd as usize),
        Ready::readable(),
        PollOpt::level(),
    );
}

pub fn reregister_for_rw(poll: &Poll, fd: RawFd) {
    let _ = poll.reregister(
        &EventedFd(&fd),
        Token(fd as usize),
        Ready::readable() | Ready::writable(),
        PollOpt::level(),
    );
}

pub fn deregister(poll: &Poll, fd: RawFd) {
    let _ = poll.deregister(&EventedFd(&fd));
}

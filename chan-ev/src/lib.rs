//! This package provides a thin wrapper around `std::sync::mpsc` channel to be able to `select()`
//! or `poll()` channels.

extern crate libc;

use std::sync::mpsc::{Sender, Receiver, SendError};
use std::sync::mpsc;

// NOTE: eventfd is closed by the receiver. Sender never writes to it if the channel is closed.
// TODO: We will probably need some instances (Send, Clone, ...)

pub struct SenderEv<T> {
    sender: Sender<T>,
    ev_fd: libc::c_int,
}

pub struct ReceiverEv<T> {
    receiver: Receiver<T>,
    ev_fd: libc::c_int,
}

/// Used to bump the eventfd.
static COUNTER_INCR : [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 1];

impl<T> SenderEv<T> {
    /// See documentation of `std::sync::mpsc::Sender::send`.
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        try!(self.sender.send(t));

        // TODO: check return values
        unsafe { assert!(libc::write(self.ev_fd, COUNTER_INCR.as_ptr() as *const libc::c_void, 8) != -1) };

        Ok(())
    }
}

impl<T> ReceiverEv<T> {
    /// Get the eventfd to `select()` or `poll()`.
    pub fn get_ev_fd(&self) -> libc::c_int {
        self.ev_fd
    }

    /// Read from the eventfd. Does not block.
    pub fn read_ev_fd(&self) -> Option<i64> {
        let mut buf : [u8; 8] = [0; 8];
        let read_ret =
            unsafe { libc::read(self.ev_fd, buf.as_mut_ptr() as *mut libc::c_void, 8) };

        if read_ret == -1 {
            None
        } else {
            Some(
                ((buf[7] as i64) <<  0) +
                ((buf[6] as i64) <<  8) +
                ((buf[5] as i64) << 16) +
                ((buf[4] as i64) << 24) +
                ((buf[3] as i64) << 32) +
                ((buf[2] as i64) << 40) +
                ((buf[1] as i64) << 48) +
                ((buf[0] as i64) << 56))
        }
    }

    /// Get the actual receiver.
    pub fn get_receiver(&self) -> &Receiver<T> {
        &self.receiver
    }
}

/// Creates a new asynchronous channel with an `eventfd`, returning the sender/receiver halves.
///
/// All data sent on the sender will become available on the receiver, and no send will block the
/// calling thread (this channel has an "infinite buffer").
pub fn channel<T>() -> (SenderEv<T>, ReceiverEv<T>) {
    let (sender, receiver) = mpsc::channel();
    let ev_fd = unsafe { libc::eventfd(0, libc::EFD_NONBLOCK) };
    assert!(ev_fd != -1);

    let sender = SenderEv {
        sender: sender,
        ev_fd: ev_fd,
    };

    let receiver = ReceiverEv {
        receiver: receiver,
        ev_fd: ev_fd,
    };

    (sender, receiver)
}

#[cfg(test)]
mod tests {

    use libc;
    use std::sync::mpsc::RecvTimeoutError;
    use std::time::Duration;
    use super::*;

    #[test]
    fn it_works() {
        let receiver = {
            let (sender, receiver) : (SenderEv<String>, ReceiverEv<String>) = channel();

            let mut pollfds : [libc::pollfd; 1] =
                [ libc::pollfd {
                      fd: receiver.get_ev_fd(),
                      events: libc::POLLIN,
                      revents: 0,
                  } ];

            assert!(unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, 10) } != -1);
            assert!(pollfds[0].revents & libc::POLLIN == 0);
            assert_eq!(receiver.read_ev_fd(), None);

            sender.send("message 1".to_string()).unwrap();
            pollfds[0].revents = 0;
            assert!(unsafe { libc::poll(pollfds.as_mut_ptr(), pollfds.len() as libc::nfds_t, 10) } != -1);
            assert!(pollfds[0].revents & libc::POLLIN != 0);
            assert_eq!(receiver.read_ev_fd(), Some(1));

            sender.send("message 2".to_string()).unwrap();
            sender.send("message 3".to_string()).unwrap();
            assert_eq!(receiver.read_ev_fd(), Some(2));

            assert_eq!(receiver.get_receiver().recv_timeout(Duration::from_millis(0)),
                       Ok("message 1".to_string()));
            assert_eq!(receiver.get_receiver().recv_timeout(Duration::from_millis(0)),
                       Ok("message 2".to_string()));
            assert_eq!(receiver.get_receiver().recv_timeout(Duration::from_millis(0)),
                       Ok("message 3".to_string()));
            assert_eq!(receiver.get_receiver().recv_timeout(Duration::from_millis(0)),
                       Err(RecvTimeoutError::Timeout));
            receiver
        };

        assert_eq!(receiver.get_receiver().recv_timeout(Duration::from_millis(0)),
                   Err(RecvTimeoutError::Disconnected));
    }
}

extern crate libc;
extern crate mio;
extern crate termbox_simple;

use mio::Events;
use mio::Poll;
use mio::PollOpt;
use mio::Ready;
use mio::Token;
use mio::unix::EventedFd;

use termbox_simple::*;

fn main() {
    let poll = Poll::new().unwrap();
    poll.register(
        &EventedFd(&libc::STDIN_FILENO),
        Token(libc::STDIN_FILENO as usize),
        Ready::readable(),
        PollOpt::level()).unwrap();

    let mut termbox = Termbox::init().unwrap();
    let mut events = Events::with_capacity(10);
    'mainloop:
    loop {
        match poll.poll(&mut events, None) {
            Err(_) => {
                termbox.resize();
            }
            Ok(_) => {
                let mut buf : Vec<u8> = vec![];
                if read_input_events(&mut buf) {
                    let string = format!("{:?}", buf);
                    termbox.clear();
                    if buf == vec![27] {
                        break 'mainloop;
                    }
                    for (char_idx, char) in string.chars().enumerate() {
                        termbox.change_cell(char_idx as libc::c_int, 0, char, TB_WHITE, TB_DEFAULT);
                    }
                    termbox.present();
                }
            }
        }
    }
}

/// Read stdin contents if it's ready for reading. Returns true when it was able
/// to read. Buffer is not modified when return value is 0.
fn read_input_events(buf : &mut Vec<u8>) -> bool {
    let mut bytes_available : i32 = 0; // this really needs to be a 32-bit value
    let ioctl_ret = unsafe { libc::ioctl(libc::STDIN_FILENO, libc::FIONREAD, &mut bytes_available) };
    // println!("ioctl_ret: {}", ioctl_ret);
    // println!("bytes_available: {}", bytes_available);
    if ioctl_ret < 0 || bytes_available == 0 {
        false
    } else {
        buf.clear();
        buf.reserve(bytes_available as usize);

        let buf_ptr : *mut libc::c_void = buf.as_ptr() as *mut libc::c_void;
        let bytes_read = unsafe { libc::read(libc::STDIN_FILENO, buf_ptr, bytes_available as usize) };
        debug_assert!(bytes_read == bytes_available as isize);

        unsafe { buf.set_len(bytes_read as usize); }
        true
    }
}

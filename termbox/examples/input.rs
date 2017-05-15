extern crate ev_loop;
extern crate libc;
extern crate termbox_simple;

use termbox_simple::*;
use ev_loop::{EvLoop, READ_EV};

fn main() {
    let mut ev_loop: EvLoop<Termbox> = EvLoop::new();

    ev_loop.add_fd(libc::STDIN_FILENO, READ_EV, Box::new(|_, ctrl, termbox| {
        let mut buf : Vec<u8> = vec![];
        if read_input_events(&mut buf) {
            let string = format!("{:?}", buf);
            termbox.clear();
            if buf == vec![27] {
                ctrl.stop();
            }
            for (char_idx, char) in string.chars().enumerate() {
                termbox.change_cell(char_idx as libc::c_int, 0, char, TB_WHITE, TB_DEFAULT);
            }
            termbox.present();
        }
    }));

    {
        let mut sig_mask: libc::sigset_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigemptyset(&mut sig_mask as *mut libc::sigset_t);
            libc::sigaddset(&mut sig_mask as *mut libc::sigset_t, libc::SIGWINCH);
        };

        ev_loop.add_signal(&sig_mask, Box::new(|_, termbox| {
            termbox.resize();
        }));
    }

    ev_loop.run(Termbox::init().unwrap());
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

extern crate libc;
extern crate termbox_simple;

use termbox_simple::*;

use std::thread::sleep;
use std::time::Duration;

struct Termbox {}

impl Termbox {
    fn new() -> Termbox {
        unsafe { tb_init(); }
        Termbox {}
    }
}

impl Drop for Termbox {
    fn drop(&mut self) {
        unsafe { tb_shutdown(); }
        println!("Dropped");
    }
}

fn main() {
    let termbox = Termbox::new();

    // Set up the descriptors for select()
    let mut fd_set : libc::fd_set = unsafe { std::mem::zeroed() };

    unsafe { libc::FD_SET(libc::STDIN_FILENO, &mut fd_set); }

    loop {
        let mut fd_set_ = fd_set.clone();
        let ret =
            unsafe {
                libc::select(1,
                             &mut fd_set_,           // read fds
                             std::ptr::null_mut(),   // write fds
                             std::ptr::null_mut(),   // error fds
                             std::ptr::null_mut())   // timeval
            };

        if ret == -1 {
            unsafe { tb_resize(); }
        } else if unsafe { libc::FD_ISSET(0, &mut fd_set_) } {
            let mut buf : Vec<u8> = vec![];
            if read_input_events(&mut buf) {
                let string = format!("{:?}", buf);
                unsafe { tb_clear(); }
                if buf == vec![27] { break; }
                for (char_idx, char) in string.chars().enumerate() {
                    unsafe { tb_change_cell(char_idx as libc::c_int,
                                            0, char as u32, TB_WHITE, TB_DEFAULT); }
                }
                unsafe { tb_present(); }
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

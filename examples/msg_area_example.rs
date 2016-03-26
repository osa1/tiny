extern crate libc;
extern crate rustbox;
extern crate tiny;

use std::borrow::Borrow;
use std::ffi::CStr;
use std::mem;

use tiny::tui::{TUI, TUIRet};

fn loop_() -> Option<String> {
    let mut tui = TUI::new();

    // I'm using select() here to test for signals/interrupts. Namely, SIGWINCH
    // needs to be handled somehow for resizing.

    let mut fd_set : libc::fd_set = unsafe { mem::zeroed() };
    unsafe { libc::FD_SET(0, &mut fd_set); }
    let nfds = 1; // stdin + 1

    loop {
        tui.draw();

        let mut fd_set_ = fd_set.clone();
        let ret = unsafe {
            libc::select(nfds,
                         &mut fd_set_,           // read fds
                         std::ptr::null_mut(),   // write fds
                         std::ptr::null_mut(),   // error fds
                         std::ptr::null_mut())   // timeval
        };

        // if ret == -1 {
        //     let err_c_msg =
        //         unsafe { CStr::from_ptr(libc::strerror(*libc::__errno_location())) }
        //             .to_string_lossy();

        //     tui.show_conn_error(
        //         format!("Internal error: select() failed: {}", err_c_msg).borrow());
        // }

        if unsafe { ret == -1 || libc::FD_ISSET(0, &mut fd_set_) } {
            match tui.keypressed() {
                TUIRet::SendMsg(cmd) => {
                    tui.show_outgoing_msg(cmd.into_iter().collect::<String>().borrow());
                },
                TUIRet::Abort => {
                    return None;
                },
                _ => {}
            }
        }
    }
}

fn main() {
    loop_().map(|err| println!("{}", err));
}

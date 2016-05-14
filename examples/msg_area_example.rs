extern crate libc;
extern crate rustbox;
extern crate time;
extern crate tiny;

use std::mem;

use tiny::tui::{TUI, TUIRet, MsgTarget};

fn loop_() -> Option<String> {
    let mut tui = TUI::new();
    tui.new_server_tab("debug");

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

        if unsafe { ret == -1 || libc::FD_ISSET(0, &mut fd_set_) } {
            match tui.keypressed_peek() {
                TUIRet::Input { msg, .. } => {
                    tui.add_msg(&msg.into_iter().collect::<String>(),
                                &time::now(),
                                &MsgTarget::Server { serv_name: "debug" });
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

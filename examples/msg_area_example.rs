extern crate tiny;
extern crate rustbox;

use std::borrow::Borrow;

use tiny::tui::{TUI, TUIRet};

fn loop_() -> Option<String> {
    let mut tui = TUI::new();

    loop {
        match tui.idle_loop() {
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

fn main() {
    loop_().map(|err| println!("{}", err));
}

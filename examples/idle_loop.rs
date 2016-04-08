extern crate rustbox;
extern crate tiny;

use std::borrow::Borrow;

use tiny::tui::{TUI, TUIRet};

fn loop_() -> Option<String> {
    let mut tui = TUI::new();
    tui.new_server_tab("debug".to_string());

    loop {
        match tui.idle_loop() {
            TUIRet::Input { serv_name, pfx, msg } => {
                tui.show_msg(&msg.into_iter().collect::<String>(), &serv_name, pfx.as_ref());
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

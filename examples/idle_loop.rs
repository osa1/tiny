extern crate rustbox;
extern crate time;
extern crate tiny;

use tiny::tui::{TUI, TUIRet, MsgTarget};

fn loop_()  {
    let mut tui = TUI::new();
    tui.new_server_tab("debug");

    loop {
        match tui.idle_loop() {
            TUIRet::Input { msg, from } => {
                tui.add_msg(&msg.into_iter().collect::<String>(),
                            &time::now(),
                            &MsgTarget::Server { serv_name: "debug" });
            },
            TUIRet::Abort => {
                break;
            },
            _ => {}
        }
    }
}

fn main() {
    loop_();
}

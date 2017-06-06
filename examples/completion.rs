extern crate ev_loop;
extern crate libc;
extern crate term_input;
extern crate termbox_simple;
extern crate time;
extern crate tiny;

use ev_loop::{EvLoop, READ_EV};
use term_input::{Input, Event};

use std::fs::File;
use std::io::Read;

use tiny::tui::{TUI, TUIRet, MsgTarget, Timestamp};

fn main() {
    let mut tui = TUI::new();
    tui.new_server_tab("debug");
    let debug_tab = MsgTarget::Server { serv_name: "debug" };

    tui.add_msg("Loading word list for auto-completion ...",
                Timestamp::now(),
                &debug_tab);
    tui.draw();

    {
        let mut contents = String::new();
        let mut file = File::open("/usr/share/dict/american").unwrap();
        file.read_to_string(&mut contents).unwrap();
        for word in contents.lines() {
            tui.add_nick(word, None, &debug_tab);
        }
    }

    tui.add_msg("Done.", Timestamp::now(), &debug_tab);
    tui.draw();

    let mut ev_loop: EvLoop<TUI> = EvLoop::new();

    let mut input = Input::new();
    let mut ev_buffer: Vec<Event> = Vec::new();

    ev_loop.add_fd(libc::STDIN_FILENO, READ_EV, Box::new(move |_, ctrl, tui| {
        input.read_input_events(&mut ev_buffer);
        for ev in ev_buffer.drain(0..) {
            match tui.handle_input_event(ev) {
                TUIRet::Input { msg, .. } => {
                    tui.add_msg(&msg.into_iter().collect::<String>(),
                    Timestamp::now(),
                    &debug_tab);
                },
                TUIRet::Abort => {
                    ctrl.stop();
                },
                _ => {}
            }
        }
        tui.draw();
    }));

    ev_loop.run(tui);
}

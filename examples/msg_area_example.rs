extern crate ev_loop;
extern crate libc;
extern crate term_input;
extern crate termbox_simple;
extern crate time;
extern crate tiny;

use ev_loop::{EvLoop, READ_EV};
use term_input::{Input, Event};
use tiny::tui::{TUI, TUIRet, MsgTarget, Timestamp};

fn main() {
    let mut tui = TUI::new();
    tui.new_server_tab("debug");
    tui.draw();

    let mut ev_loop: EvLoop<TUI> = EvLoop::new();

    {
        let mut ev_buffer: Vec<Event> = Vec::new();
        let mut input = Input::new();
        ev_loop.add_fd(libc::STDIN_FILENO, READ_EV, Box::new(move |_, ctrl, tui| {
            input.read_input_events(&mut ev_buffer);
            for ev in ev_buffer.drain(0..) {
                match tui.handle_input_event(ev) {
                    TUIRet::Input { msg, .. } => {
                        tui.add_msg(&msg.into_iter().collect::<String>(),
                                    Timestamp::now(),
                                    &MsgTarget::Server { serv_name: "debug" });
                    },
                    TUIRet::Abort => {
                        ctrl.stop();
                    },
                    _ => {}
                }
            }
            tui.draw();
        }));
    }

    ev_loop.run(tui);
}

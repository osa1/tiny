// Open a lot of tabs. 10 servers tabs, each one having 3 channels.

extern crate ev_loop;
extern crate libc;
extern crate term_input;
extern crate termbox_simple;
extern crate time;
extern crate tiny;

use ev_loop::{EvLoop, READ_EV};
use term_input::{Input, Event};
use tiny::tui::tabbed::MsgSource;
use tiny::tui::{TUI, TUIRet, MsgTarget};

fn main() {
    let mut tui = TUI::new();

    for serv_idx in 0 .. 10 {
        let server = format!("server_{}", serv_idx);
        tui.new_server_tab(&server);
        for chan_idx in 0 .. 3 {
            tui.new_chan_tab(&server, &format!("chan_{}", chan_idx));
        }
    }

    tui.draw();

    let mut ev_loop: EvLoop<TUI> = EvLoop::new();

    {
        let mut ev_buffer: Vec<Event> = Vec::new();
        let mut input = Input::new();
        ev_loop.add_fd(libc::STDIN_FILENO, READ_EV, Box::new(move |_, ctrl, tui| {
            input.read_input_events(&mut ev_buffer);
            for ev in ev_buffer.drain(0..) {
                match tui.handle_input_event(ev) {
                    TUIRet::Input { msg, from } => {
                        let msg_string = msg.iter().cloned().collect::<String>();
                        match from {
                            MsgSource::Chan { serv_name, chan_name } => {
                                tui.add_privmsg(
                                    "me",
                                    &msg_string,
                                    &time::now(),
                                    &MsgTarget::Chan { serv_name: &serv_name, chan_name: &chan_name });
                            }

                            MsgSource::Serv { .. } => {
                                tui.add_client_err_msg(
                                    "Can't send PRIVMSG to a server.",
                                    &MsgTarget::CurrentTab);
                            }

                            _ => {}
                        }
                    }
                    TUIRet::Abort => {
                        ctrl.stop();
                    }
                    _ => {}
                }
            }
            tui.draw();
        }));
    }

    ev_loop.run(tui);
}

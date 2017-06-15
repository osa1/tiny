// Open a lot of tabs. 10 servers tabs, each one having 3 channels.

extern crate libc;
extern crate mio;
extern crate term_input;
extern crate termbox_simple;
extern crate tiny;

use mio::Events;
use mio::Poll;
use mio::PollOpt;
use mio::Ready;
use mio::Token;
use mio::unix::EventedFd;
use term_input::{Input, Event};
use tiny::tui::tabbed::MsgSource;
use tiny::tui::tabbed::TabStyle;
use tiny::tui::{TUI, TUIRet, MsgTarget, Timestamp};

fn main() {
    let mut tui = TUI::new();

    for serv_idx in 0 .. 10 {
        let server = format!("server_{}", serv_idx);
        tui.new_server_tab(&server);

        tui.new_chan_tab(&server, "chan_0");
        tui.set_tab_style(TabStyle::NewMsg, &MsgTarget::Chan {
            serv_name: &server,
            chan_name: "chan_0"
        });

        tui.new_chan_tab(&server, "chan_1");
        tui.set_tab_style(TabStyle::Highlight, &MsgTarget::Chan {
            serv_name: &server,
            chan_name: "chan_1"
        });

        tui.new_chan_tab(&server, "chan_2");
    }

    tui.draw();

    let poll = Poll::new().unwrap();
    poll.register(
        &EventedFd(&libc::STDIN_FILENO),
        Token(libc::STDIN_FILENO as usize),
        Ready::readable(),
        PollOpt::level()).unwrap();

    let mut ev_buffer: Vec<Event> = Vec::new();
    let mut input = Input::new();
    let mut events = Events::with_capacity(10);
    'mainloop:
    loop {
        match poll.poll(&mut events, None) {
            Err(_) => {
                tui.resize();
                tui.draw();
            }
            Ok(_) => {
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
                                        Timestamp::now(),
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
                            break 'mainloop;
                        }
                        _ => {}
                    }
                }
                tui.draw();
            }
        }
    }
}

// In a chat window add dozens of nicks, each printing some random lines.

extern crate libc;
extern crate mio;
extern crate term_input;
extern crate termbox_simple;
extern crate time;
extern crate tiny;

use mio::Events;
use mio::Poll;
use mio::PollOpt;
use mio::Ready;
use mio::Token;
use mio::unix::EventedFd;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;

use term_input::{Input, Event};
use tiny::config::Colors;
use tiny::tui::{TUI, TUIRet, MsgTarget, Timestamp};

fn main() {
    let chan_target = MsgTarget::Chan { serv_name: "debug", chan_name: "chan" };

    let mut tui = TUI::new(Colors::default());
    tui.new_server_tab("debug");
    tui.new_chan_tab("debug", "chan");
    tui.show_topic("This is channel topic", Timestamp::now(), &chan_target);
    tui.draw();

    {
        let mut text = String::new();
        let mut file = File::open("test/lipsum.txt").unwrap();
        file.read_to_string(&mut text).unwrap();

        for (line_idx, line) in text.lines().enumerate() {
            let now = Timestamp::now();
            let nick = format!("nick_{}", line_idx);
            tui.add_nick(&nick, Some(now), &chan_target);
            tui.add_privmsg(&nick, line, now, &chan_target);
        }
    }

    tui.set_nick("debug", Rc::new("some_long_nick_name____".to_owned()));
    tui.draw();

    let poll = Poll::new().unwrap();
    poll.register(
        &EventedFd(&libc::STDIN_FILENO),
        Token(libc::STDIN_FILENO as usize),
        Ready::readable(),
        PollOpt::level()).unwrap();

    let mut input = Input::new();
    let mut ev_buffer: Vec<Event> = Vec::new();
    let mut events = Events::with_capacity(10);
    'mainloop:
    loop {
        match poll.poll(&mut events, None) {
            Err(_) => {
                // usually SIGWINCH, which is caught by term_input
                tui.resize();
                tui.draw();
            }
            Ok(_) => {
                input.read_input_events(&mut ev_buffer);
                for ev in ev_buffer.drain(0..) {
                    match tui.handle_input_event(ev) {
                        TUIRet::Input { msg, from } => {
                            if msg == "/clear".chars().collect::<Vec<char>>() {
                                tui.clear(&from.to_target())
                            } else {
                                tui.add_msg(&msg.into_iter().collect::<String>(),
                                            Timestamp::now(),
                                            &MsgTarget::Server { serv_name: "debug" });
                            }
                        },
                        TUIRet::Abort => {
                            break 'mainloop;
                        },
                        TUIRet::EventIgnored(Event::FocusGained) => {
                            tui.add_msg("focus gained",
                                        Timestamp::now(),
                                        &MsgTarget::Server { serv_name: "debug" });
                        },
                        TUIRet::EventIgnored(Event::FocusLost) => {
                            tui.add_msg("focus lost",
                                        Timestamp::now(),
                                        &MsgTarget::Server { serv_name: "debug" });
                        },
                        _ => {}
                    }
                }
                tui.draw();
            }
        }
    }
}

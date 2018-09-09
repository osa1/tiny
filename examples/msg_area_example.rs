extern crate libc;
extern crate mio;
extern crate term_input;
extern crate termbox_simple;
extern crate time;
extern crate tiny;

use mio::unix::EventedFd;
use mio::Events;
use mio::Poll;
use mio::PollOpt;
use mio::Ready;
use mio::Token;

use term_input::{Event, Input};
use tiny::config::Colors;
use tiny::tui::{MsgTarget, TUIRet, Timestamp, TUI};

fn main() {
    let mut tui = TUI::new(Colors::default(), true);
    tui.new_server_tab("debug");
    tui.draw();

    let poll = Poll::new().unwrap();
    poll.register(
        &EventedFd(&libc::STDIN_FILENO),
        Token(libc::STDIN_FILENO as usize),
        Ready::readable(),
        PollOpt::level(),
    )
    .unwrap();

    let mut ev_buffer: Vec<Event> = Vec::new();
    let mut input = Input::new();
    let mut events = Events::with_capacity(10);
    'mainloop: loop {
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
                        TUIRet::Input { msg, .. } => {
                            tui.add_msg(
                                &msg.into_iter().collect::<String>(),
                                Timestamp::now(),
                                &MsgTarget::Server { serv_name: "debug" },
                            );
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

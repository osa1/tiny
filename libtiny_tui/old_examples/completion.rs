use mio::{unix::EventedFd, Events, Poll, PollOpt, Ready, Token};
use std::{fs::File, io::Read};

use libtiny_tui::{Colors, MsgTarget, TUIRet, TUI};
use term_input::{Event, Input};

fn main() {
    let mut tui = TUI::new(Colors::default());
    tui.new_server_tab("debug");
    let debug_tab = MsgTarget::Server { serv_name: "debug" };

    tui.add_msg(
        "Loading word list for auto-completion ...",
        time::now(),
        &debug_tab,
    );
    tui.draw();

    {
        let mut contents = String::new();
        let mut file = File::open("/usr/share/dict/american-english").unwrap();
        file.read_to_string(&mut contents).unwrap();
        for word in contents.lines() {
            tui.add_nick(word, None, &debug_tab);
        }
    }

    tui.add_msg("Done.", time::now(), &debug_tab);
    tui.draw();

    let poll = Poll::new().unwrap();
    poll.register(
        &EventedFd(&libc::STDIN_FILENO),
        Token(libc::STDIN_FILENO as usize),
        Ready::readable(),
        PollOpt::level(),
    )
    .unwrap();

    let mut input = Input::new();
    let mut ev_buffer: Vec<Event> = Vec::new();
    let mut events = Events::with_capacity(10);
    'mainloop: loop {
        match poll.poll(&mut events, None) {
            Err(_) => {
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
                                time::now(),
                                &debug_tab,
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

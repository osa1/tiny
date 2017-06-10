// In a chat window add dozens of nicks, each printing some random lines.

extern crate ev_loop;
extern crate libc;
extern crate term_input;
extern crate termbox_simple;
extern crate time;
extern crate tiny;

use std::fs::File;
use std::io::Read;

use ev_loop::{EvLoop, READ_EV};
use term_input::{Input, Event};
use tiny::tui::{TUI, TUIRet, MsgTarget, Timestamp};

fn main() {
    let chan_target = MsgTarget::Chan { serv_name: "debug", chan_name: "chan" };

    let mut tui = TUI::new();
    tui.new_server_tab("debug");
    tui.new_chan_tab("debug", "chan");
    tui.show_topic("This is channel topic", Timestamp::now(), &chan_target);
    tui.draw();

    let mut ev_loop: EvLoop<TUI> = EvLoop::new();

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

    tui.draw();

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

    {
        let mut sig_mask: libc::sigset_t = unsafe { std::mem::zeroed() };
        unsafe {
            libc::sigemptyset(&mut sig_mask as *mut libc::sigset_t);
            libc::sigaddset(&mut sig_mask as *mut libc::sigset_t, libc::SIGWINCH);
        };

        ev_loop.add_signal(&sig_mask, Box::new(|_, tui| {
            tui.resize();
            tui.draw();
        }));

        tui.draw();
    }

    ev_loop.run(tui);
}

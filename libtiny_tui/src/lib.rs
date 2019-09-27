mod config;
mod exit_dialogue;
mod messaging;
// FIXME: This is "pub" to be able to use in an example
#[doc(hidden)]
pub mod msg_area;
mod notifier;
mod tab;
mod termbox;
mod text_field;
mod trie;
mod tui;
mod utils;
mod widget;

pub use libtiny_ui::{MsgSource, MsgTarget};

use crate::config::Colors;

use std::cell::RefCell;
use futures_util::stream::StreamExt;
use std::rc::{Rc, Weak};
use term_input::Input;
use tokio::runtime::current_thread::Runtime;
use tokio::sync::mpsc;

pub struct TUI {
    inner: Weak<RefCell<tui::TUI>>,
}

pub enum Event {
    Blah,
}

impl TUI {
    pub fn run(colors: Colors, runtime: &mut Runtime) -> (TUI, mpsc::Receiver<Event>) {
        let tui = Rc::new(RefCell::new(tui::TUI::new(colors)));
        let inner = Rc::downgrade(&tui);

        let (snd_ev, rcv_ev) = mpsc::channel(10);

        // Spawn input handler task
        runtime.spawn(input_handler(tui, snd_ev));

        (TUI { inner }, rcv_ev)
    }
}

async fn input_handler(tui: Rc<RefCell<tui::TUI>>, mut snd_ev: mpsc::Sender<Event>) {
    let mut input = Input::new();
    while let Some(mb_ev) = input.next().await {
        use tui::TUIRet::*;
        match mb_ev {
            Err(io_err) => {
                eprintln!("term input error: {:?}", io_err);
                return;
            }
            Ok(ev) => {
                match tui.borrow_mut().handle_input_event(ev) {
                    Abort => {
                        return;
                    }
                    KeyHandled | KeyIgnored(_) | EventIgnored(_) => {}
                    Input { msg, from } => {}
                    Lines { lines, from } => {}
                }
            }
        }
        tui.borrow_mut().draw();
    }
}

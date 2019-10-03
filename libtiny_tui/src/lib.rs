#![cfg_attr(test, feature(test))]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::cognitive_complexity)]

mod config;
mod exit_dialogue;
mod messaging;
// FIXME: This is "pub" to be able to use in an example
#[doc(hidden)]
pub mod msg_area;
mod notifier;
mod statusline;
mod tab;
mod termbox;
mod text_field;
mod trie;
mod tui;
mod utils;
mod widget;

pub use crate::config::Colors;
pub use crate::tab::TabStyle;
pub use libtiny_ui::*;

use futures_util::stream::StreamExt;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use term_input::Input;
use time::Tm;
use tokio::runtime::current_thread::Runtime;
use tokio::sync::mpsc;
use tokio_net::signal::unix::{signal, SignalKind};

#[macro_use]
extern crate log;

#[derive(Clone)]
pub struct TUI {
    inner: Weak<RefCell<tui::TUI>>,
}

impl TUI {
    pub fn run(colors: Colors, runtime: &mut Runtime) -> (TUI, mpsc::Receiver<Event>) {
        let tui = Rc::new(RefCell::new(tui::TUI::new(colors)));
        let inner = Rc::downgrade(&tui);

        let (snd_ev, rcv_ev) = mpsc::channel(10);

        // Spawn input handler task
        runtime.spawn(input_handler(tui, snd_ev));

        // Spawn SIGWINCH handler
        runtime.spawn(sigwinch_handler(inner.clone()));

        (TUI { inner }, rcv_ev)
    }
}

async fn sigwinch_handler(tui: Weak<RefCell<tui::TUI>>) {
    match signal(SignalKind::window_change()) {
        Err(err) => {
            debug!("Can't install SIGWINCH handler: {:?}", err);
        }
        Ok(mut stream) => {
            while let Some(()) = stream.next().await {
                match tui.upgrade() {
                    None => {
                        return;
                    }
                    Some(tui) => {
                        tui.borrow_mut().resize();
                    }
                }
            }
        }
    }
}

async fn input_handler(tui: Rc<RefCell<tui::TUI>>, mut snd_ev: mpsc::Sender<Event>) {
    let mut input = Input::new();
    while let Some(mb_ev) = input.next().await {
        use tui::TUIRet::*;
        match mb_ev {
            Err(io_err) => {
                debug!("term_input error: {:?}", io_err);
                return;
            }
            Ok(ev) => {
                let tui_ret = tui.borrow_mut().handle_input_event(ev);
                match tui_ret {
                    Abort => {
                        snd_ev.try_send(Event::Abort).unwrap();
                        return;
                    }
                    KeyHandled | KeyIgnored(_) | EventIgnored(_) => {}
                    Input { msg, from } => {
                        if msg[0] == '/' {
                            // Handle TUI commands, send others to downstream
                            let cmd: String = (&msg[1..]).iter().collect();
                            let handled = tui.borrow_mut().try_handle_cmd(&cmd, &from);
                            if !handled {
                                snd_ev.try_send(Event::Cmd { cmd, source: from }).unwrap();
                            }
                        } else {
                            snd_ev
                                .try_send(Event::Msg {
                                    msg: msg.into_iter().collect(),
                                    source: from,
                                })
                                .unwrap();
                        }
                    }
                    Lines { lines, from } => {
                        snd_ev
                            .try_send(Event::Lines {
                                lines,
                                source: from,
                            })
                            .unwrap();
                    }
                }
            }
        }
        tui.borrow_mut().draw();
    }
}

macro_rules! delegate {
    ( $name:ident ( $( $x:ident: $t:ty, )* ) ) => {
        fn $name(&self, $($x: $t,)*) {
            if let Some(inner) = self.inner.upgrade() {
                inner.borrow_mut().$name( $( $x, )* );
            }
        }
    }
}

macro_rules! delegate_pub {
    ( $name:ident ( $( $x:ident: $t:ty, )* ) ) => {
        pub fn $name(&self, $($x: $t,)*) {
            if let Some(inner) = self.inner.upgrade() {
                inner.borrow_mut().$name( $( $x, )* );
            }
        }
    }
}

impl UI for TUI {
    delegate!(draw());
    delegate!(new_server_tab(serv_name: &str,));
    delegate!(close_server_tab(serv_name: &str,));
    delegate!(new_chan_tab(serv_name: &str, chan: &str,));
    delegate!(close_chan_tab(serv_name: &str, chan: &str,));
    delegate!(close_user_tab(serv_name: &str, nick: &str,));
    delegate!(add_client_msg(msg: &str, target: &MsgTarget,));
    delegate!(add_msg(msg: &str, ts: Tm, target: &MsgTarget,));
    delegate!(add_err_msg(msg: &str, ts: Tm, target: &MsgTarget,));
    delegate!(add_client_err_msg(msg: &str, target: &MsgTarget,));
    delegate!(clear_nicks(serv_name: &str,));
    delegate!(set_nick(serv_name: &str, new_nick: &str,));
    delegate!(add_privmsg(
        sender: &str,
        msg: &str,
        ts: Tm,
        target: &MsgTarget,
        highlight: bool,
        is_action: bool,
    ));
    delegate!(add_nick(nick: &str, ts: Option<Tm>, target: &MsgTarget,));
    delegate!(remove_nick(nick: &str, ts: Option<Tm>, target: &MsgTarget,));
    delegate!(rename_nick(
        old_nick: &str,
        new_nick: &str,
        ts: Tm,
        target: &MsgTarget,
    ));
    delegate!(set_topic(
        topic: &str,
        ts: Tm,
        serv_name: &str,
        chan_name: &str,
    ));
    delegate!(set_tab_style(style: TabStyle, target: &MsgTarget,));

    fn user_tab_exists(&self, serv_name: &str, nick: &str) -> bool {
        match self.inner.upgrade() {
            Some(tui) => tui.borrow().user_tab_exists(serv_name, nick),
            None => false,
        }
    }
}

impl TUI {
    delegate_pub!(set_colors(colors: Colors,));
}

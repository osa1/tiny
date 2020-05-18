#![cfg_attr(test, feature(test))]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::cognitive_complexity)]
#![feature(track_caller)]

mod config;
mod editor;
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

#[cfg(test)]
mod tests;

pub use crate::tab::TabStyle;
pub use libtiny_ui::*;

use futures::select;
use futures::stream::StreamExt;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use time::Tm;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio::task::spawn_local;

#[macro_use]
extern crate log;

#[derive(Clone)]
pub struct TUI {
    inner: Weak<RefCell<tui::TUI>>,
}

impl TUI {
    pub fn run(config_path: PathBuf) -> (TUI, mpsc::Receiver<Event>) {
        let (snd_editor_out, rcv_editor_out) = mpsc::channel(1);

        let tui = Rc::new(RefCell::new(tui::TUI::new(config_path, snd_editor_out)));
        let inner = Rc::downgrade(&tui);

        let (snd_ev, rcv_ev) = mpsc::channel(10);

        // For SIGWINCH handler
        let (snd_abort, rcv_abort) = mpsc::channel::<()>(1);

        // Spawn SIGWINCH handler
        spawn_local(sigwinch_handler(inner.clone(), rcv_abort));

        // Spawn input handler task
        spawn_local(input_handler(tui, snd_ev, snd_abort, rcv_editor_out));

        (TUI { inner }, rcv_ev)
    }
}

async fn sigwinch_handler(tui: Weak<RefCell<tui::TUI>>, rcv_abort: mpsc::Receiver<()>) {
    let stream = match signal(SignalKind::window_change()) {
        Err(err) => {
            debug!("Can't install SIGWINCH handler: {:?}", err);
            return;
        }
        Ok(stream) => stream,
    };

    let mut stream_fused = stream.fuse();
    let mut rcv_abort_fused = rcv_abort.fuse();

    loop {
        select! {
            _ = stream_fused.next() => {
                match tui.upgrade() {
                    None => {
                        return;
                    }
                    Some(tui) => {
                        tui.borrow_mut().resize();
                    }
                }
            },
            _ = rcv_abort_fused.next() => {
                return;
            }
        }
    }
}

async fn input_handler(
    tui: Rc<RefCell<tui::TUI>>,
    mut snd_ev: mpsc::Sender<Event>,
    mut snd_abort: mpsc::Sender<()>,
    rcv_editor_out: mpsc::Receiver<editor::Result>,
) {
    let mut rcv_editor_out = rcv_editor_out.fuse();
    let mut event_stream = crossterm::event::EventStream::new().fuse();

    loop {
        select! {
            in_ev = event_stream.next() => {
                let abort = handle_input_event(&*tui, &mut snd_ev, &mut snd_abort, in_ev);
                if abort {
                    return;
                }
            }
            editor_out = rcv_editor_out.next() => {
                handle_editor_out(&*tui, &mut snd_ev, editor_out);
            }
        }
    }
}

fn handle_input_event(
    tui: &RefCell<tui::TUI>,
    snd_ev: &mut mpsc::Sender<Event>,
    snd_abort: &mut mpsc::Sender<()>,
    ev: Option<Result<crossterm::event::Event, crossterm::ErrorKind>>,
) -> bool {
    use tui::TUIRet::*;
    match ev {
        None => {
            debug!("crossterm error: event stream terminated");
            return true;
        }
        Some(Err(io_err)) => {
            debug!("crossterm error: {:?}", io_err);
            return true;
        }
        Some(Ok(ev)) => {
            let tui_ret = tui.borrow_mut().handle_input_event(ev);
            match tui_ret {
                Abort => {
                    snd_ev.try_send(Event::Abort).unwrap();
                    let _ = snd_abort.try_send(());
                    return true;
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
    false
}

fn handle_editor_out(
    tui: &RefCell<tui::TUI>,
    snd_ev: &mut mpsc::Sender<Event>,
    editor_out: Option<editor::Result>,
) {
    let ret = tui.borrow_mut().handle_editor_out(editor_out);
    if let tui::TUIRet::Lines { lines, from } = ret {
        snd_ev
            .try_send(Event::Lines {
                lines,
                source: from,
            })
            .unwrap();
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

impl UI for TUI {
    delegate!(draw());
    delegate!(new_server_tab(serv_name: &str, alias: Option<String>,));
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

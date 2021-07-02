#![allow(clippy::too_many_arguments)]
#![allow(clippy::cognitive_complexity)]

mod config;
mod editor;
mod exit_dialogue;
mod input_area;
mod key_map;
mod line_split;
mod messaging;
#[doc(hidden)]
pub mod msg_area; // Public to be able to use in an example
mod notifier;
mod tab;
mod termbox;
pub mod test_utils;
#[doc(hidden)]
pub mod trie; // Public for benchmarks
pub mod tui; // Public for benchmarks
mod utils;
mod widget;

#[cfg(test)]
mod tests;

use crate::tui::{CmdResult, TUIRet};
use libtiny_common::{ChanNameRef, Event, MsgSource, MsgTarget, TabStyle};
use term_input::Input;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use time::Tm;
use tokio::select;
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;
use tokio::task::spawn_local;
use tokio_stream::wrappers::{ReceiverStream, SignalStream};
use tokio_stream::{Stream, StreamExt};

#[macro_use]
extern crate log;

#[derive(Clone)]
pub struct TUI {
    inner: Weak<RefCell<tui::TUI>>,
}

impl TUI {
    pub fn run(config_path: PathBuf) -> (TUI, mpsc::Receiver<Event>) {
        let tui = Rc::new(RefCell::new(tui::TUI::new(config_path)));
        let inner = Rc::downgrade(&tui);

        let (snd_ev, rcv_ev) = mpsc::channel(10);

        // For SIGWINCH handler
        let (snd_abort, rcv_abort) = mpsc::channel::<()>(1);

        // Spawn SIGWINCH handler
        spawn_local(sigwinch_handler(inner.clone(), rcv_abort));

        // Spawn input handler task
        let input = Input::new();
        spawn_local(input_handler(input, tui, snd_ev, snd_abort));

        (TUI { inner }, rcv_ev)
    }

    /// Create a test instance that doesn't render to the terminal, just updates the termbox
    /// buffer. Useful for testing. See also [`get_front_buffer`](TUI::get_front_buffer).
    pub fn run_test<S>(width: u16, height: u16, input_stream: S) -> (TUI, mpsc::Receiver<Event>)
    where
        S: Stream<Item = std::io::Result<term_input::Event>> + Unpin + 'static,
    {
        let tui = Rc::new(RefCell::new(tui::TUI::new_test(width, height)));
        let inner = Rc::downgrade(&tui);

        let (snd_ev, rcv_ev) = mpsc::channel(10);

        // We don't need to handle SIGWINCH in testing so the receiver end is not used
        let (snd_abort, _rcv_abort) = mpsc::channel::<()>(1);

        // Spawn input handler task
        spawn_local(input_handler(input_stream, tui, snd_ev, snd_abort));

        (TUI { inner }, rcv_ev)
    }

    /// Get termbox front buffer. Useful for testing rendering.
    pub fn get_front_buffer(&self) -> termbox_simple::CellBuf {
        self.inner.upgrade().unwrap().borrow().get_front_buffer()
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

    let mut stream_fused = SignalStream::new(stream).fuse();
    let mut rcv_abort_fused = ReceiverStream::new(rcv_abort).fuse();

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

async fn input_handler<S>(
    mut input_stream: S,
    tui: Rc<RefCell<tui::TUI>>,
    snd_ev: mpsc::Sender<Event>,
    snd_abort: mpsc::Sender<()>,
) where
    S: Stream<Item = std::io::Result<term_input::Event>> + Unpin,
{
    // See module documentation of `editor` for how editor stuff works
    let mut rcv_editor_ret: Option<editor::ResultReceiver> = None;

    loop {
        if let Some(editor_ret) = rcv_editor_ret.take() {
            // $EDITOR running, don't read stdin, wait for $EDITOR to finish
            match editor_ret.await {
                Err(recv_error) => {
                    debug!("RecvError while waiting editor response: {:?}", recv_error);
                }
                Ok(editor_ret) => {
                    if let Some((lines, from)) = tui.borrow_mut().handle_editor_result(editor_ret) {
                        debug!("editor ret: {:?}", lines);
                        snd_ev
                            .try_send(Event::Lines {
                                lines,
                                source: from,
                            })
                            .unwrap();
                    }
                }
            }

            tui.borrow_mut().activate();
            tui.borrow_mut().draw();
        }

        match input_stream.next().await {
            None => {
                break;
            }
            Some(Err(io_err)) => {
                debug!("term_input error: {:?}", io_err);
                break;
            }
            Some(Ok(ev)) => {
                let tui_ret = tui.borrow_mut().handle_input_event(ev, &mut rcv_editor_ret);
                match tui_ret {
                    TUIRet::Abort => {
                        snd_ev.try_send(Event::Abort).unwrap();
                        let _ = snd_abort.try_send(());
                        return;
                    }
                    TUIRet::KeyHandled | TUIRet::KeyIgnored(_) | TUIRet::EventIgnored(_) => {}
                    TUIRet::Input { msg, from } => {
                        if msg[0] == '/' {
                            // Handle TUI commands, send others to downstream
                            let cmd: String = (&msg[1..]).iter().collect();
                            let result = tui.borrow_mut().try_handle_cmd(&cmd, &from);
                            match result {
                                CmdResult::Ok => {}
                                CmdResult::Continue => {
                                    snd_ev.try_send(Event::Cmd { cmd, source: from }).unwrap()
                                }
                                CmdResult::Quit => {
                                    snd_ev.try_send(Event::Abort).unwrap();
                                    let _ = snd_abort.try_send(());
                                    return;
                                }
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
                }
            }
        }

        tui.borrow_mut().draw();
    }
}

macro_rules! delegate {
    ( $name:ident ( $( $x:ident: $t:ty, )* ) ) => {
        pub fn $name(&self, $($x: $t,)*) {
            if let Some(inner) = self.inner.upgrade() {
                inner.borrow_mut().$name( $( $x, )* );
            }
        }
    }
}

impl TUI {
    delegate!(draw());
    delegate!(new_server_tab(serv_name: &str, alias: Option<String>,));
    delegate!(close_server_tab(serv_name: &str,));
    delegate!(new_chan_tab(serv_name: &str, chan: &ChanNameRef,));
    delegate!(close_chan_tab(serv_name: &str, chan: &ChanNameRef,));
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
        chan_name: &ChanNameRef,
    ));
    delegate!(set_tab_style(style: TabStyle, target: &MsgTarget,));

    pub fn user_tab_exists(&self, serv_name: &str, nick: &str) -> bool {
        match self.inner.upgrade() {
            Some(tui) => tui.borrow().user_tab_exists(serv_name, nick),
            None => false,
        }
    }

    pub fn current_tab(&self) -> Option<MsgSource> {
        self.inner
            .upgrade()
            .map(|tui| tui.borrow().current_tab().clone())
    }
}

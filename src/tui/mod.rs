pub mod exit_dialogue;
pub mod messaging;
pub mod msg_area;
pub mod tabbed;
pub mod termbox;
pub mod text_field;
pub mod widget;

use std::fs;
use std::str;

use config;
use self::tabbed::{Tabbed, TabbedRet, TabStyle, MsgSource};
pub use self::messaging::Timestamp;

use term_input::{Event, Key};
use termbox_simple::{Termbox, OutputMode};

pub struct TUI {
    /// Termbox instance
    termbox  : Termbox,

    /// A tab for every server + channel
    ui       : Tabbed,
}

#[derive(Debug)]
pub enum TUIRet {
    Abort,
    KeyHandled,
    KeyIgnored(Key),
    EventIgnored(Event),

    /// INVARIANT: The vec will have at least one char.
    // Can't make MsgSource a ref because of this weird error:
    // https://users.rust-lang.org/t/borrow-checker-bug/5165
    Input {
        msg  : Vec<char>,
        from : MsgSource,
    },
}

impl TUI {
    pub fn new() -> TUI {
        let mut tui = Termbox::init().unwrap(); // TODO: check errors
        tui.set_output_mode(OutputMode::Output256);
        tui.set_clear_attributes(config::CLEAR.fg, config::CLEAR.bg);

        let _ = fs::create_dir("logs");

        TUI {
            ui: Tabbed::new(tui.width() as i32, tui.height() as i32),
            termbox: tui,
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Tab stuff

impl TUI {
    pub fn count_tabs(&self) -> usize {
        self.ui.count_tabs()
    }

    pub fn new_server_tab(&mut self, serv_name: &str) {
        self.ui.new_server_tab(serv_name);
    }

    pub fn close_server_tab(&mut self, serv_name: &str) {
        self.ui.close_server_tab(serv_name);
    }

    pub fn new_chan_tab(&mut self, serv_name: &str, chan_name: &str) {
        self.ui.new_chan_tab(serv_name, chan_name);
    }

    pub fn close_chan_tab(&mut self, serv_name: &str, chan_name: &str) {
        self.ui.close_chan_tab(serv_name, chan_name);
    }

    pub fn new_user_tab(&mut self, serv_name: &str, nick: &str) {
        self.ui.new_user_tab(serv_name, nick);
    }

    pub fn close_user_tab(&mut self, serv_name: &str, nick: &str) {
        self.ui.close_user_tab(serv_name, nick);
    }

    pub fn set_tab_style(&mut self, style: TabStyle, target: &MsgTarget) {
        self.ui.set_tab_style(style, target);
    }
}

////////////////////////////////////////////////////////////////////////////////
// Event handling

impl TUI {
    pub fn handle_input_event(&mut self, ev : Event) -> TUIRet {
        match ev {
            Event::Resize => {
                // This never happens, probably because the our select() loop,
                // termbox can't really get resize signals.
                self.resize();
                TUIRet::KeyHandled
            },

            Event::Key(key) => {
                match self.ui.keypressed(key) {
                    TabbedRet::KeyHandled => TUIRet::KeyHandled,
                    TabbedRet::KeyIgnored => TUIRet::KeyIgnored(key),
                    TabbedRet::Input { msg, from } => {
                        TUIRet::Input {
                            msg: msg,
                            from: from.clone(),
                        }
                    },
                    TabbedRet::Abort => TUIRet::Abort,
                }
            },

            Event::String(str) => {
                // This happens when keys pressed too fast or a text pasted to the terminal
                if str.len() <= 8 {
                    // Assume fast key press
                    let mut ret = TUIRet::KeyHandled;
                    for ch in str.chars() {
                        ret = self.handle_input_event(Event::Key(Key::Char(ch)));
                    }
                    ret
                } else {
                    // Assume paste
                    TUIRet::EventIgnored(Event::String(str.to_owned()))
                }
            },

            ev => {
                TUIRet::EventIgnored(ev)
            },
        }
    }

    pub fn resize(&mut self) {
        self.termbox.resize();
        self.termbox.clear();
        let w = self.termbox.width();
        let h = self.termbox.height();
        self.ui.resize(w, h);
    }

/*
    /// Loop until something's entered to the user input field. Useful for
    /// waiting for a command when there's no connection yet.
    pub fn idle_loop(&mut self) -> TUIRet {
        loop {
            self.draw();

            match self.keypressed_poll() {
                ret @ TUIRet::Abort => { return ret; },
                ret @ TUIRet::Input { .. } => { return ret; },
                _ => {}
            }
        }
    }
*/

    pub fn draw(&mut self) {
        self.termbox.clear();
        self.ui.draw(&mut self.termbox, 0, 0);
        self.termbox.present();
    }
}

////////////////////////////////////////////////////////////////////////////////
// Showing messages

/// Target of a message coming from an IRC server.
#[derive(Debug)]
pub enum MsgTarget<'a> {
    Server { serv_name: &'a str },
    Chan { serv_name: &'a str, chan_name: &'a str },
    User { serv_name: &'a str, nick: &'a str },

    /// Show the message in all tabs of a server.
    AllServTabs { serv_name: &'a str },

    /// Show the message all server tabs that have the user. (i.e. channels,
    /// privmsg tabs)
    AllUserTabs { serv_name: &'a str, nick: &'a str },

    /// Show the message in all tabs.
    AllTabs,

    /// Show the message in currently active tab.
    CurrentTab,

    MultipleTabs(Vec<MsgTarget<'a>>),
}

impl TUI {
    /// An error message coming from Tiny, probably because of a command error
    /// etc. Those are not timestamped and not logged.
    pub fn add_client_err_msg(&mut self, msg : &str, target : &MsgTarget) {
        self.ui.add_client_err_msg(msg, target);
    }

    /// A message from client, usually just to indidate progress, e.g.
    /// "Connecting...". Not timestamed and not logged.
    pub fn add_client_msg(&mut self, msg : &str, target : &MsgTarget) {
        self.ui.add_client_msg(msg, target);
    }

    /// privmsg is a message coming from a server or client. Shown with sender's
    /// nick/name and receive time and logged.
    pub fn add_privmsg(&mut self, sender: &str, msg: &str, ts: Timestamp, target: &MsgTarget) {
        self.ui.add_privmsg(sender, msg, ts, target);
    }

    /// A message without any explicit sender info. Useful for e.g. in server
    /// and debug log tabs. Timestamped and logged.
    pub fn add_msg(&mut self, msg: &str, ts: Timestamp, target : &MsgTarget) {
        self.ui.add_msg(msg, ts, target);
    }

    /// Error messages related with the protocol - e.g. can't join a channel,
    /// nickname is in use etc. Timestamped and logged.
    pub fn add_err_msg(&mut self, msg: &str, ts: Timestamp, target : &MsgTarget) {
        self.ui.add_err_msg(msg, ts, target);
    }

    pub fn show_topic(&mut self, msg: &str, ts: Timestamp, target: &MsgTarget) {
        self.ui.show_topic(msg, ts, target);
    }

    pub fn add_nick(&mut self, nick: &str, ts: Option<Timestamp>, target: &MsgTarget) {
        self.ui.add_nick(nick, ts, target);
    }

    pub fn remove_nick(&mut self, nick : &str, ts: Option<Timestamp>, target: &MsgTarget) {
        self.ui.remove_nick(nick, ts, target);
    }

    pub fn rename_nick(&mut self, old_nick: &str, new_nick: &str, ts: Timestamp, target: &MsgTarget) {
        self.ui.rename_nick(old_nick, new_nick, ts, target);
    }
}

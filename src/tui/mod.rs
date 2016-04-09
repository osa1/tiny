pub mod messaging;
pub mod msg_area;
pub mod style;
pub mod tabbed;
pub mod text_field;
pub mod widget;

use std::io::Write;
use std::io;
use std::mem;
use std::str;
use std::time::Duration;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

use self::tabbed::{Tabbed, TabbedRet, MsgSource};
use self::widget::{Widget};

pub struct TUI {
    /// Termbox instance
    rustbox : RustBox,

    /// A tab for every server + channel
    ui      : Tabbed,

    /// For debugging only - `write()` method is called with incomplete lines,
    /// we collect those here. Messages are only shown with `flush()`.
    buffer  : Vec<u8>,
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
        let tui = RustBox::init(InitOptions {
            input_mode: InputMode::Esc,
            buffer_stderr: false,
        }).unwrap();

        TUI {
            ui: Tabbed::new(tui.width() as i32, tui.height() as i32),
            rustbox: tui,
            buffer: Vec::with_capacity(100),
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// Tab stuff

impl TUI {
    pub fn new_server_tab(&mut self, serv_name : String) {
        self.ui.new_server_tab(serv_name)
    }

    pub fn new_chan_tab(&mut self, serv_name : String, chan_name : String) {
        self.ui.new_chan_tab(serv_name, chan_name)
    }

    pub fn new_user_tab(&mut self, serv_name : String, nick : String) {
        self.ui.new_user_tab(serv_name, nick)
    }
}

////////////////////////////////////////////////////////////////////////////////
// Event handling

impl TUI {
    /// Should be called when stdin is ready.
    pub fn keypressed_peek(&mut self) -> TUIRet {
        match self.rustbox.peek_event(Duration::new(0, 0), false) {
            Ok(ev) => self.keypressed(ev),

            Err(err) => {
                writeln!(self, "Error during poll_event(): {}", err).unwrap();
                TUIRet::KeyHandled
            }
        }
    }

    /// Blocks until an event is read.
    pub fn keypressed_poll(&mut self) -> TUIRet {
        match self.rustbox.poll_event(false) {
            Ok(ev) => self.keypressed(ev),

            Err(err) => {
                writeln!(self, "Error during poll_event(): {}", err).unwrap();
                TUIRet::KeyHandled
            }
        }
    }

    pub fn keypressed(&mut self, ev : Event) -> TUIRet {
        match ev {
            Event::KeyEvent(Key::Esc) => {
                TUIRet::Abort
            },

            Event::ResizeEvent(width, height) => {
                // This never happens, probably because the our select() loop,
                // termbox can't really get resize signals.
                self.resize(width, height);
                TUIRet::KeyHandled
            },

            Event::KeyEvent(key) => {
                match self.ui.keypressed(key) {
                    TabbedRet::KeyHandled => TUIRet::KeyHandled,
                    TabbedRet::KeyIgnored => TUIRet::KeyIgnored(key),
                    TabbedRet::Input { msg, from } => {
                        TUIRet::Input {
                            msg: msg,
                            from: from.clone(),
                        }
                    }
                }
            },

            ev => {
                TUIRet::EventIgnored(ev)
            },
        }
    }

    pub fn resize(&mut self, width : i32, height : i32) {
        self.ui.resize(width, height);
    }

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

    pub fn draw(&self) {
        self.rustbox.clear();
        self.ui.draw(&self.rustbox, 0, 0);
        self.rustbox.present();
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

    /// Show the message in all tabs.
    AllTabs,

    /// Show the message in currently active tab.
    CurrentTab,

    MultipleTabs(Vec<Box<MsgTarget<'a>>>),
}

impl TUI {
    #[inline]
    pub fn add_msg(&mut self, msg : &str, target : &MsgTarget) {
        self.ui.add_msg(msg, target, style::USER_MSG);
    }

    #[inline]
    pub fn add_err_msg(&mut self, err : &str, target : &MsgTarget) {
        self.ui.add_msg(err, target, style::ERR_MSG);
    }
}

////////////////////////////////////////////////////////////////////////////////

// Write instance is used for debugging - messages show up in a "debug" tab.

impl Write for TUI {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut line_start = 0;

        for (byte_idx, byte) in buf.iter().enumerate() {
            if *byte == b'\n' {
                if self.buffer.len() != 0 {
                    debug_assert!(line_start == 0);
                    self.buffer.extend_from_slice(&buf[ 0 .. byte_idx ]);
                    let mut msg : Vec<u8> = Vec::with_capacity(100);
                    mem::swap(&mut msg, &mut self.buffer);
                    self.add_msg(&unsafe { String::from_utf8_unchecked(msg) },
                                 &MsgTarget::Server { serv_name: "debug" });
                } else {
                    self.add_msg(unsafe { str::from_utf8_unchecked(&buf[ line_start .. byte_idx ]) },
                                 &MsgTarget::Server { serv_name: "debug" });
                }

                line_start = byte_idx + 1;
            }
        }

        self.buffer.extend_from_slice(&buf[ line_start .. ]);
        Result::Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

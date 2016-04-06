pub mod messaging;
pub mod msg_area;
pub mod style;
pub mod tabbed;
pub mod text_field;
pub mod widget;

use std::time::Duration;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

use msg::Pfx;
use self::tabbed::{Tabbed, TabbedRet};
use self::widget::{Widget};

pub struct TUI {
    /// Termbox instance
    rustbox : RustBox,

    /// A tab for every server + channel
    ui      : Tabbed,
}

#[derive(Debug)]
pub enum TUIRet {
    Abort,
    KeyHandled,
    KeyIgnored(Key),
    EventIgnored(Event),

    /// INVARIANT: The vec will have at least one char.
    // Can't make Pfx a ref because of this weird error:
    // https://users.rust-lang.org/t/borrow-checker-bug/5165
    Input {
        serv_name : String,
        pfx       : Option<Pfx>,
        msg       : Vec<char>,
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
        }
    }

    #[inline]
    pub fn new_tab(&mut self, serv_name : String, pfx : Pfx) {
        self.ui.new_tab(serv_name, pfx)
    }

    #[inline]
    pub fn new_server_tab(&mut self, serv_name : String) {
        self.ui.new_server_tab(serv_name)
    }

    /// Should be called when stdin is ready.
    pub fn keypressed(&mut self) -> TUIRet {
        // We should use peek() instead of poll() as we now call this function
        // when a signal occurs. We don't want to wait forever if the signal
        // doesn't handled by termbox and triggered an event.
        match self.rustbox.peek_event(Duration::new(0, 0), false) {
            Ok(Event::KeyEvent(Key::Esc)) => {
                TUIRet::Abort
            },

            Ok(Event::ResizeEvent(width, height)) => {
                // This never happens, probably because the our select() loop,
                // termbox can't really get resize signals.
                self.resize(width, height);
                TUIRet::KeyHandled
            }

            Ok(Event::KeyEvent(key)) => {
                match self.ui.keypressed(key) {
                    TabbedRet::KeyHandled => TUIRet::KeyHandled,
                    TabbedRet::KeyIgnored => TUIRet::KeyIgnored(key),
                    TabbedRet::Input { serv_name, pfx, msg } => {
                        TUIRet::Input {
                            serv_name: serv_name.to_owned(),
                            pfx: pfx.map(|p| p.clone()),
                            msg: msg,
                        }
                    }
                }
            },

            Ok(ev) => {
                TUIRet::EventIgnored(ev)
            },

            Err(_) => {
                // TODO: Log for further investigation
                TUIRet::KeyHandled
            }
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

            match self.keypressed() {
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

    ////////////////////////////////////////////////////////////////////////////

    #[inline]
    pub fn show_msg(&mut self, msg : &str, serv_name : &str, pfx : Option<&Pfx>) {
        self.ui.add_msg(msg, serv_name, pfx, style::USER_MSG);
    }

    #[inline]
    pub fn show_error(&mut self, err : &str, serv_name : &str, pfx : Option<&Pfx>) {
        self.ui.add_msg(err, serv_name, pfx, style::ERR_MSG);
    }

    #[inline]
    pub fn show_error_all_tabs(&mut self, err : &str) {
        self.ui.add_msg_all_tabs(err, style::ERR_MSG);
    }

    #[inline]
    pub fn show_msg_current_tab(&mut self, msg : &str) {
        self.ui.add_msg_current_tab(msg, style::USER_MSG);
    }

    #[inline]
    pub fn show_error_current_tab(&mut self, err : &str) {
        self.ui.add_msg_current_tab(err, style::ERR_MSG);
    }
}

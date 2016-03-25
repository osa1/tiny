pub mod msg_area;
pub mod text_field;

use std::time::Duration;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

use self::msg_area::MsgArea;
use self::text_field::{TextField, TextFieldRet};

pub struct TUI {
    /// Termbox instance
    rustbox : RustBox,

    /// Incoming and sent messages appear
    msg_area : MsgArea,

    /// User input field
    text_field : TextField,
}

#[derive(Debug)]
pub enum TUIRet {
    Abort,
    KeyHandled,
    KeyIgnored(Key),
    EventIgnored(Event),

    /// INVARIANT: The vec will have at least one char.
    SendMsg(Vec<char>),
}

impl TUI {
    pub fn new() -> TUI {
        let tui = RustBox::init(InitOptions {
            input_mode: InputMode::Esc,
            buffer_stderr: false,
        }).unwrap();

        TUI {
            msg_area: MsgArea::new(tui.width() as i32, tui.height() as i32 - 1),
            text_field: TextField::new(tui.width() as i32),

            // need to move this last
            rustbox: tui,
        }
    }

    /// Should be called when stdin is ready.
    pub fn keypressed(&mut self) -> TUIRet {
        match self.rustbox.poll_event(false) {
            Ok(Event::KeyEvent(Key::Esc)) => {
                TUIRet::Abort
            },

            ////////////////////////////////////////////////////////////////////
            // Scrolling related

            Ok(Event::KeyEvent(Key::Ctrl('p'))) => {
                self.msg_area.scroll_up();
                TUIRet::KeyHandled
            },

            Ok(Event::KeyEvent(Key::Ctrl('n'))) => {
                self.msg_area.scroll_down();
                TUIRet::KeyHandled
            },

            Ok(Event::KeyEvent(Key::PageUp)) => {
                self.msg_area.page_up();
                TUIRet::KeyHandled
            },

            Ok(Event::KeyEvent(Key::PageDown)) => {
                self.msg_area.page_down();
                TUIRet::KeyHandled
            },

            ////////////////////////////////////////////////////////////////////

            Ok(Event::KeyEvent(key)) => {
                // TODO: Handle ret
                match self.text_field.keypressed(key) {
                    TextFieldRet::SendMsg(msg) => TUIRet::SendMsg(msg),
                    TextFieldRet::KeyHandled => TUIRet::KeyHandled,
                    TextFieldRet::KeyIgnored => {
                        self.show_server_msg("KEY IGNORED", format!("{:?}", key).as_ref());
                        TUIRet::KeyIgnored(key)
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

    /// Loop until something's entered to the user input field. Useful for
    /// waiting for a command when there's no connection yet.
    pub fn idle_loop(&mut self) -> TUIRet {
        loop {
            self.draw();

            match self.keypressed() {
                ret @ TUIRet::Abort => { return ret; },
                ret @ TUIRet::SendMsg(_) => { return ret; },
                _ => {}
            }
        }
    }

    pub fn draw(&self) {
        self.rustbox.clear();
        self.msg_area.draw(&self.rustbox, 0, 0);
        self.text_field.draw(&self.rustbox, 0, (self.rustbox.height() - 1) as i32);
        self.rustbox.present();
    }

    pub fn show_server_msg(&mut self, ty : &str, msg : &str) {
        self.msg_area.add_server_msg(format!("[{}] {}", ty, msg).as_ref());
    }

    pub fn show_incoming_msg(&mut self, msg : &str) {
        self.msg_area.add_msg_str(msg);
    }

    pub fn show_outgoing_msg(&mut self, msg : &str) {
        self.msg_area.add_msg_str(msg);
    }

    pub fn show_user_error(&mut self, msg : &str) {
        self.msg_area.add_err_msg_str(msg);
    }

    pub fn show_conn_error(&mut self, msg : &str) {
        self.msg_area.add_err_msg_str(msg);
    }
}

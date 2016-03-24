use std::time::Duration;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

use msg_area::MsgArea;
use text_field::{TextField, TextFieldRet};

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
        match self.rustbox.peek_event(Duration::new(0, 0), false) {
            Ok(Event::KeyEvent(Key::Esc)) => {
                TUIRet::Abort
            },
            Ok(Event::KeyEvent(key)) => {
                // TODO: Handle ret
                match self.text_field.keypressed(key) {
                    TextFieldRet::SendMsg(msg) => TUIRet::SendMsg(msg),
                    TextFieldRet::KeyHandled => TUIRet::KeyHandled,
                    TextFieldRet::KeyIgnored => TUIRet::KeyIgnored(key),
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

            match self.rustbox.poll_event(false) {
                Ok(Event::KeyEvent(Key::Esc)) => {
                    return TUIRet::Abort;
                },
                Ok(Event::KeyEvent(key)) => {
                    match self.text_field.keypressed(key) {
                        TextFieldRet::SendMsg(msg) => {
                            return TUIRet::SendMsg(msg);
                        },
                        _ => {}
                    }
                },
                Ok(_) => {},
                Err(_) => {
                    // TODO: Log for further investigation
                }
            }
        }
    }

    pub fn draw(&self) {
        self.rustbox.clear();
        self.msg_area.draw(&self.rustbox, 0, 0);
        self.text_field.draw(&self.rustbox, 0, (self.rustbox.height() - 1) as i32);
        self.rustbox.present();
    }

    pub fn show_user_error(&mut self, msg : &str) {
        self.msg_area.add_err_msg_str(msg);
    }

    pub fn show_conn_error(&mut self, msg : &str) {
        self.msg_area.add_err_msg_str(msg);
    }
}

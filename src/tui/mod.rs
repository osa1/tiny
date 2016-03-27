pub mod messaging;
pub mod msg_area;
pub mod tabbed;
pub mod text_field;
pub mod widget;

use std::cmp::max;
use std::time::Duration;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

use self::messaging::MessagingUI;
use self::msg_area::MsgArea;
use self::text_field::TextField;
use self::widget::{Widget, WidgetRet};

pub struct TUI {
    /// Termbox instance
    rustbox : RustBox,

    /// Incoming and sent messages appear
    msg_ui  : MessagingUI,
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
            msg_ui: MessagingUI::new(tui.width() as i32, tui.height() as i32),
            rustbox: tui,
        }
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
                match self.msg_ui.keypressed(key) {
                    WidgetRet::KeyHandled => TUIRet::KeyHandled,
                    WidgetRet::KeyIgnored => TUIRet::KeyIgnored(key),
                    WidgetRet::Input(v)   => TUIRet::SendMsg(v),
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
        self.msg_ui.resize(width, height);
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
        self.msg_ui.draw(&self.rustbox, 0, 0);
        self.rustbox.present();
    }
}

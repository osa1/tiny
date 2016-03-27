use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

use tui::msg_area::MsgArea;
use tui::text_field::TextField;
use tui::widget::{Widget, WidgetRet};

/// A messaging screen is just a text field to type messages and msg area to
/// show incoming/sent messages.
pub struct MessagingUI {
    /// Incoming and sent messages appear
    msg_area   : MsgArea,

    /// User input field
    text_field : TextField,

    width      : i32,
    height     : i32,
}

impl MessagingUI {
    pub fn new(width : i32, height : i32) -> MessagingUI {
        assert!(height >= 2);
        MessagingUI {
            msg_area: MsgArea::new(width, height - 1),
            text_field: TextField::new(width),
            width: width,
            height: height,
        }
    }

    fn draw_(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        self.msg_area.draw(rustbox, pos_x, pos_y);
        self.text_field.draw(rustbox, pos_x, pos_y + self.height - 1);
    }

    fn keypressed_(&mut self, key : Key) -> WidgetRet {
        match key {
            Key::Ctrl('p') => {
                self.msg_area.scroll_up();
                WidgetRet::KeyHandled
            },

            Key::Ctrl('n') => {
                self.msg_area.scroll_down();
                WidgetRet::KeyHandled
            },

            Key::PageUp => {
                self.msg_area.page_up();
                WidgetRet::KeyHandled
            },

            Key::PageDown => {
                self.msg_area.page_down();
                WidgetRet::KeyHandled
            },

            key => {
                // TODO: Handle ret
                match self.text_field.keypressed(key) {
                    WidgetRet::KeyIgnored => {
                        self.show_server_msg("KEY IGNORED", format!("{:?}", key).as_ref());
                        WidgetRet::KeyIgnored
                    },
                    ret => ret,
                }
            },
        }
    }

    fn resize_(&mut self, width : i32, height : i32) {
        assert!(height >= 2);
        self.width = width;
        self.height = height;
        self.msg_area.resize(width, height - 1);
        self.text_field.resize(width, 1);
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

impl Widget for MessagingUI {
    fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        self.draw_(rustbox, pos_x, pos_y)
    }

    fn keypressed(&mut self, key : Key) -> WidgetRet {
        self.keypressed_(key)
    }

    fn resize(&mut self, width : i32, height : i32) {
        self.resize_(width, height)
    }
}

use rustbox::{RustBox, Key};
use time::Tm;

use tui::msg_area::MsgArea;
use tui::style;
use tui::text_field::TextField;
use tui::widget::{Widget, WidgetRet};

/// A messaging screen is just a text field to type messages and msg area to
/// show incoming/sent messages.
pub struct MessagingUI {
    /// Channel topic, user info etc.
    topic      : Option<String>,

    /// Incoming and sent messages appear
    msg_area   : MsgArea,

    /// User input field
    text_field : TextField,

    width      : i32,
    height     : i32,
}

impl MessagingUI {
    pub fn new(width : i32, height : i32) -> MessagingUI {
        MessagingUI {
            topic: None,
            msg_area: MsgArea::new(width, height - 1),
            text_field: TextField::new(width),
            width: width,
            height: height,
        }
    }

    pub fn set_topic(&mut self, topic : String) {
        self.topic = Some(topic);
        self.msg_area.resize(self.width, self.height - 2);
    }

    fn draw_(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        // TODO: Most channels have long topics that don't fit into single line.
        // if let Some(ref topic) = self.topic {
        //     // rustbox.print(pos_x as usize, pos_y as usize,
        //     //               style::TOPIC.style,
        //     //               style::TOPIC.fg,
        //     //               style::TOPIC.bg,
        //     //               topic);
        //     self.msg_area.draw(rustbox, pos_x, pos_y + 1);
        // } else {
        //     self.msg_area.draw(rustbox, pos_x, pos_y);
        // }

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
                        // self.show_server_msg("KEY IGNORED", format!("{:?}", key).as_ref());
                        WidgetRet::KeyIgnored
                    },
                    ret => ret,
                }
            },
        }
    }

    fn resize_(&mut self, width : i32, height : i32) {
        self.width = width;
        self.height = height;
        self.msg_area.resize(width, height - 1);
        self.text_field.resize(width, 1);
    }
}


////////////////////////////////////////////////////////////////////////////////
// Methods delegeted to the msg_area

impl MessagingUI {
    #[inline]
    pub fn add_client_err_msg(&mut self, msg : &str) {
        self.msg_area.add_text(msg, &style::ERR_MSG_SS);
        self.msg_area.flush_line();
    }

    #[inline]
    pub fn add_client_msg(&mut self, msg : &str) {
        self.msg_area.add_text(msg, &style::USER_MSG_SS);
        self.msg_area.flush_line();
    }

    #[inline]
    pub fn add_privmsg(&mut self, sender : &str, msg : &str, tm : &Tm) {
        self.msg_area.add_text(
            &format!("[{}] <{}> {}", tm.strftime("%H:%M").unwrap(), sender, msg),
            &style::USER_MSG_SS);
        self.msg_area.flush_line();
    }

    #[inline]
    pub fn add_msg(&mut self, msg : &str, tm : &Tm) {
        self.msg_area.add_text(
            &format!("[{}] {}", tm.strftime("%H:%M").unwrap(), msg),
            &style::USER_MSG_SS);
        self.msg_area.flush_line();
    }

    #[inline]
    pub fn add_err_msg(&mut self, msg : &str, tm : &Tm) {
        self.msg_area.add_text(
            &format!("[{}] ", tm.strftime("%H:%M").unwrap()),
            &style::USER_MSG_SS);
        self.msg_area.add_text(msg, &style::ERR_MSG_SS);
        self.msg_area.flush_line();
    }
}

////////////////////////////////////////////////////////////////////////////////

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

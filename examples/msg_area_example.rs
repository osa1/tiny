extern crate tiny;
extern crate rustbox;

use std::borrow::Borrow;

use tiny::msg_area::MsgArea;
use tiny::text_field::{TextField, TextFieldRet};

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

fn loop_() -> Option<String> {
    let rustbox = RustBox::init(InitOptions {
        input_mode: InputMode::Esc,
        buffer_stderr: false,
    }).unwrap();

    let mut text_field = TextField::new(20);
    let mut msg_area  = MsgArea::new(20, (rustbox.height() - 1) as i32);

    loop {
        rustbox.clear();
        msg_area.draw(&rustbox, 0, 0);
        text_field.draw(&rustbox, 0, (rustbox.height() - 1) as i32);
        rustbox.present();

        match rustbox.poll_event(false) {
            Err(err) => return Some(format!("{:?}", err)),
            Ok(Event::KeyEvent(Key::Esc)) => return None,
            Ok(Event::KeyEvent(key)) => {
                match text_field.keypressed(key) {
                    TextFieldRet::SendMsg(msg) => {
                        msg_area.add_msg(&msg);
                    },
                    _ => {}
                }
            },
            Ok(_) => {},
        }
    }
}

fn main() {
    loop_().map(|err| println!("{}", err));
}

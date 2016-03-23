extern crate tiny;
extern crate rustbox;

use tiny::text_field::TextField;

use rustbox::{RustBox, InitOptions, InputMode, Event, Key};

fn loop_() -> Option<String> {
    let rustbox = RustBox::init(InitOptions {
        input_mode: InputMode::Esc,
        buffer_stderr: false,
    }).unwrap();

    let mut text_field = TextField::new(20);

    loop {
        rustbox.clear();
        text_field.draw(&rustbox, 0, 0);
        rustbox.present();

        match rustbox.poll_event(false) {
            Err(err) => return Some(format!("{:?}", err)),
            Ok(Event::KeyEvent(Key::Esc)) => return None,
            Ok(Event::KeyEvent(key)) => {
                text_field.keypressed(key);
            },
            Ok(_) => {},
        }
    }
}

fn main() {
    loop_().map(|err| println!("{}", err));
}

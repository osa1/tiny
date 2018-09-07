use termbox_simple::{Termbox};
use config::Colors;
use notifier::Notifier;

pub struct Header {
    width: i32,
}

impl Header {
    pub fn new(width: i32) -> Header {
        Header {
            width,
        }
    }
    pub fn draw(&self, tb: &mut Termbox, colors: &Colors, visible_name: &str,  notifier: &Notifier, ignore_mode: bool){
        let mut notify_state = "Off";
        match notifier {
            Notifier::Mentions => {
                notify_state = "Mentions"
            },
            Notifier::Messages => {
                notify_state = "Messages"
            },
            _ => {}
        };
        let mut ignore_state = "Off";
        if !ignore_mode {
            ignore_state = "On"
        }

        let left_pane = format!(" {} ", visible_name);
        let right_pane = format!(" Notify: {} | Ignore: {} ", notify_state, ignore_state );
        let spacing_length = self.width - (right_pane.chars().count() as i32) - (left_pane.chars().count() as i32);

        ::tui::termbox::print_chars(tb, 0, 0, colors.header_left, left_pane.chars());
        ::tui::termbox::print_chars(tb, left_pane.chars().count() as i32, 0, colors.header_normal," ".repeat(spacing_length as usize).chars());
        ::tui::termbox::print_chars(tb, spacing_length + left_pane.chars().count() as i32 , 0, colors.header_right, right_pane.chars());
    }
}

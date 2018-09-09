use termbox_simple::{Termbox};
use config::Colors;
use notifier::Notifier;

pub struct StatusLine {
    width: i32,
}

impl StatusLine {
    pub fn new(width: i32) -> StatusLine {
        StatusLine {
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

        ::tui::termbox::print_chars(tb, 0, 0, colors.statusline_left, left_pane.chars());
        ::tui::termbox::print_chars(tb, left_pane.chars().count() as i32, 0, colors.statusline_normal," ".repeat(spacing_length as usize).chars());
        ::tui::termbox::print_chars(tb, spacing_length + left_pane.chars().count() as i32 , 0, colors.statusline_right, right_pane.chars());
    }
}

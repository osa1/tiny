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

    pub fn resize(&mut self, width: i32) {
        self.width = width;
    }

    pub fn draw(&self, tb: &mut Termbox, colors: &Colors, show_statusline: &bool, visible_name: &str, notifier: &Notifier, ignore_mode: bool){
        if show_statusline.to_owned() {
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

            let left_pane: String = format!(" {} ", visible_name).chars().take(10).collect();
            let right_pane = format!(" Notify: {} | Ignore: {} ", notify_state, ignore_state );
            let spacing_length = self.width - (right_pane.chars().count() as i32) - (left_pane.chars().count() as i32);

            let left_pane_length = left_pane.chars().count() as i32;
            let right_pane_length = right_pane.chars().count() as i32;


            if self.width > left_pane_length + right_pane_length + 3  {
                ::tui::termbox::print_chars(tb, 0, 0, colors.statusline_normal," ".repeat(self.width as usize).chars());
                ::tui::termbox::print_chars(tb, 0, 0, colors.statusline_left, left_pane.chars());
            } else if self.width > 13 {
                ::tui::termbox::print_chars(tb, 0, 0, colors.statusline_normal," ".repeat(self.width as usize).chars());
                ::tui::termbox::print_chars(tb, 0, 1, colors.statusline_normal," ".repeat(self.width as usize).chars());
                ::tui::termbox::print_chars(tb, (self.width  - left_pane_length)/2, 0, colors.statusline_left, left_pane.chars());
            }

            if self.width > left_pane_length + right_pane_length + 3 {
                ::tui::termbox::print_chars(tb, spacing_length + left_pane.chars().count() as i32 , 0, colors.statusline_right, right_pane.chars());
            } else if self.width > 33 {
                ::tui::termbox::print_chars(tb, (self.width - right_pane_length)/2, 1, colors.statusline_right, right_pane.chars());
            } else if self.width > 20 {
                let statusline_mini = format!("N:{} | I:{}", notify_state, ignore_state);
                ::tui::termbox::print_chars(tb, (self.width - statusline_mini.chars().count() as i32)/2, 1, colors.statusline_right, statusline_mini.chars());
            } else if self.width > 13 {
                let notify_state_mini: String = notify_state.chars().take(3).collect();
                let statusline_mini = format!("N:{} | I:{}", notify_state_mini, ignore_state);
                ::tui::termbox::print_chars(tb, (self.width - statusline_mini.chars().count() as i32)/2, 1, colors.statusline_right, statusline_mini.chars());
            }
        }
    }
}

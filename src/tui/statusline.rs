use config::Colors;
use notifier::Notifier;
use termbox_simple::Termbox;

pub fn statusline_visible(width: i32, height: i32) -> bool {
    width >= 13 // min necessary
        && height >= 6 // arbitrary
}

pub fn draw_statusline(
    tb: &mut Termbox,
    width: i32,
    colors: &Colors,
    visible_name: &str,
    notifier: &Notifier,
    ignore_mode: bool,
) {
    let notify_state = match notifier {
        Notifier::Off => "Off",
        Notifier::Mentions => "Mentions",
        Notifier::Messages => "Messages",
    };
    let ignore_state = if ignore_mode { "On" } else { "Off" };

    let left_pane: String = format!(" {} ", visible_name);

    if width >= left_pane.chars().count() as i32 + 15 {
        ::tui::termbox::print_chars(
            tb,
            0,
            0,
            colors.statusline_normal,
            " ".repeat(width as usize).chars(),
        );
        ::tui::termbox::print_chars(tb, 0, 0, colors.statusline_left, left_pane.chars());
        if width >= left_pane.chars().count() as i32 + 35 {
            let right_pane = format!(" Notify: {} | Ignore: {} ", notify_state, ignore_state);
            let spacing_length =
                width - (right_pane.chars().count() as i32) - (left_pane.chars().count() as i32);
            ::tui::termbox::print_chars(
                tb,
                spacing_length + left_pane.chars().count() as i32,
                0,
                colors.statusline_right,
                right_pane.chars(),
            );
        } else {
            let notify_state_mini: String = notify_state.chars().take(3).collect();
            let right_pane = format!("N:{} | I:{}", notify_state_mini, ignore_state);
            let spacing_length =
                width - (right_pane.chars().count() as i32) - (left_pane.chars().count() as i32);
            ::tui::termbox::print_chars(
                tb,
                spacing_length + left_pane.chars().count() as i32,
                0,
                colors.statusline_right,
                right_pane.chars(),
            );
        }
    } else if width > 15 {
        ::tui::termbox::print_chars(
            tb,
            0,
            0,
            colors.statusline_normal,
            " ".repeat(width as usize).chars(),
        );
        let notify_state_mini: String = notify_state.chars().take(3).collect();
        let right_pane = format!("N:{} | I:{}", notify_state_mini, ignore_state);
        let spacing_length =
            width - (right_pane.chars().count() as i32) - (left_pane.chars().count() as i32);
        ::tui::termbox::print_chars(
            tb,
            spacing_length + left_pane.chars().count() as i32,
            0,
            colors.statusline_right,
            right_pane.chars(),
        );
    }
}

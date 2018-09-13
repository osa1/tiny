use config::Colors;
use notifier::Notifier;
use termbox_simple::Termbox;

// TODO: Always draw one line

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

    let left_pane = format!(" {} ", visible_name)
        .chars()
        .take(10)
        .collect::<String>();
    let right_pane = format!(" Notify: {} | Ignore: {} ", notify_state, ignore_state);

    let spacing_length =
        width - (right_pane.chars().count() as i32) - (left_pane.chars().count() as i32);

    let left_pane_length = left_pane.chars().count() as i32;
    let right_pane_length = right_pane.chars().count() as i32;

    if width > left_pane_length + right_pane_length + 3 {
        ::tui::termbox::print_chars(
            tb,
            0,
            0,
            colors.statusline_normal,
            " ".repeat(width as usize).chars(),
        );
        ::tui::termbox::print_chars(tb, 0, 0, colors.statusline_left, left_pane.chars());
    } else if width > 13 {
        ::tui::termbox::print_chars(
            tb,
            0,
            0,
            colors.statusline_normal,
            " ".repeat(width as usize).chars(),
        );
        ::tui::termbox::print_chars(
            tb,
            0,
            1,
            colors.statusline_normal,
            " ".repeat(width as usize).chars(),
        );
        ::tui::termbox::print_chars(
            tb,
            (width - left_pane_length) / 2,
            0,
            colors.statusline_left,
            left_pane.chars(),
        );
    }

    if width > left_pane_length + right_pane_length + 3 {
        ::tui::termbox::print_chars(
            tb,
            spacing_length + left_pane.chars().count() as i32,
            0,
            colors.statusline_right,
            right_pane.chars(),
        );
    } else if width > 33 {
        ::tui::termbox::print_chars(
            tb,
            (width - right_pane_length) / 2,
            1,
            colors.statusline_right,
            right_pane.chars(),
        );
    } else if width > 20 {
        let statusline_mini = format!("N:{} | I:{}", notify_state, ignore_state);
        ::tui::termbox::print_chars(
            tb,
            (width - statusline_mini.chars().count() as i32) / 2,
            1,
            colors.statusline_right,
            statusline_mini.chars(),
        );
    } else if width > 13 {
        let notify_state_mini: String = notify_state.chars().take(3).collect();
        let statusline_mini = format!("N:{} | I:{}", notify_state_mini, ignore_state);
        ::tui::termbox::print_chars(
            tb,
            (width - statusline_mini.chars().count() as i32) / 2,
            1,
            colors.statusline_right,
            statusline_mini.chars(),
        );
    }
}

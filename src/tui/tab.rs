use termbox_simple::{Termbox, TB_UNDERLINE};

use config::Colors;
use config::Style;
use notifier::Notifier;
use tui::messaging::MessagingUI;
use tui::MsgSource;
use tui::statusline::StatusLine;

pub struct Tab {
    pub widget: MessagingUI,
    pub statusline: StatusLine,
    pub src: MsgSource,
    pub style: TabStyle,
    /// Alt-character to use to switch to this tab.
    pub switch: Option<char>,
    pub notifier: Notifier,
}

// NOTE: Keep the variants sorted in increasing significance, to avoid updating
// style with higher significance for a less significant style (e.g. updating
// from `Highlight` to `NewMsg` in `set_tab_style`).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TabStyle {
    Normal,
    NewMsg,
    Highlight,
}

impl TabStyle {
    pub fn get_style(self, colors: &Colors) -> Style {
        match self {
            TabStyle::Normal => colors.tab_normal,
            TabStyle::NewMsg => colors.tab_new_msg,
            TabStyle::Highlight => colors.tab_highlight,
        }
    }
}

impl Tab {
    pub fn visible_name(&self) -> &str {
        self.src.visible_name()
    }

    pub fn set_style(&mut self, style: TabStyle) {
        self.style = style;
    }

    pub fn update_source<F>(&mut self, f: &F)
    where
        F: Fn(&mut MsgSource),
    {
        f(&mut self.src)
    }

    pub fn width(&self) -> i32 {
        // TODO: assuming ASCII string here. We should probably switch to a AsciiStr type.
        self.visible_name().len() as i32
    }

    pub fn draw(
        &self,
        tb: &mut Termbox,
        colors: &Colors,
        mut pos_x: i32,
        pos_y: i32,
        active: bool,
    ) {
        let style: Style = if active {
            colors.tab_active
        } else {
            self.style.get_style(colors)
        };

        let mut switch_drawn = false;
        for ch in self.visible_name().chars() {
            if Some(ch) == self.switch && !switch_drawn {
                tb.change_cell(pos_x, pos_y, ch, style.fg | TB_UNDERLINE, style.bg);
                switch_drawn = true;
            } else {
                tb.change_cell(pos_x, pos_y, ch, style.fg, style.bg);
            }
            pos_x += 1;
        }
    }
}

pub use libtiny_ui::TabStyle;
use termbox_simple::{Termbox, TB_UNDERLINE};

use crate::{
    config::{Colors, Style},
    messaging::MessagingUI,
    notifier::Notifier,
    MsgSource,
};

pub(crate) struct Tab {
    pub(crate) widget: MessagingUI,
    pub(crate) src: MsgSource,
    pub(crate) style: TabStyle,
    /// Alt-character to use to switch to this tab.
    pub(crate) switch: Option<char>,
    pub(crate) notifier: Notifier,
}

fn tab_style(style: TabStyle, colors: &Colors) -> Style {
    match style {
        TabStyle::Normal => colors.tab_normal,
        TabStyle::NewMsg => colors.tab_new_msg,
        TabStyle::Highlight => colors.tab_highlight,
    }
}

impl Tab {
    pub(crate) fn visible_name(&self) -> &str {
        self.src.visible_name()
    }

    pub(crate) fn set_style(&mut self, style: TabStyle) {
        self.style = style;
    }

    pub(crate) fn update_source<F>(&mut self, f: &F)
    where
        F: Fn(&mut MsgSource),
    {
        f(&mut self.src)
    }

    pub(crate) fn width(&self) -> i32 {
        // TODO: assuming ASCII string here. We should probably switch to a AsciiStr type.
        self.visible_name().len() as i32
    }

    pub(crate) fn draw(
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
            tab_style(self.style, colors)
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

use libtiny_common::{MsgSource, TabStyle};
use termbox_simple::{TB_UNDERLINE, Termbox};

use crate::termbox;
use crate::{
    config::{Colors, Style},
    messaging::MessagingUI,
};

pub(crate) struct Tab {
    pub(crate) visible_name: String,
    pub(crate) widget: MessagingUI,
    pub(crate) src: MsgSource,
    pub(crate) style: TabStyle,
    /// Alt-character to use to switch to this tab.
    pub(crate) switch: Option<char>,
}

fn tab_style(style: TabStyle, colors: &Colors) -> Style {
    match style {
        TabStyle::Normal => colors.tab_normal,
        TabStyle::JoinOrPart => colors.tab_joinpart,
        TabStyle::NewMsg => colors.tab_new_msg,
        TabStyle::Highlight => colors.tab_highlight,
    }
}

impl Tab {
    pub(crate) fn visible_name(&self) -> &str {
        &self.visible_name
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
        termbox::text_width(self.visible_name()) as i32
    }

    pub(crate) fn draw(
        &self,
        tb: &mut Termbox,
        colors: &Colors,
        pos_x: i32,
        pos_y: i32,
        active: bool,
    ) {
        let style: Style = if active {
            colors.tab_active
        } else {
            tab_style(self.style, colors)
        };

        let mut switch_drawn = false;
        let chars = self.visible_name().chars().map(|ch| {
            let char_style = if Some(ch) == self.switch && !switch_drawn {
                switch_drawn = true;
                Style {
                    fg: style.fg | TB_UNDERLINE,
                    bg: style.bg,
                }
            } else {
                style
            };
            (ch, char_style)
        });
        termbox::print_styled_chars(tb, pos_x, pos_y, chars);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::msg_area::Layout;

    #[test]
    fn tab_draws_column_width_and_underlines_switch() {
        let tab = Tab {
            visible_name: "\u{3099}日A".into(),
            widget: MessagingUI::new(10, 3, 10, Layout::Compact),
            src: MsgSource::Serv {
                serv: "test".into(),
            },
            style: TabStyle::Normal,
            switch: Some('日'),
        };
        let colors = Colors::default();
        let mut tb = Termbox::init_test(4, 1);
        assert_eq!(tab.width(), 4);
        tab.draw(&mut tb, &colors, 0, 0, false);
        tb.present();
        let cells = tb.get_front_buffer().cells;
        assert_eq!((cells[0].ch, cells[1].ch, cells[3].ch), ('◌', '日', 'A'));
        assert_eq!(cells[1].fg, colors.tab_normal.fg | TB_UNDERLINE);
        assert_eq!(cells[3].fg, colors.tab_normal.fg);
        assert!(
            String::from_utf8(tb.take_test_output())
                .unwrap()
                .contains("◌\u{3099}")
        );

        let mut narrow = Termbox::init_test(2, 1);
        tab.draw(&mut narrow, &colors, 0, 0, true);
        narrow.present();
        let cells = narrow.get_front_buffer().cells;
        assert_eq!(cells[1].ch, '�');
        assert_eq!(cells[1].fg, colors.tab_active.fg | TB_UNDERLINE);
    }
}

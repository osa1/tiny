use termbox_sys;

#[derive(Debug)]
pub struct Style {
    /// Termbox fg.
    pub fg  : u16,

    /// Termbox bg.
    pub bg  : u16,
}

pub static USER_MSG : Style =
    Style {
        fg: 15, // duh, 15 is "whiter" than TB_WHITE. Terminals render first 8
                // colors (TB_ prefixed ones) differently depending on the color
                // scheme.
        bg: termbox_sys::TB_DEFAULT,
    };

pub static SERVER_MSG : Style =
    Style {
        fg: termbox_sys::TB_BLUE | termbox_sys::TB_BOLD,
        bg: termbox_sys::TB_DEFAULT,
    };

pub static ERR_MSG : Style =
    Style {
        fg: termbox_sys::TB_WHITE | termbox_sys::TB_BOLD,
        bg: termbox_sys::TB_RED,
    };

pub static TOPIC : Style =
    Style {
        fg: termbox_sys::TB_BLACK,
        bg: termbox_sys::TB_GREEN,
    };

pub static CLEAR : Style =
    Style {
        fg: termbox_sys::TB_DEFAULT,
        bg: termbox_sys::TB_DEFAULT,
    };

pub static CURSOR : Style =
    Style {
        fg: termbox_sys::TB_BLACK,
        bg: 39,
    };

pub static TAB_ACTIVE : Style =
    Style {
        fg: 0,
        bg: 49,
    };

pub static TAB_PASSIVE : Style =
    Style {
        fg: 0,
        bg: 7,
    };

pub static JOIN : Style =
    Style {
        fg: 64,
        bg: termbox_sys::TB_DEFAULT,
    };

pub static LEAVE : Style =
    Style {
        fg: 160,
        bg: termbox_sys::TB_DEFAULT,
    };

pub static YELLOW : Style =
    Style {
        fg: 0,
        bg: 11,
    };

pub static GRAY : Style =
    Style {
        fg: 242,
        bg: termbox_sys::TB_DEFAULT,
    };

pub static HIGHLIGHT : Style =
    Style {
        fg: 220,
        bg: termbox_sys::TB_DEFAULT,
    };

pub const TERMBOX_COLOR_PREFIX : char = '\x00';
pub const COLOR_RESET_PREFIX   : char = '\x01';
pub const IRC_COLOR_PREFIX     : char = '\x03';

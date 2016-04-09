use rustbox;
use rustbox::Color;

#[derive(Debug, Clone, Copy)]
pub struct Style {
    pub style : rustbox::Style,
    pub fg    : rustbox::Color,
    pub bg    : rustbox::Color,
}

pub static USER_MSG : Style = Style {
    style: rustbox::RB_NORMAL,
    fg: Color::White,
    bg: Color::Default,
};

pub static SERVER_MSG : Style = Style {
    style: rustbox::RB_BOLD,
    fg: Color::Blue,
    bg: Color::Default,
};

pub static ERR_MSG : Style = Style {
    style: rustbox::RB_BOLD,
    fg: Color::White,
    bg: Color::Red,
};

pub static TOPIC : Style = Style {
    style: rustbox::RB_NORMAL,
    fg: Color::Black,
    bg: Color::Green,
};

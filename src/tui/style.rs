use termbox_sys;
use rustbox;
use rustbox::Color;

#[derive(Debug)]
pub struct Style {
    pub fg : u16,
    pub bg : u16,
}

pub static STYLES : [Style; 5] =
    [ // USER_MSG
      Style {
        fg: termbox_sys::TB_WHITE,
        bg: termbox_sys::TB_DEFAULT,
      },

      // SERVER_MSG
      Style {
        fg: termbox_sys::TB_BLUE | termbox_sys::TB_BOLD,
        bg: termbox_sys::TB_DEFAULT,
      },

      // ERR_MSG
      Style {
          fg: termbox_sys::TB_WHITE | termbox_sys::TB_BOLD,
          bg: termbox_sys::TB_RED,
      },

      // TOPIC
      Style {
          fg: termbox_sys::TB_BLACK,
          bg: termbox_sys::TB_GREEN,
      },

      // CLEAR
      Style {
          fg: termbox_sys::TB_BLACK,
          bg: termbox_sys::TB_WHITE,
      }
    ];

pub const NUM_STYLES : usize = 5;

pub type StyleRef = u8;

pub const USER_MSG   : StyleRef = 0;
pub const SERVER_MSG : StyleRef = 1;
pub const ERR_MSG    : StyleRef = 2;
pub const TOPIC      : StyleRef = 3;
pub const CLEAR      : StyleRef = 4;

pub fn get_style(sref : StyleRef) -> &'static Style {
    unsafe { STYLES.get_unchecked(sref as usize) }
}

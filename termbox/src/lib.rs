pub const TB_DEFAULT: u16 = 0x00;
pub const TB_BLACK: u16 = 0x01;
pub const TB_RED: u16 = 0x02;
pub const TB_GREEN: u16 = 0x03;
pub const TB_YELLOW: u16 = 0x04;
pub const TB_BLUE: u16 = 0x05;
pub const TB_MAGENTA: u16 = 0x06;
pub const TB_CYAN: u16 = 0x07;
pub const TB_WHITE: u16 = 0x08;

pub const TB_BOLD: u16 = 0x0100;
pub const TB_UNDERLINE: u16 = 0x0200;
pub const TB_REVERSE: u16 = 0x0400;

#[repr(C)]
pub struct Cell {
    pub ch: u32,
    pub fg: u16,
    pub bg: u16,
}

const TB_EUNSUPPORTED_TERMINAL: libc::c_int = -1;
const TB_EFAILED_TO_OPEN_TTY: libc::c_int = -2;

const TB_HIDE_CURSOR: libc::c_int = -1;

const TB_OUTPUT_CURRENT: libc::c_int = 0;
const TB_OUTPUT_NORMAL: libc::c_int = 1;
// These are not used, we just std::mem::transmute the value if it's in range
// const TB_OUTPUT_256       : libc::c_int = 2;
// const TB_OUTPUT_216       : libc::c_int = 3;
const TB_OUTPUT_GRAYSCALE: libc::c_int = 4;

extern "C" {
    pub fn tb_init() -> libc::c_int;
    pub fn tb_resize();
    pub fn tb_shutdown();
    pub fn tb_width() -> libc::c_int;
    pub fn tb_height() -> libc::c_int;
    pub fn tb_clear() -> libc::c_int;
    pub fn tb_set_clear_attributes(fg: u16, bg: u16);
    pub fn tb_present();
    pub fn tb_set_cursor(cx: libc::c_int, cy: libc::c_int);
    pub fn tb_change_cell(x: libc::c_int, y: libc::c_int, ch: u32, cw: u8, fg: u16, bg: u16);
    pub fn tb_select_output_mode(mode: libc::c_int) -> libc::c_int;
}

pub struct Termbox {}

#[derive(Debug)]
pub enum InitError {
    UnsupportedTerminal,
    FailedToOpenTty,
}

#[repr(C)]
pub enum OutputMode {
    OutputNormal = 1,
    Output256,
    Output216,
    OutputGrayscale,
}

impl Termbox {
    pub fn init() -> Result<Termbox, InitError> {
        let ret = unsafe { tb_init() };
        if ret == TB_EUNSUPPORTED_TERMINAL {
            Err(InitError::UnsupportedTerminal)
        } else if ret == TB_EFAILED_TO_OPEN_TTY {
            Err(InitError::FailedToOpenTty)
        } else {
            Ok(Termbox {})
        }
    }

    pub fn resize(&mut self) {
        unsafe {
            tb_resize();
        }
    }

    pub fn width(&self) -> i32 {
        unsafe { tb_width() as i32 }
    }

    pub fn height(&self) -> i32 {
        unsafe { tb_height() as i32 }
    }

    pub fn clear(&mut self) {
        unsafe {
            tb_clear();
        }
    }

    pub fn set_clear_attributes(&mut self, fg: u16, bg: u16) {
        unsafe { tb_set_clear_attributes(fg, bg) }
    }

    pub fn present(&mut self) {
        unsafe { tb_present() }
    }

    pub fn hide_cursor(&mut self) {
        unsafe {
            tb_set_cursor(TB_HIDE_CURSOR, TB_HIDE_CURSOR);
        }
    }

    pub fn set_cursor(&mut self, cx: i32, cy: i32) {
        unsafe { tb_set_cursor(cx as libc::c_int, cy as libc::c_int) }
    }

    pub fn change_cell(&mut self, x: i32, y: i32, ch: char, fg: u16, bg: u16) {
        let cw = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) as u8;
        unsafe { tb_change_cell(x as libc::c_int, y as libc::c_int, char_to_utf8(ch), cw, fg, bg) }
    }

    pub fn get_output_mode(&self) -> OutputMode {
        let ret = unsafe { tb_select_output_mode(TB_OUTPUT_CURRENT) };
        if ret >= TB_OUTPUT_NORMAL && ret <= TB_OUTPUT_GRAYSCALE {
            unsafe { std::mem::transmute(ret) }
        } else {
            panic!("get_output_mode(): Invalid output mode: {}", ret)
        }
    }

    pub fn set_output_mode(&mut self, mode: OutputMode) {
        unsafe {
            tb_select_output_mode(std::mem::transmute(mode));
        }
    }
}

impl Drop for Termbox {
    fn drop(&mut self) {
        unsafe {
            tb_shutdown();
        }
    }
}

// https://github.com/rust-lang/rust/blob/03bed655142dd5e42ba4539de53b3663d8a123e0/src/libcore/char.rs#L424

const TAG_CONT: u8 = 0b1000_0000;
const TAG_TWO_B: u8 = 0b1100_0000;
const TAG_THREE_B: u8 = 0b1110_0000;
const TAG_FOUR_B: u8 = 0b1111_0000;
const MAX_ONE_B: u32 = 0x80;
const MAX_TWO_B: u32 = 0x800;
const MAX_THREE_B: u32 = 0x10000;

fn char_to_utf8(c: char) -> u32 {
    let code = c as u32;
    if code < MAX_ONE_B {
        code as u32
    } else if code < MAX_TWO_B {
        ((u32::from((code >> 6 & 0x1F) as u8 | TAG_TWO_B)) << 8)
            + u32::from((code & 0x3F) as u8 | TAG_CONT)
    } else if code < MAX_THREE_B {
        (u32::from((code >> 12 & 0x0F) as u8 | TAG_THREE_B) << 16)
            + (u32::from((code >> 6 & 0x3F) as u8 | TAG_CONT) << 8)
            + (u32::from((code & 0x3F) as u8 | TAG_CONT))
    } else {
        ((u32::from((code >> 18 & 0x07) as u8 | TAG_FOUR_B)) << 24)
            + ((u32::from((code >> 12 & 0x3F) as u8 | TAG_CONT)) << 16)
            + ((u32::from((code >> 6 & 0x3F) as u8 | TAG_CONT)) << 8)
            + (u32::from((code & 0x3F) as u8 | TAG_CONT))
    }
}

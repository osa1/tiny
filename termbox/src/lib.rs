extern crate libc;

pub const TB_DEFAULT   : u16 = 0x00;
pub const TB_BLACK     : u16 = 0x01;
pub const TB_RED       : u16 = 0x02;
pub const TB_GREEN     : u16 = 0x03;
pub const TB_YELLOW    : u16 = 0x04;
pub const TB_BLUE      : u16 = 0x05;
pub const TB_MAGENTA   : u16 = 0x06;
pub const TB_CYAN      : u16 = 0x07;
pub const TB_WHITE     : u16 = 0x08;

pub const TB_BOLD      : u16 = 0x0100;
pub const TB_UNDERLINE : u16 = 0x0200;
pub const TB_REVERSE   : u16 = 0x0400;

#[repr(C)]
pub struct Cell {
    pub ch: u32,
    pub fg: u16,
    pub bg: u16,
}

const TB_EUNSUPPORTED_TERMINAL : libc::c_int = -1;
const TB_EFAILED_TO_OPEN_TTY   : libc::c_int = -2;

const TB_HIDE_CURSOR      : libc::c_int = -1;

const TB_OUTPUT_CURRENT   : libc::c_int = 0;
const TB_OUTPUT_NORMAL    : libc::c_int = 1;
// These are not used, we just std::mem::transmute the value if it's in range
// const TB_OUTPUT_256       : libc::c_int = 2;
// const TB_OUTPUT_216       : libc::c_int = 3;
const TB_OUTPUT_GRAYSCALE : libc::c_int = 4;

extern {
    fn tb_init() -> libc::c_int;
    fn tb_resize();
    fn tb_shutdown();
    fn tb_width() -> libc::c_int;
    fn tb_height() -> libc::c_int;
    fn tb_clear() -> libc::c_int;
    fn tb_set_clear_attributes(fg: u16, bg: u16);
    fn tb_present();
    fn tb_set_cursor(cx: libc::c_int, cy: libc::c_int);
    fn tb_put_cell(x: libc::c_int, y: libc::c_int, cell: Cell);
    fn tb_change_cell(x: libc::c_int, y: libc::c_int, ch: u32, fg: u16, bg: u16);
    // fn tb_cell_buffer() -> *mut tb_cell;
    fn tb_select_output_mode(mode: libc::c_int) -> libc::c_int;
}

pub struct Termbox {}

#[derive(Debug)]
pub enum InitError { UnsupportedTerminal, FailedToOpenTty }

#[repr(C)]
pub enum OutputMode {
    OutputNormal = 1, Output256, Output216, OutputGrayscale
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
        unsafe { tb_resize(); }
    }

    pub fn width(&self) -> i32 {
        unsafe { tb_width() as i32 }
    }

    pub fn height(&self) -> i32 {
        unsafe { tb_height() as i32 }
    }

    pub fn clear(&mut self) {
        unsafe { tb_clear(); }
    }

    pub fn set_clear_attributes(&mut self, fg: u16, bg: u16) {
        unsafe { tb_set_clear_attributes(fg, bg) }
    }

    pub fn present(&mut self) {
        unsafe { tb_present() }
    }

    pub fn hide_cursor(&mut self) {
        unsafe { tb_set_cursor(TB_HIDE_CURSOR, TB_HIDE_CURSOR); }
    }

    pub fn set_cursor(&mut self, cx: i32, cy: i32) {
        unsafe { tb_set_cursor(cx as libc::c_int, cy as libc::c_int) }
    }

    pub fn put_cell(&mut self, x: i32, y: i32, cell: Cell) {
        unsafe { tb_put_cell(x as libc::c_int, y as libc::c_int, cell) }
    }

    pub fn change_cell(&mut self, x: i32, y: i32, ch: char, fg: u16, bg: u16) {
        // FIXME: This is assuming that the char is represented as its utf-8 encoding!
        unsafe { tb_change_cell(x as libc::c_int, y as libc::c_int, ch as u32, fg, bg) }
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
        unsafe { tb_select_output_mode(std::mem::transmute(mode)); }
    }
}

impl Drop for Termbox {
    fn drop(&mut self) {
        unsafe { tb_shutdown(); }
    }
}

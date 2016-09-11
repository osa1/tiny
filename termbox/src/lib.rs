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
pub struct tb_cell {
    pub ch: u32,
    pub fg: u16,
    pub bg: u16,
}

pub const TB_EUNSUPPORTED_TERMINAL : libc::c_int = -1;
pub const TB_EFAILED_TO_OPEN_TTY   : libc::c_int = -2;

pub const TB_HIDE_CURSOR      : libc::c_int = -1;

pub const TB_OUTPUT_CURRENT   : libc::c_int = 0;
pub const TB_OUTPUT_NORMAL    : libc::c_int = 1;
pub const TB_OUTPUT_256       : libc::c_int = 2;
pub const TB_OUTPUT_216       : libc::c_int = 3;
pub const TB_OUTPUT_GRAYSCALE : libc::c_int = 4;

extern {
    pub fn tb_init() -> libc::c_int;
    pub fn tb_resize();
    pub fn tb_shutdown();
    pub fn tb_width() -> libc::c_int;
    pub fn tb_height() -> libc::c_int;
    pub fn tb_clear() -> libc::c_int;
    pub fn tb_set_clear_attributes(fg: u16, bg: u16);
    pub fn tb_present();
    pub fn tb_set_cursor(cx: libc::c_int, cy: libc::c_int);
    pub fn tb_put_cell(x: libc::c_int, y: libc::c_int, cell: tb_cell);
    pub fn tb_change_cell(x: libc::c_int, y: libc::c_int, ch: u32, fg: u16, bg: u16);
    pub fn tb_cell_buffer() -> *mut tb_cell;
    pub fn tb_select_output_mode(mode: libc::c_int);
}

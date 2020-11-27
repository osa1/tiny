//! Implements Termbox's "cell" abstraction in Rust.
//!
//! A note on wide characters: if you put a character like `Ｈ` that takes more than one column to
//! render (according to [Unicode Standard Annex #11](http://www.unicode.org/reports/tr11/)), the
//! character takes more than one cell in termbox's internal grid and the character you put next to
//! it gets shifted in the grid. The character above takes 2 columns to render so if you put it on
//! column 0 and on column 1 you put `e`, this is what you get: `Ｈe` where the first character is
//! on column 0 and second character is on column 2.

use std::cmp::min;
use std::fs::File;
use std::io::Write;
use unicode_width::UnicodeWidthChar;

// FIXME: Colors are actually (u8, u8) for (style, ansi color)
// FIXME: Use enter_ca_mode(smcup)/exit_ca_mode(rmcup) from terminfo

pub const TB_DEFAULT: u16 = 0x0000;
pub const TB_BOLD: u16 = 0x0100;
pub const TB_UNDERLINE: u16 = 0x0200;

pub struct Termbox {
    // Not available in test instances
    tty: Option<File>,
    // Used to save `tty` on `suspend` and restore on `activate`
    old_tty: Option<File>,
    old_term: libc::termios,
    term_width: u16,
    term_height: u16,
    buffer_size_change_request: bool,
    back_buffer: CellBuf,
    front_buffer: CellBuf,
    clear_fg: u8,
    clear_bg: u8,
    last_fg: u16,
    last_bg: u16,
    // (x, y) coordinates of the user-visible cursor. `None` when it's hidden.
    cursor: Option<(u16, u16)>,
    // (x, y) coordinates of the terminal's cursor. This is where the next printed character will
    // appear. Note that the termional coordinates start from (1, 1), use (0, 0) to invalidate this
    // value.
    terminal_cursor: (u16, u16),
    output_buffer: Vec<u8>,
    // total_flushed: u64,
}

#[derive(Clone)]
pub struct CellBuf {
    pub cells: Box<[Cell]>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: u16,
    pub bg: u16,
}

const EMPTY_CELL: Cell = Cell {
    ch: ' ',
    fg: 0,
    bg: 0,
};

impl CellBuf {
    fn new(w: u16, h: u16) -> CellBuf {
        CellBuf {
            cells: vec![EMPTY_CELL; usize::from(w) * usize::from(h)].into_boxed_slice(),
        }
    }

    fn clear(&mut self, fg: u8, bg: u8) {
        for cell in self.cells.iter_mut() {
            cell.ch = ' ';
            cell.fg = u16::from(fg);
            cell.bg = u16::from(bg);
        }
    }

    fn resize(&mut self, old_w: u16, old_h: u16, w: u16, h: u16) {
        if old_w == w && old_h == h {
            return;
        }

        // Old cells should be visible at the top-left corner
        let mut new_cells = vec![EMPTY_CELL; usize::from(w) * usize::from(h)].into_boxed_slice();
        let minw = usize::from(min(old_w, w));
        let minh = usize::from(min(old_h, h));
        {
            let w = usize::from(w);
            let self_w = usize::from(old_w);
            for i in 0..minh {
                for j in 0..minw {
                    new_cells[i * w + j] = self.cells[i * self_w + j];
                }
            }
        }

        self.cells = new_cells;
    }
}

impl Termbox {
    pub fn init() -> std::io::Result<Termbox> {
        // We don't use termion's into_raw_mode() because it doesn't let us do
        // tcsetattr(tty, TCSAFLUSH, ...)
        let mut tty = termion::get_tty()?;
        // Basically just into_raw_mode() or cfmakeraw(), but we do it manually to set TCSAFLUSH
        let mut old_term: libc::termios = unsafe { std::mem::zeroed() };
        unsafe {
            libc::tcgetattr(libc::STDOUT_FILENO, &mut old_term);
        }

        // See also Drop impl where we reverse all this
        let mut new_term: libc::termios = old_term;
        new_term.c_iflag &= !(libc::IGNBRK
            | libc::BRKINT
            | libc::PARMRK
            | libc::ISTRIP
            | libc::INLCR
            | libc::IGNCR
            | libc::ICRNL
            | libc::IXON);
        new_term.c_oflag &= !libc::OPOST;
        new_term.c_lflag &= !(libc::ECHO | libc::ECHONL | libc::ICANON | libc::ISIG | libc::IEXTEN);
        new_term.c_cflag &= !(libc::CSIZE | libc::PARENB);
        new_term.c_cflag |= libc::CS8;
        // Enabled non-canonical mode above. Also set VMIN and VTIME = 0 so that `read(stdin)`
        // won't block. References:
        // - https://www.gnu.org/software/libc/manual/html_node/Canonical-or-Not.html
        // - https://www.gnu.org/software/libc/manual/html_node/Noncanonical-Input.html
        new_term.c_cc[libc::VMIN] = 0;
        new_term.c_cc[libc::VTIME] = 0;

        unsafe { libc::tcsetattr(libc::STDOUT_FILENO, libc::TCSAFLUSH, &new_term) };
        // T_ENTER_CA for xterm
        tty.write_all(b"\x1b[?1049h").unwrap();

        // Done with setting terminal attributes

        let (term_width, term_height) = termion::terminal_size()?;
        let clear_fg = 0;
        let clear_bg = 0;
        let mut back_buffer = CellBuf::new(term_width, term_height);
        back_buffer.clear(clear_fg, clear_bg);
        let mut front_buffer = CellBuf::new(term_width, term_height);
        front_buffer.clear(clear_fg, clear_bg);
        let mut termbox = Termbox {
            tty: Some(tty),
            old_tty: None,
            old_term,
            term_width,
            term_height,
            buffer_size_change_request: false,
            back_buffer,
            front_buffer,
            clear_fg,
            clear_bg,
            last_fg: 0,
            last_bg: 0,
            cursor: Some((0, 0)),
            terminal_cursor: (0, 0),
            output_buffer: Vec::with_capacity(32 * 1024),
            // total_flushed: 0,
        };

        termbox.hide_cursor();
        termbox.send_clear();

        Ok(termbox)
    }

    pub fn init_test(w: u16, h: u16) -> Termbox {
        Termbox {
            tty: None,
            old_tty: None,
            old_term: unsafe { std::mem::zeroed() },
            term_width: w,
            term_height: h,
            buffer_size_change_request: false,
            back_buffer: CellBuf::new(w, h),
            front_buffer: CellBuf::new(w, h),
            clear_fg: 0,
            clear_bg: 0,
            last_fg: 0,
            last_bg: 0,
            cursor: Some((0, 0)),
            terminal_cursor: (0, 0),
            output_buffer: Vec::with_capacity(32 * 1024),
        }
    }

    // Swap current term with old_term
    fn flip_terms(&mut self) {
        let mut current_term: libc::termios = unsafe { std::mem::zeroed() };
        unsafe {
            libc::tcgetattr(libc::STDOUT_FILENO, &mut current_term);
        }
        unsafe { libc::tcsetattr(libc::STDOUT_FILENO, libc::TCSAFLUSH, &self.old_term) };
        self.old_term = current_term;
    }

    // HACKY
    pub fn suspend(&mut self) {
        self.flip_terms();
        self.old_tty = self.tty.take();

        self.output_buffer
            .extend_from_slice(termion::cursor::Show.as_ref());
        self.output_buffer
            .extend_from_slice(termion::style::Reset.as_ref());
        self.output_buffer
            .extend_from_slice(termion::clear::All.as_ref());
        // T_EXIT_CA for xterm
        self.output_buffer.extend_from_slice(b"\x1b[?1049l");

        self.flush_output_buffer();
    }

    // HACKY
    pub fn activate(&mut self) {
        self.flip_terms();
        self.tty = self.old_tty.take();

        // T_ENTER_CA for xterm
        if let Some(ref mut tty) = self.tty {
            tty.write_all(b"\x1b[?1049h").unwrap();
        }

        self.buffer_size_change_request = true;
        self.present();
    }

    pub fn resize(&mut self) {
        self.buffer_size_change_request = true;
    }

    pub fn width(&self) -> i32 {
        self.term_width as i32
    }

    pub fn height(&self) -> i32 {
        self.term_height as i32
    }

    pub fn clear(&mut self) {
        if self.buffer_size_change_request {
            self.update_size();
            self.buffer_size_change_request = false;
        }
        self.back_buffer.clear(self.clear_fg, self.clear_bg);
    }

    pub fn set_clear_attributes(&mut self, fg: u8, bg: u8) {
        self.clear_fg = fg;
        self.clear_bg = bg;
    }

    pub fn present(&mut self) {
        // Invalidate the terminal cursor
        self.terminal_cursor = (0, 0);

        if self.buffer_size_change_request {
            self.update_size();
            self.buffer_size_change_request = false;
        }

        for y in 0..usize::from(self.term_height) {
            let mut x = 0;
            while x < usize::from(self.term_width) {
                let front_cell =
                    &mut self.front_buffer.cells[(y * usize::from(self.term_width)) + x];
                let back_cell = self.back_buffer.cells[(y * usize::from(self.term_width)) + x];
                // TODO: For 0-width chars maybe only move to the next cell in the back buffer?
                let cw0 = UnicodeWidthChar::width(back_cell.ch).unwrap_or(1);
                let cw = std::cmp::max(cw0, 1);
                // eprintln!("UnicodeWidthChar({:?}) = {}", back_cell.ch, cw);
                if *front_cell == back_cell {
                    x += cw;
                    continue;
                }
                *front_cell = back_cell;

                self.send_attr(back_cell.fg, back_cell.bg);

                if cw > 1 && (x + (cw - 1)) >= usize::from(self.term_width) {
                    // Not enough room for wide ch, send spaces
                    for i in x..usize::from(self.term_width) {
                        self.send_char(i as u16, y as u16, ' ', 1);
                    }
                } else if cw0 == 0 {
                    self.send_char(x as u16, y as u16, ' ', 1);
                } else {
                    self.send_char(x as u16, y as u16, back_cell.ch, cw as u16);
                    // We're going to skip `cw` cells so for wide chars fill the slop in the front
                    // buffer so that if we put a non-wide character lto this cell later next
                    // columns won't be bogus.
                    for i in 1..cw {
                        let mut front_cell = &mut self.front_buffer.cells
                            [(y * usize::from(self.term_width)) + x + i];
                        front_cell.ch = ' ';
                        front_cell.fg = back_cell.fg;
                        front_cell.bg = back_cell.bg;
                    }
                }

                x += cw;
            }
        }

        if let Some((x, y)) = self.cursor {
            goto(&mut self.output_buffer, x + 1, y + 1);
        }

        self.flush_output_buffer();
    }

    pub fn hide_cursor(&mut self) {
        if self.cursor.is_some() {
            self.cursor = None;
            self.output_buffer
                .extend_from_slice(termion::cursor::Hide.as_ref());
        }
    }

    pub fn set_cursor(&mut self, xy: Option<(u16, u16)>) {
        match xy {
            None => match self.cursor {
                None => {}
                Some(_) => {
                    self.cursor = None;
                    self.output_buffer
                        .extend_from_slice(termion::cursor::Hide.as_ref());
                }
            },
            Some((x, y)) => match self.cursor {
                None => {
                    self.cursor = Some((x, y));
                    goto(&mut self.output_buffer, x + 1, y + 1);
                    self.output_buffer
                        .extend_from_slice(termion::cursor::Show.as_ref());
                }
                Some((x_, y_)) => {
                    if x != x_ || y != y_ {
                        self.cursor = Some((x, y));
                        goto(&mut self.output_buffer, x + 1, y + 1);
                    }
                }
            },
        }
    }

    // TODO: parameters should be u32
    pub fn change_cell(&mut self, x: i32, y: i32, ch: char, fg: u16, bg: u16) {
        debug_assert!(x >= 0);
        debug_assert!(y >= 0);
        let mut cell =
            &mut self.back_buffer.cells[(y as usize) * (self.term_width as usize) + (x as usize)];
        cell.ch = ch;
        cell.fg = fg;
        cell.bg = bg;
    }

    fn flush_output_buffer(&mut self) {
        // self.total_flushed += self.output_buffer.len() as u64;
        if let Some(ref mut tty) = self.tty {
            tty.write_all(&self.output_buffer).unwrap();
        }
        self.output_buffer.clear();
    }

    fn update_size(&mut self) {
        let old_w = self.term_width;
        let old_h = self.term_height;
        let (w, h) = termion::terminal_size().unwrap();
        self.term_width = w;
        self.term_height = h;
        self.back_buffer.resize(old_w, old_h, w, h);
        self.front_buffer.resize(old_w, old_h, w, h);
        self.front_buffer.clear(self.clear_fg, self.clear_bg);
        self.send_clear();
    }

    fn send_clear(&mut self) {
        self.send_attr(u16::from(self.clear_fg), u16::from(self.clear_bg));
        self.output_buffer
            .extend_from_slice(termion::clear::All.as_ref());
        // TODO: Reset cursor position
        self.flush_output_buffer();
    }

    fn send_attr(&mut self, fg: u16, bg: u16) {
        if fg == self.last_fg && bg == self.last_bg {
            return;
        }

        let bold = fg & TB_BOLD != 0;
        let underline = fg & TB_UNDERLINE != 0;

        self.last_fg = fg;
        self.last_bg = bg;

        let fg = fg as u8;
        let bg = bg as u8;

        self.output_buffer
            .extend_from_slice(termion::style::Reset.as_ref());

        if underline {
            self.output_buffer
                .extend_from_slice(termion::style::Underline.as_ref());
        }

        if bold {
            self.output_buffer
                .extend_from_slice(termion::style::Bold.as_ref());
        }

        if fg != 0 {
            write!(
                self.output_buffer,
                "{}",
                termion::color::Fg(termion::color::AnsiValue(fg)),
            )
            .unwrap();
        }

        if bg != 0 {
            write!(
                self.output_buffer,
                "{}",
                termion::color::Bg(termion::color::AnsiValue(bg))
            )
            .unwrap();
        }
    }

    // input coordiates are 0-based
    fn send_char(&mut self, to_x: u16, to_y: u16, ch: char, cw: u16) {
        let to_x = to_x + 1;
        let to_y = to_y + 1;

        // if the target cell isn't next to the last cell, then move the cursor first
        if self.terminal_cursor.0 != to_x || self.terminal_cursor.1 != to_y {
            goto(&mut self.output_buffer, to_x, to_y);
        }
        write!(&mut self.output_buffer, "{}", ch).unwrap();

        self.terminal_cursor = (to_x + cw, to_y);
    }
}

fn num_to_buf(buf: &mut Vec<u8>, mut num: u16) {
    let start_len = buf.len();
    let mut chars_len = 0;
    loop {
        let rem = num % 10;
        let ch = b'0' + rem as u8;
        buf.push(ch);
        num /= 10;
        chars_len += 1;
        if num == 0 {
            break;
        }
    }

    let swap_len = start_len + (chars_len / 2) as usize;

    for (c, i) in (start_len..swap_len).enumerate() {
        let next_swap_idx = start_len + chars_len - c - 1;
        buf.swap(next_swap_idx, i);
    }
}

// Inputs are 1-based
fn goto(buf: &mut Vec<u8>, x: u16, y: u16) {
    debug_assert!(x > 0 && y > 0);
    buf.extend_from_slice(b"\x1B[");
    num_to_buf(buf, y);
    buf.push(b';');
    num_to_buf(buf, x);
    buf.push(b'H');
}

impl Drop for Termbox {
    fn drop(&mut self) {
        self.output_buffer
            .extend_from_slice(termion::cursor::Show.as_ref());
        self.output_buffer
            .extend_from_slice(termion::style::Reset.as_ref());
        self.output_buffer
            .extend_from_slice(termion::clear::All.as_ref());
        // T_EXIT_CA for xterm
        self.output_buffer.extend_from_slice(b"\x1b[?1049l");
        self.flush_output_buffer();

        if self.tty.is_some() {
            unsafe {
                libc::tcsetattr(libc::STDOUT_FILENO, libc::TCSAFLUSH, &self.old_term);
            }
        }

        //  eprintln!("Total bytes flushed: {}", self.total_flushed);
    }
}

//
// Testing API
//

impl Termbox {
    /// Returns a copy of the front buffer. Useful when testing.
    pub fn get_front_buffer(&self) -> CellBuf {
        self.front_buffer.clone()
    }

    /// Sets size of the buffers. Useful when testing.
    pub fn set_buffer_size(&mut self, w: u16, h: u16) {
        let old_w = self.term_width;
        let old_h = self.term_height;
        self.term_width = w;
        self.term_height = h;
        self.back_buffer.resize(old_w, old_h, w, h);
        self.front_buffer.resize(old_w, old_h, w, h);
        self.front_buffer.clear(self.clear_fg, self.clear_bg);
    }
}

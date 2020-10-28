#![allow(clippy::new_without_default)]

//! Interprets the terminal events we care about (keyboard input).
//!
//! Keyboard events are read from stdin. We look for byte strings of key combinations that we care
//! about. E.g. Alt-arrow keys, C-w etc.

#[cfg(test)]
mod tests;

use std::char;
use std::collections::VecDeque;
use std::os::unix::io::{AsRawFd, RawFd};
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::stream::Stream;
use tokio::io::unix::AsyncFd;

use term_input_macros::byte_seq_parser;

////////////////////////////////////////////////////////////////////////////////////////////////////
// Public types
////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    AltArrow(Arrow),
    AltChar(char),
    Arrow(Arrow),
    Backspace,
    Char(char),
    Ctrl(char),
    CtrlArrow(Arrow),
    Del,
    End,
    Esc,
    Home,
    PageDown,
    PageUp,
    ShiftDown,
    ShiftUp,
    Tab,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A single key input.
    Key(Key),

    /// Usually a paste.
    String(String),

    // These are not generated anymore. I needed these for a feature, but never really got around
    // implementing it.
    // FocusGained,
    // FocusLost,
    /// An unknown sequence of bytes (probably for a key combination that we don't care about).
    Unknown(Vec<u8>),
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Byte sequences of key presses we want to capture
////////////////////////////////////////////////////////////////////////////////////////////////////

byte_seq_parser! {
    parse_key_bytes -> Key, // Function name. Generation function type is
                            // fn(&[u8]) -> Option<(Key, usize)>.

    [27, 91, 49, 59, 51, 66] => Key::AltArrow(Arrow::Down),
    [27, 91, 49, 59, 51, 68] => Key::AltArrow(Arrow::Left),
    [27, 91, 49, 59, 51, 67] => Key::AltArrow(Arrow::Right),
    [27, 91, 49, 59, 51, 65] => Key::AltArrow(Arrow::Up),
    [27, 91, 66] => Key::Arrow(Arrow::Down),
    [27, 79, 66] => Key::Arrow(Arrow::Down),
    [27, 91, 68] => Key::Arrow(Arrow::Left),
    [27, 79, 68] => Key::Arrow(Arrow::Left),
    [27, 91, 67] => Key::Arrow(Arrow::Right),
    [27, 79, 67] => Key::Arrow(Arrow::Right),
    [27, 91, 65] => Key::Arrow(Arrow::Up),
    [27, 79, 65] => Key::Arrow(Arrow::Up),
    [27, 91, 49, 59, 53, 66] => Key::CtrlArrow(Arrow::Down),
    [27, 91, 49, 59, 53, 68] => Key::CtrlArrow(Arrow::Left),
    [27, 91, 49, 59, 53, 67] => Key::CtrlArrow(Arrow::Right),
    [27, 91, 49, 59, 53, 65] => Key::CtrlArrow(Arrow::Up),
    [27, 91, 51, 126] => Key::Del,
    [27, 91, 54, 126] => Key::PageDown,
    [27, 91, 53, 126] => Key::PageUp,
    [27, 91, 49, 59, 50, 65] => Key::ShiftUp,
    [27, 91, 49, 59, 50, 66] => Key::ShiftDown,
    [27, 91, 72] => Key::Home,
    [27, 91, 70] => Key::End,
    [27, 79, 72] => Key::Home,
    [27, 79, 70] => Key::End,
    [27, 91, 52, 126] => Key::End,
    [9] => Key::Tab,
    [127] => Key::Backspace,
    [1] => Key::Ctrl('a'),
    [5] => Key::Ctrl('e'),
    [23] => Key::Ctrl('w'),
    [11] => Key::Ctrl('k'),
    [4] => Key::Ctrl('d'),
    [3] => Key::Ctrl('c'),
    [17] => Key::Ctrl('q'),
    [16] => Key::Ctrl('p'),
    [14] => Key::Ctrl('n'),
    [21] => Key::Ctrl('u'),
    [24] => Key::Ctrl('x'),
}

// static XTERM_FOCUS_GAINED: [u8; 3] = [27, 91, 73];
// static XTERM_FOCUS_LOST: [u8; 3] = [27, 91, 79];

////////////////////////////////////////////////////////////////////////////////////////////////////

struct RawFd_(RawFd);

impl AsRawFd for RawFd_ {
    fn as_raw_fd(&self) -> RawFd {
        self.0
    }
}

pub struct Input {
    /// Queue of events waiting to be polled.
    evs: VecDeque<Event>,

    /// Used when reading from stdin.
    buf: Vec<u8>,

    stdin: AsyncFd<RawFd_>,
}

impl Input {
    pub fn new() -> Input {
        Input {
            evs: VecDeque::new(),
            buf: Vec::with_capacity(100),
            stdin: AsyncFd::new(RawFd_(libc::STDIN_FILENO)).unwrap(),
        }
    }
}

impl Stream for Input {
    type Item = std::io::Result<Event>;

    fn poll_next(mut self: Pin<&mut Input>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let self_: &mut Input = &mut *self;

        // Try to parse any bytes in the input buffer from the last poll
        {
            let mut buf_slice: &[u8] = &self_.buf;

            while !buf_slice.is_empty() {
                // Special treatment for 127 (backspace, 0x1B) and 13 ('\r', 0xD)
                let fst = buf_slice[0];
                let parse_fn = if (fst < 32 && fst != 13) || fst == 127 {
                    parse_key_comb
                } else {
                    parse_chars
                };

                match parse_fn(buf_slice) {
                    Some((ev, used)) => {
                        buf_slice = &buf_slice[used..];
                        self_.evs.push_back(ev);
                    }
                    None => {
                        self_.evs.push_back(Event::Unknown(buf_slice.to_owned()));
                        break;
                    }
                }
            }
        }

        self_.buf.clear();

        // Yield pending events
        if let Some(ev) = self_.evs.pop_front() {
            return Poll::Ready(Some(Ok(ev)));
        }

        // Otherwise read stdin and loop if successful
        match self_.stdin.poll_read_ready(cx) {
            Poll::Ready(Ok(mut ready)) => {
                if read_stdin(&mut self_.buf) {
                    Input::poll_next(self, cx)
                } else {
                    ready.clear_ready();
                    Poll::Pending
                }
            }
            Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
pub(crate) fn parse_single_event(buf: &[u8]) -> Event {
    let fst = buf[0];
    let parse_fn = if (fst < 32 && fst != 13) || fst == 127 {
        parse_key_comb
    } else {
        parse_chars
    };

    let (ev, used) = match parse_fn(buf) {
        Some((ev, used)) => (ev, used),
        None => (Event::Unknown(buf.to_owned()), buf.len()),
    };

    assert_eq!(buf.len(), used);

    ev
}

fn parse_chars(buf: &[u8]) -> Option<(Event, usize)> {
    debug_assert!(!buf.is_empty());

    // Use a fast path for the common case: Single utf-8 character.
    // (TODO: What about other encodings?)

    utf8_char_len(buf[0]).map(|char_len| {
        if char_len as usize == buf.len() {
            // Fast path: single character
            let ev = Event::Key(Key::Char(get_utf8_char(buf, char_len)));
            (ev, char_len as usize)
        } else {
            // Probably a paste: allocate a string and collect chars
            let mut string = String::with_capacity(1000);
            let mut start_idx = 0;
            loop {
                if start_idx == buf.len() {
                    break;
                }
                match utf8_char_len(buf[start_idx]) {
                    Some(char_len) => {
                        string.push(get_utf8_char(&buf[start_idx..], char_len));
                        start_idx += char_len as usize;
                    }
                    None => {
                        break;
                    }
                }
            }
            let ev = Event::String(string);
            (ev, start_idx)
        }
    })
}

fn parse_key_comb(buf: &[u8]) -> Option<(Event, usize)> {
    debug_assert!(!buf.is_empty());

    if let Some((key, used)) = parse_key_bytes(buf) {
        return Some((Event::Key(key), used));
    }

    if buf[0] == 27 {
        // 0x1B, ESC
        // 27 not followed by anything is an actual ESC
        if buf.len() == 1 {
            return Some((Event::Key(Key::Esc), 1));
        }
        // otherwise it's probably alt + key
        else {
            debug_assert!(buf.len() >= 2);
            return utf8_char_len(buf[1]).map(|char_len| {
                let ev = Event::Key(Key::AltChar(get_utf8_char(&buf[1..], char_len)));
                (ev, char_len as usize + 1)
            });
        }
    }

    None
}

fn utf8_char_len(byte: u8) -> Option<u8> {
    if byte >> 7 == 0b0 {
        Some(1)
    } else if byte >> 5 == 0b110 {
        Some(2)
    } else if byte >> 4 == 0b1110 {
        Some(3)
    } else if byte >> 3 == 0b11110 {
        Some(4)
    } else {
        None
    }
}

fn get_utf8_char(buf: &[u8], len: u8) -> char {
    let codepoint: u32 = {
        if len == 1 {
            u32::from(buf[0] & 0b0111_1111)
        } else if len == 2 {
            ((u32::from(buf[0] & 0b0001_1111)) << 6) + (u32::from(buf[1] & 0b0011_1111))
        } else if len == 3 {
            ((u32::from(buf[0] & 0b0000_1111)) << 12)
                + ((u32::from(buf[1] & 0b0011_1111)) << 6)
                + (u32::from(buf[2] & 0b0011_1111))
        } else {
            debug_assert!(len == 4);
            ((u32::from(buf[0] & 0b0000_0111)) << 18)
                + ((u32::from(buf[1] & 0b0011_1111)) << 12)
                + ((u32::from(buf[2] & 0b0011_1111)) << 6)
                + (u32::from(buf[3] & 0b0011_1111))
        }
    };

    char::from_u32(codepoint).unwrap()
}

/// Read stdin contents if it's ready for reading. Returns true when it was able to read. Buffer is
/// not modified when return value is 0.
pub fn read_stdin(buf: &mut Vec<u8>) -> bool {
    let mut bytes_available: i32 = 0; // this really needs to be a 32-bit value
    let ioctl_ret =
        unsafe { libc::ioctl(libc::STDIN_FILENO, libc::FIONREAD, &mut bytes_available) };
    // println!("ioctl_ret: {}", ioctl_ret);
    // println!("bytes_available: {}", bytes_available);
    if ioctl_ret < 0 || bytes_available == 0 {
        false
    } else {
        buf.clear();
        buf.reserve(bytes_available as usize);

        let buf_ptr: *mut libc::c_void = buf.as_ptr() as *mut libc::c_void;
        let bytes_read =
            unsafe { libc::read(libc::STDIN_FILENO, buf_ptr, bytes_available as usize) };
        debug_assert!(bytes_read == bytes_available as isize);

        unsafe {
            buf.set_len(bytes_read as usize);
        }
        true
    }
}

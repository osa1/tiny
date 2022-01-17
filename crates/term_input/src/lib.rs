#![allow(clippy::new_without_default, clippy::collapsible_match)]

//! Interprets the terminal events we care about (keyboard input).
//!
//! Keyboard events are read from `stdin`. We look for byte strings of key combinations that we
//! care about. E.g. Alt-arrow keys, C-w etc.

#[cfg(test)]
mod tests;

use term_input_macros::byte_seq_parser;

use std::char;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::task::{Context, Poll};

use nix::fcntl::{fcntl, FcntlArg, OFlag};
use tokio::io::unix::AsyncFd;
use tokio::io::Interest;
use tokio_stream::Stream;

#[macro_use]
extern crate log;

////////////////////////////////////////////////////////////////////////////////////////////////////
// Public types
////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
pub enum Key {
    AltArrow(Arrow),
    AltChar(char),
    AltF(FKey),
    Arrow(Arrow),
    Backspace,
    Char(char),
    Ctrl(char),
    CtrlArrow(Arrow),
    CtrlF(FKey),
    Del,
    End,
    Esc,
    FKey(FKey),
    Home,
    PageDown,
    PageUp,
    ShiftArrow(Arrow),
    ShiftF(FKey),
    Tab,
}

#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
pub enum Arrow {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Copy, Debug, PartialEq, Hash, Eq)]
pub enum FKey {
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
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

// Note: `showkey -a` is an easy way to get these bytes
byte_seq_parser! {
    parse_key_bytes -> Key, // Function name. Generation function type is
                            // fn(&[u8]) -> Option<(Key, usize)>.

    [27, 91, 49, 59, 51, 66] => Key::AltArrow(Arrow::Down),
    [27, 91, 49, 59, 51, 68] => Key::AltArrow(Arrow::Left),
    [27, 91, 49, 59, 51, 67] => Key::AltArrow(Arrow::Right),
    [27, 91, 49, 59, 51, 65] => Key::AltArrow(Arrow::Up),
    [27, 91, 49, 59, 51, 80] => Key::AltF(FKey::F1),
    [27, 91, 49, 59, 51, 81] => Key::AltF(FKey::F2),
    [27, 91, 49, 59, 51, 82] => Key::AltF(FKey::F3),
    [27, 91, 49, 59, 51, 83] => Key::AltF(FKey::F4),
    [27, 91, 49, 53, 59, 51, 126] => Key::AltF(FKey::F5),
    [27, 91, 49, 55, 59, 51, 126] => Key::AltF(FKey::F6),
    [27, 91, 49, 56, 59, 51, 126] => Key::AltF(FKey::F7),
    [27, 91, 49, 57, 59, 51, 126] => Key::AltF(FKey::F8),
    [27, 91, 50, 48, 59, 51, 126] => Key::AltF(FKey::F9),
    [27, 91, 50, 49, 59, 51, 126] => Key::AltF(FKey::F10),
    [27, 91, 50, 51, 59, 51, 126] => Key::AltF(FKey::F11),
    [27, 91, 50, 52, 59, 51, 126] => Key::AltF(FKey::F12),
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
    [27, 91, 49, 59, 53, 80] => Key::CtrlF(FKey::F1),
    [27, 91, 49, 59, 53, 81] => Key::CtrlF(FKey::F2),
    [27, 91, 49, 59, 53, 82] => Key::CtrlF(FKey::F3),
    [27, 91, 49, 59, 53, 83] => Key::CtrlF(FKey::F4),
    [27, 91, 49, 53, 59, 53, 126] => Key::CtrlF(FKey::F5),
    [27, 91, 49, 55, 59, 53, 126] => Key::CtrlF(FKey::F6),
    [27, 91, 49, 56, 59, 53, 126] => Key::CtrlF(FKey::F7),
    [27, 91, 49, 57, 59, 53, 126] => Key::CtrlF(FKey::F8),
    [27, 91, 50, 48, 59, 53, 126] => Key::CtrlF(FKey::F9),
    [27, 91, 50, 49, 59, 53, 126] => Key::CtrlF(FKey::F10),
    [27, 91, 50, 51, 59, 53, 126] => Key::CtrlF(FKey::F11),
    [27, 91, 50, 52, 59, 53, 126] => Key::CtrlF(FKey::F12),
    [27, 91, 51, 126] => Key::Del,
    [27, 91, 54, 126] => Key::PageDown,
    [27, 91, 53, 126] => Key::PageUp,
    [27, 91, 49, 59, 50, 65] => Key::ShiftArrow(Arrow::Up),
    [27, 91, 49, 59, 50, 66] => Key::ShiftArrow(Arrow::Down),
    [27, 91, 49, 59, 50, 68] => Key::ShiftArrow(Arrow::Left),
    [27, 91, 49, 59, 50, 67] => Key::ShiftArrow(Arrow::Right),
    [27, 91, 49, 59, 50, 80] => Key::ShiftF(FKey::F1),
    [27, 91, 49, 59, 50, 81] => Key::ShiftF(FKey::F2),
    [27, 91, 49, 59, 50, 82] => Key::ShiftF(FKey::F3),
    [27, 91, 49, 59, 50, 83] => Key::ShiftF(FKey::F4),
    [27, 91, 49, 53, 59, 50, 126] => Key::ShiftF(FKey::F5),
    [27, 91, 49, 55, 59, 50, 126] => Key::ShiftF(FKey::F6),
    [27, 91, 49, 56, 59, 50, 126] => Key::ShiftF(FKey::F7),
    [27, 91, 49, 57, 59, 50, 126] => Key::ShiftF(FKey::F8),
    [27, 91, 50, 48, 59, 50, 126] => Key::ShiftF(FKey::F9),
    [27, 91, 50, 49, 59, 50, 126] => Key::ShiftF(FKey::F10),
    [27, 91, 50, 51, 59, 50, 126] => Key::ShiftF(FKey::F11),
    [27, 91, 50, 52, 59, 50, 126] => Key::ShiftF(FKey::F12),
    [27, 91, 72] => Key::Home,
    [27, 91, 70] => Key::End,
    [27, 79, 72] => Key::Home,
    [27, 79, 70] => Key::End,
    [27, 91, 52, 126] => Key::End,
    [27, 79, 80] => Key::FKey(FKey::F1),
    [27, 79, 81] => Key::FKey(FKey::F2),
    [27, 79, 82] => Key::FKey(FKey::F3),
    [27, 79, 83] => Key::FKey(FKey::F4),
    [27, 91, 49, 53, 126] => Key::FKey(FKey::F5),
    [27, 91, 49, 55, 126] => Key::FKey(FKey::F6),
    [27, 91, 49, 56, 126] => Key::FKey(FKey::F7),
    [27, 91, 49, 57, 126] => Key::FKey(FKey::F8),
    [27, 91, 50, 48, 126] => Key::FKey(FKey::F9),
    [27, 91, 50, 49, 126] => Key::FKey(FKey::F10),
    [27, 91, 50, 51, 126] => Key::FKey(FKey::F11),
    [27, 91, 50, 52, 126] => Key::FKey(FKey::F12),
    [9] => Key::Tab,
    [8] => Key::Backspace,
    [127] => Key::Backspace,
    [1] => Key::Ctrl('a'),
    [2] => Key::Ctrl('b'),
    [3] => Key::Ctrl('c'),
    [4] => Key::Ctrl('d'),
    [5] => Key::Ctrl('e'),
    [6] => Key::Ctrl('f'),
    [7] => Key::Ctrl('g'),
    [10] => Key::Ctrl('j'),
    [11] => Key::Ctrl('k'),
    [12] => Key::Ctrl('l'),
    [13] => Key::Ctrl('m'),
    [14] => Key::Ctrl('n'),
    [15] => Key::Ctrl('o'),
    [16] => Key::Ctrl('p'),
    [17] => Key::Ctrl('q'),
    [18] => Key::Ctrl('r'),
    [19] => Key::Ctrl('s'),
    [20] => Key::Ctrl('t'),
    [21] => Key::Ctrl('u'),
    [22] => Key::Ctrl('v'),
    [23] => Key::Ctrl('w'),
    [24] => Key::Ctrl('x'),
    [25] => Key::Ctrl('y'),
    [26] => Key::Ctrl('z'),
    [28] => Key::Ctrl('\\'),
    [29] => Key::Ctrl(']'),
    [31] => Key::Ctrl('/'),
}

// static XTERM_FOCUS_GAINED: [u8; 3] = [27, 91, 73];
// static XTERM_FOCUS_LOST: [u8; 3] = [27, 91, 79];

pub fn is_valid_key(key: Key) -> bool {
    parse_key_bytes_is_valid_key(key) | matches!(key, Key::AltChar(_) | Key::Char(_))
}

////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct Input {
    /// Queue of events waiting to be polled.
    evs: VecDeque<Event>,

    /// Used when reading from stdin.
    buf: Vec<u8>,

    stdin: AsyncFd<RawFd>,

    old_stdin_flags: Option<OFlag>,
}

impl Drop for Input {
    fn drop(&mut self) {
        if let Some(old_flags) = self.old_stdin_flags.take() {
            if let Err(err) = fcntl(libc::STDIN_FILENO, FcntlArg::F_SETFL(old_flags)) {
                error!("Unable to restore stdin flags: {:?}", err);
            }
        }
    }
}

impl Input {
    /// Create an input handler. Requires a `tokio` reactor to be running. Sets `stdin` to
    /// non-blocking mode. `stdin` flags are restored when the returned `Input` is dropped.
    ///
    /// Uses `stdin` so make sure you don't call this when there's another `Input` instance in the
    /// process.
    ///
    /// Note that if you're using this with a terminal library like termbox you probably already
    /// enable non-canonical input, in which case stdin doesn't need to be in non-blocking mode on
    /// Linux, but on WSL we still need non-blocking mode, so this just sets stdin to non-blocking
    /// mode always. See [tiny#269][1] and [WSL#3507][2].
    ///
    /// [1]: https://github.com/osa1/tiny/issues/269
    /// [2]: https://github.com/microsoft/WSL/issues/3507
    pub fn new() -> Input {
        let old_stdin_flags = set_stdin_nonblocking();
        Input {
            evs: VecDeque::new(),
            buf: Vec::with_capacity(100),
            stdin: AsyncFd::with_interest(libc::STDIN_FILENO, Interest::READABLE).unwrap(),
            old_stdin_flags,
        }
    }

    fn parse_buffer(&mut self) {
        let mut buf_slice: &[u8] = &self.buf;

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
                    self.evs.push_back(ev);
                }
                None => {
                    self.evs.push_back(Event::Unknown(buf_slice.to_owned()));
                    break;
                }
            }
        }

        self.buf.clear();
    }
}

impl Stream for Input {
    type Item = std::io::Result<Event>;

    fn poll_next(mut self: Pin<&mut Input>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let self_: &mut Input = &mut *self;

        'main: loop {
            // Try to parse any bytes in the input buffer from the last poll
            self_.parse_buffer();

            // Yield pending events
            if let Some(ev) = self_.evs.pop_front() {
                return Poll::Ready(Some(Ok(ev)));
            }

            // Otherwise read stdin and loop if successful
            let mut poll_ret = self_.stdin.poll_read_ready(cx);
            loop {
                match poll_ret {
                    Poll::Ready(Ok(mut ready)) => {
                        // We will read stdin until it's empty, clear the readiness state now
                        // (before reading) to avoid a potential race that would occur if stdin
                        // becomes available again between the last read and clear_ready.
                        ready.clear_ready();

                        // Read stdin until it fails with EWOULDBLOCK or returns 0
                        let read_ret = read_stdin(&mut self_.buf);

                        // Poll again so that we will be notified when stdin becomes ready again
                        poll_ret = self_.stdin.poll_read_ready(cx);

                        match read_ret {
                            Ok(()) => {}
                            Err(err) => {
                                // NOTE: `poll_ret` is ignored here but I think that's OK?
                                // TODO: Contents of `buf` is ignored here until `poll_next` is
                                // called again.
                                let err =
                                    std::io::Error::from(err.as_errno().expect("Weird nix error"));
                                return Poll::Ready(Some(Err(err)));
                            }
                        }

                        if self_.buf.is_empty() {
                            continue;
                        } else {
                            continue 'main;
                        }
                    }
                    Poll::Ready(Err(err)) => {
                        return Poll::Ready(Some(Err(err)));
                    }
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                }
            }
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

/// How many bytes to read from `stdin` in a single `read()`
const STDIN_READ_SIZE: u8 = 100;

/// Read `stdin` until `read` fails with `EWOULDBLOCK` (happens in non-canonical mode, when `stdin`
/// is set to non-blocking mode) or returns 0 (happens in non-canonical mode when `VMIN` and
/// `VTIME` are 0). If you are using `term_input` with `termbox` then you don't need to set `stdin`
/// to non-blocking mode as `termbox` enables non-canonical mode.
///
/// See also:
///
/// - <https://www.gnu.org/software/libc/manual/html_node/Canonical-or-Not.html>
/// - <https://www.gnu.org/software/libc/manual/html_node/Noncanonical-Input.html>
pub fn read_stdin(buf: &mut Vec<u8>) -> Result<(), nix::Error> {
    loop {
        let old_len = buf.len();
        buf.reserve(usize::from(STDIN_READ_SIZE));

        let read_ptr = unsafe { buf.as_mut_ptr().add(old_len) };

        let n_read = unsafe { libc::read(libc::STDIN_FILENO, read_ptr as *mut libc::c_void, 100) };

        assert!(n_read <= isize::from(STDIN_READ_SIZE));

        match n_read.cmp(&0) {
            Ordering::Equal => {
                // We're in non-canonical mode, or stdin is closed. We can't distinguish the two
                // here but I think it's fine to return OK when stdin is closed.
                return Ok(());
            }
            Ordering::Greater => {
                unsafe { buf.set_len(old_len + n_read as usize) };
            }
            Ordering::Less => match nix::Error::last() {
                nix::Error::Sys(nix::errno::EWOULDBLOCK) => return Ok(()),
                other => return Err(other),
            },
        }
    }
}

/// Set `stdin` to non-blocking mode. Returns old `stdin` if we were able to change the flags. Does
/// not panic; errors are logged.
fn set_stdin_nonblocking() -> Option<OFlag> {
    let current_stdin_flags: OFlag = match fcntl(libc::STDIN_FILENO, FcntlArg::F_GETFL) {
        Err(err) => {
            error!("Unable to read stdin flags: {:?}", err);
            return None;
        }
        Ok(flags) => match OFlag::from_bits(flags) {
            None => {
                error!("Unable to parse stdin flags: {:x?}", flags);
                return None;
            }
            Some(flags) => flags,
        },
    };

    let mut new_stdin_flags = current_stdin_flags;
    new_stdin_flags.set(OFlag::O_NONBLOCK, true);

    match fcntl(libc::STDIN_FILENO, FcntlArg::F_SETFL(new_stdin_flags)) {
        Err(err) => {
            // On Linux we don't really need to enable non-blocking mode so things should still
            // work. On WSL things may or may not work.. see #269.
            error!("Unable to set stdin flags: {:?}", err);
            None
        }
        Ok(_) => Some(current_stdin_flags),
    }
}

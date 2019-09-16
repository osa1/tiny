#![allow(clippy::new_without_default)]

//! Interprets the terminal events we care about:
//!
//!   - Resize events.
//!   - Keyboard input.
//!
//! Resize events are handled by registering a signal handler for SIGWINCH.
//!
//! Keyboard events are read from stdin. We look for byte strings of key combinations that we care
//! about. E.g. Alt-arrow keys, C-w etc.

use nix::sys::signal;
use nix::sys::signal::{sigaction, SigAction, SigHandler, SigSet, Signal};

use std::char;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::task::{Context, Poll};

use tokio::prelude::*;

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

    /// SIGWINCH happened.
    Resize,

    FocusGained,
    FocusLost,

    /// An unknown sequence of bytes (probably for a key combination that we don't care about).
    Unknown(Vec<u8>),
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// SIGWINCH handler
////////////////////////////////////////////////////////////////////////////////////////////////////

static GOT_SIGWINCH: AtomicBool = AtomicBool::new(false);

extern "C" fn sigwinch_handler(_: libc::c_int) {
    GOT_SIGWINCH.store(true, Ordering::Relaxed);
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Byte sequences of key pressed we want to capture
// (TODO: We only support xterm for now)
////////////////////////////////////////////////////////////////////////////////////////////////////

static XTERM_ALT_ARROW_DOWN: [u8; 6] = [27, 91, 49, 59, 51, 66];
static XTERM_ALT_ARROW_LEFT: [u8; 6] = [27, 91, 49, 59, 51, 68];
static XTERM_ALT_ARROW_RIGHT: [u8; 6] = [27, 91, 49, 59, 51, 67];
static XTERM_ALT_ARROW_UP: [u8; 6] = [27, 91, 49, 59, 51, 65];
static XTERM_ARROW_DOWN: [u8; 3] = [27, 91, 66];
static XTERM_ARROW_DOWN_2: [u8; 3] = [27, 79, 66];
static XTERM_ARROW_LEFT: [u8; 3] = [27, 91, 68];
static XTERM_ARROW_LEFT_2: [u8; 3] = [27, 79, 68];
static XTERM_ARROW_RIGHT: [u8; 3] = [27, 91, 67];
static XTERM_ARROW_RIGHT_2: [u8; 3] = [27, 79, 67];
static XTERM_ARROW_UP: [u8; 3] = [27, 91, 65];
static XTERM_ARROW_UP_2: [u8; 3] = [27, 79, 65];
static XTERM_CTRL_ARROW_DOWN: [u8; 6] = [27, 91, 49, 59, 53, 66];
static XTERM_CTRL_ARROW_LEFT: [u8; 6] = [27, 91, 49, 59, 53, 68];
static XTERM_CTRL_ARROW_RIGHT: [u8; 6] = [27, 91, 49, 59, 53, 67];
static XTERM_CTRL_ARROW_UP: [u8; 6] = [27, 91, 49, 59, 53, 65];
static XTERM_DEL: [u8; 4] = [27, 91, 51, 126];
static XTERM_PAGE_DOWN: [u8; 4] = [27, 91, 54, 126];
static XTERM_PAGE_UP: [u8; 4] = [27, 91, 53, 126];
static XTERM_SHIFT_UP: [u8; 6] = [27, 91, 49, 59, 50, 65];
static XTERM_SHIFT_DOWN: [u8; 6] = [27, 91, 49, 59, 50, 66];
static XTERM_FOCUS_GAINED: [u8; 3] = [27, 91, 73];
static XTERM_FOCUS_LOST: [u8; 3] = [27, 91, 79];
// FIXME: For some reason term_input test program gets first two of these bytes while tiny gets the
// latter two. Tried to debug this a little bit by changing termattrs but no luck...
static XTERM_HOME: [u8; 3] = [27, 91, 72];
static XTERM_END: [u8; 3] = [27, 91, 70];
static XTERM_HOME_2: [u8; 3] = [27, 79, 72];
static XTERM_END_2: [u8; 3] = [27, 79, 70];

static XTERM_KEY_SEQS: [(&[u8], Event); 27] = [
    (
        &XTERM_ALT_ARROW_DOWN,
        Event::Key(Key::AltArrow(Arrow::Down)),
    ),
    (
        &XTERM_ALT_ARROW_LEFT,
        Event::Key(Key::AltArrow(Arrow::Left)),
    ),
    (
        &XTERM_ALT_ARROW_RIGHT,
        Event::Key(Key::AltArrow(Arrow::Right)),
    ),
    (&XTERM_ALT_ARROW_UP, Event::Key(Key::AltArrow(Arrow::Up))),
    (&XTERM_ARROW_DOWN, Event::Key(Key::Arrow(Arrow::Down))),
    (&XTERM_ARROW_DOWN_2, Event::Key(Key::Arrow(Arrow::Down))),
    (&XTERM_ARROW_LEFT, Event::Key(Key::Arrow(Arrow::Left))),
    (&XTERM_ARROW_LEFT_2, Event::Key(Key::Arrow(Arrow::Left))),
    (&XTERM_ARROW_RIGHT, Event::Key(Key::Arrow(Arrow::Right))),
    (&XTERM_ARROW_RIGHT_2, Event::Key(Key::Arrow(Arrow::Right))),
    (&XTERM_ARROW_UP, Event::Key(Key::Arrow(Arrow::Up))),
    (&XTERM_ARROW_UP_2, Event::Key(Key::Arrow(Arrow::Up))),
    (
        &XTERM_CTRL_ARROW_DOWN,
        Event::Key(Key::CtrlArrow(Arrow::Down)),
    ),
    (
        &XTERM_CTRL_ARROW_LEFT,
        Event::Key(Key::CtrlArrow(Arrow::Left)),
    ),
    (
        &XTERM_CTRL_ARROW_RIGHT,
        Event::Key(Key::CtrlArrow(Arrow::Right)),
    ),
    (&XTERM_CTRL_ARROW_UP, Event::Key(Key::CtrlArrow(Arrow::Up))),
    (&XTERM_DEL, Event::Key(Key::Del)),
    (&XTERM_PAGE_DOWN, Event::Key(Key::PageDown)),
    (&XTERM_PAGE_UP, Event::Key(Key::PageUp)),
    (&XTERM_SHIFT_UP, Event::Key(Key::ShiftUp)),
    (&XTERM_SHIFT_DOWN, Event::Key(Key::ShiftDown)),
    (&XTERM_HOME, Event::Key(Key::Home)),
    (&XTERM_END, Event::Key(Key::End)),
    (&XTERM_HOME_2, Event::Key(Key::Home)),
    (&XTERM_END_2, Event::Key(Key::End)),
    (&XTERM_FOCUS_GAINED, Event::FocusGained),
    (&XTERM_FOCUS_LOST, Event::FocusLost),
];

// Make sure not to use 27 (ESC) because it's used as a prefix in many combinations.
static XTERM_SINGLE_BYTES: [(u8, Event); 13] = [
    (9, Event::Key(Key::Tab)),
    (127, Event::Key(Key::Backspace)),
    (1, Event::Key(Key::Ctrl('a'))),
    (5, Event::Key(Key::Ctrl('e'))),
    (23, Event::Key(Key::Ctrl('w'))),
    (11, Event::Key(Key::Ctrl('k'))),
    (4, Event::Key(Key::Ctrl('d'))),
    (3, Event::Key(Key::Ctrl('c'))),
    (17, Event::Key(Key::Ctrl('q'))),
    (16, Event::Key(Key::Ctrl('p'))),
    (14, Event::Key(Key::Ctrl('n'))),
    (21, Event::Key(Key::Ctrl('u'))),
    (24, Event::Key(Key::Ctrl('x'))),
];

////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct Input {
    /// Queue of events waiting to be polled.
    evs: VecDeque<Event>,

    /// Used when reading from stdin.
    buf: Vec<u8>,

    stdin: tokio_net::util::PollEvented<mio::unix::EventedFd<'static>>,
}

impl Input {
    pub fn new() -> Input {
        unsafe {
            // ignore the existing handler
            sigaction(
                Signal::SIGWINCH,
                &SigAction::new(
                    SigHandler::Handler(sigwinch_handler),
                    signal::SA_RESTART,
                    SigSet::empty(),
                ),
            )
            .unwrap();
        }

        Input {
            evs: VecDeque::new(),
            buf: Vec::with_capacity(100),
            stdin: tokio_net::util::PollEvented::new(mio::unix::EventedFd(&libc::STDIN_FILENO)),
        }
    }
}

impl Stream for Input {
    type Item = std::io::Result<Event>;

    fn poll_next(
        mut self: Pin<&mut Input>,
        cx: &mut Context,
    ) -> Poll<Option<std::io::Result<Event>>> {
        // Yield any resize events
        if GOT_SIGWINCH.swap(false, Ordering::Relaxed) {
            return Poll::Ready(Some(Ok(Event::Resize)));
        }

        let self_: &mut Input = &mut *self;

        // Try to parse any bytes in the input buffer from the last poll
        {
            let mut buf_slice: &[u8] = &self_.buf;

            while !buf_slice.is_empty() {
                // Special treatment for 127 (backspace) and 13 ('\r')
                let fst = buf_slice[0];
                let read_fn = if (fst < 32 && fst != 13) || fst == 127 {
                    read_key_comb
                } else {
                    read_chars
                };

                match read_fn(buf_slice, &mut self_.evs) {
                    Some(buf_slice_) => {
                        buf_slice = buf_slice_;
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
        match self_.stdin.poll_read_ready(cx, mio::Ready::readable()) {
            Poll::Ready(Ok(_)) => {
                if read_stdin(&mut self_.buf) {
                    Input::poll_next(self, cx)
                } else {
                    self_
                        .stdin
                        .clear_read_ready(cx, mio::Ready::readable())
                        .unwrap();
                    Poll::Pending
                }
            }
            Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
            Poll::Pending => Poll::Pending,
        }
    }
}

fn read_chars<'a>(mut buf_slice: &'a [u8], evs: &mut VecDeque<Event>) -> Option<&'a [u8]> {
    debug_assert!(!buf_slice.is_empty());

    // Use a fast path for the common case: Single utf-8 character.
    // (TODO: What about other encodings?)

    utf8_char_len(buf_slice[0]).map(|char_len| {
        if char_len as usize == buf_slice.len() {
            // fast path: single character
            evs.push_back(Event::Key(Key::Char(get_utf8_char(buf_slice, char_len))));
            &buf_slice[char_len as usize..]
        } else {
            // probably a paste: allocate a string and collect chars
            let mut string = String::with_capacity(10);
            loop {
                if buf_slice.is_empty() {
                    break;
                }
                match utf8_char_len(buf_slice[0]) {
                    Some(char_len) => {
                        string.push(get_utf8_char(buf_slice, char_len));
                        buf_slice = &buf_slice[char_len as usize..];
                    }
                    None => {
                        break;
                    }
                }
            }
            evs.push_back(Event::String(string));
            buf_slice
        }
    })
}

fn read_key_comb<'a>(buf_slice: &'a [u8], evs: &mut VecDeque<Event>) -> Option<&'a [u8]> {
    debug_assert!(!buf_slice.is_empty());

    // TODO: This is not working, see https://github.com/rust-lang/rust/issues/36401
    for &(byte, ref ev) in XTERM_SINGLE_BYTES.iter() {
        if byte == buf_slice[0] {
            evs.push_back(ev.clone());
            return Some(&buf_slice[1..]);
        }
    }

    for &(byte_seq, ref ev) in XTERM_KEY_SEQS.iter() {
        if buf_slice.starts_with(byte_seq) {
            evs.push_back(ev.clone());
            return Some(&buf_slice[byte_seq.len()..]);
        }
    }

    if buf_slice[0] == 27 {
        // 27 not followed by anything is an actual ESC
        if buf_slice.len() == 1 {
            evs.push_back(Event::Key(Key::Esc));
            return Some(&buf_slice[1..]);
        }
        // otherwise it's probably alt + key
        else {
            debug_assert!(buf_slice.len() >= 2);
            return utf8_char_len(buf_slice[1]).map(|char_len| {
                evs.push_back(Event::Key(Key::AltChar(get_utf8_char(
                    &buf_slice[1..],
                    char_len,
                ))));
                &buf_slice[char_len as usize + 1..]
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

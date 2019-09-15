pub(crate) struct InsertIterator<'iter, A: 'iter> {
    insert_point: usize,
    current_idx: usize,
    iter_orig: &'iter mut dyn Iterator<Item = A>,
    iter_insert: &'iter mut dyn Iterator<Item = A>,
}

impl<'iter, A> Iterator for InsertIterator<'iter, A> {
    type Item = A;

    fn next(&mut self) -> Option<A> {
        if self.current_idx >= self.insert_point {
            if let Some(a) = self.iter_insert.next() {
                Some(a)
            } else {
                self.iter_orig.next()
            }
        } else {
            self.current_idx += 1;
            self.iter_orig.next()
        }
    }
}

pub(crate) fn insert_iter<'iter, A>(
    iter_orig: &'iter mut dyn Iterator<Item = A>,
    iter_insert: &'iter mut dyn Iterator<Item = A>,
    insert_point: usize,
) -> InsertIterator<'iter, A> {
    InsertIterator {
        insert_point,
        current_idx: 0,
        iter_orig,
        iter_insert,
    }
}

////////////////////////////////////////////////////////////////////////////////

pub(crate) fn is_nick_char(c: char) -> bool {
    // from RFC 2812:
    //
    // nickname   =  ( letter / special ) *8( letter / digit / special / "-" )
    // special    =  %x5B-60 / %x7B-7D
    //                  ; "[", "]", "\", "`", "_", "^", "{", "|", "}"
    //
    // we use a simpler check here (allows strictly more nicks)

    c.is_alphanumeric()
        || (c as i32 >= 0x5B && c as i32 <= 0x60)
        || (c as i32 >= 0x7B && c as i32 <= 0x7D)
        || c == '-' // not valid according to RFC 2812 but servers accept it and I've seen nicks with
                    // this char in the wild
}

////////////////////////////////////////////////////////////////////////////////

use std::{iter::Peekable, str::Chars};

/// Parse at least one, at most two digits. Does not consume the iterator when
/// result is `None`.
fn parse_color_code(chars: &mut Peekable<Chars>) -> Option<u8> {
    fn to_dec(ch: char) -> Option<u8> {
        ch.to_digit(10).map(|c| c as u8)
    }

    let c1_char = *chars.peek()?;
    let c1_digit = match to_dec(c1_char) {
        None => {
            return None;
        }
        Some(c1_digit) => {
            chars.next();
            c1_digit
        }
    };

    match chars.peek().cloned() {
        None => Some(c1_digit),
        Some(c2) => match to_dec(c2) {
            None => Some(c1_digit),
            Some(c2_digit) => {
                chars.next();
                Some(c1_digit * 10 + c2_digit)
            }
        },
    }
}

////////////////////////////////////////////////////////////////////////////////

/// Translate IRC color codes using the callback, and remove ASCII control chars from the input.
pub(crate) fn translate_irc_control_chars(
    str: &str,
    push_color: fn(ret: &mut String, fg: u8, bg: Option<u8>),
) -> String {
    let mut ret = String::with_capacity(str.len());
    let mut iter = str.chars().peekable();

    while let Some(char) = iter.next() {
        if char == '\x03' {
            match parse_color_code(&mut iter) {
                None => {
                    // just skip the control char
                }
                Some(fg) => {
                    if let Some(char) = iter.peek().cloned() {
                        if char == ',' {
                            iter.next(); // consume ','
                            match parse_color_code(&mut iter) {
                                None => {
                                    // comma was not part of the color code,
                                    // add it to the new string
                                    push_color(&mut ret, fg, None);
                                    ret.push(char);
                                }
                                Some(bg) => {
                                    push_color(&mut ret, fg, Some(bg));
                                }
                            }
                        } else {
                            push_color(&mut ret, fg, None);
                        }
                    } else {
                        push_color(&mut ret, fg, None);
                    }
                }
            }
        } else if !char.is_ascii_control() {
            ret.push(char);
        }
    }

    ret
}

/// Like `translate_irc_control_chars`, but skips color codes.
pub(crate) fn remove_irc_control_chars(str: &str) -> String {
    fn push_color(_ret: &mut String, _fg: u8, _bg: Option<u8>) {}
    translate_irc_control_chars(str, push_color)
}

#[test]
fn test_translate_irc_control_chars() {
    assert_eq!(
        remove_irc_control_chars("  Le Voyageur imprudent  "),
        "  Le Voyageur imprudent  "
    );
    assert_eq!(remove_irc_control_chars("\x0301,02foo"), "foo");
    assert_eq!(remove_irc_control_chars("\x0301,2foo"), "foo");
    assert_eq!(remove_irc_control_chars("\x031,2foo"), "foo");
    assert_eq!(remove_irc_control_chars("\x031,foo"), ",foo");
    assert_eq!(remove_irc_control_chars("\x03,foo"), ",foo");
}

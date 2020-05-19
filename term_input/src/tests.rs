// Test cases from crossterm

use crate::*;

use std::collections::VecDeque;

// NB. crossterm expects to 1Bs for ESC but I think that's not right?
#[test]
fn test_esc_key() {
    let buf = "\x1B".as_bytes();
    let mut evs = VecDeque::new();
    parse_key_comb_(&buf, &mut evs);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs.pop_front().unwrap(), Event::Key(Key::Esc));
}

#[test]
fn test_alt_key() {
    let buf = "\x1Bc".as_bytes();
    let mut evs = VecDeque::new();
    parse_key_comb_(&buf, &mut evs);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs.pop_front().unwrap(), Event::Key(Key::AltChar('c')));
}

#[test]
fn test_left_key() {
    let buf = "\x1B[D".as_bytes();
    let mut evs = VecDeque::new();
    parse_key_comb_(&buf, &mut evs);
    assert_eq!(evs.len(), 1);
    assert_eq!(
        evs.pop_front().unwrap(),
        Event::Key(Key::Arrow(Arrow::Left))
    );
}

// \x1B[2D = shift-left

#[test]
fn test_del_key() {
    let buf = "\x1B[3~".as_bytes();
    let mut evs = VecDeque::new();
    parse_key_comb_(&buf, &mut evs);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs.pop_front().unwrap(), Event::Key(Key::Del));
}

#[test]
fn test_utf8_char() {
    let buf = "Ž".as_bytes();
    let mut evs = VecDeque::new();
    parse_chars_(&buf, &mut evs);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs.pop_front().unwrap(), Event::Key(Key::Char('Ž')));
}

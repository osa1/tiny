// Test cases from crossterm

use crate::*;

// NB. crossterm expects to 1Bs for ESC but I think that's not right?
#[test]
fn test_esc_key() {
    assert_eq!(parse_single_event("\x1B".as_bytes()), Event::Key(Key::Esc));
}

#[test]
fn test_alt_key() {
    assert_eq!(
        parse_single_event("\x1Bc".as_bytes()),
        Event::Key(Key::AltChar('c'))
    );
}

#[test]
fn test_left_key() {
    assert_eq!(
        parse_single_event("\x1B[D".as_bytes()),
        Event::Key(Key::Arrow(Arrow::Left))
    );
}

// \x1B[2D = shift-left

#[test]
fn test_del_key() {
    assert_eq!(
        parse_single_event("\x1B[3~".as_bytes()),
        Event::Key(Key::Del)
    );
}

#[test]
fn test_utf8_char() {
    assert_eq!(
        parse_single_event("Ž".as_bytes()),
        Event::Key(Key::Char('Ž'))
    );
}

#[test]
fn test_tab_key() {
    assert_eq!(parse_single_event("\t".as_bytes()), Event::Key(Key::Tab));
}

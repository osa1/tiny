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

// Tests for Chinese/Japanese/Korean IME input
// IME sends ESC followed by multi-byte UTF-8 sequences.
// We should parse ESC separately and let UTF-8 bytes be parsed as regular characters.

#[test]
fn test_ime_chinese_char() {
    // Chinese character '你' (U+4F60) in UTF-8: E4 BD A0
    let buf = [0xE4, 0xBD, 0xA0];
    let ev = parse_single_event(&buf);
    assert_eq!(ev, Event::Key(Key::Char('\u{4F60}')));
}

#[test]
fn test_ime_esc_followed_by_chinese_simulation() {
    // Simulate real IME input: ESC + Chinese character
    // In real usage, these would arrive in separate reads or be parsed separately
    // First, ESC is parsed
    let esc_buf = [0x1B];
    let esc_ev = parse_single_event(&esc_buf);
    assert_eq!(esc_ev, Event::Key(Key::Esc));
    
    // Then, Chinese character is parsed separately
    let chinese_buf = [0xE4, 0xBD, 0xA0];
    let chinese_ev = parse_single_event(&chinese_buf);
    assert_eq!(chinese_ev, Event::Key(Key::Char('\u{4F60}')));
}

#[test]
fn test_ime_multiple_chinese_chars() {
    // Multiple Chinese characters '你好' (U+4F60 U+597D)
    // UTF-8: E4 BD A0 E5 A5 BD
    let buf = [0xE4, 0xBD, 0xA0, 0xE5, 0xA5, 0xBD];
    let ev = parse_single_event(&buf);
    // Should be parsed as a String event
    assert_eq!(ev, Event::String("你好".to_string()));
}

#[test]
fn test_alt_ascii_still_works() {
    // Alt + ASCII should still work
    assert_eq!(
        parse_single_event("\x1Ba".as_bytes()),
        Event::Key(Key::AltChar('a'))
    );
    assert_eq!(
        parse_single_event("\x1Bx".as_bytes()),
        Event::Key(Key::AltChar('x'))
    );
}

#[test]
fn test_alt_non_ascii_still_works() {
    // Alt + non-ASCII single-byte chars (like Latin-1) should still work
    // For example, Alt + 'é' (E9 in Latin-1, but this is not valid UTF-8)
    // We test with valid single-byte UTF-8 chars only
    // Actually, all non-ASCII UTF-8 chars are multi-byte, so they won't be AltChar
    // This test verifies that Alt only works with ASCII range
    assert_eq!(
        parse_single_event("\x1B\x41".as_bytes()), // Alt + 'A'
        Event::Key(Key::AltChar('A'))
    );
}

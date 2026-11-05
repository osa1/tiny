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

fn feed(buf: &mut Vec<u8>, bytes: &[u8]) -> Vec<Event> {
    let mut events = VecDeque::new();
    buf.extend_from_slice(bytes);
    parse_input(buf, &mut events);
    events.into_iter().collect()
}

#[test]
fn japanese_and_mixed_paste_are_delivered_without_changes() {
    let mut buf = Vec::new();
    assert_eq!(
        feed(&mut buf, "日".as_bytes()),
        vec![Event::Key(Key::Char('日'))]
    );
    assert_eq!(
        feed(&mut buf, "A日本語 한글B\r\n".as_bytes()),
        vec![Event::String("A日本語 한글B\r\n".into())]
    );
    assert!(buf.is_empty());
}

#[test]
fn incomplete_scalars_wait_at_every_byte_boundary() {
    for ch in ['é', '日', '🦀'] {
        let encoded = ch.to_string();
        let bytes = encoded.as_bytes();
        for split in 1..bytes.len() {
            let mut buf = Vec::new();
            assert!(
                feed(&mut buf, &bytes[..split]).is_empty(),
                "{ch:?} at {split}"
            );
            assert_eq!(buf, bytes[..split]);
            assert_eq!(
                feed(&mut buf, &bytes[split..]),
                vec![Event::Key(Key::Char(ch))]
            );
            assert!(buf.is_empty());
        }
        let mut buf = Vec::new();
        let mut events = Vec::new();
        for byte in bytes {
            events.extend(feed(&mut buf, &[*byte]));
        }
        assert_eq!(events, vec![Event::Key(Key::Char(ch))]);
        assert!(buf.is_empty());
    }
}

#[test]
fn mixed_paste_survives_every_byte_split() {
    let text = "A日本B한글か\u{3099}";
    let bytes = text.as_bytes();
    for split in 1..bytes.len() {
        let mut buf = Vec::new();
        let mut events = feed(&mut buf, &bytes[..split]);
        events.extend(feed(&mut buf, &bytes[split..]));
        let mut result = String::new();
        for event in events {
            match event {
                Event::Key(Key::Char(ch)) => result.push(ch),
                Event::String(part) => result.push_str(&part),
                other => panic!("unexpected event at {split}: {other:?}"),
            }
        }
        assert_eq!(result, text, "split at {split}");
        assert!(buf.is_empty());
    }
}

#[test]
fn complete_prefix_is_delivered_before_the_incomplete_suffix() {
    let mut buf = Vec::new();
    let bytes = "ab日本".as_bytes();
    let split = bytes.len() - 1;
    assert_eq!(
        feed(&mut buf, &bytes[..split]),
        vec![Event::String("ab日".into())]
    );
    assert_eq!(buf, "本".as_bytes()[..2]);
    assert_eq!(
        feed(&mut buf, &bytes[split..]),
        vec![Event::Key(Key::Char('本'))]
    );
    assert!(buf.is_empty());
}

#[test]
fn incomplete_utf8_keeps_multiline_paste_from_sending_enter() {
    for text in ["\r日", "line\r日", "line\n日", "\r\n日"] {
        let bytes = text.as_bytes();
        for split in bytes.len() - 2..bytes.len() {
            let mut buf = Vec::new();
            assert!(
                feed(&mut buf, &bytes[..split]).is_empty(),
                "{text:?} at {split}"
            );
            assert_eq!(
                feed(&mut buf, &bytes[split..]),
                vec![Event::String(text.to_owned())]
            );
            assert!(buf.is_empty());
        }
    }

    let mut buf = Vec::new();
    assert_eq!(
        feed(&mut buf, &[b'\r', 0xff]),
        vec![Event::String("\r".into()), Event::Unknown(vec![0xff])]
    );
    assert_eq!(feed(&mut buf, b"\r"), vec![Event::Key(Key::Char('\r'))]);
    assert_eq!(feed(&mut buf, b"x\t"), vec![Event::String("x\t".into())]);
}

#[test]
fn alt_waits_for_a_split_utf8_character() {
    let mut sequence = vec![27];
    sequence.extend_from_slice("日".as_bytes());
    for split in 2..sequence.len() {
        let mut buf = Vec::new();
        assert!(feed(&mut buf, &sequence[..split]).is_empty());
        assert_eq!(buf, sequence[..split]);
        assert_eq!(
            feed(&mut buf, &sequence[split..]),
            vec![Event::Key(Key::AltChar('日'))]
        );
        assert!(buf.is_empty());
    }
}

#[test]
fn invalid_utf8_is_reported_without_losing_following_text() {
    let mut buf = Vec::new();
    assert_eq!(
        feed(&mut buf, &[0xe2, b'(', 0xa1, b'Z']),
        vec![
            Event::Unknown(vec![0xe2]),
            Event::Key(Key::Char('(')),
            Event::Unknown(vec![0xa1]),
            Event::Key(Key::Char('Z')),
        ]
    );
    assert_eq!(
        feed(&mut buf, &[27, 0xe6, b'X']),
        vec![Event::Unknown(vec![27, 0xe6]), Event::Key(Key::Char('X'))]
    );
    for bytes in [
        &[0xc0, 0xaf][..],
        &[0xed, 0xa0, 0x80][..],
        &[0xf4, 0x90, 0x80, 0x80][..],
    ] {
        let events = feed(&mut buf, bytes);
        assert!(
            events
                .iter()
                .all(|event| matches!(event, Event::Unknown(_)))
        );
        assert!(buf.is_empty());
    }
}

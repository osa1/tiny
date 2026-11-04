use libtiny_common::{ChanNameRef, MsgTarget};
use term_input::{Arrow, Event, Key};

use crate::msg_area::Layout;
use crate::tui::{TUI, TUIRet};

#[test]
fn aligned_messages_wrap_using_display_columns() {
    let mut tui = TUI::new_test(20, 6);
    tui.set_layout(Layout::Aligned { max_nick_len: 6 });
    let serv = "s";
    let chan = ChanNameRef::new("#日本");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "test");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();
    let target = MsgTarget::Chan { serv, chan };
    tui.add_privmsg(
        "日本語X",
        "A中B 한글",
        time::at_utc(time::Timespec::new(0, 0)),
        &target,
        false,
        false,
    );
    tui.draw();
    let buffer = tui.get_front_buffer();
    let rows: Vec<Vec<char>> = buffer
        .cells
        .chunks(20)
        .map(|row| row.iter().map(|cell| cell.ch).collect())
        .collect();
    let first = rows
        .iter()
        .position(|row| row[0..5] == ['0', '0', ':', '0', '0'])
        .unwrap();
    assert_eq!(&rows[first][6..12], &[' ', '日', ' ', '本', ' ', '…']);
    assert_eq!(&rows[first][14..19], &['A', '中', ' ', 'B', ' ']);
    assert_eq!(&rows[first + 1][14..18], &['한', ' ', '글', ' ']);
}

#[test]
fn unicode_editing_resize_history_and_send_preserve_scalars() {
    let mut tui = TUI::new_test(42, 8);
    tui.new_server_tab("s", None);
    tui.set_nick("s", "test");
    tui.next_tab();
    tui.handle_input_event(Event::String("A日本語B".into()), &mut None);
    tui.draw();
    let cells = tui.get_front_buffer().cells;
    let row: Vec<char> = cells[6 * 42..7 * 42].iter().map(|cell| cell.ch).collect();
    assert_eq!(&row[6..14], &['A', '日', ' ', '本', ' ', '語', ' ', 'B']);

    tui.handle_input_event(Event::Key(Key::Arrow(Arrow::Left)), &mut None);
    tui.handle_input_event(Event::Key(Key::Backspace), &mut None);
    tui.handle_input_event(Event::String("か\u{3099}".into()), &mut None);
    tui.set_size(12, 8);
    tui.draw();
    let sent = tui.handle_input_event(Event::Key(Key::Char('\r')), &mut None);
    match sent {
        Some(TUIRet::Input { msg, .. }) => {
            assert_eq!(msg.iter().collect::<String>(), "A日本か\u{3099}B")
        }
        other => panic!("unexpected send result: {other:?}"),
    }

    tui.handle_input_event(Event::Key(Key::Arrow(Arrow::Up)), &mut None);
    tui.draw();
    let sent = tui.handle_input_event(Event::Key(Key::Char('\r')), &mut None);
    match sent {
        Some(TUIRet::Input { msg, .. }) => {
            assert_eq!(msg.iter().collect::<String>(), "A日本か\u{3099}B")
        }
        other => panic!("unexpected history result: {other:?}"),
    }
}

use std::panic::Location;

use libtiny_common::{ChanNameRef, MsgTarget};

use crate::msg_area::Layout;
use crate::test_utils::expect_screen;
use crate::tui::TUI;

#[test]
fn test_join_part_overflow() {
    let mut tui = TUI::new_test(21, 4);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan { serv, chan };
    let ts = time::at_utc(time::Timespec::new(0, 0));
    tui.add_nick("123456", Some(ts), &target);
    tui.add_nick("abcdef", Some(ts), &target);
    tui.add_nick("hijklm", Some(ts), &target);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 +123456 +abcdef|
         |+hijklm              |
         |osa1:                |
         |< #chan              |";

    expect_screen(screen, &tui.get_front_buffer(), 21, 4, Location::caller());
}

#[test]
fn test_alignment_long_string() {
    let mut tui = TUI::new_test(40, 5);
    tui.set_layout(Layout::Aligned { max_nick_len: 12 });
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan { serv, chan };
    let ts = time::at_utc(time::Timespec::new(0, 0));
    tui.add_privmsg(
        "osa1",
        "123456789012345678901234567890",
        ts,
        &target,
        false,
        false,
    );
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                                        |
         |00:00         osa1: 12345678901234567890|
         |                    1234567890          |
         |osa1:                                   |
         |mentions irc.server_1.org #chan         |";

    expect_screen(screen, &tui.get_front_buffer(), 40, 5, Location::caller());
}

#[test]
fn test_mnemonic_generation() {
    let mut tui = TUI::new_test(10, 10);
    tui.new_chan_tab("s1", ChanNameRef::new("#ab"));
    tui.new_chan_tab("s2", ChanNameRef::new("#ab"));
    tui.new_chan_tab("s3", ChanNameRef::new("#ab"));
    tui.new_chan_tab("s4", ChanNameRef::new("#ab"));
    let tabs = tui.get_tabs();
    assert_eq!(tabs.len(), 9); // mentions, 4 servers, 4 channels
    assert_eq!(tabs[2].switch, Some('a'));
    assert_eq!(tabs[4].switch, Some('b'));
    assert_eq!(tabs[6].switch, Some('a'));
    assert_eq!(tabs[8].switch, Some('b'));
}

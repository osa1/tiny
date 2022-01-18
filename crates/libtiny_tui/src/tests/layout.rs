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
    let (serv_id, _) = tui.new_server_tab(serv, None);
    tui.set_nick(serv_id, serv, "osa1");
    tui.new_chan_tab(serv_id, serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan {
        serv_id,
        serv,
        chan,
    };
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
    let (serv_id, _) = tui.new_server_tab(serv, None);
    tui.set_nick(serv_id, serv, "osa1");
    tui.new_chan_tab(serv_id, serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan {
        serv_id,
        serv,
        chan,
    };
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
    let (s1, _) = tui.new_server_tab("s1", None);
    tui.new_chan_tab(s1, "s1", ChanNameRef::new("#ab"));
    let (s2, _) = tui.new_server_tab("s2", None);
    tui.new_chan_tab(s2, "s2", ChanNameRef::new("#ab"));
    let (s3, _) = tui.new_server_tab("s3", None);
    tui.new_chan_tab(s3, "s3", ChanNameRef::new("#ab"));
    let (s4, _) = tui.new_server_tab("s4", None);
    tui.new_chan_tab(s4, "s4", ChanNameRef::new("#ab"));
    let tabs = tui.get_tabs();
    assert_eq!(tabs.len(), 9); // mentions, 4 servers, 4 channels
    assert_eq!(tabs[2].switch, Some('a'));
    assert_eq!(tabs[4].switch, Some('b'));
    assert_eq!(tabs[6].switch, Some('a'));
    assert_eq!(tabs[8].switch, Some('b'));
}

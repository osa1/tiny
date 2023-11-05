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
    tui.next_tab(); // mentions -> server
    tui.next_tab(); // server -> channel

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
    tui.next_tab(); // mentions -> server
    tui.next_tab(); // server -> channel

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

#[test]
fn test_activity_timestamp() {
    // Test all combinations of
    //
    // 1.1 Message followed by activity
    // 1.2 Message followed by message
    // 1.3 Activity followed by message
    // 1.4 Activity followed by activity
    //
    // and
    //
    // 2.1 Same timestamps
    // 2.2 Different timestamps
    //
    // In total 8 scenarios.

    fn setup_tui() -> (TUI, MsgTarget<'static>) {
        let mut tui = TUI::new_test(40, 5);
        tui.set_layout(Layout::Aligned { max_nick_len: 12 });
        let serv = "irc.server_1.org";
        let chan = ChanNameRef::new("#chan");
        tui.new_server_tab(serv, None);
        tui.set_nick(serv, "osa1");
        tui.new_chan_tab(serv, chan);
        tui.next_tab(); // mentions -> server
        tui.next_tab(); // server -> channel
        tui.draw();

        #[rustfmt::skip]
        let screen =
           "|                                        |
            |                                        |
            |                                        |
            |osa1:                                   |
            |mentions irc.server_1.org #chan         |";

        expect_screen(screen, &tui.get_front_buffer(), 40, 5, Location::caller());

        (tui, MsgTarget::Chan { serv, chan })
    }

    // 1.1 - 2.1
    {
        let (mut tui, target) = setup_tui();
        let ts = time::at_utc(time::Timespec::new(0, 0));
        tui.add_privmsg(
            "osa1", "hi", ts, &target, false, // highlight
            false, // is_action
        );
        tui.add_nick("test", Some(ts), &target);
        tui.draw();

        #[rustfmt::skip]
        let screen =
            "|                                        |
             |00:00         osa1: hi                  |
             |                    +test               |
             |osa1:                                   |
             |mentions irc.server_1.org #chan         |";

        expect_screen(screen, &tui.get_front_buffer(), 40, 5, Location::caller());
    }

    // 1.1 - 2.2
    {
        let (mut tui, target) = setup_tui();
        let ts = time::at_utc(time::Timespec::new(0, 0));
        tui.add_privmsg(
            "osa1", "hi", ts, &target, false, // highlight
            false, // is_action
        );
        let ts = time::at_utc(time::Timespec::new(60, 0));
        tui.add_nick("test", Some(ts), &target);
        tui.draw();

        #[rustfmt::skip]
        let screen =
            "|                                        |
             |00:00         osa1: hi                  |
             |00:01               +test               |
             |osa1:                                   |
             |mentions irc.server_1.org #chan         |";

        expect_screen(screen, &tui.get_front_buffer(), 40, 5, Location::caller());
    }

    // 1.2 - 2.1
    {
        let (mut tui, target) = setup_tui();
        let ts = time::at_utc(time::Timespec::new(0, 0));
        tui.add_privmsg(
            "osa1", "hi", ts, &target, false, // highlight
            false, // is_action
        );
        tui.add_privmsg("osa1", "test", ts, &target, false, false);
        tui.draw();

        #[rustfmt::skip]
        let screen =
            "|                                        |
             |00:00         osa1: hi                  |
             |              osa1: test                |
             |osa1:                                   |
             |mentions irc.server_1.org #chan         |";

        expect_screen(screen, &tui.get_front_buffer(), 40, 5, Location::caller());
    }

    // 1.2 - 2.2
    {
        let (mut tui, target) = setup_tui();
        let ts = time::at_utc(time::Timespec::new(0, 0));
        tui.add_privmsg(
            "osa1", "hi", ts, &target, false, // highlight
            false, // is_action
        );
        let ts = time::at_utc(time::Timespec::new(60, 0));
        tui.add_privmsg("osa1", "test", ts, &target, false, false);
        tui.draw();

        #[rustfmt::skip]
        let screen =
            "|                                        |
             |00:00         osa1: hi                  |
             |00:01         osa1: test                |
             |osa1:                                   |
             |mentions irc.server_1.org #chan         |";

        expect_screen(screen, &tui.get_front_buffer(), 40, 5, Location::caller());
    }

    // 1.3 - 2.1
    {
        let (mut tui, target) = setup_tui();
        let ts = time::at_utc(time::Timespec::new(0, 0));
        tui.add_nick("test", Some(ts), &target);
        tui.add_privmsg(
            "osa1", "hi", ts, &target, false, // highlight
            false, // is_action
        );
        tui.draw();

        #[rustfmt::skip]
        let screen =
            "|                                        |
             |00:00               +test               |
             |              osa1: hi                  |
             |osa1:                                   |
             |mentions irc.server_1.org #chan         |";

        expect_screen(screen, &tui.get_front_buffer(), 40, 5, Location::caller());
    }

    // 1.3 - 2.2
    {
        let (mut tui, target) = setup_tui();
        let ts = time::at_utc(time::Timespec::new(0, 0));
        tui.add_nick("test", Some(ts), &target);
        let ts = time::at_utc(time::Timespec::new(60, 0));
        tui.add_privmsg(
            "osa1", "hi", ts, &target, false, // highlight
            false, // is_action
        );
        tui.draw();

        #[rustfmt::skip]
        let screen =
            "|                                        |
             |00:00               +test               |
             |00:01         osa1: hi                  |
             |osa1:                                   |
             |mentions irc.server_1.org #chan         |";

        expect_screen(screen, &tui.get_front_buffer(), 40, 5, Location::caller());
    }

    // 1.4 - 2.1
    {
        let (mut tui, target) = setup_tui();
        let ts = time::at_utc(time::Timespec::new(0, 0));
        tui.add_nick("test1", Some(ts), &target);
        tui.add_nick("test2", Some(ts), &target);
        tui.draw();

        #[rustfmt::skip]
        let screen =
            "|                                        |
             |                                        |
             |00:00               +test1 +test2       |
             |osa1:                                   |
             |mentions irc.server_1.org #chan         |";

        expect_screen(screen, &tui.get_front_buffer(), 40, 5, Location::caller());
    }

    // 1.4 - 2.2
    {
        let (mut tui, target) = setup_tui();
        let ts = time::at_utc(time::Timespec::new(0, 0));
        tui.add_nick("test1", Some(ts), &target);
        let ts = time::at_utc(time::Timespec::new(60, 0));
        tui.add_nick("test2", Some(ts), &target);
        tui.draw();

        #[rustfmt::skip]
        let screen =
            "|                                        |
             |00:00               +test1              |
             |00:01               +test2              |
             |osa1:                                   |
             |mentions irc.server_1.org #chan         |";

        expect_screen(screen, &tui.get_front_buffer(), 40, 5, Location::caller());
    }
}

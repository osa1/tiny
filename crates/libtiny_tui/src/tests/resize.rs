use std::fs::File;
use std::io::{BufRead, BufReader};
use std::panic::Location;

use libtiny_common::{ChanNameRef, MsgTarget};
use term_input::{Arrow, Key};

use crate::test_utils::expect_screen;
use crate::tui::TUI;

#[test]
fn test_resize_recalc_scroll() {
    let mut tui = TUI::new_test(15, 5);
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
        "s 1111 1111 1111 1111 1111 1111 1111 1111 1111 1111 1111 1111 1111 1111 1111 1111 e",
        ts,
        &target,
        false,
        false,
    );

    tui.draw();

    // at bottom with no scroll
    #[rustfmt::skip]
    let screen1 =
       "|1111 1111 1111 |
        |1111 1111 1111 |
        |1111 e         |
        |               |
        |< #chan        |";

    expect_screen(screen1, &tui.get_front_buffer(), 15, 5, Location::caller());

    // hit the home key to go to the top of the messages and then resize the screen
    let home = term_input::Event::Key(Key::Home);
    tui.handle_input_event(home, &mut None);
    tui.set_size(16, 7);
    tui.draw();

    // should be at the top of message after resize
    #[rustfmt::skip]
    let screen2 =
       "|00:00 osa1: s   |
        |1111 1111 1111  |
        |1111 1111 1111  |
        |1111 1111 1111  |
        |1111 1111 1111  |
        |                |
        |< #chan         |";

    expect_screen(screen2, &tui.get_front_buffer(), 16, 7, Location::caller());

    // go back to the bottom
    let end = term_input::Event::Key(Key::End);
    tui.handle_input_event(end, &mut None);
    tui.draw();

    // go back to the bottom
    #[rustfmt::skip]
    let screen3 = 
       "|1111 1111 1111  |
        |1111 1111 1111  |
        |1111 1111 1111  |
        |1111 1111 1111  |
        |1111 e          |
        |                |
        |< #chan         |";

    expect_screen(screen3, &tui.get_front_buffer(), 16, 7, Location::caller());
}

#[test]
fn test_resize_scroll_stick_to_top() {
    let mut tui = TUI::new_test(18, 10);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan { serv, chan };
    let ts = time::at_utc(time::Timespec::new(0, 0));

    for i in 0..15 {
        tui.add_privmsg("osa1", &format!("line{}", i), ts, &target, false, false);
    }

    tui.draw();

    #[rustfmt::skip]
    let screen1 =
       "|osa1: line7       |
        |osa1: line8       |
        |osa1: line9       |
        |osa1: line10      |
        |osa1: line11      |
        |osa1: line12      |
        |osa1: line13      |
        |osa1: line14      |
        |                  |
        |< #chan           |";

    expect_screen(screen1, &tui.get_front_buffer(), 18, 10, Location::caller());

    // scroll up two lines, resize to add one extra line and verify that the next line on the bottom shows
    for _ in 0..2 {
        tui.handle_input_event(
            term_input::Event::Key(Key::ShiftArrow(Arrow::Up)),
            &mut None,
        );
    }
    tui.draw();
    tui.set_size(18, 11);
    tui.draw();

    #[rustfmt::skip]
    let screen2 =
       "|osa1: line5       |
        |osa1: line6       |
        |osa1: line7       |
        |osa1: line8       |
        |osa1: line9       |
        |osa1: line10      |
        |osa1: line11      |
        |osa1: line12      |
        |osa1: line13      |
        |                  |
        |< #chan           |";
    expect_screen(screen2, &tui.get_front_buffer(), 18, 11, Location::caller());
}

#[test]
fn test_resize_no_scroll_stay_on_bottom() {
    let mut tui = TUI::new_test(18, 10);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan { serv, chan };
    let ts = time::at_utc(time::Timespec::new(0, 0));

    for i in 0..15 {
        tui.add_privmsg("osa1", &format!("line{}", i), ts, &target, false, false);
    }

    tui.draw();

    #[rustfmt::skip]
    let screen1 =
       "|osa1: line7       |
        |osa1: line8       |
        |osa1: line9       |
        |osa1: line10      |
        |osa1: line11      |
        |osa1: line12      |
        |osa1: line13      |
        |osa1: line14      |
        |                  |
        |< #chan           |";

    expect_screen(screen1, &tui.get_front_buffer(), 18, 10, Location::caller());

    tui.set_size(18, 11);
    tui.draw();

    // shows one extra line on the top of the screen
    #[rustfmt::skip]
    let screen2 =
       "|osa1: line6       |
        |osa1: line7       |
        |osa1: line8       |
        |osa1: line9       |
        |osa1: line10      |
        |osa1: line11      |
        |osa1: line12      |
        |osa1: line13      |
        |osa1: line14      |
        |                  |
        |< #chan           |";
    expect_screen(screen2, &tui.get_front_buffer(), 18, 11, Location::caller());

    // resize back to original screen and verify last line is still on the bottom
    tui.set_size(18, 10);
    tui.draw();
    expect_screen(screen1, &tui.get_front_buffer(), 18, 10, Location::caller());

    tui.add_privmsg("osa1", "line15", ts, &target, false, false);
    tui.set_size(18, 11);
    tui.draw();

    #[rustfmt::skip]
    let screen3 =
       "|osa1: line7       |
        |osa1: line8       |
        |osa1: line9       |
        |osa1: line10      |
        |osa1: line11      |
        |osa1: line12      |
        |osa1: line13      |
        |osa1: line14      |
        |osa1: line15      |
        |                  |
        |< #chan           |";
    expect_screen(screen3, &tui.get_front_buffer(), 18, 11, Location::caller());
}

// Testing for crashes by resizing from 50x50 -> 0x0 -> 50x50
#[test]
fn test_resize() {
    let mut tui = TUI::new_test(80, 50);

    let server = "<server>";
    tui.new_server_tab(server, None);

    let ts = time::empty_tm();
    let target = MsgTarget::CurrentTab;

    let f = File::open("test/lipsum.txt").unwrap();
    let f = BufReader::new(f);
    for line in f.lines() {
        let line = line.unwrap();
        tui.add_msg(&line, ts, &target);
    }

    for i in (0..=50).rev().chain(0..=50) {
        tui.set_size(i, i);
        tui.draw();
    }
}

// https://github.com/osa1/tiny/pull/351#issuecomment-915447462
#[test]
fn test_resize_scroll_resize() {
    let mut tui = TUI::new_test(16, 10);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan { serv, chan };
    let ts = time::at_utc(time::Timespec::new(0, 0));

    for i in 0..15 {
        tui.add_privmsg(
            "osa1",
            &format!("a long line {}", i),
            ts,
            &target,
            false,
            false,
        );
    }

    tui.draw();

    #[rustfmt::skip]
    let screen1 =
       "|osa1: a long    |
        |line 11         |
        |osa1: a long    |
        |line 12         |
        |osa1: a long    |
        |line 13         |
        |osa1: a long    |
        |line 14         |
        |                |
        |< #chan         |";

    expect_screen(screen1, &tui.get_front_buffer(), 16, 10, Location::caller());
    tui.handle_input_event(
        term_input::Event::Key(Key::ShiftArrow(Arrow::Up)),
        &mut None,
    );
    tui.set_size(20, 15);
    tui.draw();

    #[rustfmt::skip]
    let screen1 =
       "|osa1: a long line 2 |
        |osa1: a long line 3 | 
        |osa1: a long line 4 |
        |osa1: a long line 5 |
        |osa1: a long line 6 |
        |osa1: a long line 7 |
        |osa1: a long line 8 |
        |osa1: a long line 9 |
        |osa1: a long line 10|
        |osa1: a long line 11|
        |osa1: a long line 12|
        |osa1: a long line 13|
        |osa1: a long line 14|
        |osa1:               |
        |< #chan             |";

    expect_screen(screen1, &tui.get_front_buffer(), 20, 15, Location::caller());
}

// https://github.com/osa1/tiny/issues/355
#[test]
fn test_clear_resize_recalc_scroll() {
    let mut tui = TUI::new_test(15, 5);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan { serv, chan };
    let ts = time::at_utc(time::Timespec::new(0, 0));
    for _ in 0..6 {
        tui.add_privmsg("osa1", &"1111 ".repeat(3), ts, &target, false, false);
    }

    tui.draw();

    // scroll up
    tui.handle_input_event(term_input::Event::Key(Key::PageUp), &mut None);

    // clear the screen
    tui.clear(&target);

    // resize window (testing for panic)
    tui.set_size(20, 6);

    // at bottom with no scroll
    #[rustfmt::skip]
    let screen1 =
       "|                    |
        |                    |
        |                    |
        |                    |
        |osa1:               |
        |< #chan             |";

    expect_screen(screen1, &tui.get_front_buffer(), 20, 6, Location::caller());
}

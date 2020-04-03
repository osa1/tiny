use crate::tui::TUI;

use libtiny_ui::*;
use term_input::{Event, Key};
use termbox_simple::CellBuf;
use time::Tm;

use std::panic::Location;

fn buffer_str(buf: &CellBuf, w: u16, h: u16) -> String {
    let w = usize::from(w);
    let h = usize::from(h);

    let mut ret = String::with_capacity(w * h);

    for y in 0..h {
        for x in 0..w {
            let ch = buf.cells[(y * usize::from(w)) + x].ch;
            ret.push(ch);
        }
        if y != h - 1 {
            ret.push('\n');
        }
    }

    ret
}

fn expect_screen(screen: &str, tui: &TUI, w: u16, h: u16, caller: &'static Location<'static>) {
    let mut screen_filtered = String::with_capacity(screen.len());

    let mut in_screen = false;
    for c in screen.chars() {
        if in_screen {
            if c == '|' {
                screen_filtered.push('\n');
                in_screen = false;
            } else {
                screen_filtered.push(c);
            }
        } else if c == '|' {
            in_screen = true;
        }
    }
    let _ = screen_filtered.pop(); // pop the last '\n'

    let found = buffer_str(&tui.get_tb().get_front_buffer(), w, h);

    let mut line = String::new();
    for _ in 0..w {
        line.push('-');
    }

    if screen_filtered != found {
        panic!(
            "Unexpected screen\n\
            Expected:\n\
            {}\n\
            {}\n\
            {}\n\
            Found:\n\
            {}\n\
            {}\n\
            {}\n\
            Called by: {}\n",
            line, screen_filtered, line, line, found, line, caller
        );
    }
}

fn enter_string(tui: &mut TUI, s: &str) {
    for c in s.chars() {
        tui.handle_input_event(Event::Key(Key::Char(c)));
    }
}

#[test]
fn init_screen() {
    let mut tui = TUI::new_test(20, 4);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|Any mentions to you |
         |will be listed here.|
         |                    |
         |mentions            |";
    expect_screen(screen, &tui, 20, 4, Location::caller());
}

#[test]
fn close_rightmost_tab() {
    // After closing right-most tab the tab bar should scroll left.
    let mut tui = TUI::new_test(20, 4);
    tui.new_server_tab("irc.server_1.org", None);
    tui.new_server_tab("irc.server_2.org", None);
    tui.next_tab();
    tui.next_tab();
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                    |
         |                    |
         |                    |
         |< irc.server_2.org  |";
    expect_screen(screen, &tui, 20, 4, Location::caller());

    // Should scroll left when the server tab is closed. Left arrow should still be visible as
    // there are still tabs to the left.
    tui.close_server_tab("irc.server_2.org");
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                    |
         |                    |
         |                    |
         |< irc.server_1.org  |";
    expect_screen(screen, &tui, 20, 4, Location::caller());

    // Scroll left again, left arrow should disappear this time.
    tui.close_server_tab("irc.server_1.org");
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|Any mentions to you |
         |will be listed here.|
         |                    |
         |mentions            |";
    expect_screen(screen, &tui, 20, 4, Location::caller());
}

#[test]
fn small_screen_1() {
    let mut tui = TUI::new_test(21, 3);
    let serv = "irc.server_1.org";
    let chan = "#chan";
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan { serv, chan };
    let ts = time::at_utc(time::Timespec::new(0, 0));
    tui.add_nick("123456", Some(ts), &target);
    tui.add_nick("abcdef", Some(ts), &target);

    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 +123456 +abcdef|
         |osa1:                |
         |< #chan              |";

    expect_screen(screen, &tui, 21, 3, Location::caller());

    tui.set_size(24, 3);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 +123456 +abcdef   |
         |osa1:                   |
         |< irc.server_1.org #chan|";

    expect_screen(screen, &tui, 24, 3, Location::caller());

    tui.set_size(31, 3);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 +123456 +abcdef          |
         |osa1:                          |
         |mentions irc.server_1.org #chan|";

    expect_screen(screen, &tui, 31, 3, Location::caller());
}

#[test]
fn small_screen_2() {
    let mut tui = TUI::new_test(21, 4);
    let serv = "irc.server_1.org";
    let chan = "#chan";
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan { serv, chan };
    let ts = time::at_utc(time::Timespec::new(0, 0));
    tui.set_topic("Blah blah blah-", ts.clone(), serv, chan);

    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                     |
         |00:00 Blah blah blah-|
         |osa1:                |
         |< #chan              |";
    expect_screen(screen, &tui, 21, 4, Location::caller());

    tui.add_nick("123456", Some(ts), &target);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 Blah blah blah-|
         |+123456              |
         |osa1:                |
         |< #chan              |";
    expect_screen(screen, &tui, 21, 4, Location::caller());
}

#[test]
fn ctrl_w() {
    let mut tui = TUI::new_test(30, 3);
    let serv = "irc.server_1.org";
    let chan = "#chan";
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    enter_string(&mut tui, "alskdfj asldkf asldkf aslkdfj aslkdfj asf");

    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                              |
         |osa1: dkf aslkdfj aslkdfj asf |
         |< irc.server_1.org #chan      |";
    expect_screen(screen, &tui, 30, 3, Location::caller());

    tui.handle_input_event(Event::Key(Key::Ctrl('w')));
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                              |
         |osa1: asldkf aslkdfj aslkdfj  |
         |< irc.server_1.org #chan      |";

    expect_screen(screen, &tui, 30, 3, Location::caller());

    println!("~~~~~~~~~~~~~~~~~~~~~~");
    tui.handle_input_event(Event::Key(Key::Ctrl('w')));
    println!("~~~~~~~~~~~~~~~~~~~~~~");
    tui.draw();

    /*
    The buggy behavior was as below:

    let screen =
        "|                              |
         |osa1:  asldkf aslkdfj         |
         |< irc.server_1.org #chan      |";
    */

    #[rustfmt::skip]
    let screen =
        "|                              |
         |osa1:  asldkf asldkf aslkdfj  |
         |< irc.server_1.org #chan      |";

    expect_screen(screen, &tui, 30, 3, Location::caller());

    tui.handle_input_event(Event::Key(Key::Ctrl('w')));
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                              |
         |osa1: alskdfj asldkf asldkf   |
         |< irc.server_1.org #chan      |";

    expect_screen(screen, &tui, 30, 3, Location::caller());
}

// Tests text field wrapping (text_field_wrap setting)
#[test]
fn test_text_field_wrap() {
    // Screen should be wide enough to enable wrapping. See SCROLL_FALLBACK_WIDTH in text_field.rs
    let mut tui = TUI::new_test(40, 8);

    let server = "chat.freenode.net";
    tui.new_server_tab(server, None);
    tui.set_nick(server, "x");

    // Switch to server tab
    tui.next_tab();

    // Write some stuff
    let target = MsgTarget::CurrentTab;
    let ts: Tm = unsafe { ::std::mem::zeroed() };
    tui.add_msg("test test test", ts, &target);

    for _ in 0..37 {
        let event = term_input::Event::Key(Key::Char('a'));
        tui.handle_input_event(event);
    }
    for _ in 0..5 {
        let event = term_input::Event::Key(Key::Char('b'));
        tui.handle_input_event(event);
    }

    tui.draw();

    #[rustfmt::skip]
    let screen =
    "|                                        |
     |                                        |
     |                                        |
     |                                        |
     |00:00 test test test                    |
     |x: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|
     |bbbbb                                   |
     |mentions chat.freenode.net              |";

    expect_screen(screen, &tui, 40, 8, Location::caller());

    // Test resizing
    tui.set_size(46, 8);
    tui.draw();

    #[rustfmt::skip]
    let screen =
    "|                                              |
     |                                              |
     |                                              |
     |                                              |
     |                                              |
     |00:00 test test test                          |
     |x: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaabbbbb |
     |mentions chat.freenode.net                    |";

    expect_screen(screen, &tui, 46, 8, Location::caller());

    // Reset size
    tui.set_size(40, 8);

    // If we remove a few characters now the line above the text field should still be right above
    // the text field
    for _ in 0..6 {
        let event = term_input::Event::Key(Key::Backspace);
        tui.handle_input_event(event);
    }

    tui.draw();

    #[rustfmt::skip]
    let screen =
    "|                                        |
     |                                        |
     |                                        |
     |                                        |
     |                                        |
     |00:00 test test test                    |
     |x: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa |
     |mentions chat.freenode.net              |";

    expect_screen(screen, &tui, 40, 8, Location::caller());

    // On making screen smaller we should fall back to scrolling
    tui.set_size(30, 8);
    for _ in 0..5 {
        let event = term_input::Event::Key(Key::Char('b'));
        tui.handle_input_event(event);
    }
    tui.draw();

    #[rustfmt::skip]
    let screen =
    "|                              |
     |                              |
     |                              |
     |                              |
     |                              |
     |00:00 test test test          |
     |x: aaaaaaaaaaaaaaaaaaaaabbbbb |
     |mentions chat.freenode.net    |";

    expect_screen(screen, &tui, 30, 8, Location::caller());

    tui.set_size(40, 8);
    tui.draw();

    #[rustfmt::skip]
    let screen =
    "|                                        |
     |                                        |
     |                                        |
     |                                        |
     |00:00 test test test                    |
     |x: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab|
     |bbbb                                    |
     |mentions chat.freenode.net              |";

    expect_screen(screen, &tui, 40, 8, Location::caller());

    // Wrapping on words - splits lines on whitespace
    for _ in 0..6 {
        let event = term_input::Event::Key(Key::Backspace);
        tui.handle_input_event(event);
    }

    let event = term_input::Event::Key(Key::Char(' '));
    tui.handle_input_event(event);

    for _ in 0..5 {
        let event = term_input::Event::Key(Key::Char('b'));
        tui.handle_input_event(event);
    }

    tui.draw();

    #[rustfmt::skip]
    let screen =
    "|                                        |
     |                                        |
     |                                        |
     |                                        |
     |00:00 test test test                    |
     |x: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  |
     |bbbbb                                   |
     |mentions chat.freenode.net              |";

    expect_screen(screen, &tui, 40, 8, Location::caller());

    // TODO: Test changing nick (osa: I don't understand how nick length is taken into account when
    // falling back to scrolling)
}

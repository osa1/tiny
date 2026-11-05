use std::panic::Location;

use libtiny_common::{ChanNameRef, MsgTarget};
use term_input::{Arrow, Event, Key};

use crate::test_utils::{buffer_str, expect_screen};
use crate::tui::{TUI, TUIRet};

mod layout;
mod resize;

mod config;

fn enter_string(tui: &mut TUI, s: &str) {
    for c in s.chars() {
        tui.handle_input_event(Event::Key(Key::Char(c)), &mut None);
    }
}

fn completion_input(tui: &mut TUI) -> String {
    tui.draw();
    let screen = buffer_str(&tui.get_front_buffer(), 80, 5);
    screen.lines().nth(3).unwrap().trim_end().to_owned()
}

fn press(tui: &mut TUI, key: Key) {
    tui.handle_input_event(Event::Key(key), &mut None);
}

fn submit(tui: &mut TUI) -> String {
    match tui.handle_input_event(Event::Key(Key::Char('\r')), &mut None) {
        Some(TUIRet::Input { msg, .. }) => msg.into_iter().collect(),
        _ => panic!("expected submitted input"),
    }
}

#[test]
fn command_completion_cycles_and_commits_edits() {
    let mut tui = TUI::new_test(80, 5);
    tui.add_command_completion("close");
    tui.add_command_completion("connect");
    tui.add_command_completion("close");

    enter_string(&mut tui, "/");
    for expected in [
        "/clear", "/close", "/connect", "/ignore", "/notify", "/quit", "/reload", "/switch",
        "/clear",
    ] {
        press(&mut tui, Key::Tab);
        assert_eq!(completion_input(&mut tui), expected);
    }
    press(&mut tui, Key::Ctrl('a'));
    press(&mut tui, Key::Ctrl('k'));

    enter_string(&mut tui, "/c");
    press(&mut tui, Key::Tab);
    assert_eq!(completion_input(&mut tui), "/clear");
    press(&mut tui, Key::Tab);
    assert_eq!(completion_input(&mut tui), "/close");
    press(&mut tui, Key::Arrow(Arrow::Up));
    assert_eq!(completion_input(&mut tui), "/connect");
    press(&mut tui, Key::Tab);
    assert_eq!(completion_input(&mut tui), "/clear");
    press(&mut tui, Key::Arrow(Arrow::Down));
    assert_eq!(completion_input(&mut tui), "/connect");
    press(&mut tui, Key::Arrow(Arrow::Down));
    assert_eq!(completion_input(&mut tui), "/close");

    press(&mut tui, Key::Char('!'));
    assert_eq!(completion_input(&mut tui), "/close!");
    assert_eq!(submit(&mut tui), "/close!");
    press(&mut tui, Key::Arrow(Arrow::Up));
    assert_eq!(completion_input(&mut tui), "/close!");
}

#[test]
fn command_completion_is_shared_with_existing_and_new_tabs() {
    let mut tui = TUI::new_test(80, 5);
    tui.new_server_tab("existing", None);
    tui.add_command_completion("join");

    enter_string(&mut tui, "/jo");
    press(&mut tui, Key::Tab);
    assert_eq!(completion_input(&mut tui), "/join");
    assert_eq!(submit(&mut tui), "/join");

    tui.next_tab();
    enter_string(&mut tui, "/jo");
    press(&mut tui, Key::Tab);
    assert_eq!(completion_input(&mut tui), "/join");
    assert_eq!(submit(&mut tui), "/join");

    tui.new_server_tab("new", None);
    tui.next_tab();
    enter_string(&mut tui, "/jo");
    press(&mut tui, Key::Tab);
    assert_eq!(completion_input(&mut tui), "/join");
    assert_eq!(submit(&mut tui), "/join");
}

#[test]
fn completion_respects_command_boundaries_and_nick_contexts() {
    let mut tui = TUI::new_test(80, 5);
    let serv = "server";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();
    let target = MsgTarget::Chan { serv, chan };
    tui.add_nick("Jack", None, &target);
    tui.add_nick("jane", None, &target);
    tui.add_nick("nina", None, &target);
    tui.add_command_completion("join");

    for (text, cursor, expected) in [
        ("/jn café λ", Some(2), "/join café λ"),
        ("/jo\u{2003}café", Some(3), "/join\u{2003}café"),
        ("/jz", Some(2), "/jz"),
        ("/ja", None, "/ja"),
        ("/J", None, "/J"),
        ("/jo", Some(0), "Jack/jo"),
        ("/msg ni", None, "/msg nina"),
        ("/msg /ni", None, "/msg /nina"),
        ("hello ni", None, "hello nina"),
        ("hello /ni", None, "hello /nina"),
    ] {
        enter_string(&mut tui, text);
        if let Some(cursor) = cursor {
            press(&mut tui, Key::Ctrl('a'));
            for _ in 0..cursor {
                press(&mut tui, Key::Arrow(Arrow::Right));
            }
        }
        press(&mut tui, Key::Tab);
        assert_eq!(completion_input(&mut tui), expected, "input: {text:?}");
        assert_eq!(submit(&mut tui), expected, "input: {text:?}");
    }
}

#[test]
fn exit_dialogue_blocks_completion() {
    let mut tui = TUI::new_test(80, 5);
    tui.add_command_completion("join");
    enter_string(&mut tui, "/jo");
    press(&mut tui, Key::Ctrl('c'));
    press(&mut tui, Key::Tab);
    assert!(completion_input(&mut tui).starts_with("Really quit?"));
    press(&mut tui, Key::Esc);
    assert_eq!(completion_input(&mut tui), "/jo");
    press(&mut tui, Key::Tab);
    assert_eq!(completion_input(&mut tui), "/join");
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
    expect_screen(screen, &tui.get_front_buffer(), 20, 4, Location::caller());
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
    expect_screen(screen, &tui.get_front_buffer(), 20, 4, Location::caller());

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
    expect_screen(screen, &tui.get_front_buffer(), 20, 4, Location::caller());

    // Scroll left again, left arrow should disappear this time.
    tui.close_server_tab("irc.server_1.org");
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|Any mentions to you |
         |will be listed here.|
         |                    |
         |mentions            |";
    expect_screen(screen, &tui.get_front_buffer(), 20, 4, Location::caller());
}

#[test]
fn small_screen_1() {
    let mut tui = TUI::new_test(21, 3);
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

    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 +123456 +abcdef|
         |osa1:                |
         |< #chan              |";

    expect_screen(screen, &tui.get_front_buffer(), 21, 3, Location::caller());

    tui.set_size(24, 3);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 +123456 +abcdef   |
         |osa1:                   |
         |< irc.server_1.org #chan|";

    expect_screen(screen, &tui.get_front_buffer(), 24, 3, Location::caller());

    tui.set_size(31, 3);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 +123456 +abcdef          |
         |osa1:                          |
         |mentions irc.server_1.org #chan|";

    expect_screen(screen, &tui.get_front_buffer(), 31, 3, Location::caller());
}

#[test]
fn small_screen_2() {
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
    tui.set_topic("Blah blah blah-", ts, serv, chan);

    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                     |
         |00:00 Blah blah blah-|
         |osa1:                |
         |< #chan              |";
    expect_screen(screen, &tui.get_front_buffer(), 21, 4, Location::caller());

    tui.add_nick("123456", Some(ts), &target);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 Blah blah blah-|
         |+123456              |
         |osa1:                |
         |< #chan              |";
    expect_screen(screen, &tui.get_front_buffer(), 21, 4, Location::caller());
}

#[test]
fn ctrl_w() {
    let mut tui = TUI::new_test(30, 3);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
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
    expect_screen(screen, &tui.get_front_buffer(), 30, 3, Location::caller());

    tui.handle_input_event(Event::Key(Key::Ctrl('w')), &mut None);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                              |
         |osa1: asldkf aslkdfj aslkdfj  |
         |< irc.server_1.org #chan      |";

    expect_screen(screen, &tui.get_front_buffer(), 30, 3, Location::caller());

    println!("~~~~~~~~~~~~~~~~~~~~~~");
    tui.handle_input_event(Event::Key(Key::Ctrl('w')), &mut None);
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

    expect_screen(screen, &tui.get_front_buffer(), 30, 3, Location::caller());

    tui.handle_input_event(Event::Key(Key::Ctrl('w')), &mut None);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                              |
         |osa1: alskdfj asldkf asldkf   |
         |< irc.server_1.org #chan      |";

    expect_screen(screen, &tui.get_front_buffer(), 30, 3, Location::caller());
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
    let ts = time::empty_tm();
    tui.add_msg("test test test", ts, &target);

    for _ in 0..37 {
        let event = term_input::Event::Key(Key::Char('a'));
        tui.handle_input_event(event, &mut None);
    }
    for _ in 0..5 {
        let event = term_input::Event::Key(Key::Char('b'));
        tui.handle_input_event(event, &mut None);
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

    expect_screen(screen, &tui.get_front_buffer(), 40, 8, Location::caller());

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

    expect_screen(screen, &tui.get_front_buffer(), 46, 8, Location::caller());

    // Reset size
    tui.set_size(40, 8);

    // If we remove a few characters now the line above the text field should still be right above
    // the text field
    for _ in 0..6 {
        let event = term_input::Event::Key(Key::Backspace);
        tui.handle_input_event(event, &mut None);
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

    expect_screen(screen, &tui.get_front_buffer(), 40, 8, Location::caller());

    // On making screen smaller we should fall back to scrolling
    tui.set_size(30, 8);
    for _ in 0..5 {
        let event = term_input::Event::Key(Key::Char('b'));
        tui.handle_input_event(event, &mut None);
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

    expect_screen(screen, &tui.get_front_buffer(), 30, 8, Location::caller());

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

    expect_screen(screen, &tui.get_front_buffer(), 40, 8, Location::caller());

    // Wrapping on words - splits lines on whitespace
    for _ in 0..6 {
        let event = term_input::Event::Key(Key::Backspace);
        tui.handle_input_event(event, &mut None);
    }
    // InputLine cache gets invalidated after backspace, need to redraw to calculate.
    tui.draw();
    let event = term_input::Event::Key(Key::Char(' '));
    tui.handle_input_event(event, &mut None);

    for _ in 0..5 {
        let event = term_input::Event::Key(Key::Char('b'));
        tui.handle_input_event(event, &mut None);
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

    expect_screen(screen, &tui.get_front_buffer(), 40, 8, Location::caller());

    // TODO: Test changing nick (osa: I don't understand how nick length is taken into account when
    // falling back to scrolling)
}

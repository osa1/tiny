use std::panic::Location;

use libtiny_common::{ChanNameRef, MsgTarget};
use term_input::{Event, Key};

use crate::test_utils::expect_screen;
use crate::tui::TUI;

mod layout;
mod resize;

mod config;

fn enter_string(tui: &mut TUI, s: &str) {
    for c in s.chars() {
        tui.handle_input_event(Event::Key(Key::Char(c)), &mut None);
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

// Test for issue #379: Wide characters (emoji) should be displayed correctly
// When pasting 🟩🟩🟩🟩🟩, all 5 squares should be visible
#[test]
fn test_wide_emoji_display() {
    let mut tui = TUI::new_test(30, 4);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    // Enter green square emoji (🟩) which has width 2
    // In a 30-char wide screen with "osa1: " prefix (6 chars), we have 24 chars remaining
    // Each 🟩 takes 2 columns, so we can fit 12 emojis
    enter_string(&mut tui, "🟩🟩🟩🟩🟩");

    tui.draw();

    // The input line should show all 5 emojis
    // Note: In the test framework, wide characters still occupy one cell in the buffer
    // but the cursor position and line wrapping calculations account for their width
    let buffer = tui.get_front_buffer();
    let buffer_str = crate::test_utils::buffer_str(&buffer, 30, 4);

    // Check that the emojis are in the buffer
    assert!(buffer_str.contains('🟩'), "Buffer should contain the green square emoji");

    // Count how many emojis are in the buffer
    let emoji_count = buffer_str.chars().filter(|&c| c == '🟩').count();
    assert_eq!(emoji_count, 5, "All 5 emojis should be displayed, but found {}", emoji_count);
}

// Test for issue #379: Wide characters in messages should be displayed correctly
#[test]
fn test_wide_emoji_in_message() {
    let mut tui = TUI::new_test(40, 4);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan { serv, chan };
    let ts = time::at_utc(time::Timespec::new(0, 0));

    // Add a message with green square emojis
    tui.add_msg("🟩🟩🟩🟩🟩", ts, &target);

    tui.draw();

    let buffer = tui.get_front_buffer();
    let buffer_str = crate::test_utils::buffer_str(&buffer, 40, 4);

    // Check that all emojis are in the buffer
    let emoji_count = buffer_str.chars().filter(|&c| c == '🟩').count();
    assert_eq!(emoji_count, 5, "All 5 emojis should be displayed in the message, but found {}", emoji_count);
}

// Test CJK (Chinese/Japanese/Korean) character display in input line
// CJK characters have width 2 and should be handled correctly
#[test]
fn test_cjk_characters_in_input() {
    let mut tui = TUI::new_test(30, 4);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    // Enter Chinese characters "你好世界" (Hello World)
    // Each CJK character has width 2
    enter_string(&mut tui, "你好世界");

    tui.draw();

    let buffer = tui.get_front_buffer();
    let buffer_str = crate::test_utils::buffer_str(&buffer, 30, 4);

    // Check that all CJK characters are in the buffer
    assert!(buffer_str.contains('你'), "Buffer should contain Chinese character '你'");
    assert!(buffer_str.contains('好'), "Buffer should contain Chinese character '好'");
    assert!(buffer_str.contains('世'), "Buffer should contain Chinese character '世'");
    assert!(buffer_str.contains('界'), "Buffer should contain Chinese character '界'");

    // Count CJK characters
    let cjk_count = buffer_str.chars().filter(|&c| "你好世界".contains(c)).count();
    assert_eq!(cjk_count, 4, "All 4 CJK characters should be displayed, but found {}", cjk_count);
}

// Test CJK characters in messages
#[test]
fn test_cjk_characters_in_message() {
    let mut tui = TUI::new_test(40, 4);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    let target = MsgTarget::Chan { serv, chan };
    let ts = time::at_utc(time::Timespec::new(0, 0));

    // Add a message with CJK characters
    tui.add_msg("你好，这是中文测试", ts, &target);

    tui.draw();

    let buffer = tui.get_front_buffer();
    let buffer_str = crate::test_utils::buffer_str(&buffer, 40, 4);

    // Check that CJK characters are in the buffer
    assert!(buffer_str.contains('你'), "Buffer should contain Chinese character '你'");
    assert!(buffer_str.contains('好'), "Buffer should contain Chinese character '好'");
    assert!(buffer_str.contains('中'), "Buffer should contain Chinese character '中'");
    assert!(buffer_str.contains('文'), "Buffer should contain Chinese character '文'");
}

// Test mixed ASCII and CJK characters
#[test]
fn test_mixed_ascii_cjk_input() {
    let mut tui = TUI::new_test(40, 4);
    let serv = "irc.server_1.org";
    let chan = ChanNameRef::new("#chan");
    tui.new_server_tab(serv, None);
    tui.set_nick(serv, "osa1");
    tui.new_chan_tab(serv, chan);
    tui.next_tab();
    tui.next_tab();

    // Enter mixed ASCII and CJK: "Hello你好World世界"
    enter_string(&mut tui, "Hello你好World世界");

    tui.draw();

    let buffer = tui.get_front_buffer();
    let buffer_str = crate::test_utils::buffer_str(&buffer, 40, 4);

    // Check that both ASCII and CJK characters are present
    assert!(buffer_str.contains("Hello"), "Buffer should contain 'Hello'");
    assert!(buffer_str.contains('你'), "Buffer should contain Chinese character '你'");
    assert!(buffer_str.contains('好'), "Buffer should contain Chinese character '好'");
    assert!(buffer_str.contains("World"), "Buffer should contain 'World'");
    assert!(buffer_str.contains('世'), "Buffer should contain Chinese character '世'");
    assert!(buffer_str.contains('界'), "Buffer should contain Chinese character '界'");
}

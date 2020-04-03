use crate::tui::TUI;

use libtiny_ui::*;
use term_input::Key;
use termbox_simple::CellBuf;
use time::Tm;

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

fn expect_screen(screen: &str, tui: &TUI, w: u16, h: u16) {
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
    if screen_filtered != found {
        panic!(
            "Unexpected screen\nExpected:\n{:?}\nFound:\n{:?}",
            screen_filtered, found
        );
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
    expect_screen(screen, &tui, 20, 4);
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
    expect_screen(screen, &tui, 20, 4);

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
    expect_screen(screen, &tui, 20, 4);

    // Scroll left again, left arrow should disappear this time.
    tui.close_server_tab("irc.server_1.org");
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|Any mentions to you |
         |will be listed here.|
         |                    |
         |mentions            |";
    expect_screen(screen, &tui, 20, 4);
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
    let ts = unsafe { ::std::mem::zeroed() };
    tui.add_nick("123456", Some(ts), &target);
    tui.add_nick("abcdef", Some(ts), &target);

    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 +123456 +abcdef|
         |osa1:                |
         |< #chan              |";
    expect_screen(screen, &tui, 21, 3);

    tui.set_size(24, 3);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 +123456 +abcdef   |
         |osa1:                   |
         |< irc.server_1.org #chan|";
    expect_screen(screen, &tui, 24, 3);

    tui.set_size(31, 3);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 +123456 +abcdef          |
         |osa1:                          |
         |mentions irc.server_1.org #chan|";
    expect_screen(screen, &tui, 31, 3);
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
    let ts: Tm = unsafe { ::std::mem::zeroed() };
    tui.set_topic("Blah blah blah-", ts.clone(), serv, chan);

    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|                     |
         |00:00 Blah blah blah-|
         |osa1:                |
         |< #chan              |";
    expect_screen(screen, &tui, 21, 4);

    tui.add_nick("123456", Some(ts), &target);
    tui.draw();

    #[rustfmt::skip]
    let screen =
        "|00:00 Blah blah blah-|
         |+123456              |
         |osa1:                |
         |< #chan              |";
    expect_screen(screen, &tui, 21, 4);
}

///
/// Tests text wrap on
///
#[test]
fn test_text_field_wrap() {
    let mut tui = TUI::new_test(60, 8);
    tui.set_text_field_wrap_test(true);

    let server = "chat.freenode.net";
    tui.new_server_tab(server);
    tui.set_nick(server, "osa1");

    // switch to server tab
    tui.next_tab();

    // write some stuff
    let target = MsgTarget::CurrentTab;
    let ts: Tm = unsafe { ::std::mem::zeroed() };
    tui.add_msg("test test test", ts, &target);

    for _ in 0..65 {
        let event = term_input::Event::Key(Key::Char('a'));
        tui.handle_input_event(event);
    }

    tui.draw();

    #[rustfmt::skip]
    let screen = 
    "|                                                            |
     |                                                            |
     |                                                            |
     |                                                            |
     |00:00 test test test                                        |
     |osa1: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa|
     |      aaaaaaaaaaa                                           |
     |mentions chat.freenode.net                                  |";

    expect_screen(screen, &tui, 60, 8);
}

///
/// Tests text wrap on
/// Writes some text that will wrap
/// Deletes the text to check if the input field went back to 1 line
///
#[test]
fn test_text_field_wrap_add_delete() {
    let mut tui = TUI::new_test(60, 8);
    tui.set_text_field_wrap_test(true);

    let server = "chat.freenode.net";
    tui.new_server_tab(server);
    tui.set_nick(server, "osa1");

    // switch to server tab
    tui.next_tab();

    // write some stuff
    let target = MsgTarget::CurrentTab;
    let ts: Tm = unsafe { ::std::mem::zeroed() };
    tui.add_msg("test test test", ts, &target);

    for _ in 0..65 {
        let event = term_input::Event::Key(Key::Char('a'));
        tui.handle_input_event(event);
    }

    tui.draw();

    // Hit ctrl-a to go to the beginning of line
    let ctrl_a_event = term_input::Event::Key(Key::Ctrl('a'));
    tui.handle_input_event(ctrl_a_event);
    // Hit ctrl-k to clear the line
    let ctrl_k_event = term_input::Event::Key(Key::Ctrl('k'));
    tui.handle_input_event(ctrl_k_event);

    tui.draw();

    #[rustfmt::skip]
    let screen = 
    "|                                                            |
     |                                                            |
     |                                                            |
     |                                                            |
     |                                                            |
     |00:00 test test test                                        |
     |osa1:                                                       |
     |mentions chat.freenode.net                                  |";

    expect_screen(screen, &tui, 60, 8);
}

///
/// Tests scroll mode on (text wrap off)
///
#[test]
fn test_text_field_wrap_false() {
    let mut tui = TUI::new_test(60, 8);
    tui.set_text_field_wrap_test(false);

    let server = "chat.freenode.net";
    tui.new_server_tab(server);
    tui.set_nick(server, "osa1");
    // switch to server tab
    tui.next_tab();

    for _ in 0..65 {
        let event = term_input::Event::Key(Key::Char('a'));
        tui.handle_input_event(event);
    }

    tui.draw();

    #[rustfmt::skip]
    let screen = 
    "|                                                            |
     |                                                            |
     |                                                            |
     |                                                            |
     |                                                            |
     |                                                            |
     |osa1: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa |
     |mentions chat.freenode.net                                  |";
    //                                               cursor space^
    expect_screen(screen, &tui, 60, 8);
}

///
/// Tests scroll (text wrap on) small screen
///
#[test]
fn test_scroll_text_field_wrap_true() {
    let mut tui = TUI::new_test(30, 10);
    tui.set_text_field_wrap_test(true);

    let server = "chat.freenode.net";
    tui.new_server_tab(server);
    tui.set_nick(server, "osa1");
    // switch to server tab
    tui.next_tab();

    for _ in 0..65 {
        let event = term_input::Event::Key(Key::Char('a'));
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
     |                              |
     |                              |
     |                              |
     |osa1: aaaaaaaaaaaaaaaaaaaaaaa |
     |mentions chat.freenode.net    |";
    //                 cursor space^
    expect_screen(screen, &tui, 30, 10);
}

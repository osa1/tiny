use std::panic::Location;

use termbox_simple::CellBuf;

pub fn buffer_str(buf: &CellBuf, w: u16, h: u16) -> String {
    let w = usize::from(w);
    let h = usize::from(h);

    let mut ret = String::with_capacity(w * h);

    for y in 0..h {
        for x in 0..w {
            let ch = buf.cells[(y * w) + x].ch;
            ret.push(ch);
        }
        if y != h - 1 {
            ret.push('\n');
        }
    }

    ret
}

pub fn expect_screen(
    screen: &str,
    front_buffer: &CellBuf,
    w: u16,
    h: u16,
    caller: &'static Location<'static>,
) {
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

    let found = buffer_str(front_buffer, w, h);

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

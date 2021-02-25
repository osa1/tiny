use term_input::{Event, Input, Key};
use termbox_simple::*;
use tokio::stream::StreamExt;

fn main() {
    let mut tui = Termbox::init().unwrap();

    let mut fg = true;
    draw(&mut tui, fg);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();

    local.block_on(&runtime, async move {
        let mut input = Input::new();
        while let Some(mb_ev) = input.next().await {
            match mb_ev {
                Err(_) => {}
                Ok(Event::Key(Key::Tab)) => {
                    fg = !fg;
                }
                Ok(Event::Key(Key::Esc)) => {
                    return;
                }
                Ok(_) => {}
            }
            draw(&mut tui, fg);
        }
    });
}

fn draw(tui: &mut Termbox, fg: bool) {
    tui.clear();

    let row = 0;
    let row = draw_range(tui, 0, 16, row, fg);
    let row = draw_range(tui, 16, 232, row + 1, fg);
    let _ = draw_range(tui, 232, 256, row + 1, fg);

    tui.present();
}

fn draw_range(tui: &mut Termbox, begin: u16, end: u16, mut row: i32, fg: bool) -> i32 {
    let mut col = 0;
    for i in begin..end {
        if col != 0 && col % 24 == 0 {
            col = 0;
            row += 1;
        }

        let string = format!("{:>3}", i);
        let fg_ = if fg { i } else { 0 };
        let bg_ = if fg { 0 } else { i };
        tui.change_cell(col, row, string.chars().nth(0).unwrap(), fg_, bg_);
        tui.change_cell(col + 2, row, string.chars().nth(2).unwrap(), fg_, bg_);
        tui.change_cell(col + 1, row, string.chars().nth(1).unwrap(), fg_, bg_);
        col += 4;
    }

    row + 1
}

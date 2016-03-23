use std::cmp::{max, min};
use std::io::BufRead;

use rustbox::{RustBox, Style, Color};

pub struct MsgWidget {
    msgs   : Vec<String>,
    scroll : i32,

    // TODO: logging
}

impl MsgWidget {
    pub fn new() -> MsgWidget {
        MsgWidget {
            msgs: Vec::new(),
            scroll: 0,
        }
    }

    pub fn add_irc_raw_msg(&mut self, msg : &[u8]) {
        for line in msg.lines() {
            self.msgs.push(line.unwrap());
        }
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32, width : i32, mut height : i32) {
        let mut line_idx = min(self.scroll + height, self.msgs.len() as i32) - 1;
        let mut lines_printed = 0;
        while line_idx > 0 {
            rustbox.print(pos_x as usize, height as usize,
                          Style::empty(), Color::Blue, Color::Default,
                          &self.msgs[line_idx as usize]);
            height -= 1;
            line_idx -= 1;
        }
    }
}

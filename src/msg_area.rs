use std::cmp::min;

use rustbox::{RustBox, Style, Color};

pub struct MsgArea {
    msgs   : Vec<String>,
    height : i32,

    /// Vertical scroll
    scroll : i32,

    // TODO: logging
}

impl MsgArea {
    pub fn new(height : i32) -> MsgArea {
        MsgArea {
            msgs: Vec::new(),
            height: height,
            scroll: 0,
        }
    }

    pub fn add_msg(&mut self, msg : &Vec<char>) {
        // The vector shouldn't have newlines ('\n'), but we don't check it here
        // for now.

        let msg_str : String = msg.iter().cloned().collect();

        // Decide whether to scroll
        let need_to_scroll = self.scroll + self.height == self.msgs.len() as i32;
        if need_to_scroll {
            self.scroll = (self.msgs.len() as i32) + 1 - self.height;
        }

        self.msgs.push(msg_str);
    }

    pub fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32) {
        let mut row = self.height - 1;
        let mut line_idx = min(self.scroll + self.height, self.msgs.len() as i32 - 1);
        while line_idx >= 0 {
            rustbox.print(pos_x as usize, (pos_y + row) as usize,
                          Style::empty(), Color::Blue, Color::Default,
                          &self.msgs[line_idx as usize]);
            row -= 1;
            line_idx -= 1;
        }
    }
}

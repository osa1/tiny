use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};

use termbox_simple::*;

fn main() {
    let mut poll = Poll::new().unwrap();
    poll.registry()
        .register(
            &mut SourceFd(&libc::STDIN_FILENO),
            Token(libc::STDIN_FILENO as usize),
            Interest::READABLE,
        )
        .unwrap();

    let mut termbox = Termbox::init().unwrap();
    let mut events = Events::with_capacity(10);

    'mainloop: loop {
        match poll.poll(&mut events, None) {
            Err(_) => {
                termbox.resize();
            }
            Ok(_) => {
                let mut buf: Vec<u8> = vec![];
                term_input::read_stdin(&mut buf).unwrap();
                let string = format!("{:?}", buf);
                termbox.clear();
                if buf == vec![27] {
                    break 'mainloop;
                }
                for (char_idx, char) in string.chars().enumerate() {
                    termbox.change_cell(char_idx as libc::c_int, 0, char, TB_DEFAULT, TB_DEFAULT);
                }
                termbox.present();
            }
        }
    }
}

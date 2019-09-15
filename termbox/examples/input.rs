use mio::unix::EventedFd;
use mio::Events;
use mio::Poll;
use mio::PollOpt;
use mio::Ready;
use mio::Token;

use termbox_simple::*;

fn main() {
    let poll = Poll::new().unwrap();
    poll.register(
        &EventedFd(&libc::STDIN_FILENO),
        Token(libc::STDIN_FILENO as usize),
        Ready::readable(),
        PollOpt::level(),
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
                if term_input::read_stdin(&mut buf) {
                    let string = format!("{:?}", buf);
                    termbox.clear();
                    if buf == vec![27] {
                        break 'mainloop;
                    }
                    for (char_idx, char) in string.chars().enumerate() {
                        termbox.change_cell(char_idx as libc::c_int, 0, char, TB_WHITE, TB_DEFAULT);
                    }
                    termbox.present();
                }
            }
        }
    }
}

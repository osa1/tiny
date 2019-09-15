use mio::unix::EventedFd;
use mio::Events;
use mio::Poll;
use mio::PollOpt;
use mio::Ready;
use mio::Token;

use std::io;
use std::io::Write;

use term_input::{Event, Input, Key};

fn main() {
    // put the terminal in non-buffering, no-enchoing mode
    let mut old_term: libc::termios = unsafe { std::mem::zeroed() };
    unsafe {
        libc::tcgetattr(libc::STDIN_FILENO, &mut old_term);
    }

    let mut new_term: libc::termios = old_term.clone();
    new_term.c_iflag &= !(libc::IGNBRK
        | libc::BRKINT
        | libc::PARMRK
        | libc::ISTRIP
        | libc::INLCR
        | libc::IGNCR
        | libc::ICRNL
        | libc::IXON);
    new_term.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG | libc::IEXTEN);
    unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &new_term) };

    // enable focus events
    {
        let stdout = io::stdout();
        stdout.lock().write_all(b"\x1b[?1004h").unwrap();
        stdout.lock().flush().unwrap();
    }

    let poll = Poll::new().unwrap();
    poll.register(
        &EventedFd(&libc::STDIN_FILENO),
        Token(libc::STDIN_FILENO as usize),
        Ready::readable(),
        PollOpt::level(),
    )
    .unwrap();

    let mut input = Input::new();
    let mut evs = Vec::new();
    let mut events = Events::with_capacity(10);
    'mainloop: loop {
        let _poll_ret = poll.poll(&mut events, None);
        // println!("poll ret: {:?}", _poll_ret);

        // Err: probably SIGWINCH
        // Ok: stdin available
        //
        // there are events to handle either way
        input.read_input_events(&mut evs);
        println!("{:?}", evs);
        for ev in evs.iter() {
            if ev == &Event::Key(Key::Esc) {
                break 'mainloop;
            }
        }
    }

    // restore the old settings
    unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &old_term) };

    // disable focus events
    {
        let stdout = io::stdout();
        stdout.lock().write_all(b"\x1b[?1004l").unwrap();
    }
}

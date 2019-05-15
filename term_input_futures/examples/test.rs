extern crate futures;
extern crate libc;
extern crate term_input;
extern crate tokio;

use std::io;
use std::io::Write;

use futures::future;
use futures::Future;
use futures::Stream;

use term_input::{Event, Input, Key};

enum IterErr {
    Io(std::io::Error),
    Break,
}

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

    /* DO THE BUSINESS HERE */
    let input = Input::new();
    tokio::run(future::lazy(|| {
        input
            .map_err(IterErr::Io)
            .for_each(|ev| {
                println!("{:?}", ev);
                if ev == Event::Key(Key::Esc) {
                    future::err(IterErr::Break)
                } else {
                    future::ok(())
                }
            })
            .map_err(|e| match e {
                IterErr::Break => {}
                IterErr::Io(io_err) => println!("Error: {:?}", io_err),
            })
    }));

    // restore the old settings
    unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &old_term) };

    // disable focus events
    {
        let stdout = io::stdout();
        stdout.lock().write_all(b"\x1b[?1004l").unwrap();
    }
}

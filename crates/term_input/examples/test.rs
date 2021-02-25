use std::io;
use std::io::Write;
use tokio::stream::StreamExt;

use term_input::{Event, Input, Key};

fn main() {
    // put the terminal in non-buffering, no-enchoing mode
    let mut old_term: libc::termios = unsafe { std::mem::zeroed() };
    unsafe {
        libc::tcgetattr(libc::STDIN_FILENO, &mut old_term);
    }

    let mut new_term: libc::termios = old_term;
    new_term.c_iflag &= !(libc::IGNBRK
        | libc::BRKINT
        | libc::PARMRK
        | libc::ISTRIP
        | libc::INLCR
        | libc::IGNCR
        | libc::ICRNL
        | libc::IXON);
    new_term.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG | libc::IEXTEN);
    new_term.c_cc[libc::VMIN] = 0;
    new_term.c_cc[libc::VTIME] = 0;
    unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &new_term) };

    // enable focus events
    {
        let stdout = io::stdout();
        stdout.lock().write_all(b"\x1b[?1004h").unwrap();
        stdout.lock().flush().unwrap();
    }

    /* DO THE BUSINESS HERE */
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        let mut input = Input::new();
        while let Some(mb_ev) = input.next().await {
            match mb_ev {
                Ok(ev) => {
                    println!("{:?}", ev);
                    if ev == Event::Key(Key::Esc) {
                        break;
                    }
                }
                Err(io_err) => {
                    println!("Error: {:?}", io_err);
                    break;
                }
            }
        }
    });

    // restore the old settings
    unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &old_term) };

    // disable focus events
    {
        let stdout = io::stdout();
        stdout.lock().write_all(b"\x1b[?1004l").unwrap();
    }
}

extern crate ev_loop;
extern crate libc;
extern crate term_input;

use term_input::{Event, Key, Input};
use ev_loop::{EvLoop, READ_EV};

fn main() {
    // put the terminal in non-buffering, no-enchoing mode
    let mut old_term : libc::termios = unsafe { std::mem::zeroed() };
    unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut old_term); }

    let mut new_term : libc::termios = old_term.clone();
    new_term.c_iflag &= !(libc::IGNBRK | libc::BRKINT | libc::PARMRK | libc::ISTRIP | libc::INLCR |
                          libc::IGNCR | libc::ICRNL | libc::IXON);
    new_term.c_lflag &= !(libc::ICANON | libc::ECHO | libc::ISIG | libc::IEXTEN);
    unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &new_term) };

    let mut ev_loop: EvLoop<()> = EvLoop::new();
    let mut input = Input::new();
    let mut evs = Vec::new();
    ev_loop.add_fd(libc::STDIN_FILENO, READ_EV, Box::new(move |_, ctrl, _| {
        input.read_input_events(&mut evs);
        println!("{:?}", evs);
        for ev in evs.iter() {
            if ev == &Event::Key(Key::Esc) {
                ctrl.stop();
            }
        }
    }));
    ev_loop.run(());

    // restore the old settings
    unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &old_term) };
}

extern crate libc;
extern crate term_input;

use term_input::{Event, Key, Input};

fn main() {
    // put the terminal in non-buffering, no-enchoing mode
    let mut old_term : libc::termios = unsafe { std::mem::zeroed() };
    unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut old_term); }

    let mut new_term : libc::termios = old_term.clone();
    new_term.c_lflag &= !(libc::ICANON | libc::ECHO);
    unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &new_term) };

    // Set up the descriptors for select()
    let mut fd_set : libc::fd_set = unsafe { std::mem::zeroed() };

    unsafe { libc::FD_SET(libc::STDIN_FILENO, &mut fd_set); }

    let mut input = Input::new();
    let mut evs = Vec::new();

    'outer:
    loop {
        let mut fd_set_ = fd_set.clone();
        let ret =
            unsafe {
                libc::select(1,
                             &mut fd_set_,           // read fds
                             std::ptr::null_mut(),   // write fds
                             std::ptr::null_mut(),   // error fds
                             std::ptr::null_mut())   // timeval
            };

        if unsafe { ret == -1 || libc::FD_ISSET(0, &mut fd_set_) } {
            input.read_input_events(&mut evs);
            println!("{:?}", evs);
            for ev in evs.iter() {
                if ev == &Event::Key(Key::Esc) { break 'outer; }
            }
        }
    }

    // restore the old settings
    // (FIXME: This is not going to work as we have no way of exiting the loop
    // above)
    unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &old_term) };
}

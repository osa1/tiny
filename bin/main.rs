#![feature(alloc_system)]

extern crate tiny;

use tiny::Tiny;

fn main() {
    Tiny::init().mainloop();
}

extern crate gcc;

fn main() {
    gcc::compile_library("libtermbox.a", &["cbits/termbox.c"]);
    println!("cargo:rustc-flags=-l static=termbox");
}

extern crate tiny;

use tiny::Tiny;

fn main() {
    Tiny::new("tiny_test".to_owned(), "tiny@tiny".to_owned(), "yada yada".to_owned()).mainloop();
}

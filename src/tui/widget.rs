use rustbox::keyboard::Key;
use rustbox::RustBox;

pub trait Widget {
    fn draw(&self, rustbox : &RustBox, pos_x : i32, pos_y : i32);

    fn keypressed(&mut self, key : Key) -> WidgetRet;

    fn resize(&mut self, width : i32, height : i32);
}

pub enum WidgetRet {
    KeyHandled,
    KeyIgnored,
    Input(Vec<char>),
}

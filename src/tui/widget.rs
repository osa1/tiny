pub enum WidgetRet {
    KeyHandled,
    KeyIgnored,
    Input(Vec<char>),
}

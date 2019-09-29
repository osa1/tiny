pub(crate) enum WidgetRet {
    /// Key is handled by the widget.
    KeyHandled,

    /// Key is ignored by the widget.
    KeyIgnored,

    /// An input is submitted.
    Input(Vec<char>),

    /// Remove the widget. E.g. close the tab, hide the dialogue etc.
    Remove,

    /// An exit event happened.
    Abort,
}

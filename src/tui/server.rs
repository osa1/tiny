// UI that shows server messages, status etc.

use tui::msg_area::MsgArea;
use tui::text_field::TextField;
use tui::widget::{Widget, WidgetRet};

struct ServerUI {
    /// Incoming and sent messages appear
    msg_area : MsgArea,

    /// User input field
    text_field : TextField,
}

impl ServerUI {

}

impl Widget for ServerUI {

}



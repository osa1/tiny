//! A widget with messaging tabs (a GTK Notebook).

use gio::prelude::*;
use gtk::prelude::*;

use crate::messaging::MessagingUI;

pub(crate) struct Tabs {
    notebook: gtk::Notebook,
}

impl Tabs {
    pub(crate) fn new() -> Tabs {
        let notebook = gtk::Notebook::new();
        notebook.set_tab_pos(gtk::PositionType::Bottom);

        Tabs { notebook }
    }

    pub(crate) fn get_widget(&self) -> &gtk::Widget {
        self.notebook.upcast_ref()
    }
}

//
// UI protocol methods
//

impl Tabs {
    pub(crate) fn new_server_tab(&self, serv: &str) {
        let tab = MessagingUI::new();
        let label = gtk::Label::new(Some(serv));
        self.notebook.append_page(tab.get_widget(), Some(&label));
    }
}

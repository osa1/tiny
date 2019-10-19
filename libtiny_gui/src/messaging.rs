//! A widget with a text field and an entry.

use gio::prelude::*;
use gtk::prelude::*;

pub(crate) struct MessagingUI {
    text_view: gtk::TextView,
    entry: gtk::Entry,
    box_: gtk::Box,
}

impl MessagingUI {
    pub(crate) fn new() -> MessagingUI {
        // vbox -> [ text_view, entry ]
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let text_view = gtk::TextViewBuilder::new()
            .cursor_visible(false)
            .editable(false)
            .hexpand(true)
            .vexpand(true)
            .build();

        let entry = gtk::Entry::new();

        box_.pack_start(&text_view, true, true, 0);
        box_.pack_start(&entry, false, true, 0);

        MessagingUI {
            text_view,
            entry,
            box_,
        }
    }

    pub(crate) fn get_widget(&self) -> &gtk::Widget {
        self.box_.upcast_ref()
    }
}

//! A widget with a text field and an entry.

use gio::prelude::*;
use gtk::prelude::*;
use libtiny_ui::*;
use std::cell::RefCell;
use tokio::sync::mpsc;

pub(crate) struct MessagingUI {
    text_view: gtk::TextView,
    entry: gtk::Entry,
    box_: gtk::Box,
    msg_src: MsgSource,
}

impl MessagingUI {
    pub(crate) fn new(msg_src: MsgSource, snd_ev: mpsc::Sender<Event>) -> MessagingUI {
        // vbox -> [ text_view, entry ]
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let text_view = gtk::TextViewBuilder::new()
            .cursor_visible(false)
            .editable(false)
            .hexpand(true)
            .vexpand(true)
            .build();

        let entry = gtk::Entry::new();
        let msg_src_ = msg_src.clone();
        let snd_ev = RefCell::new(snd_ev);
        entry.connect_activate(move |entry| {
            if let Some(text) = entry.get_text() {
                entry.set_text("");
                snd_ev
                    .borrow_mut()
                    .try_send(Event::Msg {
                        msg: text.as_str().to_owned(),
                        source: msg_src_.clone(),
                    })
                    .unwrap();
            }
        });

        box_.pack_start(&text_view, true, true, 0);
        box_.pack_start(&entry, false, true, 0);

        MessagingUI {
            text_view,
            entry,
            box_,
            msg_src,
        }
    }

    pub(crate) fn get_widget(&self) -> &gtk::Widget {
        self.box_.upcast_ref()
    }
}

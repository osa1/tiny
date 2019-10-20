//! A widget with a text field and an entry.

use gio::prelude::*;
use gtk::prelude::*;
use libtiny_ui::*;
use std::cell::RefCell;
use time::Tm;
use tokio::sync::mpsc;

pub(crate) struct MessagingUI {
    scrolled: gtk::ScrolledWindow,
    text_view: gtk::TextView,
    entry: gtk::Entry,
    box_: gtk::Box,
    msg_src: MsgSource,
}

impl MessagingUI {
    pub(crate) fn new(msg_src: MsgSource, snd_ev: mpsc::Sender<Event>) -> MessagingUI {
        // vbox -> [ scrolled -> text_view, entry ]
        let box_ = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let scrolled = gtk::ScrolledWindow::new(gtk::NONE_ADJUSTMENT, gtk::NONE_ADJUSTMENT);

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

        box_.pack_start(&scrolled, true, true, 0);
        scrolled.add(&text_view);
        box_.pack_start(&entry, false, true, 0);

        MessagingUI {
            scrolled,
            text_view,
            entry,
            box_,
            msg_src,
        }
    }

    pub(crate) fn get_widget(&self) -> &gtk::Widget {
        self.box_.upcast_ref()
    }

    pub(crate) fn add_client_msg(&self, msg: &str) {
        let text_buffer = self.text_view.get_buffer().unwrap();
        let mut end_iter = text_buffer.get_end_iter();
        text_buffer.insert_markup(&mut end_iter, &format!("[client] {}\n", msg));
    }

    pub(crate) fn add_msg(&self, msg: &str, ts: Tm) {
        let text_buffer = self.text_view.get_buffer().unwrap();
        let mut end_iter = text_buffer.get_end_iter();
        text_buffer.insert_markup(&mut end_iter, &format!("{}\n", msg));
    }

    pub(crate) fn add_err_msg(&self, msg: &str, ts: Tm) {
        let text_buffer = self.text_view.get_buffer().unwrap();
        let mut end_iter = text_buffer.get_end_iter();
        text_buffer.insert_markup(
            &mut end_iter,
            &format!(
                "[error] [{}] {}\n",
                time::strftime("%H:%M:%S", &ts).unwrap(),
                msg
            ),
        );
    }

    pub(crate) fn add_client_err_msg(&self, msg: &str) {
        let text_buffer = self.text_view.get_buffer().unwrap();
        let mut end_iter = text_buffer.get_end_iter();
        text_buffer.insert_markup(&mut end_iter, &format!("[client error] {}\n", msg));
    }

    pub(crate) fn clear_nicks(&self) {
        // TODO
        // unimplemented!()
    }

    pub(crate) fn set_nick(&self, new_nick: &str) {
        // TODO
        // unimplemented!()
    }

    pub(crate) fn add_privmsg(
        &self,
        sender: &str,
        msg: &str,
        ts: Tm,
        highlight: bool,
        is_action: bool,
    ) {
        let text_buffer = self.text_view.get_buffer().unwrap();
        let mut end_iter = text_buffer.get_end_iter();
        text_buffer.insert_markup(&mut end_iter, &format!("<{}> {}\n", sender, msg));
    }

    pub(crate) fn join(&self, nick: &str, ts: Option<Tm>) {
        // TODO
        // unimplemented!()
    }

    pub(crate) fn part(&self, nick: &str, ts: Option<Tm>) {
        // TODO
        // unimplemented!()
    }

    pub(crate) fn show_topic(&self, topic: &str, ts: Tm) {
        // TODO
        // unimplemented!()
    }
}

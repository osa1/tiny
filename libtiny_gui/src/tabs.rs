//! A widget with messaging tabs (a GTK Notebook).

use gtk::prelude::*;
use libtiny_ui::*;
use time::Tm;
use tokio::sync::mpsc;

use crate::messaging::MessagingUI;
use crate::MsgTargetOwned;

pub(crate) struct Tabs {
    notebook: gtk::Notebook,
    snd_ev: mpsc::Sender<Event>,
    tabs: Vec<Tab>,
    active_idx: usize,
}

struct Tab {
    widget: MessagingUI,
    src: MsgSource,
}

impl Tabs {
    pub(crate) fn new(snd_ev: mpsc::Sender<Event>) -> Tabs {
        let notebook = gtk::Notebook::new();
        notebook.set_tab_pos(gtk::PositionType::Bottom);

        Tabs {
            notebook,
            snd_ev,
            tabs: vec![],
            active_idx: 0, // TODO: Incorrect
        }
    }

    pub(crate) fn get_widget(&self) -> &gtk::Widget {
        self.notebook.upcast_ref()
    }
}

//
// UI protocol methods
//

impl Tabs {
    pub(crate) fn new_server_tab(&mut self, serv: String) -> Option<usize> {
        unimplemented!()
    }

    pub(crate) fn close_server_tab(&mut self, serv: &str) {
        unimplemented!()
    }

    pub(crate) fn new_chan_tab(&mut self, serv: String, chan: String) -> Option<usize> {
        unimplemented!()
    }

    pub(crate) fn close_chan_tab(&mut self, serv: &str, chan: &str) {
        unimplemented!()
    }

    pub(crate) fn new_user_tab(&mut self, serv: String, nick: String) -> Option<usize> {
        unimplemented!()
    }

    pub(crate) fn close_user_tab(&mut self, serv: &str, nick: &str) {
        unimplemented!()
    }

    pub(crate) fn add_client_msg(&self, msg: String, target: MsgTargetOwned) {
        unimplemented!()
    }

    pub(crate) fn add_msg(&self, msg: String, target: MsgTargetOwned) {
        unimplemented!()
    }

    pub(crate) fn add_err_msg(&self, msg: String, ts: Tm, target: MsgTargetOwned) {
        unimplemented!()
    }

    pub(crate) fn add_client_err_msg(&self, msg: String, target: MsgTargetOwned) {
        unimplemented!()
    }

    pub(crate) fn clear_nicks(&self, serv: String) {
        unimplemented!()
    }

    pub(crate) fn set_nick(&self, serv: String, nick: String) {
        unimplemented!()
    }

    pub(crate) fn add_privmsg(
        &self,
        sender: String,
        msg: String,
        ts: Tm,
        target: MsgTargetOwned,
        highlight: bool,
        is_action: bool,
    ) {
        unimplemented!()
    }

    pub(crate) fn add_nick(&self, nick: String, ts: Option<Tm>, target: MsgTargetOwned) {
        unimplemented!()
    }

    pub(crate) fn remove_nick(&self, nick: String, ts: Option<Tm>, target: MsgTargetOwned) {
        unimplemented!()
    }

    pub(crate) fn rename_nick(
        &self,
        old_nick: String,
        new_nick: String,
        ts: Tm,
        target: MsgTargetOwned,
    ) {
        unimplemented!()
    }

    pub(crate) fn set_topic(&self, topic: String, ts: Tm, serv: String, chan: String) {
        unimplemented!()
    }

    pub(crate) fn set_tab_style(&self, style: TabStyle, target: MsgTargetOwned) {
        unimplemented!()
    }
}

//
// Helpers
//

impl Tabs {
    fn apply_to_target<F>(&mut self, target: &MsgTarget, f: &F)
    where
        F: Fn(&mut Tab, bool),
    {
        // Creating a vector just to make borrow checker happy (I can't have a Vec<&mut Tab>)
        // I need to collect tabs here because of the "create if not exists" logic.
        // (see `target_idxs.is_empty()` below)
        let mut target_idxs: Vec<usize> = Vec::with_capacity(1);

        match *target {
            MsgTarget::Server { serv } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if let MsgSource::Serv { serv: ref serv_ } = tab.src {
                        if serv == serv_ {
                            target_idxs.push(tab_idx);
                            break;
                        }
                    }
                }
            }

            MsgTarget::Chan { serv, chan } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if let MsgSource::Chan {
                        serv: ref serv_,
                        chan: ref chan_,
                    } = tab.src
                    {
                        if serv == serv_ && chan == chan_ {
                            target_idxs.push(tab_idx);
                            break;
                        }
                    }
                }
            }

            MsgTarget::User { serv, nick } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if let MsgSource::User {
                        serv: ref serv_,
                        nick: ref nick_,
                    } = tab.src
                    {
                        if serv == serv_ && nick == nick_ {
                            target_idxs.push(tab_idx);
                            break;
                        }
                    }
                }
            }

            MsgTarget::AllServTabs { serv } => {
                for (tab_idx, tab) in self.tabs.iter().enumerate() {
                    if tab.src.serv_name() == serv {
                        target_idxs.push(tab_idx);
                    }
                }
            }

            MsgTarget::CurrentTab => {
                target_idxs.push(self.active_idx);
            }
        }

        // Create server/chan/user tab when necessary
        if target_idxs.is_empty() {
            if let Some(idx) = self.maybe_create_tab(target) {
                target_idxs.push(idx);
            }
        }

        for tab_idx in target_idxs {
            f(&mut self.tabs[tab_idx], self.active_idx == tab_idx);
        }
    }

    fn maybe_create_tab(&mut self, target: &MsgTarget) -> Option<usize> {
        match *target {
            MsgTarget::Server { serv } | MsgTarget::AllServTabs { serv } => {
                self.new_server_tab(serv.to_owned())
            }
            MsgTarget::Chan { serv, chan } => self.new_chan_tab(serv.to_owned(), chan.to_owned()),
            MsgTarget::User { serv, nick } => self.new_user_tab(serv.to_owned(), nick.to_owned()),
            _ => None,
        }
    }
}

// TODO: Duplicate from libtiny_client::utils
fn find_idx<A, F: Fn(&A) -> bool>(slice: &[A], f: F) -> Option<usize> {
    for (idx, a) in slice.iter().enumerate() {
        if f(a) {
            return Some(idx);
        }
    }
    None
}

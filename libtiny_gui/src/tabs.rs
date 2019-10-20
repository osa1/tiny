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
        match self.find_serv_tab_idx(&serv) {
            None => {
                let tab_idx = self.tabs.len();
                self.new_tab(
                    tab_idx,
                    MsgSource::Serv {
                        serv: serv.to_owned(),
                    },
                );
                Some(tab_idx)
            }
            Some(_) => None,
        }
    }

    pub(crate) fn close_server_tab(&mut self, serv: &str) {
        if let Some(tab_idx) = self.find_serv_tab_idx(serv) {
            self.tabs.retain(|tab: &Tab| tab.src.serv_name() != serv);
            if self.active_idx() == tab_idx {
                self.select_tab(if tab_idx == 0 { 0 } else { tab_idx - 1 });
            }
        }
    }

    pub(crate) fn new_chan_tab(&mut self, serv: String, chan: String) -> Option<usize> {
        match self.find_chan_tab_idx(&serv, &chan) {
            None => match self.find_last_serv_tab_idx(&serv) {
                None => {
                    self.new_server_tab(serv.clone());
                    self.new_chan_tab(serv, chan)
                }
                Some(serv_tab_idx) => {
                    let tab_idx = serv_tab_idx + 1;
                    self.new_tab(
                        tab_idx,
                        MsgSource::Chan {
                            serv: serv.to_owned(),
                            chan: chan.to_owned(),
                        },
                    );
                    // TODO: I think this is not necessary as notebook handles this case?
                    // if self.active_idx() >= tab_idx {
                    //     self.next_tab();
                    // }
                    // FIXME
                    // if let Some(nick) = self.tabs[serv_tab_idx].widget.get_nick().map(str::to_owned)
                    // {
                    //     self.tabs[tab_idx].widget.set_nick(nick);
                    // }
                    Some(tab_idx)
                }
            },
            Some(_) => None,
        }
    }

    pub(crate) fn close_chan_tab(&mut self, serv: &str, chan: &str) {
        if let Some(tab_idx) = self.find_chan_tab_idx(serv, chan) {
            self.tabs.remove(tab_idx);
            if self.active_idx() == tab_idx {
                self.select_tab(if tab_idx == 0 { 0 } else { tab_idx - 1 });
            }
        }
    }

    pub(crate) fn new_user_tab(&mut self, serv: String, nick: String) -> Option<usize> {
        match self.find_user_tab_idx(&serv, &nick) {
            None => match self.find_last_serv_tab_idx(&serv) {
                None => {
                    self.new_server_tab(serv.clone());
                    self.new_user_tab(serv, nick)
                }
                Some(tab_idx) => {
                    self.new_tab(
                        tab_idx + 1,
                        MsgSource::User {
                            serv: serv.to_owned(),
                            nick: nick.to_owned(),
                        },
                    );
                    // FIXME
                    // if let Some(nick) = self.tabs[tab_idx].widget.get_nick().map(str::to_owned) {
                    //     self.tabs[tab_idx + 1].widget.set_nick(nick);
                    // }
                    // TODO No need for this as we don't show nick lists in user tabs?
                    //self.tabs[tab_idx + 1].widget.join(nick, None);
                    Some(tab_idx + 1)
                }
            },
            Some(_) => None,
        }
    }

    pub(crate) fn close_user_tab(&mut self, serv: &str, nick: &str) {
        if let Some(tab_idx) = self.find_user_tab_idx(serv, nick) {
            self.tabs.remove(tab_idx);
            // TODO
            // if self.active_idx == tab_idx {
            //     self.select_tab(if tab_idx == 0 { 0 } else { tab_idx - 1 });
            // }
        }
    }

    pub(crate) fn add_client_msg(&mut self, msg: String, target: MsgTargetOwned) {
        let target = target.borrow();
        self.apply_to_target(&target, &|tab, _| {
            tab.widget.add_client_msg(&msg);
        });
    }

    pub(crate) fn add_msg(&mut self, msg: String, ts: Tm, target: MsgTargetOwned) {
        let target = target.borrow();
        self.apply_to_target(&target, &|tab, _| {
            tab.widget.add_msg(&msg, ts);
        });
    }

    pub(crate) fn add_err_msg(&mut self, msg: String, ts: Tm, target: MsgTargetOwned) {
        let target = target.borrow();
        self.apply_to_target(&target, &|tab, _| {
            tab.widget.add_err_msg(&msg, ts);
        });
    }

    pub(crate) fn add_client_err_msg(&mut self, msg: String, target: MsgTargetOwned) {
        let target = target.borrow();
        self.apply_to_target(&target, &|tab, _| {
            tab.widget.add_client_err_msg(&msg);
        });
    }

    pub(crate) fn clear_nicks(&mut self, serv: String) {
        let target = MsgTarget::AllServTabs { serv: &serv };
        self.apply_to_target(&target, &|tab: &mut Tab, _| {
            tab.widget.clear_nicks();
        });
    }

    pub(crate) fn set_nick(&mut self, serv: String, new_nick: String) {
        let target = MsgTarget::AllServTabs { serv: &serv };
        self.apply_to_target(&target, &|tab, _| {
            tab.widget.set_nick(&new_nick)
        });
    }

    pub(crate) fn add_privmsg(
        &mut self,
        sender: String,
        msg: String,
        ts: Tm,
        target: MsgTargetOwned,
        highlight: bool,
        is_action: bool,
    ) {
        let target = target.borrow();
        self.apply_to_target(&target, &|tab, _| {
            tab.widget.add_privmsg(&sender, &msg, ts, highlight, is_action);
            // TODO
            // let nick = tab.widget.get_nick();
            // if let Some(nick_) = nick {
            //     tab.notifier
            //         .notify_privmsg(sender, msg, target, nick_, highlight);
            // }
        });
    }

    pub(crate) fn add_nick(&mut self, nick: String, ts: Option<Tm>, target: MsgTargetOwned) {
        let target = target.borrow();
        self.apply_to_target(&target, &|tab, _| {
            tab.widget.join(&nick, ts);
        });
    }

    pub(crate) fn remove_nick(&mut self, nick: String, ts: Option<Tm>, target: MsgTargetOwned) {
        let target = target.borrow();
        self.apply_to_target(&target, &|tab, _| {
            tab.widget.part(&nick, ts);
        });
    }

    pub(crate) fn rename_nick(
        &mut self,
        old_nick: String,
        new_nick: String,
        ts: Tm,
        target: MsgTargetOwned,
    ) {
        unimplemented!()
    }

    pub(crate) fn set_topic(&mut self, topic: String, ts: Tm, serv: String, chan: String) {
        let target = MsgTarget::Chan { serv: &serv, chan: &chan };
        self.apply_to_target(&target, &|tab, _| {
            tab.widget.show_topic(&topic, ts);
        });
    }

    pub(crate) fn set_tab_style(&mut self, style: TabStyle, target: MsgTargetOwned) {
        // TODO
        // unimplemented!()
    }
}

//
// Helpers
//

impl Tabs {
    fn new_tab(&mut self, idx: usize, src: MsgSource) {
        let src_ = src.clone();
        self.tabs.insert(
            idx,
            Tab {
                widget: MessagingUI::new(src_, self.snd_ev.clone()),
                src,
            },
        );
    }

    fn select_tab(&mut self, tab_idx: usize) {
        self.notebook.set_current_page(Some(tab_idx as u32));
        // TODO
        // self.tabs[self.active_idx].set_style(TabStyle::Normal);
    }

    fn active_idx(&self) -> usize {
        self.notebook.get_current_page().unwrap() as usize
    }

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
                target_idxs.push(self.active_idx());
            }
        }

        // Create server/chan/user tab when necessary
        if target_idxs.is_empty() {
            if let Some(idx) = self.maybe_create_tab(target) {
                target_idxs.push(idx);
            }
        }

        let active_idx = self.active_idx();
        for tab_idx in target_idxs {
            f(&mut self.tabs[tab_idx], active_idx == tab_idx);
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

    fn find_serv_tab_idx(&self, serv_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if let MsgSource::Serv { ref serv } = tab.src {
                if serv_ == serv {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    fn find_chan_tab_idx(&self, serv_: &str, chan_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if let MsgSource::Chan { ref serv, ref chan } = tab.src {
                if serv_ == serv && chan_ == chan {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    fn find_user_tab_idx(&self, serv_: &str, nick_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if let MsgSource::User { ref serv, ref nick } = tab.src {
                if serv_ == serv && nick_ == nick {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    /// Index of the last tab with the given server name.
    fn find_last_serv_tab_idx(&self, serv: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate().rev() {
            if tab.src.serv_name() == serv {
                return Some(tab_idx);
            }
        }
        None
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

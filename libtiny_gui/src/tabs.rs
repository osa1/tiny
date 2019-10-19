//! A widget with messaging tabs (a GTK Notebook).

use gio::prelude::*;
use gtk::prelude::*;
use libtiny_ui::*;
use tokio::sync::mpsc;

use crate::messaging::MessagingUI;

#[derive(Clone)]
pub(crate) struct Tabs {
    notebook: gtk::Notebook,
    snd_ev: mpsc::Sender<Event>,
    tab_srcs: Vec<MsgSource>,
}

impl Tabs {
    pub(crate) fn new(snd_ev: mpsc::Sender<Event>) -> Tabs {
        let notebook = gtk::Notebook::new();
        notebook.set_tab_pos(gtk::PositionType::Bottom);

        Tabs {
            notebook,
            snd_ev,
            tab_srcs: vec![],
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
    pub(crate) fn new_server_tab(&mut self, serv: String) {
        let label = gtk::Label::new(Some(&serv));
        let msg_src = MsgSource::Serv { serv };
        self.tab_srcs.push(msg_src.clone());
        let tab = MessagingUI::new(msg_src, self.snd_ev.clone());
        self.notebook.append_page(tab.get_widget(), Some(&label));
        self.notebook.show_all();
    }

    pub(crate) fn close_server_tab(&mut self, serv: &str) {
        // TODO: Close all tabs of this server
        while let Some(idx) = find_idx(&mut self.tab_srcs, |src| src.serv_name() == serv) {
            self.tab_srcs.remove(idx);
            self.notebook.remove_page(Some(idx as u32));
        }
    }

    pub(crate) fn new_chan_tab(&mut self, serv: String, chan: String) {
        match find_idx(&self.tab_srcs, |src| src.serv_name() == &serv) {
            None => {
                debug!("Can't find server tab for server {}", serv);
                debug!("Tabs: {:?}", self.tab_srcs);
            }
            Some(serv_tab_idx) => {
                // Insert after the last channel tab for this server
                let insert_idx = match find_idx(&self.tab_srcs[serv_tab_idx + 1..], |src| match src
                {
                    MsgSource::Serv { .. } | MsgSource::User { .. } => true,
                    MsgSource::Chan { .. } => false,
                }) {
                    None => self.tab_srcs.len(),
                    Some(idx) => idx + serv_tab_idx + 1,
                };
                let label = gtk::Label::new(Some(&chan));
                let msg_src = MsgSource::Chan { serv, chan };
                let tab = MessagingUI::new(msg_src, self.snd_ev.clone());
                println!("insert idx: {}", insert_idx);
                self.notebook
                    .insert_page(tab.get_widget(), Some(&label), Some(insert_idx as u32));
                self.notebook.show_all();
            }
        }
    }

    pub(crate) fn close_chan_tab(&mut self, serv: &str, chan: &str) {
        match find_idx(&self.tab_srcs, |src| match src {
            MsgSource::Chan {
                serv: ref serv_,
                chan: ref chan_,
            } => serv_ == serv && chan_ == chan,
            _ => false,
        }) {
            None => {
                debug!("Can't find {} tab for server {}", chan, serv);
                debug!("Tabs: {:?}", self.tab_srcs);
            }
            Some(idx) => {
                self.tab_srcs.remove(idx);
                self.notebook.remove_page(Some(idx as u32));
            }
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

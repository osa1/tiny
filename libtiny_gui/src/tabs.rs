//! A widget with messaging tabs (a GTK Notebook).

use gtk::prelude::*;
use libtiny_ui::*;
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
    pub(crate) fn new_server_tab(&mut self, serv: String) {
        let label = gtk::Label::new(Some(&serv));
        let src = MsgSource::Serv { serv };
        let widget = MessagingUI::new(src.clone(), self.snd_ev.clone());
        self.notebook.append_page(widget.get_widget(), Some(&label));
        let tab = Tab { widget, src };
        self.tabs.push(tab);
        self.notebook.show_all();
    }

    pub(crate) fn close_server_tab(&mut self, serv: &str) {
        // TODO: Close all tabs of this server
        while let Some(idx) = find_idx(&self.tabs, |tab| tab.src.serv_name() == serv) {
            self.tabs.remove(idx);
            self.notebook.remove_page(Some(idx as u32));
        }
    }

    pub(crate) fn new_chan_tab(&mut self, serv: String, chan: String) {
        match find_idx(&self.tabs, |tab| tab.src.serv_name() == serv) {
            None => {
                debug!("Can't find server tab for server {}", serv);
                // debug!("Tabs: {:?}", self.tabs);
            }
            Some(serv_tab_idx) => {
                // Insert after the last channel tab for this server
                let insert_idx =
                    match find_idx(&self.tabs[serv_tab_idx + 1..], |tab| match tab.src {
                        MsgSource::Serv { .. } | MsgSource::User { .. } => true,
                        MsgSource::Chan { .. } => false,
                    }) {
                        None => self.tabs.len(),
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
        match find_idx(&self.tabs, |tab| match tab.src {
            MsgSource::Chan {
                serv: ref serv_,
                chan: ref chan_,
            } => serv_ == serv && chan_ == chan,
            _ => false,
        }) {
            None => {
                debug!("Can't find {} tab for server {}", chan, serv);
                // debug!("Tabs: {:?}", self.tab_srcs);
            }
            Some(idx) => {
                self.tabs.remove(idx);
                self.notebook.remove_page(Some(idx as u32));
            }
        }
    }

    pub(crate) fn add_client_msg(&self, msg: String, target: MsgTargetOwned) {}
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

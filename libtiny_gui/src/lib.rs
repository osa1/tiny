#![warn(unreachable_pub)]

mod messaging;
mod tabs;

use gio::prelude::*;
use gtk::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use time::Tm;
use tokio::sync::mpsc;

use tabs::Tabs;
pub use libtiny_ui::*;

#[macro_use]
extern crate log;

#[derive(Clone)]
pub struct GUI {
    /// Channel to send commands to the GUI, which is running in another thread.
    snd_cmd: glib::Sender<GUICmd>,
}

#[derive(Debug)]
enum MsgTargetOwned {
    Server { serv: String },
    Chan { serv: String, chan: String },
    User { serv: String, nick: String },
    AllServTabs { serv: String },
    CurrentTab,
}

impl MsgTargetOwned {
    fn from(t: &MsgTarget) -> MsgTargetOwned {
        use MsgTargetOwned::*;
        match t {
            MsgTarget::Server { serv } => Server {
                serv: serv.to_string(),
            },
            MsgTarget::Chan { serv, chan } => Chan {
                serv: serv.to_string(),
                chan: chan.to_string(),
            },
            MsgTarget::User { serv, nick } => User {
                serv: serv.to_string(),
                nick: nick.to_string(),
            },
            MsgTarget::AllServTabs { serv } => AllServTabs {
                serv: serv.to_string(),
            },
            MsgTarget::CurrentTab => CurrentTab,
        }
    }

    fn borrow(&self) -> MsgTarget {
        use MsgTargetOwned::*;
        match self {
            Server { ref serv } => MsgTarget::Server { serv },
            Chan { ref serv, ref chan } => MsgTarget::Chan { serv, chan },
            User { ref serv, ref nick } => MsgTarget::User { serv, nick },
            AllServTabs { ref serv } => MsgTarget::AllServTabs { serv },
            CurrentTab => MsgTarget::CurrentTab,
        }
    }
}

#[derive(Debug)]
enum GUICmd {
    NewServerTab {
        serv: String,
    },
    CloseServerTab {
        serv: String,
    },
    NewChanTab {
        serv: String,
        chan: String,
    },
    CloseChanTab {
        serv: String,
        chan: String,
    },
    CloseUserTab {
        serv: String,
        nick: String,
    },
    AddClientMsg {
        msg: String,
        target: MsgTargetOwned,
    },
    AddMsg {
        msg: String,
        ts: Tm,
        target: MsgTargetOwned,
    },
    AddErrMsg {
        msg: String,
        ts: Tm,
        target: MsgTargetOwned,
    },
    AddClientErrMsg {
        msg: String,
        target: MsgTargetOwned,
    },
    ClearNicks {
        serv: String,
    },
    SetNick {
        serv: String,
        nick: String,
    },
    AddPrivmsg {
        sender: String,
        msg: String,
        ts: Tm,
        target: MsgTargetOwned,
        highlight: bool,
        is_action: bool,
    },
    AddNick {
        nick: String,
        ts: Option<Tm>,
        target: MsgTargetOwned,
    },
    RemoveNick {
        nick: String,
        ts: Option<Tm>,
        target: MsgTargetOwned,
    },
    RenameNick {
        old_nick: String,
        new_nick: String,
        ts: Tm,
        target: MsgTargetOwned,
    },
    SetTopic {
        topic: String,
        ts: Tm,
        serv: String,
        chan: String,
    },
    SetTabStyle {
        style: TabStyle,
        target: MsgTargetOwned,
    },
}

impl GUI {
    /// Runs a GUI in a new thread.
    pub fn run() -> (GUI, mpsc::Receiver<Event>) {
        let (snd_cmd, rcv_cmd) = glib::MainContext::channel::<GUICmd>(glib::PRIORITY_DEFAULT);
        let (snd_ev, rcv_ev) = mpsc::channel::<Event>(1000);
        thread::spawn(move || run_gui(rcv_cmd, snd_ev));
        (GUI { snd_cmd }, rcv_ev)
    }
}

fn run_gui(rcv_cmd: glib::Receiver<GUICmd>, snd_ev: mpsc::Sender<Event>) {
    let application = gtk::Application::new(Some("com.github.osa1.tiny"), Default::default())
        .expect("Initialization failed...");

    // Hack to be able to move the channel to build_ui
    // Easy to move snd_ev as it's Clone
    let rcv_cmd = Rc::new(RefCell::new(Some(rcv_cmd)));
    application.connect_activate(move |app| {
        build_ui(app, rcv_cmd.clone(), snd_ev.clone());
    });

    application.run(&std::env::args().collect::<Vec<_>>());
}

fn build_ui(
    application: &gtk::Application,
    rcv_cmd: Rc<RefCell<Option<glib::Receiver<GUICmd>>>>,
    snd_ev: mpsc::Sender<Event>,
) {
    let mut tabs = Tabs::new(snd_ev);
    tabs.new_server_tab("mentions".to_string());

    let window = gtk::ApplicationWindow::new(application);

    window.set_title("tiny");
    window.set_decorated(false);
    window.set_default_size(200, 200);
    window.add(tabs.get_widget());
    window.show_all();

    use GUICmd::*;
    rcv_cmd
        .borrow_mut()
        .take()
        .unwrap()
        .attach(None, move |cmd| {
            println!("GUI thread got cmd: {:?}", cmd);
            match cmd {
                NewServerTab { serv } => {
                    tabs.new_server_tab(serv);
                }
                CloseServerTab { ref serv } => {
                    tabs.close_server_tab(serv);
                }
                NewChanTab { serv, chan } => {
                    tabs.new_chan_tab(serv, chan);
                }
                CloseChanTab { ref serv, ref chan } => {
                    tabs.close_chan_tab(serv, chan);
                }
                CloseUserTab { ref serv, ref nick } => {
                    tabs.close_user_tab(serv, nick);
                }
                AddClientMsg { msg, target } => {
                    tabs.add_client_msg(msg, target);
                }
                AddMsg { msg, ts, target } => {
                    tabs.add_msg(msg, ts, target);
                }
                AddErrMsg { msg, ts, target } => {
                    tabs.add_err_msg(msg, ts, target);
                }
                AddClientErrMsg { msg, target } => {
                    tabs.add_client_err_msg(msg, target);
                }
                ClearNicks { serv } => {
                    tabs.clear_nicks(serv);
                }
                SetNick { serv, nick } => {
                    tabs.set_nick(serv, nick);
                }
                AddPrivmsg {
                    sender,
                    msg,
                    ts,
                    target,
                    highlight,
                    is_action,
                } => {
                    tabs.add_privmsg(sender, msg, ts, target, highlight, is_action);
                }
                AddNick { nick, ts, target } => {
                    tabs.add_nick(nick, ts, target);
                }
                RemoveNick { nick, ts, target } => {
                    tabs.remove_nick(nick, ts, target);
                }
                RenameNick {
                    old_nick,
                    new_nick,
                    ts,
                    target,
                } => {
                    tabs.rename_nick(old_nick, new_nick, ts, target);
                }
                SetTopic {
                    topic,
                    ts,
                    serv,
                    chan,
                } => {
                    tabs.set_topic(topic, ts, serv, chan);
                }
                SetTabStyle { style, target } => {
                    tabs.set_tab_style(style, target);
                }
            }
            glib::Continue(true)
        });
}

//
// Implement UI API
//

use GUICmd::*;

impl UI for GUI {
    fn draw(&self) {}

    fn new_server_tab(&self, serv: &str) {
        self.snd_cmd
            .send(NewServerTab {
                serv: serv.to_owned(),
            })
            .unwrap();
    }

    fn close_server_tab(&self, serv: &str) {
        self.snd_cmd
            .send(CloseServerTab {
                serv: serv.to_owned(),
            })
            .unwrap();
    }

    fn new_chan_tab(&self, serv: &str, chan: &str) {
        self.snd_cmd
            .send(NewChanTab {
                serv: serv.to_owned(),
                chan: chan.to_owned(),
            })
            .unwrap()
    }

    fn close_chan_tab(&self, serv: &str, chan: &str) {
        self.snd_cmd
            .send(CloseChanTab {
                serv: serv.to_owned(),
                chan: chan.to_owned(),
            })
            .unwrap()
    }

    fn close_user_tab(&self, serv: &str, nick: &str) {
        self.snd_cmd
            .send(CloseUserTab {
                serv: serv.to_owned(),
                nick: nick.to_owned(),
            })
            .unwrap()
    }

    fn add_client_msg(&self, msg: &str, target: &MsgTarget) {
        self.snd_cmd
            .send(AddClientMsg {
                msg: msg.to_owned(),
                target: MsgTargetOwned::from(target),
            })
            .unwrap();
    }

    fn add_msg(&self, msg: &str, ts: Tm, target: &MsgTarget) {
        self.snd_cmd
            .send(AddMsg {
                msg: msg.to_owned(),
                ts,
                target: MsgTargetOwned::from(target),
            })
            .unwrap();
    }

    fn add_err_msg(&self, msg: &str, ts: Tm, target: &MsgTarget) {
        self.snd_cmd
            .send(AddErrMsg {
                msg: msg.to_owned(),
                ts,
                target: MsgTargetOwned::from(target),
            })
            .unwrap();
    }

    fn add_client_err_msg(&self, msg: &str, target: &MsgTarget) {
        self.snd_cmd
            .send(AddClientErrMsg {
                msg: msg.to_owned(),
                target: MsgTargetOwned::from(target),
            })
            .unwrap();
    }

    fn clear_nicks(&self, serv: &str) {
        self.snd_cmd
            .send(ClearNicks {
                serv: serv.to_owned(),
            })
            .unwrap();
    }

    fn set_nick(&self, serv: &str, nick: &str) {
        self.snd_cmd
            .send(SetNick {
                serv: serv.to_owned(),
                nick: nick.to_owned(),
            })
            .unwrap();
    }

    fn add_privmsg(
        &self,
        sender: &str,
        msg: &str,
        ts: Tm,
        target: &MsgTarget,
        highlight: bool,
        is_action: bool,
    ) {
        self.snd_cmd
            .send(AddPrivmsg {
                sender: sender.to_owned(),
                msg: msg.to_owned(),
                ts,
                target: MsgTargetOwned::from(target),
                highlight,
                is_action,
            })
            .unwrap();
    }

    fn add_nick(&self, nick: &str, ts: Option<Tm>, target: &MsgTarget) {
        self.snd_cmd
            .send(AddNick {
                nick: nick.to_owned(),
                ts,
                target: MsgTargetOwned::from(target),
            })
            .unwrap();
    }

    fn remove_nick(&self, nick: &str, ts: Option<Tm>, target: &MsgTarget) {
        self.snd_cmd
            .send(RemoveNick {
                nick: nick.to_owned(),
                ts,
                target: MsgTargetOwned::from(target),
            })
            .unwrap();
    }

    fn rename_nick(&self, old_nick: &str, new_nick: &str, ts: Tm, target: &MsgTarget) {
        self.snd_cmd
            .send(RenameNick {
                old_nick: old_nick.to_owned(),
                new_nick: new_nick.to_owned(),
                ts,
                target: MsgTargetOwned::from(target),
            })
            .unwrap();
    }

    fn set_topic(&self, topic: &str, ts: Tm, serv: &str, chan: &str) {
        self.snd_cmd
            .send(SetTopic {
                topic: topic.to_owned(),
                ts,
                serv: serv.to_owned(),
                chan: chan.to_owned(),
            })
            .unwrap();
    }

    fn set_tab_style(&self, style: TabStyle, target: &MsgTarget) {
        self.snd_cmd
            .send(SetTabStyle {
                style,
                target: MsgTargetOwned::from(target),
            })
            .unwrap();
    }

    fn user_tab_exists(&self, _serv: &str, _nick: &str) -> bool {
        // FIXME: This part of the API will need to change
        false
    }
}

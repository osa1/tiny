#![warn(unreachable_pub)]

mod messaging;
mod tabs;

use gio::prelude::*;
use gtk::prelude::*;
use libtiny_ui::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use tokio::sync::mpsc;

use messaging::MessagingUI;
use tabs::Tabs;

pub struct GUI {
    /// Channel to send commands to the GUI, which is running in another thread.
    snd_cmd: glib::Sender<GUICmd>,
}

enum GUICmd {
    Foo,
}

impl GUI {
    /// Runs a GUI in a new thread.
    pub fn run() -> (GUI, mpsc::Receiver<Event>) {
        let (snd_cmd, rcv_cmd) = glib::MainContext::channel::<GUICmd>(glib::PRIORITY_DEFAULT);
        let (snd_ev, rcv_ev) = mpsc::channel::<Event>(10);
        thread::spawn(move || run_gui(rcv_cmd, snd_ev));
        (GUI { snd_cmd }, rcv_ev)
    }
}

fn run_gui(rcv_cmd: glib::Receiver<GUICmd>, snd_ev: mpsc::Sender<Event>) {
    let application = gtk::Application::new(Some("com.github.osa1.tiny"), Default::default())
        .expect("Initialization failed...");

    // Hack to be able to move the channel to build_ui
    let rcv_cmd = Rc::new(RefCell::new(Some(rcv_cmd)));
    application.connect_activate(move |app| {
        build_ui(app, rcv_cmd.clone());
    });

    application.run(&std::env::args().collect::<Vec<_>>());
}

fn build_ui(application: &gtk::Application, rcv_cmd: Rc<RefCell<Option<glib::Receiver<GUICmd>>>>) {
    let tabs = Tabs::new();
    tabs.new_server_tab("mentions");

    let window = gtk::ApplicationWindow::new(application);

    window.set_title("gig");
    window.set_decorated(false);
    window.set_default_size(200, 200);
    window.add(tabs.get_widget());
    window.show_all();

    rcv_cmd
        .borrow_mut()
        .take()
        .unwrap()
        .attach(None, move |cmd| {
            match cmd {
                GUICmd::Foo => {
                    println!("Got Foo cmd");
                }
            }
            glib::Continue(true)
        });
}

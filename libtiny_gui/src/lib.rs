use gio::prelude::*;
use gtk::prelude::*;

pub fn main() {
    let application = gtk::Application::new(Some("com.github.osa1.tiny"), Default::default())
        .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&std::env::args().collect::<Vec<_>>());
}

fn build_ui(application: &gtk::Application) {
    let notebook = gtk::Notebook::new();
    notebook.set_tab_pos(gtk::PositionType::Bottom);

    let test = gtk::Label::new(Some("just testing"));

    notebook.append_page(&test, None::<&gtk::Widget>);

    let window = gtk::ApplicationWindow::new(application);

    window.set_title("gig");
    window.set_decorated(false);
    window.set_default_size(200, 200);
    window.add(&notebook);
    window.show_all();
}

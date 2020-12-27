use fs::remove_dir_all;

use super::*;

static TEST_LOG_DIR_PREFIX: &str = "./test_logs_";
fn init(test_name: &str) -> Logger {
    let test_path = PathBuf::from(TEST_LOG_DIR_PREFIX.to_owned() + test_name);
    Logger::new(test_path, Box::new(|_| {})).unwrap()
}

fn cleanup(test_name: &str) {
    remove_dir_all(TEST_LOG_DIR_PREFIX.to_owned() + test_name).unwrap();
}

fn server_file_exists(logger: &LoggerInner, server_name: &str) -> bool {
    logger.servers.get(server_name).unwrap().fd.is_some()
}

fn chan_file_exists(logger: &LoggerInner, server_name: &str, chan_name: &ChanNameRef) -> bool {
    logger
        .servers
        .get(server_name)
        .unwrap()
        .chans
        .get(chan_name)
        .unwrap()
        .file
        .is_some()
}

fn user_file_exists(logger: &LoggerInner, server_name: &str, user_name: &str) -> bool {
    logger
        .servers
        .get(server_name)
        .unwrap()
        .users
        .get(user_name)
        .unwrap()
        .file
        .is_some()
}

#[test]
fn with_dir_only() {
    let logger = init("with_dir_only");
    let server_name = "server";
    let chan_name = ChanNameRef::new("#test");
    let user_name = "irc_user";

    logger.new_server_tab(server_name);
    logger.new_chan_tab(server_name, chan_name);
    logger.add_msg(
        "hi",
        time::now(),
        &MsgTarget::User {
            serv: server_name,
            nick: user_name,
        },
    );

    let logger = logger.inner.borrow();
    let server_file = server_file_exists(&logger, server_name);
    let chan_file = chan_file_exists(&logger, server_name, chan_name);
    let user_file = user_file_exists(&logger, server_name, user_name);

    assert!(server_file);
    assert!(chan_file);
    assert!(user_file);

    cleanup("with_dir_only");
}

#[test]
fn with_server_disabled_chans_enabled() {
    let logger = init("with_server_disabled_chans_enabled");
    let server_name = "server";
    let chan_name = ChanNameRef::new("#test");
    // disable channels only
    logger.add_server_config(server_name, Some(false), Some(true), None);
    logger.new_server_tab(server_name);
    logger.new_chan_tab(server_name, chan_name);

    let logger = logger.inner.borrow();
    let server_file = server_file_exists(&logger, server_name);
    let chan_file = chan_file_exists(&logger, server_name, chan_name);

    assert!(!server_file);
    assert!(chan_file);

    cleanup("with_server_disabled_chans_enabled")
}

#[test]
fn with_specific_chan_enabled_all_disabled() {
    let logger = init("with_specific_chan_enabled_all_disabled");
    let server_name = "server";
    let chan_name = ChanNameRef::new("#test");
    let chan_name2 = ChanNameRef::new("#spam");
    // defaults
    logger.add_server_config(server_name, Some(false), Some(false), Some(false));
    // enabled chan
    logger.add_chan_config(server_name, chan_name.display(), Some(true));
    // disabled by default
    logger.add_chan_config(server_name, chan_name2.display(), None);

    logger.new_server_tab(server_name);
    logger.new_chan_tab(server_name, chan_name);

    let logger = logger.inner.borrow();

    let server_file = server_file_exists(&logger, server_name);
    let chan_file = chan_file_exists(&logger, server_name, chan_name);
    let spam_chan_file = chan_file_exists(&logger, server_name, chan_name2);

    assert!(!server_file);
    assert!(chan_file);
    assert!(!spam_chan_file);

    cleanup("with_specific_chan_enabled_all_disabled")
}

#[test]
fn with_specific_chan_disabled() {
    let logger = init("with_specific_chan_disabled");
    let server_name = "server";
    let chan_name = ChanNameRef::new("#test");
    // defaults
    logger.add_server_config(server_name, None, None, None);
    // disable chan
    logger.add_chan_config(server_name, chan_name.display(), Some(false));
    logger.new_server_tab(server_name);
    logger.new_chan_tab(server_name, chan_name);

    let logger = logger.inner.borrow();

    let server_file = server_file_exists(&logger, server_name);
    let chan_file = chan_file_exists(&logger, server_name, chan_name);

    assert!(server_file);
    assert!(!chan_file);

    cleanup("with_specific_chan_disabled")
}

#[test]
fn with_user_tabs_disabled() {
    let logger = init("with_user_tabs_disabled");
    let server_name = "server";
    let user_name = "irc_user";
    // disable user tabs
    logger.add_server_config(server_name, None, None, Some(false));

    logger.new_server_tab(server_name);
    logger.add_msg(
        "hi",
        time::now(),
        &MsgTarget::User {
            serv: "server",
            nick: user_name,
        },
    );

    let logger = logger.inner.borrow();

    let server_file = server_file_exists(&logger, server_name);
    let user_file = user_file_exists(&logger, server_name, user_name);

    assert!(server_file);
    assert!(!user_file);

    cleanup("with_user_tabs_disabled")
}

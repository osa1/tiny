#![cfg_attr(test, feature(test))]
#![feature(drain_filter)]
#![feature(ptr_offset_from)]
#![allow(clippy::zero_prefixed_literal)]

mod cmd;
mod cmd_line_args;
mod config;
mod conn;
mod ui;
mod utils;

use cmd_line_args::{parse_cmd_line_args, CmdLineArgs};
use libtiny_client::{Client, ServerInfo};
use libtiny_tui::{Colors, MsgTarget, TUI};
use std::path::PathBuf;

fn main() {
    let CmdLineArgs {
        servers: server_args,
        config_path,
    } = parse_cmd_line_args(std::env::args());
    let config_path = config_path.unwrap_or_else(config::get_default_config_path);
    if config_path.is_dir() {
        println!("The config path is a directory.");
        ::std::process::exit(1);
    } else if !config_path.is_file() {
        config::generate_default_config(&config_path);
    } else {
        match config::parse_config(&config_path) {
            Err(yaml_err) => {
                println!("Can't parse config file:");
                println!("{}", yaml_err);
                ::std::process::exit(1);
            }
            Ok(config::Config {
                servers,
                defaults,
                colors,
                log_dir,
            }) => {
                let servers = if !server_args.is_empty() {
                    // connect only to servers that match at least one of
                    // the given patterns
                    servers
                        .into_iter()
                        .filter(|s| {
                            for server in &server_args {
                                if s.addr.contains(server) {
                                    return true;
                                }
                            }
                            false
                        })
                        .collect()
                } else {
                    servers
                };
                run(servers, defaults, colors, config_path, log_dir)
            }
        }
    }
}

fn run(
    servers: Vec<config::Server>,
    defaults: config::Defaults,
    colors: Colors,
    config_path: PathBuf,
    log_dir: Option<PathBuf>,
) {
    // One task for each client to handle IRC events
    // One task for TUI events
    let mut executor = tokio::runtime::current_thread::Runtime::new().unwrap();

    // Create TUI task
    let (tui, rcv_tui_ev) = TUI::run(colors, &mut executor);

    // init "mentions" tab
    tui.new_server_tab("mentions");
    tui.add_client_msg(
        "Any mentions to you will be listed here.",
        &MsgTarget::Server { serv: "mentions" },
    );
    tui.draw();

    let mut clients: Vec<Client> = Vec::with_capacity(servers.len());

    for server in servers.iter().cloned() {
        tui.new_server_tab(&server.addr);

        let server_info = ServerInfo {
            addr: server.addr,
            port: server.port,
            tls: server.tls,
            pass: server.pass,
            hostname: server.hostname,
            realname: server.realname,
            nicks: server.nicks,
            auto_join: server.join,
            nickserv_ident: server.nickserv_ident,
            sasl_auth: server.sasl_auth.map(|auth| libtiny_client::SASLAuth {
                username: auth.username,
                password: auth.password,
            }),
        };

        let (client, rcv_conn_ev) = Client::new(server_info, Some(&mut executor), log_dir.clone());
        let tui_clone = tui.clone();
        let client_clone = client.clone();

        // Spawn a task to handle connection events
        executor.spawn(conn::task(rcv_conn_ev, tui_clone, client_clone));

        clients.push(client);
    }

    // Spawn a task to handle TUI events
    executor.spawn(ui::task(
        config_path,
        log_dir,
        defaults,
        tui,
        clients,
        rcv_tui_ev,
    ));

    executor.run().unwrap(); // unwraps RunError
}

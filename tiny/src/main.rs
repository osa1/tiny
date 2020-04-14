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
use libtiny_logger::Logger;
use libtiny_tui::{MsgTarget, TUI};
use libtiny_ui::UI;
use std::path::PathBuf;

fn main() {
    let CmdLineArgs {
        servers: server_args,
        config_path,
    } = parse_cmd_line_args(std::env::args());
    let config_path = config_path.unwrap_or_else(config::get_config_path);
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
                run(servers, defaults, config_path, log_dir)
            }
        }
    }
}

fn run(
    servers: Vec<config::Server>,
    defaults: config::Defaults,
    config_path: PathBuf,
    log_dir: Option<PathBuf>,
) {
    env_logger::builder()
        .target(env_logger::Target::Stderr)
        .init();

    // One task for each client to handle IRC events
    // One task for TUI events
    let mut runtime = tokio::runtime::Builder::new()
        .basic_scheduler()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();

    local.block_on(&mut runtime, async move {
        // Create TUI task
        let (tui, rcv_tui_ev) = TUI::run(config_path.clone());
        tui.draw();

        // Create logger
        let report_logger_error = {
            let tui_clone = tui.clone();
            Box::new(move |err: String| {
                // Somehwat hacky -- only tab we have is "mentions" so we show the error there
                tui_clone.add_client_err_msg(
                    &format!("Logger error: {}", err),
                    &MsgTarget::Server { serv: "mentions" },
                )
            })
        };
        let logger: Option<Logger> =
            log_dir.and_then(|log_dir| match Logger::new(log_dir, report_logger_error) {
                Err(err) => {
                    tui.add_client_err_msg(
                        &format!("Can't create logger: {}", err),
                        &MsgTarget::CurrentTab,
                    );
                    None
                }
                Ok(logger) => {
                    // Create "mentions" log file manually -- the tab is already created in the TUI so
                    // we won't be creating a "mentions" file in the logger without this.
                    logger.new_server_tab("mentions", None);
                    Some(logger)
                }
            });

        let tui: Box<dyn UI> = match logger {
            None => Box::new(tui) as Box<dyn UI>,
            Some(logger) => Box::new(libtiny_ui::combine(tui, logger)) as Box<dyn UI>,
        };

        let mut clients: Vec<Client> = Vec::with_capacity(servers.len());

        for server in servers.iter().cloned() {
            tui.new_server_tab(&server.addr, server.alias);

            let server_info = ServerInfo {
                addr: server.addr,
                port: server.port,
                tls: server.tls,
                pass: server.pass,
                realname: server.realname,
                nicks: server.nicks,
                auto_join: server.join,
                nickserv_ident: server.nickserv_ident,
                sasl_auth: server.sasl_auth.map(|auth| libtiny_client::SASLAuth {
                    username: auth.username,
                    password: auth.password,
                }),
            };

            let (client, rcv_conn_ev) = Client::new(server_info);
            // TODO: Somehow it's quite hard to expose this objekt call with a different name and less
            // polymorphic type in libtiny_ui ...
            let tui_clone = libtiny_ui::clone_box(&*tui);
            let client_clone = client.clone();

            // Spawn a task to handle connection events
            tokio::task::spawn_local(conn::task(rcv_conn_ev, tui_clone, client_clone));

            clients.push(client);
        }

        // Block on TUI task
        ui::task(config_path, defaults, tui, clients, rcv_tui_ev).await;
    });

    runtime.block_on(local);
}

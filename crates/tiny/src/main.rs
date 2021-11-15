#![allow(clippy::zero_prefixed_literal)]

mod cli;
mod cmd;
mod config;
mod conn;
mod debug_logging;
mod ui;
mod utils;

#[cfg(test)]
mod tests;

use libtiny_client::{Client, SASLAuth, SASLExternal, ServerInfo};
use libtiny_common::{ChanNameRef, MsgTarget};
use libtiny_logger::{Logger, LoggerInitError};
use libtiny_tui::TUI;
use ui::UI;

use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::exit;

#[macro_use]
extern crate log;

fn main() {
    let cli::Args {
        servers: server_args,
        config_path,
    } = cli::parse();
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
                exit(1);
            }
            Ok(config) => {
                let config_errors = config.validate();
                if !config_errors.is_empty() {
                    println!(
                        "Config file error{}:",
                        if config_errors.len() > 1 { "s" } else { "" }
                    );
                    for error in config_errors {
                        println!("- {}", error);
                    }
                    exit(1);
                }

                let config = match config.read_passwords() {
                    None => exit(1),
                    Some(config) => config,
                };

                let config::Config {
                    servers,
                    defaults,
                    log_dir,
                } = config;

                let servers = if !server_args.is_empty() {
                    // Connect only to servers that match at least one of the given patterns
                    servers
                        .into_iter()
                        .filter(|s| server_args.iter().any(|arg| s.addr.contains(arg)))
                        .collect()
                } else {
                    servers
                };
                run(servers, defaults, config_path, log_dir)
            }
        }
    }
}

const DEBUG_LOG_FILE: &str = "tiny_debug_logs.txt";

fn run(
    servers: Vec<config::Server<String>>,
    defaults: config::Defaults,
    config_path: PathBuf,
    log_dir: Option<PathBuf>,
) {
    let debug_log_file = match log_dir.as_ref() {
        Some(log_dir) => {
            let mut log_dir = log_dir.clone();
            log_dir.push(DEBUG_LOG_FILE);
            log_dir
        }
        None => DEBUG_LOG_FILE.into(),
    };
    debug_logging::init(debug_log_file);

    // One task for each client to handle IRC events
    // One task for TUI events
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();

    local.block_on(&runtime, async move {
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
                Err(LoggerInitError::CouldNotCreateDir { dir_path, err }) => {
                    tui.add_client_err_msg(
                        &format!("Could not create log directory {:?}: {}", dir_path, err),
                        &MsgTarget::Server { serv: "mentions" },
                    );
                    tui.draw();
                    None
                }
                Ok(logger) => {
                    // Create "mentions" log file manually -- the tab is already created in the TUI so
                    // we won't be creating a "mentions" file in the logger without this.
                    logger.new_server_tab("mentions");
                    Some(logger)
                }
            });

        let tui = UI::new(tui, logger);

        let mut clients: Vec<Client> = Vec::with_capacity(servers.len());

        for server in servers.iter().cloned() {
            tui.new_server_tab(&server.addr, server.alias);

            let tls = server.tls;
            let sasl_auth = if let Some(sasl_auth) = &server.sasl_auth {
                match sasl_from_config(tls, sasl_auth) {
                    Ok(sasl) => Some(sasl),
                    Err(e) => {
                        tui.add_client_err_msg(
                            &format!("SASL error: {}", e),
                            &MsgTarget::Server { serv: &server.addr },
                        );
                        None
                    }
                }
            } else {
                None
            };

            let server_info = ServerInfo {
                addr: server.addr,
                port: server.port,
                tls,
                pass: server.pass,
                realname: server.realname,
                nicks: server.nicks,
                auto_join: server
                    .join
                    .iter()
                    .map(|c| ChanNameRef::new(c).to_owned())
                    .collect(),
                nickserv_ident: server.nickserv_ident,
                sasl_auth,
            };

            let (client, rcv_conn_ev) = Client::new(server_info);

            let tui_clone = tui.clone();
            let client_clone = client.clone();

            // Spawn a task to handle connection events
            tokio::task::spawn_local(conn::task(rcv_conn_ev, tui_clone, Box::new(client_clone)));

            clients.push(client);
        }

        // Block on TUI task
        ui::task(defaults, tui, clients, rcv_tui_ev).await;
    });

    runtime.block_on(local);
}

// Helper to parse SASL config into a libtiny_client SASL struct
fn sasl_from_config(tls: bool, sasl_config: &config::SASLAuth) -> Result<SASLAuth, String> {
    match sasl_config {
        config::SASLAuth::Plain { username, password } => Ok(SASLAuth::Plain {
            username: username.clone(),
            password: password.clone(),
        }),
        config::SASLAuth::External { pem } => {
            // TLS must be on for EXTERNAL
            if !tls {
                Err("TLS is not enabled for this server, but SASL EXTERNAL authentication requires SASL. \
                Please enable SASL for this server in the config file.".to_string())
            } else {
                // load in a cert and private key for TLS client auth
                match File::open(pem) {
                    Ok(file) => {
                        let mut buf = BufReader::new(file);
                        // extract certificate
                        let cert = rustls_pemfile::certs(&mut buf)
                            .map_err(|e| format!("Could not parse PKCS8 PEM: {}", e))?
                            .pop()
                            .ok_or("Cert PEM must have at least one cert")?;

                        // extract private key
                        buf.seek(SeekFrom::Start(0)).unwrap();
                        let key = rustls_pemfile::pkcs8_private_keys(&mut buf)
                            .map_err(|e| format!("Could not parse PKCS8 PEM: {}", e))?
                            .pop()
                            .ok_or("Cert PEM must have at least one private key")?;

                        Ok(SASLAuth::External(SASLExternal { cert, key }))
                    }
                    Err(e) => Err(format!("Could not open PEM file: {}", e)),
                }
            }
        }
    }
}

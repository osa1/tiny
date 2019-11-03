use std::{env::Args, path::PathBuf};

pub(crate) struct CmdLineArgs {
    /// Servers to connect to
    pub(crate) servers: Vec<String>,

    /// Path to config file
    pub(crate) config_path: Option<PathBuf>,
}

pub(crate) fn parse_cmd_line_args(mut args: Args) -> CmdLineArgs {
    let mut parsed_args = CmdLineArgs {
        servers: Vec::new(),
        config_path: None,
    };

    args.next(); // skip program name

    while let Some(arg) = args.next() {
        if arg == "--config" {
            if let Some(config_path) = args.next() {
                parsed_args.config_path = Some(PathBuf::from(config_path));
            }
        } else {
            parsed_args.servers.push(arg);
        }
    }

    parsed_args
}

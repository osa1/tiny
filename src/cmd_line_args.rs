use std::env::Args;
use std::path::PathBuf;

pub struct CmdLineArgs {
    /// Servers to connect to
    pub servers: Vec<String>,

    /// Path to config file
    pub config_path: Option<PathBuf>,
}

pub fn parse_cmd_line_args(args: Args) -> CmdLineArgs {
    let mut parsed_args = CmdLineArgs {
        servers: Vec::new(),
        config_path: None,
    };

    let mut args = args.into_iter();
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

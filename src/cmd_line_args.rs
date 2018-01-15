use std::path::PathBuf;

pub struct CmdLineArgs {
    /// Servers to connect to
    pub servers: Vec<String>,

    /// Path to config file
    pub config_path: Option<PathBuf>,
}

pub fn parse_cmd_line_args(args: Vec<String>) -> CmdLineArgs {
    let mut parsed_args = CmdLineArgs {
        servers: Vec::new(),
        config_path: None,
    };

    if args.len() >= 2 {
        let mut i = 1;
        while i < args.len() {
            if args[i] == "--config" {
                if i + 1 < args.len() {
                    parsed_args.config_path = Some(PathBuf::from(args[i+1].clone()));
                }
                i += 1;
            } else {
                parsed_args.servers.push(args[i].clone());
            }
            i += 1;
        }
    }

    parsed_args
}


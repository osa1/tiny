//! To see how color numbers map to actual colors in your terminal run
//! `cargo run --example colors`. Use tab to swap fg/bg colors.

use yaml_rust::Yaml;
use yaml_rust::YamlLoader;

use std::env::home_dir;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct Config {
    ///List of servers
    pub servers: Vec<Server>,
    ///Defaults: see definition for Defaults struct
    pub defaults: Defaults,
    ///Path to store chatlogs
    pub logs: PathBuf,
}

impl Config {
    pub fn new<P: AsRef<Path>>(servers: Vec<Server>, defaults: Defaults, logs: P) -> Config {
        Config {
            servers: servers,
            defaults: defaults,
            logs: logs.as_ref().to_path_buf(), //convert to owned
        }
    }
}

#[derive(Debug, Clone)]
pub struct Server {
    /// Address of the server
    pub server_addr: String,

    /// Port of the server
    pub server_port: u16,

    /// Hostname to be used in connection registration
    pub hostname: String,

    /// Real name to be used in connection registration
    pub real_name: String,

    /// Nicks to try when connecting to this server. tiny tries these sequentially, and starts
    /// adding trailing underscores to the last one if none of the nicks are available.
    pub nicks: Vec<String>,

    /// Commands to automatically run after joining to the server. Useful for e.g. registering the
    /// nick via nickserv or joining channels. Uses tiny command syntax.
    pub auto_cmds: Vec<String>,
}

/// Similar to `Server`, but used when connecting via the `/connect` command.
#[derive(Debug, Clone)]
pub struct Defaults {
    pub nicks: Vec<String>,
    pub hostname: String,
    pub realname: String,
    pub auto_cmds: Vec<String>,
}

pub fn get_defaults() -> Defaults {
    Defaults {
        nicks: vec!["tiny_user".to_owned()],
        hostname: "tiny".to_owned(),
        realname: "anonymous tiny user".to_owned(),
        auto_cmds: vec![],
    }
}

fn get_config_path() -> PathBuf {
    let mut config_path = home_dir().unwrap();
    config_path.push(".tinyrc.yml");
    config_path
}

pub fn read_config() -> Option<(Vec<Server>, Defaults, String)> {

    // sigh ... what a mess

    let config_str = {
        let config_path = get_config_path();
        if !config_path.exists() {
            return None;
        }
        let mut config_file = File::open(get_config_path()).unwrap();
        let mut config_str = String::new();
        config_file.read_to_string(&mut config_str).unwrap();
        config_str
    };

    let config_yaml = YamlLoader::load_from_str(&config_str).unwrap();

    let servers_yaml: &Yaml = config_yaml[0]
        .as_hash()
        .unwrap()
        .get(&Yaml::String("servers".to_owned()))
        .unwrap();
    // duh, allocation for lookup

    let defaults_yaml: &Yaml = config_yaml[0]
        .as_hash()
        .unwrap()
        .get(&Yaml::String("defaults".to_owned()))
        .unwrap();

    let logs_yaml: &Yaml = config_yaml[0]
        .as_hash()
        .unwrap()
        .get(&Yaml::String("logs".to_owned()))
        .unwrap();

    let mut servers = vec![];

    let yaml_addr_key = Yaml::String("addr".to_owned());
    let yaml_port_key = Yaml::String("port".to_owned());
    let yaml_hostname_key = Yaml::String("hostname".to_owned());
    let yaml_realname_key = Yaml::String("realname".to_owned());
    let yaml_nicks_key = Yaml::String("nicks".to_owned());
    let yaml_auto_cmds_key = Yaml::String("auto_cmds".to_owned());

    for server in servers_yaml.as_vec().unwrap().into_iter() {
        let server_hash = server.as_hash().unwrap();
        let server_addr = server_hash
            .get(&yaml_addr_key)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let server_port = server_hash
            .get(&yaml_port_key)
            .unwrap()
            .as_i64()
            .unwrap()
            .to_owned() as u16;
        let hostname = server_hash
            .get(&yaml_hostname_key)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let real_name = server_hash
            .get(&yaml_realname_key)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let nicks: Vec<String> = server_hash
            .get(&yaml_nicks_key)
            .unwrap()
            .as_vec()
            .unwrap()
            .into_iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect();
        let auto_cmds: Vec<String> = server_hash
            .get(&yaml_auto_cmds_key)
            .unwrap()
            .as_vec()
            .unwrap()
            .into_iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect();

        servers.push(Server {
                         server_addr,
                         server_port,
                         hostname,
                         real_name,
                         nicks,
                         auto_cmds,
                     });
    }

    let defaults = defaults_yaml.as_hash().unwrap();
    let defaults = Defaults {
        nicks: defaults
            .get(&yaml_nicks_key)
            .unwrap()
            .as_vec()
            .unwrap()
            .into_iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect(),
        hostname: defaults
            .get(&yaml_hostname_key)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned(),
        realname: defaults
            .get(&yaml_realname_key)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned(),
        auto_cmds: defaults
            .get(&yaml_auto_cmds_key)
            .unwrap()
            .as_vec()
            .unwrap()
            .into_iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect(),
    };

    let logs = logs_yaml.as_str().unwrap().to_owned();

    Some((servers, defaults, logs))
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Colors
////////////////////////////////////////////////////////////////////////////////////////////////////

/// Colors used to highlight nicks
pub static NICK_COLORS: [u8; 13] = [1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14];

pub use termbox_simple::*;

#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Termbox fg.
    pub fg: u16,

    /// Termbox bg.
    pub bg: u16,
}

pub const CLEAR: Style = Style {
    fg: TB_DEFAULT,
    bg: TB_DEFAULT,
};

pub const USER_MSG: Style = Style {
    fg: 0,
    bg: TB_DEFAULT,
};

pub const ERR_MSG: Style = Style {
    fg: 0 | TB_BOLD,
    bg: 1,
};

pub const TOPIC: Style = Style {
    fg: 14 | TB_BOLD,
    bg: TB_DEFAULT,
};

pub const CURSOR: Style = USER_MSG;

pub const JOIN: Style = Style {
    fg: 242,
    bg: TB_DEFAULT,
};

pub const PART: Style = Style {
    fg: 242,
    bg: TB_DEFAULT,
};

pub const NICK: Style = Style {
    fg: 242,
    bg: TB_DEFAULT,
};

pub const EXIT_DIALOGUE: Style = Style {
    fg: TB_DEFAULT,
    bg: 4,
};

pub const HIGHLIGHT: Style = Style {
    fg: 9 | TB_BOLD,
    bg: TB_DEFAULT,
};

// Currently unused
// pub const MENTION: Style =
//     Style {
//         fg: 220,
//         bg: TB_DEFAULT,
//     };

pub const COMPLETION: Style = Style {
    fg: 84,
    bg: TB_DEFAULT,
};

pub const TIMESTAMP: Style = Style {
    fg: 0 | TB_BOLD,
    bg: TB_DEFAULT,
};

pub const TAB_ACTIVE: Style = Style {
    fg: 0 | TB_BOLD,
    bg: 0,
};

pub const TAB_NORMAL: Style = Style { fg: 8, bg: 0 };

pub const TAB_NEW_MSG: Style = Style { fg: 5, bg: 0 };

pub const TAB_HIGHLIGHT: Style = Style {
    fg: 9 | TB_BOLD,
    bg: 0,
};

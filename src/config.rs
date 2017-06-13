//! To see how color numbers map to actual colors in your terminal run
//! `cargo run --example colors`. Use tab to swap fg/bg colors.

use std::env::home_dir;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Server {
    /// Address of the server
    pub addr: String,

    /// Port of the server
    pub port: u16,

    /// Hostname to be used in connection registration
    pub hostname: String,

    /// Real name to be used in connection registration
    pub realname: String,

    /// Nicks to try when connecting to this server. tiny tries these sequentially, and starts
    /// adding trailing underscores to the last one if none of the nicks are available.
    pub nicks: Vec<String>,

    /// Commands to automatically run after joining to the server. Useful for e.g. registering the
    /// nick via nickserv or joining channels. Uses tiny command syntax.
    pub auto_cmds: Vec<String>,
}

/// Similar to `Server`, but used when connecting via the `/connect` command.
#[derive(Debug, Clone, Deserialize)]
pub struct Defaults {
    pub nicks: Vec<String>,
    pub hostname: String,
    pub realname: String,
    pub auto_cmds: Vec<String>
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub servers: Vec<Server>,
    pub defaults: Defaults,
    pub log_dir: String,
}

pub fn get_config_path() -> PathBuf {
    let mut config_path = home_dir().unwrap();
    config_path.push(".tinyrc.yml");
    config_path
}

pub fn get_default_config_yaml() -> String {
    let mut log_dir = home_dir().unwrap();
    log_dir.push("tiny_logs");
    format!("\
# Servers to auto-connect
servers:
    - addr: irc.mozilla.org
      port: 6667
      hostname: yourhost
      realname: yourname
      nicks: [tiny_user]
      auto_cmds:
          - 'msg NickServ identify hunter2'
          - 'join #tiny'

# Defaults used when connecting to servers via the /connect command
defaults:
    nicks: [tiny_user]
    hostname: yourhost
    realname: yourname
    auto_cmds: []

# Where to put log files
log_dir: '{}'", log_dir.as_path().to_str().unwrap())
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Colors
////////////////////////////////////////////////////////////////////////////////////////////////////

/// Colors used to highlight nicks
pub static NICK_COLORS: [u8; 13] =
    [ 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14 ];

pub use termbox_simple::*;

#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// Termbox fg.
    pub fg: u16,

    /// Termbox bg.
    pub bg: u16,
}

pub const CLEAR: Style =
    Style {
        fg: TB_DEFAULT,
        bg: TB_DEFAULT,
    };

pub const USER_MSG: Style =
    Style {
        fg: 0,
        bg: TB_DEFAULT,
    };

pub const ERR_MSG: Style =
    Style {
        fg: 0 | TB_BOLD,
        bg: 1,
    };

pub const TOPIC: Style =
    Style {
        fg: 14 | TB_BOLD,
        bg: TB_DEFAULT,
    };

pub const CURSOR: Style =
    USER_MSG;

pub const JOIN: Style =
    Style {
        fg: 242,
        bg: TB_DEFAULT,
    };

pub const PART: Style =
    Style {
        fg: 242,
        bg: TB_DEFAULT,
    };

pub const NICK: Style =
    Style {
        fg: 242,
        bg: TB_DEFAULT,
    };

pub const EXIT_DIALOGUE: Style =
    Style {
        fg: TB_DEFAULT,
        bg: 4,
    };

pub const HIGHLIGHT: Style =
    Style {
        fg: 9 | TB_BOLD,
        bg: TB_DEFAULT,
    };

// Currently unused
// pub const MENTION: Style =
//     Style {
//         fg: 220,
//         bg: TB_DEFAULT,
//     };

pub const COMPLETION: Style =
    Style {
        fg: 84,
        bg: TB_DEFAULT,
    };

pub const TIMESTAMP: Style =
    Style {
        fg: 0 | TB_BOLD,
        bg: TB_DEFAULT,
    };

pub const TAB_ACTIVE: Style =
    Style {
        fg: 0 | TB_BOLD,
        bg: 0,
    };

pub const TAB_NORMAL: Style =
    Style {
        fg: 8,
        bg: 0,
    };

pub const TAB_NEW_MSG: Style =
    Style {
        fg: 5,
        bg: 0,
    };

pub const TAB_HIGHLIGHT: Style =
    Style {
        fg: 9 | TB_BOLD,
        bg: 0,
    };

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    #[test]
    fn parse_default_config() {
        match serde_yaml::from_str(&get_default_config_yaml()) {
            Err(yaml_err) => {
                println!("{}", yaml_err);
                assert!(false);
            }
            Ok(Config { .. }) => {}
        }
    }
}

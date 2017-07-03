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

pub use termbox_simple::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Style {
    /// Termbox fg.
    pub fg: u16,

    /// Termbox bg.
    pub bg: u16,
}

pub struct Colors {
    pub nick: Vec<u8>,
    pub clear: Style,
    pub user_msg: Style,
    pub err_msg: Style,
    pub topic: Style,
    pub cursor: Style,
    pub join: Style,
    pub part: Style,
    pub nick_change: Style,
    pub faded: Style,
    pub exit_dialogue: Style,
    pub highlight: Style,
    pub completion: Style,
    pub timestamp: Style,
    pub tab_active: Style,
    pub tab_normal: Style,
    pub tab_new_msg: Style,
    pub tab_highlight: Style,
}

pub fn default_colors() -> Colors {
    Colors {
        nick: vec![ 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14 ],
        clear: Style { fg: TB_DEFAULT, bg: TB_DEFAULT },
        user_msg: Style { fg: 0, bg: TB_DEFAULT },
        err_msg: Style { fg: 0 | TB_BOLD, bg: 1 },
        topic: Style { fg: 14 | TB_BOLD, bg: TB_DEFAULT },
        cursor: Style { fg: 0, bg: TB_DEFAULT },
        join: Style { fg: 10 | TB_BOLD, bg: TB_DEFAULT },
        part: Style { fg: 1 | TB_BOLD, bg: TB_DEFAULT },
        nick_change: Style { fg: 10 | TB_BOLD, bg: TB_DEFAULT },
        faded: Style { fg: 242, bg: TB_DEFAULT },
        exit_dialogue: Style { fg: TB_DEFAULT, bg: 4 },
        highlight: Style { fg: 9 | TB_BOLD, bg: TB_DEFAULT },
        completion: Style { fg: 84, bg: TB_DEFAULT },
        timestamp: Style { fg: 242, bg: TB_DEFAULT },
        tab_active: Style { fg: 0 | TB_BOLD, bg: 0 },
        tab_normal: Style { fg: 8, bg: 0 },
        tab_new_msg: Style { fg: 5, bg: 0 },
        tab_highlight: Style { fg: 9 | TB_BOLD, bg: 0 },
    }
}
////////////////////////////////////////////////////////////////////////////////////////////////////

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

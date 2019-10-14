//! To see how color numbers map to actual colors in your terminal run
//! `cargo run --example colors`. Use tab to swap fg/bg colors.
use dirs::home_dir;
use serde::Deserialize;
use serde_yaml;
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
pub(crate) struct SASLAuth {
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(Clone, Deserialize)]
pub(crate) struct Server {
    /// Address of the server
    pub(crate) addr: String,

    /// Port of the server
    pub(crate) port: u16,

    /// Use tls
    #[serde(default)]
    pub(crate) tls: bool,

    /// Server password (optional)
    #[serde(default)]
    pub(crate) pass: Option<String>,

    /// Real name to be used in connection registration
    pub(crate) realname: String,

    /// Nicks to try when connecting to this server. tiny tries these sequentially, and starts
    /// adding trailing underscores to the last one if none of the nicks are available.
    pub(crate) nicks: Vec<String>,

    /// Channels to automatically join.
    #[serde(default)]
    pub(crate) join: Vec<String>,

    /// NickServ identification password. Used on connecting to the server and nick change.
    pub(crate) nickserv_ident: Option<String>,

    /// Authenication method
    #[serde(rename = "sasl")]
    pub(crate) sasl_auth: Option<SASLAuth>,
}

/// Similar to `Server`, but used when connecting via the `/connect` command.
#[derive(Clone, Deserialize)]
pub(crate) struct Defaults {
    pub(crate) nicks: Vec<String>,
    pub(crate) realname: String,
    #[serde(default)]
    pub(crate) join: Vec<String>,
    #[serde(default)]
    pub(crate) tls: bool,
}

#[derive(Deserialize)]
pub(crate) struct Config {
    pub(crate) servers: Vec<Server>,
    pub(crate) defaults: Defaults,
    #[serde(default)]
    pub(crate) colors: libtiny_tui::Colors,
    pub(crate) log_dir: Option<PathBuf>,
}

pub(crate) fn get_default_config_path() -> PathBuf {
    let mut config_path = home_dir().unwrap();
    config_path.push(".tinyrc.yml");
    config_path
}

pub(crate) fn parse_config(config_path: &Path) -> Result<Config, serde_yaml::Error> {
    let contents = {
        let mut str = String::new();
        let mut file = File::open(config_path).unwrap();
        file.read_to_string(&mut str).unwrap();
        str
    };

    parse_config_str(&contents)
}

fn parse_config_str(contents: &str) -> Result<Config, serde_yaml::Error> {
    serde_yaml::from_str(&contents)
}

pub(crate) fn generate_default_config(config_path: &Path) {
    if let Some(parent) = config_path.parent() {
        let _ = ::std::fs::create_dir_all(parent);
    }
    let mut file = File::create(config_path).unwrap();
    file.write_all(get_default_config_yaml().as_bytes())
        .unwrap();
    println!(
        "\
tiny couldn't find a config file at {0:?}, and created a config file with defaults.
You may want to edit {0:?} before re-running tiny.",
        config_path
    );
}

fn get_default_config_yaml() -> String {
    let mut log_dir = home_dir().unwrap();
    log_dir.push("tiny_logs");
    format!(
        include_str!("../tinyrc.yml"),
        log_dir.as_path().to_str().unwrap()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    #[test]
    fn parse_default_config() {
        match serde_yaml::from_str(&get_default_config_yaml()) {
            Err(yaml_err) => {
                println!("{}", yaml_err);
                panic!();
            }
            Ok(Config { servers, .. }) => {
                assert_eq!(
                    servers[0].join,
                    vec!["#tiny".to_owned(), "#rust".to_owned()]
                );
                assert_eq!(servers[0].tls, true);
                assert_eq!(servers[0].pass, Some("hunter2".to_owned()));
                assert_eq!(
                    servers[0].sasl_auth,
                    Some(SASLAuth {
                        username: "tiny_user".to_owned(),
                        password: "hunter2".to_owned(),
                    })
                );
                assert_eq!(servers[0].nickserv_ident, Some("hunter2".to_owned()));
            }
        }
    }
}

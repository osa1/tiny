//! To see how color numbers map to actual colors in your terminal run
//! `cargo run --example colors`. Use tab to swap fg/bg colors.
use dirs::home_dir;
use serde::Deserialize;
use serde_yaml;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
pub struct SASLAuth {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Deserialize)]
pub struct Server {
    /// Address of the server
    pub addr: String,

    /// Port of the server
    pub port: u16,

    /// Use tls
    #[serde(default)]
    pub tls: bool,

    /// Server password (optional)
    #[serde(default)]
    pub pass: Option<String>,

    /// Hostname to be used in connection registration
    pub hostname: String,

    /// Real name to be used in connection registration
    pub realname: String,

    /// Nicks to try when connecting to this server. tiny tries these sequentially, and starts
    /// adding trailing underscores to the last one if none of the nicks are available.
    pub nicks: Vec<String>,

    /// Channels to automatically join.
    #[serde(default)]
    pub join: Vec<String>,

    /// NickServ identification password. Used on connecting to the server and nick change.
    pub nickserv_ident: Option<String>,

    /// Authenication method
    #[serde(rename = "sasl")]
    pub sasl_auth: Option<SASLAuth>,
}

/// Similar to `Server`, but used when connecting via the `/connect` command.
#[derive(Clone, Deserialize)]
pub struct Defaults {
    pub nicks: Vec<String>,
    pub hostname: String,
    pub realname: String,
    #[serde(default)]
    pub join: Vec<String>,
    #[serde(default)]
    pub tls: bool,
}

#[derive(Deserialize)]
pub struct Config {
    pub servers: Vec<Server>,
    pub defaults: Defaults,
    #[serde(default)]
    pub colors: libtiny_tui::Colors,
    pub log_dir: String,
}

pub fn get_default_config_path() -> PathBuf {
    let mut config_path = home_dir().unwrap();
    config_path.push(".tinyrc.yml");
    config_path
}

pub fn parse_config(config_path: &Path) -> Result<Config, serde_yaml::Error> {
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

pub fn generate_default_config(config_path: &Path) {
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
                assert!(false);
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

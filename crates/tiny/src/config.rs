use serde::{Deserialize, Deserializer};
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
pub(crate) struct SASLAuth {
    pub(crate) username: String,
    pub(crate) password: String,
}

#[derive(Clone, Deserialize)]
pub(crate) struct Server {
    /// Address of the server
    pub(crate) addr: String,

    /// Optional server alias to be shown in the tab line.
    #[serde(default)]
    pub(crate) alias: Option<String>,

    /// Port of the server
    pub(crate) port: u16,

    /// Use tls
    #[serde(default)]
    pub(crate) tls: bool,

    /// Server password (optional)
    #[serde(default)]
    pub(crate) pass: Option<String>,

    /// Real name to be used in connection registration
    #[serde(deserialize_with = "deser_trimmed_str")]
    pub(crate) realname: String,

    /// Nicks to try when connecting to this server. tiny tries these sequentially, and starts
    /// adding trailing underscores to the last one if none of the nicks are available.
    #[serde(deserialize_with = "deser_trimmed_str_vec")]
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
    #[serde(deserialize_with = "deser_trimmed_str_vec")]
    pub(crate) nicks: Vec<String>,
    #[serde(deserialize_with = "deser_trimmed_str")]
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
    pub(crate) log_dir: Option<PathBuf>,
}

fn deser_trimmed_str<'de, D>(d: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let str = String::deserialize(d)?;
    Ok(str.trim().to_owned())
}

fn deser_trimmed_str_vec<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let strs: Vec<String> = Vec::deserialize(d)?;
    Ok(strs.into_iter().map(|s| s.trim().to_owned()).collect())
}

impl Config {
    /// Returns error descriptions
    pub(crate) fn validate(&self) -> Vec<String> {
        let mut errors = vec![];

        if self.defaults.nicks.is_empty() {
            errors.push(
                "Default nick list can't be empty, please add at least one defaut nick".to_owned(),
            );
        }

        if self.defaults.realname.is_empty() {
            errors.push(
                "realname can't be empty, please update 'realname' field of 'defaults'".to_owned(),
            );
        }

        for (nick_idx, nick) in self.defaults.nicks.iter().enumerate() {
            if nick.is_empty() {
                errors.push(format!("Default nick {} is empty", nick_idx));
            }
        }

        for server in &self.servers {
            if server.nicks.is_empty() {
                errors.push(format!(
                    "Nick list for server '{}' is empty, please add at least one nick",
                    server.addr
                ));
            }

            for (nick_idx, nick) in server.nicks.iter().enumerate() {
                if nick.is_empty() {
                    errors.push(format!(
                        "Nicks can't be empty, please update nick {} for '{}'",
                        nick_idx, server.addr
                    ));
                }
            }

            if server.realname.is_empty() {
                errors.push(format!(
                    "'realname' can't be empty, please update 'realname' field of '{}'",
                    server.addr
                ));
            }
        }

        errors
    }
}

/// Returns tiny config file path. File may or may not exist.
///
/// Places to look: (in priority order)
///
/// - $XDG_CONFIG_HOME/tiny/config.yml
/// - $HOME/.config/tiny/config.yml
/// - $HOME/.tinyrc.yml (old, for backward compat)
///
/// Panics when none of $XDG_CONFIG_HOME or $HOME can be found (using the `dirs` crate).
pub(crate) fn get_config_path() -> PathBuf {
    let xdg_config_path = dirs::config_dir().map(|mut xdg_config_home| {
        xdg_config_home.push("tiny");
        let _ = fs::create_dir_all(&xdg_config_home);
        xdg_config_home.push("config.yml");
        xdg_config_home
    });

    let home_config_path = dirs::home_dir().map(|mut home_dir| {
        home_dir.push(".tinyrc.yml");
        home_dir
    });

    match (xdg_config_path, home_config_path) {
        (Some(xdg_config_path), _) if xdg_config_path.exists() => xdg_config_path,
        (_, Some(home_config_path)) if home_config_path.exists() => home_config_path,
        (Some(xdg_config_path), _) => xdg_config_path,
        (_, Some(home_config_path)) => home_config_path,
        (None, None) => {
            panic!(
                "Can't read $HOME or $XDG_CONFIG_HOME environment variables,
                please consider setting at least one of these variables"
            );
        }
    }
}

pub(crate) fn parse_config(config_path: &Path) -> Result<Config, serde_yaml::Error> {
    let contents = {
        let mut str = String::new();
        let mut file = File::open(config_path).unwrap();
        file.read_to_string(&mut str).unwrap();
        str
    };

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
    let mut log_dir = dirs::home_dir().unwrap();
    log_dir.push("tiny_logs");
    format!(
        include_str!("../config.yml"),
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
                assert_eq!(servers[0].join, vec!["#tiny".to_owned()]);
                assert_eq!(servers[0].tls, true);
            }
        }
    }

    #[test]
    fn validation() {
        // We trim the string fields when deserializing, so `validate` doesn't consider non-empty
        // strings as empty even if they have only spaces, it assumes spaces should be trimmed
        let config = Config {
            servers: vec![Server {
                addr: "my_server".to_owned(),
                alias: None,
                port: 123,
                tls: false,
                pass: None,
                realname: "".to_owned(),
                nicks: vec!["".to_owned()],
                join: vec![],
                nickserv_ident: None,
                sasl_auth: None,
            }],
            defaults: Defaults {
                nicks: vec!["".to_owned()],
                realname: "".to_owned(),
                join: vec![],
                tls: false,
            },
            log_dir: None,
        };

        let errors = config.validate();
        assert_eq!(errors.len(), 4);

        assert_eq!(
            &errors[0],
            "realname can't be empty, please update 'realname' field of 'defaults'"
        );
        assert_eq!(&errors[1], "Default nick 0 is empty");
        assert_eq!(
            &errors[2],
            "Nicks can't be empty, please update nick 0 for 'my_server'"
        );
        assert_eq!(
            &errors[3],
            "'realname' can't be empty, please update 'realname' field of 'my_server'"
        );
    }
}

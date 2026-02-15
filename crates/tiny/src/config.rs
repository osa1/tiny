use libtiny_client::SASLAuth as ClientSASLAuth;
use serde::{Deserialize, Deserializer};

use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use libtiny_tui::config::Chan;

#[derive(Clone, Deserialize, Debug, PartialEq, Eq)]
#[serde(untagged, rename_all = "snake_case")]
pub(crate) enum SASLAuth<P> {
    Plain {
        /// Registered username
        username: String,
        /// Password
        password: P,
    },
    External {
        /// Path to PEM file with private key and certificate (PKCS8 format).
        /// A fingerprint of the certificate should be registered with NickServ
        pem: PathBuf,
    },
}

impl TryFrom<SASLAuth<String>> for ClientSASLAuth {
    type Error = String;

    fn try_from(sasl: SASLAuth<String>) -> Result<Self, Self::Error> {
        Ok(match sasl {
            SASLAuth::Plain { username, password } => ClientSASLAuth::Plain { username, password },
            SASLAuth::External { pem } => ClientSASLAuth::External {
                pem: std::fs::read(pem).map_err(|e| format!("Could not read PEM file: {e}"))?,
            },
        })
    }
}

#[derive(Clone, Deserialize)]
#[serde(bound(deserialize = "P: Deserialize<'de>"))]
pub(crate) struct Server<P> {
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
    pub(crate) pass: Option<P>,

    /// User name to be used in connection registration
    /// If it is not specified, the first nick will be used instead
    #[serde(default)]
    pub(crate) user: Option<String>,

    /// Real name to be used in connection registration
    #[serde(deserialize_with = "deser_trimmed_str")]
    pub(crate) realname: String,

    /// Nicks to try when connecting to this server. tiny tries these sequentially, and starts
    /// adding trailing underscores to the last one if none of the nicks are available.
    #[serde(deserialize_with = "deser_trimmed_str_vec")]
    pub(crate) nicks: Vec<String>,

    /// Channels to automatically join.
    pub(crate) join: Vec<Chan>,

    /// NickServ identification password. Used on connecting to the server and nick change.
    pub(crate) nickserv_ident: Option<P>,

    /// Authenication method
    #[serde(rename = "sasl")]
    pub(crate) sasl_auth: Option<SASLAuth<P>>,
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
pub(crate) struct Config<P> {
    pub(crate) servers: Vec<Server<P>>,
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

/// A password, or a shell command to run the obtain a password. Used for password (server
/// password, SASL, NickServ) fields of `Config`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PassOrCmd {
    /// Password is given directly as plain text
    Pass(String),

    /// A shell command to run to get the password
    Cmd(Vec<String>),
}

impl PassOrCmd {
    fn is_empty_cmd(&self) -> bool {
        match self {
            PassOrCmd::Cmd(cmd) => cmd.is_empty(),
            PassOrCmd::Pass(_) => false,
        }
    }
}

impl<'de> Deserialize<'de> for PassOrCmd {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        use serde_yaml::Value;

        match Value::deserialize(deserializer)? {
            Value::String(str) => Ok(PassOrCmd::Pass(str)),
            Value::Mapping(map) => match map.get(&Value::String("command".to_owned())) {
                Some(Value::String(cmd)) => match shell_words::split(cmd) {
                    Ok(cmd_parts) => Ok(PassOrCmd::Cmd(cmd_parts)),
                    Err(err) => Err(D::Error::custom(format!(
                        "Unable to parse password field: {err}"
                    ))),
                },
                _ => Err(D::Error::custom(
                    "Expected a 'cmd' key in password map with string value",
                )),
            },
            _ => Err(D::Error::custom("Password field must be a string or map")),
        }
    }
}

fn run_command(command_name: &str, server_addr: &str, args: &[String]) -> Option<String> {
    println!(
        "Running {} command for {} (`{}`)",
        command_name,
        server_addr,
        shell_words::join(args)
    );

    assert!(!args.is_empty()); // should be checked in `validate`

    let mut cmd = Command::new(&args[0]);
    cmd.args(args[1..].iter());

    let output = match cmd.output() {
        Err(err) => {
            println!("Command failed: {err:?}");
            return None;
        }
        Ok(output) => output,
    };

    if !output.status.success() {
        print!("Command returned non-zero");
        if let Some(code) = output.status.code() {
            println!(": {code}");
        } else {
            println!();
        }
        if output.stdout.is_empty() {
            println!("stdout is empty");
        } else {
            println!("stdout:");
            println!("--------------------------------------");
            println!("{}", String::from_utf8_lossy(&output.stdout));
            println!("--------------------------------------");
        }

        if output.stderr.is_empty() {
            println!("stderr is empty");
        } else {
            println!("stderr:");
            println!("--------------------------------------");
            println!("{}", String::from_utf8_lossy(&output.stderr));
            println!("--------------------------------------");
        }

        return None;
    }

    if output.stdout.is_empty() {
        println!("Command returned zero, but stdout is empty. Aborting.");
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    Some(stdout.lines().last().unwrap().to_owned())
}

impl Config<PassOrCmd> {
    /// Returns error descriptions.
    pub(crate) fn validate(&self) -> Vec<String> {
        let mut errors = vec![];

        if self.defaults.nicks.is_empty() {
            errors.push(
                "Default nick list can't be empty, please add at least one default nick".to_owned(),
            );
        }

        if self.defaults.realname.is_empty() {
            errors.push(
                "realname can't be empty, please update 'realname' field of 'defaults'".to_owned(),
            );
        }

        for (nick_idx, nick) in self.defaults.nicks.iter().enumerate() {
            if nick.is_empty() {
                errors.push(format!("Default nick {nick_idx} is empty"));
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

            if let Some(ref pass) = server.pass
                && pass.is_empty_cmd()
            {
                errors.push(format!("Empty PASS command for '{}'", server.addr));
            }

            if let Some(ref nickserv_ident) = server.nickserv_ident
                && nickserv_ident.is_empty_cmd()
            {
                errors.push(format!(
                    "Empty NickServ password command for '{}'",
                    server.addr
                ));
            }

            if let Some(SASLAuth::Plain { password, .. }) = &server.sasl_auth
                && password.is_empty_cmd()
            {
                errors.push(format!("Empty SASL password command for '{}'", server.addr));
            }

            if let Some(SASLAuth::External { .. }) = &server.sasl_auth
                && !server.tls
            {
                errors.push(format!(
                    "TLS is not enabled for '{}', but SASL EXTERNAL authentication requires TLS. \
                     Please enable TLS for this server in the config file.",
                    server.addr
                ));
            }
        }

        errors
    }

    /// Runs password commands and updates the config with plain passwords obtained from the
    /// commands.
    pub(crate) fn read_passwords(self) -> Option<Config<String>> {
        let Config {
            servers,
            defaults,
            log_dir,
        } = self;

        let mut servers_: Vec<Server<String>> = Vec::with_capacity(servers.len());

        for server in servers {
            let Server {
                addr,
                alias,
                port,
                tls,
                pass,
                user,
                realname,
                nicks,
                join,
                nickserv_ident,
                sasl_auth,
            } = server;

            let pass = match pass {
                None => None,
                Some(PassOrCmd::Pass(pass)) => Some(pass),
                Some(PassOrCmd::Cmd(cmd)) => Some(run_command("server password", &addr, &cmd)?),
            };

            let nickserv_ident = match nickserv_ident {
                None => None,
                Some(PassOrCmd::Pass(pass)) => Some(pass),
                Some(PassOrCmd::Cmd(cmd)) => Some(run_command("NickServ password", &addr, &cmd)?),
            };

            let sasl_auth = match sasl_auth {
                None => None,
                Some(SASLAuth::Plain {
                    username,
                    password: PassOrCmd::Pass(pass),
                }) => Some(SASLAuth::Plain {
                    username,
                    password: pass,
                }),
                Some(SASLAuth::Plain {
                    username,
                    password: PassOrCmd::Cmd(cmd),
                }) => {
                    let password = run_command("SASL password", &addr, &cmd)?;
                    Some(SASLAuth::Plain { username, password })
                }
                Some(SASLAuth::External { pem }) => Some(SASLAuth::External { pem }),
            };

            servers_.push(Server {
                addr,
                alias,
                port,
                tls,
                pass,
                user,
                realname,
                nicks,
                join,
                nickserv_ident,
                sasl_auth,
            });
        }

        Some(Config {
            servers: servers_,
            defaults,
            log_dir,
        })
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

fn expand_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    match shellexpand::full(&s) {
        Ok(expanded) => PathBuf::from(expanded.as_ref()),
        Err(err) => {
            println!("Failed to expand path {path:?}: {err}");
            path.to_owned()
        }
    }
}

pub(crate) fn parse_config(config_path: &Path) -> Result<Config<PassOrCmd>, serde_yaml::Error> {
    let contents = {
        let mut str = String::new();
        let mut file = File::open(config_path).unwrap();
        file.read_to_string(&mut str).unwrap();
        str
    };

    let mut config: Config<PassOrCmd> = serde_yaml::from_str(&contents)?;

    if let Some(log_dir) = &mut config.log_dir {
        *log_dir = expand_path(log_dir);
    }
    for server in &mut config.servers {
        if let Some(SASLAuth::External { pem }) = &mut server.sasl_auth {
            *pem = expand_path(pem);
        }
    }

    Ok(config)
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
tiny couldn't find a config file at {config_path:?}, and created a config file with defaults.
You may want to edit {config_path:?} before re-running tiny."
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
    use libtiny_common::ChanName;

    use super::*;

    #[test]
    fn parse_default_config() {
        match serde_yaml::from_str::<Config<String>>(&get_default_config_yaml()) {
            Err(yaml_err) => {
                println!("{yaml_err}");
                panic!();
            }
            Ok(Config { servers, .. }) => {
                assert_eq!(
                    servers[0].join,
                    vec![Chan::Name(ChanName::new("#tiny".to_string()))]
                );
                assert!(servers[0].tls);
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
                user: None,
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

    #[test]
    fn parse_config_expands_log_dir() {
        let home = std::env::var("HOME").unwrap();
        let yaml = "\
servers:
  - addr: irc.test.com
    port: 6697
    tls: true
    realname: test
    nicks: [test]
    join: []
defaults:
  nicks: [test]
  realname: test
log_dir: ~/test_logs
";
        let dir = std::env::temp_dir().join("tiny_test_parse_config");
        let _ = fs::create_dir_all(&dir);
        let config_path = dir.join("config.yml");
        fs::write(&config_path, yaml).unwrap();

        let config = parse_config(&config_path).unwrap();
        assert_eq!(
            config.log_dir.unwrap(),
            PathBuf::from(format!("{home}/test_logs"))
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_config_expands_sasl_pem() {
        let home = std::env::var("HOME").unwrap();
        let yaml = "\
servers:
  - addr: irc.test.com
    port: 6697
    tls: true
    realname: test
    nicks: [test]
    join: []
    sasl:
      pem: ~/certs/my.pem
defaults:
  nicks: [test]
  realname: test
";
        let dir = std::env::temp_dir().join("tiny_test_parse_config_sasl");
        let _ = fs::create_dir_all(&dir);
        let config_path = dir.join("config.yml");
        fs::write(&config_path, yaml).unwrap();

        let config = parse_config(&config_path).unwrap();
        match &config.servers[0].sasl_auth {
            Some(SASLAuth::External { pem }) => {
                assert_eq!(*pem, PathBuf::from(format!("{home}/certs/my.pem")));
            }
            other => panic!("Expected SASLAuth::External, got {other:?}"),
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn expand_path_tilde() {
        let home = std::env::var("HOME").unwrap();
        let expanded = expand_path(Path::new("~/foo"));
        assert_eq!(expanded, PathBuf::from(format!("{home}/foo")));
    }

    #[test]
    fn expand_path_env_var() {
        let home = std::env::var("HOME").unwrap();
        let expanded = expand_path(Path::new("$HOME/foo"));
        assert_eq!(expanded, PathBuf::from(format!("{home}/foo")));
    }

    #[test]
    fn expand_path_no_expansion() {
        let path = Path::new("/absolute/path/no/vars");
        assert_eq!(expand_path(path), path);
    }

    #[test]
    fn parse_password_field() {
        let field = "command: my pass cmd";
        assert_eq!(
            serde_yaml::from_str::<PassOrCmd>(field).unwrap(),
            PassOrCmd::Cmd(vec!["my".to_owned(), "pass".to_owned(), "cmd".to_owned()])
        );

        let field = "my password";
        assert_eq!(
            serde_yaml::from_str::<PassOrCmd>(field).unwrap(),
            PassOrCmd::Pass(field.to_string())
        );

        let field = "command: \"pass show 'my password'\"";
        assert_eq!(
            serde_yaml::from_str::<PassOrCmd>(field).unwrap(),
            PassOrCmd::Cmd(vec![
                "pass".to_string(),
                "show".to_string(),
                "my password".to_string()
            ])
        );
    }
}

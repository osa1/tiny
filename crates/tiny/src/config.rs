use serde::Deserialize;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Deserialize, Debug)]
pub(crate) struct SASLAuth<P> {
    pub(crate) username: String,
    pub(crate) password: P,
}

#[derive(Clone, Deserialize)]
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

    /// Real name to be used in connection registration
    pub(crate) realname: String,

    /// Nicks to try when connecting to this server. tiny tries these sequentially, and starts
    /// adding trailing underscores to the last one if none of the nicks are available.
    pub(crate) nicks: Vec<String>,

    /// Channels to automatically join.
    #[serde(default)]
    pub(crate) join: Vec<String>,

    /// NickServ identification password. Used on connecting to the server and nick change.
    #[serde(default)]
    pub(crate) nickserv_ident: Option<P>,

    /// Authenication method
    #[serde(rename = "sasl")]
    pub(crate) sasl_auth: Option<SASLAuth<P>>,
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

// TODO FIXME: I don't understand why we need `Default` bound here on `P`
#[derive(Deserialize)]
pub(crate) struct Config<P: Default> {
    pub(crate) servers: Vec<Server<P>>,
    pub(crate) defaults: Defaults,
    pub(crate) log_dir: Option<PathBuf>,
}

/// A password, or a shell command to run the obtain a password. Used for password (server
/// password, SASL, NickServ) fields of `Config`.
#[derive(Debug, Clone)]
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

impl Default for PassOrCmd {
    fn default() -> Self {
        // HACK FIXME TODO - For some reason we need `Default` for `PassOrCmd` to be able to
        // deserialize `Config`. No idea why.
        panic!("default() called for PassOrCmd");
    }
}

impl<'de> Deserialize<'de> for PassOrCmd {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let str = String::deserialize(deserializer)?;
        let trimmed = str.trim();
        if trimmed.starts_with('$') {
            let rest = trimmed[1..].trim(); // drop '$'
            Ok(PassOrCmd::Cmd(
                rest.split_whitespace().map(str::to_owned).collect(),
            ))
        } else {
            Ok(PassOrCmd::Pass(str))
        }
    }
}

fn run_command(command_name: &str, server_addr: &str, args: &[String]) -> Option<String> {
    println!(
        "Running {} command for server {} ({:?}) ...",
        command_name, server_addr, args
    );

    assert!(!args.is_empty()); // should be checked in `validate`

    let mut cmd = Command::new(&args[0]);
    cmd.args(args[1..].iter());

    let output = match cmd.output() {
        Err(err) => {
            println!("Command failed: {:?}", err);
            return None;
        }
        Ok(output) => output,
    };

    if !output.status.success() {
        println!("Command returned non-zero: {:?}", output.status);
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

    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

impl Config<PassOrCmd> {
    /// Returns error descriptions.
    pub(crate) fn validate(&self) -> Vec<String> {
        let mut errors = vec![];

        // Check that nick lists are not empty
        if self.defaults.nicks.is_empty() {
            errors.push(
                "Default nick list can't be empty, please add at least one defaut nick".to_string(),
            );
        }

        for server in &self.servers {
            // TODO: Empty nick strings
            // TODO: Empty realname strings
            if server.nicks.is_empty() {
                errors.push(format!(
                    "Nick list for server '{}' is empty, please add at least one nick",
                    server.addr
                ));
            }

            if let Some(ref pass) = server.pass {
                if pass.is_empty_cmd() {
                    errors.push(format!("Empty PASS command for '{}'", server.addr));
                }
            }

            if let Some(ref nickserv_ident) = server.nickserv_ident {
                if nickserv_ident.is_empty_cmd() {
                    errors.push(format!(
                        "Empty NickServ password command for '{}'",
                        server.addr
                    ));
                }
            }

            if let Some(ref sasl_auth) = server.sasl_auth {
                if sasl_auth.password.is_empty_cmd() {
                    errors.push(format!("Empty SASL password command for '{}'", server.addr));
                }
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
                Some(SASLAuth {
                    username,
                    password: PassOrCmd::Pass(pass),
                }) => Some(SASLAuth {
                    username,
                    password: pass,
                }),
                Some(SASLAuth {
                    username,
                    password: PassOrCmd::Cmd(cmd),
                }) => {
                    let password = run_command("SASL password", &addr, &cmd)?;
                    Some(SASLAuth { username, password })
                }
            };

            servers_.push(Server {
                addr,
                alias,
                port,
                tls,
                pass,
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

pub(crate) fn parse_config(config_path: &Path) -> Result<Config<PassOrCmd>, serde_yaml::Error> {
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
        match serde_yaml::from_str::<Config<PassOrCmd>>(&get_default_config_yaml()) {
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
}

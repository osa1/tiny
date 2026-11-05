use libtiny_client::SASLAuth as ClientSASLAuth;
use serde::{Deserialize, Deserializer};

use shellexpand::LookupError;
use std::env::VarError;
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
#[derive(Clone, Copy, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProxyType {
    Http,
    Https,
    Socks4,
    Socks5,
}

#[derive(Clone, Deserialize)]
#[serde(
    deny_unknown_fields,
    bound(deserialize = "P: serde::de::DeserializeOwned")
)]
pub(crate) struct Proxy<P> {
    #[serde(rename = "type")]
    pub(crate) protocol: ProxyType,
    pub(crate) addr: String,
    pub(crate) port: u16,
    pub(crate) username: Option<String>,
    #[serde(default, deserialize_with = "deserialize_proxy_password")]
    pub(crate) password: Option<P>,
    pub(crate) socks4a: Option<bool>,
}

fn deserialize_proxy_password<'de, D, P>(deserializer: D) -> Result<Option<P>, D::Error>
where
    D: Deserializer<'de>,
    P: serde::de::DeserializeOwned,
{
    let value = Option::<serde_yaml::Value>::deserialize(deserializer)?;
    if let Some(serde_yaml::Value::Mapping(map)) = &value {
        let command = serde_yaml::Value::String("command".to_owned());
        if map.len() != 1 || !map.contains_key(&command) {
            return Err(serde::de::Error::custom("Invalid proxy password command"));
        }
    }
    value
        .map(|value| P::deserialize(value).map_err(serde::de::Error::custom))
        .transpose()
}

impl<P> Proxy<P> {
    fn validate_fields(&self, server_addr: &str, password: Option<&str>) -> Vec<String> {
        let mut errors = Vec::new();
        if self.addr.trim().is_empty() || self.addr.chars().any(char::is_control) {
            errors.push("Invalid proxy host".to_owned());
        }
        if self.port == 0 {
            errors.push("Invalid proxy port".to_owned());
        }
        if self.protocol != ProxyType::Socks4 && self.socks4a.is_some() {
            errors.push("socks4a is only valid for SOCKS4".to_owned());
        }
        let username = self.username.as_deref();
        if self.protocol == ProxyType::Socks4 {
            if self.password.is_some() {
                errors.push("SOCKS4 does not support a password".to_owned());
            }
            if let Some(user) = username
                && (user.is_empty() || user.len() > 255 || user.contains('\0'))
            {
                errors.push("Invalid SOCKS4 USERID".to_owned());
            }
            if self.socks4a.unwrap_or(true)
                && server_addr
                    .len()
                    .saturating_add(username.map_or(0, str::len))
                    > 503
            {
                errors.push("SOCKS4a domain and USERID are too long".to_owned());
            }
        } else {
            if self.username.is_some() != self.password.is_some() {
                errors.push("Proxy username and password must both be specified".to_owned());
            }
            match self.protocol {
                ProxyType::Http | ProxyType::Https => {
                    if username.is_some_and(|s| s.contains(':') || s.chars().any(char::is_control))
                        || password.is_some_and(|s| s.chars().any(char::is_control))
                    {
                        errors.push("Invalid HTTP proxy credentials".to_owned());
                    }
                }
                ProxyType::Socks5 => {
                    if username.is_some_and(|s| s.is_empty() || s.len() > 255)
                        || password.is_some_and(|s| s.is_empty() || s.len() > 255)
                    {
                        errors.push("Invalid SOCKS5 credentials".to_owned());
                    }
                }
                ProxyType::Socks4 => unreachable!(),
            }
        }
        errors
    }
}

impl From<Proxy<String>> for libtiny_client::ProxyConfig {
    fn from(proxy: Proxy<String>) -> Self {
        let protocol = match proxy.protocol {
            ProxyType::Http => libtiny_client::ProxyProtocol::Http,
            ProxyType::Https => libtiny_client::ProxyProtocol::Https,
            ProxyType::Socks4 => libtiny_client::ProxyProtocol::Socks4 {
                remote_dns: proxy.socks4a.unwrap_or(true),
            },
            ProxyType::Socks5 => libtiny_client::ProxyProtocol::Socks5,
        };
        let auth = match (proxy.username, proxy.password) {
            (Some(username), Some(password)) => {
                Some(libtiny_client::ProxyAuth::UserPass { username, password })
            }
            (Some(username), None) => Some(libtiny_client::ProxyAuth::UserId(username)),
            _ => None,
        };
        Self {
            protocol,
            addr: proxy.addr,
            port: proxy.port,
            auth,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(bound(deserialize = "P: serde::de::DeserializeOwned"))]
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

    /// Optional per-server proxy.
    #[serde(default)]
    pub(crate) proxy: Option<Proxy<P>>,

    /// Server password (optional)
    #[serde(default)]
    pub(crate) pass: Option<P>,

    /// Whether to auto-connect to the server
    #[serde(default = "default_true")]
    pub(crate) autoconnect: bool,

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

fn default_true() -> bool {
    true
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
#[serde(bound(deserialize = "P: serde::de::DeserializeOwned"))]
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
                    Err(_) => Err(D::Error::custom("Unable to parse password command")),
                },
                _ => Err(D::Error::custom(
                    "Expected a 'cmd' key in password map with string value",
                )),
            },
            _ => Err(D::Error::custom("Password field must be a string or map")),
        }
    }
}

fn run_command(
    command_name: &str,
    server_addr: &str,
    args: &[String],
    redact: bool,
) -> Option<String> {
    if !redact {
        println!(
            "Running {} command for {} (`{}`)",
            command_name,
            server_addr,
            shell_words::join(args)
        );
    }

    assert!(!args.is_empty()); // should be checked in `validate`

    let mut cmd = Command::new(&args[0]);
    cmd.args(args[1..].iter());

    let output = match cmd.output() {
        Err(err) => {
            if redact {
                println!("Command failed");
            } else {
                println!("Command failed: {err:?}");
            }
            return None;
        }
        Ok(output) => output,
    };

    if !output.status.success() {
        if redact {
            println!("Command returned non-zero");
            return None;
        }
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
    stdout.lines().last().map(str::to_owned).or_else(|| {
        println!("Command returned zero, but stdout is empty. Aborting.");
        None
    })
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

            if let Some(proxy) = &server.proxy {
                let password = match &proxy.password {
                    Some(PassOrCmd::Pass(pass)) => Some(pass.as_str()),
                    Some(PassOrCmd::Cmd(cmd)) => {
                        if cmd.first().is_none_or(String::is_empty) {
                            errors.push("Empty proxy password command".to_owned());
                        }
                        None
                    }
                    None => None,
                };
                errors.extend(proxy.validate_fields(&server.addr, password));
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

    pub(crate) fn expand_fields(
        &mut self,
        home_dir: impl Fn() -> Option<String>,
        env_var: impl Fn(&str) -> Result<Option<String>, VarError>,
    ) -> Result<(), LookupError<VarError>> {
        for server in &mut self.servers {
            server.sasl_auth = match &mut server.sasl_auth {
                None => None,
                Some(other @ SASLAuth::Plain { .. }) => Some(other.clone()),
                Some(SASLAuth::External { pem }) => Some(SASLAuth::External {
                    pem: expand_path(pem.to_path_buf(), &home_dir, &env_var)?,
                }),
            };
        }

        self.log_dir = match &self.log_dir {
            None => None,
            Some(dir) => Some(expand_path(dir.to_path_buf(), &home_dir, &env_var)?),
        };

        Ok(())
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
                proxy,
                pass,
                autoconnect,
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
                Some(PassOrCmd::Cmd(cmd)) => {
                    Some(run_command("server password", &addr, &cmd, false)?)
                }
            };

            let nickserv_ident = match nickserv_ident {
                None => None,
                Some(PassOrCmd::Pass(pass)) => Some(pass),
                Some(PassOrCmd::Cmd(cmd)) => {
                    Some(run_command("NickServ password", &addr, &cmd, false)?)
                }
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
                    let password = run_command("SASL password", &addr, &cmd, false)?;
                    Some(SASLAuth::Plain { username, password })
                }
                Some(SASLAuth::External { pem }) => Some(SASLAuth::External { pem }),
            };

            let proxy = match proxy {
                None => None,
                Some(proxy) => {
                    let password = match proxy.password {
                        None => None,
                        Some(PassOrCmd::Pass(pass)) => Some(pass),
                        Some(PassOrCmd::Cmd(cmd)) => {
                            if cmd.first().is_none_or(String::is_empty) {
                                println!("Empty proxy password command");
                                return None;
                            }
                            let result = run_command("proxy password", &addr, &cmd, true)?;
                            Some(result)
                        }
                    };
                    let proxy = Proxy {
                        protocol: proxy.protocol,
                        addr: proxy.addr,
                        port: proxy.port,
                        username: proxy.username,
                        password,
                        socks4a: proxy.socks4a,
                    };
                    let errors = proxy.validate_fields(&addr, proxy.password.as_deref());
                    if !errors.is_empty() {
                        for error in errors {
                            println!("{error}");
                        }
                        return None;
                    }
                    Some(proxy)
                }
            };

            servers_.push(Server {
                addr,
                alias,
                port,
                tls,
                proxy,
                pass,
                autoconnect,
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

fn expand_path(
    path: PathBuf,
    home_dir: &impl Fn() -> Option<String>,
    env_var: &impl Fn(&str) -> Result<Option<String>, VarError>,
) -> Result<PathBuf, LookupError<VarError>> {
    let path_str = path.to_string_lossy();
    let expanded = shellexpand::full_with_context(&path_str, home_dir, env_var)?;
    Ok(PathBuf::from(expanded.as_ref()))
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

    fn proxy_config(proxy: &str) -> Result<Config<PassOrCmd>, serde_yaml::Error> {
        let yaml = format!(
            "servers:\n  - addr: irc.example\n    port: 6667\n    realname: Real\n    nicks: [nick]\n    join: []\n{proxy}\ndefaults:\n  nicks: [nick]\n  realname: Real\n"
        );
        serde_yaml::from_str(&yaml)
    }

    fn proxy_yaml(kind: &str, fields: &str) -> String {
        format!(
            "    proxy:\n      type: {kind}\n      addr: proxy.example\n      port: 1080\n{fields}"
        )
    }

    #[test]
    fn proxy_legacy_null_and_protocols() {
        for value in ["", "    proxy: null\n"] {
            let config = proxy_config(value).unwrap();
            assert!(config.validate().is_empty());
            assert!(config.read_passwords().unwrap().servers[0].proxy.is_none());
        }
        for kind in ["http", "https", "socks4", "socks5"] {
            let config = proxy_config(&proxy_yaml(kind, "")).unwrap();
            assert!(config.validate().is_empty());
            let proxy = config
                .read_passwords()
                .unwrap()
                .servers
                .remove(0)
                .proxy
                .unwrap();
            let converted: libtiny_client::ProxyConfig = proxy.into();
            assert_eq!(converted.addr, "proxy.example");
            assert!(converted.auth.is_none());
        }
    }

    #[test]
    fn proxy_auth_and_commands() {
        for (kind, fields) in [
            ("http", "      username: user\n      password: pass\n"),
            ("https", "      username: user\n      password: pass\n"),
            ("socks4", "      username: user\n      socks4a: false\n"),
            ("socks5", "      username: user\n      password: pass\n"),
            (
                "socks5",
                "      username: user\n      password:\n        command: \"printf 'first\\\\nlast\\\\n'\"\n",
            ),
        ] {
            let config = proxy_config(&proxy_yaml(kind, fields)).unwrap();
            assert!(config.validate().is_empty());
            let proxy = config
                .read_passwords()
                .unwrap()
                .servers
                .remove(0)
                .proxy
                .unwrap();
            if fields.contains("command:") {
                assert_eq!(proxy.password.as_deref(), Some("last"));
            }
            let converted: libtiny_client::ProxyConfig = proxy.into();
            assert!(converted.auth.is_some());
        }
    }

    #[test]
    fn proxy_rejects_invalid_configuration() {
        for yaml in [
            proxy_yaml("invalid", ""),
            proxy_yaml("socks5", "      extra: true\n"),
            proxy_yaml(
                "socks5",
                "      username: user\n      password:\n        command: 'printf pass'\n        extra: secret\n",
            ),
            "    proxy:\n      type: socks5\n      addr: host\n      port: 65536\n".to_owned(),
        ] {
            assert!(proxy_config(&yaml).is_err());
        }
        let invalid = [
            "    proxy:\n      type: socks5\n      addr: ''\n      port: 1080\n".to_owned(),
            "    proxy:\n      type: socks5\n      addr: \"bad\\nhost\"\n      port: 1080\n"
                .to_owned(),
            "    proxy:\n      type: socks5\n      addr: host\n      port: 0\n".to_owned(),
            proxy_yaml("socks5", "      username: user\n"),
            proxy_yaml("socks5", "      password: pass\n"),
            proxy_yaml(
                "socks5",
                "      username: user\n      password: pass\n      socks4a: false\n",
            ),
            proxy_yaml("socks5", "      username: ''\n      password: pass\n"),
            proxy_yaml("socks5", "      username: user\n      password: ''\n"),
            proxy_yaml(
                "socks5",
                &format!(
                    "      username: '{}'\n      password: pass\n",
                    "a".repeat(256)
                ),
            ),
            proxy_yaml(
                "socks5",
                "      username: user\n      password:\n        command: ''\n",
            ),
            proxy_yaml(
                "socks5",
                "      username: user\n      password:\n        command: \"''\"\n",
            ),
            proxy_yaml("socks4", "      password: pass\n"),
            proxy_yaml("socks4", "      username: \"bad\\0user\"\n"),
            proxy_yaml("http", "      username: 'bad:user'\n      password: pass\n"),
            proxy_yaml(
                "https",
                "      username: user\n      password: \"bad\\npass\"\n",
            ),
        ];
        for yaml in invalid {
            let config = proxy_config(&yaml).unwrap();
            assert!(!config.validate().is_empty(), "{yaml}");
        }
        let mut config = proxy_config(&proxy_yaml("socks4", "      username: user\n")).unwrap();
        config.servers[0].addr = "a".repeat(500);
        assert!(!config.validate().is_empty());
    }

    #[test]
    fn proxy_rejects_invalid_command_output_and_redacts_parse_error() {
        for args in [
            vec!["printf".to_owned(), "".to_owned()],
            vec!["printf".to_owned(), "\\n".to_owned()],
            vec!["printf".to_owned(), "x".repeat(256)],
        ] {
            let mut config = proxy_config(&proxy_yaml(
                "socks5",
                "      username: user\n      password: pass\n",
            ))
            .unwrap();
            config.servers[0].proxy.as_mut().unwrap().password = Some(PassOrCmd::Cmd(args));
            assert!(config.validate().is_empty());
            assert!(config.read_passwords().is_none());
        }
        let mut basic = proxy_config(&proxy_yaml(
            "http",
            "      username: user\n      password: pass\n",
        ))
        .unwrap();
        basic.servers[0].proxy.as_mut().unwrap().password =
            Some(PassOrCmd::Cmd(vec!["printf".to_owned(), "\\n".to_owned()]));
        assert_eq!(
            basic.read_passwords().unwrap().servers[0]
                .proxy
                .as_ref()
                .unwrap()
                .password
                .as_deref(),
            Some("")
        );
        let yaml = proxy_yaml(
            "socks5",
            "      username: user\n      password:\n        command: \"'secret-marker\"\n",
        );
        let error = match proxy_config(&yaml) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("invalid command accepted"),
        };
        assert!(!error.contains("secret-marker"));
    }

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
                proxy: None,
                pass: None,
                autoconnect: true,
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

    #[test]
    fn config_shell_expansion() {
        let mut config: Config<PassOrCmd> = Config {
            servers: vec![Server {
                addr: "my_server".to_owned(),
                alias: None,
                port: 123,
                tls: false,
                proxy: None,
                pass: None,
                autoconnect: true,
                user: None,
                realname: "".to_owned(),
                nicks: vec!["".to_owned()],
                join: vec![],
                nickserv_ident: None,
                sasl_auth: Some(SASLAuth::External {
                    pem: "~/a/$SASL/b".into(),
                }),
            }],
            defaults: Defaults {
                nicks: vec!["".to_owned()],
                realname: "".to_owned(),
                join: vec![],
                tls: false,
            },
            log_dir: Some("~/b/$LOG/c".into()),
        };
        config
            .expand_fields(
                || Some("/home/test".to_string()),
                |s| match s {
                    "SASL" => Ok(Some("sasl_val".to_string())),
                    "LOG" => Ok(Some("log_val".to_string())),
                    _ => Err(VarError::NotPresent),
                },
            )
            .unwrap();

        assert_eq!(
            config.servers[0].sasl_auth,
            Some(SASLAuth::External {
                pem: PathBuf::from("/home/test/a/sasl_val/b"),
            })
        );
        assert_eq!(
            config.log_dir,
            Some(PathBuf::from("/home/test/b/log_val/c"))
        );
    }

    #[test]
    fn config_shell_expansion_var_fail() {
        let mut config: Config<PassOrCmd> = Config {
            servers: vec![],
            defaults: Defaults {
                nicks: vec!["nick".to_owned()],
                realname: "real".to_owned(),
                join: vec![],
                tls: false,
            },
            log_dir: Some("~/logs/$MISSING/data".into()),
        };
        let err = config
            .expand_fields(
                || Some("/home/test".to_string()),
                |_| Err(VarError::NotPresent),
            )
            .unwrap_err();
        assert_eq!(err.var_name, "MISSING");
        assert_eq!(err.cause, VarError::NotPresent);
    }
}

use std::path::PathBuf;

use clap::{crate_authors, crate_description, crate_name, crate_version, Arg, Command};

/// Command line arguments.
#[derive(Debug)]
pub(crate) struct Args {
    /// Patterns for server names. For example, when this has strings `"foo"` and `"bar"`, we'll
    /// connect to servers whose names contain have "foo" *or* "bar". When empty we connect to all
    /// servers specified in the config file.
    pub(crate) servers: Vec<String>,

    /// Path to the config file. When not specified `config::get_config_path` is used to find the
    /// config file.
    pub(crate) config_path: Option<PathBuf>,
}

/// Parses command line arguments.
pub(crate) fn parse() -> Args {
    let mut version = crate_version!().to_owned();
    let commit_hash = env!("GIT_HASH");
    if !commit_hash.is_empty() {
        version = format!("{} ({})", version, commit_hash);
    }

    let m = Command::new(crate_name!())
        .version(version.as_str())
        .about(crate_description!())
        .author(crate_authors!())
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Use this config file")
                .takes_value(true),
        )
        .arg(
            Arg::new("servers")
                .multiple_values(true)
                .takes_value(true)
                .help(
                    "Servers to connect. For example, `tiny foo bar` \
                     connects to servers whose names contain \"foo\" OR \
                     \"bar\".",
                )
                .next_line_help(false),
        )
        .get_matches();

    let servers = match m.values_of("servers") {
        None => vec![],
        Some(vals) => vals.map(str::to_owned).collect(),
    };

    let config_path = m.value_of("config").map(PathBuf::from);

    Args {
        servers,
        config_path,
    }
}

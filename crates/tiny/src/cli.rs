use std::path::PathBuf;

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

/// Parses command line arguments and handles `--version` and `--help`.
pub(crate) fn parse() -> Args {
    let mut servers: Vec<String> = Vec::new();
    let mut config_path: Option<PathBuf> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "-V" || arg == "--version" {
            print_version();
            std::process::exit(0);
        }

        if arg == "-h" || arg == "--help" {
            print_help();
            std::process::exit(0);
        }

        if arg == "-c" || arg == "--config" {
            match args.next() {
                Some(path) => {
                    config_path = Some(path.into());
                    continue;
                }

                None => {
                    eprintln!(
                        "Error: The argument '--config <FILE>' requires a file path but none was supplied"
                    );
                    eprintln!();
                    eprintln!("For more information try --help");
                    std::process::exit(1);
                }
            }
        }

        if arg.starts_with('-') {
            eprintln!("Error: Found argument '{arg}' which wasn't expected");
            eprintln!();
            eprintln!("For more information try --help");
            std::process::exit(1);
        }

        servers.push(arg);
    }

    Args {
        servers,
        config_path,
    }
}

fn print_version() {
    let crate_version = env!("CARGO_PKG_VERSION");
    let commit_hash = env!("GIT_HASH");
    println!("tiny {crate_version} ({commit_hash})");
}

fn print_help() {
    print_version();
    let crate_authors = env!("CARGO_PKG_AUTHORS");
    let crate_description = env!("CARGO_PKG_DESCRIPTION");
    println!(
        "\
{crate_authors}
{crate_description}

USAGE:
    tiny [OPTIONS] [servers]

ARGS:
    <servers>       Servers to connect. For example, `tiny foo bar` connects to servers whose
                    names contain \"foo\" OR \"bar\".

OPTIONS:
    -c, --config <FILE>    Use this config file
    -h, --help             Print help information
    -V, --version          Print version information",
    )
}

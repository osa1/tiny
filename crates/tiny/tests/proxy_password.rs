use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        loop {
            let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("tiny-proxy-password-{}-{id}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not create temporary directory: {error}"),
            }
        }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn failed_proxy_password_command_does_not_disclose_secrets() {
    const ARG_SECRET: &str = "tiny_arg_secret_3f821c";
    const STDOUT_SECRET: &str = "tiny_stdout_secret_4e392d";
    const STDERR_SECRET: &str = "tiny_stderr_secret_5a403e";

    let dir = TempDir::new();
    let script = dir.0.join("password-command.sh");
    fs::write(
        &script,
        format!(
            "printf '%s\\n' '{STDOUT_SECRET}'\nprintf '%s\\n' '{STDERR_SECRET}' >&2\nexit 23\n"
        ),
    )
    .unwrap();
    let config = dir.0.join("config.yml");
    let script_arg = shell_words::quote(script.to_str().expect("temporary path is UTF-8"));
    fs::write(
        &config,
        format!(
            "servers:\n  - addr: irc.example\n    port: 6697\n    realname: Test User\n    nicks: [tester]\n    join: []\n    proxy:\n      type: socks5\n      addr: proxy.example\n      port: 1080\n      username: proxy-user\n      password:\n        command: >-\n          /bin/sh {script_arg} {ARG_SECRET}\ndefaults:\n  nicks: [tester]\n  realname: Test User\nlog_dir: null\n"
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_tiny"))
        .arg("--config")
        .arg(&config)
        .current_dir(&dir.0)
        .output()
        .expect("failed to launch tiny");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.status.success(),
        "tiny unexpectedly succeeded: {combined}"
    );
    assert!(
        combined.contains("Command returned non-zero"),
        "missing command failure diagnostic: {combined}"
    );
    for secret in [ARG_SECRET, STDOUT_SECRET, STDERR_SECRET] {
        assert!(
            !combined.contains(secret),
            "proxy command leaked a secret: {combined}"
        );
    }
}

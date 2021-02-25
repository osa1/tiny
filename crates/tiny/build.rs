fn main() {
    let hash = rustc_tools_util::get_commit_hash().unwrap_or_default();
    println!("cargo:rustc-env=GIT_HASH={}", hash);
}

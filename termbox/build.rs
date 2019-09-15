fn main() {
    cc::Build::new()
        .file("cbits/termbox.c")
        .include("cbits")
        .define("_XOPEN_SOURCE", None)
        .compile("libtermbox.a");
    println!("cargo:rustc-flags=-l static=termbox");
}

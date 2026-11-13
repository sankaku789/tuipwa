fn main() {
    let gio = pkg_config::Config::new()
        // GIO/GLib stay OS-provided shared libraries, not bundled static code.
        .statik(false)
        .probe("gio-2.0")
        .expect("gio-2.0 development files");
    let mut c = cc::Build::new();
    c.file("src/platform/linux/ipc.c");
    for path in gio.include_paths {
        c.include(path);
    }
    c.compile("tuipwa_ipc");
    println!("cargo:rerun-if-changed=src/platform/linux/ipc.c");
}

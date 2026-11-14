fn main() {
    // tauri-build embeds icons/icon.ico into the exe as a Windows resource, but it
    // only watches tauri.conf.json: regenerating the icon with demo/make_icon.py
    // would otherwise leave the old art in the binary until a clean build.
    println!("cargo:rerun-if-changed=icons/icon.ico");
    println!("cargo:rerun-if-changed=icons/icon.png");
    println!("cargo:rerun-if-changed=icons/tray.png");
    tauri_build::build()
}

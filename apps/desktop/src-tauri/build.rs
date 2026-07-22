fn main() {
    // Cargo does not automatically rebuild when bundle assets change.
    // Keep the debug app icon in sync with the generated Tauri resources.
    for path in [
        "../src/assets/happyjlc.svg",
        "icons/32x32.png",
        "icons/128x128.png",
        "icons/128x128@2x.png",
        "icons/icon.icns",
        "icons/icon.ico",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    tauri_build::build()
}

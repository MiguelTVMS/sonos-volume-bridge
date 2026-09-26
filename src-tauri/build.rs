fn main() {
    tauri_build::build();
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Tauri embeds resources only in app binaries. Native library tests also
        // use its controls and need the v6 activation context at process startup.
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
        // App binaries already have Tauri's full manifest in resource.lib.
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
    }
}

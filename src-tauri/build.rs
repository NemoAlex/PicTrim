fn main() {
    #[cfg(not(target_os = "windows"))]
    {
        pkg_config::Config::new()
            .probe("vips")
            .expect("libvips development files were not found. Install libvips and ensure pkg-config can find vips.pc.");
    }

    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-lib=dylib=libvips");
    }

    tauri_build::build()
}

fn main() {
    pkg_config::Config::new()
        .probe("vips")
        .expect("libvips development files were not found. Install libvips and ensure pkg-config can find vips.pc.");
    tauri_build::build()
}

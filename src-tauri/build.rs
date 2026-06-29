fn main() {
    #[cfg(not(target_os = "windows"))]
    {
        pkg_config::Config::new()
            .probe("vips")
            .expect("libvips development files were not found. Install libvips and ensure pkg-config can find vips.pc.");
    }

    #[cfg(target_os = "windows")]
    {
        // Find vips: use VIPS_DIR if set, otherwise search PATH for vips.exe
        // and derive lib/bin from its parent directory.
        let (lib_dir, bin_dir) = if let Ok(vips_dir) = std::env::var("VIPS_DIR") {
            (format!("{}\\lib", vips_dir), format!("{}\\bin", vips_dir))
        } else {
            let path = std::env::var("PATH").unwrap_or_default();
            let bin = path.split(';')
                .find(|p| std::path::Path::new(p).join("vips.exe").exists());
            match bin {
                Some(bin_dir) => {
                    let lib_dir = std::path::Path::new(bin_dir)
                        .parent()
                        .map(|p| p.join("lib").to_string_lossy().to_string())
                        .unwrap_or_else(|| format!("{}\\..\\lib", bin_dir));
                    (lib_dir, bin_dir.to_string())
                }
                None => panic!("Could not find vips.exe in PATH. Add vips bin directory to PATH or set VIPS_DIR."),
            }
        };
        println!("cargo:rustc-link-search={}", lib_dir);
        println!("cargo:rustc-link-lib=dylib=libvips");
        println!("cargo:rustc-link-lib=dylib=libgobject-2.0");
        println!("cargo:rustc-link-lib=dylib=libglib-2.0");
        // Add bin dir to PATH so the DLL can be found at runtime
        let current_path = std::env::var("PATH").unwrap_or_default();
        println!("cargo:rustc-env=PATH={};{}", bin_dir, current_path);
    }

    tauri_build::build()
}

use std::path::{Path, PathBuf};

const QPDF_SOURCES: &[&str] = &[
    "AES_PDF_native.cc",
    "BitStream.cc",
    "BitWriter.cc",
    "Buffer.cc",
    "BufferInputSource.cc",
    "ClosedFileInputSource.cc",
    "ContentNormalizer.cc",
    "CryptoRandomDataProvider.cc",
    "FileInputSource.cc",
    "InputSource.cc",
    "InsecureRandomDataProvider.cc",
    "JSON.cc",
    "JSONHandler.cc",
    "MD5.cc",
    "MD5_native.cc",
    "NNTree.cc",
    "OffsetInputSource.cc",
    "PDFVersion.cc",
    "Pipeline.cc",
    "Pl_AES_PDF.cc",
    "Pl_ASCII85Decoder.cc",
    "Pl_ASCIIHexDecoder.cc",
    "Pl_Base64.cc",
    "Pl_Buffer.cc",
    "Pl_Concatenate.cc",
    "Pl_Count.cc",
    "Pl_DCT.cc",
    "Pl_Discard.cc",
    "Pl_Flate.cc",
    "Pl_Function.cc",
    "Pl_LZWDecoder.cc",
    "Pl_OStream.cc",
    "Pl_PNGFilter.cc",
    "Pl_QPDFTokenizer.cc",
    "Pl_RC4.cc",
    "Pl_RunLength.cc",
    "Pl_SHA2.cc",
    "Pl_StdioFile.cc",
    "Pl_String.cc",
    "Pl_TIFFPredictor.cc",
    "QPDF.cc",
    "QPDFAcroFormDocumentHelper.cc",
    "QPDFAnnotationObjectHelper.cc",
    "QPDFArgParser.cc",
    "QPDFCryptoProvider.cc",
    "QPDFCrypto_native.cc",
    "QPDFDocumentHelper.cc",
    "QPDFEFStreamObjectHelper.cc",
    "QPDFEmbeddedFileDocumentHelper.cc",
    "QPDFExc.cc",
    "QPDFFileSpecObjectHelper.cc",
    "QPDFFormFieldObjectHelper.cc",
    "QPDFJob.cc",
    "QPDFJob_argv.cc",
    "QPDFJob_config.cc",
    "QPDFJob_json.cc",
    "QPDFLogger.cc",
    "QPDFMatrix.cc",
    "QPDFObject.cc",
    "QPDFObjectHandle.cc",
    "QPDFObjectHelper.cc",
    "QPDFOutlineDocumentHelper.cc",
    "QPDFOutlineObjectHelper.cc",
    "QPDFPageLabelDocumentHelper.cc",
    "QPDFPageObjectHelper.cc",
    "QPDFParser.cc",
    "QPDFStreamFilter.cc",
    "QPDFSystemError.cc",
    "QPDFTokenizer.cc",
    "QPDFUsage.cc",
    "QPDFWriter.cc",
    "QPDF_Array.cc",
    "QPDF_Dictionary.cc",
    "QPDF_Stream.cc",
    "QPDF_String.cc",
    "QPDF_encryption.cc",
    "QPDF_json.cc",
    "QPDF_linearization.cc",
    "QPDF_objects.cc",
    "QPDF_pages.cc",
    "QTC.cc",
    "QUtil.cc",
    "RC4.cc",
    "RC4_native.cc",
    "ResourceFinder.cc",
    "SF_FlateLzwDecode.cc",
    "SHA2_native.cc",
    "SecureRandomDataProvider.cc",
    "global.cc",
    "qpdf-c.cc",
    "qpdfjob-c.cc",
    "qpdflogger-c.cc",
    "rijndael.cc",
];

const ZLIB_SOURCES: &[&str] = &[
    "adler32.c",
    "compress.c",
    "crc32.c",
    "deflate.c",
    "gzclose.c",
    "gzlib.c",
    "gzread.c",
    "gzwrite.c",
    "infback.c",
    "inffast.c",
    "inflate.c",
    "inftrees.c",
    "trees.c",
    "uncompr.c",
    "zutil.c",
];

const JPEG_SOURCES: &[&str] = &[
    "jaricom.c",
    "jcapimin.c",
    "jcapistd.c",
    "jcarith.c",
    "jccoefct.c",
    "jccolor.c",
    "jcdctmgr.c",
    "jchuff.c",
    "jcinit.c",
    "jcmainct.c",
    "jcmarker.c",
    "jcmaster.c",
    "jcomapi.c",
    "jcparam.c",
    "jcprepct.c",
    "jcsample.c",
    "jctrans.c",
    "jdapimin.c",
    "jdapistd.c",
    "jdarith.c",
    "jdatadst.c",
    "jdatasrc.c",
    "jdcoefct.c",
    "jdcolor.c",
    "jddctmgr.c",
    "jdhuff.c",
    "jdinput.c",
    "jdmainct.c",
    "jdmarker.c",
    "jdmaster.c",
    "jdmerge.c",
    "jdpostct.c",
    "jdsample.c",
    "jdtrans.c",
    "jerror.c",
    "jfdctflt.c",
    "jfdctfst.c",
    "jfdctint.c",
    "jidctflt.c",
    "jidctfst.c",
    "jidctint.c",
    "jquant1.c",
    "jquant2.c",
    "jutils.c",
    "jmemmgr.c",
    "jmemnobs.c",
];

fn paths(root: &Path, names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(|name| root.join(name)).collect()
}

fn build_pdf_native() {
    let qpdf = Path::new("vendor/qpdf-12.3.2");
    let qpdf_src = qpdf.join("libqpdf");
    let zlib = Path::new("vendor/zlib-1.3.1");
    let jpeg = Path::new("vendor/jpeg-9f");

    let mut zlib_build = cc::Build::new();
    zlib_build
        .files(paths(zlib, ZLIB_SOURCES))
        .include(zlib)
        .warnings(false);
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        zlib_build.define("HAVE_UNISTD_H", Some("1"));
    }
    zlib_build.compile("pictrim_zlib");

    cc::Build::new()
        .files(paths(jpeg, JPEG_SOURCES))
        .include(jpeg)
        .define("JPEG_INTERNALS", None)
        .warnings(false)
        .compile("pictrim_jpeg");

    cc::Build::new()
        .files([qpdf_src.join("sha2.c"), qpdf_src.join("sha2big.c")])
        .include(qpdf.join("include"))
        .include(&qpdf_src)
        .warnings(false)
        .compile("pictrim_qpdf_c");

    let mut qpdf_build = cc::Build::new();
    qpdf_build
        .cpp(true)
        .files(paths(&qpdf_src, QPDF_SOURCES))
        .file("native/qpdf_bridge/pictrim_qpdf.cc")
        .include(qpdf.join("include"))
        .include(&qpdf_src)
        .include(zlib)
        .include(jpeg)
        .include("native/qpdf_bridge")
        .define("QPDF_DISABLE_QTC", Some("1"))
        .warnings(false);
    if qpdf_build.get_compiler().is_like_msvc() {
        qpdf_build.flag("/std:c++20").flag("/EHsc").flag("/bigobj");
    } else {
        qpdf_build.flag("-std=c++20");
    }
    qpdf_build.compile("pictrim_qpdf");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-lib=advapi32");
    }
    println!("cargo:rerun-if-changed=native/qpdf_bridge");
    println!("cargo:rerun-if-changed=vendor/qpdf-12.3.2");
    println!("cargo:rerun-if-changed=vendor/zlib-1.3.1");
    println!("cargo:rerun-if-changed=vendor/jpeg-9f");
}

fn main() {
    println!("cargo:rerun-if-changed=icons");

    build_pdf_native();

    #[cfg(not(target_os = "windows"))]
    {
        pkg_config::Config::new()
            .probe("vips")
            .expect("libvips development files were not found. Install libvips and ensure pkg-config can find vips.pc.");
    }

    #[cfg(target_os = "windows")]
    {
        // Find vips: use VIPS_DIR/VCPKG_ROOT if set, otherwise search PATH for vips.exe
        // and derive lib/bin from its parent directory.
        let (lib_dir, bin_dir) = if let Ok(vips_dir) = std::env::var("VIPS_DIR") {
            (format!("{}\\lib", vips_dir), format!("{}\\bin", vips_dir))
        } else if let Ok(vcpkg_root) = std::env::var("VCPKG_ROOT") {
            (
                format!("{}\\installed\\x64-windows\\lib", vcpkg_root),
                format!("{}\\installed\\x64-windows\\bin", vcpkg_root),
            )
        } else {
            let path = std::env::var("PATH").unwrap_or_default();
            let bin = path
                .split(';')
                .find(|p| std::path::Path::new(p).join("vips.exe").exists());
            match bin {
                Some(bin_dir) => {
                    let lib_dir = std::path::Path::new(bin_dir)
                        .parent()
                        .map(|p| p.join("lib").to_string_lossy().to_string())
                        .unwrap_or_else(|| format!("{}\\..\\lib", bin_dir));
                    (lib_dir, bin_dir.to_string())
                }
                None => panic!("Could not find vips.exe in PATH. Add vips bin directory to PATH, or set VIPS_DIR/VCPKG_ROOT."),
            }
        };
        println!("cargo:rustc-link-search={}", lib_dir);
        println!("cargo:rustc-link-lib=dylib=libvips");
        println!("cargo:rustc-link-lib=dylib=libgobject-2.0");
        println!("cargo:rustc-link-lib=dylib=libglib-2.0");
        println!("cargo:rerun-if-env-changed=VIPS_DIR");
        println!("cargo:rerun-if-env-changed=VCPKG_ROOT");
        println!("cargo:rerun-if-changed={}", bin_dir);
    }

    #[cfg(all(target_os = "windows", target_env = "msvc"))]
    {
        // Restrict release bundle DLL loads to the application directory and System32.
        // Debug test binaries still need to load the CI-installed libvips DLLs from PATH.
        if std::env::var("PROFILE").as_deref() == Ok("release") {
            println!("cargo:rustc-link-arg=/DEPENDENTLOADFLAG:0xA00");
        }
    }

    tauri_build::build()
}

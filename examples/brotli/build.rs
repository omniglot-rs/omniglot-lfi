// -*- fill-column: 80; -*-

use std::path::{Path, PathBuf};

fn main() {
    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // Path::new is not yet stable as const
    const LOCAL_OG_BROTLI_LFI_BUILD_DIR: &str = "./og_brotli_lfi/build";

    // If there is a local build of `og_brotli_lfi` use that, otherwise download
    // and/or extract a prebuilt version:
    let og_brotli_lfi_build_path = if Path::new(LOCAL_OG_BROTLI_LFI_BUILD_DIR).exists() {
        println!(
            "cargo:warning=Using local og_brotli_lfi build from {}",
            LOCAL_OG_BROTLI_LFI_BUILD_DIR
        );
        PathBuf::from(LOCAL_OG_BROTLI_LFI_BUILD_DIR)
    } else {
        let target_path = out_path.join("og_brotli_lfi_prebuilt");

        fetch_prebuilt::fetch_prebuilt(
            // Archive name:
            "og_brotli_lfi",
            // Archive path environment variable. If this is set, then the
            // archive will be copied from this path, otherwise it will be
            // fetched from the URL specified in the manifest:
            "OG_BROTLI_LFI_PREBUILT_ARCHIVE",
            // Target path, to unpack the archive at:
            &target_path,
            // Path to store the checksum of the unpacked archive, used for
            // caching purposes:
            &out_path.join("og_brotli_lfi_prebuilt_archive_sha256.txt"),
            // Path to the JSON manifest containing the archive's URL and
            // SHA-256 checksum:
            &fetch_prebuilt::read_archive_manifest(
                // Archive name, for panic messages:
                "og_brotli_lfi",
                &Path::new("og_brotli_lfi_prebuilt.json")
                    .canonicalize()
                    .unwrap(),
            ),
        );

        // Return path to the unpacked archive:
        target_path
    };

    // To allow the actual crate to find the built LFI binaries:
    println!(
        "cargo:rustc-env=OG_BROTLI_LFI_BUILD_PATH={}",
        og_brotli_lfi_build_path.canonicalize().unwrap().display()
    );

    let bindings = bindgen::Builder::default()
        .header("og_brotli_lfi/og_brotli_lfi.h")
        .clang_args(&[
            "--sysroot",
            &format!(
                "{}",
                og_brotli_lfi_build_path
                    .join("brotli_lfi/install")
                    .display()
            ),
        ])
        .omniglot_configuration_file(Some(
            PathBuf::from("./brotli.omniglot.toml")
                .canonicalize()
                .unwrap(),
        ))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("brotli_bindings.rs"))
        .expect("Couldn't write bindings!");

    // Link to statically built native Brotli libs for "unsafe" benchmark:
    println!(
        "cargo:rustc-link-search={}",
        og_brotli_lfi_build_path
            .join("brotli_native/install/lib")
            .canonicalize()
            .unwrap()
            .display()
    );
    println!("cargo:rustc-link-lib=static=brotlienc");
    println!("cargo:rustc-link-lib=static=brotlidec");
    println!("cargo:rustc-link-lib=static=brotlicommon");
}

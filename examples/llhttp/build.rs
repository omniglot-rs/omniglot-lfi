// -*- fill-column: 80; -*-

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=./llhttp.omniglot.toml");
    println!("cargo:rerun-if-changed=./og_llhttp_lfi_prebuilt.json");
    println!("cargo:rerun-if-changed=./og_llhttp_lfi");

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // Path::new is not yet stable as const
    const LOCAL_OG_LLHTTP_LFI_BUILD_DIR: &str = "./og_llhttp_lfi/build";

    // If there is a local build of `og_llhttp_lfi` use that, otherwise download
    // and/or extract a prebuilt version:
    let og_llhttp_lfi_build_path = if Path::new(LOCAL_OG_LLHTTP_LFI_BUILD_DIR).exists() {
        println!(
            "cargo:warning=Using local og_llhttp_lfi build from {}",
            LOCAL_OG_LLHTTP_LFI_BUILD_DIR
        );
        PathBuf::from(LOCAL_OG_LLHTTP_LFI_BUILD_DIR)
    } else {
        // Save to a temporary directory. Cargo is too aggressive about cleaning
        // the `OUT_DIR`, which causes frequent re-downloads:
        let target_path_base = std::env::temp_dir().join("omniglot-lfi-example-llhttp");
        let target_path = target_path_base.join("og_llhttp_lfi_prebuilt");

        fetch_prebuilt::fetch_prebuilt(
            // Archive name:
            "og_llhttp_lfi",
            // Archive path environment variable. If this is set, then the
            // archive will be copied from this path, otherwise it will be
            // fetched from the URL specified in the manifest:
            "OG_LLHTTP_LFI_PREBUILT_ARCHIVE",
            // Target path, to unpack the archive at:
            &target_path,
            // Path to store the checksum of the unpacked archive, used for
            // caching purposes:
            &target_path_base.join("og_llhttp_lfi_prebuilt_archive_sha256.txt"),
            // Path to the JSON manifest containing the archive's URL and
            // SHA-256 checksum:
            &fetch_prebuilt::read_archive_manifest(
                // Archive name, for panic messages:
                "og_llhttp_lfi",
                &Path::new("og_llhttp_lfi_prebuilt.json")
                    .canonicalize()
                    .unwrap(),
            ),
        );

        // Return path to the unpacked archive:
        target_path
    };

    // To allow the actual crate to find the built LFI binaries:
    println!(
        "cargo:rustc-env=OG_LLHTTP_LFI_BUILD_PATH={}",
        og_llhttp_lfi_build_path.canonicalize().unwrap().display()
    );

    let bindings = bindgen::Builder::default()
        .header("og_llhttp_lfi/og_llhttp_lfi.h")
        .clang_args(&[
            "--sysroot",
            &format!(
                "{}",
                og_llhttp_lfi_build_path
                    .join("llhttp_lfi/install")
                    .display()
            ),
        ])
        .omniglot_configuration_file(Some(
            PathBuf::from("./llhttp.omniglot.toml")
                .canonicalize()
                .unwrap(),
        ))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("llhttp_bindings.rs"))
        .expect("Couldn't write bindings!");

    // Link to statically built native Llhttp libs for "unsafe" benchmark:
    println!(
        "cargo:rustc-link-search={}",
        og_llhttp_lfi_build_path
            .join("llhttp_native/install/lib")
            .canonicalize()
            .unwrap()
            .display()
    );
    println!("cargo:rustc-link-lib=static=llhttp");
}

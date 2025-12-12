// -*- fill-column: 80; -*-

use sha2::{Digest, Sha256};
use std::env;
use std::io::Cursor;
use std::path::{Path, PathBuf};

fn main() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

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
        let target_archive_checksum_file =
            out_path.join("og_brotli_lfi_prebuilt_archive_sha256.txt");

        let prebuilt_manifest_json = std::fs::read_to_string("og_brotli_lfi_prebuilt.json")
            .expect("Failed to read prebuilt og_brotli_lfi archive manifest");
        let prebuilt_manifest = json::parse(&prebuilt_manifest_json)
            .expect("Failed to parse prebuilt og_brotli_lfi archive manifest");
        let prebuilt_url = prebuilt_manifest["url"]
            .as_str()
            .expect("Field 'url' missing or invalid in prebuilt og_brotli_lfi archive manifest");
        let prebuilt_expected_sha256 = hex::decode(prebuilt_manifest["sha256"].as_str().expect(
            "Field 'sha256' missing or invalid in prebuilt og_brotli_lfi archive manifest",
        ))
        .expect("Field 'sha256' of prebuilt og_brotli_lfi archive manifest is not valid hex");

        // Ensure that the checksum of the unpacked file corresponds to the one
        // we expect, otherwise remove the `target_path` and re-fetch / unpack
        // the archive:
        let cached_csum = std::fs::read_to_string(&target_archive_checksum_file)
            .ok()
            .and_then(|csum_str| hex::decode(csum_str).ok());
        match cached_csum {
            None => {
                // No cached checksum written. We may have a partially extracted
                // archive, attempt to delete:
                let _ = std::fs::remove_dir_all(&target_path);
            }
            Some(csum) if csum != prebuilt_expected_sha256 => {
                // Checksum file does not exist or contains incorrect checksum, re-fetch:
                println!(
                    "cargo:warning=Cached og_brotli_lfi was unpacked from \
		     archive with different checksum, re-fetching (expected {} \
		     vs. cached {})",
                    hex::encode(&prebuilt_expected_sha256),
                    hex::encode(csum)
                );
                std::fs::remove_file(&target_archive_checksum_file).unwrap();
                let _ = std::fs::remove_dir_all(&target_path);
            }
            Some(_) => {
                // Cached & unpacked archive either does not exist or is
                // current, do nothing.
            }
        }

        // Re-fetch if `target_path` did not exist (or we've just deleted it):
        if target_path.exists() {
            println!("cargo:warning=Using cached og_brotli_lfi archive");
        } else {
            let prebuilt_archive_bytes = match env::var("OG_BROTLI_LFI_PREBUILT_ARCHIVE") {
                Ok(path) => {
                    println!(
                        "cargo:warning=Using prebuilt og_brotli_lfi archive from {}",
                        path
                    );
                    std::fs::read(path).expect(
			"Failed to read prebuilt og_brotli_lfi archive from OG_BROTLI_LFI_PREBUILT_ARCHIVE",
		    )
                }
                Err(_) => {
                    println!(
                        "cargo:warning=Using prebuilt og_brotli_lfi archive from {}",
                        prebuilt_url
                    );
                    reqwest::blocking::get(prebuilt_url)
                        .expect("Failed to download prebuilt archive")
                        .bytes()
                        .expect("Failed to read response bytes")
                        .to_vec()
                }
            };

            let mut hasher = Sha256::new();
            hasher.update(&prebuilt_archive_bytes);
            let prebuilt_actual_sha256 = hasher.finalize();
            if *prebuilt_actual_sha256 != *prebuilt_expected_sha256 {
                panic!(
                    "SHA256 mismatch for prebuilt archive. Expected {}, got {}",
                    hex::encode(prebuilt_expected_sha256),
                    hex::encode(prebuilt_actual_sha256),
                );
            }

            // Extract archive
            let prebuilt_archive_tar =
                flate2::read::GzDecoder::new(Cursor::new(prebuilt_archive_bytes));
            let mut prebuilt_archive = tar::Archive::new(prebuilt_archive_tar);
            prebuilt_archive
                .unpack(&target_path)
                .expect("Failed to unpack archive");

            // Write the checksum of the archive to
            // `target_archive_checksum_file`, which allows us to avoid fetching
            // the archive repeatedly if the checksum doesn't change:
            std::fs::write(
                &target_archive_checksum_file,
                hex::encode(prebuilt_actual_sha256).as_bytes(),
            )
            .unwrap();
        }

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

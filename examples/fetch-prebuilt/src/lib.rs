// -*- fill-column: 80; -*-

use sha2::{Digest as _, Sha256};
use std::env;
use std::io::Cursor;
use std::path::Path;

pub struct ArchiveManifest {
    pub url: String,
    pub sha256: Vec<u8>,
}

pub fn read_archive_manifest(archive_name: &str, archive_manifest_path: &Path) -> ArchiveManifest {
    let prebuilt_manifest_json = std::fs::read_to_string(archive_manifest_path)
        .unwrap_or_else(|_| panic!("Failed to read prebuilt {archive_name} archive manifest"));
    let prebuilt_manifest = json::parse(&prebuilt_manifest_json)
        .unwrap_or_else(|_| panic!("Failed to parse prebuilt {archive_name} archive manifest"));
    let url = prebuilt_manifest["url"]
        .as_str()
        .unwrap_or_else(|| {
            panic!("Field 'url' missing or invalid in prebuilt {archive_name} archive manifest")
        })
        .to_string();
    let sha256 = hex::decode(prebuilt_manifest["sha256"].as_str().unwrap_or_else(|| {
        panic!("Field 'sha256' missing or invalid in prebuilt {archive_name} archive manifest")
    }))
    .unwrap_or_else(|_| {
        panic!("Field 'sha256' of prebuilt {archive_name} archive manifest is not valid hex")
    });

    ArchiveManifest { url, sha256 }
}

pub fn fetch_prebuilt(
    archive_name: &str,
    archive_path_env_var: &str,
    target_path: &Path,
    target_archive_checksum_file_path: &Path,
    archive_manifest: &ArchiveManifest,
) {
    // Ensure that the checksum of the unpacked file corresponds to the one we
    // expect, otherwise remove the `target_path` and re-fetch / unpack the
    // archive:
    let cached_csum = std::fs::read_to_string(target_archive_checksum_file_path)
        .ok()
        .and_then(|csum_str| hex::decode(csum_str).ok());
    match cached_csum {
        None => {
            // No cached checksum written. We may have a partially extracted
            // archive, attempt to delete:
            let _ = std::fs::remove_dir_all(target_path);
        }
        Some(csum) if csum != archive_manifest.sha256 => {
            // Checksum file does not exist or contains incorrect checksum,
            // re-fetch:
            println!(
                "cargo:warning=Cached {} was unpacked from \
		 archive with different checksum, re-fetching (expected {} \
		 vs. cached {})",
                archive_name,
                hex::encode(&archive_manifest.sha256),
                hex::encode(csum)
            );
            std::fs::remove_file(target_archive_checksum_file_path).unwrap();
            let _ = std::fs::remove_dir_all(target_path);
        }
        Some(_) => {
            // Cached & unpacked archive either does not exist or is current, do
            // nothing.
        }
    }

    // Re-fetch if `target_path` did not exist (or we've just deleted it):
    if target_path.exists() {
        println!(
            "cargo:warning=Using cached {archive_name} archive at {:?}",
            target_path
        );
    } else {
        let prebuilt_archive_bytes = match env::var(archive_path_env_var) {
            Ok(path) => {
                println!(
                    "cargo:warning=Using prebuilt {} archive from {}, extracting to {:?}",
                    archive_name, path, target_path,
                );
                std::fs::read(path).unwrap_or_else(|_| {
                    panic!(
                        "Failed to read prebuilt {} archive from {}",
                        archive_name, archive_path_env_var
                    )
                })
            }
            Err(_) => {
                println!(
                    "cargo:warning=Using prebuilt {} archive from {}, extracting to {:?}",
                    archive_name, archive_manifest.url, target_path,
                );
                reqwest::blocking::get(&archive_manifest.url)
                    .unwrap_or_else(|_| {
                        panic!("Failed to download prebuilt {archive_name} archive")
                    })
                    .bytes()
                    .unwrap_or_else(|_| {
                        panic!("Failed to read response bytes for {archive_name} archive")
                    })
                    .to_vec()
            }
        };

        let mut hasher = Sha256::new();
        hasher.update(&prebuilt_archive_bytes);
        let prebuilt_actual_sha256 = hasher.finalize();
        if *prebuilt_actual_sha256 != *archive_manifest.sha256 {
            panic!(
                "SHA256 mismatch for prebuilt archive. Expected {}, got {}",
                hex::encode(&archive_manifest.sha256),
                hex::encode(prebuilt_actual_sha256),
            );
        }

        // Extract archive
        let prebuilt_archive_tar =
            flate2::read::GzDecoder::new(Cursor::new(prebuilt_archive_bytes));
        let mut prebuilt_archive = tar::Archive::new(prebuilt_archive_tar);
        prebuilt_archive
            .unpack(target_path)
            .expect("Failed to unpack archive");

        // Write the checksum of the archive to `target_archive_checksum_file`,
        // which allows us to avoid fetching the archive repeatedly if the
        // checksum doesn't change:
        std::fs::write(
            target_archive_checksum_file_path,
            hex::encode(prebuilt_actual_sha256).as_bytes(),
        )
        .unwrap();
    }
}

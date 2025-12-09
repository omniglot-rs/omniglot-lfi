// -*- fill-column: 80; -*-

use std::env;
use std::path::PathBuf;

fn main() {
    // Link against the `liblfi` library:
    println!("cargo:rustc-link-lib=lfi");

    let liblfi_pkg = pkg_config::probe_library("lfi")
	.expect("Cannot determine pkg-config info for `lfi`, is both `pkg-config` and the LFI runtime installed?");

    // Generate `liblfi` bindings:
    let bindings = bindgen::Builder::default()
        .clang_args(
            liblfi_pkg
                .include_paths
                .iter()
                .map(|path| format!("-I{}", path.to_string_lossy())),
        )
        .header("c_src/lfi_wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings for `liblfi`!");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("lfi_bindings.rs"))
        .expect("Couldn't write bindings for `liblfi`!");

    // Compile and link the C helper functions:
    cc::Build::new()
        .includes(liblfi_pkg.include_paths.iter())
        .opt_level(2)
        .file("c_src/lfi_threadlocal.c")
        .compile("lfi_threadlocal");
}

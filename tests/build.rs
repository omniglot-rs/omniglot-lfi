use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=./liboglfitests.omniglot.toml");
    println!("cargo:rerun-if-changed=./liboglfitests_lfi");

    let bindings = bindgen::Builder::default()
        .header("liboglfitests_lfi/og_lfi_tests.h")
        .omniglot_configuration_file(Some(
            PathBuf::from("./liboglfitests.omniglot.toml")
                .canonicalize()
                .unwrap(),
        ))
        .rustfmt_configuration_file(Some(
            PathBuf::from("./og_bindings_rustfmt.toml")
                .canonicalize()
                .unwrap(),
        ))
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("liboglfitests_bindings.rs"))
        .expect("Couldn't write bindings!");
}

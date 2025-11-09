use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=./libadd.omniglot.toml");
    println!("cargo:rerun-if-changed=./libadd_lfi");

    let bindings = bindgen::Builder::default()
        .header("libadd_lfi/add.h")
        .omniglot_configuration_file(Some(
            PathBuf::from("./libadd.omniglot.toml")
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
        .write_to_file(out_path.join("libogadd_bindings.rs"))
        .expect("Couldn't write bindings!");
}

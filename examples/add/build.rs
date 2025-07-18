use std::env;
use std::ffi::OsStr;
use std::path::PathBuf;

fn check_output_res(res: std::io::Result<std::process::Output>, msg: &'static str) {
    match res {
        Err(e) => Err(e).expect(msg),
        Ok(out) => {
            if !out.status.success() {
                panic!(
                    "{},\nstdout: \n{},\nstderr: \n{}",
                    msg,
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr)
                );
            }
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=./ogadd.omniglot.toml");
    println!("cargo:rerun-if-changed=./c_src/ogadd.h");

    let bindings = bindgen::Builder::default()
        .header("c_src/ogadd.h")
        .omniglot_configuration_file(Some(
            PathBuf::from("./ogadd.omniglot.toml")
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

    // // Build the libogadd as a shared library.
    // //
    // // We cannot use the cc crate, as it does not support building
    // // dynamic libraries. Thus, determine the compiler based on the
    // // CC environment variable:
    // let cc = env::var("CC").expect("No C compiler (CC environment variable) provided!");
    // check_output_res(
    //     std::process::Command::new(&cc)
    //         .args([
    //             OsStr::new("-g"),        // Produce debug symbols in the target's native format
    //             OsStr::new("-ggdb"),     // Provide debug symbols readable by GDB
    //             OsStr::new("-fPIC"),     // Produce PIC code to support loading as shared lib
    //             OsStr::new("-rdynamic"), // Add all symbols (not just used) to the ELF
    //             OsStr::new("-shared"),   // Produce a shared object
    //             OsStr::new("c_src/ogadd.c"),
    //             OsStr::new("-o"),
    //             out_path.join("libogadd.so").as_os_str(),
    //         ])
    //         .output(),
    //     "Failed to compile the libogadd library!",
    // );

    // // For the mock runtime, we also want to link against the library directly.
    // // This can be commented out, but there must be no code path to instantiate
    // // the MockRt, or otherwise there will be linker errors:
    // println!("cargo:rustc-link-search={}", out_path.display());
    // println!("cargo:rustc-link-lib=ogadd");
    // println!("cargo:rustc-link-arg=-Wl,-rpath,{}", out_path.display());
}

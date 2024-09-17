// build.rs

extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to link to the Potrace library
    println!("cargo:rustc-link-lib=potrace");

    // Add the library search path
    println!("cargo:rustc-link-search=native=/opt/homebrew/opt/potrace/lib");

    // Specify the include path where 'potracelib.h' is located if necessary
    // For example: let include_path = "/usr/local/include";
    // If needed, add the include path to the compiler arguments
    // .clang_arg(format!("-I{}", include_path))

    // Create the bindgen builder
    let bindings = bindgen::Builder::default()
        // Specify the header file to generate bindings for
        .header("wrapper.h")
        .clang_arg("-I/opt/homebrew/opt/potrace/include/")
        // Include all necessary functions and types
        .allowlist_function("potrace_.*")
        .allowlist_type("potrace_.*")
        .allowlist_var("POTRACE_.*")
        // Generate the bindings
        .generate()
        // Handle errors
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/potrace_bindings.rs file
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("potrace_bindings.rs"))
        .expect("Couldn't write bindings!");
}
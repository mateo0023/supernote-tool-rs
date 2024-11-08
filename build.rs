extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    // Link statically to libpotrace
    println!("cargo:rustc-link-lib=static=potrace");

    // Specify the path to where the library is located
    println!("cargo:rustc-link-search=./potrace/");

    // Specify the include path for header files
    let include_path = "./potrace/include";
    println!("cargo:include={}", include_path);

    // Use bindgen to generate Rust bindings for the header file
    let bindings = bindgen::Builder::default()
        .header("wrapper.h")
        .clang_arg(format!("-I{}", include_path))
        .allowlist_function("potrace_.*")
        .allowlist_type("potrace_.*")
        .allowlist_var("POTRACE_.*")
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/potrace_bindings.rs
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("potrace_bindings.rs"))
        .expect("Couldn't write bindings!");
}

extern crate bindgen;

use std::env;
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use winresource::WindowsResource;

fn main() {
    // Link statically to libpotrace
    println!("cargo:rustc-link-lib=static=potrace");

    // Specify the path to where the library is located
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-search=./potrace/windows");
    #[cfg(target_os = "macos")]
    println!("cargo:rustc-link-search=./potrace/macos");

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

    #[cfg(target_os = "windows")]
    if env::var_os("CARGO_CFG_WINDOWS").is_some() {
        WindowsResource::new()
            // This path can be absolute, or relative to your crate root.
            .set_icon("icons/icon.ico")
            .compile().unwrap();
    }
}

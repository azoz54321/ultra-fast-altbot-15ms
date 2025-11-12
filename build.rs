use std::path::PathBuf;
use std::env;

fn main() {
    // Get the directory of c_sbe_decoder
    let c_dir = PathBuf::from("c_sbe_decoder");
    
    // Compile the C decoder
    cc::Build::new()
        .file(c_dir.join("decoder.c"))
        .include(&c_dir)
        .warnings(true)
        .compile("sbe_decoder");
    
    // Tell cargo where to find the library
    let out_dir = env::var("OUT_DIR").unwrap();
    println!("cargo:rustc-link-search=native={}", out_dir);
    println!("cargo:rustc-link-lib=static=sbe_decoder");
    
    // Tell cargo to rerun if C files change
    println!("cargo:rerun-if-changed=c_sbe_decoder/decoder.c");
    println!("cargo:rerun-if-changed=c_sbe_decoder/decoder.h");
}

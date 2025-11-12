use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let c_sbe_decoder_dir = PathBuf::from(&manifest_dir).join("c_sbe_decoder");

    // Compile the C decoder
    cc::Build::new()
        .file(c_sbe_decoder_dir.join("decoder.c"))
        .include(&c_sbe_decoder_dir)
        .opt_level(3)
        .compile("sbe_decoder");

    // Tell cargo to rerun this build script if decoder.c or decoder.h changes
    println!("cargo:rerun-if-changed=c_sbe_decoder/decoder.c");
    println!("cargo:rerun-if-changed=c_sbe_decoder/decoder.h");
}

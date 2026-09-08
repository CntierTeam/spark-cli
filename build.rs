use std::env;
use std::io::Result;
use std::path::PathBuf;

fn main() -> Result<()> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    env::set_current_dir(&manifest)?;
    let mut config = prost_build::Config::new();
    config
        .protoc_executable(protoc_bin_vendored::protoc_bin_path().expect("vendored protoc"))
        .compile_protos(&["proto/spark.proto"], &["proto"])?;
    println!("cargo:rerun-if-changed=proto/spark.proto");
    Ok(())
}

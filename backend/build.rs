// Compiles crw/proto/search.proto into Rust client stubs for the crw gRPC
// search service (step 82). Single source of truth: crw/proto/search.proto.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let proto_dir = manifest_dir.join("../crw/proto");
    let proto_file = proto_dir.join("search.proto");

    println!("cargo:rerun-if-changed={}", proto_file.display());

    tonic_prost_build::configure()
        .build_server(false)
        .build_client(true)
        .compile_protos(&[proto_file], &[proto_dir])?;

    Ok(())
}

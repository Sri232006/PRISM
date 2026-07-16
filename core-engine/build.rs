fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile our proto file located at ../proto/prism.proto
    tonic_build::compile_protos("../proto/prism.proto")?;
    Ok(())
}

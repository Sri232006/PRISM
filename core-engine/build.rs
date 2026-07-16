fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile our proto file located at ../proto/prism.proto, adding serde derives
    tonic_build::configure()
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile_protos(&["../proto/prism.proto"], &["../proto"])?;
    Ok(())
}

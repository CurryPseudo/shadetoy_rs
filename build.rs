use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::fs;

fn convert_shader(
    source_path: &str,
    stage: naga::ShaderStage,
) -> Result<String, Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed={}", source_path);
    let source = fs::read_to_string(source_path)?;
    let mut parser = naga::front::glsl::Frontend::default();
    let module = parser.parse(
        &naga::front::glsl::Options {
            stage,
            defines: Default::default(),
        },
        &source,
    )?;

    // Validate the module
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let _info = validator.validate(&module)?;

    // Convert to WGSL
    let wgsl = naga::back::wgsl::write_string(
        &module,
        &_info,
        naga::back::wgsl::WriterFlags::empty(),
    )?;

    Ok(wgsl)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Convert vertex shader
    let vert_wgsl = convert_shader("src/shader.vert", naga::ShaderStage::Vertex)?;
    fs::write("src/shader.vert.wgsl", vert_wgsl)?;

    // Convert fragment shader
    let frag_wgsl = convert_shader("src/shader.frag", naga::ShaderStage::Fragment)?;
    fs::write("src/shader.frag.wgsl", frag_wgsl)?;

    Ok(())
}

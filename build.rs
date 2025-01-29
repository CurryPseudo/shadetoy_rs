use shaderc::{Compiler, ShaderKind};
use std::fs;

// only run in non-wasm build
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Tell cargo to rerun if shaders change
    println!("cargo:rerun-if-changed=src/shader.vert");
    println!("cargo:rerun-if-changed=src/shader.frag");

    let compiler = Compiler::new().ok_or("Failed to create shader compiler")?;

    // Compile vertex shader
    let vert_source = fs::read_to_string("src/shader.vert")?;
    let vert_spirv = compiler.compile_into_spirv(
        &vert_source,
        ShaderKind::Vertex,
        "shader.vert",
        "main",
        None,
    )?;
    fs::write("src/shader.vert.spv", vert_spirv.as_binary_u8())?;

    // Compile fragment shader
    let frag_source = fs::read_to_string("src/shader.frag")?;
    let frag_spirv = compiler.compile_into_spirv(
        &frag_source,
        ShaderKind::Fragment,
        "shader.frag",
        "main",
        None,
    )?;
    fs::write("src/shader.frag.spv", frag_spirv.as_binary_u8())?;

    Ok(())
}

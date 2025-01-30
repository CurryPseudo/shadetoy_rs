use anyhow::anyhow;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use std::borrow::Cow;
use std::path::Path;

pub fn convert_shader(source: &str, stage: naga::ShaderStage) -> crate::app::Result<String> {
    let mut parser = naga::front::glsl::Frontend::default();
    let module = parser
        .parse(
            &naga::front::glsl::Options {
                stage,
                defines: Default::default(),
            },
            source,
        )
        .map_err(|err| anyhow!("{}", err.emit_to_string(source)))?;

    // Validate the module
    let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
    let _info = validator
        .validate(&module)
        .map_err(|err| anyhow!("{}", err.emit_to_string(source)))?;

    // Convert to WGSL
    let wgsl =
        naga::back::wgsl::write_string(&module, &_info, naga::back::wgsl::WriterFlags::empty())?;

    Ok(wgsl)
}
macro_rules! load_shader {
    ($path:literal, $stage:expr) => {
        Ok(Cow::<'static, str>::from(if cfg!(target_arch = "wasm32") {
            convert_shader(include_str!($path), $stage)?
        } else {
            let path = format!("src/app/{}", $path);
            convert_shader(&std::fs::read_to_string(&Path::new(&path))?, $stage)?
        }))
    };
}

pub fn load_vertex_shader() -> crate::app::Result<Cow<'static, str>> {
    load_shader!("shader.vert", naga::ShaderStage::Vertex)
}
pub fn load_fragment_shader() -> crate::app::Result<Cow<'static, str>> {
    load_shader!("shader.frag", naga::ShaderStage::Fragment)
}

#[test]
fn shader_error() {
    let source = include_str!("test/error.frag");
    let result = convert_shader(source, naga::ShaderStage::Fragment);
    if let Err(err) = result {
        assert_eq!(err.to_string(), include_str!("test/error.frag.error"));
    } else {
        panic!("Expected an error, but got: {:?}", result);
    }
}

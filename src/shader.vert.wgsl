struct Uniforms {
    angle: f32,
}

struct VertexOutput {
    @location(0) v_color: vec4<f32>,
    @builtin(position) gl_Position: vec4<f32>,
}

const positions: array<vec2<f32>, 3> = array<vec2<f32>, 3>(vec2<f32>(0f, 1f), vec2<f32>(1f, -1f), vec2<f32>(-1f, -1f));
const colors: array<vec4<f32>, 3> = array<vec4<f32>, 3>(vec4<f32>(1f, 0f, 0f, 1f), vec4<f32>(0f, 1f, 0f, 1f), vec4<f32>(0f, 0f, 1f, 1f));

var<private> v_color: vec4<f32>;
@group(0) @binding(0) 
var<uniform> global: Uniforms;
var<private> gl_VertexIndex_1: u32;
var<private> gl_Position: vec4<f32>;

fn main_1() {
    var local: array<vec2<f32>, 3> = positions;
    var pos: vec2<f32>;
    var local_1: array<vec4<f32>, 3> = colors;

    let _e6 = gl_VertexIndex_1;
    let _e10 = local[_e6];
    pos = _e10;
    let _e13 = pos;
    let _e15 = global.angle;
    pos.x = (_e13.x * cos(_e15));
    let _e19 = pos;
    gl_Position = vec4<f32>(_e19.x, _e19.y, 0f, 1f);
    let _e25 = gl_VertexIndex_1;
    let _e29 = local_1[_e25];
    v_color = _e29;
    return;
}

@vertex 
fn main(@builtin(vertex_index) gl_VertexIndex: u32) -> VertexOutput {
    gl_VertexIndex_1 = gl_VertexIndex;
    main_1();
    let _e11 = v_color;
    let _e13 = gl_Position;
    return VertexOutput(_e11, _e13);
}

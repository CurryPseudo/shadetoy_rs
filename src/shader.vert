#version 450

layout(location = 0) out vec4 v_color;

layout(binding = 0) uniform Uniforms {
    float angle;
};

const vec2 positions[3] = vec2[3](
    vec2(0.0, 1.0),
    vec2(1.0, -1.0),
    vec2(-1.0, -1.0)
);

const vec4 colors[3] = vec4[3](
    vec4(1.0, 0.0, 0.0, 1.0),
    vec4(0.0, 1.0, 0.0, 1.0),
    vec4(0.0, 0.0, 1.0, 1.0)
);

void main() {
    vec2 pos = positions[gl_VertexIndex];
    pos.x = pos.x * cos(angle);
    gl_Position = vec4(pos, 0.0, 1.0);
    v_color = colors[gl_VertexIndex];
}


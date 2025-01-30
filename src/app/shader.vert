#version 450

const vec2 positions[6] = vec2[6](
vec2(-1.0, 1.0),
vec2(1.0, -1.0),
vec2(-1.0, -1.0),
vec2(-1.0, 1.0),
vec2(1.0, 1.0),
vec2(1.0, -1.0)
);

void main() {
    vec2 pos = positions[gl_VertexIndex];
    gl_Position = vec4(pos, 0.0, 1.0);
}


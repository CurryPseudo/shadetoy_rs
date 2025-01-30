#version 450

layout (binding = 0) uniform Uniforms {
    vec2 iResolution;
};

layout (location = 0) out vec4 _f_color;

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord.xy / iResolution.xy;
    fragColor = vec4(uv.x, uv.y, 0.0, 1.0);
}

void main() {
    mainImage(_f_color, gl_FragCoord.xy);
}


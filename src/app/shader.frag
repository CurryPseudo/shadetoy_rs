#version 450

layout (binding = 0, std140) uniform Uniforms {{
    vec2 iResolution;
}};

layout (location = 0) out vec4 _f_color;

{content}

void main() {{
    mainImage(_f_color, gl_FragCoord.xy);
}}


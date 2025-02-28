#version 450

layout (binding = 0, std140) uniform Uniforms {{
    vec2 iResolution;
    float iTime;
    float iTimeDelta;
    float iFrame;
    vec4 iChannelTime;
    vec4 iMouse;
    vec4 iDate;
    float iSampleRate;
    //vec3 iChannelResolution[4];
}};

layout (location = 0) out vec4 _f_color;

{content}

void main() {{
    vec2 fragCoord = gl_FragCoord.xy;
    fragCoord.y = iResolution.y - fragCoord.y;
    mainImage(_f_color, fragCoord);
}}


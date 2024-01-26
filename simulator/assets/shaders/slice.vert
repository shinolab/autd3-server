#version 450

layout(location = 0) in vec4 position;
layout(location = 1) in vec2 tex_coords;

layout(location = 0) out vec2 o_tex_coords;

layout(push_constant) uniform PushConstsConfig {
    mat4 pvm;
} pc;

void main() {
    gl_Position = pc.pvm * position;
    o_tex_coords = tex_coords;
}
 
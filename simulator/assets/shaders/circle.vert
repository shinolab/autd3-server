#version 450 core

layout(location = 0) in vec4 position;
layout(location = 1) in vec2 tex_coords;
layout(location = 2) in mat4 model;
layout(location = 6) in vec4 color;

layout(location = 0) out vec2 o_tex_coords;
layout(location = 1) out vec4 o_color;

layout(push_constant) uniform PushConsts {
    mat4 proj_view;
} primitive;

void main() {
    o_tex_coords = tex_coords;
    o_color = color;
    gl_Position = primitive.proj_view * model * position;
}

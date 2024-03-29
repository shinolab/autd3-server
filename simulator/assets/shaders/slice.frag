#version 450

layout(location = 0) in vec2 v_tex_coords;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 0) buffer Data {
    vec4 data[];
} data;

layout(push_constant) uniform PushConstsConfig {
    mat4 pvm;
    uint width;
    uint height;
} config;

void main() {
  uint w = uint(floor(v_tex_coords.x * config.width));
  uint h = uint(floor(v_tex_coords.y * config.height));
  uint idx = w + config.width * h;
  f_color = data.data[idx];
}

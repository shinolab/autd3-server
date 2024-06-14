#version 450

layout(set = 0, binding = 0) uniform sampler2D samplerColorMap;

layout(location = 0) in vec2 inUV;

layout(location = 0) out vec4 outFragColor;

layout(push_constant) uniform PushConsts {
  mat4 pvm;
  float baseColorR;
  float baseColorG;
  float baseColorB;
  bool hasTexture;
} pcf;

void main() {
  vec4 color = pcf.hasTexture
                   ? texture(samplerColorMap, inUV)
                   : vec4(pcf.baseColorR, pcf.baseColorG, pcf.baseColorB, 1.0f);
  outFragColor = vec4(color.rgb * 20.0f, color.a);
}

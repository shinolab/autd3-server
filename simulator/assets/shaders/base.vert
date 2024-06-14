#version 450

layout (location = 0) in vec3 position;
layout (location = 1) in vec3 norm;
layout (location = 2) in vec2 uv;

layout(push_constant) uniform PushConsts {
	mat4 pvm;
} primitive;

layout (location = 0) out vec2 outUV;

void main()
{
	outUV = uv;
	gl_Position = primitive.pvm * vec4(position, 1.0);	
}

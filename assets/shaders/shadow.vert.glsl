#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;

layout(push_constant) uniform ShadowPush {
    mat4 model;
    mat4 light_space;
} push;

void main() {
    gl_Position = push.light_space * push.model * vec4(in_position, 1.0);
}

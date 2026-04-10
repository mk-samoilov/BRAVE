#version 450

layout(location = 0) in vec3 in_pos;

layout(push_constant) uniform ShadowPC {
    mat4 model;
    mat4 light_vp;
};

void main() {
    gl_Position = light_vp * model * vec4(in_pos, 1.0);
}

#version 450

layout(location = 0) in vec3 in_position;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;

layout(set = 0, binding = 0) uniform FrameUbo {
    mat4 view;
    mat4 proj;
    vec4 dir_light_dir;
    vec4 dir_light_color;
    vec4 ambient;
    vec4 point_pos_range[8];
    vec4 point_color_intensity[8];
    int  point_count;
    int  _pad0; int _pad1; int _pad2;
    vec4 spot_pos_range[4];
    vec4 spot_color_intensity[4];
    vec4 spot_dir_angle[4];
    int  spot_count;
    int  _pad3; int _pad4; int _pad5;
    mat4 light_space_matrix;
    int  shadows_enabled;
    int  _pad6; int _pad7; int _pad8;
    vec4 cam_pos;
} frame;

layout(push_constant) uniform PushConst {
    mat4 model;
    vec4 base_color;
} push;

layout(location = 0) out vec3 frag_normal;
layout(location = 1) out vec2 frag_uv;
layout(location = 2) out vec3 frag_world_pos;
layout(location = 3) out vec4 frag_light_space_pos;

void main() {
    vec4 world_pos = push.model * vec4(in_position, 1.0);
    gl_Position = frame.proj * frame.view * world_pos;

    frag_normal          = mat3(transpose(inverse(push.model))) * in_normal;
    frag_uv              = in_uv;
    frag_world_pos       = world_pos.xyz;
    frag_light_space_pos = frame.light_space_matrix * world_pos;
}

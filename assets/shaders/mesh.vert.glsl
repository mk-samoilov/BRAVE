#version 450

layout(location = 0) in vec3 in_pos;
layout(location = 1) in vec3 in_normal;
layout(location = 2) in vec2 in_uv;

layout(set = 0, binding = 0) uniform SceneUBO {
    mat4 view_proj;
    vec4 camera_pos;
    vec4 camera_dir;
};

layout(push_constant) uniform Push {
    mat4 model;
    vec4 albedo;
    vec4 mr;
};

layout(location = 0) out vec3 frag_world_pos;
layout(location = 1) out vec3 frag_normal;
layout(location = 2) out vec2 frag_uv;

void main() {
    vec4 world_pos  = model * vec4(in_pos, 1.0);
    frag_world_pos  = world_pos.xyz;
    mat3 normal_mat = transpose(inverse(mat3(model)));
    frag_normal     = normalize(normal_mat * in_normal);
    frag_uv         = in_uv;
    gl_Position     = view_proj * world_pos;
}

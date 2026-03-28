#version 450

layout(push_constant) uniform Push {
    mat4 inv_proj_view;
} push;

layout(location = 0) out vec3 frag_dir;

void main() {
    vec2 clip;
    if      (gl_VertexIndex == 0) clip = vec2(-1.0, -1.0);
    else if (gl_VertexIndex == 1) clip = vec2( 3.0, -1.0);
    else                          clip = vec2(-1.0,  3.0);

    gl_Position = vec4(clip, 1.0, 1.0);

    vec4 world_h = push.inv_proj_view * vec4(clip, 1.0, 1.0);
    frag_dir = world_h.xyz / world_h.w;
}

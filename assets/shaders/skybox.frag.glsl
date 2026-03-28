#version 450

layout(location = 0) in vec3 frag_dir;

layout(set = 0, binding = 0) uniform sampler2D env_map;

layout(push_constant) uniform Push {
    layout(offset = 64) float lod_bias;
} push;

layout(location = 0) out vec4 out_color;

#define PI 3.141592653589793

vec2 equirect_uv(vec3 d) {
    float phi   = atan(d.z, d.x);
    float theta = asin(clamp(-d.y, -1.0, 1.0));
    return vec2(0.5 + phi / (2.0 * PI), 0.5 - theta / PI);
}

void main() {
    vec3 dir   = normalize(frag_dir);
    vec3 color = texture(env_map, equirect_uv(dir), push.lod_bias).rgb;
    color = clamp((color * (2.51 * color + 0.03)) / (color * (2.43 * color + 0.59) + 0.14), 0.0, 1.0);
    color = pow(color, vec3(1.0 / 2.2));
    out_color = vec4(color, 1.0);
}

#version 450

layout(location = 0) in vec3 frag_normal;
layout(location = 1) in vec2 frag_uv;
layout(location = 2) in vec3 frag_world_pos;
layout(location = 3) in vec4 frag_light_space_pos;

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
} frame;

layout(set = 0, binding = 1) uniform sampler2D shadow_map;

layout(location = 0) out vec4 out_color;

// ─── Shadow ──────────────────────────────────────────────────────────────────

float calc_shadow(vec4 light_space_pos) {
    if (frame.shadows_enabled == 0) return 0.0;

    vec3 proj = light_space_pos.xyz / light_space_pos.w;
    proj = proj * 0.5 + 0.5;

    if (proj.z > 1.0) return 0.0;

    float bias = 0.005;
    float shadow = 0.0;

    // PCF 3x3
    vec2 texel = 1.0 / vec2(textureSize(shadow_map, 0));
    for (int x = -1; x <= 1; x++) {
        for (int y = -1; y <= 1; y++) {
            float pcf_depth = texture(shadow_map, proj.xy + vec2(x, y) * texel).r;
            shadow += (proj.z - bias > pcf_depth) ? 1.0 : 0.0;
        }
    }
    return shadow / 9.0;
}

// ─── Attenuation ─────────────────────────────────────────────────────────────

float attenuate(float dist, float range) {
    if (dist >= range) return 0.0;
    float x = dist / range;
    return clamp(1.0 - x * x * x * x, 0.0, 1.0) / (dist * dist + 1.0);
}

// ─── Main ─────────────────────────────────────────────────────────────────────

void main() {
    vec3 normal    = normalize(frag_normal);
    vec3 base_color = vec3(0.8, 0.8, 0.8);
    vec3 result    = vec3(0.0);

    // Ambient
    result += frame.ambient.xyz * frame.ambient.w * base_color;

    // Directional light
    {
        vec3  dir       = normalize(frame.dir_light_dir.xyz);
        float intensity = frame.dir_light_dir.w;
        float diff      = max(dot(normal, dir), 0.0);
        float shadow    = calc_shadow(frag_light_space_pos);
        result += (1.0 - shadow) * diff * intensity * frame.dir_light_color.xyz * base_color;
    }

    // Point lights
    for (int i = 0; i < frame.point_count; i++) {
        vec3  pos       = frame.point_pos_range[i].xyz;
        float range     = frame.point_pos_range[i].w;
        vec3  color     = frame.point_color_intensity[i].xyz;
        float intensity = frame.point_color_intensity[i].w;

        vec3  to_light = pos - frag_world_pos;
        float dist     = length(to_light);
        float atten    = attenuate(dist, range);
        float diff     = max(dot(normal, normalize(to_light)), 0.0);
        result += diff * intensity * atten * color * base_color;
    }

    // Spot lights
    for (int i = 0; i < frame.spot_count; i++) {
        vec3  pos       = frame.spot_pos_range[i].xyz;
        float range     = frame.spot_pos_range[i].w;
        vec3  color     = frame.spot_color_intensity[i].xyz;
        float intensity = frame.spot_color_intensity[i].w;
        vec3  dir       = normalize(frame.spot_dir_angle[i].xyz);
        float cos_angle = frame.spot_dir_angle[i].w;

        vec3  to_light  = pos - frag_world_pos;
        float dist      = length(to_light);
        vec3  to_light_n = normalize(to_light);

        float theta = dot(to_light_n, -dir);
        if (theta < cos_angle) continue;

        float atten = attenuate(dist, range);
        float diff  = max(dot(normal, to_light_n), 0.0);
        result += diff * intensity * atten * color * base_color;
    }

    out_color = vec4(result, 1.0);
}

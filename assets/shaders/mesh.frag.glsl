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

    vec4 cam_pos;
    int  rt_aabb_count;
    int  _pad9; int _pad10; int _pad11;
} frame;

layout(set = 0, binding = 1) uniform sampler2D shadow_map;

struct AabbEntry {
    vec4 min_pt;
    vec4 max_pt;
};
layout(std430, set = 0, binding = 2) readonly buffer SceneAabbs {
    AabbEntry aabbs[];
} scene;

layout(location = 0) out vec4 out_color;

// https://iquilezles.org/articles/boxfunctions/

const float SHADOW_BIAS     = 0.03;
const float SHADOW_SOFTNESS = 8.0;

float length2(vec3 v) { return dot(v, v); }

float segShadow(vec3 ro, vec3 rd, vec3 pa, float sh) {
    float k1 = 1.0 - rd.x * rd.x;
    float k4 = (ro.x - pa.x) * k1;
    float k6 = (ro.x + pa.x) * k1;
    vec2  k5 = ro.yz * k1;
    vec2  k7 = pa.yz * k1;
    float k2 = -dot(ro.yz, rd.yz);
    vec2  k3 = pa.yz * rd.yz;
    for (int i = 0; i < 4; i++) {
        vec2  ss  = vec2(float(i & 1), float(i >> 1)) * 2.0 - 1.0;
        float thx = k2 + dot(ss, k3);
        if (thx < 0.0) continue;
        float thy = clamp(-rd.x * thx, k4, k6);
        sh = min(sh, length2(vec3(thy, k5 - k7 * ss) + rd * thx) / (thx * thx));
    }
    return sh;
}

float boxSoftShadow(vec3 row, vec3 rdw, vec3 bmin, vec3 bmax, float sk) {
    vec3 center = (bmin + bmax) * 0.5;
    vec3 rad    = (bmax - bmin) * 0.5;
    vec3 ro = row - center;
    vec3 rd = rdw;

    vec3  m  = 1.0 / rd;
    vec3  n  = m * ro;
    vec3  k  = abs(m) * rad;
    vec3  t1 = -n - k;
    vec3  t2 = -n + k;
    float tN = max(max(t1.x, t1.y), t1.z);
    float tF = min(min(t2.x, t2.y), t2.z);

    if (tN > tF || tF < 0.0) {
        float sh = 1.0;
        sh = segShadow(ro.xyz, rd.xyz, rad.xyz, sh);
        sh = segShadow(ro.yzx, rd.yzx, rad.yzx, sh);
        sh = segShadow(ro.zxy, rd.zxy, rad.zxy, sh);
        sh = clamp(sk * sqrt(sh), 0.0, 1.0);
        return sh * sh * (3.0 - 2.0 * sh);
    }
    return 0.0;
}

float calc_rt_shadow(vec3 world_pos, vec3 normal) {
    if (frame.rt_aabb_count == 0) return 0.0;

    vec3 sun_dir    = normalize(frame.dir_light_dir.xyz);
    vec3 ray_origin = world_pos + normal * SHADOW_BIAS;

    float light = 1.0;
    for (int i = 0; i < frame.rt_aabb_count; i++) {
        float s = boxSoftShadow(ray_origin, sun_dir,
                                scene.aabbs[i].min_pt.xyz,
                                scene.aabbs[i].max_pt.xyz,
                                SHADOW_SOFTNESS);
        light = min(light, s);
    }
    return 1.0 - light;
}

// ─── Shadow-map PCF fallback ──────────────────────────────────────────────────

float rand(vec2 co) {
    return fract(sin(dot(co, vec2(12.9898, 78.233))) * 43758.5453);
}

const vec2 POISSON[16] = vec2[](
    vec2(-0.94201624, -0.39906216), vec2( 0.94558609, -0.76890725),
    vec2(-0.09418410, -0.92938870), vec2( 0.34495938,  0.29387760),
    vec2(-0.91588581,  0.45771432), vec2(-0.81544232, -0.87912464),
    vec2(-0.38277543,  0.27676845), vec2( 0.97484398,  0.75648379),
    vec2( 0.44323325, -0.97511554), vec2( 0.53742981, -0.47373420),
    vec2(-0.26496911, -0.41893023), vec2( 0.79197514,  0.19090188),
    vec2(-0.24188840,  0.99706507), vec2(-0.81409955,  0.91437590),
    vec2( 0.19984126,  0.78641367), vec2( 0.14383161, -0.14100790)
);

float calc_shadow_map(vec4 light_space_pos, vec3 normal, vec3 light_dir) {
    if (frame.shadows_enabled == 0) return 0.0;
    vec3 proj = light_space_pos.xyz / light_space_pos.w;
    proj.xy = proj.xy * 0.5 + 0.5;
    if (proj.z > 1.0 || clamp(proj.xy, 0.0, 1.0) != proj.xy) return 0.0;

    float cos_theta = clamp(dot(normal, light_dir), 0.0, 1.0);
    float bias = clamp(0.0015 * tan(acos(cos_theta)), 0.0005, 0.006);

    float spread = 2800.0 / float(textureSize(shadow_map, 0).x);
    float angle  = rand(proj.xy) * 6.2832;
    float ca = cos(angle), sa = sin(angle);

    float shadow = 0.0;
    for (int i = 0; i < 16; i++) {
        vec2 offset = vec2(ca * POISSON[i].x - sa * POISSON[i].y,
                           sa * POISSON[i].x + ca * POISSON[i].y)
                    * spread / vec2(textureSize(shadow_map, 0));
        float d = texture(shadow_map, proj.xy + offset).r;
        shadow += (proj.z - bias > d) ? 1.0 : 0.0;
    }
    return shadow / 16.0;
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
    vec3 view_dir  = normalize(frame.cam_pos.xyz - frag_world_pos);
    vec3 base_color = vec3(0.8, 0.8, 0.8);
    vec3 result    = vec3(0.0);

    // Hemisphere ambient
    vec3 sky_col    = vec3(0.18, 0.26, 0.42);
    vec3 ground_col = vec3(0.16, 0.14, 0.10);
    result += mix(ground_col, sky_col, normal.y * 0.5 + 0.5) * base_color;

    // Configurable ambient
    result += frame.ambient.xyz * frame.ambient.w * base_color;

    // Directional light
    {
        vec3  dir       = normalize(frame.dir_light_dir.xyz);
        float intensity = frame.dir_light_dir.w;
        float diff      = max(dot(normal, dir), 0.0);
        vec3  half_vec  = normalize(dir + view_dir);
        float spec      = pow(max(dot(normal, half_vec), 0.0), 32.0) * 0.3;

        float shadow = (frame.rt_aabb_count > 0)
            ? calc_rt_shadow(frag_world_pos, normal)
            : calc_shadow_map(frag_light_space_pos, normal, dir);

        result += (1.0 - shadow) * intensity * frame.dir_light_color.xyz
                * (diff * base_color + spec);
    }

    // Point lights
    for (int i = 0; i < frame.point_count; i++) {
        vec3  pos       = frame.point_pos_range[i].xyz;
        float range     = frame.point_pos_range[i].w;
        vec3  color     = frame.point_color_intensity[i].xyz;
        float intensity = frame.point_color_intensity[i].w;
        vec3  to_light  = pos - frag_world_pos;
        float dist      = length(to_light);
        vec3  l         = normalize(to_light);
        float atten     = attenuate(dist, range);
        float diff      = max(dot(normal, l), 0.0);
        vec3  half_vec  = normalize(l + view_dir);
        float spec      = pow(max(dot(normal, half_vec), 0.0), 32.0) * 0.3;
        result += intensity * atten * color * (diff * base_color + spec);
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
        vec3  l         = normalize(to_light);
        float theta     = dot(l, -dir);
        if (theta < cos_angle) continue;
        float atten     = attenuate(dist, range);
        float diff      = max(dot(normal, l), 0.0);
        vec3  half_vec  = normalize(l + view_dir);
        float spec      = pow(max(dot(normal, half_vec), 0.0), 32.0) * 0.3;
        result += intensity * atten * color * (diff * base_color + spec);
    }

    // Reinhard tone mapping
    result = result / (result + vec3(1.0));
    // Gamma correction
    result = pow(result, vec3(1.0 / 2.2));

    out_color = vec4(result, 1.0);
}

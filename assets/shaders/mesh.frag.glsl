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
} frame;

layout(set = 0, binding = 1) uniform sampler2D shadow_map;

layout(set = 1, binding = 0) uniform sampler2D albedo_tex;
layout(set = 2, binding = 0) uniform sampler2D normal_map;
layout(set = 3, binding = 0) uniform sampler2D env_map;
layout(set = 4, binding = 0) uniform sampler2D orm_map;

layout(push_constant) uniform PushConst {
    mat4  model;
    vec4  base_color;
    float metallic;
    float roughness;
    vec2  _pad;
} push;

layout(location = 0) out vec4 out_color;

#define M_PI 3.141592653589793

// ─── PBR ─────────────────────────────────────────────────────────────────────

vec3 F_Schlick(vec3 f0, float VdotH) {
    float x  = clamp(1.0 - VdotH, 0.0, 1.0);
    float x2 = x * x;
    float x5 = x * x2 * x2;
    return f0 + (1.0 - f0) * x5;
}

float V_GGX(float NdotL, float NdotV, float alphaRoughness) {
    float a2   = alphaRoughness * alphaRoughness;
    float GGXV = NdotL * sqrt(NdotV * NdotV * (1.0 - a2) + a2);
    float GGXL = NdotV * sqrt(NdotL * NdotL * (1.0 - a2) + a2);
    float GGX  = GGXV + GGXL;
    return (GGX > 0.0) ? 0.5 / GGX : 0.0;
}

float D_GGX(float NdotH, float alphaRoughness) {
    float a2 = alphaRoughness * alphaRoughness;
    float f  = (NdotH * NdotH) * (a2 - 1.0) + 1.0;
    return a2 / (M_PI * f * f);
}

float getRangeAttenuation(float range, float distance) {
    if (range <= 0.0) return 1.0 / (distance * distance);
    return max(min(1.0 - pow(distance / range, 4.0), 1.0), 0.0) / (distance * distance);
}

// ─── Normal mapping ───────────────────────────────────────────────────────────

mat3 cotangent_frame(vec3 N, vec3 pos, vec2 uv) {
    vec3 dp1  = dFdx(pos);
    vec3 dp2  = dFdy(pos);
    vec2 duv1 = dFdx(uv);
    vec2 duv2 = dFdy(uv);
    vec3 dp2perp = cross(dp2, N);
    vec3 dp1perp = cross(N, dp1);
    vec3 T = dp2perp * duv1.x + dp1perp * duv2.x;
    vec3 B = dp2perp * duv1.y + dp1perp * duv2.y;
    float denom = max(dot(T, T), dot(B, B));
    if (denom < 1e-6) {
        vec3 up = abs(N.y) < 0.999 ? vec3(0.0, 1.0, 0.0) : vec3(1.0, 0.0, 0.0);
        T = normalize(cross(up, N));
        B = cross(N, T);
    } else {
        float inv = inversesqrt(denom);
        T *= inv;
        B *= inv;
    }
    return mat3(T, B, N);
}

vec3 perturb_normal(vec3 N) {
    vec3 map    = texture(normal_map, frag_uv).rgb * 2.0 - 1.0;
    vec3 result = cotangent_frame(N, frag_world_pos, frag_uv) * map;
    float len   = dot(result, result);
    result = (len > 1e-6) ? result * inversesqrt(len) : N;
    return (dot(result, N) < 0.0) ? reflect(result, N) : result;
}

// ─── Shadows: Vogel disk PCSS ─────────────────────────────────────────────────
// Based on: Vlachos "Shadow Techniques from Left 4 Dead 2" (GDC 2010) and
//           Simon Claesson's Vogel disk sampling (2013).
// More uniform distribution than rotated Poisson → less banding.

// Interleaved gradient noise — screen-space rotation to break up patterns
// Source: Jimenez "Filmic SMAA" (2016)
float ign(vec2 pos) {
    return fract(52.9829189 * fract(dot(pos, vec2(0.06711056, 0.00583715))));
}

vec2 vogel_disk(int index, int count, float phi) {
    const float GOLDEN_ANGLE = 2.3999632; // 2*PI*(1 - 1/phi_golden)
    float r     = sqrt(float(index) + 0.5) / sqrt(float(count));
    float theta = float(index) * GOLDEN_ANGLE + phi;
    return r * vec2(cos(theta), sin(theta));
}

float find_blocker(vec2 uv, float z_recv, float search_r, float phi) {
    float total = 0.0;
    int   count = 0;
    for (int i = 0; i < 16; i++) {
        vec2  s = vogel_disk(i, 16, phi) * search_r;
        float d = texture(shadow_map, uv + s).r;
        if (d < z_recv) { total += d; count++; }
    }
    return count > 0 ? total / float(count) : -1.0;
}

float pcf_vogel(vec2 uv, float z_recv, float bias, float radius, float phi) {
    float shadow = 0.0;
    for (int i = 0; i < 32; i++) {
        vec2  s = vogel_disk(i, 32, phi) * radius;
        float d = texture(shadow_map, uv + s).r;
        shadow += (z_recv - bias > d) ? 1.0 : 0.0;
    }
    return shadow / 32.0;
}

float calc_shadow(vec4 lsp, vec3 N, vec3 L) {
    if (frame.shadows_enabled == 0) return 0.0;

    vec3 proj = lsp.xyz / lsp.w;
    proj.xy   = proj.xy * 0.5 + 0.5;

    // Reject fragments outside the shadow frustum
    if (proj.z < 0.0 || proj.z > 1.0) return 0.0;
    if (any(lessThan(proj.xy, vec2(0.0))) || any(greaterThan(proj.xy, vec2(1.0)))) return 0.0;

    float cos_t = clamp(dot(N, L), 0.0, 1.0);
    // Bias scaled to depth range [near=1, far=80] → 1/79 ≈ 0.013 per world unit.
    // UE4-style slope bias: larger for grazing surfaces, clamped to avoid swallowing close shadows.
    float bias = clamp(0.005 * tan(acos(cos_t)), 0.001, 0.006);

    float phi = ign(gl_FragCoord.xy) * 6.2832;

    float d_blocker = find_blocker(proj.xy, proj.z, 0.02, phi);
    if (d_blocker < 0.0) return 0.0;

    float penumbra = clamp((proj.z - d_blocker) * 80.0, 0.003, 0.05);
    return pcf_vogel(proj.xy, proj.z, bias, penumbra, phi);
}

// ─── IBL: split-sum approximation ────────────────────────────────────────────
// Based on: Karis "Real Shading in Unreal Engine 4" (SIGGRAPH 2013).
// Diffuse = highest-mip sample (≈ irradiance), specular = roughness-LOD sample.

vec3 EnvBRDFApprox(vec3 F0, float roughness, float NdotV) {
    const vec4 c0 = vec4(-1.0, -0.0275, -0.572,  0.022);
    const vec4 c1 = vec4( 1.0,  0.0425,  1.04,  -0.04 );
    vec4 r  = roughness * c0 + c1;
    float a = min(r.x * r.x, exp2(-9.28 * NdotV)) * r.x + r.y;
    vec2 AB = vec2(-1.04, 1.04) * a + r.zw;
    return clamp(F0 * AB.x + AB.y, 0.0, 1.0);
}

vec2 equirect_uv(vec3 d) {
    float phi   = atan(d.z, d.x);
    float theta = asin(clamp(-d.y, -1.0, 1.0));
    return vec2(0.5 + phi / (2.0 * M_PI), 0.5 - theta / M_PI);
}

vec3 ACESFilm(vec3 x) {
    return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), 0.0, 1.0);
}

void main() {
    vec3 N_geom = normalize(frag_normal);
    vec3 V      = normalize(frame.cam_pos.xyz - frag_world_pos);
    vec3 N      = perturb_normal(N_geom);

    vec3  albedo    = texture(albedo_tex, frag_uv).rgb * push.base_color.rgb;
    vec3  orm       = texture(orm_map, frag_uv).rgb;
    float metallic  = push.metallic * orm.b;
    float roughness = clamp(push.roughness * orm.g, 0.04, 1.0);
    float alpha     = roughness * roughness;

    vec3 F0    = mix(vec3(0.04), albedo, metallic);
    float NdotV = clamp(dot(N, V), 0.001, 1.0);

    vec3  Lo     = vec3(0.0);
    float shadow = 0.0;

    // Directional light
    {
        vec3  L         = normalize(frame.dir_light_dir.xyz);
        float intensity = frame.dir_light_dir.w;
        vec3  radiance  = frame.dir_light_color.xyz * intensity;

        shadow = calc_shadow(frag_light_space_pos, N_geom, L);

        float NdotL = clamp(dot(N, L), 0.001, 1.0);
        vec3  H     = normalize(V + L);
        float NdotH = clamp(dot(N, H), 0.0, 1.0);
        float VdotH = clamp(dot(V, H), 0.0, 1.0);

        vec3  F        = F_Schlick(F0, VdotH);
        float specular = V_GGX(NdotL, NdotV, alpha) * D_GGX(NdotH, alpha);
        vec3  diffuse  = (1.0 - F) * (1.0 - metallic) * albedo / M_PI;

        Lo += (1.0 - shadow) * (diffuse + F * specular) * radiance * NdotL;
    }

    // Point lights
    for (int i = 0; i < frame.point_count; i++) {
        vec3  lpos       = frame.point_pos_range[i].xyz;
        float lrange     = frame.point_pos_range[i].w;
        vec3  lcolor     = frame.point_color_intensity[i].xyz;
        float lintensity = frame.point_color_intensity[i].w;

        vec3  to_light = lpos - frag_world_pos;
        float dist     = length(to_light);
        vec3  L        = normalize(to_light);
        float atten    = getRangeAttenuation(lrange, dist);
        vec3  radiance = lcolor * lintensity * atten;

        float NdotL = clamp(dot(N, L), 0.001, 1.0);
        vec3  H     = normalize(V + L);
        float NdotH = clamp(dot(N, H), 0.0, 1.0);
        float VdotH = clamp(dot(V, H), 0.0, 1.0);

        vec3  F        = F_Schlick(F0, VdotH);
        float specular = V_GGX(NdotL, NdotV, alpha) * D_GGX(NdotH, alpha);
        vec3  diffuse  = (1.0 - F) * (1.0 - metallic) * albedo / M_PI;

        Lo += (diffuse + F * specular) * radiance * NdotL;
    }

    // Spot lights
    for (int i = 0; i < frame.spot_count; i++) {
        vec3  spos       = frame.spot_pos_range[i].xyz;
        float srange     = frame.spot_pos_range[i].w;
        vec3  scolor     = frame.spot_color_intensity[i].xyz;
        float sintensity = frame.spot_color_intensity[i].w;
        vec3  sdir       = normalize(frame.spot_dir_angle[i].xyz);
        float cos_angle  = frame.spot_dir_angle[i].w;

        vec3  to_light = spos - frag_world_pos;
        float dist     = length(to_light);
        vec3  L        = normalize(to_light);
        if (dot(L, -sdir) < cos_angle) continue;

        float atten    = getRangeAttenuation(srange, dist);
        vec3  radiance = scolor * sintensity * atten;

        float NdotL = clamp(dot(N, L), 0.001, 1.0);
        vec3  H     = normalize(V + L);
        float NdotH = clamp(dot(N, H), 0.0, 1.0);
        float VdotH = clamp(dot(V, H), 0.0, 1.0);

        vec3  F        = F_Schlick(F0, VdotH);
        float specular = V_GGX(NdotL, NdotV, alpha) * D_GGX(NdotH, alpha);
        vec3  diffuse  = (1.0 - F) * (1.0 - metallic) * albedo / M_PI;

        Lo += (diffuse + F * specular) * radiance * NdotL;
    }

    // IBL: split-sum (Karis "Real Shading in UE4", SIGGRAPH 2013)
    // UE4 rule: diffuse sky light is ambient — unaffected by shadow maps.
    // Specular (reflection capture) is attenuated in shadow proportional to roughness:
    // rough materials (wood, stone) lose most specular in shadow;
    // smooth metals keep more because their reflections come from all directions.
    vec3  R       = reflect(-V, N);
    float max_lod = float(textureQueryLevels(env_map) - 1);

    vec3 env_diffuse  = textureLod(env_map, equirect_uv(N), max_lod).rgb;
    vec3 env_specular = textureLod(env_map, equirect_uv(R), roughness * max_lod).rgb;

    vec3  F_amb     = F_Schlick(F0, NdotV);
    vec3  kD_amb    = (1.0 - F_amb) * (1.0 - metallic);
    vec3  spec_brdf = EnvBRDFApprox(F0, roughness, NdotV);

    vec3  configurable_amb = frame.ambient.xyz * frame.ambient.w;

    // Specular occlusion in shadow: attenuate based on roughness^2 so rough surfaces
    // (wood ~0.7-0.9 roughness) lose ~60-80% specular in shadow, metals (~0.0-0.3) keep most.
    float spec_occ = 1.0 - shadow * (roughness * roughness);

    vec3 ambient = kD_amb * albedo * (env_diffuse * 0.25 + configurable_amb)
                 + spec_brdf * env_specular * spec_occ;

    vec3 result = ambient + Lo;
    result = ACESFilm(result);
    result = pow(result, vec3(1.0 / 2.2));

    out_color = vec4(result, 1.0);
}

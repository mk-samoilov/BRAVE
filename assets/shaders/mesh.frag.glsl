#version 450

const float PI = 3.14159265359;

const int MAX_DIR_LIGHTS   = 4;
const int MAX_POINT_LIGHTS = 16;
const int MAX_SPOT_LIGHTS  = 8;

layout(location = 0) in vec3 frag_world_pos;
layout(location = 1) in vec3 frag_normal;
layout(location = 2) in vec2 frag_uv;

layout(location = 0) out vec4 out_color;

layout(set = 0, binding = 0) uniform SceneUBO {
    mat4 view_proj;
    vec4 camera_pos;
    vec4 camera_dir;
};

struct DirLightGPU {
    vec4 color_intensity;
    vec4 direction;
};

struct PointLightGPU {
    vec4 color_intensity;
    vec4 position_range;
};

struct SpotLightGPU {
    vec4 color_intensity;
    vec4 position_range;
    vec4 dir_cos_angle;
};

layout(set = 0, binding = 1) uniform LightsUBO {
    ivec4          counts;
    DirLightGPU    directional[MAX_DIR_LIGHTS];
    PointLightGPU  point_lights[MAX_POINT_LIGHTS];
    SpotLightGPU   spot_lights[MAX_SPOT_LIGHTS];
    vec4           ambient_color;
};

layout(set = 0, binding = 2) uniform ShadowUBO {
    mat4 dir_light_vp_0;
    mat4 dir_light_vp_1;
    mat4 dir_light_vp_2;
    vec4 cascade_splits;
};

layout(set = 0, binding = 3) uniform texture2DArray csm_shadow_map;
layout(set = 0, binding = 4) uniform sampler shadow_sampler;

layout(set = 0, binding = 5) uniform texture2D albedo_img;
layout(set = 0, binding = 6) uniform texture2D mr_img;
layout(set = 0, binding = 7) uniform texture2D normal_img;
layout(set = 0, binding = 8) uniform sampler tex_sampler;

layout(push_constant) uniform Push {
    mat4 model;
    vec4 albedo;
    vec4 mr;
};

float D_GGX(float NdotH, float roughness) {
    float a  = roughness * roughness;
    float a2 = a * a;
    float d  = NdotH * NdotH * (a2 - 1.0) + 1.0;
    return a2 / (PI * d * d);
}

float G_SchlickGGX(float NdotV, float roughness) {
    float r = roughness + 1.0;
    float k = (r * r) / 8.0;
    return NdotV / (NdotV * (1.0 - k) + k);
}

float G_Smith(float NdotV, float NdotL, float roughness) {
    return G_SchlickGGX(NdotV, roughness) * G_SchlickGGX(NdotL, roughness);
}

vec3 F_Schlick(float cosTheta, vec3 F0) {
    return F0 + (1.0 - F0) * pow(clamp(1.0 - cosTheta, 0.0, 1.0), 5.0);
}

vec3 pbr_light(vec3 L, vec3 radiance, vec3 N, vec3 V, vec3 F0, vec3 albedo_c, float metallic, float roughness) {
    vec3  H     = normalize(V + L);
    float NdotL = max(dot(N, L), 0.0);
    float NdotV = max(dot(N, V), 0.0001);
    float NdotH = max(dot(N, H), 0.0);
    float HdotV = max(dot(H, V), 0.0);

    float D        = D_GGX(NdotH, roughness);
    float G        = G_Smith(NdotV, NdotL, roughness);
    vec3  F        = F_Schlick(HdotV, F0);
    vec3  specular = (D * G * F) / max(4.0 * NdotV * NdotL, 0.0001);
    vec3  kd       = (1.0 - F) * (1.0 - metallic);

    return (kd * albedo_c / PI + specular) * radiance * NdotL;
}

float point_attenuation(float dist, float range) {
    float nd = dist / range;
    return pow(max(1.0 - nd * nd * nd * nd, 0.0), 2.0) / (dist * dist + 1.0);
}

mat3 cotangent_frame(vec3 N, vec3 pos, vec2 uv) {
    vec3 dp1  = dFdx(pos);
    vec3 dp2  = dFdy(pos);
    vec2 duv1 = dFdx(uv);
    vec2 duv2 = dFdy(uv);
    vec3 dp2perp = cross(dp2, N);
    vec3 dp1perp = cross(N, dp1);
    vec3 T = dp2perp * duv1.x + dp1perp * duv2.x;
    vec3 B = dp2perp * duv1.y + dp1perp * duv2.y;
    float invmax = inversesqrt(max(dot(T, T), dot(B, B)));
    return mat3(T * invmax, B * invmax, N);
}

float linstep(float lo, float hi, float v) {
    return clamp((v - lo) / (hi - lo), 0.0, 1.0);
}

float vsm_factor(vec2 moments, float compare) {
    if (compare <= moments.x) return 1.0;
    float variance = max(moments.y - moments.x * moments.x, 0.001);
    float d = compare - moments.x;
    float p_max = variance / (variance + d * d);
    return linstep(0.05, 1.0, p_max);
}

const float CSM_TEXEL = 1.0 / 2048.0;

float sample_cascade(vec3 world_pos, int cascade) {
    mat4 vp = cascade == 0 ? dir_light_vp_0 : (cascade == 1 ? dir_light_vp_1 : dir_light_vp_2);
    vec4 light_clip = vp * vec4(world_pos, 1.0);
    vec3 ndc = light_clip.xyz / light_clip.w;
    vec2 uv = ndc.xy * 0.5 + 0.5;
    if (any(lessThan(uv, vec2(0.0))) || any(greaterThan(uv, vec2(1.0)))) return 1.0;
    float layer = float(cascade);
    vec2 moments = vec2(0.0);
    moments += texture(sampler2DArray(csm_shadow_map, shadow_sampler), vec3(uv + vec2(-1, -1) * CSM_TEXEL, layer)).rg;
    moments += texture(sampler2DArray(csm_shadow_map, shadow_sampler), vec3(uv + vec2( 0, -1) * CSM_TEXEL, layer)).rg;
    moments += texture(sampler2DArray(csm_shadow_map, shadow_sampler), vec3(uv + vec2( 1, -1) * CSM_TEXEL, layer)).rg;
    moments += texture(sampler2DArray(csm_shadow_map, shadow_sampler), vec3(uv + vec2(-1,  0) * CSM_TEXEL, layer)).rg;
    moments += texture(sampler2DArray(csm_shadow_map, shadow_sampler), vec3(uv                            , layer)).rg;
    moments += texture(sampler2DArray(csm_shadow_map, shadow_sampler), vec3(uv + vec2( 1,  0) * CSM_TEXEL, layer)).rg;
    moments += texture(sampler2DArray(csm_shadow_map, shadow_sampler), vec3(uv + vec2(-1,  1) * CSM_TEXEL, layer)).rg;
    moments += texture(sampler2DArray(csm_shadow_map, shadow_sampler), vec3(uv + vec2( 0,  1) * CSM_TEXEL, layer)).rg;
    moments += texture(sampler2DArray(csm_shadow_map, shadow_sampler), vec3(uv + vec2( 1,  1) * CSM_TEXEL, layer)).rg;
    moments /= 9.0;
    return vsm_factor(moments, ndc.z);
}

float csm_shadow(vec3 world_pos) {
    float dist   = length(world_pos);

    float split0 = cascade_splits.x;
    float split1 = cascade_splits.y;
    float blend0 = split0 * 0.15;
    float blend1 = (split1 - split0) * 0.15;

    if (dist < split0 - blend0) {
        return sample_cascade(world_pos, 0);
    } else if (dist < split0) {
        float t = (dist - (split0 - blend0)) / blend0;
        return mix(sample_cascade(world_pos, 0), sample_cascade(world_pos, 1), t);
    } else if (dist < split1 - blend1) {
        return sample_cascade(world_pos, 1);
    } else if (dist < split1) {
        float t = (dist - (split1 - blend1)) / blend1;
        return mix(sample_cascade(world_pos, 1), sample_cascade(world_pos, 2), t);
    } else {
        return sample_cascade(world_pos, 2);
    }
}

void main() {
    vec4 albedo_sample = texture(sampler2D(albedo_img, tex_sampler), frag_uv);
    vec3 albedo_c = albedo.rgb * albedo_sample.rgb;

    vec4  mr_sample = texture(sampler2D(mr_img, tex_sampler), frag_uv);
    float metallic  = mr.x * mr_sample.b;
    float roughness = clamp(mr.y * mr_sample.g, 0.05, 1.0);

    vec3 Ng = normalize(frag_normal);
    vec3 ts_normal = texture(sampler2D(normal_img, tex_sampler), frag_uv).rgb * 2.0 - 1.0;
    mat3 TBN = cotangent_frame(Ng, frag_world_pos, frag_uv);
    vec3 N = normalize(TBN * ts_normal);

    vec3 V  = normalize(camera_pos.xyz - frag_world_pos);
    vec3 F0 = mix(vec3(0.04), albedo_c, metallic);

    vec3 Lo = vec3(0.0);

    for (int i = 0; i < counts.x; i++) {
        vec3 L        = normalize(-directional[i].direction.xyz);
        vec3 radiance = directional[i].color_intensity.rgb * directional[i].color_intensity.a;
        float shadow  = (i == 0) ? csm_shadow(frag_world_pos) : 1.0;
        Lo += shadow * pbr_light(L, radiance, N, V, F0, albedo_c, metallic, roughness);
    }

    for (int i = 0; i < counts.y; i++) {
        vec3  lpos  = point_lights[i].position_range.xyz;
        float range = point_lights[i].position_range.w;
        vec3  diff  = lpos - frag_world_pos;
        float dist  = length(diff);
        if (dist >= range) continue;
        vec3  L        = diff / dist;
        vec3  radiance = point_lights[i].color_intensity.rgb
                       * point_lights[i].color_intensity.a
                       * point_attenuation(dist, range);
        Lo += pbr_light(L, radiance, N, V, F0, albedo_c, metallic, roughness);
    }

    for (int i = 0; i < counts.z; i++) {
        vec3  lpos      = spot_lights[i].position_range.xyz;
        float range     = spot_lights[i].position_range.w;
        vec3  spot_dir  = normalize(spot_lights[i].dir_cos_angle.xyz);
        float cos_cut   = spot_lights[i].dir_cos_angle.w;
        vec3  diff      = lpos - frag_world_pos;
        float dist      = length(diff);
        if (dist >= range) continue;
        vec3  L         = diff / dist;
        float cos_theta = dot(-L, spot_dir);
        if (cos_theta < cos_cut) continue;
        float spot_f    = smoothstep(cos_cut, mix(cos_cut, 1.0, 0.2), cos_theta);
        vec3  radiance  = spot_lights[i].color_intensity.rgb
                        * spot_lights[i].color_intensity.a
                        * point_attenuation(dist, range)
                        * spot_f;
        Lo += pbr_light(L, radiance, N, V, F0, albedo_c, metallic, roughness);
    }

    vec3 ambient;
    if (counts.w > 0) {
        ambient = ambient_color.rgb * albedo_c;
    } else {
        ambient = vec3(0.03) * albedo_c;
    }

    vec3 color = ambient + Lo;
    color = color / (color + vec3(1.0));
    color = pow(color, vec3(1.0 / 2.2));

    out_color = vec4(color, albedo.a * albedo_sample.a);
}

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

layout(binding = 2) uniform sampler2D albedo_tex;
layout(binding = 3) uniform sampler2D mr_tex;
layout(binding = 4) uniform sampler2D normal_tex;

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

void main() {
    vec4 albedo_sample = texture(albedo_tex, frag_uv);
    vec3 albedo_c = albedo.rgb * albedo_sample.rgb;

    vec4  mr_sample = texture(mr_tex, frag_uv);
    float metallic  = mr.x * mr_sample.b;
    float roughness = clamp(mr.y * mr_sample.g, 0.05, 1.0);

    vec3 Ng = normalize(frag_normal);
    vec3 ts_normal = texture(normal_tex, frag_uv).rgb * 2.0 - 1.0;
    mat3 TBN = cotangent_frame(Ng, frag_world_pos, frag_uv);
    vec3 N = normalize(TBN * ts_normal);

    vec3 V  = normalize(camera_pos.xyz - frag_world_pos);
    vec3 F0 = mix(vec3(0.04), albedo_c, metallic);

    vec3 Lo = vec3(0.0);

    for (int i = 0; i < counts.x; i++) {
        vec3 L        = normalize(directional[i].direction.xyz);
        vec3 radiance = directional[i].color_intensity.rgb * directional[i].color_intensity.a;
        Lo += pbr_light(L, radiance, N, V, F0, albedo_c, metallic, roughness);
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

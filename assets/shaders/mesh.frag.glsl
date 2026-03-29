#version 450

const float PI = 3.14159265359;

layout(location = 0) in vec3 frag_world_pos;
layout(location = 1) in vec3 frag_normal;
layout(location = 2) in vec2 frag_uv;

layout(location = 0) out vec4 out_color;

layout(set = 0, binding = 0) uniform UBO {
    mat4 view_proj;
    vec4 camera_pos;
};

layout(push_constant) uniform Push {
    mat4 model;
    vec4 albedo;
    vec4 mr;
};

float D_GGX(float NdotH, float roughness) {
    float a = roughness * roughness;
    float a2 = a * a;
    float d = NdotH * NdotH * (a2 - 1.0) + 1.0;
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

void main() {
    vec3 albedo_color = albedo.rgb;
    float metallic = mr.x;
    float roughness = clamp(mr.y, 0.05, 1.0);

    vec3 N = normalize(frag_normal);
    vec3 V = normalize(camera_pos.xyz - frag_world_pos);
    vec3 F0 = mix(vec3(0.04), albedo_color, metallic);

    vec3 light_dir = normalize(vec3(0.5, 1.0, 0.3));
    vec3 light_color = vec3(1.0, 0.98, 0.95);
    float light_intensity = 3.0;

    vec3 L = light_dir;
    vec3 H = normalize(V + L);

    float NdotL = max(dot(N, L), 0.0);
    float NdotV = max(dot(N, V), 0.0001);
    float NdotH = max(dot(N, H), 0.0);
    float HdotV = max(dot(H, V), 0.0);

    float D = D_GGX(NdotH, roughness);
    float G = G_Smith(NdotV, NdotL, roughness);
    vec3 F = F_Schlick(HdotV, F0);

    vec3 specular = (D * G * F) / max(4.0 * NdotV * NdotL, 0.0001);
    vec3 kd = (1.0 - F) * (1.0 - metallic);

    vec3 radiance = light_color * light_intensity;
    vec3 Lo = (kd * albedo_color / PI + specular) * radiance * NdotL;

    vec3 ambient = vec3(0.03) * albedo_color;
    vec3 color = ambient + Lo;

    color = color / (color + vec3(1.0));
    color = pow(color, vec3(1.0 / 2.2));

    out_color = vec4(color, albedo.a);
}

#version 450

layout(location = 0) out vec2 out_moments;

void main() {
    float d  = gl_FragCoord.z;
    float d2 = d * d;
    d2 += 0.25 * (dFdx(d) * dFdx(d) + dFdy(d) * dFdy(d));
    out_moments = vec2(d, d2);
}

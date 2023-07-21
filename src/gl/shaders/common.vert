#version 300 es

layout (location = 0) in vec2 vPos;
layout (location = 1) in vec2 vUv;

out vec2 fUv;

void main() {
    fUv = vUv;
    gl_Position = vec4(vPos, 1., 1.);
}

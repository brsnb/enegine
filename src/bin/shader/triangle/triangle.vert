#version 450
#extension GL_ARB_separate_shader_objects : enable

layout(binding = 0) uniform UniformBufferObject {
    mat4 model;
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) in vec2 in_position;
layout(location = 1) in vec3 in_color;
layout(location = 2) in vec2 in_tex_coord;

layout(location = 0) out vec3 out_color;
layout(location = 1) out vec2 out_tex_coord;

out gl_PerVertex {
    vec4 gl_Position;
};

void main() {
    out_color = in_color;
    out_tex_coord = in_tex_coord;
    gl_Position = ubo.proj * ubo.view * ubo.model * vec4(in_position, 0.0, 1.0);
}

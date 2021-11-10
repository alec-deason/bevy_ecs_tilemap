#version 300 es
precision highp float;


# define TEXTURE_2D sampler2D
# define sampler2D(a, b) (a)
# define gl_VertexIndex gl_VertexID

in vec2 v_Uv;
in vec4 v_color;

 out vec4 o_Target;

layout(std140) uniform ColorMaterial_color {
    vec4 Color;
};

# ifdef COLORMATERIAL_TEXTURE 
uniform TEXTURE_2D ColorMaterial_texture;  // set = 2, binding = 1
# endif

void main() {
    vec4 color = Color * v_color;
# ifdef COLORMATERIAL_TEXTURE
    color *= texture(
        sampler2D(ColorMaterial_texture, ColorMaterial_texture_sampler),
        v_Uv);
# endif

    if (color.a < 0.001) {
        discard;
    }

    o_Target = color;
}

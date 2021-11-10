#version 300 es
precision highp float;

in vec3 Vertex_Position;
in ivec4 Vertex_Texture;
in vec4 Vertex_Color;

out vec2 v_Uv;
out vec4 v_color;
# define gl_VertexIndex gl_VertexID


uniform CameraViewProj {
    mat4 ViewProj;
};

uniform Transform {
    mat4 Model;
};

uniform TilemapData {
    vec2 texture_size;
    vec2 tile_size;
    vec2 grid_size;
    vec2 spacing;
    vec2 chunk_pos;
    vec2 map_size;
    float time;
};
void main() {
    vec2 uv = vec2(0.0);
    vec2 position = Vertex_Position.xy;

    vec2 positions[4] = vec2[4](
        vec2(position.x, position.y),
        vec2(position.x, position.y + 1.0),
        vec2(position.x + 1.0, position.y + 1.0),
        vec2(position.x + 1.0, position.y)
    );

    position = positions[gl_VertexID % 4];
    position.xy *= tile_size;

    float frames = float(Vertex_Texture.w - Vertex_Texture.z);

    float current_animation_frame = fract(time * Vertex_Position.z) * frames;

    current_animation_frame = clamp(current_animation_frame, float(Vertex_Texture.z), float(Vertex_Texture.w));

    int texture_index = int(current_animation_frame);

    int columns = int((texture_size.x + spacing.x) / (tile_size.x + spacing.x));

    float sprite_sheet_x = floor(float(texture_index % columns)) * (tile_size.x + spacing.x);
    float sprite_sheet_y = floor(float(texture_index) / float(columns)) * (tile_size.y + spacing.y);

    float start_u = sprite_sheet_x / texture_size.x;
    float end_u = (sprite_sheet_x + tile_size.x) / texture_size.x;
    float start_v = sprite_sheet_y / texture_size.y;
    float end_v = (sprite_sheet_y + tile_size.y) / texture_size.y;

    vec2 atlas_uvs[4];

    vec2 x1[8] = vec2[](
        vec2(start_u, end_v),       // no flip/rotation
        vec2(end_u, end_v),         // flip x
        vec2(start_u, start_v),     // flip y
        vec2(end_u, start_v),       // flip x y
        vec2(end_u, start_v),       // flip     d
        vec2(end_u, end_v),         // flip x   d
        vec2(start_u, start_v),     // flip y   d
        vec2(start_u, end_v)
    );

    vec2 x2[8] = vec2[](
        vec2(start_u, start_v),
        vec2(end_u, start_v),
        vec2(start_u, end_v),
        vec2(end_u, end_v),
        vec2(start_u, start_v),
        vec2(start_u, end_v),
        vec2(end_u, start_v),
        vec2(end_u, end_v)
    );

    vec2 x3[8] = vec2[](
        vec2(end_u, start_v),
        vec2(start_u, start_v),
        vec2(end_u, end_v),
        vec2(start_u, end_v),
        vec2(start_u, end_v),
        vec2(start_u, start_v),
        vec2(end_u, end_v),
        vec2(end_u, start_v)
    );

    vec2 x4[8] = vec2[](
        vec2(end_u, end_v),
        vec2(start_u, end_v),
        vec2(end_u, start_v),
        vec2(start_u, start_v),
        vec2(end_u, end_v),
        vec2(end_u, start_v),
        vec2(start_u, end_v),
        vec2(start_u, start_v)
    );

    atlas_uvs = vec2[4](
        x1[Vertex_Texture.y],
        x2[Vertex_Texture.y],
        x3[Vertex_Texture.y],
        x4[Vertex_Texture.y]
    );

    v_Uv = atlas_uvs[gl_VertexIndex % 4];
    v_Uv += 1e-5;
    v_color = Vertex_Color;
    gl_Position = ViewProj * Model * vec4(position, 0.0, 1.0);
}

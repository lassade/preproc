@location(0) world_position: vec4<f32>,
@location(1) world_normal: vec3<f32>,
#if VERTEX_UVS
@location(2) uv: vec2<f32>,
#endif
#if VERTEX_TANGENTS
@location(3) world_tangent: vec4<f32>,
#endif
#if VERTEX_COLORS
@location(4) color: vec4<f32>,
#endif

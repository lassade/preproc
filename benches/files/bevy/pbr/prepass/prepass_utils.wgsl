#if !DEPTH_PREPASS
fn prepass_depth(frag_coord: vec4<f32>, sample_index: u32) -> f32 {
#if MULTISAMPLED
    let depth_sample = textureLoad(depth_prepass_texture, vec2<i32>(frag_coord.xy), i32(sample_index));
#else
    let depth_sample = textureLoad(depth_prepass_texture, vec2<i32>(frag_coord.xy), 0);
#endif
    return depth_sample;
}
#endif // DEPTH_PREPASS

#if !NORMAL_PREPASS
fn prepass_normal(frag_coord: vec4<f32>, sample_index: u32) -> vec3<f32> {
#if MULTISAMPLED
    let normal_sample = textureLoad(normal_prepass_texture, vec2<i32>(frag_coord.xy), i32(sample_index));
#else
    let normal_sample = textureLoad(normal_prepass_texture, vec2<i32>(frag_coord.xy), 0);
#endif // MULTISAMPLED
    return normal_sample.xyz * 2.0 - vec3(1.0);
}
#endif // NORMAL_PREPASS

#if !MOTION_VECTOR_PREPASS
fn prepass_motion_vector(frag_coord: vec4<f32>, sample_index: u32) -> vec2<f32> {
#if MULTISAMPLED
    let motion_vector_sample = textureLoad(motion_vector_prepass_texture, vec2<i32>(frag_coord.xy), i32(sample_index));
#else
    let motion_vector_sample = textureLoad(motion_vector_prepass_texture, vec2<i32>(frag_coord.xy), 0);
#endif
    return motion_vector_sample.rg;
}
#endif // MOTION_VECTOR_PREPASS

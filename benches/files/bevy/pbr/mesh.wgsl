#include "pbr/mesh_view_bindings.wgsl"
#include "pbr/mesh_bindings.wgsl"

// NOTE: Bindings must come before functions that use them!
#include "pbr/mesh_functions.wgsl"

struct Vertex {
#if VERTEX_POSITIONS
    @location(0) position: vec3<f32>,
#endif
#if VERTEX_NORMALS
    @location(1) normal: vec3<f32>,
#endif
#if VERTEX_UVS
    @location(2) uv: vec2<f32>,
#endif
#if VERTEX_TANGENTS
    @location(3) tangent: vec4<f32>,
#endif
#if VERTEX_COLORS
    @location(4) color: vec4<f32>,
#endif
#if SKINNED
    @location(5) joint_indices: vec4<u32>,
    @location(6) joint_weights: vec4<f32>,
#endif
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    #include "pbr/mesh_vertex_output.wgsl"
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;

#if SKINNED
    var model = skin_model(vertex.joint_indices, vertex.joint_weights);
#else
    var model = mesh.model;
#endif

#if VERTEX_NORMALS
#if SKINNED
    out.world_normal = skin_normals(model, vertex.normal);
#else
    out.world_normal = mesh_normal_local_to_world(vertex.normal);
#endif
#endif

#if VERTEX_POSITIONS
    out.world_position = mesh_position_local_to_world(model, vec4<f32>(vertex.position, 1.0));
    out.clip_position = mesh_position_world_to_clip(out.world_position);
#endif

#if VERTEX_UVS
    out.uv = vertex.uv;
#endif

#if VERTEX_TANGENTS
    out.world_tangent = mesh_tangent_local_to_world(model, vertex.tangent);
#endif

#if VERTEX_COLORS
    out.color = vertex.color;
#endif

    return out;
}

struct FragmentInput {
    #include "pbr/mesh_vertex_output.wgsl"
};

@fragment
fn fragment(in: FragmentInput) -> @location(0) vec4<f32> {
#if VERTEX_COLORS
    return in.color;
#else
    return vec4<f32>(1.0, 0.0, 1.0, 1.0);
#endif
}

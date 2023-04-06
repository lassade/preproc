#include "pbr/mesh_types.wgsl"
#include "pbr/mesh_view_bindings.wgsl"

@group(1) @binding(0)
var<uniform> mesh: Mesh;

#if SKINNED
@group(1) @binding(1)
var<uniform> joint_matrices: SkinnedMesh;
#include "pbr/skinning.wgsl"
#endif

// NOTE: Bindings must come before functions that use them!
#include "pbr/mesh_functions.wgsl"

struct Vertex {
    @location(0) position: vec3<f32>,
#if SKINNED
    @location(4) joint_indexes: vec4<u32>,
    @location(5) joint_weights: vec4<f32>,
#endif
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
};

@vertex
fn vertex(vertex: Vertex) -> VertexOutput {
#if SKINNED
    let model = skin_model(vertex.joint_indexes, vertex.joint_weights);
#else
    let model = mesh.model;
#endif

    var out: VertexOutput;
    out.clip_position = mesh_position_local_to_clip(model, vec4<f32>(vertex.position, 1.0));
    return out;
}

@fragment
fn fragment() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}

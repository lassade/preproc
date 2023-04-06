#include "pbr/mesh_view_types.wgsl"
#include "pbr/mesh_types.wgsl"

@group(0) @binding(0)
var<uniform> view: View;
@group(0) @binding(1)
var<uniform> globals: Globals;

#if MOTION_VECTOR_PREPASS
@group(0) @binding(2)
var<uniform> previous_view_proj: mat4x4<f32>;
#endif // MOTION_VECTOR_PREPASS

// Material bindings will be in @group(1)

@group(2) @binding(0)
var<uniform> mesh: Mesh;

#if SKINNED
@group(2) @binding(1)
var<uniform> joint_matrices: SkinnedMesh;
#include "pbr/skinning.wgsl"
#endif

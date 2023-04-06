#include "pbr/mesh_types.wgsl"

@group(2) @binding(0)
var<uniform> mesh: Mesh;
#if SKINNED
@group(2) @binding(1)
var<uniform> joint_matrices: SkinnedMesh;
#include "pbr/skinning.wgsl"
#endif

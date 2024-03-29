#include "pbr/mesh_view_types.wgsl"

@group(0) @binding(0)
var<uniform> view: View;
@group(0) @binding(1)
var<uniform> lights: Lights;
#if NO_ARRAY_TEXTURES_SUPPORT
@group(0) @binding(2)
var point_shadow_textures: texture_depth_cube;
#else
@group(0) @binding(2)
var point_shadow_textures: texture_depth_cube_array;
#endif
@group(0) @binding(3)
var point_shadow_textures_sampler: sampler_comparison;
#if NO_ARRAY_TEXTURES_SUPPORT
@group(0) @binding(4)
var directional_shadow_textures: texture_depth_2d;
#else
@group(0) @binding(4)
var directional_shadow_textures: texture_depth_2d_array;
#endif
@group(0) @binding(5)
var directional_shadow_textures_sampler: sampler_comparison;

#if LIGHTS_USE_STORAGE
@group(0) @binding(6)
var<storage> point_lights: PointLights;
@group(0) @binding(7)
var<storage> cluster_light_index_lists: ClusterLightIndexLists;
@group(0) @binding(8)
var<storage> cluster_offsets_and_counts: ClusterOffsetsAndCounts;
#else
@group(0) @binding(6)
var<uniform> point_lights: PointLights;
@group(0) @binding(7)
var<uniform> cluster_light_index_lists: ClusterLightIndexLists;
@group(0) @binding(8)
var<uniform> cluster_offsets_and_counts: ClusterOffsetsAndCounts;
#endif

@group(0) @binding(9)
var<uniform> globals: Globals;
@group(0) @binding(10)
var<uniform> fog: Fog;

@group(0) @binding(11)
var environment_map_diffuse: texture_cube<f32>;
@group(0) @binding(12)
var environment_map_specular: texture_cube<f32>;
@group(0) @binding(13)
var environment_map_sampler: sampler;

@group(0) @binding(14)
var dt_lut_texture: texture_3d<f32>;
@group(0) @binding(15)
var dt_lut_sampler: sampler;

#if MULTISAMPLED
@group(0) @binding(16)
var depth_prepass_texture: texture_depth_multisampled_2d;
@group(0) @binding(17)
var normal_prepass_texture: texture_multisampled_2d<f32>;
@group(0) @binding(18)
var motion_vector_prepass_texture: texture_multisampled_2d<f32>;
#else
@group(0) @binding(16)
var depth_prepass_texture: texture_depth_2d;
@group(0) @binding(17)
var normal_prepass_texture: texture_2d<f32>;
@group(0) @binding(18)
var motion_vector_prepass_texture: texture_2d<f32>;
#endif

# Preproc

A simple C# like pre-processor for any source file.

# Usage

```rust
// build.rs
use std::{env, fs};

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    let mut pp = preproc::PP::default().search_path(crate_dir + "/shaders/include");

    println!("cargo:rerun-if-changed=shaders/vertex_lit.wgsl");
    fs::write(out_dir + "/unlit_vertex_color.wgsl", pp.parse_file(crate_dir + "/shaders/vertex_color.wgsl"));
    fs::write(out_dir + "/lambert_vertex_color.wgsl", pp.define("LAMBERT").parse_file(crate_dir + "/shaders/vertex_color.wgsl"));
}
```

```wgsl
// shading.wgsl
#if LAMBERT
fn shading(normal: vec3<f32>, light_dir: vec3<f32>) -> f32 {
    // lambert
    max(dot(normal, light_dir), 0.0)
}
#else 
fn shading(normal: vec3<f32>, light_dir: vec3<f32>) -> f32 {
    // unlit
    1.0
}
#endif
```

```wgsl
// vertex_color.wgsl
#include <shading.wgsl>
#include <vert.wgsl>

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    return in.color * shading(in.normal, light_dir);
}
```

```wgsl
// unlit_vertex_color.wgsl
fn shading(normal: vec3<f32>, light_dir: vec3<f32>) -> f32 {
    // unlit
    1.0
}
// `vert.wgsl` contents ...

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    return in.color * shading(in.normal, light_dir);
}
```

```wgsl
// lambert_vertex_color.wgsl
fn shading(normal: vec3<f32>, light_dir: vec3<f32>) -> f32 {
    // lambert
    max(dot(normal, light_dir), 0.0)
}
// `vert.wgsl` contents ...

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    return in.color * shading(in.normal, light_dir);
}
```
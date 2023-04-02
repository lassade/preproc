# preproc

Simple and configurable SIMD pre-processor, with a throughput of up to 3 GiB/s

# Quirks and Other Notes

- Supports only UTF-8 encoded files
- SSE2 required, no NEON support for the time been
- Whitespaces are considered to be `' ' (0x20)` and `'\t' (0x09)`
- Multiline comments aren't supported, but they work in some situations

# Samples

```c
//#if MY_MACRO // this directive is commented out
#if MY_OTHER_MACRO || MY_MACRO // this directive is active, single line comments are fine
// your code here
            #endif // doesn't care about white spaces as long the '#' is the frist char in the line
```

```c
// invalid multiline comment
/*#if MY_MACRO // won't be treated as a directive and won't be able to output the right code
// your code here
#endif*/

// valid multiline comments styles
/*
#if MY_MACRO
// your code here
#endif*/

/*
#if MY_MACRO
// your code here
#endif
*/
```

# Usage

With a build script you can pre-preprocess your shaders and output the needed variants

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
// vertex_color.wgsl
#include <shading.wgsl>
#include <vert.wgsl>

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    return in.color * shading(in.normal, light_dir);
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

outputs:

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
and

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

# Milestones

- [x] basic functionality with `#include`, `#if`, `#elif`, `#else` and `endif`
- [ ] `#define` and `#undef`
- [ ] query all available defines
- [ ] better error reporing
- [ ] fuzz test
- [ ] fewer allocations use an iterator (hard)
// todo: I need f16 in order to optimize the various varinings in the frament shader
//enable f16;

@group(0) @binding(0)
var<storage> data: array<u32>;

@group(1) @binding(0) var texture_color: texture_2d<f32>;
@group(1) @binding(1) var texture_sampler: sampler;

fn rd_f32(offset: u32) -> f32 {
    return bitcast<f32>(data[offset]);
}

fn rd_vec2f32(offset: u32) -> vec2<f32> {
    return vec2<f32>(rd_f32(offset), rd_f32(offset + 1u));
}

// read a sRGB and convert it to linear space for blending, this conversion is done in shader because
// the limited precision of the 32-bit colors make more suitable to store them in the gamma space as opose to linear space,
// more on: http://poynton.ca/notes/color/GammaFQA.html, https://en.wikipedia.org/wiki/Gamma_correction#Explanation
// gamma conversion taken from https://developer.nvidia.com/gpugems/gpugems3/part-iv-image-effects/chapter-24-importance-being-linear
fn rd_srgb(offset: u32) -> vec4<f32> {
    let srgb = unpack4x8unorm(data[offset]);
#if LINEAR_SPACE
    // default gamma correction
    return vec4<f32>(pow(srgb, vec4<f32>(2.2)).rgb, srgb.a);
#else
    // blend in srgb space
    return srgb;
#endif
}

fn rd_vec4f32(offset: u32) -> vec4<f32> {
    return vec4<f32>(rd_f32(offset), rd_f32(offset + 1u), rd_f32(offset + 2u), rd_f32(offset + 3u));
}

fn rd_mat4x4f32(offset: u32) -> mat4x4<f32> {
    return mat4x4<f32>(
        rd_f32(offset),
        rd_f32(offset +  1u),
        rd_f32(offset +  2u),
        rd_f32(offset +  3u),
        rd_f32(offset +  4u),
        rd_f32(offset +  5u),
        rd_f32(offset +  6u),
        rd_f32(offset +  7u),
        rd_f32(offset +  8u),
        rd_f32(offset +  9u),
        rd_f32(offset + 10u),
        rd_f32(offset + 11u),
        rd_f32(offset + 12u),
        rd_f32(offset + 13u),
        rd_f32(offset + 14u),
        rd_f32(offset + 15u),
    );
}

fn bezier2(t: f32, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>) -> vec2<f32> {
    let d = 1.0 - t;
    return d * d * d * p0 + 3.0 * d * d * t * p1 + 3.0 * d * t * t * p2 + t * t * t * p3;
}

fn bezier_normal2(t: f32, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>) -> vec2<f32> {
    let t = (-3.0 * p0 + 9.0 * p1 - 9.0 * p2 + 3.0 * p3) * (t * t)
        + (6.0 * p0 - 12.0 * p1 + 6.0 * p2) * t
        - 3.0 * p0
        + 3.0 * p1;
    return vec2<f32>(-t.y, t.x);
}

// todo: use f16
struct Vertex {
    // position in clip space
    @builtin(position) pos_cs: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) color: vec4<f32>, // color inpterolation isn't used, so it should be marked as flat
    @location(2) @interpolate(flat) p_offset: u32, // p_offset is used in in the selectionable render pass
#if TRANSFORM || CLIP_FADE
    // screen space
    @location(3) pos_ss: vec2<f32>,
    // constants
    @location(4) @interpolate(flat) clip_ss: vec4<f32>,
#endif
#if CLIP_FADE
    @location(5) @interpolate(flat) fade_ss: vec2<f32>,
#endif
#if SDF_MASKING
    // styling
    @location(6) uv_mask: vec2<f32>,
    @location(7) @interpolate(flat) style: vec4<f32>, // dilate, stroke, drop shadown, outter glow, inner glow
    @location(8) @interpolate(flat) stroke_color: vec3<f32>,
    @location(9) @interpolate(flat) shadow_color: vec4<f32>,
    @location(10) @interpolate(flat) outterglow_color: vec4<f32>,
#endif
    // @location(11) @interpolate(flat) debug: vec4<f32>,
};

fn build_primitive_vertex(primitive: u32) -> Vertex {
    // unpack primitive
    // [31.. type ..30|29.. primitive index ..24|23.. primitive offset  ..0]
    let p_offset = extractBits(primitive,  0u, 24u);
    let p_index  = extractBits(primitive, 24u,  6u);
    let p_type   = extractBits(primitive, 30u,  2u);

    var v: Vertex;
    v.p_offset = p_offset;
    var pos_ss: vec2<f32>;

    switch (p_type) {
        case 0u: { // handle rectangle
            let shape = rd_vec4f32(p_offset);
            let shape_size = shape.zw - shape.xy;
            let uv = rd_vec4f32(p_offset + 4u);
            v.color = rd_srgb(p_offset + 8u);
            let clip_offset = data[p_offset + 9u];
            let clip = rd_vec4f32(clip_offset);
#if TRANSFORM
            var rot = rd_f32(p_offset + 10u);
            let hshear = rd_f32(p_offset + 11u);
            var pivot = rd_vec2f32(p_offset + 12u);

            // pivot in screen space
            pivot = pivot * shape_size + shape.xy;
#endif

            switch (p_index) {
                case 0u: { // bottom left
                    pos_ss = shape.xy;
                    v.uv = uv.xy;
                }
                case 1u: { // bottom right
                    pos_ss = vec2<f32>(shape.z, shape.y);
                    v.uv = vec2<f32>(uv.z, uv.y);
                }
                case 2u: { // top left
                    pos_ss = vec2<f32>(shape.x, shape.w);
                    v.uv = vec2<f32>(uv.x, uv.w);
                }  
                case 3u: { // top right
                    pos_ss = shape.zw;
                    v.uv = uv.zw;
                } 
                default: {
                    pos_ss = vec2<f32>(0.0, 0.0);
                    v.uv = vec2<f32>(0.0, 0.0);
                }
            }

#if TRANSFORM
            // enable or not vertex cliping
            rot = fract(rot / 6.283185307179586476925286766559) * 6.283185307179586476925286766559; // wrap around 2 pi
            if (abs(rot) < 1e-9 && abs(hshear) < 1e-9) {
#endif
                let tmp = clamp(pos_ss, clip.xy, clip.zw);
                v.uv = v.uv + ((tmp - pos_ss) / max(shape_size, vec2<f32>(1e-9, 1e-9)) * (uv.zw - uv.xy));
                pos_ss = tmp;
#if TRANSFORM
            } else {
                // apply transform
                let s = sin(rot);
                let c = cos(rot);
                pos_ss = pos_ss - pivot;
                // | c, s| * |1, hshear| = | c, s + c * hshear |
                // |-s, c|   |0,      1|   |-s, c - s * hshear |
                pos_ss = mat2x2<f32>(c, -s, s + c * hshear, c - s * hshear) * pos_ss;
                pos_ss = pos_ss + pivot;
            }
#endif

            // clip rectangle is in screen space coordinates
#if TRANSFORM || CLIP_FADE
            v.clip_ss = clip;
#endif
#if CLIP_FADE
            // todo: fix fade to support all TBLR the components
            v.fade_ss = rd_vec4f32(clip_offset + 4u).xz;
#endif
        }
        case 1u: { // handle sliced rectangle


            pos_ss = vec2<f32>(0.0, 0.0);
            v.uv = vec2<f32>(0.0, 0.0);
#if TRANSFORM || CLIP_FADE
            v.clip_ss = vec4<f32>(0.0, 0.0, 0.0, 0.0);
#endif
#if CLIP_FADE
            v.fade_ss = vec2<f32>(0.0, 0.0);
#endif

            var shape = rd_vec4f32(p_offset);
            let shape_size = shape.zw - shape.xy;
            var uv = rd_vec4f32(p_offset + 4u);
            let uv_size = uv.zw - uv.xy;
            let borders = rd_vec4f32(p_offset + 8u);
            let border_scale = rd_f32(p_offset + 12u);
            v.color = rd_srgb(p_offset + 13u);
            let clip_offset = data[p_offset + 14u];
            let clip = rd_vec4f32(clip_offset);
#if TRANSFORM
            var rot = rd_f32(p_offset + 16u);
            let hshear = rd_f32(p_offset + 17u);
            var pivot = rd_vec2f32(p_offset + 18u);

            // pivot in screen space
            pivot = pivot * shape_size + shape.xy;
#endif

            let texture_size = rd_vec2f32(20u); // only read the first 2 floats of texture_params
            var shape_borders = borders * ((uv_size * texture_size) * border_scale).yyxx;

            // uniform rescale borders when there's no space left
            let req = shape_borders.xz + shape_borders.yw;
            let scale = clamp(min(shape_size.x, shape_size.y) / max(req.x, req.y), 0.0, 1.0);
            shape_borders = shape_borders * scale;

            let corner = extractBits(p_index, 0u, 2u);
            let slice  = extractBits(p_index, 2u, 4u);

            // todo: don't stretch sides if too big (hard and expensive without extra geometry)
            // todo: don't squeeze sides if too short insted clamp

            // slices representation
            // ┌───┬───┬───┐ 
            // │ 6 │ 7 │ 8 │
            // ├───┼───┼───┤ 
            // │ 3 │ 4 │ 5 │
            // ├───┼───┼───┤
            // │ 0 │ 1 │ 2 │
            // └───┴───┴───┘ 
            switch (slice) {
                case 0u: {
                    shape.z = shape.x + shape_borders.z;
                    shape.y = shape.w - shape_borders.y;
                    uv.z = uv.x + uv_size.x * borders.z;
                    uv.y = uv.w - uv_size.y * borders.y;
                }
                case 1u: {
                    shape.x = shape.x + shape_borders.z;
                    shape.z = shape.z - shape_borders.w;
                    shape.y = shape.w - shape_borders.y;
                    uv.x = uv.x + uv_size.x * borders.z;
                    uv.z = uv.z - uv_size.x * borders.w;
                    uv.y = uv.w - uv_size.y * borders.y;
                }
                case 2u: {
                    shape.x = shape.z - shape_borders.w;
                    shape.y = shape.w - shape_borders.y;
                    uv.x = uv.z - uv_size.x * borders.w;
                    uv.y = uv.w - uv_size.y * borders.y;
                }
                case 3u: {
                    shape.z = shape.x + shape_borders.z;
                    shape.y = shape.y + shape_borders.x;
                    shape.w = shape.w - shape_borders.y;
                    uv.z = uv.x + uv_size.x * borders.z;
                    uv.y = uv.y + uv_size.y * borders.x;
                    uv.w = uv.w - uv_size.y * borders.y;
                }
                case 4u: {
                    shape.x = shape.x + shape_borders.z;
                    shape.z = shape.z - shape_borders.w;
                    shape.y = shape.y + shape_borders.x;
                    shape.w = shape.w - shape_borders.y;
                    uv.x = uv.x + uv_size.x * borders.z;
                    uv.z = uv.z - uv_size.x * borders.w;
                    uv.y = uv.y + uv_size.y * borders.x;
                    uv.w = uv.w - uv_size.y * borders.y;
                }
                case 5u: {
                    shape.x = shape.z - shape_borders.w;
                    shape.y = shape.y + shape_borders.x;
                    shape.w = shape.w - shape_borders.y;
                    uv.x = uv.z - uv_size.x * borders.w;
                    uv.y = uv.y + uv_size.y * borders.x;
                    uv.w = uv.w - uv_size.y * borders.y;
                }
                case 6u: {
                    shape.z = shape.x + shape_borders.z;
                    shape.w = shape.y + shape_borders.x;
                    uv.z = uv.x + uv_size.x * borders.z;
                    uv.w = uv.y + uv_size.y * borders.x;
                }
                case 7u: {
                    shape.x = shape.x + shape_borders.z;
                    shape.z = shape.z - shape_borders.w;
                    shape.w = shape.y + shape_borders.x;
                    uv.x = uv.x + uv_size.x * borders.z;
                    uv.z = uv.z - uv_size.x * borders.w;
                    uv.w = uv.y + uv_size.y * borders.x;
                }
                case 8u: {
                    shape.x = shape.z - shape_borders.w;
                    shape.w = shape.y + shape_borders.x;
                    uv.x = uv.z - uv_size.x * borders.w;
                    uv.w = uv.y + uv_size.y * borders.x;
                }
                default: {
                    // invalid side don't draw
                    shape = shape.xyxy;
                }
            }

            switch (corner) {
                case 0u: { // bottom left
                    pos_ss = shape.xy;
                    v.uv = uv.xy;
                }
                case 1u: { // bottom right
                    pos_ss = vec2<f32>(shape.z, shape.y);
                    v.uv = vec2<f32>(uv.z, uv.y);
                }
                case 2u: { // top left
                    pos_ss = vec2<f32>(shape.x, shape.w);
                    v.uv = vec2<f32>(uv.x, uv.w);
                }  
                case 3u: { // top right
                    pos_ss = shape.zw;
                    v.uv = uv.zw;
                } 
                default: { 
                    pos_ss = vec2<f32>(0.0, 0.0);
                    v.uv = vec2<f32>(0.0, 0.0);
                }
            }

#if TRANSFORM
            // enable or not vertex cliping
            rot = fract(rot / 6.283185307179586476925286766559) * 6.283185307179586476925286766559; // wrap around 2 pi
            if (abs(rot) < 1e-9 && abs(hshear) < 1e-9) {
#endif
                let tmp = clamp(pos_ss, clip.xy, clip.zw);
                v.uv = v.uv + ((tmp - pos_ss) / max(shape_size, vec2<f32>(1e-9, 1e-9)) * uv_size);
                pos_ss = tmp;
#if TRANSFORM
            } else {
                // apply transform
                let s = sin(rot);
                let c = cos(rot);
                pos_ss = pos_ss - pivot;
                // | c, s| * |1, hshear| = | c, s + c * hshear |
                // |-s, c|   |0,      1|   |-s, c - s * hshear |
                pos_ss = mat2x2<f32>(c, -s, s + c * hshear, c - s * hshear) * pos_ss;
                pos_ss = pos_ss + pivot;
            }
#endif

            // clip rectangle is in screen space coordinates
#if TRANSFORM || CLIP_FADE
            v.clip_ss = clip;
#endif
#if CLIP_FADE
            // todo: fix fade to support all TBLR the components
            v.fade_ss = rd_vec4f32(clip_offset + 4u).xz;
#endif
        }
#if EXTRA_PRIMITIVES
        case 2u: { // handle bezier
            var uv = rd_vec4f32(p_offset);
            var p0 = rd_vec2f32(p_offset + 4u);
            var p1 = rd_vec2f32(p_offset + 6u);
            var p2 = rd_vec2f32(p_offset + 8u);
            var p3 = rd_vec2f32(p_offset + 10u);
            var width = rd_f32(p_offset + 12u);
            v.color = rd_srgb(p_offset + 13u);
            let clip_offset = data[p_offset + 14u];
            let factor = rd_f32(p_offset + 15u);

            let index = f32(extractBits(p_index, 0u, 5u));
            let t = index * factor;
            let ceil = f32(extractBits(p_index, 5u, 1u));
            let sign = sign(ceil - 0.5);
            var normal = sign * width * normalize(bezier_normal2(t, p0, p1, p2, p3));

            pos_ss = bezier2(t, p0, p1, p2, p3) + normal;
            //v.uv = (uv.zw - uv.xy) * vec2<f32>(t, ceil) + uv.xy; // fast lerp
            v.uv = (vec2<f32>(1.0, 1.0) - t) * uv.xy + t * uv.zw; // precise lerp

            // clip rectangle is in screen space coordinates
            v.clip_ss = rd_vec4f32(clip_offset);
#if CLIP_FADE
            // todo: fix fade to support all TBLR the components
            v.fade_ss = rd_vec4f32(clip_offset + 4u).xz;
#endif
        }
        case 3u: { // handle meshlet
            v.color = rd_srgb(p_offset);
            let clip_offset = data[p_offset + 1u];
            let tmp = rd_vec4f32(p_offset + 4u + p_index * 4u);
            pos_ss = tmp.xy;
            v.uv = tmp.zw;

            // clip rectangle is in screen space coordinates
            v.clip_ss = rd_vec4f32(clip_offset);
#if CLIP_FADE
            // todo: fix fade to support all TBLR the components
            v.fade_ss = rd_vec4f32(clip_offset + 4u).xz;
#endif
        }
#endif // EXTRA_PRIMITIVES
        default: {
            pos_ss = vec2<f32>(0.0, 0.0);
        }
    }

#if TRANSFORM || CLIP_FADE
    // save position while still in screen space
    v.pos_ss = pos_ss;
#endif

    // final position
    v.pos_cs = vec4<f32>(pos_ss, 0.0, 1.0);

    return v;
}

#if NON_INDEXED
// todo: currently unused, by giving up the hability of having other kinds of primitives other than a quad
// we can avoid having a index buffer and thus hopefully reducing the ammount of data we have to
// send o the GPU
//
// meant to be used with a triangle list
fn build_non_indexed_rectangle_vertex(i: u32) -> Vertex {
    let r_index = i / 6u; // each rectangles uses 6 vertices, 2 triangles
    let p_offset = r_index + 28u; // rectangle index plus the common data offset
    let vert_index  = i - (r_index * 6u); // vertice index

    var v: Vertex;
    v.p_offset = p_offset;
    var pos_ss: vec2<f32>;

    let shape = rd_vec4f32(p_offset);
    let shape_size = shape.zw - shape.xy;
    let uv = rd_vec4f32(p_offset + 4u);
    v.color = rd_srgb(p_offset + 8u);
    let clip_offset = data[p_offset + 9u];
    let clip = rd_vec4f32(clip_offset);
#if TRANSFORM
    var rot = rd_f32(p_offset + 10u);
    let hshear = rd_f32(p_offset + 11u);
    var pivot = rd_vec2f32(p_offset + 12u);

    // pivot in screen space
    pivot = pivot * shape_size + shape.xy;
#endif

    // translate vertex index to rectangle corner
    // vert_index: 0, 1, 2, 3, 4, 5
    // corner:     0, 3, 1, 3, 0, 2
    switch (vert_index) {
        case 0u, 4u { // corner: 0 (bottom left)
            pos_ss = shape.xy;
            v.uv = uv.xy;
        }
        case 2u { // corner: 1 (bottom right)
            pos_ss = shape.zy;
            v.uv = uv.zy;
        }
        case 5u { // corner: 2 (top left)
            pos_ss = shape.xw;
            v.uv = uv.xw;
        }  
        case 1u, 3u { // corner: 3 (top right)
            pos_ss = shape.zw;
            v.uv = uv.zw;
        } 
        default: {
            pos_ss = vec2<f32>(0.0, 0.0);
            v.uv = vec2<f32>(0.0, 0.0);
        }
    }

#if TRANSFORM
    // enable or not vertex cliping
    rot = fract(rot / 6.283185307179586476925286766559) * 6.283185307179586476925286766559; // wrap around 2 pi
    if (abs(rot) < 1e-9 && abs(hshear) < 1e-9) {
#endif
        let tmp = clamp(pos_ss, clip.xy, clip.zw);
        v.uv = v.uv + ((tmp - pos_ss) / max(shape_size, vec2<f32>(1e-9, 1e-9)) * (uv.zw - uv.xy));
        pos_ss = tmp;
#if TRANSFORM
    } else {
        // apply transform
        let s = sin(rot);
        let c = cos(rot);
        pos_ss = pos_ss - pivot;
        // | c, s| * |1, hshear| = | c, s + c * hshear |
        // |-s, c|   |0,      1|   |-s, c - s * hshear |
        pos_ss = mat2x2<f32>(c, -s, s + c * hshear, c - s * hshear) * pos_ss;
        pos_ss = pos_ss + pivot;
    }
#endif

    // clip rectangle is in screen space coordinates
#if TRANSFORM || CLIP_FADE
    v.clip_ss = clip;
#endif
#if CLIP_FADE
    // todo: fix fade to support all TBLR the components
    v.fade_ss = rd_vec4f32(clip_offset + 4u).xz;
#endif

#if TRANSFORM || CLIP_FADE
    // save position while still in screen space
    v.pos_ss = pos_ss; // hi?
#endif

    // final position
    v.pos_cs = vec4<f32>(pos_ss, 0.0, 1.0);

    return v;
}
#endif

@vertex
fn vs_color(@builtin(vertex_index) p: u32) -> Vertex {
    var v = build_primitive_vertex(p);

    // apply transform to clip space
    let transform = rd_mat4x4f32(0u);
    v.pos_cs = transform * v.pos_cs;

    return v;
}

@fragment
fn fs_color(in: Vertex) -> @location(0) vec4<f32> {
    var out = textureSample(texture_color, texture_sampler, in.uv);

#if TRANSFORM || CLIP_FADE
    // per pixel cliping, to support transformations, complex shapes and fade borders
    let p = in.pos_ss.xy;
    let p_min = p - in.clip_ss.xy;
    let p_max = p - in.clip_ss.zw;
    if (p_min.x < 0.0 || p_max.x > 0.0 || p_min.y < 0.0 || p_max.y > 0.0) {
        discard;
    }
#endif

#if CLIP_FADE
    // clip fade
    let d = min(abs(p_max), p_min) * in.fade_ss;

    // rectangular fade
    // let fade = min(min(d.x, d.y), 1.0);

    // rounded fade
    // todo: add a third multiplification factor in `d.x * d.y * other_factor` lower than 1.0 to better round the edges
    let fade = clamp(min(d.x * d.y, min(d.x, d.y)), 0.0, 1.0);

    // fade
    out.a = out.a * fade;
#endif

#if SDF_MASKING
    // todo: sdf masking
#endif

    // color
    out = in.color * out;

    return out;
}


@vertex
fn vs_selection(@builtin(vertex_index) p: u32) -> Vertex {
    var v = build_primitive_vertex(p);

    // to clip space, but using the selection params
    var normalize = rd_vec2f32(24u);
    var offset = -rd_vec2f32(26u);
    v.pos_cs = vec4<f32>((v.pos_cs.xy + offset) * normalize * 2.0 - 1.0, v.pos_cs.zw);

    return v;
}

@fragment
fn fs_selection(in: Vertex) -> @location(0) u32 {
    var out = textureSample(texture_color, texture_sampler, in.uv);

#if TRANSFORM
    // per pixel cliping, to support transformations, complex shapes and fade borders
    let p = in.pos_ss.xy;
    let p_min = p - in.clip_ss.xy;
    let p_max = p - in.clip_ss.zw;
    if (p_min.x < 0.0 || p_max.x > 0.0 || p_min.y < 0.0 || p_max.y > 0.0) {
        discard;
    }
#endif

    // doesn't do clip fade because the primitive will remain selectable unless fully transparent

#if SDF_MASKING
    // todo: sdf masking
#endif

    // alpha clip
    if (out.a <= 1.0e-6) {
        discard;
    }

    return in.p_offset;
}

// @vertex
// fn vs_fullscreen_triangle([[builtin(vertex_index)]] i: u32) -> [[builtin(position)]] vec4<f32> {
// 	let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
// 	return vec4<f32>((2.0 * uv) - vec2<f32>(1.0, 1.0), 0.0, 1.0);
// }

// @fragment
// fn fs_selection_clear() -> [[location(0)]] u32 {
//     return 4294967295u; // 0xffffffff;
// }
Code("// todo: I need f16 in order to optimize the various varinings in the frament shader")
Code("//enable f16;")
Code("")
Code("@group(0) @binding(0)")
Code("var<storage> data: array<u32>;")
Code("")
Code("@group(1) @binding(0) var texture_color: texture_2d<f32>;")
Code("@group(1) @binding(1) var texture_sampler: sampler;")
Code("")
Code("fn rd_f32(offset: u32) -> f32 {")
Code("    return bitcast<f32>(data[offset]);")
Code("}")
Code("")
Code("fn rd_vec2f32(offset: u32) -> vec2<f32> {")
Code("    return vec2<f32>(rd_f32(offset), rd_f32(offset + 1u));")
Code("}")
Code("")
Code("// read a sRGB and convert it to linear space for blending, this conversion is done in shader because")
Code("// the limited precision of the 32-bit colors make more suitable to store them in the gamma space as opose to linear space,")
Code("// more on: http://poynton.ca/notes/color/GammaFQA.html, https://en.wikipedia.org/wiki/Gamma_correction#Explanation")
Code("// gamma conversion taken from https://developer.nvidia.com/gpugems/gpugems3/part-iv-image-effects/chapter-24-importance-being-linear")
Code("fn rd_srgb(offset: u32) -> vec4<f32> {")
Code("    let srgb = unpack4x8unorm(data[offset]);")
If(Exp { ops: [Var("LINEAR_SPACE")] })
Code("    // default gamma correction")
Code("    return vec4<f32>(pow(srgb, vec4<f32>(2.2)).rgb, srgb.a);")
Else
Code("    // blend in srgb space")
Code("    return srgb;")
Code("")
Code("// the next endif contains an extra spaces")
Endif
Rem("  ")
Code("}")
Code("")
Code("fn rd_vec4f32(offset: u32) -> vec4<f32> {")
Code("    return vec4<f32>(rd_f32(offset), rd_f32(offset + 1u), rd_f32(offset + 2u), rd_f32(offset + 3u));")
Code("}")
Code("")
Code("fn rd_mat4x4f32(offset: u32) -> mat4x4<f32> {")
Code("    return mat4x4<f32>(")
Code("        rd_f32(offset),")
Code("        rd_f32(offset +  1u),")
Code("        rd_f32(offset +  2u),")
Code("        rd_f32(offset +  3u),")
Code("        rd_f32(offset +  4u),")
Code("        rd_f32(offset +  5u),")
Code("        rd_f32(offset +  6u),")
Code("        rd_f32(offset +  7u),")
Code("        rd_f32(offset +  8u),")
Code("        rd_f32(offset +  9u),")
Code("        rd_f32(offset + 10u),")
Code("        rd_f32(offset + 11u),")
Code("        rd_f32(offset + 12u),")
Code("        rd_f32(offset + 13u),")
Code("        rd_f32(offset + 14u),")
Code("        rd_f32(offset + 15u),")
Code("    );")
Code("}")
Code("")
Code("fn bezier2(t: f32, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>) -> vec2<f32> {")
Code("    let d = 1.0 - t;")
Code("    return d * d * d * p0 + 3.0 * d * d * t * p1 + 3.0 * d * t * t * p2 + t * t * t * p3;")
Code("}")
Code("")
Code("fn bezier_normal2(t: f32, p0: vec2<f32>, p1: vec2<f32>, p2: vec2<f32>, p3: vec2<f32>) -> vec2<f32> {")
Code("    let t = (-3.0 * p0 + 9.0 * p1 - 9.0 * p2 + 3.0 * p3) * (t * t)")
Code("        + (6.0 * p0 - 12.0 * p1 + 6.0 * p2) * t")
Code("        - 3.0 * p0")
Code("        + 3.0 * p1;")
Code("    return vec2<f32>(-t.y, t.x);")
Code("}")
Code("")
Code("// todo: use f16")
Code("struct Vertex {")
Code("    // position in clip space")
Code("    @builtin(position) pos_cs: vec4<f32>,")
Code("    @location(0) uv: vec2<f32>,")
Code("    @location(1) @interpolate(flat) color: vec4<f32>, // color inpterolation isn't used, so it should be marked as flat")
Code("    @location(2) @interpolate(flat) p_offset: u32, // p_offset is used in in the selectionable render pass")
If(Exp { ops: [Var("TRANSFORM"), Var("CLIP_FADE"), Or] })
Code("    // screen space")
Code("    @location(3) pos_ss: vec2<f32>,")
Code("    // constants")
Code("    @location(4) @interpolate(flat) clip_ss: vec4<f32>,")
Endif
If(Exp { ops: [Var("CLIP_FADE")] })
Code("    @location(5) @interpolate(flat) fade_ss: vec2<f32>,")
Endif
Code("")
Code("/* Multiline comment is supported but it must be in this form")
If(Exp { ops: [Var("SDF_MASKING")] })
Code("    // styling")
Code("    @location(6) uv_mask: vec2<f32>,")
Code("    @location(7) @interpolate(flat) style: vec4<f32>, // dilate, stroke, drop shadown, outter glow, inner glow")
Code("    @location(8) @interpolate(flat) stroke_color: vec3<f32>,")
Code("    @location(9) @interpolate(flat) shadow_color: vec4<f32>,")
Code("    @location(10) @interpolate(flat) outterglow_color: vec4<f32>,")
Endif
Code("*/")
Code("")
Code("// Multiline comment is supported bad multiline comment")
Code("/*#if SDF_MASKING")
Code("    // styling")
Code("    @location(6) uv_mask: vec2<f32>,")
Code("    @location(7) @interpolate(flat) style: vec4<f32>, // dilate, stroke, drop shadown, outter glow, inner glow")
Code("    @location(8) @interpolate(flat) stroke_color: vec3<f32>,")
Code("    @location(9) @interpolate(flat) shadow_color: vec4<f32>,")
Code("    @location(10) @interpolate(flat) outterglow_color: vec4<f32>,")
Endif
Rem("*/")
Code("")
Code("    // @location(11) @interpolate(flat) debug: vec4<f32>,")
Code("};")
Code("")
Code("fn build_primitive_vertex(primitive: u32) -> Vertex {")
Code("    // unpack primitive")
Code("    // [31.. type ..30|29.. primitive index ..24|23.. primitive offset  ..0]")
Code("    let p_offset = extractBits(primitive,  0u, 24u);")
Code("    let p_index  = extractBits(primitive, 24u,  6u);")
Code("    let p_type   = extractBits(primitive, 30u,  2u);")
Code("")
Code("    var v: Vertex;")
Code("    v.p_offset = p_offset;")
Code("    var pos_ss: vec2<f32>;")
Code("")
Code("    switch (p_type) {")
Code("        case 0u: { // handle rectangle")
Code("            let shape = rd_vec4f32(p_offset);")
Code("            let shape_size = shape.zw - shape.xy;")
Code("            let uv = rd_vec4f32(p_offset + 4u);")
Code("            v.color = rd_srgb(p_offset + 8u);")
Code("            let clip_offset = data[p_offset + 9u];")
Code("            let clip = rd_vec4f32(clip_offset);")
If(Exp { ops: [Var("TRANSFORM")] })
Rem("// comment after the end if expression")
Code("            var rot = rd_f32(p_offset + 10u);")
Code("            let hshear = rd_f32(p_offset + 11u);")
Code("            var pivot = rd_vec2f32(p_offset + 12u);")
Code("")
Code("            // pivot in screen space")
Code("            pivot = pivot * shape_size + shape.xy;")
Endif
Rem(" // comment after the endif directive")
Code("")
Code("            switch (p_index) {")
Code("                case 0u: { // bottom left")
Code("                    pos_ss = shape.xy;")
Code("                    v.uv = uv.xy;")
Code("                }")
Code("                case 1u: { // bottom right")
Code("                    pos_ss = vec2<f32>(shape.z, shape.y);")
Code("                    v.uv = vec2<f32>(uv.z, uv.y);")
Code("                }")
Code("                case 2u: { // top left")
Code("                    pos_ss = vec2<f32>(shape.x, shape.w);")
Code("                    v.uv = vec2<f32>(uv.x, uv.w);")
Code("                }  ")
Code("                case 3u: { // top right")
Code("                    pos_ss = shape.zw;")
Code("                    v.uv = uv.zw;")
Code("                } ")
Code("                default: {")
Code("                    pos_ss = vec2<f32>(0.0, 0.0);")
Code("                    v.uv = vec2<f32>(0.0, 0.0);")
Code("                }")
Code("            }")
Code("")
If(Exp { ops: [Var("TRANSFORM")] })
Code("            // enable or not vertex cliping")
Code("            rot = fract(rot / 6.283185307179586476925286766559) * 6.283185307179586476925286766559; // wrap around 2 pi")
Code("            if (abs(rot) < 1e-9 && abs(hshear) < 1e-9) {")
Endif
Code("                let tmp = clamp(pos_ss, clip.xy, clip.zw);")
Code("                v.uv = v.uv + ((tmp - pos_ss) / max(shape_size, vec2<f32>(1e-9, 1e-9)) * (uv.zw - uv.xy));")
Code("                pos_ss = tmp;")
If(Exp { ops: [Var("TRANSFORM")] })
Code("            } else {")
Code("                // apply transform")
Code("                let s = sin(rot);")
Code("                let c = cos(rot);")
Code("                pos_ss = pos_ss - pivot;")
Code("                // | c, s| * |1, hshear| = | c, s + c * hshear |")
Code("                // |-s, c|   |0,      1|   |-s, c - s * hshear |")
Code("                pos_ss = mat2x2<f32>(c, -s, s + c * hshear, c - s * hshear) * pos_ss;")
Code("                pos_ss = pos_ss + pivot;")
Code("            }")
Endif
Code("")
Code("            // clip rectangle is in screen space coordinates")
If(Exp { ops: [Var("TRANSFORM"), Var("CLIP_FADE"), Or] })
Code("            v.clip_ss = clip;")
Endif
If(Exp { ops: [Var("CLIP_FADE")] })
Code("            // todo: fix fade to support all TBLR the components")
Code("            v.fade_ss = rd_vec4f32(clip_offset + 4u).xz;")
Endif
Code("        }")
Code("        case 1u: { // handle sliced rectangle")
Code("")
Code("")
Code("            pos_ss = vec2<f32>(0.0, 0.0);")
Code("            v.uv = vec2<f32>(0.0, 0.0);")
If(Exp { ops: [Var("TRANSFORM"), Var("CLIP_FADE"), Or] })
Code("            v.clip_ss = vec4<f32>(0.0, 0.0, 0.0, 0.0);")
Endif
If(Exp { ops: [Var("CLIP_FADE")] })
Code("            v.fade_ss = vec2<f32>(0.0, 0.0);")
Endif
Code("")
Code("            var shape = rd_vec4f32(p_offset);")
Code("            let shape_size = shape.zw - shape.xy;")
Code("            var uv = rd_vec4f32(p_offset + 4u);")
Code("            let uv_size = uv.zw - uv.xy;")
Code("            let borders = rd_vec4f32(p_offset + 8u);")
Code("            let border_scale = rd_f32(p_offset + 12u);")
Code("            v.color = rd_srgb(p_offset + 13u);")
Code("            let clip_offset = data[p_offset + 14u];")
Code("            let clip = rd_vec4f32(clip_offset);")
If(Exp { ops: [Var("TRANSFORM")] })
Code("            var rot = rd_f32(p_offset + 16u);")
Code("            let hshear = rd_f32(p_offset + 17u);")
Code("            var pivot = rd_vec2f32(p_offset + 18u);")
Code("")
Code("            // pivot in screen space")
Code("            pivot = pivot * shape_size + shape.xy;")
Endif
Code("")
Code("            let texture_size = rd_vec2f32(20u); // only read the first 2 floats of texture_params")
Code("            var shape_borders = borders * ((uv_size * texture_size) * border_scale).yyxx;")
Code("")
Code("            // uniform rescale borders when there's no space left")
Code("            let req = shape_borders.xz + shape_borders.yw;")
Code("            let scale = clamp(min(shape_size.x, shape_size.y) / max(req.x, req.y), 0.0, 1.0);")
Code("            shape_borders = shape_borders * scale;")
Code("")
Code("            let corner = extractBits(p_index, 0u, 2u);")
Code("            let slice  = extractBits(p_index, 2u, 4u);")
Code("")
Code("            // todo: don't stretch sides if too big (hard and expensive without extra geometry)")
Code("            // todo: don't squeeze sides if too short insted clamp")
Code("")
Code("            // slices representation")
Code("            // ┌───┬───┬───┐ ")
Code("            // │ 6 │ 7 │ 8 │")
Code("            // ├───┼───┼───┤ ")
Code("            // │ 3 │ 4 │ 5 │")
Code("            // ├───┼───┼───┤")
Code("            // │ 0 │ 1 │ 2 │")
Code("            // └───┴───┴───┘ ")
Code("            switch (slice) {")
Code("                case 0u: {")
Code("                    shape.z = shape.x + shape_borders.z;")
Code("                    shape.y = shape.w - shape_borders.y;")
Code("                    uv.z = uv.x + uv_size.x * borders.z;")
Code("                    uv.y = uv.w - uv_size.y * borders.y;")
Code("                }")
Code("                case 1u: {")
Code("                    shape.x = shape.x + shape_borders.z;")
Code("                    shape.z = shape.z - shape_borders.w;")
Code("                    shape.y = shape.w - shape_borders.y;")
Code("                    uv.x = uv.x + uv_size.x * borders.z;")
Code("                    uv.z = uv.z - uv_size.x * borders.w;")
Code("                    uv.y = uv.w - uv_size.y * borders.y;")
Code("                }")
Code("                case 2u: {")
Code("                    shape.x = shape.z - shape_borders.w;")
Code("                    shape.y = shape.w - shape_borders.y;")
Code("                    uv.x = uv.z - uv_size.x * borders.w;")
Code("                    uv.y = uv.w - uv_size.y * borders.y;")
Code("                }")
Code("                case 3u: {")
Code("                    shape.z = shape.x + shape_borders.z;")
Code("                    shape.y = shape.y + shape_borders.x;")
Code("                    shape.w = shape.w - shape_borders.y;")
Code("                    uv.z = uv.x + uv_size.x * borders.z;")
Code("                    uv.y = uv.y + uv_size.y * borders.x;")
Code("                    uv.w = uv.w - uv_size.y * borders.y;")
Code("                }")
Code("                case 4u: {")
Code("                    shape.x = shape.x + shape_borders.z;")
Code("                    shape.z = shape.z - shape_borders.w;")
Code("                    shape.y = shape.y + shape_borders.x;")
Code("                    shape.w = shape.w - shape_borders.y;")
Code("                    uv.x = uv.x + uv_size.x * borders.z;")
Code("                    uv.z = uv.z - uv_size.x * borders.w;")
Code("                    uv.y = uv.y + uv_size.y * borders.x;")
Code("                    uv.w = uv.w - uv_size.y * borders.y;")
Code("                }")
Code("                case 5u: {")
Code("                    shape.x = shape.z - shape_borders.w;")
Code("                    shape.y = shape.y + shape_borders.x;")
Code("                    shape.w = shape.w - shape_borders.y;")
Code("                    uv.x = uv.z - uv_size.x * borders.w;")
Code("                    uv.y = uv.y + uv_size.y * borders.x;")
Code("                    uv.w = uv.w - uv_size.y * borders.y;")
Code("                }")
Code("                case 6u: {")
Code("                    shape.z = shape.x + shape_borders.z;")
Code("                    shape.w = shape.y + shape_borders.x;")
Code("                    uv.z = uv.x + uv_size.x * borders.z;")
Code("                    uv.w = uv.y + uv_size.y * borders.x;")
Code("                }")
Code("                case 7u: {")
Code("                    shape.x = shape.x + shape_borders.z;")
Code("                    shape.z = shape.z - shape_borders.w;")
Code("                    shape.w = shape.y + shape_borders.x;")
Code("                    uv.x = uv.x + uv_size.x * borders.z;")
Code("                    uv.z = uv.z - uv_size.x * borders.w;")
Code("                    uv.w = uv.y + uv_size.y * borders.x;")
Code("                }")
Code("                case 8u: {")
Code("                    shape.x = shape.z - shape_borders.w;")
Code("                    shape.w = shape.y + shape_borders.x;")
Code("                    uv.x = uv.z - uv_size.x * borders.w;")
Code("                    uv.w = uv.y + uv_size.y * borders.x;")
Code("                }")
Code("                default: {")
Code("                    // invalid side don't draw")
Code("                    shape = shape.xyxy;")
Code("                }")
Code("            }")
Code("")
Code("            switch (corner) {")
Code("                case 0u: { // bottom left")
Code("                    pos_ss = shape.xy;")
Code("                    v.uv = uv.xy;")
Code("                }")
Code("                case 1u: { // bottom right")
Code("                    pos_ss = vec2<f32>(shape.z, shape.y);")
Code("                    v.uv = vec2<f32>(uv.z, uv.y);")
Code("                }")
Code("                case 2u: { // top left")
Code("                    pos_ss = vec2<f32>(shape.x, shape.w);")
Code("                    v.uv = vec2<f32>(uv.x, uv.w);")
Code("                }  ")
Code("                case 3u: { // top right")
Code("                    pos_ss = shape.zw;")
Code("                    v.uv = uv.zw;")
Code("                } ")
Code("                default: { ")
Code("                    pos_ss = vec2<f32>(0.0, 0.0);")
Code("                    v.uv = vec2<f32>(0.0, 0.0);")
Code("                }")
Code("            }")
Code("")
If(Exp { ops: [Var("TRANSFORM")] })
Code("            // enable or not vertex cliping")
Code("            rot = fract(rot / 6.283185307179586476925286766559) * 6.283185307179586476925286766559; // wrap around 2 pi")
Code("            if (abs(rot) < 1e-9 && abs(hshear) < 1e-9) {")
Endif
Code("                let tmp = clamp(pos_ss, clip.xy, clip.zw);")
Code("                v.uv = v.uv + ((tmp - pos_ss) / max(shape_size, vec2<f32>(1e-9, 1e-9)) * uv_size);")
Code("                pos_ss = tmp;")
If(Exp { ops: [Var("TRANSFORM")] })
Code("            } else {")
Code("                // apply transform")
Code("                let s = sin(rot);")
Code("                let c = cos(rot);")
Code("                pos_ss = pos_ss - pivot;")
Code("                // | c, s| * |1, hshear| = | c, s + c * hshear |")
Code("                // |-s, c|   |0,      1|   |-s, c - s * hshear |")
Code("                pos_ss = mat2x2<f32>(c, -s, s + c * hshear, c - s * hshear) * pos_ss;")
Code("                pos_ss = pos_ss + pivot;")
Code("            }")
Endif
Code("")
Code("            // clip rectangle is in screen space coordinates")
If(Exp { ops: [Var("TRANSFORM"), Var("CLIP_FADE"), Or] })
Code("            v.clip_ss = clip;")
Endif
If(Exp { ops: [Var("CLIP_FADE")] })
Code("            // todo: fix fade to support all TBLR the components")
Code("            v.fade_ss = rd_vec4f32(clip_offset + 4u).xz;")
Endif
Code("        }")
If(Exp { ops: [Var("EXTRA_PRIMITIVES")] })
Code("        case 2u: { // handle bezier")
Code("            var uv = rd_vec4f32(p_offset);")
Code("            var p0 = rd_vec2f32(p_offset + 4u);")
Code("            var p1 = rd_vec2f32(p_offset + 6u);")
Code("            var p2 = rd_vec2f32(p_offset + 8u);")
Code("            var p3 = rd_vec2f32(p_offset + 10u);")
Code("            var width = rd_f32(p_offset + 12u);")
Code("            v.color = rd_srgb(p_offset + 13u);")
Code("            let clip_offset = data[p_offset + 14u];")
Code("            let factor = rd_f32(p_offset + 15u);")
Code("")
Code("            let index = f32(extractBits(p_index, 0u, 5u));")
Code("            let t = index * factor;")
Code("            let ceil = f32(extractBits(p_index, 5u, 1u));")
Code("            let sign = sign(ceil - 0.5);")
Code("            var normal = sign * width * normalize(bezier_normal2(t, p0, p1, p2, p3));")
Code("")
Code("            pos_ss = bezier2(t, p0, p1, p2, p3) + normal;")
Code("            //v.uv = (uv.zw - uv.xy) * vec2<f32>(t, ceil) + uv.xy; // fast lerp")
Code("            v.uv = (vec2<f32>(1.0, 1.0) - t) * uv.xy + t * uv.zw; // precise lerp")
Code("")
Code("            // clip rectangle is in screen space coordinates")
Code("            v.clip_ss = rd_vec4f32(clip_offset);")
If(Exp { ops: [Var("CLIP_FADE")] })
Code("            // todo: fix fade to support all TBLR the components")
Code("            v.fade_ss = rd_vec4f32(clip_offset + 4u).xz;")
Endif
Code("        }")
Code("        case 3u: { // handle meshlet")
Code("            v.color = rd_srgb(p_offset);")
Code("            let clip_offset = data[p_offset + 1u];")
Code("            let tmp = rd_vec4f32(p_offset + 4u + p_index * 4u);")
Code("            pos_ss = tmp.xy;")
Code("            v.uv = tmp.zw;")
Code("")
Code("            // clip rectangle is in screen space coordinates")
Code("            v.clip_ss = rd_vec4f32(clip_offset);")
If(Exp { ops: [Var("CLIP_FADE")] })
Code("            // todo: fix fade to support all TBLR the components")
Code("            v.fade_ss = rd_vec4f32(clip_offset + 4u).xz;")
Endif
Code("        }")
Endif
Rem(" // EXTRA_PRIMITIVES")
Code("        default: {")
Code("            pos_ss = vec2<f32>(0.0, 0.0);")
Code("        }")
Code("    }")
Code("")
If(Exp { ops: [Var("TRANSFORM"), Var("CLIP_FADE"), Or] })
Code("    // save position while still in screen space")
Code("    v.pos_ss = pos_ss;")
Endif
Code("")
Code("    // final position")
Code("    v.pos_cs = vec4<f32>(pos_ss, 0.0, 1.0);")
Code("")
Code("    return v;")
Code("}")
Code("")
If(Exp { ops: [Var("NON_INDEXED")] })
Code("// todo: currently unused, by giving up the hability of having other kinds of primitives other than a quad")
Code("// we can avoid having a index buffer and thus hopefully reducing the ammount of data we have to")
Code("// send o the GPU")
Code("//")
Code("// meant to be used with a triangle list")
Code("fn build_non_indexed_rectangle_vertex(i: u32) -> Vertex {")
Code("    let r_index = i / 6u; // each rectangles uses 6 vertices, 2 triangles")
Code("    let p_offset = r_index + 28u; // rectangle index plus the common data offset")
Code("    let vert_index  = i - (r_index * 6u); // vertice index")
Code("")
Code("    var v: Vertex;")
Code("    v.p_offset = p_offset;")
Code("    var pos_ss: vec2<f32>;")
Code("")
Code("    let shape = rd_vec4f32(p_offset);")
Code("    let shape_size = shape.zw - shape.xy;")
Code("    let uv = rd_vec4f32(p_offset + 4u);")
Code("    v.color = rd_srgb(p_offset + 8u);")
Code("    let clip_offset = data[p_offset + 9u];")
Code("    let clip = rd_vec4f32(clip_offset);")
If(Exp { ops: [Var("TRANSFORM")] })
Code("    var rot = rd_f32(p_offset + 10u);")
Code("    let hshear = rd_f32(p_offset + 11u);")
Code("    var pivot = rd_vec2f32(p_offset + 12u);")
Code("")
Code("    // pivot in screen space")
Code("    pivot = pivot * shape_size + shape.xy;")
Endif
Code("")
Code("    // translate vertex index to rectangle corner")
Code("    // vert_index: 0, 1, 2, 3, 4, 5")
Code("    // corner:     0, 3, 1, 3, 0, 2")
Code("    switch (vert_index) {")
Code("        case 0u, 4u { // corner: 0 (bottom left)")
Code("            pos_ss = shape.xy;")
Code("            v.uv = uv.xy;")
Code("        }")
Code("        case 2u { // corner: 1 (bottom right)")
Code("            pos_ss = shape.zy;")
Code("            v.uv = uv.zy;")
Code("        }")
Code("        case 5u { // corner: 2 (top left)")
Code("            pos_ss = shape.xw;")
Code("            v.uv = uv.xw;")
Code("        }  ")
Code("        case 1u, 3u { // corner: 3 (top right)")
Code("            pos_ss = shape.zw;")
Code("            v.uv = uv.zw;")
Code("        } ")
Code("        default: {")
Code("            pos_ss = vec2<f32>(0.0, 0.0);")
Code("            v.uv = vec2<f32>(0.0, 0.0);")
Code("        }")
Code("    }")
Code("")
If(Exp { ops: [Var("TRANSFORM")] })
Code("    // enable or not vertex cliping")
Code("    rot = fract(rot / 6.283185307179586476925286766559) * 6.283185307179586476925286766559; // wrap around 2 pi")
Code("    if (abs(rot) < 1e-9 && abs(hshear) < 1e-9) {")
Endif
Code("        let tmp = clamp(pos_ss, clip.xy, clip.zw);")
Code("        v.uv = v.uv + ((tmp - pos_ss) / max(shape_size, vec2<f32>(1e-9, 1e-9)) * (uv.zw - uv.xy));")
Code("        pos_ss = tmp;")
If(Exp { ops: [Var("TRANSFORM")] })
Code("    } else {")
Code("        // apply transform")
Code("        let s = sin(rot);")
Code("        let c = cos(rot);")
Code("        pos_ss = pos_ss - pivot;")
Code("        // | c, s| * |1, hshear| = | c, s + c * hshear |")
Code("        // |-s, c|   |0,      1|   |-s, c - s * hshear |")
Code("        pos_ss = mat2x2<f32>(c, -s, s + c * hshear, c - s * hshear) * pos_ss;")
Code("        pos_ss = pos_ss + pivot;")
Code("    }")
Endif
Code("")
Code("    // clip rectangle is in screen space coordinates")
If(Exp { ops: [Var("TRANSFORM"), Var("CLIP_FADE"), Or] })
Code("    v.clip_ss = clip;")
Endif
If(Exp { ops: [Var("CLIP_FADE")] })
Code("    // todo: fix fade to support all TBLR the components")
Code("    v.fade_ss = rd_vec4f32(clip_offset + 4u).xz;")
Endif
Code("")
If(Exp { ops: [Var("TRANSFORM"), Var("CLIP_FADE"), Or] })
Code("    // save position while still in screen space")
Code("    v.pos_ss = pos_ss; // hi?")
Endif
Code("")
Code("    // final position")
Code("    v.pos_cs = vec4<f32>(pos_ss, 0.0, 1.0);")
Code("")
Code("    return v;")
Code("}")
Endif
Code("")
Code("@vertex")
Code("fn vs_color(@builtin(vertex_index) p: u32) -> Vertex {")
Code("    var v = build_primitive_vertex(p);")
Code("")
Code("    // apply transform to clip space")
Code("    let transform = rd_mat4x4f32(0u);")
Code("    v.pos_cs = transform * v.pos_cs;")
Code("")
Code("    return v;")
Code("}")
Code("")
Code("@fragment")
Code("fn fs_color(in: Vertex) -> @location(0) vec4<f32> {")
Code("    var out = textureSample(texture_color, texture_sampler, in.uv);")
Code("")
If(Exp { ops: [Var("TRANSFORM"), Var("CLIP_FADE"), Or] })
Code("    // per pixel cliping, to support transformations, complex shapes and fade borders")
Code("    let p = in.pos_ss.xy;")
Code("    let p_min = p - in.clip_ss.xy;")
Code("    let p_max = p - in.clip_ss.zw;")
Code("    if (p_min.x < 0.0 || p_max.x > 0.0 || p_min.y < 0.0 || p_max.y > 0.0) {")
Code("        discard;")
Code("    }")
Endif
Code("")
If(Exp { ops: [Var("CLIP_FADE")] })
Code("    // clip fade")
Code("    let d = min(abs(p_max), p_min) * in.fade_ss;")
Code("")
Code("    // rectangular fade")
Code("    // let fade = min(min(d.x, d.y), 1.0);")
Code("")
Code("    // rounded fade")
Code("    // todo: add a third multiplification factor in `d.x * d.y * other_factor` lower than 1.0 to better round the edges")
Code("    let fade = clamp(min(d.x * d.y, min(d.x, d.y)), 0.0, 1.0);")
Code("")
Code("    // fade")
Code("    out.a = out.a * fade;")
Endif
Code("")
If(Exp { ops: [Var("SDF_MASKING")] })
Code("    // todo: sdf masking")
Endif
Code("")
Code("    // color")
Code("    out = in.color * out;")
Code("")
Code("    return out;")
Code("}")
Code("")
Code("")
Code("@vertex")
Code("fn vs_selection(@builtin(vertex_index) p: u32) -> Vertex {")
Code("    var v = build_primitive_vertex(p);")
Code("")
Code("    // to clip space, but using the selection params")
Code("    var normalize = rd_vec2f32(24u);")
Code("    var offset = -rd_vec2f32(26u);")
Code("    v.pos_cs = vec4<f32>((v.pos_cs.xy + offset) * normalize * 2.0 - 1.0, v.pos_cs.zw);")
Code("")
Code("    return v;")
Code("}")
Code("")
Code("@fragment")
Code("fn fs_selection(in: Vertex) -> @location(0) u32 {")
Code("    var out = textureSample(texture_color, texture_sampler, in.uv);")
Code("")
If(Exp { ops: [Var("TRANSFORM")] })
Code("    // per pixel cliping, to support transformations, complex shapes and fade borders")
Code("    let p = in.pos_ss.xy;")
Code("    let p_min = p - in.clip_ss.xy;")
Code("    let p_max = p - in.clip_ss.zw;")
Code("    if (p_min.x < 0.0 || p_max.x > 0.0 || p_min.y < 0.0 || p_max.y > 0.0) {")
Code("        discard;")
Code("    }")
Endif
Code("")
Code("    // doesn't do clip fade because the primitive will remain selectable unless fully transparent")
Code("")
If(Exp { ops: [Var("SDF_MASKING")] })
Code("    // todo: sdf masking")
Endif
Code("")
Code("    // alpha clip")
Code("    if (out.a <= 1.0e-6) {")
Code("        discard;")
Code("    }")
Code("")
Code("    return in.p_offset;")
Code("}")
Code("")
Code("// @vertex")
Code("// fn vs_fullscreen_triangle([[builtin(vertex_index)]] i: u32) -> [[builtin(position)]] vec4<f32> {")
Code("// \tlet uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));")
Code("// \treturn vec4<f32>((2.0 * uv) - vec2<f32>(1.0, 1.0), 0.0, 1.0);")
Code("// }")
Code("")
Code("// @fragment")
Code("// fn fs_selection_clear() -> [[location(0)]] u32 {")
Code("//     return 4294967295u; // 0xffffffff;")
Code("// }")

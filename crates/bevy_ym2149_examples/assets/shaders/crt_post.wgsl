#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct CrtParams {
    time: f32,
    width: f32,
    height: f32,
    crt_enabled: f32,
    impact: f32,
    chroma_px: f32,
    grain: f32,
    vignette: f32,
    flash: vec4<f32>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var scene_texture: texture_2d<f32>;

@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var scene_sampler: sampler;

@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var<uniform> U: CrtParams;

const CRT_EMU_SCALE   : f32 = 6.0;
const CRT_HARD_SCAN   : f32 = -8.0;
const CRT_HARD_PIX    : f32 = -3.0;
const CRT_WARP_FACTOR : vec2<f32> = vec2<f32>(1.0 / 32.0, 1.0 / 24.0);
const CRT_MASK_DARK   : f32 = 0.5;
const CRT_MASK_LIGHT  : f32 = 1.5;

fn srgb_to_linear_channel(c: f32) -> f32 {
    if (c <= 0.04045) {
        return c / 12.92;
    }
    return pow((c + 0.055) / 1.055, 2.4);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_to_linear_channel(c.x),
        srgb_to_linear_channel(c.y),
        srgb_to_linear_channel(c.z)
    );
}

fn linear_to_srgb_channel(c: f32) -> f32 {
    if (c <= 0.0031308) {
        return c * 12.92;
    }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        linear_to_srgb_channel(c.x),
        linear_to_srgb_channel(c.y),
        linear_to_srgb_channel(c.z)
    );
}

fn rand2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn sample_scene(uv: vec2<f32>) -> vec3<f32> {
    let res = vec2<f32>(U.width, U.height);
    let chroma_uv = vec2<f32>(U.chroma_px, 0.0) / max(res, vec2<f32>(1.0));
    let ca = chroma_uv * (0.35 + 0.9 * U.impact);

    let r = textureSample(scene_texture, scene_sampler, uv + ca).r;
    let g = textureSample(scene_texture, scene_sampler, uv).g;
    let b = textureSample(scene_texture, scene_sampler, uv - ca).b;
    return vec3<f32>(r, g, b);
}

fn crt_fetch(pos: vec2<f32>, offset: vec2<f32>, emu_res: vec2<f32>) -> vec3<f32> {
    let sample_pos = floor(pos * emu_res + offset) / emu_res;
    if (max(abs(sample_pos.x - 0.5), abs(sample_pos.y - 0.5)) > 0.5) {
        return vec3<f32>(0.0);
    }
    return linear_to_srgb(sample_scene(sample_pos));
}

fn crt_distance(pos: vec2<f32>, emu_res: vec2<f32>) -> vec2<f32> {
    let scaled = pos * emu_res;
    return -((scaled - floor(scaled)) - vec2<f32>(0.5));
}

fn crt_gaussian(pos: f32, scale: f32) -> f32 {
    return exp2(scale * pos * pos);
}

fn crt_sample_horizontal_3(pos: vec2<f32>, y_offset: f32, emu_res: vec2<f32>) -> vec3<f32> {
    let b = crt_fetch(pos, vec2<f32>(-1.0, y_offset), emu_res);
    let c = crt_fetch(pos, vec2<f32>( 0.0, y_offset), emu_res);
    let d = crt_fetch(pos, vec2<f32>( 1.0, y_offset), emu_res);

    let distance = crt_distance(pos, emu_res).x;
    let scale = CRT_HARD_PIX;

    let wb = crt_gaussian(distance - 1.0, scale);
    let wc = crt_gaussian(distance + 0.0, scale);
    let wd = crt_gaussian(distance + 1.0, scale);

    return (b * wb + c * wc + d * wd) / (wb + wc + wd);
}

fn crt_sample_horizontal_5(pos: vec2<f32>, y_offset: f32, emu_res: vec2<f32>) -> vec3<f32> {
    let a = crt_fetch(pos, vec2<f32>(-2.0, y_offset), emu_res);
    let b = crt_fetch(pos, vec2<f32>(-1.0, y_offset), emu_res);
    let c = crt_fetch(pos, vec2<f32>( 0.0, y_offset), emu_res);
    let d = crt_fetch(pos, vec2<f32>( 1.0, y_offset), emu_res);
    let e = crt_fetch(pos, vec2<f32>( 2.0, y_offset), emu_res);

    let distance = crt_distance(pos, emu_res).x;
    let scale = CRT_HARD_PIX;

    let wa = crt_gaussian(distance - 2.0, scale);
    let wb = crt_gaussian(distance - 1.0, scale);
    let wc = crt_gaussian(distance + 0.0, scale);
    let wd = crt_gaussian(distance + 1.0, scale);
    let we = crt_gaussian(distance + 2.0, scale);

    return (a * wa + b * wb + c * wc + d * wd + e * we) / (wa + wb + wc + wd + we);
}

fn crt_scanline_weight(pos: vec2<f32>, y_offset: f32, emu_res: vec2<f32>) -> f32 {
    let distance = crt_distance(pos, emu_res).y;
    return crt_gaussian(distance + y_offset, CRT_HARD_SCAN);
}

fn crt_triangle_filter(pos: vec2<f32>, emu_res: vec2<f32>) -> vec3<f32> {
    let above = crt_sample_horizontal_3(pos, -1.0, emu_res);
    let center = crt_sample_horizontal_5(pos,  0.0, emu_res);
    let below = crt_sample_horizontal_3(pos,  1.0, emu_res);

    let weight_above = crt_scanline_weight(pos, -1.0, emu_res);
    let weight_center = crt_scanline_weight(pos,  0.0, emu_res);
    let weight_below = crt_scanline_weight(pos,  1.0, emu_res);

    return above * weight_above + center * weight_center + below * weight_below;
}

fn crt_warp(pos: vec2<f32>) -> vec2<f32> {
    var p = pos * 2.0 - 1.0;
    p = p * vec2<f32>(
        1.0 + (p.y * p.y) * CRT_WARP_FACTOR.x,
        1.0 + (p.x * p.x) * CRT_WARP_FACTOR.y
    );
    return p * 0.5 + 0.5;
}

fn crt_shadow_mask(pos: vec2<f32>) -> vec3<f32> {
    var p = pos;
    p.x = p.x + p.y * 3.0;

    var mask = vec3<f32>(CRT_MASK_DARK);
    let fx = fract((p.x / 6.0));

    if (fx < 0.333) {
        mask.r = CRT_MASK_LIGHT;
    } else if (fx < 0.666) {
        mask.g = CRT_MASK_LIGHT;
    } else {
        mask.b = CRT_MASK_LIGHT;
    }

    return mask;
}

fn apply_crt_effect(frag_coord: vec2<f32>, uv: vec2<f32>) -> vec3<f32> {
    let resolution = vec2<f32>(U.width, U.height);
    let emu_res = resolution / CRT_EMU_SCALE;
    // Subtle scanline shimmer + impact wobble.
    let scan = sin(frag_coord.y * 0.15 + U.time * 60.0);
    let wobble = (scan * 0.00025) * (0.5 + 1.5 * U.impact);
    let warped_position = crt_warp(uv + vec2<f32>(wobble, 0.0));

    let filtered = crt_triangle_filter(warped_position, emu_res);
    let masked = filtered * crt_shadow_mask(frag_coord);

    var color = srgb_to_linear(max(masked, vec3<f32>(0.0)));

    // Vignette.
    let p = uv * 2.0 - 1.0;
    let vig = 1.0 - smoothstep(0.6, 1.25, length(p));
    color *= mix(1.0, vig, clamp(U.vignette, 0.0, 1.0));

    // Film grain (subtle).
    let n = rand2(frag_coord + vec2<f32>(U.time * 120.0, U.time * 47.0));
    let grain = (n - 0.5) * (U.grain * (0.25 + 0.35 * U.impact));
    color += vec3<f32>(grain);

    // Flash (additive).
    color += U.flash.rgb * U.flash.a;

    return color;
}

@fragment
fn fragment(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vertex.uv;
    let frag_coord = vec2<f32>(uv.x * U.width, (1.0 - uv.y) * U.height);
    let base_color = sample_scene(uv);

    if (U.crt_enabled < 0.5) {
        return vec4<f32>(base_color, 1.0);
    }

    let final_color = apply_crt_effect(frag_coord, uv);
    return vec4<f32>(final_color, 1.0);
}

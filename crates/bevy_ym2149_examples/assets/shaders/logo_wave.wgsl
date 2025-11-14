#import bevy_sprite::mesh2d_vertex_output::VertexOutput

struct LogoShaderParams {
    time: f32,
    amp_x: f32,
    freq_x: f32,
    speed_x: f32,
    amp_y: f32,
    freq_y: f32,
    speed_y: f32,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var logo_texture: texture_2d<f32>;

@group(#{MATERIAL_BIND_GROUP}) @binding(1)
var logo_sampler: sampler;

@group(#{MATERIAL_BIND_GROUP}) @binding(2)
var<uniform> params: LogoShaderParams;

// ---------- Wobble ----------

fn sine_distort(uv: vec2<f32>) -> vec2<f32> {
    let time = params.time;

    let edge_top = smoothstep(0.03, 0.13, uv.y);
    let edge_bottom = 1.0 - smoothstep(0.87, 0.97, uv.y);
    let fade_y = edge_top * edge_bottom;

    let edge_left = smoothstep(0.02, 0.12, uv.x);
    let edge_right = 1.0 - smoothstep(0.88, 0.98, uv.x);
    let fade_x = edge_left * edge_right;

    let wave_y =
        sin(uv.x * params.freq_y + time * params.speed_y) * params.amp_y * fade_y;

    let wave_x =
        sin(uv.y * params.freq_x + time * params.speed_x) * params.amp_x * fade_x;

    var distorted = uv;
    distorted.y = clamp(distorted.y + wave_y, 0.0, 1.0);
    distorted.x = clamp(distorted.x + wave_x, 0.0, 1.0);

    let margin = vec2<f32>(0.05, 0.08);
    let scale = vec2<f32>(1.0 - 2.0 * margin.x, 1.0 - 2.0 * margin.y);
    var safe = vec2<f32>(
        margin.x + distorted.x * scale.x,
        margin.y + distorted.y * scale.y,
    );
    return safe;
}

// ---------- Vignette / Scanlines ----------

fn vignette(uv: vec2<f32>) -> f32 {
    let c = uv * 2.0 - vec2<f32>(1.0, 1.0);
    let d = length(c);
    return 1.0 - smoothstep(0.5, 1.1, d);
}

fn scanlines(uv: vec2<f32>, time: f32) -> f32 {
    let lines = 240.0;
    let phase = uv.y * lines + time * 3.0;
    let s = sin(phase * 3.14159);
    return 0.85 + 0.15 * s;
}

// ---------- Sparks ----------

fn hash(n: vec2<f32>) -> f32 {
    let h = dot(n, vec2<f32>(12.9898, 78.233));
    return fract(sin(h) * 43758.5453);
}

fn spark(uv: vec2<f32>, time: f32) -> f32 {
    let n = hash(uv * 500.0 + vec2<f32>(time * 5.0, time * 3.0));
    let rare = step(0.997, n);                       // sehr selten
    let flicker = 0.5 + 0.5 * sin(time * 30.0 + n * 100.0);
    return rare * flicker;
}

// ---------- Neon Pulse (Glow-Farbe) ----------

fn neon_tint(time: f32) -> vec3<f32> {
    let phase = time * 0.8;
    let r = 0.20 + 0.20 * sin(phase + 0.0);
    let g = 0.10 + 0.15 * sin(phase + 2.1);
    let b = 0.35 + 0.25 * sin(phase + 4.2);
    return vec3<f32>(r, g, b);
}

// ---------- Chromatic Edges / Fringe ----------

fn edge_chroma(uv: vec2<f32>, time: f32, base_color: vec4<f32>) -> vec3<f32> {
    // stark dort, wo alpha abfällt (Logo-Kanten)
    let edge = smoothstep(0.0, 0.5, 1.0 - base_color.a);

    if (edge <= 0.0) {
        return base_color.rgb;
    }

    // kleine, zeitmodulierte Verschiebung
    let shift_base = 0.0025;
    let shift_wave = 0.0015 * sin(time * 2.5);
    let shift = shift_base + shift_wave;

    let off = vec2<f32>(shift, 0.0);

    let uv_r = clamp(uv + off, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));
    let uv_b = clamp(uv - off, vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0));

    let col_r = textureSample(logo_texture, logo_sampler, uv_r);
    let col_b = textureSample(logo_texture, logo_sampler, uv_b);

    let fringe = vec3<f32>(col_r.r, base_color.g, col_b.b);

    // Edge blendet zwischen normalem und chromatic-Rand
    return mix(base_color.rgb, fringe, edge);
}

// ---------- Fragment ----------

@fragment
fn fragment(vertex: VertexOutput) -> @location(0) vec4<f32> {
    let time = params.time;

    var uv = vertex.uv;

    // deutliches horizontales Shimmern
    let shimmer = 0.01 * sin(time * 3.0 + uv.y * 25.0);
    uv.x = clamp(uv.x + shimmer, 0.0, 1.0);

    // Wobble
    uv = sine_distort(uv);

    // Basis-Textur
    var color = textureSample(logo_texture, logo_sampler, uv);
    if (color.a <= 0.01) {
        discard;
    }

    // ----- Neon Pulse Glow -----
    let pulse = 0.5 + 0.5 * sin(time * 2.0);
    let glow_strength = mix(1.5, 3.5, pulse);

    let band_pos = fract(time * 0.4);
    let band_width = 0.2;
    let band = 1.0 - smoothstep(band_pos - band_width, band_pos + band_width, uv.x);
    let band_glow = band * 2.0;

    let exp_wave = 0.5 + 0.5 * sin(time * 4.0 + uv.x * 15.0);
    let glow_exponent = mix(1.2, 2.2, exp_wave);

    let base_glow = pow(color.a, glow_exponent) * glow_strength;
    let glow = base_glow + band_glow;

    let glow_tint = neon_tint(time);

    var rgb = color.rgb + glow_tint * glow;

    // ----- Chromatic Edges (Fringe) -----
    rgb = edge_chroma(uv, time, vec4<f32>(rgb, color.a));

    // ----- Sparks nur auf dem Logo -----
    let s = spark(uv, time);
    let spark_color = vec3<f32>(1.0, 0.93, 0.7);
    rgb += spark_color * s * color.a * 1.5;

    // ----- Scanlines + Vignette -----
    let sl = scanlines(uv, time);
    let vig = vignette(uv);
    rgb *= sl * vig;

    return vec4<f32>(rgb, color.a);
}

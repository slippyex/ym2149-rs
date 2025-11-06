#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// ===== Uniforms =====
struct Params {
    time: f32,
    width: f32,
    height: f32,
    mouse: vec4<f32>,
    frame: u32,
    crt_enabled: u32,
};
@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> U : Params;

// ===== Sequencing =====
const SCENE_COUNT         : u32 = 17u;  // 16 Einzelszenen + 1 JellyCube
const SCENE_DURATION      : f32 = 8.0;  // Sekunden pro Szene
const TRANSITION_DURATION : f32 = 2.0;  // Crossfade-Dauer

// ===== Consts =====
const PI : f32 = 3.14159265359;
const BORDER : f32 = 0.02;
const BORDERCOLOR : vec4<f32> = vec4<f32>(0.08, 0.08, 0.08, 1.0);

// ===== Helpers =====
fn rot2(a: f32) -> mat2x2<f32> {
    let s = sin(a); let c = cos(a);
    return mat2x2<f32>(vec2<f32>(c, -s), vec2<f32>(s, c));
}
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let res = exp(-k*a) + exp(-k*b);
    return -log(res) / k;
}
fn fract3(v: vec3<f32>) -> vec3<f32> { return v - floor(v); }
fn fract1(x: f32) -> f32 { return x - floor(x); }
// einfache zeitmodulierte Noise (global nutzbar)
// Deterministic hash function for stable noise patterns
fn hash21(p: vec2<f32>) -> f32 {
    let h = sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453;
    return fract1(h);
}

fn noise2(p_in: vec2<f32>) -> f32 {
    let i = floor(p_in);
    let f = p_in - i;
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (vec2<f32>(3.0, 3.0) - 2.0 * f);
    let mix_ab = mix(a, b, u.x);
    let mix_cd = mix(c, d, u.x);
    return mix(mix_ab, mix_cd, u.y);
}

fn edgef(a: vec2<f32>, b: vec2<f32>, p: vec2<f32>) -> f32 {
    let ab = b - a;
    let ap = p - a;
    return ab.x * ap.y - ab.y * ap.x;
}

fn bary(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, p: vec2<f32>) -> vec3<f32> {
    let area = edgef(a, b, c);
    if (abs(area) < 1e-6) {
        return vec3<f32>(-1.0, -1.0, -1.0);
    }
    let inv_area = 1.0 / area;
    return vec3<f32>(
        edgef(b, c, p) * inv_area,
        edgef(c, a, p) * inv_area,
        edgef(a, b, p) * inv_area
    );
}

fn aspect_uv(uv0: vec2<f32>) -> vec2<f32> {
    let res = vec2<f32>(U.width, U.height);
    var uv = uv0 * 2.0 - vec2<f32>(1.0, 1.0);
    uv.x = uv.x * (res.x / max(res.y, 1.0));
    return uv;
}

// Check if UV is within border and return appropriate color
fn check_border(uv: vec2<f32>) -> bool {
    return uv.x >= BORDER && uv.y >= BORDER && uv.x <= 1.0 - BORDER && uv.y <= 1.0 - BORDER;
}

fn circle(pos: vec2<f32>, pixel: vec2<f32>, radius: f32) -> f32 {
    return smoothstep(radius, radius * 0.3, distance(pos, pixel));
}

// ===== HSV + komplexe Mathematik =====
fn hsv2rgb(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    let q = clamp(p - K.xxx, vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0));
    return c.z * mix(K.xxx, q, vec3<f32>(c.y, c.y, c.y));
}

fn c_mul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

fn c_div(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let denom = max(dot(b, b), 1e-6);
    return vec2<f32>((a.x * b.x + a.y * b.y) / denom, (a.y * b.x - a.x * b.y) / denom);
}

fn c_exp(z: vec2<f32>) -> vec2<f32> {
    let e = exp(z.x);
    return vec2<f32>(cos(z.y), sin(z.y)) * e;
}

fn c_ln(z: vec2<f32>) -> vec2<f32> {
    let mag = max(length(z), 1e-6);
    return vec2<f32>(log(mag), atan2(z.y, z.x));
}

fn c_pow(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return c_exp(c_mul(b, c_ln(a)));
}

fn c_sin(z: vec2<f32>) -> vec2<f32> {
    return c_div(c_exp(z) - c_exp(-z), vec2<f32>(0.0, 2.0));
}

fn complex_iter(z_in: vec2<f32>, t: f32) -> vec2<f32> {
    var z = z_in;
    let cuv = vec2<f32>(cos(-t / 50.0), sin(-t / 50.0)) * 0.88;
    let max_mag = 1.0e9;
    let escape_mag = 1.2676506e30; // 2^100

    for (var i: i32 = 0; i < 100; i = i + 1) {
        let len_z = length(z);
        if (len_z < max_mag) {
            z = c_sin(c_mul(z, cuv));
        } else if (len_z > escape_mag) {
            break;
        }
    }
    return z;
}

// ===== Glenz-Bars (leicht verändert) =====
fn calcSine(uv: vec2<f32>, frequency: f32, amplitude: f32, shift: f32, offset: f32, color: vec3<f32>) -> vec3<f32> {
    // kleine, zeitabhängige Phasen-Drift
    let drift = 0.15 * sin(U.time * 0.35 + shift * 0.5);
    let y = sin(U.time * frequency + shift + uv.x + drift) * amplitude + offset;
    let scale = smoothstep(0.1, 0.0, abs(y - uv.y));
    return color * scale;
}
fn Bars(uv: vec2<f32>) -> vec3<f32> {
    var color = vec3<f32>(0.0);
    color += calcSine(uv, 2.0, 0.25, 0.0, 0.5, vec3<f32>(0.1, 0.1, 1.0));
    color += calcSine(uv, 2.6, 0.15, 0.2, 0.5, vec3<f32>(0.0, 1.0, 0.1));
    color += calcSine(uv, 0.9, 0.35, 0.4, 0.5, vec3<f32>(1.0, 0.1, 0.1));
    // sanfter globaler Tint
    let tint = 0.03 * sin(U.time * 0.6);
    return color * (1.0 + tint);
}
fn Twister(p: vec3<f32>) -> vec3<f32> {
    let f = sin(U.time/3.0)*1.45;
    let c = cos(f*p.y);
    let s = sin(f*0.5*p.y);
    let m = mat2x2<f32>(vec2<f32>(c, -s), vec2<f32>(s, c));
    let xz = m * p.xz;
    return vec3<f32>(xz.x, p.y, xz.y);
}
var<private> g_cubevec : vec3<f32> = vec3<f32>(0.0,0.0,0.0);
fn Cube(p_in: vec3<f32>) -> f32 {
    var p = Twister(p_in);
    let rotv = vec2<f32>(sin(U.time), cos(U.time));
    let m = mat2x2<f32>(vec2<f32>(rotv.y, -rotv.x), vec2<f32>(rotv.x, rotv.y));
    var xy = m * p.xy; p = vec3<f32>(xy.x, xy.y, p.z);
    xy = m * p.xy;     p = vec3<f32>(xy.x, xy.y, p.z);
    var yz = m * p.yz; p = vec3<f32>(p.x, yz.x, yz.y);
    var zx = m * p.zx; p = vec3<f32>(zx.y, p.y, zx.x);
    zx = m * p.zx;     p = vec3<f32>(zx.y, p.y, zx.x);
    zx = m * p.zx;     p = vec3<f32>(zx.y, p.y, zx.x);
    g_cubevec = p;
    let q = max(abs(p) - vec3<f32>(0.4,0.4,0.4), vec3<f32>(0.0,0.0,0.0));
    return length(q) - 0.08;
}
fn FaceEdge(uv_in: vec2<f32>) -> f32 {
    var uv = uv_in;
    uv.y = uv.y - floor(uv.y);
    let a = uv.y < uv.x;
    let b = (1.0 - uv.y) < uv.x;
    return select(1.0, 0.0, a == b);
}
fn getNormalCube(p: vec3<f32>) -> vec3<f32> {
    let e = vec3<f32>(0.005, -0.005, 0.0);
    return normalize(
        e.xyy * Cube(p + e.xyy) +
        e.yyx * Cube(p + e.yyx) +
        e.yxy * Cube(p + e.yxy) +
        e.xxx * Cube(p + e.xxx)
    );
}
fn Glenz(uv: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv)) { return BORDERCOLOR; }
    let p = -1.0 + uv * 2.0;

    var Distance: f32 = 0.0;
    var Near: f32 = -1.0;
    var Far: f32 = -1.0;
    var hd: f32 = -1.0;

    let ro = vec3<f32>(0.0, 0.0, 2.1);
    let rd = normalize(vec3<f32>(p, -2.0));
    for (var i: u32 = 0u; i < 256u; i = i + 1u) {
        let Step = Cube(ro + rd * Distance);
        Distance = Distance + Step * 0.5;
        if (Distance > 4.0) { break; }
        if (Step < 0.001) {
            Far = FaceEdge(g_cubevec.yx) + FaceEdge(-g_cubevec.yx) + FaceEdge(g_cubevec.xz) + FaceEdge(-g_cubevec.xz) + FaceEdge(g_cubevec.zy) + FaceEdge(-g_cubevec.zy);
            if (hd < 0.0) { hd = Distance; }
            if (Near < 0.0) { Near = Far; }
            Distance = Distance + 0.05;
        }
    }

    var Color = Bars(uv);
    if (Near > 0.0) {
        // leicht andere Beleuchtung & Spec
        let lightPos = vec3<f32>(1.2, 0.4, 0.6);
        let sp = ro + rd * hd;
        var ld = lightPos - sp;
        let lDist = max(length(ld), 0.001);
        ld = ld / lDist;
        let ambience = 0.65;
        let sn = getNormalCube(sp);
        let diff = min(0.35, max(dot(sn, ld), 0.0));
        let spec = pow(max(dot(reflect(-ld, sn), -rd), 0.0), 28.0);
        let mixv = vec3<f32>(Near * 0.42 + Far * Far * 0.05);
        let baseA = mix(vec3<f32>(0.25, 0.05, 1.0), vec3<f32>(1.0, 1.0, 1.0), mixv);
        var tcol = baseA;
        tcol = tcol * (diff + ambience) + vec3<f32>(0.9, 0.6, 1.0) * spec / 1.4;
        Color = tcol;
    }
    return vec4<f32>(Color, 1.0);
}

// ===== Plasma =====
fn Plasma(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let time = U.time * 4.0;
    // kleine, langsame Rotation
    let ang = 0.05 * sin(U.time * 0.5);
    var uv = (rot2(ang) * (uv_in * 6.0)).xy;

    var i0 = 1.0;
    var i1 = 1.0;
    var i2 = 1.0;
    var i4 = 0.0;

    for (var s: i32 = 0; s < 7; s = s + 1) {
        var r = vec2<f32>(cos(uv.y * i0 - i4 + time / (i1 + 0.1)), sin(uv.x * i0 - i4 + time / (i1 + 0.1))) / (i2 + 0.02);
        r = r + vec2<f32>(-r.y, r.x) * (0.28 + 0.02 * sin(U.time));
        uv = uv + r;
        i0 = i0 * 1.91; i1 = i1 * 1.14; i2 = i2 * 1.68; i4 = i4 + 0.05 + 0.1 * time * i1;
    }
    // leicht anderes Farbmapping
    let r = sin(uv.x - time) * 0.5 + 0.5;
    let g = sin((uv.x + uv.y + sin(time * 0.45)) * 0.52) * 0.5 + 0.5;
    let b = sin(uv.y + time * 0.95) * 0.5 + 0.5;
    return vec4<f32>(r, g, b, 1.0);
}
fn SceneBound(p: vec3<f32>) -> f32 {
    return max(max(abs(p.x), abs(p.y)), abs(p.z)) - 5.0;
}
fn NormalBound(p: vec3<f32>) -> vec3<f32> {
	let o = vec3<f32>(0.01, 0.0, 0.0);
    return normalize(vec3<f32>(
        SceneBound(p-o.xyz)-SceneBound(p+o.xyz),
        SceneBound(p-o.zxy)-SceneBound(p+o.zxy),
        SceneBound(p-o.yzx)-SceneBound(p+o.yzx)
    ));
}
fn RayMarch(ro: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    var hd = 0.0;
    for (var i: i32 = 0; i < 128; i = i + 1) {
        let d = SceneBound(ro + rd * hd);
        hd = hd + d;
        if (d < 0.0001) { break; }
    }
    return ro + rd * hd;
}

// ===== Flow (leicht verändert) =====
fn Flow(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    var p = uv_in * 2.0 - 1.0;
    let t = U.time;
    let r = length(p);
    let a = atan2(p.y, p.x);
    let w = 0.5 + 0.5 * sin(8.0 * r - 3.1 * t);
    let g = 0.5 + 0.5 * sin(3.2 * a + 2.0 * t + 0.2);
    let b = 0.5 + 0.5 * sin(5.0 * a - 1.45 * t);
    let base = vec3<f32>(w, g, b);
    // leicht andere Vignette
    let vig = 1.0 - smoothstep(0.68, 1.0, r);
    return vec4<f32>(base * (0.62 + 0.38 * vig), 1.0);
}

// ===== Glenz Vectors =====
// ===== IFS Fractal Flames =====
fn FractalFlame(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let aspect = U.width / max(U.height, 1.0);
    var uv = uv_in * 2.0 - 1.0;
    uv.x = uv.x * aspect;
    let time = U.time * 0.35;

    var p = uv;
    var accum = vec3<f32>(0.0, 0.0, 0.0);
    var weight = 0.0;
    var ripple = 0.0;

    for (var i = 0; i < 7; i = i + 1) {
        let fi = f32(i);
        let ang = time * 0.7 + fi * 2.3999632; // golden angle spacing
        let rot = mat2x2<f32>(
            vec2<f32>(cos(ang), -sin(ang)),
            vec2<f32>(sin(ang), cos(ang))
        );
        let twist = rot * p;
        let rad = length(twist);
        let fade = exp(-rad * 4.0);
        let flame = fade * (0.6 + 0.4 * sin(rad * 12.0 - time * 3.0 + fi));

        accum = accum + vec3<f32>(
            0.6 + 0.4 * sin(twist.x * 3.0 + time),
            0.6 + 0.4 * sin(twist.y * 5.0 - time * 0.5),
            0.6 + 0.4 * sin((twist.x + twist.y) * 4.0 + time * 1.5)
        ) * flame;

        weight = weight + flame;
        ripple = ripple + flame * rad;

        p = vec2<f32>(
            twist.x * twist.x - twist.y * twist.y,
            2.0 * twist.x * twist.y
        ) + 0.35 * vec2<f32>(
            sin(time + fi * 0.73),
            cos(time * 1.1 + fi * 1.37)
        );
    }

    var color = accum / max(weight, 0.0001);
    color = mix(vec3<f32>(0.2, 0.05, 0.02), color, 0.7);
    color = color * (1.15 - clamp(ripple * 0.8, 0.0, 0.9));
    let vig = 0.65 + 0.35 * smoothstep(1.75, 0.6, length(uv));
    color = color * vig;
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== Julia Morphs =====
fn JuliaMorph(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let aspect = U.width / max(U.height, 1.0);
    var uv = uv_in * 2.0 - 1.0;
    uv.x = uv.x * aspect;
    let time = U.time * 0.4;

    let scale = 1.8;
    var z = uv * scale;
    let c = vec2<f32>(
        0.35 * sin(time) - 0.2 * cos(time * 0.3),
        0.35 * cos(time * 1.3) + 0.2 * sin(time * 0.5)
    );

    var accum = 0.0;
    var smooth_acc = 0.0;
    var trap = vec3<f32>(1e5, 1e5, 1e5);
    var dr = 1.0;
    let bailout = 12.0;
    let iterations: i32 = 48;
    var escape = 0.0;

    for (var i: i32 = 0; i < iterations; i = i + 1) {
        let zx = z.x;
        let zy = z.y;
        let r2 = zx * zx + zy * zy;
        let radius = sqrt(r2);
        dr = dr * max(radius, 0.0001) * 2.0;

        var new_z = vec2<f32>(
            zx * zx - zy * zy,
            2.0 * zx * zy
        ) + c;

        new_z = new_z + 0.15 * vec2<f32>(
            sin(new_z.y * 1.5 + time),
            cos(new_z.x * 1.2 - time)
        );

        z = new_z;
        trap = min(trap, vec3<f32>(abs(z.x), abs(z.y), r2));
        accum = accum + exp(-r2 * 0.35);
        smooth_acc = smooth_acc + exp(-abs(radius - 1.2));

        if (escape == 0.0 && r2 > bailout * bailout) {
            escape = f32(i);
        }
    }

    let intensity = accum / f32(iterations);
    let softness = smooth_acc / f32(iterations);
    let trap_color = trap / vec3<f32>(f32(iterations));

    var color = vec3<f32>(
        pow(intensity, 1.2),
        pow(intensity, 0.8) * 0.8 + trap_color.y * 0.6,
        pow(intensity, 0.6) * 0.6 + trap_color.x * 0.5
    );
    color = color + softness * vec3<f32>(0.08, 0.22, 0.35);

    var flare = 1.0;
    if (escape != 0.0) {
        flare = clamp(escape / f32(iterations), 0.0, 1.0);
    }

    color = mix(vec3<f32>(0.02, 0.0, 0.05), color, flare);
    let density = 0.6 + 0.4 * sin(dr * 0.05 - time * 2.0);
    color = color * density;

    let vignette = smoothstep(1.5, 0.25, length(uv));
    color = color * vignette;
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

fn cloud_field(p_in: vec2<f32>, time: f32) -> f32 {
    var p = p_in;
    var freq = 1.0;
    var amp = 0.55;
    var density = 0.0;

    for (var i = 0; i < 6; i = i + 1) {
        let fi = f32(i);
        let warp = vec2<f32>(
            sin(dot(p, vec2<f32>(0.8, 1.3)) + time * (0.6 + fi * 0.14)),
            cos(dot(p, vec2<f32>(-1.5, 0.9)) - time * (0.5 + fi * 0.17))
        ) * 0.35;

        let sample = p * freq + warp + vec2<f32>(time * 0.12, -time * 0.08 + fi * 0.5);
        let n = noise2(sample);
        density = density + n * amp;

        p = p + warp * 0.25;
        freq = freq * 1.82;
        amp = amp * 0.52;
    }

    return density;
}

// ===== Fractal Clouds =====
fn FractalClouds(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let aspect = U.width / max(U.height, 1.0);
    var uv = uv_in * 2.0 - 1.0;
    uv.x = uv.x * aspect;
    let time = U.time * 0.15;

    let sky = mix(vec3<f32>(0.05, 0.08, 0.12), vec3<f32>(0.28, 0.42, 0.62), clamp(uv.y * 0.4 + 0.55, 0.0, 1.0));

    let sample_pos = uv * 1.6 + vec2<f32>(time * 0.8, time * -0.6);
    let raw_density = cloud_field(sample_pos, time);
    let density = clamp((raw_density - 0.32) * 1.8, 0.0, 1.0);

    let eps = 0.01;
    let grad_x = cloud_field(sample_pos + vec2<f32>(eps, 0.0), time) - cloud_field(sample_pos - vec2<f32>(eps, 0.0), time);
    let grad_y = cloud_field(sample_pos + vec2<f32>(0.0, eps), time) - cloud_field(sample_pos - vec2<f32>(0.0, eps), time);
    let normal = normalize(vec3<f32>(-grad_x, 1.6, -grad_y));
    let sun_dir = normalize(vec3<f32>(0.45, 0.6, -0.35));

    let diffuse = clamp(dot(normal, sun_dir), 0.0, 1.0);
    let back_light = clamp(dot(normal, -sun_dir), 0.0, 1.0);

    let cloud_base = vec3<f32>(0.85, 0.88, 0.92) * (0.6 + 0.5 * diffuse) + vec3<f32>(0.35, 0.38, 0.42) * back_light * 0.35;
    let rim = pow(back_light, 3.5) * 0.4;
    var cloud_color = cloud_base + vec3<f32>(0.45, 0.48, 0.55) * rim;

    let coverage = density * (0.7 + 0.3 * smoothstep(-0.6, 0.9, uv.y));
    let animation = 0.04 * sin(time * 6.0 + uv.x * 18.0) * density;
    cloud_color = cloud_color * (1.0 + animation);

    let combined = mix(sky, cloud_color, clamp(coverage, 0.0, 1.0));
    let vig = 0.7 + 0.3 * smoothstep(1.45, 0.55, length(uv));
    return vec4<f32>(clamp(combined * vig, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== SDF Scene with Soft Shadows =====

// ===== Infinite Mirror Room =====
// ===== Rotozoomer Pro =====
fn rz_tex(p: vec2<f32>) -> vec3<f32> {
    let r = length(p - vec2<f32>(0.5, 0.5));
    let a = atan2(p.y - 0.5, p.x - 0.5);
    let v = 0.6 * sin(8.0 * a) + 0.4 * cos(40.0 * r);
    return 0.5 + 0.5 * cos(vec3<f32>(0.0, 2.0, 4.0) + vec3<f32>(v * 2.6, v * 2.6, v * 2.6));
}

fn RotozoomerPro(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let res = vec2<f32>(U.width, U.height);
    let t = U.time;
    let uv = aspect_uv(uv_in);

    let wobble = 1.15 + 0.45 * sin(t * 0.42);
    let rot = rot2(t * 0.58 + 0.12 * sin(t * 0.21));
    var tex = rot * uv * wobble;
    tex = tex + vec2<f32>(0.35 * sin(t * 0.29), 0.35 * cos(t * 0.26));

    let tile = fract(tex) - vec2<f32>(0.5, 0.5);
    let ring = sin(length(tile) * 24.0 - t * 2.4);
    let stripes = sin(tile.x * 28.0 + t * 1.4) * cos(tile.y * 24.0 - t * 1.6);
    let pattern = stripes + 0.6 * ring;

    var col = 0.5 + 0.5 * cos(vec3<f32>(0.0, 2.0, 4.0) + vec3<f32>(pattern * 2.2, pattern * 1.8, pattern * 1.6));
    col = col + 0.15 * sin(vec3<f32>(tex.xyx * 2.0 + t * 1.1));
    col = col * (0.85 + 0.15 * sin((uv_in.y * res.y) * 0.75 + t * 2.0));
    let vig = 0.65 + 0.35 * smoothstep(1.6, 0.5, length(uv));
    col = col * vig;
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.2)), 1.0);
}

// ===== Möbius Rotozoomer =====

fn FaceComposite(p: vec3<f32>, n: vec3<f32>, f: i32) -> vec3<f32> {
    var nn = max(abs(n) - vec3<f32>(0.2,0.2,0.2), vec3<f32>(0.001,0.001,0.001));
    let sum = nn.x + nn.y + nn.z;
    nn = nn / sum;
    let pp = p * 0.1 + vec3<f32>(0.5,0.5,0.5);
    if (f == 1) { return (Glenz(pp.yz).xyz  * nn.x + Glenz(pp.zx).xyz  * nn.y + Glenz(pp.xy).xyz  * nn.z); }
    if (f == 2) { return (RotatingGrid(pp.yz).xyz   * nn.x + RotatingGrid(pp.zx).xyz   * nn.y + RotatingGrid(pp.xy).xyz   * nn.z); }
    if (f == 3) { return (FractalFlame(pp.yz).xyz * nn.x + FractalFlame(pp.zx).xyz * nn.y + FractalFlame(pp.xy).xyz * nn.z); }
    if (f == 4) { return (InterferenceWells(pp.yz).xyz * nn.x + InterferenceWells(pp.zx).xyz * nn.y + InterferenceWells(pp.xy).xyz * nn.z); }
    if (f == 5) { return (FractalClouds(pp.yz).xyz   * nn.x + FractalClouds(pp.zx).xyz   * nn.y + FractalClouds(pp.xy).xyz   * nn.z); }
    if (f == 6) { return (Flow(pp.yz).xyz   * nn.x + Flow(pp.zx).xyz   * nn.y + Flow(pp.xy).xyz   * nn.z); }
    return vec3<f32>(1.0,1.0,1.0);
}
fn GetColor(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    if (dot(n, vec3<f32>(1.0,0.0,0.0)) > 0.0) { return FaceComposite(p, n, 1); } // +X
    if (dot(n, vec3<f32>(1.0,0.0,0.0)) < 0.0) { return FaceComposite(p, n, 2); } // -X
    if (dot(n, vec3<f32>(0.0,0.0,1.0)) > 0.0) { return FaceComposite(p, n, 3); } // +Z
    if (dot(n, vec3<f32>(0.0,0.0,1.0)) < 0.0) { return FaceComposite(p, n, 4); } // -Z
    if (dot(n, vec3<f32>(0.0,1.0,0.0)) > 0.0) { return FaceComposite(p, n, 5); } // +Y
    return FaceComposite(p, n, 6); // -Y → Flow
}

// ===== JellyCube Renderer =====
fn RenderJellyCube(uv01_in: vec2<f32>, fragCoord: vec2<f32>, res: vec2<f32>, time: f32) -> vec4<f32> {
    var uv = fragCoord / res.y;      // normiert auf Höhe
    let ratio = res / res.y;

    uv.x = uv.x + 0.2 * sin(4.0 * uv.y + time);

    var ro = vec3<f32>(0.0, 0.0, -20.0);
    var rd = normalize(vec3<f32>(uv - ratio * 0.5, 1.0));

    let rx = rot2(time);
    let ry = rot2(time * 2.0);

    var yz = rx * ro.yz; ro = vec3<f32>(ro.x, yz.x, yz.y);
    var xz = ry * ro.xz; ro = vec3<f32>(xz.x, ro.y, xz.y);

    yz = rx * rd.yz; rd = vec3<f32>(rd.x, yz.x, yz.y);
    xz = ry * rd.xz; rd = vec3<f32>(xz.x, rd.y, xz.y);

    let sp = RayMarch(ro, rd);
    let sn = NormalBound(sp);

    var color = vec3<f32>(0.0, 0.0, 0.0);
    let d = SceneBound(sp);
    if (abs(d) < 0.01) {
        color = GetColor(sp, sn);
        // kleines Lighting
        let lp = ro - vec3<f32>(5.0,5.0,5.0);
        var ld = lp - sp;
        let lDist = max(length(ld), 0.001);
        ld = ld / lDist;
        let diff = min(0.3, max(dot(sn, ld), 0.0));
        let spec = pow(max(dot(reflect(-ld, sn), -rd), 0.0), 24.0);
        color = color * (1.0 + diff) + vec3<f32>(1.0,1.0,1.0) * spec * 0.4;
    }
    return vec4<f32>(color, 1.0);
}

// ===== Rotating Grid (scan-inspired infinite zoom) =====
fn RotatingGrid(uv_in: vec2<f32>) -> vec4<f32> {
    if (uv_in.x < BORDER || uv_in.y < BORDER || uv_in.x > 1.0 - BORDER || uv_in.y > 1.0 - BORDER) {
        return BORDERCOLOR;
    }

    let res = vec2<f32>(U.width, U.height);
    var fragCoord = vec2<f32>(uv_in.x * U.width, (1.0 - uv_in.y) * U.height);
    var color = vec3<f32>(0.0, 0.0, 0.0);

    var uv = fragCoord / res - vec2<f32>(0.5, 0.5);
    uv.y = uv.y * (res.y / max(res.x, 1.0));

    var y = -U.time * 0.25;
    for (var i = 0.0; i < 8.0; i = i + 2.0) {
        let t = fract(y + i / 8.0) * 8.0;
        let depth = t * t;
        let shift = vec2<f32>(sin(-y * 1.57), cos(y * 1.57)) / max(depth, 0.001);
        let grid = floor(fract((uv + shift) * depth) * 2.0);
        let d = grid.x + grid.y - 1.0;
        color = max(color, vec3<f32>(d * d / max(t, 0.0001) - 0.1));
    }

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== Komplexe Farbtentakel =====
fn ComplexCascade(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    let fragCoord = vec2<f32>(uv_in.x * res.x, (1.0 - uv_in.y) * res.y);

    let aspect = res.x / max(res.y, 1.0);
    var uv = fragCoord / res.y;
    uv = uv - vec2<f32>(aspect * 0.5, 0.5);

    let warped = complex_iter(uv, U.time);
    var angle = atan2(warped.y, warped.x);
    if (angle < 0.0) {
        angle = angle + 2.0 * PI;
    }

    let mag = max(length(warped), 1e-6);
    let log_len = log(mag);
    let band_val = abs(log_len) * 100.0;
    let band_idx = f32(i32(floor(band_val)) % 100);

    var value = 1.0 - band_idx / 200.0;
    if (log_len <= 0.0) {
        value = 0.5 + band_idx / 200.0;
    }
    value = clamp(value, 0.0, 1.0);

    let hue = angle / (2.0 * PI);
    let col = hsv2rgb(vec3<f32>(fract1(hue), 1.0, value));
    return vec4<f32>(col, 1.0);
}

// ===== Old School Rasterbars =====
fn OldSchoolRasterbars(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    let fragCoord = vec2<f32>(uv_in.x * res.x, (1.0 - uv_in.y) * res.y);
    var uv = fragCoord / res;
    uv = uv - vec2<f32>(0.5, 0.5);
    uv.x = uv.x * (res.x / max(res.y, 1.0));

    var rgb = vec3<f32>(0.0);
    let frequency = 1.5;
    let amplitude = 0.3;
    let speed = 2.0;
    let rh = 0.07;

    var sr = sin(U.time * speed + uv.x * frequency) * amplitude;
    var sg = sin((U.time + 0.2) * speed + uv.x * frequency) * amplitude;
    var sb = sin((U.time + 0.4) * speed + uv.x * frequency) * amplitude;

    sr = sr + cos((U.time + sin(uv.x)) * 0.4) * 0.2;
    sg = sg + cos((U.time + sin(uv.x)) * 0.5) * 0.2;
    sb = sb + cos((U.time + sin(uv.x)) * 0.6) * 0.2;

    rgb.r = rgb.r + smoothstep(sr + rh, sr, uv.y) - smoothstep(sr, sr - rh, uv.y);
    rgb.g = rgb.g + smoothstep(sg + rh, sg, uv.y) - smoothstep(sg, sg - rh, uv.y);
    rgb.b = rgb.b + smoothstep(sb + rh, sb, uv.y) - smoothstep(sb, sb - rh, uv.y);

    let t = U.time;
    for (var i: i32 = 0; i < 64; i = i + 1) {
        let fi = f32(i);
        let cx = cos(t * cos(clamp(t * 1.139, 1.22, 1.8)) * 2.0 + fi * 0.08) * 0.8;
        let cy = sin(t * sin(clamp(t * 0.01, 0.8, 0.9)) * 1.2 + fi * 0.06) * 0.4;
        rgb = rgb + circle(vec2<f32>(cx, cy), uv, 0.03) * 0.97;
    }

    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== Interference Wells =====
fn InterferenceWells(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    var uv = vec2<f32>(uv_in.x * res.x, uv_in.y * res.y);
    let min_dim = min(res.x, res.y);
    uv = uv / min_dim;
    uv = uv - vec2<f32>(0.5 * (res.x / min_dim), 0.5 * (res.y / min_dim));
    uv = uv * 2.0;

    let t = U.time * 0.25;
    let center1 = vec2<f32>(sin(t) / 2.5 + sin(2.0 * t) / 2.5, cos(t) / 2.5 + sin(2.0 * t) / 2.5);
    let center2 = vec2<f32>(cos(t) / 2.5 + cos(2.0 * t) / 2.5, sin(t) / 2.5 + cos(2.0 * t) / 2.5);

    let red_coeff = 0.995;
    let green_coeff = 1.0;
    let blue_coeff = 1.005;

    let r1 = length(uv - center1);
    let r2 = length(uv - center2);

    var r = -sin(sqrt(r1) * 100.0 * red_coeff) + 0.5 - r1 * 0.5 * red_coeff;
    r = r + (-sin(sqrt(r2) * 100.0 * red_coeff) + 0.5 - r2 * 0.5 * red_coeff);
    r = r * 0.25;

    var g = -sin(sqrt(r1) * 100.0 * green_coeff) + 0.5 - r1 * 0.5 * green_coeff;
    g = g + (-sin(sqrt(r2) * 100.0 * green_coeff) + 0.5 - r2 * 0.5 * green_coeff);
    g = g * 0.25;

    var b = -sin(sqrt(r1) * 100.0 * blue_coeff) + 0.5 - r1 * 0.5 * blue_coeff;
    b = b + (-sin(sqrt(r2) * 100.0 * blue_coeff) + 0.5 - r2 * 0.5 * blue_coeff);
    b = b * 0.25;

    let final_color = vec3<f32>(r + g, 0.5 * r + g + 0.5 * b, g + b);
    return vec4<f32>(clamp(final_color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== Raymarched Sine Sphere =====
const RM_MAX_STEPS : u32 = 128u;
const RM_MAX_DIST : f32 = 1000.0;
const RM_MIN_HIT_DIST : f32 = 0.001;

fn rm_min_dist(point: vec3<f32>) -> f32 {
    let sphere_disp = sin(1.5 * point.x + U.time) * sin(1.5 * point.y + U.time) * sin(2.5 * point.z + U.time) * 0.5;
    let plane_disp = sin(length(point.xy) - U.time) * 0.5;

    let sphere_center = vec3<f32>(0.0, 0.0, 0.0);
    let sphere_radius = 6.0;
    let sphere_dist = length(point - sphere_center) - sphere_radius + sphere_disp;
    let plane_dist = point.z + 4.0 + plane_disp;
    return min(sphere_dist, plane_dist);
}

fn rm_raymarch(ray_origin: vec3<f32>, ray_dir: vec3<f32>) -> vec3<f32> {
    let dir = normalize(ray_dir);
    var total_dist = 0.0;
    var current_pos = ray_origin;

    for (var i: u32 = 0u; i < RM_MAX_STEPS; i = i + 1u) {
        current_pos = ray_origin + dir * total_dist;
        let dist = rm_min_dist(current_pos);

        if (dist < RM_MIN_HIT_DIST) {
            return normalize(current_pos) * 0.6 + vec3<f32>(0.6, 0.6, 0.6);
        }

        if (total_dist > RM_MAX_DIST) {
            break;
        }

        total_dist = total_dist + dist;
    }

    return normalize(current_pos) * 0.5 + vec3<f32>(0.5, 0.5, 0.5);
}

fn RaymarchedSineSphere(uv_in: vec2<f32>, fragCoord: vec2<f32>, res: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }

    let R = res;
    var uv = (2.0 * fragCoord - R) / min(R.x, R.y);

    let a = U.time;
    let b = PI / 2.0;
    let c = a + PI;
    let d = 10.0;

    let camera = vec3<f32>(d * sin(a), d * cos(a), 0.0);
    let camera_look = vec3<f32>(
        uv.x * sin(a + b) + sin(c),
        uv.x * cos(a + b) + cos(c),
        uv.y
    );

    let col = rm_raymarch(camera, camera_look);
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== Rotating Cosine Grid =====
fn RotatingCosineGrid(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    let fragCoord = vec2<f32>(uv_in.x * res.x, (1.0 - uv_in.y) * res.y);
    let angle = radians(U.time * 90.0);
    let rot = mat2x2<f32>(
        vec2<f32>(cos(angle), -sin(angle)),
        vec2<f32>(sin(angle),  cos(angle))
    );

    let rotated = rot * fragCoord;
    let scale = 20.0;
    let col = vec3<f32>(
        0.5 + 0.5 * sin(rotated.x / scale),
        0.5 + 0.5 * sin(rotated.y / scale),
        0.5 + 0.5 * sin((rotated.x + rotated.y) / scale)
    );
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== Neon Scanlines =====
fn NeonScanlines(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    let fragCoord = vec2<f32>(uv_in.x * res.x, (1.0 - uv_in.y) * res.y);
    let y = fragCoord.y / max(res.x, 1.0) + 1.0;
    let t = U.time * 3.0;

    var color = vec3<f32>(0.0, 0.0, cos(y * 6.0 - 5.0));
    var hit = false;

    for (var i: i32 = 0; i < 18; i = i + 1) {
        let k = f32(i);
        let s = sin(t + k / 3.4) / 6.0 + 1.25;
        if (y > s && y < s + 0.05) {
            let beam = vec3<f32>(s, sin(y + t * 0.3), k / 16.0);
            let envelope = (y - s) * sin((y - s) * 20.0 * PI) * 38.0;
            color = beam * envelope;
            hit = true;
        }
    }

    if (!hit) {
        color = vec3<f32>(0.0, 0.0, max(color.z, 0.0));
    }

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== Perspective Checkerboard =====
fn PerspectiveCheckerboard(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    let fragCoord = vec2<f32>(uv_in.x * res.x, (1.0 - uv_in.y) * res.y);
    var uv = (fragCoord / res) - vec2<f32>(0.5, 0.5);
    uv = uv * 1.5;
    let z = 2.0 / max(abs(uv.y), 1e-4);
    let x = uv.x * z;

    if (z >= 8.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    let pos = vec2<f32>(3.0 * sin(U.time) + x, 3.0 * U.time + z);
    let tex = step(vec2<f32>(0.5, 0.5), fract(pos));
    let c = f32(tex.x != tex.y) * smoothstep(9.0, 0.0, z);
    return vec4<f32>(vec3<f32>(c, c, c), 1.0);
}

// ===== Szene-Dispatcher =====
fn scene_color(scene_idx: u32, uv01: vec2<f32>, fragCoord: vec2<f32>, res: vec2<f32>) -> vec4<f32> {
    if (scene_idx == 0u)  { return RotatingGrid(uv01); }
    if (scene_idx == 1u)  { return FractalClouds(uv01); }
    if (scene_idx == 2u)  { return Plasma(uv01); }
    if (scene_idx == 3u)  { return Flow(uv01); }
    if (scene_idx == 4u)  { return FractalFlame(uv01); }
    if (scene_idx == 5u)  { return JuliaMorph(uv01); }
    if (scene_idx == 6u)  { return RotozoomerPro(uv01); }
    if (scene_idx == 7u)  { return Glenz(uv01); }
    if (scene_idx == 8u)  { return ComplexCascade(uv01); }
    if (scene_idx == 9u)  { return OldSchoolRasterbars(uv01); }
    if (scene_idx == 10u) { return InterferenceWells(uv01); }
    if (scene_idx == 11u) { return RaymarchedSineSphere(uv01, fragCoord, res); }
    if (scene_idx == 12u) { return RotatingCosineGrid(uv01); }
    if (scene_idx == 13u) { return NeonScanlines(uv01); }
    if (scene_idx == 14u) { return PerspectiveCheckerboard(uv01); }
    if (scene_idx == 15u) { return RenderJellyCube(uv01, fragCoord, res, U.time); }
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}

// ===== CRT (Timothy Lottes single-pass adaptation) =====
struct CRTContext {
    res: vec2<f32>,
    scene_idx: u32,
    next_idx: u32,
    tr_alpha: f32,
};

const CRT_EMU_SCALE   : f32 = 6.0;
const CRT_HARD_SCAN   : f32 = -8.0;
const CRT_HARD_PIX    : f32 = -3.0;
const CRT_WARP_FACTOR : vec2<f32> = vec2<f32>(1.0 / 32.0, 1.0 / 24.0);
const CRT_MASK_DARK   : f32 = 0.5;
const CRT_MASK_LIGHT  : f32 = 1.5;

fn to_linear_1(c: f32) -> f32 {
    if (c <= 0.04045) { return c / 12.92; }
    return pow((c + 0.055) / 1.055, 2.4);
}
fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(to_linear_1(c.x), to_linear_1(c.y), to_linear_1(c.z));
}
fn to_srgb_1(c: f32) -> f32 {
    if (c < 0.0031308) { return c * 12.92; }
    return 1.055 * pow(c, 0.41666) - 0.055;
}
fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(to_srgb_1(c.x), to_srgb_1(c.y), to_srgb_1(c.z));
}

fn crt_sample_scene(ctx: CRTContext, uv01: vec2<f32>) -> vec3<f32> {
    let fragCoord = vec2<f32>(uv01.x * ctx.res.x, (1.0 - uv01.y) * ctx.res.y);
    let col_a = scene_color(ctx.scene_idx, uv01, fragCoord, ctx.res);
    let col_b = scene_color(ctx.next_idx,  uv01, fragCoord, ctx.res);
    return mix(col_a.rgb, col_b.rgb, ctx.tr_alpha);
}

fn crt_fetch(ctx: CRTContext, pos: vec2<f32>, off: vec2<f32>, emu_res: vec2<f32>) -> vec3<f32> {
    let sample_pos = floor(pos * emu_res + off) / emu_res;
    if (max(abs(sample_pos.x - 0.5), abs(sample_pos.y - 0.5)) > 0.5) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    return to_linear(crt_sample_scene(ctx, sample_pos));
}

fn crt_dist(pos: vec2<f32>, emu_res: vec2<f32>) -> vec2<f32> {
    let scaled = pos * emu_res;
    return -((scaled - floor(scaled)) - vec2<f32>(0.5, 0.5));
}

fn crt_gaus(pos: f32, scale: f32) -> f32 {
    return exp2(scale * pos * pos);
}

fn crt_horz3(ctx: CRTContext, pos: vec2<f32>, off: f32, emu_res: vec2<f32>) -> vec3<f32> {
    let b = crt_fetch(ctx, pos, vec2<f32>(-1.0, off), emu_res);
    let c = crt_fetch(ctx, pos, vec2<f32>( 0.0, off), emu_res);
    let d = crt_fetch(ctx, pos, vec2<f32>( 1.0, off), emu_res);
    let dst = crt_dist(pos, emu_res).x;
    let scale = CRT_HARD_PIX;
    let wb = crt_gaus(dst - 1.0, scale);
    let wc = crt_gaus(dst + 0.0, scale);
    let wd = crt_gaus(dst + 1.0, scale);
    return (b * wb + c * wc + d * wd) / (wb + wc + wd);
}

fn crt_horz5(ctx: CRTContext, pos: vec2<f32>, off: f32, emu_res: vec2<f32>) -> vec3<f32> {
    let a = crt_fetch(ctx, pos, vec2<f32>(-2.0, off), emu_res);
    let b = crt_fetch(ctx, pos, vec2<f32>(-1.0, off), emu_res);
    let c = crt_fetch(ctx, pos, vec2<f32>( 0.0, off), emu_res);
    let d = crt_fetch(ctx, pos, vec2<f32>( 1.0, off), emu_res);
    let e = crt_fetch(ctx, pos, vec2<f32>( 2.0, off), emu_res);
    let dst = crt_dist(pos, emu_res).x;
    let scale = CRT_HARD_PIX;
    let wa = crt_gaus(dst - 2.0, scale);
    let wb = crt_gaus(dst - 1.0, scale);
    let wc = crt_gaus(dst + 0.0, scale);
    let wd = crt_gaus(dst + 1.0, scale);
    let we = crt_gaus(dst + 2.0, scale);
    return (a * wa + b * wb + c * wc + d * wd + e * we) / (wa + wb + wc + wd + we);
}

fn crt_scan(pos: vec2<f32>, off: f32, emu_res: vec2<f32>) -> f32 {
    let dst = crt_dist(pos, emu_res).y;
    return crt_gaus(dst + off, CRT_HARD_SCAN);
}

fn crt_tri(ctx: CRTContext, pos: vec2<f32>, emu_res: vec2<f32>) -> vec3<f32> {
    let a = crt_horz3(ctx, pos, -1.0, emu_res);
    let b = crt_horz5(ctx, pos,  0.0, emu_res);
    let c = crt_horz3(ctx, pos,  1.0, emu_res);
    let wa = crt_scan(pos, -1.0, emu_res);
    let wb = crt_scan(pos,  0.0, emu_res);
    let wc = crt_scan(pos,  1.0, emu_res);
    return a * wa + b * wb + c * wc;
}

fn crt_warp(pos: vec2<f32>) -> vec2<f32> {
    var p = pos * 2.0 - 1.0;
    p = p * vec2<f32>(1.0 + (p.y * p.y) * CRT_WARP_FACTOR.x, 1.0 + (p.x * p.x) * CRT_WARP_FACTOR.y);
    return p * 0.5 + 0.5;
}

fn crt_mask(pos: vec2<f32>) -> vec3<f32> {
    var p = pos;
    p.x = p.x + p.y * 3.0;
    var mask = vec3<f32>(CRT_MASK_DARK, CRT_MASK_DARK, CRT_MASK_DARK);
    let frac_x = fract(p.x / 6.0);
    if (frac_x < 0.333) { mask.r = CRT_MASK_LIGHT; }
    else if (frac_x < 0.666) { mask.g = CRT_MASK_LIGHT; }
    else { mask.b = CRT_MASK_LIGHT; }
    return mask;
}

fn crt_apply(ctx: CRTContext, fragCoord: vec2<f32>) -> vec3<f32> {
    let emu_res = ctx.res / CRT_EMU_SCALE;
    let pos = crt_warp(fragCoord / ctx.res);
    let color_linear = crt_tri(ctx, pos, emu_res);
    let masked = color_linear * crt_mask(fragCoord);
    return to_srgb(max(masked, vec3<f32>(0.0, 0.0, 0.0)));
}

// ===== Fragment =====
@fragment
fn frag_main(v: VertexOutput) -> @location(0) vec4<f32> {
    // Pixelcoords in Bevy (Y-Flip)
    let fragCoord = vec2<f32>(v.uv.x * U.width, (1.0 - v.uv.y) * U.height);
    let res       = vec2<f32>(U.width, U.height);
    let uv01      = v.uv;

    // Sequencer
    let t         = U.time;
    let slot_f    = t / SCENE_DURATION;
    let slot_i    = u32(floor(slot_f));
    let scene_idx = slot_i % SCENE_COUNT;
    let next_idx  = (scene_idx + 1u) % SCENE_COUNT;

    let local_t   = fract1(slot_f); // 0..1 innerhalb Szene
    let tr_start  = 1.0 - (TRANSITION_DURATION / SCENE_DURATION);
    let tr_phase  = clamp((local_t - tr_start) / max(TRANSITION_DURATION / SCENE_DURATION, 1e-5), 0.0, 1.0);
    let tr_alpha  = smoothstep(0.0, 1.0, tr_phase);

    let col_a = scene_color(scene_idx, uv01, fragCoord, res);
    let col_b = scene_color(next_idx,  uv01, fragCoord, res);
    var color = mix(col_a.rgb, col_b.rgb, tr_alpha);

    if (U.crt_enabled == 1u) {
        let ctx = CRTContext(res, scene_idx, next_idx, tr_alpha);
        color = crt_apply(ctx, fragCoord);
    }

    return vec4<f32>(color, 1.0);
}

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// ===== Uniforms =====
struct Params {
    time: f32,
    width: f32,
    height: f32,
    mouse: vec4<f32>,
    frame: u32,
};
@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> U : Params;

// ===== Sequencing =====
const SCENE_COUNT         : u32 = 18u;  // 17 Einzelszenen + 1 JellyCube
const SCENE_DURATION      : f32 = 6.0;  // Sekunden pro Szene
const TRANSITION_DURATION : f32 = 1.0;  // Crossfade-Dauer

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

// ===== Ring (leicht verändert) =====
const IN_RADIUS : f32 = 0.25;
const OUT_RADIUS : f32 = 0.70;
const NUM_FACES : i32 = 4;
const XSCROLL_SPEED : f32 = -0.9;
var<private> g_aaSize : f32 = 0.0;

fn slice_fn(x0: f32, x1: f32, uv: vec2<f32>) -> vec4<f32> {
    let u = (uv.x - x0) / (x1 - x0);
    let w = (x1 - x0);
    var col = mix(vec3<f32>(0.50, 0.90, 0.95), vec3<f32>(0.95, 0.60, 0.10), u);
    let denom = sqrt(2.0 * IN_RADIUS * IN_RADIUS * (1.0 - cos(PI * 2.0 / f32(NUM_FACES))));
    col = col * (w / denom);
    col = col * (smoothstep(0.05, 0.10, u) * smoothstep(0.95, 0.90, u) + 0.5);

    var uv2 = uv; uv2.y = uv2.y + U.time * XSCROLL_SPEED;
    // dezenter Wellenfaktor + leichte Glitzer-Spikes
    let wave = (-1.0 + 2.0 * smoothstep(-0.03, 0.03, sin(u * PI * 4.0) * cos(uv2.y * 16.0))) * (1.0/16.0) + 0.7;
    let sparkle = 1.0 + 0.06 * smoothstep(0.98, 1.0, sin(uv2.y * 8.0 + U.time * 3.0));
    col = col * wave * sparkle;

    let clip = (1.0 - smoothstep(0.5 - g_aaSize / w, 0.5 + g_aaSize / w, abs(u - 0.5))) * select(0.0, 1.0, x0 <= x1);
    return vec4<f32>(col, clip);
}
fn Ring(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    g_aaSize = 2.0 / U.height;
    var p = uv_in * 2.0 - 1.0;
    var uvr = vec2<f32>(length(p), atan2(p.y, p.x) + PI);
    uvr.x = uvr.x - OUT_RADIUS;
    var col = vec3<f32>(0.05, 0.05, 0.05);
    let angle = uvr.y + 2.0 * U.time + sin(uvr.y) * sin(U.time) * PI;

    for (var i: i32 = 0; i < NUM_FACES; i = i + 1) {
        // mini-Pulse pro Face
        let pulse = 0.02 * sin(U.time * 1.6 + f32(i));
        let x0 = (IN_RADIUS + pulse) * sin(angle + 2.0 * PI * (f32(i) / f32(NUM_FACES)));
        let x1 = (IN_RADIUS + pulse) * sin(angle + 2.0 * PI * (f32(i + 1) / f32(NUM_FACES)));
        let face = slice_fn(x0, x1, uvr);
        col = mix(col, face.rgb, face.a);
    }
    // leichte radiale Abdunklung
    let rad = smoothstep(0.0, 0.9, uvr.x + OUT_RADIUS);
    col = col * (0.9 + 0.1 * rad);
    return vec4<f32>(col, 1.0);
}

// ===== Plasma / Twirl (leicht verändert) =====
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
fn tw(uv: vec2<f32>) -> vec4<f32> {
    let j = sin(uv.y * PI + U.time * 5.0);
    let i = sin(uv.x * 15.0 - uv.y * 2.0 * PI + U.time * 3.0);
    let n = -clamp(i, -0.2, 0.0);
    // dezente Chroma-Variation
    let chroma = vec3<f32>(0.22, 0.5, 1.0) + 0.05 * vec3<f32>(sin(U.time*0.7), sin(U.time*0.9+1.0), sin(U.time*1.1+2.0));
    return 3.5 * vec4<f32>(chroma, 1.0) * n;
}
fn Twirl(p_in: vec2<f32>) -> vec4<f32> {
    if (p_in.x < BORDER || p_in.y < BORDER || p_in.x > 1.0 - BORDER || p_in.y > 1.0 - BORDER) { return BORDERCOLOR; }
    var p = -1.0 + p_in * 2.0;
    // sanfter Radius-Puls
    let rp = 1.0 + 0.03 * sin(U.time * 1.2);
    p = p * rp;
    let r = sqrt(dot(p, p));
    let a = atan2(
        p.y * (0.3 + 0.1 * cos(U.time * 2.0 + p.y)),
        p.x * (0.3 + 0.1 * sin(U.time + p.x))
    ) + U.time;
    let uv = vec2<f32>(U.time + 1.0 / (r + 0.01), 4.0 * a / PI);
    return mix(vec4<f32>(0.0,0.0,0.0,0.0), tw(uv) * r * r * 2.0, 1.0);
}

// ===== Room (leicht verändert) =====
fn sdBoxXY(p: vec3<f32>, b: vec3<f32>) -> f32 {
  let d = abs(p.xy) - b.xy;
  return min(max(d.x,d.y),0.0) + length(max(d,vec2<f32>(0.0,0.0)));
}
fn udRoundBox(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
  return length(max(abs(p)-b,vec3<f32>(0.0,0.0,0.0)))-r;
}
fn map_room(p_in: vec3<f32>) -> f32 {
    var p = p_in;
    var k = 1.0 * 0.5 * 2.0;
    var q = (fract3((p - vec3<f32>(0.25, 0.0, 0.25))/ k) - vec3<f32>(0.5,0.5,0.5)) * k;
    var s = vec3<f32>(q.x, p.y, q.z);
    var d = udRoundBox(s, vec3<f32>(0.1, 1.0, 0.1), 0.05);
    k = 0.5;
    q = (fract3(p / k) - vec3<f32>(0.5,0.5,0.5)) * k;
    s = vec3<f32>(q.x, abs(p.y) - 1.5, q.z);
    let g = udRoundBox(s, vec3<f32>(0.17, 0.5, 0.17), 0.2);
    let sq = sqrt(0.5);
    var u = p;
    let r2 = mat2x2<f32>(vec2<f32>(sq, sq), vec2<f32>(-sq, sq));
    let xz = r2 * u.xz;
    u = vec3<f32>(xz.x, u.y, xz.y);
    d = max(d, -sdBoxXY(u, vec3<f32>(0.8, 1.0, 0.8)));
    return smin(d, g, 16.0);
}
fn normal_room(p: vec3<f32>) -> vec3<f32> {
	let o = vec3<f32>(0.001, 0.0, 0.0);
    return normalize(vec3<f32>(
        map_room(p+o.xyy) - map_room(p-o.xyy),
        map_room(p+o.yxy) - map_room(p-o.yxy),
        map_room(p+o.yyx) - map_room(p-o.yyx)
    ));
}
fn trace_room(o: vec3<f32>, r: vec3<f32>) -> f32 {
    var t = 0.0;
    for (var i: i32 = 0; i < 32; i = i + 1) {
        t = t + map_room(o + r * t);
    }
    return t;
}
fn Room(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    var uv = uv_in * 2.0 - 1.0;
    uv.x = uv.x * (U.width / U.height);
    let gt = U.time / 5.0;
    // leichte Kamera-Roll
    var r = normalize(vec3<f32>(uv, 1.7 - dot(uv, uv) * 0.1));
    let rx = rot2(sin(gt * PI * 2.0) * PI / 8.0 + 0.05 * sin(U.time*0.7));
    let xyr = rx * r.xy; r = vec3<f32>(xyr.x, xyr.y, r.z);
    var xzr = rot2(gt * PI * 2.0) * r.xz; r = vec3<f32>(xzr.x, r.y, xzr.y);
    xzr = rot2(PI * -0.25) * r.xz; r = vec3<f32>(xzr.x, r.y, xzr.y);
    var o = vec3<f32>(0.0, 0.0, gt * 5.0 * sqrt(2.0) * 2.0);
    var oxz = rot2(PI * -0.25) * o.xz; o = vec3<f32>(oxz.x, o.y, oxz.y);

    let t = trace_room(o, r);
    let w = o + r * t;
    let sn = normal_room(w);
    let fd = map_room(w);
    // etwas anderer Base-Ton
    let col = vec3<f32>(0.50, 0.84, 0.95) * 0.52;
    // anderes Licht
    let ldir = normalize(vec3<f32>(-0.8, -0.4, 1.2));
    let fog = 1.0 / (1.0 + t * t * 0.09 + fd * 120.0);
    let refv = max(dot(r, reflect(-ldir, sn)), 0.0);
    let grn = pow(abs(sn.y), 3.0);
    var cl = vec3<f32>(grn, grn, grn);
    cl = cl + mix(col * vec3<f32>(1.5,1.5,1.5), vec3<f32>(0.23,0.23,0.23), grn) * pow(refv, 16.0);
    cl = mix(col, cl, fog);
    return vec4<f32>(cl, 1.0);
}

// ===== Szene / Raymarch des großen Würfels =====
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

// ===== Signed Distance Helper Types =====
struct SdfSample {
    dist: f32,
    color: vec3<f32>,
}

struct TraceResult {
    t: f32,
    color: vec3<f32>,
    hit: bool,
}

fn sdf_sphere(p: vec3<f32>, radius: f32) -> f32 {
    return length(p) - radius;
}

fn sdf_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0, 0.0, 0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

fn sdf_torus(p: vec3<f32>, t: vec2<f32>) -> f32 {
    let q = vec2<f32>(length(p.xz) - t.x, p.y);
    return length(q) - t.y;
}

fn sdf_plane(p: vec3<f32>, normal: vec3<f32>, height: f32) -> f32 {
    return dot(p, normal) + height;
}

fn sdf_union(a: SdfSample, b: SdfSample) -> SdfSample {
    if (a.dist < b.dist) { return a; }
    return b;
}

fn sdf_smooth_union(a: SdfSample, b: SdfSample, k: f32) -> SdfSample {
    let h = clamp(0.5 + 0.5 * (b.dist - a.dist) / k, 0.0, 1.0);
    let dist = mix(b.dist, a.dist, h) - k * h * (1.0 - h);
    let color = mix(b.color, a.color, h);
    return SdfSample(dist, color);
}

fn sdf_scene(p_in: vec3<f32>) -> SdfSample {
    let time = U.time;
    var p = p_in;

    // animated sphere
    let sphere_center = vec3<f32>(0.0, -0.2 + 0.25 * sin(time * 0.8), 0.0);
    let sphere = SdfSample(
        sdf_sphere(p - sphere_center, 0.6),
        vec3<f32>(0.92, 0.45, 0.34)
    );

    // rotating box
    var box_p = p - vec3<f32>(1.05, -0.25, 0.4 * sin(time * 0.7));
    let rot_y = rot2(time * 0.6);
    let bz = rot2(time * 0.9);
    let xz = rot_y * box_p.xz; box_p = vec3<f32>(xz.x, box_p.y, xz.y);
    let yz = bz * box_p.yz; box_p = vec3<f32>(box_p.x, yz.x, yz.y);
    let box = SdfSample(
        sdf_box(box_p, vec3<f32>(0.35, 0.22, 0.35)),
        vec3<f32>(0.32, 0.68, 0.9)
    );

    // torus
    var torus_p = p - vec3<f32>(-1.1, -0.05, 0.45 * cos(time * 0.6));
    let txz = rot2(time * 0.4) * torus_p.xz; torus_p = vec3<f32>(txz.x, torus_p.y, txz.y);
    let torus = SdfSample(
        sdf_torus(torus_p, vec2<f32>(0.6, 0.18)),
        vec3<f32>(0.85, 0.8, 0.42)
    );

    // small orbiting sphere
    let orbit = vec3<f32>(
        sin(time * 1.6) * 0.9,
        -0.05 + 0.25 * cos(time * 1.2),
        cos(time * 1.4) * 0.9
    );
    let small_sphere = SdfSample(
        sdf_sphere(p - orbit, 0.18),
        vec3<f32>(0.9, 0.72, 0.95)
    );

    // ground plane
    let floor_sample = SdfSample(
        sdf_plane(p, vec3<f32>(0.0, 1.0, 0.0), 1.2),
        vec3<f32>(0.18, 0.2, 0.24)
    );

    // blend shapes together
    var res = sdf_smooth_union(sphere, torus, 0.45);
    res = sdf_smooth_union(res, box, 0.32);
    res = sdf_smooth_union(res, small_sphere, 0.28);
    res = sdf_union(res, floor_sample);

    return res;
}

fn sdf_normal(p: vec3<f32>) -> vec3<f32> {
    let eps = 0.001;
    let ex = vec3<f32>(eps, 0.0, 0.0);
    let ey = vec3<f32>(0.0, eps, 0.0);
    let ez = vec3<f32>(0.0, 0.0, eps);
    let dx = sdf_scene(p + ex).dist - sdf_scene(p - ex).dist;
    let dy = sdf_scene(p + ey).dist - sdf_scene(p - ey).dist;
    let dz = sdf_scene(p + ez).dist - sdf_scene(p - ez).dist;
    return normalize(vec3<f32>(dx, dy, dz));
}

fn sdf_ambient_occlusion(p: vec3<f32>, normal: vec3<f32>) -> f32 {
    var occ = 0.0;
    var scale = 1.0;
    for (var i = 0; i < 5; i = i + 1) {
        let step_dist = 0.05 * (f32(i) + 1.0);
        let sample = sdf_scene(p + normal * step_dist).dist;
        occ = occ + (step_dist - sample) * scale;
        scale = scale * 0.5;
    }
    return clamp(1.0 - occ * 0.9, 0.0, 1.0);
}

fn sdf_soft_shadow(ro: vec3<f32>, rd: vec3<f32>, t_min: f32, t_max: f32, softness: f32) -> f32 {
    var res = 1.0;
    var t = t_min;
    for (var i = 0; i < 48; i = i + 1) {
        let h = sdf_scene(ro + rd * t).dist;
        if (h < 0.001) {
            return 0.0;
        }
        res = min(res, softness * h / t);
        t = t + clamp(h, 0.01, 0.2);
        if (t > t_max) {
            break;
        }
    }
    return clamp(res, 0.0, 1.0);
}

fn sdf_trace(ro: vec3<f32>, rd: vec3<f32>) -> TraceResult {
    var t = 0.0;
    let max_t = 35.0;
    for (var i = 0; i < 160; i = i + 1) {
        let pos = ro + rd * t;
        let sample = sdf_scene(pos);
        if (sample.dist < 0.001) {
            return TraceResult(t, sample.color, true);
        }
        t = t + sample.dist;
        if (t > max_t) {
            break;
        }
    }
    return TraceResult(t, vec3<f32>(0.0, 0.0, 0.0), false);
}

// ===== Glenz Vectors =====
fn GlenzVectors(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let res = vec2<f32>(U.width, U.height);
    let t = U.time;
    var uv = uv_in * 2.0 - vec2<f32>(1.0, 1.0);
    uv.x = uv.x * (res.x / max(res.y, 1.0));

    var V = array<vec3<f32>, 4>(
        normalize(vec3<f32>(1.0, 1.0, 1.0)),
        normalize(vec3<f32>(-1.0, -1.0, 1.0)),
        normalize(vec3<f32>(-1.0, 1.0, -1.0)),
        normalize(vec3<f32>(1.0, -1.0, -1.0))
    );
    var F = array<vec3<i32>, 4>(
        vec3<i32>(0, 1, 2),
        vec3<i32>(0, 3, 1),
        vec3<i32>(0, 2, 3),
        vec3<i32>(1, 3, 2)
    );
    var C = array<vec3<f32>, 4>(
        vec3<f32>(1.0, 0.4, 0.8),
        vec3<f32>(0.3, 1.0, 0.9),
        vec3<f32>(1.0, 0.9, 0.3),
        vec3<f32>(0.7, 0.6, 1.0)
    );

    let Rx = mat3x3<f32>(
        vec3<f32>(1.0, 0.0, 0.0),
        vec3<f32>(0.0, cos(t * 0.41), -sin(t * 0.41)),
        vec3<f32>(0.0, sin(t * 0.41), cos(t * 0.41))
    );
    let Ry = mat3x3<f32>(
        vec3<f32>(cos(t * 0.83), 0.0, sin(t * 0.83)),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(-sin(t * 0.83), 0.0, cos(t * 0.83))
    );
    let Rz = mat3x3<f32>(
        vec3<f32>(cos(t * 0.57), -sin(t * 0.57), 0.0),
        vec3<f32>(sin(t * 0.57), cos(t * 0.57), 0.0),
        vec3<f32>(0.0, 0.0, 1.0)
    );
    let R3 = Rz * Ry * Rx;

    for (var i = 0; i < 4; i = i + 1) {
        V[i] = (R3 * V[i]) * 0.9;
        V[i].z = V[i].z + 2.3;
    }

    var P = array<vec2<f32>, 4>(
        V[0].xy / V[0].z,
        V[1].xy / V[1].z,
        V[2].xy / V[2].z,
        V[3].xy / V[3].z
    );

    var col = vec3<f32>(0.0, 0.0, 0.0);
    var alpha = 0.0;

    for (var fi = 0; fi < 4; fi = fi + 1) {
        let id = F[fi];
        let a = P[u32(id.x)];
        let b = P[u32(id.y)];
        let c = P[u32(id.z)];
        let faceN = edgef(a, b, c);
        let back = select(0.0, 1.0, faceN < 0.0);
        let w = bary(a, b, c, uv);
        if (all(w >= vec3<f32>(0.0, 0.0, 0.0))) {
            let z = 1.0 / (w.x * V[u32(id.x)].z + w.y * V[u32(id.y)].z + w.z * V[u32(id.z)].z);
            let e0 = abs(edgef(a, b, uv)) / max(length(b - a), 1e-4);
            let e1 = abs(edgef(b, c, uv)) / max(length(c - b), 1e-4);
            let e2 = abs(edgef(c, a, uv)) / max(length(a - c), 1e-4);
            let min_edge = min(e0, min(e1, e2));
            let edge_soft = clamp(1.0 - smoothstep(0.0, 0.02, min_edge), 0.0, 1.0);
            let aFace = 0.32 * edge_soft * z * mix(1.0, 0.55, back);
            let faceCol = C[fi] * (0.6 + 0.4 * cos(vec3<f32>(0.0, 2.0, 4.0) + vec3<f32>(t, t, t) + vec3<f32>(f32(fi), f32(fi), f32(fi))));
            col = col + faceCol * aFace;
            alpha = alpha + aFace * 0.5;
        }
    }

    let r = length(uv);
    col = col + vec3<f32>(0.08 / (r * 12.0 + 0.2));
    col = col * smoothstep(1.3, 0.2, r);
    col = pow(col, vec3<f32>(0.9, 0.9, 0.9));
    return vec4<f32>(clamp(col, vec3<f32>(0.0, 0.0, 0.0), vec3<f32>(1.0, 1.0, 1.0)), clamp(alpha, 0.0, 1.0));
}

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
    let vignette = smoothstep(1.6, 0.3, length(uv));
    color = color * vignette;
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
    let vignette = smoothstep(1.3, 0.35, length(uv));
    return vec4<f32>(clamp(combined * vignette, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== SDF Scene with Soft Shadows =====
fn SdfSoftScene(uv_in: vec2<f32>, fragCoord: vec2<f32>, res: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let aspect = res.x / max(res.y, 1.0);
    var uv = uv_in * 2.0 - 1.0;
    uv.x = uv.x * aspect;

    let time = U.time * 0.5;
    let cam_pos = vec3<f32>(
        sin(time * 0.7) * 2.2,
        0.4 + 0.3 * sin(time * 0.9),
        3.4 + 0.4 * cos(time * 0.5)
    );
    let look_at = vec3<f32>(0.0, -0.2, 0.0);
    let forward = normalize(look_at - cam_pos);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), forward));
   let up = cross(forward, right);
    let fov = 1.2;
    let ray_dir = normalize(forward + uv.x * right * fov + uv.y * up * fov);

    let trace = sdf_trace(cam_pos, ray_dir);
    if (!trace.hit) {
        let sky_grad = clamp(uv.y * 0.4 + 0.6, 0.0, 1.0);
        let sky = mix(vec3<f32>(0.06, 0.07, 0.1), vec3<f32>(0.38, 0.45, 0.55), sky_grad);
        let stars = hash21(fragCoord / 2.0) * 0.02;
        return vec4<f32>(sky + stars, 1.0);
    }

    let hit_pos = cam_pos + ray_dir * trace.t;
    let normal = sdf_normal(hit_pos);
    let light_dir = normalize(vec3<f32>(-0.6, 0.85, 0.4));
    let light_color = vec3<f32>(1.0, 0.97, 0.92);

    let base_color = trace.color;
    let diffuse = max(dot(normal, light_dir), 0.0);
    let shadow = sdf_soft_shadow(hit_pos + normal * 0.02, light_dir, 0.05, 10.0, 18.0);
    let ao = sdf_ambient_occlusion(hit_pos, normal);

    let view_dir = normalize(cam_pos - hit_pos);
    let half_dir = normalize(light_dir + view_dir);
    let specular = pow(max(dot(normal, half_dir), 0.0), 48.0);

    var color = base_color * (0.3 + 0.7 * diffuse * shadow) * ao;
    color = color + light_color * specular * shadow * 0.6;

    let bounce = 0.15 + 0.1 * max(dot(normal, vec3<f32>(0.0, 1.0, 0.0)), 0.0);
    color = color + vec3<f32>(0.15, 0.18, 0.2) * bounce;

    let rim = pow(max(dot(normal, -view_dir), 0.0), 3.0);
    color = color + vec3<f32>(0.12, 0.14, 0.2) * rim;

    let fog = exp(-trace.t * 0.12);
    color = mix(vec3<f32>(0.04, 0.06, 0.09), color, fog);

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== Infinite Mirror Room =====
fn MirrorBox(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let aspect = U.width / max(U.height, 1.0);
    var uv = uv_in * 2.0 - 1.0;
    uv.x = uv.x * aspect;

    let time = U.time * 0.4;
    let cam = vec3<f32>(0.0, 0.0, time * 3.2);

    let forward = normalize(vec3<f32>(0.0, 0.0, -1.0));
    let right = vec3<f32>(1.0, 0.0, 0.0);
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let ray_dir = normalize(forward + uv.x * right * 1.2 + uv.y * up * 1.0);

    let cell_size = vec3<f32>(2.4, 2.0, 3.0);
    var accum = vec3<f32>(0.0, 0.0, 0.0);
    var throughput = 1.0;

    for (var step = 0; step < 48; step = step + 1) {
        let dist = 0.6 + f32(step) * 0.58;
        let pos = cam + ray_dir * dist;

        let cell_index_f = floor((pos / cell_size) + vec3<f32>(0.5, 0.5, 0.5));
        let cell_index = vec3<f32>(cell_index_f.x, cell_index_f.y, cell_index_f.z);

        let folded = abs(fract3((pos / cell_size) + vec3<f32>(0.5, 0.5, 0.5)) * 2.0 - vec3<f32>(1.0, 1.0, 1.0));
        let edge = clamp(1.0 - min(folded.x, min(folded.y, folded.z)), 0.0, 1.0);

        let palette = vec3<f32>(
            0.4 + 0.3 * sin(dot(cell_index, vec3<f32>(0.9, 1.1, 1.7)) + time * 1.7),
            0.4 + 0.3 * sin(dot(cell_index, vec3<f32>(1.3, 0.8, 1.5)) + time * 1.1 + 1.2),
            0.4 + 0.3 * sin(dot(cell_index, vec3<f32>(1.7, 1.4, 0.9)) + time * 0.9 + 2.4)
        );

        let line_a = abs(dot(folded.xy, vec2<f32>(1.0, -1.0)));
        let line_b = abs(folded.z - 0.5);
        let glow = pow(edge, 6.0) + 0.35 * exp(-line_a * 45.0) + 0.25 * exp(-line_b * 72.0);

        accum = accum + throughput * palette * glow;
        throughput = throughput * 0.84;
    }

    let vignette = smoothstep(1.35, 0.3, length(uv));
    accum = accum * vignette;
    return vec4<f32>(clamp(accum, vec3<f32>(0.0), vec3<f32>(1.3)), 1.0);
}

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
    let ang = t * 0.7 + 0.12 * sin(t * 0.31);
    let zoom = 0.8 + 0.6 * sin(t * 0.37);
    let scale = 1.0 / (1.2 + 0.9 * cos(zoom));
    let M = rot2(ang) * scale;
    var p = M * uv + vec2<f32>(0.5 + 0.15 * sin(t * 0.17), 0.5 + 0.15 * cos(t * 0.11));

    let i = floor(p);
    let f = fract(p);
    let c00 = rz_tex(fract(i + vec2<f32>(0.0, 0.0)));
    let c10 = rz_tex(fract(i + vec2<f32>(1.0, 0.0)));
    let c01 = rz_tex(fract(i + vec2<f32>(0.0, 1.0)));
    let c11 = rz_tex(fract(i + vec2<f32>(1.0, 1.0)));
    var col = mix(mix(c00, c10, f.x), mix(c01, c11, f.x), f.y);
    col = col * (0.9 + 0.1 * sin(uv_in.y * res.y));
    col = col * smoothstep(1.25, 0.2, length(uv));
    return vec4<f32>(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// ===== Log Polar Spiral Zoom =====
fn lp_pattern(p: vec2<f32>) -> vec3<f32> {
    let stripes = sin(p.x * 6.28318 * 0.8) * cos(p.y * 4.0);
    return 0.5 + 0.5 * cos(vec3<f32>(0.0, 2.0, 4.0) + vec3<f32>(stripes * 2.0));
}

fn LogPolarSpiralZoom(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let t = U.time;
    let uv = aspect_uv(uv_in);
    let r = length(uv) + 1e-5;
    let a = atan2(uv.y, uv.x);
    var u = log(r) - t * 0.6;
    var v = a + t * 0.9;
    u = fract(u * 0.25);
    v = fract(v / (2.0 * PI));
    var col = lp_pattern(vec2<f32>(u, v));
    col = col + vec3<f32>(0.08 / (r * 10.0 + 0.2));
    col = col * smoothstep(1.2, 0.2, r);
    return vec4<f32>(pow(col, vec3<f32>(0.92, 0.92, 0.92)), 1.0);
}

// ===== Mandelbrot Zoom =====
fn mb_palette(t: f32) -> vec3<f32> {
    return 0.5 + 0.5 * cos(vec3<f32>(0.0, 0.6, 1.0) + vec3<f32>(t * 6.2, t * 6.2, t * 6.2));
}

fn MandelbrotZoom(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let uv = aspect_uv(uv_in);
    let t = U.time;
    let center = vec2<f32>(-0.743643887037151, 0.131825904205330);
    let zoom = exp(-t * 0.35);
    let rot = rot2(0.2 * t);
    var cp = center + (rot * uv) * (1.8 * zoom);
    var z = vec2<f32>(0.0, 0.0);
    let max_it: i32 = 160;
    var i: i32 = 0;
    loop {
        if (!(i < max_it && dot(z, z) < 256.0)) { break; }
        z = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + cp;
        i = i + 1;
    }
    var it = f32(i);
    if (i < max_it) {
        let zn = length(z);
        let nu = log2(log(zn));
        it = it - nu + 4.0;
    }
    let k = it / f32(max_it);
    var col = mb_palette(k) * (0.95 + 0.05 * cos(k * 20.0));
    col = col * smoothstep(1.2, 0.2, length(uv));
    return vec4<f32>(pow(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(0.95, 0.95, 0.95)), 1.0);
}

// ===== Julia Tunnel =====
fn julia_iter(z0: vec2<f32>, c: vec2<f32>, max_it: i32) -> f32 {
    var z = z0;
    var i: i32 = 0;
    loop {
        if (!(i < max_it && dot(z, z) < 256.0)) { break; }
        z = vec2<f32>(z.x * z.x - z.y * z.y, 2.0 * z.x * z.y) + c;
        i = i + 1;
    }
    if (i >= max_it) {
        return f32(max_it);
    }
    let zn = length(z);
    let nu = log2(log(zn));
    return f32(i) - nu + 4.0;
}

fn JuliaTunnel(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let t = U.time;
    let uv = aspect_uv(uv_in);
    let r = max(1e-5, length(uv));
    let a = atan2(uv.y, uv.x);
    var u = log(r) - t * 0.8;
    var v = a + t * 0.6;
    let tile_u = fract(u * 0.5) * 2.0 - 1.0;
    let tile_v = fract(v / (2.0 * PI)) * 2.0 - 1.0;
    let z0 = vec2<f32>(tile_u * 1.6, tile_v * 1.2);
    let c = vec2<f32>(0.285 + 0.15 * cos(t * 0.37), 0.01 + 0.18 * sin(t * 0.23));
    let it = julia_iter(z0, c, 140);
    let k = it / 140.0;
    var col = 0.5 + 0.5 * cos(vec3<f32>(0.0, 2.0, 4.0) + vec3<f32>(k * 7.0 + v * 0.7));
    col = col * (0.85 + 0.15 * cos(k * 10.0));
    col = col + vec3<f32>(0.06 / (r * 10.0 + 0.2));
    col = col * smoothstep(1.25, 0.25, length(uv));
    return vec4<f32>(pow(col, vec3<f32>(0.95, 0.95, 0.95)), 1.0);
}

// ===== Möbius Rotozoomer =====
fn cmul(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(x.x * y.x - x.y * y.y, x.x * y.y + x.y * y.x);
}

fn cdiv(x: vec2<f32>, y: vec2<f32>) -> vec2<f32> {
    let den = dot(y, y);
    let yc = vec2<f32>(y.x, -y.y);
    return cmul(x, yc) / den;
}

fn mobius_tex(p: vec2<f32>) -> vec3<f32> {
    let r = length(p - vec2<f32>(0.5, 0.5));
    let a = atan2(p.y - 0.5, p.x - 0.5);
    let v = 0.7 * sin(10.0 * a) + 0.3 * cos(30.0 * r);
    return 0.5 + 0.5 * cos(vec3<f32>(0.0, 2.2, 4.2) + vec3<f32>(v * 2.4));
}

fn MobiusRotozoomer(uv_in: vec2<f32>) -> vec4<f32> {
    if (!check_border(uv_in)) { return BORDERCOLOR; }
    let t = U.time;
    var z = aspect_uv(uv_in) * (1.0 + 0.25 * sin(t * 0.37));
    let a = vec2<f32>(0.7 * cos(t * 0.3), 0.7 * sin(t * 0.3));
    let b = vec2<f32>(0.3 * cos(t * 0.5 + 1.7), 0.3 * sin(t * 0.5 + 1.7));
    let c = vec2<f32>(0.25 * cos(t * 0.41 + 0.9), 0.25 * sin(t * 0.41 + 0.9));
    let d = vec2<f32>(1.0, 0.0);
    let num = cmul(a, z) + b;
    let den = cmul(c, z) + d;
    var zp = cdiv(num, den);
    zp = cmul(zp, vec2<f32>(cos(t * 0.6), sin(t * 0.6)));
    var p = fract(zp + vec2<f32>(0.5, 0.5));
    let i = floor(p);
    let f = fract(p);
    let c00 = mobius_tex(fract(i + vec2<f32>(0.0, 0.0)));
    let c10 = mobius_tex(fract(i + vec2<f32>(1.0, 0.0)));
    let c01 = mobius_tex(fract(i + vec2<f32>(0.0, 1.0)));
    let c11 = mobius_tex(fract(i + vec2<f32>(1.0, 1.0)));
    var col = mix(mix(c00, c10, f.x), mix(c01, c11, f.x), f.y);
    col = col * (0.92 + 0.08 * sin(uv_in.y * U.height));
    col = col * smoothstep(1.3, 0.25, length(aspect_uv(uv_in)));
    return vec4<f32>(pow(col, vec3<f32>(0.95, 0.95, 0.95)), 1.0);
}

fn FaceComposite(p: vec3<f32>, n: vec3<f32>, f: i32) -> vec3<f32> {
    var nn = max(abs(n) - vec3<f32>(0.2,0.2,0.2), vec3<f32>(0.001,0.001,0.001));
    let sum = nn.x + nn.y + nn.z;
    nn = nn / sum;
    let pp = p * 0.1 + vec3<f32>(0.5,0.5,0.5);
    if (f == 1) { return (Glenz(pp.yz).xyz  * nn.x + Glenz(pp.zx).xyz  * nn.y + Glenz(pp.xy).xyz  * nn.z); }
    if (f == 2) { return (Ring(pp.yz).xyz   * nn.x + Ring(pp.zx).xyz   * nn.y + Ring(pp.xy).xyz   * nn.z); }
    if (f == 3) { return (Plasma(pp.yz).xyz * nn.x + Plasma(pp.zx).xyz * nn.y + Plasma(pp.xy).xyz * nn.z); }
    if (f == 4) { return (Twirl(pp.yz).xyz  * nn.x + Twirl(pp.zx).xyz  * nn.y + Twirl(pp.xy).xyz  * nn.z); }
    if (f == 5) { return (Room(pp.yz).xyz   * nn.x + Room(pp.zx).xyz   * nn.y + Room(pp.xy).xyz   * nn.z); }
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

// ===== Szene-Dispatcher =====
fn scene_color(scene_idx: u32, uv01: vec2<f32>, fragCoord: vec2<f32>, res: vec2<f32>) -> vec4<f32> {
    if (scene_idx == 0u)  { return Glenz(uv01); }
    if (scene_idx == 1u)  { return Ring(uv01); }
    if (scene_idx == 2u)  { return Plasma(uv01); }
    if (scene_idx == 3u)  { return Twirl(uv01); }
    if (scene_idx == 4u)  { return Room(uv01); }
    if (scene_idx == 5u)  { return RotatingGrid(uv01); }
    if (scene_idx == 6u)  { return FractalFlame(uv01); }
    if (scene_idx == 7u)  { return JuliaMorph(uv01); }
    if (scene_idx == 8u)  { return GlenzVectors(uv01); }
    if (scene_idx == 9u)  { return FractalClouds(uv01); }
    if (scene_idx == 10u) { return SdfSoftScene(uv01, fragCoord, res); }
    if (scene_idx == 11u) { return MirrorBox(uv01); }
    if (scene_idx == 12u) { return RotozoomerPro(uv01); }
    if (scene_idx == 13u) { return LogPolarSpiralZoom(uv01); }
    if (scene_idx == 14u) { return MandelbrotZoom(uv01); }
    if (scene_idx == 15u) { return JuliaTunnel(uv01); }
    if (scene_idx == 16u) { return MobiusRotozoomer(uv01); }
    // 17u -> JellyCube
    return RenderJellyCube(uv01, fragCoord, res, U.time);
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

    let ctx = CRTContext(res, scene_idx, next_idx, tr_alpha);
    let color = crt_apply(ctx, fragCoord);

    return vec4<f32>(color, 1.0);
}

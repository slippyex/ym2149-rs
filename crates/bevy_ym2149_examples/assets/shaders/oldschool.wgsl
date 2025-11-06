#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// =============================================================================
// UNIFORMS & CONSTANTS
// =============================================================================

/// Shader parameters passed from the application
struct Params {
    time: f32,              // Current time in seconds
    width: f32,             // Viewport width in pixels
    height: f32,            // Viewport height in pixels
    mouse: vec4<f32>,       // Mouse position/state
    frame: u32,             // Frame counter
    crt_enabled: u32,       // CRT effect toggle (0=off, 1=on)
};
@group(#{MATERIAL_BIND_GROUP}) @binding(0)
var<uniform> U : Params;

/// Scene sequencing configuration
const SCENE_COUNT         : u32 = 17u;  // Total number of scenes (16 + 1 JellyCube)
const SCENE_DURATION      : f32 = 8.0;  // Duration of each scene in seconds
const TRANSITION_DURATION : f32 = 2.0;  // Crossfade duration between scenes

/// Visual constants
const PI : f32 = 3.14159265359;
const BORDER : f32 = 0.02;                                  // Border width (normalized)
const BORDERCOLOR : vec4<f32> = vec4<f32>(0.08, 0.08, 0.08, 1.0);  // Dark gray border

// =============================================================================
// UTILITY FUNCTIONS
// =============================================================================

/// Create a 2D rotation matrix
fn create_rotation_matrix(angle: f32) -> mat2x2<f32> {
    let s = sin(angle);
    let c = cos(angle);
    return mat2x2<f32>(vec2<f32>(c, -s), vec2<f32>(s, c));
}

/// Smooth minimum function for blending distances
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    let res = exp(-k * a) + exp(-k * b);
    return -log(res) / k;
}

/// Fractional part for vec3
fn fract_vec3(v: vec3<f32>) -> vec3<f32> {
    return v - floor(v);
}

/// Fractional part for scalar
fn fract_scalar(x: f32) -> f32 {
    return x - floor(x);
}

/// Deterministic hash function for generating stable noise patterns
fn hash_2d_to_1d(p: vec2<f32>) -> f32 {
    let h = sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453;
    return fract_scalar(h);
}

/// 2D value noise with smooth interpolation
fn value_noise_2d(p_in: vec2<f32>) -> f32 {
    let i = floor(p_in);
    let f = p_in - i;

    // Sample corners of the grid cell
    let a = hash_2d_to_1d(i);
    let b = hash_2d_to_1d(i + vec2<f32>(1.0, 0.0));
    let c = hash_2d_to_1d(i + vec2<f32>(0.0, 1.0));
    let d = hash_2d_to_1d(i + vec2<f32>(1.0, 1.0));

    // Smooth interpolation
    let u = f * f * (vec2<f32>(3.0, 3.0) - 2.0 * f);
    let mix_ab = mix(a, b, u.x);
    let mix_cd = mix(c, d, u.x);
    return mix(mix_ab, mix_cd, u.y);
}

/// Calculate signed area of edge for triangle testing
fn edge_function(a: vec2<f32>, b: vec2<f32>, p: vec2<f32>) -> f32 {
    let ab = b - a;
    let ap = p - a;
    return ab.x * ap.y - ab.y * ap.x;
}

/// Calculate barycentric coordinates for point in triangle
fn barycentric_coords(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>, p: vec2<f32>) -> vec3<f32> {
    let area = edge_function(a, b, c);
    if (abs(area) < 1e-6) {
        return vec3<f32>(-1.0, -1.0, -1.0);  // Degenerate triangle
    }
    let inv_area = 1.0 / area;
    return vec3<f32>(
        edge_function(b, c, p) * inv_area,
        edge_function(c, a, p) * inv_area,
        edge_function(a, b, p) * inv_area
    );
}

/// Convert UV coordinates to aspect-corrected coordinates
fn apply_aspect_ratio(uv0: vec2<f32>) -> vec2<f32> {
    let res = vec2<f32>(U.width, U.height);
    var uv = uv0 * 2.0 - vec2<f32>(1.0, 1.0);
    uv.x = uv.x * (res.x / max(res.y, 1.0));
    return uv;
}

/// Check if UV is within the renderable area (outside border)
fn is_inside_border(uv: vec2<f32>) -> bool {
    return uv.x >= BORDER && uv.y >= BORDER &&
           uv.x <= 1.0 - BORDER && uv.y <= 1.0 - BORDER;
}

/// Draw an antialiased circle
fn draw_circle(pos: vec2<f32>, pixel: vec2<f32>, radius: f32) -> f32 {
    return smoothstep(radius, radius * 0.3, distance(pos, pixel));
}

// =============================================================================
// COLOR SPACE CONVERSIONS
// =============================================================================

/// Convert HSV color to RGB
fn hsv_to_rgb(c: vec3<f32>) -> vec3<f32> {
    let K = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract_vec3(c.xxx + K.xyz) * 6.0 - K.www);
    let q = clamp(p - K.xxx, vec3<f32>(0.0), vec3<f32>(1.0));
    return c.z * mix(K.xxx, q, vec3<f32>(c.y));
}

// =============================================================================
// COMPLEX NUMBER OPERATIONS
// =============================================================================

/// Complex multiplication: (a.x + a.y*i) * (b.x + b.y*i)
fn complex_multiply(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

/// Complex division: (a.x + a.y*i) / (b.x + b.y*i)
fn complex_divide(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    let denom = max(dot(b, b), 1e-6);
    return vec2<f32>(
        (a.x * b.x + a.y * b.y) / denom,
        (a.y * b.x - a.x * b.y) / denom
    );
}

/// Complex exponential: e^(z.x + z.y*i)
fn complex_exp(z: vec2<f32>) -> vec2<f32> {
    let e = exp(z.x);
    return vec2<f32>(cos(z.y), sin(z.y)) * e;
}

/// Complex natural logarithm: ln(z.x + z.y*i)
fn complex_ln(z: vec2<f32>) -> vec2<f32> {
    let mag = max(length(z), 1e-6);
    return vec2<f32>(log(mag), atan2(z.y, z.x));
}

/// Complex power: a^b where both are complex numbers
fn complex_pow(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return complex_exp(complex_multiply(b, complex_ln(a)));
}

/// Complex sine: sin(z.x + z.y*i)
fn complex_sin(z: vec2<f32>) -> vec2<f32> {
    return complex_divide(
        complex_exp(z) - complex_exp(-z),
        vec2<f32>(0.0, 2.0)
    );
}

/// Iterate complex function for fractal generation
fn iterate_complex_function(z_in: vec2<f32>, t: f32) -> vec2<f32> {
    var z = z_in;
    let cuv = vec2<f32>(cos(-t / 50.0), sin(-t / 50.0)) * 0.88;
    let max_mag = 1.0e9;
    let escape_mag = 1.2676506e30;  // 2^100

    for (var i: i32 = 0; i < 100; i = i + 1) {
        let len_z = length(z);
        if (len_z < max_mag) {
            z = complex_sin(complex_multiply(z, cuv));
        } else if (len_z > escape_mag) {
            break;
        }
    }
    return z;
}

// =============================================================================
// SCENE: GLENZ BARS (Classic demoscene sine wave bars)
// =============================================================================

/// Calculate a single sine wave with smooth falloff
fn calculate_sine_wave(
    uv: vec2<f32>,
    frequency: f32,
    amplitude: f32,
    phase_shift: f32,
    y_offset: f32,
    color: vec3<f32>
) -> vec3<f32> {
    // Time-dependent phase drift for organic movement
    let drift = 0.15 * sin(U.time * 0.35 + phase_shift * 0.5);
    let y = sin(U.time * frequency + phase_shift + uv.x + drift) * amplitude + y_offset;
    let scale = smoothstep(0.1, 0.0, abs(y - uv.y));
    return color * scale;
}

/// Render layered sine wave bars (Glenz-style)
fn render_glenz_bars(uv: vec2<f32>) -> vec3<f32> {
    var color = vec3<f32>(0.0);

    // Layer multiple sine waves with different frequencies and colors
    color += calculate_sine_wave(uv, 2.0, 0.25, 0.0, 0.5, vec3<f32>(0.1, 0.1, 1.0));
    color += calculate_sine_wave(uv, 2.6, 0.15, 0.2, 0.5, vec3<f32>(0.0, 1.0, 0.1));
    color += calculate_sine_wave(uv, 0.9, 0.35, 0.4, 0.5, vec3<f32>(1.0, 0.1, 0.1));

    // Apply subtle global color tint
    let tint = 0.03 * sin(U.time * 0.6);
    return color * (1.0 + tint);
}

// =============================================================================
// SCENE: 3D ROTATING CUBE (Raymarch with twisted geometry)
// =============================================================================

/// Apply twist effect to 3D position
fn apply_twist(p: vec3<f32>) -> vec3<f32> {
    let twist_factor = sin(U.time / 3.0) * 1.45;
    let c = cos(twist_factor * p.y);
    let s = sin(twist_factor * 0.5 * p.y);
    let rotation = mat2x2<f32>(vec2<f32>(c, -s), vec2<f32>(s, c));
    let xz = rotation * p.xz;
    return vec3<f32>(xz.x, p.y, xz.y);
}

/// Global variable to store cube position (used for face texture mapping)
var<private> g_cube_position : vec3<f32> = vec3<f32>(0.0, 0.0, 0.0);

/// Signed distance function for a rotating, twisted cube
fn distance_to_cube(p_in: vec3<f32>) -> f32 {
    var p = apply_twist(p_in);

    // Apply multiple rotations for complex movement
    let rot_vec = vec2<f32>(sin(U.time), cos(U.time));
    let rotation = mat2x2<f32>(vec2<f32>(rot_vec.y, -rot_vec.x), vec2<f32>(rot_vec.x, rot_vec.y));

    var xy = rotation * p.xy; p = vec3<f32>(xy.x, xy.y, p.z);
    xy = rotation * p.xy;     p = vec3<f32>(xy.x, xy.y, p.z);
    var yz = rotation * p.yz; p = vec3<f32>(p.x, yz.x, yz.y);
    var zx = rotation * p.zx; p = vec3<f32>(zx.y, p.y, zx.x);
    zx = rotation * p.zx;     p = vec3<f32>(zx.y, p.y, zx.x);
    zx = rotation * p.zx;     p = vec3<f32>(zx.y, p.y, zx.x);

    g_cube_position = p;

    // Rounded cube: max of axis distances minus corner radius
    let box_size = vec3<f32>(0.4, 0.4, 0.4);
    let q = max(abs(p) - box_size, vec3<f32>(0.0));
    return length(q) - 0.08;
}

/// Calculate edge patterns on cube faces
fn calculate_face_edge(uv_in: vec2<f32>) -> f32 {
    var uv = uv_in;
    uv.y = uv.y - floor(uv.y);
    let a = uv.y < uv.x;
    let b = (1.0 - uv.y) < uv.x;
    return select(1.0, 0.0, a == b);
}

/// Calculate surface normal for the cube using finite differences
fn calculate_cube_normal(p: vec3<f32>) -> vec3<f32> {
    let epsilon = vec3<f32>(0.005, -0.005, 0.0);
    return normalize(
        epsilon.xyy * distance_to_cube(p + epsilon.xyy) +
        epsilon.yyx * distance_to_cube(p + epsilon.yyx) +
        epsilon.yxy * distance_to_cube(p + epsilon.yxy) +
        epsilon.xxx * distance_to_cube(p + epsilon.xxx)
    );
}

/// Render the glenz cube with raymarching
fn render_glenz_cube(uv: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv)) { return BORDERCOLOR; }

    let p = -1.0 + uv * 2.0;

    var distance: f32 = 0.0;
    var near_face: f32 = -1.0;
    var far_faces: f32 = -1.0;
    var hit_distance: f32 = -1.0;

    // Raymarching setup
    let ray_origin = vec3<f32>(0.0, 0.0, 2.1);
    let ray_direction = normalize(vec3<f32>(p, -2.0));

    // March the ray through the scene
    for (var i: u32 = 0u; i < 256u; i = i + 1u) {
        let step_distance = distance_to_cube(ray_origin + ray_direction * distance);
        distance = distance + step_distance * 0.5;

        if (distance > 4.0) { break; }

        if (step_distance < 0.001) {
            // Hit! Calculate face edge patterns
            far_faces = calculate_face_edge(g_cube_position.yx) +
                       calculate_face_edge(-g_cube_position.yx) +
                       calculate_face_edge(g_cube_position.xz) +
                       calculate_face_edge(-g_cube_position.xz) +
                       calculate_face_edge(g_cube_position.zy) +
                       calculate_face_edge(-g_cube_position.zy);

            if (hit_distance < 0.0) { hit_distance = distance; }
            if (near_face < 0.0) { near_face = far_faces; }
            distance = distance + 0.05;
        }
    }

    // Start with bar background
    var color = render_glenz_bars(uv);

    if (near_face > 0.0) {
        // Apply lighting to the cube
        let light_pos = vec3<f32>(1.2, 0.4, 0.6);
        let surface_point = ray_origin + ray_direction * hit_distance;
        var light_dir = light_pos - surface_point;
        let light_distance = max(length(light_dir), 0.001);
        light_dir = light_dir / light_distance;

        let ambient = 0.65;
        let surface_normal = calculate_cube_normal(surface_point);
        let diffuse = min(0.35, max(dot(surface_normal, light_dir), 0.0));
        let specular = pow(max(dot(reflect(-light_dir, surface_normal), -ray_direction), 0.0), 28.0);

        let mix_value = vec3<f32>(near_face * 0.42 + far_faces * far_faces * 0.05);
        let base_color = mix(vec3<f32>(0.25, 0.05, 1.0), vec3<f32>(1.0, 1.0, 1.0), mix_value);
        var final_color = base_color;
        final_color = final_color * (diffuse + ambient) + vec3<f32>(0.9, 0.6, 1.0) * specular / 1.4;
        color = final_color;
    }

    return vec4<f32>(color, 1.0);
}

// =============================================================================
// SCENE: PLASMA (Rotating fractal plasma effect)
// =============================================================================

fn render_plasma(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let time = U.time * 4.0;

    // Apply gentle rotation
    let rotation_angle = 0.05 * sin(U.time * 0.5);
    var uv = (create_rotation_matrix(rotation_angle) * (uv_in * 6.0)).xy;

    var i0 = 1.0;
    var i1 = 1.0;
    var i2 = 1.0;
    var i4 = 0.0;

    // Fractal distortion layers
    for (var s: i32 = 0; s < 7; s = s + 1) {
        var r = vec2<f32>(
            cos(uv.y * i0 - i4 + time / (i1 + 0.1)),
            sin(uv.x * i0 - i4 + time / (i1 + 0.1))
        ) / (i2 + 0.02);

        r = r + vec2<f32>(-r.y, r.x) * (0.28 + 0.02 * sin(U.time));
        uv = uv + r;

        i0 = i0 * 1.91;
        i1 = i1 * 1.14;
        i2 = i2 * 1.68;
        i4 = i4 + 0.05 + 0.1 * time * i1;
    }

    // Map distorted coordinates to RGB
    let r = sin(uv.x - time) * 0.5 + 0.5;
    let g = sin((uv.x + uv.y + sin(time * 0.45)) * 0.52) * 0.5 + 0.5;
    let b = sin(uv.y + time * 0.95) * 0.5 + 0.5;

    return vec4<f32>(r, g, b, 1.0);
}

// =============================================================================
// SCENE: FLOW (Polar coordinate visualization)
// =============================================================================

fn render_flow_pattern(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    var p = uv_in * 2.0 - 1.0;
    let t = U.time;
    let radius = length(p);
    let angle = atan2(p.y, p.x);

    // Animated polar patterns
    let wave = 0.5 + 0.5 * sin(8.0 * radius - 3.1 * t);
    let rotation = 0.5 + 0.5 * sin(3.2 * angle + 2.0 * t + 0.2);
    let spiral = 0.5 + 0.5 * sin(5.0 * angle - 1.45 * t);

    let base_color = vec3<f32>(wave, rotation, spiral);

    // Apply vignette
    let vignette = 1.0 - smoothstep(0.68, 1.0, radius);
    return vec4<f32>(base_color * (0.62 + 0.38 * vignette), 1.0);
}

// =============================================================================
// SCENE: FRACTAL FLAME (IFS-based particle system visualization)
// =============================================================================

fn render_fractal_flame(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let aspect = U.width / max(U.height, 1.0);
    var uv = uv_in * 2.0 - 1.0;
    uv.x = uv.x * aspect;
    let time = U.time * 0.35;

    var p = uv;
    var color_accum = vec3<f32>(0.0);
    var weight_accum = 0.0;
    var ripple_accum = 0.0;

    // Iterative Function System with golden angle spacing
    for (var i = 0; i < 7; i = i + 1) {
        let fi = f32(i);
        let angle = time * 0.7 + fi * 2.3999632;  // Golden angle for even distribution

        let rotation = mat2x2<f32>(
            vec2<f32>(cos(angle), -sin(angle)),
            vec2<f32>(sin(angle), cos(angle))
        );
        let twisted = rotation * p;
        let radius = length(twisted);

        // Flame-like falloff
        let fade = exp(-radius * 4.0);
        let flame_intensity = fade * (0.6 + 0.4 * sin(radius * 12.0 - time * 3.0 + fi));

        // Animated color based on position
        color_accum = color_accum + vec3<f32>(
            0.6 + 0.4 * sin(twisted.x * 3.0 + time),
            0.6 + 0.4 * sin(twisted.y * 5.0 - time * 0.5),
            0.6 + 0.4 * sin((twisted.x + twisted.y) * 4.0 + time * 1.5)
        ) * flame_intensity;

        weight_accum = weight_accum + flame_intensity;
        ripple_accum = ripple_accum + flame_intensity * radius;

        // IFS transformation: complex square + offset
        p = vec2<f32>(
            twisted.x * twisted.x - twisted.y * twisted.y,
            2.0 * twisted.x * twisted.y
        ) + 0.35 * vec2<f32>(
            sin(time + fi * 0.73),
            cos(time * 1.1 + fi * 1.37)
        );
    }

    // Normalize and apply effects
    var final_color = color_accum / max(weight_accum, 0.0001);
    final_color = mix(vec3<f32>(0.2, 0.05, 0.02), final_color, 0.7);
    final_color = final_color * (1.15 - clamp(ripple_accum * 0.8, 0.0, 0.9));

    let vignette = 0.65 + 0.35 * smoothstep(1.75, 0.6, length(uv));
    final_color = final_color * vignette;

    return vec4<f32>(clamp(final_color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// =============================================================================
// SCENE: JULIA MORPH (Animated Julia set with orbit traps)
// =============================================================================

fn render_julia_morph(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let aspect = U.width / max(U.height, 1.0);
    var uv = uv_in * 2.0 - 1.0;
    uv.x = uv.x * aspect;
    let time = U.time * 0.4;

    let scale = 1.8;
    var z = uv * scale;

    // Animated Julia set parameter
    let c = vec2<f32>(
        0.35 * sin(time) - 0.2 * cos(time * 0.3),
        0.35 * cos(time * 1.3) + 0.2 * sin(time * 0.5)
    );

    var orbit_intensity = 0.0;
    var smooth_intensity = 0.0;
    var orbit_trap = vec3<f32>(1e5, 1e5, 1e5);
    var derivative = 1.0;
    let bailout = 12.0;
    let max_iterations: i32 = 48;
    var escape_iteration = 0.0;

    // Julia set iteration with orbit traps
    for (var i: i32 = 0; i < max_iterations; i = i + 1) {
        let zx = z.x;
        let zy = z.y;
        let r_squared = zx * zx + zy * zy;
        let radius = sqrt(r_squared);

        derivative = derivative * max(radius, 0.0001) * 2.0;

        // Standard Julia iteration z = z^2 + c
        var new_z = vec2<f32>(zx * zx - zy * zy, 2.0 * zx * zy) + c;

        // Add organic distortion
        new_z = new_z + 0.15 * vec2<f32>(
            sin(new_z.y * 1.5 + time),
            cos(new_z.x * 1.2 - time)
        );

        z = new_z;

        // Track orbit trap (distance to axes and origin)
        orbit_trap = min(orbit_trap, vec3<f32>(abs(z.x), abs(z.y), r_squared));
        orbit_intensity = orbit_intensity + exp(-r_squared * 0.35);
        smooth_intensity = smooth_intensity + exp(-abs(radius - 1.2));

        if (escape_iteration == 0.0 && r_squared > bailout * bailout) {
            escape_iteration = f32(i);
        }
    }

    // Build color from orbit data
    let intensity = orbit_intensity / f32(max_iterations);
    let softness = smooth_intensity / f32(max_iterations);
    let trap_color = orbit_trap / vec3<f32>(f32(max_iterations));

    var color = vec3<f32>(
        pow(intensity, 1.2),
        pow(intensity, 0.8) * 0.8 + trap_color.y * 0.6,
        pow(intensity, 0.6) * 0.6 + trap_color.x * 0.5
    );
    color = color + softness * vec3<f32>(0.08, 0.22, 0.35);

    // Apply escape-based brightness
    var flare = 1.0;
    if (escape_iteration != 0.0) {
        flare = clamp(escape_iteration / f32(max_iterations), 0.0, 1.0);
    }

    color = mix(vec3<f32>(0.02, 0.0, 0.05), color, flare);

    // Animate based on derivative
    let density = 0.6 + 0.4 * sin(derivative * 0.05 - time * 2.0);
    color = color * density;

    let vignette = smoothstep(1.5, 0.25, length(uv));
    color = color * vignette;

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// =============================================================================
// SCENE: FRACTAL CLOUDS (Multi-octave noise with lighting)
// =============================================================================

/// Generate multi-octave cloud density field
fn generate_cloud_field(p_in: vec2<f32>, time: f32) -> f32 {
    var p = p_in;
    var frequency = 1.0;
    var amplitude = 0.55;
    var density = 0.0;

    // Stack multiple noise octaves with domain warping
    for (var i = 0; i < 6; i = i + 1) {
        let fi = f32(i);

        // Domain warping for organic shapes
        let warp = vec2<f32>(
            sin(dot(p, vec2<f32>(0.8, 1.3)) + time * (0.6 + fi * 0.14)),
            cos(dot(p, vec2<f32>(-1.5, 0.9)) - time * (0.5 + fi * 0.17))
        ) * 0.35;

        let sample_pos = p * frequency + warp + vec2<f32>(time * 0.12, -time * 0.08 + fi * 0.5);
        let noise = value_noise_2d(sample_pos);
        density = density + noise * amplitude;

        p = p + warp * 0.25;
        frequency = frequency * 1.82;
        amplitude = amplitude * 0.52;
    }

    return density;
}

fn render_fractal_clouds(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let aspect = U.width / max(U.height, 1.0);
    var uv = uv_in * 2.0 - 1.0;
    uv.x = uv.x * aspect;
    let time = U.time * 0.15;

    // Sky gradient
    let sky = mix(
        vec3<f32>(0.05, 0.08, 0.12),
        vec3<f32>(0.28, 0.42, 0.62),
        clamp(uv.y * 0.4 + 0.55, 0.0, 1.0)
    );

    // Sample cloud density
    let sample_pos = uv * 1.6 + vec2<f32>(time * 0.8, time * -0.6);
    let raw_density = generate_cloud_field(sample_pos, time);
    let density = clamp((raw_density - 0.32) * 1.8, 0.0, 1.0);

    // Calculate cloud normal using finite differences
    let epsilon = 0.01;
    let grad_x = generate_cloud_field(sample_pos + vec2<f32>(epsilon, 0.0), time) -
                 generate_cloud_field(sample_pos - vec2<f32>(epsilon, 0.0), time);
    let grad_y = generate_cloud_field(sample_pos + vec2<f32>(0.0, epsilon), time) -
                 generate_cloud_field(sample_pos - vec2<f32>(0.0, epsilon), time);
    let normal = normalize(vec3<f32>(-grad_x, 1.6, -grad_y));
    let sun_direction = normalize(vec3<f32>(0.45, 0.6, -0.35));

    // Lighting: diffuse + backlighting
    let diffuse = clamp(dot(normal, sun_direction), 0.0, 1.0);
    let back_light = clamp(dot(normal, -sun_direction), 0.0, 1.0);

    let cloud_base = vec3<f32>(0.85, 0.88, 0.92) * (0.6 + 0.5 * diffuse) +
                     vec3<f32>(0.35, 0.38, 0.42) * back_light * 0.35;
    let rim = pow(back_light, 3.5) * 0.4;
    var cloud_color = cloud_base + vec3<f32>(0.45, 0.48, 0.55) * rim;

    // Apply coverage and animation
    let coverage = density * (0.7 + 0.3 * smoothstep(-0.6, 0.9, uv.y));
    let animation = 0.04 * sin(time * 6.0 + uv.x * 18.0) * density;
    cloud_color = cloud_color * (1.0 + animation);

    let combined = mix(sky, cloud_color, clamp(coverage, 0.0, 1.0));
    let vignette = 0.7 + 0.3 * smoothstep(1.45, 0.55, length(uv));

    return vec4<f32>(clamp(combined * vignette, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// =============================================================================
// SCENE: ROTOZOOMER PRO (Classic rotozooming texture effect)
// =============================================================================

fn render_rotozoomer(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let t = U.time;
    let uv = apply_aspect_ratio(uv_in);

    // Animated zoom with wobble
    let zoom = 1.15 + 0.45 * sin(t * 0.42);
    let rotation = create_rotation_matrix(t * 0.58 + 0.12 * sin(t * 0.21));
    var texture_coord = rotation * uv * zoom;
    texture_coord = texture_coord + vec2<f32>(
        0.35 * sin(t * 0.29),
        0.35 * cos(t * 0.26)
    );

    // Create tiled pattern
    let tile = fract(texture_coord) - vec2<f32>(0.5, 0.5);
    let ring_pattern = sin(length(tile) * 24.0 - t * 2.4);
    let stripe_pattern = sin(tile.x * 28.0 + t * 1.4) * cos(tile.y * 24.0 - t * 1.6);
    let combined_pattern = stripe_pattern + 0.6 * ring_pattern;

    // Map pattern to color
    var color = 0.5 + 0.5 * cos(
        vec3<f32>(0.0, 2.0, 4.0) +
        vec3<f32>(combined_pattern * 2.2, combined_pattern * 1.8, combined_pattern * 1.6)
    );
    color = color + 0.15 * sin(vec3<f32>(texture_coord.xyx * 2.0 + t * 1.1));

    // Add scanline effect
    let scanline = 0.85 + 0.15 * sin((uv_in.y * U.height) * 0.75 + t * 2.0);
    color = color * scanline;

    let vignette = 0.65 + 0.35 * smoothstep(1.6, 0.5, length(uv));
    color = color * vignette;

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.2)), 1.0);
}

// =============================================================================
// SCENE: ROTATING GRID (Infinite zoom tunnel effect)
// =============================================================================

fn render_rotating_grid(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    var frag_coord = vec2<f32>(uv_in.x * U.width, (1.0 - uv_in.y) * U.height);
    var color = vec3<f32>(0.0);

    var uv = frag_coord / res - vec2<f32>(0.5, 0.5);
    uv.y = uv.y * (res.y / max(res.x, 1.0));

    // Infinite zoom effect with layered grids
    var time_offset = -U.time * 0.25;
    for (var i = 0.0; i < 8.0; i = i + 2.0) {
        let layer_time = fract_scalar(time_offset + i / 8.0) * 8.0;
        let depth = layer_time * layer_time;
        let rotation_shift = vec2<f32>(sin(-time_offset * 1.57), cos(time_offset * 1.57)) / max(depth, 0.001);
        let grid = floor(fract((uv + rotation_shift) * depth) * 2.0);
        let checker = grid.x + grid.y - 1.0;
        color = max(color, vec3<f32>(checker * checker / max(layer_time, 0.0001) - 0.1));
    }

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// =============================================================================
// SCENE: COMPLEX CASCADE (Complex function visualization with banding)
// =============================================================================

fn render_complex_cascade(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    let frag_coord = vec2<f32>(uv_in.x * res.x, (1.0 - uv_in.y) * res.y);
    let aspect = res.x / max(res.y, 1.0);
    var uv = frag_coord / res.y - vec2<f32>(aspect * 0.5, 0.5);

    // Iterate complex function
    let warped = iterate_complex_function(uv, U.time);

    // Convert to polar coordinates
    var angle = atan2(warped.y, warped.x);
    if (angle < 0.0) { angle = angle + 2.0 * PI; }

    let magnitude = max(length(warped), 1e-6);
    let log_magnitude = log(magnitude);

    // Create banding effect based on logarithmic magnitude
    let band_value = abs(log_magnitude) * 100.0;
    let band_index = f32(i32(floor(band_value)) % 100);

    var brightness = 1.0 - band_index / 200.0;
    if (log_magnitude <= 0.0) {
        brightness = 0.5 + band_index / 200.0;
    }
    brightness = clamp(brightness, 0.0, 1.0);

    // Map to HSV color based on angle
    let hue = angle / (2.0 * PI);
    let color = hsv_to_rgb(vec3<f32>(fract_scalar(hue), 1.0, brightness));

    return vec4<f32>(color, 1.0);
}

// =============================================================================
// SCENE: OLD SCHOOL RASTERBARS (Classic sine wave bars with circles)
// =============================================================================

fn render_old_school_rasterbars(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    let frag_coord = vec2<f32>(uv_in.x * res.x, (1.0 - uv_in.y) * res.y);
    var uv = frag_coord / res - vec2<f32>(0.5, 0.5);
    uv.x = uv.x * (res.x / max(res.y, 1.0));

    var rgb = vec3<f32>(0.0);

    // Sine wave parameters
    let frequency = 1.5;
    let amplitude = 0.3;
    let speed = 2.0;
    let bar_height = 0.07;

    // Three colored sine bars with phase offsets
    var red_pos = sin(U.time * speed + uv.x * frequency) * amplitude;
    var green_pos = sin((U.time + 0.2) * speed + uv.x * frequency) * amplitude;
    var blue_pos = sin((U.time + 0.4) * speed + uv.x * frequency) * amplitude;

    // Add secondary modulation
    red_pos = red_pos + cos((U.time + sin(uv.x)) * 0.4) * 0.2;
    green_pos = green_pos + cos((U.time + sin(uv.x)) * 0.5) * 0.2;
    blue_pos = blue_pos + cos((U.time + sin(uv.x)) * 0.6) * 0.2;

    // Draw smooth bars
    rgb.r = rgb.r + smoothstep(red_pos + bar_height, red_pos, uv.y) -
            smoothstep(red_pos, red_pos - bar_height, uv.y);
    rgb.g = rgb.g + smoothstep(green_pos + bar_height, green_pos, uv.y) -
            smoothstep(green_pos, green_pos - bar_height, uv.y);
    rgb.b = rgb.b + smoothstep(blue_pos + bar_height, blue_pos, uv.y) -
            smoothstep(blue_pos, blue_pos - bar_height, uv.y);

    // Add moving circles
    let t = U.time;
    for (var i: i32 = 0; i < 64; i = i + 1) {
        let fi = f32(i);
        let circle_x = cos(t * cos(clamp(t * 1.139, 1.22, 1.8)) * 2.0 + fi * 0.08) * 0.8;
        let circle_y = sin(t * sin(clamp(t * 0.01, 0.8, 0.9)) * 1.2 + fi * 0.06) * 0.4;
        rgb = rgb + draw_circle(vec2<f32>(circle_x, circle_y), uv, 0.03) * 0.97;
    }

    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// =============================================================================
// SCENE: INTERFERENCE WELLS (Dual-source wave interference)
// =============================================================================

fn render_interference_wells(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    var uv = vec2<f32>(uv_in.x * res.x, uv_in.y * res.y);
    let min_dimension = min(res.x, res.y);
    uv = uv / min_dimension - vec2<f32>(0.5 * (res.x / min_dimension), 0.5 * (res.y / min_dimension));
    uv = uv * 2.0;

    let t = U.time * 0.25;

    // Two moving wave sources
    let center1 = vec2<f32>(
        sin(t) / 2.5 + sin(2.0 * t) / 2.5,
        cos(t) / 2.5 + sin(2.0 * t) / 2.5
    );
    let center2 = vec2<f32>(
        cos(t) / 2.5 + cos(2.0 * t) / 2.5,
        sin(t) / 2.5 + cos(2.0 * t) / 2.5
    );

    // Chromatic dispersion coefficients
    let red_coeff = 0.995;
    let green_coeff = 1.0;
    let blue_coeff = 1.005;

    let r1 = length(uv - center1);
    let r2 = length(uv - center2);

    // Calculate interference for each channel
    var red = -sin(sqrt(r1) * 100.0 * red_coeff) + 0.5 - r1 * 0.5 * red_coeff;
    red = red + (-sin(sqrt(r2) * 100.0 * red_coeff) + 0.5 - r2 * 0.5 * red_coeff);
    red = red * 0.25;

    var green = -sin(sqrt(r1) * 100.0 * green_coeff) + 0.5 - r1 * 0.5 * green_coeff;
    green = green + (-sin(sqrt(r2) * 100.0 * green_coeff) + 0.5 - r2 * 0.5 * green_coeff);
    green = green * 0.25;

    var blue = -sin(sqrt(r1) * 100.0 * blue_coeff) + 0.5 - r1 * 0.5 * blue_coeff;
    blue = blue + (-sin(sqrt(r2) * 100.0 * blue_coeff) + 0.5 - r2 * 0.5 * blue_coeff);
    blue = blue * 0.25;

    let final_color = vec3<f32>(red + green, 0.5 * red + green + 0.5 * blue, green + blue);
    return vec4<f32>(clamp(final_color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// =============================================================================
// RAYMARCHING: Sine-deformed sphere
// =============================================================================

const RM_MAX_STEPS : u32 = 128u;
const RM_MAX_DIST : f32 = 1000.0;
const RM_MIN_HIT_DIST : f32 = 0.001;

/// Signed distance to scene with animated sine deformation
fn distance_to_scene(point: vec3<f32>) -> f32 {
    // Sphere with sine displacement
    let sphere_displacement = sin(1.5 * point.x + U.time) *
                             sin(1.5 * point.y + U.time) *
                             sin(2.5 * point.z + U.time) * 0.5;

    // Plane with sine displacement
    let plane_displacement = sin(length(point.xy) - U.time) * 0.5;

    let sphere_center = vec3<f32>(0.0, 0.0, 0.0);
    let sphere_radius = 6.0;
    let sphere_distance = length(point - sphere_center) - sphere_radius + sphere_displacement;
    let plane_distance = point.z + 4.0 + plane_displacement;

    return min(sphere_distance, plane_distance);
}

/// March a ray through the scene
fn raymarch_sine_sphere(ray_origin: vec3<f32>, ray_direction: vec3<f32>) -> vec3<f32> {
    let direction = normalize(ray_direction);
    var total_distance = 0.0;
    var current_position = ray_origin;

    for (var i: u32 = 0u; i < RM_MAX_STEPS; i = i + 1u) {
        current_position = ray_origin + direction * total_distance;
        let distance = distance_to_scene(current_position);

        if (distance < RM_MIN_HIT_DIST) {
            // Hit! Use position as color
            return normalize(current_position) * 0.6 + vec3<f32>(0.6, 0.6, 0.6);
        }

        if (total_distance > RM_MAX_DIST) {
            break;
        }

        total_distance = total_distance + distance;
    }

    return normalize(current_position) * 0.5 + vec3<f32>(0.5, 0.5, 0.5);
}

fn render_raymarched_sine_sphere(uv_in: vec2<f32>, frag_coord: vec2<f32>, res: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    var uv = (2.0 * frag_coord - res) / min(res.x, res.y);

    // Animated camera position
    let angle = U.time;
    let distance = 10.0;
    let camera_pos = vec3<f32>(distance * sin(angle), distance * cos(angle), 0.0);
    let camera_target = vec3<f32>(
        uv.x * sin(angle + PI / 2.0) + sin(angle + PI),
        uv.x * cos(angle + PI / 2.0) + cos(angle + PI),
        uv.y
    );

    let color = raymarch_sine_sphere(camera_pos, camera_target);
    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// =============================================================================
// SCENE: ROTATING COSINE GRID (Simple but hypnotic pattern)
// =============================================================================

fn render_rotating_cosine_grid(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    let frag_coord = vec2<f32>(uv_in.x * res.x, (1.0 - uv_in.y) * res.y);
    let angle = radians(U.time * 90.0);

    let rotation = mat2x2<f32>(
        vec2<f32>(cos(angle), -sin(angle)),
        vec2<f32>(sin(angle),  cos(angle))
    );

    let rotated = rotation * frag_coord;
    let scale = 20.0;

    let color = vec3<f32>(
        0.5 + 0.5 * sin(rotated.x / scale),
        0.5 + 0.5 * sin(rotated.y / scale),
        0.5 + 0.5 * sin((rotated.x + rotated.y) / scale)
    );

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// =============================================================================
// SCENE: NEON SCANLINES (Animated beam sweeps)
// =============================================================================

fn render_neon_scanlines(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    let frag_coord = vec2<f32>(uv_in.x * res.x, (1.0 - uv_in.y) * res.y);
    let y_coord = frag_coord.y / max(res.x, 1.0) + 1.0;
    let time = U.time * 3.0;

    var color = vec3<f32>(0.0, 0.0, cos(y_coord * 6.0 - 5.0));
    var hit = false;

    // Multiple moving scanlines
    for (var i: i32 = 0; i < 18; i = i + 1) {
        let index = f32(i);
        let scanline_pos = sin(time + index / 3.4) / 6.0 + 1.25;

        if (y_coord > scanline_pos && y_coord < scanline_pos + 0.05) {
            let beam = vec3<f32>(scanline_pos, sin(y_coord + time * 0.3), index / 16.0);
            let envelope = (y_coord - scanline_pos) * sin((y_coord - scanline_pos) * 20.0 * PI) * 38.0;
            color = beam * envelope;
            hit = true;
        }
    }

    if (!hit) {
        color = vec3<f32>(0.0, 0.0, max(color.z, 0.0));
    }

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}

// =============================================================================
// SCENE: PERSPECTIVE CHECKERBOARD (Classic tunnel effect)
// =============================================================================

fn render_perspective_checkerboard(uv_in: vec2<f32>) -> vec4<f32> {
    if (!is_inside_border(uv_in)) { return BORDERCOLOR; }

    let res = vec2<f32>(U.width, U.height);
    let frag_coord = vec2<f32>(uv_in.x * res.x, (1.0 - uv_in.y) * res.y);
    var uv = (frag_coord / res) - vec2<f32>(0.5, 0.5);
    uv = uv * 1.5;

    // Perspective projection
    let depth = 2.0 / max(abs(uv.y), 1e-4);
    let x = uv.x * depth;

    if (depth >= 8.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    // Animated checkerboard pattern
    let pos = vec2<f32>(3.0 * sin(U.time) + x, 3.0 * U.time + depth);
    let texture = step(vec2<f32>(0.5, 0.5), fract(pos));
    let checker = f32(texture.x != texture.y) * smoothstep(9.0, 0.0, depth);

    return vec4<f32>(vec3<f32>(checker), 1.0);
}

// =============================================================================
// JELLY CUBE: Multi-textured rotating cube with face compositing
// =============================================================================

/// SDF for scene bounds (large box around everything)
fn distance_to_bounds(p: vec3<f32>) -> f32 {
    return max(max(abs(p.x), abs(p.y)), abs(p.z)) - 5.0;
}

/// Calculate normal for scene bounds
fn calculate_bounds_normal(p: vec3<f32>) -> vec3<f32> {
    let offset = vec3<f32>(0.01, 0.0, 0.0);
    return normalize(vec3<f32>(
        distance_to_bounds(p - offset.xyz) - distance_to_bounds(p + offset.xyz),
        distance_to_bounds(p - offset.zxy) - distance_to_bounds(p + offset.zxy),
        distance_to_bounds(p - offset.yzx) - distance_to_bounds(p + offset.yzx)
    ));
}

/// Raymarch the bounded scene
fn raymarch_bounds(ray_origin: vec3<f32>, ray_direction: vec3<f32>) -> vec3<f32> {
    var hit_distance = 0.0;
    for (var i: i32 = 0; i < 128; i = i + 1) {
        let distance = distance_to_bounds(ray_origin + ray_direction * hit_distance);
        hit_distance = hit_distance + distance;
        if (distance < 0.0001) { break; }
    }
    return ray_origin + ray_direction * hit_distance;
}

/// Composite texture across cube face based on normal
fn composite_face_texture(position: vec3<f32>, normal: vec3<f32>, face_index: i32) -> vec3<f32> {
    // Weight normal components for smooth blending
    var weighted_normal = max(abs(normal) - vec3<f32>(0.2), vec3<f32>(0.001));
    weighted_normal = weighted_normal / (weighted_normal.x + weighted_normal.y + weighted_normal.z);

    let projected_pos = position * 0.1 + vec3<f32>(0.5);

    // Map different scenes to different faces
    if (face_index == 1) {
        return render_glenz_cube(projected_pos.yz).xyz * weighted_normal.x +
               render_glenz_cube(projected_pos.zx).xyz * weighted_normal.y +
               render_glenz_cube(projected_pos.xy).xyz * weighted_normal.z;
    }
    if (face_index == 2) {
        return render_rotating_grid(projected_pos.yz).xyz * weighted_normal.x +
               render_rotating_grid(projected_pos.zx).xyz * weighted_normal.y +
               render_rotating_grid(projected_pos.xy).xyz * weighted_normal.z;
    }
    if (face_index == 3) {
        return render_fractal_flame(projected_pos.yz).xyz * weighted_normal.x +
               render_fractal_flame(projected_pos.zx).xyz * weighted_normal.y +
               render_fractal_flame(projected_pos.xy).xyz * weighted_normal.z;
    }
    if (face_index == 4) {
        return render_interference_wells(projected_pos.yz).xyz * weighted_normal.x +
               render_interference_wells(projected_pos.zx).xyz * weighted_normal.y +
               render_interference_wells(projected_pos.xy).xyz * weighted_normal.z;
    }
    if (face_index == 5) {
        return render_fractal_clouds(projected_pos.yz).xyz * weighted_normal.x +
               render_fractal_clouds(projected_pos.zx).xyz * weighted_normal.y +
               render_fractal_clouds(projected_pos.xy).xyz * weighted_normal.z;
    }
    if (face_index == 6) {
        return render_flow_pattern(projected_pos.yz).xyz * weighted_normal.x +
               render_flow_pattern(projected_pos.zx).xyz * weighted_normal.y +
               render_flow_pattern(projected_pos.xy).xyz * weighted_normal.z;
    }
    return vec3<f32>(1.0);
}

/// Determine color based on which cube face the normal points to
fn get_face_color(position: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    if (dot(normal, vec3<f32>(1.0, 0.0, 0.0)) > 0.0) { return composite_face_texture(position, normal, 1); }
    if (dot(normal, vec3<f32>(1.0, 0.0, 0.0)) < 0.0) { return composite_face_texture(position, normal, 2); }
    if (dot(normal, vec3<f32>(0.0, 0.0, 1.0)) > 0.0) { return composite_face_texture(position, normal, 3); }
    if (dot(normal, vec3<f32>(0.0, 0.0, 1.0)) < 0.0) { return composite_face_texture(position, normal, 4); }
    if (dot(normal, vec3<f32>(0.0, 1.0, 0.0)) > 0.0) { return composite_face_texture(position, normal, 5); }
    return composite_face_texture(position, normal, 6);
}

/// Render the jelly cube (multi-textured cube in 3D space)
fn render_jelly_cube(uv01_in: vec2<f32>, frag_coord: vec2<f32>, res: vec2<f32>, time: f32) -> vec4<f32> {
    var uv = frag_coord / res.y;
    let aspect_ratio = res / res.y;

    // Distort UVs for jelly effect
    uv.x = uv.x + 0.2 * sin(4.0 * uv.y + time);

    var ray_origin = vec3<f32>(0.0, 0.0, -20.0);
    var ray_direction = normalize(vec3<f32>(uv - aspect_ratio * 0.5, 1.0));

    // Apply camera rotations
    let rotation_x = create_rotation_matrix(time);
    let rotation_y = create_rotation_matrix(time * 2.0);

    var yz = rotation_x * ray_origin.yz; ray_origin = vec3<f32>(ray_origin.x, yz.x, yz.y);
    var xz = rotation_y * ray_origin.xz; ray_origin = vec3<f32>(xz.x, ray_origin.y, xz.y);

    yz = rotation_x * ray_direction.yz; ray_direction = vec3<f32>(ray_direction.x, yz.x, yz.y);
    xz = rotation_y * ray_direction.xz; ray_direction = vec3<f32>(xz.x, ray_direction.y, xz.y);

    let surface_point = raymarch_bounds(ray_origin, ray_direction);
    let surface_normal = calculate_bounds_normal(surface_point);

    var color = vec3<f32>(0.0);
    let distance = distance_to_bounds(surface_point);

    if (abs(distance) < 0.01) {
        color = get_face_color(surface_point, surface_normal);

        // Apply simple lighting
        let light_pos = ray_origin - vec3<f32>(5.0, 5.0, 5.0);
        var light_dir = light_pos - surface_point;
        let light_distance = max(length(light_dir), 0.001);
        light_dir = light_dir / light_distance;

        let diffuse = min(0.3, max(dot(surface_normal, light_dir), 0.0));
        let specular = pow(max(dot(reflect(-light_dir, surface_normal), -ray_direction), 0.0), 24.0);

        color = color * (1.0 + diffuse) + vec3<f32>(1.0) * specular * 0.4;
    }

    return vec4<f32>(color, 1.0);
}

// =============================================================================
// CRT EFFECT (Timothy Lottes single-pass adaptation)
// =============================================================================

/// Context for CRT rendering
struct CRTContext {
    resolution: vec2<f32>,
    current_scene: u32,
    next_scene: u32,
    transition_alpha: f32,
};

const CRT_EMU_SCALE   : f32 = 6.0;              // Emulated resolution scale
const CRT_HARD_SCAN   : f32 = -8.0;             // Scanline hardness
const CRT_HARD_PIX    : f32 = -3.0;             // Pixel sharpness
const CRT_WARP_FACTOR : vec2<f32> = vec2<f32>(1.0 / 32.0, 1.0 / 24.0);  // Screen curvature
const CRT_MASK_DARK   : f32 = 0.5;              // Shadow mask darkness
const CRT_MASK_LIGHT  : f32 = 1.5;              // Shadow mask brightness

/// Convert linear color to sRGB
fn linear_to_srgb_channel(c: f32) -> f32 {
    if (c <= 0.04045) { return c / 12.92; }
    return pow((c + 0.055) / 1.055, 2.4);
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        linear_to_srgb_channel(c.x),
        linear_to_srgb_channel(c.y),
        linear_to_srgb_channel(c.z)
    );
}

/// Convert sRGB color to linear
fn srgb_to_linear_channel(c: f32) -> f32 {
    if (c < 0.0031308) { return c * 12.92; }
    return 1.055 * pow(c, 0.41666) - 0.055;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        srgb_to_linear_channel(c.x),
        srgb_to_linear_channel(c.y),
        srgb_to_linear_channel(c.z)
    );
}

/// Sample scene with transition blending
fn sample_scene_for_crt(ctx: CRTContext, uv: vec2<f32>) -> vec3<f32> {
    let frag_coord = vec2<f32>(uv.x * ctx.resolution.x, (1.0 - uv.y) * ctx.resolution.y);
    let scene_a = dispatch_scene(ctx.current_scene, uv, frag_coord, ctx.resolution);
    let scene_b = dispatch_scene(ctx.next_scene, uv, frag_coord, ctx.resolution);
    return mix(scene_a.rgb, scene_b.rgb, ctx.transition_alpha);
}

/// Fetch color at specific texture coordinate with bounds checking
fn crt_fetch(ctx: CRTContext, pos: vec2<f32>, offset: vec2<f32>, emu_res: vec2<f32>) -> vec3<f32> {
    let sample_pos = floor(pos * emu_res + offset) / emu_res;
    if (max(abs(sample_pos.x - 0.5), abs(sample_pos.y - 0.5)) > 0.5) {
        return vec3<f32>(0.0);
    }
    return linear_to_srgb(sample_scene_for_crt(ctx, sample_pos));
}

/// Calculate distance from pixel center in emulated resolution
fn crt_distance(pos: vec2<f32>, emu_res: vec2<f32>) -> vec2<f32> {
    let scaled = pos * emu_res;
    return -((scaled - floor(scaled)) - vec2<f32>(0.5));
}

/// Gaussian blur kernel
fn crt_gaussian(pos: f32, scale: f32) -> f32 {
    return exp2(scale * pos * pos);
}

/// Sample 3 horizontal pixels with gaussian blur
fn crt_sample_horizontal_3(ctx: CRTContext, pos: vec2<f32>, y_offset: f32, emu_res: vec2<f32>) -> vec3<f32> {
    let b = crt_fetch(ctx, pos, vec2<f32>(-1.0, y_offset), emu_res);
    let c = crt_fetch(ctx, pos, vec2<f32>( 0.0, y_offset), emu_res);
    let d = crt_fetch(ctx, pos, vec2<f32>( 1.0, y_offset), emu_res);

    let distance = crt_distance(pos, emu_res).x;
    let scale = CRT_HARD_PIX;

    let wb = crt_gaussian(distance - 1.0, scale);
    let wc = crt_gaussian(distance + 0.0, scale);
    let wd = crt_gaussian(distance + 1.0, scale);

    return (b * wb + c * wc + d * wd) / (wb + wc + wd);
}

/// Sample 5 horizontal pixels with gaussian blur
fn crt_sample_horizontal_5(ctx: CRTContext, pos: vec2<f32>, y_offset: f32, emu_res: vec2<f32>) -> vec3<f32> {
    let a = crt_fetch(ctx, pos, vec2<f32>(-2.0, y_offset), emu_res);
    let b = crt_fetch(ctx, pos, vec2<f32>(-1.0, y_offset), emu_res);
    let c = crt_fetch(ctx, pos, vec2<f32>( 0.0, y_offset), emu_res);
    let d = crt_fetch(ctx, pos, vec2<f32>( 1.0, y_offset), emu_res);
    let e = crt_fetch(ctx, pos, vec2<f32>( 2.0, y_offset), emu_res);

    let distance = crt_distance(pos, emu_res).x;
    let scale = CRT_HARD_PIX;

    let wa = crt_gaussian(distance - 2.0, scale);
    let wb = crt_gaussian(distance - 1.0, scale);
    let wc = crt_gaussian(distance + 0.0, scale);
    let wd = crt_gaussian(distance + 1.0, scale);
    let we = crt_gaussian(distance + 2.0, scale);

    return (a * wa + b * wb + c * wc + d * wd + e * we) / (wa + wb + wc + wd + we);
}

/// Calculate scanline intensity
fn crt_scanline_weight(pos: vec2<f32>, y_offset: f32, emu_res: vec2<f32>) -> f32 {
    let distance = crt_distance(pos, emu_res).y;
    return crt_gaussian(distance + y_offset, CRT_HARD_SCAN);
}

/// Triangular blur filter (3-tap vertical, 5-tap horizontal center)
fn crt_triangle_filter(ctx: CRTContext, pos: vec2<f32>, emu_res: vec2<f32>) -> vec3<f32> {
    let above = crt_sample_horizontal_3(ctx, pos, -1.0, emu_res);
    let center = crt_sample_horizontal_5(ctx, pos,  0.0, emu_res);
    let below = crt_sample_horizontal_3(ctx, pos,  1.0, emu_res);

    let weight_above = crt_scanline_weight(pos, -1.0, emu_res);
    let weight_center = crt_scanline_weight(pos,  0.0, emu_res);
    let weight_below = crt_scanline_weight(pos,  1.0, emu_res);

    return above * weight_above + center * weight_center + below * weight_below;
}

/// Apply barrel distortion (CRT screen curvature)
fn crt_warp(pos: vec2<f32>) -> vec2<f32> {
    var p = pos * 2.0 - 1.0;
    p = p * vec2<f32>(
        1.0 + (p.y * p.y) * CRT_WARP_FACTOR.x,
        1.0 + (p.x * p.x) * CRT_WARP_FACTOR.y
    );
    return p * 0.5 + 0.5;
}

/// Generate shadow mask pattern (RGB phosphor emulation)
fn crt_shadow_mask(pos: vec2<f32>) -> vec3<f32> {
    var p = pos;
    p.x = p.x + p.y * 3.0;  // Slant the mask

    var mask = vec3<f32>(CRT_MASK_DARK);
    let fractional_x = fract_scalar(p.x / 6.0);

    if (fractional_x < 0.333) {
        mask.r = CRT_MASK_LIGHT;  // Red phosphor
    } else if (fractional_x < 0.666) {
        mask.g = CRT_MASK_LIGHT;  // Green phosphor
    } else {
        mask.b = CRT_MASK_LIGHT;  // Blue phosphor
    }

    return mask;
}

/// Apply complete CRT effect
fn apply_crt_effect(ctx: CRTContext, frag_coord: vec2<f32>) -> vec3<f32> {
    let emulated_resolution = ctx.resolution / CRT_EMU_SCALE;
    let warped_position = crt_warp(frag_coord / ctx.resolution);

    let filtered_color = crt_triangle_filter(ctx, warped_position, emulated_resolution);
    let masked_color = filtered_color * crt_shadow_mask(frag_coord);

    return srgb_to_linear(max(masked_color, vec3<f32>(0.0)));
}

// =============================================================================
// SCENE DISPATCHER
// =============================================================================

/// Dispatch to the appropriate scene renderer
fn dispatch_scene(scene_index: u32, uv: vec2<f32>, frag_coord: vec2<f32>, resolution: vec2<f32>) -> vec4<f32> {
    if (scene_index == 0u)  { return render_rotating_grid(uv); }
    if (scene_index == 1u)  { return render_fractal_clouds(uv); }
    if (scene_index == 2u)  { return render_plasma(uv); }
    if (scene_index == 3u)  { return render_flow_pattern(uv); }
    if (scene_index == 4u)  { return render_fractal_flame(uv); }
    if (scene_index == 5u)  { return render_julia_morph(uv); }
    if (scene_index == 6u)  { return render_rotozoomer(uv); }
    if (scene_index == 7u)  { return render_glenz_cube(uv); }
    if (scene_index == 8u)  { return render_complex_cascade(uv); }
    if (scene_index == 9u)  { return render_old_school_rasterbars(uv); }
    if (scene_index == 10u) { return render_interference_wells(uv); }
    if (scene_index == 11u) { return render_raymarched_sine_sphere(uv, frag_coord, resolution); }
    if (scene_index == 12u) { return render_rotating_cosine_grid(uv); }
    if (scene_index == 13u) { return render_neon_scanlines(uv); }
    if (scene_index == 14u) { return render_perspective_checkerboard(uv); }
    if (scene_index == 15u) { return render_jelly_cube(uv, frag_coord, resolution, U.time); }
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);  // Black fallback
}

// =============================================================================
// FRAGMENT SHADER ENTRY POINT
// =============================================================================

@fragment
fn frag_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    // Convert Bevy UVs to pixel coordinates (Y-flipped)
    let frag_coord = vec2<f32>(vertex.uv.x * U.width, (1.0 - vertex.uv.y) * U.height);
    let resolution = vec2<f32>(U.width, U.height);
    let uv = vertex.uv;

    // === SCENE SEQUENCER ===
    // Calculate which scene to show based on time
    let time = U.time;
    let scene_slot = time / SCENE_DURATION;
    let scene_slot_index = u32(floor(scene_slot));
    let current_scene = scene_slot_index % SCENE_COUNT;
    let next_scene = (current_scene + 1u) % SCENE_COUNT;

    // Calculate transition progress
    let local_time = fract_scalar(scene_slot);  // 0..1 within current scene
    let transition_start = 1.0 - (TRANSITION_DURATION / SCENE_DURATION);
    let transition_phase = clamp(
        (local_time - transition_start) / max(TRANSITION_DURATION / SCENE_DURATION, 1e-5),
        0.0,
        1.0
    );
    let transition_alpha = smoothstep(0.0, 1.0, transition_phase);

    // Render current and next scene, blend between them
    let color_current = dispatch_scene(current_scene, uv, frag_coord, resolution);
    let color_next = dispatch_scene(next_scene, uv, frag_coord, resolution);
    var final_color = mix(color_current.rgb, color_next.rgb, transition_alpha);

    // === OPTIONAL CRT EFFECT ===
    if (U.crt_enabled == 1u) {
        let crt_context = CRTContext(resolution, current_scene, next_scene, transition_alpha);
        final_color = apply_crt_effect(crt_context, frag_coord);
    }

    return vec4<f32>(final_color, 1.0);
}

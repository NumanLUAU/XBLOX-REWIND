/*
    written by: @corescripts
*/

// Roblox-style (2016 "compatibility") lighting:
//   ambient + outdoor ambient, sun diffuse tinted by ColorShift_Top/Bottom,
//   sun shadow map, point/surface/spot lights, specular per material, reflectance,
//   linear fog, procedural studs/inlets and material patterns.

struct Light {
    pos_range: vec4<f32>,
    color_kind: vec4<f32>,
    dir_angle: vec4<f32>,
};

struct Globals {
    view_proj: mat4x4<f32>,
    shadow_mat: mat4x4<f32>,
    cam_pos: vec4<f32>,        // w: time
    sun_dir: vec4<f32>,        // w: brightness
    ambient: vec4<f32>,        // w: shadows on
    outdoor: vec4<f32>,        // w: light count
    shift_top: vec4<f32>,
    shift_bottom: vec4<f32>,
    fog_color: vec4<f32>,
    fog: vec4<f32>,            // start, end, sun visible, shadow texel
    lights: array<Light, 48>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_tex: texture_depth_2d;
@group(0) @binding(2) var shadow_smp: sampler_comparison;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var smp: sampler;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) vcolor: vec4<f32>,
    @location(4) m0: vec4<f32>,
    @location(5) m1: vec4<f32>,
    @location(6) m2: vec4<f32>,
    @location(7) m3: vec4<f32>,
    @location(8) scale_mat: vec4<f32>,
    @location(9) color: vec4<f32>,
    @location(10) extra: vec4<u32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) obj: vec3<f32>,
    @location(3) uv: vec2<f32>,
    @location(4) vcolor: vec4<f32>,
    @location(5) color: vec4<f32>,
    @location(6) @interpolate(flat) extra: vec4<u32>,
    @location(7) @interpolate(flat) material: f32,
    @location(8) objn: vec3<f32>,
    @location(9) @interpolate(flat) size: vec3<f32>,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    let model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    let local = v.pos * v.scale_mat.xyz;
    let world = (model * vec4<f32>(local, 1.0)).xyz;
    let nobj = normalize(v.normal / max(abs(v.scale_mat.xyz), vec3<f32>(1e-4)) * sign(v.scale_mat.xyz + vec3<f32>(1e-9)));
    var o: VOut;
    o.clip = g.view_proj * vec4<f32>(world, 1.0);
    o.world = world;
    o.normal = normalize((model * vec4<f32>(nobj, 0.0)).xyz);
    o.obj = local;
    o.objn = v.normal;
    o.uv = v.uv;
    o.vcolor = v.vcolor;
    o.color = v.color;
    o.extra = v.extra;
    o.material = v.scale_mat.w;
    o.size = abs(v.scale_mat.xyz);
    return o;
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
    let model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    let world = (model * vec4<f32>(v.pos * v.scale_mat.xyz, 1.0)).xyz;
    return g.shadow_mat * vec4<f32>(world, 1.0);
}

fn hash3(p: vec3<f32>) -> f32 {
    var q = fract(p * 0.3183099 + vec3<f32>(0.1, 0.17, 0.13));
    q = q * 17.0;
    return fract(q.x * q.y * q.z * (q.x + q.y + q.z));
}

fn vnoise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(mix(hash3(i), hash3(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash3(i + vec3<f32>(0.0, 1.0, 0.0)), hash3(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y),
        mix(mix(hash3(i + vec3<f32>(0.0, 0.0, 1.0)), hash3(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash3(i + vec3<f32>(0.0, 1.0, 1.0)), hash3(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y), u.z);
}

fn fbm(p: vec3<f32>) -> f32 {
    var a = 0.5;
    var s = 0.0;
    var q = p;
    for (var i = 0; i < 4; i = i + 1) {
        s = s + a * vnoise(q);
        q = q * 2.03;
        a = a * 0.5;
    }
    return s;
}

// Returns (albedo multiplier, specular strength, specular power)
fn material(mat: u32, p: vec3<f32>) -> vec3<f32> {
    switch mat {
        case 272u: { return vec3<f32>(1.0, 0.35, 40.0); }                              // SmoothPlastic
        case 288u: { return vec3<f32>(1.0, 0.0, 1.0); }                                // Neon
        case 512u: {                                                                     // Wood
            let r = sin((p.x * 0.4 + fbm(p * vec3<f32>(0.3, 3.0, 0.3)) * 3.0) * 6.0);
            return vec3<f32>(0.85 + 0.15 * r, 0.1, 10.0);
        }
        case 528u: {                                                                     // WoodPlanks
            let plank = floor(p.z * 0.5 + floor(p.y));
            let edge = smoothstep(0.0, 0.05, abs(fract(p.z * 0.5) - 0.5) * 2.0);
            return vec3<f32>((0.8 + 0.2 * hash3(vec3<f32>(plank, 3.0, 1.0))) * mix(0.6, 1.0, 1.0 - (1.0 - edge) * 0.5) + fbm(p * vec3<f32>(0.2, 4.0, 0.2)) * 0.1, 0.1, 10.0);
        }
        case 784u, 832u: { return vec3<f32>(0.85 + 0.25 * fbm(p * 0.7), 0.3, 30.0); }  // Marble, Granite
        case 800u: { return vec3<f32>(0.75 + 0.35 * fbm(p * 0.35), 0.15, 12.0); }       // Slate
        case 816u: { return vec3<f32>(0.85 + 0.2 * fbm(p * 2.5), 0.05, 6.0); }          // Concrete
        case 848u: {                                                                     // Brick
            let row = floor(p.y * 2.0);
            let bx = p.x + p.z + row * 0.5;
            let mortar = step(0.92, fract(p.y * 2.0)) + step(0.95, fract(bx));
            return vec3<f32>(mix(0.85 + 0.15 * hash3(vec3<f32>(floor(bx), row, 0.0)), 0.6, clamp(mortar, 0.0, 1.0)), 0.05, 6.0);
        }
        case 864u, 880u: { return vec3<f32>(0.7 + 0.4 * vnoise(p * 2.0), 0.1, 8.0); }  // Pebble, Cobblestone
        case 1040u: { return vec3<f32>(0.7 + 0.4 * fbm(p * 1.5), 0.25, 20.0); }         // CorrodedMetal
        case 1056u: {                                                                    // DiamondPlate
            let d = abs(fract(p.x + p.z) - 0.5) + abs(fract(p.x - p.z + p.y) - 0.5);
            return vec3<f32>(0.9 + 0.15 * step(0.8, d), 0.7, 60.0);
        }
        case 1072u, 1088u: { return vec3<f32>(0.95 + 0.1 * fbm(p * 3.0), 0.8, 50.0); } // Foil, Metal
        case 1280u, 1284u: { return vec3<f32>(0.8 + 0.3 * fbm(p * 1.3), 0.02, 4.0); }  // Grass
        case 1296u: { return vec3<f32>(0.9 + 0.12 * vnoise(p * 6.0), 0.02, 4.0); }     // Sand
        case 1312u: { return vec3<f32>(0.9 + 0.1 * sin(p.x * 20.0) * sin(p.z * 20.0), 0.0, 4.0); } // Fabric
        case 804u: { return vec3<f32>(0.85 + 0.2 * fbm(p * 0.6), 0.1, 8.0); }            // CrackedLava
        case 1536u: { return vec3<f32>(1.0, 0.9, 80.0); }                                // Ice
        case 1568u: { return vec3<f32>(1.0, 1.0, 120.0); }                               // Glass
        default: { return vec3<f32>(1.0, 0.18, 20.0); }                                  // Plastic
    }
}

fn shadow_factor(world: vec3<f32>, n: vec3<f32>) -> f32 {
    if (g.ambient.w < 0.5) { return 1.0; }
    let p = g.shadow_mat * vec4<f32>(world + n * 0.35, 1.0);
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 || p.z > 1.0) { return 1.0; }
    let ts = g.fog.w;
    var s = 0.0;
    for (var x = -1; x <= 1; x = x + 1) {
        for (var y = -1; y <= 1; y = y + 1) {
            s = s + textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(f32(x), f32(y)) * ts * 1.5, p.z - 0.0015);
        }
    }
    return s / 9.0;
}

fn face_of(n: vec3<f32>) -> u32 {
    let a = abs(n);
    if (a.x >= a.y && a.x >= a.z) { return select(3u, 0u, n.x > 0.0); }
    if (a.y >= a.z) { return select(4u, 1u, n.y > 0.0); }
    return select(5u, 2u, n.z > 0.0);
}

// Procedural stud / inlet relief. Returns (normal perturbation xy in face space, occlusion)
fn surface_relief(kind: u32, uv: vec2<f32>) -> vec3<f32> {
    let c = fract(uv) - vec2<f32>(0.5);
    let d = length(c);
    if (kind == 3u || kind == 5u) { // Studs / Universal
        let rim = smoothstep(0.24, 0.28, d) * (1.0 - smoothstep(0.28, 0.33, d));
        let top = 1.0 - smoothstep(0.22, 0.27, d);
        let dirv = c / max(d, 1e-4);
        let shade = top * 0.06 + rim * 0.0;
        return vec3<f32>(dirv * rim * 0.9, 1.0 + shade - rim * 0.12 * (0.5 + 0.5 * dirv.y));
    }
    if (kind == 4u) { // Inlet
        let hole = 1.0 - smoothstep(0.22, 0.27, d);
        let rim = smoothstep(0.2, 0.25, d) * (1.0 - smoothstep(0.25, 0.3, d));
        let dirv = c / max(d, 1e-4);
        return vec3<f32>(-dirv * rim * 0.9, 1.0 - hole * 0.25);
    }
    if (kind == 1u || kind == 2u) { // Glue / Weld: faint pattern
        let l = step(0.93, fract((uv.x + uv.y) * 2.0));
        return vec3<f32>(0.0, 0.0, 1.0 - l * 0.08);
    }
    return vec3<f32>(0.0, 0.0, 1.0);
}

fn sky_color(dir: vec3<f32>) -> vec3<f32> {
    let t = clamp(dir.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(g.fog_color.rgb * 0.8, mix(g.fog_color.rgb, vec3<f32>(0.55, 0.7, 0.95), 0.5), t);
}

@fragment
fn fs_main(v: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    let flags = v.extra.y;
    let surfaces = v.extra.x;
    let reflectance = bitcast<f32>(v.extra.z);
    var mat = u32(v.material + 0.5);
    if ((flags & 8u) != 0u) { mat = u32(v.uv.x + 0.5); }

    var base = v.color.rgb;
    var alpha = v.color.a;
    if ((flags & 2u) != 0u) { base = v.vcolor.rgb; }
    if ((flags & 4u) != 0u) {
        let t = textureSample(tex, smp, v.uv);
        base = t.rgb * v.color.rgb;
        alpha = alpha * t.a;
    }
    if (alpha < 0.01) { discard; }

    // double-sided parts (the avatar): the inside faces the other way
    var n = normalize(v.normal);
    if (!front) { n = -n; }
    var occl = 1.0;
    if ((flags & 1u) != 0u) {
        let f = face_of(v.objn);
        let kind = (surfaces >> (f * 4u)) & 15u;
        if (kind != 0u && kind != 10u) {
            var uv: vec2<f32>;
            let o = v.obj;
            if (f == 1u || f == 4u) { uv = o.xz; } else if (f == 0u || f == 3u) { uv = o.zy; } else { uv = o.xy; }
            // studs are aligned to the face edge
            let half = v.size * 0.5;
            if (f == 1u || f == 4u) { uv = uv + half.xz; } else if (f == 0u || f == 3u) { uv = uv + half.zy; } else { uv = uv + half.xy; }
            let r = surface_relief(kind, uv);
            occl = r.z;
            // perturb in world space using the face tangents
            var t1: vec3<f32>;
            var t2: vec3<f32>;
            if (f == 1u || f == 4u) { t1 = vec3<f32>(1.0, 0.0, 0.0); t2 = vec3<f32>(0.0, 0.0, 1.0); }
            else if (f == 0u || f == 3u) { t1 = vec3<f32>(0.0, 0.0, 1.0); t2 = vec3<f32>(0.0, 1.0, 0.0); }
            else { t1 = vec3<f32>(1.0, 0.0, 0.0); t2 = vec3<f32>(0.0, 1.0, 0.0); }
            // tangents into world: reuse the normal transform by building from the model basis is costly,
            // so approximate with screen-space derivatives of world position
            let dpx = dpdx(v.world);
            let dpy = dpdy(v.world);
            let duvx = dpdx(uv);
            let duvy = dpdy(uv);
            let det = duvx.x * duvy.y - duvx.y * duvy.x;
            if (abs(det) > 1e-8) {
                let T = normalize((dpx * duvy.y - dpy * duvx.y) / det);
                let B = normalize((dpy * duvx.x - dpx * duvy.x) / det);
                n = normalize(n + T * r.x + B * r.y);
            }
        }
    }

    let m = material(mat, v.obj);
    base = base * m.x;
    if ((flags & 8u) != 0u) {
        // terrain: mottled, slightly earthier tones like the textured terrain materials
        let n = fbm(v.world * 0.12);
        base = mix(base, base * vec3<f32>(1.0, 0.86, 0.7), 0.45) * (0.72 + 0.5 * n);
    }

    let view = normalize(g.cam_pos.xyz - v.world);
    let L = g.sun_dir.xyz;
    let ndl = dot(n, L);

    if (mat == 288u) {
        // Neon: self-lit and blooms
        let c = base * 1.9 + vec3<f32>(0.15);
        let dist = length(g.cam_pos.xyz - v.world);
        let fogf = clamp((dist - g.fog.x) / max(g.fog.y - g.fog.x, 1.0), 0.0, 1.0);
        return vec4<f32>(mix(c, g.fog_color.rgb, fogf * 0.7), alpha);
    }

    let sh = select(1.0, shadow_factor(v.world, n), ndl > 0.0) * g.fog.z;
    let sky_vis = 0.35 + 0.3 * clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    var light = g.ambient.rgb + g.outdoor.rgb * sky_vis;
    let sun_col = vec3<f32>(0.8) * g.sun_dir.w + g.shift_top.rgb * 0.7;
    light = light + sun_col * max(ndl, 0.0) * sh;
    light = light + g.shift_bottom.rgb * max(-ndl, 0.0);

    var spec = vec3<f32>(0.0);
    let h = normalize(L + view);
    spec = spec + sun_col * pow(max(dot(n, h), 0.0), m.z) * m.y * sh * step(0.0, ndl);

    let nl = u32(g.outdoor.w);
    for (var i = 0u; i < nl; i = i + 1u) {
        let li = g.lights[i];
        let d = li.pos_range.xyz - v.world;
        let dist = length(d);
        let rng = li.pos_range.w;
        if (dist > rng) { continue; }
        let ld = d / max(dist, 1e-4);
        var att = clamp(1.0 - dist / rng, 0.0, 1.0);
        att = att * att;
        let kind = li.color_kind.w;
        if (kind > 0.5) {
            // surface / spot: cone around the emitting direction
            let cone = dot(-ld, li.dir_angle.xyz);
            let cosA = cos(radians(li.dir_angle.w) * 0.5);
            if (cone < cosA) { continue; }
            att = att * smoothstep(cosA, min(cosA + 0.15, 1.0), cone);
        }
        let diff = max(dot(n, ld), 0.0);
        light = light + li.color_kind.rgb * diff * att;
        let hh = normalize(ld + view);
        spec = spec + li.color_kind.rgb * pow(max(dot(n, hh), 0.0), m.z) * m.y * att;
    }

    var c = base * light * occl + spec;
    if (reflectance > 0.0) {
        let r = reflect(-view, n);
        c = mix(c, sky_color(r) * (0.6 + 0.4 * sh), clamp(reflectance, 0.0, 1.0));
    }
    if (mat == 1568u) { alpha = alpha * 0.9; }
    if (mat == 804u) {
        // glowing cracks between the lava plates
        let f = fbm(v.world * 0.08);
        let crack = 1.0 - smoothstep(0.015, 0.05, abs(f - 0.5));
        c = mix(c, vec3<f32>(1.7, 0.55, 0.12), crack);
    }

    let dist = length(g.cam_pos.xyz - v.world);
    let fogf = clamp((dist - g.fog.x) / max(g.fog.y - g.fog.x, 1.0), 0.0, 1.0);
    c = mix(c, g.fog_color.rgb, fogf);
    return vec4<f32>(c, alpha);
}

// >> particles & decals
// textured, camera-facing or face-aligned quads in world space

struct PIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) emission: f32,
};
struct POut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) world: vec3<f32>,
    @location(3) emission: f32,
};

@vertex
fn vs_quad(v: PIn) -> POut {
    var o: POut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.uv = v.uv;
    o.color = v.color;
    o.world = v.pos;
    o.emission = v.emission;
    return o;
}

@fragment
fn fs_quad(v: POut) -> @location(0) vec4<f32> {
    let t = textureSample(tex, smp, v.uv);
    var c = t.rgb * v.color.rgb;
    let a = t.a * v.color.a;
    if (a < 0.003) { discard; }
    // particles are mostly unlit in this era; decals get the scene ambient
    let lit = mix(g.ambient.rgb + g.outdoor.rgb + vec3<f32>(0.6) * g.sun_dir.w, vec3<f32>(1.0), v.emission);
    c = c * lit;
    let dist = length(g.cam_pos.xyz - v.world);
    let fogf = clamp((dist - g.fog.x) / max(g.fog.y - g.fog.x, 1.0), 0.0, 1.0);
    c = mix(c, g.fog_color.rgb, fogf);
    return vec4<f32>(c, a);
}

// >> end << //

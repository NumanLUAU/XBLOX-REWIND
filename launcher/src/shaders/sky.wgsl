/*
    written by: @corescripts
*/

// >> sky
// per-face Roblox skybox textures in a 2D array (Bk, Dn, Ft, Lf, Rt, Up)

@group(1) @binding(0) var sky_tex: texture_2d_array<f32>;
@group(1) @binding(1) var sky_smp: sampler;

struct SkyOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) i: u32) -> SkyOut {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u)) * 2.0 - 1.0;
    var o: SkyOut;
    o.clip = vec4<f32>(p, 0.0, 1.0);
    o.ndc = p;
    return o;
}

struct SkyGlobals {
    inv_view_proj: mat4x4<f32>,
    sun: vec4<f32>,   // dir, angular size (deg)
    misc: vec4<f32>,  // celestial, loaded
    cam_pos: vec4<f32>,
};
@group(0) @binding(0) var<uniform> sg: SkyGlobals;

@fragment
fn fs_sky(v: SkyOut) -> @location(0) vec4<f32> {
    let far = sg.inv_view_proj * vec4<f32>(v.ndc, 0.0001, 1.0);
    let dir = normalize(far.xyz / far.w - sg.cam_pos.xyz);
    let a = abs(dir);
    var layer: i32;
    var uv: vec2<f32>;
    if (a.x >= a.y && a.x >= a.z) {
        // Roblox puts SkyboxLf on +X and SkyboxRt on -X; the Up image is turned 180 degrees
        // (found by matching the face edges of the Map's skies: the other way shows seams)
        if (dir.x > 0.0) { layer = 3; uv = vec2<f32>(dir.z, -dir.y) / a.x; }   // Lf
        else { layer = 4; uv = vec2<f32>(-dir.z, -dir.y) / a.x; }              // Rt
    } else if (a.y >= a.z) {
        if (dir.y > 0.0) { layer = 5; uv = vec2<f32>(-dir.z, -dir.x) / a.y; }  // Up
        else { layer = 1; uv = vec2<f32>(-dir.z, dir.x) / a.y; }               // Dn
    } else {
        if (dir.z > 0.0) { layer = 0; uv = vec2<f32>(-dir.x, -dir.y) / a.z; }  // Bk
        else { layer = 2; uv = vec2<f32>(dir.x, -dir.y) / a.z; }               // Ft
    }
    uv = uv * 0.5 + 0.5;
    var c = textureSampleLevel(sky_tex, sky_smp, uv, layer, 0.0).rgb;
    if (sg.misc.y < 0.5) {
        c = mix(vec3<f32>(0.42, 0.55, 0.75), vec3<f32>(0.2, 0.35, 0.65), clamp(dir.y, 0.0, 1.0));
    }
    if (sg.misc.x > 0.5) {
        let d = dot(dir, sg.sun.xyz);
        let r = radians(sg.sun.w) * 0.5 * 0.2;
        let disc = smoothstep(cos(r * 1.15), cos(r * 0.85), d);
        let halo = pow(max(d, 0.0), 300.0) * 0.6 + pow(max(d, 0.0), 30.0) * 0.12;
        c = c + vec3<f32>(1.0, 0.95, 0.85) * (disc * 1.5 + halo);
    }
    return vec4<f32>(c, 1.0);
}

// >> end << //

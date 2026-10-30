/*
    written by: @corescripts
*/

// Post: downsample, separable gaussian (BlurEffect), bloom and ColorCorrectionEffect.

struct Params {
    texel: vec2<f32>,
    dir: vec2<f32>,
    sigma: f32,
    threshold: f32,
    _p0: f32,
    _p1: f32,
};

struct CC {
    brightness: f32,
    contrast: f32,
    saturation: f32,
    blur_mix: f32,
    tint: vec4<f32>,
    bloom: vec4<f32>,
};

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var smp: sampler;
@group(0) @binding(2) var<uniform> p: Params;

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_full(@builtin(vertex_index) i: u32) -> VOut {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    var o: VOut;
    o.clip = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}

@fragment
fn fs_down(v: VOut) -> @location(0) vec4<f32> {
    let o = p.texel * 0.5;
    var c = textureSampleLevel(src, smp, v.uv + vec2<f32>(-o.x, -o.y), 0.0).rgb
          + textureSampleLevel(src, smp, v.uv + vec2<f32>(o.x, -o.y), 0.0).rgb
          + textureSampleLevel(src, smp, v.uv + vec2<f32>(-o.x, o.y), 0.0).rgb
          + textureSampleLevel(src, smp, v.uv + vec2<f32>(o.x, o.y), 0.0).rgb;
    c = c * 0.25;
    if (p.threshold > 0.0) {
        let l = max(c.r, max(c.g, c.b));
        c = c * smoothstep(p.threshold, p.threshold + 0.35, l);
    }
    return vec4<f32>(c, 1.0);
}

@fragment
fn fs_blur(v: VOut) -> @location(0) vec4<f32> {
    let sigma = max(p.sigma, 0.001);
    let radius = min(i32(ceil(sigma * 2.5)), 48);
    var acc = textureSampleLevel(src, smp, v.uv, 0.0).rgb;
    var wsum = 1.0;
    // linear-sampling trick: pairs of taps
    for (var i = 1; i <= radius; i = i + 2) {
        let w0 = exp(-f32(i * i) / (2.0 * sigma * sigma));
        let w1 = exp(-f32((i + 1) * (i + 1)) / (2.0 * sigma * sigma));
        let w = w0 + w1;
        let off = (f32(i) * w0 + f32(i + 1) * w1) / max(w, 1e-6);
        let d = p.dir * p.texel * off;
        acc = acc + (textureSampleLevel(src, smp, v.uv + d, 0.0).rgb + textureSampleLevel(src, smp, v.uv - d, 0.0).rgb) * w;
        wsum = wsum + 2.0 * w;
    }
    return vec4<f32>(acc / wsum, 1.0);
}

@group(1) @binding(0) var scene_tex: texture_2d<f32>;
@group(1) @binding(1) var blur_tex: texture_2d<f32>;
@group(1) @binding(2) var bloom_tex: texture_2d<f32>;
@group(1) @binding(3) var<uniform> cc: CC;

fn dither(pos: vec2<f32>) -> f32 {
    return fract(sin(dot(pos, vec2<f32>(12.9898, 78.233))) * 43758.5453) / 255.0;
}

@fragment
fn fs_composite(v: VOut) -> @location(0) vec4<f32> {
    let sharp = textureSampleLevel(scene_tex, smp, v.uv, 0.0).rgb;
    let blurred = textureSampleLevel(blur_tex, smp, v.uv, 0.0).rgb;
    // bloom.y > 0: the filter fades out towards a clear ellipse in the middle (avatar editor)
    let d = length((v.uv - vec2<f32>(0.5, 0.5)) * vec2<f32>(1.7778, 1.0));
    var m = 1.0;
    if (cc.bloom.y > 0.0) {
        m = smoothstep(cc.bloom.y * 0.45, cc.bloom.y, d);
    }
    var c = mix(sharp, blurred, cc.blur_mix * m);
    c = c + textureSampleLevel(bloom_tex, smp, v.uv, 0.0).rgb * cc.bloom.x;
    let raw = c;
    // ColorCorrectionEffect
    let lum = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    c = mix(vec3<f32>(lum), c, 1.0 + cc.saturation);
    c = (c - 0.5) * (1.0 + cc.contrast) + 0.5;
    c = c + cc.brightness;
    c = c * cc.tint.rgb;
    c = mix(raw, c, m);
    c = clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)) + dither(v.clip.xy);
    return vec4<f32>(c, 1.0);
}

// >> end << //

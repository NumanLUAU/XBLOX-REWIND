/*
    written by: @corescripts
*/

struct U {
    screen: vec2<f32>,
    _p: vec2<f32>,
};
@group(0) @binding(0) var<uniform> u: U;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var smp: sampler;

struct VIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.clip = vec4<f32>(v.pos.x / u.screen.x * 2.0 - 1.0, 1.0 - v.pos.y / u.screen.y * 2.0, 0.0, 1.0);
    o.uv = v.uv;
    o.color = v.color;
    return o;
}

@fragment
fn fs_main(v: VOut) -> @location(0) vec4<f32> {
    return textureSample(tex, smp, v.uv) * v.color;
}

// >> end << //

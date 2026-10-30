/*
    written by: @corescripts
*/

//! The background world: parts built from Map.rbxl, the zone animations from
//! CameraManagerZone_*.lua, the camera flythrough from CameraManager.lua and the
//! per-zone lighting from CameraManager_ZoneManager's SkyboxManager.

// >> imports
use crate::rbxl::{content_id, CFrame, Dom, Value};
use glam::{Vec2, Vec3};
use rand::Rng;
use std::collections::HashMap;
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum MeshKey {
    Box,
    Wedge,
    CornerWedge,
    CylX,
    CylY,
    Sphere,
    Head,
    Asset(u64),
    Csg(u64),
    Terrain(u32),
    /// meshes built by the avatar loader (already scaled to the part)
    Avatar(u32),
}

pub const FLAG_STUDS: u32 = 1;
pub const FLAG_VERTEX_COLOR: u32 = 2;
pub const FLAG_TEXTURED: u32 = 4;
/// material id comes from the vertex (smooth terrain)
pub const FLAG_TERRAIN: u32 = 8;

/// SmoothGrid blob, MaterialColors, WaterColor, WaterTransparency
pub struct TerrainSource {
    pub grid: Vec<u8>,
    pub colors: Vec<u8>,
    pub water_color: Vec3,
    pub water_transparency: f32,
}

pub struct Part {
    pub inst: usize,
    pub cf: CFrame,
    pub size: Vec3,
    pub mesh: MeshKey,
    /// Scale applied to mesh vertices (part size for unit primitives).
    pub mesh_scale: Vec3,
    pub mesh_offset: Vec3,
    /// Size used if an asset mesh fails to load and we fall back to a box.
    pub fallback_scale: Vec3,
    pub color: Vec3,
    pub alpha: f32,
    pub reflectance: f32,
    pub material: u32,
    pub surfaces: u32,
    pub flags: u32,
    pub texture: Option<String>,
    pub visible: bool,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LightKind {
    Point,
    Surface,
    Spot,
}

pub struct Light {
    pub part: usize,
    pub kind: LightKind,
    pub color: Vec3,
    pub brightness: f32,
    pub range: f32,
    pub face: u32,
    pub angle: f32,
}

pub struct Decal {
    pub part: usize,
    pub face: u32,
    pub texture: String,
    pub color: Vec3,
    pub alpha: f32,
    /// Some for `Texture` (studs per tile), None for `Decal` (stretched).
    pub tile: Option<Vec2>,
}

pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
    pub life: f32,
    pub rot: f32,
    pub rot_speed: f32,
}

pub struct Emitter {
    pub part: usize,
    pub texture: String,
    pub color: Vec<(f32, Vec3)>,
    pub size: Vec<(f32, f32, f32)>,
    pub transparency: Vec<(f32, f32, f32)>,
    pub lifetime: (f32, f32),
    pub rate: f32,
    pub speed: (f32, f32),
    pub spread: Vec2,
    pub accel: Vec3,
    pub drag: f32,
    pub rotation: (f32, f32),
    pub rot_speed: (f32, f32),
    pub light_emission: f32,
    pub z_offset: f32,
    pub direction: u32,
    pub particles: Vec<Particle>,
    accum: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Lighting {
    pub ambient: Vec3,
    pub outdoor_ambient: Vec3,
    pub brightness: f32,
    pub shift_top: Vec3,
    pub shift_bottom: Vec3,
    pub fog_color: Vec3,
    pub fog_start: f32,
    pub fog_end: f32,
    pub clock: f32,
    pub latitude: f32,
    pub shadows: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Sky {
    /// Bk, Dn, Ft, Lf, Rt, Up
    pub faces: [String; 6],
    pub celestial: bool,
    pub sun_size: f32,
}

/// A group of parts moved rigidly relative to a base CFrame (CFrameOffsetter in the Lua).
struct Rig {
    parts: Vec<usize>,
    offsets: Vec<CFrame>,
}

impl Rig {
    fn apply(&self, parts: &mut [Part], cf: &CFrame) {
        for (i, &p) in self.parts.iter().enumerate() {
            parts[p].cf = cf.mul(&self.offsets[i]);
        }
    }
}

struct Track {
    cf0: CFrame,
    length: f32,
}

struct Flow {
    /// parts[i] = (part index, children offsets)
    nodes: Vec<(usize, Vec<(usize, CFrame)>)>,
    cframes: Vec<CFrame>,
    length: f32,
    cycle: usize,
    unit_offset: f32,
    t0: f32,
}

struct ZoneAnims {
    station: Option<(CFrame, Vec<Rig>)>,
    orbit: Option<(CFrame, Rig, Vec<(CFrame, Rig)>)>,
    trains: Vec<(Rig, Option<usize>, f32, f32)>,
    tracks: Vec<Track>,
    flows: Vec<Flow>,
    lightning: Vec<Vec<usize>>,
    lightning_lights: Vec<Vec<usize>>,
    flash: Option<(usize, f32)>,
    next_flash: f32,
}

pub struct Scene {
    pub parts: Vec<Part>,
    pub lights: Vec<Light>,
    pub decals: Vec<Decal>,
    pub emitters: Vec<Emitter>,
    pub base_lighting: Lighting,
    pub zone_lighting: HashMap<String, (Lighting, Sky)>,
    pub lighting: Lighting,
    pub sky: Sky,
    /// Sky under Lighting itself (Lobby3D has one, Map.rbxl relies on Skyboxes)
    pub base_sky: Sky,
    pub zone: String,
    pub camera_sets: HashMap<String, Vec<CFrame>>,
    pub terrain: Option<TerrainSource>,
    part_by_inst: HashMap<usize, usize>,
    anims: ZoneAnims,
    zone_t0: f32,
}

fn color_of(dom: &Dom, i: usize) -> Vec3 {
    let inst = &dom.instances[i];
    if let Some(Value::Color3(c)) = inst.get("Color3uint8") {
        return *c;
    }
    if let Some(Value::BrickColor(b)) = inst.get("BrickColor") {
        return brick_color(*b);
    }
    Vec3::splat(0.64)
}

fn brick_color(n: u32) -> Vec3 {
    let (r, g, b) = match n {
        1 => (242, 243, 243),
        5 => (215, 197, 154),
        21 => (196, 40, 28),
        23 => (13, 105, 172),
        24 => (245, 205, 48),
        26 => (27, 42, 53),
        28 => (40, 127, 71),
        37 => (75, 151, 75),
        106 => (218, 133, 65),
        119 => (164, 189, 71),
        141 => (39, 70, 45),
        192 => (105, 64, 40),
        194 => (163, 162, 165),
        199 => (99, 95, 98),
        208 => (229, 228, 223),
        1001 => (248, 248, 248),
        1003 => (17, 17, 17),
        _ => (163, 162, 165),
    };
    Vec3::new(r as f32, g as f32, b as f32) / 255.0
}

fn is_base_part(class: &str) -> bool {
    matches!(
        class,
        "Part" | "WedgePart" | "CornerWedgePart" | "TrussPart" | "MeshPart" | "UnionOperation" | "Seat" | "SpawnLocation" | "VehicleSeat" | "SkateboardPlatform" | "NegateOperation"
    )
}

fn parse_time(s: &str) -> f32 {
    let mut it = s.split(':').map(|p| p.parse::<f32>().unwrap_or(0.0));
    let h = it.next().unwrap_or(14.0);
    let m = it.next().unwrap_or(0.0);
    let sec = it.next().unwrap_or(0.0);
    h + m / 60.0 + sec / 3600.0
}

fn seq_f(v: Option<&Value>, default: f32) -> Vec<(f32, f32, f32)> {
    match v {
        Some(Value::NumberSequence(s)) if !s.is_empty() => s.clone(),
        Some(Value::Float(f)) => vec![(0.0, *f, 0.0), (1.0, *f, 0.0)],
        _ => vec![(0.0, default, 0.0), (1.0, default, 0.0)],
    }
}

pub fn eval_num(seq: &[(f32, f32, f32)], t: f32) -> f32 {
    for w in seq.windows(2) {
        if t <= w[1].0 {
            let f = ((t - w[0].0) / (w[1].0 - w[0].0).max(1e-5)).clamp(0.0, 1.0);
            return w[0].1 + (w[1].1 - w[0].1) * f;
        }
    }
    seq.last().map(|s| s.1).unwrap_or(0.0)
}

pub fn eval_color(seq: &[(f32, Vec3)], t: f32) -> Vec3 {
    for w in seq.windows(2) {
        if t <= w[1].0 {
            let f = ((t - w[0].0) / (w[1].0 - w[0].0).max(1e-5)).clamp(0.0, 1.0);
            return w[0].1.lerp(w[1].1, f);
        }
    }
    seq.last().map(|s| s.1).unwrap_or(Vec3::ONE)
}

fn range(v: Option<&Value>, d: f32) -> (f32, f32) {
    match v {
        Some(Value::NumberRange(a, b)) => (*a, *b),
        Some(Value::Float(f)) => (*f, *f),
        _ => (d, d),
    }
}

pub fn face_normal(face: u32) -> Vec3 {
    match face {
        0 => Vec3::X,
        1 => Vec3::Y,
        2 => Vec3::Z,
        3 => -Vec3::X,
        4 => -Vec3::Y,
        _ => -Vec3::Z,
    }
}

impl Scene {
    pub fn build(dom: &Dom) -> Scene {
        let ws = dom.find_root("Workspace").expect("place has no Workspace");
        Self::build_from(dom, ws)
    }

    /// Build from any model in the place (the avatar editor room lives in ReplicatedStorage).
    pub fn build_from(dom: &Dom, ws: usize) -> Scene {
        let cameras = dom.find_child(ws, "Cameras");
        let mut parts = Vec::new();
        let mut part_by_inst = HashMap::new();
        let mut lights = Vec::new();
        let mut decals = Vec::new();
        let mut emitters = Vec::new();

        let mut all = Vec::new();
        dom.descendants(ws, &mut all);
        for &i in &all {
            let inst = &dom.instances[i];
            if !is_base_part(&inst.class) {
                continue;
            }
            let hidden = cameras.map(|c| dom.is_ancestor(c, i)).unwrap_or(false);
            if let Some(p) = build_part(dom, i, hidden) {
                part_by_inst.insert(i, parts.len());
                parts.push(p);
            }
        }

        for &i in &all {
            let inst = &dom.instances[i];
            let Some(parent) = inst.parent else { continue };
            let Some(&pi) = part_by_inst.get(&parent) else { continue };
            match inst.class.as_str() {
                "PointLight" | "SurfaceLight" | "SpotLight" => {
                    if inst.bool("Enabled") == Some(false) {
                        continue;
                    }
                    lights.push(Light {
                        part: pi,
                        kind: match inst.class.as_str() {
                            "PointLight" => LightKind::Point,
                            "SurfaceLight" => LightKind::Surface,
                            _ => LightKind::Spot,
                        },
                        color: inst.vec3("Color").unwrap_or(Vec3::ONE),
                        brightness: inst.f32("Brightness").unwrap_or(1.0),
                        range: inst.f32("Range").unwrap_or(8.0),
                        face: inst.enum_("Face").unwrap_or(5),
                        angle: inst.f32("Angle").unwrap_or(90.0),
                    });
                }
                "Decal" | "Texture" => {
                    let Some(tex) = content_id(inst.get("Texture")) else { continue };
                    let tile = (inst.class == "Texture").then(|| {
                        Vec2::new(inst.f32("StudsPerTileU").unwrap_or(2.0), inst.f32("StudsPerTileV").unwrap_or(2.0))
                    });
                    decals.push(Decal {
                        part: pi,
                        face: inst.enum_("Face").unwrap_or(5),
                        texture: tex,
                        color: inst.vec3("Color3").unwrap_or(Vec3::ONE),
                        alpha: 1.0 - inst.f32("Transparency").unwrap_or(0.0),
                        tile,
                    });
                }
                "ParticleEmitter" => {
                    if inst.bool("Enabled") == Some(false) {
                        continue;
                    }
                    let color = match inst.get("Color") {
                        Some(Value::ColorSequence(s)) if !s.is_empty() => s.clone(),
                        _ => vec![(0.0, Vec3::ONE), (1.0, Vec3::ONE)],
                    };
                    emitters.push(Emitter {
                        part: pi,
                        texture: content_id(inst.get("Texture")).unwrap_or_else(|| "rbxasset://textures/particles/sparkles_main.dds".into()),
                        color,
                        size: seq_f(inst.get("Size"), 1.0),
                        transparency: seq_f(inst.get("Transparency"), 0.0),
                        lifetime: range(inst.get("Lifetime"), 5.0),
                        rate: inst.f32("Rate").unwrap_or(20.0),
                        speed: range(inst.get("Speed"), 5.0),
                        spread: match inst.get("SpreadAngle") {
                            Some(Value::Vector2(v)) => *v,
                            _ => Vec2::ZERO,
                        },
                        accel: inst.vec3("Acceleration").unwrap_or(Vec3::ZERO),
                        drag: inst.f32("Drag").unwrap_or(0.0),
                        rotation: range(inst.get("Rotation"), 0.0),
                        rot_speed: range(inst.get("RotSpeed"), 0.0),
                        light_emission: inst.f32("LightEmission").unwrap_or(0.0),
                        z_offset: inst.f32("ZOffset").unwrap_or(0.0),
                        direction: inst.enum_("EmissionDirection").unwrap_or(1),
                        particles: Vec::new(),
                        accum: 0.0,
                    });
                }
                _ => {}
            }
        }

        // camera paths: Workspace.Cameras.<Zone>.Cam<N>
        let mut camera_sets = HashMap::new();
        if let Some(c) = cameras {
            for &f in &dom.instances[c].children {
                let mut cams: Vec<(u32, CFrame)> = dom.instances[f]
                    .children
                    .iter()
                    .filter_map(|&p| {
                        let n = dom.instances[p].name.get(3..)?.parse().ok()?;
                        Some((n, dom.instances[p].cframe("CFrame")?))
                    })
                    .collect();
                cams.sort_by_key(|c| c.0);
                camera_sets.insert(dom.instances[f].name.clone(), cams.into_iter().map(|c| c.1).collect());
            }
        }

        // lighting
        let lighting_inst = dom.find_root("Lighting").map(|l| &dom.instances[l]);
        let base = {
            let l = lighting_inst;
            let g = |k: &str, d: Vec3| l.and_then(|l| l.vec3(k)).unwrap_or(d);
            let f = |k: &str, d: f32| l.and_then(|l| l.f32(k)).unwrap_or(d);
            Lighting {
                ambient: g("Ambient", Vec3::splat(0.5)),
                outdoor_ambient: g("OutdoorAmbient", Vec3::splat(0.5)),
                brightness: f("Brightness", 1.0),
                shift_top: g("ColorShift_Top", Vec3::ZERO),
                shift_bottom: g("ColorShift_Bottom", Vec3::ZERO),
                fog_color: g("FogColor", Vec3::splat(0.75)),
                fog_start: f("FogStart", 0.0),
                fog_end: f("FogEnd", 100000.0),
                clock: l.and_then(|l| l.string("TimeOfDay")).map(|s| parse_time(&s)).or_else(|| l.and_then(|l| l.f32("ClockTime"))).unwrap_or(14.0),
                latitude: f("GeographicLatitude", 41.73),
                shadows: l.and_then(|l| l.bool("GlobalShadows")).unwrap_or(true),
            }
        };
        let sky_of = |si: &crate::rbxl::Instance| {
            let face = |k: &str| si.string(k).unwrap_or_default();
            Sky {
                faces: [face("SkyboxBk"), face("SkyboxDn"), face("SkyboxFt"), face("SkyboxLf"), face("SkyboxRt"), face("SkyboxUp")],
                celestial: si.bool("CelestialBodiesShown").unwrap_or(true),
                sun_size: si.f32("SunAngularSize").unwrap_or(21.0),
            }
        };
        let base_sky = dom
            .find_root("Lighting")
            .and_then(|l| dom.instances[l].children.iter().map(|&c| &dom.instances[c]).find(|c| c.class == "Sky"))
            .map(sky_of)
            .unwrap_or_else(default_sky);
        let mut zone_lighting = HashMap::new();
        let skyboxes = dom.find_root("ReplicatedStorage").and_then(|r| dom.find_child(r, "Skyboxes"));
        if let Some(sb) = skyboxes {
            for &s in &dom.instances[sb].children {
                let si = &dom.instances[s];
                let mut lt = base;
                for &v in &si.children {
                    let vi = &dom.instances[v];
                    let val = vi.get("Value");
                    match (vi.name.as_str(), val) {
                        ("Ambient", Some(Value::Color3(c))) => lt.ambient = *c,
                        ("OutdoorAmbient", Some(Value::Color3(c))) => lt.outdoor_ambient = *c,
                        ("ColorShift_Top", Some(Value::Color3(c))) => lt.shift_top = *c,
                        ("ColorShift_Bottom", Some(Value::Color3(c))) => lt.shift_bottom = *c,
                        ("FogColor", Some(Value::Color3(c))) => lt.fog_color = *c,
                        ("FogEnd", _) => lt.fog_end = vi.f32("Value").unwrap_or(lt.fog_end),
                        ("FogStart", _) => lt.fog_start = vi.f32("Value").unwrap_or(lt.fog_start),
                        ("Brightness", _) => lt.brightness = vi.f32("Value").unwrap_or(lt.brightness),
                        ("ClockTime", _) => lt.clock = vi.f32("Value").unwrap_or(lt.clock),
                        ("TimeOfDay", _) => {
                            if let Some(s) = vi.string("Value") {
                                lt.clock = parse_time(&s)
                            }
                        }
                        ("GlobalShadows", Some(Value::Bool(b))) => lt.shadows = *b,
                        _ => {}
                    }
                }
                let sky = if si.class == "Sky" {
                    sky_of(si)
                } else {
                    default_sky()
                };
                zone_lighting.insert(si.name.clone(), (lt, sky));
            }
        }

        let anims = build_anims(dom, ws, &mut parts, &part_by_inst, &lights);
        let terrain = dom.find_child(ws, "Terrain").and_then(|t| {
            let ti = &dom.instances[t];
            let grid = ti.get("SmoothGrid").and_then(|v| dom.shared(v)).map(|b| b.to_vec())?;
            let colors = ti.get("MaterialColors").and_then(|v| dom.shared(v)).map(|b| b.to_vec()).unwrap_or_default();
            Some(TerrainSource {
                grid,
                colors,
                water_color: ti.vec3("WaterColor").unwrap_or(Vec3::new(0.05, 0.33, 0.36)),
                water_transparency: ti.f32("WaterTransparency").unwrap_or(0.3),
            })
        });

        Scene {
            parts,
            lights,
            decals,
            emitters,
            base_lighting: base,
            zone_lighting,
            lighting: base,
            sky: default_sky(),
            base_sky,
            zone: String::new(),
            camera_sets,
            terrain,
            part_by_inst,
            anims,
            zone_t0: 0.0,
        }
    }

    /// Add a smooth-terrain chunk mesh as a static part centred on `center`.
    pub fn add_terrain_part(&mut self, key: MeshKey, center: Vec3, water: bool) {
        let (color, alpha) = match &self.terrain {
            Some(t) if water => (t.water_color, 1.0 - t.water_transparency),
            _ => (Vec3::ONE, 1.0),
        };
        self.parts.push(Part {
            inst: usize::MAX,
            cf: CFrame::new(center),
            size: Vec3::splat(128.0 * 1.8),
            mesh: key,
            mesh_scale: Vec3::ONE,
            mesh_offset: Vec3::ZERO,
            fallback_scale: Vec3::ZERO,
            color,
            alpha,
            reflectance: if water { 0.35 } else { 0.0 },
            material: if water { 272 } else { 256 },
            surfaces: 0,
            flags: if water { 0 } else { FLAG_TERRAIN | FLAG_VERTEX_COLOR },
            texture: None,
            visible: true,
        });
    }

    pub fn set_zone(&mut self, zone: &str, now: f32) {
        if self.zone == zone {
            return;
        }
        self.zone = zone.to_string();
        self.zone_t0 = now;
        if let Some((l, s)) = self.zone_lighting.get(zone) {
            self.lighting = *l;
            self.sky = s.clone();
        } else {
            self.lighting = self.base_lighting;
            self.sky = default_sky();
        }
    }

    /// Places without zones (Lobby3D): light the whole scene with the Lighting service and its Sky.
    pub fn use_base_zone(&mut self, name: &str) {
        self.zone_lighting.insert(name.to_string(), (self.base_lighting, self.base_sky.clone()));
        self.set_zone(name, 0.0);
    }

    pub fn sun_direction(&self) -> Vec3 {
        sun_direction(self.lighting.clock, self.lighting.latitude)
    }

    pub fn part_index(&self, inst: usize) -> Option<usize> {
        self.part_by_inst.get(&inst).copied()
    }

    /// Run the per-zone animators. `t` is seconds since the app started.
    pub fn update(&mut self, now: f32, dt: f32) {
        let t = now - self.zone_t0;
        let a = &mut self.anims;
        match self.zone.as_str() {
            "Space" => {
                if let Some((base, rigs)) = &a.station {
                    let m = |x: f32| x % (PI * 2.0);
                    let cfs = [
                        CFrame::angles(0.0, m(t / 6.0), 0.0).mul(&CFrame::new(Vec3::new(0.0, (m(t / 4.0)).sin(), 0.0))),
                        CFrame::angles(0.0, -m(t / 4.0), 0.0).mul(&CFrame::new(Vec3::new(0.0, (2.0 + m(t / 2.0)).sin(), 0.0))),
                        CFrame::angles(0.0, m(t / 3.5), 0.0).mul(&CFrame::new(Vec3::new(0.0, (1.0 + m(t / 2.0)).sin(), 0.0))),
                        CFrame::angles(0.0, m(t / -32.0), 0.0).mul(&CFrame::new(Vec3::new(0.0, m(t / 4.0).sin() * 3.0 - 2.0, 0.0))),
                    ];
                    for (rig, cf) in rigs.iter().zip(cfs.iter()) {
                        rig.apply(&mut self.parts, &base.mul(cf));
                    }
                }
                if let Some((base, moons, disks)) = &a.orbit {
                    let m = |x: f32| x % (PI * 2.0);
                    let cf = CFrame::angles(m(t / 80.0), m(t / 60.0), m(t / 40.0));
                    moons.apply(&mut self.parts, &base.mul(&cf));
                    for (dbase, rig) in disks {
                        rig.apply(&mut self.parts, &dbase.mul(&CFrame::angles(0.0, 0.25 + m(t / 60.0), 0.0)));
                    }
                }
            }
            "City" => {
                // renderTrains: (track index, phase, period) per train
                for (rig, track, phase, period) in &a.trains {
                    let Some(ti) = track else { continue };
                    let Some(tr) = a.tracks.get(*ti) else { continue };
                    let time = (t - phase).rem_euclid(*period);
                    let dist = time * 48.0;
                    if dist >= 0.0 && dist <= tr.length {
                        rig.apply(&mut self.parts, &tr.cf0.mul(&CFrame::new(Vec3::new(0.0, dist, 0.0))));
                    }
                }
                for f in &mut a.flows {
                    update_flow(f, &mut self.parts, t);
                }
            }
            "Volcano" => {
                if let Some((i, until)) = a.flash {
                    if t >= until {
                        for &p in &a.lightning[i] {
                            self.parts[p].visible = false;
                        }
                        a.flash = None;
                    }
                }
                if t >= a.next_flash && !a.lightning.is_empty() {
                    let mut rng = rand::thread_rng();
                    let i = rng.gen_range(0..a.lightning.len());
                    for &p in &a.lightning[i] {
                        self.parts[p].visible = true;
                    }
                    a.flash = Some((i, t + 0.125));
                    a.next_flash = t + rng.gen::<f32>() * 16.0 + 1.0;
                }
            }
            _ => {}
        }
        self.update_particles(dt);
    }

    /// Lights that should currently contribute (lightning lights only while flashing).
    pub fn light_active(&self, li: usize) -> bool {
        let a = &self.anims;
        for (i, group) in a.lightning_lights.iter().enumerate() {
            if group.contains(&li) {
                return a.flash.map(|f| f.0) == Some(i) && self.zone == "Volcano";
            }
        }
        true
    }

    fn update_particles(&mut self, dt: f32) {
        let mut rng = rand::thread_rng();
        for e in &mut self.emitters {
            let part = &self.parts[e.part];
            for p in &mut e.particles {
                p.age += dt;
                p.vel += e.accel * dt;
                if e.drag > 0.0 {
                    p.vel *= (-e.drag * dt).exp2();
                }
                p.pos += p.vel * dt;
                p.rot += p.rot_speed * dt;
            }
            e.particles.retain(|p| p.age < p.life);
            e.accum += e.rate * dt;
            let n = e.accum.floor() as usize;
            e.accum -= n as f32;
            let n = n.min(64);
            let dir_local = face_normal(e.direction);
            for _ in 0..n {
                if e.particles.len() > 400 {
                    break;
                }
                // spawn on the emitting face
                let h = part.size * 0.5;
                let mut lp = Vec3::new(rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0), rng.gen_range(-1.0..1.0)) * h;
                let a = dir_local.abs();
                let axis = if a.x >= a.y && a.x >= a.z { 0 } else if a.y >= a.z { 1 } else { 2 };
                lp[axis] = dir_local[axis] * h[axis];
                let pos = part.cf.point(lp);
                let sx = (rng.gen::<f32>() - 0.5) * e.spread.x.to_radians();
                let sy = (rng.gen::<f32>() - 0.5) * e.spread.y.to_radians();
                let local_dir = glam::Quat::from_euler(glam::EulerRot::XYZ, sy, 0.0, sx) * dir_local;
                let dir = part.cf.rot * local_dir;
                e.particles.push(Particle {
                    pos,
                    vel: dir * rng.gen_range(e.speed.0..=e.speed.1.max(e.speed.0)),
                    age: 0.0,
                    life: rng.gen_range(e.lifetime.0..=e.lifetime.1.max(e.lifetime.0)).max(0.05),
                    rot: rng.gen_range(e.rotation.0..=e.rotation.1.max(e.rotation.0)).to_radians(),
                    rot_speed: rng.gen_range(e.rot_speed.0..=e.rot_speed.1.max(e.rot_speed.0)).to_radians(),
                });
            }
        }
    }
}

fn default_sky() -> Sky {
    let f = |s: &str| format!("rbxasset://sky/null_plainsky512_{s}.jpg");
    Sky { faces: [f("bk"), f("dn"), f("ft"), f("lf"), f("rt"), f("up")], celestial: true, sun_size: 21.0 }
}

/// Approximation of Roblox's sun position for a clock time and latitude.
pub fn sun_direction(clock: f32, latitude: f32) -> Vec3 {
    let h = (clock / 24.0) * 2.0 * PI - PI;
    let lat = latitude.to_radians();
    let decl = 23.5f32.to_radians() * 0.4;
    let y = h.cos() * decl.cos() * lat.cos() + decl.sin() * lat.sin();
    let x = -h.sin() * decl.cos();
    let z = h.cos() * decl.cos() * lat.sin() - decl.sin() * lat.cos();
    Vec3::new(x, y, z).normalize()
}

fn build_part(dom: &Dom, i: usize, hidden: bool) -> Option<Part> {
    let inst = &dom.instances[i];
    let cf = inst.cframe("CFrame")?;
    let size = inst.vec3("size").or_else(|| inst.vec3("Size")).unwrap_or(Vec3::new(4.0, 1.2, 2.0));
    let transparency = inst.f32("Transparency").unwrap_or(0.0);
    let mut flags = 0;
    let mut texture = None;
    let mut mesh_offset = Vec3::ZERO;
    let mut color = color_of(dom, i);

    let (mut mesh, mut mesh_scale) = match inst.class.as_str() {
        "WedgePart" => (MeshKey::Wedge, size),
        "CornerWedgePart" => (MeshKey::CornerWedge, size),
        "MeshPart" => {
            let id = content_id(inst.get("MeshId")).or_else(|| content_id(inst.get("MeshID"))).and_then(|s| crate::assets::parse_asset_id(&s));
            if let Some(t) = content_id(inst.get("TextureID")) {
                texture = Some(t);
                flags |= FLAG_TEXTURED;
            }
            let init = inst.vec3("InitialSize").unwrap_or(size);
            match id {
                Some(id) => (MeshKey::Asset(id), size / init.max(Vec3::splat(1e-4))),
                None => (MeshKey::Box, size),
            }
        }
        "UnionOperation" | "NegateOperation" => {
            if inst.class == "NegateOperation" {
                return None;
            }
            let id = content_id(inst.get("AssetId")).and_then(|s| crate::assets::parse_asset_id(&s));
            let init = inst.vec3("InitialSize").unwrap_or(size);
            if inst.bool("UsePartColor") == Some(false) {
                flags |= FLAG_VERTEX_COLOR;
            }
            match id {
                Some(id) => (MeshKey::Csg(id), size / init.max(Vec3::splat(1e-4))),
                None => (MeshKey::Box, size),
            }
        }
        _ => {
            let shape = inst.enum_("shape").or_else(|| inst.enum_("Shape")).unwrap_or(1);
            match shape {
                0 => (MeshKey::Sphere, Vec3::splat(size.min_element())),
                2 => {
                    let d = size.y.min(size.z);
                    (MeshKey::CylX, Vec3::new(size.x, d, d))
                }
                3 => (MeshKey::Wedge, size),
                4 => (MeshKey::CornerWedge, size),
                _ => {
                    flags |= FLAG_STUDS;
                    (MeshKey::Box, size)
                }
            }
        }
    };

    // mesh children override the visual shape
    for &c in &inst.children {
        let ci = &dom.instances[c];
        let scale = ci.vec3("Scale").unwrap_or(Vec3::ONE);
        let offset = ci.vec3("Offset").unwrap_or(Vec3::ZERO);
        match ci.class.as_str() {
            "SpecialMesh" => {
                flags &= !FLAG_STUDS;
                mesh_offset = offset;
                let ty = ci.enum_("MeshType").unwrap_or(5);
                let (m, s) = match ty {
                    0 => (MeshKey::Head, size * scale),
                    1 | 6 => (MeshKey::Box, size * scale),
                    2 => (MeshKey::Wedge, size * scale),
                    3 => (MeshKey::Sphere, size * scale),
                    4 => (MeshKey::CylX, size * scale),
                    5 => {
                        let id = content_id(ci.get("MeshId")).and_then(|s| crate::assets::parse_asset_id(&s));
                        if let Some(t) = content_id(ci.get("TextureId")) {
                            texture = Some(t);
                            flags |= FLAG_TEXTURED;
                            if let Some(vc) = ci.vec3("VertexColor") {
                                color = vc;
                            }
                        }
                        match id {
                            Some(id) => (MeshKey::Asset(id), scale),
                            None => (MeshKey::Box, size * scale),
                        }
                    }
                    _ => (MeshKey::Box, size * scale),
                };
                mesh = m;
                mesh_scale = s;
            }
            "CylinderMesh" => {
                flags &= !FLAG_STUDS;
                mesh_offset = offset;
                let d = size.x.min(size.z);
                mesh = MeshKey::CylY;
                mesh_scale = Vec3::new(d, size.y, d) * scale;
            }
            "BlockMesh" => {
                flags &= !FLAG_STUDS;
                mesh_offset = offset;
                mesh = MeshKey::Box;
                mesh_scale = size * scale;
            }
            _ => {}
        }
    }

    let surf = |k: &str, d: u32| inst.enum_(k).unwrap_or(d) & 15;
    // NormalId order: Right, Top, Back, Left, Bottom, Front
    let surfaces = surf("RightSurface", 0)
        | surf("TopSurface", 0) << 4
        | surf("BackSurface", 0) << 8
        | surf("LeftSurface", 0) << 12
        | surf("BottomSurface", 0) << 16
        | surf("FrontSurface", 0) << 20;

    Some(Part {
        inst: i,
        cf,
        size,
        mesh,
        mesh_scale,
        mesh_offset,
        fallback_scale: size,
        color,
        alpha: 1.0 - transparency,
        reflectance: inst.f32("Reflectance").unwrap_or(0.0),
        material: inst.enum_("Material").unwrap_or(256),
        surfaces,
        flags,
        texture,
        visible: !hidden && transparency < 0.999,
    })
}

fn collect_parts(dom: &Dom, root: usize, map: &HashMap<usize, usize>) -> Vec<usize> {
    let mut d = vec![root];
    dom.descendants(root, &mut d);
    d.into_iter().filter_map(|i| map.get(&i).copied()).collect()
}

fn rig(dom: &Dom, root: usize, base: &CFrame, parts: &[Part], map: &HashMap<usize, usize>) -> Rig {
    let ps = collect_parts(dom, root, map);
    let inv = base.inverse();
    let offsets = ps.iter().map(|&p| inv.mul(&parts[p].cf)).collect();
    Rig { parts: ps, offsets }
}

fn build_anims(dom: &Dom, ws: usize, parts: &mut Vec<Part>, map: &HashMap<usize, usize>, lights: &[Light]) -> ZoneAnims {
    let mut a = ZoneAnims {
        station: None,
        orbit: None,
        trains: Vec::new(),
        tracks: Vec::new(),
        flows: Vec::new(),
        lightning: Vec::new(),
        lightning_lights: Vec::new(),
        flash: None,
        next_flash: 3.0,
    };
    let Some(zones) = dom.find_child(ws, "Zones") else { return a };
    let cf_of = |i: usize| dom.instances[i].cframe("CFrame");

    if let Some(space) = dom.find_child(zones, "Space") {
        if let (Some(station), Some(center)) = (dom.find_child(space, "Station"), dom.path(space, &["Station", "rings", "center"])) {
            if let Some(base) = cf_of(center) {
                let mut rigs = Vec::new();
                for path in [&["rings", "Ring1"][..], &["rings", "Ring2"], &["rings", "Ring3"], &["station"]] {
                    if let Some(m) = dom.path(station, path) {
                        rigs.push(rig(dom, m, &base, parts, map));
                    } else {
                        rigs.push(Rig { parts: vec![], offsets: vec![] });
                    }
                }
                a.station = Some((base, rigs));
            }
        }
        if let (Some(orbit), Some(center)) = (dom.find_child(space, "Orbit"), dom.path(space, &["Orbit", "center"])) {
            if let Some(base) = cf_of(center) {
                let moons = rig(dom, orbit, &base, parts, map);
                let mut disks = Vec::new();
                for name in ["Disk", "Disk2", "Disk3"] {
                    if let Some(d) = dom.find_child(space, name) {
                        if let Some(dcf) = cf_of(d) {
                            disks.push((dcf, rig(dom, d, &dcf, parts, map)));
                        }
                    }
                }
                a.orbit = Some((base, moons, disks));
            }
        }
    }

    if let Some(city) = dom.find_child(zones, "City") {
        // 7 cloned trains; the original model stays where it was placed
        if let (Some(train), Some(track_part)) = (dom.find_child(city, "Train"), dom.path(city, &["Train", "Track"])) {
            let train_parts = dom.find_child(train, "Parts").unwrap_or(train);
            let tbase = cf_of(track_part).unwrap_or_default();
            let src = collect_parts(dom, train_parts, map);
            let schedule: [(Option<usize>, f32, f32); 7] = [
                (Some(0), 0.0, 15.0),
                (Some(1), 3.0, 20.0),
                (Some(2), 5.5, 18.0),
                (Some(3), 5.5, 25.0),
                (Some(6), 2.5, 30.0),
                (None, 0.0, 1.0),
                (None, 0.0, 1.0),
            ];
            for (track, phase, period) in schedule {
                let mut idx = Vec::new();
                let mut offsets = Vec::new();
                let inv = tbase.inverse();
                for &p in &src {
                    let off = inv.mul(&parts[p].cf);
                    let mut copy = clone_part(&parts[p]);
                    copy.visible = parts[p].visible && track.is_some();
                    idx.push(parts.len());
                    parts.push(copy);
                    offsets.push(off);
                }
                a.trains.push((Rig { parts: idx, offsets }, track, phase, period));
            }
        }
        if let Some(tracks) = dom.find_child(city, "Tracks") {
            let mut rng = rand::thread_rng();
            for name in ["TrackA1", "TrackA2", "TrackA3", "TrackB1", "TrackB2", "TrackC1", "TrackC2", "TrackC3"] {
                let Some(tp) = dom.find_child(tracks, name) else {
                    a.tracks.push(Track { cf0: CFrame::default(), length: 0.0 });
                    continue;
                };
                let inst = &dom.instances[tp];
                let len = inst.vec3("size").map(|s| s.y).unwrap_or(0.0);
                let dir: f32 = if rng.gen_bool(0.5) { 1.0 } else { -1.0 };
                let cf = inst.cframe("CFrame").unwrap_or_default().mul(&CFrame::angles(PI / 2.0 * (dir + 1.0), 0.0, 0.0));
                let cf0 = cf.mul(&CFrame::new(Vec3::new(0.0, -len / 2.0, 0.0)));
                a.tracks.push(Track { cf0, length: len });
            }
        }
        for (pipe, len) in [("Pipe1", 0.5), ("Pipe2", 0.65)] {
            let Some(flow) = dom.path(city, &[pipe, "Flow"]) else { continue };
            let mut nodes: Vec<(u32, usize)> = dom.instances[flow]
                .children
                .iter()
                .filter_map(|&c| Some((dom.instances[c].name.parse().ok()?, c)))
                .collect();
            nodes.sort_by_key(|n| n.0);
            let mut f = Flow { nodes: Vec::new(), cframes: Vec::new(), length: len, cycle: 0, unit_offset: 0.0, t0: 0.0 };
            for (_, c) in nodes {
                let Some(&pi) = map.get(&c) else { continue };
                let pcf = parts[pi].cf;
                let inv = pcf.inverse();
                let kids = dom.instances[c]
                    .children
                    .iter()
                    .filter_map(|k| map.get(k).copied())
                    .map(|k| (k, inv.mul(&parts[k].cf)))
                    .collect();
                f.cframes.push(pcf);
                f.nodes.push((pi, kids));
            }
            if f.nodes.len() >= 2 {
                a.flows.push(f);
            }
        }
    }

    if let Some(volcano) = dom.find_child(zones, "Volcano") {
        if let Some(l) = dom.find_child(volcano, "Lightning") {
            for &m in &dom.instances[l].children {
                let ps = collect_parts(dom, m, map);
                for &p in &ps {
                    parts[p].visible = false;
                }
                let ls = lights.iter().enumerate().filter(|(_, li)| ps.contains(&li.part)).map(|(i, _)| i).collect();
                a.lightning.push(ps);
                a.lightning_lights.push(ls);
            }
        }
    }
    a
}

fn clone_part(p: &Part) -> Part {
    Part {
        inst: p.inst,
        cf: p.cf,
        size: p.size,
        mesh: p.mesh,
        mesh_scale: p.mesh_scale,
        mesh_offset: p.mesh_offset,
        fallback_scale: p.fallback_scale,
        color: p.color,
        alpha: p.alpha,
        reflectance: p.reflectance,
        material: p.material,
        surfaces: p.surfaces,
        flags: p.flags,
        texture: p.texture.clone(),
        visible: p.visible,
    }
}

fn update_flow(f: &mut Flow, parts: &mut [Part], t: f32) {
    if t < f.t0 {
        f.t0 = t;
    }
    let mut unit = (t - f.t0) / f.length + f.unit_offset;
    let n = f.nodes.len();
    if unit >= 1.0 {
        f.cycle = (f.cycle + unit.floor() as usize) % n;
        unit %= 1.0;
        f.unit_offset = unit;
        f.t0 = t;
    }
    for x in 0..n - 1 {
        let i = ((x as isize - f.cycle as isize).rem_euclid(n as isize)) as usize;
        let cf = f.cframes[x].lerp(&f.cframes[x + 1], unit);
        let (pi, kids) = &f.nodes[i];
        parts[*pi].cf = cf;
        for (k, off) in kids {
            parts[*k].cf = cf.mul(off);
        }
    }
}

// >> camera manager.lua: flythrough over the city -> space -> volcano camera sets

pub struct FrameInfo {
    pub cframe: CFrame,
    pub contrast: f32,
    pub saturation: f32,
    pub tint: Vec3,
    pub blur: f32,
}

pub const BACKGROUND_TINT: Vec3 = Vec3::new(0.0784, 0.1686, 0.2353);
const SERIES: [&str; 3] = ["City", "Space", "Volcano"];
const LENGTH: f32 = 60.0;
const TRANSITION: f32 = 1.7;

pub struct Flythrough {
    index: usize,
    t0: f32,
    first: bool,
    look_lerp: Vec2,
    pub brightness_from: f32,
    pub brightness_to: f32,
    pub brightness_t0: f32,
    pub brightness_dur: f32,
}

impl Flythrough {
    pub fn new() -> Self {
        Self { index: usize::MAX, t0: 0.0, first: true, look_lerp: Vec2::ZERO, brightness_from: -1.0, brightness_to: 0.3, brightness_t0: 0.0, brightness_dur: 0.25 }
    }

    pub fn start(&mut self, scene: &mut Scene, now: f32) {
        self.advance(scene, now);
        // transitionToCameraAnimator(flythrough, 0.25, 0.3)
        self.brightness_from = -1.0;
        self.brightness_to = 0.3;
        self.brightness_t0 = now;
        self.brightness_dur = 0.25;
    }

    fn advance(&mut self, scene: &mut Scene, now: f32) {
        self.index = self.index.wrapping_add(1) % SERIES.len();
        scene.set_zone(SERIES[self.index], now);
        self.t0 = now;
    }

    /// Debug: jump to a zone in the series and an offset into its path.
    pub fn force(&mut self, scene: &mut Scene, zone: &str, seek: f32, now: f32) {
        if let Some(i) = SERIES.iter().position(|z| z.eq_ignore_ascii_case(zone)) {
            self.index = (i + SERIES.len() - 1) % SERIES.len();
            self.advance(scene, now);
            self.t0 = now - seek;
            self.first = true;
        }
    }

    pub fn brightness(&self, now: f32) -> f32 {
        let t = ((now - self.brightness_t0) / self.brightness_dur).clamp(0.0, 1.0);
        self.brightness_from + (self.brightness_to - self.brightness_from) * crate::ui::ease_in_out_quad(t)
    }

    pub fn frame(&mut self, scene: &mut Scene, now: f32, dt: f32, look: Vec2) -> FrameInfo {
        if self.index == usize::MAX {
            self.start(scene, now);
        }
        let mut e = now - self.t0;
        if e >= LENGTH {
            self.advance(scene, now);
            self.first = false;
            e = 0.0;
        }
        let ease = crate::ui::ease_in_out_quad;
        let (contrast, blur) = if e > LENGTH - TRANSITION {
            let k = ease(((e - (LENGTH - TRANSITION)) / TRANSITION).min(1.0));
            (0.5 + (-1.0 - 0.5) * k, 10.0 + 40.0 * k)
        } else if !self.first && e < TRANSITION {
            let k = ease(e / TRANSITION);
            (-1.0 + 1.5 * k, 50.0 - 40.0 * k)
        } else {
            (0.5, 10.0)
        };

        let cams = scene.camera_sets.get(SERIES[self.index]).cloned().unwrap_or_default();
        let t = (e / LENGTH).clamp(0.0, 1.0);
        let cf = if cams.is_empty() { CFrame::new(Vec3::new(0.0, 20.0, 50.0)) } else { bezier(&cams, t) };

        // getGamepadInputCFrame
        let unit = 0.125f32.powf(dt);
        self.look_lerp = self.look_lerp * unit + look * (1.0 - unit);
        let g = self.look_lerp;
        let pad = CFrame::new(Vec3::new(g.x / 8.0, g.y / 8.0, 0.0))
            .mul(&CFrame::angles(0.0, -g.x / 12.0, 0.0))
            .mul(&CFrame::angles(g.y / 12.0, 0.0, 0.0));

        FrameInfo { cframe: cf.mul(&pad), contrast, saturation: -1.0, tint: BACKGROUND_TINT, blur }
    }
}

fn bezier(cfs: &[CFrame], t: f32) -> CFrame {
    if cfs.len() == 1 {
        return cfs[0];
    }
    let next: Vec<CFrame> = cfs.windows(2).map(|w| w[0].lerp(&w[1], t)).collect();
    bezier(&next, t)
}

// >> end << //

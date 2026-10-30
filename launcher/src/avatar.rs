/*
    written by: @corescripts
*/

//! The signed-in user's avatar as a real 3D character: rig from the stock R6/R15 layout
//! (assets/rigs.json), body parts / head / face / clothing / accessories from the avatar API and
//! asset delivery, clothing composited onto each body part like the engine does, and the idle
//! animation played from its KeyframeSequence.

// >> imports
use crate::api::Session;
use crate::assets::{decode_image, parse_asset_id, Content, Rgba};
use crate::mesh::{self, MeshData};
use crate::rbxl::{self, content_id, CFrame, Dom};
use crate::scene::{self, MeshKey};
use anyhow::{anyhow, Result};
use glam::{Mat3, Vec2, Vec3};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_MESH: AtomicU32 = AtomicU32::new(1);

pub struct APart {
    pub name: String,
    pub size: Vec3,
    pub mesh: MeshKey,
    pub mesh_scale: Vec3,
    pub mesh_offset: Vec3,
    pub color: Vec3,
    pub texture: Option<String>,
    pub attachments: HashMap<String, CFrame>,
    /// accessories: welded to a body part with this offset
    pub weld: Option<(usize, CFrame)>,
    pub visible: bool,
}

pub struct AJoint {
    pub part0: usize,
    pub part1: usize,
    pub c0: CFrame,
    pub c1: CFrame,
}

#[derive(Clone, Copy)]
struct Key {
    t: f32,
    cf: CFrame,
    style: u32,
    dir: u32,
}

pub struct Anim {
    pub length: f32,
    tracks: HashMap<String, Vec<Key>>,
}

pub struct AvatarModel {
    pub uid: u64,
    pub r15: bool,
    pub parts: Vec<APart>,
    pub joints: Vec<AJoint>,
    pub meshes: Vec<(MeshKey, MeshData)>,
    pub textures: Vec<(String, Rgba)>,
    /// weighted idle animations (Animate script "idle" set)
    pub idle: Vec<(f32, Anim)>,
    /// "toolnone" while a gear is held: overrides the arm it poses
    pub tool: Option<Anim>,
    pub center: Vec3,
    pub height: f32,
}

// >> helpers

fn cf_from(v: &Value) -> CFrame {
    let a: Vec<f32> = v.as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_default();
    if a.len() < 12 {
        return CFrame::default();
    }
    CFrame { pos: Vec3::new(a[0], a[1], a[2]), rot: Mat3::from_cols(Vec3::new(a[3], a[6], a[9]), Vec3::new(a[4], a[7], a[10]), Vec3::new(a[5], a[8], a[11])) }
}

fn v3(v: &Value) -> Vec3 {
    let a: Vec<f32> = v.as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()).unwrap_or_default();
    if a.len() < 3 {
        Vec3::ONE
    } else {
        Vec3::new(a[0], a[1], a[2])
    }
}

fn hex_color(s: &str) -> Vec3 {
    let s = s.trim_start_matches('#');
    let p = |i: usize| u8::from_str_radix(s.get(i..i + 2).unwrap_or("A3"), 16).unwrap_or(163) as f32 / 255.0;
    Vec3::new(p(0), p(2), p(4))
}

fn scale_cf(c: &CFrame, s: Vec3) -> CFrame {
    CFrame { pos: c.pos * s, rot: c.rot }
}

/// An R15 part's size relative to its Classic shape at BodyTypeScale 1, for ProportionScale 0
/// (ProportionsNormal) and 1 (ProportionsSlender). Measured from Studio's
/// Players:CreateHumanoidModelFromDescription; sizes in between are linear in both scales.
fn proportion_tables(part: &str) -> (Vec3, Vec3) {
    match part.trim_start_matches("Left").trim_start_matches("Right") {
        "Head" => (Vec3::splat(0.942), Vec3::new(0.8963, 0.942, 0.8963)),
        "UpperTorso" => (Vec3::new(1.033, 1.309, 1.14), Vec3::new(0.9053, 1.2042, 1.0133)),
        "LowerTorso" => (Vec3::new(1.033, 1.309, 1.14), Vec3::new(0.9857, 1.0046, 1.0133)),
        "UpperArm" | "LowerArm" => (Vec3::new(1.129, 1.342, 1.132), Vec3::new(1.0036, 1.2079, 1.0062)),
        "Hand" => (Vec3::new(1.066, 1.174, 1.231), Vec3::new(0.9476, 1.174, 1.0942)),
        "UpperLeg" => (Vec3::new(1.023, 1.506, 1.023), Vec3::new(0.9761, 1.4009, 0.9093)),
        "LowerLeg" => (Vec3::new(1.023, 1.506, 1.023), Vec3::new(0.9761, 1.3005, 0.9093)),
        "Foot" => (Vec3::new(1.079, 1.267, 1.129), Vec3::new(1.0296, 1.1333, 1.0036)),
        _ => (Vec3::ONE, Vec3::ONE),
    }
}

/// The avatar's scales (avatar API "scales"); R6 bodies are never scaled.
struct Scales {
    r15: bool,
    body: Vec3,
    head: f32,
    body_type: f32,
    proportion: f32,
}

impl Scales {
    /// How much a part authored as `ty` (its AvatarPartScaleType: Classic, ProportionsNormal or
    /// ProportionsSlender) is scaled: towards the chosen body type / proportions, then by
    /// width/height/depth (the head by head scale only).
    fn part(&self, part: &str, ty: &str) -> Vec3 {
        if !self.r15 {
            return Vec3::ONE;
        }
        let (normal, slender) = proportion_tables(part);
        let target = Vec3::ONE.lerp(normal.lerp(slender, self.proportion), self.body_type);
        let authored = match ty {
            "ProportionsNormal" => normal,
            "ProportionsSlender" => slender,
            _ => Vec3::ONE,
        };
        let whd = if part == "Head" { Vec3::splat(self.head) } else { self.body };
        whd * target / authored
    }
}

/// A layered clothing item (WrapLayer), fitted onto the body once the rig is built.
struct Layered {
    name: String,
    /// the clothing mesh in its Handle's space (already scaled)
    mesh: MeshData,
    texture: Option<String>,
    color: Vec3,
    /// body part the Handle hangs from and its offset (like any accessory)
    weld: (usize, CFrame),
    /// inner cage (ReferenceMeshId) in Handle space: the body shape the item was made around
    reference: Option<MeshData>,
    order: i32,
}

/// Cage points keyed by UV: Roblox cages share one UV layout, which is how a clothing cage
/// vertex finds the same spot on any body's cage.
fn cage_key(uv: [f32; 2]) -> (i32, i32) {
    ((uv[0] * 500.0).round() as i32, (uv[1] * 500.0).round() as i32)
}

/// Points with a displacement each, looked up by position (inverse-distance blend of the
/// nearest ones).
struct DisplacementField {
    cell: f32,
    grid: HashMap<(i32, i32, i32), Vec<usize>>,
    points: Vec<(Vec3, Vec3)>,
}

impl DisplacementField {
    fn new(points: Vec<(Vec3, Vec3)>) -> Self {
        let cell = 0.25;
        let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
        for (i, (p, _)) in points.iter().enumerate() {
            grid.entry(Self::cell_of(*p, cell)).or_default().push(i);
        }
        DisplacementField { cell, grid, points }
    }

    fn cell_of(p: Vec3, cell: f32) -> (i32, i32, i32) {
        ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32, (p.z / cell).floor() as i32)
    }

    fn at(&self, p: Vec3) -> Vec3 {
        const K: usize = 8;
        let c = Self::cell_of(p, self.cell);
        let mut near: Vec<(f32, usize)> = Vec::new();
        for ring in 0i32..12 {
            for dx in -ring..=ring {
                for dy in -ring..=ring {
                    for dz in -ring..=ring {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != ring {
                            continue;
                        }
                        if let Some(v) = self.grid.get(&(c.0 + dx, c.1 + dy, c.2 + dz)) {
                            near.extend(v.iter().map(|&i| (self.points[i].0.distance_squared(p), i)));
                        }
                    }
                }
            }
            // a ring further out can still hold closer points than the corners of this one
            if near.len() >= K && ring >= 1 {
                break;
            }
        }
        near.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (mut sum, mut wsum) = (Vec3::ZERO, 0.0);
        for (d2, i) in near.into_iter().take(K) {
            let w = 1.0 / (d2 + 1e-4);
            sum += self.points[i].1 * w;
            wsum += w;
        }
        if wsum > 0.0 { sum / wsum } else { Vec3::ZERO }
    }
}

/// Fit layered clothing onto the body: every point of the item's inner cage is moved to the
/// same (UV-matched) point of the body's cages, the clothing follows the blended move of its
/// nearest cage points, and each triangle is then carried by the body part it is closest to.
fn fit_layered(l: &Loader, model: &mut AvatarModel, cages: &HashMap<String, (String, CFrame, Vec3)>, items: Vec<Layered>) {
    let rest = model.rest();
    let mut target: HashMap<(i32, i32), Vec3> = HashMap::new();
    for (name, (uri, origin, s)) in cages {
        let Some(pi) = model.parts.iter().position(|p| &p.name == name) else { continue };
        let Some(pcf) = rest[pi] else { continue };
        let Some(cage) = l.mesh(uri, None, *s) else { continue };
        let o = pcf.mul(&CFrame { pos: origin.pos * *s, rot: origin.rot });
        for v in &cage.vertices {
            target.insert(cage_key(v.uv), o.point(Vec3::from(v.pos)));
        }
    }
    // the body parts clothing can ride on
    let body: Vec<(usize, CFrame, Vec3)> = model.parts.iter().enumerate().filter(|(_, p)| p.weld.is_none() && p.visible).filter_map(|(i, p)| Some((i, rest[i]?, p.size))).collect();
    for it in items {
        let Some(base) = rest[it.weld.0] else { continue };
        let hcf = base.mul(&it.weld.1);
        let mut verts: Vec<(Vec3, Vec3)> = it.mesh.vertices.iter().map(|v| (hcf.point(Vec3::from(v.pos)), hcf.rot * Vec3::from(v.normal))).collect();
        let mut fitted = false;
        if let (Some(cage), false) = (&it.reference, target.is_empty()) {
            let mut matched = 0;
            let points: Vec<(Vec3, Vec3)> = cage
                .vertices
                .iter()
                .map(|v| {
                    let p = hcf.point(Vec3::from(v.pos));
                    let (u, w) = cage_key(v.uv);
                    let hit = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)].iter().find_map(|(du, dw)| target.get(&(u + du, w + dw)));
                    match hit {
                        Some(t) => {
                            matched += 1;
                            (p, *t - p)
                        }
                        None => (p, Vec3::ZERO),
                    }
                })
                .collect();
            // a body without cages for most of it (classic packages) wears it as authored
            if matched * 10 >= cage.vertices.len() * 7 {
                let field = DisplacementField::new(points);
                for (p, _) in verts.iter_mut() {
                    *p += field.at(*p);
                }
                fitted = true;
            }
        }
        if std::env::var("XBOX_AVATAR_DEBUG").is_ok() {
            eprintln!("layered {}: {} verts, fitted to the body cage: {fitted} ({} cage points)", it.name, verts.len(), target.len());
        }
        // each layer sits a little further out so layers and the body don't fight
        let push = 0.01 + 0.004 * it.order.clamp(0, 20) as f32;
        for (p, n) in verts.iter_mut() {
            *p += n.normalize_or_zero() * push;
        }
        // split by the nearest body part, in that part's space
        let mut split: HashMap<usize, (MeshData, HashMap<u32, u32>)> = HashMap::new();
        for tri in it.mesh.indices.chunks(3) {
            if tri.len() < 3 {
                continue;
            }
            let c = (verts[tri[0] as usize].0 + verts[tri[1] as usize].0 + verts[tri[2] as usize].0) / 3.0;
            let Some(&(pi, pcf, _)) = body.iter().min_by(|a, b| {
                let d = |(_, cf, size): &(usize, CFrame, Vec3)| (cf.inverse().point(c).abs() - *size * 0.5).max(Vec3::ZERO).length_squared();
                d(a).total_cmp(&d(b))
            }) else {
                continue;
            };
            let inv = pcf.inverse();
            let (m, remap) = split.entry(pi).or_default();
            for &vi in tri {
                let ni = *remap.entry(vi).or_insert_with(|| {
                    let src = it.mesh.vertices[vi as usize];
                    let (p, n) = verts[vi as usize];
                    m.vertices.push(mesh::Vertex { pos: inv.point(p).into(), normal: (inv.rot * n).into(), ..src });
                    (m.vertices.len() - 1) as u32
                });
                m.indices.push(ni);
            }
        }
        for (pi, (m, _)) in split {
            let mk = new_key();
            model.meshes.push((mk, m));
            model.parts.push(APart {
                name: it.name.clone(),
                size: Vec3::ONE,
                mesh: mk,
                mesh_scale: Vec3::ONE,
                mesh_offset: Vec3::ZERO,
                color: it.color,
                texture: it.texture.clone(),
                attachments: HashMap::new(),
                weld: Some((pi, CFrame::default())),
                visible: true,
            });
        }
    }
}

/// A MeshPart's mesh: MeshId, or the newer MeshContent.
fn part_mesh_id(dom: &Dom, inst: usize) -> String {
    let i = &dom.instances[inst];
    content_id(i.get("MeshId")).or_else(|| content_id(i.get("MeshContent"))).unwrap_or_default()
}

/// A MeshPart's texture: TextureID / TextureContent, or (UGC bodies, heads and accessories) the
/// ColorMap of its SurfaceAppearance.
fn part_texture(dom: &Dom, inst: usize) -> Option<String> {
    let i = &dom.instances[inst];
    content_id(i.get("TextureID")).or_else(|| content_id(i.get("TextureContent"))).or_else(|| {
        let sa = i.children.iter().map(|&c| &dom.instances[c]).find(|c| c.class == "SurfaceAppearance")?;
        content_id(sa.get("ColorMap")).or_else(|| content_id(sa.get("ColorMapContent")))
    })
}

/// A part's AvatarPartScaleType (StringValue child), Classic when it has none.
fn part_scale_type(dom: &Dom, inst: usize) -> String {
    dom.instances[inst].children.iter().map(|&c| &dom.instances[c]).find(|c| c.name == "AvatarPartScaleType").and_then(|c| c.string("Value")).unwrap_or_else(|| "Classic".into())
}

struct Loader<'a> {
    content: &'a Content,
    session: &'a Session,
}

impl Loader<'_> {
    fn model(&self, id: u64) -> Result<Dom> {
        let r = self.content.fetch_model(id, Some(self.session)).and_then(|b| rbxl::parse_any(&b));
        if let Err(e) = &r {
            eprintln!("avatar: model {id}: {e}");
        }
        r
    }
    fn image(&self, uri: &str) -> Option<Rgba> {
        let r = self.content.load(uri, Some(self.session)).and_then(|b| decode_image(&b));
        if let Err(e) = &r {
            eprintln!("avatar: image {uri}: {e}");
        }
        r.ok()
    }
    /// A mesh asset normalised to `target` size (MeshPart Size / SpecialMesh Head) or scaled by `scale` (FileMesh).
    fn mesh(&self, uri: &str, fit: Option<Vec3>, scale: Vec3) -> Option<MeshData> {
        let bytes = if let Some(id) = parse_asset_id(uri) { self.content.fetch_asset(id, Some(self.session)).ok()? } else { self.content.read_local(uri).ok()? };
        let mut m = match mesh::parse_mesh(&bytes) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("avatar: mesh {uri}: {e} ({:?})", String::from_utf8_lossy(&bytes[..bytes.len().min(12)]));
                return None;
            }
        };
        let s = match fit {
            Some(target) => {
                let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                for v in &m.vertices {
                    lo = lo.min(Vec3::from(v.pos));
                    hi = hi.max(Vec3::from(v.pos));
                }
                let ext = (hi - lo).max(Vec3::splat(1e-4));
                let mid = (hi + lo) * 0.5;
                for v in m.vertices.iter_mut() {
                    v.pos = (Vec3::from(v.pos) - mid).into();
                }
                if uri.ends_with("head.mesh") {
                    // MeshType.Head keeps the head's proportions; its height follows the part
                    Vec3::splat(target.y / ext.y)
                } else {
                    target / ext
                }
            }
            None => scale,
        };
        for v in m.vertices.iter_mut() {
            v.pos = (Vec3::from(v.pos) * s).into();
            let n = Vec3::from(v.normal) / s.max(Vec3::splat(1e-4));
            v.normal = n.normalize_or_zero().into();
        }
        Some(m)
    }
}

/// Atlas UVs on package meshes run bottom-up.
fn flip_v(mut m: MeshData) -> MeshData {
    for v in m.vertices.iter_mut() {
        v.uv[1] = 1.0 - v.uv[1];
    }
    m
}

fn new_key() -> MeshKey {
    MeshKey::Avatar(NEXT_MESH.fetch_add(1, Ordering::Relaxed))
}

// >> clothing template
// (585 x 559)

/// (x, y, w, h) template rectangles for the six faces: +X (R), -X (L), +Y (U), -Y (D), +Z (B), -Z (F).
type Faces = [[f32; 4]; 6];
const TORSO: Faces = [[165.0, 74.0, 64.0, 128.0], [361.0, 74.0, 64.0, 128.0], [231.0, 8.0, 128.0, 64.0], [231.0, 204.0, 128.0, 64.0], [427.0, 74.0, 128.0, 128.0], [231.0, 74.0, 128.0, 128.0]];
const RIGHT_LIMB: Faces = [[151.0, 355.0, 64.0, 128.0], [19.0, 355.0, 64.0, 128.0], [217.0, 289.0, 64.0, 64.0], [217.0, 485.0, 64.0, 64.0], [85.0, 355.0, 64.0, 128.0], [217.0, 355.0, 64.0, 128.0]];
const LEFT_LIMB: Faces = [[506.0, 355.0, 64.0, 128.0], [374.0, 355.0, 64.0, 128.0], [308.0, 289.0, 64.0, 64.0], [308.0, 485.0, 64.0, 64.0], [440.0, 355.0, 64.0, 128.0], [308.0, 355.0, 64.0, 128.0]];
const TW: f32 = 585.0;
const TH: f32 = 559.0;

/// Box whose faces sample the clothing template like an R6 limb, with the edges rounded off
/// like the default body Roblox renders (radius `ROUND`).
const ROUND: f32 = 0.08;
const GRID: usize = 8;
fn template_box(size: Vec3, faces: &Faces) -> MeshData {
    let mut m = MeshData::default();
    let h = size * 0.5;
    let inner = (h - Vec3::splat(ROUND)).max(Vec3::ZERO);
    // (normal, u axis, v axis) for each face as seen from outside; u runs left->right, v top->bottom in the template
    let axes = [
        (Vec3::X, -Vec3::Z, -Vec3::Y),
        (-Vec3::X, Vec3::Z, -Vec3::Y),
        (Vec3::Y, -Vec3::X, -Vec3::Z),
        (-Vec3::Y, -Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, -Vec3::Y),
        (-Vec3::Z, -Vec3::X, -Vec3::Y),
    ];
    for (f, (n, u, v)) in axes.iter().enumerate() {
        let r = faces[f];
        let c = *n * h;
        let eu = (*u * h).length();
        let ev = (*v * h).length();
        let corner = |a: f32, b: f32| c + *u * eu * a + *v * ev * b;
        let uv = |a: f32, b: f32| Vec2::new((r[0] + r[2] * (a + 1.0) * 0.5) / TW, (r[1] + r[3] * (b + 1.0) * 0.5) / TH);
        let base = m.vertices.len() as u32;
        for j in 0..=GRID {
            for i in 0..=GRID {
                let (a, b) = (i as f32 / GRID as f32 * 2.0 - 1.0, j as f32 / GRID as f32 * 2.0 - 1.0);
                let p = corner(a, b);
                // round the edges: pull each point onto a rounded box around the inner core
                let q = p.clamp(-inner, inner);
                let d = p - q;
                let (pos, nrm) = if d.length() > 1e-5 { (q + d.normalize() * ROUND.min(h.min_element()), d.normalize()) } else { (p, *n) };
                m.vertices.push(mesh::Vertex { pos: pos.into(), normal: nrm.into(), uv: uv(a, b).into(), color: [255; 4] });
            }
        }
        let outward = u.cross(*v).dot(*n) > 0.0;
        let w = GRID as u32 + 1;
        for j in 0..GRID as u32 {
            for i in 0..GRID as u32 {
                let (a, b, c, d) = (base + j * w + i, base + j * w + i + 1, base + (j + 1) * w + i + 1, base + (j + 1) * w + i);
                if outward {
                    m.indices.extend([a, b, c, a, c, d]);
                } else {
                    m.indices.extend([a, c, b, a, d, c]);
                }
            }
        }
    }
    m
}

fn blit(dst: &mut Rgba, src: &Rgba, rect: Option<[f32; 4]>) {
    // src covers the whole template (shirt/pants) or is stretched into `rect` (t-shirt)
    let [rx, ry, rw, rh] = rect.unwrap_or([0.0, 0.0, dst.w as f32, dst.h as f32]);
    for y in ry as u32..(ry + rh) as u32 {
        for x in rx as u32..(rx + rw) as u32 {
            if x >= dst.w || y >= dst.h {
                continue;
            }
            let sx = (((x as f32 - rx) / rw) * src.w as f32) as u32;
            let sy = (((y as f32 - ry) / rh) * src.h as f32) as u32;
            let si = ((sy.min(src.h - 1) * src.w + sx.min(src.w - 1)) * 4) as usize;
            let di = ((y * dst.w + x) * 4) as usize;
            // "over" compositing, alpha included (layers start transparent)
            let a = src.data[si + 3] as f32 / 255.0;
            let da = dst.data[di + 3] as f32 / 255.0;
            let out_a = a + da * (1.0 - a);
            for c in 0..3 {
                let v = (src.data[si + c] as f32 * a + dst.data[di + c] as f32 * da * (1.0 - a)) / out_a.max(1e-4);
                dst.data[di + c] = v as u8;
            }
            dst.data[di + 3] = (out_a * 255.0) as u8;
        }
    }
}

/// R15 limb texture: body colour, package texture, then the clothing template laid out by the
/// part's R15Composit*Base mesh (legs use the arm layouts).
fn r15_limb_texture(content: &Content, name: &str, color: Vec3, package: Option<&Rgba>, clothes: &Rgba) -> Option<Rgba> {
    let (mesh_name, w, h) = match name {
        n if n.contains("Torso") => ("R15CompositTorsoBase", 388, 264),
        n if n.starts_with("Right") => ("R15CompositRightArmBase", 264, 284),
        _ => ("R15CompositLeftArmBase", 264, 284),
    };
    let m = content.read_local(&format!("rbxasset://compositing/{mesh_name}.mesh")).ok().and_then(|b| mesh::parse_mesh(&b).ok())?;
    let mut canvas = filled(w, h, color);
    if let Some(p) = package {
        blit(&mut canvas, p, None);
    }
    draw_composit(&mut canvas, &m, Some(clothes), Vec3::ONE);
    Some(canvas)
}

/// Faces an R15 part wraps around itself: the torso starts at its front (front, left, back,
/// right); limbs go side, front, side, back (right limbs from their outer side, left limbs from
/// their inner side). (x, width) in the
/// template, plus the row (y, height).
fn r15_strip(name: &str) -> ([(f32, f32); 4], f32, f32) {
    match name {
        n if n.contains("Torso") => ([(231.0, 128.0), (361.0, 64.0), (427.0, 128.0), (165.0, 64.0)], 74.0, 128.0),
        n if n.starts_with("Right") => ([(151.0, 64.0), (217.0, 64.0), (19.0, 64.0), (85.0, 64.0)], 355.0, 128.0),
        _ => ([(506.0, 64.0), (308.0, 64.0), (374.0, 64.0), (440.0, 64.0)], 355.0, 128.0),
    }
}

/// The four faces side by side, in wrap order.
fn strip(src: &Rgba, (faces, y, h): ([(f32, f32); 4], f32, f32)) -> Rgba {
    let w: u32 = faces.iter().map(|f| f.1 as u32).sum();
    let mut out = Rgba { w, h: h as u32, data: vec![0; (w * h as u32 * 4) as usize] };
    let mut ox = 0;
    for (fx, fw) in faces {
        let face = crop(src, [fx, y, fw, h]);
        for yy in 0..face.h {
            for xx in 0..face.w {
                let si = ((yy * face.w + xx) * 4) as usize;
                let di = ((yy * w + ox + xx) * 4) as usize;
                out.data[di..di + 4].copy_from_slice(&face.data[si..si + 4]);
            }
        }
        ox += fw as u32;
    }
    out
}

fn crop(src: &Rgba, r: [f32; 4]) -> Rgba {
    let (x0, y0, w, h) = (r[0] as u32, r[1] as u32, r[2] as u32, r[3] as u32);
    let mut out = Rgba { w, h, data: vec![0; (w * h * 4) as usize] };
    for y in 0..h {
        for x in 0..w {
            let si = (((y0 + y).min(src.h - 1) * src.w + (x0 + x).min(src.w - 1)) * 4) as usize;
            let di = ((y * w + x) * 4) as usize;
            out.data[di..di + 4].copy_from_slice(&src.data[si..si + 4]);
        }
    }
    out
}

/// The clothing in a composited strip, with the bare body colour made transparent again.
fn clothing_only(strip: &Rgba, body: Vec3) -> Rgba {
    let b = [(body.x * 255.0) as u8, (body.y * 255.0) as u8, (body.z * 255.0) as u8];
    let mut out = Rgba { w: strip.w, h: strip.h, data: strip.data.clone() };
    for px in out.data.chunks_mut(4) {
        if px[0] == b[0] && px[1] == b[1] && px[2] == b[2] {
            px[3] = 0;
        }
    }
    out
}

fn filled(w: u32, h: u32, c: Vec3) -> Rgba {
    let px = [(c.x * 255.0) as u8, (c.y * 255.0) as u8, (c.z * 255.0) as u8, 255];
    Rgba { w, h, data: px.iter().copied().cycle().take((w * h * 4) as usize).collect() }
}

// >> r15 clothing atlas
// (1024 x 512)

/// Rasterise one compositing mesh: vertex x/y are atlas pixels, uv samples `src` (or a flat colour).
fn draw_composit(atlas: &mut Rgba, m: &MeshData, src: Option<&Rgba>, color: Vec3) {
    let px = |i: u32| {
        let v = &m.vertices[i as usize];
        (Vec2::new(v.pos[0], v.pos[1]), Vec2::new(v.uv[0], v.uv[1]))
    };
    let flat = [(color.x * 255.0) as u8, (color.y * 255.0) as u8, (color.z * 255.0) as u8, 255];
    for t in m.indices.chunks(3) {
        if t.len() < 3 {
            continue;
        }
        let (a, b, c) = (px(t[0]), px(t[1]), px(t[2]));
        let lo = a.0.min(b.0).min(c.0).floor().max(Vec2::ZERO);
        let hi = a.0.max(b.0).max(c.0).ceil().min(Vec2::new(atlas.w as f32 - 1.0, atlas.h as f32 - 1.0));
        let area = (b.0 - a.0).perp_dot(c.0 - a.0);
        if area.abs() < 1e-6 {
            continue;
        }
        for y in lo.y as u32..=hi.y as u32 {
            for x in lo.x as u32..=hi.x as u32 {
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = (b.0 - p).perp_dot(c.0 - p) / area;
                let w1 = (c.0 - p).perp_dot(a.0 - p) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    continue;
                }
                let s = match src {
                    Some(img) => {
                        let uv = a.1 * w0 + b.1 * w1 + c.1 * w2;
                        let sx = ((uv.x * img.w as f32) as u32).min(img.w - 1);
                        let sy = ((uv.y * img.h as f32) as u32).min(img.h - 1);
                        let i = ((sy * img.w + sx) * 4) as usize;
                        [img.data[i], img.data[i + 1], img.data[i + 2], img.data[i + 3]]
                    }
                    None => flat,
                };
                let di = ((y * atlas.w + x) * 4) as usize;
                let al = s[3] as f32 / 255.0;
                for k in 0..3 {
                    atlas.data[di + k] = (s[k] as f32 * al + atlas.data[di + k] as f32 * (1.0 - al)) as u8;
                }
                atlas.data[di + 3] = atlas.data[di + 3].max(s[3]);
            }
        }
    }
}

/// The engine's R15 composite: body colours, (package texture), pants, shirt, t-shirt.
fn r15_atlas(content: &Content, colors: &[(&str, Vec3)], package: Option<&Rgba>, pants: Option<&Rgba>, shirt: Option<&Rgba>, tshirt: Option<&Rgba>) -> Rgba {
    let load = |n: &str| content.read_local(&format!("rbxasset://fonts/{n}.mesh")).ok().and_then(|b| mesh::parse_mesh(&b).ok());
    let mut atlas = Rgba { w: 1024, h: 512, data: vec![0; 1024 * 512 * 4] };
    for (base, c) in colors {
        if let Some(m) = load(base) {
            draw_composit(&mut atlas, &m, None, *c);
        }
    }
    if let (Some(p), Some(m)) = (package, load("CompositFullAtlasBaseTexture")) {
        draw_composit(&mut atlas, &m, Some(p), Vec3::ONE);
    }
    for (img, name) in [(pants, "CompositPantsTemplate"), (shirt, "CompositShirtTemplate"), (tshirt, "CompositTShirt")] {
        if let (Some(i), Some(m)) = (img, load(name)) {
            draw_composit(&mut atlas, &m, Some(i), Vec3::ONE);
        }
    }
    // anything the meshes did not cover stays opaque so the part colour never shows through
    for px in atlas.data.chunks_mut(4) {
        px[3] = 255;
    }
    atlas
}

// >> animation

/// One FloatCurve key: interpolation (0 constant, 1 linear, 2 cubic), value and its tangents.
struct CurveKey {
    t: f32,
    interp: u8,
    v: f32,
    left: f32,
    right: f32,
}

/// FloatCurve.ValuesAndTimes: u32 version (2), u32 n, n x {u8 interpolation, u8 tangent mode,
/// f32 value, f32 left tangent, f32 right tangent}, then u32 version (1), u32 n, n x i32 time in
/// ticks (2400 / s).
fn float_curve(b: &[u8]) -> Vec<CurveKey> {
    let u32_at = |o: usize| b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
    let f32_at = |o: usize| b.get(o..o + 4).map(|x| f32::from_le_bytes([x[0], x[1], x[2], x[3]])).unwrap_or(0.0);
    let Some(n) = u32_at(4).map(|n| n as usize) else { return Vec::new() };
    let times = 8 + n * 14;
    if u32_at(times + 4).map(|m| m as usize) != Some(n) {
        return Vec::new();
    }
    (0..n)
        .map(|i| {
            let o = 8 + i * 14;
            let ticks = u32_at(times + 8 + i * 4).unwrap_or(0) as i32;
            CurveKey { t: ticks as f32 / 2400.0, interp: b[o], v: f32_at(o + 2), left: f32_at(o + 6), right: f32_at(o + 10) }
        })
        .collect()
}

fn sample_curve(keys: &[CurveKey], t: f32) -> f32 {
    let Some(first) = keys.first() else { return 0.0 };
    if t <= first.t {
        return first.v;
    }
    for w in keys.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if t <= b.t {
            let h = (b.t - a.t).max(1e-6);
            let u = (t - a.t) / h;
            return match a.interp {
                0 => a.v,
                1 => a.v + (b.v - a.v) * u,
                // cubic Hermite with the keys' tangents (per second)
                _ => {
                    let (u2, u3) = (u * u, u * u * u);
                    (2.0 * u3 - 3.0 * u2 + 1.0) * a.v + (u3 - 2.0 * u2 + u) * a.right * h + (-2.0 * u3 + 3.0 * u2) * b.v + (u3 - u2) * b.left * h
                }
            };
        }
    }
    keys.last().map(|k| k.v).unwrap_or(0.0)
}

/// CurveAnimation (newer emotes): a folder per joint holding a Rotation EulerRotationCurve and a
/// Position Vector3Curve, each with X/Y/Z FloatCurves. Resampled at 30 Hz into keyframe tracks.
fn load_curve_anim(dom: &Dom) -> Option<Anim> {
    dom.instances.iter().position(|i| i.class == "CurveAnimation")?;
    let xyz = |owner: usize| -> [Vec<CurveKey>; 3] {
        let mut out: [Vec<CurveKey>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for &c in &dom.instances[owner].children {
            let ci = &dom.instances[c];
            let axis = match ci.name.as_str() {
                "X" => 0,
                "Y" => 1,
                "Z" => 2,
                _ => continue,
            };
            if ci.class == "FloatCurve" {
                out[axis] = float_curve(ci.bytes("ValuesAndTimes").unwrap_or(&[]));
            }
        }
        out
    };
    // part name -> (rotation xyz, position xyz)
    let mut joints: HashMap<String, (Option<[Vec<CurveKey>; 3]>, Option<[Vec<CurveKey>; 3]>)> = HashMap::new();
    let mut length: f32 = 0.0;
    for (i, inst) in dom.instances.iter().enumerate() {
        let is_rot = inst.class == "EulerRotationCurve";
        let is_pos = inst.class == "Vector3Curve";
        if !is_rot && !is_pos {
            continue;
        }
        let Some(part) = inst.parent.map(|p| dom.instances[p].name.clone()) else { continue };
        let ch = xyz(i);
        length = ch.iter().flat_map(|c| c.iter().map(|k| k.t)).fold(length, f32::max);
        let e = joints.entry(part).or_default();
        if is_rot {
            e.0 = Some(ch);
        } else {
            e.1 = Some(ch);
        }
    }
    let length = length.max(0.01);
    let steps = (length * 30.0).ceil() as usize;
    let mut tracks: HashMap<String, Vec<Key>> = HashMap::new();
    for (part, (rot, pos)) in joints {
        let keys = (0..=steps)
            .map(|k| {
                let t = (k as f32 / 30.0).min(length);
                let r = rot.as_ref().map(|c| [sample_curve(&c[0], t), sample_curve(&c[1], t), sample_curve(&c[2], t)]).unwrap_or([0.0; 3]);
                let p = pos.as_ref().map(|c| Vec3::new(sample_curve(&c[0], t), sample_curve(&c[1], t), sample_curve(&c[2], t))).unwrap_or(Vec3::ZERO);
                // RotationOrder XYZ (the default): CFrame.fromEulerAnglesXYZ
                let mut cf = CFrame::angles(r[0], r[1], r[2]);
                cf.pos = p;
                Key { t, cf, style: 0, dir: 0 }
            })
            .collect();
        tracks.insert(part, keys);
    }
    (!tracks.is_empty()).then_some(Anim { length, tracks })
}

fn load_anim(dom: &Dom) -> Option<Anim> {
    if let Some(a) = load_curve_anim(dom) {
        return Some(a);
    }
    let root = dom.instances.iter().position(|i| i.class == "KeyframeSequence")?;
    let mut tracks: HashMap<String, Vec<Key>> = HashMap::new();
    let mut length: f32 = 0.0;
    for &k in &dom.instances[root].children {
        let kf = &dom.instances[k];
        if kf.class != "Keyframe" {
            continue;
        }
        let t = kf.f32("Time").unwrap_or(0.0);
        length = length.max(t);
        let mut poses = Vec::new();
        dom.descendants(k, &mut poses);
        for p in poses {
            let pi = &dom.instances[p];
            if pi.class != "Pose" || pi.f32("Weight").unwrap_or(1.0) <= 0.0 {
                continue;
            }
            let cf = pi.cframe("CFrame").unwrap_or_default();
            tracks.entry(pi.name.clone()).or_default().push(Key { t, cf, style: pi.enum_("EasingStyle").unwrap_or(0), dir: pi.enum_("EasingDirection").unwrap_or(0) });
        }
    }
    for v in tracks.values_mut() {
        v.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap());
    }
    (!tracks.is_empty()).then_some(Anim { length: length.max(0.01), tracks })
}

impl Anim {
    pub fn track_count(&self) -> usize {
        self.tracks.len()
    }
}

/// The animation an animation-pack or emote asset plays: the asset is a KeyframeSequence itself,
/// or a model whose Animation objects point at one (packs hold one per StringValue: walk, idle...).
pub fn load_preview_anim(content: &Content, session: &Session, asset: u64) -> Option<Anim> {
    let l = Loader { content, session };
    let dom = l.model(asset).ok()?;
    if let Some(a) = load_anim(&dom) {
        return Some(a);
    }
    dom.instances
        .iter()
        .filter(|i| i.class == "Animation")
        .filter_map(|i| content_id(i.get("AnimationId")).and_then(|u| parse_asset_id(&u)))
        .find_map(|id| l.model(id).ok().and_then(|d| load_anim(&d)))
}

/// PoseEasingStyle / PoseEasingDirection (Linear, Constant, Elastic, Cubic, Bounce, CubicV2).
fn ease(style: u32, dir: u32, t: f32) -> f32 {
    let cubic_in = |t: f32| t * t * t;
    let f = |t: f32| match style {
        1 => 0.0,
        2 | 3 | 4 | 5 => cubic_in(t),
        _ => t,
    };
    if style == 0 || style == 1 {
        return f(t);
    }
    match dir {
        1 => 1.0 - f(1.0 - t),
        2 => {
            if t < 0.5 {
                f(t * 2.0) / 2.0
            } else {
                1.0 - f((1.0 - t) * 2.0) / 2.0
            }
        }
        // Roblox's "In" for poses behaves like an ease out of the previous key
        _ => 1.0 - f(1.0 - t),
    }
}

impl Anim {
    pub fn sample(&self, name: &str, t: f32) -> Option<CFrame> {
        let keys = self.tracks.get(name)?;
        let t = t % self.length;
        let j = keys.iter().position(|k| k.t > t).unwrap_or(keys.len());
        if j == 0 {
            return Some(keys[0].cf);
        }
        let a = keys[j - 1];
        let Some(b) = keys.get(j) else { return Some(a.cf) };
        let k = ((t - a.t) / (b.t - a.t).max(1e-4)).clamp(0.0, 1.0);
        Some(a.cf.lerp(&b.cf, ease(a.style, a.dir, k)))
    }
}

// >> loading

/// The account's current avatar.
pub fn load(content: &Content, session: &Session, uid: u64) -> Result<AvatarModel> {
    load_desc(content, session, uid, &session.avatar_info(uid)?)
}

/// An outfit, rendered from its saved description without wearing it.
pub fn load_outfit(content: &Content, session: &Session, uid: u64, outfit: u64) -> Result<AvatarModel> {
    let d = session.outfit_details(outfit)?;
    let desc = serde_json::json!({
        "playerAvatarType": d["playerAvatarType"],
        "scales": d["scale"],
        "bodyColor3s": d["bodyColor3s"],
        "assets": d["assets"],
    });
    load_desc(content, session, uid, &desc)
}

thread_local! {
    /// set on the background prefetch thread: its helper threads run at the lowest priority too
    static BACKGROUND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run the current thread (and the helpers `par` starts from it) at the lowest priority, so
/// background loading never takes frames from the menu.
pub fn lower_priority() {
    BACKGROUND.with(|b| b.set(true));
    set_lowest_priority();
}

fn set_lowest_priority() {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_LOWEST};
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_LOWEST);
    }
}

fn par<F: Fn(u64) + Sync>(ids: &[u64], f: F) {
    if ids.is_empty() {
        return;
    }
    let f = &f;
    let low = BACKGROUND.with(|b| b.get());
    if low {
        // background prefetch: one at a time, so the menu keeps the CPU
        for &id in ids {
            f(id);
        }
        return;
    }
    std::thread::scope(|sc| {
        for chunk in ids.chunks(ids.len().div_ceil(8).max(1)) {
            sc.spawn(move || {
                for &id in chunk {
                    f(id);
                }
            });
        }
    });
}

/// Download asset models (and the meshes/textures/animations they use) into the disk cache.
pub fn prefetch_assets(content: &Content, session: &Session, ids: &[u64]) {
    prefetch(&Loader { content, session }, ids);
}

/// Fetch everything an avatar needs at once (models first, then what they reference).
fn prefetch(l: &Loader, models: &[u64]) {
    par(models, |id| {
        let _ = l.content.fetch_model(id, Some(l.session));
    });
    let mut assets = Vec::new();
    let mut anims = Vec::new();
    for &id in models {
        let Ok(dom) = l.content.fetch_model(id, Some(l.session)).and_then(|b| rbxl::parse_any(&b)) else { continue };
        for inst in &dom.instances {
            for (k, v) in &inst.props {
                let id = match v {
                    rbxl::Value::String(b) => parse_asset_id(&String::from_utf8_lossy(b)),
                    rbxl::Value::Int64(n) if inst.class == "CharacterMesh" && *n > 0 => Some(*n as u64),
                    _ => None,
                };
                if let Some(id) = id.filter(|i| *i > 0) {
                    if k == "AnimationId" {
                        anims.push(id);
                    } else {
                        assets.push(id);
                    }
                }
            }
        }
    }
    assets.sort_unstable();
    assets.dedup();
    par(&assets, |id| {
        let _ = l.content.fetch_asset(id, Some(l.session));
    });
    par(&anims, |id| {
        let _ = l.content.fetch_model(id, Some(l.session));
    });
}

pub fn load_desc(content: &Content, session: &Session, uid: u64, info: &Value) -> Result<AvatarModel> {
    let l = Loader { content, session };
    let rigs: Value = serde_json::from_slice(&content.read_local("rbxasset://rigs.json")?)?;
    let r15 = info["playerAvatarType"].as_str() == Some("R15");
    if std::env::var("XBOX_AVATAR_DEBUG").is_ok() {
        let types: Vec<String> = info["assets"].as_array().map(|a| a.iter().map(|x| format!("{}:{}", x["id"], x["assetType"]["id"])).collect()).unwrap_or_default();
        eprintln!("avatar desc: r15={r15} scales {} assets {types:?}", info["scales"]);
    }
    let rig = &rigs[if r15 { "R15" } else { "R6" }];
    {
        let mut ids: Vec<u64> = info["assets"].as_array().map(|a| a.iter().filter_map(|x| x["id"].as_u64()).collect()).unwrap_or_default();
        ids.extend(rig["idle"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64()).collect::<Vec<_>>()).unwrap_or_default());
        ids.extend(rig["toolnone"].as_u64());
        prefetch(&l, &ids);
    }
    let sc = &info["scales"];
    let f = |k: &str| sc[k].as_f64().unwrap_or(1.0) as f32;
    let fz = |k: &str| sc[k].as_f64().unwrap_or(0.0) as f32;
    let scales = Scales { r15, body: Vec3::new(f("width"), f("height"), f("depth")), head: f("head"), body_type: fz("bodyType"), proportion: fz("proportion") };
    let head_scale = scales.part("Head", "Classic");
    let bc = &info["bodyColor3s"];
    let col = |k: &str| hex_color(bc[k].as_str().unwrap_or("A3A2A5"));
    let color_of = |name: &str| -> Vec3 {
        match name {
            "Head" => col("headColor3"),
            n if n.contains("Torso") => col("torsoColor3"),
            n if n.starts_with("Left") && (n.contains("Arm") || n.contains("Hand")) => col("leftArmColor3"),
            n if n.starts_with("Right") && (n.contains("Arm") || n.contains("Hand")) => col("rightArmColor3"),
            n if n.starts_with("Left") => col("leftLegColor3"),
            n if n.starts_with("Right") => col("rightLegColor3"),
            _ => col("torsoColor3"),
        }
    };

    // stock rig
    let mut parts: Vec<APart> = Vec::new();
    let mut part_mesh: HashMap<String, (String, Option<String>)> = HashMap::new();
    for p in rig["parts"].as_array().cloned().unwrap_or_default() {
        let name = p["name"].as_str().unwrap_or("").to_string();
        let s = scales.part(&name, "Classic");
        let mut att = HashMap::new();
        for a in p["attachments"].as_array().cloned().unwrap_or_default() {
            att.insert(a["name"].as_str().unwrap_or("").to_string(), scale_cf(&cf_from(&a["cf"]), s));
        }
        if let Some(m) = p["mesh"].as_str() {
            part_mesh.insert(name.clone(), (m.to_string(), None));
        }
        parts.push(APart {
            visible: name != "HumanoidRootPart",
            color: color_of(&name),
            size: v3(&p["size"]) * s,
            mesh: MeshKey::Box,
            mesh_scale: v3(&p["size"]) * s,
            mesh_offset: Vec3::ZERO,
            texture: None,
            attachments: att,
            weld: None,
            name,
        });
    }
    let index = |parts: &Vec<APart>, n: &str| parts.iter().position(|p| p.name == n);

    let mut shirt = None;
    let mut pants = None;
    let mut tshirt = None;
    let mut face = None;
    let mut head_mesh: Option<(String, Vec3)> = None;
    let mut head_tex: Option<String> = None;
    let mut head_no_face = false;
    let mut head_type = String::from("Classic");
    let mut r6_meshes: HashMap<u32, (u64, u64)> = HashMap::new();
    let mut accessories: Vec<Dom> = Vec::new();
    let mut layered: Vec<Layered> = Vec::new();
    // body part -> its WrapTarget cage (mesh, CageOrigin, the part's scale)
    let mut body_cages: HashMap<String, (String, CFrame, Vec3)> = HashMap::new();
    // accessory index -> its "meta": position, rotation (degrees), scale
    let mut acc_meta: HashMap<usize, (Vec3, Vec3, Vec3)> = HashMap::new();
    let mut tools: Vec<Dom> = Vec::new();
    let mut idle_ids: Vec<(f32, u64)> = Vec::new();
    let mut meshes = Vec::new();
    let mut textures = Vec::new();

    let mut worn = info["assets"].as_array().cloned().unwrap_or_default();
    // debug: XBOX_LAYERED_TEST=id,id adds layered items to this render only (nothing is saved)
    if let Ok(t) = std::env::var("XBOX_LAYERED_TEST") {
        worn.extend(t.split(',').filter_map(|i| i.trim().parse::<u64>().ok()).map(|id| serde_json::json!({ "id": id, "assetType": { "id": 67 } })));
    }
    for a in worn {
        let id = a["id"].as_u64().unwrap_or(0);
        let ty = a["assetType"]["id"].as_u64().unwrap_or(0);
        let Ok(dom) = l.model(id) else { continue };
        let first = |class: &str| dom.instances.iter().find(|i| i.class == class);
        match ty {
            2 => tshirt = first("ShirtGraphic").and_then(|i| content_id(i.get("Graphic"))),
            11 => shirt = first("Shirt").and_then(|i| content_id(i.get("ShirtTemplate"))),
            12 => pants = first("Pants").and_then(|i| content_id(i.get("PantsTemplate"))),
            18 => face = dom.instances.iter().find(|i| i.class == "Decal").and_then(|i| content_id(i.get("Texture"))),
            // classic head (SpecialMesh) or a MeshPart / dynamic head (type 79)
            17 | 79 => {
                if let Some(mi) = dom.instances.iter().position(|i| i.class == "SpecialMesh") {
                    let m = &dom.instances[mi];
                    let mid = content_id(m.get("MeshId")).unwrap_or_default();
                    head_mesh = Some((if mid.is_empty() { "rbxasset://fonts/head.mesh".into() } else { mid }, m.vec3("Scale").unwrap_or(Vec3::ONE)));
                    // R15 / dynamic heads ship as a SpecialMesh with their own texture ("NoFace":
                    // the face is painted in), scale type and attachment points (Vector3Values)
                    head_tex = content_id(m.get("TextureId")).filter(|t| !t.is_empty());
                    head_no_face = m.string("Tags").is_some_and(|t| t.contains("NoFace"));
                    head_type = part_scale_type(&dom, mi);
                    if let Some(k) = index(&parts, "Head") {
                        let s = scales.part("Head", &head_type);
                        for &c in &m.children {
                            let ci = &dom.instances[c];
                            if ci.class == "Vector3Value" && ci.name.ends_with("Attachment") {
                                let rot = parts[k].attachments.get(&ci.name).map(|a| a.rot).unwrap_or(CFrame::default().rot);
                                parts[k].attachments.insert(ci.name.clone(), CFrame { pos: ci.vec3("Value").unwrap_or_default() * s, rot });
                            }
                        }
                    }
                } else if let Some(hi) = dom.instances.iter().position(|i| i.class == "MeshPart" && i.name == "Head") {
                    let mp = &dom.instances[hi];
                    part_mesh.insert("Head".into(), (part_mesh_id(&dom, hi), part_texture(&dom, hi)));
                    // a MeshPart head brings its own size and attachments (hat, face, neck)
                    if let Some(k) = index(&parts, "Head") {
                        let s = scales.part("Head", &part_scale_type(&dom, hi));
                        if let Some(size) = mp.vec3("size").or_else(|| mp.vec3("Size")) {
                            parts[k].size = size * s;
                        }
                        for &c in &mp.children {
                            let ci = &dom.instances[c];
                            if ci.class == "Attachment" {
                                parts[k].attachments.insert(ci.name.clone(), scale_cf(&ci.cframe("CFrame").unwrap_or_default(), s));
                            }
                        }
                    }
                }
            }
            27..=31 => {
                if r15 {
                    // body package: prefer the artist-intent meshes
                    let folder = ["R15ArtistIntent", "R15Fixed", "R15"].iter().find_map(|n| dom.instances.iter().position(|i| i.name == *n));
                    let mut ds = Vec::new();
                    if let Some(fo) = folder {
                        dom.descendants(fo, &mut ds);
                    }
                    for i in ds {
                        let mp = &dom.instances[i];
                        if mp.class != "MeshPart" {
                            continue;
                        }
                        let Some(k) = index(&parts, &mp.name) else { continue };
                        let stock = parts[k].size / scales.part(&mp.name, "Classic");
                        let s = scales.part(&mp.name, &part_scale_type(&dom, i));
                        parts[k].size = mp.vec3("size").or_else(|| mp.vec3("Size")).unwrap_or(stock) * s;
                        for &c in &mp.children {
                            let ci = &dom.instances[c];
                            if ci.class == "Attachment" {
                                parts[k].attachments.insert(ci.name.clone(), scale_cf(&ci.cframe("CFrame").unwrap_or_default(), s));
                            }
                        }
                        part_mesh.insert(mp.name.clone(), (part_mesh_id(&dom, i), part_texture(&dom, i)));
                        if let Some(wt) = mp.children.iter().map(|&c| &dom.instances[c]).find(|c| c.class == "WrapTarget") {
                            if let Some(u) = content_id(wt.get("CageMeshId")) {
                                body_cages.insert(mp.name.clone(), (u, wt.cframe("CageOrigin").unwrap_or_default(), s));
                            }
                        }
                    }
                } else {
                    for cm in dom.instances.iter().filter(|i| i.class == "CharacterMesh") {
                        let mesh_id = match cm.get("MeshId") {
                            Some(rbxl::Value::Int64(v)) => *v as u64,
                            Some(rbxl::Value::Int(v)) => *v as u64,
                            _ => 0,
                        };
                        let tex = match cm.get("BaseTextureId") {
                            Some(rbxl::Value::Int64(v)) => *v as u64,
                            Some(rbxl::Value::Int(v)) => *v as u64,
                            _ => 0,
                        };
                        r6_meshes.insert(cm.enum_("BodyPart").unwrap_or(1), (mesh_id, tex));
                    }
                }
            }
            8 | 41..=47 | 57 | 58 | 64..=72 => {
                let m = &a["meta"];
                if m.is_object() {
                    let v = |k: &str, d: f32| {
                        let c = |a: &str| m[k][a].as_f64().map(|x| x as f32).unwrap_or(d);
                        Vec3::new(c("X"), c("Y"), c("Z"))
                    };
                    acc_meta.insert(accessories.len(), (v("position", 0.0), v("rotation", 0.0), v("scale", 1.0)));
                }
                accessories.push(dom);
            }
            19 => tools.push(dom),
            51 => {
                // IdleAnimation pack: StringValue "idle" > Animation (+ Weight)
                for an in dom.instances.iter().filter(|i| i.class == "Animation") {
                    if let Some(aid) = content_id(an.get("AnimationId")).and_then(|u| parse_asset_id(&u)) {
                        let w = an.children.iter().map(|&c| &dom.instances[c]).find(|c| c.name == "Weight").and_then(|c| c.f32("Value")).unwrap_or(1.0);
                        idle_ids.push((w, aid));
                    }
                }
            }
            _ => {}
        }
    }

    // body meshes, clothing composites
    let mut atlases: HashMap<String, Rgba> = HashMap::new();
    let colors = [
        ("CompositTorsoBase", col("torsoColor3")),
        ("CompositLeftArmBase", col("leftArmColor3")),
        ("CompositRightArmBase", col("rightArmColor3")),
        ("CompositLeftLegBase", col("leftLegColor3")),
        ("CompositRightLegBase", col("rightLegColor3")),
    ];
    let mut shirt_img = shirt.as_deref().and_then(|u| l.image(u));
    let mut pants_img = pants.as_deref().and_then(|u| l.image(u));
    // debug: a labelled template on every face (XBOX_TEMPLATE_TEST=path.png)
    if let Some(t) = std::env::var("XBOX_TEMPLATE_TEST").ok().and_then(|p| std::fs::read(p).ok()).and_then(|b| decode_image(&b).ok()) {
        shirt_img = Some(Rgba { w: t.w, h: t.h, data: t.data.clone() });
        pants_img = Some(t);
    }
    let tshirt_img = tshirt.as_deref().and_then(|u| l.image(u));
    for k in 0..parts.len() {
        let name = parts[k].name.clone();
        if name == "HumanoidRootPart" {
            continue;
        }
        let color = parts[k].color;
        let key = format!("avatar://{uid}/{name}");
        if name == "Head" {
            let (uri, tex) = part_mesh.get("Head").cloned().unwrap_or_else(|| (String::new(), None));
            let tex = tex.or_else(|| head_tex.clone());
            let m = if !uri.is_empty() {
                l.mesh(&uri, Some(parts[k].size), Vec3::ONE)
            } else {
                let (u, s) = head_mesh.clone().unwrap_or(("rbxasset://fonts/head.mesh".into(), Vec3::splat(1.25)));
                if u.ends_with("head.mesh") {
                    l.mesh(&u, Some(Vec3::new(2.0, 0.96, 1.0) * s * head_scale), Vec3::ONE)
                } else {
                    // a SpecialMesh head draws at the mesh's own size; the Head part takes it on
                    let m = l.mesh(&u, None, s * scales.part("Head", &head_type));
                    if let Some(m) = &m {
                        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
                        for v in &m.vertices {
                            lo = lo.min(Vec3::from(v.pos));
                            hi = hi.max(Vec3::from(v.pos));
                        }
                        if lo.x < hi.x {
                            parts[k].size = hi - lo;
                        }
                    }
                    m
                }
            };
            let mut img = match tex.as_deref().and_then(|t| l.image(t)) {
                Some(t) => {
                    let mut base = filled(t.w, t.h, color);
                    blit(&mut base, &t, None);
                    base
                }
                None => filled(256, 256, color),
            };
            // the face decal maps onto the head mesh's UVs; a dynamic head (a textured MeshPart
            // head) has its face painted in and no decal
            let dynamic = head_no_face || (!uri.is_empty() && tex.is_some());
            let face_img = if dynamic { None } else { face.as_deref().and_then(|u| l.image(u)).or_else(|| l.image("rbxasset://textures/face.png")) };
            if let Some(fi) = face_img {
                blit(&mut img, &fi, None);
            }
            if let Some(m) = m {
                let mk = new_key();
                meshes.push((mk, m));
                parts[k].mesh = mk;
                parts[k].mesh_scale = Vec3::ONE;
                textures.push((key.clone(), img));
                parts[k].texture = Some(key);
                parts[k].color = Vec3::ONE;
            }
            continue;
        }
        // clothing template for this part
        let mut tmpl = filled(TW as u32, TH as u32, color);
        let legs = name.contains("Leg") || name.contains("Foot");
        let arms = name.contains("Arm") || name.contains("Hand");
        let torso = name.contains("Torso");
        if legs || torso {
            if let Some(p) = &pants_img {
                blit(&mut tmpl, p, None);
            }
        }
        if arms || torso {
            if let Some(s) = &shirt_img {
                blit(&mut tmpl, s, None);
            }
        }
        if torso && shirt_img.is_none() {
            if let Some(t) = &tshirt_img {
                blit(&mut tmpl, t, Some(TORSO[5]));
            }
        }
        let mut mesh_data = None;
        let mut atlas_key: Option<String> = None;
        if r15 {
            if let Some((uri, tex)) = part_mesh.get(&name).cloned() {
                if !uri.is_empty() {
                    mesh_data = l.mesh(&uri, Some(parts[k].size), Vec3::ONE).map(flip_v);
                }
                // R15 limbs get their own texture, laid out by the engine's R15Composit*Base meshes
                // from the clothing template; a package texture sits under the clothing
                let package = tex.as_deref().and_then(|t| l.image(t));
                let mut layer = Rgba { w: TW as u32, h: TH as u32, data: vec![0; (TW * TH * 4.0) as usize] };
                if legs || torso {
                    if let Some(p) = &pants_img {
                        blit(&mut layer, p, None);
                    }
                }
                if arms || torso {
                    if let Some(sh) = &shirt_img {
                        blit(&mut layer, sh, None);
                    }
                }
                if torso && shirt_img.is_none() {
                    if let Some(t) = &tshirt_img {
                        blit(&mut layer, t, Some(TORSO[5]));
                    }
                }
                if let Some(t) = r15_limb_texture(content, &name, color, package.as_ref(), &layer) {
                    tmpl = t;
                }
            }
        } else {
            let body_part = match name.as_str() {
                "Torso" => 1,
                "Left Arm" => 2,
                "Right Arm" => 3,
                "Left Leg" => 4,
                _ => 5,
            };
            if let Some((mid, tex)) = r6_meshes.get(&body_part).copied().filter(|m| m.0 != 0) {
                // CharacterMesh draws the mesh at its own size around the limb's centre (R6 is
                // never scaled), not stretched to the part
                mesh_data = l.mesh(&format!("rbxassetid://{mid}"), None, Vec3::ONE).map(flip_v);
                // package meshes sample the same composited atlas as R15 bodies
                let akey = format!("avatar://{uid}/atlas/{tex}");
                if !atlases.contains_key(&akey) {
                    let package = (tex != 0).then(|| l.image(&format!("rbxassetid://{tex}"))).flatten();
                    atlases.insert(akey.clone(), r15_atlas(content, &colors, package.as_ref(), pants_img.as_ref(), shirt_img.as_ref(), tshirt_img.as_ref()));
                }
                atlas_key = Some(akey);
            }
            if mesh_data.is_none() {
                let faces = match name.as_str() {
                    "Torso" => &TORSO,
                    "Right Arm" | "Right Leg" => &RIGHT_LIMB,
                    _ => &LEFT_LIMB,
                };
                mesh_data = Some(template_box(parts[k].size, faces));
            }
        }
        if let (Some(m), Ok(_)) = (&mesh_data, std::env::var("XBOX_AVATAR_DEBUG")) {
            let (mut lo, mut hi) = (Vec2::splat(9.0), Vec2::splat(-9.0));
            for v in &m.vertices {
                lo = lo.min(Vec2::from(v.uv));
                hi = hi.max(Vec2::from(v.uv));
            }
            eprintln!("avatar uv {name}: {:.3?} .. {:.3?} mesh {:?}", lo, hi, part_mesh.get(&name).map(|p| p.0.clone()));
        }
        if let Some(m) = mesh_data {
            let mk = new_key();
            meshes.push((mk, m));
            parts[k].mesh = mk;
            parts[k].mesh_scale = Vec3::ONE;
            match atlas_key {
                Some(a) => parts[k].texture = Some(a),
                None => {
                    textures.push((key.clone(), tmpl));
                    parts[k].texture = Some(key);
                }
            }
            parts[k].color = Vec3::ONE;
        }
    }
    textures.extend(atlases);

    // joints: R15 from the rig attachments (so packages line up), R6 from the stock Motor6Ds
    let mut joints = Vec::new();
    for j in rig["joints"].as_array().cloned().unwrap_or_default() {
        let (Some(p0), Some(p1)) = (index(&parts, j["part0"].as_str().unwrap_or("")), index(&parts, j["part1"].as_str().unwrap_or(""))) else { continue };
        let name = j["name"].as_str().unwrap_or("");
        let att = format!("{name}RigAttachment");
        let (c0, c1) = match (r15, parts[p0].attachments.get(&att), parts[p1].attachments.get(&att)) {
            (true, Some(a), Some(b)) => (*a, *b),
            _ => (cf_from(&j["c0"]), cf_from(&j["c1"])),
        };
        joints.push(AJoint { part0: p0, part1: p1, c0, c1 });
    }

    // accessories: Handle welded by matching attachments (or the old Hat AttachmentPoint)
    for (acc_i, dom) in accessories.iter().enumerate() {
        let Some(acc) = dom.instances.iter().position(|i| i.class == "Accessory" || i.class == "Hat") else { continue };
        let Some(h) = dom.find_child(acc, "Handle") else { continue };
        let hi = &dom.instances[h];
        let size = hi.vec3("size").or_else(|| hi.vec3("Size")).unwrap_or(Vec3::ONE);
        let (mut uri, mut tex, mut scale, mut offset, mut fit) = (String::new(), None, Vec3::ONE, Vec3::ZERO, None);
        if hi.class == "MeshPart" {
            uri = part_mesh_id(dom, h);
            tex = part_texture(dom, h);
            fit = Some(size);
        } else if let Some(m) = hi.children.iter().map(|&c| &dom.instances[c]).find(|c| c.class == "SpecialMesh") {
            uri = content_id(m.get("MeshId")).unwrap_or_default();
            tex = content_id(m.get("TextureId"));
            scale = m.vec3("Scale").unwrap_or(Vec3::ONE);
            offset = m.vec3("Offset").unwrap_or(Vec3::ZERO);
        }
        if uri.is_empty() {
            continue;
        }
        // where it attaches: the accessory scales like the body part it sits on (as if that part
        // were authored the accessory's way); the owner's own adjustment (avatar API "meta")
        // scales, moves and turns it at the attachment
        let meta = acc_meta.get(&acc_i).copied();
        let acc_type = part_scale_type(dom, h);
        let handle_att = hi.children.iter().map(|&c| &dom.instances[c]).find(|c| c.class == "Attachment");
        let mut weld = None;
        let mut s = head_scale;
        if let Some(ha) = handle_att {
            let hcf = ha.cframe("CFrame").unwrap_or_default();
            for (pi, p) in parts.iter().enumerate() {
                if let Some(bcf) = p.attachments.get(&ha.name) {
                    let fit_scale = scales.part(&p.name, &acc_type);
                    let (m_pos, m_rot, m_scale) = meta.unwrap_or((Vec3::ZERO, Vec3::ZERO, Vec3::ONE));
                    s = fit_scale * m_scale;
                    let turn = CFrame::angles(m_rot.x.to_radians(), m_rot.y.to_radians(), m_rot.z.to_radians());
                    let hcf = CFrame { pos: hcf.pos * s - m_pos * fit_scale, rot: hcf.rot }.mul(&turn.inverse());
                    weld = Some((pi, bcf.mul(&hcf.inverse())));
                    break;
                }
            }
        }
        if weld.is_none() {
            let ap = dom.instances[acc].cframe("AttachmentPoint").unwrap_or_default();
            if let Some(head) = index(&parts, "Head") {
                weld = Some((head, CFrame::new(Vec3::new(0.0, 0.5, 0.0)).mul(&ap.inverse())));
            }
        }
        if std::env::var("XBOX_AVATAR_DEBUG").is_ok() {
            eprintln!("avatar acc {} att {:?} cf {:?}", dom.instances[acc].name, handle_att.map(|a| a.name.clone()), handle_att.and_then(|a| a.cframe("CFrame")));
        }
        let Some(weld) = weld else { continue };
        let Some(m) = l.mesh(&uri, fit.map(|f| f * s), scale * s) else { continue };
        let key = format!("avatar://{uid}/acc{}_{acc_i}", parts.len());
        let has_tex = match tex.as_deref().and_then(|t| l.image(t)) {
            Some(img) => {
                textures.push((key.clone(), img));
                true
            }
            None => false,
        };
        // layered clothing is fitted to the body after the rig is built
        if let Some(w) = hi.children.iter().map(|&c| &dom.instances[c]).find(|c| c.class == "WrapLayer") {
            let reference = content_id(w.get("ReferenceMeshId")).and_then(|u| l.mesh(&u, None, s)).map(|mut c| {
                let o = w.cframe("ReferenceOrigin").unwrap_or_default();
                let o = CFrame { pos: o.pos * s, rot: o.rot };
                for v in c.vertices.iter_mut() {
                    v.pos = o.point(Vec3::from(v.pos)).into();
                }
                c
            });
            let order = match w.get("Order") {
                Some(rbxl::Value::Int(o)) => *o,
                _ => 1,
            };
            layered.push(Layered {
                name: dom.instances[acc].name.clone(),
                mesh: m,
                texture: has_tex.then_some(key),
                color: if has_tex { Vec3::ONE } else { Vec3::splat(0.64) },
                weld,
                reference,
                order,
            });
            continue;
        }
        let mk = new_key();
        meshes.push((mk, m));
        parts.push(APart {
            name: dom.instances[acc].name.clone(),
            size,
            mesh: mk,
            mesh_scale: Vec3::ONE,
            mesh_offset: offset * s,
            color: if has_tex { Vec3::ONE } else { Vec3::splat(0.64) },
            texture: has_tex.then_some(key),
            attachments: HashMap::new(),
            weld: Some((weld.0, scale_cf(&weld.1, Vec3::ONE))),
            visible: true,
        });
    }

    // gear: Handle held at the right-hand grip (Tool.Grip), arm raised by "toolnone"
    let mut holding = false;
    for dom in &tools {
        let Some(t) = dom.instances.iter().position(|i| i.class == "Tool" || i.class == "HopperBin") else { continue };
        let Some(h) = dom.find_child(t, "Handle") else { continue };
        let grip = dom.instances[t].cframe("Grip").unwrap_or_default();
        let hi = &dom.instances[h];
        let size = hi.vec3("size").or_else(|| hi.vec3("Size")).unwrap_or(Vec3::ONE);
        let (mut uri, mut tex, mut scale, mut offset, mut fit) = (String::new(), None, Vec3::ONE, Vec3::ZERO, None);
        if hi.class == "MeshPart" {
            uri = part_mesh_id(dom, h);
            tex = part_texture(dom, h);
            fit = Some(size);
        } else if let Some(m) = hi.children.iter().map(|&c| &dom.instances[c]).find(|c| c.class == "SpecialMesh") {
            uri = content_id(m.get("MeshId")).unwrap_or_default();
            tex = content_id(m.get("TextureId"));
            scale = m.vec3("Scale").unwrap_or(Vec3::ONE);
            offset = m.vec3("Offset").unwrap_or(Vec3::ZERO);
        }
        let weld = if r15 {
            index(&parts, "RightHand").map(|p| (p, parts[p].attachments.get("RightGripAttachment").copied().unwrap_or_default().mul(&grip.inverse())))
        } else {
            index(&parts, "Right Arm").map(|p| (p, CFrame::new(Vec3::new(0.0, -1.0, 0.0)).mul(&CFrame::angles(-std::f32::consts::FRAC_PI_2, 0.0, 0.0)).mul(&grip.inverse())))
        };
        let Some(weld) = weld else { continue };
        let mesh = if uri.is_empty() {
            let mut m = mesh::box_mesh();
            for v in m.vertices.iter_mut() {
                v.pos = (Vec3::from(v.pos) * size).into();
            }
            Some(m)
        } else {
            l.mesh(&uri, fit, scale)
        };
        let Some(m) = mesh else { continue };
        let mk = new_key();
        meshes.push((mk, m));
        let key = format!("avatar://{uid}/tool{}", parts.len());
        let has_tex = match tex.as_deref().and_then(|t| l.image(t)) {
            Some(img) => {
                textures.push((key.clone(), img));
                true
            }
            None => false,
        };
        parts.push(APart {
            name: dom.instances[t].name.clone(),
            size,
            mesh: mk,
            mesh_scale: Vec3::ONE,
            mesh_offset: offset,
            color: if has_tex { Vec3::ONE } else { hi.vec3("Color3uint8").or_else(|| hi.vec3("Color")).unwrap_or(Vec3::splat(0.64)) },
            texture: has_tex.then_some(key),
            attachments: HashMap::new(),
            weld: Some(weld),
            visible: true,
        });
        holding = true;
    }
    let tool = if holding { rig["toolnone"].as_u64().and_then(|id| l.model(id).ok()).and_then(|d| load_anim(&d)) } else { None };

    // idle animation (the account's pack, otherwise the Animate script defaults)
    if idle_ids.is_empty() {
        let w = [9.0, 1.0, 1.0];
        for (i, id) in rig["idle"].as_array().cloned().unwrap_or_default().iter().enumerate() {
            idle_ids.push((w.get(i).copied().unwrap_or(1.0), id.as_u64().unwrap_or(0)));
        }
    }
    let mut idle = Vec::new();
    for (w, id) in idle_ids {
        let parsed = l.model(id).ok();
        if std::env::var("XBOX_AVATAR_DEBUG").is_ok() {
            eprintln!("avatar idle {id} w={w}: {:?}", parsed.as_ref().map(|d| d.instances.iter().take(4).map(|i| format!("{}:{}", i.class, i.name)).collect::<Vec<_>>()));
        }
        let anim = parsed.and_then(|d| load_anim(&d));
        if std::env::var("XBOX_AVATAR_DEBUG").is_ok() {
            eprintln!("avatar idle {id}: {:?}", anim.as_ref().map(|a| (a.length, a.tracks.len(), a.sample("RightUpperArm", 0.0).map(|c| c.to_quat()))));
        }
        if let Some(a) = anim {
            idle.push((w, a));
        }
    }

    let mut model = AvatarModel { uid, r15, parts, joints, meshes, textures, idle, tool, center: Vec3::ZERO, height: 5.0 };
    if !layered.is_empty() {
        fit_layered(&l, &mut model, &body_cages, layered);
    }
    // framing from the rest pose
    let rest = model.pose(None, 0.0);
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    for p in rest.iter().filter(|p| p.visible) {
        lo = lo.min(p.cf.pos - p.size * 0.5);
        hi = hi.max(p.cf.pos + p.size * 0.5);
    }
    if lo.x < hi.x {
        model.center = (lo + hi) * 0.5;
        model.height = hi.y - lo.y;
    }
    if model.parts.iter().filter(|p| p.visible).count() == 0 {
        return Err(anyhow!("avatar had no parts"));
    }
    if std::env::var("XBOX_AVATAR_DEBUG").is_ok() {
        // rest pose relative to the HumanoidRootPart, to compare with Studio's built rig
        for (p, a) in model.pose(None, 0.0).iter().zip(&model.parts) {
            eprintln!("avatar part {} size=({:.3},{:.3},{:.3}) pos=({:.3},{:.3},{:.3})", a.name, a.size.x, a.size.y, a.size.z, p.cf.pos.x, p.cf.pos.y, p.cf.pos.z);
        }
    }
    Ok(model)
}

impl AvatarModel {
    /// Solve the rig for animation `anim` at time `t`: Part1 = Part0 * C0 * Transform * C1^-1.
    pub fn pose(&self, anim: Option<&Anim>, t: f32) -> Vec<scene::Part> {
        self.pose_with(&|name| self.joint(anim, t, name))
    }

    /// One joint's Transform for `anim` at `t` (a held gear's pose wins for its arm).
    pub fn joint(&self, anim: Option<&Anim>, t: f32, name: &str) -> CFrame {
        self.tool.as_ref().and_then(|a| a.sample(name, t)).or_else(|| anim.and_then(|a| a.sample(name, t))).unwrap_or_default()
    }

    /// The names of the parts moved by joints (the keys `joint` is asked for).
    pub fn joint_names(&self) -> impl Iterator<Item = &str> {
        self.joints.iter().map(|j| self.parts[j.part1].name.as_str())
    }

    /// Solve the rig with each joint's Transform given by `tr` (by Part1's name).
    pub fn pose_with(&self, tr: &dyn Fn(&str) -> CFrame) -> Vec<scene::Part> {
        let cf = self.solve(tr);
        self.parts
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                let c = cf[i]?;
                Some(scene::Part {
                    inst: 0,
                    cf: c,
                    size: p.size,
                    mesh: p.mesh,
                    mesh_scale: p.mesh_scale,
                    mesh_offset: p.mesh_offset,
                    fallback_scale: p.size,
                    color: p.color,
                    alpha: 1.0,
                    reflectance: 0.0,
                    material: 272,
                    surfaces: 0,
                    flags: if p.texture.is_some() { scene::FLAG_TEXTURED } else { 0 },
                    texture: p.texture.clone(),
                    visible: p.visible,
                })
            })
            .collect()
    }

    /// Every part's CFrame in the rest pose (no animation), relative to the HumanoidRootPart.
    fn rest(&self) -> Vec<Option<CFrame>> {
        self.solve(&|_| CFrame::default())
    }

    fn solve(&self, tr: &dyn Fn(&str) -> CFrame) -> Vec<Option<CFrame>> {
        let n = self.parts.len();
        let mut cf: Vec<Option<CFrame>> = vec![None; n];
        let root = self.parts.iter().position(|p| p.name == "HumanoidRootPart").unwrap_or(0);
        cf[root] = Some(CFrame::default());
        // joints resolve in dependency order
        let mut progress = true;
        while progress {
            progress = false;
            for j in &self.joints {
                if cf[j.part1].is_some() {
                    continue;
                }
                let Some(c0) = cf[j.part0] else { continue };
                let name = &self.parts[j.part1].name;
                cf[j.part1] = Some(c0.mul(&j.c0).mul(&tr(name)).mul(&j.c1.inverse()));
                progress = true;
            }
        }
        for (i, p) in self.parts.iter().enumerate() {
            if let Some((b, off)) = p.weld {
                if let Some(base) = cf[b] {
                    cf[i] = Some(base.mul(&off));
                }
            }
        }
        cf
    }
}

/// Plays the weighted idle set: a new pick at the end of every loop (Animate script behaviour).
pub struct Player {
    current: usize,
    t0: f32,
}

impl Player {
    pub fn new() -> Self {
        Player { current: 0, t0: 0.0 }
    }

    pub fn frame<'a>(&mut self, model: &'a AvatarModel, now: f32) -> (Option<&'a Anim>, f32) {
        if model.idle.is_empty() {
            return (None, 0.0);
        }
        self.current = self.current.min(model.idle.len() - 1);
        let len = model.idle[self.current].1.length;
        if now - self.t0 >= len {
            self.t0 = now;
            let total: f32 = model.idle.iter().map(|a| a.0).sum();
            let mut r = rand::random::<f32>() * total;
            self.current = model.idle.iter().position(|a| {
                r -= a.0;
                r <= 0.0
            }).unwrap_or(0);
        }
        (Some(&model.idle[self.current].1), now - self.t0)
    }
}

/// Fallback when the room has no CharacterPlacement: Workspace.NewModel.CharacterPlacement, facing +X.
fn default_stage() -> CFrame {
    CFrame { pos: Vec3::new(-2.127, 3.452, 0.135), rot: Mat3::from_rotation_y(-std::f32::consts::FRAC_PI_2) }
}

/// The posed avatar with its HumanoidRootPart at the placement part, turned by `yaw`.
pub fn place_on_stage(stage: Option<CFrame>, parts: Vec<scene::Part>, yaw: f32) -> Vec<scene::Part> {
    let base = stage.unwrap_or_else(default_stage).mul(&CFrame { pos: Vec3::ZERO, rot: Mat3::from_rotation_y(yaw) });
    parts.into_iter().map(|mut p| {
        p.cf = base.mul(&p.cf);
        p
    }).collect()
}

/// The editor camera: straight in front of the placement part, looking back at the character.
pub fn editor_camera(stage: Option<CFrame>) -> CFrame {
    let st = stage.unwrap_or_else(default_stage);
    let front = -st.rot.z_axis;
    // panned so the avatar sits just left of centre and fills ~60% of the height, like the console
    let pan = front.cross(Vec3::Y).normalize() * -0.45;
    let mut target = st.pos + Vec3::Y * -0.3 + pan;
    let mut eye = st.pos + front * 7.6 + Vec3::Y * 0.55 + pan;
    // debug: XBOX_EDITOR_CAM="ex,ey,ez,tx,ty,tz"
    if let Some(v) = std::env::var("XBOX_EDITOR_CAM").ok().map(|s| s.split(',').filter_map(|x| x.parse::<f32>().ok()).collect::<Vec<_>>()).filter(|v| v.len() == 6) {
        eye = Vec3::new(v[0], v[1], v[2]);
        target = Vec3::new(v[3], v[4], v[5]);
    }
    let look = (target - eye).normalize();
    let right = look.cross(Vec3::Y).normalize();
    let up = right.cross(look);
    CFrame { pos: eye, rot: Mat3::from_cols(right, up, -look) }
}

/// Camera framing the whole character from the front-left, like the avatar renders.
pub fn camera(model: &AvatarModel, fov_deg: f32, turn: f32) -> CFrame {
    let yaw = 20f32.to_radians() + turn;
    let dist = (model.height * 0.62) / (fov_deg.to_radians() * 0.5).tan();
    let dir = Vec3::new(yaw.sin(), 0.08, -yaw.cos()).normalize();
    let eye = model.center + dir * dist;
    let look = (model.center - eye).normalize();
    let right = look.cross(Vec3::Y).normalize();
    let up = right.cross(look);
    CFrame { pos: eye, rot: Mat3::from_cols(right, up, -look) }
}

// >> end << //

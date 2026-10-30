/*
    written by: @corescripts
*/

//! CPU-side mesh data: part primitives, Roblox `.mesh` files and CSG union meshes.

// >> imports
use anyhow::{anyhow, bail, Result};
use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use std::f32::consts::PI;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [u8; 4],
}

#[derive(Default, Clone)]
pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    fn push(&mut self, p: Vec3, n: Vec3, uv: Vec2) -> u32 {
        self.vertices.push(Vertex { pos: p.into(), normal: n.into(), uv: uv.into(), color: [255; 4] });
        (self.vertices.len() - 1) as u32
    }
    fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let i = self.push(a, n, Vec2::new(0.0, 1.0));
        self.push(b, n, Vec2::new(1.0, 1.0));
        self.push(c, n, Vec2::new(1.0, 0.0));
        self.push(d, n, Vec2::new(0.0, 0.0));
        self.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    }
    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3) {
        let n = (b - a).cross(c - a).normalize_or_zero();
        let i = self.push(a, n, Vec2::ZERO);
        self.push(b, n, Vec2::X);
        self.push(c, n, Vec2::Y);
        self.indices.extend_from_slice(&[i, i + 1, i + 2]);
    }

    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for v in &self.vertices {
            let p = Vec3::from(v.pos);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        (lo, hi)
    }

    /// Recompute smooth-ish normals where a mesh shipped without usable ones.
    fn ensure_normals(&mut self) {
        let bad = self.vertices.iter().filter(|v| Vec3::from(v.normal).length_squared() < 0.5).count();
        if bad * 4 < self.vertices.len() {
            return;
        }
        let mut acc = vec![Vec3::ZERO; self.vertices.len()];
        for t in self.indices.chunks(3) {
            let [a, b, c] = [t[0] as usize, t[1] as usize, t[2] as usize];
            let n = (Vec3::from(self.vertices[b].pos) - Vec3::from(self.vertices[a].pos))
                .cross(Vec3::from(self.vertices[c].pos) - Vec3::from(self.vertices[a].pos));
            acc[a] += n;
            acc[b] += n;
            acc[c] += n;
        }
        for (v, n) in self.vertices.iter_mut().zip(acc) {
            v.normal = n.normalize_or_zero().into();
        }
    }
}

// >> primitives
// (unit-sized, centred on the origin; scaled by the part size in the shader)

pub fn box_mesh() -> MeshData {
    let mut m = MeshData::default();
    let h = 0.5;
    let v = |x: f32, y: f32, z: f32| Vec3::new(x * h, y * h, z * h);
    m.quad(v(-1., 1., 1.), v(1., 1., 1.), v(1., 1., -1.), v(-1., 1., -1.)); // top
    m.quad(v(-1., -1., -1.), v(1., -1., -1.), v(1., -1., 1.), v(-1., -1., 1.)); // bottom
    m.quad(v(-1., -1., 1.), v(1., -1., 1.), v(1., 1., 1.), v(-1., 1., 1.)); // back (+z)
    m.quad(v(1., -1., -1.), v(-1., -1., -1.), v(-1., 1., -1.), v(1., 1., -1.)); // front (-z)
    m.quad(v(1., -1., 1.), v(1., -1., -1.), v(1., 1., -1.), v(1., 1., 1.)); // right
    m.quad(v(-1., -1., -1.), v(-1., -1., 1.), v(-1., 1., 1.), v(-1., 1., -1.)); // left
    m
}

pub fn wedge_mesh() -> MeshData {
    let mut m = MeshData::default();
    let h = 0.5;
    let v = |x: f32, y: f32, z: f32| Vec3::new(x * h, y * h, z * h);
    m.quad(v(-1., -1., -1.), v(1., -1., -1.), v(1., -1., 1.), v(-1., -1., 1.)); // bottom
    m.quad(v(-1., -1., 1.), v(1., -1., 1.), v(1., 1., 1.), v(-1., 1., 1.)); // back
    m.quad(v(1., -1., -1.), v(-1., -1., -1.), v(-1., 1., 1.), v(1., 1., 1.)); // slope
    m.tri(v(1., -1., 1.), v(1., -1., -1.), v(1., 1., 1.));
    m.tri(v(-1., -1., -1.), v(-1., -1., 1.), v(-1., 1., 1.));
    m
}

pub fn corner_wedge_mesh() -> MeshData {
    let mut m = MeshData::default();
    let h = 0.5;
    let v = |x: f32, y: f32, z: f32| Vec3::new(x * h, y * h, z * h);
    let top = v(1., 1., -1.);
    m.quad(v(-1., -1., -1.), v(1., -1., -1.), v(1., -1., 1.), v(-1., -1., 1.)); // bottom
    m.tri(v(1., -1., 1.), v(1., -1., -1.), top); // right
    m.tri(v(1., -1., -1.), v(-1., -1., -1.), top); // front
    m.tri(v(-1., -1., -1.), v(-1., -1., 1.), top); // slope left
    m.tri(v(-1., -1., 1.), v(1., -1., 1.), top); // slope back
    m
}

/// Cylinder along +Y with radius/half-height 0.5.
pub fn cylinder_y(segs: usize) -> MeshData {
    let mut m = MeshData::default();
    let ring = |i: usize| {
        let a = i as f32 / segs as f32 * 2.0 * PI;
        Vec2::new(a.cos() * 0.5, a.sin() * 0.5)
    };
    for i in 0..segs {
        let (a, b) = (ring(i), ring(i + 1));
        let na = Vec3::new(a.x, 0.0, a.y).normalize();
        let nb = Vec3::new(b.x, 0.0, b.y).normalize();
        let base = m.vertices.len() as u32;
        let u0 = i as f32 / segs as f32;
        let u1 = (i + 1) as f32 / segs as f32;
        m.push(Vec3::new(a.x, -0.5, a.y), na, Vec2::new(u0, 1.0));
        m.push(Vec3::new(b.x, -0.5, b.y), nb, Vec2::new(u1, 1.0));
        m.push(Vec3::new(b.x, 0.5, b.y), nb, Vec2::new(u1, 0.0));
        m.push(Vec3::new(a.x, 0.5, a.y), na, Vec2::new(u0, 0.0));
        m.indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        // caps
        let c = m.push(Vec3::new(0.0, 0.5, 0.0), Vec3::Y, Vec2::splat(0.5));
        let p = m.push(Vec3::new(a.x, 0.5, a.y), Vec3::Y, a + 0.5);
        let q = m.push(Vec3::new(b.x, 0.5, b.y), Vec3::Y, b + 0.5);
        m.indices.extend_from_slice(&[c, q, p]);
        let c = m.push(Vec3::new(0.0, -0.5, 0.0), -Vec3::Y, Vec2::splat(0.5));
        let p = m.push(Vec3::new(a.x, -0.5, a.y), -Vec3::Y, a + 0.5);
        let q = m.push(Vec3::new(b.x, -0.5, b.y), -Vec3::Y, b + 0.5);
        m.indices.extend_from_slice(&[c, p, q]);
    }
    m
}

/// Part.Shape = Cylinder runs along the part's X axis.
pub fn cylinder_x(segs: usize) -> MeshData {
    let mut m = cylinder_y(segs);
    for v in &mut m.vertices {
        let [x, y, z] = v.pos;
        v.pos = [y, -x, z];
        let [x, y, z] = v.normal;
        v.normal = [y, -x, z];
    }
    m
}

pub fn sphere(rings: usize, segs: usize) -> MeshData {
    let mut m = MeshData::default();
    for r in 0..=rings {
        let phi = r as f32 / rings as f32 * PI;
        for s in 0..=segs {
            let th = s as f32 / segs as f32 * 2.0 * PI;
            let n = Vec3::new(phi.sin() * th.cos(), phi.cos(), phi.sin() * th.sin());
            m.push(n * 0.5, n, Vec2::new(s as f32 / segs as f32, r as f32 / rings as f32));
        }
    }
    let w = segs as u32 + 1;
    for r in 0..rings as u32 {
        for s in 0..segs as u32 {
            let a = r * w + s;
            let b = a + w;
            m.indices.extend_from_slice(&[a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    m
}

/// Approximation of the classic SpecialMesh "Head" (rounded cylinder).
pub fn head_mesh() -> MeshData {
    let mut m = MeshData::default();
    let segs = 24;
    let rings = 12;
    for r in 0..=rings {
        let t = r as f32 / rings as f32;
        // y profile: flat sides with rounded top/bottom
        let phi = t * PI;
        let y = -(phi.cos()) * 0.5;
        let rad = (phi.sin().powf(0.35)) * 0.5;
        for s in 0..=segs {
            let th = s as f32 / segs as f32 * 2.0 * PI;
            let p = Vec3::new(rad * th.cos(), y, rad * th.sin());
            let n = Vec3::new(phi.sin() * th.cos(), -phi.cos() * 0.6, phi.sin() * th.sin()).normalize();
            m.push(p, n, Vec2::new(s as f32 / segs as f32, t));
        }
    }
    let w = segs as u32 + 1;
    for r in 0..rings as u32 {
        for s in 0..segs as u32 {
            let a = r * w + s;
            let b = a + w;
            m.indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    m
}

// >> roblox .mesh files

struct R<'a> {
    d: &'a [u8],
    p: usize,
}
impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.p + n > self.d.len() {
            bail!("mesh truncated");
        }
        let d: &'a [u8] = self.d;
        let s = &d[self.p..self.p + n];
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into()?))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into()?))
    }
}

fn read_vertex(r: &mut R, size: usize) -> Result<Vertex> {
    let start = r.p;
    let pos = [r.f32()?, r.f32()?, r.f32()?];
    let normal = [r.f32()?, r.f32()?, r.f32()?];
    let uv = [r.f32()?, r.f32()?];
    let mut color = [255u8; 4];
    // 36-byte vertices carry a 3rd (unused) uv component; 40-byte ones add tangent + rgba.
    let consumed = r.p - start;
    if size >= 40 {
        r.take(4)?; // tangent / uv.w
        let c = r.take(4)?;
        color = [c[0], c[1], c[2], c[3]];
    }
    let consumed2 = r.p - start;
    if size > consumed2 {
        r.take(size - consumed2)?;
    }
    let _ = consumed;
    Ok(Vertex { pos, normal, uv, color })
}

fn read_faces(r: &mut R, n: usize, out: &mut Vec<u32>) -> Result<()> {
    for _ in 0..n * 3 {
        out.push(r.u32()?);
    }
    Ok(())
}

pub fn parse_mesh(data: &[u8]) -> Result<MeshData> {
    let nl = data.iter().position(|&b| b == b'\n').unwrap_or(0);
    let header = String::from_utf8_lossy(&data[..nl.min(32)]).trim().to_string();
    let body = &data[(nl + 1).min(data.len())..];
    let mut m = MeshData::default();
    if header.starts_with("version 1.0") {
        let scale = if header.contains("1.00") { 0.5 } else { 1.0 };
        let text = String::from_utf8_lossy(body);
        let mut lines = text.lines();
        let nfaces: usize = lines.next().unwrap_or("0").trim().parse().unwrap_or(0);
        let rest: String = lines.collect::<Vec<_>>().join("");
        let nums: Vec<f32> = rest
            .split(|c| c == '[' || c == ']' || c == ',')
            .filter_map(|s| s.trim().parse::<f32>().ok())
            .collect();
        for (i, v) in nums.chunks(9).take(nfaces * 3).enumerate() {
            if v.len() < 9 {
                break;
            }
            m.vertices.push(Vertex {
                pos: [v[0] * scale, v[1] * scale, v[2] * scale],
                normal: [v[3], v[4], v[5]],
                uv: [v[6], 1.0 - v[7]],
                color: [255; 4],
            });
            m.indices.push(i as u32);
        }
    } else if header.starts_with("version 2.00") {
        let mut r = R { d: body, p: 0 };
        let hsize = r.u16()? as usize;
        let vsize = r.u8()? as usize;
        let _fsize = r.u8()?;
        let nv = r.u32()? as usize;
        let nf = r.u32()? as usize;
        r.p = hsize;
        for _ in 0..nv {
            m.vertices.push(read_vertex(&mut r, vsize)?);
        }
        read_faces(&mut r, nf, &mut m.indices)?;
    } else if header.starts_with("version 3.0") {
        let mut r = R { d: body, p: 0 };
        let hsize = r.u16()? as usize;
        let vsize = r.u8()? as usize;
        let _fsize = r.u8()?;
        let _lodsize = r.u16()?;
        let nlods = r.u16()? as usize;
        let nv = r.u32()? as usize;
        let nf = r.u32()? as usize;
        r.p = hsize;
        for _ in 0..nv {
            m.vertices.push(read_vertex(&mut r, vsize)?);
        }
        let mut faces = Vec::new();
        read_faces(&mut r, nf, &mut faces)?;
        let mut lods = Vec::new();
        for _ in 0..nlods {
            lods.push(r.u32()? as usize);
        }
        let end = if lods.len() >= 2 { lods[1].min(nf) } else { nf };
        m.indices = faces[..end * 3].to_vec();
    } else if header.starts_with("version 4.0") || header.starts_with("version 5.0") {
        let mut r = R { d: body, p: 0 };
        let hsize = r.u16()? as usize;
        let _lodtype = r.u16()?;
        let nv = r.u32()? as usize;
        let nf = r.u32()? as usize;
        let nlods = r.u16()? as usize;
        let nbones = r.u16()? as usize;
        r.p = hsize;
        for _ in 0..nv {
            m.vertices.push(read_vertex(&mut r, 40)?);
        }
        if nbones > 0 {
            r.take(nv * 8)?;
        }
        let mut faces = Vec::new();
        read_faces(&mut r, nf, &mut faces)?;
        let mut lods = Vec::new();
        for _ in 0..nlods {
            lods.push(r.u32()? as usize);
        }
        let end = if lods.len() >= 2 { lods[1].min(nf) } else { nf };
        m.indices = faces[..end * 3].to_vec();
    } else if header.starts_with("version 6.0") || header.starts_with("version 7.0") {
        // chunked: COREMESH v1 is plain; v2 is Draco-compressed (unsupported)
        let mut r = R { d: body, p: 0 };
        while r.p + 16 <= body.len() {
            let name = r.take(8)?;
            let ver = r.u32()?;
            let size = r.u32()? as usize;
            let chunk = r.take(size)?;
            if name == b"COREMESH" {
                if ver == 2 {
                    // u32 length, then a Draco bitstream
                    let n = u32::from_le_bytes(chunk.get(..4).ok_or_else(|| anyhow!("short COREMESH"))?.try_into()?) as usize;
                    decode_draco(chunk.get(4..4 + n).unwrap_or(&chunk[4..]), &mut m)?;
                    continue;
                }
                if ver != 1 {
                    bail!("COREMESH v{ver} not supported");
                }
                let mut c = R { d: chunk, p: 0 };
                let nv = c.u32()? as usize;
                for _ in 0..nv {
                    m.vertices.push(read_vertex(&mut c, 40)?);
                }
                let nf = c.u32()? as usize;
                read_faces(&mut c, nf, &mut m.indices)?;
            } else if name == b"LODS\0\0\0\0" && !m.indices.is_empty() {
                let mut c = R { d: chunk, p: 0 };
                c.u16()?;
                c.u8()?;
                let n = c.u32()? as usize;
                let mut lods = Vec::new();
                for _ in 0..n {
                    lods.push(c.u32()? as usize);
                }
                // v7 files can carry an empty table ([0, 0]): keep the whole mesh then
                if lods.len() >= 2 && lods[1] > 0 {
                    let end = lods[1] * 3;
                    m.indices.truncate(end);
                }
            }
        }
    } else {
        bail!("unsupported mesh header {header:?}");
    }
    let nv = m.vertices.len() as u32;
    m.indices.retain(|&i| i < nv);
    if m.indices.len() % 3 != 0 {
        let n = m.indices.len() / 3 * 3;
        m.indices.truncate(n);
    }
    for v in &mut m.vertices {
        // Roblox mesh UVs are top-down
        v.uv[1] = v.uv[1];
    }
    m.ensure_normals();
    Ok(m)
}

// >> csg
// (UnionOperation) mesh data
//
// Legacy union assets are "CSGMDL" blobs. Versions 2/4 are obfuscated with a static
// XOR key. Rather than depend on every revision's exact header, the decoder locates
// the vertex block (count + fixed-stride records) and the index block that follows.

const CSG_KEY: [u8; 31] = [
    86, 46, 110, 88, 49, 32, 48, 4, 52, 105, 12, 119, 12, 1, 94, 0, 26, 96, 55, 105, 29, 82, 43, 7, 79, 36, 89, 101, 83, 4, 122,
];

pub fn parse_csg(data: &[u8]) -> Result<MeshData> {
    if data.starts_with(b"version ") {
        return parse_mesh(data);
    }
    // 2016-era union assets are PartOperationAsset models carrying the mesh in MeshData
    if data.starts_with(b"<roblox!") {
        let dom = crate::rbxl::parse(data)?;
        for inst in &dom.instances {
            for key in ["MeshData", "MeshData2"] {
                if let Some(v) = inst.get(key) {
                    if let Some(b) = dom.shared(v) {
                        if !b.is_empty() {
                            return parse_csg(b);
                        }
                    }
                }
            }
        }
        bail!("union asset has no MeshData");
    }
    let mut candidates: Vec<Vec<u8>> = vec![data.to_vec()];
    for start in [0usize, 6, 10] {
        let mut d = data.to_vec();
        for (i, b) in d.iter_mut().enumerate().skip(start) {
            *b ^= CSG_KEY[(i - start) % CSG_KEY.len()];
        }
        candidates.push(d);
        let mut d = data.to_vec();
        for (i, b) in d.iter_mut().enumerate() {
            *b ^= CSG_KEY[i % CSG_KEY.len()];
        }
        if start == 0 {
            candidates.push(d);
        }
    }
    for c in &candidates {
        if let Some(m) = scan_vertex_block(c) {
            return Ok(m);
        }
    }
    bail!("unrecognised CSG data ({} bytes, header {:?})", data.len(), String::from_utf8_lossy(&data[..data.len().min(8)]))
}

fn sane(f: f32, lim: f32) -> bool {
    f.is_finite() && f.abs() < lim
}

fn scan_vertex_block(d: &[u8]) -> Option<MeshData> {
    let rd = |p: usize| -> Option<u32> { d.get(p..p + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())) };
    let rf = |p: usize| -> f32 { f32::from_le_bytes(d[p..p + 4].try_into().unwrap()) };
    for off in 0..d.len().min(256) {
        let Some(nv) = rd(off) else { break };
        let nv = nv as usize;
        if nv < 3 || nv > 2_000_000 {
            continue;
        }
        // either `count, records...` or `count, stride, records...` (CSGMDL v2)
        let explicit = rd(off + 4).map(|s| s as usize).filter(|s| [84usize, 80, 76, 72, 64, 60, 56, 52, 48, 44, 40, 36, 32].contains(s));
        let strides: Vec<usize> = match explicit {
            Some(s) => vec![s],
            None => vec![84, 80, 76, 72, 64, 60, 56, 52, 48, 44, 40, 36, 32],
        };
        for stride in strides {
            let vstart = off + if explicit.is_some() { 8 } else { 4 };
            let vend = vstart + nv * stride;
            if vend + 4 > d.len() {
                continue;
            }
            let Some(ni) = rd(vend) else { continue };
            let ni = ni as usize;
            if ni < 3 || ni % 3 != 0 || vend + 4 + ni * 4 > d.len() {
                continue;
            }
            // positions and normals must look plausible
            let ok = (0..nv.min(64)).all(|i| {
                let p = vstart + i * stride;
                let pos_ok = (0..3).all(|k| sane(rf(p + k * 4), 1.0e5));
                let n = Vec3::new(rf(p + 12), rf(p + 16), rf(p + 20));
                pos_ok && (n.length() - 1.0).abs() < 0.05
            });
            if !ok {
                continue;
            }
            let istart = vend + 4;
            let idx_ok = (0..ni).all(|i| (rd(istart + i * 4).unwrap_or(u32::MAX) as usize) < nv);
            if !idx_ok {
                continue;
            }
            let mut m = MeshData::default();
            for i in 0..nv {
                let p = vstart + i * stride;
                let mut color = [255u8; 4];
                if stride >= 60 {
                    // pos3 nrm3 uv3 color4 (bytes) ...
                    let c = &d[p + 36..p + 40];
                    color = [c[0], c[1], c[2], c[3]];
                }
                m.vertices.push(Vertex {
                    pos: [rf(p), rf(p + 4), rf(p + 8)],
                    normal: [rf(p + 12), rf(p + 16), rf(p + 20)],
                    uv: [rf(p + 24), rf(p + 28)],
                    color,
                });
            }
            for i in 0..ni {
                m.indices.push(rd(istart + i * 4).unwrap());
            }
            return Some(m);
        }
    }
    None
}

/// COREMESH v2: positions, normals and UVs from the Draco-compressed core mesh.
fn decode_draco(data: &[u8], m: &mut MeshData) -> Result<()> {
    use draco_core::{DecoderBuffer, GeometryAttributeType as T, Mesh, MeshDecoder};
    let mut dm = Mesh::new();
    MeshDecoder::new().decode(&mut DecoderBuffer::new(data), &mut dm).map_err(|e| anyhow!("draco: {e:?}"))?;
    let n = dm.num_points();
    let read = |t: T, c: usize| dm.named_attribute(t).map(|a| a.read_f32s(n, c)).unwrap_or_else(|| vec![0.0; n * c]);
    let (pos, nor, uv) = (read(T::Position, 3), read(T::Normal, 3), read(T::TexCoord, 2));
    for i in 0..n {
        m.vertices.push(Vertex { pos: [pos[i * 3], pos[i * 3 + 1], pos[i * 3 + 2]], normal: [nor[i * 3], nor[i * 3 + 1], nor[i * 3 + 2]], uv: [uv[i * 2], uv[i * 2 + 1]], color: [255; 4] });
    }
    for f in dm.faces() {
        m.indices.extend([f[0].0, f[1].0, f[2].0]);
    }
    Ok(())
}

// >> end << //

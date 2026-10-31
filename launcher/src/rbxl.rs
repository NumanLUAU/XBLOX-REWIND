/*
    written by: @corescripts
*/

//! Roblox binary place/model (.rbxl / .rbxm) reader.
//!
//! Decodes the chunked format (META/SSTR/INST/PROP/PRNT/END) with LZ4 or Zstd
//! chunk compression into a flat instance tree with typed property values.

// >> imports
use anyhow::{bail, Context, Result};
use glam::{Mat3, Vec2, Vec3};
use std::collections::HashMap;
use std::io::Read;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CFrame {
    pub pos: Vec3,
    /// Columns are the Right, Up and Back vectors (same as Roblox's XVector/YVector/ZVector).
    pub rot: Mat3,
}

impl Default for CFrame {
    fn default() -> Self {
        Self { pos: Vec3::ZERO, rot: Mat3::IDENTITY }
    }
}

impl CFrame {
    pub fn new(pos: Vec3) -> Self {
        Self { pos, rot: Mat3::IDENTITY }
    }
    pub fn mul(&self, o: &CFrame) -> CFrame {
        CFrame { pos: self.pos + self.rot * o.pos, rot: self.rot * o.rot }
    }
    pub fn inverse(&self) -> CFrame {
        let rt = self.rot.transpose();
        CFrame { pos: -(rt * self.pos), rot: rt }
    }
    pub fn point(&self, p: Vec3) -> Vec3 {
        self.pos + self.rot * p
    }
    pub fn angles(rx: f32, ry: f32, rz: f32) -> CFrame {
        // Roblox CFrame.Angles applies Z, then Y, then X (R = Rx * Ry * Rz)
        CFrame { pos: Vec3::ZERO, rot: Mat3::from_rotation_x(rx) * Mat3::from_rotation_y(ry) * Mat3::from_rotation_z(rz) }
    }
    pub fn look_vector(&self) -> Vec3 {
        -self.rot.z_axis
    }
    pub fn to_quat(&self) -> glam::Quat {
        glam::Quat::from_mat3(&orthonormalize(self.rot))
    }
    pub fn from_quat(pos: Vec3, q: glam::Quat) -> CFrame {
        CFrame { pos, rot: Mat3::from_quat(q) }
    }
    /// Roblox CFrame:lerp — slerp on rotation, linear on position.
    pub fn lerp(&self, o: &CFrame, t: f32) -> CFrame {
        let a = self.to_quat();
        let mut b = o.to_quat();
        if a.dot(b) < 0.0 {
            b = -b;
        }
        CFrame::from_quat(self.pos.lerp(o.pos, t), a.slerp(b, t))
    }
    pub fn to_mat4(&self) -> glam::Mat4 {
        glam::Mat4::from_cols(
            self.rot.x_axis.extend(0.0),
            self.rot.y_axis.extend(0.0),
            self.rot.z_axis.extend(0.0),
            self.pos.extend(1.0),
        )
    }
}

fn orthonormalize(m: Mat3) -> Mat3 {
    let x = m.x_axis.normalize_or_zero();
    let y = (m.y_axis - x * x.dot(m.y_axis)).normalize_or_zero();
    let z = x.cross(y);
    if x == Vec3::ZERO || y == Vec3::ZERO {
        return Mat3::IDENTITY;
    }
    Mat3::from_cols(x, y, z)
}

#[derive(Clone, Debug)]
pub enum Value {
    String(Vec<u8>),
    Bool(bool),
    Int(i32),
    Float(f32),
    Double(f64),
    UDim(f32, i32),
    UDim2(f32, i32, f32, i32),
    Faces(u8),
    BrickColor(u32),
    Color3(Vec3),
    Vector2(Vec2),
    Vector3(Vec3),
    CFrame(CFrame),
    Enum(u32),
    Ref(i32),
    NumberSequence(Vec<(f32, f32, f32)>),
    ColorSequence(Vec<(f32, Vec3)>),
    NumberRange(f32, f32),
    Rect([f32; 4]),
    Int64(i64),
    SharedString(u32),
    Font(String),
    Other,
}

#[derive(Debug, Default)]
pub struct Instance {
    pub class: String,
    pub name: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub props: HashMap<String, Value>,
}

#[derive(Debug, Default)]
pub struct Dom {
    pub instances: Vec<Instance>,
    pub roots: Vec<usize>,
    pub shared_strings: Vec<Vec<u8>>,
}

impl Instance {
    pub fn get(&self, k: &str) -> Option<&Value> {
        self.props.get(k)
    }
    pub fn f32(&self, k: &str) -> Option<f32> {
        match self.props.get(k)? {
            Value::Float(f) => Some(*f),
            Value::Double(d) => Some(*d as f32),
            Value::Int(i) => Some(*i as f32),
            _ => None,
        }
    }
    pub fn bool(&self, k: &str) -> Option<bool> {
        match self.props.get(k)? {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
    pub fn vec3(&self, k: &str) -> Option<Vec3> {
        match self.props.get(k)? {
            Value::Vector3(v) | Value::Color3(v) => Some(*v),
            _ => None,
        }
    }
    pub fn cframe(&self, k: &str) -> Option<CFrame> {
        match self.props.get(k)? {
            Value::CFrame(c) => Some(*c),
            _ => None,
        }
    }
    pub fn enum_(&self, k: &str) -> Option<u32> {
        match self.props.get(k)? {
            Value::Enum(e) => Some(*e),
            Value::Int(i) => Some(*i as u32),
            _ => None,
        }
    }
    pub fn string(&self, k: &str) -> Option<String> {
        match self.props.get(k)? {
            Value::String(s) => Some(String::from_utf8_lossy(s).into_owned()),
            _ => None,
        }
    }
    pub fn bytes(&self, k: &str) -> Option<&[u8]> {
        match self.props.get(k)? {
            Value::String(s) => Some(s),
            _ => None,
        }
    }
}

impl Dom {
    pub fn find_child(&self, parent: usize, name: &str) -> Option<usize> {
        self.instances[parent].children.iter().copied().find(|&c| self.instances[c].name == name)
    }
    pub fn find_root(&self, class_or_name: &str) -> Option<usize> {
        self.roots
            .iter()
            .copied()
            .find(|&r| self.instances[r].class == class_or_name || self.instances[r].name == class_or_name)
    }
    /// Walk a path of child names from `start`.
    pub fn path(&self, start: usize, path: &[&str]) -> Option<usize> {
        let mut cur = start;
        for p in path {
            cur = self.find_child(cur, p)?;
        }
        Some(cur)
    }
    pub fn descendants(&self, root: usize, out: &mut Vec<usize>) {
        for &c in &self.instances[root].children {
            out.push(c);
            self.descendants(c, out);
        }
    }
    pub fn is_ancestor(&self, anc: usize, mut of: usize) -> bool {
        while let Some(p) = self.instances[of].parent {
            if p == anc {
                return true;
            }
            of = p;
        }
        false
    }
    pub fn shared<'s>(&'s self, v: &'s Value) -> Option<&'s [u8]> {
        match v {
            Value::SharedString(i) => self.shared_strings.get(*i as usize).map(|v| v.as_slice()),
            Value::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn full_name(&self, mut i: usize) -> String {
        let mut parts = vec![self.instances[i].name.clone()];
        while let Some(p) = self.instances[i].parent {
            parts.push(self.instances[p].name.clone());
            i = p;
        }
        parts.reverse();
        parts.join(".")
    }
}

struct Cursor<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Cursor<'a> {
    fn new(d: &'a [u8]) -> Self {
        Self { d, p: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.p + n > self.d.len() {
            bail!("unexpected end of chunk ({} + {} > {})", self.p, n, self.d.len());
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
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<Vec<u8>> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    fn remaining(&self) -> usize {
        self.d.len() - self.p
    }
    /// Byte-interleaved big-endian array of `n` values `w` bytes wide.
    fn interleaved(&mut self, n: usize, w: usize) -> Result<Vec<u64>> {
        let raw = self.take(n * w)?;
        let mut out = vec![0u64; n];
        for (i, o) in out.iter_mut().enumerate() {
            let mut v = 0u64;
            for b in 0..w {
                v = (v << 8) | raw[b * n + i] as u64;
            }
            *o = v;
        }
        Ok(out)
    }
    fn i32s(&mut self, n: usize) -> Result<Vec<i32>> {
        Ok(self.interleaved(n, 4)?.into_iter().map(|v| untransform_i32(v as u32)).collect())
    }
    fn f32s(&mut self, n: usize) -> Result<Vec<f32>> {
        Ok(self.interleaved(n, 4)?.into_iter().map(|v| rbx_f32(v as u32)).collect())
    }
    fn u32s(&mut self, n: usize) -> Result<Vec<u32>> {
        Ok(self.interleaved(n, 4)?.into_iter().map(|v| v as u32).collect())
    }
    fn refs(&mut self, n: usize) -> Result<Vec<i32>> {
        let mut v = self.i32s(n)?;
        let mut acc = 0i32;
        for x in v.iter_mut() {
            acc = acc.wrapping_add(*x);
            *x = acc;
        }
        Ok(v)
    }
}

fn untransform_i32(v: u32) -> i32 {
    ((v >> 1) as i32) ^ -((v & 1) as i32)
}

fn rbx_f32(v: u32) -> f32 {
    f32::from_bits((v >> 1) | (v << 31))
}

fn normal_id(i: u8) -> Vec3 {
    match i {
        0 => Vec3::X,
        1 => Vec3::Y,
        2 => Vec3::Z,
        3 => -Vec3::X,
        4 => -Vec3::Y,
        _ => -Vec3::Z,
    }
}

fn basic_rotation(id: u8) -> Mat3 {
    let id = id - 1;
    let r0 = normal_id(id / 6);
    let r1 = normal_id(id % 6);
    let r2 = r0.cross(r1);
    // the two normals are the rotation's X and Y columns (0x03 = rows (1,0,0) (0,0,-1) (0,1,0))
    Mat3::from_cols(r0, r1, r2)
}

fn read_cframes(c: &mut Cursor, n: usize) -> Result<Vec<CFrame>> {
    let mut rots = Vec::with_capacity(n);
    for _ in 0..n {
        let id = c.u8()?;
        if id == 0 {
            let mut m = [0f32; 9];
            for v in m.iter_mut() {
                *v = c.f32()?;
            }
            // row-major R00 R01 R02 R10 ...
            rots.push(Mat3::from_cols_array(&m).transpose());
        } else {
            rots.push(basic_rotation(id));
        }
    }
    let x = c.f32s(n)?;
    let y = c.f32s(n)?;
    let z = c.f32s(n)?;
    Ok((0..n).map(|i| CFrame { pos: Vec3::new(x[i], y[i], z[i]), rot: rots[i] }).collect())
}

fn read_values(c: &mut Cursor, ty: u8, n: usize) -> Result<Option<Vec<Value>>> {
    let v = match ty {
        0x01 | 0x1D => (0..n).map(|_| c.string().map(Value::String)).collect::<Result<_>>()?,
        0x02 => (0..n).map(|_| c.u8().map(|b| Value::Bool(b != 0))).collect::<Result<_>>()?,
        0x03 => c.i32s(n)?.into_iter().map(Value::Int).collect(),
        0x04 => c.f32s(n)?.into_iter().map(Value::Float).collect(),
        0x05 => (0..n)
            .map(|_| c.take(8).map(|b| Value::Double(f64::from_le_bytes(b.try_into().unwrap()))))
            .collect::<Result<_>>()?,
        0x06 => {
            let s = c.f32s(n)?;
            let o = c.i32s(n)?;
            (0..n).map(|i| Value::UDim(s[i], o[i])).collect()
        }
        0x07 => {
            let sx = c.f32s(n)?;
            let sy = c.f32s(n)?;
            let ox = c.i32s(n)?;
            let oy = c.i32s(n)?;
            (0..n).map(|i| Value::UDim2(sx[i], ox[i], sy[i], oy[i])).collect()
        }
        0x08 => {
            c.take(24 * n)?;
            vec![Value::Other; n]
        }
        0x09 | 0x0A => (0..n).map(|_| c.u8().map(Value::Faces)).collect::<Result<_>>()?,
        0x0B => c.u32s(n)?.into_iter().map(Value::BrickColor).collect(),
        0x0C => {
            let r = c.f32s(n)?;
            let g = c.f32s(n)?;
            let b = c.f32s(n)?;
            (0..n).map(|i| Value::Color3(Vec3::new(r[i], g[i], b[i]))).collect()
        }
        0x0D => {
            let x = c.f32s(n)?;
            let y = c.f32s(n)?;
            (0..n).map(|i| Value::Vector2(Vec2::new(x[i], y[i]))).collect()
        }
        0x0E => {
            let x = c.f32s(n)?;
            let y = c.f32s(n)?;
            let z = c.f32s(n)?;
            (0..n).map(|i| Value::Vector3(Vec3::new(x[i], y[i], z[i]))).collect()
        }
        0x10 => read_cframes(c, n)?.into_iter().map(Value::CFrame).collect(),
        0x12 => c.u32s(n)?.into_iter().map(Value::Enum).collect(),
        0x13 => c.refs(n)?.into_iter().map(Value::Ref).collect(),
        0x14 => {
            c.take(6 * n)?;
            vec![Value::Other; n]
        }
        0x15 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let k = c.u32()? as usize;
                let mut seq = Vec::with_capacity(k);
                for _ in 0..k {
                    seq.push((c.f32()?, c.f32()?, c.f32()?));
                }
                out.push(Value::NumberSequence(seq));
            }
            out
        }
        0x16 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let k = c.u32()? as usize;
                let mut seq = Vec::with_capacity(k);
                for _ in 0..k {
                    let t = c.f32()?;
                    let col = Vec3::new(c.f32()?, c.f32()?, c.f32()?);
                    c.f32()?;
                    seq.push((t, col));
                }
                out.push(Value::ColorSequence(seq));
            }
            out
        }
        0x17 => (0..n).map(|_| Ok(Value::NumberRange(c.f32()?, c.f32()?))).collect::<Result<_>>()?,
        0x18 => {
            let a = c.f32s(n)?;
            let b = c.f32s(n)?;
            let cc = c.f32s(n)?;
            let d = c.f32s(n)?;
            (0..n).map(|i| Value::Rect([a[i], b[i], cc[i], d[i]])).collect()
        }
        0x19 => {
            for _ in 0..n {
                let flag = c.u8()?;
                if flag & 1 != 0 {
                    c.take(20)?;
                    if flag & 2 != 0 {
                        c.take(4)?;
                    }
                }
            }
            vec![Value::Other; n]
        }
        0x1A => {
            let r = c.take(n)?.to_vec();
            let g = c.take(n)?.to_vec();
            let b = c.take(n)?.to_vec();
            (0..n)
                .map(|i| Value::Color3(Vec3::new(r[i] as f32, g[i] as f32, b[i] as f32) / 255.0))
                .collect()
        }
        0x1B => c
            .interleaved(n, 8)?
            .into_iter()
            .map(|v| Value::Int64(((v >> 1) as i64) ^ -((v & 1) as i64)))
            .collect(),
        0x1C => c.u32s(n)?.into_iter().map(Value::SharedString).collect(),
        0x1E => {
            let _ty = c.u8()?;
            let cfs = read_cframes(c, n)?;
            let _bty = c.u8()?;
            let has: Vec<u8> = c.take(n)?.to_vec();
            (0..n).map(|i| if has[i] != 0 { Value::CFrame(cfs[i]) } else { Value::Other }).collect()
        }
        0x20 => {
            let mut out = Vec::with_capacity(n);
            for _ in 0..n {
                let fam = c.string()?;
                c.u16()?;
                c.u8()?;
                c.string()?;
                out.push(Value::Font(String::from_utf8_lossy(&fam).into_owned()));
            }
            out
        }
        0x22 => {
            // Content: a source type per value (0 none, 1 uri, 2 object), then the uris, then
            // object and external referents (kept as Other)
            let kinds = c.i32s(n)?;
            let nuri = c.u32()? as usize;
            let mut uris = (0..nuri).map(|_| c.string()).collect::<Result<Vec<_>>>()?.into_iter();
            let nobj = c.u32()? as usize;
            c.refs(nobj)?;
            let next = c.u32()? as usize;
            c.refs(next)?;
            kinds.into_iter().map(|k| if k == 1 { uris.next().map(Value::String).unwrap_or(Value::Other) } else { Value::Other }).collect()
        }
        _ => return Ok(None),
    };
    Ok(Some(v))
}

fn decompress(c: &mut Cursor) -> Result<([u8; 4], Vec<u8>)> {
    let name: [u8; 4] = c.take(4)?.try_into().unwrap();
    let clen = c.u32()? as usize;
    let ulen = c.u32()? as usize;
    c.u32()?;
    if clen == 0 {
        return Ok((name, c.take(ulen)?.to_vec()));
    }
    let comp = c.take(clen)?;
    if comp.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
        let mut dec = ruzstd::StreamingDecoder::new(comp).map_err(|e| anyhow::anyhow!("zstd: {e:?}"))?;
        let mut out = Vec::with_capacity(ulen);
        dec.read_to_end(&mut out)?;
        Ok((name, out))
    } else {
        Ok((name, lz4_flex::block::decompress(comp, ulen).context("lz4")?))
    }
}

pub fn parse(data: &[u8]) -> Result<Dom> {
    if !data.starts_with(b"<roblox!") {
        bail!("not a binary roblox file (XML .rbxlx is not supported)");
    }
    let mut c = Cursor::new(data);
    c.take(14)?;
    c.u16()?;
    let _classes = c.u32()?;
    let ninst = c.u32()? as usize;
    c.take(8)?;

    let mut dom = Dom::default();
    // referent -> index
    let mut by_ref: HashMap<i32, usize> = HashMap::with_capacity(ninst);
    // class id -> (class name, referents)
    let mut classes: HashMap<u32, (String, Vec<i32>)> = HashMap::new();
    let mut parents: Vec<(i32, i32)> = Vec::new();

    while c.remaining() > 0 {
        let (name, chunk) = decompress(&mut c)?;
        let mut k = Cursor::new(&chunk);
        match &name {
            b"INST" => {
                let id = k.u32()?;
                let cname = String::from_utf8_lossy(&k.string()?).into_owned();
                let _fmt = k.u8()?;
                let n = k.u32()? as usize;
                let refs = k.refs(n)?;
                for &r in &refs {
                    by_ref.insert(r, dom.instances.len());
                    dom.instances.push(Instance { class: cname.clone(), ..Default::default() });
                }
                classes.insert(id, (cname, refs));
            }
            b"PROP" => {
                let id = k.u32()?;
                let pname = String::from_utf8_lossy(&k.string()?).into_owned();
                let ty = k.u8()?;
                let Some((_, refs)) = classes.get(&id) else { continue };
                let n = refs.len();
                let vals = match read_values(&mut k, ty, n) {
                    Ok(Some(v)) => v,
                    _ => continue,
                };
                for (r, v) in refs.iter().zip(vals) {
                    let inst = &mut dom.instances[by_ref[r]];
                    if pname == "Name" {
                        if let Value::String(s) = &v {
                            inst.name = String::from_utf8_lossy(s).into_owned();
                        }
                    }
                    inst.props.insert(pname.clone(), v);
                }
            }
            b"PRNT" => {
                k.u8()?;
                let n = k.u32()? as usize;
                let ch = k.refs(n)?;
                let pa = k.refs(n)?;
                parents.extend(ch.into_iter().zip(pa));
            }
            b"SSTR" => {
                k.u32()?;
                let n = k.u32()? as usize;
                for _ in 0..n {
                    k.take(16)?;
                    dom.shared_strings.push(k.string()?);
                }
            }
            b"END\0" => break,
            _ => {}
        }
    }

    for (ch, pa) in parents {
        let Some(&ci) = by_ref.get(&ch) else { continue };
        match by_ref.get(&pa) {
            Some(&pi) => {
                dom.instances[ci].parent = Some(pi);
                dom.instances[pi].children.push(ci);
            }
            None => dom.roots.push(ci),
        }
    }
    Ok(dom)
}

pub fn content_id(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.is_empty() => Some(String::from_utf8_lossy(s).into_owned()),
        _ => None,
    }
}

/// Binary or XML (.rbxmx, older catalog assets) model.
pub fn parse_any(data: &[u8]) -> Result<Dom> {
    let head = &data[..data.len().min(64)];
    if head.windows(7).any(|w| w == b"<roblox") && !data.starts_with(b"<roblox!") {
        parse_xml(&String::from_utf8_lossy(data))
    } else {
        parse(data)
    }
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn xml_attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let i = tag.find(&key)? + key.len();
    let j = tag[i..].find('"')? + i;
    Some(xml_unescape(&tag[i..j]))
}

/// Text of the first `<child>...</child>` inside `body`.
fn xml_child<'a>(body: &'a str, child: &str) -> Option<&'a str> {
    let open = format!("<{child}>");
    let i = body.find(&open)? + open.len();
    let j = body[i..].find(&format!("</{child}>"))? + i;
    Some(&body[i..j])
}

fn xml_value(kind: &str, body: &str) -> Value {
    let f = |c: &str| xml_child(body, c).and_then(|t| t.trim().parse::<f32>().ok()).unwrap_or(0.0);
    match kind {
        "string" | "ProtectedString" => Value::String(xml_unescape(body).into_bytes()),
        "Content" => {
            let url = xml_child(body, "url").map(|u| xml_unescape(u.trim())).unwrap_or_default();
            Value::String(url.into_bytes())
        }
        "bool" => Value::Bool(body.trim() == "true"),
        "int" => Value::Int(body.trim().parse().unwrap_or(0)),
        "int64" => Value::Int64(body.trim().parse().unwrap_or(0)),
        "float" => Value::Float(body.trim().parse().unwrap_or(0.0)),
        "double" => Value::Double(body.trim().parse().unwrap_or(0.0)),
        "token" => Value::Enum(body.trim().parse().unwrap_or(0)),
        "Vector3" => Value::Vector3(Vec3::new(f("X"), f("Y"), f("Z"))),
        "Color3" => Value::Color3(Vec3::new(f("R"), f("G"), f("B"))),
        "Color3uint8" => {
            let v: u32 = body.trim().parse().unwrap_or(0);
            Value::Color3(Vec3::new(((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0))
        }
        "BrickColor" => Value::BrickColor(body.trim().parse().unwrap_or(194)),
        "CoordinateFrame" | "CFrame" => {
            let r = |c: &str| f(c);
            Value::CFrame(CFrame {
                pos: Vec3::new(r("X"), r("Y"), r("Z")),
                rot: Mat3::from_cols(Vec3::new(r("R00"), r("R10"), r("R20")), Vec3::new(r("R01"), r("R11"), r("R21")), Vec3::new(r("R02"), r("R12"), r("R22"))),
            })
        }
        _ => Value::Other,
    }
}

/// Minimal .rbxmx reader: Item/Properties trees with the property types avatar assets use.
pub fn parse_xml(text: &str) -> Result<Dom> {
    let mut dom = Dom::default();
    let mut stack: Vec<usize> = Vec::new();
    let mut i = 0;
    let b = text.as_bytes();
    while let Some(off) = text[i..].find('<') {
        let start = i + off;
        let Some(end_off) = text[start..].find('>') else { break };
        let end = start + end_off;
        let tag = &text[start + 1..end];
        i = end + 1;
        if tag.starts_with('!') || tag.starts_with('?') {
            continue;
        }
        if let Some(rest) = tag.strip_prefix("Item ") {
            let class = xml_attr(rest, "class").unwrap_or_default();
            let idx = dom.instances.len();
            dom.instances.push(Instance { class, parent: stack.last().copied(), ..Default::default() });
            match stack.last() {
                Some(&p) => dom.instances[p].children.push(idx),
                None => dom.roots.push(idx),
            }
            stack.push(idx);
        } else if tag == "/Item" {
            stack.pop();
        } else if tag == "Properties" {
            // read property elements until </Properties>
            let Some(close) = text[i..].find("</Properties>") else { break };
            let props = &text[i..i + close];
            i += close + "</Properties>".len();
            let Some(&cur) = stack.last() else { continue };
            let mut k = 0;
            while let Some(o) = props[k..].find('<') {
                let s = k + o;
                let Some(e) = props[s..].find('>') else { break };
                let ptag = &props[s + 1..s + e];
                k = s + e + 1;
                if ptag.starts_with('/') || ptag.starts_with('!') {
                    continue;
                }
                let self_closing = ptag.ends_with('/');
                let kind = ptag.split_whitespace().next().unwrap_or("").trim_end_matches('/').to_string();
                let name = xml_attr(ptag, "name").unwrap_or_default();
                let body = if self_closing {
                    ""
                } else {
                    let closer = format!("</{kind}>");
                    let Some(c) = props[k..].find(&closer) else { break };
                    let body = &props[k..k + c];
                    k += c + closer.len();
                    body
                };
                let v = xml_value(&kind, body);
                if name == "Name" {
                    if let Value::String(s) = &v {
                        dom.instances[cur].name = String::from_utf8_lossy(s).into_owned();
                    }
                }
                dom.instances[cur].props.insert(name, v);
            }
        }
        let _ = b;
    }
    Ok(dom)
}

// >> end << //
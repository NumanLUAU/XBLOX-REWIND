/*
    written by: @corescripts
*/

//! Smooth Terrain: decodes `Terrain.SmoothGrid` and builds smooth meshes with Surface Nets.
//!
//! SmoothGrid layout: u8 version, u8 log2(chunk size) (5 → 32³ voxels, 4 studs each), then
//! per chunk a 12-byte header holding the chunk coordinate delta as three byte-interleaved
//! big-endian i32s, followed by run-length voxels: byte = material (low 6 bits) | 0x40 if an
//! occupancy byte follows | 0x80 if a run-count byte (count-1) follows.
//! Voxel index within a chunk = x + z*32 + y*1024.

// >> imports
use crate::mesh::{MeshData, Vertex};
use anyhow::{bail, Result};
use glam::Vec3;
use std::collections::HashMap;

pub const VOXEL: f32 = 4.0;
pub const WATER: u8 = 1;

pub struct Grid {
    pub size: i32,
    /// chunk -> (material, occupancy) per voxel
    pub chunks: HashMap<(i32, i32, i32), Vec<(u8, u8)>>,
}

pub fn decode(d: &[u8]) -> Result<Grid> {
    if d.len() < 2 {
        bail!("empty SmoothGrid");
    }
    let size = 1i32 << d[1];
    let n = (size * size * size) as usize;
    let mut p = 2usize;
    let mut c = (0i32, 0i32, 0i32);
    let mut chunks = HashMap::new();
    while p + 12 <= d.len() {
        let raw = &d[p..p + 12];
        p += 12;
        let v = |i: usize| i32::from_be_bytes([raw[i], raw[3 + i], raw[6 + i], raw[9 + i]]);
        c = (c.0 + v(0), c.1 + v(1), c.2 + v(2));
        let mut vox = Vec::with_capacity(n);
        while vox.len() < n {
            let Some(&b) = d.get(p) else { bail!("truncated SmoothGrid") };
            p += 1;
            let mat = b & 63;
            let mut occ = 255u8;
            if b & 64 != 0 {
                occ = d[p];
                p += 1;
            }
            let mut cnt = 1usize;
            if b & 128 != 0 {
                cnt = d[p] as usize + 1;
                p += 1;
            }
            if mat == 0 {
                occ = 0;
            }
            for _ in 0..cnt.min(n - vox.len()) {
                vox.push((mat, occ));
            }
        }
        chunks.insert(c, vox);
    }
    Ok(Grid { size, chunks })
}

impl Grid {
    fn voxel(&self, x: i32, y: i32, z: i32) -> (u8, u8) {
        let s = self.size;
        let key = (x.div_euclid(s), y.div_euclid(s), z.div_euclid(s));
        match self.chunks.get(&key) {
            Some(v) => {
                let (lx, ly, lz) = (x.rem_euclid(s), y.rem_euclid(s), z.rem_euclid(s));
                v[(lx + lz * s + ly * s * s) as usize]
            }
            None => (0, 0),
        }
    }
}

/// Roblox Material enum value for a terrain material id (used by the scene shader).
pub fn shader_material(m: u8) -> u32 {
    match m {
        2 => 1280,  // Grass
        3 => 800,   // Slate
        4 => 816,   // Concrete
        5 => 848,   // Brick
        6 => 1296,  // Sand
        7 => 528,   // WoodPlanks
        8 => 832,   // Rock
        9 | 18 => 1536, // Glacier / Ice
        10 => 1296, // Snow
        11 => 800,  // Sandstone
        12 => 816,  // Mud
        13 => 800,  // Basalt
        14 => 1296, // Ground
        15 => 804,  // CrackedLava (glowing cracks)
        16 => 816,  // Asphalt
        17 => 880,  // Cobblestone
        19 => 1284, // LeafyGrass
        20 => 1296, // Salt
        21 => 784,  // Limestone
        22 => 816,  // Pavement
        _ => 256,
    }
}

/// One mesh per chunk: (chunk key, solid surface, water surface). Positions are in world studs.
pub fn build(grid: &Grid, colors: &[u8]) -> Vec<((i32, i32, i32), MeshData, MeshData)> {
    let color = |m: u8| -> [u8; 4] {
        let i = m as usize * 3;
        if i + 2 < colors.len() && m > 1 {
            [colors[i], colors[i + 1], colors[i + 2], 255]
        } else {
            [128, 128, 128, 255]
        }
    };
    let mut keys: Vec<_> = grid.chunks.keys().copied().collect();
    keys.sort();
    keys.into_iter()
        .map(|k| {
            // any voxel holding a material is inside; occupancy only pushes the surface outwards
            let solid = surface_nets(grid, k, |(m, o)| if m > 1 { 0.5 + o as f32 / 510.0 } else { 0.0 }, &color, false);
            let water = surface_nets(grid, k, |(m, o)| if m == WATER { 0.5 + o as f32 / 510.0 } else { 0.0 }, &color, true);
            (k, solid, water)
        })
        .collect()
}

fn surface_nets(grid: &Grid, chunk: (i32, i32, i32), density: impl Fn((u8, u8)) -> f32, color: &dyn Fn(u8) -> [u8; 4], water: bool) -> MeshData {
    let s = grid.size;
    let base = (chunk.0 * s, chunk.1 * s, chunk.2 * s);
    // samples for voxels base-1 ..= base+s (s+2 per axis)
    let n = (s + 2) as usize;
    let idx = |x: usize, y: usize, z: usize| x + z * n + y * n * n;
    let mut dens = vec![0f32; n * n * n];
    let mut mats = vec![0u8; n * n * n];
    let mut any = false;
    for y in 0..n {
        for z in 0..n {
            for x in 0..n {
                let v = grid.voxel(base.0 + x as i32 - 1, base.1 + y as i32 - 1, base.2 + z as i32 - 1);
                let d = density(v);
                dens[idx(x, y, z)] = d;
                mats[idx(x, y, z)] = v.0;
                any |= d > 0.0;
            }
        }
    }
    let mut m = MeshData::default();
    if !any {
        return m;
    }
    const ISO: f32 = 0.5;
    // vertex per cell; cell (x,y,z) spans samples x..x+1 (n-1 cells per axis)
    let c = n - 1;
    let cidx = |x: usize, y: usize, z: usize| x + z * c + y * c * c;
    let mut cell_vert = vec![u32::MAX; c * c * c];
    let center = chunk_center(grid, chunk);
    let world = |x: f32, y: f32, z: f32| {
        // sample (x,y,z) is the centre of voxel base + (x-1); positions are relative to the chunk centre
        -center + Vec3::new((base.0 as f32 + x - 1.0 + 0.5) * VOXEL, (base.1 as f32 + y - 1.0 + 0.5) * VOXEL, (base.2 as f32 + z - 1.0 + 0.5) * VOXEL)
    };
    for y in 0..c {
        for z in 0..c {
            for x in 0..c {
                let mut corners = [0f32; 8];
                let mut inside = 0;
                for i in 0..8 {
                    let (dx, dy, dz) = (i & 1, (i >> 1) & 1, (i >> 2) & 1);
                    corners[i] = dens[idx(x + dx, y + dy, z + dz)];
                    if corners[i] > ISO {
                        inside += 1;
                    }
                }
                if inside == 0 || inside == 8 {
                    continue;
                }
                // average edge crossings
                let mut acc = Vec3::ZERO;
                let mut cnt = 0.0;
                for &(a, b) in &EDGES {
                    let (da, db) = (corners[a], corners[b]);
                    if (da > ISO) != (db > ISO) {
                        let t = (ISO - da) / (db - da);
                        let pa = corner_pos(a);
                        let pb = corner_pos(b);
                        acc += pa + (pb - pa) * t;
                        cnt += 1.0;
                    }
                }
                let local = acc / cnt;
                let pos = world(x as f32 + local.x, y as f32 + local.y, z as f32 + local.z);
                // gradient for the normal (points out of the solid)
                let g = Vec3::new(
                    (corners[1] + corners[3] + corners[5] + corners[7]) - (corners[0] + corners[2] + corners[4] + corners[6]),
                    (corners[2] + corners[3] + corners[6] + corners[7]) - (corners[0] + corners[1] + corners[4] + corners[5]),
                    (corners[4] + corners[5] + corners[6] + corners[7]) - (corners[0] + corners[1] + corners[2] + corners[3]),
                );
                let normal = (-g).normalize_or_zero();
                // material of the most occupied solid corner
                let mut best = (0u8, -1.0f32);
                for i in 0..8 {
                    let (dx, dy, dz) = (i & 1, (i >> 1) & 1, (i >> 2) & 1);
                    let k = idx(x + dx, y + dy, z + dz);
                    if dens[k] > best.1 && mats[k] > 0 {
                        best = (mats[k], dens[k]);
                    }
                }
                let col = if water { [255; 4] } else { color(best.0) };
                cell_vert[cidx(x, y, z)] = m.vertices.len() as u32;
                m.vertices.push(Vertex {
                    pos: pos.into(),
                    normal: normal.into(),
                    uv: [shader_material(best.0) as f32, 0.0],
                    color: col,
                });
            }
        }
    }
    // quads across sign-changing edges between samples; only emit for edges owned by this chunk
    for y in 1..n - 1 {
        for z in 1..n - 1 {
            for x in 1..n - 1 {
                let d0 = dens[idx(x, y, z)] > ISO;
                for axis in 0..3 {
                    let (nx, ny, nz) = match axis {
                        0 => (x + 1, y, z),
                        1 => (x, y + 1, z),
                        _ => (x, y, z + 1),
                    };
                    if nx >= n || ny >= n || nz >= n {
                        continue;
                    }
                    let d1 = dens[idx(nx, ny, nz)] > ISO;
                    if d0 == d1 {
                        continue;
                    }
                    // the four cells around this edge
                    let cells: [(usize, usize, usize); 4] = match axis {
                        0 => [(x, y - 1, z - 1), (x, y, z - 1), (x, y, z), (x, y - 1, z)],
                        1 => [(x - 1, y, z - 1), (x - 1, y, z), (x, y, z), (x, y, z - 1)],
                        _ => [(x - 1, y - 1, z), (x, y - 1, z), (x, y, z), (x - 1, y, z)],
                    };
                    let v: Vec<u32> = cells.iter().map(|&(a, b, cz)| cell_vert[cidx(a, b, cz)]).collect();
                    if v.iter().any(|&i| i == u32::MAX) {
                        continue;
                    }
                    // winding so the face points the same way as the density gradient normal
                    let pos = |i: u32| Vec3::from(m.vertices[i as usize].pos);
                    let fnrm = (pos(v[1]) - pos(v[0])).cross(pos(v[2]) - pos(v[0]));
                    let vn: Vec3 = v.iter().map(|&i| Vec3::from(m.vertices[i as usize].normal)).sum();
                    if fnrm.dot(vn) >= 0.0 {
                        m.indices.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
                    } else {
                        m.indices.extend_from_slice(&[v[0], v[2], v[1], v[0], v[3], v[2]]);
                    }
                }
            }
        }
    }
    m
}

pub fn chunk_center(grid: &Grid, c: (i32, i32, i32)) -> Vec3 {
    let s = grid.size as f32 * VOXEL;
    Vec3::new((c.0 as f32 + 0.5) * s, (c.1 as f32 + 0.5) * s, (c.2 as f32 + 0.5) * s)
}

const EDGES: [(usize, usize); 12] = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];

fn corner_pos(i: usize) -> Vec3 {
    Vec3::new((i & 1) as f32, ((i >> 1) & 1) as f32, ((i >> 2) & 1) as f32)
}

// >> end << //

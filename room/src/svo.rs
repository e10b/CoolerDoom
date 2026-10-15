//! Sparse voxel octree of the map, in the layout `octree-tracer` traces.
//!
//! Each GPU word keeps a pointer in the top 28 bits and a 4-bit hit counter
//! in the bottom. A leaf's pointer is `VOXEL_OFFSET` plus a packed colour.
//! Rays increment the counter. Counters at 4 or more split a leaf; a counter
//! of 0 folds an internal node back up. The CPU tree supplies the eight child
//! colours. A moving door restamps only that sector and splices those texels
//! into the buckets they touch, so the rest of the map stays put.

use crate::hw::Vertex;
use std::collections::BTreeMap;

const VOXEL_OFFSET: u32 = 134_217_728;
/// One map unit is one texel on Doom's walls and flats. The grid stays a
/// power of two so the tracer's shifts line up. The browser cannot spend
/// the memory of an 8192 cube, so it keeps a coarser cell.
#[cfg(target_arch = "wasm32")]
const MAX_GRID: u32 = 1024;
#[cfg(not(target_arch = "wasm32"))]
const MAX_GRID: u32 = 8192;

pub struct Built {
    /// Solid texels grouped by the top octree digits. A door lives in a
    /// handful of these, so moving it rewrites those groups and not the map.
    buckets: Vec<Bucket>,
    bits: u32,
    /// How many top path digits select a bucket. Zero means one bucket.
    bucket_digits: u32,
    view: View,
    pub origin: [f32; 3],
    /// World length of the cube. Voxel size is `cube / grid`.
    pub cube: f32,
    pub grid: u32,
    changed: bool,
}

/// Texels to delete and texels to insert. Keys are octree path keys.
pub struct Patch {
    pub remove: Vec<u64>,
    pub add: Vec<(u64, u32)>,
    pub eye: [f32; 3],
}

impl Built {
    /// The GPU octree, in the tracer's layout: eight children per node, pointer
    /// in the top 28 bits, hit counter in the low 4.
    pub fn nodes(&self) -> &[u32] {
        &self.view.nodes
    }

    pub fn take_nodes(&mut self) -> Option<Vec<u32>> {
        if !self.changed {
            return None;
        }
        self.changed = false;
        Some(self.view.nodes.clone())
    }

    /// Open the octree around the eye the way `prime_view` does: everything
    /// coarse, and cells that contain the camera down to a single texel.
    pub fn prime(&mut self, eye_world: [f32; 3]) {
        let eye = self.oct(eye_world);
        if self
            .view
            .prime_view(&self.buckets, self.bits, self.bucket_digits, eye)
        {
            self.changed = true;
        }
    }

    /// Split coarse nodes near the eye and fold distant ones back up.
    pub fn refine(&mut self, eye_world: [f32; 3]) {
        let eye = self.oct(eye_world);
        if self.view.refine(
            &self.buckets,
            self.bits,
            self.bucket_digits,
            eye,
            2_500,
            4,
        ) {
            self.changed = true;
        }
    }

    /// Swap texels in the buckets a moving sector touches, then fold those
    /// buckets back to a leaf so the next `prime` opens only what changed.
    pub fn edit(&mut self, remove: &[u64], add: &[(u64, u32)]) {
        if remove.is_empty() && add.is_empty() {
            return;
        }
        let mut groups: BTreeMap<u64, (Vec<u64>, Vec<(u64, u32)>)> = BTreeMap::new();
        for &key in remove {
            groups
                .entry(prefix_of(key, self.bits, self.bucket_digits))
                .or_default()
                .0
                .push(key);
        }
        for &(key, color) in add {
            groups
                .entry(prefix_of(key, self.bits, self.bucket_digits))
                .or_default()
                .1
                .push((key, color));
        }
        let touched: Vec<u64> = groups.keys().copied().collect();
        for (prefix, (mut dead, mut born)) in groups {
            dead.sort_unstable();
            dead.dedup();
            born.sort_unstable_by_key(|s| s.0);
            born.dedup_by_key(|s| s.0);
            let born_keys: Vec<u64> = born.iter().map(|s| s.0).collect();
            match self.buckets.binary_search_by_key(&prefix, |b| b.prefix) {
                Ok(i) => {
                    let solids = &mut self.buckets[i].solids;
                    solids.retain(|s| {
                        dead.binary_search(&s.key).is_err()
                            && born_keys.binary_search(&s.key).is_err()
                    });
                    for (key, color) in born {
                        solids.push(Solid { key, color });
                    }
                    solids.sort_unstable_by_key(|s| s.key);
                    solids.dedup_by_key(|s| s.key);
                }
                Err(i) => {
                    if born.is_empty() {
                        continue;
                    }
                    let solids: Vec<Solid> = born
                        .into_iter()
                        .map(|(key, color)| Solid { key, color })
                        .collect();
                    self.buckets.insert(i, Bucket { prefix, solids });
                }
            }
        }
        self.buckets.retain(|b| !b.solids.is_empty());
        for prefix in touched {
            self.view.resync_bucket(
                &self.buckets,
                self.bits,
                self.bucket_digits,
                prefix,
            );
        }
        self.changed = true;
    }

    /// Apply the compute pass's queues. `subdivide` lists leaves whose hit
    /// counter reached 4. `unsubdivide` lists internal nodes nobody looked at.
    pub fn apply_hits(&mut self, subdivide: &[u32], unsubdivide: &[u32]) -> bool {
        let mut changed = false;
        let mut made = 0usize;
        for &id in subdivide {
            if made >= 1_024_000 || self.view.nodes.len() >= 2_000_000 {
                break;
            }
            if self.view.subdivide(
                &self.buckets,
                self.bits,
                self.bucket_digits,
                id as usize,
            ) {
                made += 1;
                changed = true;
            }
        }
        let mut folded = 0usize;
        for &id in unsubdivide {
            if folded >= 1_024_000 || self.view.nodes.len() >= 2_000_000 {
                break;
            }
            if self.view.unsubdivide(id as usize) {
                folded += 1;
                changed = true;
            }
        }
        if changed {
            self.changed = true;
        }
        changed
    }

    fn oct(&self, world: [f32; 3]) -> [f32; 3] {
        [
            (world[0] - self.origin[0]) / self.cube * 2.0 - 1.0,
            (world[1] - self.origin[1]) / self.cube * 2.0 - 1.0,
            (world[2] - self.origin[2]) / self.cube * 2.0 - 1.0,
        ]
    }
}

/// Voxelise map triangles into an octree. `indices` cover the whole `verts` slice.
pub fn build(verts: &[Vertex], indices: &[u32], pixels: &[u8], atlas: i32) -> Option<Built> {
    if verts.is_empty() || indices.len() < 3 {
        return None;
    }
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in verts {
        for k in 0..3 {
            min[k] = min[k].min(v.pos[k]);
            max[k] = max[k].max(v.pos[k]);
        }
    }
    for k in 0..3 {
        min[k] -= 64.0;
        max[k] += 64.0;
    }
    let span = (0..3).map(|k| max[k] - min[k]).fold(1.0_f32, f32::max);
    // A cell is one map unit, which is one texel, until the level is bigger
    // than the grid. The sorted texel list is the CPU octree: children are
    // spans of that list, created when a ray asks for them.
    let grid = (span.ceil() as u32)
        .next_power_of_two()
        .clamp(2, MAX_GRID);
    let cell = if (span.ceil() as u32) <= MAX_GRID {
        1.0
    } else {
        span / grid as f32
    };
    let cube = grid as f32 * cell;
    let tris: Vec<[usize; 3]> = indices
        .chunks_exact(3)
        .map(|t| [t[0] as usize, t[1] as usize, t[2] as usize])
        .filter(|t| t.iter().all(|i| *i < verts.len()))
        .collect();
    let solids = voxelize(verts, &tris, min, cell, grid, pixels, atlas);
    if solids.is_empty() {
        return None;
    }
    let bits = grid.trailing_zeros();
    let bucket_digits = bucket_digits(bits);
    let buckets = into_buckets(solids, bits, bucket_digits);
    let voxels: usize = buckets.iter().map(|b| b.solids.len()).sum();
    log::info!(
        "octree voxels {voxels} grid {grid} cell {cell} buckets {}",
        buckets.len()
    );
    let mut view = View::from_root(&buckets, bits, bucket_digits);
    let changed = view.refine(&buckets, bits, bucket_digits, [0.0, 0.0, 0.0], 2_500, 4);
    Some(Built {
        buckets,
        bits,
        bucket_digits,
        view,
        origin: min,
        cube,
        grid,
        changed,
    })
}

/// Voxelise a mesh with the same grid the octree was built on.
pub fn stamp_mesh(
    origin: [f32; 3],
    cell: f32,
    grid: u32,
    verts: &[Vertex],
    indices: &[u32],
    pixels: &[u8],
    atlas: i32,
) -> Vec<(u64, u32)> {
    if verts.is_empty() || indices.len() < 3 || grid < 2 || cell <= 0.0 {
        return Vec::new();
    }
    let tris: Vec<[usize; 3]> = indices
        .chunks_exact(3)
        .map(|t| [t[0] as usize, t[1] as usize, t[2] as usize])
        .filter(|t| t.iter().all(|i| *i < verts.len()))
        .collect();
    voxelize(verts, &tris, origin, cell, grid, pixels, atlas)
        .into_iter()
        .map(|s| (s.key, s.color))
        .collect()
}

fn voxelize(
    verts: &[Vertex],
    tris: &[[usize; 3]],
    origin: [f32; 3],
    cell: f32,
    grid: u32,
    pixels: &[u8],
    atlas: i32,
) -> Vec<Solid> {
    let raw = stamp_tris(verts, tris, origin, cell, grid, pixels, atlas);
    if raw.is_empty() {
        return Vec::new();
    }
    let bits = grid.trailing_zeros() as i32;
    let mut solids: Vec<Solid> = raw
        .into_iter()
        .map(|raw| Solid {
            key: path_key(raw.x as i32, raw.y as i32, raw.z as i32, bits),
            color: raw.color,
        })
        .collect();
    solids.sort_unstable_by_key(|s| s.key);
    solids.dedup_by_key(|s| s.key);
    solids
}

struct Raw {
    x: u16,
    y: u16,
    z: u16,
    color: u32,
}

struct Solid {
    key: u64,
    color: u32,
}

struct Bucket {
    prefix: u64,
    solids: Vec<Solid>,
}

fn bucket_digits(bits: u32) -> u32 {
    if bits <= 2 {
        0
    } else {
        (bits - 1).min(6)
    }
}

fn prefix_of(key: u64, bits: u32, digits: u32) -> u64 {
    if digits == 0 {
        0
    } else {
        key >> ((bits - digits) * 3)
    }
}

fn into_buckets(mut solids: Vec<Solid>, bits: u32, digits: u32) -> Vec<Bucket> {
    if solids.is_empty() {
        return Vec::new();
    }
    if digits == 0 {
        return vec![Bucket {
            prefix: 0,
            solids,
        }];
    }
    let shift = (bits - digits) * 3;
    let mut buckets = Vec::new();
    while !solids.is_empty() {
        let prefix = solids.last().map(|s| s.key >> shift).unwrap_or(0);
        let mut cut = solids.len();
        while cut > 0 && (solids[cut - 1].key >> shift) == prefix {
            cut -= 1;
        }
        buckets.push(Bucket {
            prefix,
            solids: solids.split_off(cut),
        });
    }
    buckets.reverse();
    buckets
}

/// `fine`, bucket prefix, solid span, and a representative colour.
fn describe(buckets: &[Bucket], digits: u32, depth: u32, route: u64) -> (u8, u64, usize, usize, u32) {
    if depth >= digits {
        let (start, end) = bucket_bounds(buckets, digits, digits, route);
        if end == start + 1 {
            let bucket = &buckets[start];
            let color = bucket.solids.first().map(|s| s.color).unwrap_or(0);
            return (1, bucket.prefix, 0, bucket.solids.len(), color);
        }
        return (0, 0, 0, 0, 0);
    }
    let (start, end) = bucket_bounds(buckets, digits, depth, route);
    let color = if start < end {
        buckets[start]
            .solids
            .first()
            .map(|solid| solid.color)
            .unwrap_or(0)
    } else {
        0
    };
    (0, 0, 0, 0, color)
}

fn bucket_bounds(buckets: &[Bucket], digits: u32, depth: u32, route: u64) -> (usize, usize) {
    if depth == 0 || depth > digits {
        return (0, 0);
    }
    let shift = (digits - depth) * 3;
    let lo = route << shift;
    let hi = (route + 1) << shift;
    let start = buckets.partition_point(|b| b.prefix < lo);
    let end = buckets.partition_point(|b| b.prefix < hi);
    (start, end)
}

fn stamp_tris(
    verts: &[Vertex],
    tris: &[[usize; 3]],
    origin: [f32; 3],
    cell: f32,
    grid: u32,
    pixels: &[u8],
    atlas: i32,
) -> Vec<Raw> {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, 8);
    let chunk = tris.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let mut joins = Vec::new();
        for part in tris.chunks(chunk) {
            joins.push(scope.spawn(move || {
                let mut out = Vec::new();
                for tri in part {
                    stamp(
                        &verts[tri[0]],
                        &verts[tri[1]],
                        &verts[tri[2]],
                        origin,
                        cell,
                        grid,
                        pixels,
                        atlas,
                        &mut out,
                    );
                }
                out
            }));
        }
        let mut all = Vec::new();
        for join in joins {
            all.extend(join.join().unwrap_or_default());
        }
        all
    })
}

fn stamp(
    a: &Vertex,
    b: &Vertex,
    c: &Vertex,
    origin: [f32; 3],
    cell: f32,
    grid: u32,
    pixels: &[u8],
    atlas: i32,
    voxels: &mut Vec<Raw>,
) {
    let to_grid = |p: [f32; 3]| -> [f32; 3] {
        [
            (p[0] - origin[0]) / cell,
            (p[1] - origin[1]) / cell,
            (p[2] - origin[2]) / cell,
        ]
    };
    // Grid space, so one cell is 1. The triangle is a surface: walk the two
    // axes facing the camera of the plane, and only a couple of cells along
    // the normal. The old loop walked the whole box, which is the map for a
    // big floor and a solid block for a diagonal wall.
    let v = [to_grid(a.pos), to_grid(b.pos), to_grid(c.pos)];
    let e1 = sub(v[1], v[0]);
    let e2 = sub(v[2], v[0]);
    let nrm = cross(e1, e2);
    let len2 = nrm[0] * nrm[0] + nrm[1] * nrm[1] + nrm[2] * nrm[2];
    if len2 < 1e-8 {
        return;
    }
    let len = len2.sqrt();
    let n = [nrm[0] / len, nrm[1] / len, nrm[2] / len];
    let dom = if n[0].abs() >= n[1].abs() && n[0].abs() >= n[2].abs() {
        0
    } else if n[1].abs() >= n[2].abs() {
        1
    } else {
        2
    };
    let (u_axis, w_axis) = [(1, 2), (0, 2), (0, 1)][dom];
    let last = grid as i32 - 1;
    let mut umin = i32::MAX;
    let mut umax = i32::MIN;
    let mut wmin = i32::MAX;
    let mut wmax = i32::MIN;
    for p in v {
        umin = umin.min(p[u_axis].floor() as i32);
        umax = umax.max(p[u_axis].floor() as i32);
        wmin = wmin.min(p[w_axis].floor() as i32);
        wmax = wmax.max(p[w_axis].floor() as i32);
    }
    umin = (umin - 1).clamp(0, last);
    umax = (umax + 1).clamp(0, last);
    wmin = (wmin - 1).clamp(0, last);
    wmax = (wmax + 1).clamp(0, last);
    if umin > umax || wmin > wmax {
        return;
    }
    let half = 0.5 * (n[0].abs() + n[1].abs() + n[2].abs());
    let reach = ((half / n[dom].abs()).ceil() as i32).clamp(1, 3);

    for iu in umin..=umax {
        for iw in wmin..=wmax {
            let cu = iu as f32 + 0.5;
            let cw = iw as f32 + 0.5;
            let plane = n[u_axis] * (cu - v[0][u_axis]) + n[w_axis] * (cw - v[0][w_axis]);
            let cd = v[0][dom] - plane / n[dom];
            let center = cd.floor() as i32;
            for id in (center - reach)..=(center + reach) {
                if id < 0 || id > last {
                    continue;
                }
                let mut g = [0.0; 3];
                g[u_axis] = cu;
                g[w_axis] = cw;
                g[dom] = id as f32 + 0.5;
                let Some(bary) = barycentric(g, v[0], v[1], v[2]) else {
                    continue;
                };
                let uv = [
                    a.uv[0] * bary[0] + b.uv[0] * bary[1] + c.uv[0] * bary[2],
                    a.uv[1] * bary[0] + b.uv[1] * bary[1] + c.uv[1] * bary[2],
                ];
                let rect = [
                    a.atlas[0] * bary[0] + b.atlas[0] * bary[1] + c.atlas[0] * bary[2],
                    a.atlas[1] * bary[0] + b.atlas[1] * bary[1] + c.atlas[1] * bary[2],
                    a.atlas[2] * bary[0] + b.atlas[2] * bary[1] + c.atlas[2] * bary[2],
                    a.atlas[3] * bary[0] + b.atlas[3] * bary[1] + c.atlas[3] * bary[2],
                ];
                let light = a.light * bary[0] + b.light * bary[1] + c.light * bary[2];
                let sample = sample_atlas(pixels, atlas, uv, rect);
                if sample[3] < 128 {
                    continue;
                }
                let ch = |c: u8| {
                    let v = (c as f32 * light).round() as u8;
                    if v == 0 { 1 } else { v }
                };
                let color = ((ch(sample[0]) as u32) << 16)
                    | ((ch(sample[1]) as u32) << 8)
                    | ch(sample[2]) as u32;
                let mut xyz = [0i32; 3];
                xyz[u_axis] = iu;
                xyz[w_axis] = iw;
                xyz[dom] = id;
                voxels.push(Raw {
                    x: xyz[0] as u16,
                    y: xyz[1] as u16,
                    z: xyz[2] as u16,
                    color,
                });
            }
        }
    }
}

fn sample_atlas(pixels: &[u8], size: i32, uv: [f32; 2], rect: [f32; 4]) -> [u8; 4] {
    if size <= 0 || pixels.len() < (size as usize) * (size as usize) * 4 {
        return [180, 180, 180, 255];
    }
    let u = uv[0].rem_euclid(1.0);
    let v = uv[1].rem_euclid(1.0);
    let su = rect[0] + (rect[2] - rect[0]) * u;
    let sv = rect[1] + (rect[3] - rect[1]) * v;
    let x = (su * size as f32) as i32;
    let y = (sv * size as f32) as i32;
    let x = x.clamp(0, size - 1) as usize;
    let y = y.clamp(0, size - 1) as usize;
    let i = (y * size as usize + x) * 4;
    [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
}

/// True when the cell centre lies on the triangle, thickened by half a cell.
fn barycentric(p: [f32; 3], v0: [f32; 3], v1: [f32; 3], v2: [f32; 3]) -> Option<[f32; 3]> {
    let e1 = sub(v1, v0);
    let e2 = sub(v2, v0);
    let n = cross(e1, e2);
    let len2 = n[0] * n[0] + n[1] * n[1] + n[2] * n[2];
    if len2 < 1e-8 {
        return None;
    }
    let len = len2.sqrt();
    let n = [n[0] / len, n[1] / len, n[2] / len];
    let d = sub(p, v0);
    let dist = (n[0] * d[0] + n[1] * d[1] + n[2] * d[2]).abs();
    let max_dist = 0.5 * (n[0].abs() + n[1].abs() + n[2].abs());
    if dist > max_dist {
        return None;
    }
    let dot00 = dot(e2, e2);
    let dot01 = dot(e2, e1);
    let dot02 = dot(e2, d);
    let dot11 = dot(e1, e1);
    let dot12 = dot(e1, d);
    let denom = dot00 * dot11 - dot01 * dot01;
    if denom.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / denom;
    let u = (dot11 * dot02 - dot01 * dot12) * inv;
    let v = (dot00 * dot12 - dot01 * dot02) * inv;
    if u >= -0.02 && v >= -0.02 && u + v <= 1.02 {
        Some([1.0 - u - v, v, u])
    } else {
        None
    }
}

fn path_key(x: i32, y: i32, z: i32, bits: i32) -> u64 {
    let mut key = 0u64;
    for shift in (0..bits).rev() {
        key = (key << 3) | child_index(x, y, z, shift as u32) as u64;
    }
    key
}

fn digit_of(key: u64, shift: u32) -> usize {
    ((key >> (shift * 3)) & 7) as usize
}

/// First index in `begin..end` whose path digit at `shift` is at least `digit`.
fn lower_bound(solids: &[Solid], begin: usize, end: usize, shift: u32, digit: usize) -> usize {
    let mut lo = begin;
    let mut hi = end;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if digit_of(solids[mid].key, shift) < digit {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

fn child_index(x: i32, y: i32, z: i32, shift: u32) -> usize {
    let xi = ((x >> shift) & 1) as usize;
    let yi = ((y >> shift) & 1) as usize;
    let zi = ((z >> shift) & 1) as usize;
    xi * 4 + yi * 2 + zi
}

fn gpu_leaf(color: u32) -> u32 {
    (VOXEL_OFFSET + color) << 4
}

fn pos_offset(child: usize, depth: u32) -> [f32; 3] {
    let x = ((child >> 2) & 1) as f32;
    let y = ((child >> 1) & 1) as f32;
    let z = (child & 1) as f32;
    let s = 1.0 / (1u32 << depth) as f32;
    [(x * 2.0 - 1.0) * s, (y * 2.0 - 1.0) * s, (z * 2.0 - 1.0) * s]
}

struct Slot {
    word: u32,
    pos: [f32; 3],
    depth: u32,
    route: u64,
    fine: u8,
    bucket_prefix: u64,
    start: usize,
    end: usize,
    color: u32,
}

/// The octree the GPU is tracing. It starts as eight coarse voxels and splits
/// where the camera is, which is the tracer's dynamic octree.
struct View {
    nodes: Vec<u32>,
    positions: Vec<[f32; 3]>,
    depths: Vec<u32>,
    /// Top octree digits that lead to this node, packed from the root.
    route: Vec<u64>,
    /// One when `span` indexes a single bucket's texels.
    fine: Vec<u8>,
    bucket_prefix: Vec<u64>,
    /// Start and end of this node's texels inside its bucket.
    span: Vec<(u32, u32)>,
    colors: Vec<u32>,
    holes: Vec<usize>,
}

impl View {
    fn from_root(buckets: &[Bucket], bits: u32, bucket_digits: u32) -> Self {
        let mut view = Self {
            nodes: Vec::new(),
            positions: Vec::new(),
            depths: Vec::new(),
            route: Vec::new(),
            fine: Vec::new(),
            bucket_prefix: Vec::new(),
            span: Vec::new(),
            colors: Vec::new(),
            holes: Vec::new(),
        };
        if bucket_digits == 0 {
            let solids = buckets.first().map(|b| b.solids.as_slice()).unwrap_or(&[]);
            let shift = bits.saturating_sub(1);
            let mut edge = [0usize; 9];
            edge[0] = 0;
            edge[8] = solids.len();
            for digit in 1..8 {
                edge[digit] = lower_bound(solids, 0, solids.len(), shift, digit);
            }
            for i in 0..8 {
                let start = edge[i];
                let end = edge[i + 1];
                let color = if start < end { solids[start].color } else { 0 };
                view.push(Slot {
                    word: gpu_leaf(color),
                    pos: pos_offset(i, 1),
                    depth: 1,
                    route: i as u64,
                    fine: 1,
                    bucket_prefix: 0,
                    start,
                    end,
                    color,
                });
            }
            return view;
        }
        for i in 0..8 {
            let (fine, prefix, start, end, color) =
                describe(buckets, bucket_digits, 1, i as u64);
            view.push(Slot {
                word: gpu_leaf(color),
                pos: pos_offset(i, 1),
                depth: 1,
                route: i as u64,
                fine,
                bucket_prefix: prefix,
                start,
                end,
                color,
            });
        }
        view
    }

    fn push(&mut self, slot: Slot) {
        self.nodes.push(slot.word);
        self.positions.push(slot.pos);
        self.depths.push(slot.depth);
        self.route.push(slot.route);
        self.fine.push(slot.fine);
        self.bucket_prefix.push(slot.bucket_prefix);
        self.span.push((slot.start as u32, slot.end as u32));
        self.colors.push(slot.color);
    }

    fn write_slot(&mut self, at: usize, slot: Slot) {
        self.nodes[at] = slot.word;
        self.positions[at] = slot.pos;
        self.depths[at] = slot.depth;
        self.route[at] = slot.route;
        self.fine[at] = slot.fine;
        self.bucket_prefix[at] = slot.bucket_prefix;
        self.span[at] = (slot.start as u32, slot.end as u32);
        self.colors[at] = slot.color;
    }

    /// Fold one bucket's leaves back together after its texels change.
    /// A new bucket is grafted onto the coarse tree. The camera pass then
    /// opens just that leaf, and the rest of the octree stays as it was.
    fn resync_bucket(
        &mut self,
        buckets: &[Bucket],
        bits: u32,
        bucket_digits: u32,
        prefix: u64,
    ) {
        if bucket_digits == 0 {
            *self = Self::from_root(buckets, bits, 0);
            return;
        }
        let (color, len) = buckets
            .binary_search_by_key(&prefix, |b| b.prefix)
            .ok()
            .and_then(|i| buckets[i].solids.first().map(|s| (s.color, buckets[i].solids.len())))
            .unwrap_or((0, 0));
        let found = self.shallowest_fine(prefix);
        if let Some(idx) = found {
            self.install_fine(idx, prefix, color, len as u32);
        } else if len > 0 {
            if let Some(idx) = self.walk_bucket(buckets, bits, bucket_digits, prefix) {
                self.install_fine(idx, prefix, color, len as u32);
            }
        }
    }

    fn shallowest_fine(&self, prefix: u64) -> Option<usize> {
        let mut best: Option<(usize, u32)> = None;
        for i in 0..self.nodes.len() {
            if self.depths[i] == 0 || self.fine[i] == 0 || self.bucket_prefix[i] != prefix {
                continue;
            }
            let depth = self.depths[i];
            if best.map(|(_, d)| depth < d).unwrap_or(true) {
                best = Some((i, depth));
            }
        }
        best.map(|(i, _)| i)
    }

    fn install_fine(&mut self, idx: usize, prefix: u64, color: u32, len: u32) {
        if idx >= self.nodes.len() || self.depths[idx] == 0 {
            return;
        }
        if (self.nodes[idx] >> 4) < VOXEL_OFFSET {
            self.unsubdivide(idx);
        }
        self.nodes[idx] = gpu_leaf(color);
        self.colors[idx] = color;
        self.span[idx] = (0, len);
        self.fine[idx] = 1;
        self.bucket_prefix[idx] = prefix;
    }

    fn walk_bucket(
        &mut self,
        buckets: &[Bucket],
        bits: u32,
        bucket_digits: u32,
        prefix: u64,
    ) -> Option<usize> {
        if bucket_digits == 0 || self.nodes.len() < 8 {
            return None;
        }
        let top = ((prefix >> ((bucket_digits - 1) * 3)) & 7) as usize;
        let mut idx = top;
        let mut depth = 1u32;
        loop {
            if idx >= self.nodes.len() || self.depths[idx] == 0 {
                return None;
            }
            if depth >= bucket_digits {
                return Some(idx);
            }
            let ptr = self.nodes[idx] >> 4;
            if ptr >= VOXEL_OFFSET && !self.subdivide(buckets, bits, bucket_digits, idx) {
                return Some(idx);
            }
            let ptr = self.nodes[idx] >> 4;
            if ptr >= VOXEL_OFFSET {
                return Some(idx);
            }
            let digit = ((prefix >> ((bucket_digits - depth - 1) * 3)) & 7) as usize;
            idx = ptr as usize + digit;
            depth += 1;
        }
    }

    /// `prime_view` from octree-tracer. Split the whole cube a little, and
    /// keep splitting any leaf whose centre is within one and a half cells
    /// of the eye, down to a texel.
    fn prime_view(
        &mut self,
        buckets: &[Bucket],
        bits: u32,
        bucket_digits: u32,
        eye: [f32; 3],
    ) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i < self.nodes.len() {
            if self.nodes.len() > 2_000_000 {
                break;
            }
            if self.depths[i] != 0 && (self.nodes[i] >> 4) >= VOXEL_OFFSET {
                let depth = self.depths[i];
                let pos = self.positions[i];
                let cell = 2.0 / (1u32 << depth) as f32;
                let dx = pos[0] - eye[0];
                let dy = pos[1] - eye[1];
                let dz = pos[2] - eye[2];
                let near = (dx * dx + dy * dy + dz * dz).sqrt() <= cell * 1.5;
                if depth < bits
                    && (near || depth <= 2)
                    && self.subdivide(buckets, bits, bucket_digits, i)
                {
                    changed = true;
                }
            }
            i += 1;
        }
        changed
    }

    fn refine(
        &mut self,
        buckets: &[Bucket],
        bits: u32,
        bucket_digits: u32,
        eye: [f32; 3],
        budget: usize,
        open_depth: u32,
    ) -> bool {
        let mut changed = false;
        let mut made = 0usize;
        let mut i = 0;
        while i < self.nodes.len() && made < budget && self.nodes.len() < 1_500_000 {
            if self.depths[i] == 0 {
                i += 1;
                continue;
            }
            let ptr = self.nodes[i] >> 4;
            if ptr >= VOXEL_OFFSET {
                let depth = self.depths[i];
                let pos = self.positions[i];
                let cell = 2.0 / (1u32 << depth) as f32;
                let dx = pos[0] - eye[0];
                let dy = pos[1] - eye[1];
                let dz = pos[2] - eye[2];
                let near = dx * dx + dy * dy + dz * dz <= (cell * 8.0) * (cell * 8.0);
                if depth < bits
                    && (near || depth < open_depth)
                    && self.subdivide(buckets, bits, bucket_digits, i)
                {
                    made += 1;
                    changed = true;
                }
            }
            i += 1;
        }
        if self.collapse_far(eye) {
            changed = true;
        }
        changed
    }

    fn subdivide(
        &mut self,
        buckets: &[Bucket],
        bits: u32,
        bucket_digits: u32,
        node: usize,
    ) -> bool {
        if node >= self.nodes.len() || self.depths[node] == 0 {
            return false;
        }
        // Already an internal node. The tracer calls this a double-up.
        if (self.nodes[node] >> 4) < VOXEL_OFFSET {
            return false;
        }
        let depth = self.depths[node];
        if depth >= bits {
            return false;
        }
        if self.fine[node] == 0 && bucket_digits > 0 && depth >= bucket_digits {
            let (start, end) = bucket_bounds(buckets, bucket_digits, depth, self.route[node]);
            if end != start + 1 {
                return false;
            }
            let bucket = &buckets[start];
            self.fine[node] = 1;
            self.bucket_prefix[node] = bucket.prefix;
            self.span[node] = (0, bucket.solids.len() as u32);
            self.colors[node] = bucket.solids.first().map(|s| s.color).unwrap_or(0);
        }
        if self.fine[node] == 1 || bucket_digits == 0 {
            return self.subdivide_fine(buckets, bits, bucket_digits, node);
        }
        let (start, end) = bucket_bounds(buckets, bucket_digits, depth, self.route[node]);
        if start >= end {
            return false;
        }
        let child_depth = depth + 1;
        let pos = self.positions[node];
        let route = self.route[node];
        let at = self.alloc_children();
        self.nodes[node] = (at as u32) << 4;
        for digit in 0..8 {
            let child_route = (route << 3) | digit as u64;
            let (fine, prefix, child_start, child_end, color) =
                describe(buckets, bucket_digits, child_depth, child_route);
            self.write_slot(
                at + digit,
                Slot {
                    word: gpu_leaf(color),
                    pos: add3(pos, pos_offset(digit, child_depth)),
                    depth: child_depth,
                    route: child_route,
                    fine,
                    bucket_prefix: prefix,
                    start: child_start,
                    end: child_end,
                    color,
                },
            );
        }
        true
    }

    fn subdivide_fine(
        &mut self,
        buckets: &[Bucket],
        bits: u32,
        bucket_digits: u32,
        node: usize,
    ) -> bool {
        let prefix = if bucket_digits == 0 {
            0
        } else {
            self.bucket_prefix[node]
        };
        let Ok(bucket_i) = buckets.binary_search_by_key(&prefix, |b| b.prefix) else {
            return false;
        };
        let solids = &buckets[bucket_i].solids;
        let (start, end) = self.span[node];
        let start = start as usize;
        let end = end as usize;
        if start >= end || start >= solids.len() {
            return false;
        }
        let end = end.min(solids.len());
        let depth = self.depths[node];
        let shift = bits - depth - 1;
        let mut edge = [start; 9];
        edge[8] = end;
        for digit in 1..8 {
            edge[digit] = lower_bound(solids, start, end, shift, digit);
        }
        let child_depth = depth + 1;
        let pos = self.positions[node];
        let route = self.route[node];
        let at = self.alloc_children();
        self.nodes[node] = (at as u32) << 4;
        for digit in 0..8 {
            let child_start = edge[digit];
            let child_end = edge[digit + 1];
            let color = if child_start < child_end {
                solids[child_start].color
            } else {
                0
            };
            self.write_slot(
                at + digit,
                Slot {
                    word: gpu_leaf(color),
                    pos: add3(pos, pos_offset(digit, child_depth)),
                    depth: child_depth,
                    route: (route << 3) | digit as u64,
                    fine: 1,
                    bucket_prefix: prefix,
                    start: child_start,
                    end: child_end,
                    color,
                },
            );
        }
        true
    }

    fn alloc_children(&mut self) -> usize {
        if let Some(hole) = self.holes.pop() {
            return hole;
        }
        let at = self.nodes.len();
        for _ in 0..8 {
            self.push(Slot {
                word: 0,
                pos: [0.0; 3],
                depth: 0,
                route: 0,
                fine: 0,
                bucket_prefix: 0,
                start: 0,
                end: 0,
                color: 0,
            });
        }
        at
    }

    /// Fold an internal node back into the colour it was split from.
    /// Child blocks go on the hole stack so a later split can reuse them.
    fn unsubdivide(&mut self, node: usize) -> bool {
        if node >= self.nodes.len() || self.depths[node] == 0 {
            return false;
        }
        let ptr = self.nodes[node] >> 4;
        if ptr >= VOXEL_OFFSET {
            return false;
        }
        let at = ptr as usize;
        if at + 8 > self.nodes.len() {
            return false;
        }
        for k in 0..8 {
            let child = at + k;
            if (self.nodes[child] >> 4) < VOXEL_OFFSET {
                self.unsubdivide(child);
            }
        }
        self.nodes[node] = gpu_leaf(self.colors[node]);
        for k in 0..8 {
            self.depths[at + k] = 0;
        }
        self.holes.push(at);
        true
    }

    fn collapse_far(&mut self, eye: [f32; 3]) -> bool {
        let mut changed = false;
        let mut i = 0;
        while i < self.nodes.len() {
            if self.depths[i] == 0 {
                i += 1;
                continue;
            }
            let ptr = self.nodes[i] >> 4;
            if ptr < VOXEL_OFFSET {
                let at = ptr as usize;
                let depth = self.depths[i];
                let kids_are_leaves = (0..8).all(|k| {
                    at + k < self.nodes.len() && (self.nodes[at + k] >> 4) >= VOXEL_OFFSET
                });
                let pos = self.positions[i];
                let cell = 2.0 / (1u32 << depth) as f32;
                let dx = pos[0] - eye[0];
                let dy = pos[1] - eye[1];
                let dz = pos[2] - eye[2];
                let far = dx * dx + dy * dy + dz * dz > (cell * 14.0) * (cell * 14.0);
                if kids_are_leaves && depth > 3 && far {
                    self.nodes[i] = gpu_leaf(self.colors[i]);
                    for k in 0..8 {
                        self.depths[at + k] = 0;
                    }
                    self.holes.push(at);
                    changed = true;
                }
            }
            i += 1;
        }
        changed
    }
}

fn add3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Column-major 4×4 inverse. Returns the input when the matrix is singular.
pub fn invert(m: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut a = [[0.0; 8]; 4];
    for c in 0..4 {
        for r in 0..4 {
            a[r][c] = m[c][r];
            a[r][c + 4] = if r == c { 1.0 } else { 0.0 };
        }
    }
    for col in 0..4 {
        let mut pivot = col;
        for r in col..4 {
            if a[r][col].abs() > a[pivot][col].abs() {
                pivot = r;
            }
        }
        a.swap(col, pivot);
        let div = a[col][col];
        if div.abs() < 1e-12 {
            return m;
        }
        for c in 0..8 {
            a[col][c] /= div;
        }
        for r in 0..4 {
            if r == col {
                continue;
            }
            let f = a[r][col];
            for c in 0..8 {
                a[r][c] -= f * a[col][c];
            }
        }
    }
    let mut o = [[0.0; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            o[c][r] = a[r][c + 4];
        }
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vert(pos: [f32; 3]) -> Vertex {
        Vertex {
            pos,
            uv: [0.0, 0.0],
            atlas: [0.0, 0.0, 1.0, 1.0],
            light: 1.0,
            tint: [1.0; 3],
            _pad: 0.0,
        }
    }

    #[test]
    fn a_floor_quad_becomes_solid_leaves() {
        let verts = [
            vert([0.0, 0.0, 0.0]),
            vert([3.0, 0.0, 0.0]),
            vert([3.0, 0.0, 3.0]),
            vert([0.0, 0.0, 3.0]),
        ];
        let indices = [0, 1, 2, 0, 2, 3];
        let pixels = vec![200u8, 40, 40, 255];
        let built = build(&verts, &indices, &pixels, 1).expect("octree");
        assert!(built.nodes().len() >= 8);
        assert!(built.grid >= 2);
        let solid = built
            .nodes()
            .iter()
            .filter(|n| (*n >> 4) > VOXEL_OFFSET)
            .count();
        assert!(solid >= 1, "solid leaves {solid}");
    }

    #[test]
    fn a_diagonal_wall_stays_a_surface() {
        let verts = [
            vert([0.0, 0.0, 0.0]),
            vert([80.0, 0.0, 80.0]),
            vert([80.0, 40.0, 80.0]),
            vert([0.0, 40.0, 0.0]),
        ];
        let indices = [0, 1, 2, 0, 2, 3];
        let started = std::time::Instant::now();
        let built = build(&verts, &indices, &[180, 180, 180, 255], 1).expect("octree");
        let ms = started.elapsed().as_millis();
        let solid = built
            .nodes()
            .iter()
            .filter(|n| (*n >> 4) > VOXEL_OFFSET)
            .count();
        assert!(ms < 500, "diagonal wall took {ms} ms");
        assert!(solid >= 1, "solid leaves {solid}");
        assert!(built.nodes().len() < 20_000, "nodes {}", built.nodes().len());
    }

    #[test]
    fn a_broad_floor_does_not_explode_the_tree() {
        let verts = [
            vert([0.0, 0.0, 0.0]),
            vert([32.0, 0.0, 0.0]),
            vert([32.0, 0.0, 32.0]),
            vert([0.0, 0.0, 32.0]),
        ];
        let built = build(&verts, &[0, 1, 2, 0, 2, 3], &[180, 180, 180, 255], 1).expect("octree");
        assert!(
            built.nodes().len() < 20_000,
            "nodes {} grid {}",
            built.nodes().len(),
            built.grid
        );
    }

    #[test]
    fn hit_counters_split_a_leaf_and_fold_it_back() {
        let verts = [
            vert([0.0, 0.0, 0.0]),
            vert([32.0, 0.0, 0.0]),
            vert([32.0, 0.0, 32.0]),
            vert([0.0, 0.0, 32.0]),
        ];
        let mut built = build(&verts, &[0, 1, 2, 0, 2, 3], &[180, 180, 180, 255], 1).expect("octree");
        let before = built.nodes().len();
        let ids: Vec<u32> = (0..before as u32).collect();
        assert!(built.apply_hits(&ids, &[]), "a looked-at leaf should split");
        assert!(built.nodes().len() > before);
        let internal: Vec<u32> = built
            .nodes()
            .iter()
            .enumerate()
            .filter(|(_, word)| **word != 0 && (**word >> 4) < VOXEL_OFFSET)
            .map(|(i, _)| i as u32)
            .collect();
        assert!(!internal.is_empty());
        assert!(built.apply_hits(&[], &internal), "an unseen node should fold up");
    }

    #[test]
    fn moving_one_floor_leaves_the_other_in_place() {
        let verts = [
            vert([0.0, 0.0, 0.0]),
            vert([8.0, 0.0, 0.0]),
            vert([8.0, 0.0, 8.0]),
            vert([0.0, 0.0, 8.0]),
            vert([40.0, 0.0, 0.0]),
            vert([48.0, 0.0, 0.0]),
            vert([48.0, 0.0, 8.0]),
            vert([40.0, 0.0, 8.0]),
        ];
        let indices = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
        let pixels = [180u8, 180, 180, 255];
        let mut built = build(&verts, &indices, &pixels, 1).expect("octree");
        let cell = built.cube / built.grid as f32;
        let quad = [0u32, 1, 2, 0, 2, 3];
        let stay = stamp_mesh(
            built.origin,
            cell,
            built.grid,
            &verts[..4],
            &quad,
            &pixels,
            1,
        );
        let old = stamp_mesh(
            built.origin,
            cell,
            built.grid,
            &verts[4..],
            &quad,
            &pixels,
            1,
        );
        let raised = [
            vert([40.0, 16.0, 0.0]),
            vert([48.0, 16.0, 0.0]),
            vert([48.0, 16.0, 8.0]),
            vert([40.0, 16.0, 8.0]),
        ];
        let new = stamp_mesh(built.origin, cell, built.grid, &raised, &quad, &pixels, 1);
        assert!(!old.is_empty() && !new.is_empty() && !stay.is_empty());
        let remove: Vec<u64> = old.iter().map(|s| s.0).collect();
        let started = std::time::Instant::now();
        built.edit(&remove, &new);
        let ms = started.elapsed().as_millis();
        assert!(ms < 50, "splicing a floor took {ms} ms");
        let has = |key: u64| {
            built
                .buckets
                .iter()
                .any(|bucket| bucket.solids.iter().any(|solid| solid.key == key))
        };
        for (key, _) in &stay {
            assert!(has(*key), "untouched floor texel disappeared");
        }
        for key in &remove {
            assert!(!has(*key), "old floor texel was left behind");
        }
        for (key, _) in &new {
            assert!(has(*key), "raised floor texel was not inserted");
        }
    }

    #[test]
    fn inverse_roundtrips_translation() {
        let m = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [4.0, -2.0, 7.0, 1.0],
        ];
        let inv = invert(m);
        let p = [3.0, 1.0, -5.0, 1.0];
        let mut world = [0.0; 4];
        for r in 0..4 {
            world[r] = m[0][r] * p[0] + m[1][r] * p[1] + m[2][r] * p[2] + m[3][r] * p[3];
        }
        let mut back = [0.0; 4];
        for r in 0..4 {
            back[r] = inv[0][r] * world[0]
                + inv[1][r] * world[1]
                + inv[2][r] * world[2]
                + inv[3][r] * world[3];
        }
        for k in 0..3 {
            assert!((back[k] - p[k]).abs() < 1e-4, "{k} {back:?}");
        }
    }
}

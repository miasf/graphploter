//! 共有する辺を同時に分割し、像の曲がりに合わせて平面の入力メッシュを細かくする。

use crate::adaptive::HashMap;
use crate::eval::ProgramSet;
use crate::mesh::Mesh;

/// 平面領域の切り出しで複製された頂点をまとめる。像の法線はこの頂点を共有して計算する。
pub(super) fn weld(mesh: &mut Mesh) {
    let mut by_position = HashMap::default();
    let mut positions = Vec::new();
    let remap: Vec<u32> = mesh
        .positions
        .chunks_exact(3)
        .map(|p| {
            let key = [0, 1, 2].map(|i| if p[i] == 0.0 { 0 } else { p[i].to_bits() });
            *by_position.entry(key).or_insert_with(|| {
                let i = positions.len() as u32 / 3;
                positions.extend_from_slice(p);
                i
            })
        })
        .collect();
    for index in &mut mesh.indices {
        *index = remap[*index as usize];
    }
    mesh.positions = positions;
}

fn edge(a: u32, b: u32) -> (u32, u32) {
    (a.min(b), a.max(b))
}

fn point(values: &[f32], i: u32) -> [f32; 3] {
    let i = i as usize * 3;
    [values[i], values[i + 1], values[i + 2]]
}

fn midpoint(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| ((a[i] as f64 + b[i] as f64) * 0.5) as f32)
}

fn deviates(values: &[f64], a: usize, b: usize, mapped: &[f64], tol_sq: f64) -> bool {
    let n = mapped.len();
    let error = (0..n)
        .map(|j| (mapped[j] - 0.5 * (values[a * n + j] + values[b * n + j])).powi(2))
        .sum::<f64>();
    error.is_finite() && error > tol_sq
}

pub(super) fn curve(positions: &mut Vec<f32>, values: &mut Vec<f64>, set: &ProgramSet, tol: f64) {
    let n = set.len();
    for _ in 0..8 {
        let count = positions.len() / 3;
        if !(2..32_768).contains(&count) {
            break;
        }
        let mids: Vec<f32> = (0..count - 1)
            .flat_map(|i| midpoint(point(positions, i as u32), point(positions, i as u32 + 1)))
            .collect();
        let mapped = super::evaluate(set, &mids);
        let mut new_positions = Vec::new();
        let mut new_values = Vec::new();
        for i in 0..count {
            new_positions.extend_from_slice(&positions[3 * i..3 * (i + 1)]);
            new_values.extend_from_slice(&values[n * i..n * (i + 1)]);
            if i + 1 < count {
                let mid = &mapped[i * n..(i + 1) * n];
                if deviates(values, i, i + 1, mid, tol * tol) {
                    new_positions.extend_from_slice(&mids[3 * i..3 * (i + 1)]);
                    new_values.extend_from_slice(mid);
                }
            }
        }
        if new_positions.len() == positions.len() {
            break;
        }
        *positions = new_positions;
        *values = new_values;
    }
}

pub(super) fn surface(mesh: &mut Mesh, values: &mut Vec<f64>, set: &ProgramSet, tol: f64) {
    const MAX_TRIANGLES: usize = 100_000;
    const PASSES: usize = 4;
    let n = set.len();
    let tol_sq = tol * tol;
    for _ in 0..PASSES {
        let room = MAX_TRIANGLES.saturating_sub(mesh.indices.len() / 3) / 2;
        if mesh.indices.is_empty() || room == 0 {
            break;
        }
        let mut mids = Vec::new();
        let mut edges = Vec::new();
        let mut slots = HashMap::default();
        for t in mesh.indices.chunks_exact(3) {
            for (a, b) in [edge(t[0], t[1]), edge(t[1], t[2]), edge(t[2], t[0])] {
                slots.entry((a, b)).or_insert_with(|| {
                    let slot = edges.len();
                    edges.push((a, b));
                    mids.extend(midpoint(point(&mesh.positions, a), point(&mesh.positions, b)));
                    slot
                });
            }
        }
        let mapped = super::evaluate(set, &mids);
        let mut split = vec![None; edges.len()];
        let mut count = 0;
        for (i, (a, b)) in edges.into_iter().enumerate() {
            let m = &mapped[i * n..(i + 1) * n];
            if deviates(values, a as usize, b as usize, m, tol_sq) {
                split[i] = Some((mesh.positions.len() / 3) as u32);
                mesh.positions.extend_from_slice(&mids[i * 3..(i + 1) * 3]);
                values.extend_from_slice(m);
                count += 1;
                if count >= room {
                    break;
                }
            }
        }
        if count == 0 {
            break;
        }
        let mut tris = Vec::with_capacity(mesh.indices.len() + count * 6);
        for t in mesh.indices.chunks_exact(3) {
            let a = [t[0], t[1], t[2]];
            let m = [0, 1, 2].map(|i| split[slots[&edge(a[i], a[(i + 1) % 3])]]);
            split_triangle(a, m, &mut tris);
        }
        mesh.indices = tris;
    }
}

fn split_triangle(t: [u32; 3], m: [Option<u32>; 3], out: &mut Vec<u32>) {
    if let [Some(a), Some(b), Some(c)] = m {
        out.extend([t[0], a, c, a, t[1], b, c, b, t[2], a, b, c]);
        return;
    }
    for i in 0..3 {
        let j = (i + 1) % 3;
        let k = (i + 2) % 3;
        if let (Some(a), None) = (m[i], m[k]) {
            out.extend([t[i], a, t[k]]);
            if let Some(b) = m[j] {
                out.extend([a, t[j], b, a, b, t[k]]);
            } else {
                out.extend([a, t[j], t[k]]);
            }
            return;
        }
    }
    out.extend(t);
}

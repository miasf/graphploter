//! 座標系（計画 9 章の S36）。
//!
//! 座標系は、座標の値の組 q = (q₁, q₂, q₃)（表示中の 3 つの座標）を空間の点へ移す写像 T とする。
//! 図形は今までどおり座標の値の空間で作り、最後に頂点ごとに T(q) を計算して、位置とは別の配列（[`Mesh::target`]）で返す。
//! 描画側は移行のスライダー s で (1 − s) q + s T(q) と混ぜる（s を動かしても計算し直さない）。
//!
//! 座標系の空間では「作る範囲」と「画面の箱」が別になるので、作る範囲からはみ出した所（陰関数は範囲の 1 マス外まで作る、など）は
//! 画面の箱の平面では切り落とせない。[`trim_to_domain`] で、メッシュの側を作る範囲の箱で切る。

use crate::eval::{Program, ProgramSet};
use crate::mesh::{Domain, Mesh};

/// 座標系の写像 T（3 つの成分。変数は表示中の 3 つの座標）
#[derive(Debug, Clone)]
pub struct CoordMap {
    progs: [Program; 3],
}

impl CoordMap {
    pub fn new(progs: [Program; 3]) -> CoordMap {
        CoordMap { progs }
    }

    /// 式が使っているパラメータの名前（値が変わったら、座標系を使う式を作り直すため）
    pub fn used_params(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for p in &self.progs {
            for n in p.used_params() {
                if !out.contains(n) {
                    out.push(n.clone());
                }
            }
        }
        out
    }

    /// 点の列（3 つずつ）を T で移す。有限でない点・移した先が有限でない（実数でない）点は、3 つとも NaN
    pub fn map_points(&self, pts: &[f32]) -> Vec<f32> {
        as_display_points(evaluate_map(&ProgramSet::new(&self.progs), pts))
    }

    /// メッシュの全部の頂点（陰関数の粗い版 `lod_mesh` も）で T(q) を計算して [`Mesh::target`] に入れる。
    /// 移した先が有限でない頂点は、面ならその頂点を使う三角形（粗い版の三角形も）を捨て、線ならその点を NaN にして線を切る。
    /// 2D 表示（`flat`）では、移した先の縦の座標を 0 にする
    pub fn apply(&self, m: &mut Mesh, curve: bool, flat: bool) {
        if let Some(l) = m.lod_mesh.as_deref_mut() {
            self.apply(l, curve, flat);
        }
        let set = ProgramSet::new(&self.progs);
        let mut mapped = evaluate_map(&set, &m.positions);
        if refine_mapped(m, &mapped, 3, curve, if flat { 2 } else { 3 }) {
            mapped = evaluate_map(&set, &m.positions);
        }
        let mut t = as_display_points(mapped);
        if flat {
            for v in t.iter_mut().skip(2).step_by(3) {
                if v.is_finite() {
                    *v = 0.0;
                }
            }
        }
        let n = t.len() / 3;
        let bad: Vec<bool> = (0..n).map(|i| !t[3 * i].is_finite()).collect();
        if curve || m.indices.is_empty() {
            // 線・点: 位置も NaN にして、混ぜた後も同じ所で切れるようにする
            for (i, &b) in bad.iter().enumerate() {
                if b {
                    m.positions[3 * i..3 * i + 3].fill(f32::NAN);
                }
            }
        } else if bad.iter().any(|&b| b) {
            let keep = |tri: &[u32]| tri.iter().all(|&v| !bad[v as usize]);
            m.indices = m.indices.chunks_exact(3).filter(|t| keep(t)).flatten().copied().collect();
            m.lod = m.lod.chunks_exact(3).filter(|t| keep(t)).flatten().copied().collect();
            // 使わなくなった頂点は、元の位置をそのまま入れておく（描画側で NaN を混ぜないため）
            for (i, &b) in bad.iter().enumerate() {
                if b {
                    t[3 * i..3 * i + 3].copy_from_slice(&m.positions[3 * i..3 * i + 3]);
                }
            }
        }
        m.target = t;
    }

    /// 点の列（線。NaN で切れ目）を移した先。移せない点は、位置も NaN にする（線を切る）
    pub fn apply_line(&self, positions: &mut [f32], flat: bool) -> Vec<f32> {
        let mut t = self.map_points(positions);
        for (i, q) in t.chunks_exact_mut(3).enumerate() {
            if !q[0].is_finite() {
                positions[3 * i..3 * i + 3].fill(f32::NAN);
            } else if flat {
                q[2] = 0.0;
            }
        }
        t
    }
}

/// 3 座標の入力点に写像を適用する。線の切れ目は定数写像でも切れ目のままにする。
pub(crate) fn evaluate_map(set: &ProgramSet, positions: &[f32]) -> Vec<f64> {
    let cols: [Vec<f64>; 3] = [0, 1, 2].map(|c| positions.chunks_exact(3).map(|p| p[c] as f64).collect());
    let mut values = Vec::new();
    set.eval_points(&cols.each_ref().map(Vec::as_slice), positions.len() / 3, &mut values);
    for (p, mapped) in positions.chunks_exact(3).zip(values.chunks_exact_mut(set.len())) {
        if p.iter().any(|v| !v.is_finite()) {
            mapped.fill(f64::NAN);
        }
    }
    values
}

fn as_display_points(mapped: Vec<f64>) -> Vec<f32> {
    mapped
        .chunks_exact(3)
        .flat_map(|p| {
            let q = [p[0] as f32, p[1] as f32, p[2] as f32];
            if q.iter().all(|v| v.is_finite()) {
                q
            } else {
                [f32::NAN; 3]
            }
        })
        .collect()
}

/// 写像の先で辺が長くなるメッシュを、元の座標で細分化する。呼び出し側は追加した頂点にも写像を適用する。
/// 線は線分ごとに、面は共有する辺が同じ分割になるよう一律に分割する。
pub(crate) fn refine_mapped(m: &mut Mesh, t: &[f64], arity: usize, curve: bool, length_axes: usize) -> bool {
    let n = t.len() / arity;
    let len = |a: usize, b: usize| -> f64 {
        let d2: f64 = (0..length_axes).map(|c| (t[arity * a + c] - t[arity * b + c]).powi(2)).sum();
        d2.sqrt()
    };
    // 移した先の大きさ（有限な点を囲む箱の対角線）
    let (mut lo, mut hi) = (vec![f64::INFINITY; arity], vec![f64::NEG_INFINITY; arity]);
    let ok = |i: usize| {
        t[arity * i..arity * (i + 1)]
            .iter()
            .all(|v| v.is_finite() && v.abs() <= f32::MAX as f64)
    };
    for (i, p) in t.chunks_exact(arity).enumerate() {
        if !ok(i) {
            continue;
        }
        for c in 0..arity {
            lo[c] = lo[c].min(p[c]);
            hi[c] = hi[c].max(p[c]);
        }
    }
    let diag = (0..arity).map(|c| (hi[c] - lo[c]).max(0.0).powi(2)).sum::<f64>().sqrt();
    if !(diag > 0.0 && diag.is_finite()) {
        return false;
    }
    // 頂点ごとの値（法線・格子線の座標・複素数の色）も、位置と同じく 1 次式で足す
    let np = m.positions.len();
    let attr_on = [m.normals.len() == np, m.grid_coords.len() == np, m.complex.len() == np];
    if curve || m.indices.is_empty() {
        const MAX_POINTS: usize = 120_000;
        let limit = diag / 800.0;
        let splits: Vec<usize> = (1..n)
            .map(|i| {
                if ok(i - 1) && ok(i) {
                    ((len(i - 1, i) / limit).ceil() as usize).clamp(1, 64)
                } else {
                    1
                }
            })
            .collect();
        let total: usize = n + splits.iter().map(|s| s - 1).sum::<usize>();
        if total == n || total > MAX_POINTS {
            return false;
        }
        let lerp_all = |v: &[f32]| -> Vec<f32> {
            let mut out = Vec::with_capacity(3 * total);
            out.extend_from_slice(&v[0..3]);
            for i in 1..n {
                let s = splits[i - 1];
                for j in 1..s {
                    let f = j as f32 / s as f32;
                    for c in 0..3 {
                        out.push(v[3 * (i - 1) + c] + f * (v[3 * i + c] - v[3 * (i - 1) + c]));
                    }
                }
                out.extend_from_slice(&v[3 * i..3 * i + 3]);
            }
            out
        };
        m.positions = lerp_all(&m.positions);
        if attr_on[0] {
            m.normals = lerp_all(&m.normals);
        }
        if attr_on[1] {
            m.grid_coords = lerp_all(&m.grid_coords);
        }
        if attr_on[2] {
            m.complex = lerp_all(&m.complex);
        }
        return true;
    }
    const MAX_TRIANGLES: usize = 160_000;
    let tris = m.indices.len() / 3;
    let limit = diag / 150.0;
    let mut longest: f64 = 0.0;
    for tri in m.indices.chunks_exact(3) {
        let [a, b, c] = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
        if ok(a) && ok(b) && ok(c) {
            longest = longest.max(len(a, b)).max(len(b, c)).max(len(c, a));
        }
    }
    let budget = ((MAX_TRIANGLES as f64 / tris.max(1) as f64).sqrt().floor() as usize).max(1);
    let k = ((longest / limit).ceil() as usize).clamp(1, 8).min(budget);
    if k <= 1 {
        return false;
    }
    subdivide(m, k, attr_on);
    true
}

/// メッシュを、作る範囲の箱 `dom` の中だけにする（座標系を使うとき。範囲からはみ出して作った所を落とす）。
/// 面は [`crate::system::trim_mesh`]、線は [`crate::system::clip_segments`] で、箱の面との境目を辺の上で求めて切る。
/// 2D 表示（`flat`）では、縦の座標では切らない（図形は縦の座標 0 の平面にある）。
/// 頂点ごとの値（色分けなど）を持つ線は、境目を求めずに、箱の外の点を NaN にして切る（値の並びを崩さないため）
pub fn trim_to_domain(m: &mut Mesh, dom: Domain, curve: bool, flat: bool) {
    let axes = if flat { 2 } else { 3 };
    // 範囲ちょうどの所にある頂点（範囲の端の格子点）を外と見ないように、ほんの少し広げる
    let pad: [f64; 3] = [0, 1, 2].map(|a| 1e-9 * (dom.hi[a] - dom.lo[a]).abs().max(1.0));
    let outside = move |p: &[f64; 3]| -> f64 {
        (0..axes)
            .map(|a| (dom.lo[a] - pad[a] - p[a]).max(p[a] - dom.hi[a] - pad[a]))
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let mut value = |pts: &[[f64; 3]]| -> Vec<f64> { pts.iter().map(outside).collect() };
    if !curve && !m.indices.is_empty() {
        crate::system::trim_mesh(m, &mut value, 40);
        return;
    }
    if !curve {
        return;
    }
    let attrs = m.complex.len() == m.positions.len() || m.grid_coords.len() == m.positions.len() || m.normals.len() == m.positions.len();
    if !attrs {
        crate::system::clip_segments(&mut m.positions, &mut value, 40);
        return;
    }
    for q in m.positions.chunks_exact_mut(3) {
        if q.iter().all(|v| v.is_finite()) && outside(&[q[0] as f64, q[1] as f64, q[2] as f64]) > 0.0 {
            q.fill(f32::NAN);
        }
    }
}

/// 座標系の格子（座標曲線。計画 9 章の S39）の、元の位置（座標の値）。作る範囲の箱 `dom` の面のうち、`faces` のビット a が立っている
/// 「座標 a が範囲の最小」の面ごとに、残り 2 つの座標のどちらか 1 つを動かし、もう 1 つを範囲を `lines` 等分した値に固定した線を引く
/// （1 本 64 点。線の間は NaN）。2D 表示（`flat`）では、縦の座標の面（ビット 2）だけを、縦の座標 0 の平面に引く
pub fn grid_lines(dom: Domain, lines: usize, faces: u32, flat: bool) -> Vec<f32> {
    const POINTS: usize = 64;
    let lines = lines.clamp(1, 64);
    let mut out = Vec::new();
    for a in 0..3 {
        if flat && a != 2 {
            continue;
        }
        if !flat && faces & (1 << a) == 0 {
            continue;
        }
        let (b, c) = ((a + 1) % 3, (a + 2) % 3);
        let fixed = if flat { 0.0 } else { dom.lo[a] };
        for (vary, hold) in [(b, c), (c, b)] {
            for k in 0..=lines {
                let hv = dom.lo[hold] + (dom.hi[hold] - dom.lo[hold]) * k as f64 / lines as f64;
                for j in 0..POINTS {
                    let mut p = [0.0; 3];
                    p[a] = fixed;
                    p[hold] = hv;
                    p[vary] = dom.lo[vary] + (dom.hi[vary] - dom.lo[vary]) * j as f64 / (POINTS - 1) as f64;
                    out.extend(p.map(|v| v as f32));
                }
                out.extend_from_slice(&[f32::NAN; 3]);
            }
        }
    }
    out
}

/// 全部の三角形を、辺を k 等分して k² 個の三角形に分ける（座標の値の空間で 1 次式。辺の上の点は隣の三角形と共有する）。
/// `attr` は [法線, 格子線の座標, 複素数の色] を一緒に分けるか
fn subdivide(m: &mut Mesh, k: usize, attr: [bool; 3]) {
    use std::collections::HashMap;
    // 新しい点を足す（a と b の間の f の所、または三角形の重心座標）
    fn push(m: &mut Mesh, attr: [bool; 3], w: &[(u32, f32)]) -> u32 {
        let id = (m.positions.len() / 3) as u32;
        let mix = |v: &Vec<f32>| -> [f32; 3] {
            let mut o = [0.0f32; 3];
            for &(i, f) in w {
                for c in 0..3 {
                    o[c] += f * v[3 * i as usize + c];
                }
            }
            o
        };
        let p = mix(&m.positions);
        m.positions.extend(p);
        if attr[0] {
            let mut nv = mix(&m.normals);
            let l = (nv[0] * nv[0] + nv[1] * nv[1] + nv[2] * nv[2]).sqrt();
            if l > 0.0 {
                nv.iter_mut().for_each(|v| *v /= l);
            }
            m.normals.extend(nv);
        }
        if attr[1] {
            let g = mix(&m.grid_coords);
            m.grid_coords.extend(g);
        }
        if attr[2] {
            let c = mix(&m.complex);
            m.complex.extend(c);
        }
        id
    }
    let old = std::mem::take(&mut m.indices);
    let mut edges: HashMap<(u32, u32), Vec<u32>> = HashMap::new();
    let mut out = Vec::with_capacity(old.len() * k * k);
    let kf = k as f32;
    for tri in old.chunks_exact(3) {
        let (a, b, c) = (tri[0], tri[1], tri[2]);
        // 辺 u → v の上の j 番目（0 < j < k）の点
        let mut edge = |m: &mut Mesh, u: u32, v: u32| -> Vec<u32> {
            let key = (u.min(v), u.max(v));
            let pts = edges
                .entry(key)
                .or_insert_with(|| {
                    (1..k)
                        .map(|j| {
                            let f = j as f32 / kf;
                            push(m, attr, &[(key.0, 1.0 - f), (key.1, f)])
                        })
                        .collect()
                })
                .clone();
            let mut full = Vec::with_capacity(k + 1);
            full.push(u);
            if u < v {
                full.extend(pts);
            } else {
                full.extend(pts.into_iter().rev());
            }
            full.push(v);
            full
        };
        let ab = edge(m, a, b);
        let ac = edge(m, a, c);
        let bc = edge(m, b, c);
        // 重心座標の格子 (i, j)（i が b へ、j が c へ）の点の番号
        let mut grid = vec![0u32; (k + 1) * (k + 1)];
        for j in 0..=k {
            for i in 0..=(k - j) {
                grid[j * (k + 1) + i] = if j == 0 {
                    ab[i]
                } else if i == 0 {
                    ac[j]
                } else if i + j == k {
                    bc[j]
                } else {
                    let (fi, fj) = (i as f32 / kf, j as f32 / kf);
                    push(m, attr, &[(a, 1.0 - fi - fj), (b, fi), (c, fj)])
                };
            }
        }
        let g = |i: usize, j: usize| grid[j * (k + 1) + i];
        for j in 0..k {
            for i in 0..(k - j) {
                out.extend([g(i, j), g(i + 1, j), g(i, j + 1)]);
                if i + j + 1 < k {
                    out.extend([g(i + 1, j), g(i + 1, j + 1), g(i, j + 1)]);
                }
            }
        }
    }
    m.indices = out;
}

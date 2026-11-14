//! 座標系（計画 9 章の S36）。
//!
//! 座標系は、座標の値の組 q = (q₁, q₂, q₃)（表示中の 3 つの座標）を空間の点へ移す写像 T とする。
//! 図形は今までどおり座標の値の空間で作り、最後に頂点ごとに T(q) を計算して、位置とは別の配列（[`Mesh::target`]）で返す。
//! 描画側は移行のスライダー s で (1 − s) q + s T(q) と混ぜる（s を動かしても計算し直さない）。
//!
//! 座標系の空間では「作る範囲」と「画面の箱」が別になるので、作る範囲からはみ出した所（陰関数は範囲の 1 マス外まで作る、など）は
//! 画面の箱の平面では切り落とせない。[`trim_to_domain`] で、メッシュの側を作る範囲の箱で切る。

use crate::eval::{as_real, Program};
use crate::mesh::{Domain, Mesh};
use num_complex::Complex64 as C;

/// 座標系の写像 T（3 つの成分。変数は表示中の 3 つの座標）
#[derive(Debug, Clone)]
pub struct CoordMap {
    progs: [Program; 3],
}

impl CoordMap {
    pub fn new(progs: [Program; 3]) -> CoordMap {
        CoordMap { progs }
    }

    /// 点の列（3 つずつ）を T で移す。有限でない点・移した先が有限でない（実数でない）点は、3 つとも NaN
    pub fn map_points(&self, pts: &[f32]) -> Vec<f32> {
        let n = pts.len() / 3;
        let mut out = vec![f32::NAN; 3 * n];
        if n == 0 {
            return out;
        }
        let cols: [Vec<f64>; 3] = [0, 1, 2].map(|c| (0..n).map(|i| pts[3 * i + c] as f64).collect());
        let inputs: Vec<&[f64]> = cols.iter().map(Vec::as_slice).collect();
        let mut buf = vec![C::new(0.0, 0.0); n];
        for (c, p) in self.progs.iter().enumerate() {
            p.eval_batch(&inputs, &mut buf);
            for (i, v) in buf.iter().enumerate() {
                out[3 * i + c] = as_real(*v) as f32;
            }
        }
        for i in 0..n {
            let ok = (0..3).all(|c| pts[3 * i + c].is_finite() && out[3 * i + c].is_finite());
            if !ok {
                out[3 * i..3 * i + 3].fill(f32::NAN);
            }
        }
        out
    }

    /// メッシュの全部の頂点（陰関数の粗い版 `lod_mesh` も）で T(q) を計算して [`Mesh::target`] に入れる。
    /// 移した先が有限でない頂点は、面ならその頂点を使う三角形（粗い版の三角形も）を捨て、線ならその点を NaN にして線を切る。
    /// 2D 表示（`flat`）では、移した先の縦の座標を 0 にする
    pub fn apply(&self, m: &mut Mesh, curve: bool, flat: bool) {
        if let Some(l) = m.lod_mesh.as_deref_mut() {
            self.apply(l, curve, flat);
        }
        let mut t = self.map_points(&m.positions);
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

/// メッシュを、作る範囲の箱 `dom` の中だけにする（座標系を使うとき。範囲からはみ出して作った所を落とす）。
/// 面は [`crate::system::trim_mesh`]、線は [`crate::system::clip_segments`] で、箱の面との境目を辺の上で求めて切る。
/// 2D 表示（`flat`）では、縦の座標では切らない（図形は縦の座標 0 の平面にある）。
/// 頂点ごとの値（色分けなど）を持つ線は、境目を求めずに、箱の外の点を NaN にして切る（値の並びを崩さないため）
pub fn trim_to_domain(m: &mut Mesh, dom: Domain, curve: bool, flat: bool) {
    let axes = if flat { 2 } else { 3 };
    // 範囲ちょうどの所にある頂点（範囲の端の格子点）を外と見ないように、ほんの少し広げる
    let pad: [f64; 3] = [0, 1, 2].map(|a| 1e-9 * (dom.hi[a] - dom.lo[a]).abs().max(1.0));
    let outside = move |p: &[f64; 3]| -> f64 {
        (0..axes).map(|a| (dom.lo[a] - pad[a] - p[a]).max(p[a] - dom.hi[a] - pad[a])).fold(f64::NEG_INFINITY, f64::max)
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

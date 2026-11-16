//! ベクトル場の矢印（計画 `docs/plan-math-notation.md` の M8）。
//!
//! 表示範囲を各軸 n 個に分けた格子の中心で場の値を求め、矢印を置く。矢印 1 本は、軸の線分（線として返す）と、
//! 矢じりの四角錐（メッシュ。2D 表示では平らな三角形）。色は大きさ ‖F‖ を頂点ごとの値に入れて、色の対応表で塗る

use crate::eval::{as_real, Program};
use crate::mesh::{Domain, Mesh};
use num_complex::Complex64 as C;

/// 矢印の作り方
#[derive(Debug, Clone, Copy)]
pub struct Style {
    /// 各軸の矢印の数（4〜24）
    pub count: usize,
    /// 長さをそろえる（向きだけを見る）
    pub unit: bool,
    /// 2D 表示（縦の成分を 0 にして、横・奥の平面に描く）
    pub flat: bool,
}

/// 作った矢印
pub struct Arrows {
    /// 矢じり（頂点ごとの値の 1 つ目が大きさ）
    pub mesh: Mesh,
    /// 軸の線分（1 本ごとに、始点・矢じりの底・NaN の 3 点）
    pub shafts: Vec<f32>,
    /// 大きさの範囲（色の対応表の両端）。矢印が無ければ None
    pub range: Option<(f64, f64)>,
    /// 矢印の本数
    pub count: usize,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn len(a: [f64; 3]) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// 矢印を足していく入れ物
struct Builder {
    mesh: Mesh,
    shafts: Vec<f32>,
    flat: bool,
}

impl Builder {
    fn vertex(&mut self, p: [f64; 3], n: [f64; 3], value: f64) -> u32 {
        let k = (self.mesh.positions.len() / 3) as u32;
        self.mesh.positions.extend(p.map(|c| c as f32));
        self.mesh.normals.extend(n.map(|c| c as f32));
        self.mesh.complex.extend([value as f32, 0.0, 0.0]);
        k
    }

    /// 尾 `tail` から先 `tip` への矢印。`value` は色にする値
    fn arrow(&mut self, tail: [f64; 3], tip: [f64; 3], value: f64) {
        let v = sub(tip, tail);
        let l = len(v);
        if !(l > 0.0) || !l.is_finite() {
            return;
        }
        let d = scale(v, 1.0 / l);
        // 矢じり: 長さは矢印の 0.35 倍、底の幅の半分はその 0.4 倍
        let h = 0.35 * l;
        let w = 0.4 * h;
        let base = sub(tip, scale(d, h));
        self.shafts.extend(tail.map(|c| c as f32));
        self.shafts.extend(base.map(|c| c as f32));
        self.shafts.extend([f32::NAN; 3]);
        if self.flat {
            // 平面の中の三角形（法線は縦）
            let side = [-d[1], d[0], 0.0];
            let up = [0.0, 0.0, 1.0];
            let a = self.vertex(tip, up, value);
            let b = self.vertex(add(base, scale(side, w)), up, value);
            let c = self.vertex(sub(base, scale(side, w)), up, value);
            self.mesh.indices.extend([a, b, c]);
            return;
        }
        // 向きに垂直な 2 つの方向
        let helper = if d[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let e1 = {
            let c = cross(d, helper);
            scale(c, 1.0 / len(c))
        };
        let e2 = cross(d, e1);
        let t = self.vertex(tip, d, value);
        let corners = [add(base, scale(e1, w)), add(base, scale(e2, w)), sub(base, scale(e1, w)), sub(base, scale(e2, w))];
        let normals = [e1, e2, scale(e1, -1.0), scale(e2, -1.0)];
        let ids: Vec<u32> = corners.iter().zip(normals).map(|(p, n)| self.vertex(*p, n, value)).collect();
        for k in 0..4 {
            self.mesh.indices.extend([t, ids[k], ids[(k + 1) % 4]]);
        }
        // 底
        self.mesh.indices.extend([ids[0], ids[2], ids[1], ids[0], ids[3], ids[2]]);
    }

    fn finish(mut self, range: Option<(f64, f64)>, count: usize) -> Arrows {
        // 小さな矢じりの頂点をまとめる粗い版は作らない
        self.mesh.keep_full = true;
        Arrows { mesh: self.mesh, shafts: self.shafts, range, count }
    }
}

/// ベクトル場の矢印。`progs` は 3 つの成分（変数は表示中の 3 つの座標）。`rot` は、場の向きに掛ける行列
/// （表示中の座標どうしの回転。式の座標は、もう回した後の座標の式になっている）
pub fn field(progs: &[Program], rot: Option<[[f64; 3]; 3]>, dom: Domain, style: Style) -> Arrows {
    let n = style.count.clamp(2, 64);
    let nz = if style.flat { 1 } else { n };
    let step: [f64; 3] = [0, 1, 2].map(|a| (dom.hi[a] - dom.lo[a]) / if a == 2 { nz as f64 } else { n as f64 });
    let total = n * n * nz;
    let mut pts: [Vec<f64>; 3] = [Vec::with_capacity(total), Vec::with_capacity(total), Vec::with_capacity(total)];
    for k in 0..nz {
        for j in 0..n {
            for i in 0..n {
                pts[0].push(dom.lo[0] + (i as f64 + 0.5) * step[0]);
                pts[1].push(dom.lo[1] + (j as f64 + 0.5) * step[1]);
                // 2D 表示は、縦の座標 0 の平面
                pts[2].push(if style.flat { 0.0 } else { dom.lo[2] + (k as f64 + 0.5) * step[2] });
            }
        }
    }
    let inputs: [&[f64]; 3] = [&pts[0], &pts[1], &pts[2]];
    let mut comps: Vec<Vec<f64>> = Vec::with_capacity(3);
    let mut buf = vec![C::new(0.0, 0.0); total];
    for p in progs.iter().take(3) {
        p.eval_batch(&inputs, &mut buf);
        comps.push(buf.iter().map(|z| as_real(*z)).collect());
    }
    while comps.len() < 3 {
        comps.push(vec![0.0; total]);
    }
    // 向き（回転と、2D 表示の縦の成分）と大きさ
    let mut vecs: Vec<[f64; 3]> = Vec::with_capacity(total);
    let mut mags: Vec<f64> = Vec::with_capacity(total);
    for k in 0..total {
        let f = [comps[0][k], comps[1][k], comps[2][k]];
        let mut v = match rot {
            Some(m) => [0, 1, 2].map(|i| m[i][0] * f[0] + m[i][1] * f[1] + m[i][2] * f[2]),
            None => f,
        };
        if style.flat {
            v[2] = 0.0;
        }
        let m = len(v);
        mags.push(if m.is_finite() { m } else { f64::NAN });
        vecs.push(v);
    }
    let max = mags.iter().copied().filter(|m| m.is_finite()).fold(0.0, f64::max);
    let mut b = Builder { mesh: Mesh::default(), shafts: Vec::new(), flat: style.flat };
    if !(max > 0.0) {
        return b.finish(None, 0);
    }
    let cell = if style.flat { step[0].min(step[1]) } else { step[0].min(step[1]).min(step[2]) };
    let mut count = 0;
    let mut min = f64::INFINITY;
    for k in 0..total {
        let m = mags[k];
        // 大きさが 0（最大の 10⁻⁹ 倍以下）・有限でない点は描かない
        if !m.is_finite() || m <= 1e-9 * max {
            continue;
        }
        // 最大の大きさの矢印が、格子の間隔の 0.9 倍。長さをそろえるなら、全部 0.7 倍
        let l = if style.unit { 0.7 * cell } else { 0.9 * cell * m / max };
        let half = scale(vecs[k], 0.5 * l / m);
        let c = [pts[0][k], pts[1][k], pts[2][k]];
        b.arrow(sub(c, half), add(c, half), m);
        min = min.min(m);
        count += 1;
    }
    b.finish(Some((if min < max { min } else { 0.0 }, max)), count)
}

/// 原点からのベクトル（行列の列ベクトル・ベクトルの定義）の矢印。色にする値は、ベクトルの番号
pub fn vectors(vs: &[[f64; 3]], flat: bool) -> Arrows {
    let mut b = Builder { mesh: Mesh::default(), shafts: Vec::new(), flat };
    let mut count = 0;
    for (k, v) in vs.iter().enumerate() {
        let tip = if flat { [v[0], v[1], 0.0] } else { *v };
        if len(tip) > 0.0 {
            b.arrow([0.0; 3], tip, k as f64);
            count += 1;
        }
    }
    b.finish(None, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rotation_field() -> Vec<Program> {
        ["-y", "x", "0 x"].iter().map(|s| Program::compile(&crate::parser::parse(s).unwrap(), &["x", "y", "z"]).unwrap()).collect()
    }

    #[test]
    fn arrows_follow_the_field() {
        let dom = Domain { lo: [-3.0; 3], hi: [3.0; 3] };
        let a = field(&rotation_field(), None, dom, Style { count: 9, unit: false, flat: false });
        // 9³ の格子のうち、z 軸の上（大きさ 0）の 9 点には矢印が無い
        assert_eq!(a.count, 9 * 9 * 9 - 9);
        assert_eq!(a.shafts.len(), a.count * 9);
        for s in a.shafts.chunks_exact(9) {
            let (p0, p1) = ([s[0] as f64, s[1] as f64, s[2] as f64], [s[3] as f64, s[4] as f64, s[5] as f64]);
            // 矢印の中心は、尾から全長の半分（軸は全長の 0.65 倍）
            let dir = sub(p1, p0);
            let c = add(p0, scale(dir, 0.5 / 0.65));
            let want = [-c[1], c[0], 0.0];
            assert!(len(cross(dir, want)) < 1e-6, "{c:?} {dir:?}");
            assert!(dir[0] * want[0] + dir[1] * want[1] > 0.0);
            // 原点に矢印は無い
            assert!(len([c[0], c[1], 0.0]) > 0.1);
        }
        // 色にする値は大きさ
        let (lo, hi) = a.range.unwrap();
        assert!(lo > 0.0 && (hi - (2.0 * (8.0f64 / 3.0).powi(2)).sqrt()).abs() < 1e-9);
        // 三角形は矢印 1 本に 6 枚（24³ 本でも 10 万枚以下）
        assert_eq!(a.mesh.indices.len(), a.count * 18);
        let big = field(&rotation_field(), None, dom, Style { count: 24, unit: true, flat: false });
        assert!(big.mesh.indices.len() / 3 <= 100_000);
    }

    #[test]
    fn flat_arrows_stay_in_the_plane() {
        let dom = Domain { lo: [-3.0; 3], hi: [3.0; 3] };
        let a = field(&rotation_field(), None, dom, Style { count: 18, unit: false, flat: true });
        assert_eq!(a.count, 18 * 18);
        assert!(a.mesh.positions.chunks_exact(3).all(|p| p[2] == 0.0));
        assert_eq!(a.mesh.indices.len(), a.count * 3);
    }
}

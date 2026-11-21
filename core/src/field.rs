//! ベクトル場の矢印（計画 `docs/plan-math-notation.md` の M8）。
//!
//! 表示範囲を各軸 n 個に分けた格子の中心で場の値を求め、矢印を置く。矢印 1 本は、太さのある軸（3D は筒、2D は帯）と、
//! 後ろが切れ込んだ矢じり（3D は円錐、2D はダーツの形）のメッシュ。色は大きさ ‖F‖ を頂点ごとの値に入れて、色の対応表で塗る

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
    flat: bool,
    /// 矢じりの円錐の側面の数（3D。軸の筒は、その数の 3/4）
    sides: usize,
}

impl Builder {
    fn new(flat: bool, arrows: usize) -> Builder {
        // 矢印が多いときは、側面を減らして頂点を抑える（24³ 本でも 30 万枚程度）
        Builder { mesh: Mesh::default(), flat, sides: if arrows > 4000 { 6 } else { 8 } }
    }

    fn vertex(&mut self, p: [f64; 3], n: [f64; 3], value: f64) -> u32 {
        let k = (self.mesh.positions.len() / 3) as u32;
        self.mesh.positions.extend(p.map(|c| c as f32));
        self.mesh.normals.extend(n.map(|c| c as f32));
        self.mesh.complex.extend([value as f32, 0.0, 0.0]);
        k
    }

    /// 尾 `tail` から先 `tip` への矢印。`value` は色にする値、`size` は太さ・矢じりの大きさの基準（格子の間隔など）。
    ///
    /// 形: 太さの一定な軸（3D は筒、2D は帯）と、後ろが切れ込んだ矢じり（ダーツの形）。矢じりの長さは `size` の 0.34 倍で、
    /// 矢印が短いときは全長の 0.6 倍まで縮める（短い矢印でも、軸と矢じりがはっきり見えるように）
    fn arrow(&mut self, tail: [f64; 3], tip: [f64; 3], value: f64, size: f64) {
        let v = sub(tip, tail);
        let l = len(v);
        if !(l > 0.0) || !l.is_finite() || !(size > 0.0) {
            return;
        }
        let d = scale(v, 1.0 / l);
        let mut h = 0.34 * size;
        let k = if h > 0.6 * l { 0.6 * l / h } else { 1.0 };
        h *= k;
        // 羽の半幅・切れ込みの深さ（矢じりの長さに対する割合）・軸の半幅
        let w = 0.17 * size * k.sqrt();
        let notch = 0.3 * h;
        let r = (0.05 * size).min(0.45 * w);
        let wing_at = sub(tip, scale(d, h));
        let notch_at = sub(tip, scale(d, h - notch));
        if self.flat {
            let side = [-d[1], d[0], 0.0];
            let up = [0.0, 0.0, 1.0];
            // 軸の帯（尾から切れ込みの点まで）
            let a = self.vertex(add(tail, scale(side, r)), up, value);
            let b = self.vertex(sub(tail, scale(side, r)), up, value);
            let c = self.vertex(sub(notch_at, scale(side, r)), up, value);
            let e = self.vertex(add(notch_at, scale(side, r)), up, value);
            self.mesh.indices.extend([a, b, c, a, c, e]);
            // 矢じり: 先・左の羽・切れ込み・右の羽
            let t = self.vertex(tip, up, value);
            let lw = self.vertex(add(wing_at, scale(side, w)), up, value);
            let n = self.vertex(notch_at, up, value);
            let rw = self.vertex(sub(wing_at, scale(side, w)), up, value);
            self.mesh.indices.extend([t, lw, n, t, n, rw]);
            return;
        }
        // 向きに垂直な 2 つの方向
        let helper = if d[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let e1 = {
            let c = cross(d, helper);
            scale(c, 1.0 / len(c))
        };
        let e2 = cross(d, e1);
        let ring = |n: usize, k: usize| {
            let a = std::f64::consts::TAU * k as f64 / n as f64;
            add(scale(e1, a.cos()), scale(e2, a.sin()))
        };
        let s = self.sides;
        // 矢じりの前の面（円錐）: 法線は母線に垂直。先の頂点は面ごとに置く（先で法線がつぶれないように）
        let front = |e: [f64; 3]| {
            let n = add(scale(e, h), scale(d, w));
            scale(n, 1.0 / len(n))
        };
        let base = (self.mesh.positions.len() / 3) as u32;
        for k in 0..s {
            let e = ring(s, k);
            self.vertex(add(wing_at, scale(e, w)), front(e), value);
        }
        for k in 0..s {
            let mid = ring(2 * s, 2 * k + 1);
            self.vertex(tip, front(mid), value);
        }
        for k in 0..s as u32 {
            let s32 = s as u32;
            self.mesh.indices.extend([base + s32 + k, base + k, base + (k + 1) % s32]);
        }
        // 矢じりの後ろの面（切れ込み。内向きの円錐）
        let back = |e: [f64; 3]| {
            let n = add(scale(d, -w), scale(e, -notch));
            scale(n, 1.0 / len(n))
        };
        let base = (self.mesh.positions.len() / 3) as u32;
        for k in 0..s {
            let e = ring(s, k);
            self.vertex(add(wing_at, scale(e, w)), back(e), value);
        }
        for k in 0..s {
            let mid = ring(2 * s, 2 * k + 1);
            self.vertex(notch_at, back(mid), value);
        }
        for k in 0..s as u32 {
            let s32 = s as u32;
            self.mesh.indices.extend([base + s32 + k, base + (k + 1) % s32, base + k]);
        }
        // 軸の筒（尾から切れ込みの点まで）と、尾のふた
        let m = (s * 3 / 4).max(4);
        let base = (self.mesh.positions.len() / 3) as u32;
        for k in 0..m {
            let e = ring(m, k);
            self.vertex(add(tail, scale(e, r)), e, value);
            self.vertex(add(notch_at, scale(e, r)), e, value);
        }
        let m32 = m as u32;
        for k in 0..m32 {
            let (a0, a1) = (base + 2 * k, base + 2 * k + 1);
            let (b0, b1) = (base + 2 * ((k + 1) % m32), base + 2 * ((k + 1) % m32) + 1);
            self.mesh.indices.extend([a0, b0, b1, a0, b1, a1]);
        }
        let cap = (self.mesh.positions.len() / 3) as u32;
        let back_n = scale(d, -1.0);
        for k in 0..m {
            self.vertex(add(tail, scale(ring(m, k), r)), back_n, value);
        }
        for k in 1..m32 - 1 {
            self.mesh.indices.extend([cap, cap + k + 1, cap + k]);
        }
    }

    fn finish(mut self, range: Option<(f64, f64)>, count: usize) -> Arrows {
        // 小さな矢じりの頂点をまとめる粗い版は作らない
        self.mesh.keep_full = true;
        Arrows { mesh: self.mesh, range, count }
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
    let mut b = Builder::new(style.flat, total);
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
        // 最大の大きさの矢印が、格子の間隔の 0.9 倍、最小でも 0.3 倍（小さい矢印も見えるように。大きさの違いは色でも分かる）。
        // 長さをそろえるなら、全部 0.7 倍
        let l = if style.unit { 0.7 * cell } else { cell * (0.3 + 0.6 * m / max) };
        let half = scale(vecs[k], 0.5 * l / m);
        let c = [pts[0][k], pts[1][k], pts[2][k]];
        b.arrow(sub(c, half), add(c, half), m, cell);
        min = min.min(m);
        count += 1;
    }
    b.finish(Some((if min < max { min } else { 0.0 }, max)), count)
}

/// 流線（計画 M9）: 各軸 `m` 個の格子の点を始点に、$\mathbf x' = \mathbf F(\mathbf x)$ を前と後ろへ解いた曲線。
/// 弧長は、それぞれの向きに表示範囲の対角線の 2 倍まで。表示範囲を出たら止める。点の列を NaN で区切って返す。
/// 2 つ目は点ごとの大きさ ‖F‖（色を付けるため。区切りの所は NaN）
pub fn streamlines(progs: &[Program], rot: Option<[[f64; 3]; 3]>, dom: Domain, m: usize, flat: bool) -> (Vec<f32>, Vec<f32>) {
    let m = m.clamp(1, 12);
    let diag = len(sub(dom.hi, dom.lo));
    let mut buf = [C::new(0.0, 0.0)];
    let mut f = |_: f64, y: &[f64], dy: &mut [f64]| {
        let p = [[y[0]], [y[1]], [if flat { 0.0 } else { y[2] }]];
        let inputs: [&[f64]; 3] = [&p[0], &p[1], &p[2]];
        let mut v = [0.0; 3];
        for (k, prog) in progs.iter().take(3).enumerate() {
            prog.eval_batch(&inputs, &mut buf);
            v[k] = as_real(buf[0]);
        }
        let v = match rot {
            Some(r) => [0, 1, 2].map(|i| r[i][0] * v[0] + r[i][1] * v[1] + r[i][2] * v[2]),
            None => v,
        };
        dy.copy_from_slice(&v);
        if flat {
            dy[2] = 0.0;
        }
        // 表示範囲の外では止める（右辺を有限でなくすと、解法がそこで止まる）
        let pad: [f64; 3] = [0, 1, 2].map(|a| 1e-9 * (dom.hi[a] - dom.lo[a]).abs());
        if (0..if flat { 2 } else { 3 }).any(|a| y[a] < dom.lo[a] - pad[a] || y[a] > dom.hi[a] + pad[a]) {
            dy.iter_mut().for_each(|d| *d = f64::NAN);
        }
    };
    let opt = crate::ode::Options { limit: f64::INFINITY, chord: diag / 300.0, max_arc: 2.0 * diag, arc_dims: 3, max_steps: 4_000 };
    let mut out: Vec<f32> = Vec::new();
    let mut mags: Vec<f32> = Vec::new();
    let mut dy = [0.0; 3];
    let mz = if flat { 1 } else { m };
    for k in 0..mz {
        for j in 0..m {
            for i in 0..m {
                let at = |a: usize, q: usize, n: usize| dom.lo[a] + (q as f64 + 0.5) / n as f64 * (dom.hi[a] - dom.lo[a]);
                let p0 = [at(0, i, m), at(1, j, m), if flat { 0.0 } else { at(2, k, mz) }];
                // 時間の範囲は、弧長の上限で止まるように十分長く
                let back = crate::ode::solve(&mut f, 0.0, &p0, -1e6, opt);
                let fwd = crate::ode::solve(&mut f, 0.0, &p0, 1e6, opt);
                if back.points.len() + fwd.points.len() < 3 {
                    continue;
                }
                if !out.is_empty() {
                    out.extend([f32::NAN; 3]);
                    mags.push(f32::NAN);
                }
                for (_, y) in back.points.iter().rev().chain(fwd.points.iter().skip(1)) {
                    out.extend([y[0] as f32, y[1] as f32, if flat { 0.0 } else { y[2] as f32 }]);
                    f(0.0, &y[..], &mut dy);
                    // 表示範囲の端をわずかに出た最後の点は、1 つ前の値
                    let v = len(dy);
                    let prev = mags.last().copied().filter(|p| p.is_finite()).unwrap_or(0.0);
                    mags.push(if v.is_finite() { v as f32 } else { prev });
                }
            }
        }
    }
    (out, mags)
}

/// 原点からのベクトル。軸は線分、先端は表示範囲に合わせた小さな三角形・円錐。
pub fn vectors(vs: &[[f64; 3]], flat: bool, span: f64) -> Mesh {
    let mut b = Builder::new(flat, vs.len());
    for v in vs {
        let tip = if flat { [v[0], v[1], 0.0] } else { *v };
        let length = len(tip);
        if length == 0.0 || !length.is_finite() || tip.iter().any(|&x| !(x as f32).is_finite()) {
            continue;
        }
        let d = scale(tip, 1.0 / length);
        let height = (0.16 * length).min(0.035 * span);
        let radius = 0.36 * height;
        let base = sub(tip, scale(d, height));
        b.mesh.edge_lines.extend([0.0; 3]);
        b.mesh.edge_lines.extend(base.map(|x| x as f32));
        b.mesh.edge_lines.extend([f32::NAN; 3]);
        if flat {
            let side = scale([-d[1], d[0], 0.0], radius);
            let n = [0.0, 0.0, 1.0];
            let left = b.vertex(sub(base, side), n, 0.0);
            let apex = b.vertex(tip, n, 0.0);
            let right = b.vertex(add(base, side), n, 0.0);
            b.mesh.indices.extend([left, apex, right]);
            continue;
        }

        const SIDES: usize = 12;
        let helper = if d[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let a = cross(d, helper);
        let a = scale(a, 1.0 / len(a));
        let e = cross(d, a);
        let ring = |step: f64| {
            let angle = std::f64::consts::TAU * step / SIDES as f64;
            add(scale(a, angle.cos()), scale(e, angle.sin()))
        };
        let cap = b.vertex(base, scale(d, -1.0), 0.0);
        for k in 0..SIDES {
            let (r1, r2) = (ring(k as f64), ring((k + 1) as f64));
            let (p, q) = (add(base, scale(r1, radius)), add(base, scale(r2, radius)));
            let normal = |r| scale(add(r, scale(d, 0.36)), 1.0 / 1.0_f64.hypot(0.36));
            let i = b.vertex(p, normal(r1), 0.0);
            let j = b.vertex(q, normal(r2), 0.0);
            let apex = b.vertex(tip, normal(ring(k as f64 + 0.5)), 0.0);
            let i_cap = b.vertex(p, scale(d, -1.0), 0.0);
            let j_cap = b.vertex(q, scale(d, -1.0), 0.0);
            b.mesh.indices.extend([apex, i, j, cap, j_cap, i_cap]);
        }
    }
    // 単色の矢じり。複素数の偏角で塗る頂点値は送らない。
    b.mesh.complex.clear();
    b.mesh.keep_full = true;
    b.mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rotation_field() -> Vec<Program> {
        ["-y", "x", "0 x"].iter().map(|s| Program::compile(&crate::parser::parse(s).unwrap(), &["x", "y", "z"]).unwrap()).collect()
    }

    /// 矢印ごとの (中心, 尾から先への向き, 色の値)。先と尾の中心は、[`Builder::arrow`] の頂点の並びから読む
    fn arrow_axes(a: &Arrows, flat: bool) -> Vec<([f64; 3], [f64; 3], f64)> {
        let per = a.mesh.positions.len() / 3 / a.count;
        let at = |k: usize| [0, 1, 2].map(|c| a.mesh.positions[3 * k + c] as f64);
        (0..a.count)
            .map(|i| {
                let o = i * per;
                let (tail, tip) = if flat {
                    (scale(add(at(o), at(o + 1)), 0.5), at(o + 4))
                } else {
                    // 先の頂点は、羽の s 個の後。尾のふたは、最後の m 個
                    let s = if a.count > 4000 { 6 } else { 8 };
                    let m = s * 3 / 4;
                    let tail = (per - m..per).fold([0.0; 3], |acc, k| add(acc, scale(at(o + k), 1.0 / m as f64)));
                    (tail, at(o + s))
                };
                (scale(add(tail, tip), 0.5), sub(tip, tail), a.mesh.complex[3 * o] as f64)
            })
            .collect()
    }

    #[test]
    fn arrows_follow_the_field() {
        let dom = Domain { lo: [-3.0; 3], hi: [3.0; 3] };
        let a = field(&rotation_field(), None, dom, Style { count: 9, unit: false, flat: false });
        // 9³ の格子のうち、z 軸の上（大きさ 0）の 9 点には矢印が無い
        assert_eq!(a.count, 9 * 9 * 9 - 9);
        for (c, dir, m) in arrow_axes(&a, false) {
            // 矢印は場の向き
            let want = [-c[1], c[0], 0.0];
            assert!(len(cross(dir, want)) < 1e-4 * len(dir) * len(want), "{c:?} {dir:?}");
            assert!(dir[0] * want[0] + dir[1] * want[1] > 0.0);
            assert!(len([c[0], c[1], 0.0]) > 0.1);
            assert!((m - len(want)).abs() < 0.2, "{m} {c:?}");
            // 長さは格子の間隔（6/9）の 0.3〜0.9 倍
            let l = len(dir);
            assert!(l > 0.299 * 6.0 / 9.0 && l < 0.901 * 6.0 / 9.0, "{l}");
        }
        // 色にする値は大きさ
        let (lo, hi) = a.range.unwrap();
        assert!(lo > 0.0 && (hi - (2.0 * (8.0f64 / 3.0).powi(2)).sqrt()).abs() < 1e-9);
        assert!(a.mesh.normals.iter().chain(&a.mesh.positions).all(|v| v.is_finite()));
        // 24³ 本でも 30 万枚程度
        let big = field(&rotation_field(), None, dom, Style { count: 24, unit: true, flat: false });
        assert!(big.mesh.indices.len() / 3 <= 320_000, "{}", big.mesh.indices.len() / 3);
    }

    #[test]
    fn small_arrows_stay_visible() {
        // 大きさが 100 倍違う場でも、小さい矢印は最大の矢印の 1/3 以上の長さ
        let progs: Vec<Program> = ["x^3", "0 x", "0 x"].iter().map(|s| Program::compile(&crate::parser::parse(s).unwrap(), &["x", "y", "z"]).unwrap()).collect();
        let dom = Domain { lo: [-3.0; 3], hi: [3.0; 3] };
        let a = field(&progs, None, dom, Style { count: 9, unit: false, flat: true });
        let ls: Vec<f64> = arrow_axes(&a, true).iter().map(|(_, d, _)| len(*d)).collect();
        let (lo, hi) = ls.iter().fold((f64::INFINITY, 0.0f64), |(a, b), &l| (a.min(l), b.max(l)));
        assert!(lo > hi / 3.2, "{lo} {hi}");
    }

    #[test]
    fn streamlines_of_rotation_are_circles() {
        let dom = Domain { lo: [-3.0; 3], hi: [3.0; 3] };
        let (pts, mags) = streamlines(&rotation_field(), None, dom, 3, false);
        assert!(pts.len() > 300);
        assert_eq!(mags.len() * 3, pts.len());
        // 点ごとの値は、その点での大きさ（回転の場では、軸からの距離）
        for (p, m) in pts.chunks_exact(3).zip(&mags) {
            if !p[0].is_nan() {
                assert!(((p[0] * p[0] + p[1] * p[1]).sqrt() - m).abs() < 1e-3);
            }
        }
        // 1 本ずつ: 半径が変わらない（円）
        let mut cur: Vec<[f32; 3]> = Vec::new();
        let mut check = |c: &Vec<[f32; 3]>| {
            if let Some(f) = c.first() {
                let r0 = (f[0] * f[0] + f[1] * f[1]).sqrt();
                assert!(c.iter().all(|p| ((p[0] * p[0] + p[1] * p[1]).sqrt() - r0).abs() < 1e-3 && p[2] == f[2]));
            }
        };
        for p in pts.chunks_exact(3) {
            if p[0].is_nan() {
                check(&cur);
                cur.clear();
            } else {
                cur.push([p[0], p[1], p[2]]);
            }
        }
        check(&cur);
    }

    #[test]
    fn flat_arrows_stay_in_the_plane() {
        let dom = Domain { lo: [-3.0; 3], hi: [3.0; 3] };
        let a = field(&rotation_field(), None, dom, Style { count: 18, unit: false, flat: true });
        assert_eq!(a.count, 18 * 18);
        assert!(a.mesh.positions.chunks_exact(3).all(|p| p[2] == 0.0));
        // 軸の帯 2 枚と矢じり 2 枚
        assert_eq!(a.mesh.indices.len(), a.count * 12);
    }
}

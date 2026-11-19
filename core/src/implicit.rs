//! 等式で書いた曲線・曲面（1 次式に解けない等式 x² + y² = 1・x² + y² + z² = 1 など）の上の積分の点と重み。
//!
//! 変数 u ∈ ℝᵖ（p = 2, 3）の等式 F_j(u) = 0（q 個）と不等式 G_k(u) ≥ 0 で決まる、k = p − q 次元の集合 M（曲線・曲面）の上の
//! k 次元の測度（弧長・面積）についての積分 ∫_M f dμ を、点と重みの和 Σ w_i f(u_i) にする。
//!
//! 1. 1 の分割: M の点ごとに、座標の組 S への射影の拡大率 g_S（曲線は単位接線の成分 |t_a|、曲面は単位法線の成分 |ν_c|。
//!    Σ_S g_S² = 1 なので、どこかの S で g_S ≥ 1/√(S の数)）から、なめらかな重み φ_S = ψ(g_S) / Σ ψ を作る。
//!    ψ は g_S が小さい所（M が射影の向きに折り返す所）で 0 になる、なめらかな階段
//! 2. S ごとに、S の座標を求積の点に固定し、残りの座標（q 個）について等式を解く（切り口の点）
//! 3. dμ = d(S の座標) / g_S なので、点の重みは「求積の重み × φ_S / g_S」。折り返す所では φ_S = 0 なので、
//!    S の座標の関数として有界でなめらか（ガウス・ルジャンドルがよく効く）
//!
//! 不等式で切った端（値が飛ぶ所）は、切り口の点の数が変わる所を二分法で探して、求積の区間の端にする。
//! 向き: 点ごとの枠 [`Node::frame`] は、曲線（平面）なら {F < 0} を左に見る向きの単位接線、空間の曲線なら ∇F₁ × ∇F₂ の向き、
//! 曲面なら e₁ × e₂ = ∇F/|∇F|（{F < 0} から外へ向かう単位法線）になる接ベクトルの組

use crate::eval::{as_real, Program};

type C = num_complex::Complex64;

/// 等式と不等式で書いた集合
pub struct Manifold {
    /// 変数の数（2 か 3）
    pub p: usize,
    /// 等式の左辺 − 右辺 F_j（q 個）
    pub eqs: Vec<Program>,
    /// ∂F_j/∂u_l
    pub grads: Vec<Vec<Program>>,
    /// 不等式 G_k ≥ 0 の G_k
    pub ineqs: Vec<Program>,
    /// 数で書かれた変数の端（0 ≤ z ≤ 1 など）
    pub known: Vec<(Option<f64>, Option<f64>)>,
}

/// 積分の点 1 つ
#[derive(Clone, Copy, Debug)]
pub struct Node {
    pub u: [f64; 3],
    /// 弧長・面積の重み
    pub w: f64,
    /// 曲線: [単位接線, 0]、曲面: [e₁, e₂]（正規直交で、e₁ × e₂ = ∇F/|∇F|）
    pub frame: [[f64; 3]; 2],
}

/// 切り口の 1 次元の標本の数（等式が 1 つ）
const SAMPLES_1D: usize = 40;
/// 切り口の 2 次元の格子の分割数（等式が 2 つ）
const GRID_2D: usize = 32;
/// 求積の最初の区間の数（変数の範囲全体あたり）と、区間を 2 つに分ける回数の上限
const PANELS: usize = 4;
const INNER_DEPTH: u32 = 8;
/// 曲面の外側の変数の、最初の区間の数と、分ける回数の上限
const OUTER_PANELS: usize = 3;
const OUTER_DEPTH: u32 = 10;
/// 切り口の点の数が変わる所を探す標本の数と、二分法の回数
const SIGNATURE: usize = 40;
const SIG_BISECTIONS: usize = 22;
/// 区間を分けるかを決める、クロンロッドとガウスの値の差の目標（相対）
const TOL: f64 = 1e-5;
/// 1 の分割の重みの指数 m
const POWER: i32 = 12;
/// 箱を広げる回数の上限
const MAX_GROW: usize = 14;

fn eval(prog: &Program, pts: &[[f64; 3]], p: usize) -> Vec<f64> {
    let cols: Vec<Vec<f64>> = (0..p).map(|l| pts.iter().map(|q| q[l]).collect()).collect();
    let inputs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
    let mut out = vec![C::new(0.0, 0.0); pts.len()];
    prog.eval_batch(&inputs, &mut out);
    out.iter().map(|v| as_real(*v)).collect()
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn unit(a: [f64; 3]) -> Option<[f64; 3]> {
    let n = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    (n.is_finite() && n > 1e-300).then(|| [a[0] / n, a[1] / n, a[2] / n])
}

/// 射影の組 1 つ: 求積で動かす座標 `sweep` と、等式を解く座標 `free`
#[derive(Clone)]
struct Chart {
    sweep: Vec<usize>,
    free: Vec<usize>,
}

struct Solver<'a> {
    m: &'a Manifold,
    q: usize,
    lo: [f64; 3],
    hi: [f64; 3],
    charts: Vec<Chart>,
}

impl Solver<'_> {
    /// 切り口: seeds の点（free の座標は使わない）ごとに、free の座標について等式を解き、不等式を満たす点を返す
    fn slice(&self, free: &[usize], seeds: &[[f64; 3]]) -> Vec<Vec<[f64; 3]>> {
        let roots = if free.len() == 1 { self.roots_1d(free[0], seeds) } else { self.roots_2d(free, seeds) };
        // 不等式と箱
        let flat: Vec<(usize, [f64; 3])> = roots.iter().enumerate().flat_map(|(s, rs)| rs.iter().map(move |r| (s, *r))).collect();
        let pts: Vec<[f64; 3]> = flat.iter().map(|(_, r)| *r).collect();
        let mut ok = vec![true; pts.len()];
        if !pts.is_empty() {
            for g in &self.m.ineqs {
                for (o, v) in ok.iter_mut().zip(eval(g, &pts, self.m.p)) {
                    *o = *o && v >= 0.0;
                }
            }
        }
        let mut out = vec![Vec::new(); seeds.len()];
        for ((s, r), o) in flat.into_iter().zip(ok) {
            if o {
                out[s].push(r);
            }
        }
        out
    }

    /// 等式 1 つ: 座標 c の標本で符号の変わる所を探し、二分法で根にする（極で符号が変わる所は除く）
    fn roots_1d(&self, c: usize, seeds: &[[f64; 3]]) -> Vec<Vec<[f64; 3]>> {
        let p = self.m.p;
        let f = &self.m.eqs[0];
        let (lo, hi) = (self.lo[c], self.hi[c]);
        let ns = SAMPLES_1D;
        let at = |j: usize| lo + (hi - lo) * j as f64 / (ns - 1) as f64;
        let mut pts = Vec::with_capacity(seeds.len() * ns);
        for s in seeds {
            for j in 0..ns {
                let mut q = *s;
                q[c] = at(j);
                pts.push(q);
            }
        }
        let vals = eval(f, &pts, p);
        let mut out = vec![Vec::new(); seeds.len()];
        // 二分法の区間（種の番号, 左, 右, 左の値, 両端の値の大きさ）
        // 挟んだ区間（種の番号, 左, 右, 左の値, 右の値, 両端の値の大きさ）
        let mut br: Vec<(usize, f64, f64, f64, f64, f64)> = Vec::new();
        for (s, seed) in seeds.iter().enumerate() {
            for j in 0..ns {
                let va = vals[s * ns + j];
                if va == 0.0 {
                    let mut q = *seed;
                    q[c] = at(j);
                    out[s].push(q);
                    continue;
                }
                if j + 1 < ns {
                    let vb = vals[s * ns + j + 1];
                    if va.is_finite() && vb.is_finite() && vb != 0.0 && (va < 0.0) != (vb < 0.0) {
                        br.push((s, at(j), at(j + 1), va, vb, va.abs().max(vb.abs())));
                    }
                }
            }
        }
        // 近い 2 つの根（標本の 1 区間に 2 つ入り、両端の符号が同じ。円錐の頂点の近く・細い所）: |F| が標本で極小になる所で、
        // F を c で微分した値が 0 になる点（F の山・谷）を二分法で探し、そこで符号が変われば、2 つの区間に分けて挟む
        let dfc = &self.m.grads[0][c];
        let mut dips: Vec<(usize, f64, f64, f64, f64)> = Vec::new();
        // （標本の番号, 曲がり具合, 放物線の谷の深さ）
        let mut depth: Vec<(usize, f64, f64)> = Vec::new();
        for s in 0..seeds.len() {
            for j in 1..ns - 1 {
                let (u, v, w) = (vals[s * ns + j - 1], vals[s * ns + j], vals[s * ns + j + 1]);
                if !(u.is_finite() && v.is_finite() && w.is_finite() && v != 0.0 && (u < 0.0) == (v < 0.0) && (w < 0.0) == (v < 0.0) && v.abs() <= u.abs() && v.abs() <= w.abs()) {
                    continue;
                }
                // 3 点を通る放物線の谷の深さ: 谷が 0 の近くまで届きそうなときだけ調べる（遠くの線の、ゆるやかな谷は飛ばす）
                let (ua, va, wa) = (u.abs(), v.abs(), w.abs());
                let curv = ua - 2.0 * va + wa;
                let low = if curv > 0.0 { va - (wa - ua) * (wa - ua) / (8.0 * curv) } else { va };
                // 同じ値の 2 点が並ぶ谷（左右対称）は、左の 1 つだけ
                if low < 0.1 * va && !(v.abs() == u.abs() && j > 1) {
                    dips.push((s, at(j - 1), at(j + 1), u, w));
                    depth.push((j, curv, low));
                }
            }
        }
        // 折り返し（F を c で微分した値が 0 で、勾配のほかの成分が大きい所）の近くの根の組は、射影の拡大率 g_c が小さく、
        // 1 の分割の重み g_c^m がほぼ 0 なので、探さない。根の組の間の傾きから g_c を見積もる（特異点 — 円錐の頂点 — では大きい）
        if !dips.is_empty() {
            let h = (hi - lo) / (ns - 1) as f64;
            let mid: Vec<[f64; 3]> = dips
                .iter()
                .zip(&depth)
                .map(|(d, &(j, _, _))| {
                    let mut q = seeds[d.0];
                    q[c] = at(j);
                    q
                })
                .collect();
            let mut perp = vec![0.0; mid.len()];
            for l in (0..p).filter(|&l| l != c) {
                for (s2, v) in perp.iter_mut().zip(eval(&self.m.grads[0][l], &mid, p)) {
                    *s2 += v * v;
                }
            }
            let keep: Vec<bool> = depth
                .iter()
                .zip(&perp)
                .map(|(&(_, curv, low), &pp)| {
                    // 谷が 0 より上と見積もれば、勾配がほかの向きに大きくない所（特異点の近く）だけ調べる
                    let g = (2.0 * low.abs() * curv.max(0.0)).sqrt() / h / pp.sqrt();
                    // g が NaN（勾配のほかの成分が 0 = 特異点）でも、谷が 0 より下なら調べる
                    (g.is_nan() || g > 0.05) && (low < 0.0 || g > 0.2)
                })
                .collect();
            let mut k = keep.iter();
            dips.retain(|_| *k.next().unwrap_or(&false));
        }
        if !dips.is_empty() {
            let put = |s: usize, x: f64| {
                let mut q = seeds[s];
                q[c] = x;
                q
            };
            let ends: Vec<[f64; 3]> = dips.iter().flat_map(|d| [put(d.0, d.1), put(d.0, d.2)]).collect();
            let de = eval(dfc, &ends, p);
            // (種, 左, 右, 左の微分の符号)
            let mut search: Vec<(usize, f64, f64, bool, usize)> = Vec::new();
            for (i, d) in dips.iter().enumerate() {
                let (da, db) = (de[2 * i], de[2 * i + 1]);
                if da.is_finite() && db.is_finite() && (da < 0.0) != (db < 0.0) {
                    search.push((d.0, d.1, d.2, da < 0.0, i));
                }
            }
            for _ in 0..32 {
                if search.is_empty() {
                    break;
                }
                let mids: Vec<[f64; 3]> = search.iter().map(|e| put(e.0, 0.5 * (e.1 + e.2))).collect();
                let dm = eval(dfc, &mids, p);
                for (e, v) in search.iter_mut().zip(dm) {
                    let m = 0.5 * (e.1 + e.2);
                    if (v < 0.0) == e.3 {
                        e.1 = m;
                    } else {
                        e.2 = m;
                    }
                }
            }
            let mids: Vec<[f64; 3]> = search.iter().map(|e| put(e.0, 0.5 * (e.1 + e.2))).collect();
            let fm = eval(f, &mids, p);
            for (e, v) in search.iter().zip(fm) {
                let d = dips[e.4];
                if v.is_finite() && v != 0.0 && (v < 0.0) != (d.3 < 0.0) {
                    let m = 0.5 * (e.1 + e.2);
                    br.push((d.0, d.1, m, d.3, v, d.3.abs().max(v.abs())));
                    br.push((d.0, m, d.2, v, d.4, d.4.abs().max(v.abs())));
                }
            }
        }
        // イリノイ法（はさみうち法で、同じ側が続けて動いたら、動かない側の値を半分にする）。根を挟んだまま速く縮む
        let tol = 4.0 * f64::EPSILON * (hi - lo).abs().max(lo.abs()).max(hi.abs());
        let mut side = vec![0i8; br.len()];
        let mut root = vec![f64::NAN; br.len()];
        let mut active: Vec<usize> = (0..br.len()).collect();
        for _ in 0..80 {
            if active.is_empty() {
                break;
            }
            let xs: Vec<f64> = active
                .iter()
                .map(|&i| {
                    let (_, a, b, fa, fb, _) = br[i];
                    let x = (a * fb - b * fa) / (fb - fa);
                    if x.is_finite() && x > a && x < b {
                        x
                    } else {
                        0.5 * (a + b)
                    }
                })
                .collect();
            let pts: Vec<[f64; 3]> = active
                .iter()
                .zip(&xs)
                .map(|(&i, &x)| {
                    let mut q = seeds[br[i].0];
                    q[c] = x;
                    q
                })
                .collect();
            let fx = eval(f, &pts, p);
            let mut next = Vec::with_capacity(active.len());
            for ((&i, &x), v) in active.iter().zip(&xs).zip(fx) {
                let e = &mut br[i];
                root[i] = x;
                if v == 0.0 || !v.is_finite() {
                    continue;
                }
                if (v < 0.0) == (e.3 < 0.0) {
                    e.1 = x;
                    e.3 = v;
                    if side[i] == -1 {
                        e.4 *= 0.5;
                    }
                    side[i] = -1;
                } else {
                    e.2 = x;
                    e.4 = v;
                    if side[i] == 1 {
                        e.3 *= 0.5;
                    }
                    side[i] = 1;
                }
                if e.2 - e.1 <= tol {
                    root[i] = 0.5 * (e.1 + e.2);
                } else {
                    next.push(i);
                }
            }
            active = next;
        }
        let fin: Vec<[f64; 3]> = br
            .iter()
            .zip(&root)
            .map(|(e, &x)| {
                let mut q = seeds[e.0];
                q[c] = x;
                q
            })
            .collect();
        let fv = eval(f, &fin, p);
        for ((e, q), v) in br.iter().zip(fin).zip(fv) {
            // 根なら値はほぼ 0。極（1/x の符号の変わり目）なら大きいまま
            if v.is_finite() && v.abs() <= 1e-6 * e.5 {
                out[e.0].push(q);
            }
        }
        out
    }

    /// 等式 2 つ: free の 2 座標の格子で、両方の符号が変わる升目からニュートン法で解く
    fn roots_2d(&self, free: &[usize], seeds: &[[f64; 3]]) -> Vec<Vec<[f64; 3]>> {
        let p = self.m.p;
        let (b, c) = (free[0], free[1]);
        let n = GRID_2D;
        let hb = (self.hi[b] - self.lo[b]) / (n - 1) as f64;
        let hc = (self.hi[c] - self.lo[c]) / (n - 1) as f64;
        let mut pts = Vec::with_capacity(seeds.len() * n * n);
        for s in seeds {
            for i in 0..n {
                for j in 0..n {
                    let mut q = *s;
                    q[b] = self.lo[b] + hb * i as f64;
                    q[c] = self.lo[c] + hc * j as f64;
                    pts.push(q);
                }
            }
        }
        let f1 = eval(&self.m.eqs[0], &pts, p);
        let f2 = eval(&self.m.eqs[1], &pts, p);
        let changes = |v: &[f64], k: usize| {
            let cs = [v[k], v[k + 1], v[k + n], v[k + n + 1]];
            if cs.iter().any(|x| !x.is_finite()) {
                return false;
            }
            let mn = cs.iter().cloned().fold(f64::INFINITY, f64::min);
            let mx = cs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            mn <= 0.0 && mx >= 0.0
        };
        // 始点（種の番号, 点, 升目の中心）
        let mut cur: Vec<(usize, [f64; 3], [f64; 3])> = Vec::new();
        for (s, seed) in seeds.iter().enumerate() {
            let base = s * n * n;
            for i in 0..n - 1 {
                for j in 0..n - 1 {
                    let k = base + i * n + j;
                    if changes(&f1, k) && changes(&f2, k) {
                        let mut q = *seed;
                        q[b] = self.lo[b] + hb * (i as f64 + 0.5);
                        q[c] = self.lo[c] + hc * (j as f64 + 0.5);
                        cur.push((s, q, q));
                    }
                }
            }
        }
        let mut out = vec![Vec::new(); seeds.len()];
        if cur.is_empty() {
            return out;
        }
        let g = &self.m.grads;
        for _ in 0..30 {
            let ps: Vec<[f64; 3]> = cur.iter().map(|c| c.1).collect();
            let v1 = eval(&self.m.eqs[0], &ps, p);
            let v2 = eval(&self.m.eqs[1], &ps, p);
            let a11 = eval(&g[0][b], &ps, p);
            let a12 = eval(&g[0][c], &ps, p);
            let a21 = eval(&g[1][b], &ps, p);
            let a22 = eval(&g[1][c], &ps, p);
            for (k, e) in cur.iter_mut().enumerate() {
                let det = a11[k] * a22[k] - a12[k] * a21[k];
                if !det.is_finite() || det == 0.0 {
                    continue;
                }
                let db = (v1[k] * a22[k] - v2[k] * a12[k]) / det;
                let dc = (a11[k] * v2[k] - a21[k] * v1[k]) / det;
                // 1 回に升目 2 つ分まで
                let scale = (db.abs() / (2.0 * hb)).max(dc.abs() / (2.0 * hc)).max(1.0);
                e.1[b] -= db / scale;
                e.1[c] -= dc / scale;
            }
        }
        let ps: Vec<[f64; 3]> = cur.iter().map(|c| c.1).collect();
        let v1 = eval(&self.m.eqs[0], &ps, p);
        let v2 = eval(&self.m.eqs[1], &ps, p);
        let n1: Vec<f64> = (0..p).map(|l| eval(&g[0][l], &ps, p)).fold(vec![0.0; ps.len()], |acc, col| acc.iter().zip(col).map(|(a, x)| a + x * x).collect());
        let n2: Vec<f64> = (0..p).map(|l| eval(&g[1][l], &ps, p)).fold(vec![0.0; ps.len()], |acc, col| acc.iter().zip(col).map(|(a, x)| a + x * x).collect());
        let tol = 1e-9 * hb.max(hc);
        for (k, (s, q, center)) in cur.into_iter().enumerate() {
            let d1 = v1[k].abs() / n1[k].sqrt();
            let d2 = v2[k].abs() / n2[k].sqrt();
            // 始めた升目の近く（隣の升目まで）に収まった根だけ（同じ根を遠くの升目から拾わない）
            let near = (q[b] - center[b]).abs() <= 1.5 * hb && (q[c] - center[c]).abs() <= 1.5 * hc;
            if !(d1 <= tol && d2 <= tol && near) {
                continue;
            }
            if (q[b] - self.lo[b]) < -tol || (q[b] - self.hi[b]) > tol || (q[c] - self.lo[c]) < -tol || (q[c] - self.hi[c]) > tol {
                continue;
            }
            if out[s].iter().any(|r: &[f64; 3]| (r[b] - q[b]).abs() < 0.25 * hb && (r[c] - q[c]).abs() < 0.25 * hc) {
                continue;
            }
            out[s].push(q);
        }
        out
    }

    /// 点の枠（[`Node::frame`]）。勾配が 0（特異点）なら None
    fn frames(&self, pts: &[[f64; 3]]) -> Vec<Option<[[f64; 3]; 2]>> {
        let p = self.m.p;
        let grad = |j: usize| -> Vec<[f64; 3]> {
            let cols: Vec<Vec<f64>> = (0..p).map(|l| eval(&self.m.grads[j][l], pts, p)).collect();
            (0..pts.len()).map(|i| [cols[0][i], cols[1][i], if p == 3 { cols[2][i] } else { 0.0 }]).collect()
        };
        let g1 = grad(0);
        if self.q == 2 {
            let g2 = grad(1);
            return g1.iter().zip(&g2).map(|(a, b)| unit(cross(*a, *b)).map(|t| [t, [0.0; 3]])).collect();
        }
        g1.iter()
            .map(|g| {
                let nu = unit(*g)?;
                if p == 2 {
                    return Some([[-nu[1], nu[0], 0.0], [0.0; 3]]);
                }
                // e₁: 法線にいちばん垂直に近い座標の向きとの外積
                let k = (0..3).min_by(|&i, &j| nu[i].abs().total_cmp(&nu[j].abs())).unwrap_or(0);
                let mut axis = [0.0; 3];
                axis[k] = 1.0;
                let e1 = unit(cross(nu, axis))?;
                let e2 = cross(nu, e1);
                Some([e1, e2])
            })
            .collect()
    }

    /// 射影 S への拡大率 g_S（曲線は |t_a|、曲面は |ν_c|）
    fn jac(&self, chart: &Chart, frame: &[[f64; 3]; 2]) -> f64 {
        if chart.sweep.len() == 1 {
            frame[0][chart.sweep[0]].abs()
        } else {
            cross(frame[0], frame[1])[chart.free[0]].abs()
        }
    }

    /// 1 の分割の重み φ_S = g_S^m / Σ g^m（射影の組ごと）。折り返す所（g_S → 0）では、重み / g_S が g_S^(m−1) で 0 になる
    fn partition(&self, frame: &[[f64; 3]; 2]) -> Vec<f64> {
        let pw: Vec<f64> = self.charts.iter().map(|c| self.jac(c, frame).powi(POWER)).collect();
        let total: f64 = pw.iter().sum();
        pw.iter().map(|v| if total > 0.0 { v / total } else { 0.0 }).collect()
    }

    /// 区間 [a, b] を端 `cuts` で分けた区間の列
    fn intervals(a: f64, b: f64, cuts: &[f64]) -> Vec<(f64, f64)> {
        let mut ends: Vec<f64> = std::iter::once(a).chain(cuts.iter().cloned().filter(|&c| c > a && c < b)).chain(std::iter::once(b)).collect();
        ends.sort_by(f64::total_cmp);
        ends.dedup_by(|x, y| (*x - *y).abs() <= 1e-13 * (b - a).abs().max(1e-300));
        ends.windows(2).map(|w| (w[0], w[1])).collect()
    }

    /// 何本かの線の上の、まとめて進める適応型の求積（区間ごとにガウス・クロンロッドの 7–15 点。2 つの値の差が目標より大きい区間は
    /// 2 つに分ける）。`intervals[i]` は線 i の区間の列。`eval` は（線の番号, 座標の値）の列から、値ごとの点の列（重みは密度）を返す。
    /// `locate` は（線の番号, 点の数が違う 2 つの値）の列から、点の数が変わる所を返す（あれば、区間をそこで分ける）。
    /// 返すのは線ごとの点で、重みは「求積の重み × 密度」
    #[allow(clippy::type_complexity)]
    fn adapt(
        intervals: &[Vec<(f64, f64)>],
        unit: f64,
        max_depth: u32,
        eval: &dyn Fn(&[(usize, f64)]) -> Vec<Vec<Node>>,
        locate: Option<&dyn Fn(&[(usize, f64, f64)]) -> Vec<f64>>,
    ) -> Vec<Vec<Node>> {
        use crate::quad::{WG, WGK, XGK};
        // 15 点の、区間の中心からの位置（半幅を 1 とする）と、クロンロッド・ガウスの重み
        let mut rule: Vec<(f64, f64, f64)> = Vec::with_capacity(15);
        for j in 0..7 {
            let wg = if j % 2 == 1 { WG[j / 2] } else { 0.0 };
            rule.push((-XGK[j], WGK[j], wg));
            rule.push((XGK[j], WGK[j], wg));
        }
        rule.push((0.0, WGK[7], WG[3]));
        // 位置の順に並べた番号（点の数が変わる、隣り合う 2 点を探す）
        let mut order: Vec<usize> = (0..15).collect();
        order.sort_by(|&i, &j| rule[i].0.total_cmp(&rule[j].0));
        let mut work: Vec<(usize, f64, f64, u32)> = Vec::new();
        for (line, ivs) in intervals.iter().enumerate() {
            for &(l, r) in ivs {
                let n = (((r - l) / unit).ceil() as usize).max(1);
                let h = (r - l) / n as f64;
                for k in 0..n {
                    work.push((line, l + h * k as f64, if k + 1 == n { r } else { l + h * (k + 1) as f64 }, 0));
                }
            }
        }
        let mut out = vec![Vec::new(); intervals.len()];
        while !work.is_empty() {
            let xs: Vec<(usize, f64)> = work.iter().flat_map(|&(i, l, r, _)| rule.iter().map(move |&(x, _, _)| (i, 0.5 * (l + r) + 0.5 * (r - l) * x))).collect();
            let mut vals = eval(&xs).into_iter();
            // 分ける区間（線, 左, 右, 深さ）と、点の数が変わる所を探す区間の組
            let mut split: Vec<(usize, f64, f64, u32)> = Vec::new();
            let mut asks: Vec<(usize, f64, f64)> = Vec::new();
            let mut ask_of: Vec<usize> = Vec::new();
            for &(i, l, r, d) in &work {
                let h = 0.5 * (r - l);
                let pts: Vec<Vec<Node>> = (0..15).map(|_| vals.next().unwrap_or_default()).collect();
                let (mut k, mut g) = (0.0, 0.0);
                for (ns, &(_, wk, wg)) in pts.iter().zip(&rule) {
                    let v: f64 = ns.iter().map(|n| n.w).sum();
                    k += wk * h * v;
                    g += wg * h * v;
                }
                if (k - g).abs() <= TOL * k.abs() + 1e-15 * h || d >= max_depth {
                    for (ns, &(_, wk, _)) in pts.into_iter().zip(&rule) {
                        out[i].extend(ns.into_iter().map(|n| Node { w: n.w * wk * h, ..n }));
                    }
                    continue;
                }
                if locate.is_some() {
                    for w in order.windows(2) {
                        if pts[w[0]].len() != pts[w[1]].len() {
                            let at = |j: usize| 0.5 * (l + r) + h * rule[j].0;
                            asks.push((i, at(w[0]), at(w[1])));
                            ask_of.push(split.len());
                        }
                    }
                }
                split.push((i, l, r, d));
            }
            // 点の数が変わる所（標本で拾えなかった短い切り口・すき間）があれば、そこでも分ける
            let found = match locate {
                Some(loc) if !asks.is_empty() => loc(&asks),
                _ => Vec::new(),
            };
            let mut cuts: Vec<Vec<f64>> = split.iter().map(|&(_, l, r, _)| vec![0.5 * (l + r)]).collect();
            for (&k, x) in ask_of.iter().zip(found) {
                cuts[k].push(x);
            }
            let mut next = Vec::new();
            for (&(i, l, r, d), mut cs) in split.iter().zip(cuts) {
                cs.sort_by(f64::total_cmp);
                let mut from = l;
                for c in cs.into_iter().filter(|&c| c > l && c < r) {
                    if c - from > 1e-14 * (r - l) {
                        next.push((i, from, c, d + 1));
                        from = c;
                    }
                }
                next.push((i, from, r, d + 1));
            }
            work = next;
        }
        out
    }

    /// 数で書かれた端（箱の中にあるもの）
    fn known_cuts(&self, axis: usize) -> Vec<f64> {
        let (l, h) = self.m.known[axis];
        [l, h].into_iter().flatten().collect()
    }

    /// 動かす座標の値の組 seeds ごとの、切り口の点（重みは密度 φ_S / g_S）
    fn density(&self, ci: usize, seeds: &[[f64; 3]]) -> Vec<Vec<Node>> {
        let chart = &self.charts[ci];
        let found = self.slice(&chart.free, seeds);
        let flat: Vec<(usize, [f64; 3])> = found.iter().enumerate().flat_map(|(s, rs)| rs.iter().map(move |r| (s, *r))).collect();
        let pts: Vec<[f64; 3]> = flat.iter().map(|f| f.1).collect();
        let frames = self.frames(&pts);
        let mut out = vec![Vec::new(); seeds.len()];
        for ((s, u), fr) in flat.into_iter().zip(frames) {
            let Some(fr) = fr else { continue };
            let phi = self.partition(&fr)[ci];
            let g = self.jac(chart, &fr);
            if phi > 0.0 && g > 0.0 {
                out[s].push(Node { u, w: phi / g, frame: fr });
            }
        }
        out
    }

    /// 座標 a を動かす線（ほかの座標は bases のそれぞれ）の点と重み（まとめて計算する）。
    /// 切り口の点の数が変わる所（折り返し・不等式の端）を区間の端にする
    fn lines(&self, ci: usize, bases: &[[f64; 3]], a: usize) -> Vec<Vec<Node>> {
        let chart = &self.charts[ci];
        let (lo, hi) = (self.lo[a], self.hi[a]);
        let at = |j: usize| lo + (hi - lo) * j as f64 / (SIGNATURE - 1) as f64;
        let put = |i: usize, x: f64| {
            let mut q = bases[i];
            q[a] = x;
            q
        };
        let count = |asks: &[(usize, f64)]| -> Vec<usize> {
            let seeds: Vec<[f64; 3]> = asks.iter().map(|&(i, x)| put(i, x)).collect();
            self.slice(&chart.free, &seeds).iter().map(|v| v.len()).collect()
        };
        let samples: Vec<(usize, f64)> = (0..bases.len()).flat_map(|i| (0..SIGNATURE).map(move |j| (i, at(j)))).collect();
        let counts = count(&samples);
        // 点の数が変わる所を、二分法で詰める（線 i, 左, 右, 左の数 → 見つけた所）
        let bisect = |br: &mut Vec<(usize, f64, f64, usize)>, rounds: usize| {
            for _ in 0..rounds {
                if br.is_empty() {
                    break;
                }
                let mids: Vec<(usize, f64)> = br.iter().map(|&(i, l, r, _)| (i, 0.5 * (l + r))).collect();
                for (b, c) in br.iter_mut().zip(count(&mids)) {
                    let m = 0.5 * (b.1 + b.2);
                    if c == b.3 {
                        b.1 = m;
                    } else {
                        b.2 = m;
                    }
                }
            }
        };
        let mut br: Vec<(usize, f64, f64, usize)> = Vec::new();
        for i in 0..bases.len() {
            let cs = &counts[i * SIGNATURE..(i + 1) * SIGNATURE];
            for j in 0..SIGNATURE - 1 {
                if cs[j] != cs[j + 1] {
                    br.push((i, at(j), at(j + 1), cs[j]));
                }
            }
        }
        bisect(&mut br, SIG_BISECTIONS);
        let known = self.known_cuts(a);
        let mut ivs: Vec<Vec<(f64, f64)>> = Vec::with_capacity(bases.len());
        for i in 0..bases.len() {
            let cs = &counts[i * SIGNATURE..(i + 1) * SIGNATURE];
            let (Some(first), Some(last)) = (cs.iter().position(|&c| c > 0), cs.iter().rposition(|&c| c > 0)) else {
                ivs.push(Vec::new());
                continue;
            };
            let found: Vec<f64> = br.iter().filter(|b| b.0 == i).map(|b| 0.5 * (b.1 + b.2)).collect();
            // 点の無い端の区間は落とす（求積の点を、点のある所に集める）
            let from = if first == 0 { lo } else { found.iter().cloned().filter(|&x| x <= at(first)).fold(at(first - 1), f64::max) };
            let to = if last == SIGNATURE - 1 { hi } else { found.iter().cloned().filter(|&x| x >= at(last)).fold(at(last + 1), f64::min) };
            let mut cuts = known.clone();
            cuts.extend(found);
            ivs.push(Self::intervals(from, to, &cuts));
        }
        let eval = |xs: &[(usize, f64)]| self.density(ci, &xs.iter().map(|&(i, x)| put(i, x)).collect::<Vec<_>>());
        let locate = |asks: &[(usize, f64, f64)]| -> Vec<f64> {
            let left = count(&asks.iter().map(|&(i, l, _)| (i, l)).collect::<Vec<_>>());
            let mut br: Vec<(usize, f64, f64, usize)> = asks.iter().zip(left).map(|(&(i, l, r), c)| (i, l.min(r), l.max(r), c)).collect();
            bisect(&mut br, SIG_BISECTIONS + 8);
            br.iter().map(|b| 0.5 * (b.1 + b.2)).collect()
        };
        Self::adapt(&ivs, (hi - lo) / PANELS as f64, INNER_DEPTH, &eval, Some(&locate))
    }

    /// 射影 1 つの点と重み
    fn chart_nodes(&self, ci: usize) -> Vec<Node> {
        let chart = &self.charts[ci];
        let mid = [0.5 * (self.lo[0] + self.hi[0]), 0.5 * (self.lo[1] + self.hi[1]), 0.5 * (self.lo[2] + self.hi[2])];
        if chart.sweep.len() == 1 {
            return self.lines(ci, &[mid], chart.sweep[0]).pop().unwrap_or_default();
        }
        let (a, b) = (chart.sweep[0], chart.sweep[1]);
        // 外側の座標 a の値ごとに、内側の座標 b の線（同じ回にまとめて）。線の点の重みが、外側の求積の「密度」になる
        let eval = |xs: &[(usize, f64)]| -> Vec<Vec<Node>> {
            let bases: Vec<[f64; 3]> = xs
                .iter()
                .map(|&(_, x)| {
                    let mut q = mid;
                    q[a] = x;
                    q
                })
                .collect();
            self.lines(ci, &bases, b)
        };
        let iv = vec![Self::intervals(self.lo[a], self.hi[a], &self.known_cuts(a))];
        Self::adapt(&iv, (self.hi[a] - self.lo[a]) / OUTER_PANELS as f64, OUTER_DEPTH, &eval, None).pop().unwrap_or_default()
    }

    /// 粗い標本の点（箱を決めるため）
    fn coarse(&self) -> Vec<[f64; 3]> {
        let mut pts = Vec::new();
        for chart in &self.charts {
            let n = if chart.sweep.len() == 1 { 48 } else { 20 };
            let mut seeds = Vec::new();
            let grid = |a: usize, i: usize| self.lo[a] + (self.hi[a] - self.lo[a]) * (i as f64 + 0.5) / n as f64;
            if chart.sweep.len() == 1 {
                for i in 0..n {
                    let mut q = [0.0; 3];
                    q[chart.sweep[0]] = grid(chart.sweep[0], i);
                    seeds.push(q);
                }
            } else {
                for i in 0..n {
                    for j in 0..n {
                        let mut q = [0.0; 3];
                        q[chart.sweep[0]] = grid(chart.sweep[0], i);
                        q[chart.sweep[1]] = grid(chart.sweep[1], j);
                        seeds.push(q);
                    }
                }
            }
            pts.extend(self.slice(&chart.free, &seeds).into_iter().flatten());
        }
        pts
    }
}

/// 集合の上の積分の点と重み。集合が空・限りなく続く・次元が合わないなら、その説明
pub fn nodes(m: &Manifold) -> Result<Vec<Node>, String> {
    let p = m.p;
    let q = m.eqs.len();
    if !(2..=3).contains(&p) || q == 0 || q >= p {
        return Err("内部エラー: 等式で書いた集合の次元".into());
    }
    let k = p - q;
    let charts: Vec<Chart> = if k == 1 {
        (0..p).map(|a| Chart { sweep: vec![a], free: (0..p).filter(|&b| b != a).collect() }).collect()
    } else {
        (0..3).map(|c| Chart { sweep: (0..3).filter(|&b| b != c).collect(), free: vec![c] }).collect()
    };
    let mut s = Solver { m, q, lo: [0.0; 3], hi: [0.0; 3], charts };
    // 箱: 数で書かれた端はそのまま、無い向きは原点のまわりから広げる
    let mut open = [[false; 2]; 3];
    for a in 0..p {
        let (l, h) = m.known[a];
        let (l, h) = match (l, h) {
            (Some(l), Some(h)) => (l, h),
            (Some(l), None) => (l, l + 4.0f64.max(2.0 * l.abs())),
            (None, Some(h)) => (h - 4.0f64.max(2.0 * h.abs()), h),
            (None, None) => (-2.0, 2.0),
        };
        if l.is_nan() || h.is_nan() || l >= h {
            return Err("等式で書いた曲線・曲面の範囲が空です".into());
        }
        s.lo[a] = l;
        s.hi[a] = h;
        open[a] = [m.known[a].0.is_none(), m.known[a].1.is_none()];
    }
    let mut pts = Vec::new();
    for round in 0..=MAX_GROW {
        pts = s.coarse();
        let mut grow = false;
        for a in 0..p {
            let span = s.hi[a] - s.lo[a];
            let near_lo = pts.iter().any(|q| q[a] - s.lo[a] < 0.04 * span);
            let near_hi = pts.iter().any(|q| s.hi[a] - q[a] < 0.04 * span);
            // 点が見つからないなら、端の無い向きをみな広げる（遠くにある図形）
            if open[a][0] && (near_lo || pts.is_empty()) {
                s.lo[a] -= span;
                grow = true;
            }
            if open[a][1] && (near_hi || pts.is_empty()) {
                s.hi[a] += span;
                grow = true;
            }
        }
        if !grow {
            break;
        }
        if round == MAX_GROW {
            return Err(if pts.is_empty() {
                "等式の曲線・曲面が見つかりません（範囲が空か、等式の両辺の差が符号を変えません）".into()
            } else {
                "曲線・曲面が限りなく続いています。不等式で範囲を区切ってください（例: −1 ≤ z ≤ 1）".into()
            });
        }
    }
    if pts.is_empty() {
        return Err("等式の曲線・曲面が見つかりません（範囲が空か、等式の両辺の差が符号を変えません）".into());
    }
    // 端の書かれていない向きは、点のある所まで箱を詰める（粗い標本の升目 2 つ分ほどの余白）。
    // 書かれた端（0 ≤ z）は詰めない（粗い標本が拾わなかった細い所 — 円錐の頂点の近くなど — を切り落とさない）
    for a in 0..p {
        let span = s.hi[a] - s.lo[a];
        let mn = pts.iter().map(|q| q[a]).fold(f64::INFINITY, f64::min);
        let mx = pts.iter().map(|q| q[a]).fold(f64::NEG_INFINITY, f64::max);
        let pad = span / 10.0;
        if open[a][0] {
            s.lo[a] = s.lo[a].max(mn - pad);
        }
        if open[a][1] {
            s.hi[a] = s.hi[a].min(mx + pad);
        }
    }
    let mut out = Vec::new();
    for ci in 0..s.charts.len() {
        out.extend(s.chart_nodes(ci));
    }
    if out.is_empty() {
        return Err("等式の曲線・曲面が見つかりません（範囲が空か、等式の両辺の差が符号を変えません）".into());
    }
    Ok(out)
}

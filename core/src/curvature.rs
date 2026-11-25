//! 曲面の曲率（計画 N2）: 頂点ごとのガウス曲率 $K$・平均曲率 $H$ を求めて、色分けに使う。
//!
//! 導関数は、式（[`Program`]）を頂点のまわりの点で計算した中心差分で求める（刻みは表示範囲の $10^{-5}$ 倍。
//! 64 ビットで計算するので、丸めの誤差と刻みの誤差はどちらも $10^{-8}$ 程度）。記号微分にしないのは、
//! 陰関数の式は命令列（`Program`）しか残っておらず、段差・場合分けの関数も同じ道で扱えるため。
//!
//! 平均曲率の符号の決まり（3 つの道でそろえる）:
//! - 陰関数 $F = 0$: $H = \tfrac12 \nabla \cdot (\nabla F / \lVert\nabla F\rVert)$。球 $x^2+y^2+z^2 = R^2$（中が $F < 0$）で $H = 1/R$
//! - 陽関数 $w = f(u, v)$: $F = f - w$ と同じ。$z = x^2 + y^2$（下に凸）で $H > 0$
//! - 媒介変数の曲面 $\mathbf r(u, v)$: 法線 $\mathbf n = \mathbf r_u \times \mathbf r_v$ の向きから見て、$\mathbf r_{uu} \cdot \mathbf n$ が正なら $H > 0$
//!   （$\mathbf r = (u, v, f(u, v))$ なら陽関数と同じ）
//!
//! ガウス曲率は向きによらない。

use crate::eval::{Program, ProgramSet, as_real};
use num_complex::Complex64 as C;

/// 色分けに使う曲率
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    /// ガウス曲率 $K = \kappa_1 \kappa_2$
    Gauss,
    /// 平均曲率 $H = (\kappa_1 + \kappa_2) / 2$
    Mean,
}

impl Which {
    /// `plot` の `flags` のビット 6・7 から（どちらも立っていなければ None。両方ならガウス曲率）
    pub fn from_flags(flags: u32) -> Option<Which> {
        if flags & 64 != 0 {
            Some(Which::Gauss)
        } else if flags & 128 != 0 {
            Some(Which::Mean)
        } else {
            None
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Which::Gauss => "ガウス曲率 K",
            Which::Mean => "平均曲率 H",
        }
    }
}

fn eval_real(prog: &Program, cols: &[Vec<f64>]) -> Vec<f64> {
    let n = cols.first().map_or(0, Vec::len);
    let mut out = vec![C::new(0.0, 0.0); n];
    let inputs: Vec<&[f64]> = cols.iter().map(Vec::as_slice).collect();
    prog.eval_batch(&inputs, &mut out);
    out.into_iter().map(as_real).collect()
}

/// 3 変数の中心差分の点のずらし方（中心・各軸の ±・2 軸の組の ±±）。19 点
const STENCIL3: [[i8; 3]; 19] = [
    [0, 0, 0],
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
    [1, 1, 0],
    [1, -1, 0],
    [-1, 1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [1, 0, -1],
    [-1, 0, 1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, 1, -1],
    [0, -1, 1],
    [0, -1, -1],
];

/// 2 変数の中心差分の点のずらし方。9 点
const STENCIL2: [[i8; 2]; 9] = [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1], [1, 1], [1, -1], [-1, 1], [-1, -1]];

/// 点ごとに、ずらし方 `stencil` の点を並べた列（変数ごと）
fn stencil_cols<const D: usize>(points: &[[f64; D]], stencil: &[[i8; D]], step: [f64; D]) -> Vec<Vec<f64>> {
    let mut cols = vec![Vec::with_capacity(points.len() * stencil.len()); D];
    for p in points {
        for s in stencil {
            for (a, col) in cols.iter_mut().enumerate() {
                col.push(p[a] + s[a] as f64 * step[a]);
            }
        }
    }
    cols
}

/// 19 点の値から、勾配とヘッセ行列（中心差分）
fn derivatives3(f: &[f64], d: f64) -> ([f64; 3], [[f64; 3]; 3]) {
    let g = [(f[1] - f[2]) / (2.0 * d), (f[3] - f[4]) / (2.0 * d), (f[5] - f[6]) / (2.0 * d)];
    let dd = d * d;
    let mut h = [[0.0; 3]; 3];
    for a in 0..3 {
        h[a][a] = (f[1 + 2 * a] - 2.0 * f[0] + f[2 + 2 * a]) / dd;
    }
    // 組 (0, 1)・(0, 2)・(1, 2) は 7・11・15 番から 4 点ずつ（++, +−, −+, −−）
    for (k, (a, b)) in [(0, 1), (0, 2), (1, 2)].into_iter().enumerate() {
        let i = 7 + 4 * k;
        let v = (f[i] - f[i + 1] - f[i + 2] + f[i + 3]) / (4.0 * dd);
        h[a][b] = v;
        h[b][a] = v;
    }
    (g, h)
}

/// 陰関数 $F = 0$ の曲率（勾配 g とヘッセ行列 h から）
fn implicit_from(g: [f64; 3], h: [[f64; 3]; 3], which: Which) -> f64 {
    let g2 = g[0] * g[0] + g[1] * g[1] + g[2] * g[2];
    if !(g2.is_finite() && g2 > 0.0) {
        return f64::NAN;
    }
    match which {
        Which::Gauss => {
            // K = gᵀ adj(H) g / |g|⁴（adj は余因子行列の転置。H は対称）
            let [[a, b, c], [_, d, e], [_, _, f]] = h;
            let adj = [[d * f - e * e, c * e - b * f, b * e - c * d], [c * e - b * f, a * f - c * c, b * c - a * e], [b * e - c * d, b * c - a * e, a * d - b * b]];
            let mut q = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    q += g[i] * adj[i][j] * g[j];
                }
            }
            q / (g2 * g2)
        }
        Which::Mean => {
            // H = (|g|² tr H − gᵀ H g) / (2 |g|³) = ∇·(∇F/|∇F|) / 2
            let tr = h[0][0] + h[1][1] + h[2][2];
            let mut q = 0.0;
            for i in 0..3 {
                for j in 0..3 {
                    q += g[i] * h[i][j] * g[j];
                }
            }
            (g2 * tr - q) / (2.0 * g2 * g2.sqrt())
        }
    }
}

/// 陰関数 $F(x, y, z) = 0$ の曲面の点ごとの曲率。点は式の変数の順の座標。`scale` は表示範囲の大きさ（差分の刻みと、点を面の上へ寄せる量の上限に使う）。
/// 点はメッシュの頂点（格子の補間で、面から少し離れている）なので、ニュートン法で面の上へ寄せてから求める（寄せきれなければ、その点のまま）
pub fn implicit(prog: &Program, points: &[[f64; 3]], scale: f64, which: Which) -> Vec<f64> {
    if prog.vars().len() != 3 || points.is_empty() {
        return vec![f64::NAN; points.len()];
    }
    let d = 1e-5 * scale;
    let limit = 1e-2 * scale;
    let mut cur = points.to_vec();
    // ニュートン法（中心と各軸の ± の 7 点）。1 回で、面からの距離 δ は δ² κ の程度になる。格子の補間の頂点（δ はマスの 10⁻³ 倍ほど）は
    // ほとんど 1 回で足りるので、動いた量が表示範囲の 10⁻⁵ 倍を超えた点だけ、3 回まで続ける
    let mut active: Vec<usize> = (0..cur.len()).collect();
    for _ in 0..3 {
        if active.is_empty() {
            break;
        }
        let pts: Vec<[f64; 3]> = active.iter().map(|&k| cur[k]).collect();
        let cols = stencil_cols(&pts, &STENCIL3[..7], [d; 3]);
        let f = eval_real(prog, &cols);
        let mut next = Vec::new();
        for (j, &k) in active.iter().enumerate() {
            let v = &f[7 * j..7 * j + 7];
            let g = [(v[1] - v[2]) / (2.0 * d), (v[3] - v[4]) / (2.0 * d), (v[5] - v[6]) / (2.0 * d)];
            let g2 = g[0] * g[0] + g[1] * g[1] + g[2] * g[2];
            if !(v[0].is_finite() && g2.is_finite() && g2 > 0.0) {
                continue;
            }
            let t = v[0] / g2;
            let step = [t * g[0], t * g[1], t * g[2]];
            let len = (step[0] * step[0] + step[1] * step[1] + step[2] * step[2]).sqrt();
            if len <= limit {
                for a in 0..3 {
                    cur[k][a] -= step[a];
                }
                if len > 1e-5 * scale {
                    next.push(k);
                }
            }
        }
        active = next;
    }
    let cols = stencil_cols(&cur, &STENCIL3, [d; 3]);
    let f = eval_real(prog, &cols);
    f.chunks_exact(19)
        .map(|v| {
            let (g, h) = derivatives3(v, d);
            implicit_from(g, h, which)
        })
        .collect()
}

/// 陽関数 $w = f(u, v)$ の曲面の点ごとの曲率。`uv` は点ごとの (u, v)（式の変数の順）。`scale` は表示範囲の大きさ
pub fn graph(prog: &Program, uv: &[[f64; 2]], scale: f64, which: Which) -> Vec<f64> {
    if prog.vars().len() != 2 || uv.is_empty() {
        return vec![f64::NAN; uv.len()];
    }
    let d = 1e-5 * scale;
    let cols = stencil_cols(uv, &STENCIL2, [d; 2]);
    let f = eval_real(prog, &cols);
    f.chunks_exact(9)
        .map(|v| {
            let (fu, fv) = ((v[1] - v[2]) / (2.0 * d), (v[3] - v[4]) / (2.0 * d));
            let dd = d * d;
            let fuu = (v[1] - 2.0 * v[0] + v[2]) / dd;
            let fvv = (v[3] - 2.0 * v[0] + v[4]) / dd;
            let fuv = (v[5] - v[6] - v[7] + v[8]) / (4.0 * dd);
            let w = 1.0 + fu * fu + fv * fv;
            match which {
                Which::Gauss => (fuu * fvv - fuv * fuv) / (w * w),
                Which::Mean => ((1.0 + fv * fv) * fuu - 2.0 * fu * fv * fuv + (1.0 + fu * fu) * fvv) / (2.0 * w * w.sqrt()),
            }
        })
        .collect()
}

/// 媒介変数の曲面 $\mathbf r(u, v)$（3 成分）の点ごとの曲率。`uv` は点ごとの (u, v)、`span` は u・v の範囲の長さ（差分の刻みに使う）
pub fn parametric(set: &ProgramSet, uv: &[[f64; 2]], span: [f64; 2], which: Which) -> Vec<f64> {
    if set.len() != 3 || uv.is_empty() {
        return vec![f64::NAN; uv.len()];
    }
    let step = [1e-5 * span[0].abs().max(1e-12), 1e-5 * span[1].abs().max(1e-12)];
    let cols = stencil_cols(uv, &STENCIL2, step);
    let inputs: Vec<&[f64]> = cols.iter().map(Vec::as_slice).collect();
    let mut r = Vec::new();
    set.eval_points(&inputs, cols[0].len(), &mut r);
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    r.chunks_exact(27)
        .map(|v| {
            let p = |s: usize| [v[3 * s], v[3 * s + 1], v[3 * s + 2]];
            let [du, dv] = step;
            let ru = [0, 1, 2].map(|a| (p(1)[a] - p(2)[a]) / (2.0 * du));
            let rv = [0, 1, 2].map(|a| (p(3)[a] - p(4)[a]) / (2.0 * dv));
            let ruu = [0, 1, 2].map(|a| (p(1)[a] - 2.0 * p(0)[a] + p(2)[a]) / (du * du));
            let rvv = [0, 1, 2].map(|a| (p(3)[a] - 2.0 * p(0)[a] + p(4)[a]) / (dv * dv));
            let ruv = [0, 1, 2].map(|a| (p(5)[a] - p(6)[a] - p(7)[a] + p(8)[a]) / (4.0 * du * dv));
            let n = [ru[1] * rv[2] - ru[2] * rv[1], ru[2] * rv[0] - ru[0] * rv[2], ru[0] * rv[1] - ru[1] * rv[0]];
            let len = dot(n, n).sqrt();
            if !(len.is_finite() && len > 0.0) {
                return f64::NAN;
            }
            let n = n.map(|c| c / len);
            let (e, f, g) = (dot(ru, ru), dot(ru, rv), dot(rv, rv));
            let (l, m, nn) = (dot(ruu, n), dot(ruv, n), dot(rvv, n));
            let det = e * g - f * f;
            match which {
                Which::Gauss => (l * nn - m * m) / det,
                Which::Mean => (e * nn - 2.0 * f * m + g * l) / (2.0 * det),
            }
        })
        .collect()
}

/// 色の範囲 [−q, q] の q: |値| の 95 % の点が入る値（極・特異点の近くの大きな値に引っぱられないように）。
/// 有限の値が無いか、全部 0 なら 1
pub fn color_range(values: &[f64]) -> f64 {
    let mut mags: Vec<f64> = values.iter().filter(|v| v.is_finite()).map(|v| v.abs()).collect();
    if mags.is_empty() {
        return 1.0;
    }
    let i = (mags.len() * 95 / 100).min(mags.len() - 1);
    let (_, q, _) = mags.select_nth_unstable_by(i, |a, b| a.total_cmp(b));
    if *q > 0.0 { *q } else { 1.0 }
}

/// 頂点ごとの値を、メッシュの `complex`（値は 1 つ目。色分けの道）の形にする。求められなかった点は 0（中央の色）
pub fn as_vertex_values(values: &[f64]) -> Vec<f32> {
    values.iter().flat_map(|&v| [if v.is_finite() { v as f32 } else { 0.0 }, 0.0, 0.0]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Params;

    fn prog(src: &str, vars: &[&str]) -> Program {
        let e = crate::parser::parse(src).unwrap();
        Program::compile_with(&e, vars, &Params::new()).unwrap()
    }

    #[test]
    fn sphere_implicit() {
        let p = prog("x^2+y^2+z^2-4", &["x", "y", "z"]);
        // 面から少し離れた点（ニュートン法で寄せる）
        let pts = [[2.01, 0.0, 0.0], [0.0, 1.0, 1.7], [1.0, 1.0, 1.4]];
        for (which, want) in [(Which::Gauss, 0.25), (Which::Mean, 0.5)] {
            for v in implicit(&p, &pts, 20.0, which) {
                assert!((v - want).abs() < 1e-6, "{which:?}: {v}");
            }
        }
    }

    #[test]
    fn paraboloid_graph_and_parametric_agree() {
        let f = prog("x^2+y^2", &["x", "y"]);
        let uv = [[0.0, 0.0], [0.5, -0.3]];
        let set = ProgramSet::new(&[prog("u", &["u", "v"]), prog("v", &["u", "v"]), prog("u^2+v^2", &["u", "v"])]);
        let imp = prog("x^2+y^2-z", &["x", "y", "z"]);
        let pts: Vec<[f64; 3]> = uv.iter().map(|&[u, v]| [u, v, u * u + v * v]).collect();
        for which in [Which::Gauss, Which::Mean] {
            let a = graph(&f, &uv, 10.0, which);
            let b = parametric(&set, &uv, [2.0, 2.0], which);
            let c = implicit(&imp, &pts, 10.0, which);
            for k in 0..uv.len() {
                assert!((a[k] - b[k]).abs() < 1e-6 && (a[k] - c[k]).abs() < 1e-6, "{which:?}: {} {} {}", a[k], b[k], c[k]);
            }
        }
        // 原点: K = 4, H = 2（下に凸で正）
        assert!((graph(&f, &uv[..1], 10.0, Which::Gauss)[0] - 4.0).abs() < 1e-6);
        assert!((graph(&f, &uv[..1], 10.0, Which::Mean)[0] - 2.0).abs() < 1e-6);
    }
}

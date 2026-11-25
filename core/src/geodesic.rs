//! 曲面の上の測地線（計画 N3。`docs/plan-n3-geodesics.md`）。始点と、接平面の中の向き θ・弧長 L を決めて、初期値問題として解く。
//!
//! - 媒介変数の曲面 $\mathbf r(u, v)$（陽関数 $w = f(u, v)$ は $\mathbf r = (u, v, f)$）: 状態 $(u, v, \dot u, \dot v)$。
//!   加速度の接する成分が 0 になる条件
//!   $$\begin{pmatrix} E & F \\ F & G \end{pmatrix}\begin{pmatrix}\ddot u \\ \ddot v\end{pmatrix}
//!   = -\begin{pmatrix}\mathbf r_u \cdot \mathbf a \\ \mathbf r_v \cdot \mathbf a\end{pmatrix},\qquad
//!   \mathbf a = \mathbf r_{uu}\dot u^2 + 2\mathbf r_{uv}\dot u\dot v + \mathbf r_{vv}\dot v^2$$
//!   を解く（$\ddot u^k + \Gamma^k_{ij}\dot u^i\dot u^j = 0$ と同じ。クリストッフェル記号を作らない）
//! - 陰関数 $F = 0$: 状態 $(\mathbf x, \dot{\mathbf x})$ で
//!   $\ddot{\mathbf x} = -\dfrac{\dot{\mathbf x}^{\top}\mathrm H_F\,\dot{\mathbf x}}{\lVert\nabla F\rVert^2}\nabla F$。出力の点は面の上へ寄せ直す
//!
//! 導関数は [`crate::curvature`] と同じ中心差分。速さ 1 で始めるので、時刻がそのまま弧長になる。
//! 解法は [`crate::ode`]（ドルマン・プリンス法。同じ入力なら同じ出力）

use crate::curvature::{STENCIL2, STENCIL3, derivatives3, eval_real, stencil_cols};
use crate::eval::Program;
use crate::mesh::Domain;
use crate::ode;
use std::cell::Cell;

pub type V3 = [f64; 3];

/// 曲面 $\mathbf r(u, v)$ を、点の並びでまとめて計算するもの（定義されない点は NaN）
pub type SurfaceFn<'a> = dyn Fn(&[[f64; 2]]) -> Vec<V3> + 'a;

/// 測地線が止まった理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// 弧長 L まで進んだ
    Length,
    /// 媒介変数の範囲・表示範囲の端に来た
    Edge,
    /// 座標の特異点（$\mathbf r_u \times \mathbf r_v = \mathbf 0$・$\nabla F = \mathbf 0$）に来た
    Singular,
    /// 曲面が定義されない所に来た（始点で定義されないときも）
    Undefined,
    /// 陰関数の始点の近くに面が無い
    NoSurface,
    /// 歩数の上限
    Steps,
}

/// 測地線 1 本: 曲面の上の点と、その点の単位法線
#[derive(Clone, Debug)]
pub struct Path {
    pub points: Vec<V3>,
    pub normals: Vec<V3>,
    pub stop: Stop,
}

impl Path {
    fn empty(stop: Stop) -> Path {
        Path { points: Vec::new(), normals: Vec::new(), stop }
    }
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: V3) -> Option<V3> {
    let l = dot(a, a).sqrt();
    (l.is_finite() && l > 0.0).then(|| a.map(|c| c / l))
}

/// 1 歩の歩数の上限（1 本）
const MAX_STEPS: usize = 20_000;

/// 点 (u, v) での $\mathbf r$ と、1 階・2 階の偏導関数
struct Jet {
    r: V3,
    ru: V3,
    rv: V3,
    ruu: V3,
    ruv: V3,
    rvv: V3,
}

fn jets(r: &SurfaceFn, uv: &[[f64; 2]], step: [f64; 2]) -> Vec<Option<Jet>> {
    let pts: Vec<[f64; 2]> = uv
        .iter()
        .flat_map(|p| STENCIL2.iter().map(move |s| [p[0] + s[0] as f64 * step[0], p[1] + s[1] as f64 * step[1]]))
        .collect();
    let v = r(&pts);
    let [du, dv] = step;
    v.chunks_exact(9)
        .map(|q| {
            if !q.iter().all(|p| p.iter().all(|c| c.is_finite())) {
                return None;
            }
            Some(Jet {
                r: q[0],
                ru: [0, 1, 2].map(|a| (q[1][a] - q[2][a]) / (2.0 * du)),
                rv: [0, 1, 2].map(|a| (q[3][a] - q[4][a]) / (2.0 * dv)),
                ruu: [0, 1, 2].map(|a| (q[1][a] - 2.0 * q[0][a] + q[2][a]) / (du * du)),
                rvv: [0, 1, 2].map(|a| (q[3][a] - 2.0 * q[0][a] + q[4][a]) / (dv * dv)),
                ruv: [0, 1, 2].map(|a| (q[5][a] - q[6][a] - q[7][a] + q[8][a]) / (4.0 * du * dv)),
            })
        })
        .collect()
}

/// 媒介変数の範囲と、両端で曲面の点が一致する（つないで続けてよい）向き
#[derive(Clone, Copy, Debug)]
pub struct ParamDomain {
    pub range: [(f64, f64); 2],
    pub periodic: [bool; 2],
}

/// 範囲の両端で曲面の点が一致する向きを、数値で確かめる（もう片方の媒介変数の 7 か所で、距離が `tol` 以下）
pub fn periodic_dirs(r: &SurfaceFn, range: [(f64, f64); 2], tol: f64) -> [bool; 2] {
    [0, 1].map(|k| {
        let o = 1 - k;
        let (lo, hi) = range[k];
        if !(hi - lo).is_finite() || hi == lo {
            return false;
        }
        let mut pts = Vec::new();
        for j in 0..7 {
            let t = range[o].0 + (range[o].1 - range[o].0) * (j as f64 + 0.5) / 7.0;
            for x in [lo, hi] {
                let mut p = [0.0; 2];
                p[k] = x;
                p[o] = t;
                pts.push(p);
            }
        }
        let v = r(&pts);
        v.chunks_exact(2).all(|q| q.iter().all(|p| p.iter().all(|c| c.is_finite())) && dot(sub(q[0], q[1]), sub(q[0], q[1])).sqrt() <= tol)
    })
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// 媒介変数の曲面の測地線。`start` は (u₀, v₀)、θ は $\mathbf r_u$ の向きから法線 $\mathbf r_u \times \mathbf r_v$ のまわりに反時計回りの角度、
/// `length` は弧長。`chord` は出力の点の間隔の目安（3D の長さ）
pub fn on_parametric(r: &SurfaceFn, start: [f64; 2], theta: f64, length: f64, dom: &ParamDomain, chord: f64) -> Path {
    let span = [dom.range[0].1 - dom.range[0].0, dom.range[1].1 - dom.range[1].0];
    let step = span.map(|s| 1e-5 * s.abs().max(1e-9));
    let Some(Some(j)) = jets(r, &[start], step).pop() else { return Path::empty(Stop::Undefined) };
    let Some(n) = unit(cross(j.ru, j.rv)) else { return Path::empty(Stop::Singular) };
    let Some(e1) = unit(j.ru) else { return Path::empty(Stop::Singular) };
    let e2 = cross(n, e1);
    let t = [0, 1, 2].map(|a| theta.cos() * e1[a] + theta.sin() * e2[a]);
    let (e, f, g) = (dot(j.ru, j.ru), dot(j.ru, j.rv), dot(j.rv, j.rv));
    let det = e * g - f * f;
    let det0 = det;
    let (bu, bv) = (dot(j.ru, t), dot(j.rv, t));
    let y0 = [start[0], start[1], (g * bu - f * bv) / det, (e * bv - f * bu) / det];
    let last = Cell::new(Stop::Length);
    let outside = |y: &[f64], margin: f64| {
        (0..2).any(|k| {
            let (lo, hi) = (dom.range[k].0.min(dom.range[k].1), dom.range[k].0.max(dom.range[k].1));
            !dom.periodic[k] && (y[k] < lo - margin * span[k].abs() || y[k] > hi + margin * span[k].abs())
        })
    };
    let mut rhs = |_t: f64, y: &[f64], dy: &mut [f64]| {
        // 前の段が NaN だと、ここにも NaN の状態が来る（止まった理由は、最初に NaN を返した所のものを残す）
        if !y.iter().all(|v| v.is_finite()) {
            dy.fill(f64::NAN);
            return;
        }
        if outside(y, 0.02) {
            last.set(Stop::Edge);
            dy.fill(f64::NAN);
            return;
        }
        let Some(Some(j)) = jets(r, &[[y[0], y[1]]], step).pop() else {
            last.set(Stop::Undefined);
            dy.fill(f64::NAN);
            return;
        };
        let (e, f, g) = (dot(j.ru, j.ru), dot(j.ru, j.rv), dot(j.rv, j.rv));
        let det = e * g - f * f;
        let (du, dv) = (y[2], y[3]);
        // 座標の特異点: 面積の要素 √(EG − F²) が始点の 10⁻³ 倍より小さい（球の極など）。
        // 測地線は速さを保つので、速さが 1 から 10⁻³ 以上ずれたら、数値で追えていない（特異点の近く）とみなす
        let speed2 = e * du * du + 2.0 * f * du * dv + g * dv * dv;
        if !(det > 1e-6 * det0 && (speed2 - 1.0).abs() < 2e-3) {
            last.set(Stop::Singular);
            dy.fill(f64::NAN);
            return;
        }
        let a = [0, 1, 2].map(|k| j.ruu[k] * du * du + 2.0 * j.ruv[k] * du * dv + j.rvv[k] * dv * dv);
        let (b0, b1) = (-dot(j.ru, a), -dot(j.rv, a));
        dy[0] = du;
        dy[1] = dv;
        dy[2] = (g * b0 - f * b1) / det;
        dy[3] = (e * b1 - f * b0) / det;
    };
    // 出力の点の間隔: 媒介変数の空間での弦。3D の間隔 chord を、始点での |r_u|・|r_v| で媒介変数の長さに直した目安
    let scale = e.sqrt().max(g.sqrt()).max(1e-12);
    let opt = ode::Options { limit: f64::MAX, chord: chord / scale, max_arc: f64::INFINITY, arc_dims: 2, max_steps: MAX_STEPS };
    let sol = ode::solve(&mut rhs, 0.0, &y0, length, opt);
    let reached = sol.points.last().is_some_and(|(t, _)| *t >= length * (1.0 - 1e-9));
    let mut stop = if reached {
        Stop::Length
    } else if sol.truncated {
        Stop::Steps
    } else {
        last.get()
    };
    // 範囲の端で切る（端をまたいだ歩は、端までの所で止める）
    let mut uv: Vec<[f64; 2]> = Vec::with_capacity(sol.points.len());
    for (_, y) in &sol.points {
        if outside(y, 0.0) {
            if let Some(&p) = uv.last() {
                let q = [y[0], y[1]];
                let mut s = 1.0f64;
                for k in 0..2 {
                    if dom.periodic[k] {
                        continue;
                    }
                    let (lo, hi) = (dom.range[k].0.min(dom.range[k].1), dom.range[k].0.max(dom.range[k].1));
                    let edge = if q[k] > hi { hi } else if q[k] < lo { lo } else { continue };
                    if (q[k] - p[k]).abs() > 0.0 {
                        s = s.min((edge - p[k]) / (q[k] - p[k]));
                    }
                }
                let s = s.clamp(0.0, 1.0);
                uv.push([p[0] + s * (q[0] - p[0]), p[1] + s * (q[1] - p[1])]);
            }
            stop = Stop::Edge;
            break;
        }
        uv.push([y[0], y[1]]);
    }
    let js = jets(r, &uv, step);
    let mut path = Path { points: Vec::with_capacity(uv.len()), normals: Vec::with_capacity(uv.len()), stop };
    for j in js {
        let Some(j) = j else { break };
        let Some(n) = unit(cross(j.ru, j.rv)) else { break };
        path.points.push(j.r);
        path.normals.push(n);
    }
    path
}

/// 陰関数 $F = 0$ の値・勾配（7 点）
fn grads(prog: &Program, pts: &[V3], d: f64) -> Vec<(f64, V3)> {
    let cols = stencil_cols(pts, &STENCIL3[..7], [d; 3]);
    let f = eval_real(prog, &cols);
    f.chunks_exact(7).map(|v| (v[0], [(v[1] - v[2]) / (2.0 * d), (v[3] - v[4]) / (2.0 * d), (v[5] - v[6]) / (2.0 * d)])).collect()
}

/// 点を面 F = 0 の上へ寄せる（ニュートン法を `iters` 回まで）。寄せきれなければ None。`d` は差分の刻み、`limit` は 1 回で動かす量の上限
fn onto(prog: &Program, p: V3, d: f64, limit: f64, iters: usize) -> Option<(V3, V3)> {
    let mut p = p;
    for _ in 0..iters {
        let (v, g) = grads(prog, &[p], d)[0];
        let g2 = dot(g, g);
        if !(v.is_finite() && g2.is_finite() && g2 > 0.0) {
            return None;
        }
        let t = v / g2;
        let step = g.map(|c| t * c);
        let len = dot(step, step).sqrt();
        if len > limit {
            return None;
        }
        p = sub(p, step);
        if len <= 1e-12 * limit.max(1.0) {
            break;
        }
    }
    let (v, g) = grads(prog, &[p], d)[0];
    let gl = dot(g, g).sqrt();
    (v.is_finite() && gl > 0.0 && (v / gl).abs() <= 1e-7 * limit.max(1e-300)).then(|| (p, g.map(|c| c / gl)))
}

/// 陰関数の曲面の測地線。`start` は面の近くの点（面の上へ寄せる）。θ は x 軸（x 軸が法線に近ければ y 軸）を接平面に射影した向きから、
/// 法線 ∇F のまわりに反時計回りの角度。`dom` は表示範囲（出たら止める）、`scale` は表示範囲の大きさ
pub fn on_implicit(prog: &Program, start: V3, theta: f64, length: f64, dom: Domain, scale: f64, chord: f64) -> Path {
    if prog.vars().len() != 3 {
        return Path::empty(Stop::Undefined);
    }
    let d = 1e-5 * scale;
    let Some((p0, n)) = onto(prog, start, d, 0.25 * scale, 40) else { return Path::empty(Stop::NoSurface) };
    let proj = |a: V3| sub(a, n.map(|c| c * dot(a, n)));
    let e1 = match unit(proj([1.0, 0.0, 0.0])) {
        Some(e) if dot(proj([1.0, 0.0, 0.0]), proj([1.0, 0.0, 0.0])) > 0.01 => e,
        _ => match unit(proj([0.0, 1.0, 0.0])) {
            Some(e) => e,
            None => return Path::empty(Stop::Singular),
        },
    };
    let e2 = cross(n, e1);
    let v0 = [0, 1, 2].map(|a| theta.cos() * e1[a] + theta.sin() * e2[a]);
    let y0 = [p0[0], p0[1], p0[2], v0[0], v0[1], v0[2]];
    let last = Cell::new(Stop::Length);
    let outside = |y: &[f64], margin: f64| {
        (0..3).any(|a| {
            let m = margin * (dom.hi[a] - dom.lo[a]);
            y[a] < dom.lo[a] - m || y[a] > dom.hi[a] + m
        })
    };
    let mut rhs = |_t: f64, y: &[f64], dy: &mut [f64]| {
        // 前の段が NaN だと、ここにも NaN の状態が来る（止まった理由は、最初に NaN を返した所のものを残す）
        if !y.iter().all(|v| v.is_finite()) {
            dy.fill(f64::NAN);
            return;
        }
        if outside(y, 0.02) {
            last.set(Stop::Edge);
            dy.fill(f64::NAN);
            return;
        }
        let cols = stencil_cols(&[[y[0], y[1], y[2]]], &STENCIL3, [d; 3]);
        let f = eval_real(prog, &cols);
        if !f.iter().all(|v| v.is_finite()) {
            last.set(Stop::Undefined);
            dy.fill(f64::NAN);
            return;
        }
        let (g, h) = derivatives3(&f, d);
        let g2 = dot(g, g);
        if !(g2 > 0.0) {
            last.set(Stop::Singular);
            dy.fill(f64::NAN);
            return;
        }
        let v = [y[3], y[4], y[5]];
        let mut q = 0.0;
        for i in 0..3 {
            for k in 0..3 {
                q += v[i] * h[i][k] * v[k];
            }
        }
        let s = -q / g2;
        dy[..3].copy_from_slice(&v);
        for a in 0..3 {
            dy[3 + a] = s * g[a];
        }
    };
    let opt = ode::Options { limit: f64::MAX, chord, max_arc: f64::INFINITY, arc_dims: 3, max_steps: MAX_STEPS };
    let sol = ode::solve(&mut rhs, 0.0, &y0, length, opt);
    let reached = sol.points.last().is_some_and(|(t, _)| *t >= length * (1.0 - 1e-9));
    let mut stop = if reached {
        Stop::Length
    } else if sol.truncated {
        Stop::Steps
    } else {
        last.get()
    };
    let mut pts: Vec<V3> = Vec::with_capacity(sol.points.len());
    for (_, y) in &sol.points {
        if outside(y, 0.0) {
            stop = Stop::Edge;
            break;
        }
        pts.push([y[0], y[1], y[2]]);
    }
    // 出力の点を面の上へ寄せ直す（ニュートン法 1 回。積み重なったずれを見た目に残さない）と、法線
    let gs = grads(prog, &pts, d);
    let mut path = Path { points: Vec::with_capacity(pts.len()), normals: Vec::with_capacity(pts.len()), stop };
    for (p, (v, g)) in pts.into_iter().zip(gs) {
        let g2 = dot(g, g);
        if !(g2 > 0.0 && v.is_finite()) {
            break;
        }
        let t = v / g2;
        path.points.push(sub(p, g.map(|c| t * c)));
        path.normals.push(g.map(|c| c / g2.sqrt()));
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Params, ProgramSet};

    fn prog(src: &str, vars: &[&str]) -> Program {
        let e = crate::parser::parse(src).unwrap();
        Program::compile_with(&e, vars, &Params::new()).unwrap()
    }

    fn surface(set: &ProgramSet) -> impl Fn(&[[f64; 2]]) -> Vec<V3> + '_ {
        move |uv: &[[f64; 2]]| {
            let us: Vec<f64> = uv.iter().map(|p| p[0]).collect();
            let vs: Vec<f64> = uv.iter().map(|p| p[1]).collect();
            let mut out = Vec::new();
            set.eval_points(&[&us, &vs], uv.len(), &mut out);
            out.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect()
        }
    }

    /// 球の大円: 全部の点が、始点の位置と向きで決まる平面の上（原点を通る）にあり、半径 2 の上
    #[test]
    fn sphere_great_circle_parametric() {
        let set = ProgramSet::new(&[prog("2sin(u)cos(v)", &["u", "v"]), prog("2sin(u)sin(v)", &["u", "v"]), prog("2cos(u)", &["u", "v"])]);
        let r = surface(&set);
        let range = [(0.0, std::f64::consts::PI), (0.0, std::f64::consts::TAU)];
        let periodic = periodic_dirs(&r, range, 1e-9);
        assert_eq!(periodic, [false, true]);
        let dom = ParamDomain { range, periodic };
        // 赤道の上の点から、45° の向きへ 1 周（2π·2）
        let len = 4.0 * std::f64::consts::PI;
        let p = on_parametric(&r, [std::f64::consts::FRAC_PI_2, 0.3], std::f64::consts::FRAC_PI_4, len, &dom, 0.02);
        assert_eq!(p.stop, Stop::Length, "{:?}", p.stop);
        let a = p.points[0];
        let b = p.points[p.points.len() / 8];
        let n = unit(cross(a, b)).unwrap();
        let worst = p.points.iter().map(|q| dot(*q, n).abs().max((dot(*q, *q).sqrt() - 2.0).abs())).fold(0.0, f64::max);
        assert!(worst < 1e-4, "{worst}");
        // 1 周で始点に戻る
        let end = *p.points.last().unwrap();
        assert!(dot(sub(end, a), sub(end, a)).sqrt() < 1e-4);
    }

    #[test]
    fn sphere_great_circle_implicit() {
        let f = prog("x^2+y^2+z^2-4", &["x", "y", "z"]);
        let dom = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        let len = 4.0 * std::f64::consts::PI;
        let p = on_implicit(&f, [2.1, 0.3, 0.2], 0.7, len, dom, 17.3, 0.02);
        assert_eq!(p.stop, Stop::Length);
        let n = unit(cross(p.points[0], p.points[p.points.len() / 8])).unwrap();
        let worst = p.points.iter().map(|q| dot(*q, n).abs().max((dot(*q, *q).sqrt() - 2.0).abs())).fold(0.0, f64::max);
        assert!(worst < 1e-4, "{worst}");
        let end = *p.points.last().unwrap();
        assert!(dot(sub(end, p.points[0]), sub(end, p.points[0])).sqrt() < 1e-4);
    }

    /// 円柱 (cos u, sin u, v) の測地線はらせん: z が弧長に比例して増え、半径 1 のまま
    #[test]
    fn cylinder_helix() {
        let set = ProgramSet::new(&[prog("cos(u)", &["u", "v"]), prog("sin(u)", &["u", "v"]), prog("v", &["u", "v"])]);
        let r = surface(&set);
        let range = [(0.0, std::f64::consts::TAU), (-10.0, 10.0)];
        let dom = ParamDomain { range, periodic: periodic_dirs(&r, range, 1e-9) };
        assert_eq!(dom.periodic, [true, false]);
        // 45°: dz/ds = sin 45°。u の範囲を何周もまたぐ（つないで続ける）
        let p = on_parametric(&r, [1.0, 0.0], std::f64::consts::FRAC_PI_4, 12.0, &dom, 0.02);
        assert_eq!(p.stop, Stop::Length);
        let end = *p.points.last().unwrap();
        assert!((end[2] - 12.0 * std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-4, "{end:?}");
        // さらに進むと v の端（z = 10）で止まる
        let p = on_parametric(&r, [1.0, 0.0], std::f64::consts::FRAC_PI_4, 30.0, &dom, 0.02);
        assert_eq!(p.stop, Stop::Edge);
        assert!((p.points.last().unwrap()[2] - 10.0).abs() < 1e-6);
        assert!(p.points.iter().all(|q| (q[0].hypot(q[1]) - 1.0).abs() < 1e-6));
    }

    /// 平面の陽関数は直線。範囲の端で止まる。同じ入力なら同じ出力
    #[test]
    fn plane_is_straight_and_stops_at_edge() {
        let f = prog("2u+3v", &["u", "v"]);
        let r = |uv: &[[f64; 2]]| -> Vec<V3> {
            let us: Vec<f64> = uv.iter().map(|p| p[0]).collect();
            let vs: Vec<f64> = uv.iter().map(|p| p[1]).collect();
            let z = eval_real(&f, &[us.clone(), vs.clone()]);
            (0..uv.len()).map(|k| [us[k], vs[k], z[k]]).collect()
        };
        let dom = ParamDomain { range: [(-5.0, 5.0), (-5.0, 5.0)], periodic: [false, false] };
        let p = on_parametric(&r, [0.0, 0.0], 0.3, 100.0, &dom, 0.05);
        assert_eq!(p.stop, Stop::Edge);
        let (a, b) = (p.points[0], *p.points.last().unwrap());
        let dir = unit(sub(b, a)).unwrap();
        let worst = p.points.iter().map(|q| {
            let w = sub(*q, a);
            let c = cross(w, dir);
            dot(c, c).sqrt()
        }).fold(0.0, f64::max);
        assert!(worst < 1e-6, "{worst}");
        // 端（u か v が ±5）で止まっている
        assert!((b[0].abs() - 5.0).abs() < 1e-6 || (b[1].abs() - 5.0).abs() < 1e-6, "{b:?}");
        let q = on_parametric(&r, [0.0, 0.0], 0.3, 100.0, &dom, 0.05);
        assert_eq!(p.points, q.points);
    }

    /// 球の極（座標の特異点）に向かうと、そこで止まる
    #[test]
    fn stops_at_pole() {
        let set = ProgramSet::new(&[prog("2sin(u)cos(v)", &["u", "v"]), prog("2sin(u)sin(v)", &["u", "v"]), prog("2cos(u)", &["u", "v"])]);
        let r = surface(&set);
        let range = [(0.0, std::f64::consts::PI), (0.0, std::f64::consts::TAU)];
        let dom = ParamDomain { range, periodic: periodic_dirs(&r, range, 1e-9) };
        // θ = π: −r_u の向き（u が減る = 北極へ）
        let p = on_parametric(&r, [1.0, 0.5], std::f64::consts::PI, 20.0, &dom, 0.02);
        let zs: Vec<f64> = p.points.iter().step_by(p.points.len() / 10 + 1).map(|q| q[2]).collect();
        assert!(matches!(p.stop, Stop::Singular | Stop::Edge), "{:?} {zs:?}", p.stop);
        let end = *p.points.last().unwrap();
        assert!(end[2] > 1.99, "{end:?}");
    }
}

//! 停留点（計画 N4 の前半）: 1〜3 変数の関数 $f$ の $\nabla f = \mathbf 0$ の点を表示範囲の中で探し、
//! ヘッセ行列 $\mathrm H_f$ で極大・極小・鞍点に分ける（`@crit f` だけの行。LaTeX は `\operatorname{crit} f`）。
//!
//! - 探し方: 表示範囲を格子に分けたマスの中心を種にして、$\nabla f = \mathbf 0$ をニュートン法で解く。
//!   $\mathrm H_f$ が特異でも進めるよう、刻みは $(\mathrm H^\top \mathrm H + \mu I)\,\mathbf d = \mathrm H^\top \nabla f$（レーベンバーグ・マルカート）で決める。
//!   偏導関数は記号微分（[`crate::diff`]）の式。全部の種を 1 回の命令列の実行でまとめて進める
//! - 種のマスより小さい所に固まった停留点は見落とすことがある（説明に書く）
//! - 分け方: 1 変数は $f''$ の符号。$f'' = 0$ なら、0 でない最初の高階導関数 $f^{(m)}$ の $m$ の偶奇と符号（8 階まで）。
//!   2・3 変数は主小行列式（シルベスターの判定法）。$\det \mathrm H = 0$ なら「判定できない」

use crate::eval::{as_real, Params, Program};
use crate::parser::Expr;
use num_complex::Complex64 as C;

/// 停留点の種類
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Min,
    Max,
    Saddle,
    /// 1 変数で、0 でない最初の高階導関数が奇数階（極値でない。変曲点）
    NotExtremum,
    /// ヘッセ行列が退化していて、2 次の項では判定できない
    Degenerate,
}

/// 見つけた停留点
#[derive(Debug, Clone)]
pub struct Critical {
    pub point: Vec<f64>,
    pub value: f64,
    pub class: Class,
    /// 判定の根拠（説明に出す。「det H = 12 > 0, ∂²f/∂x² = 6 > 0」など）
    pub reason: String,
}

/// 結果。`more` は、停留点が多すぎて途中で打ち切ったか（線・面に沿って続いている可能性）
pub struct Found {
    pub points: Vec<Critical>,
    pub more: bool,
}

const MAX_POINTS: usize = 100;

fn real(prog: &Program, xs: &[Vec<f64>]) -> Vec<f64> {
    let n = xs.first().map_or(0, |x| x.len());
    let inputs: Vec<&[f64]> = xs.iter().map(|x| x.as_slice()).collect();
    let mut out = vec![C::new(0.0, 0.0); n];
    prog.eval_batch(&inputs, &mut out);
    out.into_iter().map(as_real).collect()
}

/// k × k（k ≤ 3）の連立 1 次方程式 A x = b（部分ピボットの掃き出し）。特異なら None
fn solve(mut a: Vec<f64>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let k = b.len();
    for c in 0..k {
        let p = (c..k).max_by(|&i, &j| a[i * k + c].abs().total_cmp(&a[j * k + c].abs()))?;
        if a[p * k + c].abs() < 1e-300 || !a[p * k + c].is_finite() {
            return None;
        }
        for j in 0..k {
            a.swap(c * k + j, p * k + j);
        }
        b.swap(c, p);
        for i in c + 1..k {
            let m = a[i * k + c] / a[c * k + c];
            for j in c..k {
                a[i * k + j] -= m * a[c * k + j];
            }
            b[i] -= m * b[c];
        }
    }
    let mut x = vec![0.0; k];
    for i in (0..k).rev() {
        let s: f64 = (i + 1..k).map(|j| a[i * k + j] * x[j]).sum();
        x[i] = (b[i] - s) / a[i * k + i];
    }
    Some(x)
}

/// 主小行列式 D₁, …, D_k（k ≤ 3。対称行列 h は行優先）
fn minors(h: &[f64], k: usize) -> Vec<f64> {
    let at = |i: usize, j: usize| h[i * k + j];
    let mut d = vec![at(0, 0)];
    if k >= 2 {
        d.push(at(0, 0) * at(1, 1) - at(0, 1) * at(1, 0));
    }
    if k >= 3 {
        d.push(
            at(0, 0) * (at(1, 1) * at(2, 2) - at(1, 2) * at(2, 1)) - at(0, 1) * (at(1, 0) * at(2, 2) - at(1, 2) * at(2, 0))
                + at(0, 2) * (at(1, 0) * at(2, 1) - at(1, 1) * at(2, 0)),
        );
    }
    d
}

/// 説明に出す数（整数はそのまま、ほかは有効 6 桁。負号は −）
pub fn num_text(v: f64) -> String {
    let v = if v.abs() < 1e-12 { 0.0 } else { v };
    if (v - v.round()).abs() < 1e-9 * v.abs().max(1.0) && v.abs() < 1e15 {
        format!("{}", v.round() as i64).replace('-', "−")
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').replace('-', "−")
    }
}

fn sign_text(v: f64) -> &'static str {
    if v > 0.0 {
        "> 0"
    } else {
        "< 0"
    }
}

/// 1〜3 変数の f の、箱 `boxes` の中の停留点
pub fn find(f: &Expr, vars: &[String], boxes: &[(f64, f64)], params: &Params) -> Result<Found, String> {
    let k = vars.len();
    assert!((1..=3).contains(&k) && boxes.len() == k);
    let names: Vec<&str> = vars.iter().map(|s| s.as_str()).collect();
    let compile = |e: &Expr| Program::compile_with(e, &names, params);
    let fp = compile(f)?;
    let grads: Vec<Expr> = vars.iter().map(|v| crate::diff::diff(f, v)).collect::<Result<_, _>>()?;
    let mut hess_e = vec![Expr::Num(0.0); k * k];
    for i in 0..k {
        for j in i..k {
            let d = crate::diff::diff(&grads[i], &vars[j])?;
            hess_e[i * k + j] = d.clone();
            hess_e[j * k + i] = d;
        }
    }
    let gp: Vec<Program> = grads.iter().map(compile).collect::<Result<_, _>>()?;
    let hp: Vec<Program> = hess_e.iter().map(compile).collect::<Result<_, _>>()?;

    // 種: 各軸を m 等分したマスの中心
    let m = [600usize, 48, 16][k - 1];
    let total = m.pow(k as u32);
    let mut xs: Vec<Vec<f64>> = vec![Vec::with_capacity(total); k];
    for idx in 0..total {
        let mut r = idx;
        for (a, (lo, hi)) in boxes.iter().enumerate() {
            let c = r % m;
            r /= m;
            xs[a].push(lo + (hi - lo) * (c as f64 + 0.5) / m as f64);
        }
    }
    let size: Vec<f64> = boxes.iter().map(|(lo, hi)| (hi - lo).abs().max(1e-300)).collect();
    let diag = size.iter().map(|s| s * s).sum::<f64>().sqrt();
    // 勾配の大きさの目安（収束の判定に使う）
    let g0: Vec<Vec<f64>> = gp.iter().map(|p| real(p, &xs)).collect();
    let mut gscale = 0.0f64;
    for i in 0..total {
        let n = (0..k).map(|a| g0[a][i] * g0[a][i]).sum::<f64>().sqrt();
        if n.is_finite() {
            gscale = gscale.max(n);
        }
    }
    let gscale = gscale.max(1e-300);

    // ニュートン法（全部の種をまとめて）
    let mut alive: Vec<bool> = vec![true; total];
    for _ in 0..60 {
        let g: Vec<Vec<f64>> = gp.iter().map(|p| real(p, &xs)).collect();
        let h: Vec<Vec<f64>> = hp.iter().map(|p| real(p, &xs)).collect();
        let mut moved = false;
        for i in 0..total {
            if !alive[i] {
                continue;
            }
            let gi: Vec<f64> = (0..k).map(|a| g[a][i]).collect();
            let hi: Vec<f64> = (0..k * k).map(|a| h[a][i]).collect();
            if gi.iter().chain(&hi).any(|v| !v.is_finite()) {
                alive[i] = false;
                continue;
            }
            // (HᵀH + μI) d = Hᵀ g
            let hs = hi.iter().fold(0.0f64, |s, v| s.max(v.abs()));
            let mu = (1e-10 * hs * hs).max(1e-300);
            let mut a = vec![0.0; k * k];
            let mut b = vec![0.0; k];
            for r in 0..k {
                for c in 0..k {
                    a[r * k + c] = (0..k).map(|t| hi[t * k + r] * hi[t * k + c]).sum::<f64>() + if r == c { mu } else { 0.0 };
                }
                b[r] = (0..k).map(|t| hi[t * k + r] * gi[t]).sum();
            }
            let Some(mut d) = solve(a, b) else {
                alive[i] = false;
                continue;
            };
            // 1 回に動く量は、表示範囲の 1/4 まで
            let len = d.iter().zip(&size).map(|(v, s)| (v / s).powi(2)).sum::<f64>().sqrt();
            if len > 0.25 {
                d.iter_mut().for_each(|v| *v *= 0.25 / len);
            }
            for a in 0..k {
                xs[a][i] -= d[a];
            }
            if len > 1e-15 {
                moved = true;
            }
            // 範囲から大きく出たものは捨てる
            if (0..k).any(|a| xs[a][i] < boxes[a].0 - 0.2 * size[a] || xs[a][i] > boxes[a].1 + 0.2 * size[a]) {
                alive[i] = false;
            }
        }
        if !moved {
            break;
        }
    }

    // 収束したものを集めて、近いものをまとめる
    let g: Vec<Vec<f64>> = gp.iter().map(|p| real(p, &xs)).collect();
    let mut found: Vec<(Vec<f64>, f64)> = Vec::new();
    let mut more = false;
    for i in 0..total {
        if !alive[i] {
            continue;
        }
        let p: Vec<f64> = (0..k).map(|a| xs[a][i]).collect();
        if (0..k).any(|a| p[a] < boxes[a].0 - 1e-9 * size[a] || p[a] > boxes[a].1 + 1e-9 * size[a]) {
            continue;
        }
        let gn = (0..k).map(|a| g[a][i] * g[a][i]).sum::<f64>().sqrt();
        if !(gn <= 1e-9 * gscale.max(1.0)) {
            continue;
        }
        let near = found.iter_mut().find(|(q, _)| q.iter().zip(&p).map(|(a, b)| (a - b).powi(2)).sum::<f64>().sqrt() <= 1e-4 * diag);
        match near {
            Some(q) if gn < q.1 => *q = (p, gn),
            Some(_) => {}
            None => {
                if found.len() >= MAX_POINTS {
                    more = true;
                    break;
                }
                found.push((p, gn));
            }
        }
    }
    // 小さい誤差を丸める（0 の近く、整数の近く）
    for (p, _) in &mut found {
        for (a, v) in p.iter_mut().enumerate() {
            let tol = 1e-9 * size[a].max(1.0);
            if (*v - v.round()).abs() < tol {
                *v = v.round();
            }
        }
    }
    found.sort_by(|a, b| a.0.iter().zip(&b.0).map(|(x, y)| x.total_cmp(y)).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal));

    // 分ける
    let pts: Vec<Vec<f64>> = (0..k).map(|a| found.iter().map(|(p, _)| p[a]).collect()).collect();
    if found.is_empty() {
        return Ok(Found { points: Vec::new(), more });
    }
    let fv = real(&fp, &pts);
    let hv: Vec<Vec<f64>> = hp.iter().map(|p| real(p, &pts)).collect();
    // 1 変数の高階導関数（f'' = 0 のとき）
    let mut higher: Vec<Program> = Vec::new();
    if k == 1 {
        let mut d = hess_e[0].clone();
        for _ in 3..=8 {
            d = crate::diff::diff(&d, &vars[0])?;
            higher.push(compile(&d)?);
        }
    }
    let higher_v: Vec<Vec<f64>> = higher.iter().map(|p| real(p, &pts)).collect();
    let mut points = Vec::new();
    for (i, (p, _)) in found.iter().enumerate() {
        let h: Vec<f64> = (0..k * k).map(|a| hv[a][i]).collect();
        let hs = h.iter().fold(0.0f64, |s, v| s.max(v.abs()));
        // 0 とみなす大きさ（f の大きさと範囲で決める）
        let fscale = fv[i].abs().max(gscale * diag).max(1e-300);
        let zero2 = 1e-7 * fscale / (diag * diag);
        let (class, reason) = if k == 1 {
            if hs > zero2 {
                let c = if h[0] > 0.0 { Class::Min } else { Class::Max };
                (c, format!("f″ = {} {}", num_text(h[0]), sign_text(h[0])))
            } else {
                // 0 でない最初の高階導関数
                let mut res = (Class::Degenerate, "f″ = 0 で、8 階までの導関数も 0".to_string());
                let primes = ["f‴", "f⁽⁴⁾", "f⁽⁵⁾", "f⁽⁶⁾", "f⁽⁷⁾", "f⁽⁸⁾"];
                let mut zeros = vec!["f″"];
                for (j, d) in higher_v.iter().enumerate() {
                    let order = j + 3;
                    let v = d[i];
                    let tol = 1e-6 * fscale / diag.powi(order as i32);
                    if !v.is_finite() {
                        res = (Class::Degenerate, format!("{} = 0 で、{} が求まらない", zeros.join(" = "), primes[j]));
                        break;
                    }
                    if v.abs() > tol {
                        let z = format!("{} = 0, {} = {}", zeros.join(" = "), primes[j], num_text(v));
                        res = if order % 2 == 1 {
                            (Class::NotExtremum, format!("{z} ≠ 0。奇数階なので極値でない"))
                        } else if v > 0.0 {
                            (Class::Min, format!("{z} {}", sign_text(v)))
                        } else {
                            (Class::Max, format!("{z} {}", sign_text(v)))
                        };
                        break;
                    }
                    zeros.push(primes[j]);
                }
                res
            }
        } else {
            let d = minors(&h, k);
            let det = d[k - 1];
            let second = |a: usize| format!("∂²f/∂{}²", vars[a]);
            if det.abs() <= 1e-7 * hs.powi(k as i32).max(zero2.powi(k as i32)) || hs <= zero2 {
                (Class::Degenerate, "det H = 0 なので、2 次の項では判定できない".to_string())
            } else if k == 2 {
                if det < 0.0 {
                    (Class::Saddle, format!("det H = {} < 0", num_text(det)))
                } else {
                    let c = if d[0] > 0.0 { Class::Min } else { Class::Max };
                    (c, format!("det H = {} > 0, {} = {} {}", num_text(det), second(0), num_text(d[0]), sign_text(d[0])))
                }
            } else {
                let list = d.iter().map(|v| num_text(*v)).collect::<Vec<_>>().join(", ");
                if d.iter().all(|v| *v > 0.0) {
                    (Class::Min, format!("主小行列式 {list} がすべて正"))
                } else if d[0] < 0.0 && d[1] > 0.0 && d[2] < 0.0 {
                    (Class::Max, format!("主小行列式 {list} の符号が −, +, −"))
                } else {
                    (Class::Saddle, format!("主小行列式 {list}。det H ≠ 0 で、正定値でも負定値でもない"))
                }
            }
        };
        points.push(Critical { point: p.clone(), value: fv[i], class, reason });
    }
    Ok(Found { points, more })
}

/// 種類の名前
pub fn class_name(c: Class) -> &'static str {
    match c {
        Class::Min => "極小",
        Class::Max => "極大",
        Class::Saddle => "鞍点",
        Class::NotExtremum => "極値でない",
        Class::Degenerate => "判定できない",
    }
}

/// 注目する点の種類（画面側の番号と同じ）
pub const POI_MAX: f64 = 0.0;
pub const POI_MIN: f64 = 1.0;
pub const POI_SADDLE: f64 = 2.0;
pub const POI_ZERO: f64 = 3.0;
pub const POI_INTERCEPT: f64 = 4.0;
pub const POI_OTHER: f64 = 5.0;

/// 陽関数（縦の軸 `axis` の座標 = e）の、表示範囲の中の注目する点（点を押すと値が出る機能。計画のおすすめ 1）。
/// 1 変数なら極値・零点・縦軸との交点、2 変数なら停留点（極大・極小・鞍点）。
/// 結果は表示中の座標の点と種類の並び [p₀, p₁, p₂, 種類, …]。`flat`（2D 表示）なら奥の座標は 0
pub fn points_of_interest(axis: usize, e: &Expr, shown: &[String; 3], domain: &[f64; 6], params: &Params, flat: bool) -> Vec<f64> {
    let others: Vec<usize> = (0..3).filter(|&k| k != axis).collect();
    let used: Vec<usize> = others.iter().copied().filter(|&k| e.uses_var(&shown[k])).collect();
    let range = |k: usize| (domain[2 * k].min(domain[2 * k + 1]), domain[2 * k].max(domain[2 * k + 1]));
    let (vlo, vhi) = range(axis);
    let margin = 1e-9 * (vhi - vlo).abs().max(1e-300);
    let in_view = |v: f64| v.is_finite() && v >= vlo - margin && v <= vhi + margin;
    let mut out = Vec::new();
    // 使わない座標の値（2D 表示では 0。3D では 0 が範囲に入らなければ真ん中）
    let rest = |k: usize| {
        let (lo, hi) = range(k);
        if flat || (lo <= 0.0 && 0.0 <= hi) { 0.0 } else { (lo + hi) / 2.0 }
    };
    let push = |coords: &[(usize, f64)], value: f64, kind: f64, out: &mut Vec<f64>| {
        let mut p = [0.0; 3];
        for k in 0..3 {
            p[k] = rest(k);
        }
        for &(k, v) in coords {
            p[k] = v;
        }
        p[axis] = value;
        out.extend_from_slice(&[p[0], p[1], p[2], kind]);
    };
    if used.is_empty() || used.len() > 2 || (flat && used.iter().any(|&k| k == 2)) {
        return out;
    }
    let vars: Vec<String> = used.iter().map(|&k| shown[k].clone()).collect();
    let boxes: Vec<(f64, f64)> = used.iter().map(|&k| range(k)).collect();
    if let Ok(found) = find(e, &vars, &boxes, params) {
        for c in found.points.iter().take(60) {
            if !in_view(c.value) {
                continue;
            }
            let kind = match c.class {
                Class::Max => POI_MAX,
                Class::Min => POI_MIN,
                Class::Saddle => POI_SADDLE,
                _ => POI_OTHER,
            };
            let coords: Vec<(usize, f64)> = used.iter().copied().zip(c.point.iter().copied()).collect();
            push(&coords, c.value, kind, &mut out);
        }
    }
    if used.len() == 1 {
        let k = used[0];
        let Ok(prog) = Program::compile_with(e, &[vars[0].as_str()], params) else { return out };
        let f = |x: f64| -> f64 {
            let mut o = [C::new(0.0, 0.0)];
            prog.eval_batch(&[&[x]], &mut o);
            as_real(o[0])
        };
        let (lo, hi) = boxes[0];
        // 零点: 符号が変わる所を二分法で詰める（極をはさむ所は、詰めても値が小さくならないので捨てる）
        let m = 1600;
        let xs: Vec<f64> = (0..=m).map(|i| lo + (hi - lo) * i as f64 / m as f64).collect();
        let ys: Vec<f64> = xs.iter().map(|&x| f(x)).collect();
        let mut zeros = 0;
        for i in 0..m {
            if zeros >= 60 {
                break;
            }
            let (y0, y1) = (ys[i], ys[i + 1]);
            if !(y0.is_finite() && y1.is_finite()) {
                continue;
            }
            if y0 == 0.0 {
                push(&[(k, xs[i])], 0.0, POI_ZERO, &mut out);
                zeros += 1;
                continue;
            }
            if y0.signum() == y1.signum() || y1 == 0.0 {
                continue;
            }
            let (mut a, mut b, mut fa) = (xs[i], xs[i + 1], y0);
            for _ in 0..100 {
                let c = 0.5 * (a + b);
                let fc = f(c);
                if !fc.is_finite() {
                    break;
                }
                if fc == 0.0 {
                    a = c;
                    b = c;
                    break;
                }
                if fc.signum() == fa.signum() {
                    a = c;
                    fa = fc;
                } else {
                    b = c;
                }
                if b - a <= 1e-15 * a.abs().max(b.abs()).max(1e-300) {
                    break;
                }
            }
            let r = 0.5 * (a + b);
            let fr = f(r);
            if fr.is_finite() && fr.abs() <= 1e-7 * (y0.abs() + y1.abs()) {
                let r = if r.abs() < 1e-12 * (hi - lo) { 0.0 } else { r };
                push(&[(k, r)], 0.0, POI_ZERO, &mut out);
                zeros += 1;
            }
        }
        if ys[m] == 0.0 {
            push(&[(k, xs[m])], 0.0, POI_ZERO, &mut out);
        }
        // 縦の軸との交点（変数 = 0 での値）
        if lo <= 0.0 && 0.0 <= hi {
            let v = f(0.0);
            if in_view(v) {
                push(&[(k, 0.0)], v, POI_INTERCEPT, &mut out);
            }
        }
    }
    out
}

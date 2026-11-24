//! 被積分関数の、値が発散する・飛ぶ・折れる所（積分の区間を分ける点）を、式の形から探す
//! （計画 `docs/plan-stage2b-integrals.md` の 2.1。#110 #159 #109）。
//!
//! 適応型の積分は、誤差の大きい所だけを細かくするので、特異点や場合分けの境目に点が乗らないと気づかない
//! （$\int_0^1 |t - 1/4|^{-1/2} dt$ が 10% ずれる・幅 0.01 のパルスが 0 になる）。そこで、式の中の
//! 分母・負の累乗・ln・tan（発散する所）、√・絶対値・符号・場合分けの条件・min・max（折れる・飛ぶ所）、
//! floor・mod など（整数をまたぐ所）の「もとになる式」 g を集め、区間の中で g = 0 になる点を求める。
//! その点で区間を分けると、特異点・境目が小区間の端に来るので、端の扱い（`quad.rs` の端の検査）で正しく計算できる
//! （QUADPACK の QAGP と同じ考え方。分ける点を、利用者が書く代わりに式から見つける）

use crate::eval::{as_real, Params, Program};
use crate::functions::Func;
use crate::parser::{BinOp, Expr};
use num_complex::Complex64 as C;

/// 区間を分ける点の数の上限
pub const MAX_POINTS: usize = 200;
/// 符号の変わり目を探すときの、区間の分け方の数
const SAMPLES: usize = 512;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    /// g = 0 で値が発散しうる（分母・負の累乗・ln・tan）
    Singular,
    /// g = 0 で折れる・飛ぶ（√・絶対値・符号・場合分けの条件・min・max）
    Jump,
    /// g が整数をまたぐ所で飛ぶ（floor・ceil・round・mod）
    Integer,
}

/// 見つけた点
pub struct Found {
    /// 区間の中（端は除く）の、分ける点（小さい順）
    pub points: Vec<f64>,
    /// 発散しうる点が多すぎて、全部は分けられない（tan t を長い区間で積分したなど）
    pub too_many_singular: bool,
}

/// 式 `e` から、区間を分ける点のもとになる式を集める（変数 `var` を使うものだけ）
fn collect(e: &Expr, var: &str, out: &mut Vec<(Expr, Kind)>) {
    if out.len() > 64 {
        return;
    }
    let push = |g: Expr, k: Kind, out: &mut Vec<(Expr, Kind)>| {
        if g.uses_var(var) && !out.iter().any(|(h, j)| *j == k && *h == g) {
            out.push((g, k));
        }
    };
    match e {
        Expr::Bin(BinOp::Div, a, b) => {
            push((**b).clone(), Kind::Singular, out);
            collect(a, var, out);
            collect(b, var, out);
        }
        Expr::Bin(BinOp::Pow, a, c) => {
            match &**c {
                // 正の整数乗は、どこでも有限でなめらか
                Expr::Num(n) if *n >= 0.0 && n.fract() == 0.0 => {}
                Expr::Num(n) if *n < 0.0 => push((**a).clone(), Kind::Singular, out),
                // 整数でない正の累乗（√ の形）・式の累乗は、底が 0 の所で折れる・定義の端になる
                _ if a.uses_var(var) => push((**a).clone(), if c.uses_var(var) { Kind::Singular } else { Kind::Jump }, out),
                _ => {}
            }
            collect(a, var, out);
            collect(c, var, out);
        }
        Expr::Bin(_, a, b) => {
            collect(a, var, out);
            collect(b, var, out);
        }
        Expr::Neg(a) => collect(a, var, out),
        Expr::Call(f, args) => {
            let arg = |k: usize| args.get(k).cloned().unwrap_or(Expr::Num(0.0));
            match f {
                Func::Ln | Func::Log10 => push(arg(0), Kind::Singular, out),
                Func::Tan | Func::Sec => push(Expr::Call(Func::Cos, vec![arg(0)]), Kind::Singular, out),
                Func::Cot | Func::Csc => push(Expr::Call(Func::Sin, vec![arg(0)]), Kind::Singular, out),
                Func::Sqrt | Func::Abs | Func::Sign => push(arg(0), Kind::Jump, out),
                Func::Floor | Func::Ceil | Func::Round => push(arg(0), Kind::Integer, out),
                Func::Mod if args.len() == 2 => push(Expr::Bin(BinOp::Div, Box::new(arg(0)), Box::new(arg(1))), Kind::Integer, out),
                Func::Min | Func::Max if args.len() <= 6 => {
                    for i in 0..args.len() {
                        for j in i + 1..args.len() {
                            push(Expr::Bin(BinOp::Sub, Box::new(arg(i)), Box::new(arg(j))), Kind::Jump, out);
                        }
                    }
                }
                Func::Piece => {
                    // [条件の数 k, 厳密の印, 余裕 m₁, …, m_k, 値] の並び。余裕 m = 0 の所が境目
                    let mut pos = 0;
                    while pos + 2 < args.len() {
                        let Expr::Num(k) = args[pos] else { break };
                        let k = k as usize;
                        for m in args.iter().skip(pos + 2).take(k) {
                            push(m.clone(), Kind::Jump, out);
                        }
                        pos += 3 + k;
                    }
                }
                _ => {}
            }
            for a in args {
                collect(a, var, out);
            }
        }
        Expr::Tuple(items) | Expr::List(items) => {
            for a in items {
                collect(a, var, out);
            }
        }
        _ => {}
    }
}

/// 標本の点（有限の区間は等分、∞ の端は t = a + L s/(1 − s) で写す）
fn sample_points(a: f64, b: f64) -> Vec<f64> {
    let n = SAMPLES;
    match (a.is_finite(), b.is_finite()) {
        (true, true) => (0..=n).map(|k| a + (b - a) * k as f64 / n as f64).collect(),
        (true, false) => {
            let l = a.abs().max(1.0);
            (0..n).map(|k| {
                let s = k as f64 / n as f64;
                a + l * s / (1.0 - s)
            }).collect()
        }
        (false, true) => {
            let l = b.abs().max(1.0);
            let mut v: Vec<f64> = (0..n).map(|k| {
                let s = k as f64 / n as f64;
                b - l * s / (1.0 - s)
            }).collect();
            v.reverse();
            v
        }
        (false, false) => {
            let mut v = sample_points(f64::NEG_INFINITY, 0.0);
            v.pop();
            v.extend(sample_points(0.0, f64::INFINITY));
            v
        }
    }
}

/// 1 変数の式の値（複素数なら NaN）
fn value(p: &Program, t: f64) -> f64 {
    let mut out = [C::new(0.0, 0.0)];
    p.eval_batch(&[&[t]], &mut out);
    as_real(out[0])
}

/// 区間 [a, b]（端は ±∞ でもよい）の中で、被積分関数 `f`（変数 `var`）の区間を分ける点を探す
pub fn find(f: &Expr, var: &str, params: &Params, a: f64, b: f64) -> Found {
    let (a, b) = (a.min(b), a.max(b));
    let mut found = Found { points: Vec::new(), too_many_singular: false };
    if !(a < b) {
        return found;
    }
    let mut parts = Vec::new();
    collect(f, var, &mut parts);
    let xs = sample_points(a, b);
    let width = if a.is_finite() && b.is_finite() { b - a } else { 1.0 };
    let mut singular = Vec::new();
    let mut other = Vec::new();
    for (g, kind) in &parts {
        let Ok(p) = Program::compile_with(g, &[var], params) else { continue };
        let ys: Vec<f64> = xs.iter().map(|&t| value(&p, t)).collect();
        let scale = ys.iter().filter(|y| y.is_finite()).fold(0.0f64, |m, y| m.max(y.abs()));
        let target = if *kind == Kind::Singular { &mut singular } else { &mut other };
        for k in 0..xs.len() - 1 {
            let (x0, x1, y0, y1) = (xs[k], xs[k + 1], ys[k], ys[k + 1]);
            if !(y0.is_finite() && y1.is_finite()) {
                continue;
            }
            if *kind == Kind::Integer {
                if y0.floor() != y1.floor() {
                    // 整数をまたぐ所を、二分法で詰める
                    let (mut lo, mut hi, side) = (x0, x1, y0.floor());
                    for _ in 0..80 {
                        let m = 0.5 * (lo + hi);
                        if m == lo || m == hi {
                            break;
                        }
                        if value(&p, m).floor() == side { lo = m } else { hi = m }
                    }
                    target.push(0.5 * (lo + hi));
                }
                continue;
            }
            if y0 == 0.0 {
                target.push(x0);
                continue;
            }
            if y0.signum() != y1.signum() && y1 != 0.0 {
                // 符号の変わり目を二分法で詰める
                let (mut lo, mut hi) = (x0, x1);
                for _ in 0..80 {
                    let m = 0.5 * (lo + hi);
                    if m == lo || m == hi {
                        break;
                    }
                    let ym = value(&p, m);
                    if !ym.is_finite() {
                        break;
                    }
                    if ym.signum() == y0.signum() { lo = m } else { hi = m }
                }
                target.push(0.5 * (lo + hi));
                continue;
            }
            // 符号が変わらずに 0 に触れる所（t² の 0）: |g| の小さい所を黄金分割で詰める
            if k > 0 && ys[k - 1].is_finite() && y0.abs() <= ys[k - 1].abs() && y0.abs() <= y1.abs() && y0.abs() < 1e-3 * scale {
                let (mut lo, mut hi) = (xs[k - 1], x1);
                let phi = 0.618_033_988_749_895;
                for _ in 0..120 {
                    let (m1, m2) = (hi - phi * (hi - lo), lo + phi * (hi - lo));
                    if value(&p, m1).abs() <= value(&p, m2).abs() { hi = m2 } else { lo = m1 }
                    if hi - lo <= 1e-15 * width.max(lo.abs()) {
                        break;
                    }
                }
                let m = 0.5 * (lo + hi);
                if value(&p, m).abs() <= 1e-10 * scale.max(f64::MIN_POSITIVE) {
                    target.push(m);
                }
            }
        }
        if b.is_finite() && ys.last().is_some_and(|y| *y == 0.0) {
            target.push(b);
        }
    }
    // 端と、ほぼ同じ所の点を除く。区間の幅に比べてとても小さい点は 0 にそろえる（二分法で −1e−17 のようになる）
    let tiny = |x: f64| 1e-12 * width.max(x.abs());
    let clean = |mut v: Vec<f64>| -> Vec<f64> {
        for x in v.iter_mut() {
            if x.abs() <= 1e-12 * width {
                *x = 0.0;
            }
        }
        v.retain(|&x| x > a + tiny(a) && x < b - tiny(b));
        v.sort_by(f64::total_cmp);
        v.dedup_by(|x, y| (*x - *y).abs() <= tiny(*y));
        v
    };
    let singular = clean(singular);
    if singular.len() > MAX_POINTS {
        found.too_many_singular = true;
        return found;
    }
    let mut all = singular.clone();
    all.extend(clean(other));
    let mut all = clean(all);
    if all.len() > MAX_POINTS {
        // 飛ぶ・折れる所が多すぎる（floor を長い区間で積分した）: 発散しうる点だけで分ける
        all = singular;
    }
    found.points = all;
    found
}

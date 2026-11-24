//! 打ち切ったべき級数の計算（テイラー係数を、式の木の上で直接求める。自動微分の Taylor モード）。
//!
//! $u(a + h) = \sum_{k=0}^{n} u_k h^k$ の係数 $u_k$ を、葉から順に四則・初等関数の漸化式で作る。
//! 記号微分で $n$ 階まで微分すると式が大きくなりすぎる（$\tan x$ の 20 次など）ので、1 変数・中心が数のテイラー多項式はこちらで作る。
//! 使えない節（特殊関数・`abs` など）があれば None を返し、呼ぶ側は記号微分の道に戻る

use crate::functions::Func;
use crate::parser::{BinOp, Constant, Expr};
use num_complex::Complex64 as C;

type S = Vec<C>;

fn zero(n: usize) -> S {
    vec![C::new(0.0, 0.0); n + 1]
}

fn constant(c: C, n: usize) -> S {
    let mut s = zero(n);
    s[0] = c;
    s
}

fn mul(a: &S, b: &S) -> S {
    let n = a.len() - 1;
    (0..=n).map(|k| (0..=k).map(|j| a[j] * b[k - j]).sum()).collect()
}

fn div(a: &S, b: &S) -> Option<S> {
    let n = a.len() - 1;
    if b[0].norm() == 0.0 {
        return None;
    }
    let mut w = zero(n);
    for k in 0..=n {
        let s: C = (1..=k).map(|j| b[j] * w[k - j]).sum();
        w[k] = (a[k] - s) / b[0];
    }
    Some(w)
}

/// 導関数の級数（次数は 1 つ下がるので、最後は 0）
fn deriv(a: &S) -> S {
    let n = a.len() - 1;
    let mut d = zero(n);
    for k in 0..n {
        d[k] = a[k + 1] * (k as f64 + 1.0);
    }
    d
}

/// 不定積分（定数項は c0）
fn integ(a: &S, c0: C) -> S {
    let n = a.len() - 1;
    let mut w = zero(n);
    w[0] = c0;
    for k in 1..=n {
        w[k] = a[k - 1] / k as f64;
    }
    w
}

fn exp(u: &S) -> S {
    let n = u.len() - 1;
    let mut w = zero(n);
    w[0] = u[0].exp();
    for k in 1..=n {
        let s: C = (1..=k).map(|j| u[j] * w[k - j] * j as f64).sum();
        w[k] = s / k as f64;
    }
    w
}

fn ln(u: &S) -> Option<S> {
    if u[0].norm() == 0.0 {
        return None;
    }
    Some(integ(&div(&deriv(u), u)?, u[0].ln()))
}

/// (sin u, cos u)。`hyper` なら (sinh u, cosh u)
fn sin_cos(u: &S, hyper: bool) -> (S, S) {
    let n = u.len() - 1;
    let (mut s, mut c) = (zero(n), zero(n));
    if hyper {
        s[0] = u[0].sinh();
        c[0] = u[0].cosh();
    } else {
        s[0] = u[0].sin();
        c[0] = u[0].cos();
    }
    let sign = if hyper { 1.0 } else { -1.0 };
    for k in 1..=n {
        let ds: C = (1..=k).map(|j| u[j] * c[k - j] * j as f64).sum();
        let dc: C = (1..=k).map(|j| u[j] * s[k - j] * j as f64).sum();
        s[k] = ds / k as f64;
        c[k] = dc * sign / k as f64;
    }
    (s, c)
}

/// u^p（p は定数）。u(a) ≠ 0 なら w' u = p u' w の漸化式、u(a) = 0 なら 0 以上の整数のべきだけ
fn pow_const(u: &S, p: C) -> Option<S> {
    let n = u.len() - 1;
    if u[0].norm() == 0.0 {
        if p.im != 0.0 || p.re < 0.0 || p.re.fract() != 0.0 || p.re > 64.0 {
            return None;
        }
        let mut w = constant(C::new(1.0, 0.0), n);
        for _ in 0..p.re as usize {
            w = mul(&w, u);
        }
        return Some(w);
    }
    // 整数のべきは掛け算で（負の数の整数乗も正しく）
    if p.im == 0.0 && p.re.fract() == 0.0 && p.re.abs() <= 64.0 {
        let mut w = constant(C::new(1.0, 0.0), n);
        for _ in 0..p.re.abs() as usize {
            w = mul(&w, u);
        }
        return if p.re < 0.0 { div(&constant(C::new(1.0, 0.0), n), &w) } else { Some(w) };
    }
    let mut w = zero(n);
    w[0] = u[0].powc(p);
    for k in 1..=n {
        // k u₀ w_k = Σ_{j=1}^{k} (p j − (k − j)) u_j w_{k−j}
        let s: C = (1..=k).map(|j| u[j] * w[k - j] * (p * j as f64 - (k - j) as f64)).sum();
        w[k] = s / (u[0] * k as f64);
    }
    Some(w)
}

/// 式 e の、変数 `var` の点 a のまわりの n 次までの係数。`value` は var を含まない部分式の値（数・パラメータ。値にならなければ None）
pub fn taylor(e: &Expr, var: &str, a: C, n: usize, value: &dyn Fn(&Expr) -> Option<C>) -> Option<S> {
    let one = C::new(1.0, 0.0);
    if !e.uses_var(var) {
        let v = value(e)?;
        return (v.re.is_finite() && v.im.is_finite()).then(|| constant(v, n));
    }
    let s = match e {
        Expr::Var(_) => {
            let mut s = constant(a, n);
            if n >= 1 {
                s[1] = one;
            }
            s
        }
        Expr::Neg(u) => taylor(u, var, a, n, value)?.into_iter().map(|c| -c).collect(),
        Expr::Bin(op, l, r) => {
            if *op == BinOp::Pow && !r.uses_var(var) {
                let p = value(r)?;
                return pow_const(&taylor(l, var, a, n, value)?, p);
            }
            let (x, y) = (taylor(l, var, a, n, value)?, taylor(r, var, a, n, value)?);
            match op {
                BinOp::Add => x.iter().zip(&y).map(|(p, q)| p + q).collect(),
                BinOp::Sub => x.iter().zip(&y).map(|(p, q)| p - q).collect(),
                BinOp::Mul => mul(&x, &y),
                BinOp::Div => div(&x, &y)?,
                // u^v = exp(v ln u)
                BinOp::Pow => exp(&mul(&y, &ln(&x)?)),
            }
        }
        Expr::Call(f, args) if args.len() == 1 => {
            let u = taylor(&args[0], var, a, n, value)?;
            let unit = || constant(one, n);
            match f {
                Func::Exp => exp(&u),
                Func::Ln => ln(&u)?,
                Func::Log10 => ln(&u)?.into_iter().map(|c| c / std::f64::consts::LN_10).collect(),
                Func::Sqrt => pow_const(&u, C::new(0.5, 0.0))?,
                Func::Sin => sin_cos(&u, false).0,
                Func::Cos => sin_cos(&u, false).1,
                Func::Tan => {
                    let (s, c) = sin_cos(&u, false);
                    div(&s, &c)?
                }
                Func::Sec => div(&unit(), &sin_cos(&u, false).1)?,
                Func::Csc => div(&unit(), &sin_cos(&u, false).0)?,
                Func::Cot => {
                    let (s, c) = sin_cos(&u, false);
                    div(&c, &s)?
                }
                Func::Sinh => sin_cos(&u, true).0,
                Func::Cosh => sin_cos(&u, true).1,
                Func::Tanh => {
                    let (s, c) = sin_cos(&u, true);
                    div(&s, &c)?
                }
                // 逆関数は、導関数の級数を積分する
                Func::Atan => integ(&div(&deriv(&u), &(0..=n).map(|k| unit()[k] + mul(&u, &u)[k]).collect())?, u[0].atan()),
                Func::Asin | Func::Acos => {
                    let q: S = (0..=n).map(|k| unit()[k] - mul(&u, &u)[k]).collect();
                    let d = div(&deriv(&u), &pow_const(&q, C::new(0.5, 0.0))?)?;
                    if *f == Func::Asin {
                        integ(&d, u[0].asin())
                    } else {
                        integ(&d.into_iter().map(|c| -c).collect(), u[0].acos())
                    }
                }
                Func::Asinh => {
                    let q: S = (0..=n).map(|k| unit()[k] + mul(&u, &u)[k]).collect();
                    integ(&div(&deriv(&u), &pow_const(&q, C::new(0.5, 0.0))?)?, u[0].asinh())
                }
                Func::Atanh => integ(&div(&deriv(&u), &(0..=n).map(|k| unit()[k] - mul(&u, &u)[k]).collect())?, u[0].atanh()),
                _ => return None,
            }
        }
        Expr::Const(Constant::Pi | Constant::E | Constant::I) | Expr::Num(_) => unreachable!(),
        _ => return None,
    };
    s.iter().all(|c| c.re.is_finite() && c.im.is_finite()).then_some(s)
}

/// 点 $\mathbf a$ のまわりでの式の値の様子（テイラー多項式を作れるかを確かめる。#116）
#[derive(Debug, Clone, Copy)]
pub enum Near {
    /// どの向きから近づいても同じ極限（極限, その誤差の見積もり）
    Limit(C, f64),
    /// ある向きの両側で極限が違う（負の側, 正の側）。多変数では、向きによって極限が違うときも
    Jump(C, C),
    /// 近づくほど大きくなる
    Diverges,
    /// 極限が求まらない（振動する・片側で値が無いなど）
    Unknown,
}

/// $\mathbf a \pm h\,\mathbf u$（$h = 1, 10^{-1}, \dots, 10^{-9}$ 倍）の値から、各向き $\mathbf u$ の片側極限を求めてくらべる。
/// 向きは座標軸と、2 変数以上では斜め $(1, \dots, 1)$・$(1, -1, 0, \dots)$。
/// 片側の値の列で、となりどうしの差 $d_i$ がいちばん小さい所を使う。なめらかなら $d_i$ は $h$ とともに縮み（1 桁で $1/10$）、
/// 小さい $h$ では桁落ち（$\frac{\sin x}{x}$ の導関数の式など）で大きくなるので、その間の底が極限の値と誤差。
/// 底に向かって縮んでいなければ（$\ln x$・$1/\sqrt x$ は差が縮まない・増える）極限は無い。そのとき、絶対値が増え続け、
/// 符号がそろっていれば発散（桁落ちの雑音も増えていくが、符号がばらばら）。
/// 両側・どの向きの極限も、その誤差の 10 倍の中でそろえば、その平均を極限とする
pub fn near(prog: &crate::eval::Program, a: &[f64]) -> Near {
    const STEPS: usize = 10;
    let k = a.len();
    let s = a.iter().fold(1.0f64, |m, v| m.max(v.abs()));
    let mut dirs: Vec<Vec<f64>> = (0..k).map(|i| (0..k).map(|j| if i == j { 1.0 } else { 0.0 }).collect()).collect();
    if k >= 2 {
        dirs.push(vec![1.0 / (k as f64).sqrt(); k]);
        let mut d = vec![0.0; k];
        (d[0], d[1]) = (std::f64::consts::FRAC_1_SQRT_2, -std::f64::consts::FRAC_1_SQRT_2);
        dirs.push(d);
    }
    let mut cols: Vec<Vec<f64>> = vec![Vec::new(); k];
    for d in &dirs {
        for sign in [1.0, -1.0] {
            for e in 0..STEPS {
                let h = s * 10f64.powi(-(e as i32));
                for i in 0..k {
                    cols[i].push(a[i] + sign * h * d[i]);
                }
            }
        }
    }
    let mut out = vec![C::new(0.0, 0.0); cols[0].len()];
    let inputs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
    prog.eval_batch(&inputs, &mut out);
    let finite = |v: &C| v.re.is_finite() && v.im.is_finite();
    // 片側ごとの (極限, 誤差)
    let mut sides: Vec<(C, f64)> = Vec::new();
    for side in out.chunks(STEPS) {
        // 値のある所（∞・未定義でない所）がいちばん長く続く区間（同じ長さなら、展開点に近いほう）
        let (mut best, mut start) = ((0, 0), None);
        for i in 0..=STEPS {
            match (i < STEPS && finite(&side[i]), start) {
                (true, None) => start = Some(i),
                (false, Some(b)) => {
                    if i - b >= best.1 - best.0 {
                        best = (b, i);
                    }
                    start = None;
                }
                _ => {}
            }
        }
        let v = &side[best.0..best.1];
        if v.len() < 4 {
            return if side[best.1.min(STEPS - 1)..].iter().any(|v| v.norm().is_infinite()) { Near::Diverges } else { Near::Unknown };
        }
        let d: Vec<f64> = v.windows(2).map(|w| (w[1] - w[0]).norm()).collect();
        let i = (0..d.len()).min_by(|&x, &y| d[x].total_cmp(&d[y])).unwrap();
        // 底に向かって縮んでいること（1 つ前の差の 1/3 以下）。どこも差が 0（定数）でもよい
        let converged = d[i] == 0.0 || (i >= 1 && d[i - 1] >= 3.0 * d[i]);
        if !converged {
            let tail = &v[v.len() - 4..];
            let grows = tail.windows(2).all(|w| w[1].norm() > 1.05 * w[0].norm());
            let same_sign = tail.windows(2).all(|w| (w[1] * w[0].conj()).re > 0.0);
            return if grows && same_sign { Near::Diverges } else { Near::Unknown };
        }
        let noise = 1e-12 * v[i + 1].norm();
        sides.push((v[i + 1], d[i] + noise));
    }
    // sides は 向き 0 の正の側, 負の側, 向き 1 の正の側, …
    let (l0, e0) = sides[0];
    for &(l, e) in &sides[1..] {
        if (l - l0).norm() > 10.0 * (e + e0) {
            return Near::Jump(l, l0);
        }
    }
    let err = sides.iter().fold(0.0f64, |m, (_, e)| m.max(*e));
    Near::Limit(sides.iter().map(|(l, _)| *l).sum::<C>() / sides.len() as f64, err)
}

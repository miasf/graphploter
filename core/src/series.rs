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

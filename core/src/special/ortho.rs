//! 直交多項式と球面調和関数。次数が 0 以上の整数なら漸化式、そうでなければ超幾何関数で計算する。

use super::hyper::{hyp1f1, hyp2f1, hyp_u, nonnegative_int, nonpositive_int, nudge};
use super::{binomial, gamma, rgamma};
use crate::special::gamma_ext::pochhammer;
use num_complex::Complex64 as C;
use std::f64::consts::PI;

/// 漸化式で計算する次数の上限
const MAX_DEGREE: u64 = 5000;

fn degree(n: C) -> Option<usize> {
    nonnegative_int(n).filter(|&k| k <= MAX_DEGREE).map(|k| k as usize)
}

/// 3 項漸化式 p_{k+1} = step(k, p_k, p_{k−1}) を n まで回す
fn recur(n: usize, p0: C, p1: C, step: impl Fn(f64, C, C) -> C) -> C {
    if n == 0 {
        return p0;
    }
    let (mut a, mut b) = (p0, p1);
    for k in 1..n {
        let c = step(k as f64, b, a);
        a = b;
        b = c;
    }
    b
}

/// ルジャンドル多項式（関数）Pₙ(x)
pub fn legendre_p(n: C, x: C) -> C {
    if let Some(n) = degree(n) {
        return recur(n, C::new(1.0, 0.0), x, |k, p, q| ((2.0 * k + 1.0) * x * p - k * q) / (k + 1.0));
    }
    hyp2f1(-n, n + 1.0, C::new(1.0, 0.0), (1.0 - x) / 2.0)
}

/// ルジャンドル陪関数 Pₙᵐ(x)（Wolfram の LegendreP[n, m, x]、コンドン・ショートレーの位相つき）
pub fn legendre_p_assoc(n: C, m: C, x: C) -> C {
    let general = |m: C| {
        ((1.0 + x) / (1.0 - x)).powc(m / 2.0) * rgamma(1.0 - m)
            * hyp2f1(-n, n + 1.0, 1.0 - m, (1.0 - x) / 2.0)
    };
    match nonnegative_int(m) {
        Some(0) => legendre_p(n, x),
        Some(mi) => {
            // n が整数なら P_{−n−1} = Pₙ で 0 以上にそろえ、m 階微分 = (2m − 1)!! C^{(m+1/2)}_{n−m}
            let n_int = nonnegative_int(n).or_else(|| nonpositive_int(n + 1.0).map(|k| (-k) as u64));
            match n_int {
                Some(ni) if ni <= MAX_DEGREE => {
                    if mi > ni {
                        return C::new(0.0, 0.0);
                    }
                    let dfact: f64 = (1..=mi).map(|k| (2 * k - 1) as f64).product();
                    let deriv = dfact
                        * gegenbauer_c(C::new((ni - mi) as f64, 0.0), C::new(mi as f64 + 0.5, 0.0), x);
                    let mf = mi as f64;
                    let sign = if mi % 2 == 1 { -1.0 } else { 1.0 };
                    sign * ((1.0 + x) / (1.0 - x)).powf(mf / 2.0) * (1.0 - x).powf(mf) * deriv
                }
                _ => nudge(general, m),
            }
        }
        None => general(m),
    }
}

/// エルミート多項式（関数）Hₙ(x)
pub fn hermite_h(n: C, x: C) -> C {
    if let Some(n) = degree(n) {
        return recur(n, C::new(1.0, 0.0), 2.0 * x, |k, p, q| 2.0 * x * p - 2.0 * k * q);
    }
    let x2 = x * x;
    let two_n = C::new(2.0, 0.0).powc(n);
    if x.re > 0.0 && x.norm() > 3.0 {
        // Hₙ(x) = 2ⁿ U(−n/2, 1/2, x²)（Re x > 0）。₁F₁ の差を取るより桁落ちしない
        return two_n * hyp_u(-n / 2.0, C::new(0.5, 0.0), x2);
    }
    two_n
        * PI.sqrt()
        * (hyp1f1(-n / 2.0, C::new(0.5, 0.0), x2) * rgamma((1.0 - n) / 2.0)
            - 2.0 * x * hyp1f1((1.0 - n) / 2.0, C::new(1.5, 0.0), x2) * rgamma(-n / 2.0))
}

/// ラゲール陪多項式 Lₙᵃ(x)（a = 0 で普通のラゲール多項式）
pub fn laguerre_l(n: C, a: C, x: C) -> C {
    if let Some(n) = degree(n) {
        return recur(n, C::new(1.0, 0.0), 1.0 + a - x, |k, p, q| {
            ((2.0 * k + 1.0 + a - x) * p - (k + a) * q) / (k + 1.0)
        });
    }
    binomial(n + a, n) * hyp1f1(-n, a + 1.0, x)
}

/// 第1種チェビシェフ多項式 Tₙ(x) = cos(n arccos x)
pub fn chebyshev_t(n: C, x: C) -> C {
    if let Some(n) = degree(n) {
        return recur(n, C::new(1.0, 0.0), x, |_, p, q| 2.0 * x * p - q);
    }
    (n * x.acos()).cos()
}

/// 第2種チェビシェフ多項式 Uₙ(x) = sin((n + 1) θ) / sin θ（x = cos θ）
pub fn chebyshev_u(n: C, x: C) -> C {
    if let Some(n) = degree(n) {
        return recur(n, C::new(1.0, 0.0), 2.0 * x, |_, p, q| 2.0 * x * p - q);
    }
    let t = x.acos();
    ((n + 1.0) * t).sin() / t.sin()
}

/// ゲーゲンバウアー多項式 Cₙ^(a)(x)
pub fn gegenbauer_c(n: C, a: C, x: C) -> C {
    if let Some(n) = degree(n) {
        return recur(n, C::new(1.0, 0.0), 2.0 * a * x, |k, p, q| {
            (2.0 * x * (k + a) * p - (k + 2.0 * a - 1.0) * q) / (k + 1.0)
        });
    }
    gamma(n + 2.0 * a) * rgamma(n + 1.0) * rgamma(2.0 * a)
        * hyp2f1(-n, n + 2.0 * a, a + 0.5, (1.0 - x) / 2.0)
}

/// ヤコビ多項式 Pₙ^(a, b)(x)
pub fn jacobi_p(n: C, a: C, b: C, x: C) -> C {
    if let Some(n) = degree(n) {
        // Σ C(n+a, n−s) C(n+b, s) ((x−1)/2)ˢ ((x+1)/2)^(n−s)
        let nf = n as f64;
        let (xm, xp) = ((x - 1.0) / 2.0, (x + 1.0) / 2.0);
        let mut sum = C::new(0.0, 0.0);
        for s in 0..=n {
            let sf = s as f64;
            sum += binomial(a + nf, C::new(nf - sf, 0.0))
                * binomial(b + nf, C::new(sf, 0.0))
                * xm.powi(s as i32)
                * xp.powi((n - s) as i32);
        }
        return sum;
    }
    pochhammer(a + 1.0, n) * rgamma(n + 1.0) * hyp2f1(-n, n + a + b + 1.0, a + 1.0, (1.0 - x) / 2.0)
}

/// 球面調和関数 Y_l^m(θ, φ)
pub fn spherical_harmonic_y(l: C, m: C, theta: C, phi: C) -> C {
    if let (Some(li), true) = (nonnegative_int(l), m.im == 0.0 && m.re == m.re.round()) {
        if m.re.abs() > li as f64 {
            return C::new(0.0, 0.0);
        }
    }
    let norm = ((2.0 * l + 1.0) / (4.0 * PI) * gamma(l - m + 1.0) * rgamma(l + m + 1.0)).sqrt();
    norm * legendre_p_assoc(l, m, theta.cos()) * (C::new(0.0, 1.0) * m * phi).exp()
}

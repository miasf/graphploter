//! ベッセル関数の残り（Y、K、球ベッセル関数）とエアリー関数。

use super::hyper::{hyp0f1, nudge};
use super::{bessel_i, bessel_j};
use num_complex::Complex64 as C;
use std::f64::consts::PI;

fn is_integer(z: C) -> bool {
    z.im == 0.0 && z.re == z.re.round()
}

/// |z| が大きいとき漸近展開を使うかどうか
pub(crate) fn use_asymptotic(nu: C, z: C) -> bool {
    z.norm() > 25.0 + nu.norm() * nu.norm() / 2.0
}

/// 10 点ガウス・ルジャンドル則（正の側の節点と重み）
const GL_X: [f64; 5] = [
    0.148_874_338_981_631_22,
    0.433_395_394_129_247_2,
    0.679_409_568_299_024_4,
    0.865_063_366_688_984_5,
    0.973_906_528_517_171_7,
];
const GL_W: [f64; 5] = [
    0.295_524_224_714_752_8,
    0.269_266_719_309_996_5,
    0.219_086_362_515_982,
    0.149_451_349_150_580_4,
    0.066_671_344_308_688_14,
];

/// [a, b] の 10 点ガウス・ルジャンドル積分（f は 2 つの値を同時に返す）
fn gauss2(f: &impl Fn(f64) -> (C, C), a: f64, b: f64) -> (C, C) {
    let (mid, half) = ((a + b) / 2.0, (b - a) / 2.0);
    let (mut s1, mut s2) = (C::new(0.0, 0.0), C::new(0.0, 0.0));
    for (x, w) in GL_X.iter().zip(GL_W.iter()) {
        for t in [mid - half * x, mid + half * x] {
            let (u, v) = f(t);
            s1 += w * u;
            s2 += w * v;
        }
    }
    (s1 * half, s2 * half)
}

/// シュレーフリの積分表示で (J_ν(z), Y_ν(z)) を計算する（Re z > 0）。
/// 級数の桁落ちが大きく、漸近展開もまだ使えない中くらいの |z| 用
pub(crate) fn schlafli(nu: C, z: C) -> (C, C) {
    // ∫₀^π cos(ντ − z sin τ) dτ と ∫₀^π sin(z sin τ − ντ) dτ
    let panels = ((z.norm() + nu.norm()) / 2.0).ceil() as usize + 4;
    let width = PI / panels as f64;
    let osc = |t: f64| {
        let th = nu * t - z * t.sin();
        (th.cos(), -th.sin())
    };
    let (mut ic, mut is) = (C::new(0.0, 0.0), C::new(0.0, 0.0));
    for k in 0..panels {
        let (a, b) = gauss2(&osc, k as f64 * width, (k + 1) as f64 * width);
        ic += a;
        is += b;
    }
    // ∫₀^∞ e^(−z sinh t) e^(∓νt) dt
    let tail = |t: f64| {
        let e = (-z * t.sinh()).exp();
        (e * (-nu * t).exp(), e * (nu * t).exp())
    };
    let (mut ta, mut tb) = (C::new(0.0, 0.0), C::new(0.0, 0.0));
    let mut top = f64::NEG_INFINITY;
    for k in 0..400 {
        let (t0, t1) = (k as f64 * 0.25, (k + 1) as f64 * 0.25);
        let (a, b) = gauss2(&tail, t0, t1);
        ta += a;
        tb += b;
        let level = nu.re.abs() * t1 - z.re * t1.sinh();
        top = top.max(level);
        if level < top - 40.0 {
            break;
        }
    }
    let (s, c) = ((nu * PI).sin(), (nu * PI).cos());
    let j = (ic - s * ta) / PI;
    let y = (is - (tb + c * ta)) / PI;
    (j, y)
}

/// Re z < 0 のとき z = w e^(±iπ)（Re w > 0）と書いたときの (w, ±1)
pub(crate) fn reflect(z: C) -> (C, f64) {
    (-z, if z.arg() > 0.0 { 1.0 } else { -1.0 })
}

/// シュレーフリの積分を使う範囲か
pub(crate) fn use_schlafli(nu: C, z: C) -> bool {
    z.re > 0.0 && z.norm() > 10.0 && !use_asymptotic(nu, z)
}

/// ハンケルの漸近展開の P, Q（項が一番小さくなる所で止める）
pub(crate) fn hankel_pq(nu: C, z: C) -> (C, C) {
    let mu = 4.0 * nu * nu;
    let mut p = C::new(1.0, 0.0);
    let mut q = C::new(0.0, 0.0);
    let mut t = C::new(1.0, 0.0);
    let mut last = f64::INFINITY;
    let eight_z = 8.0 * z;
    for k in 1..60 {
        let kk = (2 * k - 1) as f64;
        let next = t * (mu - kk * kk) / (k as f64 * eight_z);
        if next.norm() > last && k > 2 {
            break;
        }
        last = next.norm();
        t = next;
        if k % 2 == 1 {
            q += if (k / 2) % 2 == 0 { t } else { -t };
        } else {
            p += if (k / 2) % 2 == 1 { -t } else { t };
        }
        if t.norm() < 1e-17 {
            break;
        }
    }
    (p, q)
}

/// 第2種ベッセル関数 Y_ν(z)
pub fn bessel_y(nu: C, z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return C::new(f64::NEG_INFINITY, 0.0);
    }
    if z.re < 0.0 && z.norm() > 10.0 {
        // Y_ν(w e^(iπs)) = e^(−iπνs) Y_ν(w) + 2is cos(πν) J_ν(w)
        let (w, s) = reflect(z);
        let i = C::new(0.0, 1.0);
        return (-i * PI * nu * s).exp() * bessel_y(nu, w) + 2.0 * i * s * (PI * nu).cos() * bessel_j(nu, w);
    }
    if use_schlafli(nu, z) {
        return schlafli(nu, z).1;
    }
    if use_asymptotic(nu, z) {
        let (p, q) = hankel_pq(nu, z);
        let w = z - nu * (PI / 2.0) - PI / 4.0;
        return (2.0 / (PI * z)).sqrt() * (w.sin() * p + w.cos() * q);
    }
    let f = |nu: C| {
        ((nu * PI).cos() * bessel_j(nu, z) - bessel_j(-nu, z)) / (nu * PI).sin()
    };
    if is_integer(nu) {
        nudge(f, nu)
    } else {
        f(nu)
    }
}

/// 第2種変形ベッセル関数 K_ν(z)
pub fn bessel_k(nu: C, z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return C::new(f64::INFINITY, 0.0);
    }
    if z.re < 0.0 && z.norm() > 10.0 {
        // K_ν(w e^(iπs)) = e^(−iπνs) K_ν(w) − iπs I_ν(w)
        let (w, s) = reflect(z);
        let i = C::new(0.0, 1.0);
        return (-i * PI * nu * s).exp() * bessel_k(nu, w) - i * PI * s * bessel_i(nu, w);
    }
    let theta = z.arg();
    if use_asymptotic(nu, z) {
        // K_ν(z) ≈ √(π / 2z) e^(−z) Σ a_k(ν) / zᵏ
        let mu = 4.0 * nu * nu;
        let mut t = C::new(1.0, 0.0);
        let mut sum = t;
        let mut last = f64::INFINITY;
        for k in 1..60 {
            let kk = (2 * k - 1) as f64;
            let next = t * (mu - kk * kk) / (k as f64 * 8.0 * z);
            if (next.norm() > last && k > 2) || next.norm() < 1e-17 * sum.norm() {
                break;
            }
            last = next.norm();
            t = next;
            sum += t;
        }
        return (PI / (2.0 * z)).sqrt() * (-z).exp() * sum;
    }
    if z.re > 0.0 && z.norm() >= 2.0 && theta.abs() < 1.45 {
        // K_ν(z) = ∫₀^∞ e^(−z cosh t) cosh(νt) dt を台形公式で（I の差を取るより桁落ちしない）
        let d = (PI / 2.0 - theta.abs()) * 0.8;
        let h = (d / 6.0).min(0.1);
        let integrand = |t: f64| (-z * t.cosh()).exp() * (nu * t).cosh();
        let mut sum = integrand(0.0) / 2.0;
        for k in 1..20000 {
            let t = k as f64 * h;
            sum += integrand(t);
            if z.re * t.cosh() - nu.re.abs() * t > z.re + 42.0 {
                break;
            }
        }
        return sum * h;
    }
    let f = |nu: C| PI / 2.0 * (bessel_i(-nu, z) - bessel_i(nu, z)) / (nu * PI).sin();
    if is_integer(nu) {
        nudge(f, nu)
    } else {
        f(nu)
    }
}

/// 球ベッセル関数 jₙ(z) = √(π / 2z) J_{n+1/2}(z)
pub fn spherical_j(n: C, z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return if n == C::new(0.0, 0.0) { C::new(1.0, 0.0) } else { z };
    }
    (PI / (2.0 * z)).sqrt() * bessel_j(n + 0.5, z)
}

/// 球ベッセル関数 yₙ(z) = √(π / 2z) Y_{n+1/2}(z)
pub fn spherical_y(n: C, z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return C::new(f64::NEG_INFINITY, 0.0);
    }
    (PI / (2.0 * z)).sqrt() * bessel_y(n + 0.5, z)
}

/// 1 / (3^(2/3) Γ(2/3))
const AIRY_C1: f64 = 0.355_028_053_887_817_2;
/// 1 / (3^(1/3) Γ(1/3))
const AIRY_C2: f64 = 0.258_819_403_792_806_8;

/// エアリー関数の組 (Ai(z), Bi(z))
pub fn airy(z: C) -> (C, C) {
    let s3 = 3f64.sqrt();
    if z.norm() <= 5.0 {
        let w = z * z * z / 9.0;
        let f = hyp0f1(C::new(2.0 / 3.0, 0.0), w);
        let g = z * hyp0f1(C::new(4.0 / 3.0, 0.0), w);
        return (AIRY_C1 * f - AIRY_C2 * g, s3 * (AIRY_C1 * f + AIRY_C2 * g));
    }
    let third = C::new(1.0 / 3.0, 0.0);
    if z.re >= 0.0 {
        let zeta = 2.0 / 3.0 * z.powf(1.5);
        let r = (z / 3.0).sqrt();
        let ai = r / PI * bessel_k(third, zeta);
        let bi = r * (bessel_i(-third, zeta) + bessel_i(third, zeta));
        (ai, bi)
    } else {
        let w = -z;
        let zeta = 2.0 / 3.0 * w.powf(1.5);
        let (jp, jm) = (bessel_j(third, zeta), bessel_j(-third, zeta));
        (w.sqrt() / 3.0 * (jp + jm), (w / 3.0).sqrt() * (jm - jp))
    }
}

/// エアリー関数の導関数の組 (Ai′(z), Bi′(z))
pub fn airy_prime(z: C) -> (C, C) {
    let s3 = 3f64.sqrt();
    if z.norm() <= 5.0 {
        let w = z * z * z / 9.0;
        let fp = z * z / 2.0 * hyp0f1(C::new(5.0 / 3.0, 0.0), w);
        let gp = hyp0f1(C::new(4.0 / 3.0, 0.0), w) + z * z * z / 4.0 * hyp0f1(C::new(7.0 / 3.0, 0.0), w);
        return (AIRY_C1 * fp - AIRY_C2 * gp, s3 * (AIRY_C1 * fp + AIRY_C2 * gp));
    }
    let two_thirds = C::new(2.0 / 3.0, 0.0);
    if z.re >= 0.0 {
        let zeta = 2.0 / 3.0 * z.powf(1.5);
        let ai = -z / (PI * s3) * bessel_k(two_thirds, zeta);
        let bi = z / s3 * (bessel_i(-two_thirds, zeta) + bessel_i(two_thirds, zeta));
        (ai, bi)
    } else {
        let w = -z;
        let zeta = 2.0 / 3.0 * w.powf(1.5);
        let (jp, jm) = (bessel_j(two_thirds, zeta), bessel_j(-two_thirds, zeta));
        (w / 3.0 * (jp - jm), w / s3 * (jm + jp))
    }
}

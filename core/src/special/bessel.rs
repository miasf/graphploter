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

/// シュレーフリの積分の計算に使う数（実数と複素数）。実数の ν, z では実数だけで計算する
/// （複素数の cos・sin・exp は実数の 4 倍ほどの手間がかかる。ベッセル関数の膜のグラフで効く）
pub(crate) trait SchlafliNum:
    Copy
    + std::ops::Add<Output = Self>
    + std::ops::Sub<Output = Self>
    + std::ops::Mul<Output = Self>
    + std::ops::Mul<f64, Output = Self>
    + std::ops::Div<f64, Output = Self>
    + std::ops::Neg<Output = Self>
    + std::ops::AddAssign
{
    fn zero() -> Self;
    fn cos(self) -> Self;
    fn sin(self) -> Self;
    fn exp(self) -> Self;
    fn real(self) -> f64;
    fn abs(self) -> f64;
}

impl SchlafliNum for f64 {
    fn zero() -> f64 {
        0.0
    }
    fn cos(self) -> f64 {
        f64::cos(self)
    }
    fn sin(self) -> f64 {
        f64::sin(self)
    }
    fn exp(self) -> f64 {
        f64::exp(self)
    }
    fn real(self) -> f64 {
        self
    }
    fn abs(self) -> f64 {
        f64::abs(self)
    }
}

impl SchlafliNum for C {
    fn zero() -> C {
        C::new(0.0, 0.0)
    }
    fn cos(self) -> C {
        C::cos(self)
    }
    fn sin(self) -> C {
        C::sin(self)
    }
    fn exp(self) -> C {
        C::exp(self)
    }
    fn real(self) -> f64 {
        self.re
    }
    fn abs(self) -> f64 {
        self.norm()
    }
}

/// [a, b] の 10 点ガウス・ルジャンドル積分（f は 2 つの値を同時に返す）
fn gauss2<T: SchlafliNum>(f: &impl Fn(f64) -> (T, T), a: f64, b: f64) -> (T, T) {
    let (mid, half) = ((a + b) / 2.0, (b - a) / 2.0);
    let (mut s1, mut s2) = (T::zero(), T::zero());
    for (x, w) in GL_X.iter().zip(GL_W.iter()) {
        for t in [mid - half * x, mid + half * x] {
            let (u, v) = f(t);
            s1 += u * *w;
            s2 += v * *w;
        }
    }
    (s1 * half, s2 * half)
}

/// シュレーフリの積分表示で (J_ν(z), Y_ν(z)) を計算する（Re z > 0）。
/// 級数の桁落ちが大きく、漸近展開もまだ使えない中くらいの |z| 用。ν, z が実数なら `T = f64` で呼ぶ
pub(crate) fn schlafli<T: SchlafliNum>(nu: T, z: T) -> (T, T) {
    // ∫₀^π cos(ντ − z sin τ) dτ と ∫₀^π sin(z sin τ − ντ) dτ
    let panels = ((z.abs() + nu.abs()) / 2.0).ceil() as usize + 4;
    let width = PI / panels as f64;
    let osc = |t: f64| {
        let th = nu * t - z * t.sin();
        (th.cos(), -th.sin())
    };
    let (mut ic, mut is) = (T::zero(), T::zero());
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
    let (mut ta, mut tb) = (T::zero(), T::zero());
    let mut top = f64::NEG_INFINITY;
    for k in 0..400 {
        let (t0, t1) = (k as f64 * 0.25, (k + 1) as f64 * 0.25);
        let (a, b) = gauss2(&tail, t0, t1);
        ta += a;
        tb += b;
        let level = nu.real().abs() * t1 - z.real() * t1.sinh();
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
    if nu.im == 0.0 && nu.re < 0.0 {
        let mu = -nu.re;
        // Y_{−n} = (−1)ⁿ Yₙ、Y_{−μ} = cos(μπ) Y_μ + sin(μπ) J_μ（sin・cos は整数・半整数でちょうど 0。#203: Y_{−2.5}(0.01) は J_{2.5}(0.01)。
        // (cos(νπ) J_ν − J_{−ν}) / sin(νπ) では、cos(−2.5π) ≈ 10⁻¹⁶ に大きな J_{2.5} が掛かって相対誤差 10⁻⁴ になっていた）
        if mu == mu.round() {
            let f = if mu % 2.0 == 1.0 { -1.0 } else { 1.0 };
            return f * bessel_y(C::new(mu, 0.0), z);
        }
        let (s, c) = (super::sin_pi(C::new(mu, 0.0)).re, super::sin_pi(C::new(mu + 0.5, 0.0)).re);
        let mut v = c * bessel_y(C::new(mu, 0.0), z);
        if s != 0.0 {
            v += s * bessel_j(C::new(mu, 0.0), z);
        }
        return v;
    }
    // 実数の次数 ν ≥ 2 は、Y_μ・Y_{μ+1}（0 ≤ μ < 1）から漸化式 Y_{k+1} = (2k/z) Y_k − Y_{k−1} で上げる（#153）。
    // Y は漸化式の優勢な解なので、上げる向きは安定。シュレーフリの積分は、虚部の大きい z で e^|Im z| の打ち消し合いになっていた
    if nu.im == 0.0 && nu.re >= 2.0 && nu.re <= 100_000.0 {
        // 虚部が大きい z では、漸化式の優勢な解はハンケル関数 H⁽¹⁾（Im z > 0）で、Y を上げると誤差が e^(2|Im z|) 倍ほどになる。
        // そこでは Y = ±iJ − (2/π) e^(∓iνπ/2) K_ν(∓iz)（±Im z > 0）。K は K の漸化式で（Y₅₀(1 + 30i) が 10¹⁰ 倍ずれていた）
        if z.im.abs() > 3.0 {
            let s = z.im.signum();
            let i = C::new(0.0, 1.0);
            return s * i * bessel_j(nu, z) - 2.0 / PI * (-s * i * nu * (PI / 2.0)).exp() * bessel_k(nu, -s * i * z);
        }
        let n = nu.re.floor();
        let mu = nu.re - n;
        let (mut a, mut b) = (bessel_y(C::new(mu, 0.0), z), bessel_y(C::new(mu + 1.0, 0.0), z));
        for k in 1..n as usize {
            let c = 2.0 * (mu + k as f64) / z * b - a;
            a = b;
            b = c;
        }
        return b;
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
    if nu.im == 0.0 && nu.re < 0.0 {
        return bessel_k(-nu, z); // K_{−ν} = K_ν
    }
    // 実数の次数 ν ≥ 2 は、K_μ・K_{μ+1}（0 ≤ μ < 1）から漸化式 K_{k+1} = K_{k−1} + (2k/z) K_k で上げる（#153。K も優勢な解）。
    // 台形公式の積分は、次数が大きいと cosh(νt) が大きく、振動する被積分関数の打ち消し合いになっていた
    if nu.im == 0.0 && nu.re >= 2.0 && nu.re <= 100_000.0 {
        let n = nu.re.floor();
        let mu = nu.re - n;
        let (mut a, mut b) = (bessel_k(C::new(mu, 0.0), z), bessel_k(C::new(mu + 1.0, 0.0), z));
        for k in 1..n as usize {
            let c = a + 2.0 * (mu + k as f64) / z * b;
            a = b;
            b = c;
        }
        return b;
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

/// |z| ≤ 5 の級数での (Ai, Bi)
fn airy_series(z: C) -> (C, C) {
    let s3 = 3f64.sqrt();
    let w = z * z * z / 9.0;
    let f = hyp0f1(C::new(2.0 / 3.0, 0.0), w);
    let g = z * hyp0f1(C::new(4.0 / 3.0, 0.0), w);
    (AIRY_C1 * f - AIRY_C2 * g, s3 * (AIRY_C1 * f + AIRY_C2 * g))
}

/// |z| ≤ 5 の級数での (Ai′, Bi′)
fn airy_prime_series(z: C) -> (C, C) {
    let s3 = 3f64.sqrt();
    let w = z * z * z / 9.0;
    let fp = z * z / 2.0 * hyp0f1(C::new(5.0 / 3.0, 0.0), w);
    let gp = hyp0f1(C::new(4.0 / 3.0, 0.0), w) + z * z * z / 4.0 * hyp0f1(C::new(7.0 / 3.0, 0.0), w);
    (AIRY_C1 * fp - AIRY_C2 * gp, s3 * (AIRY_C1 * fp + AIRY_C2 * gp))
}

/// 漸近展開を使う |z| の下限（ζ = 2/3 z^(3/2) ≥ 15 で、一番小さい項が 10⁻¹³ ほど）
const AIRY_FAR: f64 = 8.0;

/// 漸近展開の係数 u_k（derivative なら v_k = −(6k + 1)/(6k − 1) u_k）を、項が一番小さくなる所まで
fn airy_coefficients(zeta: C, derivative: bool) -> Vec<C> {
    let mut out = Vec::new();
    let mut u = 1.0f64;
    let mut last = f64::INFINITY;
    for k in 0..80 {
        if k > 0 {
            let kf = k as f64;
            u *= (6.0 * kf - 5.0) * (6.0 * kf - 3.0) * (6.0 * kf - 1.0) / (216.0 * kf * (2.0 * kf - 1.0));
        }
        let c = if derivative && k > 0 { -(6.0 * k as f64 + 1.0) / (6.0 * k as f64 - 1.0) * u } else { u };
        let t = c / zeta.powi(k as i32);
        if t.norm() > last && k > 2 {
            break;
        }
        last = t.norm();
        out.push(t);
        if last < 1e-17 {
            break;
        }
    }
    out
}

/// |z| ≥ [`AIRY_FAR`] の Ai(z)（derivative なら Ai′(z)）の漸近展開（DLMF 9.7.5–9.7.10）。
/// |arg z| ≤ 2π/3 は e^(−ζ) の形、それより負の実軸に近い所は Ai(−w) の振動の形
fn airy_far(z: C, derivative: bool) -> C {
    let sqrt_pi = PI.sqrt();
    if z.arg().abs() <= 2.0 * PI / 3.0 {
        let zeta = 2.0 / 3.0 * z.powf(1.5);
        let terms = airy_coefficients(zeta, derivative);
        let sum: C = terms.iter().enumerate().map(|(k, t)| if k % 2 == 0 { *t } else { -*t }).sum();
        let q = z.powf(0.25);
        return if derivative { -q * (-zeta).exp() / (2.0 * sqrt_pi) * sum } else { (-zeta).exp() / (2.0 * sqrt_pi * q) * sum };
    }
    let w = -z;
    let zeta = 2.0 / 3.0 * w.powf(1.5);
    let terms = airy_coefficients(zeta, derivative);
    let (mut even, mut odd) = (C::new(0.0, 0.0), C::new(0.0, 0.0));
    for (k, t) in terms.iter().enumerate() {
        let sign = if (k / 2) % 2 == 0 { 1.0 } else { -1.0 };
        if k % 2 == 0 {
            even += sign * t;
        } else {
            odd += sign * t;
        }
    }
    let (s, c) = ((zeta - PI / 4.0).sin(), (zeta - PI / 4.0).cos());
    let q = w.powf(0.25);
    if derivative {
        q / sqrt_pi * (s * even - c * odd)
    } else {
        (c * even + s * odd) / (sqrt_pi * q)
    }
}

/// 複素数の z の Ai(z)（derivative なら Ai′(z)）。|z| ≥ 8 は漸近展開、5 < |z| < 8 は |arg z| < 0.95 で K_{1/3}・K_{2/3}、
/// それ以外は級数（虚軸の近くでは Ai が大きくなるので、級数の打ち消し合いは小さい）
fn airy_ai_complex(z: C, derivative: bool) -> C {
    if z.norm() >= AIRY_FAR {
        return airy_far(z, derivative);
    }
    if z.norm() > 3.0 && z.arg().abs() < 0.95 {
        let zeta = 2.0 / 3.0 * z.powf(1.5);
        return if derivative {
            -z / (PI * 3f64.sqrt()) * bessel_k(C::new(2.0 / 3.0, 0.0), zeta)
        } else {
            (z / 3.0).sqrt() / PI * bessel_k(C::new(1.0 / 3.0, 0.0), zeta)
        };
    }
    if derivative { airy_prime_series(z).0 } else { airy_series(z).0 }
}

/// 複素数の z の Bi(z)（derivative なら Bi′(z)）: Bi(z) = e^(iπ/6) Ai(zω) + e^(−iπ/6) Ai(zω̄)（ω = e^(2πi/3)）、
/// Bi′(z) = e^(5iπ/6) Ai′(zω) + e^(−5iπ/6) Ai′(zω̄)
fn airy_bi_complex(z: C, derivative: bool) -> C {
    let omega = C::from_polar(1.0, 2.0 * PI / 3.0);
    let phase = C::from_polar(1.0, if derivative { 5.0 * PI / 6.0 } else { PI / 6.0 });
    phase * airy_ai_complex(z * omega, derivative) + phase.conj() * airy_ai_complex(z * omega.conj(), derivative)
}

/// エアリー関数の組 (Ai(z), Bi(z))
pub fn airy(z: C) -> (C, C) {
    // 減衰する向き（|arg z| < 0.95）で |z| > 3 は、級数だと Ai が小さく項が打ち消し合うので、K_{1/3} で（Bi は級数のまま）
    if z.norm() <= 5.0 {
        let (ai, bi) = airy_series(z);
        if z.norm() > 3.0 && z.arg().abs() < 0.95 {
            return (airy_ai_complex(z, false), bi);
        }
        return (ai, bi);
    }
    // 複素数の z（#151）: 漸近展開と接続公式。K_{1/3}(ζ) は虚軸の近く（arg ζ が π/2 を超える所）で、
    // I_ν の大きな値との打ち消し合いになり、Ai(0.5 + 50i) が 10¹⁴ 倍ずれていた
    if z.im != 0.0 {
        return (airy_ai_complex(z, false), airy_bi_complex(z, false));
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
        let (ai, bi) = airy_prime_series(z);
        if z.norm() > 3.0 && z.arg().abs() < 0.95 {
            return (airy_ai_complex(z, true), bi);
        }
        return (ai, bi);
    }
    if z.im != 0.0 {
        return (airy_ai_complex(z, true), airy_bi_complex(z, true));
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

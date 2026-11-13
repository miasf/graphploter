//! 指数積分・対数積分・三角積分と、誤差関数の仲間（erfi、ドーソン関数、フレネル積分）。

use super::hyper::nonnegative_int;
use super::{erf, gamma_upper};
use num_complex::Complex64 as C;
use std::f64::consts::PI;

/// オイラーの定数 γ
pub const EULER_GAMMA: f64 = 0.577_215_664_901_532_9;

/// E₁(z) の級数 −γ − ln z − Σ (−z)ᵏ / (k · k!)
fn e1_series(z: C) -> C {
    let mut term = C::new(1.0, 0.0);
    let mut sum = C::new(0.0, 0.0);
    for k in 1..2000 {
        term *= -z / k as f64;
        let t = term / k as f64;
        sum += t;
        if t.norm() < 1e-17 * sum.norm().max(1e-300) {
            break;
        }
    }
    -EULER_GAMMA - z.ln() - sum
}

/// E₁(z) の連分数（収束しなければ None）
fn e1_cf(z: C) -> Option<C> {
    let tiny = 1e-300;
    let mut b = z + 1.0;
    let mut c = C::new(1.0 / tiny, 0.0);
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..3000 {
        let an = -((i * i) as f64);
        b += 2.0;
        d = an * d + b;
        if d.norm() < tiny {
            d = C::new(tiny, 0.0);
        }
        c = b + an / c;
        if c.norm() < tiny {
            c = C::new(tiny, 0.0);
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).norm() < 1e-16 {
            return Some(h * (-z).exp());
        }
    }
    None
}

/// 指数積分 E₁(z) = ∫_z^∞ e^(−t) / t dt
pub fn e1(z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return C::new(f64::INFINITY, 0.0);
    }
    if z.norm() <= 4.0 {
        return e1_series(z);
    }
    e1_cf(z).unwrap_or_else(|| e1_series(z))
}

/// 一般化指数積分 Eₙ(z) = ∫₁^∞ e^(−zt) / tⁿ dt = z^(n−1) Γ(1 − n, z)
pub fn expint_e(n: C, z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return if n.re > 1.0 {
            1.0 / (n - 1.0)
        } else {
            C::new(f64::INFINITY, 0.0)
        };
    }
    if nonnegative_int(n) == Some(1) {
        return e1(z);
    }
    z.powc(n - 1.0) * gamma_upper(1.0 - n, z)
}

/// 指数積分 Ei(z) = −PV∫_{−z}^∞ e^(−t) / t dt
pub fn ei(z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return C::new(f64::NEG_INFINITY, 0.0);
    }
    // Ei(z) = −E₁(−z) + (ln z − ln(1/z)) / 2 − ln(−z)
    -e1(-z) + (z.ln() - (1.0 / z).ln()) / 2.0 - (-z).ln()
}

/// 対数積分 li(z) = Ei(ln z)
pub fn li(z: C) -> C {
    if z == C::new(1.0, 0.0) {
        return C::new(f64::NEG_INFINITY, 0.0);
    }
    if z == C::new(0.0, 0.0) {
        return z;
    }
    ei(z.ln())
}

/// Cin(z) = ∫₀ᶻ (1 − cos t) / t dt（整関数で偶関数）
fn cin(z: C) -> C {
    if z.norm() <= 8.0 {
        // −Σ_{k≥1} (−z²)ᵏ / (2k · (2k)!)
        let z2 = -z * z;
        let mut term = C::new(1.0, 0.0);
        let mut sum = C::new(0.0, 0.0);
        for k in 1..500 {
            let kf = k as f64;
            term *= z2 / ((2.0 * kf - 1.0) * (2.0 * kf));
            let t = term / (2.0 * kf);
            sum += t;
            if t.norm() < 1e-17 * sum.norm().max(1e-300) {
                break;
            }
        }
        return -sum;
    }
    if z.re == 0.0 {
        // Cin(iy) = γ + ln|y| − Chi(|y|)
        let y = C::new(z.im.abs(), 0.0);
        return EULER_GAMMA + y.ln() - chi(y);
    }
    // Re w > 0 の側で Ci(w) = −(E₁(iw) + E₁(−iw)) / 2 を使い、Cin(w) = γ + ln w − Ci(w)
    let w = if z.re > 0.0 { z } else { -z };
    let i = C::new(0.0, 1.0);
    let ci = -(e1(i * w) + e1(-i * w)) / 2.0;
    EULER_GAMMA + w.ln() - ci
}

/// Si の級数 Σ (−1)ᵏ z^(2k+1) / ((2k+1) (2k+1)!)（sign = −1）と Shi の級数（sign = +1）
fn sinint_series(z: C, sign: f64) -> C {
    let z2 = sign * z * z;
    let mut term = z;
    let mut sum = z;
    for k in 1..500 {
        let kf = k as f64;
        term *= z2 / ((2.0 * kf) * (2.0 * kf + 1.0));
        let t = term / (2.0 * kf + 1.0);
        sum += t;
        if t.norm() < 1e-17 * sum.norm() {
            break;
        }
    }
    sum
}

/// 正弦積分 Si(z) = ∫₀ᶻ sin t / t dt
pub fn si(z: C) -> C {
    if z.norm() <= 8.0 {
        return sinint_series(z, -1.0);
    }
    if z.re == 0.0 {
        // Si(iy) = i Shi(y)
        return C::new(0.0, 1.0) * shi(C::new(z.im, 0.0));
    }
    if z.re < 0.0 {
        return -si(-z);
    }
    let i = C::new(0.0, 1.0);
    PI / 2.0 + (e1(i * z) - e1(-i * z)) / (2.0 * i)
}

/// 余弦積分 Ci(z) = γ + ln z − Cin(z)
pub fn ci(z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return C::new(f64::NEG_INFINITY, 0.0);
    }
    EULER_GAMMA + z.ln() - cin(z)
}

/// E₁(−z) の対数の枝のずれを打ち消す項 i (arg(−z) − arg z) / 2（Re z > 0 用）
fn branch_fix(z: C) -> C {
    C::new(0.0, ((-z).arg() - z.arg()) / 2.0)
}

/// 双曲線正弦積分 Shi(z) = ∫₀ᶻ sinh t / t dt
pub fn shi(z: C) -> C {
    if z.norm() <= 8.0 {
        return sinint_series(z, 1.0);
    }
    if z.re == 0.0 {
        // Shi(iy) = i Si(y)
        return C::new(0.0, 1.0) * si(C::new(z.im, 0.0));
    }
    if z.re < 0.0 {
        return -shi(-z);
    }
    (e1(z) - e1(-z)) / 2.0 - branch_fix(z)
}

/// 双曲線余弦積分 Chi(z) = γ + ln z − Cin(iz)
pub fn chi(z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return C::new(f64::NEG_INFINITY, 0.0);
    }
    if z.norm() <= 8.0 || z.re == 0.0 {
        return EULER_GAMMA + z.ln() - cin(C::new(0.0, 1.0) * z);
    }
    if z.re < 0.0 {
        // Cin(iz) は偶関数なので Chi(z) = ln z − ln(−z) + Chi(−z)
        return z.ln() - (-z).ln() + chi(-z);
    }
    -(e1(z) + e1(-z)) / 2.0 - branch_fix(z)
}

/// 虚数誤差関数 erfi(z) = −i erf(iz)
pub fn erfi(z: C) -> C {
    let i = C::new(0.0, 1.0);
    -i * erf(i * z)
}

/// ドーソン関数 F(z) = e^(−z²) ∫₀ᶻ e^(t²) dt = √π / 2 · e^(−z²) erfi(z)
pub fn dawson(z: C) -> C {
    if z.norm() > 6.0 && z.im.abs() < 0.5 * z.re.abs() {
        // 漸近展開 F(z) ≈ 1/(2z) Σ (2k − 1)!! / (2z²)ᵏ（項が一番小さくなる所まで）
        let w = 1.0 / (2.0 * z * z);
        let mut term = C::new(1.0, 0.0);
        let mut sum = term;
        let mut last = f64::INFINITY;
        for k in 1..100 {
            let next = term * (2.0 * k as f64 - 1.0) * w;
            if next.norm() >= last || next.norm() < 1e-17 * sum.norm() {
                break;
            }
            last = next.norm();
            term = next;
            sum += term;
        }
        return sum / (2.0 * z);
    }
    PI.sqrt() / 2.0 * (-z * z).exp() * erfi(z)
}

/// erf((1 ± i) √π z / 2) の組
fn fresnel_parts(z: C) -> (C, C) {
    let s = PI.sqrt() / 2.0 * z;
    (erf(C::new(1.0, 1.0) * s), erf(C::new(1.0, -1.0) * s))
}

/// フレネル積分 S(z) = ∫₀ᶻ sin(π t² / 2) dt
pub fn fresnel_s(z: C) -> C {
    let (p, m) = fresnel_parts(z);
    C::new(0.25, 0.25) * (p - C::new(0.0, 1.0) * m)
}

/// フレネル積分 C(z) = ∫₀ᶻ cos(π t² / 2) dt
pub fn fresnel_c(z: C) -> C {
    let (p, m) = fresnel_parts(z);
    C::new(0.25, -0.25) * (p + C::new(0.0, 1.0) * m)
}

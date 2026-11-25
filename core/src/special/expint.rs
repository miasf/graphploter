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
    // 負の実数（切れ目の上）は級数（項はすべて同じ符号で、桁落ちしない）。連分数はここでは収束せず、
    // 3000 回まわしてから級数に戻っていた（1 回 40 µs ほど）
    // 負の実軸の近く（|z| + Re z < 8）も級数にする。連分数はそこでは収束が遅く（Ei(x + 0.5i) で 1 回 70 µs）、
    // 級数は項の向きがそろうので、桁落ちは e^(|z| + Re z) 倍（3000 倍）まで
    if z.norm() <= 4.0 || (z.re < 0.0 && z.norm() + z.re < 8.0) {
        return e1_series(z);
    }
    e1_cf(z).unwrap_or_else(|| e1_series(z))
}

/// 一般化指数積分 Eₙ(z) = ∫₁^∞ e^(−zt) / tⁿ dt = z^(n−1) Γ(1 − n, z)
pub fn expint_e(n: C, z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return if n.re > 1.0 { 1.0 / (n - 1.0) } else { C::new(f64::INFINITY, 0.0) };
    }
    if nonnegative_int(n) == Some(1) {
        return e1(z);
    }
    // Re z > 0、|z| > 1 は Eₙ の連分数 e^(−z) / (z + n − 1·n / (z + n + 2 − 2(n + 1) / (z + n + 4 − …)))（#155）。
    // z^(n−1) Γ(1 − n, z) は、n が整数だと Γ(0, z) = E₁(z) から漸化式で上げていき、桁が消えていた（E₂₀(100) が 18 万倍）
    if z.re > 0.0 && z.norm() > 1.0 {
        if let Some(v) = en_cf(n, z) {
            return v;
        }
    }
    z.powc(n - 1.0) * gamma_upper(1.0 - n, z)
}

/// Eₙ(z) の連分数（Lentz の方法。収束しなければ None）
pub(super) fn en_cf(n: C, z: C) -> Option<C> {
    let tiny = 1e-300;
    let mut b = z + n;
    let mut c = C::new(1.0 / tiny, 0.0);
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..5000 {
        let fi = i as f64;
        let an = -fi * (n - 1.0 + fi);
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

/// 指数積分 Ei(z) = −PV∫_{−z}^∞ e^(−t) / t dt
pub fn ei(z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return C::new(f64::NEG_INFINITY, 0.0);
    }
    // 正の実数は実数で直接計算する。下の式は E₁ を切れ目（負の実軸）の上で使うので、遅いうえに、x ≳ 40 では
    // 連分数が切れ目の分（iπ）を落とした値を返し、虚部に π が残っていた
    if z.im == 0.0 && z.re > 0.0 {
        return C::new(ei_positive(z.re), 0.0);
    }
    // 負の実数: Ei(x) = −E₁(−x)（実数）
    if z.im == 0.0 {
        return C::new(-e1(C::new(-z.re, 0.0)).re, 0.0);
    }
    // Ei(z) = −E₁(−z) + (ln z − ln(1/z)) / 2 − ln(−z)。対数の部分は、実軸の外では虚部の符号で決まる定数 ±iπ。
    // 対数のまま計算すると ln|z| ほどの値の足し引きの丸め誤差が残り、Ei の小さい所（Re z が負で大きい所）では
    // 値そのものを消していた（Ei(−50) が 0 になった）
    let branch = C::new(0.0, if z.im > 0.0 { PI } else { -PI });
    -e1(-z) + branch
}

#[cfg(test)]
fn ei_by_logs(z: C) -> C {
    -e1(-z) + (z.ln() - (1.0 / z).ln()) / 2.0 - (-z).ln()
}

#[cfg(test)]
mod ei_tests {
    use super::*;
    #[test]
    fn ei_branch_constant_matches_logs() {
        for &(re, im) in &[(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (3.0, -0.5), (-30.0, 2.0), (0.2, 5.0), (10.0, 0.1), (-0.5, -0.01)] {
            let z = C::new(re, im);
            let (a, b) = (ei(z), ei_by_logs(z));
            assert!((a - b).norm() <= 1e-12 * b.norm().max(1.0), "{z}: {a} vs {b}");
        }
        // 負の実数・正の実数（参照値）
        assert!((ei(C::new(-50.0, 0.0)).re / -3.783264029550459e-24 - 1.0).abs() < 1e-13);
        assert!((ei(C::new(-10.0, 0.0)).re / -4.156968929685324e-6 - 1.0).abs() < 1e-14);
        assert!((ei(C::new(1.0, 0.0)).re / 1.8951178163559368 - 1.0).abs() < 1e-15);
        assert!((ei(C::new(50.0, 0.0)).re / 1.0585636897131690e20 - 1.0).abs() < 1e-14);
        assert_eq!(ei(C::new(50.0, 0.0)).im, 0.0);
    }
}

/// Ei(x)（x > 0）。x ≤ 40 は級数 γ + ln x + Σ xᵏ / (k · k!)（項はすべて正）、それより大きいときは
/// 漸近展開 eˣ / x · Σ k! / xᵏ（いちばん小さい項で止める。x > 40 なら誤差は e⁻ˣ ほど）
fn ei_positive(x: f64) -> f64 {
    if x <= 40.0 {
        let mut term = 1.0;
        let mut sum = 0.0;
        for k in 1..500 {
            let kf = k as f64;
            term *= x / kf;
            let t = term / kf;
            sum += t;
            if t < 1e-17 * sum {
                break;
            }
        }
        return EULER_GAMMA + x.ln() + sum;
    }
    let mut term = 1.0;
    let mut sum = 1.0;
    for k in 1..200 {
        let next = term * k as f64 / x;
        if next > term {
            break;
        }
        term = next;
        sum += term;
        if term < 1e-17 * sum {
            break;
        }
    }
    // eˣ が桁あふれしても（x > 709）、eˣ / x が有限な所までは出す
    (x - x.ln()).exp() * sum
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
    if z.im == 0.0 {
        return C::new(erfi_real(z.re), 0.0);
    }
    let i = C::new(0.0, 1.0);
    -i * erf(i * z)
}

/// erfi(x)（実数）。複素数の erf(ix) を通すと 1 回 10〜30 µs かかっていた。
/// |x| ≤ 6 は級数 2/√π Σ x^(2k+1) / (k! (2k+1))（項はすべて同じ符号）、それより外は 2/√π · e^(x²) · F(x)
/// （F はドーソン関数の漸近展開。x² > 36 なら誤差は e^(−x²) ほど）。|x| > 26.7 ほどで桁あふれする
fn erfi_real(x: f64) -> f64 {
    let c = 2.0 / PI.sqrt();
    if x.abs() <= 6.0 {
        let x2 = x * x;
        let mut term = x; // x^(2k+1) / k!
        let mut sum = x;
        for k in 1..300 {
            let kf = k as f64;
            term *= x2 / kf;
            let t = term / (2.0 * kf + 1.0);
            sum += t;
            if t.abs() < 1e-17 * sum.abs() {
                break;
            }
        }
        return c * sum;
    }
    let f = dawson(C::new(x, 0.0)).re;
    let e = (x * x).exp();
    if e.is_finite() {
        c * e * f
    } else {
        f64::INFINITY.copysign(x)
    }
}

#[cfg(test)]
mod erfi_tests {
    use super::*;
    #[test]
    fn erfi_real_matches_complex() {
        let i = C::new(0.0, 1.0);
        let mut x = -26.0;
        while x <= 26.0 {
            let a = erfi_real(x);
            let b = (-i * erf(i * C::new(x, 0.0))).re;
            assert!((a - b).abs() <= 1e-13 * b.abs().max(1e-300), "erfi({x}): {a} vs {b}");
            x += 0.173;
        }
        assert!((erfi_real(1.0) - 1.6504257587975428).abs() < 1e-15);
        assert_eq!(erfi_real(30.0), f64::INFINITY);
        assert_eq!(erfi_real(-30.0), f64::NEG_INFINITY);
    }

    #[test]
    fn e1_series_near_negative_axis_matches_cf() {
        for &(re, im) in &[(-5.0, 2.0), (-20.0, 4.0), (-60.0, 5.0), (-3.0, -3.5), (-100.0, -6.0)] {
            let z = C::new(re, im);
            if z.norm() + z.re >= 8.0 {
                continue;
            }
            let (a, b) = (e1_series(z), e1_cf(z).expect("連分数が収束しない"));
            assert!((a - b).norm() <= 1e-11 * b.norm(), "E1({z}): {a} vs {b}");
        }
    }
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

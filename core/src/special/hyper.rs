//! 超幾何関数 ₀F₁, ₁F₁, ₂F₁ とトリコミの U。
//!
//! ほかの特殊関数（直交多項式、エアリー関数、不完全ベータ関数など）もこれを使って計算する。

use super::{gamma, rgamma};
use std::f64::consts::PI;
use num_complex::Complex64 as C;

const NAN: C = C::new(f64::NAN, f64::NAN);

/// 0 以下の整数なら、その値
pub(crate) fn nonpositive_int(z: C) -> Option<i64> {
    (z.im == 0.0 && z.re <= 0.0 && z.re == z.re.round() && z.re > -1e7).then_some(z.re as i64)
}

/// 0 以上の整数なら、その値
pub(crate) fn nonnegative_int(z: C) -> Option<u64> {
    (z.im == 0.0 && z.re >= 0.0 && z.re == z.re.round() && z.re < 1e7).then_some(z.re as u64)
}

/// 一般の超幾何級数 Σ (a₁)ₖ…(a_p)ₖ / ((b₁)ₖ…(b_q)ₖ) · zᵏ / k!。
/// 項がちょうど 0 になったら（多項式）そこで終わる。`max_terms` までに収束しなければ NaN
pub fn pfq(a: &[C], b: &[C], z: C, max_terms: usize) -> C {
    let mut term = C::new(1.0, 0.0);
    let mut sum = term;
    for k in 0..max_terms {
        let kf = k as f64;
        let mut ratio = z / (kf + 1.0);
        for ai in a {
            ratio *= ai + kf;
        }
        for bi in b {
            ratio /= bi + kf;
        }
        term *= ratio;
        sum += term;
        if term == C::new(0.0, 0.0) || (k > 2 && term.norm() <= 1e-16 * sum.norm()) {
            return sum;
        }
        if !sum.norm().is_finite() {
            return sum;
        }
    }
    NAN
}

/// 引数 x がちょうど「計算できない値」（整数など）のとき、x ± δ と x ± 2δ の値から
/// リチャードソン補外で x での値を近似する（誤差は δ⁴ 程度）
pub(crate) fn nudge(f: impl Fn(C) -> C, x: C) -> C {
    let d = 2e-4 * x.norm().max(1.0);
    let near = (f(x + d) + f(x - d)) / 2.0;
    let far = (f(x + 2.0 * d) + f(x - 2.0 * d)) / 2.0;
    (4.0 * near - far) / 3.0
}

/// ₀F₁(; b; z)
pub fn hyp0f1(b: C, z: C) -> C {
    pfq(&[], &[b], z, 3000)
}

/// ₁F₁(a; b; z)（クンマーの合流型超幾何関数 M）
pub fn hyp1f1(a: C, b: C, z: C) -> C {
    // 多項式（a が 0 以下の整数）ならそのまま足す
    if nonpositive_int(a).is_some() {
        return pfq(&[a], &[b], z, 100_000);
    }
    // Re z < 0 では打ち消し合いで精度が落ちるので、クンマー変換 M(a, b, z) = eᶻ M(b − a, b, −z) を使う
    if z.re < 0.0 {
        return z.exp() * pfq(&[b - a], &[b], -z, 5000);
    }
    pfq(&[a], &[b], z, 5000)
}

/// ₂F₁(a, b; c; z)（ガウスの超幾何関数）。|z| < 1 の外は変換公式でつなぐ
pub fn hyp2f1(a: C, b: C, c: C, z: C) -> C {
    // 多項式（a か b が 0 以下の整数）
    if nonpositive_int(a).is_some() || nonpositive_int(b).is_some() {
        return pfq(&[a, b], &[c], z, 100_000);
    }
    if z == C::new(1.0, 0.0) {
        // ガウスの公式 ₂F₁(a, b; c; 1) = Γ(c) Γ(c − a − b) / (Γ(c − a) Γ(c − b))（Re(c − a − b) > 0）
        return gamma(c) * gamma(c - a - b) * rgamma(c - a) * rgamma(c - b);
    }
    // 実軸上の分岐切断 (1, ∞) では、Wolfram と同じく下側からの値にそろえる
    let z = if z.im == 0.0 && z.re > 1.0 { C::new(z.re, -0.0) } else { z };
    let one = C::new(1.0, 0.0);
    if z.norm() < 0.8 {
        return pfq(&[a, b], &[c], z, 5000);
    }
    // パフ変換: ₂F₁(a, b; c; z) = (1 − z)^(−a) ₂F₁(a, c − b; c; z / (z − 1))
    let w = z / (z - 1.0);
    if w.norm() < 0.8 {
        return (one - z).powc(-a) * pfq(&[a, c - b], &[c], w, 5000);
    }
    // 1 − z の変換（c − a − b が整数のときは少しずらした値の平均）
    if (one - z).norm() < 0.8 {
        let f = |c: C| {
            let s = c - a - b;
            gamma(c) * gamma(s) * rgamma(c - a) * rgamma(c - b) * pfq(&[a, b], &[one - s], one - z, 5000)
                + (one - z).powc(s) * gamma(c) * gamma(-s) * rgamma(a) * rgamma(b)
                    * pfq(&[c - a, c - b], &[s + 1.0], one - z, 5000)
        };
        let s = c - a - b;
        return if s.im == 0.0 && s.re == s.re.round() { nudge(f, c) } else { f(c) };
    }
    // 1 / z の変換（a − b が整数のときは少しずらした値の平均）
    if z.norm() > 1.25 {
        let f = |a: C| {
            let zi = one / z;
            gamma(c) * gamma(b - a) * rgamma(b) * rgamma(c - a)
                * (-z).powc(-a)
                * pfq(&[a, a - c + 1.0], &[a - b + 1.0], zi, 5000)
                + gamma(c) * gamma(a - b) * rgamma(a) * rgamma(c - b)
                    * (-z).powc(-b)
                    * pfq(&[b, b - c + 1.0], &[b - a + 1.0], zi, 5000)
        };
        let d = a - b;
        return if d.im == 0.0 && d.re == d.re.round() { nudge(f, a) } else { f(a) };
    }
    // |z| ≈ 1 の残りの所は、ゆっくり収束する級数で
    pfq(&[a, b], &[c], z, 200_000)
}

/// トリコミの合流型超幾何関数 U(a, b, z)
pub fn hyp_u(a: C, b: C, z: C) -> C {
    let integral_ok = a.re > 0.05 && z.re > 0.0 && z.arg().abs() < 1.4;
    let asymptotic_at = 18.0 + 2.0 * (a.norm() + (a - b + 1.0).norm());
    if integral_ok && z.norm() > 2.0 && z.norm() <= asymptotic_at {
        return hyp_u_integral(a, b, z);
    }
    if z.norm() > asymptotic_at || (!integral_ok && z.norm() > 18.0 + a.norm()) {
        // 漸近展開 U ≈ z^(−a) Σ (a)ₖ (a − b + 1)ₖ / k! · (−1/z)ᵏ（項が一番小さくなる所で止める）
        let mut term = C::new(1.0, 0.0);
        let mut sum = term;
        let mut last = f64::INFINITY;
        for k in 0..200 {
            let kf = k as f64;
            let next = term * (a + kf) * (a - b + 1.0 + kf) / (kf + 1.0) * (-1.0 / z);
            if next.norm() >= last || next.norm() < 1e-17 * sum.norm() {
                break;
            }
            last = next.norm();
            term = next;
            sum += term;
        }
        return z.powc(-a) * sum;
    }
    let f = |b: C| {
        let one = C::new(1.0, 0.0);
        gamma(one - b) * rgamma(a - b + 1.0) * hyp1f1(a, b, z)
            + gamma(b - 1.0) * rgamma(a) * z.powc(one - b) * hyp1f1(a - b + 1.0, 2.0 - b, z)
    };
    if b.im == 0.0 && b.re == b.re.round() {
        nudge(f, b)
    } else {
        f(b)
    }
}

/// U(a, b, z) = 1/Γ(a) ∫₀^∞ e^(−zt) t^(a−1) (1 + t)^(b−a−1) dt を t = eˢ にして台形公式で（Re a > 0, Re z > 0）
fn hyp_u_integral(a: C, b: C, z: C) -> C {
    let d = ((PI / 2.0 - z.arg().abs()) * 0.8).min(PI * 0.8);
    let h = (d / 6.0).min(0.1);
    let c = b - a - 1.0;
    let f = |s: f64| {
        let es = s.exp();
        (-z * es + a * s + c * (1.0 + es).ln()).exp()
    };
    let level = |s: f64| {
        let es = s.exp();
        -z.re * es + a.re * s + c.re * (1.0 + es).ln()
    };
    let s0 = ((a.norm() + 1.0) / z.norm()).ln();
    let mut sum = f(s0);
    let mut top = level(s0);
    for dir in [1.0, -1.0] {
        for k in 1..200_000 {
            let s = s0 + dir * k as f64 * h;
            sum += f(s);
            let l = level(s);
            top = top.max(l);
            if l < top - 40.0 {
                break;
            }
        }
    }
    sum * h * rgamma(a)
}

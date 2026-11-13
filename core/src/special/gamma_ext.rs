//! ガンマ関数・ゼータ関数の仲間:
//! ln Γ、不完全ガンマ関数、不完全ベータ関数、ポリガンマ関数、ポッホハマー記号、
//! フルヴィッツのゼータ関数、多重対数関数、ディリクレのイータ関数。

use super::hyper::{hyp2f1, nonnegative_int, nonpositive_int};
use super::{beta, digamma, gamma, rgamma, zeta};
use crate::special::expint::e1;
use num_complex::Complex64 as C;
use std::f64::consts::PI;

const NAN: C = C::new(f64::NAN, f64::NAN);

/// ln Γ(z)。Re z ≥ 1/2 ではスターリングの公式（z を大きくずらしてから）、それ以外は相反公式
pub fn log_gamma(z: C) -> C {
    if nonpositive_int(z).is_some() {
        return C::new(f64::INFINITY, 0.0); // 極
    }
    if z.re < -5000.0 {
        return NAN;
    }
    // ln Γ(z) = ln Γ(z + 1) − ln z で右へずらす（主枝の対数の和なので、Wolfram と同じ枝になる）
    let mut z = z;
    let mut acc = C::new(0.0, 0.0);
    while z.re < 15.0 {
        acc -= z.ln();
        z += 1.0;
    }
    let z2 = 1.0 / (z * z);
    let series = (1.0 / z)
        * (1.0 / 12.0
            - z2 * (1.0 / 360.0 - z2 * (1.0 / 1260.0 - z2 * (1.0 / 1680.0 - z2 * (1.0 / 1188.0)))));
    acc + (z - 0.5) * z.ln() - z + 0.5 * (2.0 * PI).ln() + series
}

/// ポッホハマー記号 (a)ₙ = Γ(a + n) / Γ(a)。n が 0 以上の整数なら掛け算で正確に
pub fn pochhammer(a: C, n: C) -> C {
    if let Some(k) = nonnegative_int(n) {
        if k <= 2000 {
            let mut p = C::new(1.0, 0.0);
            for i in 0..k {
                p *= a + i as f64;
            }
            return p;
        }
    }
    gamma(a + n) * rgamma(a)
}

/// 下側不完全ガンマ関数の級数 γ(a, z) = zᵃ e^(−z) Σ zᵏ / (a (a+1) … (a+k))
fn gamma_lower_series(a: C, z: C) -> C {
    let mut term = 1.0 / a;
    let mut sum = term;
    for k in 1..5000 {
        term *= z / (a + k as f64);
        sum += term;
        if term.norm() < 1e-17 * sum.norm() {
            break;
        }
    }
    z.powc(a) * (-z).exp() * sum
}

/// 上側不完全ガンマ関数の連分数（|z| が大きく Re z > 0 のとき）
fn gamma_upper_cf(a: C, z: C) -> C {
    let tiny = 1e-300;
    let mut b = z + 1.0 - a;
    let mut c = C::new(1.0 / tiny, 0.0);
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..2000 {
        let an = -(i as f64) * (C::new(i as f64, 0.0) - a);
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
            break;
        }
    }
    (-z).exp() * z.powc(a) * h
}

/// 上側不完全ガンマ関数 Γ(a, z) = ∫_z^∞ t^(a−1) e^(−t) dt（Wolfram の Gamma[a, z]）
pub fn gamma_upper(a: C, z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return gamma(a);
    }
    // a が 0 以下の整数: Γ(0, z) = E₁(z) から、Γ(a, z) = (Γ(a + 1, z) − zᵃ e^(−z)) / a で下げていく
    if let Some(n) = nonpositive_int(a) {
        let mut g = e1(z);
        let ez = (-z).exp();
        for j in (n..0).rev() {
            let jf = j as f64;
            g = (g - z.powf(jf) * ez) / jf;
        }
        return g;
    }
    if z.re > 0.0 && z.norm() > a.norm() + 4.0 {
        gamma_upper_cf(a, z)
    } else {
        gamma(a) - gamma_lower_series(a, z)
    }
}

/// 正則化した下側不完全ガンマ関数 P(a, z) = γ(a, z) / Γ(a)
pub fn gamma_p(a: C, z: C) -> C {
    if nonpositive_int(a).is_some() {
        return C::new(1.0, 0.0); // Γ(a) が無限大なので Q = 0
    }
    if z.re > 0.0 && z.norm() > a.norm() + 4.0 {
        1.0 - gamma_upper(a, z) / gamma(a)
    } else {
        gamma_lower_series(a, z) / gamma(a)
    }
}

/// 正則化した上側不完全ガンマ関数 Q(a, z) = Γ(a, z) / Γ(a)
pub fn gamma_q(a: C, z: C) -> C {
    1.0 - gamma_p(a, z)
}

/// 不完全ベータ関数 B(z; a, b) = ∫₀ᶻ t^(a−1) (1 − t)^(b−1) dt = zᵃ / a · ₂F₁(a, 1 − b; a + 1; z)
pub fn beta_incomplete(z: C, a: C, b: C) -> C {
    z.powc(a) / a * hyp2f1(a, 1.0 - b, a + 1.0, z)
}

/// 正則化した不完全ベータ関数 I_z(a, b) = B(z; a, b) / B(a, b)
pub fn beta_regularized(z: C, a: C, b: C) -> C {
    beta_incomplete(z, a, b) / beta(a, b)
}

/// ポリガンマ関数 ψ⁽ⁿ⁾(z)。n = 0 はディガンマ、n ≥ 1 は (−1)ⁿ⁺¹ n! ζ(n + 1, z)
pub fn polygamma(n: C, z: C) -> C {
    match nonnegative_int(n) {
        Some(0) => digamma(z),
        Some(k) => {
            let sign = if k % 2 == 1 { 1.0 } else { -1.0 };
            sign * gamma(n + 1.0) * hurwitz_zeta(n + 1.0, z)
        }
        None => NAN,
    }
}

/// ベルヌーイ数 B₂, B₄, …, B₂₄
const B2K: [f64; 12] = [
    1.0 / 6.0,
    -1.0 / 30.0,
    1.0 / 42.0,
    -1.0 / 30.0,
    5.0 / 66.0,
    -691.0 / 2730.0,
    7.0 / 6.0,
    -3617.0 / 510.0,
    43867.0 / 798.0,
    -174611.0 / 330.0,
    854513.0 / 138.0,
    -236364091.0 / 2730.0,
];

/// フルヴィッツのゼータ関数 ζ(s, a) = Σ_{k≥0} (k + a)^(−s)。オイラー・マクローリンの和公式で計算する
pub fn hurwitz_zeta(s: C, a: C) -> C {
    if s == C::new(1.0, 0.0) {
        return C::new(f64::INFINITY, 0.0);
    }
    let n = (15.0 + s.norm()).min(400.0) as usize;
    let mut sum = C::new(0.0, 0.0);
    for k in 0..n {
        sum += (a + k as f64).powc(-s);
    }
    let w = a + n as f64;
    sum += w.powc(1.0 - s) / (s - 1.0) + 0.5 * w.powc(-s);
    // 補正項 Σ B₂ⱼ / (2j)! · s (s+1) … (s+2j−2) · w^(−s−2j+1)
    let mut fact = s * w.powc(-s - 1.0); // j = 1 の s · w^(−s−1)
    let mut f2j = 2.0; // (2j)!
    for (j, b) in B2K.iter().enumerate() {
        let j = j + 1;
        let t = b / f2j * fact;
        sum += t;
        if t.norm() < 1e-17 * sum.norm() {
            break;
        }
        let jf = j as f64;
        fact *= (s + 2.0 * jf - 1.0) * (s + 2.0 * jf) / (w * w);
        f2j *= (2.0 * jf + 1.0) * (2.0 * jf + 2.0);
    }
    sum
}

/// ディリクレのイータ関数 η(s) = (1 − 2^(1−s)) ζ(s)
pub fn dirichlet_eta(s: C) -> C {
    if s == C::new(1.0, 0.0) {
        return C::new(2f64.ln(), 0.0);
    }
    (1.0 - C::new(2.0, 0.0).powc(1.0 - s)) * zeta(s)
}

/// ベルヌーイ数 B₀, B₁, …, Bₙ（B₁ = −1/2）
fn bernoulli_numbers(n: usize) -> Vec<f64> {
    // Σ_{k=0}^{m} C(m+1, k) B_k = 0 から順に求める
    let mut b = vec![0.0f64; n + 1];
    b[0] = 1.0;
    for m in 1..=n {
        let mut s = 0.0;
        let mut c = 1.0; // C(m+1, k)
        for (k, bk) in b.iter().enumerate().take(m) {
            s += c * bk;
            c *= (m + 1 - k) as f64 / (k + 1) as f64;
        }
        b[m] = -s / (m + 1) as f64;
    }
    b
}

/// ベルヌーイ多項式 Bₙ(x)
fn bernoulli_poly(n: usize, x: C) -> C {
    let b = bernoulli_numbers(n);
    let mut sum = C::new(0.0, 0.0);
    let mut c = 1.0; // C(n, k)
    for (k, bk) in b.iter().enumerate() {
        sum += c * bk * x.powi((n - k) as i32);
        c *= (n - k) as f64 / (k + 1) as f64;
    }
    sum
}

thread_local! {
    /// 多重対数で使う ζ(s − k)（k = 0, 1, …）を、直前の s について覚えておく
    static ZETA_SHIFTS: std::cell::RefCell<(C, Vec<C>)> = const { std::cell::RefCell::new((NAN, Vec::new())) };
}

fn zeta_shifted(s: C, count: usize) -> Vec<C> {
    ZETA_SHIFTS.with(|cache| {
        let mut c = cache.borrow_mut();
        if c.0 != s || c.1.len() < count {
            c.0 = s;
            c.1 = (0..count).map(|k| zeta(s - k as f64)).collect();
        }
        c.1[..count].to_vec()
    })
}

/// 多重対数関数 Li_s(z) = Σ_{k≥1} zᵏ / kˢ（Wolfram の PolyLog[s, z]）
pub fn polylog(s: C, z: C) -> C {
    if z == C::new(0.0, 0.0) {
        return z;
    }
    // 実軸上の分岐切断 (1, ∞) では下側からの値にそろえる
    let z = if z.im == 0.0 && z.re > 1.0 {
        C::new(z.re, -0.0)
    } else {
        z
    };
    if z.norm() <= 0.5 {
        let mut sum = C::new(0.0, 0.0);
        let mut zk = z;
        for k in 1..2000 {
            let t = zk * C::new(k as f64, 0.0).powc(-s);
            sum += t;
            if t.norm() < 1e-17 * sum.norm() {
                break;
            }
            zk *= z;
        }
        return sum;
    }
    let int_s = nonnegative_int(s).filter(|n| (1..=40).contains(n));
    // s が正の整数で |z| が大きいとき: 逆数の公式
    // Li_n(z) = −(−1)ⁿ Li_n(1/z) − (2πi)ⁿ / n! · Bₙ(1/2 + ln(−z) / (2πi))
    if let Some(n) = int_s {
        if z.norm() >= 2.0 {
            let two_pi_i = C::new(0.0, 2.0 * PI);
            let sign = if n % 2 == 0 { -1.0 } else { 1.0 };
            let nf: f64 = (1..=n).map(|k| k as f64).product();
            return sign * polylog(s, 1.0 / z)
                - two_pi_i.powi(n as i32) / nf
                    * bernoulli_poly(n as usize, 0.5 + (-z).ln() / two_pi_i);
        }
    }
    // μ = ln z の級数（|μ| < 2π）
    let mu = z.ln();
    if mu.norm() >= 2.0 * PI * 0.98 {
        return NAN;
    }
    let terms = 90;
    let zs = zeta_shifted(s, terms);
    let mut sum = C::new(0.0, 0.0);
    let mut muk = C::new(1.0, 0.0); // μᵏ / k!
    match int_s {
        Some(n) => {
            // Li_n(e^μ) = μⁿ⁻¹ / (n−1)! (H_{n−1} − ln(−μ)) + Σ_{k≠n−1} ζ(n − k) μᵏ / k!
            let n = n as usize;
            for (k, zk) in zs.iter().enumerate() {
                if k == n - 1 {
                    let h: f64 = (1..n).map(|j| 1.0 / j as f64).sum();
                    sum += muk * (h - (-mu).ln());
                } else {
                    sum += zk * muk;
                }
                muk *= mu / (k + 1) as f64;
            }
        }
        None => {
            // Li_s(e^μ) = Γ(1 − s) (−μ)^(s−1) + Σ ζ(s − k) μᵏ / k!
            sum += gamma(1.0 - s) * (-mu).powc(s - 1.0);
            for (k, zk) in zs.iter().enumerate() {
                sum += zk * muk;
                muk *= mu / (k + 1) as f64;
            }
        }
    }
    sum
}

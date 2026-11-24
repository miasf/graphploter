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
        * (1.0 / 12.0 - z2 * (1.0 / 360.0 - z2 * (1.0 / 1260.0 - z2 * (1.0 / 1680.0 - z2 * (1.0 / 1188.0)))));
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
    // Re z < 0 は γ(a, z) = zᵃ Σ (−z)ᵏ / (k! (a + k))（#154）。上の形は e^(−z) を外に出すので、z = −50 では
    // 大きさ 10²⁰ ほどの項が打ち消し合って 1/50 ほどになり、桁が全部消えていた
    if z.re < 0.0 {
        let mut term = C::new(1.0, 0.0);
        let mut sum = 1.0 / a;
        for k in 1..5000 {
            term *= -z / k as f64;
            let t = term / (a + k as f64);
            sum += t;
            if t.norm() < 1e-17 * sum.norm() {
                break;
            }
        }
        return power(z, a) * sum;
    }
    let mut term = 1.0 / a;
    let mut sum = term;
    for k in 1..5000 {
        term *= z / (a + k as f64);
        sum += term;
        if term.norm() < 1e-17 * sum.norm() {
            break;
        }
    }
    power(z, a) * (-z).exp() * sum
}

/// zᵃ（主枝）。a が整数なら掛け算で（負の実数 z で虚部のゴミを出さない）
fn power(z: C, a: C) -> C {
    if a.im == 0.0 && a.re.fract() == 0.0 && a.re.abs() <= 1024.0 {
        z.powi(a.re as i32)
    } else {
        z.powc(a)
    }
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
        // Re z > 0、|z| > 1 は Γ(a, z) = zᵃ E_{1−a}(z) の連分数（漸化式は桁が消える。#155）
        if z.re > 0.0 && z.norm() > 1.0 {
            if let Some(v) = crate::special::expint::en_cf(1.0 - a, z) {
                return power(z, a) * v;
            }
        }
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
        1.0 - gamma_upper(a, z) * rgamma(a)
    } else {
        gamma_lower_series(a, z) * rgamma(a)
    }
}

/// 正則化した上側不完全ガンマ関数 Q(a, z) = Γ(a, z) / Γ(a)
pub fn gamma_q(a: C, z: C) -> C {
    // 上側を連分数で求める範囲では、Γ(a, z) / Γ(a) をそのまま返す。1 − P にすると、Q が小さい所（gammaQ(0.5, 100) ≈ 2·10⁻⁴⁵）で
    // 桁が全部消えて 0 になっていた（P のほうも 1 − Γ(a, z)/Γ(a) で計算しているので、二重に引き算していた）
    if nonpositive_int(a).is_none() && z.re > 0.0 && z.norm() > a.norm() + 4.0 {
        return gamma_upper(a, z) * rgamma(a);
    }
    1.0 - gamma_p(a, z)
}

/// 不完全ベータ関数 B(z; a, b) = ∫₀ᶻ t^(a−1) (1 − t)^(b−1) dt = zᵃ / a · ₂F₁(a, 1 − b; a + 1; z)。
/// 0 ≤ z ≤ 1、a, b > 0 の実数は、連分数の I_z(a, b) · B(a, b)（[`beta_real`]）
pub fn beta_incomplete(z: C, a: C, b: C) -> C {
    if let Some(i) = beta_real(z, a, b) {
        return i * beta(a, b);
    }
    z.powc(a) / a * hyp2f1(a, 1.0 - b, a + 1.0, z)
}

/// 正則化した不完全ベータ関数 I_z(a, b) = B(z; a, b) / B(a, b)
pub fn beta_regularized(z: C, a: C, b: C) -> C {
    if let Some(i) = beta_real(z, a, b) {
        return i;
    }
    crate::eval::div(z.powc(a) / a * hyp2f1(a, 1.0 - b, a + 1.0, z), beta(a, b))
}

/// I_x(a, b)（0 ≤ x ≤ 1、a, b > 0 の実数。それ以外は None）。連分数（Lentz の方法）と、
/// x > (a + 1)/(a + b + 2) では I_x(a, b) = 1 − I_{1−x}(b, a)。前の因子 xᵃ (1 − x)ᵇ / (a B(a, b)) は対数で計算する。
/// ₂F₁(a, 1 − b; a + 1; x) の有限和は、a, b が大きいと正負の大きい項が打ち消し合う（#148。I_{0.5}(100, 100) が実数にならなかった）
fn beta_real(z: C, a: C, b: C) -> Option<C> {
    if z.im != 0.0 || a.im != 0.0 || b.im != 0.0 {
        return None;
    }
    let (x, a, b) = (z.re, a.re, b.re);
    if !(0.0..=1.0).contains(&x) || a <= 0.0 || b <= 0.0 || !a.is_finite() || !b.is_finite() {
        return None;
    }
    if x == 0.0 || x == 1.0 || (x == 0.5 && a == b) {
        // 両端と、左右対称な分布の中央（ちょうど 1/2）
        return Some(C::new(x, 0.0));
    }
    let lbeta = |p: f64, q: f64| log_gamma(C::new(p, 0.0)).re + log_gamma(C::new(q, 0.0)).re - log_gamma(C::new(p + q, 0.0)).re;
    let part = |x: f64, a: f64, b: f64| -> Option<f64> {
        let front = (a * x.ln() + b * (-x).ln_1p() - lbeta(a, b)).exp() / a;
        // 連分数 1 / (1 + d₁ / (1 + d₂ / …))
        const TINY: f64 = 1e-300;
        let mut c = 1.0;
        let mut d = 1.0 - (a + b) * x / (a + 1.0);
        d = 1.0 / if d.abs() < TINY { TINY } else { d };
        let mut h = d;
        for m in 1..=10_000 {
            let m = m as f64;
            for k in [m * (b - m) * x / ((a + 2.0 * m - 1.0) * (a + 2.0 * m)), -(a + m) * (a + b + m) * x / ((a + 2.0 * m) * (a + 2.0 * m + 1.0))] {
                d = 1.0 + k * d;
                d = 1.0 / if d.abs() < TINY { TINY } else { d };
                c = 1.0 + k / c;
                if c.abs() < TINY {
                    c = TINY;
                }
                h *= d * c;
            }
            if (d * c - 1.0).abs() < 1e-16 {
                return Some(front * h);
            }
        }
        None
    };
    let v = if x <= (a + 1.0) / (a + b + 2.0) { part(x, a, b)? } else { 1.0 - part(1.0 - x, b, a)? };
    Some(C::new(v, 0.0))
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
    // 負の整数 s = −n は ζ(−n, a) = −B_{n+1}(a) / (n + 1)（#173）。下の和公式では、項 (a + k)ⁿ の大きな和と補正項が打ち消し合い、
    // 小さな答え（ζ(−12, 0.2) = 0.038）が丸め誤差に埋もれていた。a = 1 はリーマンのゼータ（負の偶数でちょうど 0）
    if let Some(m) = nonpositive_int(s) {
        if m >= -60 {
            if a == C::new(1.0, 0.0) {
                return zeta(s);
            }
            let n = (-m) as usize;
            return -bernoulli_poly(n + 1, a) / (n + 1) as f64;
        }
    }
    // Re s ≤ −4、実数の a > 0 は、フルヴィッツの公式（0 < a ≤ 1 にずらしてから）
    // ζ(s, a) = 2Γ(σ) / (2π)^σ · Σ_{k≥1} cos(2πka − πσ/2) / k^σ（σ = 1 − s）。和公式は負の整数の近く（s = −12.0001）でも打ち消し合う。
    // cos を e^(±i(2πka − πσ/2)) に分け、Γ(σ) e^(∓iπσ/2) / (2π)^σ は対数で（虚部が大きい s で Γ と cos が桁あふれしないように）
    if s.re <= -4.0 && a.im == 0.0 && a.re > 0.0 && a.re < 1e6 {
        let shift = (a.re.ceil() - 1.0).max(0.0);
        let a0 = a.re - shift;
        let sigma = 1.0 - s;
        let i = C::new(0.0, 1.0);
        let base = log_gamma(sigma) - sigma * (2.0 * PI).ln();
        let (front_plus, front_minus) = ((base - i * PI / 2.0 * sigma).exp(), (base + i * PI / 2.0 * sigma).exp());
        let (mut plus, mut minus) = (C::new(0.0, 0.0), C::new(0.0, 0.0));
        let terms = (1e-17f64.powf(1.0 / (sigma.re - 1.0)).recip()).clamp(10.0, 100_000.0) as usize;
        for k in 1..=terms {
            let k = k as f64;
            // 2πka は ka の小数部分から（大きい k でも丸め誤差を増やさない）
            let turn = C::from_polar(1.0, 2.0 * PI * (k * a0).fract());
            let pk = C::new(k, 0.0).powc(-sigma);
            plus += turn * pk;
            minus += turn.conj() * pk;
        }
        let mut v = front_plus * plus + front_minus * minus;
        for k in 0..shift as usize {
            v -= C::new(a0 + k as f64, 0.0).powc(-s);
        }
        return v;
    }
    // 補正項の級数が収束するように、w = a + n を |s| / (2π · 0.3) より大きくする（虚部の大きい s。#150）
    let n = (15.0 + s.norm() / (2.0 * PI * 0.3)).min(200_000.0) as usize;
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
    let z = zeta(s);
    if z == C::new(0.0, 0.0) {
        return z; // 負の偶数（−0 にしない）
    }
    (1.0 - C::new(2.0, 0.0).powc(1.0 - s)) * z
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
    let z = if z.im == 0.0 && z.re > 1.0 { C::new(z.re, -0.0) } else { z };
    // s = −n（0 以下の整数）は有理式 Li_{−n}(z) = Σ_{k=0}^{n} k! S(n+1, k+1) (z/(1 − z))^(k+1)（S は第2種スターリング数。#203）。
    // 下の μ = ln z の級数は |ln z| < 2π でしか使えず、Li₀(−100) などが値にならなかった
    if let Some(m) = nonpositive_int(s) {
        if m >= -60 {
            if z == C::new(1.0, 0.0) {
                return C::new(f64::INFINITY, 0.0);
            }
            let n = (-m) as usize;
            // S(n+1, j)（j = 0..=n+1）の表
            let mut stirling = vec![vec![0.0f64; n + 2]; n + 2];
            stirling[0][0] = 1.0;
            for i in 1..=n + 1 {
                for j in 1..=i {
                    stirling[i][j] = j as f64 * stirling[i - 1][j] + stirling[i - 1][j - 1];
                }
            }
            let w = crate::eval::div(z, 1.0 - z);
            let (mut sum, mut wk, mut fact) = (C::new(0.0, 0.0), w, 1.0);
            for k in 0..=n {
                if k > 0 {
                    fact *= k as f64;
                }
                sum += fact * stirling[n + 1][k + 1] * wk;
                wk *= w;
            }
            return sum;
        }
    }
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
                - two_pi_i.powi(n as i32) / nf * bernoulli_poly(n as usize, 0.5 + (-z).ln() / two_pi_i);
        }
    }
    // z = 1 は ζ(s)（下の μ = ln z の級数は μ = 0 で 0 · ∞ になる）。Re s ≤ 1 では発散する
    if z == C::new(1.0, 0.0) {
        return if s.re > 1.0 { zeta(s) } else { NAN };
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

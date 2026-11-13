//! 特殊関数（複素数対応）。今後ここに増やしていく。

mod bessel;
mod elliptic;
mod expint;
mod gamma_ext;
mod hyper;
mod ortho;

pub use bessel::*;
pub use elliptic::*;
pub use expint::*;
pub use gamma_ext::*;
pub use hyper::*;
pub use ortho::*;

use num_complex::Complex64 as C;
use std::f64::consts::PI;

/// Lanczos 近似の係数（g = 7, n = 9）。倍精度でおよそ 15 桁の精度。
const LANCZOS_G: f64 = 7.0;
const LANCZOS_P: [f64; 9] = [
    0.999_999_999_999_809_9,
    676.520_368_121_885_1,
    -1_259.139_216_722_402_8,
    771.323_428_777_653_1,
    -176.615_029_162_140_6,
    12.507_343_278_686_905,
    -0.138_571_095_265_720_12,
    9.984_369_578_019_572e-6,
    1.505_632_735_149_311_6e-7,
];

/// ガンマ関数 Γ(z)。複素数全体で計算できる（極 z = 0, -1, -2, ... では無限大）。
///
/// - Re z < 1/2 では相反公式 Γ(z)Γ(1-z) = π / sin(πz) を使う
/// - それ以外は Lanczos 近似
pub fn gamma(z: C) -> C {
    if z.re < 0.5 {
        // 極（0 以下の整数）。sin(πz) は丸め誤差でぴったり 0 にならないので先に判定する
        if z.im == 0.0 && z.re == z.re.round() {
            return C::new(f64::INFINITY, 0.0);
        }
        return PI / ((PI * z).sin() * gamma(1.0 - z));
    }
    let z = z - 1.0;
    let mut x = C::new(LANCZOS_P[0], 0.0);
    for (k, p) in LANCZOS_P.iter().enumerate().skip(1) {
        x += *p / (z + k as f64);
    }
    let t = z + LANCZOS_G + 0.5;
    (2.0 * PI).sqrt() * t.powc(z + 0.5) * (-t).exp() * x
}

/// 1 / Γ(z)（極では 0）
pub fn rgamma(z: C) -> C {
    if z.re < 0.5 && z.im == 0.0 && z.re == z.re.round() {
        return C::new(0.0, 0.0);
    }
    1.0 / gamma(z)
}

/// 実数の整数なら、その値を返す
fn as_integer(z: C) -> Option<f64> {
    (z.im == 0.0 && z.re.fract() == 0.0 && z.re.abs() < 1e15).then_some(z.re)
}

/// 整数の積 (a+1)(a+2)…(a+k) / (1·2·…·k)^div をそのまま計算する（k が小さいとき用）
fn falling_product(n: f64, k: f64, divide_by_factorial: bool) -> f64 {
    let mut r = 1.0;
    let mut i = 1.0;
    while i <= k {
        r *= n - k + i;
        if divide_by_factorial {
            r /= i;
        }
        i += 1.0;
    }
    r
}

/// 整数で直接掛け算する上限（これより大きいときはガンマ関数で計算する）
const DIRECT_LIMIT: f64 = 2000.0;

/// 二項係数 nCr = n! / (r! (n−r)!)。整数なら掛け算で正確に、それ以外はガンマ関数で計算する。
pub fn binomial(n: C, r: C) -> C {
    if let (Some(ni), Some(ri)) = (as_integer(n), as_integer(r)) {
        if ri < 0.0 {
            return C::new(0.0, 0.0);
        }
        if ni >= 0.0 {
            if ri > ni {
                return C::new(0.0, 0.0);
            }
            let k = ri.min(ni - ri);
            if k <= DIRECT_LIMIT {
                return C::new(falling_product(ni, k, true).round(), 0.0);
            }
        } else {
            // n が負の整数: nCr = (−1)^r · (r−n−1)Cr
            let sign = if ri % 2.0 == 0.0 { 1.0 } else { -1.0 };
            return sign * binomial(C::new(ri - ni - 1.0, 0.0), r);
        }
    }
    gamma(n + 1.0) / (gamma(r + 1.0) * gamma(n - r + 1.0))
}

/// 順列の数 nPr = n! / (n−r)!。整数なら掛け算で正確に、それ以外はガンマ関数で計算する。
pub fn permutation(n: C, r: C) -> C {
    if let (Some(ni), Some(ri)) = (as_integer(n), as_integer(r)) {
        if ni >= 0.0 && ri >= 0.0 {
            if ri > ni {
                return C::new(0.0, 0.0);
            }
            if ri <= DIRECT_LIMIT {
                return C::new(falling_product(ni, ri, false), 0.0);
            }
        }
    }
    gamma(n + 1.0) / gamma(n - r + 1.0)
}

// ---------------------------------------------------------------------------
// ベータ関数・ディガンマ関数
// ---------------------------------------------------------------------------

/// ベータ関数 B(a, b) = Γ(a) Γ(b) / Γ(a + b)
pub fn beta(a: C, b: C) -> C {
    gamma(a) * gamma(b) / gamma(a + b)
}

/// ディガンマ関数 ψ(z) = Γ'(z) / Γ(z)。
/// Re z < 1/2 は相反公式 ψ(1 − z) − ψ(z) = π cot(πz)、それ以外は ψ(z) = ψ(z + 1) − 1/z で
/// z を大きくしてから漸近展開 ψ(z) ≈ ln z − 1/(2z) − Σ B₂ₖ / (2k z²ᵏ) を使う。
pub fn digamma(z: C) -> C {
    if z.re < 0.5 {
        if z.im == 0.0 && z.re == z.re.round() {
            return C::new(f64::NAN, f64::NAN); // 極（0 以下の整数）
        }
        // cot は cos / sin で計算する（tan だと z = −1/2 などで 0 / 0 になる）
        let w = PI * z;
        return digamma(1.0 - z) - PI * w.cos() / w.sin();
    }
    let mut z = z;
    let mut acc = C::new(0.0, 0.0);
    while z.norm() < 10.0 {
        acc -= 1.0 / z;
        z += 1.0;
    }
    let z2 = 1.0 / (z * z);
    // B₂/2, B₄/4, B₆/6, B₈/8, B₁₀/10, B₁₂/12
    let series = z2
        * (1.0 / 12.0
            - z2 * (1.0 / 120.0 - z2 * (1.0 / 252.0 - z2 * (1.0 / 240.0 - z2 * (1.0 / 132.0 - z2 * (691.0 / 32760.0))))));
    acc + z.ln() - 0.5 / z - series
}

// ---------------------------------------------------------------------------
// 誤差関数
// ---------------------------------------------------------------------------

/// 誤差関数 erf(z) = (2/√π) ∫₀ᶻ e^(−t²) dt
pub fn erf(z: C) -> C {
    if use_erf_series(z) {
        erf_series(z)
    } else if z.re >= 0.0 {
        1.0 - erfc_cf(z)
    } else {
        -(1.0 - erfc_cf(-z))
    }
}

/// 相補誤差関数 erfc(z) = 1 − erf(z)
pub fn erfc(z: C) -> C {
    if use_erf_series(z) {
        1.0 - erf_series(z)
    } else if z.re >= 0.0 {
        erfc_cf(z)
    } else {
        2.0 - erfc_cf(-z)
    }
}

/// 級数を使うかどうか。級数の桁落ちはおよそ e^(2 (Re z)²) 倍なので、|Re z| が小さければ
/// |z| が大きくても級数を使う（虚軸の近くでは連分数の収束が悪い）
fn use_erf_series(z: C) -> bool {
    z.norm() <= 3.0 || z.re.abs() <= 2.5
}

/// erf の級数 (2/√π) Σ (−1)ⁿ z²ⁿ⁺¹ / (n! (2n + 1))（|z| が小さいとき）
fn erf_series(z: C) -> C {
    let z2 = z * z;
    let mut term = z; // (−1)ⁿ z²ⁿ⁺¹ / n!
    let mut sum = z;
    for n in 1..5000 {
        term *= -z2 / n as f64;
        let t = term / (2 * n + 1) as f64;
        sum += t;
        if t.norm() < 1e-17 * sum.norm() {
            break;
        }
    }
    sum * (2.0 / PI.sqrt())
}

/// erfc の連分数（Re z > 0 で |z| が大きいとき）:
/// erfc z = e^(−z²)/√π · 1 / (z + (1/2) / (z + 1 / (z + (3/2) / (z + 2 / (z + …)))))
fn erfc_cf(z: C) -> C {
    // 後ろから計算する（項の数は十分大きく取る）
    let mut f = z;
    for k in (1..300).rev() {
        f = z + (k as f64 / 2.0) / f;
    }
    (-z * z).exp() / (PI.sqrt() * f)
}

// ---------------------------------------------------------------------------
// ゼータ関数
// ---------------------------------------------------------------------------

/// リーマンのゼータ関数 ζ(s)。
/// Re s ≥ 1/2 はボーウェインの方法（交代級数 η(s) を速く収束させる）、Re s < 1/2 は関数等式
/// ζ(s) = 2ˢ πˢ⁻¹ sin(πs/2) Γ(1 − s) ζ(1 − s) を使う。s = 1 は極。
pub fn zeta(s: C) -> C {
    if s == C::new(1.0, 0.0) {
        return C::new(f64::INFINITY, 0.0);
    }
    if s == C::new(0.0, 0.0) {
        return C::new(-0.5, 0.0); // 関数等式では 0 × ∞ になるので、値を直接返す
    }
    if s.re < 0.5 {
        let two = C::new(2.0, 0.0);
        let pi = C::new(PI, 0.0);
        return two.powc(s) * pi.powc(s - 1.0) * (PI * s / 2.0).sin() * gamma(1.0 - s) * zeta(1.0 - s);
    }
    // 項の数は、虚部が大きいほど増やす（ボーウェインの誤差の見積もりによる）
    let n = (50.0 + 1.3 * s.im.abs()).min(400.0) as usize;
    BORWEIN.with(|cache| {
        let mut cache = cache.borrow_mut();
        let (d, ln_k) = cache.entry(n).or_insert_with(|| borwein_coefficients(n));
        let dn = d[n];
        let mut sum = C::new(0.0, 0.0);
        for k in 0..n {
            let sign = if k % 2 == 0 { 1.0 } else { -1.0 };
            // (k + 1)^(−s) = e^(−s ln(k + 1))
            sum += sign * (d[k] - dn) * (-s * ln_k[k]).exp();
        }
        -sum / (dn * (1.0 - C::new(2.0, 0.0).powc(1.0 - s)))
    })
}

/// 項の数 n → (係数 d, ln(k + 1) の表)
type BorweinCache = std::collections::HashMap<usize, (Vec<f64>, Vec<f64>)>;

thread_local! {
    /// ボーウェインの方法の係数（項の数 n ごとに1回だけ計算して使い回す）
    static BORWEIN: std::cell::RefCell<BorweinCache> = std::cell::RefCell::new(BorweinCache::new());
}

/// 係数 d_k = n Σ_{i=0}^{k} (n + i − 1)! 4ⁱ / ((n − i)! (2i)!)（k = 0..n）と、ln(k + 1)（k = 0..n−1）。
/// 桁あふれを防ぐため対数で計算する
fn borwein_coefficients(n: usize) -> (Vec<f64>, Vec<f64>) {
    // ln(m!) の表
    let mut lf = vec![0.0f64; 2 * n + 1];
    for m in 1..lf.len() {
        lf[m] = lf[m - 1] + (m as f64).ln();
    }
    let mut d = vec![0.0f64; n + 1];
    let mut sum_i = 0.0f64;
    for (i, slot) in d.iter_mut().enumerate() {
        let log_t = lf[n + i - 1] - lf[n - i] - lf[2 * i] + i as f64 * 4f64.ln();
        sum_i += log_t.exp();
        *slot = n as f64 * sum_i;
    }
    let ln_k = (0..n).map(|k| ((k + 1) as f64).ln()).collect();
    (d, ln_k)
}

// ---------------------------------------------------------------------------
// ベッセル関数
// ---------------------------------------------------------------------------

/// 第1種ベッセル関数 J_ν(z)（ν も z も複素数でよい）
pub fn bessel_j(nu: C, z: C) -> C {
    bessel_series(nu, z, -1.0)
}

/// 第1種変形ベッセル関数 I_ν(z)
pub fn bessel_i(nu: C, z: C) -> C {
    bessel_series(nu, z, 1.0)
}

/// J_ν（sign = −1）と I_ν（sign = +1）の共通の計算。
/// |z| が小さいときは級数 Σ (sign)ᵏ (z/2)^(2k+ν) / (k! Γ(k + ν + 1))、
/// J で |z| が大きいときは漸近展開を使う
fn bessel_series(nu: C, z: C, sign: f64) -> C {
    // ν が負の整数: J₋ₙ = (−1)ⁿ Jₙ、I₋ₙ = Iₙ
    if nu.im == 0.0 && nu.re < 0.0 && nu.re == nu.re.round() {
        let n = -nu.re;
        let f = if sign < 0.0 && n % 2.0 == 1.0 { -1.0 } else { 1.0 };
        return f * bessel_series(-nu, z, sign);
    }
    if sign < 0.0 && z.re < 0.0 && z.norm() > 10.0 {
        // J_ν(w e^(iπs)) = e^(iπνs) J_ν(w)
        let (w, s) = bessel::reflect(z);
        return (C::new(0.0, PI * s) * nu).exp() * bessel_series(nu, w, sign);
    }
    if sign < 0.0 && bessel::use_schlafli(nu, z) {
        return bessel::schlafli(nu, z).0;
    }
    if sign < 0.0 && bessel::use_asymptotic(nu, z) {
        return bessel_j_asymptotic(nu, z);
    }
    if z == C::new(0.0, 0.0) {
        return if nu == C::new(0.0, 0.0) { C::new(1.0, 0.0) } else { C::new(0.0, 0.0) };
    }
    let h = z / 2.0;
    let h2 = sign * h * h;
    let mut term = h.powc(nu) / gamma(nu + 1.0);
    let mut sum = term;
    for k in 1..500 {
        term *= h2 / (k as f64 * (nu + k as f64));
        sum += term;
        if term.norm() < 1e-17 * sum.norm() && k > 2 {
            break;
        }
    }
    sum
}

/// J_ν(z) の漸近展開（|z| が大きいとき）
fn bessel_j_asymptotic(nu: C, z: C) -> C {
    let (p, q) = bessel::hankel_pq(nu, z);
    let w = z - nu * (PI / 2.0) - PI / 4.0;
    (2.0 / (PI * z)).sqrt() * (w.cos() * p - w.sin() * q)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: C, b: C, tol: f64) -> bool {
        (a - b).norm() <= tol * b.norm().max(1.0)
    }

    #[test]
    fn gamma_real() {
        // Γ(n) = (n-1)!
        assert!(close(gamma(C::new(5.0, 0.0)), C::new(24.0, 0.0), 1e-12));
        assert!(close(gamma(C::new(0.5, 0.0)), C::new(PI.sqrt(), 0.0), 1e-12));
        // Γ(-1/2) = -2√π
        assert!(close(gamma(C::new(-0.5, 0.0)), C::new(-2.0 * PI.sqrt(), 0.0), 1e-12));
    }

    #[test]
    fn gamma_complex() {
        // Γ(i) ≈ -0.15494982830181 - 0.49801566811836 i
        let g = gamma(C::new(0.0, 1.0));
        assert!(close(g, C::new(-0.154_949_828_301_810_7, -0.498_015_668_118_356), 1e-12));
    }

    #[test]
    fn combinatorics() {
        let r = |x: f64| C::new(x, 0.0);
        assert_eq!(binomial(r(5.0), r(2.0)), r(10.0));
        assert_eq!(binomial(r(52.0), r(5.0)), r(2_598_960.0));
        assert_eq!(binomial(r(3.0), r(5.0)), r(0.0));
        assert_eq!(binomial(r(-2.0), r(3.0)), r(-4.0)); // (−2)(−3)(−4)/3! = −4
        assert!(close(binomial(r(0.5), r(2.0)), r(-0.125), 1e-12));
        assert_eq!(permutation(r(5.0), r(2.0)), r(20.0));
        assert_eq!(permutation(r(5.0), r(0.0)), r(1.0));
        assert_eq!(permutation(r(2.0), r(5.0)), r(0.0));
        assert!(close(permutation(r(4.5), r(1.0)), r(4.5), 1e-12));
    }

    #[test]
    fn beta_digamma() {
        let r = |x: f64| C::new(x, 0.0);
        assert!(close(beta(r(2.0), r(3.0)), r(1.0 / 12.0), 1e-12));
        assert!(close(digamma(r(1.0)), r(-0.577_215_664_901_532_9), 1e-12));
        assert!(close(digamma(r(0.5)), r(-1.963_510_026_021_423_5), 1e-12));
        assert!(close(digamma(r(-0.5)), r(0.036_489_973_978_576_52), 1e-11));
        assert!(close(digamma(C::new(0.0, 1.0)), C::new(0.094_650_320_622_476_97, 2.076_674_047_468_581), 1e-11));
    }

    #[test]
    fn error_function() {
        let r = |x: f64| C::new(x, 0.0);
        assert!(close(erf(r(0.5)), r(0.520_499_877_813_046_5), 1e-13));
        assert!(close(erf(r(3.5)), r(0.999_999_256_901_627_7), 1e-13));
        assert!(close(erf(r(-4.0)), r(-0.999_999_984_582_742_1), 1e-13));
        assert!(close(erfc(r(5.0)), r(1.537_459_794_428_034_8e-12), 1e-10));
        assert!(close(erf(C::new(1.0, 1.0)), C::new(1.316_151_281_697_947_7, 0.190_453_469_237_834_7), 1e-12));
        assert!(close(erf(C::new(3.0, 2.0)), C::new(0.998_963_278_856_817_3, -0.000_011_546_724_379_290_6), 1e-11));
    }

    #[test]
    fn zeta_function() {
        let r = |x: f64| C::new(x, 0.0);
        assert!(close(zeta(r(2.0)), r(PI * PI / 6.0), 1e-12));
        assert!(close(zeta(r(3.0)), r(1.202_056_903_159_594_2), 1e-12));
        assert!(close(zeta(r(0.0)), r(-0.5), 1e-12));
        assert!(close(zeta(r(-1.0)), r(-1.0 / 12.0), 1e-11));
        assert!(close(zeta(r(0.5)), r(-1.460_354_508_809_586_8), 1e-11));
        // 最初の非自明な零点 1/2 + 14.1347…i
        assert!(zeta(C::new(0.5, 14.134_725_141_734_693)).norm() < 1e-8);
        assert!(!zeta(r(1.0)).norm().is_finite());
    }

    #[test]
    fn bessel() {
        let r = |x: f64| C::new(x, 0.0);
        assert!(close(bessel_j(r(0.0), r(1.0)), r(0.765_197_686_557_966_6), 1e-13));
        assert!(close(bessel_j(r(1.0), r(2.5)), r(0.497_094_102_464_274_1), 1e-13));
        assert!(close(bessel_j(r(0.0), r(10.0)), r(-0.245_935_764_451_348_3), 1e-11));
        assert!(close(bessel_j(r(0.0), r(30.0)), r(-0.086_367_983_581_040_2), 1e-7));
        assert!(close(bessel_j(r(0.5), r(1.0)), r((2.0 / PI).sqrt() * 1f64.sin()), 1e-13));
        assert!(close(bessel_j(r(-1.0), r(2.5)), r(-0.497_094_102_464_274_1), 1e-13));
        assert!(close(bessel_i(r(0.0), r(1.0)), r(1.266_065_877_752_008_4), 1e-13));
        assert!(close(bessel_j(r(2.0), r(0.0)), r(0.0), 1e-15));
    }

    #[test]
    fn gamma_pole() {
        assert!(!gamma(C::new(-2.0, 0.0)).norm().is_finite());
    }
}

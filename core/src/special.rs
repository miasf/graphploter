//! 特殊関数（複素数対応）。今後ここに増やしていく。

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
    fn gamma_pole() {
        assert!(!gamma(C::new(-2.0, 0.0)).norm().is_finite());
    }
}

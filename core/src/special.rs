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
    fn gamma_pole() {
        assert!(!gamma(C::new(-2.0, 0.0)).norm().is_finite());
    }
}

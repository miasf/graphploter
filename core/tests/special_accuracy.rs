//! 特殊関数の値を、極端な引数で参照値（mpmath 40 桁）と比べる（第6段階: #148 #150–156 #170–174 #203）。
//! あわせて、関数の恒等式・対称性と、計算方法を切り替える境目での連続性を確かめる

use graph_core::equation::eval_constant;
use graph_core::special as sp;
use num_complex::Complex64 as C;

const TABLE: &[(&str, f64, f64, f64)] = include!("special_accuracy_table.inc");

fn value(src: &str) -> C {
    let re = eval_constant(&format!("Re({src})")).unwrap_or_else(|e| panic!("Re({src}): {e}"));
    let im = eval_constant(&format!("Im({src})")).unwrap_or_else(|e| panic!("Im({src}): {e}"));
    C::new(re, im)
}

/// 相対誤差（参照値が 0 なら絶対誤差）
fn rel(got: C, want: C) -> f64 {
    let err = (got - want).norm();
    if want.norm() > 0.0 { err / want.norm() } else { err }
}

#[test]
fn matches_reference_values() {
    let mut bad = Vec::new();
    for &(src, re, im, tol) in TABLE {
        let want = C::new(re, im);
        let got = value(src);
        let r = rel(got, want);
        if !(r <= tol) {
            bad.push(format!("{src}: {got} （正しくは {want}、相対誤差 {r:.1e}）"));
        }
    }
    assert!(bad.is_empty(), "{} 件ずれている:\n{}", bad.len(), bad.join("\n"));
}

fn c(re: f64, im: f64) -> C {
    C::new(re, im)
}

fn r(x: f64) -> C {
    C::new(x, 0.0)
}

fn close(a: C, b: C, tol: f64, what: &str) {
    assert!(rel(a, b) <= tol, "{what}: {a} と {b}（相対差 {:.1e}）", rel(a, b));
}

/// #148: B(a, b) = B(b, a)、I_x(a, b) + I_{1−x}(b, a) = 1、分布関数は単調
#[test]
fn beta_identities() {
    for (a, b) in [(100.0, 1.0), (100.0, 100.0), (300.0, 0.5), (2.5, 170.0), (0.3, 400.0)] {
        close(sp::beta(r(a), r(b)), sp::beta(r(b), r(a)), 1e-14, &format!("B({a}, {b})"));
        for x in [0.01, 0.3, 0.5, 0.7, 0.99] {
            let sum = sp::beta_regularized(r(x), r(a), r(b)) + sp::beta_regularized(r(1.0 - x), r(b), r(a));
            close(sum, r(1.0), 1e-13, &format!("I_{x}({a}, {b}) + I_{{1−x}}({b}, {a})"));
        }
    }
    assert_eq!(sp::beta_regularized(r(0.5), r(100.0), r(100.0)), r(0.5));
    let mut last = -1.0;
    for k in 0..=200 {
        let v = sp::beta_regularized(r(k as f64 / 200.0), r(100.0), r(100.0)).re;
        assert!(v >= last, "I_x(100, 100) が x = {} で減った", k as f64 / 200.0);
        last = v;
    }
}

/// #150・#173: 臨界線の上の共役対称、ζ(s, 1) = ζ(s)、負の整数の近くでの連続性
#[test]
fn zeta_identities() {
    for t in [50.0, 200.0, 1000.0] {
        close(sp::zeta(c(0.5, -t)), sp::zeta(c(0.5, t)).conj(), 1e-13, &format!("ζ(1/2 ∓ {t}i)"));
        close(sp::hurwitz_zeta(c(0.5, t), r(1.0)), sp::zeta(c(0.5, t)), 1e-13, &format!("ζ(1/2 + {t}i, 1)"));
    }
    // ボーウェインの方法と和公式の境目 Im s = 40
    close(sp::zeta(c(0.5, 40.0 - 1e-9)), sp::zeta(c(0.5, 40.0 + 1e-9)), 1e-8, "ζ の Im s = 40");
    for n in [3.0, 8.0, 12.0] {
        for a in [0.2, 0.7, 2.5] {
            let at = sp::hurwitz_zeta(r(-n), r(a));
            for d in [1e-7, -1e-7] {
                close(sp::hurwitz_zeta(r(-n + d), r(a)), at, 1e-5, &format!("ζ(−{n} + {d}, {a})"));
            }
        }
    }
    // フルヴィッツの公式と和公式の境目 Re s = −4
    close(sp::hurwitz_zeta(r(-4.0 - 1e-9), r(0.3)), sp::hurwitz_zeta(r(-4.0 + 1e-9), r(0.3)), 1e-6, "ζ(s, 0.3) の Re s = −4");
}

/// #151: 共役対称と、漸近展開・級数・K を切り替える境目（|z| = 5, 8、arg z = 2π/3, 0.95）での連続性
#[test]
fn airy_continuity() {
    for z in [c(0.5, 50.0), c(-30.0, 5.0), c(10.0, 10.0), c(3.0, -4.0)] {
        let (ai, bi) = sp::airy(z);
        let (ai2, bi2) = sp::airy(z.conj());
        close(ai2, ai.conj(), 1e-14, &format!("Ai({z}) の共役"));
        close(bi2, bi.conj(), 1e-14, &format!("Bi({z}) の共役"));
    }
    let edges = [
        (5.0, 0.4), (5.0, 1.5), (5.0, 2.5),
        (8.0, 0.3), (8.0, 1.2), (8.0, 2.0), (8.0, 2.8),
        (6.5, 0.95), (12.0, 2.0 * std::f64::consts::PI / 3.0), (6.0, 2.0 * std::f64::consts::PI / 3.0),
    ];
    for (radius, angle) in edges {
        // 両側の点は 2×10⁻¹³ だけ離す（関数そのものの変化が、許す差 10⁻⁹ より十分小さくなるように）
        for (dr, da) in [(1e-13, 0.0), (0.0, 1e-13)] {
            let a = C::from_polar(radius - dr, angle - da);
            let b = C::from_polar(radius + dr, angle + da);
            let ((ai_a, bi_a), (ai_b, bi_b)) = (sp::airy(a), sp::airy(b));
            close(ai_a, ai_b, 1e-9, &format!("Ai の |z| = {radius}, arg = {angle}"));
            close(bi_a, bi_b, 1e-9, &format!("Bi の |z| = {radius}, arg = {angle}"));
            let ((d_a, e_a), (d_b, e_b)) = (sp::airy_prime(a), sp::airy_prime(b));
            close(d_a, d_b, 1e-9, &format!("Ai′ の |z| = {radius}, arg = {angle}"));
            close(e_a, e_b, 1e-9, &format!("Bi′ の |z| = {radius}, arg = {angle}"));
        }
    }
    // ロンスキアン W(Ai, Bi) = 1/π
    for z in [c(0.5, 50.0), c(-7.0, 3.0), c(9.0, -9.0), c(6.0, 2.0)] {
        let ((ai, bi), (aip, bip)) = (sp::airy(z), sp::airy_prime(z));
        let w = ai * bip - aip * bi;
        let scale = (ai * bip).norm().max((aip * bi).norm());
        assert!((w - 1.0 / std::f64::consts::PI).norm() <= 1e-12 * scale.max(1.0), "W(Ai, Bi)({z}) = {w}");
    }
}

/// #152・#153・#203: ベッセル関数の漸化式・反射公式と、計算方法の境目での連続性
#[test]
fn bessel_identities() {
    // 半整数の反射: Y_{−n−1/2}(x) = (−1)ⁿ J_{n+1/2}(x)
    for (n, x) in [(0.0, 0.3), (2.0, 0.01), (5.0, 7.0)] {
        let sign = if n % 2.0 == 0.0 { 1.0 } else { -1.0 };
        close(sp::bessel_y(r(-n - 0.5), r(x)), sign * sp::bessel_j(r(n + 0.5), r(x)), 1e-13, &format!("Y_{{−{n}−1/2}}({x})"));
    }
    // 漸化式 C_{ν−1} + C_{ν+1} = (2ν/z) C_ν（J, Y）、K_{ν+1} − K_{ν−1} = (2ν/z) K_ν
    for (nu, z) in [(50.0, c(1.0, 30.0)), (30.0, c(5.0, 5.0)), (99.0, c(20.0, 10.0)), (10.5, c(2.0, 3.0))] {
        for f in [sp::bessel_j as fn(C, C) -> C, sp::bessel_y] {
            let lhs = f(r(nu - 1.0), z) + f(r(nu + 1.0), z);
            let rhs = 2.0 * nu / z * f(r(nu), z);
            close(lhs, rhs, 1e-10, &format!("漸化式 ν = {nu}, z = {z}"));
        }
        let lhs = sp::bessel_k(r(nu + 1.0), z) - sp::bessel_k(r(nu - 1.0), z);
        close(lhs, 2.0 * nu / z * sp::bessel_k(r(nu), z), 1e-10, &format!("K の漸化式 ν = {nu}, z = {z}"));
    }
    // Y の |Im z| = 3 の境目、J の級数の境目
    close(sp::bessel_y(r(20.0), c(30.0, 3.0 - 1e-9)), sp::bessel_y(r(20.0), c(30.0, 3.0 + 1e-9)), 1e-8, "Y の Im z = 3");
    let edge = 4.0 * 101f64.sqrt() + 4.0;
    close(sp::bessel_j(r(100.0), r(edge - 1e-9)), sp::bessel_j(r(100.0), r(edge + 1e-9)), 1e-7, "J₁₀₀ の級数の境目");
    // I の虚軸寄りの境目 |Im z| = |Re z|
    close(sp::bessel_i(r(40.0), c(30.0, 30.0 - 1e-9)), sp::bessel_i(r(40.0), c(30.0, 30.0 + 1e-9)), 1e-8, "I の |Im z| = |Re z|");
}

/// #154: 整数の次数での P(1, z) = 1 − e^(−z)、共役対称
#[test]
fn incomplete_gamma_identities() {
    for z in [-50.0, -20.0, -3.0, 0.5, 4.0] {
        close(sp::gamma_p(r(1.0), r(z)), r(1.0 - (-z).exp()), 1e-13, &format!("P(1, {z})"));
        close(sp::gamma_q(r(1.0), r(z)), r((-z).exp()), 1e-13, &format!("Q(1, {z})"));
    }
    let (up, down) = (sp::gamma_q(r(0.5), c(-5.0, 1e-3)), sp::gamma_q(r(0.5), c(-5.0, -1e-3)));
    close(down, up.conj(), 1e-14, "Q(1/2, −5 ± 0.001i)");
}

/// #155: 正の実数での単調性（次数が上がると小さくなる）と E₀(x) = e^(−x)/x
#[test]
fn expint_identities() {
    for x in [1.5, 30.0, 100.0, 300.0] {
        close(sp::expint_e(r(0.0), r(x)), r((-x).exp() / x), 1e-14, &format!("E₀({x})"));
        let mut last = f64::INFINITY;
        for n in 0..=40 {
            let v = sp::expint_e(r(n as f64), r(x)).re;
            assert!(v > 0.0 && v < last && v <= (-x).exp() / x * (1.0 + 1e-14), "E_{n}({x}) = {v}");
            last = v;
        }
    }
}

/// #156: sn² + cn² = 1、m sn² + dn² = 1、共役対称
#[test]
fn jacobi_identities() {
    for (u, m) in [(c(2.0, 4.0), c(0.5, 0.2)), (r(50.0), c(0.5, 0.2)), (c(10.0, -3.0), c(-2.0, 1.0)), (c(0.3, 0.1), c(0.9, 0.3))] {
        let (s, cn, d) = sp::jacobi_elliptic(u, m);
        close(s * s + cn * cn, r(1.0), 1e-12, &format!("sn² + cn² ({u}, {m})"));
        close(m * s * s + d * d, r(1.0), 1e-12, &format!("m sn² + dn² ({u}, {m})"));
        let (s2, _, _) = sp::jacobi_elliptic(u.conj(), m.conj());
        close(s2, s.conj(), 1e-12, &format!("sn の共役 ({u}, {m})"));
    }
}

/// #170・#171・#172: P^(0,0)_n = Pₙ、北極・南極、Y_l^{−m} = (−1)ᵐ conj(Y_l^m)、規格化
#[test]
fn orthogonal_identities() {
    for n in [20.0, 50.0, 100.0, 180.0] {
        for x in [0.0, 0.2, -0.7, 0.99] {
            close(sp::jacobi_p(r(n), r(0.0), r(0.0), r(x)), sp::legendre_p(r(n), r(x)), 1e-12, &format!("P^(0,0)_{n}({x})"));
        }
    }
    for (n, m) in [(1.0, 1.0), (2.0, 2.0), (5.0, 3.0), (100.0, 50.0)] {
        for x in [1.0, -1.0] {
            assert_eq!(sp::legendre_p_assoc(r(n), r(m), r(x)).norm(), 0.0, "P_{n}^{m}({x})");
        }
    }
    for (l, m, theta, phi) in [(3.0, 2.0, 0.7, 0.4), (100.0, 100.0, 1.5, 0.3), (50.0, 7.0, 2.0, 1.0)] {
        let a = sp::spherical_harmonic_y(r(l), r(m), r(theta), r(phi));
        let b = sp::spherical_harmonic_y(r(l), r(-m), r(theta), r(phi));
        let sign = if m % 2.0 == 0.0 { 1.0 } else { -1.0 };
        close(b, sign * a.conj(), 1e-13, &format!("Y_{l}^(−{m})"));
    }
    // m = 0 の北極は √((2l + 1)/(4π))
    for l in [10.0, 100.0, 180.0, 1000.0] {
        let want = ((2.0 * l + 1.0) / (4.0 * std::f64::consts::PI)).sqrt();
        close(sp::spherical_harmonic_y(r(l), r(0.0), r(0.0), r(0.0)), r(want), 1e-11, &format!("Y_{l}^0(0, 0)"));
    }
    // 規格化 ∫|Y_l^m|² dΩ = 1（θ のガウス・ルジャンドル積分。φ は 2π）
    let n = 400;
    for (l, m) in [(30.0, 10.0), (100.0, 100.0)] {
        let mut sum = 0.0;
        for k in 0..n {
            let t = std::f64::consts::PI * (k as f64 + 0.5) / n as f64;
            let y = sp::spherical_harmonic_y(r(l), r(m), r(t), r(0.0)).norm();
            sum += y * y * t.sin() * std::f64::consts::PI / n as f64;
        }
        close(r(sum * 2.0 * std::f64::consts::PI), r(1.0), 1e-6, &format!("Y_{l}^{m} の規格化"));
    }
}

/// #174: n = 0 では第1種と一致、共役対称
#[test]
fn elliptic_pi_identities() {
    for phi in [c(1.0, 2.0), c(0.5, 1.0), c(2.0, 0.5)] {
        close(sp::elliptic_pi_inc(r(0.0), phi, r(0.3)), sp::elliptic_f(phi, r(0.3)), 1e-13, &format!("Π(0; {phi} | 0.3)"));
        let a = sp::elliptic_pi_inc(r(0.5), phi, r(0.1));
        close(sp::elliptic_pi_inc(r(0.5), phi.conj(), r(0.1)), a.conj(), 1e-12, &format!("Π(0.5; {phi} | 0.1) の共役"));
    }
}

/// #203: ₂F₁ の a, b の入れ替え、Lambert W の w + ln w = ln z、非正の整数次数の polylog の有理式
#[test]
fn misc_identities() {
    for (a, b, cc, z) in [(2.0, 11.0, 1.0, -100.0), (0.5, 1.5, 2.0, -50.0), (3.0, 7.0, 2.5, -20.0), (1.5, 4.5, 3.0, 0.9)] {
        close(sp::hyp2f1(r(a), r(b), r(cc), r(z)), sp::hyp2f1(r(b), r(a), r(cc), r(z)), 1e-15, &format!("₂F₁({a}, {b}; {cc}; {z})"));
    }
    for e in [50.0, 150.0, 200.0, 300.0] {
        let w = sp::lambert_w(0.0, r(10f64.powf(e))).re;
        assert!((w + w.ln() - e * 10f64.ln()).abs() <= 1e-13 * w, "W(10^{e}) = {w}");
    }
    let z = -100.0;
    close(sp::polylog(r(0.0), r(z)), r(z / (1.0 - z)), 1e-15, "Li₀(−100)");
    close(sp::polylog(r(-1.0), r(z)), r(z / ((1.0 - z) * (1.0 - z))), 1e-14, "Li₋₁(−100)");
}

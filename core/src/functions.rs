//! 組み込み関数の一覧と、その複素数での計算。
//!
//! 新しい関数を足すときは:
//!   1. `Func` に名前を追加
//!   2. `TABLE` に「関数・入力で使える名前・引数の個数」を1行追加
//!   3. `call()` に計算を書く（引数の個数で意味が変わる関数は、個数ごとに書く）
//!   4. 必要なら `special/` に実装を書く

use crate::special as sp;
use num_complex::Complex64 as C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    // 初等関数
    Sin,
    Cos,
    Tan,
    Sec,
    Csc,
    Cot,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Asinh,
    Acosh,
    Atanh,
    Exp,
    Ln,
    Log10,
    Sqrt,
    Abs,
    Sinc,
    // 整数・丸め
    Floor,
    Ceil,
    Round,
    Sign,
    Mod,
    Min,
    Max,
    // 組合せ
    NCr,
    NPr,
    Pochhammer,
    // 複素数用
    Re,
    Im,
    Conj,
    Arg,
    // ガンマ関数の仲間
    Gamma,
    LogGamma,
    GammaRegP,
    GammaRegQ,
    Beta,
    BetaReg,
    Digamma,
    Polygamma,
    // ゼータ関数の仲間
    Zeta,
    Polylog,
    Eta,
    // 誤差関数の仲間
    Erf,
    Erfc,
    Erfi,
    Dawson,
    FresnelS,
    FresnelC,
    // 指数積分の仲間
    Ei,
    ExpIntE,
    LogInt,
    SinInt,
    CosInt,
    SinhInt,
    CoshInt,
    // ベッセル関数の仲間
    BesselJ,
    BesselY,
    BesselI,
    BesselK,
    HankelH1,
    HankelH2,
    SphericalJ,
    SphericalY,
    AiryAi,
    AiryBi,
    AiryAiPrime,
    AiryBiPrime,
    // 直交多項式
    LegendreP,
    HermiteH,
    LaguerreL,
    ChebyshevT,
    ChebyshevU,
    GegenbauerC,
    JacobiP,
    SphericalHarmonicY,
    // 超幾何関数
    Hyp0F1,
    Hyp1F1,
    Hyp2F1,
    HypU,
    // 楕円積分・楕円関数
    EllipticK,
    EllipticE,
    EllipticF,
    EllipticPi,
    JacobiSN,
    JacobiCN,
    JacobiDN,
    // その他
    LambertW,
}

/// 関数・入力で使える名前（最初のものが代表名）・使える引数の個数
const TABLE: &[(Func, &[&str], &[usize])] = &[
    (Func::Sin, &["sin"], &[1]),
    (Func::Cos, &["cos"], &[1]),
    (Func::Tan, &["tan"], &[1]),
    (Func::Sec, &["sec"], &[1]),
    (Func::Csc, &["csc"], &[1]),
    (Func::Cot, &["cot"], &[1]),
    (Func::Asin, &["arcsin", "asin"], &[1]),
    (Func::Acos, &["arccos", "acos"], &[1]),
    (Func::Atan, &["arctan", "atan"], &[1]),
    (Func::Sinh, &["sinh"], &[1]),
    (Func::Cosh, &["cosh"], &[1]),
    (Func::Tanh, &["tanh"], &[1]),
    (Func::Asinh, &["arcsinh", "asinh"], &[1]),
    (Func::Acosh, &["arccosh", "acosh"], &[1]),
    (Func::Atanh, &["arctanh", "atanh"], &[1]),
    (Func::Exp, &["exp"], &[1]),
    (Func::Ln, &["ln"], &[1]),
    (Func::Log10, &["log"], &[1]),
    (Func::Sqrt, &["sqrt"], &[1]),
    (Func::Abs, &["abs"], &[1]),
    (Func::Sinc, &["sinc"], &[1]),
    (Func::Floor, &["floor"], &[1]),
    (Func::Ceil, &["ceil"], &[1]),
    (Func::Round, &["round"], &[1]),
    (Func::Sign, &["sign"], &[1]),
    (Func::Mod, &["mod"], &[2]),
    (Func::Min, &["min"], &[2]),
    (Func::Max, &["max"], &[2]),
    (Func::NCr, &["nCr", "binomial"], &[2]),
    (Func::NPr, &["nPr"], &[2]),
    (Func::Pochhammer, &["pochhammer"], &[2]),
    (Func::Re, &["Re"], &[1]),
    (Func::Im, &["Im"], &[1]),
    (Func::Conj, &["conj"], &[1]),
    (Func::Arg, &["arg"], &[1]),
    (Func::Gamma, &["gamma", "Γ"], &[1, 2]),
    (Func::LogGamma, &["loggamma", "lngamma"], &[1]),
    (Func::GammaRegP, &["gammaP"], &[2]),
    (Func::GammaRegQ, &["gammaQ"], &[2]),
    (Func::Beta, &["beta"], &[2, 3]),
    (Func::BetaReg, &["betaRegularized"], &[3]),
    (Func::Digamma, &["digamma", "psi", "ψ"], &[1]),
    (Func::Polygamma, &["polygamma"], &[2]),
    (Func::Zeta, &["zeta", "ζ"], &[1, 2]),
    (Func::Polylog, &["polylog", "Li"], &[2]),
    (Func::Eta, &["dirichletEta"], &[1]),
    (Func::Erf, &["erf"], &[1]),
    (Func::Erfc, &["erfc"], &[1]),
    (Func::Erfi, &["erfi"], &[1]),
    (Func::Dawson, &["dawson"], &[1]),
    (Func::FresnelS, &["fresnelS"], &[1]),
    (Func::FresnelC, &["fresnelC"], &[1]),
    (Func::Ei, &["Ei", "expIntegralEi"], &[1]),
    (Func::ExpIntE, &["En", "expIntegralE"], &[2]),
    (Func::LogInt, &["li", "logIntegral"], &[1]),
    (Func::SinInt, &["Si", "sinIntegral"], &[1]),
    (Func::CosInt, &["Ci", "cosIntegral"], &[1]),
    (Func::SinhInt, &["Shi", "sinhIntegral"], &[1]),
    (Func::CoshInt, &["Chi", "coshIntegral"], &[1]),
    (Func::BesselJ, &["besselJ"], &[2]),
    (Func::BesselY, &["besselY"], &[2]),
    (Func::BesselI, &["besselI"], &[2]),
    (Func::BesselK, &["besselK"], &[2]),
    (Func::HankelH1, &["hankelH1"], &[2]),
    (Func::HankelH2, &["hankelH2"], &[2]),
    (Func::SphericalJ, &["sphericalJ", "sphericalBesselJ"], &[2]),
    (Func::SphericalY, &["sphericalY", "sphericalBesselY"], &[2]),
    (Func::AiryAi, &["airyAi"], &[1]),
    (Func::AiryBi, &["airyBi"], &[1]),
    (Func::AiryAiPrime, &["airyAiPrime"], &[1]),
    (Func::AiryBiPrime, &["airyBiPrime"], &[1]),
    (Func::LegendreP, &["legendreP"], &[2, 3]),
    (Func::HermiteH, &["hermiteH"], &[2]),
    (Func::LaguerreL, &["laguerreL"], &[2, 3]),
    (Func::ChebyshevT, &["chebyshevT"], &[2]),
    (Func::ChebyshevU, &["chebyshevU"], &[2]),
    (Func::GegenbauerC, &["gegenbauerC"], &[3]),
    (Func::JacobiP, &["jacobiP"], &[4]),
    (Func::SphericalHarmonicY, &["sphericalHarmonicY"], &[4]),
    (Func::Hyp0F1, &["hyp0F1"], &[2]),
    (Func::Hyp1F1, &["hyp1F1"], &[3]),
    (Func::Hyp2F1, &["hyp2F1"], &[4]),
    (Func::HypU, &["hypU"], &[3]),
    (Func::EllipticK, &["ellipticK"], &[1]),
    (Func::EllipticE, &["ellipticE"], &[1, 2]),
    (Func::EllipticF, &["ellipticF"], &[2]),
    (Func::EllipticPi, &["ellipticPi"], &[2, 3]),
    (Func::JacobiSN, &["jacobiSN"], &[2]),
    (Func::JacobiCN, &["jacobiCN"], &[2]),
    (Func::JacobiDN, &["jacobiDN"], &[2]),
    (Func::LambertW, &["lambertW", "productLog"], &[1, 2]),
];

impl Func {
    /// すべての関数
    pub fn all() -> impl Iterator<Item = Func> {
        TABLE.iter().map(|(f, _, _)| *f)
    }

    fn entry(self) -> &'static (Func, &'static [&'static str], &'static [usize]) {
        TABLE.iter().find(|(f, _, _)| *f == self).expect("TABLE に無い関数")
    }

    /// 入力で使える名前（最初のものが代表名）
    pub fn names(self) -> &'static [&'static str] {
        self.entry().1
    }

    /// 使える引数の個数（例: gamma は 1 個なら Γ(z)、2 個なら上側不完全ガンマ関数 Γ(a, z)）
    pub fn arities(self) -> &'static [usize] {
        self.entry().2
    }

    /// 1引数で計算する（よく使うので専用にしてある）
    pub fn eval1(self, z: C) -> C {
        self.call(&[z])
    }

    /// 2引数で計算する
    pub fn eval2(self, a: C, b: C) -> C {
        self.call(&[a, b])
    }

    /// 引数 `x` で計算する。引数の個数は `arities()` のどれかであること
    pub fn call(self, x: &[C]) -> C {
        let nan = C::new(f64::NAN, f64::NAN);
        let r = |v: f64| C::new(v, 0.0);
        match (self, x.len()) {
            (Func::Sin, 1) => x[0].sin(),
            (Func::Cos, 1) => x[0].cos(),
            (Func::Tan, 1) => x[0].tan(),
            (Func::Sec, 1) => 1.0 / x[0].cos(),
            (Func::Csc, 1) => 1.0 / x[0].sin(),
            (Func::Cot, 1) => x[0].cos() / x[0].sin(),
            (Func::Asin, 1) => x[0].asin(),
            (Func::Acos, 1) => x[0].acos(),
            (Func::Atan, 1) => x[0].atan(),
            (Func::Sinh, 1) => x[0].sinh(),
            (Func::Cosh, 1) => x[0].cosh(),
            (Func::Tanh, 1) => x[0].tanh(),
            (Func::Asinh, 1) => x[0].asinh(),
            (Func::Acosh, 1) => x[0].acosh(),
            (Func::Atanh, 1) => x[0].atanh(),
            (Func::Exp, 1) => x[0].exp(),
            (Func::Ln, 1) => x[0].ln(),
            (Func::Log10, 1) => x[0].log10(),
            (Func::Sqrt, 1) => x[0].sqrt(),
            (Func::Abs, 1) => r(x[0].norm()),
            (Func::Sinc, 1) => {
                if x[0].norm() < 1e-8 {
                    r(1.0) - x[0] * x[0] / 6.0
                } else {
                    x[0].sin() / x[0]
                }
            }
            // 複素数では実部と虚部をそれぞれ丸める
            (Func::Floor, 1) => C::new(x[0].re.floor(), x[0].im.floor()),
            (Func::Ceil, 1) => C::new(x[0].re.ceil(), x[0].im.ceil()),
            (Func::Round, 1) => C::new(x[0].re.round(), x[0].im.round()),
            // 符号: z / |z|（0 なら 0）。実数なら -1, 0, 1
            (Func::Sign, 1) => {
                let n = x[0].norm();
                if n == 0.0 {
                    x[0]
                } else {
                    x[0] / n
                }
            }
            // a を b で割った余り（b と同じ符号）: a − b·floor(a / b)
            (Func::Mod, 2) => {
                let q = x[0] / x[1];
                x[0] - x[1] * C::new(q.re.floor(), q.im.floor())
            }
            // 大小は実部で比べる
            (Func::Min, 2) => {
                if x[1].re < x[0].re {
                    x[1]
                } else {
                    x[0]
                }
            }
            (Func::Max, 2) => {
                if x[1].re > x[0].re {
                    x[1]
                } else {
                    x[0]
                }
            }
            (Func::NCr, 2) => sp::binomial(x[0], x[1]),
            (Func::NPr, 2) => sp::permutation(x[0], x[1]),
            (Func::Pochhammer, 2) => sp::pochhammer(x[0], x[1]),
            (Func::Re, 1) => r(x[0].re),
            (Func::Im, 1) => r(x[0].im),
            (Func::Conj, 1) => x[0].conj(),
            (Func::Arg, 1) => r(x[0].arg()),
            (Func::Gamma, 1) => sp::gamma(x[0]),
            (Func::Gamma, 2) => sp::gamma_upper(x[0], x[1]),
            (Func::LogGamma, 1) => sp::log_gamma(x[0]),
            (Func::GammaRegP, 2) => sp::gamma_p(x[0], x[1]),
            (Func::GammaRegQ, 2) => sp::gamma_q(x[0], x[1]),
            (Func::Beta, 2) => sp::beta(x[0], x[1]),
            (Func::Beta, 3) => sp::beta_incomplete(x[0], x[1], x[2]),
            (Func::BetaReg, 3) => sp::beta_regularized(x[0], x[1], x[2]),
            (Func::Digamma, 1) => sp::digamma(x[0]),
            (Func::Polygamma, 2) => sp::polygamma(x[0], x[1]),
            (Func::Zeta, 1) => sp::zeta(x[0]),
            (Func::Zeta, 2) => sp::hurwitz_zeta(x[0], x[1]),
            (Func::Polylog, 2) => sp::polylog(x[0], x[1]),
            (Func::Eta, 1) => sp::dirichlet_eta(x[0]),
            (Func::Erf, 1) => sp::erf(x[0]),
            (Func::Erfc, 1) => sp::erfc(x[0]),
            (Func::Erfi, 1) => sp::erfi(x[0]),
            (Func::Dawson, 1) => sp::dawson(x[0]),
            (Func::FresnelS, 1) => sp::fresnel_s(x[0]),
            (Func::FresnelC, 1) => sp::fresnel_c(x[0]),
            (Func::Ei, 1) => sp::ei(x[0]),
            (Func::ExpIntE, 2) => sp::expint_e(x[0], x[1]),
            (Func::LogInt, 1) => sp::li(x[0]),
            (Func::SinInt, 1) => sp::si(x[0]),
            (Func::CosInt, 1) => sp::ci(x[0]),
            (Func::SinhInt, 1) => sp::shi(x[0]),
            (Func::CoshInt, 1) => sp::chi(x[0]),
            // besselJ(ν, z) など: 1つ目が次数 ν、2つ目が変数 z
            (Func::BesselJ, 2) => sp::bessel_j(x[0], x[1]),
            (Func::BesselY, 2) => sp::bessel_y(x[0], x[1]),
            (Func::BesselI, 2) => sp::bessel_i(x[0], x[1]),
            (Func::BesselK, 2) => sp::bessel_k(x[0], x[1]),
            (Func::HankelH1, 2) => sp::bessel_j(x[0], x[1]) + C::i() * sp::bessel_y(x[0], x[1]),
            (Func::HankelH2, 2) => sp::bessel_j(x[0], x[1]) - C::i() * sp::bessel_y(x[0], x[1]),
            (Func::SphericalJ, 2) => sp::spherical_j(x[0], x[1]),
            (Func::SphericalY, 2) => sp::spherical_y(x[0], x[1]),
            (Func::AiryAi, 1) => sp::airy(x[0]).0,
            (Func::AiryBi, 1) => sp::airy(x[0]).1,
            (Func::AiryAiPrime, 1) => sp::airy_prime(x[0]).0,
            (Func::AiryBiPrime, 1) => sp::airy_prime(x[0]).1,
            (Func::LegendreP, 2) => sp::legendre_p(x[0], x[1]),
            (Func::LegendreP, 3) => sp::legendre_p_assoc(x[0], x[1], x[2]),
            (Func::HermiteH, 2) => sp::hermite_h(x[0], x[1]),
            (Func::LaguerreL, 2) => sp::laguerre_l(x[0], r(0.0), x[1]),
            (Func::LaguerreL, 3) => sp::laguerre_l(x[0], x[1], x[2]),
            (Func::ChebyshevT, 2) => sp::chebyshev_t(x[0], x[1]),
            (Func::ChebyshevU, 2) => sp::chebyshev_u(x[0], x[1]),
            (Func::GegenbauerC, 3) => sp::gegenbauer_c(x[0], x[1], x[2]),
            (Func::JacobiP, 4) => sp::jacobi_p(x[0], x[1], x[2], x[3]),
            (Func::SphericalHarmonicY, 4) => sp::spherical_harmonic_y(x[0], x[1], x[2], x[3]),
            (Func::Hyp0F1, 2) => sp::hyp0f1(x[0], x[1]),
            (Func::Hyp1F1, 3) => sp::hyp1f1(x[0], x[1], x[2]),
            (Func::Hyp2F1, 4) => sp::hyp2f1(x[0], x[1], x[2], x[3]),
            (Func::HypU, 3) => sp::hyp_u(x[0], x[1], x[2]),
            // 楕円積分・楕円関数は、Wolfram と同じくパラメータ m（= k²）で表す
            (Func::EllipticK, 1) => sp::elliptic_k(x[0]),
            (Func::EllipticE, 1) => sp::elliptic_e(x[0]),
            (Func::EllipticE, 2) => sp::elliptic_e_inc(x[0], x[1]),
            (Func::EllipticF, 2) => sp::elliptic_f(x[0], x[1]),
            (Func::EllipticPi, 2) => sp::elliptic_pi(x[0], x[1]),
            (Func::EllipticPi, 3) => sp::elliptic_pi_inc(x[0], x[1], x[2]),
            (Func::JacobiSN, 2) => sp::jacobi_elliptic(x[0], x[1]).0,
            (Func::JacobiCN, 2) => sp::jacobi_elliptic(x[0], x[1]).1,
            (Func::JacobiDN, 2) => sp::jacobi_elliptic(x[0], x[1]).2,
            (Func::LambertW, 1) => sp::lambert_w(0.0, x[0]),
            (Func::LambertW, 2) => sp::lambert_w(x[0].re.round(), x[1]),
            _ => nan,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64) -> C {
        C::new(x, 0.0)
    }

    #[test]
    fn rounding() {
        assert_eq!(Func::Floor.eval1(r(-1.5)), r(-2.0));
        assert_eq!(Func::Ceil.eval1(r(-1.5)), r(-1.0));
        assert_eq!(Func::Round.eval1(r(2.5)), r(3.0));
        assert_eq!(Func::Sign.eval1(r(-3.0)), r(-1.0));
        assert_eq!(Func::Sign.eval1(r(0.0)), r(0.0));
    }

    #[test]
    fn modulo() {
        assert_eq!(Func::Mod.eval2(r(7.0), r(3.0)), r(1.0));
        assert_eq!(Func::Mod.eval2(r(-7.0), r(3.0)), r(2.0));
        assert!((Func::Mod.eval2(r(5.5), r(2.0)) - r(1.5)).norm() < 1e-15);
    }

    #[test]
    fn min_max() {
        assert_eq!(Func::Min.eval2(r(2.0), r(-1.0)), r(-1.0));
        assert_eq!(Func::Max.eval2(r(2.0), r(-1.0)), r(2.0));
    }

    #[test]
    fn every_function_has_a_formula() {
        // 表にある関数と引数の個数の組み合わせは、すべて計算できる（NaN 固定になっていない）
        for f in Func::all() {
            for &n in f.arities() {
                let mut args = vec![C::new(0.3, 0.1); n];
                if f == Func::Polygamma {
                    args[0] = C::new(2.0, 0.0); // 階数は 0 以上の整数だけ
                }
                let v = f.call(&args);
                assert!(!(v.re.is_nan() && v.im.is_nan()), "{:?}（引数 {n} 個）が計算されない", f);
            }
        }
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn special_values_match_reference() {
        // 参照値は mpmath（30 桁）で計算したもの
        #[rustfmt::skip]
        let cases: Vec<(&str, Vec<C>, C)> = vec![
        ("sinc", vec![C::new(2.5, 0.0)], C::new(0.2393888576415826, 0.0)),
        ("pochhammer", vec![C::new(2.5, 0.0), C::new(3.0, 0.0)], C::new(39.375, 0.0)),
        ("pochhammer", vec![C::new(0.5, 0.0), C::new(1.5, 0.0)], C::new(0.5641895835477563, 0.0)),
        ("binomial", vec![C::new(5.5, 0.0), C::new(2.0, 0.0)], C::new(12.375, 0.0)),
        ("gamma", vec![C::new(1.5, 0.0), C::new(2.0, 0.0)], C::new(0.23171655200098068, 0.0)),
        ("gamma", vec![C::new(0.0, 0.0), C::new(3.3, 0.0)], C::new(0.00893904254203214, 0.0)),
        ("gamma", vec![C::new(-2.0, 0.0), C::new(1.5, 0.0)], C::new(0.025217551186824123, 0.0)),
        ("loggamma", vec![C::new(-1.3, 0.0)], C::new(1.2024757863901114, -6.283185307179586)),
        ("loggamma", vec![C::new(3.0, 4.0)], C::new(-1.7566267846037842, 4.742664438034658)),
        ("gammaP", vec![C::new(2.0, 0.0), C::new(1.5, 0.0)], C::new(0.4421745996289254, 0.0)),
        ("gammaQ", vec![C::new(2.5, 0.0), C::new(10.0, 0.0)], C::new(0.0012497305630313753, 0.0)),
        ("beta", vec![C::new(0.3, 0.0), C::new(1.5, 0.0), C::new(2.5, 0.0)], C::new(0.08162011893537471, 0.0)),
        ("betaRegularized", vec![C::new(0.6, 0.0), C::new(2.0, 0.0), C::new(3.0, 0.0)], C::new(0.8208, 0.0)),
        ("polygamma", vec![C::new(1.0, 0.0), C::new(2.5, 0.0)], C::new(0.49035775610023485, 0.0)),
        ("polygamma", vec![C::new(3.0, 0.0), C::new(-0.5, 1.0)], C::new(-2.508842885277068, -0.5664043783267628)),
        ("zeta", vec![C::new(2.0, 0.0), C::new(0.5, 0.0)], C::new(4.934802200544679, 0.0)),
        ("zeta", vec![C::new(3.0, 1.0), C::new(2.0, 0.0)], C::new(0.1072144084314092, -0.14829086717817536)),
        ("polylog", vec![C::new(2.0, 0.0), C::new(0.5, 0.0)], C::new(0.5822405264650125, 0.0)),
        ("polylog", vec![C::new(3.0, 0.0), C::new(-4.0, 0.0)], C::new(-2.9671076939431944, 0.0)),
        ("polylog", vec![C::new(2.5, 0.0), C::new(0.9, 0.0)], C::new(1.1390030252021568, 0.0)),
        ("polylog", vec![C::new(-1.0, 0.0), C::new(0.3, 0.5)], C::new(-0.5076698319941563, 0.6026296566837107)),
        ("dirichletEta", vec![C::new(0.5, 0.0)], C::new(0.6048986434216304, 0.0)),
        ("dirichletEta", vec![C::new(-2.0, 1.0)], C::new(-0.05954931475057942, 0.24091199819933137)),
        ("erfi", vec![C::new(1.5, 0.0)], C::new(4.584733257284427, 0.0)),
        ("erfi", vec![C::new(-3.007, 0.0)], C::new(-1695.362248025778, 0.0)),
        ("dawson", vec![C::new(2.0, 0.0)], C::new(0.30134038892379195, 0.0)),
        ("dawson", vec![C::new(10.0, 0.0)], C::new(0.05025384718759853, 0.0)),
        ("fresnelS", vec![C::new(1.2, 0.0)], C::new(0.6234009185462497, 0.0)),
        ("fresnelS", vec![C::new(6.331, -0.037)], C::new(0.435967332045625, -0.005134997827390842)),
        ("fresnelC", vec![C::new(1.2, 0.0)], C::new(0.7154377229230734, 0.0)),
        ("Ei", vec![C::new(1.5, 0.0)], C::new(3.301285449129798, 0.0)),
        ("Ei", vec![C::new(-2.0, 0.0)], C::new(-0.04890051070806112, 0.0)),
        ("Ei", vec![C::new(20.0, 0.0)], C::new(25615652.664056588, 0.0)),
        ("En", vec![C::new(2.0, 0.0), C::new(1.5, 0.0)], C::new(0.07310078653848084, 0.0)),
        ("En", vec![C::new(0.5, 0.0), C::new(0.7, 0.0)], C::new(0.5014959375020237, 0.0)),
        ("li", vec![C::new(10.0, 0.0)], C::new(6.165599504787298, 0.0)),
        ("li", vec![C::new(0.5, 0.0)], C::new(-0.37867104306108795, 0.0)),
        ("Si", vec![C::new(3.0, 0.0)], C::new(1.8486525279994683, 0.0)),
        ("Si", vec![C::new(20.0, 0.0)], C::new(1.54824170104344, 0.0)),
        ("Si", vec![C::new(-0.0, -10.0)], C::new(0.0, -1246.1144901994232)),
        ("Ci", vec![C::new(3.0, 0.0)], C::new(0.11962978600800032, 0.0)),
        ("Ci", vec![C::new(15.0, 0.0)], C::new(0.04627867767436044, 0.0)),
        ("Shi", vec![C::new(2.0, 0.0)], C::new(2.5015674333549756, 0.0)),
        ("Shi", vec![C::new(-8.775, 0.0)], C::new(-427.1274551174436, 0.0)),
        ("Chi", vec![C::new(2.0, 0.0)], C::new(2.4526669226469147, 0.0)),
        ("Chi", vec![C::new(12.0, 0.0)], C::new(7479.7663329612105, 0.0)),
        ("besselJ", vec![C::new(0.0, 0.0), C::new(30.0, 0.0)], C::new(-0.08636798358104021, 0.0)),
        ("besselJ", vec![C::new(10.0, 0.0), C::new(30.0, 0.0)], C::new(-0.12987689399858876, 0.0)),
        ("besselJ", vec![C::new(2.5, 0.0), C::new(-12.0, 0.0)], C::new(0.0, 0.07242267383180952)),
        ("besselY", vec![C::new(0.0, 0.0), C::new(2.0, 0.0)], C::new(0.5103756726497451, 0.0)),
        ("besselY", vec![C::new(1.0, 0.0), C::new(26.0, 0.0)], C::new(-0.15579655322960265, 0.0)),
        ("besselY", vec![C::new(0.0, 0.0), C::new(-30.0, 0.0)], C::new(-0.11729573168666403, -0.17273596716208042)),
        ("besselI", vec![C::new(1.0, 0.0), C::new(3.0, 0.0)], C::new(3.9533702174026093, 0.0)),
        ("besselK", vec![C::new(0.0, 0.0), C::new(2.0, 0.0)], C::new(0.11389387274953344, 0.0)),
        ("besselK", vec![C::new(1.5, 0.0), C::new(8.0, 0.0)], C::new(0.00016722900749831942, 0.0)),
        ("besselK", vec![C::new(0.0, 0.0), C::new(-30.0, 0.0)], C::new(2.1324774964630563e-14, -2455695948358.4604)),
        ("sphericalBesselJ", vec![C::new(1.0, 0.0), C::new(2.0, 0.0)], C::new(0.4353977749799916, 0.0)),
        ("sphericalBesselY", vec![C::new(2.0, 0.0), C::new(3.0, 0.0)], C::new(-0.26703833526449916, 0.0)),
        ("airyAi", vec![C::new(1.0, 0.0)], C::new(0.13529241631288141, 0.0)),
        ("airyAi", vec![C::new(-8.2, 0.0)], C::new(-0.2215994548036039, 0.0)),
        ("airyAi", vec![C::new(8.0, 0.0)], C::new(4.6922076160992316e-08, 0.0)),
        ("airyBi", vec![C::new(-3.5, 0.5)], C::new(0.244447846474437, -0.4035091561166998)),
        ("airyBi", vec![C::new(7.0, 0.0)], C::new(80327.79070943025, 0.0)),
        ("airyAiPrime", vec![C::new(2.0, 0.0)], C::new(-0.05309038443365363, 0.0)),
        ("airyAiPrime", vec![C::new(-10.0, 0.0)], C::new(0.99626504413279, 0.0)),
        ("airyBiPrime", vec![C::new(1.5, 0.0)], C::new(1.8862122548481655, 0.0)),
        ("legendreP", vec![C::new(3.0, 0.0), C::new(0.4, 0.0)], C::new(-0.44, 0.0)),
        ("legendreP", vec![C::new(2.5, 0.0), C::new(0.3, 0.0)], C::new(-0.4636610134572232, 0.0)),
        ("legendreP", vec![C::new(3.0, 0.0), C::new(1.0, 0.0), C::new(0.5, 0.0)], C::new(-0.3247595264191645, 0.0)),
        ("legendreP", vec![C::new(2.0, 0.0), C::new(-1.0, 0.0), C::new(0.3, 0.0)], C::new(0.14309088021254185, 0.0)),
        ("legendreP", vec![C::new(1.5, 0.0), C::new(0.5, 0.0), C::new(0.2, 0.0)], C::new(-0.7415835379803722, 0.0)),
        ("hermiteH", vec![C::new(5.0, 0.0), C::new(1.3, 0.0)], C::new(-76.70624000000001, 0.0)),
        ("hermiteH", vec![C::new(2.5, 0.0), C::new(10.0, 0.0)], C::new(1772.0786476033043, 0.0)),
        ("laguerreL", vec![C::new(4.0, 0.0), C::new(2.5, 0.0)], C::new(0.9609375, 0.0)),
        ("laguerreL", vec![C::new(3.0, 0.0), C::new(1.5, 0.0), C::new(2.0, 0.0)], C::new(-1.5208333333333333, 0.0)),
        ("chebyshevT", vec![C::new(5.0, 0.0), C::new(0.3, 0.0)], C::new(0.99888, 0.0)),
        ("chebyshevT", vec![C::new(2.5, 0.0), C::new(0.4, 0.0)], C::new(-0.9705256307795276, 0.0)),
        ("chebyshevU", vec![C::new(4.0, 0.0), C::new(1.5, 0.0)], C::new(55.0, 0.0)),
        ("gegenbauerC", vec![C::new(3.0, 0.0), C::new(1.5, 0.0), C::new(0.7, 0.0)], C::new(0.7524999999999992, 0.0)),
        ("jacobiP", vec![C::new(3.0, 0.0), C::new(-2.0, 0.0), C::new(0.7, 0.0), C::new(0.4, 0.0)], C::new(0.2382615, 0.0)),
        ("jacobiP", vec![C::new(2.5, 0.0), C::new(0.5, 0.0), C::new(0.5, 0.0), C::new(0.2, 0.0)], C::new(-0.5921239794839915, 0.0)),
        ("sphericalHarmonicY", vec![C::new(2.0, 0.0), C::new(1.0, 0.0), C::new(0.7, 0.0), C::new(0.4, 0.0)], C::new(-0.3506053748459931, -0.14823357493803718)),
        ("hyp0F1", vec![C::new(1.5, 0.0), C::new(-6.0, 0.0)], C::new(-0.20058104804562038, 0.0)),
        ("hyp1F1", vec![C::new(0.5, 0.0), C::new(1.5, 0.0), C::new(-30.0, 0.0)], C::new(0.16180215937964007, 0.0)),
        ("hyp2F1", vec![C::new(1.0, 0.0), C::new(1.0, 0.0), C::new(2.0, 0.0), C::new(-1.0, 0.0)], C::new(0.6931471805599453, 0.0)),
        ("hyp2F1", vec![C::new(0.3, 0.0), C::new(0.7, 0.0), C::new(1.5, 0.0), C::new(10.0, 0.0)], C::new(0.593129103425216, -0.5073418142534278)),
        ("hyp2F1", vec![C::new(0.5, 0.0), C::new(0.5, 0.0), C::new(1.0, 0.0), C::new(0.99, 0.0)], C::new(2.3527158167797424, 0.0)),
        ("hypU", vec![C::new(2.5, 0.0), C::new(1.0, 0.0), C::new(22.0, 0.0)], C::new(0.0003421831766341301, 0.0)),
        ("hypU", vec![C::new(0.3, 0.0), C::new(0.7, 0.0), C::new(5.0, 0.0)], C::new(0.598237513081305, 0.0)),
        ("hypU", vec![C::new(1.0, 0.0), C::new(2.0, 0.0), C::new(0.5, 0.0)], C::new(2.0, 0.0)),
        ("ellipticK", vec![C::new(0.5, 0.0)], C::new(1.8540746773013719, 0.0)),
        ("ellipticK", vec![C::new(2.0, 0.0)], C::new(1.3110287771460598, -1.3110287771460598)),
        ("ellipticE", vec![C::new(0.5, 0.0)], C::new(1.3506438810476755, 0.0)),
        ("ellipticE", vec![C::new(-3.0, 0.0)], C::new(2.422112055136919, 0.0)),
        ("ellipticE", vec![C::new(1.2, 0.0), C::new(0.5, 0.0)], C::new(1.082717119300184, 0.0)),
        ("ellipticE", vec![C::new(4.0, 0.0), C::new(0.3, 0.0)], C::new(3.7210652682686023, 0.0)),
        ("ellipticF", vec![C::new(1.2, 0.0), C::new(0.5, 0.0)], C::new(1.340733523660133, 0.0)),
        ("ellipticPi", vec![C::new(0.5, 0.0), C::new(0.7, 0.0)], C::new(3.089663490480848, 0.0)),
        ("ellipticPi", vec![C::new(2.0, 0.0), C::new(0.5, 0.0)], C::new(-0.31354468346518405, -1.8137993642342178)),
        ("ellipticPi", vec![C::new(0.3, 0.0), C::new(1.2, 0.0), C::new(0.5, 0.0)], C::new(1.5303674742027078, 0.0)),
        ("jacobiSN", vec![C::new(0.7, 0.0), C::new(0.5, 0.0)], C::new(0.6243400909662173, 0.0)),
        ("jacobiSN", vec![C::new(2.0, 1.0), C::new(0.3, 0.0)], C::new(1.314604252923062, -0.1759953031617644)),
        ("jacobiSN", vec![C::new(5.0, 0.0), C::new(-6.0, 0.0)], C::new(0.5420868002662286, 0.0)),
        ("jacobiCN", vec![C::new(2.0, 0.0), C::new(1.5, 0.0)], C::new(0.611027646597911, -8.417992558341002e-37)),
        ("jacobiDN", vec![C::new(10.0, 0.0), C::new(0.99, 0.0)], C::new(0.16405504440062638, 0.0)),
        ("lambertW", vec![C::new(1.0, 0.0)], C::new(0.5671432904097838, 0.0)),
        ("lambertW", vec![C::new(-0.3, 0.0)], C::new(-0.4894022271802149, 0.0)),
        ("lambertW", vec![C::new(0.0, 2.0)], C::new(0.683408016326576, 0.7433860047413521)),
        ("lambertW", vec![C::new(-1.0, 0.0), C::new(-0.2, 0.0)], C::new(-2.5426413577735265, 0.0)),
        ("lambertW", vec![C::new(1.0, 0.0), C::new(3.0, 0.0)], C::new(-0.43584653317240457, 4.6182937405071645)),

        ];
        for (name, args, want) in cases {
            let f = Func::all().find(|f| f.names().contains(&name)).unwrap();
            let got = f.call(&args);
            let err = (got - want).norm() / want.norm().max(1e-300);
            assert!(err < 1e-8, "{name}{args:?} = {got}（正しくは {want}、相対誤差 {err:.1e}）");
        }
    }
}

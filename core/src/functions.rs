//! 組み込み関数の一覧と、その複素数での計算。
//!
//! 新しい関数を足すときは:
//!   1. `Func` に名前を追加
//!   2. `ALL`・`names()`・`arity()` に1行ずつ追加
//!   3. 引数が1つなら `eval1()`、2つなら `eval2()` に計算を書く
//!   4. 必要なら `special.rs` に実装を書く

use crate::special;
use num_complex::Complex64 as C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Func {
    // 初等関数
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Sinh,
    Cosh,
    Tanh,
    Exp,
    Ln,
    Log10,
    Sqrt,
    Abs,
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
    // 複素数用
    Re,
    Im,
    Conj,
    Arg,
    // 特殊関数
    Gamma,
}

impl Func {
    pub const ALL: &'static [Func] = &[
        Func::Sin,
        Func::Cos,
        Func::Tan,
        Func::Asin,
        Func::Acos,
        Func::Atan,
        Func::Sinh,
        Func::Cosh,
        Func::Tanh,
        Func::Exp,
        Func::Ln,
        Func::Log10,
        Func::Sqrt,
        Func::Abs,
        Func::Floor,
        Func::Ceil,
        Func::Round,
        Func::Sign,
        Func::Mod,
        Func::Min,
        Func::Max,
        Func::NCr,
        Func::NPr,
        Func::Re,
        Func::Im,
        Func::Conj,
        Func::Arg,
        Func::Gamma,
    ];

    /// 入力で使える名前（最初のものが代表名）
    pub fn names(self) -> &'static [&'static str] {
        match self {
            Func::Sin => &["sin"],
            Func::Cos => &["cos"],
            Func::Tan => &["tan"],
            Func::Asin => &["arcsin", "asin"],
            Func::Acos => &["arccos", "acos"],
            Func::Atan => &["arctan", "atan"],
            Func::Sinh => &["sinh"],
            Func::Cosh => &["cosh"],
            Func::Tanh => &["tanh"],
            Func::Exp => &["exp"],
            Func::Ln => &["ln"],
            Func::Log10 => &["log"],
            Func::Sqrt => &["sqrt"],
            Func::Abs => &["abs"],
            Func::Floor => &["floor"],
            Func::Ceil => &["ceil"],
            Func::Round => &["round"],
            Func::Sign => &["sign"],
            Func::Mod => &["mod"],
            Func::Min => &["min"],
            Func::Max => &["max"],
            Func::NCr => &["nCr"],
            Func::NPr => &["nPr"],
            Func::Re => &["Re"],
            Func::Im => &["Im"],
            Func::Conj => &["conj"],
            Func::Arg => &["arg"],
            Func::Gamma => &["gamma", "Γ"],
        }
    }

    /// 引数の個数
    pub fn arity(self) -> usize {
        match self {
            Func::Mod | Func::Min | Func::Max | Func::NCr | Func::NPr => 2,
            _ => 1,
        }
    }

    /// 1引数関数を複素数で計算する
    pub fn eval1(self, z: C) -> C {
        match self {
            Func::Sin => z.sin(),
            Func::Cos => z.cos(),
            Func::Tan => z.tan(),
            Func::Asin => z.asin(),
            Func::Acos => z.acos(),
            Func::Atan => z.atan(),
            Func::Sinh => z.sinh(),
            Func::Cosh => z.cosh(),
            Func::Tanh => z.tanh(),
            Func::Exp => z.exp(),
            Func::Ln => z.ln(),
            Func::Log10 => z.log10(),
            Func::Sqrt => z.sqrt(),
            Func::Abs => C::new(z.norm(), 0.0),
            // 複素数では実部と虚部をそれぞれ丸める
            Func::Floor => C::new(z.re.floor(), z.im.floor()),
            Func::Ceil => C::new(z.re.ceil(), z.im.ceil()),
            Func::Round => C::new(z.re.round(), z.im.round()),
            // 符号: z / |z|（0 なら 0）。実数なら -1, 0, 1
            Func::Sign => {
                let r = z.norm();
                if r == 0.0 {
                    z
                } else {
                    z / r
                }
            }
            Func::Re => C::new(z.re, 0.0),
            Func::Im => C::new(z.im, 0.0),
            Func::Conj => z.conj(),
            Func::Arg => C::new(z.arg(), 0.0),
            Func::Gamma => special::gamma(z),
            _ => C::new(f64::NAN, f64::NAN),
        }
    }

    /// 2引数関数を複素数で計算する
    pub fn eval2(self, a: C, b: C) -> C {
        match self {
            // a を b で割った余り（b と同じ符号）: a − b·floor(a / b)
            Func::Mod => {
                let q = a / b;
                a - b * C::new(q.re.floor(), q.im.floor())
            }
            // 大小は実部で比べる
            Func::Min => {
                if b.re < a.re {
                    b
                } else {
                    a
                }
            }
            Func::Max => {
                if b.re > a.re {
                    b
                } else {
                    a
                }
            }
            Func::NCr => special::binomial(a, b),
            Func::NPr => special::permutation(a, b),
            _ => C::new(f64::NAN, f64::NAN),
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
}

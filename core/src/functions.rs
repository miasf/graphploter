//! 組み込み関数の一覧と、その複素数での計算。
//!
//! 新しい関数を足すときは:
//!   1. `Func` に名前を追加
//!   2. `ALL`・`names()`・`arity()`・`eval()` に1行ずつ追加
//!   3. 必要なら `special.rs` に実装を書く

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
            Func::Re => &["Re"],
            Func::Im => &["Im"],
            Func::Conj => &["conj"],
            Func::Arg => &["arg"],
            Func::Gamma => &["gamma", "Γ"],
        }
    }

    /// 引数の個数
    pub fn arity(self) -> usize {
        1
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
            Func::Re => C::new(z.re, 0.0),
            Func::Im => C::new(z.im, 0.0),
            Func::Conj => z.conj(),
            Func::Arg => C::new(z.arg(), 0.0),
            Func::Gamma => special::gamma(z),
        }
    }
}

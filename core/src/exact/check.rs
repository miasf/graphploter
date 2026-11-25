//! 照らし合わせ（計画 `docs/plan-exact-core.md` の 2.4）。
//!
//! 厳密値の道が出した答えを、元の問題を別の道で計算した値と比べる。相手は今の f64 の道（`equation::eval_constant_complex`）。
//! EX10 の後は、高い精度の数値（元の積分・和・極限を規則を使わずに数値で計算したもの）も相手にする。

use super::ball;
use super::term::Term;
use crate::eval::Params;

/// 照らし合わせの結果
#[derive(Debug, Clone, PartialEq)]
pub enum Check {
    /// 一致した
    Agree,
    /// 食い違った（厳密値の道の値, 相手の値）
    Disagree { exact: (f64, f64), other: (f64, f64) },
    /// 相手の値が無い（今の道で計算できない。理由つき）
    NoPartner(String),
    /// 厳密値の道の答えの値を計算できない
    NoValue(String),
}

/// f64 の相手との一致の基準: |A − B| ≤ 10^{−8} max(1, |B|)
pub const F64_TOLERANCE: f64 = 1e-8;

/// 答え `t`（文字を含まない）を、`source` を今の f64 の道で計算した値と比べる
pub fn against_f64(t: &Term, source: &str, params: &Params) -> Check {
    let a = match ball::value(t, 64) {
        Ok(b) => b.to_f64(),
        Err(e) => return Check::NoValue(format!("{e:?}")),
    };
    let b = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::equation::eval_constant_complex(source, params))) {
        Ok(Ok(c)) => (c.re, c.im),
        Ok(Err(e)) => return Check::NoPartner(e),
        Err(_) => return Check::NoPartner("今の道でパニック".into()),
    };
    let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let m = (b.0 * b.0 + b.1 * b.1).sqrt().max(1.0);
    if d <= F64_TOLERANCE * m {
        Check::Agree
    } else {
        Check::Disagree { exact: a, other: b }
    }
}

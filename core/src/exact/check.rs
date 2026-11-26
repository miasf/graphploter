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

/// 四則と整数の関数だけ（極限・積分・微分などの規則を使わない）の答えの基準。規則の誤りは大きくずれるが、
/// 今の道の f64 は桁落ち（ヒルベルト行列の逆行列で 4 × 10^{−8}）でずれるので、少し緩める
pub const F64_TOLERANCE_ARITHMETIC: f64 = 1e-6;

thread_local! {
    static TOLERANCE: std::cell::Cell<f64> = const { std::cell::Cell::new(F64_TOLERANCE) };
}

/// 照らし合わせの基準を決めて `f` を動かす
pub fn with_tolerance<T>(tol: f64, f: impl FnOnce() -> T) -> T {
    let before = TOLERANCE.with(|t| t.replace(tol));
    let r = f();
    TOLERANCE.with(|t| t.set(before));
    r
}

fn tolerance() -> f64 {
    TOLERANCE.with(|t| t.get())
}

/// 組・リストの答えを、成分ごとに今の f64 の道の値と比べる
pub fn items_against_f64(items: &[Term], source: &str, params: &Params) -> Check {
    let mut a = Vec::new();
    for t in items {
        match ball::value(t, 64) {
            Ok(b) => a.push(b.to_f64()),
            Err(e) => return Check::NoValue(format!("{e:?}")),
        }
    }
    let b: Vec<(f64, f64)> = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| crate::equation::eval_constant_items(source, params))) {
        Ok(Ok(v)) => v.iter().map(|c| (c.re, c.im)).collect(),
        Ok(Err(e)) => return Check::NoPartner(e),
        Err(_) => return Check::NoPartner("今の道でパニック".into()),
    };
    if a.len() != b.len() {
        return Check::NoPartner(format!("成分の数が違います（{} と {}）", a.len(), b.len()));
    }
    for (x, y) in a.iter().zip(&b) {
        let d = ((x.0 - y.0).powi(2) + (x.1 - y.1).powi(2)).sqrt();
        if d > tolerance() * (y.0 * y.0 + y.1 * y.1).sqrt().max(1.0) {
            return Check::Disagree { exact: *x, other: *y };
        }
    }
    Check::Agree
}

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
    if d <= tolerance() * m {
        Check::Agree
    } else {
        Check::Disagree { exact: a, other: b }
    }
}

/// 原始関数 g を微分した式と、中身 f の差が、いくつかの点で 0 か（値が計算できる点が 3 つ以上要る）
pub fn antiderivative(g: &Term, f: &Term, x: &str) -> Check {
    let d = match super::diff::diff(g, x) {
        Ok(d) => super::term::add(vec![d, super::term::neg(f.clone())]),
        Err(e) => return Check::NoValue(format!("{e:?}")),
    };
    let mut ok = 0;
    for (n, dd) in [(3i64, 7u64), (5, 8), (4, 5), (7, 5), (11, 5), (13, 4), (-2, 3), (-9, 4)] {
        let v = Term::rat(n, dd);
        let Ok(t) = super::diff::subst_checked(&d, x, &v) else { continue };
        if !t.is_number() {
            return Check::NoValue("文字が残ります".into());
        }
        let Ok(b) = ball::value(&t, 128) else { continue };
        // 実数の点で中身が複素数になる所（定義域の外）は数えない
        let fv = super::diff::subst(f, x, &v);
        if let Ok(fb) = ball::value(&fv, 64) {
            if !fb.is_real() {
                continue;
            }
        } else {
            continue;
        }
        if !b.contains_zero() {
            let (re, im) = b.to_f64();
            return Check::Disagree { exact: (re, im), other: (0.0, 0.0) };
        }
        ok += 1;
    }
    if ok >= 3 {
        Check::Agree
    } else {
        Check::NoValue("確かめられる点が足りません".into())
    }
}

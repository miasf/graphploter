//! 記号の不定積分（積分定数を省いた形）。表で扱える形だけを積分し、扱えなければ None を返す
//! （呼び出し側 `expand.rs` は、そのとき数値の $\int_0^x f(t)\,dt$ にする）。
//!
//! 扱う形: 変数を含まない式（c → c x）、和・差・定数倍・定数での割り算、
//! 中身が 1 次式 $u = a x + b$（a, b は x を含まない）の基本の関数:
//! $u^n$（$n \ne -1$）、$\frac{1}{u}$・$u^{-1}$（→ $\ln|u|$）、$\sqrt u$、$c^u$、$e^u$、$\sin u$、$\cos u$、$\tan u$、$\sinh u$、$\cosh u$。
//! 結果を微分すると元の式に戻ることを、テストで確かめる。

use crate::diff::{add, diff, div, mul, neg, pow, sub};
use crate::functions::Func;
use crate::parser::{BinOp, Expr};

fn num(v: f64) -> Expr {
    Expr::Num(v)
}

fn c1(f: Func, a: Expr) -> Expr {
    Expr::Call(f, vec![a])
}

/// u が x の 1 次式なら、その傾き a（x を含まない式）
fn linear_slope(u: &Expr, var: &str) -> Option<Expr> {
    let a = diff(u, var).ok()?;
    (!a.uses_var(var)).then_some(a)
}

/// x を含まない値として読める数（−1 の判定などに使う）
fn as_num(e: &Expr) -> Option<f64> {
    match e {
        Expr::Num(v) => Some(*v),
        Expr::Neg(a) => as_num(a).map(|v| -v),
        _ => None,
    }
}

/// `e` の x での不定積分。扱えなければ None
pub fn antiderivative(e: &Expr, var: &str) -> Option<Expr> {
    let x = Expr::Var(var.to_string());
    if !e.uses_var(var) {
        return Some(mul(e.clone(), x));
    }
    match e {
        Expr::Var(_) => Some(div(pow(x, num(2.0)), num(2.0))),
        Expr::Neg(a) => antiderivative(a, var).map(neg),
        Expr::Bin(BinOp::Add, a, b) => Some(add(antiderivative(a, var)?, antiderivative(b, var)?)),
        Expr::Bin(BinOp::Sub, a, b) => Some(sub(antiderivative(a, var)?, antiderivative(b, var)?)),
        Expr::Bin(BinOp::Mul, a, b) if !a.uses_var(var) => Some(mul((**a).clone(), antiderivative(b, var)?)),
        Expr::Bin(BinOp::Mul, a, b) if !b.uses_var(var) => Some(mul(antiderivative(a, var)?, (**b).clone())),
        Expr::Bin(BinOp::Div, a, b) if !b.uses_var(var) => Some(div(antiderivative(a, var)?, (**b).clone())),
        // c / u（u は 1 次式）→ c ln|u| / a
        Expr::Bin(BinOp::Div, c, u) if !c.uses_var(var) => {
            let a = linear_slope(u, var)?;
            Some(div(mul((**c).clone(), c1(Func::Ln, c1(Func::Abs, (**u).clone()))), a))
        }
        Expr::Bin(BinOp::Pow, u, n) if !n.uses_var(var) => {
            let a = linear_slope(u, var)?;
            if as_num(n) == Some(-1.0) {
                return Some(div(c1(Func::Ln, c1(Func::Abs, (**u).clone())), a));
            }
            // 指数が −1 になりうる式（パラメータ）は扱わない（−1 のとき 0 で割る）
            as_num(n)?;
            let n1 = add((**n).clone(), num(1.0));
            Some(div(pow((**u).clone(), n1.clone()), mul(n1, a)))
        }
        // c^u（c は x を含まない）→ c^u / (a ln c)
        Expr::Bin(BinOp::Pow, c, u) if !c.uses_var(var) => {
            let a = linear_slope(u, var)?;
            Some(div(e.clone(), mul(a, c1(Func::Ln, (**c).clone()))))
        }
        Expr::Call(f, args) if args.len() == 1 => {
            let u = args[0].clone();
            let a = linear_slope(&u, var)?;
            let base = match f {
                Func::Sin => neg(c1(Func::Cos, u)),
                Func::Cos => c1(Func::Sin, u),
                Func::Tan => neg(c1(Func::Ln, c1(Func::Abs, c1(Func::Cos, u)))),
                Func::Exp => c1(Func::Exp, u),
                Func::Sinh => c1(Func::Cosh, u),
                Func::Cosh => c1(Func::Sinh, u),
                // √u = u^(1/2) → (2/3) u^(3/2)
                Func::Sqrt => mul(num(2.0 / 3.0), pow(u, num(1.5))),
                _ => return None,
            };
            Some(div(base, a))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Program;
    use crate::parser::parse;
    use num_complex::Complex64 as C;

    fn eval_at(e: &Expr, x: f64) -> f64 {
        let p = Program::compile(e, &["x", "y"]).unwrap();
        let mut out = [C::new(0.0, 0.0)];
        p.eval_batch(&[&[x], &[0.7]], &mut out);
        crate::eval::as_real(out[0])
    }

    /// 不定積分を微分すると元に戻る（中心差分で確かめる）
    fn check(src: &str) {
        let f = parse(src).unwrap();
        let big_f = antiderivative(&f, "x").unwrap_or_else(|| panic!("{src} が積分できない"));
        for x in [0.3, 0.9, 1.7, 2.6] {
            let h = 1e-5;
            let d = (eval_at(&big_f, x + h) - eval_at(&big_f, x - h)) / (2.0 * h);
            let want = eval_at(&f, x);
            assert!((d - want).abs() < 1e-6 * want.abs().max(1.0), "{src} の x = {x}: F' = {d}, f = {want}");
        }
    }

    #[test]
    fn table() {
        for src in [
            "3", "x", "x^2 + 2x + 1", "x^5 / 4 - 3 x^(-2)", "1/x", "2/(3x + 1)", "x^(-1)", "(2x - 1)^3", "sqrt(x)", "sqrt(2x + 1)",
            "sin(x)", "cos(2x + 1)", "tan(x/3)", "exp(3x)", "e^(2x)", "2^x", "sinh(x) + cosh(2x)", "y x^2", "sin(x) y + 1",
            "-(x^2)", "(x^3 + 1)/2",
        ] {
            check(src);
        }
    }

    #[test]
    fn not_in_table() {
        for src in ["exp(-x^2)", "sin(x)/x", "x sin(x)", "sin(x^2)", "x^y"] {
            assert!(antiderivative(&parse(src).unwrap(), "x").is_none(), "{src}");
        }
    }
}

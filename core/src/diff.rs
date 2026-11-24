//! 記号微分（計画 `docs/plan-math-notation.md` の M3）。今までの節だけの式（展開の後）を、変数で微分した式にする。
//!
//! - 変数は実数として扱う。値が複素数の式も微分できる（実の変数での微分。$\frac{d}{dx}|u| = \frac{\operatorname{Re}(\bar u\,u')}{|u|}$ など）
//! - 導関数の表に無い関数（ゼータ関数・楕円関数など）は、中心差分の式
//!   $\dfrac{g(u + h) - g(u - h)}{2h}$、$h = 10^{-5}\max(1, |u|)$ に置き換える。これも式なので、同じ道（区間演算・段差の切り分け）を通る
//! - 微分できない点（`abs` の折れ目・`floor` の段差）は、片側の値（段差の上では 0）。段差の上での微分を δ 関数にはしない。
//!   利用者が書いた微分（`d/dx`・`f′`。[`diff_n_marked`]）では、そういう点に印を付ける（#161。[`kink`]・[`jump`]）
//! - 節を作るたびに簡約する（$0 + u$、$0 \cdot u$、$1 \cdot u$、$u^1$、定数どうし）。簡約しないと、高い階の微分で式が大きくなりすぎる

use crate::functions::Func;
use crate::parser::{BinOp, Constant, Expr};
use std::cell::Cell;

thread_local! {
    /// 導関数が存在しない点の印を付けるか（[`diff_n_marked`] の間だけ）
    static MARK: Cell<bool> = const { Cell::new(false) };
}

/// 折れ目の印: 値は v のまま（余裕 |s| ≥ 0 の [`Func::Guard`]）。s = 0 の所で導関数の片側の値が違えば、
/// 描くときにその点を除いて白丸にする（`holes.rs` の `kinks`）。|x|³ の導関数 3x|x| のように、
/// 外の掛け算で跳びが消えれば、そのまま描く
fn kink(v: Expr, s: Expr) -> Expr {
    if MARK.get() {
        call(Func::Guard, vec![v, c1(Func::Abs, &s)])
    } else {
        v
    }
}

/// 段差の印: s = 0 の所（`floor` の引数が整数の所など）で値を NaN にする（余裕 |s| − 10⁻³⁰⁰ の [`Func::Guard`]）。
/// 関数そのものが跳ぶので、導関数は存在しない
fn jump(v: Expr, s: Expr) -> Expr {
    if MARK.get() {
        call(Func::Guard, vec![v, sub(c1(Func::Abs, &s), num(1e-300))])
    } else {
        v
    }
}

/// 整数との差 u − round(u)（整数で 0、そこで符号が変わる）
fn off_integer(u: &Expr) -> Expr {
    sub(u.clone(), c1(Func::Round, u))
}

// ---------------------------------------------------------------------------
// 簡約しながら節を作る
// ---------------------------------------------------------------------------

fn num(v: f64) -> Expr {
    Expr::Num(v)
}

fn as_num(e: &Expr) -> Option<f64> {
    match e {
        Expr::Num(v) => Some(*v),
        Expr::Neg(a) => as_num(a).map(|v| -v),
        _ => None,
    }
}

fn bin(op: BinOp, a: Expr, b: Expr) -> Expr {
    Expr::Bin(op, Box::new(a), Box::new(b))
}

pub fn add(a: Expr, b: Expr) -> Expr {
    match (as_num(&a), as_num(&b)) {
        (Some(x), Some(y)) => num(x + y),
        (Some(x), _) if x == 0.0 => b,
        (_, Some(y)) if y == 0.0 => a,
        _ => bin(BinOp::Add, a, b),
    }
}

pub fn sub(a: Expr, b: Expr) -> Expr {
    match (as_num(&a), as_num(&b)) {
        (Some(x), Some(y)) => num(x - y),
        (Some(x), _) if x == 0.0 => neg(b),
        (_, Some(y)) if y == 0.0 => a,
        _ => bin(BinOp::Sub, a, b),
    }
}

pub fn mul(a: Expr, b: Expr) -> Expr {
    match (as_num(&a), as_num(&b)) {
        (Some(x), Some(y)) => num(x * y),
        (Some(x), _) | (_, Some(x)) if x == 0.0 => num(0.0),
        (Some(x), _) if x == 1.0 => b,
        (_, Some(y)) if y == 1.0 => a,
        (Some(x), _) if x == -1.0 => neg(b),
        (_, Some(y)) if y == -1.0 => neg(a),
        _ => bin(BinOp::Mul, a, b),
    }
}

pub fn div(a: Expr, b: Expr) -> Expr {
    match (as_num(&a), as_num(&b)) {
        (Some(x), Some(y)) if y != 0.0 => num(x / y),
        (Some(x), _) if x == 0.0 => num(0.0),
        (_, Some(y)) if y == 1.0 => a,
        _ => bin(BinOp::Div, a, b),
    }
}

pub fn neg(a: Expr) -> Expr {
    match a {
        Expr::Num(v) => num(-v),
        Expr::Neg(x) => *x,
        _ => Expr::Neg(Box::new(a)),
    }
}

pub fn pow(a: Expr, b: Expr) -> Expr {
    match as_num(&b) {
        Some(y) if y == 1.0 => a,
        Some(y) if y == 0.0 => num(1.0),
        _ => bin(BinOp::Pow, a, b),
    }
}

fn call(f: Func, args: Vec<Expr>) -> Expr {
    Expr::Call(f, args)
}

fn c1(f: Func, a: &Expr) -> Expr {
    call(f, vec![a.clone()])
}

fn sq(a: &Expr) -> Expr {
    pow(a.clone(), num(2.0))
}

/// 式が虚数単位 i を含むか（`abs` の微分を、実数の速い形にしてよいかの判断に使う）
fn has_i(e: &Expr) -> bool {
    match e {
        Expr::Const(Constant::I) => true,
        Expr::Num(_) | Expr::Const(_) | Expr::Var(_) => false,
        Expr::Neg(a) => has_i(a),
        Expr::Bin(_, a, b) => has_i(a) || has_i(b),
        Expr::Call(f, args) => matches!(f, Func::Conj | Func::Arg) || args.iter().any(has_i),
        Expr::Tuple(a) | Expr::List(a) => a.iter().any(has_i),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// 微分
// ---------------------------------------------------------------------------

/// 中心差分の刻み $h = 10^{-5}\max(1, |u|)$
fn step(u: &Expr) -> Expr {
    mul(num(1e-5), call(Func::Max, vec![num(1.0), c1(Func::Abs, u)]))
}

/// 関数 f の、k 番目の引数での偏微分を中心差分で
fn numeric_partial(f: Func, args: &[Expr], k: usize) -> Expr {
    let h = step(&args[k]);
    let shifted = |s: f64| {
        let mut a = args.to_vec();
        a[k] = add(args[k].clone(), mul(num(s), h.clone()));
        call(f, a)
    };
    div(sub(shifted(1.0), shifted(-1.0)), mul(num(2.0), h))
}

/// 引数 1 つの関数の導関数 f'(u)。表に無ければ None
fn derivative1(f: Func, u: &Expr) -> Option<Expr> {
    use Func::*;
    let sqrt_pi = (std::f64::consts::PI).sqrt();
    Some(match f {
        Sin => c1(Cos, u),
        Cos => neg(c1(Sin, u)),
        Tan => div(num(1.0), sq(&c1(Cos, u))),
        Sec => mul(c1(Sec, u), c1(Tan, u)),
        Csc => neg(mul(c1(Csc, u), c1(Cot, u))),
        Cot => neg(div(num(1.0), sq(&c1(Sin, u)))),
        Asin => div(num(1.0), c1(Sqrt, &sub(num(1.0), sq(u)))),
        Acos => neg(div(num(1.0), c1(Sqrt, &sub(num(1.0), sq(u))))),
        Atan => div(num(1.0), add(num(1.0), sq(u))),
        Sinh => c1(Cosh, u),
        Cosh => c1(Sinh, u),
        Tanh => div(num(1.0), sq(&c1(Cosh, u))),
        Asinh => div(num(1.0), c1(Sqrt, &add(sq(u), num(1.0)))),
        Acosh => div(num(1.0), mul(c1(Sqrt, &sub(u.clone(), num(1.0))), c1(Sqrt, &add(u.clone(), num(1.0))))),
        Atanh => div(num(1.0), sub(num(1.0), sq(u))),
        Exp => c1(Exp, u),
        Ln => div(num(1.0), u.clone()),
        Log10 => div(num(1.0), mul(u.clone(), num(std::f64::consts::LN_10))),
        Sqrt => div(num(1.0), mul(num(2.0), c1(Sqrt, u))),
        Floor | Ceil | Round | Sign => num(0.0),
        Re | Im | Conj => return None, // 線形なので、呼び出し側で中身の微分に掛ける
        Gamma => mul(c1(Gamma, u), c1(Digamma, u)),
        LogGamma => c1(Digamma, u),
        Digamma => call(Polygamma, vec![num(1.0), u.clone()]),
        Erf => mul(num(2.0 / sqrt_pi), c1(Exp, &neg(sq(u)))),
        Erfc => mul(num(-2.0 / sqrt_pi), c1(Exp, &neg(sq(u)))),
        Erfi => mul(num(2.0 / sqrt_pi), c1(Exp, &sq(u))),
        Dawson => sub(num(1.0), mul(mul(num(2.0), u.clone()), c1(Dawson, u))),
        FresnelS => c1(Sin, &mul(num(std::f64::consts::FRAC_PI_2), sq(u))),
        FresnelC => c1(Cos, &mul(num(std::f64::consts::FRAC_PI_2), sq(u))),
        Ei => div(c1(Exp, u), u.clone()),
        LogInt => div(num(1.0), c1(Ln, u)),
        SinInt => div(c1(Sin, u), u.clone()),
        CosInt => div(c1(Cos, u), u.clone()),
        SinhInt => div(c1(Sinh, u), u.clone()),
        CoshInt => div(c1(Cosh, u), u.clone()),
        AiryAi => c1(AiryAiPrime, u),
        AiryBi => c1(AiryBiPrime, u),
        AiryAiPrime => mul(u.clone(), c1(AiryAi, u)),
        AiryBiPrime => mul(u.clone(), c1(AiryBi, u)),
        _ => return None,
    })
}

/// 関数 f の、k 番目の引数での偏微分（表に無ければ中心差分）
fn partial(f: Func, args: &[Expr], k: usize) -> Expr {
    use Func::*;
    if args.len() == 1 {
        return derivative1(f, &args[0]).unwrap_or_else(|| numeric_partial(f, args, 0));
    }
    let (a, b) = (&args[0], &args[1]);
    let half = |s: f64| div(add(num(1.0), mul(num(s), c1(Sign, &sub(a.clone(), b.clone())))), num(2.0));
    match (f, k, args.len()) {
        // mod(a, b) = a − b ⌊a/b⌋
        (Mod, 0, 2) => num(1.0),
        (Mod, 1, 2) => neg(c1(Floor, &div(a.clone(), b.clone()))),
        // max(a, b) = (a + b + |a − b|)/2
        (Max, 0, 2) => half(1.0),
        (Max, 1, 2) => half(-1.0),
        (Min, 0, 2) => half(-1.0),
        (Min, 1, 2) => half(1.0),
        // ベッセル関数の z での微分（漸化式）。次数 ν での微分は中心差分
        (BesselJ | BesselY, 1, 2) => {
            div(sub(call(f, vec![sub(a.clone(), num(1.0)), b.clone()]), call(f, vec![add(a.clone(), num(1.0)), b.clone()])), num(2.0))
        }
        (BesselI, 1, 2) => {
            div(add(call(f, vec![sub(a.clone(), num(1.0)), b.clone()]), call(f, vec![add(a.clone(), num(1.0)), b.clone()])), num(2.0))
        }
        (BesselK, 1, 2) => neg(div(
            add(call(f, vec![sub(a.clone(), num(1.0)), b.clone()]), call(f, vec![add(a.clone(), num(1.0)), b.clone()])),
            num(2.0),
        )),
        (Polygamma, 1, 2) => call(Polygamma, vec![add(a.clone(), num(1.0)), b.clone()]),
        _ => numeric_partial(f, args, k),
    }
}

/// `e` を変数 `var` で微分する（`e` は展開の後の、今までの節だけの式）
pub fn diff(e: &Expr, var: &str) -> Result<Expr, String> {
    if !e.uses_var(var) {
        return Ok(match e {
            Expr::Tuple(items) => Expr::Tuple(items.iter().map(|_| num(0.0)).collect()),
            Expr::List(items) => Expr::List(items.iter().map(|_| num(0.0)).collect()),
            _ => num(0.0),
        });
    }
    Ok(match e {
        Expr::Num(_) | Expr::Const(_) => num(0.0),
        Expr::Var(v) => num(if v == var { 1.0 } else { 0.0 }),
        Expr::Neg(a) => neg(diff(a, var)?),
        Expr::Bin(op, a, b) => {
            let (da, db) = (diff(a, var)?, diff(b, var)?);
            let (a, b) = ((**a).clone(), (**b).clone());
            match op {
                BinOp::Add => add(da, db),
                BinOp::Sub => sub(da, db),
                BinOp::Mul => add(mul(da, b), mul(a, db)),
                // (a/b)' = (a' b − a b') / b²
                BinOp::Div => div(sub(mul(da, b.clone()), mul(a, db)), sq(&b)),
                BinOp::Pow => {
                    if !b.uses_var(var) {
                        // (u^c)' = c u^(c−1) u'
                        mul(mul(b.clone(), pow(a, sub(b, num(1.0)))), da)
                    } else if !a.uses_var(var) {
                        // (c^v)' = c^v ln(c) v'
                        mul(mul(pow(a.clone(), b), c1(Func::Ln, &a)), db)
                    } else {
                        // (u^v)' = u^v (v' ln u + v u'/u)
                        mul(pow(a.clone(), b.clone()), add(mul(db, c1(Func::Ln, &a)), div(mul(b, da), a)))
                    }
                }
            }
        }
        // 描かない点の印 Guard(v, m) は、v だけを微分する
        Expr::Call(Func::Guard, args) if args.len() == 2 => call(Func::Guard, vec![diff(&args[0], var)?, args[1].clone()]),
        // 場合分けは、値だけを微分する（条件はそのまま）
        Expr::Call(Func::Piece, args) => {
            let mut out = args.clone();
            let mut pos = 0;
            while pos + 2 < args.len() {
                let k = as_num(&args[pos]).unwrap_or(0.0) as usize;
                let v = pos + 2 + k;
                if v >= args.len() {
                    break;
                }
                out[v] = diff(&args[v], var)?;
                pos = v + 1;
            }
            // 条件の境目（余裕 = 0）は、枝の導関数がそろわなければ折れ目
            let mut d = call(Func::Piece, out);
            let mut pos = 0;
            while pos + 2 < args.len() {
                let k = as_num(&args[pos]).unwrap_or(0.0) as usize;
                for margin in args.iter().skip(pos + 2).take(k) {
                    if margin.uses_var(var) {
                        d = kink(d, margin.clone());
                    }
                }
                pos += 2 + k + 1;
            }
            d
        }
        Expr::Call(f, args) => {
            match (f, args.as_slice()) {
                (Func::Re | Func::Im | Func::Conj, [u]) => return Ok(c1(*f, &diff(u, var)?)),
                // |u|' = Re(ū u')/|u|（実数の式なら sign(u) u'）
                (Func::Abs, [u]) => {
                    let du = diff(u, var)?;
                    return Ok(if has_i(u) || has_i(&du) {
                        div(c1(Func::Re, &mul(c1(Func::Conj, u), du)), c1(Func::Abs, u))
                    } else {
                        kink(mul(c1(Func::Sign, u), du), u.clone())
                    });
                }
                // arg(u)' = Im(u'/u)
                (Func::Arg, [u]) => return Ok(c1(Func::Im, &div(diff(u, var)?, u.clone()))),
                // 段差の上では導関数が無い（round は半整数、sign は 0 で跳ぶ）
                (Func::Floor | Func::Ceil, [u]) => return Ok(jump(num(0.0), off_integer(u))),
                (Func::Round, [u]) => return Ok(jump(num(0.0), off_integer(&sub(u.clone(), num(0.5))))),
                (Func::Sign, [u]) => return Ok(jump(num(0.0), u.clone())),
                _ => {}
            }
            let mut total = num(0.0);
            for (k, a) in args.iter().enumerate() {
                if !a.uses_var(var) {
                    continue;
                }
                total = add(total, mul(partial(*f, args, k), diff(a, var)?));
            }
            match (f, args.as_slice()) {
                // min・max は a = b の所で折れる。mod(a, b) は a/b が整数の所で跳ぶ
                (Func::Min | Func::Max, [a, b]) => kink(total, sub(a.clone(), b.clone())),
                (Func::Mod, [a, b]) => jump(total, off_integer(&div(a.clone(), b.clone()))),
                _ => total,
            }
        }
        Expr::Tuple(items) => Expr::Tuple(items.iter().map(|a| diff(a, var)).collect::<Result<_, _>>()?),
        Expr::List(items) => Expr::List(items.iter().map(|a| diff(a, var)).collect::<Result<_, _>>()?),
        _ => return Err("内部エラー: 展開されていない式は微分できません".into()),
    })
}

/// k 回微分する
pub fn diff_n(e: &Expr, var: &str, k: usize) -> Result<Expr, String> {
    let mut out = e.clone();
    for _ in 0..k {
        out = diff(&out, var)?;
    }
    Ok(out)
}

/// 利用者が書いた微分（`d/dx`・`f′`）: [`diff_n`] に、導関数が存在しない点の印を付ける（#161）。
/// 印は値を変えないか（折れ目）、段差の上だけ NaN にする。停留点・テイラー展開などの中の微分には付けない
pub fn diff_n_marked(e: &Expr, var: &str, k: usize) -> Result<Expr, String> {
    let before = MARK.replace(true);
    let out = diff_n(e, var, k);
    MARK.set(before);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Program;
    use crate::parser::parse;
    use num_complex::Complex64 as C;

    fn eval_at(e: &Expr, x: f64) -> C {
        let p = Program::compile(e, &["x"]).unwrap();
        let mut out = [C::new(0.0, 0.0)];
        p.eval_batch(&[&[x]], &mut out);
        out[0]
    }

    /// 記号微分と中心差分（h = 1e-6）を、点 xs で比べる
    fn check(src: &str, xs: &[f64]) {
        let e = parse(src).unwrap();
        let d = diff(&e, "x").unwrap();
        for &x in xs {
            let h = 1e-6 * x.abs().max(1.0);
            let num = (eval_at(&e, x + h) - eval_at(&e, x - h)) / (2.0 * h);
            let sym = eval_at(&d, x);
            let err = (num - sym).norm() / sym.norm().max(1.0);
            assert!(err < 1e-6, "{src} の x = {x}: 記号 {sym}, 差分 {num}");
        }
    }

    #[test]
    fn elementary_functions() {
        let xs = [0.3, 0.7, 1.3, 2.1];
        for f in ["sin", "cos", "tan", "sec", "csc", "cot", "sinh", "cosh", "tanh", "asinh", "exp", "ln", "log", "sqrt", "atan"] {
            check(&format!("{f}(x^2 + 0.5 x)"), &xs);
        }
        for f in ["arcsin", "arccos", "arctanh"] {
            check(&format!("{f}(x/3)"), &xs);
        }
        check("arccosh(x + 1.5)", &xs);
        check("x^3 - 2x + 1/x", &xs);
        check("2^x + x^x + x^(1/3)", &xs);
        check("abs(x - 1) + floor(x) + sign(x - 1)", &[0.3, 0.7, 1.4, 2.1]);
        check("mod(3x, 2) + max(x, 1) + min(x^2, 1)", &[0.3, 0.7, 1.3, 2.1]);
    }

    #[test]
    fn complex_values() {
        let xs = [0.3, 0.7, 1.3];
        check("abs(x + 2i)", &xs);
        check("arg(x + 2i) + Re(exp(i x)) + Im(x^2 i) + conj(x + i x)", &xs);
        check("exp(i x^2)", &xs);
    }

    #[test]
    fn special_functions() {
        let xs = [0.4, 1.2, 2.3];
        for f in ["gamma", "loggamma", "digamma", "erf", "erfc", "erfi", "dawson", "fresnelS", "fresnelC", "Ei", "Si", "Ci", "Shi", "Chi", "airyAi", "airyBi", "airyAiPrime", "airyBiPrime", "zeta", "ellipticK"] {
            check(&format!("{f}(x/2 + 0.3)"), &xs);
        }
        check("li(x + 1.5)", &xs);
        for f in ["besselJ", "besselY", "besselI", "besselK"] {
            check(&format!("{f}(1.5, x + 0.5)"), &xs);
            check(&format!("{f}(x, 2.5)"), &xs);
        }
        check("polygamma(2, x + 0.5)", &xs);
    }

    #[test]
    fn simplification_keeps_size_small() {
        let e = parse("sin(x^2) cos(x)").unwrap();
        let d4 = diff_n(&e, "x", 4).unwrap();
        assert!(d4.node_count() < 3_000, "{}", d4.node_count());
        assert_eq!(diff(&parse("y^2 + 3").unwrap(), "x").unwrap(), num(0.0));
        assert_eq!(diff(&parse("x").unwrap(), "x").unwrap(), num(1.0));
    }
}

//! 式を変数について解く（記号で）。
//!
//! - [`quadratic`]: g を変数 v の 2 次以下の式 a v² + b v + c と見たときの係数。係数は v = 0, ±1 を入れた式から作り、
//!   いくつかの点で g と一致するかを数で確かめる（多項式に展開しなくても、x² + y² + z² − 1 や (x − z)² + y² − 4 を扱える）
//! - [`roots`]: g = 0 を v について解いた式の並び（2 次以下なら根の公式、v が 1 か所にしか出てこなければ孤立法）
//!
//! 不等式の領域の境目の面 G = 0 を、陰関数（格子）ではなく陽関数 v = f(…) として作るのに使う（`system.rs` の [`crate::system::region_faces`]）。
//! 不等式を変数の範囲に直すもの（積分の領域）は `expand.rs` の `isolate`

use crate::diff::{add, div, mul, sub};
use crate::eval::{Params, Program};
use crate::functions::Func;
use crate::parser::{BinOp, Expr};
use num_complex::Complex64 as C;

/// v の 2 次以下の式 a v² + b v + c の (a, b, c) と、a・b が定数ならその値
pub type Quadratic = (Expr, Expr, Expr, Option<f64>, Option<f64>);

/// g（変数 v と、`names` のほかの変数の式）を v の 2 次以下の式と見たときの係数。一致しなければ None
pub fn quadratic(g: &Expr, v: &str, names: &[&str], params: &Params) -> Option<Quadratic> {
    let at = |t: f64| g.substitute(&|name: &str| (name == v).then_some(Expr::Num(t)));
    let (c, p1, m1) = (at(0.0), at(1.0), at(-1.0));
    let b = div(sub(p1.clone(), m1.clone()), Expr::Num(2.0));
    let a = sub(div(add(p1, m1), Expr::Num(2.0)), c.clone());
    let k = names.iter().position(|n| *n == v)?;
    let progs: Vec<Program> = [g, &a, &b, &c].iter().map(|e| Program::compile_with(e, names, params).ok()).collect::<Option<_>>()?;
    // 確かめる点（どの変数にも、ありふれた値にならない数）
    let pts = [[0.37, -0.81, 0.53], [-1.29, 0.44, 1.71], [0.93, 1.37, -0.66], [2.13, -1.57, -1.21], [-0.52, 0.71, 0.29]];
    let mut checked = 0;
    // 確かめた点での a, b の値（どの点でも同じなら定数）
    let (mut a_seen, mut b_seen): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    for pt in pts {
        let cols: Vec<[f64; 1]> = (0..names.len()).map(|i| [pt[i % 3]]).collect();
        let inputs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
        let val = |p: &Program| {
            let mut out = [C::new(0.0, 0.0)];
            p.eval_batch(&inputs, &mut out);
            crate::eval::as_real(out[0])
        };
        let (gv, av, bv, cv) = (val(&progs[0]), val(&progs[1]), val(&progs[2]), val(&progs[3]));
        if !(gv.is_finite() && av.is_finite() && bv.is_finite() && cv.is_finite()) {
            continue;
        }
        let t = pt[k % 3];
        let fit = av * t * t + bv * t + cv;
        if (gv - fit).abs() > 1e-9 * (gv.abs() + (av * t * t).abs() + (bv * t).abs() + cv.abs() + 1.0) {
            return None;
        }
        checked += 1;
        a_seen.push(av);
        b_seen.push(bv);
    }
    let same = |xs: &[f64]| xs.first().copied().filter(|&x0| xs.iter().all(|x| (x - x0).abs() <= 1e-12 * (1.0 + x0.abs())));
    (checked >= 3).then_some((a, b, c, same(&a_seen), same(&b_seen)))
}

/// g = 0 を v について解いた式の並び（v を含まない式。`names` のほかの変数の式）。解けなければ None。
/// 解いた式が定義されない所（√ の中が負）は NaN になる。余分な解（孤立法で 2 乗したときの枝）を含むことがあるので、
/// 使う側で g ≈ 0 を確かめる
pub fn roots(g: &Expr, v: &str, names: &[&str], params: &Params) -> Option<Vec<Expr>> {
    if !g.uses_var(v) {
        return None;
    }
    if let Some((a, b, c, a_const, _)) = quadratic(g, v, names, params) {
        let neg = |e: Expr| Expr::Neg(Box::new(e));
        // 1 次式（a が 0）: v = −c / b
        if a_const.is_some_and(|a| a.abs() <= 1e-12) {
            return Some(vec![div(neg(c), b)]);
        }
        // 2 次式: v = (−b ± √(b² − 4ac)) / (2a)
        let disc = sub(mul(b.clone(), b.clone()), mul(mul(Expr::Num(4.0), a.clone()), c));
        let root = Expr::Call(Func::Sqrt, vec![disc]);
        let two_a = mul(Expr::Num(2.0), a);
        return Some(vec![div(add(neg(b.clone()), root.clone()), two_a.clone()), div(sub(neg(b), root), two_a)]);
    }
    let mut out = Vec::new();
    solve_into(g, Expr::Num(0.0), v, &mut out)?;
    Some(out)
}

/// 和・差を (符号, 項) に分ける
fn add_terms(e: &Expr, sign: f64, out: &mut Vec<(f64, Expr)>) {
    match e {
        Expr::Bin(BinOp::Add, a, b) => {
            add_terms(a, sign, out);
            add_terms(b, sign, out);
        }
        Expr::Bin(BinOp::Sub, a, b) => {
            add_terms(a, sign, out);
            add_terms(b, -sign, out);
        }
        Expr::Neg(a) => add_terms(a, -sign, out),
        _ => out.push((sign, e.clone())),
    }
}

fn number(e: &Expr) -> Option<f64> {
    match e {
        Expr::Num(k) => Some(*k),
        Expr::Neg(a) => number(a).map(|k| -k),
        _ => None,
    }
}

/// u = rhs を v について解いて、解を out に足す（孤立法: u の外側の演算から順に逆の演算を当てる）。解けなければ None
fn solve_into(u: &Expr, rhs: Expr, v: &str, out: &mut Vec<Expr>) -> Option<()> {
    let neg = |e: Expr| Expr::Neg(Box::new(e));
    if matches!(u, Expr::Var(n) if n == v) {
        out.push(rhs);
        return Some(());
    }
    match u {
        Expr::Bin(BinOp::Add | BinOp::Sub, ..) | Expr::Neg(_) => {
            // u = A + σ w（w だけが v を含む）: w = σ (rhs − A)
            let mut terms = Vec::new();
            add_terms(u, 1.0, &mut terms);
            let with: Vec<&(f64, Expr)> = terms.iter().filter(|t| t.1.uses_var(v)).collect();
            if with.len() != 1 {
                return None;
            }
            let (sigma, w) = with[0].clone();
            let mut rest = rhs;
            for (s, t) in terms.iter().filter(|t| !t.1.uses_var(v)) {
                rest = if *s > 0.0 { sub(rest, t.clone()) } else { add(rest, t.clone()) };
            }
            solve_into(&w, if sigma > 0.0 { rest } else { neg(rest) }, v, out)
        }
        // 片方が v を含まない積・商（係数が座標の式でもよい: 2x z = 1 → z = 1 / (2x)）
        Expr::Bin(BinOp::Mul, a, b) if !a.uses_var(v) => solve_into(b, div(rhs, (**a).clone()), v, out),
        Expr::Bin(BinOp::Mul, a, b) if !b.uses_var(v) => solve_into(a, div(rhs, (**b).clone()), v, out),
        Expr::Bin(BinOp::Div, a, b) if !b.uses_var(v) => solve_into(a, mul(rhs, (**b).clone()), v, out),
        Expr::Bin(BinOp::Div, a, b) if !a.uses_var(v) => solve_into(b, div((**a).clone(), rhs), v, out),
        Expr::Call(Func::Sqrt, args) if args.len() == 1 => solve_into(&args[0], mul(rhs.clone(), rhs), v, out),
        Expr::Call(Func::Abs, args) if args.len() == 1 => {
            solve_into(&args[0], rhs.clone(), v, out)?;
            solve_into(&args[0], neg(rhs), v, out)
        }
        Expr::Call(Func::Exp, args) if args.len() == 1 => solve_into(&args[0], Expr::Call(Func::Ln, vec![rhs]), v, out),
        Expr::Call(Func::Ln, args) if args.len() == 1 => solve_into(&args[0], Expr::Call(Func::Exp, vec![rhs]), v, out),
        Expr::Bin(BinOp::Pow, w, e) if !e.uses_var(v) => {
            let k = number(e)?;
            if k == 2.0 {
                let r = Expr::Call(Func::Sqrt, vec![rhs]);
                solve_into(w, r.clone(), v, out)?;
                solve_into(w, neg(r), v, out)
            } else if k.fract() == 0.0 && k.rem_euclid(2.0) == 1.0 {
                // 奇数乗: 実数の k 乗根（符号を保つ）sign(r) |r|^(1/k)
                let root = mul(Expr::Call(Func::Sign, vec![rhs.clone()]), Expr::Bin(BinOp::Pow, Box::new(Expr::Call(Func::Abs, vec![rhs])), Box::new(Expr::Num(1.0 / k))));
                solve_into(w, root, v, out)
            } else if k != 0.0 {
                solve_into(w, Expr::Bin(BinOp::Pow, Box::new(rhs), Box::new(Expr::Num(1.0 / k))), v, out)
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    /// g = 0 の解を、いくつかの (x, y) で計算して、g に戻したときに 0 になるか
    fn check(src: &str, v: &str, count: usize) {
        let g = parse(src).unwrap();
        let names = ["x", "y", "z"];
        let rs = roots(&g, v, &names, &Params::new()).unwrap_or_else(|| panic!("{src} を {v} について解けない"));
        assert_eq!(rs.len(), count, "{src}");
        let gp = Program::compile(&g, &names).unwrap();
        let k = names.iter().position(|n| *n == v).unwrap();
        let others: Vec<&str> = names.iter().copied().filter(|n| *n != v).collect();
        let mut hits = 0;
        for r in &rs {
            let rp = Program::compile(r, &others).unwrap();
            for (p, q) in [(0.3, 0.2), (0.1, 0.15), (0.2, 0.05)] {
                let mut out = [C::new(0.0, 0.0)];
                rp.eval_batch(&[&[p], &[q]], &mut out);
                let t = crate::eval::as_real(out[0]);
                if !t.is_finite() {
                    continue;
                }
                // 解いた変数の所に t、ほかの所に順に p, q
                let mut rest = [p, q].into_iter();
                let pt: [f64; 3] = std::array::from_fn(|i| if i == k { t } else { rest.next().unwrap_or(0.0) });
                let mut o = [C::new(0.0, 0.0)];
                gp.eval_batch(&[&[pt[0]], &[pt[1]], &[pt[2]]], &mut o);
                if crate::eval::as_real(o[0]).abs() < 1e-9 {
                    hits += 1;
                }
            }
        }
        assert!(hits >= 3, "{src}: 解が g = 0 に乗らない");
    }

    #[test]
    fn solves_common_surfaces() {
        check("z - (x^3 + y^3)", "z", 1);
        check("x^2 + y^2 + z^2 - 1", "z", 2);
        check("x^2 + y^2 - 1", "x", 2);
        check("sqrt(x) + sqrt(y) + sqrt(z) - 1", "z", 1);
        check("abs(x) + abs(y) + abs(z) - 1", "z", 2);
        check("exp(z) - x - 2", "z", 1);
        check("z^3 - x y", "z", 1);
        check("2 x z - 1", "z", 1);
        assert!(roots(&parse("sin(z) - x").unwrap(), "z", &["x", "y", "z"], &Params::new()).is_none());
    }
}

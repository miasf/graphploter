//! 厳密値の道の微分と代入（計画 `docs/plan-exact-areas.md` の DF0）。
//!
//! 規則は `diff.rs`（今の道の記号微分）の表と同じ。表に無い関数は「見つからない」（今の道の中心差分の代わりは無い）。
//! 代入は作り直しながら行う（正規化と特別な値の表が効く: $\sin x$ に $x = \frac{\pi}{6}$ を入れると $\frac12$）

use super::term::{add, call, mul, neg, pow, Term};
use super::Stop;
use crate::functions::Func;

fn has(t: &Term, x: &str) -> bool {
    match t {
        Term::Sym(s) => s == x,
        Term::Num(_) | Term::Atom(_) => false,
        Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) => v.iter().any(|a| has(a, x)),
        Term::Pow(b, e) => has(b, x) || has(e, x),
    }
}

/// $\frac{d}{dx}t$
pub fn diff(t: &Term, x: &str) -> Result<Term, Stop> {
    if super::exhausted() {
        return Err(Stop::TimeUp);
    }
    if !has(t, x) {
        return Ok(Term::zero());
    }
    Ok(match t {
        Term::Sym(_) => Term::one(),
        Term::Add(v) => add(v.iter().map(|a| diff(a, x)).collect::<Result<_, _>>()?),
        Term::Mul(v) => {
            // 積の微分: Σ_i f_i' Π_{j≠i} f_j
            let mut parts = Vec::new();
            for i in 0..v.len() {
                if !has(&v[i], x) {
                    continue;
                }
                let mut fs: Vec<Term> = v.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, f)| f.clone()).collect();
                fs.push(diff(&v[i], x)?);
                parts.push(mul(fs));
            }
            add(parts)
        }
        Term::Pow(b, e) => {
            let (b, e) = (&**b, &**e);
            if !has(e, x) {
                // (b^e)' = e b^{e−1} b'
                mul(vec![e.clone(), pow(b.clone(), add(vec![e.clone(), Term::int(-1)])), diff(b, x)?])
            } else if !has(b, x) {
                // (c^e)' = c^e ln c · e'
                mul(vec![t.clone(), call(Func::Ln, vec![b.clone()]), diff(e, x)?])
            } else {
                // (b^e)' = b^e (e' ln b + e b'/b)
                let a = mul(vec![diff(e, x)?, call(Func::Ln, vec![b.clone()])]);
                let c = mul(vec![e.clone(), diff(b, x)?, pow(b.clone(), Term::int(-1))]);
                mul(vec![t.clone(), add(vec![a, c])])
            }
        }
        Term::Fn(f, args) => {
            let [u] = args.as_slice() else { return Err(Stop::NotFound(format!("{f:?} の微分はまだ厳密に計算できません"))) };
            let du = diff(u, x)?;
            let inv = |t: Term| pow(t, Term::int(-1));
            let outer = match f {
                Func::Sin => call(Func::Cos, vec![u.clone()]),
                Func::Cos => neg(call(Func::Sin, vec![u.clone()])),
                Func::Tan => inv(pow(call(Func::Cos, vec![u.clone()]), Term::int(2))),
                Func::Sec => mul(vec![call(Func::Sec, vec![u.clone()]), call(Func::Tan, vec![u.clone()])]),
                Func::Csc => neg(mul(vec![call(Func::Csc, vec![u.clone()]), call(Func::Cot, vec![u.clone()])])),
                Func::Cot => neg(inv(pow(call(Func::Sin, vec![u.clone()]), Term::int(2)))),
                Func::Exp => call(Func::Exp, vec![u.clone()]),
                Func::Ln => inv(u.clone()),
                Func::Log10 => inv(mul(vec![u.clone(), call(Func::Ln, vec![Term::int(10)])])),
                Func::Sqrt => mul(vec![Term::rat(1, 2), pow(u.clone(), Term::rat(-1, 2))]),
                Func::Asin => pow(add(vec![Term::one(), neg(pow(u.clone(), Term::int(2)))]), Term::rat(-1, 2)),
                Func::Acos => neg(pow(add(vec![Term::one(), neg(pow(u.clone(), Term::int(2)))]), Term::rat(-1, 2))),
                Func::Atan => inv(add(vec![Term::one(), pow(u.clone(), Term::int(2))])),
                Func::Sinh => call(Func::Cosh, vec![u.clone()]),
                Func::Cosh => call(Func::Sinh, vec![u.clone()]),
                Func::Tanh => inv(pow(call(Func::Cosh, vec![u.clone()]), Term::int(2))),
                Func::Asinh => pow(add(vec![pow(u.clone(), Term::int(2)), Term::one()]), Term::rat(-1, 2)),
                // |u|′ = u u′ / |u|（u = 0 では存在しない。代入で 0 で割ると「見つからない」）
                Func::Abs => mul(vec![u.clone(), inv(call(Func::Abs, vec![u.clone()]))]),
                _ => return Err(Stop::NotFound(format!("{f:?} の微分はまだ厳密に計算できません"))),
            };
            mul(vec![outer, du])
        }
        Term::Tuple(v) => Term::Tuple(v.iter().map(|a| diff(a, x)).collect::<Result<_, _>>()?),
        Term::List(v) => Term::List(v.iter().map(|a| diff(a, x)).collect::<Result<_, _>>()?),
        Term::Num(_) | Term::Atom(_) => Term::zero(),
    })
}

/// 文字 `x` に `v` を入れて作り直す（正規化と特別な値の表が効く）
pub fn subst(t: &Term, x: &str, v: &Term) -> Term {
    match t {
        Term::Sym(s) if s == x => v.clone(),
        Term::Num(_) | Term::Atom(_) | Term::Sym(_) => t.clone(),
        Term::Add(a) => add(a.iter().map(|y| subst(y, x, v)).collect()),
        Term::Mul(a) => mul(a.iter().map(|y| subst(y, x, v)).collect()),
        Term::Pow(b, e) => pow(subst(b, x, v), subst(e, x, v)),
        Term::Fn(f, a) => call(*f, a.iter().map(|y| subst(y, x, v)).collect()),
        Term::Tuple(a) => Term::Tuple(a.iter().map(|y| subst(y, x, v)).collect()),
        Term::List(a) => Term::List(a.iter().map(|y| subst(y, x, v)).collect()),
    }
}

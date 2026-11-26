//! 定積分・広義積分（計画 `docs/plan-exact-areas.md` の IG2）。
//!
//! 1. よく出る型の表（原始関数が無いか、∞ の端の極限が LM2 の要るもの）: ガンマの型 $\int_0^\infty P(x)e^{-cx}dx$、ガウス積分、
//!    ディリクレ積分 $\int_0^\infty \frac{\sin x}{x}dx$、$\int_0^\infty e^{-x}\ln x\,dx = -\gamma$、ベータの型 $\int_0^\infty \frac{x^m}{1 + x^n}dx$、
//!    $\int_0^1 \frac{\arctan x}{x}dx = G$、$\int_0^{2\pi}\frac{dx}{a + b\cos x} = \frac{2\pi}{\sqrt{a^2 - b^2}}$
//! 2. 区間の中の特異点（分母の 0・対数の真数の 0・$\tan$ の極）と、$\lvert g\rvert$ の折れ目で区間を分ける。
//!    特異点が有理数で求まらない（無理数の根かもしれない）ときは「見つからない」
//! 3. 各区間で原始関数（IG0）を求め、端の値を片側の極限（LM1）で取る（微積分の基本定理）。極限が ±∞ なら「発散します」
//!
//! 原始関数は、中身が区間の中で連続なら連続な形で作る（$\ln\lvert u\rvert$・1 次式の中身の $\arctan$）。
//! $\tan\frac{x}{2}$ の置換（区間の中で不連続になる原始関数）は使わない

use super::limit::{limit, Lim, Point};
use super::num::{self, Int, Nat, Rat};
use super::poly;
use super::term::{add, call, mul, neg, pow, Atom, Term};
use super::{ball, Stop};
use crate::functions::Func;

fn nf(m: &str) -> Stop {
    Stop::NotFound(m.to_string())
}

fn has(t: &Term, x: &str) -> bool {
    match t {
        Term::Sym(s) => s == x,
        Term::Num(_) | Term::Atom(_) => false,
        Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) | Term::Matrix(_, _, v) => v.iter().any(|a| has(a, x)),
        Term::Pow(b, e) => has(b, x) || has(e, x),
    }
}

fn is_zero_pt(p: &Point) -> bool {
    matches!(p, Point::Finite(t) if t.is_zero())
}

fn to_f64(p: &Point) -> f64 {
    match p {
        Point::PlusInf => f64::INFINITY,
        Point::MinusInf => f64::NEG_INFINITY,
        Point::Finite(t) => ball::value(t, 64).map(|b| b.to_f64().0).unwrap_or(f64::NAN),
    }
}

/// 端の値（片側の極限）。side は 1（右から: 下端）か −1（左から: 上端）
fn end_value(g: &Term, x: &str, p: &Point, side: i32, singular: bool) -> Result<Term, Stop> {
    if let (Point::Finite(v), false) = (p, singular) {
        if let Ok(t) = super::diff::subst_checked(g, x, v) {
            if t.is_number() && ball::value(&t, 64).is_ok() {
                return Ok(t);
            }
        }
    }
    match limit(g, x, p, side) {
        Ok(Lim::Value(t)) => Ok(t),
        Ok(Lim::PlusInf) | Ok(Lim::MinusInf) => Err(Stop::NotExist("発散します（原始関数の端の極限が ±∞）".into())),
        Err(Stop::NotExist(r)) => Err(Stop::NotExist(format!("発散します（{r}）"))),
        Err(e) => Err(e),
    }
}

/// 区間の中（端を除く）の特異点と折れ目。有理数で求まらないものがあれば Err
fn split_points(f: &Term, x: &str, lo: f64, hi: f64) -> Result<(Vec<Rat>, Vec<Rat>), Stop> {
    let mut guards: Vec<(Term, bool /* 折れ目 */)> = Vec::new();
    collect_guards(f, &mut guards);
    let mut sing = Vec::new();
    let mut kinks = Vec::new();
    for (g, kink) in guards {
        if !has(&g, x) {
            continue;
        }
        let roots = zeros_inside(&g, x, lo, hi)?;
        for r in roots {
            if kink { kinks.push(r) } else { sing.push(r) }
        }
    }
    sing.sort();
    sing.dedup();
    kinks.sort();
    kinks.dedup();
    Ok((sing, kinks))
}

/// 0 になると困る部分式（分母・対数の真数・分数べきの底・tan の cos）と、折れ目（|g| の g）
fn collect_guards(t: &Term, out: &mut Vec<(Term, bool)>) {
    match t {
        Term::Pow(b, e) => {
            if let Some(q) = e.as_rat() {
                if num::sign(q) < 0 || *q.denominator() != Nat::ONE {
                    out.push(((**b).clone(), false));
                }
            }
            collect_guards(b, out);
            collect_guards(e, out);
        }
        Term::Fn(f, a) => {
            for y in a {
                collect_guards(y, out);
            }
            if let [g] = a.as_slice() {
                match f {
                    Func::Ln | Func::Log10 => out.push((g.clone(), false)),
                    Func::Tan | Func::Sec => out.push((call(Func::Cos, vec![g.clone()]), false)),
                    Func::Cot | Func::Csc => out.push((call(Func::Sin, vec![g.clone()]), false)),
                    Func::Abs => out.push((g.clone(), true)),
                    _ => {}
                }
            }
        }
        Term::Add(v) | Term::Mul(v) => {
            for y in v {
                collect_guards(y, out);
            }
        }
        _ => {}
    }
}

/// g の 0 で、(lo, hi) の中にあるもの（有理数）。有理数でない 0 がありそうなら Err
fn zeros_inside(g: &Term, x: &str, lo: f64, hi: f64) -> Result<Vec<Rat>, Stop> {
    let inside = |v: f64| v > lo && v < hi;
    let mut out = Vec::new();
    let mut rest_poly = None;
    if let Some(p) = poly::to_poly(g, x) {
        let budget = super::Budget::new(2_000_000);
        let rs = poly::rational_roots(&p, &budget).map_err(|_| nf("特異点を求められません"))?;
        let mut rest = p.clone();
        for r in &rs {
            rest = rest.divrem(&poly::Poly::new(vec![-r.clone(), Rat::ONE])).0;
            if inside(r.to_f64().value()) {
                out.push(r.clone());
            }
        }
        rest_poly = Some(rest);
    }
    // 残り（有理数でない 0）を数値で探す: 符号の変わり目か、とても小さい値があれば諦める
    let sample = |t: f64| -> Option<f64> {
        let v = Term::Num(num::rat_of_literal(t)?);
        let e = match &rest_poly {
            Some(p) => Term::Num(p.eval(v.as_rat()?)),
            None => super::diff::subst(g, x, &v),
        };
        ball::value(&e, 64).ok().map(|b| b.to_f64().0)
    };
    let map = |s: f64| -> f64 {
        // 無限の区間は s ∈ (0, 1) を写す
        match (lo.is_finite(), hi.is_finite()) {
            (true, true) => lo + (hi - lo) * s,
            (true, false) => lo + s / (1.0 - s),
            (false, true) => hi - (1.0 - s) / s,
            (false, false) => (s - 0.5) / (s * (1.0 - s)),
        }
    };
    let n = 400;
    let mut prev: Option<f64> = None;
    for i in 1..n {
        let t = map(i as f64 / n as f64);
        if !t.is_finite() {
            continue;
        }
        let Some(v) = sample(t) else { continue };
        if let Some(p) = prev {
            if (p > 0.0) != (v > 0.0) && !out.iter().any(|r| (r.to_f64().value() - t).abs() < (hi - lo).abs().min(1.0) * 4.0 / n as f64) {
                return Err(nf("区間の中に、有理数でない特異点がありそうです"));
            }
        }
        prev = Some(v);
    }
    Ok(out)
}

fn pi() -> Term {
    Term::Atom(Atom::Pi)
}

/// 表の型（当てはまらなければ None）
fn table(f: &Term, x: &str, a: &Point, b: &Point) -> Result<Option<Term>, Stop> {
    let xs = Term::Sym(x.into());
    let zero_to_inf = is_zero_pt(a) && matches!(b, Point::PlusInf);
    if zero_to_inf {
        // P(x) e^{−c x}（c > 0）: Σ p_n n!/c^{n+1}
        let fs: Vec<Term> = match f {
            Term::Mul(v) => v.clone(),
            other => vec![other.clone()],
        };
        let mut c: Option<Term> = None;
        let mut rest = Vec::new();
        for t in &fs {
            if let Term::Pow(bb, e) = t {
                if **bb == Term::Atom(Atom::E) {
                    if let Some(p) = poly::to_poly(e, x) {
                        if p.deg() == 1 && p.0[0].is_zero() && num::sign(&p.0[1]) < 0 && c.is_none() {
                            c = Some(Term::Num(-p.0[1].clone()));
                            continue;
                        }
                        if p.deg() == 2 && p.0[0].is_zero() && p.0[1].is_zero() && num::sign(&p.0[2]) < 0 && fs.len() == 1 {
                            // ガウス: ∫_0^∞ e^{−c x²} = √π / (2√c)
                            let cc = Term::Num(-p.0[2].clone());
                            return Ok(Some(mul(vec![Term::rat(1, 2), pow(pi(), Term::rat(1, 2)), pow(cc, Term::rat(-1, 2))])));
                        }
                    }
                }
            }
            rest.push(t.clone());
        }
        if let Some(c) = c {
            let r = mul(rest.clone());
            if let Some(p) = poly::to_poly(&r, x) {
                let mut out = Vec::new();
                let mut fact = Int::ONE;
                for (k, coef) in p.0.iter().enumerate() {
                    if k > 0 {
                        fact *= Int::from(k);
                    }
                    if !coef.is_zero() {
                        out.push(mul(vec![Term::Num(coef.clone() * Rat::from(fact.clone())), pow(c.clone(), Term::int(-(k as i64) - 1))]));
                    }
                }
                return Ok(Some(add(out)));
            }
            // e^{−x} ln x
            if c.is_one() {
                if let [Term::Fn(Func::Ln, arg)] = rest.as_slice() {
                    if arg.len() == 1 && arg[0] == xs {
                        return Ok(Some(neg(Term::Atom(Atom::EulerGamma))));
                    }
                }
            }
        }
        // sin(a x)/x = π/2 · sign(a)
        if let Term::Mul(v) = f {
            if v.len() == 2 && v.contains(&pow(xs.clone(), Term::int(-1))) {
                if let Some(Term::Fn(Func::Sin, arg)) = v.iter().find(|t| matches!(t, Term::Fn(Func::Sin, _))) {
                    if let Some(p) = poly::to_poly(&arg[0], x) {
                        if p.deg() == 1 && p.0[0].is_zero() {
                            let s = num::sign(&p.0[1]);
                            return Ok(Some(mul(vec![Term::rat(s as i64, 2), pi()])));
                        }
                    }
                }
            }
        }
        // x^m / (1 + x^n)（n > m + 1）= π / (n sin(π(m+1)/n))
        if let Some((nn, dd)) = poly::to_rational(f, x) {
            let mono = |p: &poly::Poly| p.0.iter().filter(|c| !c.is_zero()).count() == 1;
            if mono(&nn) && dd.0.iter().filter(|c| !c.is_zero()).count() == 2 && dd.0[0] == Rat::ONE && dd.lc() == Rat::ONE {
                let m = nn.deg();
                let n = dd.deg();
                if n > m + 1 {
                    let c = nn.lc();
                    let s = call(Func::Sin, vec![mul(vec![Term::Num(Rat::from_parts(Int::from(m + 1), Nat::from(n as u64))), pi()])]);
                    return Ok(Some(mul(vec![Term::Num(c / Rat::from(Int::from(n))), pi(), pow(s, Term::int(-1))])));
                }
            }
        }
    }
    // ∫_0^1 arctan(x)/x = G
    if is_zero_pt(a) && matches!(b, Point::Finite(t) if t.is_one()) {
        if *f == mul(vec![call(Func::Atan, vec![xs.clone()]), pow(xs.clone(), Term::int(-1))]) {
            return Ok(Some(Term::Atom(Atom::Catalan)));
        }
    }
    // ∫_0^{2π} 1/(a + b cos x) = 2π/√(a² − b²)（a > |b|）
    if is_zero_pt(a) && matches!(b, Point::Finite(t) if *t == mul(vec![Term::int(2), pi()])) {
        if let Term::Pow(bb, e) = f {
            if **e == Term::int(-1) {
                if let Term::Add(v) = &**bb {
                    let constant = v.iter().find_map(|t| t.as_rat().cloned());
                    let cosc = v.iter().find_map(|t| {
                        let (c, r) = super::term::split_coeff(t);
                        (r == call(Func::Cos, vec![xs.clone()])).then_some(c)
                    });
                    if let (Some(aa), Some(bc), 2) = (constant, cosc, v.len()) {
                        let d = aa.clone() * &aa - bc.clone() * &bc;
                        if num::sign(&aa) > 0 && num::sign(&d) > 0 {
                            return Ok(Some(mul(vec![Term::int(2), pi(), pow(Term::Num(d), Term::rat(-1, 2))])));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

/// $|g|$ を区間の中の符号で ±g に置き換える
fn resolve_abs(f: &Term, x: &str, mid: &Term) -> Term {
    match f {
        Term::Fn(Func::Abs, a) if a.len() == 1 && has(&a[0], x) => {
            let v = super::diff::subst(&a[0], x, mid);
            match super::values::real_sign(&v) {
                Some(s) if s < 0 => neg(a[0].clone()),
                Some(_) => a[0].clone(),
                None => f.clone(),
            }
        }
        Term::Add(v) => add(v.iter().map(|t| resolve_abs(t, x, mid)).collect()),
        Term::Mul(v) => mul(v.iter().map(|t| resolve_abs(t, x, mid)).collect()),
        Term::Pow(b, e) => pow(resolve_abs(b, x, mid), resolve_abs(e, x, mid)),
        Term::Fn(g, a) => call(*g, a.iter().map(|t| resolve_abs(t, x, mid)).collect()),
        _ => f.clone(),
    }
}

/// 奇関数か（f(−x) = −f(x) が式の形で成り立つ）
fn is_odd(f: &Term, x: &str) -> bool {
    let m = super::diff::subst(f, x, &neg(Term::Sym(x.into())));
    add(vec![m, f.clone()]).is_zero()
}

/// $\int_a^b f\,dx$
pub fn definite(f: &Term, x: &str, a: &Point, b: &Point) -> Result<Term, Stop> {
    let (lo, hi) = (to_f64(a), to_f64(b));
    if lo.is_nan() || hi.is_nan() {
        return Err(nf("積分の端の値が決まりません"));
    }
    if lo == hi {
        return Ok(Term::zero());
    }
    if lo > hi {
        return definite(f, x, b, a).map(neg);
    }
    if let Some(v) = table(f, x, a, b)? {
        return Ok(v);
    }
    let (sing, kinks) = split_points(f, x, lo, hi)?;
    // 対称な有限の区間で奇関数（特異点が無ければ）は 0
    if sing.is_empty() && lo.is_finite() && (lo + hi).abs() < 1e-300 && is_odd(f, x) {
        if let (Point::Finite(ta), Point::Finite(tb)) = (a, b) {
            if add(vec![ta.clone(), tb.clone()]).is_zero() {
                return Ok(Term::zero());
            }
        }
    }
    // 区間を分ける
    let mut cuts: Vec<(Rat, bool)> = sing.iter().map(|r| (r.clone(), true)).chain(kinks.iter().map(|r| (r.clone(), false))).collect();
    cuts.sort_by(|p, q| p.0.cmp(&q.0));
    cuts.dedup_by(|p, q| {
        if p.0 == q.0 {
            q.1 |= p.1;
            true
        } else {
            false
        }
    });
    let mut pts: Vec<(Point, bool)> = vec![(a.clone(), is_endpoint_singular(f, x, a))];
    for (r, s) in &cuts {
        pts.push((Point::Finite(Term::Num(r.clone())), *s));
    }
    pts.push((b.clone(), is_endpoint_singular(f, x, b)));
    let mut total = Vec::new();
    for w in pts.windows(2) {
        let ((pa, sa), (pb, sb)) = (&w[0], &w[1]);
        let piece = if kinks.is_empty() && !has_abs(f) {
            f.clone()
        } else {
            resolve_abs(f, x, &midpoint(pa, pb))
        };
        let g = super::integrate::antiderivative(&piece, x)?.ok_or_else(|| nf("原始関数が見つかりません"))?;
        let vb = end_value(&g, x, pb, -1, *sb)?;
        let va = end_value(&g, x, pa, 1, *sa)?;
        total.push(add(vec![vb, neg(va)]));
    }
    Ok(add(total))
}

fn has_abs(t: &Term) -> bool {
    match t {
        Term::Fn(Func::Abs, _) => true,
        Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) => v.iter().any(has_abs),
        Term::Pow(b, e) => has_abs(b) || has_abs(e),
        _ => false,
    }
}

fn midpoint(a: &Point, b: &Point) -> Term {
    match (a, b) {
        (Point::Finite(p), Point::Finite(q)) => mul(vec![Term::rat(1, 2), add(vec![p.clone(), q.clone()])]),
        (Point::Finite(p), Point::PlusInf) => add(vec![p.clone(), Term::one()]),
        (Point::MinusInf, Point::Finite(q)) => add(vec![q.clone(), Term::int(-1)]),
        _ => Term::zero(),
    }
}

/// 端が特異点か（中身に入れると値が決まらない）
fn is_endpoint_singular(f: &Term, x: &str, p: &Point) -> bool {
    match p {
        Point::Finite(v) => match super::diff::subst_checked(f, x, v) {
            Ok(t) => !t.is_number() || ball::value(&t, 64).is_err(),
            Err(()) => true,
        },
        _ => true,
    }
}

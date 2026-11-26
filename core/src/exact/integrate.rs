//! 不定積分（計画 `docs/plan-exact-areas.md` の IG0・IG1）。
//!
//! 規則を上から順に当てる（SymPy の manualintegrate・Rubi の考え方）。答えは必ず微分して戻ることを、呼ぶ側（`mod.rs`）が値で確かめる:
//! 1. 定数・和・定数倍
//! 2. 有理関数（IG1）: 部分分数にして、$\frac{c}{(x+a)^j}$ は $\ln\lvert x+a\rvert$ かべき、既約な 2 次式は $\ln$ と $\arctan$
//! 3. 中身が 1 次式 $u = ax + b$ の基本の関数の表（$u^n$・$e^u$・$\sin u$・$\cos u$・$\sinh u$ など）
//! 4. 三角関数のべき $\sin^m\cos^n$（奇数のべきは置換、偶数どうしは半角の公式）
//! 5. 根号の型（$\sqrt{a^2 - x^2}$・$\frac{1}{\sqrt{a^2 - x^2}}$・$\frac{1}{\sqrt{x^2 + c}}$・$\sqrt{x^2 + c}$）
//! 6. 置換（中身の微分が外にある: $\int f(g(x))g'(x)\,dx$）
//! 7. 部分積分（多項式 × $e^{ax}$・$\sin$・$\cos$、多項式 × $\ln$・$\arctan$・$\arcsin$、$e^{ax}\sin bx$ は公式）
//!
//! 実数の変数の対数は $\ln\lvert u\rvert$（計画の D5）。積分定数 $C$ は書き出すときに足す

use super::num::{self, Int, Nat, Rat};
use super::poly::{self, Poly};
use super::term::{add, call, div, mul, neg, pow, Atom, Term};
use super::{Budget, Stop};
use crate::functions::Func;

fn has(t: &Term, x: &str) -> bool {
    match t {
        Term::Sym(s) => s == x,
        Term::Num(_) | Term::Atom(_) => false,
        Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) | Term::Matrix(_, _, v) => v.iter().any(|a| has(a, x)),
        Term::Pow(b, e) => has(b, x) || has(e, x),
    }
}

/// $u = a x + b$ なら (a, b)
fn linear(u: &Term, x: &str) -> Option<(Term, Term)> {
    let a = super::diff::diff(u, x).ok()?;
    if has(&a, x) || a.is_zero() {
        return None;
    }
    Some((a, super::diff::subst(u, x, &Term::zero())))
}

fn ln_abs(u: Term) -> Term {
    call(Func::Ln, vec![call(Func::Abs, vec![u])])
}

/// t の中の target を with に置き換える（構造で一致する所だけ）
fn replace(t: &Term, target: &Term, with: &Term) -> Term {
    if t == target {
        return with.clone();
    }
    match t {
        Term::Add(v) => add(v.iter().map(|a| replace(a, target, with)).collect()),
        Term::Mul(v) => mul(v.iter().map(|a| replace(a, target, with)).collect()),
        Term::Pow(b, e) => {
            // target が累乗 b^k のとき、b^{nk} も (target)^n にする
            if let Term::Pow(tb, te) = target {
                if tb == b {
                    if let (Some(n), Some(k)) = (e.as_rat(), te.as_rat()) {
                        let q = n.clone() / k.clone();
                        if num::as_int(&q).is_some() {
                            return pow(with.clone(), Term::Num(q));
                        }
                    }
                }
            }
            pow(replace(b, target, with), replace(e, target, with))
        }
        Term::Fn(f, a) => call(*f, a.iter().map(|y| replace(y, target, with)).collect()),
        _ => t.clone(),
    }
}

/// 中身の候補（関数の引数・累乗の底と指数で、x を含むもの）
fn inner_candidates(t: &Term, x: &str, out: &mut Vec<Term>) {
    match t {
        Term::Fn(_, a) => {
            for y in a {
                if has(y, x) && !matches!(y, Term::Sym(_)) {
                    out.push(y.clone());
                }
                inner_candidates(y, x, out);
            }
            if has(t, x) {
                out.push(t.clone());
            }
        }
        Term::Pow(b, e) => {
            if has(b, x) && !matches!(&**b, Term::Sym(_)) {
                out.push((**b).clone());
            }
            if has(e, x) && !matches!(&**e, Term::Sym(_)) {
                out.push((**e).clone());
            }
            inner_candidates(b, x, out);
            inner_candidates(e, x, out);
        }
        Term::Add(v) | Term::Mul(v) => {
            for y in v {
                inner_candidates(y, x, out);
            }
        }
        _ => {}
    }
}

/// $\int f\,dx$（見つからなければ None）
pub fn antiderivative(f: &Term, x: &str) -> Result<Option<Term>, Stop> {
    integrate(f, x, 0)
}

fn integrate(f: &Term, x: &str, depth: usize) -> Result<Option<Term>, Stop> {
    if super::exhausted() {
        return Err(Stop::TimeUp);
    }
    if depth > 24 {
        return Ok(None);
    }
    if !has(f, x) {
        return Ok(Some(mul(vec![f.clone(), Term::Sym(x.into())])));
    }
    // 1. 和
    if let Term::Add(v) = f {
        let mut parts = Vec::new();
        for t in v {
            match integrate(t, x, depth + 1)? {
                Some(p) => parts.push(p),
                None => return Ok(None),
            }
        }
        return Ok(Some(add(parts)));
    }
    // 定数倍
    if let Term::Mul(v) = f {
        let (cs, rest): (Vec<Term>, Vec<Term>) = v.iter().cloned().partition(|t| !has(t, x));
        if !cs.is_empty() {
            return Ok(integrate(&mul(rest), x, depth + 1)?.map(|g| mul(vec![mul(cs), g])));
        }
    }
    // 2. 有理関数
    if let Some((n, d)) = poly::to_rational(f, x) {
        if let Some(g) = rational(&n, &d, x)? {
            return Ok(Some(g));
        }
    }
    // 3. 表
    if let Some(g) = table(f, x) {
        return Ok(Some(g));
    }
    // 4. 三角関数のべき
    if let Some(g) = trig_powers(f, x, depth)? {
        return Ok(Some(g));
    }
    // 5. 根号の型
    if let Some(g) = radicals(f, x) {
        return Ok(Some(g));
    }
    // 6. 置換
    if let Some(g) = substitution(f, x, depth)? {
        return Ok(Some(g));
    }
    // 7. 部分積分
    if let Some(g) = by_parts(f, x, depth)? {
        return Ok(Some(g));
    }
    Ok(None)
}

/// 有理関数の積分（IG1）
fn rational(n: &Poly, d: &Poly, x: &str) -> Result<Option<Term>, Stop> {
    let budget = Budget::new(5_000_000);
    let xs = Term::Sym(x.into());
    if d.deg() == 0 {
        // 多項式
        let c = d.0[0].clone();
        let p = n.scale(&(Rat::ONE / c));
        let integ = Poly::new(std::iter::once(Rat::ZERO).chain(p.0.iter().enumerate().map(|(k, a)| a.clone() / Rat::from(Int::from(k as i64 + 1)))).collect());
        return Ok(Some(poly::from_poly(&integ, x)));
    }
    let Some((q, parts)) = poly::apart(n, d, &budget) else { return Ok(None) };
    let mut out = Vec::new();
    if !q.is_zero() {
        let integ = Poly::new(std::iter::once(Rat::ZERO).chain(q.0.iter().enumerate().map(|(k, a)| a.clone() / Rat::from(Int::from(k as i64 + 1)))).collect());
        out.push(poly::from_poly(&integ, x));
    }
    for (numer, factor, j) in parts {
        let fx = poly::from_poly(&factor, x);
        match factor.deg() {
            1 => {
                // numer / (p1 x + p0)^j
                let p1 = factor.0[1].clone();
                let c = numer.0.first().cloned().unwrap_or(Rat::ZERO);
                if j == 1 {
                    out.push(mul(vec![Term::Num(c / p1), ln_abs(fx)]));
                } else {
                    let k = Rat::from(Int::from(j as i64 - 1));
                    out.push(mul(vec![Term::Num(-c / (p1 * k.clone())), pow(fx, Term::Num(-k))]));
                }
            }
            2 if j == 1 => {
                // (p x + r) / (α x² + β x + γ) = A q'/q + B/q
                let (g0, g1, g2) = (factor.0[0].clone(), factor.0[1].clone(), factor.0[2].clone());
                let p = numer.0.get(1).cloned().unwrap_or(Rat::ZERO);
                let r = numer.0.first().cloned().unwrap_or(Rat::ZERO);
                let a = p / (Rat::from(Int::from(2)) * &g2);
                let b = r - a.clone() * &g1;
                if !a.is_zero() {
                    // 既約な 2 次式は符号が一定。α > 0 なら正
                    let lnq = if num::sign(&g2) > 0 { call(Func::Ln, vec![fx.clone()]) } else { ln_abs(fx.clone()) };
                    out.push(mul(vec![Term::Num(a), lnq]));
                }
                if !b.is_zero() {
                    // ∫ 1/q = 2/√D arctan((2αx + β)/√D)、D = 4αγ − β²
                    let dd = Rat::from(Int::from(4)) * &g2 * &g0 - g1.clone() * &g1;
                    if num::sign(&dd) <= 0 {
                        return Ok(None);
                    }
                    let sd = pow(Term::Num(dd), Term::rat(1, 2));
                    let arg = mul(vec![add(vec![mul(vec![Term::Num(Rat::from(Int::from(2)) * g2), xs.clone()]), Term::Num(g1)]), pow(sd.clone(), Term::int(-1))]);
                    out.push(mul(vec![Term::Num(b * Rat::from(Int::from(2))), pow(sd, Term::int(-1)), call(Func::Atan, vec![arg])]));
                }
            }
            _ => return Ok(None),
        }
    }
    Ok(Some(add(out)))
}

/// 中身が 1 次式の基本の関数
fn table(f: &Term, x: &str) -> Option<Term> {
    let inv = |t: Term| pow(t, Term::int(-1));
    match f {
        Term::Sym(_) => Some(mul(vec![Term::rat(1, 2), pow(f.clone(), Term::int(2))])),
        Term::Pow(b, e) => {
            if !has(e, x) {
                let (a, _) = linear(b, x)?;
                let n = (**e).clone();
                if n == Term::int(-1) {
                    return Some(mul(vec![inv(a), ln_abs((**b).clone())]));
                }
                let n1 = add(vec![n, Term::one()]);
                return Some(mul(vec![pow((**b).clone(), n1.clone()), inv(mul(vec![a, n1]))]));
            }
            if !has(b, x) {
                // c^u = e^{u ln c}
                let (a, _) = linear(e, x)?;
                let lnc = if **b == Term::Atom(Atom::E) { Term::one() } else { call(Func::Ln, vec![(**b).clone()]) };
                return Some(mul(vec![f.clone(), inv(mul(vec![a, lnc]))]));
            }
            None
        }
        Term::Fn(func, args) if args.len() == 1 => {
            let u = &args[0];
            let (a, _) = linear(u, x)?;
            let ia = inv(a);
            let c = |g: Func| call(g, vec![u.clone()]);
            Some(match func {
                Func::Sin => mul(vec![Term::int(-1), ia, c(Func::Cos)]),
                Func::Cos => mul(vec![ia, c(Func::Sin)]),
                Func::Tan => mul(vec![Term::int(-1), ia, ln_abs(c(Func::Cos))]),
                Func::Exp => mul(vec![ia, c(Func::Exp)]),
                Func::Sinh => mul(vec![ia, c(Func::Cosh)]),
                Func::Cosh => mul(vec![ia, c(Func::Sinh)]),
                _ => return None,
            })
        }
        _ => None,
    }
}

/// $\sin^m(ax+b)\cos^n(ax+b)$
fn trig_powers(f: &Term, x: &str, depth: usize) -> Result<Option<Term>, Stop> {
    let fs: Vec<Term> = match f {
        Term::Mul(v) => v.clone(),
        other => vec![other.clone()],
    };
    let mut arg: Option<Term> = None;
    let (mut m, mut n) = (0i64, 0i64);
    for t in &fs {
        let (func, a, k) = match t {
            Term::Fn(g @ (Func::Sin | Func::Cos), a) if a.len() == 1 => (*g, a[0].clone(), 1),
            Term::Pow(b, e) => match (&**b, e.as_rat().and_then(num::as_int).and_then(|k| i64::try_from(&k).ok())) {
                (Term::Fn(g @ (Func::Sin | Func::Cos), a), Some(k)) if a.len() == 1 && k > 0 => (*g, a[0].clone(), k),
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        if arg.as_ref().is_some_and(|x0| *x0 != a) {
            return Ok(None);
        }
        arg = Some(a);
        if func == Func::Sin {
            m += k;
        } else {
            n += k;
        }
    }
    let Some(u) = arg else { return Ok(None) };
    let Some((a, _)) = linear(&u, x) else { return Ok(None) };
    if m + n < 2 {
        return Ok(None);
    }
    let w = Term::Sym("#w".into());
    let back = |g: Term, target: Term| super::diff::subst(&g, "#w", &target);
    let ia = pow(a, Term::int(-1));
    if m % 2 == 1 {
        // sin^m cos^n = (1 − cos²)^{(m−1)/2} cos^n sin、w = cos u、dw = −a sin u dx
        let p = mul(vec![pow(add(vec![Term::one(), neg(pow(w.clone(), Term::int(2)))]), Term::int((m - 1) / 2)), pow(w.clone(), Term::int(n))]);
        let p = expand_poly(&p, "#w");
        let Some(g) = integrate(&p, "#w", depth + 1)? else { return Ok(None) };
        return Ok(Some(mul(vec![Term::int(-1), ia, back(g, call(Func::Cos, vec![u]))])));
    }
    if n % 2 == 1 {
        let p = mul(vec![pow(add(vec![Term::one(), neg(pow(w.clone(), Term::int(2)))]), Term::int((n - 1) / 2)), pow(w.clone(), Term::int(m))]);
        let p = expand_poly(&p, "#w");
        let Some(g) = integrate(&p, "#w", depth + 1)? else { return Ok(None) };
        return Ok(Some(mul(vec![ia, back(g, call(Func::Sin, vec![u]))])));
    }
    // 偶数どうし: sin² = (1 − cos 2u)/2、cos² = (1 + cos 2u)/2 を入れて展開（cos 2u のべきに戻る）
    let c2 = call(Func::Cos, vec![mul(vec![Term::int(2), u.clone()])]);
    let s2 = mul(vec![Term::rat(1, 2), add(vec![Term::one(), neg(c2.clone())])]);
    let co2 = mul(vec![Term::rat(1, 2), add(vec![Term::one(), c2])]);
    let g = mul(vec![pow(s2, Term::int(m / 2)), pow(co2, Term::int(n / 2))]);
    let g = expand_trig(&g);
    integrate(&g, x, depth + 1)
}

/// 多項式を展開した形にする
fn expand_poly(t: &Term, v: &str) -> Term {
    match poly::to_poly(t, v) {
        Some(p) => poly::from_poly(&p, v),
        None => t.clone(),
    }
}

/// 積と整数乗を展開する（文字を含む和も）
fn expand_trig(t: &Term) -> Term {
    match t {
        Term::Mul(v) => {
            let mut acc: Vec<Term> = vec![Term::one()];
            for f in v {
                let f = expand_trig(f);
                let parts = match f {
                    Term::Add(p) => p,
                    other => vec![other],
                };
                let mut next = Vec::new();
                for a in &acc {
                    for p in &parts {
                        next.push(mul(vec![a.clone(), p.clone()]));
                    }
                }
                acc = next;
            }
            add(acc)
        }
        Term::Pow(b, e) => match e.as_rat().and_then(num::as_int).and_then(|k| u32::try_from(&k).ok()) {
            Some(k) if k <= 8 && matches!(&**b, Term::Add(_)) => expand_trig(&Term::Mul(vec![(**b).clone(); k as usize])),
            _ => t.clone(),
        },
        Term::Add(v) => add(v.iter().map(expand_trig).collect()),
        _ => t.clone(),
    }
}

/// 根号の型（x の 2 次式の平方根）
fn radicals(f: &Term, x: &str) -> Option<Term> {
    let xs = Term::Sym(x.into());
    let (base, e) = match f {
        Term::Pow(b, e) => ((**b).clone(), e.as_rat()?.clone()),
        _ => return None,
    };
    let half = Rat::from_parts(Int::ONE, Nat::from(2u8));
    let p = poly::to_poly(&base, x)?;
    if p.deg() != 2 || !p.0[1].is_zero() {
        return None;
    }
    let (c, s) = (p.0[0].clone(), p.0[2].clone());
    let sq = |t: Term| pow(t, Term::rat(1, 2));
    if s == -Rat::ONE && num::sign(&c) > 0 {
        let a = sq(Term::Num(c.clone()));
        let asin = call(Func::Asin, vec![mul(vec![xs.clone(), pow(a, Term::int(-1))])]);
        if e == half {
            // √(a² − x²) → (x √(a² − x²) + a² arcsin(x/a)) / 2
            return Some(mul(vec![Term::rat(1, 2), add(vec![mul(vec![xs, f.clone()]), mul(vec![Term::Num(c), asin])])]));
        }
        if e == -half.clone() {
            return Some(asin);
        }
    }
    if s == Rat::ONE && !c.is_zero() {
        let l = call(Func::Ln, vec![call(Func::Abs, vec![add(vec![xs.clone(), sq(base.clone())])])]);
        let l = if num::sign(&c) > 0 { call(Func::Ln, vec![add(vec![xs.clone(), sq(base.clone())])]) } else { l };
        if e == -half.clone() {
            return Some(l);
        }
        if e == half {
            return Some(mul(vec![Term::rat(1, 2), add(vec![mul(vec![xs, f.clone()]), mul(vec![Term::Num(c), l])])]));
        }
    }
    None
}

/// 置換: 中身 u の候補ごとに、f / u′ を u だけの式にできるか
fn substitution(f: &Term, x: &str, depth: usize) -> Result<Option<Term>, Stop> {
    let mut cands = Vec::new();
    inner_candidates(f, x, &mut cands);
    cands.dedup();
    let w = Term::Sym("#u".into());
    for u in cands {
        if linear(&u, x).is_some() {
            continue; // 1 次式の中身は表で済む
        }
        let du = super::diff::diff(&u, x)?;
        if du.is_zero() {
            continue;
        }
        let q = div(f.clone(), du);
        let r = replace(&q, &u, &w);
        if has(&r, x) {
            continue;
        }
        if let Some(g) = integrate(&r, "#u", depth + 1)? {
            return Ok(Some(super::diff::subst(&g, "#u", &u)));
        }
    }
    Ok(None)
}

/// 部分積分
fn by_parts(f: &Term, x: &str, depth: usize) -> Result<Option<Term>, Stop> {
    let fs: Vec<Term> = match f {
        Term::Mul(v) => v.clone(),
        other => vec![other.clone()],
    };
    // e^{ax+b} sin/cos(cx+d): ∫ e^{αx} sin βx = e^{αx}(α sin βx − β cos βx)/(α² + β²)
    if fs.len() == 2 {
        let (mut ex, mut tr) = (None, None);
        for t in &fs {
            match t {
                Term::Pow(b, e) if **b == Term::Atom(Atom::E) => ex = Some((**e).clone()),
                Term::Fn(g @ (Func::Sin | Func::Cos), a) if a.len() == 1 => tr = Some((*g, a[0].clone())),
                _ => {}
            }
        }
        if let (Some(e1), Some((g, v))) = (ex, tr) {
            if let (Some((al, _)), Some((be, _))) = (linear(&e1, x), linear(&v, x)) {
                let den = add(vec![mul(vec![al.clone(), al.clone()]), mul(vec![be.clone(), be.clone()])]);
                let (s, c) = (call(Func::Sin, vec![v.clone()]), call(Func::Cos, vec![v]));
                let inner = if g == Func::Sin { add(vec![mul(vec![al, s]), neg(mul(vec![be, c]))]) } else { add(vec![mul(vec![al, c]), mul(vec![be, s])]) };
                return Ok(Some(mul(vec![pow(Term::Atom(Atom::E), e1), inner, pow(den, Term::int(-1))])));
            }
        }
    }
    // 多項式の部分 P と、そのほか g
    let (ps, gs): (Vec<Term>, Vec<Term>) = fs.iter().cloned().partition(|t| poly::to_poly(t, x).is_some());
    let p = mul(ps);
    let Some(pp) = poly::to_poly(&p, x) else { return Ok(None) };
    let g = mul(gs);
    if pp.deg() < 0 {
        return Ok(None);
    }
    match &g {
        // P · (e^u・sin u・cos u・sinh・cosh): ∫ P g = P G − ∫ P′ G
        Term::Pow(b, e) if **b == Term::Atom(Atom::E) && linear(e, x).is_some() && pp.deg() >= 1 => parts_down(&p, &g, x, depth),
        Term::Fn(Func::Sin | Func::Cos | Func::Sinh | Func::Cosh, a) if a.len() == 1 && linear(&a[0], x).is_some() && pp.deg() >= 1 => parts_down(&p, &g, x, depth),
        // P · ln・arctan・arcsin: ∫ P g = Q g − ∫ Q g′（Q = ∫ P）
        Term::Fn(Func::Ln | Func::Atan | Func::Asin | Func::Acos | Func::Log10, a) if a.len() == 1 => {
            let Some(qq) = integrate(&p, x, depth + 1)? else { return Ok(None) };
            let dg = super::diff::diff(&g, x)?;
            let rest = expand_trig(&mul(vec![qq.clone(), dg]));
            let Some(r) = integrate(&rest, x, depth + 1)? else { return Ok(None) };
            Ok(Some(add(vec![mul(vec![qq, g.clone()]), neg(r)])))
        }
        _ => Ok(None),
    }
}

/// 多項式 P を微分していく部分積分（表の方法）
fn parts_down(p: &Term, g: &Term, x: &str, depth: usize) -> Result<Option<Term>, Stop> {
    let Some(gi) = integrate(g, x, depth + 1)? else { return Ok(None) };
    let dp = super::diff::diff(p, x)?;
    let rest = mul(vec![dp, gi.clone()]);
    let Some(r) = integrate(&rest, x, depth + 1)? else { return Ok(None) };
    Ok(Some(add(vec![mul(vec![p.clone(), gi]), neg(r)])))
}

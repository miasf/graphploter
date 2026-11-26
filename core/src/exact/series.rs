//! 式の級数展開（計画 `docs/plan-exact-areas.md` の LM0）。
//!
//! 展開の変数は $h \to 0^+$（いつも正）。$x$ との関係は [`Map`]: $x = a + h$（右から）、$x = a - h$（左から）、$x = \frac1h$（$+\infty$）、$x = -\frac1h$（$-\infty$）。
//! 形は $\sum c_k h^{e_k}$（指数 $e_k$ は有理数。ピュイズー級数）で、係数は [`Term`]。$\ln h$ は係数の中の文字 [`LOG_H`] として持つ
//! （$h$ のどのべきよりもゆっくり変わるので、べきの順を決めるときは定数と同じに扱う）。
//!
//! 打ち切り: 指数が `cap` 以上の項は捨て、`order`（それより先は分からない指数）を持ち回る。先頭が相殺で消えたら、呼ぶ側（[`leading`]）が `cap` を上げてやり直す。
//! 関数の展開は、定数の部分 $s_0$ と残り $u$（指数が正）に分けて、$e^{s_0+u} = e^{s_0}\sum\frac{u^m}{m!}$、$\sin(s_0+u) = \sin s_0\cos u + \cos s_0\sin u$ などで作る

use super::num::{self, Int, Nat, Rat};
use super::term::{add, call, mul, neg, pow, Atom, Term};
use super::{ball, spend, Stop};
use crate::functions::Func;

/// $\ln h$ を表す係数の中の文字
pub const LOG_H: &str = "ℓ";

/// $x$ と $h$ の関係
#[derive(Debug, Clone)]
pub enum Map {
    /// $x = a + s h$（s は 1 か −1）
    Finite(Term, i32),
    /// $x = s / h$
    Infinite(i32),
}

/// 打ち切った級数
#[derive(Debug, Clone, PartialEq)]
pub struct Ser {
    /// (指数, 係数)。指数の小さい順。係数は構造で 0 でない
    pub terms: Vec<(Rat, Term)>,
    /// これ以上の指数の項は分からない
    pub order: Rat,
}

/// ちょうどの値の order（打ち切りより十分大きい）
fn exact_order() -> Rat {
    Rat::from(Int::from(1_000_000))
}

fn r(n: i64) -> Rat {
    Rat::from(Int::from(n))
}

fn coef_is_zero(t: &Term) -> bool {
    if t.is_zero() {
        return true;
    }
    if t.is_number() {
        let mut rules = Vec::new();
        return ball::is_zero(t, &mut rules) == Some(true);
    }
    false
}

impl Ser {
    /// 定数（ちょうどの値なので、打ち切りの誤差は無い。order は「とても大きい」）
    pub fn constant(c: Term, _cap: &Rat) -> Ser {
        let terms = if c.is_zero() { Vec::new() } else { vec![(Rat::ZERO, c)] };
        Ser { terms, order: exact_order() }
    }

    fn monomial(e: Rat, c: Term, cap: &Rat) -> Ser {
        if e >= *cap {
            return Ser { terms: Vec::new(), order: cap.clone() };
        }
        Ser { terms: vec![(e, c)], order: exact_order() }
    }

    /// 0 でない最初の項（値で 0 の係数は捨てる）。全部 0 なら None
    pub fn lead(&self) -> Option<(Rat, Term)> {
        self.terms.iter().find(|(_, c)| !coef_is_zero(c)).cloned()
    }

    /// 先頭の指数（無ければ order）
    fn valuation(&self) -> Rat {
        self.lead().map(|(e, _)| e).unwrap_or_else(|| self.order.clone())
    }

    /// 指数 0 の係数
    fn const_term(&self) -> Term {
        self.terms.iter().find(|(e, _)| e.is_zero()).map(|(_, c)| c.clone()).unwrap_or_else(Term::zero)
    }

    /// 指数が負の項があるか（$h \to 0$ で発散する）
    fn has_negative(&self) -> bool {
        self.terms.iter().any(|(e, c)| num::sign(e) < 0 && !coef_is_zero(c))
    }

    /// 指数が正の部分（定数の部分を除いたもの）
    fn positive_part(&self) -> Ser {
        Ser { terms: self.terms.iter().filter(|(e, _)| num::sign(e) > 0).cloned().collect(), order: self.order.clone() }
    }
}

fn normalize(mut terms: Vec<(Rat, Term)>, order: Rat) -> Ser {
    terms.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out: Vec<(Rat, Term)> = Vec::new();
    for (e, c) in terms {
        if e >= order {
            continue;
        }
        match out.last_mut() {
            Some((le, lc)) if *le == e => *lc = add(vec![lc.clone(), c]),
            _ => out.push((e, c)),
        }
    }
    out.retain(|(_, c)| !c.is_zero());
    spend(out.len() as u64 + 1);
    Ser { terms: out, order }
}

pub fn s_add(a: &Ser, b: &Ser) -> Ser {
    let order = a.order.clone().min(b.order.clone());
    let mut t = a.terms.clone();
    t.extend(b.terms.iter().cloned());
    normalize(t, order)
}

pub fn s_neg(a: &Ser) -> Ser {
    Ser { terms: a.terms.iter().map(|(e, c)| (e.clone(), neg(c.clone()))).collect(), order: a.order.clone() }
}

pub fn s_scale(c: &Term, a: &Ser) -> Ser {
    normalize(a.terms.iter().map(|(e, x)| (e.clone(), mul(vec![c.clone(), x.clone()]))).collect(), a.order.clone())
}

pub fn s_mul(a: &Ser, b: &Ser, cap: &Rat) -> Ser {
    let (va, vb) = (a.valuation(), b.valuation());
    let order = (a.order.clone() + &vb).min(b.order.clone() + &va).min(cap.clone());
    let mut t = Vec::with_capacity(a.terms.len() * b.terms.len());
    for (ea, ca) in &a.terms {
        for (eb, cb) in &b.terms {
            let e = ea.clone() + eb;
            if e < order {
                t.push((e, mul(vec![ca.clone(), cb.clone()])));
            }
        }
    }
    normalize(t, order)
}

/// $\sum_{m=0}^{M} k_m u^m$（$u$ の指数は正）。`coef(m)` が None なら、そこで止める（多項式）
fn compose(coef: &dyn Fn(usize) -> Option<Term>, u: &Ser, cap: &Rat) -> Result<Ser, Stop> {
    let w = u.valuation();
    if num::sign(&w) <= 0 {
        return Err(Stop::NotFound("級数の合成で、中身の先頭が 0 に近づきません".into()));
    }
    // u^m の指数は m w 以上。m w ≥ cap になれば止める
    let m_max = usize::try_from(&(cap.clone() / &w).ceil()).unwrap_or(64).min(64);
    let mut acc = Ser::constant(coef(0).unwrap_or_else(Term::zero), cap);
    let mut power = Ser::constant(Term::one(), cap);
    for m in 1..=m_max + 1 {
        if super::exhausted() {
            return Err(Stop::TimeUp);
        }
        power = s_mul(&power, u, cap);
        let Some(k) = coef(m) else { break };
        if !k.is_zero() {
            acc = s_add(&acc, &s_scale(&k, &power));
        }
    }
    // 打ち切った次の項の指数
    let tail = w * r(m_max as i64 + 2);
    // 中身の分からない部分 O(h^{u.order}) は、結果にも同じ指数で効く
    acc.order = acc.order.min(tail).min(u.order.clone()).min(cap.clone());
    Ok(acc)
}

fn factorial_rat(m: usize) -> Rat {
    let mut f = Int::ONE;
    for k in 2..=m {
        f *= Int::from(k);
    }
    Rat::from(f)
}

/// $\frac{1}{s}$
pub fn s_recip(s: &Ser, cap: &Rat) -> Result<Ser, Stop> {
    let Some((v, c0)) = s.lead() else { return Err(Stop::NotFound("0 になる級数の逆数".into())) };
    // s = c0 h^v (1 + u)
    let inv_c0 = pow(c0.clone(), Term::int(-1));
    let u = Ser {
        terms: s.terms.iter().filter(|(e, _)| *e > v).map(|(e, c)| (e.clone() - &v, mul(vec![c.clone(), inv_c0.clone()]))).collect(),
        order: s.order.clone() - &v,
    };
    // 1/(1+u) = Σ (−u)^m。指数をずらす前に cap + v まで要る
    let inner_cap = cap.clone() + &v;
    let geo = compose(&|m| Some(if m % 2 == 0 { Term::one() } else { Term::int(-1) }), &u, &inner_cap)?;
    Ok(shift(&s_scale(&inv_c0, &geo), &(-v), cap))
}

/// $h^d$ を掛ける
fn shift(s: &Ser, d: &Rat, cap: &Rat) -> Ser {
    normalize(s.terms.iter().map(|(e, c)| (e.clone() + d, c.clone())).collect(), (s.order.clone() + d).min(cap.clone()))
}

/// $s^r$（$r$ は有理数。主値）
pub fn s_pow_rat(s: &Ser, rr: &Rat, cap: &Rat) -> Result<Ser, Stop> {
    if rr.is_zero() {
        return Ok(Ser::constant(Term::one(), cap));
    }
    if let Some(k) = num::as_int(rr).and_then(|k| i64::try_from(&k).ok()) {
        if (1..=64).contains(&k) {
            let mut acc = Ser::constant(Term::one(), cap);
            for _ in 0..k {
                acc = s_mul(&acc, s, cap);
            }
            return Ok(acc);
        }
        if (-64..0).contains(&k) {
            return s_pow_rat(&s_recip(s, cap)?, &(-rr.clone()), cap);
        }
    }
    let Some((v, c0)) = s.lead() else { return Err(Stop::NotFound("0 になる級数の分数べき".into())) };
    let inv_c0 = pow(c0.clone(), Term::int(-1));
    let u = Ser {
        terms: s.terms.iter().filter(|(e, _)| *e > v).map(|(e, c)| (e.clone() - &v, mul(vec![c.clone(), inv_c0.clone()]))).collect(),
        order: s.order.clone() - &v,
    };
    let lead_exp = v.clone() * rr;
    let inner_cap = cap.clone() - &lead_exp;
    // (1+u)^r = Σ C(r, m) u^m
    let rt = Term::Num(rr.clone());
    let binom = |m: usize| -> Option<Term> {
        let mut c = Rat::ONE;
        for j in 0..m {
            c = c * (rr.clone() - r(j as i64)) / r(j as i64 + 1);
        }
        Some(Term::Num(c))
    };
    let _ = rt;
    let series = compose(&binom, &u, &inner_cap.max(Rat::ONE))?;
    Ok(shift(&s_scale(&pow(c0, Term::Num(rr.clone())), &series), &lead_exp, cap))
}

/// $e^s$（$s$ は発散しない）
pub fn s_exp(s: &Ser, cap: &Rat) -> Result<Ser, Stop> {
    if s.has_negative() {
        return Err(Stop::NotFound("本質的特異点（e の肩が発散する）の展開はまだできません".into()));
    }
    let s0 = s.const_term();
    let u = s.positive_part();
    let ser = compose(&|m| Some(Term::Num(Rat::ONE / factorial_rat(m))), &u, cap)?;
    Ok(s_scale(&pow(Term::Atom(Atom::E), s0), &ser))
}

/// $\ln s$（主値。$\ln h$ は係数の文字 ℓ）
pub fn s_ln(s: &Ser, cap: &Rat) -> Result<Ser, Stop> {
    let Some((v, c0)) = s.lead() else { return Err(Stop::NotFound("0 になる級数の対数".into())) };
    let inv_c0 = pow(c0.clone(), Term::int(-1));
    let u = Ser {
        terms: s.terms.iter().filter(|(e, _)| *e > v).map(|(e, c)| (e.clone() - &v, mul(vec![c.clone(), inv_c0.clone()]))).collect(),
        order: s.order.clone() - &v,
    };
    // ln(1+u) = Σ (−1)^{m+1} u^m / m
    let ser = compose(&|m| if m == 0 { Some(Term::zero()) } else { Some(Term::Num(Rat::from_parts(Int::from(if m % 2 == 1 { 1 } else { -1 }), Nat::from(m)))) }, &u, cap)?;
    let mut c = call(Func::Ln, vec![c0]);
    if !v.is_zero() {
        c = add(vec![c, mul(vec![Term::Num(v), Term::Sym(LOG_H.into())])]);
    }
    Ok(s_add(&Ser::constant(c, cap), &ser))
}

/// $\sin u$・$\cos u$ の級数（$u$ の指数は正）
fn sin_cos_series(u: &Ser, cap: &Rat) -> Result<(Ser, Ser), Stop> {
    let s = compose(&|m| Some(if m % 2 == 1 { Term::Num(Rat::from(Int::from(if (m / 2) % 2 == 0 { 1 } else { -1 })) / factorial_rat(m)) } else { Term::zero() }), u, cap)?;
    let c = compose(&|m| Some(if m % 2 == 0 { Term::Num(Rat::from(Int::from(if (m / 2) % 2 == 0 { 1 } else { -1 })) / factorial_rat(m)) } else { Term::zero() }), u, cap)?;
    Ok((s, c))
}

/// 実数の係数の符号（値で）
fn sign_of(t: &Term) -> Option<i32> {
    super::values::real_sign(t)
}

/// 関数の展開
fn s_call(f: Func, args: &[Ser], cap: &Rat) -> Result<Ser, Stop> {
    let [s] = args else { return Err(Stop::NotFound(format!("{f:?} の展開はまだできません"))) };
    let bounded = !s.has_negative();
    match f {
        Func::Exp => s_exp(s, cap),
        Func::Ln => s_ln(s, cap),
        Func::Log10 => Ok(s_scale(&pow(call(Func::Ln, vec![Term::int(10)]), Term::int(-1)), &s_ln(s, cap)?)),
        Func::Sqrt => s_pow_rat(s, &Rat::from_parts(Int::ONE, Nat::from(2u8)), cap),
        Func::Sin | Func::Cos if bounded => {
            let s0 = s.const_term();
            let (su, cu) = sin_cos_series(&s.positive_part(), cap)?;
            let (a, b) = (call(Func::Sin, vec![s0.clone()]), call(Func::Cos, vec![s0]));
            Ok(if f == Func::Sin { s_add(&s_scale(&a, &cu), &s_scale(&b, &su)) } else { s_add(&s_scale(&b, &cu), &s_neg(&s_scale(&a, &su))) })
        }
        Func::Tan | Func::Sec | Func::Csc | Func::Cot if bounded => {
            let sn = s_call(Func::Sin, args, cap)?;
            let cs = s_call(Func::Cos, args, cap)?;
            Ok(match f {
                Func::Tan => s_mul(&sn, &s_recip(&cs, cap)?, cap),
                Func::Sec => s_recip(&cs, cap)?,
                Func::Csc => s_recip(&sn, cap)?,
                _ => s_mul(&cs, &s_recip(&sn, cap)?, cap),
            })
        }
        Func::Sinh | Func::Cosh | Func::Tanh if bounded => {
            let a = s_exp(s, cap)?;
            let b = s_exp(&s_neg(s), cap)?;
            let half = Term::rat(1, 2);
            Ok(match f {
                Func::Sinh => s_scale(&half, &s_add(&a, &s_neg(&b))),
                Func::Cosh => s_scale(&half, &s_add(&a, &b)),
                _ => s_mul(&s_add(&a, &s_neg(&b)), &s_recip(&s_add(&a, &b), cap)?, cap),
            })
        }
        Func::Atan => {
            if !bounded {
                // s → ±∞: atan s = ±π/2 − atan(1/s)
                let (_, c) = s.lead().unwrap();
                let sg = sign_of(&c).ok_or_else(|| Stop::NotFound("符号の決まらない係数".into()))?;
                let inv = s_recip(s, cap)?;
                let half_pi = mul(vec![Term::rat(sg as i64, 2), Term::Atom(Atom::Pi)]);
                return Ok(s_add(&Ser::constant(half_pi, cap), &s_neg(&s_call(Func::Atan, &[inv], cap)?)));
            }
            // atan(s0 + u) = atan s0 + atan(u / (1 + s0 s))
            let s0 = s.const_term();
            let u = s.positive_part();
            let w = if s0.is_zero() { u } else { s_mul(&u, &s_recip(&s_add(&Ser::constant(Term::one(), cap), &s_scale(&s0, s)), cap)?, cap) };
            let ser = compose(&|m| Some(if m % 2 == 1 { Term::Num(Rat::from_parts(Int::from(if (m / 2) % 2 == 0 { 1 } else { -1 }), Nat::from(m))) } else { Term::zero() }), &w, cap)?;
            Ok(s_add(&Ser::constant(call(Func::Atan, vec![s0]), cap), &ser))
        }
        Func::Asin | Func::Acos if bounded => {
            // asin s = atan(s / √(1 − s²))（|s| < 1）
            let one_minus = s_add(&Ser::constant(Term::one(), cap), &s_neg(&s_mul(s, s, cap)));
            let q = s_mul(s, &s_pow_rat(&one_minus, &Rat::from_parts(Int::from(-1), Nat::from(2u8)), cap)?, cap);
            let a = s_call(Func::Atan, &[q], cap)?;
            Ok(if f == Func::Asin { a } else { s_add(&Ser::constant(mul(vec![Term::rat(1, 2), Term::Atom(Atom::Pi)]), cap), &s_neg(&a)) })
        }
        Func::Asinh => {
            // asinh s = ln(s + √(s² + 1))
            let r = s_pow_rat(&s_add(&s_mul(s, s, cap), &Ser::constant(Term::one(), cap)), &Rat::from_parts(Int::ONE, Nat::from(2u8)), cap)?;
            s_ln(&s_add(s, &r), cap)
        }
        Func::Abs | Func::Sign => {
            let Some((_, c)) = s.lead() else { return Err(Stop::NotFound("0 になる級数の絶対値".into())) };
            let sg = sign_of(&c).ok_or_else(|| Stop::NotFound("符号の決まらない係数".into()))?;
            Ok(if f == Func::Sign { Ser::constant(Term::int(sg as i64), cap) } else if sg < 0 { s_neg(s) } else { s.clone() })
        }
        Func::Sin | Func::Cos => Err(Stop::NotFound("振動（三角関数の中身が発散する）".into())),
        _ => Err(Stop::NotFound(format!("{f:?} の展開はまだできません"))),
    }
}

/// 式 `t` を、`x` について `map` で展開する（指数 `cap` 未満）
pub fn expand(t: &Term, x: &str, map: &Map, cap: &Rat) -> Result<Ser, Stop> {
    if super::exhausted() {
        return Err(Stop::TimeUp);
    }
    if !has(t, x) {
        return Ok(Ser::constant(t.clone(), cap));
    }
    match t {
        Term::Sym(_) => Ok(match map {
            Map::Finite(a, sg) => s_add(&Ser::constant(a.clone(), cap), &Ser::monomial(Rat::ONE, Term::int(*sg as i64), cap)),
            Map::Infinite(sg) => Ser::monomial(r(-1), Term::int(*sg as i64), cap),
        }),
        Term::Add(v) => {
            let mut acc = Ser::constant(Term::zero(), cap);
            for a in v {
                acc = s_add(&acc, &expand(a, x, map, cap)?);
            }
            Ok(acc)
        }
        Term::Mul(v) => {
            // 発散する因子があると、ほかの因子をもっと先まで要る（h^{-1} × O(h^cap)）。先頭の指数の和で cap を広げる
            let parts: Vec<Ser> = v.iter().map(|a| expand(a, x, map, cap)).collect::<Result<_, _>>()?;
            let neg_sum: Rat = parts.iter().map(|p| p.valuation().min(Rat::ZERO)).fold(Rat::ZERO, |a, b| a + b);
            let parts = if num::sign(&neg_sum) < 0 {
                let wide = cap.clone() - &neg_sum;
                v.iter().map(|a| expand(a, x, map, &wide)).collect::<Result<Vec<_>, _>>()?
            } else {
                parts
            };
            let mut acc = Ser::constant(Term::one(), &(cap.clone() - &neg_sum));
            for p in &parts {
                acc = s_mul(&acc, p, &(cap.clone() - &neg_sum));
            }
            Ok(normalize(acc.terms, acc.order.min(cap.clone())))
        }
        Term::Pow(b, e) => {
            if !has(e, x) {
                if let Some(q) = e.as_rat() {
                    let bs = expand(b, x, map, cap)?;
                    // s = c h^v (1 + u) の u は s.order − v まで分かり、s^q の誤差は h^{s.order + v(q − 1)}。
                    // cap まで正しくするには、底を cap + v (1 − q) まで要る（割り算 q = −1 なら cap + 2v）
                    let v = bs.valuation();
                    let need = Rat::ONE - q.clone();
                    if num::sign(&v) > 0 && num::sign(&need) > 0 {
                        let wide = cap.clone() + v * need;
                        let bs = expand(b, x, map, &wide)?;
                        return s_pow_rat(&bs, q, cap);
                    }
                    return s_pow_rat(&bs, q, cap);
                }
            }
            // b^e = exp(e ln b)
            let lb = s_ln(&expand(b, x, map, cap)?, cap)?;
            let es = expand(e, x, map, cap)?;
            s_exp(&s_mul(&es, &lb, cap), cap)
        }
        Term::Fn(f, args) => {
            let a = args.iter().map(|a| expand(a, x, map, cap)).collect::<Result<Vec<_>, _>>()?;
            s_call(*f, &a, cap)
        }
        _ => Err(Stop::NotFound("この式の展開はまだできません".into())),
    }
}

fn has(t: &Term, x: &str) -> bool {
    match t {
        Term::Sym(s) => s == x,
        Term::Num(_) | Term::Atom(_) => false,
        Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) | Term::Matrix(_, _, v) => v.iter().any(|a| has(a, x)),
        Term::Pow(b, e) => has(b, x) || has(e, x),
    }
}

/// 先頭の項 $c h^p$。相殺で消えたら打ち切りを上げてやり直す（指数 40 まで）
pub fn leading(t: &Term, x: &str, map: &Map) -> Result<(Rat, Term), Stop> {
    let mut last = Stop::NotFound("先頭の項が決まりません".into());
    for cap in [4i64, 8, 16, 40] {
        match expand(t, x, map, &r(cap)) {
            Ok(s) => {
                if let Some((e, c)) = s.lead() {
                    if e < s.order {
                        return Ok((e, c));
                    }
                }
            }
            Err(Stop::TimeUp) => return Err(Stop::TimeUp),
            Err(e) => last = e,
        }
    }
    Err(last)
}


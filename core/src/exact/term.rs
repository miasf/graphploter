//! 厳密値の道の式の形 [`Term`] と、自動の正規化（計画 `docs/plan-exact-core.md` の EX2）。
//!
//! 式は作るたびに正規化する（Wolfram の自動の評価と同じ）。正規化は**いつも正しい書き換えだけ**:
//! - 数どうしは分数のまま計算する。数の根号は「有理数 × $R^{1/n}$」（$R$ は $n$ 乗の因子を持たない 2 以上の整数）にする（$\sqrt8 = 2\sqrt2$）
//! - 和は入れ子をほどき、同類項をまとめ、並べる（数の項は最後に 1 つ）。積も同じ（数の係数は先頭に 1 つ、同じ底のべきはまとめる）
//! - **数だけの式**（文字を含まない式）は、積・整数乗を展開して和の形にする（$(1+\sqrt2)^2 = 3 + 2\sqrt2$）。
//!   分母が「有理数 + 有理数 × 根号 1 つ」か「有理数 + 有理数 × $i$」なら共役を掛けて分母を払う
//! - 文字を含む式は展開しない（$(x+1)^2$ のまま）。数の係数だけは和に配る（$2(x+1) = 2x + 2$）
//! - $\sqrt{x^2}$ を $x$ に、$(x^a)^b$ を $x^{ab}$ にするような、文字の性質が要る書き換えはしない（EX8）。底が正の数なら $(c^a)^b = c^{ab}$ にする
//! - $\ln$ は正の有理数なら素数の対数の和にする（$\ln 8 = 3\ln 2$）。$e^{c\ln a}$ は $a^c$（$a$ は正の数）
//! - 関数は自明な値だけ（$\sin 0$・$\ln 1$）と、整数の関数（階乗・二項係数・GCD・mod）。特別な値の表は文書 8 の SC0

use super::num::{self, Int, Nat, Rat};
use super::{spend, Budget};
use crate::functions::Func;
use dashu::base::Abs;
use std::collections::BTreeMap;

/// 定数の原子（計画の 2.8）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Atom {
    Pi,
    E,
    I,
    EulerGamma,
    Catalan,
}

/// 式の形。`Ord` は「正規の順」（同類項を見つけるための順で、見せる順ではない。見せる順は EX4）
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Term {
    Num(Rat),
    Atom(Atom),
    /// 文字（座標・束縛された変数）
    Sym(String),
    /// 2 個以上。正規の順に並べ、同類項はまとめ済み。数の項は最後に 1 つだけ
    Add(Vec<Term>),
    /// 2 個以上。数の係数は先頭に 1 つだけ。同じ底はまとめ済み
    Mul(Vec<Term>),
    Pow(Box<Term>, Box<Term>),
    Fn(Func, Vec<Term>),
    Tuple(Vec<Term>),
    List(Vec<Term>),
}

impl Term {
    pub fn int(v: i64) -> Term {
        Term::Num(Rat::from(Int::from(v)))
    }

    pub fn rat(n: i64, d: u64) -> Term {
        Term::Num(Rat::from_parts(Int::from(n), Nat::from(d)))
    }

    pub fn zero() -> Term {
        Term::int(0)
    }

    pub fn one() -> Term {
        Term::int(1)
    }

    pub fn as_rat(&self) -> Option<&Rat> {
        match self {
            Term::Num(q) => Some(q),
            _ => None,
        }
    }

    pub fn is_zero(&self) -> bool {
        matches!(self, Term::Num(q) if q.is_zero())
    }

    pub fn is_one(&self) -> bool {
        matches!(self, Term::Num(q) if *q == Rat::ONE)
    }

    /// 文字を含まない（数だけの）式か
    pub fn is_number(&self) -> bool {
        match self {
            Term::Num(_) | Term::Atom(_) => true,
            Term::Sym(_) => false,
            Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) => v.iter().all(Term::is_number),
            Term::Pow(b, e) => b.is_number() && e.is_number(),
            Term::Tuple(_) | Term::List(_) => false,
        }
    }

    /// 節の数（大きさの上限に使う）
    pub fn size(&self) -> usize {
        match self {
            Term::Num(_) | Term::Atom(_) | Term::Sym(_) => 1,
            Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) => 1 + v.iter().map(Term::size).sum::<usize>(),
            Term::Pow(b, e) => 1 + b.size() + e.size(),
        }
    }

    /// 0 で割った所（$0^{-k}$）を含むか
    pub fn has_division_by_zero(&self) -> bool {
        match self {
            Term::Pow(b, e) => (b.is_zero() && e.as_rat().is_some_and(|q| num::sign(q) < 0)) || b.has_division_by_zero() || e.has_division_by_zero(),
            Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) => v.iter().any(Term::has_division_by_zero),
            _ => false,
        }
    }
}

/// 係数と残り（`3x` → (3, x)、`5` → (5, 1)）
pub fn split_coeff(t: &Term) -> (Rat, Term) {
    match t {
        Term::Num(q) => (q.clone(), Term::one()),
        Term::Mul(v) => match v.first() {
            Some(Term::Num(q)) => {
                let rest: Vec<Term> = v[1..].to_vec();
                (q.clone(), if rest.len() == 1 { rest.into_iter().next().unwrap() } else { Term::Mul(rest) })
            }
            _ => (Rat::ONE, t.clone()),
        },
        _ => (Rat::ONE, t.clone()),
    }
}

/// 係数 × 残り（残りは正規化済み・Add でない）
fn with_coeff(c: Rat, rest: Term) -> Term {
    if c.is_zero() {
        return Term::zero();
    }
    if rest.is_one() {
        return Term::Num(c);
    }
    if c == Rat::ONE {
        return rest;
    }
    match rest {
        Term::Mul(v) => {
            let mut out = vec![Term::Num(c)];
            out.extend(v);
            Term::Mul(out)
        }
        other => Term::Mul(vec![Term::Num(c), other]),
    }
}

/// 和（正規化する）
pub fn add(terms: Vec<Term>) -> Term {
    let mut constant = Rat::ZERO;
    let mut map: BTreeMap<Term, Rat> = BTreeMap::new();
    fn push(t: Term, constant: &mut Rat, map: &mut BTreeMap<Term, Rat>) {
        match t {
            Term::Add(v) => {
                for x in v {
                    push(x, constant, map);
                }
            }
            Term::Num(q) => *constant += q,
            other => {
                let (c, r) = split_coeff(&other);
                *map.entry(r).or_insert(Rat::ZERO) += c;
            }
        }
    }
    for t in terms {
        push(t, &mut constant, &mut map);
    }
    spend(map.len() as u64 + 1);
    let mut out: Vec<Term> = map.into_iter().filter(|(_, c)| !c.is_zero()).map(|(r, c)| with_coeff(c, r)).collect();
    if !constant.is_zero() {
        out.push(Term::Num(constant));
    }
    match out.len() {
        0 => Term::zero(),
        1 => out.pop().unwrap(),
        _ => Term::Add(out),
    }
}

pub fn neg(t: Term) -> Term {
    mul(vec![Term::int(-1), t])
}

pub fn sub(a: Term, b: Term) -> Term {
    add(vec![a, neg(b)])
}

pub fn div(a: Term, b: Term) -> Term {
    mul(vec![a, pow(b, Term::int(-1))])
}

/// 積の因子を集める: 係数と、底 → 指数の和
fn collect(factors: Vec<Term>, coef: &mut Rat, bases: &mut BTreeMap<Term, Vec<Term>>) {
    for f in factors {
        match f {
            Term::Num(q) => *coef *= q,
            Term::Mul(v) => collect(v, coef, bases),
            Term::Pow(b, e) => bases.entry(*b).or_default().push(*e),
            other => bases.entry(other).or_default().push(Term::one()),
        }
    }
}

/// 積（正規化する）
pub fn mul(factors: Vec<Term>) -> Term {
    let mut coef = Rat::ONE;
    let mut bases: BTreeMap<Term, Vec<Term>> = BTreeMap::new();
    collect(factors, &mut coef, &mut bases);
    // 底ごとに指数を足して pow で作り直す。pow の結果に係数や新しい因子が出たら、もう一度集める（数の根号の整理）
    let mut fs: Vec<Term> = Vec::new();
    for _round in 0..4 {
        if coef.is_zero() {
            return Term::zero();
        }
        let mut again: Vec<Term> = Vec::new();
        let mut settled = true;
        for (b, es) in std::mem::take(&mut bases) {
            let e = add(es);
            let p = pow(b, e);
            match p {
                Term::Num(q) => coef *= q,
                Term::Mul(_) => {
                    settled = false;
                    again.push(p);
                }
                other => again.push(other),
            }
        }
        if settled {
            fs = again;
            break;
        }
        collect(again, &mut coef, &mut bases);
        if bases.is_empty() {
            break;
        }
    }
    if !bases.is_empty() {
        // 4 回で落ち着かなかった（起きないはず）: そのまま並べる
        for (b, es) in bases {
            fs.push(pow(b, add(es)));
        }
    }
    if coef.is_zero() {
        return Term::zero();
    }
    combine_radicals(&mut coef, &mut fs);
    spend(fs.len() as u64 + 1);
    // 数だけの積に和があれば展開する。文字を含む積でも、因子が和 1 つだけなら数の係数を配る
    let all_numbers = fs.iter().all(Term::is_number);
    let has_add = fs.iter().any(|f| matches!(f, Term::Add(_)));
    if has_add && all_numbers && fs.len() > 1 {
        return expand_product(coef, fs);
    }
    if fs.len() == 1 && coef != Rat::ONE {
        if let Term::Add(v) = &fs[0] {
            return add(v.iter().map(|t| mul(vec![Term::Num(coef.clone()), t.clone()])).collect());
        }
    }
    fs.sort();
    match (fs.len(), coef == Rat::ONE) {
        (0, _) => Term::Num(coef),
        (1, true) => fs.pop().unwrap(),
        _ => {
            let mut out = Vec::with_capacity(fs.len() + 1);
            if coef != Rat::ONE {
                out.push(Term::Num(coef));
            }
            out.extend(fs);
            Term::Mul(out)
        }
    }
}

/// 数の根号 $R^{1/n}$ どうしで、同じ $n$ のものを 1 つにまとめる（$\sqrt2\sqrt3 = \sqrt6$、$\sqrt2\sqrt6 = 2\sqrt3$）
fn combine_radicals(coef: &mut Rat, fs: &mut Vec<Term>) {
    let mut by_n: BTreeMap<u64, Int> = BTreeMap::new();
    let mut rest = Vec::new();
    for f in fs.drain(..) {
        if let Some((r, n)) = radical_parts(&f) {
            let e = by_n.entry(n).or_insert(Int::ONE);
            *e *= r;
        } else {
            rest.push(f);
        }
    }
    for (n, r) in by_n {
        match rational_power(&Rat::from(r), &Rat::from_parts(Int::ONE, Nat::from(n))) {
            Term::Num(q) => *coef *= q,
            Term::Mul(v) => {
                for t in v {
                    match t {
                        Term::Num(q) => *coef *= q,
                        other => rest.push(other),
                    }
                }
            }
            other => rest.push(other),
        }
    }
    *fs = rest;
}

/// $R^{1/n}$（$R$ は 2 以上の整数）なら (R, n)
fn radical_parts(t: &Term) -> Option<(Int, u64)> {
    let Term::Pow(b, e) = t else { return None };
    let (Term::Num(r), Term::Num(q)) = (&**b, &**e) else { return None };
    let r = num::as_int(r)?;
    if r < Int::from(2) || *q.numerator() != Int::ONE {
        return None;
    }
    Some((r, u64::try_from(q.denominator()).ok()?))
}

/// 数だけの因子の積を、和の形に展開する（大きさは仕事の量で数える）
fn expand_product(coef: Rat, fs: Vec<Term>) -> Term {
    let mut acc: Vec<Term> = vec![Term::Num(coef)];
    for f in fs {
        let parts: Vec<Term> = match f {
            Term::Add(v) => v,
            other => vec![other],
        };
        let mut next = Vec::with_capacity(acc.len() * parts.len());
        for a in &acc {
            for p in &parts {
                next.push(mul(vec![a.clone(), p.clone()]));
            }
        }
        spend(next.len() as u64);
        acc = match add(next) {
            Term::Add(v) => v,
            other => vec![other],
        };
    }
    add(acc)
}

/// $(-1)^s$（$-1 < s \le 1$）の主値。cos・sin の値が表にある $s$ だけ
fn minus_one_power(s: &Rat) -> Option<Term> {
    let d = u64::try_from(s.denominator()).ok()?;
    let nm = i64::try_from(s.numerator()).ok()?;
    // e^{iπs} = cos πs + i sin πs。s = nm/d、d ∈ {1, 2, 3, 4, 6}
    let (c, sn) = trig_pi(nm, d)?;
    Some(add(vec![c, mul(vec![sn, Term::Atom(Atom::I)])]))
}

/// $(\cos\frac{p\pi}{q}, \sin\frac{p\pi}{q})$（$q \in \{1, 2, 3, 4, 6\}$）。文書 8 の SC0 で表を広げる
pub(crate) fn trig_pi(p: i64, q: u64) -> Option<(Term, Term)> {
    if ![1, 2, 3, 4, 6].contains(&q) {
        return None;
    }
    // 12 等分の角の番号（0..24）
    let k = (p * (12 / q as i64)).rem_euclid(24);
    let s2 = || pow(Term::int(2), Term::rat(1, 2));
    let s3 = || pow(Term::int(3), Term::rat(1, 2));
    let half = |t: Term| mul(vec![Term::rat(1, 2), t]);
    // 第 1 象限の値 (cos, sin) を k mod 6 で
    let quad = |j: i64| -> (Term, Term) {
        match j {
            0 => (Term::one(), Term::zero()),
            2 => (half(s3()), Term::rat(1, 2)),
            3 => (half(s2()), half(s2())),
            4 => (Term::rat(1, 2), half(s3())),
            6 => (Term::zero(), Term::one()),
            _ => unreachable!(),
        }
    };
    if k % 6 == 1 || k % 6 == 5 {
        return None; // π/12 の奇数倍（SC0）
    }
    let (c, s) = match k {
        0..=6 => quad(k),
        7..=12 => {
            let (c, s) = quad(12 - k);
            (neg(c), s)
        }
        13..=18 => {
            let (c, s) = quad(k - 12);
            (neg(c), neg(s))
        }
        _ => {
            let (c, s) = quad(24 - k);
            (c, neg(s))
        }
    };
    Some((c, s))
}

/// 有理数の有理数乗（計画の EX2 の 1）
pub fn rational_power(b: &Rat, e: &Rat) -> Term {
    if let Some(k) = num::as_int(e) {
        if b.is_zero() {
            return if num::sign(e) > 0 { Term::zero() } else if num::sign(e) == 0 { Term::one() } else { Term::Pow(Box::new(Term::zero()), Box::new(Term::Num(e.clone()))) };
        }
        let Ok(k) = i64::try_from(&k) else { return Term::Pow(Box::new(Term::Num(b.clone())), Box::new(Term::Num(e.clone()))) };
        if k.unsigned_abs() > 100_000 {
            return Term::Pow(Box::new(Term::Num(b.clone())), Box::new(Term::Num(e.clone())));
        }
        spend(k.unsigned_abs() / 64 + 1);
        return Term::Num(b.pow(k as isize));
    }
    if b.is_zero() {
        return if num::sign(e) > 0 { Term::zero() } else { Term::Pow(Box::new(Term::zero()), Box::new(Term::Num(e.clone()))) };
    }
    if num::sign(b) < 0 {
        // 主値: (−b)^e = |b|^e · (−1)^e
        let s = reduce_mod_two(e);
        let mag = rational_power(&b.clone().abs(), e);
        let unit = match minus_one_power(&s) {
            Some(u) => u,
            None => Term::Pow(Box::new(Term::int(-1)), Box::new(Term::Num(s))),
        };
        return mul(vec![mag, unit]);
    }
    // b > 0、e = m/n（n > 1）。b^{m/n} = (x/y)^{1/n}（m < 0 なら分子と分母を入れ替える）
    let m = e.numerator().clone();
    let Ok(n) = u64::try_from(e.denominator()) else { return Term::Pow(Box::new(Term::Num(b.clone())), Box::new(Term::Num(e.clone()))) };
    let (num_part, den_part) = (Int::from(b.numerator().clone()), Int::from(b.denominator().clone()));
    let (xb, yb) = if num::sign(&Rat::from(m.clone())) < 0 { (den_part, num_part) } else { (num_part, den_part) };
    let Ok(ma) = usize::try_from(m.clone().abs()) else { return Term::Pow(Box::new(Term::Num(b.clone())), Box::new(Term::Num(e.clone()))) };
    if ma > 10_000 {
        return Term::Pow(Box::new(Term::Num(b.clone())), Box::new(Term::Num(e.clone())));
    }
    let x = xb.pow(ma);
    let y = yb.pow(ma);
    // (x/y)^{1/n} = (x y^{n−1})^{1/n} / y
    let big_n = &x * y.pow(n as usize - 1);
    let (outside, radicand, degree) = root_split(&big_n, n);
    let coef = Rat::from_parts(outside, Nat::ONE) / Rat::from(y);
    if radicand == Int::ONE {
        return Term::Num(coef);
    }
    let rad = Term::Pow(Box::new(Term::Num(Rat::from(radicand))), Box::new(Term::rat(1, degree)));
    if coef == Rat::ONE {
        rad
    } else {
        Term::Mul(vec![Term::Num(coef), rad])
    }
}

/// $s \bmod 2$ を $(-1, 1]$ に寄せる
fn reduce_mod_two(s: &Rat) -> Rat {
    let two = Rat::from(Int::from(2));
    let mut r = s.clone();
    // floor((r + 1) / 2) を引く
    let k = (r.clone() + Rat::ONE) / &two;
    let fl = k.floor();
    r -= Rat::from(fl) * &two;
    if r <= -Rat::ONE {
        r += two;
    }
    r
}

/// $N^{1/n} = c \cdot R^{1/d}$（$R$ は $d$ 乗の因子を持たない。$d \mid n$ まで次数を下げる）
fn root_split(nn: &Int, n: u64) -> (Int, Int, u64) {
    let budget = Budget::new(20_000);
    let (fs, rest) = match num::factor_int(nn, &budget) {
        Ok(v) => v,
        Err(_) => (Vec::new(), Some(nn.clone())),
    };
    let mut outside = Int::ONE;
    let mut parts: Vec<(Int, u64)> = Vec::new();
    for (p, k) in fs {
        let k = k as u64;
        outside *= p.pow((k / n) as usize);
        if k % n != 0 {
            parts.push((p, k % n));
        }
    }
    // 次数を下げる: g = gcd(n, 指数たち)
    let mut g = n;
    for (_, k) in &parts {
        g = gcd_u64(g, *k);
    }
    if rest.is_some() {
        g = 1;
    }
    let d = n / g.max(1);
    let mut radicand = Int::ONE;
    for (p, k) in parts {
        radicand *= p.pow((k / g.max(1)) as usize);
    }
    if let Some(r) = rest {
        radicand *= r.pow(1);
    }
    let degree = if radicand == Int::ONE { 1 } else { d };
    (outside, radicand, degree)
}

fn gcd_u64(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd_u64(b, a % b)
    }
}

/// べき（正規化する）
pub fn pow(b: Term, e: Term) -> Term {
    if e.is_zero() {
        return Term::one();
    }
    if e.is_one() {
        return b;
    }
    if b.is_one() {
        return Term::one();
    }
    match (&b, &e) {
        (Term::Num(qb), Term::Num(qe)) => return rational_power(qb, qe),
        (Term::Atom(Atom::I), Term::Num(qe)) => {
            if let Some(k) = num::as_int(qe) {
                let r = i64::try_from(&(k % Int::from(4))).unwrap_or(0).rem_euclid(4);
                return match r {
                    0 => Term::one(),
                    1 => Term::Atom(Atom::I),
                    2 => Term::int(-1),
                    _ => neg(Term::Atom(Atom::I)),
                };
            }
            // i^s = e^{iπs/2}
            return pow(Term::Atom(Atom::E), mul(vec![e.clone(), Term::Atom(Atom::Pi), Term::Atom(Atom::I), Term::rat(1, 2)]));
        }
        (Term::Atom(Atom::I), _) => {
            return pow(Term::Atom(Atom::E), mul(vec![e.clone(), Term::Atom(Atom::Pi), Term::Atom(Atom::I), Term::rat(1, 2)]));
        }
        (Term::Atom(Atom::E), _) => {
            if let Some(t) = exp_rules(&e) {
                return t;
            }
        }
        (Term::Pow(c, f), Term::Num(qe)) if num::as_int(qe).is_some() => return pow((**c).clone(), mul(vec![(**f).clone(), e.clone()])),
        (Term::Pow(c, f), _) if is_positive_number(c) => return pow((**c).clone(), mul(vec![(**f).clone(), e.clone()])),
        (Term::Mul(v), Term::Num(qe)) if num::as_int(qe).is_some() => return mul(v.iter().map(|t| pow(t.clone(), e.clone())).collect()),
        (Term::Mul(v), _) => {
            // 正の数の因子は外に出せる: (4x)^{1/2} = 2 x^{1/2}
            let (pos, other): (Vec<Term>, Vec<Term>) = v.iter().cloned().partition(is_positive_number);
            if !pos.is_empty() && !other.is_empty() {
                let mut out: Vec<Term> = pos.into_iter().map(|t| pow(t, e.clone())).collect();
                out.push(Term::Pow(Box::new(mul(other)), Box::new(e.clone())));
                return mul(out);
            }
        }
        (Term::Add(_), Term::Num(qe)) if b.is_number() => {
            if let Some(k) = num::as_int(qe).and_then(|k| i64::try_from(&k).ok()) {
                if k > 0 && k <= 64 {
                    return expand_product(Rat::ONE, vec![b.clone(); k as usize]);
                }
                if k < 0 && k >= -64 {
                    if let Some(inv) = conjugate_inverse(&b) {
                        return pow(inv, Term::int(-k));
                    }
                }
            }
        }
        _ => {}
    }
    Term::Pow(Box::new(b), Box::new(e))
}

/// 正の数と分かる式（正の有理数・π・e・正の数の根号とその積）
pub fn is_positive_number(t: &Term) -> bool {
    match t {
        Term::Num(q) => num::sign(q) > 0,
        Term::Atom(Atom::Pi | Atom::E | Atom::EulerGamma | Atom::Catalan) => true,
        Term::Pow(b, e) => is_positive_number(b) && e.as_rat().is_some(),
        Term::Mul(v) => v.iter().all(is_positive_number),
        _ => false,
    }
}

/// $e^{x}$ の規則: $x = \sum c_j \ln a_j + (\text{残り})$ で $a_j$ が正の数なら $\prod a_j^{c_j} \cdot e^{\text{残り}}$。
/// 残りの中の $i\pi s$（$s$ は 6 で割り切れる分母の有理数）は $\cos\pi s + i\sin\pi s$
fn exp_rules(x: &Term) -> Option<Term> {
    let parts: Vec<Term> = match x {
        Term::Add(v) => v.clone(),
        other => vec![other.clone()],
    };
    let mut pulled: Vec<Term> = Vec::new();
    let mut rest: Vec<Term> = Vec::new();
    let mut ipi = Rat::ZERO;
    for p in parts {
        let (c, r) = split_coeff(&p);
        match &r {
            Term::Fn(Func::Ln, a) if a.len() == 1 && is_positive_number(&a[0]) => pulled.push(pow(a[0].clone(), Term::Num(c))),
            Term::Mul(v) if v.len() == 2 && v.contains(&Term::Atom(Atom::Pi)) && v.contains(&Term::Atom(Atom::I)) => ipi += c,
            _ => rest.push(p),
        }
    }
    let mut euler = None;
    if !ipi.is_zero() {
        let s = reduce_mod_two(&ipi);
        match minus_one_power(&s) {
            Some(u) => euler = Some(u),
            None => rest.push(mul(vec![Term::Num(ipi), Term::Atom(Atom::Pi), Term::Atom(Atom::I)])),
        }
    }
    if pulled.is_empty() && euler.is_none() {
        return None;
    }
    let mut out = pulled;
    if let Some(u) = euler {
        out.push(u);
    }
    if !rest.is_empty() {
        out.push(Term::Pow(Box::new(Term::Atom(Atom::E)), Box::new(add(rest))));
    }
    Some(mul(out))
}

/// 実部と虚部（数だけの式で、$i$ 以外の原子が全部実数のとき）
pub fn re_im(t: &Term) -> Option<(Term, Term)> {
    let parts: Vec<Term> = match t {
        Term::Add(v) => v.clone(),
        other => vec![other.clone()],
    };
    let mut re = Vec::new();
    let mut im = Vec::new();
    for p in parts {
        let fs: Vec<Term> = match &p {
            Term::Mul(v) => v.clone(),
            other => vec![other.clone()],
        };
        let has_i = fs.iter().filter(|f| **f == Term::Atom(Atom::I)).count();
        let others: Vec<Term> = fs.iter().filter(|f| **f != Term::Atom(Atom::I)).cloned().collect();
        if !others.iter().all(is_real_number) {
            return None;
        }
        match has_i {
            0 => re.push(p),
            1 => im.push(mul(others)),
            _ => return None,
        }
    }
    Some((add(re), add(im)))
}

/// 実数と分かる数（有理数・π・e・正の数の根号・正の数の対数）
fn is_real_number(t: &Term) -> bool {
    match t {
        Term::Num(_) => true,
        Term::Atom(a) => *a != Atom::I,
        Term::Pow(b, e) => is_positive_number(b) && e.as_rat().is_some(),
        Term::Mul(v) | Term::Add(v) => v.iter().all(is_real_number),
        Term::Fn(Func::Ln, a) => a.len() == 1 && is_positive_number(&a[0]),
        _ => false,
    }
}

/// 分母を払う: $a + b\sqrt d$ と $a + bi$（$a, b$ は有理数）の逆数
fn conjugate_inverse(t: &Term) -> Option<Term> {
    let Term::Add(v) = t else { return None };
    if v.len() != 2 {
        return None;
    }
    let a = v.iter().find_map(|p| p.as_rat().cloned())?;
    let other = v.iter().find(|p| p.as_rat().is_none())?;
    let (b, r) = split_coeff(other);
    let norm = match &r {
        Term::Atom(Atom::I) => a.clone() * &a + b.clone() * &b,
        Term::Pow(base, e) if matches!(&**e, Term::Num(q) if *q == Rat::from_parts(Int::ONE, Nat::from(2u8))) => {
            let d = base.as_rat()?.clone();
            a.clone() * &a - b.clone() * &b * d
        }
        _ => return None,
    };
    if norm.is_zero() {
        return None;
    }
    // 1/(a + b r) = (a − b r) / norm
    Some(add(vec![Term::Num(a / &norm), with_coeff(-b / norm, r)]))
}

/// 関数の呼び出し（自明な値と整数の関数だけ計算する）
pub fn call(f: Func, args: Vec<Term>) -> Term {
    if let Some(t) = call_rules(f, &args) {
        return t;
    }
    Term::Fn(f, args)
}

fn int_arg(t: &Term) -> Option<Int> {
    t.as_rat().and_then(num::as_int)
}

fn call_rules(f: Func, a: &[Term]) -> Option<Term> {
    let one = |v: &[Term]| if v.len() == 1 { Some(v[0].clone()) } else { None };
    match f {
        Func::Sqrt => Some(pow(one(a)?, Term::rat(1, 2))),
        Func::Exp => Some(pow(Term::Atom(Atom::E), one(a)?)),
        Func::Sin | Func::Tan | Func::Sinh | Func::Tanh | Func::Asin | Func::Atan | Func::Asinh | Func::Atanh if a.len() == 1 && a[0].is_zero() => Some(Term::zero()),
        Func::Cos | Func::Cosh if a.len() == 1 && a[0].is_zero() => Some(Term::one()),
        Func::Ln => ln_rules(&one(a)?),
        Func::Log10 => {
            let x = one(a)?;
            let q = x.as_rat()?;
            // 10^k なら k
            let mut k: i64 = 0;
            let mut r = q.clone();
            let ten = Rat::from(Int::from(10));
            if num::sign(&r) <= 0 {
                return None;
            }
            while r >= ten && k < 400 {
                r /= &ten;
                k += 1;
            }
            while r < Rat::ONE && k > -400 {
                r *= &ten;
                k -= 1;
            }
            (r == Rat::ONE).then(|| Term::int(k))
        }
        Func::Abs => {
            let x = one(a)?;
            if let Some(q) = x.as_rat() {
                return Some(Term::Num(q.clone().abs()));
            }
            if is_positive_number(&x) {
                return Some(x);
            }
            let (re, im) = re_im(&x)?;
            if im.is_zero() {
                return None;
            }
            Some(pow(add(vec![mul(vec![re.clone(), re]), mul(vec![im.clone(), im])]), Term::rat(1, 2)))
        }
        Func::Gamma => {
            let n = int_arg(&one(a)?)?;
            if n < Int::ONE || n > Int::from(100_000) {
                return None;
            }
            let k = u64::try_from(&(n - Int::ONE)).ok()?;
            let budget = Budget::new(1_000_000);
            num::factorial(k, &budget).ok().map(|v| Term::Num(Rat::from(v)))
        }
        Func::NCr if a.len() == 2 => {
            let (n, k) = (int_arg(&a[0])?, int_arg(&a[1])?);
            let budget = Budget::new(1_000_000);
            num::binomial(&n, &k, &budget).ok().map(|v| Term::Num(Rat::from(v)))
        }
        Func::Gcd if a.len() == 2 => Some(Term::Num(Rat::from(num::gcd(&int_arg(&a[0])?, &int_arg(&a[1])?)))),
        Func::Lcm if a.len() == 2 => Some(Term::Num(Rat::from(num::lcm(&int_arg(&a[0])?, &int_arg(&a[1])?)))),
        Func::Mod if a.len() == 2 => {
            // mod(a, b) = a − b⌊a/b⌋（今の道と同じ: 結果は b と同じ符号）
            let (x, y) = (a[0].as_rat()?, a[1].as_rat()?);
            if y.is_zero() {
                return None;
            }
            let fl = (x.clone() / y).floor();
            Some(Term::Num(x.clone() - y.clone() * Rat::from(fl)))
        }
        _ => None,
    }
}

/// $\ln$ の規則（正の有理数は素数の対数の和、負の数は $\ln|x| + i\pi$、$\ln e^x = x$（$x$ は実数））
fn ln_rules(x: &Term) -> Option<Term> {
    match x {
        Term::Num(q) => {
            if q.is_zero() {
                return None;
            }
            if num::sign(q) < 0 {
                return Some(add(vec![ln_rules(&Term::Num(q.clone().abs())).unwrap_or_else(|| Term::Fn(Func::Ln, vec![Term::Num(q.clone().abs())])), mul(vec![Term::Atom(Atom::Pi), Term::Atom(Atom::I)])]));
            }
            if *q == Rat::ONE {
                return Some(Term::zero());
            }
            let budget = Budget::new(20_000);
            let (pn, rn) = num::factor_int(&Int::from(q.numerator().clone()), &budget).ok()?;
            let (pd, rd) = num::factor_int(&Int::from(q.denominator().clone()), &budget).ok()?;
            if rn.is_some() || rd.is_some() {
                return None;
            }
            if pn.len() + pd.len() == 1 && pn.iter().chain(pd.iter()).all(|(_, e)| *e == 1) && pd.is_empty() {
                return None; // 素数そのもの: ln p のまま
            }
            let mut out = Vec::new();
            for (p, e) in pn {
                out.push(mul(vec![Term::int(e as i64), Term::Fn(Func::Ln, vec![Term::Num(Rat::from(p))])]));
            }
            for (p, e) in pd {
                out.push(mul(vec![Term::int(-(e as i64)), Term::Fn(Func::Ln, vec![Term::Num(Rat::from(p))])]));
            }
            Some(add(out))
        }
        Term::Atom(Atom::E) => Some(Term::one()),
        Term::Atom(Atom::I) => Some(mul(vec![Term::rat(1, 2), Term::Atom(Atom::Pi), Term::Atom(Atom::I)])),
        Term::Pow(b, e) if **b == Term::Atom(Atom::E) && is_real_number(e) => Some((**e).clone()),
        Term::Pow(b, e) if is_positive_number(b) && e.as_rat().is_some() => {
            let inner = call(Func::Ln, vec![(**b).clone()]);
            Some(mul(vec![(**e).clone(), inner]))
        }
        Term::Mul(v) if v.iter().all(is_positive_number) => Some(add(v.iter().map(|t| call(Func::Ln, vec![t.clone()])).collect())),
        _ => None,
    }
}

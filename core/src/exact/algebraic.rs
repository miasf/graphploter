//! 平方根の数の体 $\mathbb Q(\sqrt{p_1}, \ldots, \sqrt{p_k})$ の計算（計画 `docs/plan-exact-core.md` の EX7 の一部）。
//!
//! - 有理化: 分母に平方根がいくつあっても、素数 $p$ ごとに「$\sqrt p$ の符号を変えた共役」を掛けて消す
//!   （$\frac{1}{1 + \sqrt2 + \sqrt3} = \frac{2 + \sqrt2 - \sqrt6}{4}$、$\frac{2}{\sqrt5 + \sqrt3} = \sqrt5 - \sqrt3$）
//! - 入れ子の根号: $\sqrt{a + b\sqrt c}$ で $a^2 - b^2c$ が有理数の平方なら $\sqrt{\frac{a+d}{2}} \pm \sqrt{\frac{a-d}{2}}$
//!
//! 数は「素数の集合 → 有理数の係数」の表で持つ（$\sqrt{6} = \sqrt2\sqrt3$ は {2, 3}）。$\sqrt p \cdot \sqrt p = p$

use super::num::{self, Int, Nat, Rat};
use super::term::{add, mul, pow, Term};
use super::Budget;
use std::collections::BTreeMap;

/// $\sum q_S \prod_{p \in S}\sqrt p$
#[derive(Debug, Clone, PartialEq)]
pub struct QRad(BTreeMap<Vec<u64>, Rat>);

impl QRad {
    pub fn rational(q: Rat) -> QRad {
        let mut m = BTreeMap::new();
        if !q.is_zero() {
            m.insert(Vec::new(), q);
        }
        QRad(m)
    }

    pub fn add(&self, o: &QRad) -> QRad {
        let mut m = self.0.clone();
        for (k, v) in &o.0 {
            let e = m.entry(k.clone()).or_insert(Rat::ZERO);
            *e += v;
        }
        m.retain(|_, v| !v.is_zero());
        QRad(m)
    }

    pub fn mul(&self, o: &QRad) -> QRad {
        let mut m: BTreeMap<Vec<u64>, Rat> = BTreeMap::new();
        for (ka, va) in &self.0 {
            for (kb, vb) in &o.0 {
                // 対称差と、共通の素数の積（√p √p = p）
                let s: Vec<u64> = ka.iter().filter(|p| !kb.contains(p)).chain(kb.iter().filter(|p| !ka.contains(p))).copied().collect();
                let mut s = s;
                let mut both = Int::ONE;
                for p in ka {
                    if kb.contains(p) {
                        both *= Int::from(*p);
                    }
                }
                s.sort();
                let e = m.entry(s).or_insert(Rat::ZERO);
                *e += va.clone() * vb * Rat::from(both);
            }
        }
        m.retain(|_, v| !v.is_zero());
        QRad(m)
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_empty()
    }

    /// 有理数なら、その値
    pub fn as_rational(&self) -> Option<Rat> {
        match self.0.len() {
            0 => Some(Rat::ZERO),
            1 => self.0.get(&Vec::new()).cloned(),
            _ => None,
        }
    }

    /// f64 の近似値（並べるため）
    pub fn approx(&self) -> f64 {
        self.0.iter().map(|(k, v)| v.to_f64().value() * (k.iter().product::<u64>() as f64).sqrt()).sum()
    }

    /// $\sqrt p \mapsto -\sqrt p$
    fn conj(&self, p: u64) -> QRad {
        QRad(self.0.iter().map(|(k, v)| (k.clone(), if k.contains(&p) { -v.clone() } else { v.clone() })).collect())
    }

    fn primes(&self) -> Vec<u64> {
        let mut ps: Vec<u64> = self.0.keys().flatten().copied().collect();
        ps.sort();
        ps.dedup();
        ps
    }

    /// 逆数（0 なら None）
    pub fn inverse(&self) -> Option<QRad> {
        if self.0.is_empty() {
            return None;
        }
        let mut num = QRad::rational(Rat::ONE);
        let mut den = self.clone();
        for p in self.primes() {
            let c = den.conj(p);
            num = num.mul(&c);
            den = den.mul(&c);
        }
        // 分母は有理数になっている
        if den.0.len() != 1 || !den.0.contains_key(&Vec::new()) {
            return None;
        }
        let d = den.0[&Vec::new()].clone();
        Some(num.mul(&QRad::rational(Rat::ONE / d)))
    }

    pub fn to_term(&self) -> Term {
        add(self.0.iter().map(|(k, v)| {
            let r: u64 = k.iter().product();
            if k.is_empty() {
                Term::Num(v.clone())
            } else {
                mul(vec![Term::Num(v.clone()), pow(Term::int(r as i64), Term::rat(1, 2))])
            }
        }).collect())
    }
}

/// 2 以上の平方因子の無い整数を素数に分ける
fn squarefree_primes(r: &Int) -> Option<Vec<u64>> {
    let budget = Budget::new(20_000);
    let (fs, rest) = num::factor_int(r, &budget).ok()?;
    if rest.is_some() {
        return None;
    }
    let mut out = Vec::new();
    for (p, e) in fs {
        if e != 1 {
            return None;
        }
        out.push(u64::try_from(&p).ok()?);
    }
    Some(out)
}

/// Term（有理数と、整数の平方根の有理数倍の和）を QRad に。それ以外は None
pub fn of_term(t: &Term) -> Option<QRad> {
    match t {
        Term::Num(q) => Some(QRad::rational(q.clone())),
        Term::Pow(b, e) => {
            let (Term::Num(r), Term::Num(q)) = (&**b, &**e) else { return None };
            if *q != Rat::from_parts(Int::ONE, Nat::from(2u8)) {
                return None;
            }
            let ps = squarefree_primes(&num::as_int(r)?)?;
            let mut m = BTreeMap::new();
            m.insert(ps, Rat::ONE);
            Some(QRad(m))
        }
        Term::Mul(v) => {
            let mut acc = QRad::rational(Rat::ONE);
            for f in v {
                acc = acc.mul(&of_term(f)?);
            }
            Some(acc)
        }
        Term::Add(v) => {
            let mut acc = QRad::rational(Rat::ZERO);
            for f in v {
                acc = acc.add(&of_term(f)?);
            }
            Some(acc)
        }
        _ => None,
    }
}

/// 平方根の和の逆数を、分母に根号の無い形で
pub fn inverse(t: &Term) -> Option<Term> {
    let q = of_term(t)?;
    if q.primes().len() > 6 {
        return None;
    }
    Some(q.inverse()?.to_term())
}

/// $\sqrt{a + b\sqrt c}$ の入れ子を外す（外せなければ None）
pub fn denest_sqrt(t: &Term) -> Option<Term> {
    let q = of_term(t)?;
    // a + b √c の形（根号の項が 1 つ）
    let a = q.0.get(&Vec::new()).cloned().unwrap_or(Rat::ZERO);
    let rad: Vec<(&Vec<u64>, &Rat)> = q.0.iter().filter(|(k, _)| !k.is_empty()).collect();
    let [(ks, b)] = rad.as_slice() else { return None };
    let c: Rat = Rat::from(Int::from(ks.iter().product::<u64>()));
    if num::sign(&a) <= 0 {
        return None;
    }
    let d2 = a.clone() * &a - (*b).clone() * *b * &c;
    if num::sign(&d2) < 0 {
        return None;
    }
    let dn = num::exact_root(d2.numerator(), 2)?;
    let dd = num::exact_root(&Int::from(d2.denominator().clone()), 2)?;
    let d = Rat::from_parts_signed(dn, dd);
    let half = Rat::from_parts(Int::ONE, Nat::from(2u8));
    let (p, m) = ((a.clone() + &d) * &half, (a - &d) * &half);
    if num::sign(&m) < 0 {
        return None;
    }
    let s = |x: Rat| pow(Term::Num(x), Term::rat(1, 2));
    let second = if num::sign(b) < 0 { mul(vec![Term::int(-1), s(m)]) } else { s(m) };
    let out = add(vec![s(p), second]);
    // 外した結果も入れ子でない（二重に入れ子の数は外さない）
    of_term(&out).map(|_| out)
}

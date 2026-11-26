//! 1 変数の多項式（係数は有理数）（計画 `docs/plan-exact-core.md` の EX6）。
//!
//! 四則・割り算の余り・GCD・微分・無平方分解（Yun）・有理数の根・因数分解・部分分数・[`Term`] との変換。
//!
//! 因数分解は、有理数の根（1 次の因数）を取り、残りは Kronecker の方法で低い次数の因数から順に探す
//! （$d$ 次の因数は $d + 1$ 個の点の値の約数の組から補間して試す）。次数 $\lfloor n/2 \rfloor$ まで探して無ければ既約。
//! 探す手間が予算を超えたら、分解しきれなかった印（`complete = false`）を返す。
//! 計画の Zassenhaus（$\bmod p$ の分解とヘンゼル持ち上げ）の代わり（学校・工学の範囲の次数と係数で足りるため。0 章）

use super::num::{self, Int, Nat, Rat};
use super::term::{add, mul, pow, Term};
use super::{spend, Budget, TimeUp};
use dashu::base::Abs;

/// 多項式（係数は低い次数から。末尾の 0 は持たない）
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Poly(pub Vec<Rat>);

fn ri(n: i64) -> Rat {
    Rat::from(Int::from(n))
}

impl Poly {
    pub fn new(mut c: Vec<Rat>) -> Poly {
        while c.last().is_some_and(|x| x.is_zero()) {
            c.pop();
        }
        Poly(c)
    }

    pub fn zero() -> Poly {
        Poly(Vec::new())
    }

    pub fn constant(c: Rat) -> Poly {
        Poly::new(vec![c])
    }

    /// $x$
    pub fn x() -> Poly {
        Poly(vec![Rat::ZERO, Rat::ONE])
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_empty()
    }

    /// 次数（0 の多項式は −1）
    pub fn deg(&self) -> i64 {
        self.0.len() as i64 - 1
    }

    /// 最高次の係数
    pub fn lc(&self) -> Rat {
        self.0.last().cloned().unwrap_or(Rat::ZERO)
    }

    pub fn add(&self, o: &Poly) -> Poly {
        let n = self.0.len().max(o.0.len());
        Poly::new((0..n).map(|k| self.0.get(k).cloned().unwrap_or(Rat::ZERO) + o.0.get(k).cloned().unwrap_or(Rat::ZERO)).collect())
    }

    pub fn sub(&self, o: &Poly) -> Poly {
        self.add(&o.scale(&-Rat::ONE))
    }

    pub fn scale(&self, c: &Rat) -> Poly {
        Poly::new(self.0.iter().map(|a| a.clone() * c).collect())
    }

    pub fn mul(&self, o: &Poly) -> Poly {
        if self.is_zero() || o.is_zero() {
            return Poly::zero();
        }
        let mut c = vec![Rat::ZERO; self.0.len() + o.0.len() - 1];
        for (i, a) in self.0.iter().enumerate() {
            if a.is_zero() {
                continue;
            }
            for (j, b) in o.0.iter().enumerate() {
                c[i + j] += a.clone() * b;
            }
        }
        spend(c.len() as u64);
        Poly::new(c)
    }

    pub fn pow(&self, k: u32) -> Poly {
        let mut r = Poly::constant(Rat::ONE);
        for _ in 0..k {
            r = r.mul(self);
        }
        r
    }

    /// 割り算の商と余り（o は 0 でない）
    pub fn divrem(&self, o: &Poly) -> (Poly, Poly) {
        let mut r = self.clone();
        let mut q = vec![Rat::ZERO; (self.deg() - o.deg() + 1).max(0) as usize];
        let lc = o.lc();
        while !r.is_zero() && r.deg() >= o.deg() {
            let k = (r.deg() - o.deg()) as usize;
            let c = r.lc() / &lc;
            q[k] = c.clone();
            let mut sh = vec![Rat::ZERO; k];
            sh.extend(o.0.iter().map(|a| a.clone() * &c));
            r = r.sub(&Poly::new(sh));
        }
        (Poly::new(q), r)
    }

    pub fn monic(&self) -> Poly {
        if self.is_zero() {
            return Poly::zero();
        }
        let lc = self.lc();
        self.scale(&(Rat::ONE / lc))
    }

    pub fn deriv(&self) -> Poly {
        Poly::new(self.0.iter().enumerate().skip(1).map(|(k, a)| a.clone() * ri(k as i64)).collect())
    }

    pub fn eval(&self, x: &Rat) -> Rat {
        let mut acc = Rat::ZERO;
        for c in self.0.iter().rev() {
            acc = acc * x + c;
        }
        acc
    }

    /// 係数を整数にして、係数の GCD で割り、最高次の係数を正にしたもの（原始多項式）と、その倍率（self = c × 原始）
    pub fn primitive(&self) -> (Rat, Poly) {
        if self.is_zero() {
            return (Rat::ONE, Poly::zero());
        }
        let mut den = Int::ONE;
        for c in &self.0 {
            den = num::lcm(&den, &Int::from(c.denominator().clone()));
        }
        let ints: Vec<Int> = self.0.iter().map(|c| num::as_int(&(c.clone() * Rat::from(den.clone()))).unwrap()).collect();
        let mut g = Int::ZERO;
        for a in &ints {
            g = num::gcd(&g, a);
        }
        if *ints.last().unwrap() < Int::ZERO {
            g = -g;
        }
        let prim = Poly::new(ints.iter().map(|a| Rat::from(a / &g)).collect());
        (Rat::from_parts_signed(g, den), prim)
    }
}

/// 最大公約数（モニック）
pub fn gcd(a: &Poly, b: &Poly) -> Poly {
    let (mut x, mut y) = (a.clone(), b.clone());
    while !y.is_zero() {
        let r = x.divrem(&y).1;
        x = y;
        y = r;
    }
    x.monic()
}

/// 拡張ユークリッド: s a + t b = gcd(a, b)（モニック）
pub fn ext_gcd(a: &Poly, b: &Poly) -> (Poly, Poly, Poly) {
    let (mut r0, mut r1) = (a.clone(), b.clone());
    let (mut s0, mut s1) = (Poly::constant(Rat::ONE), Poly::zero());
    let (mut t0, mut t1) = (Poly::zero(), Poly::constant(Rat::ONE));
    while !r1.is_zero() {
        let (q, r) = r0.divrem(&r1);
        r0 = std::mem::replace(&mut r1, r);
        let s = s0.sub(&q.mul(&s1));
        s0 = std::mem::replace(&mut s1, s);
        let t = t0.sub(&q.mul(&t1));
        t0 = std::mem::replace(&mut t1, t);
    }
    let lc = r0.lc();
    let inv = Rat::ONE / lc;
    (r0.scale(&inv), s0.scale(&inv), t0.scale(&inv))
}

/// 無平方分解（Yun）: p = c ∏ g_k^k（g_k はモニックで互いに素）
pub fn squarefree(p: &Poly) -> Vec<(Poly, u32)> {
    let mut out = Vec::new();
    if p.deg() < 1 {
        return out;
    }
    let f = p.monic();
    let d = f.deriv();
    let a0 = gcd(&f, &d);
    let mut b = f.divrem(&a0).0;
    let mut c = d.divrem(&a0).0;
    let mut dd = c.sub(&b.deriv());
    let mut k = 1;
    while b.deg() >= 1 {
        let a = gcd(&b, &dd);
        if a.deg() >= 1 {
            out.push((a.clone(), k));
        }
        b = b.divrem(&a).0;
        c = dd.divrem(&a).0;
        dd = c.sub(&b.deriv());
        k += 1;
        if k > 1000 {
            break;
        }
    }
    out
}

fn divisors(n: &Int, budget: &Budget) -> Result<Vec<Int>, TimeUp> {
    let n = n.clone().abs();
    if n.is_zero() {
        return Ok(Vec::new());
    }
    let (fs, rest) = num::factor_int(&n, budget)?;
    if rest.is_some() {
        return Err(TimeUp);
    }
    let mut ds = vec![Int::ONE];
    for (p, e) in fs {
        let mut next = Vec::new();
        for d in &ds {
            let mut m = d.clone();
            for _ in 0..=e {
                next.push(m.clone());
                m *= &p;
            }
        }
        ds = next;
        if ds.len() > 4096 {
            return Err(TimeUp);
        }
    }
    Ok(ds)
}

/// 有理数の根（重なりは 1 つにする）
pub fn rational_roots(p: &Poly, budget: &Budget) -> Result<Vec<Rat>, TimeUp> {
    let (_, f) = p.primitive();
    if f.deg() < 1 {
        return Ok(Vec::new());
    }
    let mut roots = Vec::new();
    // 0 の根
    let mut g = f.clone();
    if g.0[0].is_zero() {
        roots.push(Rat::ZERO);
        while g.0.first().is_some_and(|c| c.is_zero()) {
            g.0.remove(0);
        }
    }
    if g.deg() < 1 {
        return Ok(roots);
    }
    let a0 = num::as_int(&g.0[0]).unwrap();
    let an = num::as_int(&g.lc()).unwrap();
    let ps = divisors(&a0, budget)?;
    let qs = divisors(&an, budget)?;
    for pp in &ps {
        for qq in &qs {
            budget.charge(1)?;
            for s in [1i64, -1] {
                let r = Rat::from_parts_signed(pp.clone() * Int::from(s), qq.clone());
                if !roots.contains(&r) && g.eval(&r).is_zero() {
                    roots.push(r);
                }
            }
        }
    }
    roots.sort();
    Ok(roots)
}

/// ラグランジュの補間（点 xs で値 ys の、次数 xs.len() − 1 以下の多項式）
fn interpolate(xs: &[Rat], ys: &[Rat]) -> Poly {
    let mut acc = Poly::zero();
    for i in 0..xs.len() {
        let mut term = Poly::constant(ys[i].clone());
        for j in 0..xs.len() {
            if i != j {
                let lin = Poly::new(vec![-xs[j].clone(), Rat::ONE]);
                term = term.mul(&lin).scale(&(Rat::ONE / (xs[i].clone() - &xs[j])));
            }
        }
        acc = acc.add(&term);
    }
    acc
}

/// 整数の係数で、`f` を割り切る d 次の因数を Kronecker の方法で探す
fn kronecker(f: &Poly, d: usize, budget: &Budget) -> Result<Option<Poly>, TimeUp> {
    // 値の約数の少ない点を選ぶ
    let mut cands: Vec<(usize, Rat, Rat)> = Vec::new();
    for k in -8i64..=8 {
        let x = ri(k);
        let v = f.eval(&x);
        if v.is_zero() {
            continue;
        }
        let n = divisors(&num::as_int(&v).unwrap(), budget)?.len();
        cands.push((n, x, v));
    }
    cands.sort_by(|a, b| a.0.cmp(&b.0));
    if cands.len() < d + 1 {
        return Ok(None);
    }
    let pts: Vec<(Rat, Vec<Int>)> = cands[..d + 1].iter().map(|(_, x, v)| (x.clone(), divisors(&num::as_int(v).unwrap(), budget).unwrap_or_default())).collect();
    let xs: Vec<Rat> = pts.iter().map(|p| p.0.clone()).collect();
    // 組み合わせを数え上げる（最初の点の値は正だけ。−g も同じ因数なので）
    let sizes: Vec<usize> = pts.iter().enumerate().map(|(i, p)| p.1.len() * if i == 0 { 1 } else { 2 }).collect();
    let mut idx = vec![0usize; d + 1];
    loop {
        budget.charge(1)?;
        let ys: Vec<Rat> = (0..=d)
            .map(|i| {
                let k = idx[i];
                let (dv, sg) = if i == 0 { (k, 1) } else { (k / 2, if k % 2 == 0 { 1 } else { -1 }) };
                Rat::from(pts[i].1[dv].clone() * Int::from(sg))
            })
            .collect();
        let g = interpolate(&xs, &ys);
        if g.deg() == d as i64 && g.0.iter().all(|c| *c.denominator() == Nat::ONE) {
            let (_, r) = f.divrem(&g);
            if r.is_zero() {
                return Ok(Some(g));
            }
        }
        // 次の組
        let mut k = 0;
        loop {
            idx[k] += 1;
            if idx[k] < sizes[k] {
                break;
            }
            idx[k] = 0;
            k += 1;
            if k > d {
                return Ok(None);
            }
        }
    }
}

/// 因数分解の結果: 定数倍・(既約な原始多項式, 重なり)・分解しきれたか
#[derive(Debug, Clone)]
pub struct Factored {
    pub unit: Rat,
    pub factors: Vec<(Poly, u32)>,
    pub complete: bool,
}

/// 有理数の上の因数分解
pub fn factor(p: &Poly, budget: &Budget) -> Factored {
    let (unit, prim) = p.primitive();
    let mut factors: Vec<(Poly, u32)> = Vec::new();
    let mut complete = true;
    for (g, k) in squarefree(&prim) {
        let (_, g) = g.primitive();
        let mut stack = vec![g];
        while let Some(f) = stack.pop() {
            if f.deg() <= 1 {
                if f.deg() == 1 {
                    factors.push((f, k));
                }
                continue;
            }
            // 1 次の因数（有理数の根）
            match rational_roots(&f, budget) {
                Ok(rs) if !rs.is_empty() => {
                    let mut rest = f.clone();
                    for r in rs {
                        let lin = Poly::new(vec![-r.clone(), Rat::ONE]).primitive().1;
                        rest = rest.divrem(&lin).0;
                        factors.push((lin, k));
                    }
                    let (_, rest) = rest.primitive();
                    if rest.deg() >= 1 {
                        stack.push(rest);
                    }
                    continue;
                }
                Ok(_) => {}
                Err(_) => {
                    complete = false;
                    factors.push((f, k));
                    continue;
                }
            }
            // 2 次以上の因数を、低い次数から探す
            let mut split = None;
            let mut gave_up = false;
            for d in 2..=(f.deg() as usize / 2) {
                match kronecker(&f, d, budget) {
                    Ok(Some(g)) => {
                        split = Some(g);
                        break;
                    }
                    Ok(None) => {}
                    Err(_) => {
                        gave_up = true;
                        break;
                    }
                }
            }
            match split {
                Some(g) => {
                    let (_, g) = g.primitive();
                    let (h, _) = f.divrem(&g);
                    stack.push(g);
                    stack.push(h.primitive().1);
                }
                None => {
                    if gave_up {
                        complete = false;
                    }
                    factors.push((f, k));
                }
            }
        }
    }
    // 定数倍を合わせる: p = unit × ∏ f^k（f は原始）。原始にした分の比を unit に入れる
    let mut back = Poly::constant(Rat::ONE);
    for (f, k) in &factors {
        back = back.mul(&f.pow(*k));
    }
    let unit = if back.is_zero() { unit } else { p.lc() / back.lc() };
    factors.sort_by(|a, b| a.0.deg().cmp(&b.0.deg()).then(a.0.cmp(&b.0)));
    Factored { unit, factors, complete }
}

/// 部分分数（有理数の上）: num/den = 多項式の部分 + Σ a_{ij} / f_i^j（a は f_i より低い次数）
pub fn apart(num: &Poly, den: &Poly, budget: &Budget) -> Option<(Poly, Vec<(Poly, Poly, u32)>)> {
    let fac = factor(den, budget);
    if !fac.complete {
        return None;
    }
    let (q, mut r) = num.divrem(den);
    r = r.scale(&(Rat::ONE / fac.unit.clone()));
    // den / unit = ∏ F_i（F_i = f_i^{k_i}）
    let parts: Vec<Poly> = fac.factors.iter().map(|(f, k)| f.pow(*k)).collect();
    let mut out = Vec::new();
    let mut rest_den = Poly::constant(Rat::ONE);
    for p in &parts {
        rest_den = rest_den.mul(p);
    }
    for (i, (f, k)) in fac.factors.iter().enumerate() {
        let fi = &parts[i];
        let g = rest_den.divrem(fi).0;
        // r / (fi g) = A/fi + B/g、A g + B fi = r
        let (one, s, t) = ext_gcd(fi, &g);
        if one.deg() != 0 {
            return None;
        }
        let a = t.mul(&r).divrem(fi).1;
        let b = r.sub(&a.mul(&g)).divrem(fi).0;
        // A / f^k を Σ a_j / f^j に（f で割っていく）
        let mut cur = a;
        for j in (1..=*k).rev() {
            let (qq, rr) = cur.divrem(f);
            if !rr.is_zero() {
                out.push((rr, f.clone(), j));
            }
            cur = qq;
        }
        let _ = s;
        r = b;
        rest_den = g;
    }
    out.reverse();
    Some((q, out))
}

/// Term を x の多項式に（係数は有理数）。多項式でなければ None
pub fn to_poly(t: &Term, x: &str) -> Option<Poly> {
    Some(match t {
        Term::Num(q) => Poly::constant(q.clone()),
        Term::Sym(s) if s == x => Poly::x(),
        Term::Add(v) => {
            let mut acc = Poly::zero();
            for a in v {
                acc = acc.add(&to_poly(a, x)?);
            }
            acc
        }
        Term::Mul(v) => {
            let mut acc = Poly::constant(Rat::ONE);
            for a in v {
                acc = acc.mul(&to_poly(a, x)?);
            }
            acc
        }
        Term::Pow(b, e) => {
            let k = e.as_rat().and_then(num::as_int).and_then(|k| u32::try_from(&k).ok())?;
            if k > 200 {
                return None;
            }
            to_poly(b, x)?.pow(k)
        }
        _ => return None,
    })
}

/// Term を x の有理関数 (分子, 分母) に。なれなければ None
pub fn to_rational(t: &Term, x: &str) -> Option<(Poly, Poly)> {
    Some(match t {
        Term::Num(_) | Term::Sym(_) => (to_poly(t, x)?, Poly::constant(Rat::ONE)),
        Term::Add(v) => {
            let (mut n, mut d) = (Poly::zero(), Poly::constant(Rat::ONE));
            for a in v {
                let (an, ad) = to_rational(a, x)?;
                n = n.mul(&ad).add(&an.mul(&d));
                d = d.mul(&ad);
            }
            (n, d)
        }
        Term::Mul(v) => {
            let (mut n, mut d) = (Poly::constant(Rat::ONE), Poly::constant(Rat::ONE));
            for a in v {
                let (an, ad) = to_rational(a, x)?;
                n = n.mul(&an);
                d = d.mul(&ad);
            }
            (n, d)
        }
        Term::Pow(b, e) => {
            let k = e.as_rat().and_then(num::as_int).and_then(|k| i64::try_from(&k).ok())?;
            if k.abs() > 200 {
                return None;
            }
            let (bn, bd) = to_rational(b, x)?;
            if k >= 0 {
                (bn.pow(k as u32), bd.pow(k as u32))
            } else {
                (bd.pow((-k) as u32), bn.pow((-k) as u32))
            }
        }
        _ => return None,
    })
}

/// 多項式を Term に
pub fn from_poly(p: &Poly, x: &str) -> Term {
    add(p.0.iter().enumerate().filter(|(_, c)| !c.is_zero()).map(|(k, c)| mul(vec![Term::Num(c.clone()), pow(Term::Sym(x.into()), Term::int(k as i64))])).collect())
}

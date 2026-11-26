//! 方程式の厳密な解（計画 `docs/plan-exact-areas.md` の SC1）。
//!
//! 1 変数の方程式 $f(x) = g(x)$ の実数の解を、$h = f - g = 0$ にして求める:
//! 1. 積は因子ごとに解く（分母の因子の 0 は除く）
//! 2. 有理関数は分子の多項式を因数分解し、1 次・2 次は公式、既約な 3 次は三角関数の形（実根が 3 つ）かカルダノの形、
//!    既約な 4 次は複 2 次式か、分解方程式（3 次）に正の有理数の根があるときのフェラーリの方法。5 次以上の既約因数は見つからない
//! 3. それ以外は、$x$ を含む「核」（根号・$a^{e(x)}$・$\ln$・三角関数）が 1 種類だけになるように置き換え（$4^x = (2^x)^2$、
//!    $\cos^2 = 1 - \sin^2$、$\ln a + \ln b = \ln ab$）、核 $= u$ の有理方程式を解いてから核を逆に解く。
//!    三角関数は一般解（中身が $x$ の 1 次式のときだけ）
//! 4. 候補はどれも、元の式に入れて 0 になることを確かめる（2 乗したときの余分な解・定義域の外を捨てる）
//!
//! 解は実数だけ（複素数の解は出さない。0 章）

use super::num::{self, Int, Nat, Rat};
use super::poly::{self, Poly};
use super::term::{self, add, call, mul, neg, pow, Atom, Term};
use super::{algebraic, ball, values, Budget, Stop};
use crate::functions::Func;

/// 1 変数の方程式の解
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Solutions {
    /// 有限個の解（小さい順）
    pub points: Vec<Term>,
    /// 一般解の族: $x = b + p n$（$n$ は整数。$0 \le b < p$）
    pub families: Vec<(Term, Term)>,
    /// どの $x$ でも成り立つ（恒等式）
    pub all: bool,
}

impl Solutions {
    fn none() -> Solutions {
        Solutions::default()
    }
    fn points(v: Vec<Term>) -> Solutions {
        Solutions { points: v, ..Solutions::default() }
    }
    pub fn is_empty(&self) -> bool {
        self.points.is_empty() && self.families.is_empty() && !self.all
    }
    fn merge(&mut self, o: Solutions) {
        self.points.extend(o.points);
        self.families.extend(o.families);
        self.all |= o.all;
    }
}

fn nf(s: &str) -> Stop {
    Stop::NotFound(s.into())
}

/// 文字 x を含むか
pub fn contains(t: &Term, x: &str) -> bool {
    match t {
        Term::Sym(s) => s == x,
        Term::Num(_) | Term::Atom(_) => false,
        Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) | Term::Matrix(_, _, v) => v.iter().any(|a| contains(a, x)),
        Term::Pow(b, e) => contains(b, x) || contains(e, x),
    }
}

/// 部分式 `pat` を `with` に置き換えて作り直す
fn replace(t: &Term, pat: &Term, with: &Term) -> Term {
    if t == pat {
        return with.clone();
    }
    match t {
        Term::Add(v) => add(v.iter().map(|a| replace(a, pat, with)).collect()),
        Term::Mul(v) => mul(v.iter().map(|a| replace(a, pat, with)).collect()),
        Term::Pow(b, e) => pow(replace(b, pat, with), replace(e, pat, with)),
        Term::Fn(f, v) => call(*f, v.iter().map(|a| replace(a, pat, with)).collect()),
        _ => t.clone(),
    }
}

fn half() -> Rat {
    Rat::from_parts(Int::ONE, Nat::from(2u8))
}

fn sqrt(t: Term) -> Term {
    let r = pow(t, Term::rat(1, 2));
    algebraic::denest_sqrt(&r).unwrap_or(r)
}

/// 実数の値（f64。並べる・比べるため）
fn approx(t: &Term) -> Option<f64> {
    let b = ball::value(t, 64).ok()?;
    let (re, im) = b.to_f64();
    (im.abs() <= 1e-12 * (1.0 + re.abs())).then_some(re)
}

/// 符号（決まらなければ None）
fn sign(t: &Term) -> Option<i32> {
    if t.is_zero() {
        return Some(0);
    }
    values::real_sign(t)
}

/// 実数の 3 乗根（負の数は −∛|w|）
fn real_cbrt(w: Term) -> Option<Term> {
    Some(match sign(&w)? {
        0 => Term::zero(),
        s if s > 0 => pow(w, Term::rat(1, 3)),
        _ => neg(pow(neg(w), Term::rat(1, 3))),
    })
}

// ---------------------------------------------------------------- 多項式の実根

/// 2 次式 $a x^2 + b x + c$（係数は数の式）の実根
fn quadratic_roots(a: &Term, b: &Term, c: &Term) -> Option<Vec<Term>> {
    let d = add(vec![mul(vec![b.clone(), b.clone()]), mul(vec![Term::int(-4), a.clone(), c.clone()])]);
    let two_a = mul(vec![Term::int(2), a.clone()]);
    Some(match sign(&d)? {
        s if s < 0 => Vec::new(),
        0 => vec![term::div(neg(b.clone()), two_a)],
        _ => {
            let r = sqrt(d);
            vec![term::div(add(vec![neg(b.clone()), neg(r.clone())]), two_a.clone()), term::div(add(vec![neg(b.clone()), r]), two_a)]
        }
    })
}

/// cos(aπ) を、a を [0, 1/2] に寄せて（cos(−θ) = cos θ、cos(π − θ) = −cos θ）
fn cos_folded(ang: Term) -> Term {
    let Some(a) = values::as_pi_multiple(&ang) else { return call(Func::Cos, vec![ang]) };
    let two = Rat::from(Int::from(2));
    let mut a = a.clone() - Rat::from((a.clone() / &two).floor()) * &two;
    if a > Rat::ONE {
        a = two - a;
    }
    let (a, s) = if a > half() { (Rat::ONE - a, -1) } else { (a, 1) };
    let c = call(Func::Cos, vec![mul(vec![Term::Num(a), Term::Atom(Atom::Pi)])]);
    if s < 0 { neg(c) } else { c }
}

/// 既約な 3 次式（モニック。係数は低い次数から $c, b, a$）の実根
fn cubic_roots(c: &[Rat]) -> Option<Vec<Term>> {
    let (c0, c1, c2) = (&c[0], &c[1], &c[2]);
    let three = Rat::from(Int::from(3));
    // x = t − a/3: t³ + p t + q
    let shift = c2.clone() / &three;
    let p = c1.clone() - c2.clone() * c2 / &three;
    let q = Rat::from(Int::from(2)) * c2 * c2 * c2 / Rat::from(Int::from(27)) - c2.clone() * c1 / &three + c0;
    let disc = -(Rat::from(Int::from(4)) * &p * &p * &p + Rat::from(Int::from(27)) * &q * &q);
    let shift_t = Term::Num(-shift);
    let ts: Vec<Term> = if num::sign(&disc) > 0 {
        // 実根が 3 つ: t_k = 2√(−p/3) cos(θ/3 − 2πk/3)、cos θ = (3q / 2p) √(−3/p)
        let m = mul(vec![Term::int(2), sqrt(Term::Num(-p.clone() / &three))]);
        let w = mul(vec![Term::Num(three.clone() * &q / (Rat::from(Int::from(2)) * &p)), sqrt(Term::Num(-three.clone() / &p))]);
        let theta = mul(vec![Term::rat(1, 3), call(Func::Acos, vec![w])]);
        (0..3)
            .map(|k| {
                let ang = add(vec![theta.clone(), mul(vec![Term::rat(-2 * k, 3), Term::Atom(Atom::Pi)])]);
                mul(vec![m.clone(), cos_folded(ang)])
            })
            .collect()
    } else {
        // 実根が 1 つ: カルダノ。D = q²/4 + p³/27 > 0
        let d = q.clone() * &q / Rat::from(Int::from(4)) + p.clone() * &p * &p / Rat::from(Int::from(27));
        let s = sqrt(Term::Num(d));
        let hq = Term::Num(-q.clone() * half());
        let u = real_cbrt(add(vec![hq.clone(), s.clone()]))?;
        let v = real_cbrt(add(vec![hq, neg(s)]))?;
        vec![add(vec![u, v])]
    };
    Some(ts.into_iter().map(|t| add(vec![t, shift_t.clone()])).collect())
}

/// 既約な 4 次式（モニック。係数は低い次数から）の実根。複 2 次式か、分解方程式に正の有理数の根があるときだけ
fn quartic_roots(c: &[Rat], budget: &Budget) -> Result<Vec<Term>, Stop> {
    let a = &c[3];
    let four = Rat::from(Int::from(4));
    // x = y − a/4: y⁴ + p y² + q y + r
    let s = a.clone() / &four;
    let sh = Poly::new(vec![-s.clone(), Rat::ONE]);
    let f = Poly::new(c.iter().cloned().chain([Rat::ONE]).collect());
    let mut g = Poly::zero();
    for (k, ck) in f.0.iter().enumerate().rev() {
        g = g.mul(&sh).add(&Poly::constant(ck.clone()));
        let _ = k;
    }
    let get = |k: usize| g.0.get(k).cloned().unwrap_or(Rat::ZERO);
    let (r, q, p) = (get(0), get(1), get(2));
    let shift = Term::Num(-s);
    let mut ys: Vec<Term> = Vec::new();
    if q.is_zero() {
        // 複 2 次式: z² + p z + r、y = ±√z
        for z in quadratic_roots(&Term::one(), &Term::Num(p), &Term::Num(r)).ok_or_else(|| nf("符号が決まりません"))? {
            if sign(&z) == Some(1) {
                let w = sqrt(z);
                ys.push(neg(w.clone()));
                ys.push(w);
            }
        }
    } else {
        // 分解方程式 8m³ + 8p m² + (2p² − 8r) m − q² = 0 の正の有理数の根
        let res = Poly::new(vec![-q.clone() * &q, Rat::from(Int::from(2)) * &p * &p - Rat::from(Int::from(8)) * &r, Rat::from(Int::from(8)) * &p, Rat::from(Int::from(8))]);
        let roots = poly::rational_roots(&res, budget).map_err(|_| Stop::TimeUp)?;
        let m = roots.into_iter().find(|m| num::sign(m) > 0).ok_or_else(|| nf("4 次の因数を根号で表せません（分解方程式に有理数の根がありません）"))?;
        let sq = sqrt(Term::Num(Rat::from(Int::from(2)) * &m));
        let base = Term::Num(p * half() + &m);
        let qq = term::div(Term::Num(q), mul(vec![Term::int(2), sq.clone()]));
        // (y² − s y + base + q/(2s)) (y² + s y + base − q/(2s))
        for (b, cc) in [(neg(sq.clone()), add(vec![base.clone(), qq.clone()])), (sq.clone(), add(vec![base, neg(qq)]))] {
            ys.extend(quadratic_roots(&Term::one(), &b, &cc).ok_or_else(|| nf("符号が決まりません"))?);
        }
    }
    Ok(ys.into_iter().map(|y| add(vec![y, shift.clone()])).collect())
}

/// 有理数係数の多項式の相異なる実根（小さい順）
pub fn real_poly_roots(p: &Poly, budget: &Budget) -> Result<Vec<Term>, Stop> {
    if p.is_zero() {
        return Err(nf("0 の多項式"));
    }
    let fac = poly::factor(p, budget);
    if !fac.complete {
        return Err(Stop::TimeUp);
    }
    let mut out = Vec::new();
    for (f, _) in &fac.factors {
        let f = f.monic();
        let c = &f.0;
        // 3 次より上は、実根が無ければ公式を使わない（x⁴ + x + 1 など）
        if f.deg() >= 4 && super::roots::isolate(&f, budget).map_err(|_| Stop::TimeUp)?.is_empty() {
            continue;
        }
        match f.deg() {
            1 => out.push(Term::Num(-c[0].clone())),
            2 => out.extend(quadratic_roots(&Term::one(), &Term::Num(c[1].clone()), &Term::Num(c[0].clone())).ok_or_else(|| nf("符号が決まりません"))?),
            3 => out.extend(cubic_roots(c).ok_or_else(|| nf("符号が決まりません"))?),
            4 => out.extend(quartic_roots(c, budget)?),
            _ => return Err(nf("5 次以上の既約な因数の根は、まだ根号などで表しません")),
        }
    }
    sort_points(&mut out);
    Ok(out)
}

// ---------------------------------------------------------------- 複素数の根（固有値。文書 8 の LA1）

/// 2 次式 $y^2 + b y + c$（係数は実数の数の式）の 2 つの根。判別式が負なら共役な複素数の組（虚部が負の方が先）
fn quadratic_all(b: &Term, c: &Term) -> Option<[Term; 2]> {
    let d = add(vec![mul(vec![b.clone(), b.clone()]), mul(vec![Term::int(-4), c.clone()])]);
    let hb = mul(vec![Term::rat(-1, 2), b.clone()]);
    let r = match sign(&d)? {
        s if s < 0 => mul(vec![Term::Atom(Atom::I), sqrt(neg(d))]),
        _ => sqrt(d),
    };
    let hr = mul(vec![Term::rat(1, 2), r]);
    Some([add(vec![hb.clone(), neg(hr.clone())]), add(vec![hb, hr])])
}

/// 既約な 3 次式（モニック）の 3 つの根。実根が 1 つなら、カルダノの形の実根と共役な複素数の組
fn cubic_all(c: &[Rat]) -> Option<Vec<Term>> {
    let (c0, c1, c2) = (&c[0], &c[1], &c[2]);
    let three = Rat::from(Int::from(3));
    let p = c1.clone() - c2.clone() * c2 / &three;
    let q = Rat::from(Int::from(2)) * c2 * c2 * c2 / Rat::from(Int::from(27)) - c2.clone() * c1 / &three + c0;
    let disc = -(Rat::from(Int::from(4)) * &p * &p * &p + Rat::from(Int::from(27)) * &q * &q);
    if num::sign(&disc) > 0 {
        return cubic_roots(c);
    }
    // t³ + p t + q の実根 u + v と、ω u + ω² v = −(u + v)/2 ± (√3/2)(u − v) i
    let d = q.clone() * &q / Rat::from(Int::from(4)) + p.clone() * &p * &p / Rat::from(Int::from(27));
    let s = sqrt(Term::Num(d));
    let hq = Term::Num(-q.clone() * half());
    let u = real_cbrt(add(vec![hq.clone(), s.clone()]))?;
    let v = real_cbrt(add(vec![hq, neg(s)]))?;
    let shift = Term::Num(-c2.clone() / &three);
    let re = add(vec![mul(vec![Term::rat(-1, 2), add(vec![u.clone(), v.clone()])]), shift.clone()]);
    let im = mul(vec![Term::rat(1, 2), sqrt(Term::int(3)), add(vec![u.clone(), neg(v.clone())]), Term::Atom(Atom::I)]);
    let real = add(vec![u, v, shift]);
    let (lo, hi) = (add(vec![re.clone(), neg(im.clone())]), add(vec![re, im]));
    // 虚部が負の方を先に
    let lo_first = ball::value(&lo, 64).map(|b| b.to_f64().1 < 0.0).unwrap_or(true);
    Some(if lo_first { vec![real, lo, hi] } else { vec![real, hi, lo] })
}

/// 既約な 4 次式（モニック）の 4 つの根。複 2 次式で $z$ が実数のときと、分解方程式に正の有理数の根があるとき（フェラーリ）だけ
fn quartic_all(c: &[Rat], budget: &Budget) -> Result<Option<Vec<Term>>, Stop> {
    let four = Rat::from(Int::from(4));
    let s = c[3].clone() / &four;
    let sh = Poly::new(vec![-s.clone(), Rat::ONE]);
    let f = Poly::new(c.iter().cloned().chain([Rat::ONE]).collect());
    let mut g = Poly::zero();
    for ck in f.0.iter().rev() {
        g = g.mul(&sh).add(&Poly::constant(ck.clone()));
    }
    let get = |k: usize| g.0.get(k).cloned().unwrap_or(Rat::ZERO);
    let (r, q, p) = (get(0), get(1), get(2));
    let shift = Term::Num(-s);
    let mut ys: Vec<Term> = Vec::new();
    if q.is_zero() {
        // y⁴ + p y² + r: z = y² の 2 次式。z が実数のときだけ（z < 0 なら y = ±√(−z) i）
        let dz = p.clone() * &p - four.clone() * &r;
        if num::sign(&dz) < 0 {
            return Ok(None);
        }
        let Some(zs) = quadratic_all(&Term::Num(p), &Term::Num(r)) else { return Ok(None) };
        for z in zs {
            let w = match sign(&z) {
                Some(s) if s < 0 => mul(vec![Term::Atom(Atom::I), sqrt(neg(z))]),
                Some(_) => sqrt(z),
                None => return Ok(None),
            };
            ys.push(neg(w.clone()));
            ys.push(w);
        }
    } else {
        let res = Poly::new(vec![-q.clone() * &q, Rat::from(Int::from(2)) * &p * &p - Rat::from(Int::from(8)) * &r, Rat::from(Int::from(8)) * &p, Rat::from(Int::from(8))]);
        let roots = poly::rational_roots(&res, budget).map_err(|_| Stop::TimeUp)?;
        let Some(m) = roots.into_iter().find(|m| num::sign(m) > 0) else { return Ok(None) };
        let sq = sqrt(Term::Num(Rat::from(Int::from(2)) * &m));
        let base = Term::Num(p * half() + &m);
        let qq = term::div(Term::Num(q), mul(vec![Term::int(2), sq.clone()]));
        for (b, cc) in [(neg(sq.clone()), add(vec![base.clone(), qq.clone()])), (sq.clone(), add(vec![base, neg(qq)]))] {
            let Some(two) = quadratic_all(&b, &cc) else { return Ok(None) };
            ys.extend(two);
        }
    }
    Ok(Some(ys.into_iter().map(|y| add(vec![y, shift.clone()])).collect()))
}

/// 既約な多項式の複素数の根の全部（4 次まで。根号で表せなければ None）。並びは、実数（小さい順）→ 実数でないもの（実部、虚部の順）
pub fn irreducible_roots(f: &Poly, budget: &Budget) -> Result<Option<Vec<Term>>, Stop> {
    let f = f.monic();
    let c = &f.0;
    let mut out = match f.deg() {
        1 => vec![Term::Num(-c[0].clone())],
        2 => match quadratic_all(&Term::Num(c[1].clone()), &Term::Num(c[0].clone())) {
            Some(v) => v.to_vec(),
            None => return Ok(None),
        },
        3 => match cubic_all(c) {
            Some(v) => v,
            None => return Ok(None),
        },
        4 => match quartic_all(c, budget)? {
            Some(v) => v,
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    let key = |t: &Term| ball::value(t, 64).map(|b| b.to_f64()).unwrap_or((f64::NAN, f64::NAN));
    out.sort_by(|a, b| {
        let (ka, kb) = (key(a), key(b));
        let real = |k: (f64, f64)| k.1.abs() <= 1e-12 * (1.0 + k.0.abs());
        real(kb).cmp(&real(ka)).then(ka.0.total_cmp(&kb.0)).then(ka.1.total_cmp(&kb.1))
    });
    Ok(Some(out))
}

fn sort_points(v: &mut Vec<Term>) {
    v.sort_by(|a, b| approx(a).unwrap_or(f64::NAN).total_cmp(&approx(b).unwrap_or(f64::NAN)));
    v.dedup_by(|a, b| match (approx(a), approx(b)) {
        (Some(x), Some(y)) => (x - y).abs() <= 1e-12 * (1.0 + x.abs()),
        _ => a == b,
    });
}

// ---------------------------------------------------------------- 核の置き換え

/// x を含む、多項式でない部分（核）を集める
fn kernels(t: &Term, x: &str, out: &mut Vec<Term>) {
    if !contains(t, x) {
        return;
    }
    match t {
        Term::Add(v) | Term::Mul(v) => v.iter().for_each(|a| kernels(a, x, out)),
        Term::Pow(b, e) if !contains(e, x) && e.as_rat().is_some_and(|q| *q.denominator() == Nat::ONE) => kernels(b, x, out),
        Term::Sym(_) => {}
        _ => {
            if !out.contains(t) {
                out.push(t.clone());
            }
        }
    }
}

/// 中身が x の 1 次式 a x + b なら (a, b)
fn linear(t: &Term, x: &str) -> Option<(Rat, Term)> {
    // 係数が数の式（π を含む）でもよいように、x に 0 と 1 を入れて調べる
    let b = super::diff::subst(t, x, &Term::zero());
    let a1 = super::diff::subst(t, x, &Term::one());
    let a = add(vec![a1, neg(b.clone())]);
    let a = a.as_rat()?.clone();
    if a.is_zero() {
        return None;
    }
    // 本当に 1 次か: a x + b に戻る
    let back = add(vec![mul(vec![Term::Num(a.clone()), Term::Sym(x.into())]), b.clone()]);
    (add(vec![t.clone(), neg(back)]).is_zero()).then_some((a, b))
}

/// $a^{e}$（a は正の数か e）の (底, 指数)
fn exponential(t: &Term, x: &str) -> Option<(Term, Term)> {
    match t {
        Term::Pow(b, e) if contains(e, x) && !contains(b, x) && (b.as_rat().is_some_and(|q| num::sign(q) > 0) || **b == Term::Atom(Atom::E)) => Some(((**b).clone(), (**e).clone())),
        _ => None,
    }
}

/// 正の有理数 a を、いちばん小さい底 r の整数乗 r^m にする
fn smallest_base(a: &Rat) -> (Rat, i64) {
    for m in (2..=40).rev() {
        if let (Some(n), Some(d)) = (num::exact_root(a.numerator(), m), num::exact_root(&Int::from(a.denominator().clone()), m)) {
            return (Rat::from_parts_signed(n, d), m as i64);
        }
    }
    (a.clone(), 1)
}

const U: &str = "\u{1}u";

/// 方程式 h = 0 の実数の解（h は x の式）。深さ `depth` は置き換えの入れ子の数
pub fn solve_zero(h: &Term, x: &str, budget: &Budget, depth: u32) -> Result<Solutions, Stop> {
    if depth > 6 {
        return Err(nf("置き換えが深すぎます"));
    }
    budget.charge(h.size() as u64 + 1).map_err(|_| Stop::TimeUp)?;
    if h.is_zero() {
        return Ok(Solutions { all: true, ..Solutions::default() });
    }
    if !contains(h, x) {
        return Ok(Solutions::none());
    }
    // 有理関数: 分子の根で、分母の 0 でないもの
    if let Some((n, d)) = poly::to_rational(h, x) {
        let (n, d) = {
            let g = poly::gcd(&n, &d);
            (n.divrem(&g).0, d.divrem(&g).0)
        };
        if n.is_zero() {
            return Ok(Solutions { all: true, ..Solutions::default() });
        }
        let roots = real_poly_roots(&n, budget)?;
        return Ok(Solutions::points(roots.into_iter().filter(|r| d.deg() < 1 || !super::diff::subst(&poly::from_poly(&d, x), x, r).is_zero()).collect()));
    }
    // 積: 因子ごとに（分母の因子は除く所）
    if let Term::Mul(v) = h {
        let mut out = Solutions::none();
        let mut any = false;
        for f in v {
            if !contains(f, x) {
                continue;
            }
            if let Term::Pow(_, e) = f {
                if e.as_rat().is_some_and(|q| num::sign(q) < 0) {
                    continue;
                }
            }
            any = true;
            out.merge(solve_zero(f, x, budget, depth + 1)?);
        }
        if any {
            return Ok(out);
        }
    }
    let mut ks = Vec::new();
    kernels(h, x, &mut ks);
    if ks.is_empty() {
        // x の多項式で、係数に定数（ln 5 など）を含む: 1 次と 2 次だけ
        let cs = poly_in(h, x).ok_or_else(|| nf("多項式にできません"))?;
        let mut cs = cs;
        while cs.last().is_some_and(Term::is_zero) {
            cs.pop();
        }
        return Ok(Solutions::points(match cs.len() {
            0 => return Ok(Solutions { all: true, ..Solutions::default() }),
            1 => Vec::new(),
            2 => vec![term::div(neg(cs[0].clone()), cs[1].clone())],
            3 => quadratic_roots(&cs[2], &cs[1], &cs[0]).ok_or_else(|| nf("符号が決まりません"))?,
            _ => return Err(nf("係数に定数を含む 3 次以上の式は、まだ解けません")),
        }));
    }
    if ks.len() == 1 {
        match solve_kernel(h, &ks[0], x, budget, depth) {
            Err(Stop::NotFound(_)) if radical(&ks[0], x).is_some() || matches!(&ks[0], Term::Fn(Func::Abs, _)) => {}
            r => return r,
        }
    }
    // 絶対値を 1 つ孤立させる: A + B|g| = 0 → B g = −A か B g = A（余分な解は最後に元の式で捨てる）
    if let Some(k) = ks.iter().find(|k| matches!(k, Term::Fn(Func::Abs, a) if a.len() == 1)) {
        let Term::Fn(_, a) = k else { unreachable!() };
        let hu = replace(h, k, &Term::Sym(U.into()));
        if let Some(cs) = poly_in(&hu, U).filter(|c| c.len() == 2) {
            let bg = mul(vec![cs[1].clone(), a[0].clone()]);
            let mut out = solve_zero(&add(vec![bg.clone(), cs[0].clone()]), x, budget, depth + 1)?;
            out.merge(solve_zero(&add(vec![bg, neg(cs[0].clone())]), x, budget, depth + 1)?);
            return Ok(out);
        }
    }
    // 根号を 1 つ孤立させて n 乗する（√(x + 2) = x → x + 2 = x²。余分な解は最後に元の式で捨てる）
    if let Some(k) = ks.iter().find(|k| radical(k, x).is_some()) {
        if let Some(eq) = isolate_radical(h, k, x) {
            return solve_zero(&eq, x, budget, depth + 1);
        }
    }
    // 指数: 底をそろえ、u = r^{g x} にする（4^x − 3·2^x + 2 = 0 → u² − 3u + 2 = 0）
    if let Some(s) = solve_exponentials(h, &ks, x, budget, depth)? {
        return Ok(s);
    }
    // sin と cos: 片方が偶数乗だけなら、cos² = 1 − sin² で片方にする
    if let Some(s) = solve_trig_pair(h, &ks, x, budget, depth)? {
        return Ok(s);
    }
    // 対数の和: Σ c ln g = k → ∏ g^c = e^k（c は整数）
    if let Some(s) = solve_logs(h, &ks, x, budget, depth)? {
        return Ok(s);
    }
    Err(nf("この形の方程式は、まだ解けません"))
}

/// 根号 g^{1/n}（g は x を含む）の (g, n)
fn radical(k: &Term, x: &str) -> Option<(Term, i64)> {
    let Term::Pow(g, e) = k else { return None };
    let q = e.as_rat()?;
    if !contains(g, x) || contains(e, x) || *q.numerator() != Int::ONE {
        return None;
    }
    let n = i64::try_from(&Int::from(q.denominator().clone())).ok()?;
    (n >= 2).then(|| ((**g).clone(), n))
}

/// h を根号 k = g^{1/n} の多項式 A + B k + … と見て（k^n = g で次数を下げる）、1 次なら (−A)^n − B^n g
fn isolate_radical(h: &Term, k: &Term, x: &str) -> Option<Term> {
    let (g, n) = radical(k, x)?;
    let hu = replace(h, k, &Term::Sym(U.into()));
    let cs = poly_in(&hu, U)?;
    let mut red = vec![Vec::new(); n as usize];
    for (j, c) in cs.into_iter().enumerate() {
        red[j % n as usize].push(mul(vec![c, pow(g.clone(), Term::int(j as i64 / n))]));
    }
    let red: Vec<Term> = red.into_iter().map(add).collect();
    if red[2..].iter().any(|c| !c.is_zero()) || red[1].is_zero() {
        return None;
    }
    let (a, b) = (&red[0], &red[1]);
    let eq = add(vec![pow(neg(a.clone()), Term::int(n)), neg(mul(vec![pow(b.clone(), Term::int(n)), g]))]);
    Some(eq)
}

/// 核が 1 つ: 核 = u の有理方程式を解き、核を逆に解く
fn solve_kernel(h: &Term, k: &Term, x: &str, budget: &Budget, depth: u32) -> Result<Solutions, Stop> {
    let hu = replace(h, k, &Term::Sym(U.into()));
    if contains(&hu, x) {
        return Err(nf("置き換えきれません"));
    }
    let us = match solve_zero(&hu, U, budget, depth + 1)? {
        s if s.all => return Err(nf("核の値によらず成り立ちます")),
        s if !s.families.is_empty() => return Err(nf("置き換えた式に一般解があります")),
        s => s.points,
    };
    let mut out = Solutions::none();
    for u in us {
        out.merge(invert(k, &u, x, budget, depth)?);
    }
    Ok(out)
}

/// 核 k = u（u は数の式）を x について解く
fn invert(k: &Term, u: &Term, x: &str, budget: &Budget, depth: u32) -> Result<Solutions, Stop> {
    match k {
        // 根号 g^{p/q}: g = u^{q/p}（偶数乗根なら u ≥ 0。候補は最後に元の式で確かめる）
        Term::Pow(g, e) if !contains(e, x) => {
            let q = e.as_rat().ok_or_else(|| nf("べきが数でありません"))?;
            if *q.denominator() != Nat::ONE && q.denominator() % Nat::from(2u8) == Nat::ZERO && sign(u) == Some(-1) {
                return Ok(Solutions::none());
            }
            let inv = Term::Num(Rat::ONE / q.clone());
            let target = pow(u.clone(), inv);
            solve_zero(&add(vec![(**g).clone(), neg(target)]), x, budget, depth + 1)
        }
        // a^{e(x)} = u → e(x) = ln u / ln a（u > 0）
        Term::Pow(b, e) if !contains(b, x) => {
            if sign(u) != Some(1) {
                return Ok(Solutions::none());
            }
            let l = if **b == Term::Atom(Atom::E) { call(Func::Ln, vec![u.clone()]) } else { term::div(call(Func::Ln, vec![u.clone()]), call(Func::Ln, vec![(**b).clone()])) };
            solve_zero(&add(vec![(**e).clone(), neg(l)]), x, budget, depth + 1)
        }
        // |g| = u → g = ±u（u ≥ 0）
        Term::Fn(Func::Abs, a) if a.len() == 1 => {
            let mut out = Solutions::none();
            match sign(u) {
                Some(-1) => {}
                Some(0) => out.merge(solve_zero(&a[0], x, budget, depth + 1)?),
                Some(_) => {
                    out.merge(solve_zero(&add(vec![a[0].clone(), neg(u.clone())]), x, budget, depth + 1)?);
                    out.merge(solve_zero(&add(vec![a[0].clone(), u.clone()]), x, budget, depth + 1)?);
                }
                None => return Err(nf("符号が決まりません")),
            }
            Ok(out)
        }
        Term::Fn(Func::Ln, a) if a.len() == 1 => solve_zero(&add(vec![a[0].clone(), neg(pow(Term::Atom(Atom::E), u.clone()))]), x, budget, depth + 1),
        Term::Fn(Func::Log10, a) if a.len() == 1 => solve_zero(&add(vec![a[0].clone(), neg(pow(Term::int(10), u.clone()))]), x, budget, depth + 1),
        Term::Fn(f @ (Func::Sin | Func::Cos | Func::Tan), a) if a.len() == 1 => {
            if *f != Func::Tan {
                let v = approx(u).ok_or_else(|| nf("値が決まりません"))?;
                if v.abs() > 1.0 + 1e-12 {
                    return Ok(Solutions::none());
                }
            }
            let (ca, cb) = linear(&a[0], x).ok_or_else(|| nf("三角関数の中身が x の 1 次式でありません"))?;
            let pi = Term::Atom(Atom::Pi);
            // 中身 L の一般解: (角, 周期)
            let angles: Vec<(Term, Term)> = match f {
                Func::Sin => {
                    let t = call(Func::Asin, vec![u.clone()]);
                    vec![(t.clone(), mul(vec![Term::int(2), pi.clone()])), (add(vec![pi.clone(), neg(t)]), mul(vec![Term::int(2), pi.clone()]))]
                }
                Func::Cos => {
                    let t = call(Func::Acos, vec![u.clone()]);
                    vec![(t.clone(), mul(vec![Term::int(2), pi.clone()])), (neg(t), mul(vec![Term::int(2), pi.clone()]))]
                }
                _ => vec![(call(Func::Atan, vec![u.clone()]), pi.clone())],
            };
            let mut out = Solutions::none();
            for (ang, per) in angles {
                // a x + b = ang + per n → x = (ang − b)/a + (per/|a|) n
                let base = term::div(add(vec![ang, neg(cb.clone())]), Term::Num(ca.clone()));
                let period = term::div(per, Term::Num(ca.clone().abs_rat()));
                out.families.push((base, period));
            }
            Ok(out)
        }
        _ => Err(nf("この関数の方程式は、まだ解けません")),
    }
}

trait AbsRat {
    fn abs_rat(self) -> Rat;
}
impl AbsRat for Rat {
    fn abs_rat(self) -> Rat {
        if num::sign(&self) < 0 { -self } else { self }
    }
}

fn solve_exponentials(h: &Term, ks: &[Term], x: &str, budget: &Budget, depth: u32) -> Result<Option<Solutions>, Stop> {
    // どの核も a^{c x + d}（a は正の有理数か e）で、底が同じ r の整数乗
    let mut parts = Vec::new();
    for k in ks {
        let Some((b, e)) = exponential(k, x) else { return Ok(None) };
        let Some((c, d)) = linear(&e, x) else { return Ok(None) };
        let (r, m) = match b.as_rat() {
            Some(a) if *a == Rat::ONE => return Ok(None),
            Some(a) => {
                let (r, m) = smallest_base(a);
                (Term::Num(r), m)
            }
            None => (b.clone(), 1),
        };
        parts.push((k.clone(), r, Rat::from(Int::from(m)) * c, d, b));
    }
    let r0 = parts[0].1.clone();
    if parts.iter().any(|p| p.1 != r0) {
        return Ok(None);
    }
    // u = r^{g x}、g は指数の係数の最大公約数（有理数）
    let mut g = parts[0].2.clone().abs_rat();
    for p in &parts[1..] {
        g = rat_gcd(&g, &p.2.clone().abs_rat());
    }
    let mut hu = h.clone();
    for (k, _, c, d, b) in &parts {
        let n = c.clone() / &g;
        let Some(n) = num::as_int(&n) else { return Ok(None) };
        let n = i64::try_from(&n).map_err(|_| nf("指数が大きすぎます"))?;
        // a^{c x / m ... }: k = b^{d} · u^{n}
        let with = mul(vec![pow(b.clone(), d.clone()), pow(Term::Sym(U.into()), Term::int(n))]);
        hu = replace(&hu, k, &with);
    }
    if contains(&hu, x) {
        return Ok(None);
    }
    let us = solve_zero(&hu, U, budget, depth + 1)?;
    if us.all || !us.families.is_empty() {
        return Ok(None);
    }
    let mut out = Solutions::none();
    for u in us.points {
        if sign(&u) != Some(1) {
            continue;
        }
        // r^{g x} = u → x = ln u / (g ln r)
        let l = if r0 == Term::Atom(Atom::E) { call(Func::Ln, vec![u]) } else { term::div(call(Func::Ln, vec![u]), call(Func::Ln, vec![r0.clone()])) };
        out.points.push(term::div(l, Term::Num(g.clone())));
    }
    Ok(Some(out))
}

fn rat_gcd(a: &Rat, b: &Rat) -> Rat {
    // gcd(p/q, r/s) = gcd(ps, rq) / (qs)
    let (p, q) = (a.numerator().clone(), Int::from(a.denominator().clone()));
    let (r, s) = (b.numerator().clone(), Int::from(b.denominator().clone()));
    Rat::from_parts_signed(num::gcd(&(p * &s), &(r * &q)), q * s)
}

fn solve_trig_pair(h: &Term, ks: &[Term], x: &str, budget: &Budget, depth: u32) -> Result<Option<Solutions>, Stop> {
    let [a, b] = ks else { return Ok(None) };
    let arg = |t: &Term| match t {
        Term::Fn(f @ (Func::Sin | Func::Cos), v) if v.len() == 1 => Some((*f, v[0].clone())),
        _ => None,
    };
    let (Some((fa, la)), Some((fb, lb))) = (arg(a), arg(b)) else { return Ok(None) };
    if la != lb || fa == fb {
        return Ok(None);
    }
    let (s, c) = if fa == Func::Sin { (a, b) } else { (b, a) };
    // 片方を文字にして、もう片方が偶数乗だけか
    let (su, cu) = (Term::Sym("\u{1}s".into()), Term::Sym("\u{1}c".into()));
    let hp = replace(&replace(h, s, &su), c, &cu);
    let Some(pc) = poly_in(&hp, "\u{1}c") else { return Ok(None) };
    let even = |p: &[Term]| p.iter().enumerate().all(|(k, t)| k % 2 == 0 || t.is_zero());
    let rewritten = if even(&pc) {
        // cos² = 1 − sin²
        let one_minus = add(vec![Term::one(), neg(pow(s.clone(), Term::int(2)))]);
        add(pc.iter().enumerate().map(|(k, t)| mul(vec![replace(t, &su, s), pow(one_minus.clone(), Term::int(k as i64 / 2))])).collect())
    } else {
        let Some(ps) = poly_in(&hp, "\u{1}s") else { return Ok(None) };
        if !even(&ps) {
            return Ok(None);
        }
        let one_minus = add(vec![Term::one(), neg(pow(c.clone(), Term::int(2)))]);
        add(ps.iter().enumerate().map(|(k, t)| mul(vec![replace(t, &cu, c), pow(one_minus.clone(), Term::int(k as i64 / 2))])).collect())
    };
    let mut ks2 = Vec::new();
    kernels(&rewritten, x, &mut ks2);
    if ks2.len() != 1 {
        return Ok(None);
    }
    solve_kernel(&rewritten, &ks2[0], x, budget, depth).map(Some)
}

/// t を文字 v の多項式と見た係数（低い次数から。係数は v を含まない式）。積・整数乗は展開する。多項式でなければ None
fn poly_in(t: &Term, v: &str) -> Option<Vec<Term>> {
    fn conv(a: &[Term], b: &[Term]) -> Vec<Term> {
        let mut out = vec![Vec::new(); a.len() + b.len() - 1];
        for (i, x) in a.iter().enumerate() {
            for (j, y) in b.iter().enumerate() {
                out[i + j].push(mul(vec![x.clone(), y.clone()]));
            }
        }
        out.into_iter().map(add).collect()
    }
    if !contains(t, v) {
        return Some(vec![t.clone()]);
    }
    Some(match t {
        Term::Sym(s) if s == v => vec![Term::zero(), Term::one()],
        Term::Add(xs) => {
            let mut out: Vec<Vec<Term>> = Vec::new();
            for x in xs {
                for (k, c) in poly_in(x, v)?.into_iter().enumerate() {
                    if out.len() <= k {
                        out.resize(k + 1, Vec::new());
                    }
                    out[k].push(c);
                }
            }
            out.into_iter().map(add).collect()
        }
        Term::Mul(xs) => {
            let mut acc = vec![Term::one()];
            for x in xs {
                acc = conv(&acc, &poly_in(x, v)?);
                if acc.len() > 64 {
                    return None;
                }
            }
            acc
        }
        Term::Pow(b, e) => {
            let k = usize::try_from(i64::try_from(&num::as_int(e.as_rat()?)?).ok()?).ok()?;
            if k > 32 {
                return None;
            }
            let pb = poly_in(b, v)?;
            let mut acc = vec![Term::one()];
            for _ in 0..k {
                acc = conv(&acc, &pb);
            }
            acc
        }
        _ => return None,
    })
}

fn solve_logs(h: &Term, ks: &[Term], x: &str, budget: &Budget, depth: u32) -> Result<Option<Solutions>, Stop> {
    // h = Σ cᵢ ln gᵢ + k（cᵢ = c × 整数、k は x を含まない）→ ∏ gᵢ^{cᵢ/c} = e^{−k/c}
    if !ks.iter().all(|k| matches!(k, Term::Fn(Func::Ln, a) if a.len() == 1)) {
        return Ok(None);
    }
    let terms: Vec<Term> = match h {
        Term::Add(a) => a.clone(),
        _ => vec![h.clone()],
    };
    let mut logs: Vec<(Term, Term)> = Vec::new();
    let mut konst = Vec::new();
    for t in terms {
        if !contains(&t, x) {
            konst.push(t);
            continue;
        }
        // t = (x を含まない係数) · ln g
        let factors: Vec<Term> = match &t {
            Term::Mul(v) => v.clone(),
            _ => vec![t.clone()],
        };
        let (with_x, coef): (Vec<Term>, Vec<Term>) = factors.into_iter().partition(|f| contains(f, x));
        let [Term::Fn(Func::Ln, g)] = with_x.as_slice() else { return Ok(None) };
        logs.push((mul(coef), g[0].clone()));
    }
    let c = logs[0].0.clone();
    let mut num_side = Vec::new();
    let mut den_side = Vec::new();
    for (ci, g) in &logs {
        let r = term::div(ci.clone(), c.clone());
        let Some(r) = r.as_rat().and_then(num::as_int) else { return Ok(None) };
        let r = i64::try_from(&r).map_err(|_| nf("係数が大きすぎます"))?;
        let gp = pow(g.clone(), Term::int(r.abs()));
        if r > 0 { num_side.push(gp) } else { den_side.push(gp) }
    }
    let rhs = mul(std::iter::once(pow(Term::Atom(Atom::E), neg(term::div(add(konst), c)))).chain(den_side).collect());
    let eq = add(vec![mul(num_side), neg(rhs)]);
    solve_zero(&eq, x, budget, depth + 1).map(Some)
}

// ---------------------------------------------------------------- 確かめ

/// 候補を元の式 h に入れて 0 になるものだけ残す（2 乗したときの余分な解・定義域の外・分母が 0 を捨てる）。
/// 一般解の族は n = 0, 1 で確かめる。確かめられない候補があれば Err
pub fn verify(h: &Term, x: &str, s: Solutions) -> Result<Solutions, Stop> {
    let ok = |v: &Term| -> Result<bool, Stop> {
        let Ok(t) = super::diff::subst_checked(h, x, v) else { return Ok(false) };
        // 値が実数でない（定義域の外）なら捨てる
        if let Ok(b) = ball::value(v, 64) {
            if !b.is_real() {
                return Ok(false);
            }
        }
        let mut rules = Vec::new();
        match ball::is_zero(&t, &mut rules) {
            Some(z) => Ok(z),
            None => Err(nf("解を元の式に入れて確かめられません")),
        }
    };
    let mut out = Solutions { all: s.all, ..Solutions::default() };
    for p in s.points {
        if ok(&p)? {
            out.points.push(p);
        }
    }
    sort_points(&mut out.points);
    for (b, per) in s.families {
        // 0 ≤ b < 周期 に寄せる
        let (bv, pv) = (approx(&b).ok_or_else(|| nf("値が決まりません"))?, approx(&per).ok_or_else(|| nf("値が決まりません"))?);
        let k = (bv / pv).floor();
        let b = if k != 0.0 { add(vec![b, mul(vec![Term::int(-(k as i64)), per.clone()])]) } else { b };
        let b1 = add(vec![b.clone(), per.clone()]);
        if !(ok(&b)? && ok(&b1)?) {
            continue;
        }
        // 同じ族（周期が同じで、b が同じ）を 1 つに
        let bv = approx(&b).unwrap_or(f64::NAN);
        if out.families.iter().any(|(c, q)| q == &per && approx(c).is_some_and(|cv| (cv - bv).abs() <= 1e-12 * (1.0 + bv.abs()))) {
            continue;
        }
        out.families.push((b, per));
    }
    merge_families(&mut out.families);
    out.families.sort_by(|a, b| approx(&a.0).unwrap_or(0.0).total_cmp(&approx(&b.0).unwrap_or(0.0)));
    // 族に含まれる点は除く（x sin x = 0 の 0 は πn に入る）
    let fams: Vec<(f64, f64)> = out.families.iter().filter_map(|(b, p)| Some((approx(b)?, approx(p)?))).collect();
    out.points.retain(|p| {
        let Some(v) = approx(p) else { return true };
        !fams.iter().any(|&(b, q)| {
            let t = (v - b) / q;
            (t - t.round()).abs() <= 1e-9
        })
    });
    Ok(out)
}

/// 同じ周期 P の族の底が、P/k おきに k 個並ぶなら 1 つの族（周期 P/k）にまとめる（2πn と π + 2πn → πn）
fn merge_families(fs: &mut Vec<(Term, Term)>) {
    loop {
        let mut changed = false;
        let periods: Vec<Term> = fs.iter().map(|f| f.1.clone()).collect();
        for p in periods {
            let mut group: Vec<(f64, Term)> = fs.iter().filter(|f| f.1 == p).filter_map(|f| Some((approx(&f.0)?, f.0.clone()))).collect();
            let k = group.len();
            if k < 2 {
                continue;
            }
            let Some(pv) = approx(&p) else { continue };
            group.sort_by(|a, b| a.0.total_cmp(&b.0));
            // 全部で等間隔、か、P/2 だけ離れた 2 つ
            let step = pv / k as f64;
            let even = group.windows(2).all(|w| ((w[1].0 - w[0].0) - step).abs() <= 1e-9 * pv);
            let pair = if even {
                Some((group[0].1.clone(), k as i64, None))
            } else {
                let mut found = None;
                'outer: for i in 0..k {
                    for j in i + 1..k {
                        if ((group[j].0 - group[i].0) - pv / 2.0).abs() <= 1e-9 * pv {
                            found = Some((group[i].1.clone(), 2, Some(group[j].1.clone())));
                            break 'outer;
                        }
                    }
                }
                found
            };
            let Some((b0, kk, other)) = pair else { continue };
            let np = term::div(p.clone(), Term::int(kk));
            if other.is_none() {
                fs.retain(|f| f.1 != p);
            } else {
                let o = other.unwrap();
                fs.retain(|f| !(f.1 == p && (f.0 == b0 || f.0 == o)));
            }
            fs.push((b0, np));
            changed = true;
            break;
        }
        if !changed {
            break;
        }
    }
}

/// x を含む常用対数 log g を ln g / ln 10 にする（対数の和をまとめるため）
fn log10_to_ln(t: &Term, x: &str) -> Term {
    match t {
        Term::Fn(Func::Log10, a) if a.len() == 1 && contains(&a[0], x) => term::div(call(Func::Ln, vec![log10_to_ln(&a[0], x)]), call(Func::Ln, vec![Term::int(10)])),
        Term::Add(v) => add(v.iter().map(|a| log10_to_ln(a, x)).collect()),
        Term::Mul(v) => mul(v.iter().map(|a| log10_to_ln(a, x)).collect()),
        Term::Pow(b, e) => pow(log10_to_ln(b, x), log10_to_ln(e, x)),
        Term::Fn(f, v) => call(*f, v.iter().map(|a| log10_to_ln(a, x)).collect()),
        _ => t.clone(),
    }
}

/// 方程式 lhs = rhs を x について解く（確かめ済み）
pub fn solve(lhs: &Term, rhs: &Term, x: &str, budget: &Budget) -> Result<Solutions, Stop> {
    let h = log10_to_ln(&add(vec![lhs.clone(), neg(rhs.clone())]), x);
    let s = solve_zero(&h, x, budget, 0)?;
    verify(&h, x, s)
}

// ---------------------------------------------------------------- 方程式の行

/// 文字の名前を集める
pub(crate) fn collect_syms(t: &Term, out: &mut Vec<String>) {
    syms(t, out)
}

/// 文字の名前を集める
fn syms(t: &Term, out: &mut Vec<String>) {
    match t {
        Term::Sym(s) => {
            if !out.contains(s) {
                out.push(s.clone());
            }
        }
        Term::Num(_) | Term::Atom(_) => {}
        Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) | Term::Matrix(_, _, v) => v.iter().for_each(|a| syms(a, out)),
        Term::Pow(b, e) => {
            syms(b, out);
            syms(e, out);
        }
    }
}

/// 組 m ± d（m は有理数、d > 0）の LaTeX。d が「有理数 × 根号など 1 つ」なら 1 本の分数線にまとめる（$\frac{-1\pm\sqrt5}{2}$）
fn plus_minus_latex(m: &Rat, d: &Term, st: &super::latex::Style) -> String {
    let (c, rest) = term::split_coeff(d);
    let single = !matches!(rest, Term::Add(_)) && !rest.is_one();
    if single {
        let den = num::lcm(&Int::from(m.denominator().clone()), &Int::from(c.denominator().clone()));
        let mi = num::as_int(&(m.clone() * Rat::from(den.clone()))).unwrap();
        let ci = num::as_int(&(c.clone() * Rat::from(den.clone()))).unwrap();
        let r = super::latex::to_latex(&rest, st);
        let cpart = if ci == Int::ONE { r } else { format!("{ci}{r}") };
        let top = if mi == Int::ZERO { format!("\\pm {cpart}") } else { format!("{mi}\\pm {cpart}") };
        return if den == Int::ONE { top } else { format!("\\frac{{{top}}}{{{den}}}") };
    }
    let body = super::latex::to_latex(d, st);
    let body = if matches!(d, Term::Add(_)) && !body.starts_with("\\frac") { format!("\\left({body}\\right)") } else { body };
    if m.is_zero() {
        format!("\\pm {body}")
    } else {
        format!("{}\\pm {body}", super::latex::to_latex(&Term::Num(m.clone()), st))
    }
}

/// 解の並びの LaTeX。中点が有理数の組は ±（$\pm\sqrt2$・$1 \pm \sqrt2$）にまとめる
fn points_latex(points: &[Term], st: &super::latex::Style) -> String {
    let mut used = vec![false; points.len()];
    // (並べる大きさ, LaTeX)。± の組は中点と幅で、ほかは値で並べる
    let mut parts: Vec<(f64, String)> = Vec::new();
    for i in 0..points.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        let p = &points[i];
        // 中点 (p + q)/2 が有理数で、p ≠ q
        let partner = (i + 1..points.len()).find_map(|j| {
            if used[j] {
                return None;
            }
            let mid = term::div(add(vec![points[j].clone(), p.clone()]), Term::int(2));
            let m = mid.as_rat()?.clone();
            let d = add(vec![points[j].clone(), neg(Term::Num(m.clone()))]);
            (sign(&d) == Some(1) && (m.is_zero() || !matches!(d, Term::Num(_)))).then_some((j, m, d))
        });
        match partner {
            Some((j, m, d)) => {
                used[j] = true;
                let key = m.to_f64().value().abs() + approx(&d).unwrap_or(0.0) * 1e-9;
                parts.push((if m.is_zero() { approx(&d).unwrap_or(0.0) } else { key.max(m.to_f64().value()) }, plus_minus_latex(&m, &d, st)));
            }
            None => parts.push((approx(p).unwrap_or(0.0), super::latex::to_latex(p, st))),
        }
    }
    parts.sort_by(|a, b| a.0.total_cmp(&b.0));
    parts.into_iter().map(|p| p.1).collect::<Vec<_>>().join(",\\ ")
}

/// 1 変数の方程式の解の LaTeX（`x = …`）
pub fn solutions_latex(x: &str, s: &Solutions, st: &super::latex::Style) -> String {
    if s.all {
        return format!("{x}\\in\\mathbb{{R}}");
    }
    if s.is_empty() {
        return "\\text{実数の解はありません}".into();
    }
    let mut items = Vec::new();
    if !s.points.is_empty() {
        items.push(points_latex(&s.points, st));
    }
    let n = Term::Sym("n".into());
    for (b, p) in &s.families {
        items.push(super::latex::to_latex(&add(vec![b.clone(), mul(vec![p.clone(), n.clone()])]), st));
    }
    let tail = if s.families.is_empty() { String::new() } else { "\\quad\\left(n\\in\\mathbb{Z}\\right)".into() };
    format!("{x}={}{tail}", items.join(",\\ "))
}

/// 今の f64 の道で、h の符号が変わる所（[−20, 20]）を探し、厳密値の道の解に無いものを返す（取りこぼしの確かめ。計画の 8）
fn missing_root(lhs: &str, rhs: &str, x: &str, s: &Solutions, excluded: &[f64], params: &crate::eval::Params) -> Option<f64> {
    let src = format!("({lhs})-({rhs})");
    let n = 4001;
    let xs: Vec<f64> = (0..n).map(|k| -20.0 + 40.0 * k as f64 / (n - 1) as f64 + 1.2345e-7).collect();
    let prog = crate::equation::compile_1var(&src, x, params).ok()?;
    let mut vals = vec![num_complex::Complex64::new(0.0, 0.0); xs.len()];
    prog.eval_batch(&[&xs], &mut vals);
    let f = |v: f64| -> Option<f64> {
        let mut c = [num_complex::Complex64::new(0.0, 0.0)];
        prog.eval_batch(&[&[v]], &mut c);
        (c[0].im == 0.0 && c[0].re.is_finite()).then_some(c[0].re)
    };
    let pts: Vec<f64> = s.points.iter().filter_map(approx).collect();
    let fams: Vec<(f64, f64)> = s.families.iter().filter_map(|(b, p)| Some((approx(b)?, approx(p)?))).collect();
    for k in 0..n - 1 {
        let (a, b) = (vals[k], vals[k + 1]);
        if a.im != 0.0 || b.im != 0.0 || !a.re.is_finite() || !b.re.is_finite() || a.re.signum() == b.re.signum() {
            continue;
        }
        let (mut lo, mut hi, flo) = (xs[k], xs[k + 1], a.re);
        for _ in 0..60 {
            let m = 0.5 * (lo + hi);
            let Some(fm) = f(m) else { break };
            if fm.signum() == flo.signum() { lo = m } else { hi = m }
        }
        let r = 0.5 * (lo + hi);
        // 極（分母の 0）でなく、本当に 0 に近づいた所
        let Some(fr) = f(r) else { continue };
        if fr.abs() > 1e-6 * (1.0 + a.re.abs().max(b.re.abs())) {
            continue;
        }
        if s.all {
            return None;
        }
        let near = |v: f64| (v - r).abs() <= 1e-6 * (1.0 + r.abs());
        let in_family = |(b, p): &(f64, f64)| {
            let t = (r - b) / p;
            (t - t.round()).abs() * p.abs() <= 1e-6 * (1.0 + r.abs())
        };
        // 定義域の外として除いた点（(x − 1)²/(x − 1) の x = 1。近くでは値が 0 に近づく）は数えない。
        // 近くの分母の小さい分数で値が有限でなければ、その穴とみなす
        let hole = (1..=12).any(|d| {
            let q = (r * d as f64).round() / d as f64;
            (q - r).abs() <= 1e-6 * (1.0 + r.abs()) && f(q).is_none()
        });
        if !hole && !pts.iter().any(|&v| near(v)) && !fams.iter().any(in_family) && !excluded.iter().any(|&v| near(v)) {
            return Some(r);
        }
    }
    None
}

/// 方程式の行（`x^2 = 2`・連立 `x^2 + y^2 = 1, y = x`）なら、その解の答え。方程式の行でなければ None
pub fn equation_report(source: &str, params: &crate::eval::Params, work: u64) -> Option<super::Report> {
    use super::{latex, Outcome, Report};
    use crate::parser::Rel;
    if !source.contains('=') {
        return None;
    }
    let defs = crate::expand::current_defs();
    let names = defs.names();
    let chains = crate::parser::parse_system_with(source, &names).ok()?;
    if chains.is_empty() || !chains.iter().all(|c| c.exprs.len() == 2 && c.rels == [Rel::Eq]) {
        return None;
    }
    let (r, used) = super::with_work_limit(work, || -> Result<Option<Report>, Stop> {
        let mut c = super::convert::Converter::new(&defs, params);
        let mut sides = Vec::new();
        for ch in &chains {
            sides.push((c.term(&ch.exprs[0])?, c.term(&ch.exprs[1])?));
        }
        let mut vars = Vec::new();
        for (l, r) in &sides {
            syms(l, &mut vars);
            syms(r, &mut vars);
        }
        vars.sort_by_key(|v| crate::equation::coord_index(v).unwrap_or(99));
        let st = latex::Style { decimal: c.decimal, input: false };
        let budget = Budget::new(work);
        let mut rep = Report { decimal: c.decimal, uses_ln: c.uses_ln, equation: true, ..Report::new(Outcome::NotFound(String::new())) };
        if sides.len() == 1 {
            let [x] = vars.as_slice() else { return Ok(None) };
            let (l, r) = &sides[0];
            // x = 3 のような行は、描いた縦の直線そのもの（解を出しても同じ）
            if (*l == Term::Sym(x.clone()) && !contains(r, x)) || (*r == Term::Sym(x.clone()) && !contains(l, x)) {
                return Ok(None);
            }
            let mut s = solve(l, r, x, &budget)?;
            let mut excluded: Vec<f64> = Vec::new();
            let (ls, rs) = source.split_once('=').unwrap_or((source, "0"));
            if source.matches('=').count() == 1 {
                // 元の式（正規化の前。(x − 1)²/(x − 1) は x = 1 で 0 で割る）を今の f64 の道で計算し、値が有限でない解は除く。
                // 値が 0 から離れていれば、厳密値の道の間違いなので捨てる
                if let (Ok(pl), Ok(pr)) = (crate::equation::compile_1var(ls, x, params), crate::equation::compile_1var(rs, x, params)) {
                    let at = |p: &crate::eval::Program, v: f64| {
                        let mut c = [num_complex::Complex64::new(0.0, 0.0)];
                        p.eval_batch(&[&[v]], &mut c);
                        c[0]
                    };
                    let mut keep = Vec::new();
                    let mut dropped: Vec<f64> = Vec::new();
                    for p in std::mem::take(&mut s.points) {
                        let Some(v) = approx(&p) else { keep.push(p); continue };
                        let (a, b) = (at(&pl, v), at(&pr, v));
                        if !(a.re.is_finite() && a.im.is_finite() && b.re.is_finite() && b.im.is_finite()) {
                            rep.rules.push("今の道で値が有限でない解を除いた");
                            dropped.push(v);
                            continue;
                        }
                        if (a - b).norm() > 1e-6 * (1.0 + a.norm().max(b.norm())) {
                            rep.outcome = Outcome::Rejected(format!("解 {v} を今の道で元の式に入れても 0 になりません"));
                            return Ok(Some(rep));
                        }
                        keep.push(p);
                    }
                    s.points = keep;
                    excluded = dropped;
                }
                if let Some(m) = missing_root(ls, rs, x, &s, &excluded, params) {
                    rep.outcome = Outcome::Rejected(format!("今の道で見つかった解 {m} が、厳密値の道の答えにありません"));
                    return Ok(Some(rep));
                }
            }
            rep.latex = solutions_latex(x, &s, &st);
            rep.rules.push("方程式を解き、解を元の式に入れて確かめた");
            rep.outcome = Outcome::Found(Term::List(s.points.clone()));
            return Ok(Some(rep));
        }
        // 連立方程式の点（多項式。変数は 2 つか 3 つで、式と同じ数）
        if !(2..=3).contains(&vars.len()) || vars.len() != sides.len() {
            return Ok(None);
        }
        let exprs: Vec<crate::parser::Expr> = chains
            .iter()
            .map(|ch| crate::parser::Expr::Bin(crate::parser::BinOp::Sub, Box::new(ch.exprs[0].clone()), Box::new(ch.exprs[1].clone())))
            .collect();
        let refs: Vec<&str> = vars.iter().map(|s| s.as_str()).collect();
        let Some(points) = crate::groebner::solve_exact(&exprs, &refs, params, &budget) else {
            return Err(nf("連立方程式の点を厳密に求められません"));
        };
        // 照らし合わせ: 元の式に入れて 0、f64 の道の点の数と同じ
        for p in &points {
            for (l, r) in &sides {
                let mut h = add(vec![l.clone(), neg(r.clone())]);
                for (v, val) in vars.iter().zip(p) {
                    h = super::diff::subst(&h, v, val);
                }
                if ball::is_zero(&h, &mut Vec::new()) != Some(true) {
                    rep.outcome = Outcome::Rejected("連立方程式の点を元の式に入れても 0 になりません".into());
                    return Ok(Some(rep));
                }
            }
        }
        if let Some(f) = crate::groebner::solve(&exprs, &refs, params) {
            if f.len() != points.len() {
                rep.outcome = Outcome::Rejected(format!("点の数が今の道（{}）と違います（{}）", f.len(), points.len()));
                return Ok(Some(rep));
            }
        }
        let head = format!("\\left({}\\right)", vars.join(","));
        rep.latex = if points.is_empty() {
            "\\text{実数の解はありません}".into()
        } else {
            format!("{head}={}", points.iter().map(|p| latex::to_latex(&Term::Tuple(p.clone()), &st)).collect::<Vec<_>>().join(",\\ "))
        };
        rep.rules.push("連立方程式をグレブナー基底で解き、点を元の式に入れて確かめた");
        rep.outcome = Outcome::Found(Term::List(points.into_iter().map(Term::Tuple).collect()));
        Ok(Some(rep))
    });
    match r {
        Ok(Some(mut rep)) => {
            rep.work = used;
            Some(rep)
        }
        Ok(None) => None,
        Err(Stop::NotFound(s)) => Some(super::Report { work: used, equation: true, ..super::Report::new(Outcome::NotFound(s)) }),
        Err(Stop::NotExist(s)) => Some(super::Report { work: used, equation: true, ..super::Report::new(Outcome::NotExist(s)) }),
        Err(Stop::TimeUp) => Some(super::Report { work: used, equation: true, ..super::Report::new(Outcome::NotFound("時間切れ".into())) }),
    }
}

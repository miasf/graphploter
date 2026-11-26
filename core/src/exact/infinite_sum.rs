//! 無限級数の値（計画 `docs/plan-exact-areas.md` の SR1）。
//!
//! 項 $T(k)$ を $c \cdot r^k \cdot R(k)$（$R$ は $k$ の有理関数）か、階乗を含む形に分けて、型ごとに閉じた形にする:
//! - $r = 1$（有理関数の級数）: 部分分数 $\sum_i \sum_j \frac{c_{ij}}{(k + a_i)^j}$。$j = 1$ の部分は $-\sum_i c_{i1}\psi(k_0 + a_i)$
//!   （$\sum_i c_{i1} = 0$ のときだけ収束）、$j \ge 2$ はフルヴィッツのゼータ $\zeta(j, k_0 + a_i)$。望遠鏡のように消える和もここに入る
//! - $r = -1$（交代級数）: 隣り合う 2 項を組にして、$r = 1$ の有理関数の級数にする
//! - $|r| < 1$: 多項式の部分は $\sum_{k\ge0}k^m r^k$（$S_0 = \frac{1}{1-r}$、$S_m = r\frac{d}{dr}S_{m-1}$）、$\frac{1}{k + a}$（$a$ は整数）の部分は $-\ln(1 - r)$ から
//! - 階乗: $\sum \frac{P(k)\,x^k}{(k+c)!}$ は $P$ を $(k + c)$ の下降階乗で書き直して $e^x$ の級数に、$\sum \frac{x^k}{(2k + c)!}$ は $\cosh$・$\sinh$ に
//!
//! 発散する形（有理関数で次数の差が 1 以下・$|r| \ge 1$）には値を出さない（判定は SR0）

use super::num::{self, Int, Nat, Rat};
use super::poly::{self, Poly};
use super::term::{add, call, mul, neg, pow, Atom, Term};
use super::values::{digamma_rational, hurwitz};
use super::{Budget, Stop};
use crate::functions::Func;

fn nf(msg: &str) -> Stop {
    Stop::NotFound(msg.to_string())
}

fn ri(n: i64) -> Rat {
    Rat::from(Int::from(n))
}

fn has(t: &Term, x: &str) -> bool {
    match t {
        Term::Sym(s) => s == x,
        Term::Num(_) | Term::Atom(_) => false,
        Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) | Term::Matrix(_, _, v) => v.iter().any(|a| has(a, x)),
        Term::Pow(b, e) => has(b, x) || has(e, x),
    }
}

/// $e = a k + b$ なら (a, b)（a, b は k を含まない数）
fn linear_in(e: &Term, k: &str) -> Option<(Term, Term)> {
    let a = super::diff::diff(e, k).ok()?;
    if has(&a, k) {
        return None;
    }
    let b = super::diff::subst(e, k, &Term::zero());
    Some((a, b))
}

/// 項を c · r^k · 残り に分ける（底が k を含まず、指数が k の 1 次式の因子を集める）
fn split_geometric(t: &Term, k: &str) -> (Term, Term, Term) {
    let fs: Vec<Term> = match t {
        Term::Mul(v) => v.clone(),
        other => vec![other.clone()],
    };
    let (mut c, mut r, mut rest) = (Vec::new(), Vec::new(), Vec::new());
    for f in fs {
        match &f {
            Term::Pow(b, e) if !has(b, k) && has(e, k) => match linear_in(e, k) {
                Some((a, bb)) => {
                    r.push(pow((**b).clone(), a));
                    c.push(pow((**b).clone(), bb));
                }
                None => rest.push(f),
            },
            _ if !has(&f, k) => c.push(f),
            _ => rest.push(f),
        }
    }
    (mul(c), mul(r), mul(rest))
}

/// 1 次の因子 p₁k + p₀ の根の反対 a = p₀/p₁（k + a の a）
fn shift_of(f: &Poly) -> Option<Rat> {
    (f.deg() == 1).then(|| f.0[0].clone() / f.0[1].clone())
}

/// $\sum_{k \ge lo} \frac{n(k)}{d(k)}$（有理関数。収束するときだけ）
pub fn rational_sum(n: &Poly, d: &Poly, lo: &Int) -> Result<Term, Stop> {
    if n.is_zero() {
        return Ok(Term::zero());
    }
    // 発散の判定（文書 8 の SR0 の一部）: 有理関数の項は、最後には符号が一定
    if d.deg() - n.deg() <= 0 {
        return Err(Stop::NotExist("項が 0 に近づかないので発散します".into()));
    }
    if d.deg() - n.deg() == 1 {
        return Err(Stop::NotExist("項が 1/k と同じ速さで 0 に近づくので発散します（比較判定法）".into()));
    }
    let budget = Budget::new(5_000_000);
    let (q, parts) = poly::apart(n, d, &budget).ok_or_else(|| nf("分母を因数分解できません"))?;
    if !q.is_zero() {
        return Err(nf("発散します"));
    }
    let lo_r = Rat::from(lo.clone());
    let mut out = Vec::new();
    let mut c1_sum = Rat::ZERO;
    for (numer, factor, j) in &parts {
        let a = shift_of(factor).ok_or_else(|| nf("分母に 1 次でない因数があります（k² + 1 など）"))?;
        // numer / (p₁ k + p₀)^j = (numer / p₁^j) / (k + a)^j。numer は定数（1 次の因数なので）
        let c = numer.0.first().cloned().unwrap_or(Rat::ZERO) / factor.0[1].pow(*j as isize);
        let b = lo_r.clone() + &a;
        // 範囲の中に極（k + a = 0）があれば値は無い
        if num::sign(&b) <= 0 && num::as_int(&b).is_some() {
            return Err(nf("項の分母が 0 になる k があります"));
        }
        if *j == 1 {
            c1_sum += &c;
            let psi = digamma_rational(&b).ok_or_else(|| nf("ディガンマ関数のこの値はまだ求められません"))?;
            out.push(mul(vec![Term::Num(-c), psi]));
        } else {
            let z = hurwitz(*j as i64, &b).ok_or_else(|| nf("フルヴィッツのゼータ関数のこの値はまだ求められません"))?;
            out.push(mul(vec![Term::Num(c), z]));
        }
    }
    if !c1_sum.is_zero() {
        return Err(nf("発散します"));
    }
    Ok(add(out))
}

/// $\sum_{k \ge lo} (-1)^k \frac{n(k)}{d(k)}$: $k = lo + 2m$ と $lo + 2m + 1$ を組にする
fn alternating_sum(n: &Poly, d: &Poly, lo: &Int) -> Result<Term, Stop> {
    if n.is_zero() {
        return Ok(Term::zero());
    }
    if d.deg() - n.deg() < 1 {
        return Err(Stop::NotExist("項が 0 に近づかないので発散します".into()));
    }
    // R(lo + 2m) − R(lo + 2m + 1) を m の有理関数に
    let lo_r = Rat::from(lo.clone());
    let sub = |p: &Poly, off: Rat| -> Poly {
        // p(lo + off + 2m)
        let lin = Poly::new(vec![lo_r.clone() + off, ri(2)]);
        let mut acc = Poly::zero();
        for (i, c) in p.0.iter().enumerate() {
            acc = acc.add(&lin.pow(i as u32).scale(c));
        }
        acc
    };
    let (n0, d0) = (sub(n, Rat::ZERO), sub(d, Rat::ZERO));
    let (n1, d1) = (sub(n, Rat::ONE), sub(d, Rat::ONE));
    let nn = n0.mul(&d1).sub(&n1.mul(&d0));
    let dd = d0.mul(&d1);
    let s = rational_sum(&nn, &dd, &Int::ZERO)?;
    Ok(if dashu::base::BitTest::bit(lo, 0) { neg(s) } else { s })
}

/// $\sum_{k\ge0} k^m r^k$（r は文字 #r の式）
fn power_geometric(m: usize) -> Result<Term, Stop> {
    let r = Term::Sym("#r".into());
    let mut s = pow(add(vec![Term::one(), neg(r.clone())]), Term::int(-1));
    for _ in 0..m {
        s = mul(vec![r.clone(), super::diff::diff(&s, "#r")?]);
    }
    Ok(s)
}

/// $\sum_{k \ge lo} r^k \frac{n(k)}{d(k)}$（$|r| < 1$）
fn geometric_rational(r: &Term, n: &Poly, d: &Poly, lo: &Int) -> Result<Term, Stop> {
    let budget = Budget::new(5_000_000);
    let (q, parts) = poly::apart(n, d, &budget).ok_or_else(|| nf("分母を因数分解できません"))?;
    let lo_i = i64::try_from(lo).map_err(|_| nf("和の下端が大きすぎます"))?;
    let mut out = Vec::new();
    // 多項式の部分: Σ_{k≥0} q(k) r^k − Σ_{k<lo} q(k) r^k
    for (m, c) in q.0.iter().enumerate() {
        if c.is_zero() {
            continue;
        }
        let s = super::diff::subst(&power_geometric(m)?, "#r", r);
        out.push(mul(vec![Term::Num(c.clone()), s]));
    }
    for kk in 0..lo_i.max(0) {
        let v = q.eval(&ri(kk));
        if !v.is_zero() {
            out.push(neg(mul(vec![Term::Num(v), pow(r.clone(), Term::int(kk))])));
        }
    }
    // 分数の部分: c / (k + a)（a は整数）→ r^{−a} (−ln(1 − r) − Σ_{m=1}^{lo+a−1} r^m/m)
    for (numer, factor, j) in &parts {
        if *j != 1 {
            return Err(nf("多重対数（Li₂ など）になる級数はまだ求められません"));
        }
        let a = shift_of(factor).ok_or_else(|| nf("分母に 1 次でない因数があります"))?;
        let a = num::as_int(&a).and_then(|a| i64::try_from(&a).ok()).ok_or_else(|| nf("分母のずれが整数でない級数はまだ求められません"))?;
        let c = numer.0.first().cloned().unwrap_or(Rat::ZERO) / factor.0[1].clone();
        let start = lo_i + a;
        if start < 1 {
            return Err(nf("項の分母が 0 になる k があります"));
        }
        let mut head = Vec::new();
        for m in 1..start {
            head.push(mul(vec![Term::Num(Rat::from_parts(Int::ONE, Nat::from(m as u64))), pow(r.clone(), Term::int(m))]));
        }
        let ln = call(Func::Ln, vec![add(vec![Term::one(), neg(r.clone())])]);
        let inner = add(vec![neg(ln), neg(add(head))]);
        out.push(mul(vec![Term::Num(c), pow(r.clone(), Term::int(-a)), inner]));
    }
    Ok(add(out))
}

/// 階乗の形: 項 = c · x^k · P(k) / (k + s)!（s は整数）、または c · x^k / (2k + s)!
fn factorial_series(t: &Term, k: &str, lo: &Int) -> Result<Option<Term>, Stop> {
    let (c, r, rest) = split_geometric(t, k);
    let fs: Vec<Term> = match &rest {
        Term::Mul(v) => v.clone(),
        other => vec![other.clone()],
    };
    // 分母の階乗 Γ(a k + b)^{−1}
    let mut fact: Option<(Rat, Rat)> = None;
    let mut others = Vec::new();
    for f in &fs {
        if let Term::Pow(base, e) = f {
            if let (Term::Fn(Func::Gamma, args), Some(q)) = (&**base, e.as_rat()) {
                if *q == ri(-1) && args.len() == 1 {
                    if let Some((a, b)) = linear_in(&args[0], k) {
                        if let (Some(a), Some(b)) = (a.as_rat(), b.as_rat()) {
                            if fact.is_none() {
                                fact = Some((a.clone(), b.clone()));
                                continue;
                            }
                        }
                    }
                }
            }
        }
        others.push(f.clone());
    }
    let Some((a, b)) = fact else { return Ok(None) };
    let p = poly::to_poly(&mul(others), k).ok_or_else(|| nf("階乗の級数の分子が多項式ではありません"))?;
    let x = if r.is_one() { Term::one() } else { r.clone() };
    let lo_i = i64::try_from(lo).map_err(|_| nf("和の下端が大きすぎます"))?;
    let b_int = num::as_int(&b).and_then(|b| i64::try_from(&b).ok()).ok_or_else(|| nf("階乗の中が整数でありません"))?;
    let exp_x = pow(Term::Atom(Atom::E), x.clone());
    if a == ri(1) {
        // (k + s)! で s = b − 1。P(k) を u = k + s の下降階乗で: P = Σ a_j u(u−1)…(u−j+1)
        let s = b_int - 1;
        let pu: Vec<Rat> = (0..=p.deg().max(0)).map(|i| p.eval(&(ri(i) - ri(s)))).collect();
        // 前進差分 Δ^j p(0) / j!
        let mut diffs = pu.clone();
        let mut coeffs = Vec::new();
        let mut fact_j = Rat::ONE;
        for j in 0..pu.len() {
            if j > 0 {
                fact_j *= ri(j as i64);
            }
            coeffs.push(diffs[0].clone() / fact_j.clone());
            diffs = diffs.windows(2).map(|w| w[1].clone() - w[0].clone()).collect();
            if diffs.is_empty() {
                break;
            }
        }
        // Σ_{k≥lo} x^k / (k + s − j)! = x^{j − s} (e^x − Σ_{m=0}^{lo+s−j−1} x^m/m!)
        let mut out = Vec::new();
        for (j, aj) in coeffs.iter().enumerate() {
            if aj.is_zero() {
                continue;
            }
            let start = lo_i + s - j as i64;
            let mut head = Vec::new();
            let mut mf = Rat::ONE;
            for m in 0..start.max(0) {
                if m > 0 {
                    mf *= ri(m);
                }
                head.push(mul(vec![Term::Num(Rat::ONE / mf.clone()), pow(x.clone(), Term::int(m))]));
            }
            let tail = add(vec![exp_x.clone(), neg(add(head))]);
            out.push(mul(vec![Term::Num(aj.clone()), pow(x.clone(), Term::int(j as i64 - s)), tail]));
        }
        return Ok(Some(mul(vec![c, add(out)])));
    }
    if a == ri(2) && p.deg() == 0 && x.is_one() {
        // Σ_{k≥lo} 1/(2k + b − 1)!: n = 2k + b − 1 の偶奇の和。cosh 1 = (e + 1/e)/2、sinh 1 = (e − 1/e)/2
        let off = b_int - 1;
        let e1 = Term::Atom(Atom::E);
        let einv = pow(Term::Atom(Atom::E), Term::int(-1));
        let full = if off.rem_euclid(2) == 0 { mul(vec![Term::rat(1, 2), add(vec![e1, einv])]) } else { mul(vec![Term::rat(1, 2), add(vec![e1, neg(einv)])]) };
        let start = 2 * lo_i + off;
        let mut head = Rat::ZERO;
        let mut n = off.rem_euclid(2);
        while n < start {
            let mut f = Rat::ONE;
            for m in 2..=n {
                f *= ri(m);
            }
            head += Rat::ONE / f;
            n += 2;
        }
        let v = add(vec![full, Term::Num(-head)]);
        return Ok(Some(mul(vec![c, Term::Num(p.0.first().cloned().unwrap_or(Rat::ZERO)), v])));
    }
    Err(nf("この階乗の形の級数はまだ求められません"))
}

/// $\sum_{k \ge lo}^{\infty} t$
pub fn infinite_sum(t: &Term, k: &str, lo: &Int) -> Result<Term, Stop> {
    if let Some(v) = factorial_series(t, k, lo)? {
        return Ok(v);
    }
    let (c, r, rest) = split_geometric(t, k);
    let (n, d) = poly::to_rational(&rest, k).ok_or_else(|| nf("この形の級数はまだ求められません"))?;
    let core = if r.is_one() {
        rational_sum(&n, &d, lo)?
    } else if r == Term::int(-1) {
        alternating_sum(&n, &d, lo)?
    } else {
        // |r| < 1 を確かめる
        let b = super::ball::value(&r, 64).map_err(|_| nf("公比の値が決まりません"))?;
        let (re, im) = b.to_f64();
        let m = (re * re + im * im).sqrt();
        if m > 1.0 + 1e-9 && !n.is_zero() {
            // |r| > 1: 指数の増え方は有理関数のどんな減り方にも勝つので、項が 0 に近づかない
            return Err(Stop::NotExist("項が 0 に近づかないので発散します（公比の大きさが 1 より大きい）".into()));
        }
        if m >= 1.0 - 1e-12 {
            return Err(nf("公比の大きさが 1 以上なので、値は出しません"));
        }
        geometric_rational(&r, &n, &d, lo)?
    };
    Ok(mul(vec![c, core]))
}

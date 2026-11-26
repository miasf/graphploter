//! 関数の特別な値の表（計画 `docs/plan-exact-areas.md` の SC0）。
//!
//! - 三角関数: 角を $\frac{p}{q}\pi$ に直し、対称性で第 1 象限 $[0, \frac{\pi}{2}]$ に寄せて表を引く。
//!   表の分母は $q \in \{1, 2, 3, 4, 5, 6, 8, 10, 12\}$（$\sin\frac{\pi}{5} = \frac{\sqrt{10 - 2\sqrt5}}{4}$、$\tan\frac{\pi}{8} = \sqrt2 - 1$ など）。
//!   それ以外の角（$\sin\frac{\pi}{7}$）はそのまま
//! - 逆三角関数: 表の値と式の形が一致する角（主値）
//! - $\arg$: 実部と虚部の符号と $\arctan$ の表から
//! - 常用対数の和: 有理数の常用対数の和が 10 の整数乗の対数になれば、その整数（$\log 2 + \log 5 = 1$）。[`super::term::add`] が呼ぶ
//! - ガンマ関数の半整数（$\Gamma(\frac52) = \frac{3\sqrt\pi}{4}$）、ゼータ関数の偶数と 0 以下の整数（ベルヌーイ数）、
//!   ディガンマ関数の整数と半整数（$\psi(\frac12) = -\gamma - 2\ln 2$）

use super::ball;
use super::num::{self, Int, Nat, Rat};
use super::term::{add, mul, neg, pow, Atom, Term};
use crate::functions::Func;

fn sqrt(t: Term) -> Term {
    pow(t, Term::rat(1, 2))
}

fn half(t: Term) -> Term {
    mul(vec![Term::rat(1, 2), t])
}

fn quarter(t: Term) -> Term {
    mul(vec![Term::rat(1, 4), t])
}

fn s(n: i64) -> Term {
    sqrt(Term::int(n))
}

/// 第 1 象限の角 $r\pi$（$0 \le r \le \frac12$）の $(\cos, \sin, \tan)$。表に無ければ None。$\tan\frac{\pi}{2}$ は None
fn first_quadrant(r: &Rat) -> Option<(Term, Term, Option<Term>)> {
    let d = u64::try_from(r.denominator()).ok()?;
    let n = i64::try_from(r.numerator()).ok()?;
    // 角を 120 等分の番号にする（分母 1, 2, 3, 4, 5, 6, 8, 10, 12 は 120 を割り切る）
    if 120 % d != 0 {
        return None;
    }
    let k = n * (120 / d as i64);
    let one = Term::one;
    let r5 = || s(5);
    Some(match k {
        0 => (one(), Term::zero(), Some(Term::zero())),
        10 => (quarter(add(vec![s(6), s(2)])), quarter(add(vec![s(6), neg(s(2))])), Some(add(vec![Term::int(2), neg(s(3))]))),
        12 => (quarter(sqrt(add(vec![Term::int(10), mul(vec![Term::int(2), r5()])]))), quarter(add(vec![r5(), Term::int(-1)])), Some(mul(vec![Term::rat(1, 5), sqrt(add(vec![Term::int(25), mul(vec![Term::int(-10), r5()])]))]))),
        15 => (half(sqrt(add(vec![Term::int(2), s(2)]))), half(sqrt(add(vec![Term::int(2), neg(s(2))]))), Some(add(vec![s(2), Term::int(-1)]))),
        20 => (half(s(3)), Term::rat(1, 2), Some(mul(vec![Term::rat(1, 3), s(3)]))),
        24 => (quarter(add(vec![Term::int(1), r5()])), quarter(sqrt(add(vec![Term::int(10), mul(vec![Term::int(-2), r5()])]))), Some(sqrt(add(vec![Term::int(5), mul(vec![Term::int(-2), r5()])])))),
        30 => (half(s(2)), half(s(2)), Some(one())),
        36 => (quarter(sqrt(add(vec![Term::int(10), mul(vec![Term::int(-2), r5()])]))), quarter(add(vec![Term::int(1), r5()])), Some(mul(vec![Term::rat(1, 5), sqrt(add(vec![Term::int(25), mul(vec![Term::int(10), r5()])]))]))),
        40 => (Term::rat(1, 2), half(s(3)), Some(s(3))),
        45 => (half(sqrt(add(vec![Term::int(2), neg(s(2))]))), half(sqrt(add(vec![Term::int(2), s(2)]))), Some(add(vec![s(2), Term::int(1)]))),
        48 => (quarter(add(vec![r5(), Term::int(-1)])), quarter(sqrt(add(vec![Term::int(10), mul(vec![Term::int(2), r5()])]))), Some(sqrt(add(vec![Term::int(5), mul(vec![Term::int(2), r5()])])))),
        50 => (quarter(add(vec![s(6), neg(s(2))])), quarter(add(vec![s(6), s(2)])), Some(add(vec![Term::int(2), s(3)]))),
        60 => (Term::zero(), one(), None),
        _ => return None,
    })
}

/// 角 $r\pi$ の $(\cos, \sin, \tan)$（$r$ は有理数）。表に無ければ None
pub fn trig_of_pi_multiple(r: &Rat) -> Option<(Term, Term, Option<Term>)> {
    // [0, 2) に寄せる
    let two = Rat::from(Int::from(2));
    let mut a = r.clone() - Rat::from((r.clone() / &two).floor()) * &two;
    if a < Rat::ZERO {
        a += &two;
    }
    let halfr = Rat::from_parts(Int::ONE, Nat::from(2u8));
    let one = Rat::ONE;
    // 象限ごとに第 1 象限の角と符号
    let (base, cs, ss) = if a <= halfr {
        (a.clone(), 1, 1)
    } else if a <= one {
        (one.clone() - &a, -1, 1)
    } else if a <= one.clone() + &halfr {
        (a.clone() - &one, -1, -1)
    } else {
        (two.clone() - &a, 1, -1)
    };
    let (c, s, t) = first_quadrant(&base)?;
    let c = if cs < 0 { neg(c) } else { c };
    let s = if ss < 0 { neg(s) } else { s };
    let t = t.map(|t| if cs * ss < 0 { neg(t) } else { t });
    Some((c, s, t))
}

/// 式が $\frac{p}{q}\pi$ の形なら $\frac{p}{q}$
pub fn as_pi_multiple(t: &Term) -> Option<Rat> {
    match t {
        Term::Num(q) if q.is_zero() => Some(Rat::ZERO),
        Term::Atom(Atom::Pi) => Some(Rat::ONE),
        Term::Mul(v) if v.len() == 2 && v[1] == Term::Atom(Atom::Pi) => v[0].as_rat().cloned(),
        _ => None,
    }
}

/// 主値の範囲の、表に載っている角（$r\pi$）。逆三角関数で、値の式の形が一致するものを探す
fn table_angles() -> Vec<Rat> {
    let mut out = Vec::new();
    for k in [0i64, 10, 12, 15, 20, 24, 30, 36, 40, 45, 48, 50, 60] {
        out.push(Rat::from_parts(Int::from(k), Nat::from(120u8)));
    }
    out
}

fn pi_times(r: Rat) -> Term {
    mul(vec![Term::Num(r), Term::Atom(Atom::Pi)])
}

/// 実数の数の符号（値で決める。決まらなければ None）
pub fn real_sign(t: &Term) -> Option<i32> {
    if let Some(q) = t.as_rat() {
        return Some(num::sign(q));
    }
    let b = ball::value(t, 64).ok()?;
    if !b.is_real() || b.contains_zero() {
        return None;
    }
    Some(if b.re.sign() == dashu::base::Sign::Positive { 1 } else { -1 })
}

/// 特別な値（無ければ None）
pub fn special(f: Func, args: &[Term]) -> Option<Term> {
    let one = |v: &[Term]| if v.len() == 1 { Some(v[0].clone()) } else { None };
    match f {
        Func::Sin | Func::Cos | Func::Tan | Func::Sec | Func::Csc | Func::Cot => {
            let r = as_pi_multiple(&one(args)?)?;
            let (c, s, t) = trig_of_pi_multiple(&r)?;
            match f {
                Func::Sin => Some(s),
                Func::Cos => Some(c),
                Func::Tan => t,
                Func::Sec => (!c.is_zero()).then(|| pow(c, Term::int(-1))),
                Func::Csc => (!s.is_zero()).then(|| pow(s, Term::int(-1))),
                _ => {
                    // cot = tan(π/2 − x)
                    let half = Rat::from_parts(Int::ONE, Nat::from(2u8));
                    trig_of_pi_multiple(&(half - r))?.2
                }
            }
        }
        Func::Asin | Func::Acos | Func::Atan => {
            let v = one(args)?;
            if !v.is_number() {
                return None;
            }
            // 符号で分けて、0 以上の値を表の第 1 象限の値と比べる
            let sg = real_sign(&v)?;
            let a = if sg < 0 { neg(v.clone()) } else { v.clone() };
            let mut found: Option<Rat> = None;
            for r in table_angles() {
                let Some((c, s, t)) = first_quadrant(&r) else { continue };
                let target = match f {
                    Func::Asin | Func::Acos => s,
                    _ => match t {
                        Some(t) => t,
                        None => continue,
                    },
                };
                if target == a {
                    found = Some(r);
                    break;
                }
                let _ = c;
            }
            let r = found?;
            let half = Rat::from_parts(Int::ONE, Nat::from(2u8));
            match f {
                // arcsin(−v) = −arcsin v
                Func::Asin => Some(pi_times(if sg < 0 { -r } else { r })),
                // arccos v = π/2 − arcsin v
                Func::Acos => Some(pi_times(half - if sg < 0 { -r } else { r })),
                _ => Some(pi_times(if sg < 0 { -r } else { r })),
            }
        }
        Func::Arg => {
            let z = one(args)?;
            let (re, im) = super::term::re_im(&z)?;
            let (sr, si) = (real_sign(&re)?, real_sign(&im)?);
            if sr == 0 && si == 0 {
                return None;
            }
            if sr == 0 {
                return Some(pi_times(Rat::from_parts(Int::from(si), Nat::from(2u8))));
            }
            let at = special(Func::Atan, &[mul(vec![im, pow(re, Term::int(-1))])])?;
            Some(match (sr, si) {
                (1, _) => at,
                (_, -1) => add(vec![at, neg(Term::Atom(Atom::Pi))]),
                _ => add(vec![at, Term::Atom(Atom::Pi)]),
            })
        }
        Func::Re | Func::Im | Func::Conj => {
            let (re, im) = super::term::re_im(&one(args)?)?;
            Some(match f {
                Func::Re => re,
                Func::Im => im,
                _ => add(vec![re, mul(vec![Term::int(-1), im, Term::Atom(Atom::I)])]),
            })
        }
        Func::Gamma => {
            let q = one(args)?.as_rat()?.clone();
            // 半整数 n + 1/2: Γ(n + 1/2) = (2n)! √π / (4^n n!)、負は Γ(x) = Γ(x+1)/x で
            if *q.denominator() != Nat::from(2u8) {
                return None;
            }
            let n = (q.clone() - Rat::from_parts(Int::ONE, Nat::from(2u8))).floor();
            let n = i64::try_from(&n).ok()?;
            if n.abs() > 2000 {
                return None;
            }
            let sqrt_pi = pow(Term::Atom(Atom::Pi), Term::rat(1, 2));
            if n >= 0 {
                let budget = super::Budget::new(1_000_000);
                let f2n = num::factorial(2 * n as u64, &budget).ok()?;
                let fact = num::factorial(n as u64, &budget).ok()?;
                let c = Rat::from_parts_signed(f2n, Int::from(4).pow(n as usize) * fact);
                Some(mul(vec![Term::Num(c), sqrt_pi]))
            } else {
                // Γ(1/2 − m) = (−4)^m m! √π / (2m)!
                let m = (-n) as u64;
                let budget = super::Budget::new(1_000_000);
                let fm = num::factorial(m, &budget).ok()?;
                let f2m = num::factorial(2 * m, &budget).ok()?;
                let c = Rat::from_parts_signed(Int::from(-4).pow(m as usize) * fm, f2m);
                Some(mul(vec![Term::Num(c), sqrt_pi]))
            }
        }
        Func::Zeta => {
            let n = num::as_int(one(args)?.as_rat()?)?;
            let n = i64::try_from(&n).ok()?;
            if n == 1 || n.abs() > 400 {
                return None;
            }
            if n == 0 {
                return Some(Term::rat(-1, 2));
            }
            if n < 0 {
                // ζ(−m) = (−1)^m B_{m+1} / (m+1)
                let m = (-n) as usize;
                let b = num::bernoulli(m + 1);
                let v = b / Rat::from(Int::from(m as i64 + 1));
                return Some(Term::Num(if m % 2 == 1 { -v } else { v }));
            }
            if n % 2 == 1 {
                return None; // 奇数: 閉じた形は知られていない（原子のまま）
            }
            // ζ(2k) = (−1)^{k+1} B_{2k} (2π)^{2k} / (2 (2k)!)
            let k = n as usize;
            let b = num::bernoulli(k);
            let budget = super::Budget::new(1_000_000);
            let fk = num::factorial(k as u64, &budget).ok()?;
            let c = b * Rat::from(Int::from(2).pow(k)) / Rat::from(Int::from(2) * fk);
            let c = if (k / 2) % 2 == 0 { -c } else { c };
            Some(mul(vec![Term::Num(c), pow(Term::Atom(Atom::Pi), Term::int(n))]))
        }
        Func::Digamma => digamma_rational(one(args)?.as_rat()?),
        _ => None,
    }
}

/// 常用対数の項（有理数の係数 × log10(正の有理数)）の和が、10 の整数乗の対数になるなら、その整数
pub fn fold_log10(items: &[(Rat, Rat)]) -> Option<Rat> {
    // Σ c_i log10(a_i) = log10(∏ a_i^{c_i})。c_i は整数のときだけ
    let mut prod = Rat::ONE;
    for (c, a) in items {
        let k = num::as_int(c)?;
        let k = i64::try_from(&k).ok()?;
        if k.abs() > 64 {
            return None;
        }
        prod *= a.pow(k as isize);
    }
    // prod = 10^m か
    let ten = Rat::from(Int::from(10));
    let mut m: i64 = 0;
    let mut r = prod;
    if num::sign(&r) <= 0 {
        return None;
    }
    while r >= ten && m < 400 {
        r /= &ten;
        m += 1;
    }
    while r < Rat::ONE && m > -400 {
        r *= &ten;
        m -= 1;
    }
    (r == Rat::ONE).then(|| Rat::from(Int::from(m)))
}

/// $\psi(b)$（$b$ は有理数で、分母が 1, 2, 3, 4, 6。ガウスの定理の値と、反射公式 $\psi(1-x) = \psi(x) + \pi\cot\pi x$、漸化式 $\psi(x+1) = \psi(x) + \frac1x$）
pub fn digamma_rational(b: &Rat) -> Option<Term> {
    if let Some(n) = num::as_int(b) {
        if n <= Int::ZERO {
            return None; // 極
        }
    }
    // b = n + f（0 < f ≤ 1）
    let n = (b.clone() - Rat::ONE).ceil();
    let f = b.clone() - Rat::from(n.clone());
    let n = i64::try_from(&n).ok()?;
    if n.abs() > 10_000 {
        return None;
    }
    let gamma = || Term::Atom(Atom::EulerGamma);
    let ln = |k: i64| super::term::call(Func::Ln, vec![Term::int(k)]);
    let pi = || Term::Atom(Atom::Pi);
    let base_small = |f: &Rat| -> Option<Term> {
        let d = u64::try_from(f.denominator()).ok()?;
        let nn = i64::try_from(f.numerator()).ok()?;
        Some(match (nn, d) {
            (1, 1) => neg(gamma()),
            (1, 2) => add(vec![neg(gamma()), mul(vec![Term::int(-2), ln(2)])]),
            // ψ(1/4) = −γ − π/2 − 3 ln 2
            (1, 4) => add(vec![neg(gamma()), mul(vec![Term::rat(-1, 2), pi()]), mul(vec![Term::int(-3), ln(2)])]),
            // ψ(1/3) = −γ − π/(2√3) − (3/2) ln 3
            (1, 3) => add(vec![neg(gamma()), mul(vec![Term::rat(-1, 6), s(3), pi()]), mul(vec![Term::rat(-3, 2), ln(3)])]),
            // ψ(1/6) = −γ − (√3/2) π − 2 ln 2 − (3/2) ln 3
            (1, 6) => add(vec![neg(gamma()), mul(vec![Term::rat(-1, 2), s(3), pi()]), mul(vec![Term::int(-2), ln(2)]), mul(vec![Term::rat(-3, 2), ln(3)])]),
            _ => return None,
        })
    };
    let half = Rat::from_parts(Int::ONE, Nat::from(2u8));
    let base = if f > half && f != Rat::ONE {
        // ψ(f) = ψ(1 − f) + π cot(π(1 − f))
        let g = Rat::ONE - f.clone();
        let cot = special(Func::Cot, &[mul(vec![Term::Num(g.clone()), pi()])])?;
        add(vec![base_small(&g)?, mul(vec![pi(), cot])])
    } else {
        base_small(&f)?
    };
    // 漸化式で f から b へ
    let mut extra = Rat::ZERO;
    if n >= 0 {
        for m in 0..n {
            extra += Rat::ONE / (f.clone() + Rat::from(Int::from(m)));
        }
    } else {
        for m in n..0 {
            extra -= Rat::ONE / (f.clone() + Rat::from(Int::from(m)));
        }
    }
    Some(add(vec![base, Term::Num(extra)]))
}

/// ディリクレのベータ関数 $\beta(s) = \sum_{k\ge0}\frac{(-1)^k}{(2k+1)^s}$（$s = 2$ は $G$、奇数はオイラー数で）
pub fn dirichlet_beta(s_: i64) -> Option<Term> {
    if s_ == 2 {
        return Some(Term::Atom(Atom::Catalan));
    }
    if s_ < 1 || s_ % 2 == 0 || s_ > 9 {
        return None;
    }
    // β(2m+1) = (−1)^m E_{2m} π^{2m+1} / (4^{m+1} (2m)!)
    let m = (s_ - 1) / 2;
    let euler = [1i64, -1, 5, -61, 1385][m as usize];
    let mut fact = Int::ONE;
    for j in 2..=2 * m {
        fact *= Int::from(j);
    }
    let sign = if m % 2 == 0 { 1 } else { -1 };
    let c = Rat::from_parts_signed(Int::from(sign * euler), Int::from(4).pow((m + 1) as usize) * fact);
    Some(mul(vec![Term::Num(c), pow(Term::Atom(Atom::Pi), Term::int(s_))]))
}

/// フルヴィッツのゼータ $\zeta(s, b) = \sum_{k\ge0}\frac{1}{(k+b)^s}$（$s \ge 2$ の整数、$b > 0$ の有理数で、分母が 1, 2, 4）
pub fn hurwitz(s_: i64, b: &Rat) -> Option<Term> {
    if s_ < 2 || num::sign(b) <= 0 {
        return None;
    }
    let n = (b.clone() - Rat::ONE).ceil();
    let f = b.clone() - Rat::from(n.clone());
    let n = i64::try_from(&n).ok()?;
    if n > 100_000 {
        return None;
    }
    let zeta = super::term::call(Func::Zeta, vec![Term::int(s_)]);
    let two_s = Rat::from(Int::from(2).pow(s_ as usize));
    let four_s = Rat::from(Int::from(4).pow(s_ as usize));
    let d = u64::try_from(f.denominator()).ok()?;
    let nn = i64::try_from(f.numerator()).ok()?;
    let base = match (nn, d) {
        (1, 1) => zeta,
        (1, 2) => mul(vec![Term::Num(two_s - Rat::ONE), zeta]),
        (1, 4) | (3, 4) => {
            let beta = dirichlet_beta(s_)?;
            let sym = mul(vec![Term::Num((four_s.clone() - two_s) * Rat::from_parts(Int::ONE, Nat::from(2u8))), zeta]);
            let anti = mul(vec![Term::Num(four_s * Rat::from_parts(Int::from(if nn == 1 { 1 } else { -1 }), Nat::from(2u8))), beta]);
            add(vec![sym, anti])
        }
        _ => return None,
    };
    // ζ(s, f + n) = ζ(s, f) − Σ_{m=0}^{n−1} (f + m)^{−s}
    let mut head = Rat::ZERO;
    for m in 0..n.max(0) {
        head += (f.clone() + Rat::from(Int::from(m))).pow(-(s_ as isize));
    }
    Some(add(vec![base, Term::Num(-head)]))
}

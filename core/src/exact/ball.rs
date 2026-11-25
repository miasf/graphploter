//! 式の高い精度の値（誤差の幅つき）と 0 の判定（計画 `docs/plan-exact-core.md` の EX3）。
//!
//! 値は複素数のボール: 中心 `re + i·im`（dashu の `FBig`。正しく丸めた演算）と、半径 `rad`（真の値は中心から `rad` 以内）。
//! 半径は f64 では小さい値（$2^{-1024}$ など）があふれるので、仮数と 2 の指数の組 [`Mag`] で、いつも上向きに丸めて持つ。
//!
//! - 四則: 半径を伝える式（積なら $|a|r_b + |b|r_a + r_ar_b$）に、丸めの誤差（結果の大きさ × $2^{1-p}$）を足す
//! - 関数: 引数の半径を、導関数の大きさの上限で伝える（$\exp$ は $|e^z|(e^r - 1)$、$\ln$ は $\frac{r}{|z| - r}$、$\sin$ は実数なら $r$）
//! - 分岐のある関数（$\ln$・べき）は、ボールが分岐の切れ目（負の実軸）にかかったら「決まらない」
//! - 定数（$\gamma$・$G$・$\zeta(n)$・$\Gamma$・$\psi$）は [`consts`](super::consts) の方法で、誤差が $2^{-p}$ より小さくなるように計算する

use super::num::{Int, Rat};
use super::term::{Atom, Term};
use crate::functions::Func;
use dashu::base::{Abs, BitTest, Sign};
use dashu::float::round::mode::HalfEven;
use dashu::float::FBig;
use std::cell::RefCell;
use std::collections::HashMap;

pub type F = FBig<HalfEven, 2>;

/// 上向きに丸めた正の大きさ $m \cdot 2^e$（$0.5 \le m < 1$、または 0）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mag {
    m: f64,
    e: i64,
}

const UP: f64 = 1.0 + 4.0 * f64::EPSILON;

impl Mag {
    pub const ZERO: Mag = Mag { m: 0.0, e: 0 };

    fn norm(m: f64, e: i64) -> Mag {
        if m == 0.0 || !m.is_finite() {
            return if m.is_finite() { Mag::ZERO } else { Mag { m: 0.75, e: i64::MAX / 4 } };
        }
        let (fm, fe) = frexp(m);
        Mag { m: fm * UP, e: e + fe }
    }

    pub fn from_f64(x: f64) -> Mag {
        Mag::norm(x.abs(), 0)
    }

    /// $2^k$
    pub fn pow2(k: i64) -> Mag {
        Mag { m: 0.5, e: k + 1 }
    }

    pub fn is_zero(&self) -> bool {
        self.m == 0.0
    }

    pub fn add(self, o: Mag) -> Mag {
        if self.is_zero() {
            return o;
        }
        if o.is_zero() {
            return self;
        }
        let (a, b) = if self.e >= o.e { (self, o) } else { (o, self) };
        let d = a.e - b.e;
        let bm = if d > 1100 { f64::MIN_POSITIVE } else { b.m * 2f64.powi(-(d as i32)) };
        Mag::norm(a.m + bm, a.e)
    }

    pub fn mul(self, o: Mag) -> Mag {
        if self.is_zero() || o.is_zero() {
            return Mag::ZERO;
        }
        Mag::norm(self.m * o.m, self.e + o.e)
    }

    pub fn scale(self, k: f64) -> Mag {
        self.mul(Mag::from_f64(k))
    }

    /// 下向きの比べ: self < o
    pub fn lt(&self, o: &Mag) -> bool {
        if self.is_zero() {
            return !o.is_zero();
        }
        if o.is_zero() {
            return false;
        }
        (self.e, self.m) < (o.e, o.m)
    }

    /// f64 にする（あふれたら ∞、小さすぎたら 0）
    pub fn to_f64(&self) -> f64 {
        if self.is_zero() {
            0.0
        } else if self.e > 1100 {
            f64::INFINITY
        } else if self.e < -1100 {
            0.0
        } else {
            self.m * 2f64.powi(self.e as i32)
        }
    }

    /// $\log_2$ の大きさ（おおよそ）
    pub fn log2(&self) -> f64 {
        if self.is_zero() {
            f64::NEG_INFINITY
        } else {
            self.e as f64 + self.m.log2()
        }
    }
}

fn frexp(x: f64) -> (f64, i64) {
    if x == 0.0 {
        return (0.0, 0);
    }
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i64;
    if exp == 0 {
        // 非正規化数
        let (m, e) = frexp(x * 2f64.powi(64));
        return (m, e - 64);
    }
    let m = f64::from_bits((bits & !(0x7ff << 52)) | (1022 << 52));
    (m, exp - 1022)
}

/// FBig の大きさ（上向き）
pub fn mag_of(x: &F) -> Mag {
    let r = x.repr();
    let s = r.significand();
    if s.is_zero() {
        return Mag::ZERO;
    }
    let bits = s.clone().abs().bit_len() as i64;
    let shift = (bits - 60).max(0);
    let top: Int = Int::from(s.clone().abs()) >> shift as usize;
    let topf = top.to_f64().value();
    Mag::norm(topf + 1.0, r.exponent() as i64 + shift)
}

/// 丸めの誤差の上限（|x| × 2^{1−p}）
fn round_err(x: &F, p: usize) -> Mag {
    mag_of(x).mul(Mag::pow2(1 - p as i64))
}

pub fn fl_int(n: &Int, p: usize) -> F {
    F::from(n.clone()).with_precision(p).value()
}

pub fn fl_rat(q: &Rat, p: usize) -> F {
    let n = fl_int(q.numerator(), p);
    let d = fl_int(&Int::from(q.denominator().clone()), p);
    n / d
}

fn zero(p: usize) -> F {
    F::ZERO.with_precision(p).value()
}

/// 決まらない理由
#[derive(Debug, Clone, PartialEq)]
pub enum Undecided {
    /// 分岐の切れ目にかかった
    Branch,
    /// 精度が足りない（0 で割りそう・半径が大きすぎる）
    Precision,
    /// 値を計算できない式（文字・まだ扱えない関数）
    Domain(String),
}

/// 複素数のボール
#[derive(Debug, Clone)]
pub struct Ball {
    pub re: F,
    pub im: F,
    pub rad: Mag,
    p: usize,
}

impl Ball {
    pub fn exact_real(x: F, p: usize) -> Ball {
        Ball { re: x, im: zero(p), rad: Mag::ZERO, p }
    }

    pub fn real(x: F, rad: Mag, p: usize) -> Ball {
        Ball { re: x, im: zero(p), rad, p }
    }

    fn of_rat(q: &Rat, p: usize) -> Ball {
        let x = fl_rat(q, p);
        let err = if *q.denominator() == dashu::integer::UBig::ONE && q.numerator().clone().abs().bit_len() <= p { Mag::ZERO } else { round_err(&x, p).scale(2.0) };
        Ball::real(x, err, p)
    }

    /// 中心の大きさの上限 |re| + |im|
    pub fn mag(&self) -> Mag {
        mag_of(&self.re).add(mag_of(&self.im))
    }

    /// 中心の大きさの下限（おおよそ、半径を引く前）
    fn mag_lower(&self) -> f64 {
        let a = self.re.to_f64().value().abs();
        let b = self.im.to_f64().value().abs();
        a.max(b)
    }

    pub fn is_real(&self) -> bool {
        self.im.repr().significand().is_zero()
    }

    /// 0 を含むか
    pub fn contains_zero(&self) -> bool {
        // |中心| ≤ rad か（|中心| ≥ max(|re|, |im|) で下から比べる）
        let mr = mag_of(&self.re);
        let mi = mag_of(&self.im);
        let lower = if mr.lt(&mi) { mi } else { mr };
        // lower は上向きの値なので、1 段下げて下限として使う
        let lower = lower.scale(0.5);
        !self.rad.lt(&lower)
    }

    pub fn to_f64(&self) -> (f64, f64) {
        (self.re.to_f64().value(), self.im.to_f64().value())
    }

    /// 相対の誤差（log2）。値が 0 に近ければ半径そのもの
    pub fn rel_log2(&self) -> f64 {
        let m = self.mag();
        if m.is_zero() {
            return self.rad.log2();
        }
        self.rad.log2() - m.log2()
    }
}

pub fn add(a: &Ball, b: &Ball) -> Ball {
    let p = a.p.max(b.p);
    let re = &a.re + &b.re;
    let im = &a.im + &b.im;
    let rad = a.rad.add(b.rad).add(round_err(&re, p)).add(round_err(&im, p));
    Ball { re, im, rad, p }
}

pub fn neg(a: &Ball) -> Ball {
    Ball { re: -a.re.clone(), im: -a.im.clone(), rad: a.rad, p: a.p }
}

pub fn mul(a: &Ball, b: &Ball) -> Ball {
    let p = a.p.max(b.p);
    let re = &a.re * &b.re - &a.im * &b.im;
    let im = &a.re * &b.im + &a.im * &b.re;
    let rad = a.mag().mul(b.rad).add(b.mag().mul(a.rad)).add(a.rad.mul(b.rad)).add(a.mag().mul(b.mag()).mul(Mag::pow2(3 - p as i64)));
    Ball { re, im, rad, p }
}

pub fn inv(a: &Ball) -> Result<Ball, Undecided> {
    let p = a.p;
    if a.contains_zero() {
        return Err(Undecided::Precision);
    }
    let n2 = &a.re * &a.re + &a.im * &a.im;
    let re = &a.re / &n2;
    let im = -(&a.im / &n2);
    // |1/(m+δ) − 1/m| ≤ r / (|m|(|m| − r))。|m| ≥ max(|re|, |im|)
    let lower = a.mag_lower();
    let r = a.rad.to_f64();
    let rad = if a.rad.is_zero() {
        Mag::ZERO
    } else if lower > 0.0 && lower.is_finite() && r.is_finite() && r < lower * 0.5 {
        Mag::from_f64(2.0 * r / (lower * lower))
    } else {
        // 小さい値・大きい値は Mag で: r / (lower²/2)
        let low = mag_of(&a.re).add(mag_of(&a.im)).scale(0.35);
        if !a.rad.lt(&low) {
            return Err(Undecided::Precision);
        }
        let inv_low2 = Mag { m: 0.5, e: 1 - 2 * low.e + 2 };
        a.rad.mul(inv_low2)
    };
    let back = Ball { re: re.clone(), im: im.clone(), rad: Mag::ZERO, p };
    let rounding = back.mag().mul(Mag::pow2(4 - p as i64));
    Ok(Ball { re, im, rad: rad.add(rounding), p })
}

/// 整数乗（2 進の繰り返し 2 乗）
pub fn powi(a: &Ball, k: i64) -> Result<Ball, Undecided> {
    if k == 0 {
        return Ok(Ball::exact_real(fl_int(&Int::ONE, a.p), a.p));
    }
    let mut base = if k < 0 { inv(a)? } else { a.clone() };
    let mut n = k.unsigned_abs();
    let mut acc: Option<Ball> = None;
    while n > 0 {
        if n & 1 == 1 {
            acc = Some(match acc {
                None => base.clone(),
                Some(x) => mul(&x, &base),
            });
        }
        n >>= 1;
        if n > 0 {
            base = mul(&base, &base);
        }
    }
    Ok(acc.unwrap())
}

/// $e^z$
pub fn exp(z: &Ball) -> Result<Ball, Undecided> {
    let p = z.p;
    let ea = z.re.exp();
    let (re, im) = if z.is_real() {
        (ea.clone(), zero(p))
    } else {
        let (s, c) = (z.im.sin(), z.im.cos());
        (&ea * c, &ea * s)
    };
    let r = z.rad.to_f64();
    if !(r < 1.0) && !z.rad.is_zero() {
        return Err(Undecided::Precision);
    }
    // |e^{z+δ} − e^z| ≤ |e^z| (e^{|δ|} − 1) ≤ |e^z| · |δ| · e^{|δ|}
    let ez = mag_of(&ea);
    let rad = ez.mul(z.rad).scale(std::f64::consts::E.powf(r.min(1.0)) * 1.01).add(ez.mul(Mag::pow2(4 - p as i64)));
    Ok(Ball { re, im, rad, p })
}

/// 主値の $\ln z$（負の実軸にかかったら決まらない）
pub fn ln(z: &Ball) -> Result<Ball, Undecided> {
    let p = z.p;
    if z.contains_zero() {
        return Err(Undecided::Precision);
    }
    let lower = z.mag_lower();
    if z.is_real() && z.re.sign() == Sign::Positive {
        let x = z.re.ln();
        let r = z.rad.to_f64();
        let rad = if z.rad.is_zero() { Mag::ZERO } else if lower.is_finite() && lower > 0.0 && r < lower * 0.5 { Mag::from_f64(2.0 * r / lower) } else { return Err(Undecided::Precision) };
        let rad = rad.add(round_err(&x, p).scale(2.0)).add(Mag::pow2(2 - p as i64));
        return Ok(Ball::real(x, rad, p));
    }
    // 負の実軸の近く: 実部が負で、虚部が半径の中で 0 になりうるなら、切れ目にかかる（ちょうど負の実数なら主値 ln|x| + iπ）
    if z.re.sign() == Sign::Negative {
        let exact_real = z.is_real() && z.rad.is_zero();
        if !exact_real && !z.rad.lt(&mag_of(&z.im).scale(0.5)) {
            return Err(Undecided::Branch);
        }
    }
    let n2 = &z.re * &z.re + &z.im * &z.im;
    let half = fl_rat(&Rat::from_parts(Int::ONE, dashu::integer::UBig::from(2u8)), p);
    let re = n2.ln() * half;
    let im = z.im.atan2(&z.re);
    let r = z.rad.to_f64();
    if !(lower.is_finite() && lower > 0.0 && r < lower * 0.5) {
        return Err(Undecided::Precision);
    }
    let rad = Mag::from_f64(2.0 * r / lower).add(Mag::pow2(4 - p as i64));
    Ok(Ball { re, im, rad, p })
}

/// 実数の関数で、導関数の大きさが 1 以下のもの（sin・cos・atan）
fn real_lipschitz1(z: &Ball, f: impl Fn(&F) -> F) -> Result<Ball, Undecided> {
    if !z.is_real() {
        return Err(Undecided::Domain("複素数の引数".into()));
    }
    let x = f(&z.re);
    let rad = z.rad.add(round_err(&x, z.p).scale(2.0)).add(Mag::pow2(2 - z.p as i64));
    Ok(Ball::real(x, rad, z.p))
}

/// 中心だけを正しく丸めた値で置き換えたボール（半径は導関数の上限 `d` × 引数の半径）
fn real_with(z: &Ball, x: F, d: f64) -> Ball {
    let rad = z.rad.scale(d).add(round_err(&x, z.p).scale(2.0)).add(Mag::pow2(2 - z.p as i64));
    Ball::real(x, rad, z.p)
}

thread_local! {
    static CONSTS: RefCell<HashMap<(String, usize), F>> = RefCell::new(HashMap::new());
}

/// 覚えておく定数の値
fn cached(key: &str, p: usize, f: impl FnOnce() -> F) -> F {
    if let Some(v) = CONSTS.with(|c| c.borrow().get(&(key.to_string(), p)).cloned()) {
        return v;
    }
    let v = f();
    CONSTS.with(|c| c.borrow_mut().insert((key.to_string(), p), v.clone()));
    v
}

fn constant(a: Atom, p: usize) -> Ball {
    match a {
        Atom::Pi => Ball::real(cached("pi", p, || F::pi(p)), Mag::pow2(3 - p as i64), p),
        Atom::E => Ball::real(cached("e", p, || F::e(p)), Mag::pow2(3 - p as i64), p),
        Atom::I => Ball { re: zero(p), im: fl_int(&Int::ONE, p), rad: Mag::ZERO, p },
        Atom::EulerGamma => Ball::real(cached("gamma", p, || super::consts::euler_gamma(p)), Mag::pow2(4 - p as i64), p),
        Atom::Catalan => Ball::real(cached("catalan", p, || super::consts::catalan(p)), Mag::pow2(4 - p as i64), p),
    }
}

/// 式の値を、作業の精度 `p` ビットで計算する
pub fn eval(t: &Term, p: usize) -> Result<Ball, Undecided> {
    match t {
        Term::Num(q) => Ok(Ball::of_rat(q, p)),
        Term::Atom(a) => Ok(constant(*a, p)),
        Term::Sym(s) => Err(Undecided::Domain(format!("文字 {s}"))),
        Term::Add(v) => {
            let mut acc = eval(&v[0], p)?;
            for x in &v[1..] {
                acc = add(&acc, &eval(x, p)?);
            }
            Ok(acc)
        }
        Term::Mul(v) => {
            let mut acc = eval(&v[0], p)?;
            for x in &v[1..] {
                acc = mul(&acc, &eval(x, p)?);
            }
            Ok(acc)
        }
        Term::Pow(b, e) => pow(b, e, p),
        Term::Fn(f, args) => call(*f, args, p),
        Term::Tuple(_) | Term::List(_) => Err(Undecided::Domain("組・リスト".into())),
    }
}

fn pow(b: &Term, e: &Term, p: usize) -> Result<Ball, Undecided> {
    if let Term::Num(q) = e {
        if *q.denominator() == dashu::integer::UBig::ONE {
            if let Ok(k) = i64::try_from(q.numerator()) {
                if k.unsigned_abs() <= 1 << 20 {
                    return powi(&eval(b, p)?, k);
                }
            }
        }
        // 正の有理数の底の有理数乗は n 乗根で（正しく丸める）
        if let Term::Num(bq) = b {
            if super::num::sign(bq) > 0 {
                if let (Ok(m), Ok(n)) = (i64::try_from(q.numerator()), usize::try_from(q.denominator())) {
                    if n <= 4096 && m.unsigned_abs() <= 1 << 20 {
                        let base = fl_rat(bq, p + 8);
                        let root = base.nth_root(n).with_precision(p).value();
                        let rb = Ball::real(root.clone(), round_err(&root, p).scale(4.0), p);
                        return powi(&rb, m);
                    }
                }
            }
        }
    }
    // 一般: b^e = exp(e ln b)（主値）
    let lb = ln(&eval(b, p)?)?;
    exp(&mul(&eval(e, p)?, &lb))
}

fn call(f: Func, args: &[Term], p: usize) -> Result<Ball, Undecided> {
    let one = || -> Result<Ball, Undecided> {
        match args {
            [a] => eval(a, p),
            _ => Err(Undecided::Domain(format!("{f:?} の引数の数"))),
        }
    };
    match f {
        Func::Exp => exp(&one()?),
        Func::Ln => ln(&one()?),
        Func::Log10 => {
            let l = ln(&one()?)?;
            let l10 = ln(&Ball::exact_real(fl_int(&Int::from(10), p), p))?;
            Ok(mul(&l, &inv(&l10)?))
        }
        Func::Sqrt => pow(&args[0], &Term::rat(1, 2), p),
        Func::Sin | Func::Cos => {
            let z = one()?;
            if z.is_real() {
                return real_lipschitz1(&z, |x| if f == Func::Sin { x.sin() } else { x.cos() });
            }
            // sin z = (e^{iz} − e^{−iz}) / 2i、cos z = (e^{iz} + e^{−iz}) / 2
            let iz = mul(&z, &constant(Atom::I, p));
            let a = exp(&iz)?;
            let b = exp(&neg(&iz))?;
            let half = Ball::of_rat(&Rat::from_parts(Int::ONE, dashu::integer::UBig::from(2u8)), p);
            if f == Func::Cos {
                Ok(mul(&add(&a, &b), &half))
            } else {
                let minus_half_i = Ball { re: zero(p), im: -fl_rat(&Rat::from_parts(Int::ONE, dashu::integer::UBig::from(2u8)), p), rad: Mag::ZERO, p };
                Ok(mul(&add(&a, &neg(&b)), &minus_half_i))
            }
        }
        Func::Tan => {
            let s = call(Func::Sin, args, p)?;
            let c = call(Func::Cos, args, p)?;
            Ok(mul(&s, &inv(&c)?))
        }
        Func::Atan => real_lipschitz1(&one()?, |x| x.atan()),
        Func::Asin | Func::Acos => {
            let z = one()?;
            if !z.is_real() {
                return Err(Undecided::Domain("複素数の引数".into()));
            }
            let x = z.re.to_f64().value().abs();
            let r = z.rad.to_f64();
            if !(x + r < 1.0) {
                return Err(Undecided::Domain("|x| ≥ 1 の逆正弦".into()));
            }
            let d = 1.0 / (1.0 - (x + r) * (x + r)).sqrt() * 1.01;
            let v = if f == Func::Asin { z.re.asin() } else { z.re.acos() };
            Ok(real_with(&z, v, d))
        }
        Func::Sinh | Func::Cosh | Func::Tanh => {
            let z = one()?;
            let a = exp(&z)?;
            let b = exp(&neg(&z))?;
            let half = Ball::of_rat(&Rat::from_parts(Int::ONE, dashu::integer::UBig::from(2u8)), p);
            match f {
                Func::Sinh => Ok(mul(&add(&a, &neg(&b)), &half)),
                Func::Cosh => Ok(mul(&add(&a, &b), &half)),
                _ => Ok(mul(&add(&a, &neg(&b)), &inv(&add(&a, &b))?)),
            }
        }
        Func::Abs => {
            let z = one()?;
            let n = (&z.re * &z.re + &z.im * &z.im).sqrt();
            let rad = z.rad.add(round_err(&n, p).scale(2.0));
            Ok(Ball::real(n, rad, p))
        }
        Func::Gamma | Func::LogGamma | Func::Digamma | Func::Zeta => {
            let [Term::Num(q)] = args else { return Err(Undecided::Domain(format!("{f:?} の引数が数でない"))) };
            let key = format!("{f:?}({q})");
            let v = match f {
                Func::Gamma => cached(&key, p, || super::consts::gamma(q, p)),
                Func::LogGamma => cached(&key, p, || super::consts::ln_gamma(q, p)),
                Func::Digamma => cached(&key, p, || super::consts::digamma(q, p)),
                _ => {
                    let n = super::num::as_int(q).and_then(|n| i64::try_from(&n).ok()).filter(|n| *n >= 2).ok_or_else(|| Undecided::Domain("ζ の引数".into()))?;
                    cached(&key, p, || super::consts::zeta(n as u64, p))
                }
            };
            if !v.repr().is_finite() {
                return Err(Undecided::Domain(format!("{f:?} の極")));
            }
            Ok(Ball::real(v.clone(), mag_of(&v).mul(Mag::pow2(6 - p as i64)).add(Mag::pow2(6 - p as i64)), p))
        }
        _ => Err(Undecided::Domain(format!("関数 {f:?} の高い精度の値はまだ計算できません"))),
    }
}

/// 相対の精度が `bits` ビットに届くまで、作業の精度を 2 倍にして計算する（最大 4096 ビット）
pub fn value(t: &Term, bits: usize) -> Result<Ball, Undecided> {
    let mut p = bits + 32;
    loop {
        let b = eval(t, p)?;
        if b.rel_log2() <= -(bits as f64) || b.rad.is_zero() {
            return Ok(b);
        }
        if p >= 4096 {
            return Ok(b);
        }
        p *= 2;
    }
}

/// 0 かどうか。構造で 0 なら真。256 ビットと 1024 ビットの両方で 0 を含めば真（数値で 0 とみなす。D12）。
/// どちらかで 0 を含まなければ偽。値が計算できなければ None
pub fn is_zero(t: &Term, rules: &mut Vec<&'static str>) -> Option<bool> {
    if t.is_zero() {
        return Some(true);
    }
    for p in [256, 1024] {
        let b = eval(t, p + 32).ok()?;
        if !b.contains_zero() {
            return Some(false);
        }
    }
    rules.push("数値で 0 とみなした");
    Some(true)
}

/// 10 進の文字列（有効 `digits` 桁）
pub fn to_decimal(x: &F, digits: usize) -> String {
    let d = x.clone().with_base_and_precision::<10>(digits).value();
    format!("{d}")
}

//! 区間演算: 箱 B（変数ごとの区間）の上で、式の値がどの範囲に入るかを求める。
//!
//! 結果が [a, b] で 0 を含まなければ、その箱の中に F = 0 の曲面は無いと言える（陰関数で、曲面の無い所を飛ばすのに使う）。
//! 上下が分からない（定義されない点・極を含むかもしれない）ときは [`Range::Unknown`] を返す。呼び出し側は
//! 「曲面があるかもしれない」と扱う。
//!
//! 丸めの向きは制御しない。代わりに、命令ごとの結果を外側に 10⁻¹² (|x| + 1) だけ広げる。
//! 同じ変数が何度も出る式（x · x）は区間が広がる（実際より広い区間になるだけで、間違いにはならない）。

use std::f64::consts::{FRAC_PI_2, PI};

/// 区間 [lo, hi]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

/// 区間演算の結果
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Range {
    /// 値はこの区間に入る
    Known(Interval),
    /// 上下が分からない（実数に収まらないかもしれない・極を含むかもしれない）
    Unknown,
}

impl Range {
    /// 0 を含むかもしれないか（`Unknown` なら true）
    pub fn may_contain_zero(&self) -> bool {
        match self {
            Range::Known(i) => i.lo <= 0.0 && 0.0 <= i.hi,
            Range::Unknown => true,
        }
    }
}

/// 区間の計算の途中結果。`None` は「分からない」
pub(crate) type Iv = Option<Interval>;

impl Interval {
    pub fn new(lo: f64, hi: f64) -> Interval {
        Interval { lo, hi }
    }

    pub fn point(x: f64) -> Interval {
        Interval { lo: x, hi: x }
    }

    /// x が区間に入るか
    pub fn contains(&self, x: f64) -> bool {
        self.lo <= x && x <= self.hi
    }

    /// 丸め誤差の分だけ外側に広げる。有限でなければ「分からない」
    pub(crate) fn widen(self) -> Iv {
        if !(self.lo.is_finite() && self.hi.is_finite()) || self.lo > self.hi {
            return None;
        }
        let pad = |x: f64| 1e-12 * (x.abs() + 1.0);
        Some(Interval { lo: self.lo - pad(self.lo), hi: self.hi + pad(self.hi) })
    }
}

/// 単調に増える関数の値の区間
fn increasing(a: Interval, f: impl Fn(f64) -> f64) -> Iv {
    Interval::new(f(a.lo), f(a.hi)).widen()
}

/// 単調に減る関数の値の区間
fn decreasing(a: Interval, f: impl Fn(f64) -> f64) -> Iv {
    Interval::new(f(a.hi), f(a.lo)).widen()
}

pub(crate) fn neg(a: Interval) -> Iv {
    Interval::new(-a.hi, -a.lo).widen()
}

pub(crate) fn add(a: Interval, b: Interval) -> Iv {
    Interval::new(a.lo + b.lo, a.hi + b.hi).widen()
}

pub(crate) fn sub(a: Interval, b: Interval) -> Iv {
    Interval::new(a.lo - b.hi, a.hi - b.lo).widen()
}

pub(crate) fn mul(a: Interval, b: Interval) -> Iv {
    let p = [a.lo * b.lo, a.lo * b.hi, a.hi * b.lo, a.hi * b.hi];
    Interval::new(p.iter().copied().fold(f64::INFINITY, f64::min), p.iter().copied().fold(f64::NEG_INFINITY, f64::max)).widen()
}

/// 1 / b。分母が 0 を含めば「分からない」
fn recip(b: Interval) -> Iv {
    if b.contains(0.0) {
        return None;
    }
    Interval::new(1.0 / b.hi, 1.0 / b.lo).widen()
}

pub(crate) fn div(a: Interval, b: Interval) -> Iv {
    mul(a, recip(b)?)
}

/// 整数乗 aᵉ。偶数乗は 0 をまたぐとき下が 0。負の指数は 1 / a^|e|
pub(crate) fn powi(a: Interval, e: i32) -> Iv {
    if e == 0 {
        return Some(Interval::point(1.0));
    }
    if e < 0 {
        return recip(powi(a, -e)?);
    }
    if e % 2 == 1 {
        return increasing(a, |x| x.powi(e));
    }
    let (l, h) = (a.lo.powi(e), a.hi.powi(e));
    nonneg(if a.contains(0.0) { Interval::new(0.0, l.max(h)).widen() } else { Interval::new(l.min(h), l.max(h)).widen() })
}

/// 値が 0 以上と分かっている区間で、丸めの分だけ広げて負になった下の端を 0 に戻す
/// （sqrt((x²)) などが、0 をまたぐ箱で「分からない」にならないように）
fn nonneg(a: Iv) -> Iv {
    a.map(|a| Interval::new(a.lo.max(0.0), a.hi))
}

/// 実数乗 a^b（計算エンジンと同じ場合分け: 整数乗、または 0 以上の数の実数乗）
pub(crate) fn pow(a: Interval, b: Interval) -> Iv {
    if b.lo == b.hi {
        let e = b.lo;
        if e.fract() == 0.0 && e.abs() <= 1024.0 {
            return powi(a, e as i32);
        }
        // 0 以上の数の実数乗は単調（指数が負なら 0 を含んではいけない）
        if a.lo < 0.0 || (e < 0.0 && a.lo <= 0.0) {
            return None;
        }
        return if e > 0.0 { increasing(a, |x| x.powf(e)) } else { decreasing(a, |x| x.powf(e)) };
    }
    // 指数も動くとき: 底が正なら a^b = exp(b ln a)
    if a.lo <= 0.0 {
        return None;
    }
    increasing(mul(b, increasing(a, f64::ln)?)?, f64::exp)
}

/// sin: 区間に極値（π/2 + 2kπ で 1、−π/2 + 2kπ で −1）が入るかで上下を決める
pub(crate) fn sin(a: Interval) -> Iv {
    if a.hi - a.lo >= 2.0 * PI {
        return Some(Interval::new(-1.0, 1.0));
    }
    // 区間 [lo, hi] に c + 2kπ の形の点が入るか
    let has = |c: f64| ((a.lo - c) / (2.0 * PI)).ceil() * 2.0 * PI + c <= a.hi;
    let (s0, s1) = (a.lo.sin(), a.hi.sin());
    let hi = if has(FRAC_PI_2) { 1.0 } else { s0.max(s1) };
    let lo = if has(-FRAC_PI_2) { -1.0 } else { s0.min(s1) };
    Interval::new(lo, hi).widen()
}

/// cos(x) = sin(x + π/2)
pub(crate) fn cos(a: Interval) -> Iv {
    if a.hi - a.lo >= 2.0 * PI {
        return Some(Interval::new(-1.0, 1.0));
    }
    let has = |c: f64| ((a.lo - c) / (2.0 * PI)).ceil() * 2.0 * PI + c <= a.hi;
    let (c0, c1) = (a.lo.cos(), a.hi.cos());
    let hi = if has(0.0) { 1.0 } else { c0.max(c1) };
    let lo = if has(PI) { -1.0 } else { c0.min(c1) };
    Interval::new(lo, hi).widen()
}

/// 区間に c + kπ の形の点（tan などの極）が入るか。端のすぐ近くも「入る」とみなす
fn has_pole(a: Interval, c: f64) -> bool {
    let k = ((a.lo - c) / PI).ceil();
    k * PI + c <= a.hi + 1e-9 * (a.hi.abs() + 1.0) || (k - 1.0) * PI + c >= a.lo - 1e-9 * (a.lo.abs() + 1.0)
}

/// tan: 極（π/2 + kπ）が入れば分からない。入らなければ単調に増える
pub(crate) fn tan(a: Interval) -> Iv {
    if a.hi - a.lo >= PI || has_pole(a, FRAC_PI_2) {
        return None;
    }
    increasing(a, f64::tan)
}

/// cot: 極（kπ）が入れば分からない。入らなければ単調に減る
pub(crate) fn cot(a: Interval) -> Iv {
    if a.hi - a.lo >= PI || has_pole(a, 0.0) {
        return None;
    }
    decreasing(a, |x| x.cos() / x.sin())
}

/// sec = 1 / cos、csc = 1 / sin
pub(crate) fn sec(a: Interval) -> Iv {
    recip(cos(a)?)
}

pub(crate) fn csc(a: Interval) -> Iv {
    recip(sin(a)?)
}

/// 0 で最小になる偶関数（cosh, |x|）: 0 をまたぐかで分ける
fn even(a: Interval, f: impl Fn(f64) -> f64) -> Iv {
    let (l, h) = (f(a.lo), f(a.hi));
    if a.contains(0.0) {
        Interval::new(f(0.0), l.max(h)).widen()
    } else {
        Interval::new(l.min(h), l.max(h)).widen()
    }
}

pub(crate) fn exp(a: Interval) -> Iv {
    increasing(a, f64::exp)
}

pub(crate) fn sinh(a: Interval) -> Iv {
    increasing(a, f64::sinh)
}

pub(crate) fn tanh(a: Interval) -> Iv {
    increasing(a, f64::tanh)
}

pub(crate) fn atan(a: Interval) -> Iv {
    increasing(a, f64::atan)
}

pub(crate) fn asinh(a: Interval) -> Iv {
    increasing(a, f64::asinh)
}

pub(crate) fn cosh(a: Interval) -> Iv {
    even(a, f64::cosh)
}

pub(crate) fn abs(a: Interval) -> Iv {
    nonneg(even(a, f64::abs))
}

/// 丸め・符号（単調に増える。値は飛ぶが、両端の値の間に収まる）
pub(crate) fn floor(a: Interval) -> Iv {
    increasing(a, f64::floor)
}

pub(crate) fn ceil(a: Interval) -> Iv {
    increasing(a, f64::ceil)
}

pub(crate) fn round(a: Interval) -> Iv {
    increasing(a, f64::round)
}

pub(crate) fn sign(a: Interval) -> Iv {
    let s = |x: f64| if x == 0.0 { 0.0 } else { x.signum() };
    increasing(a, s)
}

/// 定義域が lo > 0（ln, log₁₀）
pub(crate) fn ln(a: Interval) -> Iv {
    if a.lo <= 0.0 {
        return None;
    }
    increasing(a, f64::ln)
}

pub(crate) fn log10(a: Interval) -> Iv {
    if a.lo <= 0.0 {
        return None;
    }
    increasing(a, f64::log10)
}

pub(crate) fn sqrt(a: Interval) -> Iv {
    // 丸めの分だけ広げて負になっただけの下の端（x² + y² で 0 をまたぐ箱など）は 0 とみなす
    // （そこで本当に負の値になっても、値は √(ほぼ 0) の実部 0 で、区間に入る）
    if a.lo < -1e-9 * (a.hi.abs() + 1.0) {
        return None;
    }
    increasing(Interval::new(a.lo.max(0.0), a.hi), f64::sqrt)
}

/// 定義域 [−1, 1]（はみ出せば分からない）
pub(crate) fn asin(a: Interval) -> Iv {
    if a.lo < -1.0 || a.hi > 1.0 {
        return None;
    }
    increasing(a, f64::asin)
}

pub(crate) fn acos(a: Interval) -> Iv {
    if a.lo < -1.0 || a.hi > 1.0 {
        return None;
    }
    decreasing(a, f64::acos)
}

/// 定義域 (−1, 1)（±1 では無限大になる）
pub(crate) fn atanh(a: Interval) -> Iv {
    if a.lo <= -1.0 || a.hi >= 1.0 {
        return None;
    }
    increasing(a, f64::atanh)
}

/// 定義域 [1, ∞)
pub(crate) fn acosh(a: Interval) -> Iv {
    if a.lo < 1.0 {
        return None;
    }
    increasing(a, f64::acosh)
}

/// sinc x = sin x / x の値は [−0.2173, 1] に入る
pub(crate) fn sinc(_: Interval) -> Iv {
    Some(Interval::new(-0.22, 1.0))
}

pub(crate) fn min(a: Interval, b: Interval) -> Iv {
    Interval::new(a.lo.min(b.lo), a.hi.min(b.hi)).widen()
}

pub(crate) fn max(a: Interval, b: Interval) -> Iv {
    Interval::new(a.lo.max(b.lo), a.hi.max(b.hi)).widen()
}

// ---------------------------------------------------------------------------
// 飾り付きの区間（計画 12 章の G1。IEEE Std 1788-2015 の decoration）
// ---------------------------------------------------------------------------

/// 区間の飾り。弱い順に `Trv` < `Def` < `Dac` < `Com`（IEEE 1788 と同じ順）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dec {
    /// 何も言えない（箱の一部で定義されない・極を含むかもしれない）
    Trv,
    /// 箱の全部で定義されているが、連続とは言えない（floor・mod の飛ぶ所を含む）
    Def,
    /// 箱の全部で定義されていて連続（値の範囲は有界とは限らない）
    Dac,
    /// 箱の全部で定義されていて連続で、値の範囲が有界
    Com,
}

/// 飾り付きの区間。`lo > hi` は空（箱のどこでも定義されない）
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecInterval {
    pub lo: f64,
    pub hi: f64,
    pub dec: Dec,
}

impl DecInterval {
    /// 区間 i と飾り d（有界でなければ Com を Dac に下げる）
    pub fn new(i: Interval, dec: Dec) -> DecInterval {
        let bounded = i.lo.is_finite() && i.hi.is_finite();
        DecInterval { lo: i.lo, hi: i.hi, dec: if dec == Dec::Com && !bounded { Dec::Dac } else { dec } }
    }

    /// 全部の実数
    pub fn entire(dec: Dec) -> DecInterval {
        DecInterval { lo: f64::NEG_INFINITY, hi: f64::INFINITY, dec: dec.min(Dec::Dac) }
    }

    /// 空（どこでも定義されない）
    pub fn empty() -> DecInterval {
        DecInterval { lo: f64::INFINITY, hi: f64::NEG_INFINITY, dec: Dec::Trv }
    }

    pub fn is_empty(&self) -> bool {
        !(self.lo <= self.hi)
    }

    pub fn interval(&self) -> Interval {
        Interval::new(self.lo, self.hi)
    }

    /// 0 を含むかもしれないか（空なら false）
    pub fn may_contain_zero(&self) -> bool {
        self.lo <= 0.0 && 0.0 <= self.hi
    }

    /// 上下が有限か
    pub fn bounded(&self) -> bool {
        self.lo.is_finite() && self.hi.is_finite()
    }

    /// 飾りを d 以下に下げる
    pub fn weaken(self, d: Dec) -> DecInterval {
        DecInterval { dec: self.dec.min(d), ..self }
    }
}

/// 今の区間演算の関数（`Iv`。None は「分からない」）の結果を飾り付きにする。None なら、全部の実数で飾り `on_none`
pub(crate) fn dec_of(r: Iv, dec: Dec, on_none: Dec) -> DecInterval {
    match r {
        Some(i) => DecInterval::new(i, dec),
        None => DecInterval::entire(on_none),
    }
}

/// 定義域 [lo, hi] の関数 f（単調に増える）を、定義域と重なる所だけで計算する。はみ出せば飾りは Trv、重ならなければ空
pub(crate) fn dec_domain(a: Interval, dom_lo: f64, dom_hi: f64, f: impl Fn(Interval) -> Iv) -> DecInterval {
    if a.hi < dom_lo || a.lo > dom_hi {
        return DecInterval::empty();
    }
    let inside = Interval::new(a.lo.max(dom_lo), a.hi.min(dom_hi));
    let partial = a.lo < dom_lo || a.hi > dom_hi;
    dec_of(f(inside), if partial { Dec::Trv } else { Dec::Com }, if partial { Dec::Trv } else { Dec::Dac })
}

/// 飛ぶ所がある関数（floor など）: 両端の値が同じなら Com、違えば Def
pub(crate) fn dec_step(a: Interval, f: impl Fn(f64) -> f64, r: Iv) -> DecInterval {
    let same = f(a.lo) == f(a.hi);
    dec_of(r, if same { Dec::Com } else { Dec::Def }, Dec::Def)
}

/// mod(a, b) = a − b⌊a/b⌋。b が 0 を含めば何も言えない。⌊a/b⌋ が箱で一定なら連続
pub(crate) fn dec_mod(a: Interval, b: Interval) -> DecInterval {
    if b.contains(0.0) {
        return DecInterval::entire(Dec::Trv);
    }
    let Some(q) = div(a, b) else { return DecInterval::entire(Dec::Trv) };
    let (k0, k1) = (q.lo.floor(), q.hi.floor());
    if k0 == k1 {
        // 一定の k で a − k b
        let r = mul(b, Interval::point(k0)).and_then(|kb| sub(a, kb));
        return dec_of(r, Dec::Com, Dec::Dac);
    }
    // 値は b と同じ符号の側で [0, |b|) に入る
    let i = if b.lo > 0.0 { Interval::new(0.0, b.hi) } else { Interval::new(b.lo, 0.0) };
    dec_of(i.widen(), Dec::Def, Dec::Def)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::Program;
    use crate::parser::parse;
    use num_complex::Complex64 as C;

    const XYZ: [&str; 3] = ["x", "y", "z"];

    fn prog(src: &str) -> Program {
        Program::compile(&parse(src).unwrap_or_else(|e| panic!("{src}: {e:?}")), &XYZ).unwrap_or_else(|e| panic!("{src}: {e}"))
    }

    /// 決まった種の乱数（0〜1）
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// 上の命令を全部1回は使う式
    const EXPRS: [&str; 34] = [
        "x^2+y^2+z^2-4",
        "sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)",
        "(sqrt(x^2+y^2)-3)^2+z^2-0.04",
        "z (x^2+y^2) - 1",
        "tan(x) - z",
        "1/x - z",
        "exp(-x^2) sin(5y) - z",
        "sec(x) - z",
        "csc(y) + x",
        "cot(x) - y",
        "asin(x/5) + acos(y/5) - z",
        "atan(x y) - z",
        "sinh(x) - cosh(y) + z",
        "tanh(x z) - y",
        "asinh(x) + acosh(abs(y) + 1) - z",
        "atanh(x/6) - z",
        "ln(x^2 + 1) - z",
        "log(abs(x) + 0.5) + y",
        "sqrt(abs(x y)) - z",
        "sinc(x) - y",
        "floor(x) + ceil(y) - z",
        "round(x z) - y",
        "sign(x) + sign(y - z)",
        "min(x, y) - max(y, z)",
        "x^3 - y^(-2) + z^0",
        "x^(1/3) - z",
        "abs(x)^1.5 - y",
        "(x + 2)^(-0.5) - z",
        "abs(x)^y - z",
        "-x / (y - 0.3) + z",
        "Re(x) + Im(y) + conj(z)",
        "cos(3x) cos(3y) - z",
        "sin(x)^2 + cos(y)^2 - 1 - z",
        "exp(x) - y z",
    ];

    #[test]
    fn interval_contains_point_values() {
        let mut rng = Lcg(12345);
        for src in EXPRS {
            let p = prog(src);
            assert!(p.supports_interval(), "{src}");
            let mut known = 0;
            for _ in 0..200 {
                // 中心は ±5、幅は 0〜2（細い箱から太い箱まで）
                let boxes: Vec<Interval> = (0..3)
                    .map(|_| {
                        let (c, w) = (10.0 * rng.next() - 5.0, 2.0 * rng.next() * rng.next());
                        Interval::new(c - w / 2.0, c + w / 2.0)
                    })
                    .collect();
                let range = p.eval_interval(&boxes);
                let Range::Known(r) = range else { continue };
                known += 1;
                // 箱の中の点（角も含める）
                let mut cols: [Vec<f64>; 3] = Default::default();
                for k in 0..50 {
                    for (ax, b) in boxes.iter().enumerate() {
                        let t = if k < 8 { ((k >> ax) & 1) as f64 } else { rng.next() };
                        cols[ax].push(b.lo + (b.hi - b.lo) * t);
                    }
                }
                let mut out = vec![C::new(0.0, 0.0); 50];
                p.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
                for (k, v) in out.iter().enumerate() {
                    let v = crate::eval::as_real(*v);
                    if v.is_finite() {
                        assert!(
                            r.contains(v),
                            "{src}: 点 ({}, {}, {}) の値 {v} が区間 [{}, {}] に入らない（箱 {boxes:?}）",
                            cols[0][k], cols[1][k], cols[2][k], r.lo, r.hi
                        );
                    }
                }
            }
            assert!(known > 0, "{src}: 区間が1回も求まらない");
        }
    }

    #[test]
    fn interval_is_tight_enough() {
        let p = prog("x^2+y^2+z^2-4");
        let b = |lo: f64, hi: f64| [Interval::new(lo, hi); 3];
        assert!(!p.eval_interval(&b(3.0, 4.0)).may_contain_zero(), "原点から離れた箱");
        assert!(p.eval_interval(&b(1.0, 1.5)).may_contain_zero(), "球の面をまたぐ箱");
        assert!(!p.eval_interval(&b(-0.5, 0.5)).may_contain_zero(), "球の中の箱");
        // 0 をまたぐ箱での sqrt(x² + y²)（丸めの分だけ広げて x² の下の端が負になると「分からない」になっていた）
        let t = prog("(sqrt(x^2+y^2)-3)^2+z^2-0.04");
        assert!(!t.eval_interval(&b(-0.2, 0.2)).may_contain_zero(), "トーラスの軸の上の箱");
        assert!(!prog("sqrt(abs(x)) + 1").eval_interval(&b(-0.2, 0.2)).may_contain_zero());
    }

    /// 飾り付きの区間の箱（中心は ±5、幅は 0〜2）
    fn random_boxes(rng: &mut Lcg) -> Vec<Interval> {
        (0..3)
            .map(|_| {
                let (c, w) = (10.0 * rng.next() - 5.0, 2.0 * rng.next() * rng.next());
                Interval::new(c - w / 2.0, c + w / 2.0)
            })
            .collect()
    }

    #[test]
    fn decorated_contains_point_values() {
        let mut rng = Lcg(777);
        let mut exprs: Vec<&str> = EXPRS.to_vec();
        exprs.extend(["mod(x y, z) - 1", "mod(x, 2) + y", "floor(x y) - z floor(z)", "x / (y z)", "sqrt(x) + ln(y) - z", "(x - 1)^(1.5) + asin(y/3)"]);
        let mut checked = 0;
        for src in exprs {
            let p = prog(src);
            for _ in 0..300 {
                let boxes = random_boxes(&mut rng);
                let d = p.eval_interval_dec(&boxes);
                // 今の区間演算が答える所では、同じ区間で、飾りは Trv ではない
                if let Range::Known(old) = p.eval_interval(&boxes) {
                    assert!(d.dec > Dec::Trv && d.lo == old.lo && d.hi == old.hi, "{src}: {d:?} と {old:?}（箱 {boxes:?}）");
                }
                let mut cols: [Vec<f64>; 3] = Default::default();
                for k in 0..40 {
                    for (ax, b) in boxes.iter().enumerate() {
                        let t = if k < 8 { ((k >> ax) & 1) as f64 } else { rng.next() };
                        cols[ax].push(b.lo + (b.hi - b.lo) * t);
                    }
                }
                let mut out = vec![C::new(0.0, 0.0); 40];
                p.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
                for (k, v) in out.iter().enumerate() {
                    let v = crate::eval::as_real(*v);
                    let at = (cols[0][k], cols[1][k], cols[2][k]);
                    if v.is_finite() {
                        assert!(d.lo <= v && v <= d.hi, "{src}: 点 {at:?} の値 {v} が {d:?} に入らない（箱 {boxes:?}）");
                    } else if d.dec >= Dec::Def {
                        panic!("{src}: 飾り {:?} なのに、点 {at:?} で定義されない（箱 {boxes:?}）", d.dec);
                    }
                    checked += 1;
                }
            }
        }
        assert!(checked >= 400_000, "{checked}");
    }

    #[test]
    fn decorations_mark_poles_and_jumps() {
        let x = |lo: f64, hi: f64| [Interval::new(lo, hi), Interval::point(0.0), Interval::point(0.0)];
        // tan の極（π/2）をまたぐ箱は Trv、またがなければ Com
        assert_eq!(prog("tan(x)").eval_interval_dec(&x(1.0, 2.0)).dec, Dec::Trv);
        assert_eq!(prog("tan(x)").eval_interval_dec(&x(0.0, 1.0)).dec, Dec::Com);
        assert_eq!(prog("1/x").eval_interval_dec(&x(-1.0, 1.0)).dec, Dec::Trv);
        // floor の飛びをまたぐ箱は Def（定義されているが連続でない）
        assert_eq!(prog("floor(x)").eval_interval_dec(&x(0.5, 1.5)).dec, Dec::Def);
        assert_eq!(prog("floor(x)").eval_interval_dec(&x(0.2, 0.8)).dec, Dec::Com);
        assert_eq!(prog("mod(x, 2)").eval_interval_dec(&x(1.0, 3.0)).dec, Dec::Def);
        assert_eq!(prog("mod(x, 2)").eval_interval_dec(&x(0.5, 1.5)).dec, Dec::Com);
        assert_eq!(prog("sign(x)").eval_interval_dec(&x(-1.0, 1.0)).dec, Dec::Def);
        // 定義域をはみ出す箱は Trv、外れれば空
        assert_eq!(prog("sqrt(x)").eval_interval_dec(&x(-1.0, 1.0)).dec, Dec::Trv);
        assert!(prog("sqrt(x)").eval_interval_dec(&x(-2.0, -1.0)).is_empty());
        assert!(prog("ln(x) + 1").eval_interval_dec(&x(-2.0, -1.0)).is_empty());
        // 弱い飾りは、式の外側へ伝わる
        assert_eq!(prog("sin(floor(x)) + 1").eval_interval_dec(&x(0.5, 1.5)).dec, Dec::Def);
        // 区間演算が使えない式は Trv
        assert_eq!(prog("gamma(x)").eval_interval_dec(&x(1.0, 2.0)).dec, Dec::Trv);
    }

    #[test]
    fn continuous_boxes_have_zeros_where_signs_change() {
        // Com の箱の、向かい合う 2 つの角で符号が逆なら、その線分の上に零点がある（中間値の定理）
        let mut rng = Lcg(4242);
        let mut found = 0;
        for src in ["sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)", "x^2+y^2+z^2-4", "exp(-x^2) sin(5y) - z", "tanh(x z) - y", "floor(x) + y - z"] {
            let p = prog(src);
            for _ in 0..400 {
                let b = random_boxes(&mut rng);
                if p.eval_interval_dec(&b).dec != Dec::Com {
                    continue;
                }
                let at = |t: f64| -> f64 {
                    let pt: Vec<f64> = b.iter().map(|i| i.lo + (i.hi - i.lo) * t).collect();
                    let mut o = [C::new(0.0, 0.0)];
                    p.eval_batch(&[&[pt[0]], &[pt[1]], &[pt[2]]], &mut o);
                    crate::eval::as_real(o[0])
                };
                let (mut lo, mut hi) = (0.0, 1.0);
                let (f0, f1) = (at(lo), at(hi));
                if f0 * f1 >= 0.0 {
                    continue;
                }
                for _ in 0..60 {
                    let m = 0.5 * (lo + hi);
                    if at(m) * f0 > 0.0 {
                        lo = m;
                    } else {
                        hi = m;
                    }
                }
                let v = at(0.5 * (lo + hi));
                assert!(v.abs() < 1e-6, "{src}: 箱 {b:?} で零点が見つからない（{v}）");
                found += 1;
            }
        }
        assert!(found > 20, "{found}");
    }

    #[test]
    fn interval_support() {
        assert!(!prog("gamma(x) - z").supports_interval());
        assert!(!prog("x + i y").supports_interval());
        assert_eq!(prog("gamma(x) - z").eval_interval(&[Interval::point(1.0); 3]), Range::Unknown);
        for src in EXPRS {
            assert!(prog(src).supports_interval(), "{src}");
        }
    }
}

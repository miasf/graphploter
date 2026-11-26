//! 多項式の連立方程式を、グレブナー基底で正確に解く（実数の解。変数は 2 つか 3 つ）。
//!
//! 連立方程式の点（2D 表示の等式 2 本・3D の等式 3 本）は、1 本目の曲線（曲面の交線）の上で次の式の符号が変わる所を探して
//! ニュートン法で寄せている（`system.rs`）。符号が変わらない解（接するだけの交点。y = x² と y = 0 の原点、円と接線の接点）は
//! 見つからない。式が多項式なら、ここで代数的に解く:
//! 1. 式を有理数係数の多項式にする（係数は大きな整数の分数。計画 `docs/plan-exact-core.md` の EX12）
//! 2. 辞書式順序のグレブナー基底を Buchberger 法で求める（先頭項が互いに素な組は飛ばす。最後に被約な基底にする）。
//!    解が有限個なら、基底は「最後の変数だけの多項式」から始まる三角形の形になる（形の補題）。
//!    基底は整数の係数の原始的な多項式で計算し（分母を作らない）、組は lcm の次数の小さい順に選ぶ。
//!    係数の大きさ（[`MAX_BITS`] ビットまで）と手間（[`Budget`]）を超えたら諦めて、今までの道に任せる
//! 3. 最後の変数だけの多項式（重根を除いた部分 p / gcd(p, p′)）の実根を求め、前の変数へ順に代入して解く。
//!    1 変数の実根は、導関数の根で区間を分け、区間ごとの符号の変化と、導関数の根で値が 0 に触れる所（重根。接点）から求める
//!
//! 結果の点は、呼び出し側でもとの式のニュートン法で磨く。[`solve_exact`] は同じ基底から、解を厳密な数（根号）で返す

use crate::eval::Params;
use crate::exact::algebraic::{self, QRad};
use crate::exact::num::{Int, Nat};
use crate::exact::term::{self, Term};
use crate::exact::{poly, Budget};
use crate::parser::{BinOp, Expr};
use dashu::base::BitTest;
use std::cmp::Ordering;

type Big = crate::exact::num::Rat;

/// 係数（基底の計算は整数の係数）の大きさの上限（ビット）。超えたら諦める（前は i128 があふれたら諦めていた）
pub const MAX_BITS: usize = 1 << 15;

/// [`solve`] の手間の上限（作った項の数と、係数の 64 ビットの語の数の和）。ネイティブで約 0.1 秒（1 秒に約 2300 万）
pub const SOLVE_WORK: u64 = 2_000_000;

/// 有理数（既約・分母は正。大きな整数）。行列の階数の正確な計算（`linalg.rs`）でも使う。
/// 演算は、結果の分子か分母が [`MAX_BITS`] ビットを超えたら None
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Rat(pub(crate) Big);

impl Rat {
    pub(crate) fn one() -> Rat {
        Rat(Big::ONE)
    }
    pub(crate) fn zero() -> Rat {
        Rat(Big::ZERO)
    }
    fn checked(b: Big) -> Option<Rat> {
        (b.numerator().bit_len() <= MAX_BITS && b.denominator().bit_len() <= MAX_BITS).then_some(Rat(b))
    }
    pub(crate) fn new(n: i128, d: i128) -> Option<Rat> {
        if d == 0 {
            return None;
        }
        let s = if d < 0 { -1 } else { 1 };
        Some(Rat(Big::from_parts(Int::from(n) * Int::from(s), Nat::from(d.unsigned_abs()))))
    }
    pub(crate) fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
    pub(crate) fn add(&self, o: &Rat) -> Option<Rat> {
        Rat::checked(&self.0 + &o.0)
    }
    pub(crate) fn neg(&self) -> Rat {
        Rat(-self.0.clone())
    }
    pub(crate) fn mul(&self, o: &Rat) -> Option<Rat> {
        Rat::checked(&self.0 * &o.0)
    }
    pub(crate) fn div(&self, o: &Rat) -> Option<Rat> {
        if o.is_zero() {
            return None;
        }
        Rat::checked(&self.0 / &o.0)
    }
    pub(crate) fn f(&self) -> f64 {
        self.0.to_f64().value()
    }
    /// self · 2⁻ᵉ の f64（係数の大きい多項式を、比を保ったまま f64 にする）
    fn f_scaled(&self, e: i64) -> f64 {
        let (n, d) = (self.0.numerator().clone(), self.0.denominator().clone());
        let b = if e >= 0 { Big::from_parts(n, d << e as usize) } else { Big::from_parts(n << (-e) as usize, d) };
        b.to_f64().value()
    }
    /// 浮動小数点数をぴったり表す分数。有限桁の小数（打った数・刻みのスライダー。小数点以下 22 桁まで、n は 2⁵³ まで）は n / 10ᵈ
    /// （0.9999999 = 9999999 / 10⁷。#141）、ほかは分母が 10⁶ 以下の分数（連分数。1/3 など）。無ければ None（π などは多項式の係数にしない）
    pub(crate) fn from_f64(v: f64) -> Option<Rat> {
        // 整数は 2⁵³ を超えても（10¹⁶ など）そのまま
        if v.is_finite() && v == v.trunc() && v.abs() < 1e30 {
            return Rat::new(v as i128, 1);
        }
        if !v.is_finite() || v.abs() > 1e15 {
            return None;
        }
        let text = format!("{v}");
        if let Some((_, frac)) = text.split_once('.') {
            let d = frac.len() as i32;
            let scale = 10f64.powi(d);
            let n = (v * scale).round();
            if d <= 22 && !text.contains('e') && n.abs() <= 9_007_199_254_740_992.0 && n / scale == v {
                return Some(Rat(Big::from_parts(Int::from(n as i64), Nat::from(10u8).pow(d as usize))));
            }
        }
        let (mut h0, mut h1, mut k0, mut k1) = (0i128, 1i128, 1i128, 0i128);
        let mut x = v;
        for _ in 0..40 {
            let a = x.floor();
            let ai = a as i128;
            let (h2, k2) = (ai.checked_mul(h1)?.checked_add(h0)?, ai.checked_mul(k1)?.checked_add(k0)?);
            if k2 > 1_000_000 {
                return None;
            }
            (h0, h1, k0, k1) = (h1, h2, k1, k2);
            if h1 as f64 / k1 as f64 == v {
                return Rat::new(h1, k1);
            }
            let frac = x - a;
            if frac == 0.0 {
                break;
            }
            x = 1.0 / frac;
        }
        None
    }
}

/// 単項式の指数（変数 0, 1, 2）
type Mono = [u16; 3];

/// 辞書式順序（変数 0 > 1 > 2）
fn lex(a: &Mono, b: &Mono) -> Ordering {
    a.cmp(b)
}

/// 係数が有理数の多項式（式からの変換と多項式の積分。項は単項式の大きい順。係数は 0 でない）
#[derive(Clone, Debug, PartialEq)]
struct Poly(Vec<(Mono, Rat)>);

impl Poly {
    fn zero() -> Poly {
        Poly(Vec::new())
    }
    fn constant(c: Rat) -> Poly {
        if c.is_zero() { Poly::zero() } else { Poly(vec![([0; 3], c)]) }
    }
    fn var(k: usize) -> Poly {
        let mut m = [0; 3];
        m[k] = 1;
        Poly(vec![(m, Rat::one())])
    }
    /// 項を並べ直して、同じ単項式をまとめる
    fn normalize(mut terms: Vec<(Mono, Rat)>) -> Option<Poly> {
        terms.sort_by(|a, b| lex(&b.0, &a.0));
        let mut out: Vec<(Mono, Rat)> = Vec::with_capacity(terms.len());
        for (m, c) in terms {
            match out.last_mut() {
                Some(last) if last.0 == m => last.1 = last.1.add(&c)?,
                _ => out.push((m, c)),
            }
        }
        out.retain(|t| !t.1.is_zero());
        Some(Poly(out))
    }
    fn add(&self, o: &Poly) -> Option<Poly> {
        Poly::normalize(self.0.iter().chain(&o.0).cloned().collect())
    }
    /// self − o（どちらも項は大きい順なので、並べ直さずに合わせる）
    fn sub(&self, o: &Poly) -> Option<Poly> {
        let (a, b) = (&self.0, &o.0);
        let mut out = Vec::with_capacity(a.len() + b.len());
        let (mut i, mut j) = (0, 0);
        while i < a.len() || j < b.len() {
            let ord = match (a.get(i), b.get(j)) {
                (Some(x), Some(y)) => lex(&x.0, &y.0),
                (Some(_), None) => Ordering::Greater,
                _ => Ordering::Less,
            };
            match ord {
                Ordering::Greater => {
                    out.push(a[i].clone());
                    i += 1;
                }
                Ordering::Less => {
                    out.push((b[j].0, b[j].1.neg()));
                    j += 1;
                }
                Ordering::Equal => {
                    let c = Rat::checked(&a[i].1 .0 - &b[j].1 .0)?;
                    if !c.is_zero() {
                        out.push((a[i].0, c));
                    }
                    i += 1;
                    j += 1;
                }
            }
        }
        Some(Poly(out))
    }
    fn mul(&self, o: &Poly) -> Option<Poly> {
        let mut terms = Vec::with_capacity(self.0.len() * o.0.len());
        for (m1, c1) in &self.0 {
            for (m2, c2) in &o.0 {
                let m = [m1[0].checked_add(m2[0])?, m1[1].checked_add(m2[1])?, m1[2].checked_add(m2[2])?];
                terms.push((m, c1.mul(c2)?));
            }
        }
        Poly::normalize(terms)
    }
    /// 単項式 m・係数 c を掛ける
    fn scale(&self, m: Mono, c: &Rat) -> Option<Poly> {
        let terms = self.0.iter().map(|(mm, cc)| Some(([mm[0] + m[0], mm[1] + m[1], mm[2] + m[2]], cc.mul(c)?))).collect::<Option<Vec<_>>>()?;
        Some(Poly(terms))
    }
}

fn divides(a: &Mono, b: &Mono) -> bool {
    (0..3).all(|k| a[k] <= b[k])
}

fn lcm_mono(a: &Mono, b: &Mono) -> Mono {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
}

fn shift(a: &Mono, b: &Mono) -> Mono {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// 係数が整数の多項式（項は単項式の大きい順。係数は 0 でない）。基底の計算はこの形で、分母を作らない
/// （分数の係数で 1 にそろえると、途中の分母が大きくなって遅い。計画の EX12 の 0 章）
#[derive(Clone, Debug, PartialEq)]
struct IPoly(Vec<(Mono, Int)>);

impl IPoly {
    /// 分母を払って原始的にする
    fn from_rational(p: &Poly) -> Option<IPoly> {
        let mut l = Int::ONE;
        for (_, c) in &p.0 {
            l = crate::exact::num::lcm(&l, &Int::from(c.0.denominator().clone()));
        }
        let terms = p.0.iter().map(|(m, c)| (*m, c.0.numerator().clone() * (l.clone() / Int::from(c.0.denominator().clone())))).collect();
        Some(IPoly(terms).primitive())
    }
    fn is_zero(&self) -> bool {
        self.0.is_empty()
    }
    fn lead_mono(&self) -> Mono {
        self.0[0].0
    }
    fn lead_coef(&self) -> &Int {
        &self.0[0].1
    }
    /// 係数の最大公約数で割り、先頭の係数を正にする
    fn primitive(mut self) -> IPoly {
        let Some(first) = self.0.first() else { return self };
        let mut g = first.1.clone();
        for (_, c) in &self.0[1..] {
            if g == Int::ONE || g == -Int::ONE {
                break;
            }
            g = crate::exact::num::gcd(&g, c);
        }
        g = crate::exact::num::gcd(&g, &Int::ZERO);
        if self.0[0].1 < Int::ZERO {
            g = -g;
        }
        if g != Int::ONE {
            for t in &mut self.0 {
                t.1 = &t.1 / &g;
            }
        }
        self
    }
    /// α·x^sa·self − β·x^sb·q（どちらも項は大きい順なので、並べ直さずに合わせる）
    fn lincomb(&self, alpha: &Int, sa: Mono, q: &IPoly, beta: &Int, sb: Mono, budget: &Budget) -> Option<IPoly> {

        let up = |m: &Mono, s: &Mono| [m[0] + s[0], m[1] + s[1], m[2] + s[2]];
        let (a, b) = (&self.0, &q.0);
        let mut out = Vec::with_capacity(a.len() + b.len());
        let (mut i, mut j) = (0, 0);
        while i < a.len() || j < b.len() {
            let (ma, mb) = (a.get(i).map(|t| up(&t.0, &sa)), b.get(j).map(|t| up(&t.0, &sb)));
            let ord = match (ma, mb) {
                (Some(x), Some(y)) => lex(&x, &y),
                (Some(_), None) => Ordering::Greater,
                _ => Ordering::Less,
            };
            match ord {
                Ordering::Greater => {
                    out.push((ma.unwrap(), alpha * &a[i].1));
                    i += 1;
                }
                Ordering::Less => {
                    out.push((mb.unwrap(), -(beta * &b[j].1)));
                    j += 1;
                }
                Ordering::Equal => {
                    let c = alpha * &a[i].1 - beta * &b[j].1;
                    if c != Int::ZERO {
                        out.push((ma.unwrap(), c));
                    }
                    i += 1;
                    j += 1;
                }
            }
        }
        // 手間は項の数と係数の長さ（64 ビットの語の数）で数える
        let mut cost = 0u64;
        for t in &out {
            let bits = t.1.bit_len();
            if bits > MAX_BITS {
                return None;
            }
            cost += 1 + bits as u64 / 64;
        }
        budget.charge(cost).ok()?;
        Some(IPoly(out))
    }
    fn degree_in(&self, k: usize) -> u16 {
        self.0.iter().map(|t| t.0[k]).max().unwrap_or(0)
    }
    /// 変数 k を含むか
    fn uses(&self, k: usize) -> bool {
        self.0.iter().any(|t| t.0[k] > 0)
    }
}

/// p の、g の並びによる正規形（全部の項を簡約する。結果は原始的）
fn reduce(p: &IPoly, g: &[IPoly], budget: &Budget) -> Option<IPoly> {
    let mut p = p.clone();
    // 簡約し終えた項の数（p の先頭から）
    let mut done = 0;
    let mut steps = 0;
    while done < p.0.len() {
        steps += 1;
        if steps > 20_000 {
            return None;
        }
        let (m, c) = (p.0[done].0, p.0[done].1.clone());
        match g.iter().find(|q| divides(&q.lead_mono(), &m)) {
            Some(q) => {
                let a = q.lead_coef();
                let d = crate::exact::num::gcd(a, &c);
                let (alpha, beta) = (a / &d, &c / &d);
                p = p.lincomb(&alpha, [0; 3], q, &beta, shift(&m, &q.lead_mono()), budget)?;
                if steps % 8 == 0 {
                    p = p.primitive();
                }
            }
            None => done += 1,
        }
    }
    Some(p.primitive())
}

fn s_poly(f: &IPoly, g: &IPoly, budget: &Budget) -> Option<IPoly> {
    let (fm, gm) = (f.lead_mono(), g.lead_mono());
    let l = lcm_mono(&fm, &gm);
    let (a, b) = (f.lead_coef(), g.lead_coef());
    let d = crate::exact::num::gcd(a, b);
    f.lincomb(&(b / &d), shift(&l, &fm), g, &(a / &d), shift(&l, &gm), budget)
}

/// 辞書式順序の被約グレブナー基底（Buchberger 法。組は lcm の次数の小さい順に選び、Buchberger の第 2 基準で飛ばす）。
/// 大きくなりすぎる・係数が大きすぎる・予算を超えるときは None
fn groebner(fs: Vec<IPoly>, budget: &Budget) -> Option<Vec<IPoly>> {
    let mut g: Vec<IPoly> = fs.into_iter().filter(|p| !p.is_zero()).collect();
    let mut pairs: Vec<(usize, usize)> = (0..g.len()).flat_map(|j| (0..j).map(move |i| (i, j))).collect();
    let mut rounds = 0;
    let deg = |m: &Mono| m.iter().map(|&e| e as u32).sum::<u32>();
    while !pairs.is_empty() {
        rounds += 1;
        if rounds > 3000 || g.len() > 60 {
            return None;
        }
        // lcm の次数のいちばん小さい組（同じなら辞書式で小さい組）
        let key = |&(i, j): &(usize, usize)| {
            let l = lcm_mono(&g[i].lead_mono(), &g[j].lead_mono());
            (deg(&l), l)
        };
        let at = (0..pairs.len()).min_by(|&x, &y| key(&pairs[x]).cmp(&key(&pairs[y])))?;
        let (i, j) = pairs.swap_remove(at);
        let (a, b) = (g[i].lead_mono(), g[j].lead_mono());
        // 先頭項が互いに素なら、S 多項式は 0 に簡約される（Buchberger の第 1 基準）
        if (0..3).all(|k| a[k] == 0 || b[k] == 0) {
            continue;
        }
        // 第 2 基準: 先頭項が lcm を割る k で、(i, k)・(j, k) がもう調べ終わっているなら飛ばす
        let l = lcm_mono(&a, &b);
        let pending = |x: usize, y: usize| pairs.contains(&(x.min(y), x.max(y)));
        if (0..g.len()).any(|k| k != i && k != j && divides(&g[k].lead_mono(), &l) && !pending(i, k) && !pending(j, k)) {
            continue;
        }
        let r = reduce(&s_poly(&g[i], &g[j], budget)?, &g, budget)?;
        if !r.is_zero() {
            g.push(r);
            let n = g.len() - 1;
            pairs.extend((0..n).map(|k| (k, n)));
        }
    }
    // 最小の基底（先頭項がほかの先頭項で割り切れるものを除く）にして、互いに簡約する
    let mut min: Vec<IPoly> = Vec::new();
    for (k, p) in g.iter().enumerate() {
        let m = p.lead_mono();
        let redundant = g.iter().enumerate().any(|(j, q)| j != k && divides(&q.lead_mono(), &m) && (q.lead_mono() != m || j < k));
        if !redundant {
            min.push(p.clone());
        }
    }
    let mut out = Vec::with_capacity(min.len());
    for k in 0..min.len() {
        let others: Vec<IPoly> = min.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, p)| p.clone()).collect();
        let r = reduce(&min[k], &others, budget)?;
        if !r.is_zero() {
            out.push(r);
        }
    }
    out.sort_by(|a, b| lex(&a.lead_mono(), &b.lead_mono()));
    Some(out)
}

/// 式を多項式にする（変数は names の順に 0, 1, 2）。多項式でない・係数が分数にならないときは None
fn from_expr(e: &Expr, names: &[&str], params: &Params) -> Option<Poly> {
    match e {
        Expr::Num(v) => Some(Poly::constant(Rat::from_f64(*v)?)),
        Expr::Var(n) => match names.iter().position(|m| m == n) {
            Some(k) => Some(Poly::var(k)),
            None => {
                let c = params.get(n.as_str())?;
                (c.im == 0.0).then_some(())?;
                Some(Poly::constant(Rat::from_f64(c.re)?))
            }
        },
        Expr::Neg(a) => Poly::zero().sub(&from_expr(a, names, params)?),
        Expr::Bin(op, a, b) => {
            let pa = from_expr(a, names, params)?;
            match op {
                BinOp::Add => pa.add(&from_expr(b, names, params)?),
                BinOp::Sub => pa.sub(&from_expr(b, names, params)?),
                BinOp::Mul => pa.mul(&from_expr(b, names, params)?),
                BinOp::Div => {
                    // 定数で割る
                    let pb = from_expr(b, names, params)?;
                    if pb.0.len() != 1 || pb.0[0].0 != [0; 3] {
                        return None;
                    }
                    pa.scale([0; 3], &Rat::one().div(&pb.0[0].1)?)
                }
                BinOp::Pow => {
                    let k = match &**b {
                        Expr::Num(k) if *k >= 0.0 && k.fract() == 0.0 && *k <= 16.0 => *k as u32,
                        _ => return None,
                    };
                    let mut r = Poly::constant(Rat::one());
                    for _ in 0..k {
                        r = r.mul(&pa)?;
                    }
                    Some(r)
                }
            }
        }
        _ => None,
    }
}

/// 1 変数の多項式（係数は低い次数から）の値
fn eval1(c: &[f64], x: f64) -> f64 {
    c.iter().rev().fold(0.0, |acc, &a| acc * x + a)
}

/// 1 変数の多項式（係数は低い次数から）の、[−bound, bound] の中の実根。導関数の根で区間を分け、
/// 区間ごとの符号の変化（はさみうちで詰める）と、導関数の根で値が 0 に触れる所（重根）から求める
fn real_roots(c: &[f64]) -> Vec<f64> {
    let mut c = c.to_vec();
    while c.len() > 1 && c.last() == Some(&0.0) {
        c.pop();
    }
    let deg = c.len() - 1;
    if deg == 0 {
        return Vec::new();
    }
    let lead = c[deg];
    // 根の上限（コーシーの上界）
    let bound = 1.0 + c[..deg].iter().map(|a| (a / lead).abs()).fold(0.0, f64::max);
    let scale = |x: f64| c.iter().enumerate().map(|(i, a)| a.abs() * x.abs().powi(i as i32)).sum::<f64>().max(f64::MIN_POSITIVE);
    if deg == 1 {
        return vec![-c[0] / c[1]];
    }
    let d: Vec<f64> = (1..=deg).map(|i| i as f64 * c[i]).collect();
    let mut stops = vec![-bound];
    let mut crit = real_roots(&d);
    crit.retain(|x| x.abs() < bound);
    crit.sort_by(f64::total_cmp);
    stops.extend(&crit);
    stops.push(bound);
    let mut out = Vec::new();
    for w in stops.windows(2) {
        let (mut a, mut b) = (w[0], w[1]);
        let (mut fa, fb) = (eval1(&c, a), eval1(&c, b));
        if fa == 0.0 {
            out.push(a);
            continue;
        }
        if fa.signum() == fb.signum() || fb == 0.0 {
            continue;
        }
        for _ in 0..200 {
            let m = 0.5 * (a + b);
            if m == a || m == b {
                break;
            }
            let fm = eval1(&c, m);
            if fm.signum() == fa.signum() {
                (a, fa) = (m, fm);
            } else {
                b = m;
            }
        }
        out.push(0.5 * (a + b));
    }
    // 導関数の根で値が 0 に触れる（符号が変わらない重根）
    for &x in &crit {
        if eval1(&c, x).abs() <= 1e-9 * scale(x) {
            out.push(x);
        }
    }
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() <= 1e-9 * (1.0 + a.abs()));
    out
}

/// 多項式に、変数 k より後ろの変数の値を入れて、変数 k の 1 次元の多項式（係数は低い次数から）にする。
/// 係数が f64 に収まらないほど大きくても、いちばん大きい係数で割ってから f64 にする（根は変わらない）
fn univariate(p: &IPoly, k: usize, vals: &[f64; 3]) -> Vec<f64> {
    let top = p.0.iter().map(|t| t.1.bit_len() as i64).max().unwrap_or(0);
    let mut c = vec![0.0; p.degree_in(k) as usize + 1];
    for (m, r) in &p.0 {
        let mut v = Rat(Big::from(r.clone())).f_scaled(top);
        for j in k + 1..3 {
            v *= vals[j].powi(m[j] as i32);
        }
        c[m[k] as usize] += v;
    }
    c
}

/// 式 e が変数 `var` の多項式（係数は有理数。パラメータは値を入れる）なら、その定積分 ∫_a^b e d var を有理数で正確に求めた値（#204）。
/// (t + 10¹⁶) − 10¹⁶ は t、∫_{−1}^{1} (10¹⁶ t + 1) dt は 2。多項式でない・端が有理数で表せない・途中であふれるなら None
pub(crate) fn polynomial_integral(e: &Expr, var: &str, params: &Params, a: f64, b: f64) -> Option<f64> {
    let p = from_expr(e, &[var], params)?;
    let (ra, rb) = (Rat::from_f64(a)?, Rat::from_f64(b)?);
    let pow = |r: &Rat, k: u32| -> Option<Rat> { (0..k).try_fold(Rat::one(), |acc, _| acc.mul(r)) };
    let mut total = Rat::zero();
    for (m, c) in &p.0 {
        let k = m[0] as u32;
        // c (b^{k+1} − a^{k+1}) / (k + 1)
        let diff = pow(&rb, k + 1)?.add(&pow(&ra, k + 1)?.neg())?;
        total = total.add(&c.mul(&diff)?.div(&Rat::new(k as i128 + 1, 1)?)?)?;
    }
    Some(total.f())
}

/// 連立方程式のグレブナー基底
enum Basis {
    /// 基底が {1}: 解が無い
    Empty,
    Finite(Vec<IPoly>),
}

fn basis(qs: &[Poly], budget: &Budget) -> Option<Basis> {
    let polys = qs.iter().map(IPoly::from_rational).collect::<Option<Vec<_>>>()?;
    let g = groebner(polys, budget)?;
    if g.iter().any(|p| p.0.len() == 1 && p.lead_mono() == [0; 3]) {
        return Some(Basis::Empty);
    }
    Some(Basis::Finite(g))
}

/// 有限個の解のとき、どの変数 k にも、それより前の変数を含まない式で、先頭が x_k の純粋なべきのものがある。その段の式の並び
fn level(g: &[IPoly], k: usize) -> Option<Vec<&IPoly>> {
    let ps: Vec<&IPoly> = g.iter().filter(|p| (0..k).all(|j| !p.uses(j)) && p.uses(k)).collect();
    ps.iter().any(|p| { let m = p.lead_mono(); m[k] > 0 && (0..3).all(|j| j == k || m[j] == 0) }).then_some(ps)
}

/// 連立方程式 eqs = 0（変数は names。2 つか 3 つ）の実数の解（有限個のときだけ）。多項式でない・解が無限個・
/// 計算が大きくなりすぎるときは None（呼び出し側が今までの道で探す）。戻り値の点は names の順（足りない成分は 0）
pub fn solve(eqs: &[Expr], names: &[&str], params: &Params) -> Option<Vec<[f64; 3]>> {
    let n = names.len();
    if !(2..=3).contains(&n) || eqs.len() < n {
        return None;
    }
    let budget = Budget::new(SOLVE_WORK);
    let qs = eqs.iter().map(|e| from_expr(e, names, params)).collect::<Option<Vec<_>>>()?;
    let Basis::Finite(g) = basis(&qs, &budget)? else { return Some(Vec::new()) };
    // 最後の変数だけの式（係数は厳密な整数）の実根は、厳密に分離して求める（係数が大きいと f64 では桁落ちで偽の根が出る）
    let last = level(&g, n - 1)?;
    let common = last.iter().map(|p| to_univariate(p, n - 1)).reduce(|a, b| poly::gcd(&a, &b))?;
    let mut sols: Vec<[f64; 3]> = crate::exact::roots::real_roots_f64(&common, &budget)
        .ok()?
        .into_iter()
        .map(|r| {
            let mut p = [0.0; 3];
            p[n - 1] = r;
            p
        })
        .collect();
    for k in (0..n - 1).rev() {
        let ps = level(&g, k)?;
        let mut next = Vec::new();
        for s in &sols {
            // 値を入れて 0 にならない式のうち、次数のいちばん低いものの根を候補にし、ほかの式も 0 になるものだけ残す
            let us: Vec<Vec<f64>> = ps.iter().map(|p| univariate(p, k, s)).collect();
            let size = |c: &Vec<f64>| c.iter().map(|a| a.abs()).fold(0.0, f64::max);
            let Some(base) = us.iter().filter(|c| c.len() > 1 && c[1..].iter().any(|a| a.abs() > 1e-12 * size(c))).min_by_key(|c| c.len()) else {
                continue;
            };
            for r in real_roots(base) {
                // ほかの式も 0 になるか（根の近くの項の大きさに比べて、と、係数の大きさに比べて。根 0 は 1e-61 などで求まる）
                let ok = us.iter().all(|c| {
                    let terms = c.iter().enumerate().map(|(i, a)| a.abs() * r.abs().powi(i as i32)).sum::<f64>();
                    let coefs = c.iter().map(|a| a.abs()).sum::<f64>();
                    eval1(c, r).abs() <= 1e-6 * terms + 1e-9 * coefs
                });
                if ok {
                    let mut p = *s;
                    p[k] = r;
                    next.push(p);
                }
            }
        }
        sols = next;
    }
    // 前の変数は f64 の代入で求めたので、係数の大きい基底では 10⁻⁵ ほどずれる。もとの式で磨く
    for p in &mut sols {
        *p = polish(&qs, n, *p);
    }
    Some(sols)
}

/// 多項式 q の点 p での値と勾配
fn eval_grad(q: &Poly, p: &[f64; 3]) -> (f64, [f64; 3], f64) {
    let (mut v, mut g, mut size) = (0.0, [0.0; 3], 0.0);
    for (m, c) in &q.0 {
        let c = c.f();
        let t = c * (0..3).map(|k| p[k].powi(m[k] as i32)).product::<f64>();
        v += t;
        size += t.abs();
        for k in 0..3 {
            if m[k] > 0 {
                g[k] += c * m[k] as f64 * (0..3).map(|j| p[j].powi(m[j] as i32 - (j == k) as i32)).product::<f64>();
            }
        }
    }
    (v, g, size)
}

/// もとの式 qs = 0 のガウス・ニュートン法で点を磨く（残りが小さくなる歩みだけ採る）
fn polish(qs: &[Poly], n: usize, mut p: [f64; 3]) -> [f64; 3] {
    let residual = |p: &[f64; 3]| qs.iter().map(|q| { let (v, _, s) = eval_grad(q, p); (v / s.max(f64::MIN_POSITIVE)).powi(2) }).sum::<f64>();
    let mut r = residual(&p);
    for _ in 0..8 {
        if r == 0.0 {
            break;
        }
        // 正規方程式 JᵀJ d = −JᵀF（n ≤ 3）
        let mut a = [[0.0; 4]; 3];
        for q in qs {
            let (v, g, _) = eval_grad(q, &p);
            for i in 0..n {
                for j in 0..n {
                    a[i][j] += g[i] * g[j];
                }
                a[i][3] -= g[i] * v;
            }
        }
        // ガウスの消去（部分ピボット）
        for c in 0..n {
            let piv = (c..n).max_by(|&x, &y| a[x][c].abs().total_cmp(&a[y][c].abs())).unwrap();
            a.swap(c, piv);
            if a[c][c] == 0.0 {
                return p;
            }
            for i in 0..n {
                if i != c {
                    let f = a[i][c] / a[c][c];
                    for j in c..4 {
                        a[i][j] -= f * a[c][j];
                    }
                }
            }
        }
        let mut next = p;
        for i in 0..n {
            next[i] += a[i][3] / a[i][i];
        }
        let rn = residual(&next);
        if !(rn < r) {
            break;
        }
        (p, r) = (next, rn);
    }
    p
}


/// 連立方程式 eqs = 0 の実数の解を、厳密な数（有理数と平方根）で（計画の EX12）。戻り値の点は names の順で、成分は names の数だけ。
///
/// 基底の三角形の形を後ろの変数から解く。値を入れた 1 変数の式が、
/// - 係数が全部有理数なら、それらの GCD を有理数の上で因数分解し、1 次と 2 次の因数の実根を取る
/// - 係数に根号があれば、いちばん低い次数の式が 1 次のときだけ、その根を取る（ほかの式も 0 になることを確かめる）
///
/// 3 次以上の既約な因数・根号の係数の 2 次式・二重の根号が要るときは None（文書 8 の SC1 で、EX7 の根で広げる）。
/// 解が無い・多項式でない・解が無限個・予算を超えるときも None（解が無いは Some(空)）
pub fn solve_exact(eqs: &[Expr], names: &[&str], params: &Params, budget: &Budget) -> Option<Vec<Vec<Term>>> {
    let n = names.len();
    if !(2..=3).contains(&n) || eqs.len() < n {
        return None;
    }
    let qs = eqs.iter().map(|e| from_expr(e, names, params)).collect::<Option<Vec<_>>>()?;
    let Basis::Finite(g) = basis(&qs, budget)? else { return Some(Vec::new()) };
    let zero = QRad::rational(Big::ZERO);
    let mut sols: Vec<Vec<QRad>> = vec![vec![zero.clone(), zero.clone(), zero]];
    for k in (0..n).rev() {
        let ps = level(&g, k)?;
        let mut next = Vec::new();
        for s in &sols {
            let us: Vec<Vec<QRad>> = ps.iter().map(|p| univariate_exact(p, k, s)).collect();
            for r in common_roots(us, budget)? {
                let mut p = s.clone();
                p[k] = r;
                next.push(p);
            }
        }
        sols = next;
    }
    sols.sort_by(|a, b| {
        let key = |p: &Vec<QRad>| p.iter().map(QRad::approx).collect::<Vec<f64>>();
        key(a).partial_cmp(&key(b)).unwrap_or(Ordering::Equal)
    });
    Some(sols.into_iter().map(|p| p[..n].iter().map(QRad::to_term).collect()).collect())
}

/// 変数 k だけの式を、1 変数の多項式にする
fn to_univariate(p: &IPoly, k: usize) -> poly::Poly {
    let mut c = vec![Big::ZERO; p.degree_in(k) as usize + 1];
    for (m, a) in &p.0 {
        c[m[k] as usize] += Big::from(a.clone());
    }
    poly::Poly::new(c)
}

/// [`univariate`] の厳密な版（後ろの変数の値は平方根の数）
fn univariate_exact(p: &IPoly, k: usize, vals: &[QRad]) -> Vec<QRad> {
    let mut c = vec![QRad::rational(Big::ZERO); p.degree_in(k) as usize + 1];
    for (m, r) in &p.0 {
        let mut v = QRad::rational(Big::from(r.clone()));
        for j in k + 1..3 {
            for _ in 0..m[j] {
                v = v.mul(&vals[j]);
            }
        }
        c[m[k] as usize] = c[m[k] as usize].add(&v);
    }
    while c.last().is_some_and(QRad::is_zero) {
        c.pop();
    }
    c
}

/// 1 変数の式の並び（係数は低い次数から）の、共通の実根
fn common_roots(us: Vec<Vec<QRad>>, budget: &Budget) -> Option<Vec<QRad>> {
    let us: Vec<Vec<QRad>> = us.into_iter().filter(|c| !c.is_empty()).collect();
    if us.is_empty() {
        return None; // どの式も 0: 解が無限個
    }
    if us.iter().any(|c| c.len() == 1) {
        return Some(Vec::new()); // 0 でない定数: この枝に解は無い
    }
    let rational: Option<Vec<poly::Poly>> = us.iter().map(|c| c.iter().map(QRad::as_rational).collect::<Option<Vec<_>>>().map(poly::Poly::new)).collect();
    if let Some(ps) = rational {
        let g = ps[1..].iter().fold(ps[0].clone(), |acc, p| poly::gcd(&acc, p));
        let fac = poly::factor(&g, budget);
        if !fac.complete {
            return None;
        }
        let mut roots = Vec::new();
        for (f, _) in &fac.factors {
            let c = &f.0;
            match f.deg() {
                1 => roots.push(QRad::rational(-c[0].clone() / &c[1])),
                2 => {
                    let disc = &c[1] * &c[1] - Big::from(4u8) * &c[0] * &c[2];
                    if disc < Big::ZERO {
                        continue;
                    }
                    let sq = algebraic::of_term(&term::rational_power(&disc, &Big::from_parts(Int::ONE, Nat::from(2u8))))?;
                    let den = QRad::rational(Big::ONE / (Big::from(2u8) * &c[2]));
                    let b = QRad::rational(-c[1].clone());
                    let minus = sq.mul(&QRad::rational(-Big::ONE));
                    roots.push(b.add(&minus).mul(&den));
                    roots.push(b.add(&sq).mul(&den));
                }
                _ => return None,
            }
        }
        return Some(roots);
    }
    // 根号の係数: いちばん低い次数の式が 1 次なら、その根
    let base = us.iter().min_by_key(|c| c.len())?;
    if base.len() != 2 {
        return None;
    }
    let r = base[0].mul(&base[1].inverse()?).mul(&QRad::rational(-Big::ONE));
    let vanishes = |c: &Vec<QRad>| {
        let mut acc = QRad::rational(Big::ZERO);
        for a in c.iter().rev() {
            acc = acc.mul(&r).add(a);
        }
        acc.is_zero()
    };
    Some(if us.iter().all(vanishes) { vec![r] } else { Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn sols(srcs: &[&str], names: &[&str]) -> Vec<[f64; 3]> {
        let eqs: Vec<Expr> = srcs.iter().map(|s| parse(s).unwrap()).collect();
        let mut s = solve(&eqs, names, &Params::new()).unwrap_or_else(|| panic!("{srcs:?} が解けない"));
        s.sort_by(|a, b| a.partial_cmp(b).unwrap());
        s
    }

    fn near(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|k| (a[k] - b[k]).abs() < 1e-7)
    }

    #[test]
    fn tangent_and_transversal_intersections() {
        // 接するだけの交点（符号が変わらない）
        let s = sols(&["y - x^2", "y"], &["x", "y"]);
        assert_eq!(s.len(), 1);
        assert!(near(s[0], [0.0, 0.0, 0.0]), "{s:?}");
        // 円と接線: (0, 1)
        let s = sols(&["x^2 + y^2 - 1", "y - 1"], &["x", "y"]);
        assert!(s.len() == 1 && near(s[0], [0.0, 1.0, 0.0]), "{s:?}");
        // ふつうの交点 y = x, y = x²: (0, 0), (1, 1)
        let s = sols(&["y - x", "y - x^2"], &["x", "y"]);
        assert!(s.len() == 2 && near(s[0], [0.0, 0.0, 0.0]) && near(s[1], [1.0, 1.0, 0.0]), "{s:?}");
        // 2 つの円: (0.5, ±√3/2)
        let s = sols(&["x^2 + y^2 - 1", "(x - 1)^2 + y^2 - 1"], &["x", "y"]);
        let h = 3f64.sqrt() / 2.0;
        assert!(s.len() == 2 && near(s[0], [0.5, -h, 0.0]) && near(s[1], [0.5, h, 0.0]), "{s:?}");
        // 解が無い（実数でない）
        assert!(sols(&["x^2 + y^2 + 1", "x - y"], &["x", "y"]).is_empty());
        // 3 変数: 球と 2 つの平面
        let s = sols(&["x^2 + y^2 + z^2 - 3", "x - y", "y - z"], &["x", "y", "z"]);
        assert!(s.len() == 2 && near(s[0], [-1.0, -1.0, -1.0]) && near(s[1], [1.0, 1.0, 1.0]), "{s:?}");
        // 3 変数で接する: 2 つの球が 1 点で接し、平面が通る
        let s = sols(&["x^2 + y^2 + z^2 - 1", "(x - 2)^2 + y^2 + z^2 - 1", "z"], &["x", "y", "z"]);
        assert!(s.len() == 1 && near(s[0], [1.0, 0.0, 0.0]), "{s:?}");
        // 解が無限個（曲線）は None
        let eqs: Vec<Expr> = ["x - y", "2x - 2y"].iter().map(|s| parse(s).unwrap()).collect();
        assert!(solve(&eqs, &["x", "y"], &Params::new()).is_none());
    }

    fn exact(srcs: &[&str], names: &[&str]) -> Option<Vec<String>> {
        let eqs: Vec<Expr> = srcs.iter().map(|s| parse(s).unwrap()).collect();
        let st = crate::exact::latex::Style { decimal: false, input: false };
        let sols = solve_exact(&eqs, names, &Params::new(), &Budget::new(SOLVE_WORK))?;
        Some(sols.iter().map(|p| p.iter().map(|t| crate::exact::latex::to_latex(t, &st)).collect::<Vec<_>>().join(", ")).collect())
    }

    #[test]
    fn exact_points() {
        // 2 つの円: (1/2, ±√3/2)
        let s = exact(&["x^2 + y^2 - 1", "(x - 1)^2 + y^2 - 1"], &["x", "y"]).unwrap();
        assert_eq!(s, ["\\frac{1}{2}, -\\frac{\\sqrt{3}}{2}", "\\frac{1}{2}, \\frac{\\sqrt{3}}{2}"], "{s:?}");
        // 円と接線（重根）
        assert_eq!(exact(&["x^2 + y^2 - 1", "y - 1"], &["x", "y"]).unwrap(), ["0, 1"]);
        // 後ろの変数の根号を、1 次式に入れて前の変数を出す: y² = 2, xy = 1 → (±√2/2, ±√2)
        let s = exact(&["y^2 - 2", "x y - 1"], &["x", "y"]).unwrap();
        assert_eq!(s, ["-\\frac{\\sqrt{2}}{2}, -\\sqrt{2}", "\\frac{\\sqrt{2}}{2}, \\sqrt{2}"], "{s:?}");
        // 3 変数: 球と 2 つの平面
        assert_eq!(exact(&["x^2 + y^2 + z^2 - 3", "x - y", "y - z"], &["x", "y", "z"]).unwrap(), ["-1, -1, -1", "1, 1, 1"]);
        // 係数の大きい系（前は i128 があふれて諦めていた種類）。x = 7, y = 11, z = 13 を 3 つの式で
        let s = exact(&["x y z - 1001", "x^2 + y^2 + z^2 - 339", "x + y + z - 31"], &["x", "y", "z"]).unwrap();
        assert_eq!(s.len(), 6, "{s:?}");
        assert!(s.contains(&"7, 11, 13".to_string()) && s.contains(&"13, 11, 7".to_string()), "{s:?}");
        // 実数の解が無い
        assert!(exact(&["x^2 + y^2 + 1", "x - y"], &["x", "y"]).unwrap().is_empty());
        // 既約な 3 次の因数は、まだ根号で書かない（SC1）
        assert!(exact(&["x^3 - 2", "y - x"], &["x", "y"]).is_none());
        // 根号の係数の 2 次式（二重の根号が要る）も、まだ扱わない
        assert!(exact(&["y^2 - 2", "x^2 - y"], &["x", "y"]).is_none());
    }

    #[test]
    fn large_coefficients() {
        // 前は i128 の分数があふれて諦めていた系（係数の大きい 3 変数）。解の数と、もとの式に入れた残りの小ささ
        // （根は f64 で求めるので、係数の大きい消去式では 10⁻⁵ ほどずれる。呼び出し側がもとの式のニュートン法で磨く）
        let cases: [(&[&str], usize); 3] = [
            // 消去式は 27 次で相異なる実根が 7 つ（SymPy で確かめた。f64 で根を探すと偽の根が出て 9 つになっていた）
            (&["x^3 - 7y z + 13", "y^3 - 11x z - 17", "z^3 - 19x y + 23"], 7),
            (&["x^2 y + 31z^2 - 17", "y^3 - 23x z + 43", "z^2 x + 19y^2 - 71x - 3"], 2),
            (&["x^2 + y^2 + z^2 - 1000003", "x y + y z + z x - 7777", "x y z - 123457"], 6),
        ];
        for (srcs, count) in cases {
            let eqs: Vec<Expr> = srcs.iter().map(|s| parse(s).unwrap()).collect();
            let s = solve(&eqs, &["x", "y", "z"], &Params::new()).unwrap_or_else(|| panic!("{srcs:?} が解けない"));
            assert_eq!(s.len(), count, "{srcs:?} {s:?}");
            for p in &s {
                for e in &eqs {
                    let q = from_expr(e, &["x", "y", "z"], &Params::new()).unwrap();
                    let v: f64 = q.0.iter().map(|(m, c)| c.f() * (0..3).map(|k| p[k].powi(m[k] as i32)).product::<f64>()).sum();
                    let size: f64 = q.0.iter().map(|(m, c)| (c.f() * (0..3).map(|k| p[k].powi(m[k] as i32)).product::<f64>()).abs()).sum();
                    assert!(v.abs() <= 1e-7 * size, "{srcs:?} {p:?} {v}");
                }
            }
        }
    }

}

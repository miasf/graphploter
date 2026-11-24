//! 多項式の連立方程式を、グレブナー基底で正確に解く（実数の解。変数は 2 つか 3 つ）。
//!
//! 連立方程式の点（2D 表示の等式 2 本・3D の等式 3 本）は、1 本目の曲線（曲面の交線）の上で次の式の符号が変わる所を探して
//! ニュートン法で寄せている（`system.rs`）。符号が変わらない解（接するだけの交点。y = x² と y = 0 の原点、円と接線の接点）は
//! 見つからない。式が多項式なら、ここで代数的に解く:
//! 1. 式を有理数係数の多項式にする（係数は i128 の分数。途中であふれたら諦めて、今までの道に任せる）
//! 2. 辞書式順序のグレブナー基底を Buchberger 法で求める（先頭項が互いに素な組は飛ばす。最後に被約な基底にする）。
//!    解が有限個なら、基底は「最後の変数だけの多項式」から始まる三角形の形になる（形の補題）
//! 3. 最後の変数だけの多項式（重根を除いた部分 p / gcd(p, p′)）の実根を求め、前の変数へ順に代入して解く。
//!    1 変数の実根は、導関数の根で区間を分け、区間ごとの符号の変化と、導関数の根で値が 0 に触れる所（重根。接点）から求める
//!
//! 結果の点は、呼び出し側でもとの式のニュートン法で磨く

use crate::eval::Params;
use crate::parser::{BinOp, Expr};
use std::cmp::Ordering;

/// 有理数（分母は正、約分済み）。行列の階数の正確な計算（`linalg.rs`）でも使う
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rat {
    pub(crate) n: i128,
    pub(crate) d: i128,
}

fn gcd(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Rat {
    pub(crate) const ONE: Rat = Rat { n: 1, d: 1 };

    pub(crate) fn new(n: i128, d: i128) -> Option<Rat> {
        if d == 0 {
            return None;
        }
        let g = gcd(n, d).max(1);
        let s = if d < 0 { -1 } else { 1 };
        Some(Rat { n: s * n / g, d: s * d / g })
    }
    pub(crate) fn add(self, o: Rat) -> Option<Rat> {
        let g = gcd(self.d, o.d).max(1);
        let (a, b) = (self.d / g, o.d / g);
        Rat::new(self.n.checked_mul(b)?.checked_add(o.n.checked_mul(a)?)?, self.d.checked_mul(b)?)
    }
    pub(crate) fn neg(self) -> Rat {
        Rat { n: -self.n, d: self.d }
    }
    pub(crate) fn mul(self, o: Rat) -> Option<Rat> {
        let (g1, g2) = (gcd(self.n, o.d).max(1), gcd(o.n, self.d).max(1));
        Rat::new((self.n / g1).checked_mul(o.n / g2)?, (self.d / g2).checked_mul(o.d / g1)?)
    }
    pub(crate) fn div(self, o: Rat) -> Option<Rat> {
        if o.n == 0 {
            return None;
        }
        self.mul(Rat { n: o.d * o.n.signum(), d: o.n.abs() })
    }
    pub(crate) fn f(self) -> f64 {
        self.n as f64 / self.d as f64
    }
    /// 浮動小数点数をぴったり表す分数。有限桁の小数（打った数・刻みのスライダー。小数点以下 22 桁まで、n は 2⁵³ まで）は n / 10ᵈ
    /// （0.9999999 = 9999999 / 10⁷。#141）、ほかは分母が 10⁶ 以下の分数（連分数。1/3 など）。無ければ None（π などは多項式の係数にしない）
    pub(crate) fn from_f64(v: f64) -> Option<Rat> {
        if !v.is_finite() || v.abs() > 1e15 {
            return None;
        }
        let text = format!("{v}");
        if let Some((_, frac)) = text.split_once('.') {
            let d = frac.len() as i32;
            let scale = 10f64.powi(d);
            let n = (v * scale).round();
            if d <= 22 && !text.contains('e') && n.abs() <= 9_007_199_254_740_992.0 && n / scale == v {
                return Rat::new(n as i128, 10i128.pow(d as u32));
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

/// 多項式（項は単項式の大きい順。係数は 0 でない）
#[derive(Clone, Debug, PartialEq)]
struct Poly(Vec<(Mono, Rat)>);

impl Poly {
    fn zero() -> Poly {
        Poly(Vec::new())
    }
    fn constant(c: Rat) -> Poly {
        if c.n == 0 { Poly::zero() } else { Poly(vec![([0; 3], c)]) }
    }
    fn var(k: usize) -> Poly {
        let mut m = [0; 3];
        m[k] = 1;
        Poly(vec![(m, Rat::ONE)])
    }
    fn is_zero(&self) -> bool {
        self.0.is_empty()
    }
    fn lead(&self) -> (Mono, Rat) {
        self.0[0]
    }
    /// 項を並べ直して、同じ単項式をまとめる
    fn normalize(mut terms: Vec<(Mono, Rat)>) -> Option<Poly> {
        terms.sort_by(|a, b| lex(&b.0, &a.0));
        let mut out: Vec<(Mono, Rat)> = Vec::with_capacity(terms.len());
        for (m, c) in terms {
            match out.last_mut() {
                Some(last) if last.0 == m => last.1 = last.1.add(c)?,
                _ => out.push((m, c)),
            }
        }
        out.retain(|t| t.1.n != 0);
        Some(Poly(out))
    }
    fn add(&self, o: &Poly) -> Option<Poly> {
        Poly::normalize(self.0.iter().chain(&o.0).copied().collect())
    }
    fn sub(&self, o: &Poly) -> Option<Poly> {
        Poly::normalize(self.0.iter().copied().chain(o.0.iter().map(|&(m, c)| (m, c.neg()))).collect())
    }
    fn mul(&self, o: &Poly) -> Option<Poly> {
        let mut terms = Vec::with_capacity(self.0.len() * o.0.len());
        for &(m1, c1) in &self.0 {
            for &(m2, c2) in &o.0 {
                let m = [m1[0].checked_add(m2[0])?, m1[1].checked_add(m2[1])?, m1[2].checked_add(m2[2])?];
                terms.push((m, c1.mul(c2)?));
            }
        }
        Poly::normalize(terms)
    }
    /// 単項式 m・係数 c を掛ける
    fn scale(&self, m: Mono, c: Rat) -> Option<Poly> {
        let terms = self.0.iter().map(|&(mm, cc)| Some(([mm[0] + m[0], mm[1] + m[1], mm[2] + m[2]], cc.mul(c)?))).collect::<Option<Vec<_>>>()?;
        Some(Poly(terms))
    }
    fn monic(&self) -> Option<Poly> {
        let c = self.lead().1;
        Some(Poly(self.0.iter().map(|&(m, cc)| Some((m, cc.div(c)?))).collect::<Option<Vec<_>>>()?))
    }
    fn degree_in(&self, k: usize) -> u16 {
        self.0.iter().map(|t| t.0[k]).max().unwrap_or(0)
    }
    /// 変数 k を含むか
    fn uses(&self, k: usize) -> bool {
        self.0.iter().any(|t| t.0[k] > 0)
    }
}

fn divides(a: &Mono, b: &Mono) -> bool {
    (0..3).all(|k| a[k] <= b[k])
}

/// p の、g の並びによる正規形（全部の項を簡約する）
fn reduce(p: &Poly, g: &[Poly]) -> Option<Poly> {
    let mut p = p.clone();
    let mut out: Vec<(Mono, Rat)> = Vec::new();
    let mut steps = 0;
    while !p.is_zero() {
        steps += 1;
        if steps > 20_000 {
            return None;
        }
        let (m, c) = p.lead();
        match g.iter().find(|q| divides(&q.lead().0, &m)) {
            Some(q) => {
                let (qm, qc) = q.lead();
                let shift = [m[0] - qm[0], m[1] - qm[1], m[2] - qm[2]];
                p = p.sub(&q.scale(shift, c.div(qc)?)?)?;
            }
            None => {
                out.push((m, c));
                p.0.remove(0);
            }
        }
    }
    Some(Poly(out))
}

fn s_poly(f: &Poly, g: &Poly) -> Option<Poly> {
    let ((fm, fc), (gm, gc)) = (f.lead(), g.lead());
    let l = [fm[0].max(gm[0]), fm[1].max(gm[1]), fm[2].max(gm[2])];
    let a = f.scale([l[0] - fm[0], l[1] - fm[1], l[2] - fm[2]], Rat::ONE.div(fc)?)?;
    let b = g.scale([l[0] - gm[0], l[1] - gm[1], l[2] - gm[2]], Rat::ONE.div(gc)?)?;
    a.sub(&b)
}

/// 辞書式順序の被約グレブナー基底（Buchberger 法）。大きくなりすぎる・係数があふれるときは None
fn groebner(fs: Vec<Poly>) -> Option<Vec<Poly>> {
    let mut g: Vec<Poly> = fs.into_iter().filter(|p| !p.is_zero()).map(|p| p.monic()).collect::<Option<_>>()?;
    let mut pairs: Vec<(usize, usize)> = (0..g.len()).flat_map(|j| (0..j).map(move |i| (i, j))).collect();
    let mut rounds = 0;
    while let Some((i, j)) = pairs.pop() {
        rounds += 1;
        if rounds > 3000 || g.len() > 60 {
            return None;
        }
        let (a, b) = (g[i].lead().0, g[j].lead().0);
        // 先頭項が互いに素なら、S 多項式は 0 に簡約される（Buchberger の第 1 基準）
        if (0..3).all(|k| a[k] == 0 || b[k] == 0) {
            continue;
        }
        let r = reduce(&s_poly(&g[i], &g[j])?, &g)?;
        if !r.is_zero() {
            g.push(r.monic()?);
            let n = g.len() - 1;
            pairs.extend((0..n).map(|k| (k, n)));
        }
    }
    // 最小の基底（先頭項がほかの先頭項で割り切れるものを除く）にして、互いに簡約する
    let mut min: Vec<Poly> = Vec::new();
    for (k, p) in g.iter().enumerate() {
        let m = p.lead().0;
        let redundant = g.iter().enumerate().any(|(j, q)| j != k && divides(&q.lead().0, &m) && (q.lead().0 != m || j < k));
        if !redundant {
            min.push(p.clone());
        }
    }
    let mut out = Vec::with_capacity(min.len());
    for k in 0..min.len() {
        let others: Vec<Poly> = min.iter().enumerate().filter(|(j, _)| *j != k).map(|(_, p)| p.clone()).collect();
        let r = reduce(&min[k], &others)?;
        if !r.is_zero() {
            out.push(r.monic()?);
        }
    }
    out.sort_by(|a, b| lex(&a.lead().0, &b.lead().0));
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
                    pa.scale([0; 3], Rat::ONE.div(pb.0[0].1)?)
                }
                BinOp::Pow => {
                    let k = match &**b {
                        Expr::Num(k) if *k >= 0.0 && k.fract() == 0.0 && *k <= 16.0 => *k as u32,
                        _ => return None,
                    };
                    let mut r = Poly::constant(Rat::ONE);
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

/// 多項式に、変数 k より後ろの変数の値を入れて、変数 k の 1 次元の多項式（係数は低い次数から）にする
fn univariate(p: &Poly, k: usize, vals: &[f64; 3]) -> Vec<f64> {
    let mut c = vec![0.0; p.degree_in(k) as usize + 1];
    for &(m, r) in &p.0 {
        let mut v = r.f();
        for j in k + 1..3 {
            v *= vals[j].powi(m[j] as i32);
        }
        c[m[k] as usize] += v;
    }
    c
}

/// 連立方程式 eqs = 0（変数は names。2 つか 3 つ）の実数の解（有限個のときだけ）。多項式でない・解が無限個・
/// 計算が大きくなりすぎるときは None（呼び出し側が今までの道で探す）。戻り値の点は names の順（足りない成分は 0）
pub fn solve(eqs: &[Expr], names: &[&str], params: &Params) -> Option<Vec<[f64; 3]>> {
    let n = names.len();
    if !(2..=3).contains(&n) || eqs.len() < n {
        return None;
    }
    let polys = eqs.iter().map(|e| from_expr(e, names, params)).collect::<Option<Vec<_>>>()?;
    let g = groebner(polys)?;
    if g.iter().any(|p| p.0.len() == 1 && p.lead().0 == [0; 3]) {
        return Some(Vec::new()); // 基底が {1}: 解が無い
    }
    // 変数 k を先頭項に持つ多項式（k より前の変数を含まない）の並び
    let level = |k: usize| -> Vec<&Poly> { g.iter().filter(|p| (0..k).all(|j| !p.uses(j)) && p.uses(k)).collect() };
    let mut sols: Vec<[f64; 3]> = vec![[0.0; 3]];
    for k in (0..n).rev() {
        let ps = level(k);
        // 有限個の解なら、どの変数にも、それより前の変数を含まない式で、先頭がその変数の純粋なべきのものがある
        if !ps.iter().any(|p| { let m = p.lead().0; m[k] > 0 && (0..3).all(|j| j == k || m[j] == 0) }) {
            return None;
        }
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
    Some(sols)
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
}

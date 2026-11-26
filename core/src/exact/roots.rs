//! 有理数係数の 1 変数の多項式の実根の分離（計画 `docs/plan-exact-core.md` の EX7 の残りの一部）。
//!
//! 重根を除いた部分 $q = p / \gcd(p, p')$ の実根を、互いに重ならない区間に 1 つずつ閉じ込める
//! （Vincent–Collins–Akritas の方法: $(0, 1)$ の根の数の上限をデカルトの符号の規則で数え、0 か 1 になるまで区間を半分に分ける）。
//! 根の大きさの上限 $B = 2^e$（コーシーの上界）で $q(\pm Bx)$ にして、正の根と負の根を別に $(0, 1)$ で探す。
//! 区間は [`refine`] で好きな幅まで詰められる（端の値の符号を厳密に調べる二分法）。
//!
//! 係数が大きくても（連立方程式の消去式は数千ビット）、f64 の根探しのような桁落ちが無い。

use super::num::{self, Int, Nat, Rat};
use super::poly::{self, Poly};
use super::{Budget, TimeUp};
use dashu::base::BitTest;

/// 実根を閉じ込めた区間。`lo == hi` なら、ちょうどその有理数が根。そうでなければ開区間 (lo, hi) に根がちょうど 1 つあり、端は根でない
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interval {
    pub lo: Rat,
    pub hi: Rat,
}

impl Interval {
    pub fn is_exact(&self) -> bool {
        self.lo == self.hi
    }
    /// 真ん中の値の f64
    pub fn mid_f64(&self) -> f64 {
        ((self.lo.clone() + &self.hi) / Rat::from(Int::from(2))).to_f64().value()
    }
}

/// 重根を除き、係数を整数にした多項式（係数は低い次数から）
fn squarefree_ints(p: &Poly) -> Vec<Int> {
    let g = poly::gcd(p, &p.deriv());
    let q = if g.deg() >= 1 { p.divrem(&g).0 } else { p.clone() };
    let (_, prim) = q.primitive();
    prim.0.iter().map(|c| num::as_int(c).unwrap()).collect()
}

/// 係数の符号の変わる回数（0 は飛ばす）
fn variations(c: &[Int]) -> usize {
    let mut last = 0i8;
    let mut n = 0;
    for a in c {
        let s = if *a > Int::ZERO { 1 } else if *a < Int::ZERO { -1 } else { 0 };
        if s != 0 {
            if last != 0 && s != last {
                n += 1;
            }
            last = s;
        }
    }
    n
}

/// f(x + 1)（テイラーの移動。係数は低い次数から）
fn shift1(c: &[Int], budget: &Budget) -> Result<Vec<Int>, TimeUp> {
    let mut a = c.to_vec();
    let n = a.len();
    budget.charge((n * n) as u64 / 2 + 1)?;
    for i in 0..n {
        for j in (i..n.saturating_sub(1)).rev() {
            let t = a[j + 1].clone();
            a[j] += t;
        }
    }
    Ok(a)
}

/// $(0, 1)$ の根の数の上限: $(x + 1)^n f(1/(x + 1))$ の係数の符号の変化
fn bound01(c: &[Int], budget: &Budget) -> Result<usize, TimeUp> {
    let rev: Vec<Int> = c.iter().rev().cloned().collect();
    Ok(variations(&shift1(&rev, budget)?))
}

/// q(s·2ᵉ·x) の (0, 1) の根を閉じ込める。結果は (c, k, ちょうど): t ∈ (c/2ᵏ, (c+1)/2ᵏ)、ちょうどなら t = c/2ᵏ
fn isolate01(c: Vec<Int>, budget: &Budget) -> Result<Vec<(Int, u32, bool)>, TimeUp> {
    let mut out = Vec::new();
    let mut stack: Vec<(Vec<Int>, Int, u32)> = vec![(c, Int::ZERO, 0)];
    while let Some((mut f, cc, k)) = stack.pop() {
        if k > 4000 {
            return Err(TimeUp);
        }
        // 左の端が根
        if f.first().is_some_and(|a| *a == Int::ZERO) {
            if k > 0 {
                out.push((cc.clone(), k, true));
            }
            while f.first().is_some_and(|a| *a == Int::ZERO) {
                f.remove(0);
            }
        }
        if f.len() < 2 {
            continue;
        }
        match bound01(&f, budget)? {
            0 => continue,
            1 => {
                out.push((cc, k, false));
                continue;
            }
            _ => {}
        }
        // 半分に: 左 2ⁿ f(x/2)、右 2ⁿ f((x+1)/2)
        let n = f.len() - 1;
        let left: Vec<Int> = f.iter().enumerate().map(|(i, a)| a.clone() << (n - i)).collect();
        let right = shift1(&left, budget)?;
        let c2 = cc << 1;
        stack.push((right, c2.clone() + Int::ONE, k + 1));
        stack.push((left, c2, k + 1));
    }
    Ok(out)
}

/// p の相異なる実根を、小さい順に区間で閉じ込める（p は 0 でない）
pub fn isolate(p: &Poly, budget: &Budget) -> Result<Vec<Interval>, TimeUp> {
    if p.deg() < 1 {
        return Ok(Vec::new());
    }
    isolate_squarefree(squarefree_ints(p), budget)
}

/// [`isolate`] の、重根を除いて整数の係数にした後
fn isolate_squarefree(mut q: Vec<Int>, budget: &Budget) -> Result<Vec<Interval>, TimeUp> {
    let mut out = Vec::new();
    if q[0] == Int::ZERO {
        out.push(Interval { lo: Rat::ZERO, hi: Rat::ZERO });
        q.remove(0);
    }
    if q.len() < 2 {
        return Ok(out);
    }
    // コーシーの上界: |根| < 1 + max |aᵢ / aₙ| ≤ 2ᵉ
    let n = q.len() - 1;
    let lead_bits = q[n].bit_len() as i64;
    let top = q[..n].iter().map(|a| a.bit_len() as i64).max().unwrap_or(0);
    let e = (top - lead_bits + 2).max(1) as usize;
    for sign in [-1i8, 1] {
        // q(s · 2ᵉ · x)
        let c: Vec<Int> = q
            .iter()
            .enumerate()
            .map(|(i, a)| {
                let v = a.clone() << (e * i);
                if sign < 0 && i % 2 == 1 { -v } else { v }
            })
            .collect();
        for (cc, k, exact) in isolate01(c, budget)? {
            // x = s · 2ᵉ · t、t ∈ (cc/2ᵏ, (cc+1)/2ᵏ)
            let scale = |m: Int| -> Rat {
                let v = if e >= k as usize { Rat::from(m << (e - k as usize)) } else { Rat::from_parts(m, Nat::ONE << (k as usize - e)) };
                if sign < 0 { -v } else { v }
            };
            let a = scale(cc.clone());
            if exact {
                out.push(Interval { lo: a.clone(), hi: a });
            } else {
                let b = scale(cc + Int::ONE);
                let (lo, hi) = if sign < 0 { (b, a) } else { (a, b) };
                out.push(Interval { lo, hi });
            }
        }
    }
    out.sort_by(|a, b| a.lo.cmp(&b.lo));
    Ok(out)
}

/// 区間の幅が `width` 以下になるまで詰める（p は [`isolate`] に渡したもの。根は区間の中に 1 つ）
pub fn refine(p: &Poly, iv: &Interval, width: &Rat, budget: &Budget) -> Result<Interval, TimeUp> {
    let mut iv = iv.clone();
    if iv.is_exact() {
        return Ok(iv);
    }
    let q = Poly::new(squarefree_ints(p).into_iter().map(Rat::from).collect());
    let sign = |x: &Rat| num::sign(&q.eval(x));
    // 端の符号（端がもとの p の根のとき（0 の根のすぐ隣の区間）は、反対の端の符号の逆）
    let s_lo = match sign(&iv.lo) {
        0 => -sign(&iv.hi),
        s => s,
    };
    let half = Rat::from_parts(Int::ONE, Nat::from(2u8));
    while iv.hi.clone() - &iv.lo > *width {
        budget.charge(q.0.len() as u64 * 4)?;
        let m = (iv.lo.clone() + &iv.hi) * &half;
        match sign(&m) {
            0 => return Ok(Interval { lo: m.clone(), hi: m }),
            s if s == s_lo => iv.lo = m,
            _ => iv.hi = m,
        }
    }
    Ok(iv)
}

/// 1 次・2 次の式の実根（小さい順）。2 次は判別式の符号を厳密に決め、根は桁落ちしない形 $q = -\frac{b + \operatorname{sgn}(b)\sqrt D}{2}$ で
fn small_roots(p: &Poly) -> Vec<f64> {
    let (_, prim) = p.primitive();
    let c: Vec<Int> = prim.0.iter().map(|x| num::as_int(x).unwrap()).collect();
    match c.len() {
        0 | 1 => Vec::new(),
        2 => vec![Rat::from_parts_signed(-c[0].clone(), c[1].clone()).to_f64().value()],
        _ => {
            let (a, b, cc) = (&c[2], &c[1], &c[0]);
            let d = b.clone() * b - Int::from(4) * a * cc;
            if d < Int::ZERO {
                return Vec::new();
            }
            if d == Int::ZERO {
                return vec![Rat::from_parts_signed(-b.clone(), Int::from(2) * a).to_f64().value()];
            }
            // 係数を 2^s でそろえて割り、f64 に収める（D は 2^{2s} で割る）
            let bits = [a, b, cc].iter().map(|x| x.bit_len()).max().unwrap_or(0);
            let s = bits.saturating_sub(500);
            let fa = Rat::from_parts(a.clone(), Nat::ONE << s).to_f64().value();
            let fb = Rat::from_parts(b.clone(), Nat::ONE << s).to_f64().value();
            let fc = Rat::from_parts(cc.clone(), Nat::ONE << s).to_f64().value();
            let fd = Rat::from_parts(d, Nat::ONE << (2 * s)).to_f64().value();
            let sgn = if fb < 0.0 { -1.0 } else { 1.0 };
            let q = -0.5 * (fb + sgn * fd.sqrt());
            let (r1, r2) = (q / fa, fc / q);
            if r1 < r2 { vec![r1, r2] } else { vec![r2, r1] }
        }
    }
}

/// 2 進の分数 m / 2ᵏ の符号を、整数だけで（q は整数の係数。q(m/2ᵏ)·2^{kn} = Σ aᵢ mⁱ 2^{k(n−i)}）
fn dyadic_sign(q: &[Int], m: &Int, k: usize) -> i32 {
    let n = q.len() - 1;
    let mut acc = q[n].clone();
    for i in (0..n).rev() {
        acc = acc * m + (q[i].clone() << (k * (n - i)));
    }
    if acc > Int::ZERO { 1 } else if acc < Int::ZERO { -1 } else { 0 }
}

/// 2 進の分数（分母が 2 の累乗）を (分子, 指数) に
fn to_dyadic(r: &Rat) -> (Int, usize) {
    let d = r.denominator();
    (r.numerator().clone(), d.bit_len() - 1)
}

/// 実根の f64 の値（小さい順。区間を、根の大きさに比べて 2⁻⁶⁰ の幅まで詰めた真ん中）
pub fn real_roots_f64(p: &Poly, budget: &Budget) -> Result<Vec<f64>, TimeUp> {
    if p.deg() <= 2 {
        return Ok(small_roots(p));
    }
    let q = squarefree_ints(p);
    let ivs = isolate_squarefree(q.clone(), budget)?;
    let mut out = Vec::new();
    for iv in ivs {
        if iv.is_exact() {
            out.push(iv.lo.to_f64().value());
            continue;
        }
        // 端は isolate が作った 2 進の分数。同じ指数にそろえる
        let ((mut a, ka), (mut b, kb)) = (to_dyadic(&iv.lo), to_dyadic(&iv.hi));
        let mut k = ka.max(kb);
        a <<= k - ka;
        b <<= k - kb;
        let s_lo = match dyadic_sign(&q, &a, k) {
            0 => -dyadic_sign(&q, &b, k),
            s => s,
        };
        let mut exact = None;
        for _ in 0..4000 {
            // 幅が、端の小さいほうの大きさの 2⁻⁶⁰ 以下（0 をまたがない）
            let same_side = (a > Int::ZERO && b > Int::ZERO) || (a < Int::ZERO && b < Int::ZERO);
            if same_side {
                let small = if a > Int::ZERO { a.clone() } else { -b.clone() };
                if ((b.clone() - &a) << 60) <= small {
                    break;
                }
            }
            budget.charge(q.len() as u64 * (1 + (k * q.len()) as u64 / 64))?;
            let m = a.clone() + &b;
            k += 1;
            a <<= 1;
            b <<= 1;
            match dyadic_sign(&q, &m, k) {
                0 => {
                    exact = Some(Rat::from_parts(m, Nat::ONE << k));
                    break;
                }
                s if s == s_lo => a = m,
                _ => b = m,
            }
        }
        out.push(match exact {
            Some(r) => r.to_f64().value(),
            None => Rat::from_parts(a + b, Nat::ONE << (k + 1)).to_f64().value(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(c: &[i64]) -> Poly {
        Poly::new(c.iter().map(|&a| Rat::from(Int::from(a))).collect())
    }

    #[test]
    fn roots() {
        let b = Budget::unlimited();
        // (x − 1)(x + 2)(x − 3) = x³ − 2x² − 5x + 6
        assert_eq!(real_roots_f64(&p(&[6, -5, -2, 1]), &b).unwrap(), [-2.0, 1.0, 3.0]);
        // x² − 2: ±√2
        let r = real_roots_f64(&p(&[-2, 0, 1]), &b).unwrap();
        assert!(r.len() == 2 && (r[1] - 2f64.sqrt()).abs() < 1e-15 && (r[0] + 2f64.sqrt()).abs() < 1e-15, "{r:?}");
        // 重根: (x − 1)² (x + 1) → −1, 1
        assert_eq!(real_roots_f64(&p(&[1, -1, -1, 1]), &b).unwrap(), [-1.0, 1.0]);
        // 実根なし
        assert!(real_roots_f64(&p(&[1, 0, 1]), &b).unwrap().is_empty());
        // 0 の根と、近い 2 根: x (x − 1/1000)(x − 2/1000) → 1000000 x³ − 3000 x² + 2 x
        let r = real_roots_f64(&p(&[0, 2, -3000, 1_000_000]), &b).unwrap();
        assert!(r.len() == 3 && r[0] == 0.0 && (r[1] - 0.001).abs() < 1e-18 && (r[2] - 0.002).abs() < 1e-18, "{r:?}");
        // ウィルキンソンの多項式 ∏_{k=1}^{20} (x − k)（f64 の係数では根が大きくずれる）
        let mut w = p(&[1]);
        for k in 1..=20 {
            w = w.mul(&p(&[-k, 1]));
        }
        let r = real_roots_f64(&w, &b).unwrap();
        assert_eq!(r, (1..=20).map(|k| k as f64).collect::<Vec<_>>());
        // x³ − 2 の実根 ∛2
        let r = real_roots_f64(&p(&[-2, 0, 0, 1]), &b).unwrap();
        assert!(r.len() == 1 && (r[0] - 2f64.cbrt()).abs() < 1e-15, "{r:?}");
    }
}

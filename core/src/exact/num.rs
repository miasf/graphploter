//! 大きな整数・分数と、整数の計算（計画 `docs/plan-exact-core.md` の EX1）。
//!
//! 数そのものは dashu（MIT / Apache-2.0）の `IBig`・`UBig`・`RBig` を使う。ここは、その上の数論の関数:
//! 字句の f64 を分数に戻す・GCD・合同式の逆元・中国剰余定理・オイラーの φ・階乗・二項係数・
//! 素数判定（Miller–Rabin と BPSW）・素因数分解（Pollard–Brent の ρ）・ベルヌーイ数。

use super::{Budget, TimeUp};
use dashu::base::{Abs, BitTest, ExtendedGcd, Gcd, Sign, UnsignedAbs};
use dashu::integer::fast_div::ConstDivisor;
pub use dashu::integer::{IBig, UBig};
pub use dashu::rational::RBig;
use std::cell::RefCell;

/// 整数
pub type Int = IBig;
/// 0 以上の整数
pub type Nat = UBig;
/// 分数（いつも既約で、分母は正）
pub type Rat = RBig;

/// 字句の f64 を、それを正しく戻す最短の 10 進表記の分数にする（`0.3` → 3/10、`9.81` → 981/100）。非有限は None。
///
/// Rust の `{}` の書式は、f64 を正しく戻す最短の桁を、指数を使わない形で出す（`1e300` も全部の桁）
pub fn rat_of_literal(v: f64) -> Option<Rat> {
    if !v.is_finite() {
        return None;
    }
    let s = format!("{v}");
    let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
    let digits: Int = format!("{int}{frac}").parse().ok()?;
    Some(Rat::from_parts(digits, Nat::from(10u8).pow(frac.len())))
}

/// 分数が整数ならその整数
pub fn as_int(r: &Rat) -> Option<Int> {
    (*r.denominator() == Nat::ONE).then(|| r.numerator().clone())
}

/// 最大公約数（0 以上。gcd(0, 0) = 0、gcd(a, 0) = |a|）
pub fn gcd(a: &Int, b: &Int) -> Int {
    if a.is_zero() {
        return b.clone().abs();
    }
    if b.is_zero() {
        return a.clone().abs();
    }
    Int::from(a.clone().gcd(b.clone()))
}

pub fn lcm(a: &Int, b: &Int) -> Int {
    if a.is_zero() || b.is_zero() {
        return Int::ZERO;
    }
    let g = gcd(a, b);
    (a / &g * b).abs()
}

/// $a x \equiv 1 \pmod m$ の $x$（$0 \le x < m$）。$\gcd(a, m) \ne 1$ か $m \le 0$ なら None
pub fn mod_inverse(a: &Int, m: &Int) -> Option<Int> {
    if *m <= Int::ONE {
        return (*m == Int::ONE).then_some(Int::ZERO);
    }
    let (g, x, _) = a.clone().gcd_ext(m.clone());
    if g != Nat::ONE {
        return None;
    }
    Some(modulo(&x, m))
}

/// 0 以上 m 未満の余り（m > 0）
pub fn modulo(a: &Int, m: &Int) -> Int {
    let r = a % m;
    if r < Int::ZERO {
        r + m
    } else {
        r
    }
}

/// 中国剰余定理: $x \equiv r_i \pmod{m_i}$ の全部を満たす $x$ と、法 $\operatorname{lcm}(m_i)$。両立しなければ None
pub fn crt(parts: &[(Int, Int)]) -> Option<(Int, Int)> {
    let mut x = Int::ZERO;
    let mut m = Int::ONE;
    for (r, mi) in parts {
        if *mi <= Int::ZERO {
            return None;
        }
        let r = modulo(r, mi);
        let g = gcd(&m, mi);
        let diff = &r - &x;
        if !(&diff % &g).is_zero() {
            return None;
        }
        // x + m t ≡ r (mod mi) → (m/g) t ≡ diff/g (mod mi/g)
        let m_g = &m / &g;
        let mi_g = mi / &g;
        let inv = mod_inverse(&modulo(&m_g, &mi_g), &mi_g)?;
        let t = modulo(&(&diff / &g * inv), &mi_g);
        x += &m * t;
        m *= mi_g;
        x = modulo(&x, &m);
    }
    Some((x, m))
}

/// $a^e \bmod m$（$e \ge 0$、$m > 0$）。$a^e$ を作らずに計算する
pub fn mod_pow(a: &Int, e: &Nat, m: &Int) -> Int {
    if *m == Int::ONE {
        return Int::ZERO;
    }
    let ring = ConstDivisor::new(m.unsigned_abs());
    let base = modulo(a, m).unsigned_abs();
    Int::from(ring.reduce(base).pow(e).residue())
}

/// $n!$（二分割で掛ける: 小さい数どうしを先に掛けると、大きな数の掛け算の回数が減る）
pub fn factorial(n: u64, budget: &Budget) -> Result<Int, TimeUp> {
    budget.charge(n / 8 + 1)?;
    Ok(Int::from(range_product(1, n + 1)))
}

/// $\prod_{k=a}^{b-1} k$
fn range_product(a: u64, b: u64) -> Nat {
    if b <= a {
        return Nat::ONE;
    }
    if b - a <= 16 {
        let mut p = Nat::ONE;
        for k in a..b {
            p *= Nat::from(k);
        }
        return p;
    }
    let mid = a + (b - a) / 2;
    range_product(a, mid) * range_product(mid, b)
}

/// 二項係数 $\binom{n}{k}$。$n$ が負なら $(-1)^k\binom{k-n-1}{k}$、$k < 0$ と $0 \le n < k$ は 0
pub fn binomial(n: &Int, k: &Int, budget: &Budget) -> Result<Int, TimeUp> {
    if *k < Int::ZERO {
        return Ok(Int::ZERO);
    }
    if *n < Int::ZERO {
        let m = k - n - Int::ONE;
        let v = binomial(&m, k, budget)?;
        return Ok(if k.bit(0) { -v } else { v });
    }
    if k > n {
        return Ok(Int::ZERO);
    }
    let k = std::cmp::min(k.clone(), n - k);
    let Ok(ku) = u64::try_from(&k) else { return Err(TimeUp) };
    budget.charge(ku / 8 + 1)?;
    let mut r = Int::ONE;
    for i in 0..ku {
        // r = C(n, i)、C(n, i+1) = C(n, i) (n − i) / (i + 1)（割り切れる）
        r = r * (n - Int::from(i)) / Int::from(i + 1);
    }
    Ok(r)
}

/// 多項係数 $\frac{(k_1 + \cdots + k_m)!}{k_1! \cdots k_m!}$
pub fn multinomial(ks: &[u64], budget: &Budget) -> Result<Int, TimeUp> {
    let mut total = Int::ZERO;
    let mut r = Int::ONE;
    for &k in ks {
        let before = total.clone();
        total += Int::from(k);
        r *= binomial(&total, &(&total - &before), budget)?;
    }
    Ok(r)
}

/// 第 2 種スターリング数 $S(n, k)$（$n$ 個を $k$ 個の空でない組に分ける数）
pub fn stirling2(n: u64, k: u64, budget: &Budget) -> Result<Int, TimeUp> {
    if k > n {
        return Ok(Int::ZERO);
    }
    budget.charge(n * k / 64 + 1)?;
    // 漸化式 S(i, j) = j S(i−1, j) + S(i−1, j−1) の表を 1 行ずつ
    let k = k as usize;
    let mut row = vec![Int::ZERO; k + 1];
    row[0] = Int::ONE;
    for _ in 0..n {
        for j in (1..=k).rev() {
            row[j] = Int::from(j) * &row[j] + &row[j - 1];
        }
        row[0] = Int::ZERO;
    }
    Ok(row[k].clone())
}

/// $a$ のちょうど $n$ 乗根（整数で割り切れるときだけ）。負の $a$ は $n$ が奇数のときだけ
pub fn exact_root(a: &Int, n: usize) -> Option<Int> {
    if n == 0 {
        return None;
    }
    if *a < Int::ZERO {
        if n % 2 == 0 {
            return None;
        }
        return exact_root(&-a, n).map(|r| -r);
    }
    let r = Int::from(a.unsigned_abs().nth_root(n));
    (r.pow(n) == *a).then_some(r)
}

/// Miller–Rabin の 1 回（底 b）。n は奇数で 3 以上、n − 1 = d·2^s
fn strong_probable_prime(n: &Int, d: &Nat, s: usize, b: u64, ring: &ConstDivisor) -> bool {
    let nm1 = n - Int::ONE;
    let nm1 = nm1.unsigned_abs();
    let mut x = ring.reduce(Nat::from(b)).pow(d);
    let one = ring.reduce(Nat::ONE);
    let minus_one = ring.reduce(nm1);
    if x == one || x == minus_one {
        return true;
    }
    for _ in 1..s {
        x = x.sqr();
        if x == minus_one {
            return true;
        }
        if x == one {
            return false;
        }
    }
    false
}

/// 強いリュカの確率的素数判定（Selfridge の方法 A で D を選ぶ）。n は奇数で平方数でない
fn strong_lucas(n: &Int) -> bool {
    // D = 5, −7, 9, −11, … で ヤコビ記号 (D/n) = −1 になる最初のもの
    let mut d: i64 = 5;
    loop {
        let j = jacobi(&Int::from(d), n);
        if j == -1 {
            break;
        }
        if j == 0 && Int::from(d.unsigned_abs()) != *n {
            return false;
        }
        d = if d > 0 { -(d + 2) } else { -d + 2 };
        if d.abs() > 10_000 {
            return false;
        }
    }
    let p = Int::ONE;
    let q = Int::from((1 - d) / 4);
    // n + 1 = d' 2^s
    let np1 = n + Int::ONE;
    let s = np1.clone().unsigned_abs().trailing_zeros().unwrap_or(0);
    let dd = (&np1 >> s).unsigned_abs();
    // U_k, V_k, Q^k を 2 進の上の桁から（k = d'）
    let modn = |a: Int| modulo(&a, n);
    let half = |a: Int| {
        let a = if a.bit(0) { a + n } else { a };
        modn(a >> 1)
    };
    let mut u = Int::ZERO;
    let mut v = Int::from(2);
    let mut qk = Int::ONE;
    let dbig = Int::from(d);
    for i in (0..dd.bit_len()).rev() {
        // 2 倍: U_{2k} = U_k V_k、V_{2k} = V_k² − 2 Q^k
        u = modn(&u * &v);
        v = modn(&v * &v - Int::from(2) * &qk);
        qk = modn(&qk * &qk);
        if dd.bit(i) {
            // 1 足す: U_{k+1} = (P U + V)/2、V_{k+1} = (D U + P V)/2
            let nu = half(&p * &u + &v);
            let nv = half(&dbig * &u + &p * &v);
            u = nu;
            v = nv;
            qk = modn(&qk * &q);
        }
    }
    if u.is_zero() || v.is_zero() {
        return true;
    }
    for _ in 1..s {
        v = modn(&v * &v - Int::from(2) * &qk);
        qk = modn(&qk * &qk);
        if v.is_zero() {
            return true;
        }
    }
    false
}

/// ヤコビ記号 $(a/n)$（n は正の奇数）
fn jacobi(a: &Int, n: &Int) -> i32 {
    let mut a = modulo(a, n);
    let mut n = n.clone();
    let mut t = 1;
    while !a.is_zero() {
        while !a.bit(0) {
            a >>= 1;
            let r = (&n % Int::from(8)).to_f64().value() as i64;
            if r == 3 || r == 5 {
                t = -t;
            }
        }
        std::mem::swap(&mut a, &mut n);
        let a4 = (&a % Int::from(4)).to_f64().value() as i64;
        let n4 = (&n % Int::from(4)).to_f64().value() as i64;
        if a4 == 3 && n4 == 3 {
            t = -t;
        }
        a = modulo(&a, &n);
    }
    if n == Int::ONE {
        t
    } else {
        0
    }
}

const SMALL_PRIMES: [u64; 13] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41];

/// 素数か。$3.3 \times 10^{24}$ 未満は決まった 13 個の底の Miller–Rabin で確定。
/// それより大きい数は BPSW（底 2 の Miller–Rabin と強いリュカの判定。反例は知られていない。「たぶん素数」）
pub fn is_prime(n: &Int) -> bool {
    if *n < Int::from(2) {
        return false;
    }
    for &p in &SMALL_PRIMES {
        let p = Int::from(p);
        if *n == p {
            return true;
        }
        if (n % &p).is_zero() {
            return false;
        }
    }
    let nm1 = (n - Int::ONE).unsigned_abs();
    let s = nm1.trailing_zeros().unwrap_or(0);
    let d = &nm1 >> s;
    let ring = ConstDivisor::new(n.unsigned_abs());
    let bound: Int = "3317044064679887385961981".parse().unwrap();
    if *n < bound {
        return SMALL_PRIMES.iter().all(|&b| strong_probable_prime(n, &d, s, b, &ring));
    }
    if !strong_probable_prime(n, &d, s, 2, &ring) {
        return false;
    }
    if exact_root(n, 2).is_some() {
        return false;
    }
    strong_lucas(n)
}

/// 素因数分解の結果: 素因数と指数（小さい順）と、予算内に分解できなかった部分（1 より大きい合成数。無ければ None）
pub type Factors = (Vec<(Int, u32)>, Option<Int>);

/// 正の整数の素因数分解。$10^4$ までの素数で割り、残りは素数判定と Pollard–Brent の ρ
pub fn factor_int(n: &Int, budget: &Budget) -> Result<Factors, TimeUp> {
    let mut n = n.abs();
    let mut out: Vec<(Int, u32)> = Vec::new();
    if n.is_zero() {
        return Ok((out, None));
    }
    let mut p = 2u64;
    while p <= 10_000 {
        let pb = Int::from(p);
        if &pb * &pb > n {
            break;
        }
        let mut e = 0;
        while (&n % &pb).is_zero() {
            n /= &pb;
            e += 1;
        }
        if e > 0 {
            out.push((pb, e));
        }
        p += if p == 2 { 1 } else { 2 };
        budget.charge(1)?;
    }
    let mut stuck: Option<Int> = None;
    let mut stack = vec![n];
    while let Some(m) = stack.pop() {
        if m == Int::ONE {
            continue;
        }
        budget.charge(m.clone().unsigned_abs().bit_len() as u64)?;
        if is_prime(&m) {
            push_factor(&mut out, m);
            continue;
        }
        if let Some(r) = perfect_power(&m) {
            for _ in 0..r.1 {
                stack.push(r.0.clone());
            }
            continue;
        }
        match pollard_brent(&m, budget)? {
            Some(f) => {
                stack.push(&m / &f);
                stack.push(f);
            }
            None => stuck = Some(stuck.map_or(m.clone(), |s| s * &m)),
        }
    }
    out.sort();
    Ok((out, stuck))
}

fn push_factor(out: &mut Vec<(Int, u32)>, p: Int) {
    match out.iter_mut().find(|(q, _)| *q == p) {
        Some(e) => e.1 += 1,
        None => out.push((p, 1)),
    }
}

/// $m = r^k$（$k \ge 2$）なら (r, k)（k はいちばん大きいもの）
fn perfect_power(m: &Int) -> Option<(Int, u32)> {
    let bits = m.unsigned_abs().bit_len();
    for k in (2..=bits.max(2)).rev() {
        if let Some(r) = exact_root(m, k) {
            if r > Int::ONE {
                return Some((r, k as u32));
            }
        }
    }
    None
}

/// Pollard–Brent の ρ で、m（合成数）の 1 でも m でもない約数を 1 つ探す。見つからなければ None
fn pollard_brent(m: &Int, budget: &Budget) -> Result<Option<Int>, TimeUp> {
    let modm = |a: Int| modulo(&a, m);
    for c in 1..20u64 {
        let c = Int::from(c);
        let f = |y: &Int| modm(y * y + &c);
        let mut y = Int::from(2);
        let mut r = 1u64;
        let mut q = Int::ONE;
        let mut g = Int::ONE;
        let mut x = y.clone();
        let mut ys = y.clone();
        while g == Int::ONE {
            x = y.clone();
            for _ in 0..r {
                y = f(&y);
            }
            let mut k = 0;
            while k < r && g == Int::ONE {
                ys = y.clone();
                let steps = std::cmp::min(128, r - k);
                for _ in 0..steps {
                    y = f(&y);
                    q = modm(q * (&x - &y).abs());
                }
                budget.charge(steps)?;
                g = gcd(&q, m);
                k += steps;
            }
            r *= 2;
            if r > 1 << 22 {
                break;
            }
        }
        if g == *m {
            // 戻って 1 歩ずつ
            loop {
                ys = f(&ys);
                g = gcd(&(&x - &ys).abs(), m);
                budget.charge(1)?;
                if g != Int::ONE {
                    break;
                }
            }
        }
        if g != *m && g != Int::ONE {
            return Ok(Some(g));
        }
    }
    Ok(None)
}

/// オイラーの $\varphi(n)$（$n \ge 1$）。分解できない部分が残れば時間切れとして扱う
pub fn totient(n: &Int, budget: &Budget) -> Result<Int, TimeUp> {
    let (fs, rest) = factor_int(n, budget)?;
    if rest.is_some() {
        return Err(TimeUp);
    }
    let mut r = Int::ONE;
    for (p, e) in fs {
        r *= (&p - Int::ONE) * p.pow(e as usize - 1);
    }
    Ok(r)
}

thread_local! {
    static BERNOULLI: RefCell<Vec<Rat>> = const { RefCell::new(Vec::new()) };
}

/// ベルヌーイ数 $B_n$（$B_1 = -\frac12$ の流儀）。
/// 漸化式 $B_m = -\frac{1}{m+1}\sum_{k=0}^{m-1}\binom{m+1}{k}B_k$。計算した値は覚えておく
pub fn bernoulli(n: usize) -> Rat {
    if n > 1 && n % 2 == 1 {
        return Rat::ZERO;
    }
    BERNOULLI.with(|cache| {
        let mut b = cache.borrow_mut();
        if b.is_empty() {
            b.push(Rat::ONE);
        }
        while b.len() <= n {
            let m = b.len();
            let mut s = Rat::ZERO;
            let mut c = Int::ONE; // C(m+1, k)
            for (k, bk) in b.iter().enumerate() {
                if !bk.is_zero() {
                    s += Rat::from(c.clone()) * bk;
                }
                c = c * Int::from(m + 1 - k) / Int::from(k + 1);
            }
            b.push(-s / Rat::from(Int::from(m + 1)));
        }
        b[n].clone()
    })
}

/// 分数の符号（−1, 0, 1）
pub fn sign(r: &Rat) -> i32 {
    if r.is_zero() {
        0
    } else if r.sign() == Sign::Negative {
        -1
    } else {
        1
    }
}

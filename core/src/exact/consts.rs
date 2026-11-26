//! 定数の原子と特殊関数の値を、好きな精度で計算する（計画 `docs/plan-exact-core.md` の EX3 の 2）。
//!
//! どれも作業の精度を `p + 64` ビットに上げて計算し、打ち切りの誤差が $2^{-p-32}$ より小さくなるように項の数を決める:
//! - オイラーの定数 $\gamma$: Brent–McMillan（$\gamma = \frac{A}{B} - \ln n$、$A = \sum \left(\frac{n^k}{k!}\right)^2 H_k$、$B = \sum \left(\frac{n^k}{k!}\right)^2$。誤差 $< \pi e^{-4n}$）
//! - カタランの定数 $G = \frac{\pi}{8}\ln(2 + \sqrt3) + \frac38\sum_{k\ge0}\frac{(k!)^2}{(2k)!\,(2k+1)^2}$（項の比は $\frac14$ に近づく）
//! - $\zeta(n)$（$n \ge 2$）: Borwein の交代級数（誤差 $\le \frac{3}{(3+\sqrt8)^m} \cdot \frac{1}{1 - 2^{1-n}}$）
//! - $\ln\Gamma(x)$（$x > 0$ の有理数）: 漸化式で $z = x + N$ にずらし、スターリングの級数。打ち切りの誤差は次の項の大きさで押さえる（$z$ が実数で正なので）
//! - $\psi(x)$: 同じく漸化式と漸近展開

use super::ball::{fl_int, fl_rat, F};
use super::num::{self, Int, Nat, Rat};
use dashu::base::BitTest;

fn guard(p: usize) -> usize {
    p + 64
}

/// $\gamma$
pub fn euler_gamma(p: usize) -> F {
    let w = guard(p);
    // 誤差 π e^{−4n} < 2^{−w}: n > w ln2 / 4
    let n = ((w as f64) * std::f64::consts::LN_2 / 4.0).ceil() as u64 + 2;
    let nf = fl_int(&Int::from(n), w);
    let n2 = &nf * &nf;
    let mut term = fl_int(&Int::ONE, w); // (n^k / k!)^2
    let mut a = fl_int(&Int::ZERO, w);
    let mut b = term.clone();
    let mut h = fl_int(&Int::ZERO, w); // H_k
    let mut k: u64 = 0;
    let tiny_bits = -(w as isize) - 16;
    loop {
        k += 1;
        let kf = fl_int(&Int::from(k), w);
        term = term * &n2 / (&kf * &kf);
        h = h + fl_int(&Int::ONE, w) / &kf;
        a = a + &term * &h;
        b = b + &term;
        // 項が B に比べて十分小さくなったら止める（k > n を過ぎると減る）
        if k > n && (term.repr().exponent() + term.repr().significand().clone().unsigned_abs_bits() as isize) < (b.repr().exponent() + b.repr().significand().clone().unsigned_abs_bits() as isize) + tiny_bits {
            break;
        }
        if k > 20 * n + 100 {
            break;
        }
    }
    let v = a / b - nf.ln();
    v.with_precision(p).value()
}

trait BitsOf {
    fn unsigned_abs_bits(self) -> usize;
}

impl BitsOf for Int {
    fn unsigned_abs_bits(self) -> usize {
        dashu::base::UnsignedAbs::unsigned_abs(self).bit_len()
    }
}

/// 2 進の大きさ（およそ log2|x|）
fn log2_of(x: &F) -> isize {
    let r = x.repr();
    if r.significand().is_zero() {
        return isize::MIN / 2;
    }
    r.exponent() + r.significand().clone().unsigned_abs_bits() as isize
}

/// カタランの定数 $G$
pub fn catalan(p: usize) -> F {
    let w = guard(p);
    let pi = F::pi(w);
    let two = fl_int(&Int::from(2), w);
    let three = fl_int(&Int::from(3), w);
    let l = (two + three.sqrt()).ln();
    let first = pi * l / fl_int(&Int::from(8), w);
    // Σ (k!)² / ((2k)! (2k+1)²)。項 t_k = (k!)²/(2k)!、t_{k+1} = t_k (k+1)² / ((2k+1)(2k+2))
    let mut t = fl_int(&Int::ONE, w);
    let mut s = fl_int(&Int::ZERO, w);
    let mut k: u64 = 0;
    loop {
        let d = fl_int(&Int::from(2 * k + 1), w);
        let term = &t / (&d * &d);
        s = s + &term;
        if log2_of(&term) < log2_of(&s) - w as isize - 8 {
            break;
        }
        let kp = fl_int(&Int::from(k + 1), w);
        t = t * &kp * &kp / (fl_int(&Int::from(2 * k + 1), w) * fl_int(&Int::from(2 * k + 2), w));
        k += 1;
        if k > 4 * w as u64 {
            break;
        }
    }
    let v = first + s * three_eighths(w);
    v.with_precision(p).value()
}

fn three_eighths(w: usize) -> F {
    fl_rat(&Rat::from_parts(Int::from(3), Nat::from(8u8)), w)
}

/// $\zeta(n)$（$n \ge 2$ の整数）。Borwein の方法（係数は整数で正確に作る）
pub fn zeta(n: u64, p: usize) -> F {
    let w = guard(p);
    // 誤差 ≤ 3 / (3+√8)^m < 2^{−w}: m > (w + 2) / log2(5.828)
    let m = ((w as f64 + 4.0) / (3.0 + 8f64.sqrt()).log2()).ceil() as u64 + 1;
    // d_k = m Σ_{i=0}^{k} (m+i−1)! 4^i / ((m−i)! (2i)!)（整数）
    let mut d: Vec<Int> = Vec::with_capacity(m as usize + 1);
    let mut acc = Rat::ZERO;
    let budget = super::Budget::unlimited();
    for i in 0..=m {
        let num = num::factorial(m + i - 1, &budget).unwrap() * Int::from(4).pow(i as usize);
        let den = num::factorial(m - i, &budget).unwrap() * num::factorial(2 * i, &budget).unwrap();
        acc += Rat::from_parts_signed(num, den);
        d.push(num::as_int(&(acc.clone() * Rat::from(Int::from(m)))).expect("d_k は整数"));
    }
    let dm = d[m as usize].clone();
    let mut s = Rat::ZERO;
    for k in 0..m {
        let sign = if k % 2 == 0 { Int::ONE } else { -Int::ONE };
        let numer = sign * (&d[k as usize] - &dm);
        let den = Int::from(k + 1).pow(n as usize);
        s += Rat::from_parts_signed(numer, den);
    }
    // ζ(n) = −1 / (d_m (1 − 2^{1−n})) · s
    let factor = Rat::ONE - Rat::from_parts(Int::ONE, Nat::from(2u8).pow(n as usize - 1));
    let v = -(fl_rat(&s, w) / (fl_int(&dm, w) * fl_rat(&factor, w)));
    v.with_precision(p).value()
}

/// $\ln\Gamma(x)$（$x > 0$ の有理数）
pub fn ln_gamma(x: &Rat, p: usize) -> F {
    let w = guard(p);
    if num::sign(x) <= 0 {
        return F::INFINITY;
    }
    // z = x + N を w ビットに見合う大きさに（スターリングの項 B_{2k}/(2k(2k−1)z^{2k−1}) が速く小さくなる所）
    let target = (w as f64 * 0.35).max(10.0);
    let xf = x.to_f64().value();
    let shift = if xf < target { (target - xf).ceil() as u64 } else { 0 };
    let z = x.clone() + Rat::from(Int::from(shift));
    // ln Γ(x) = ln Γ(z) − ln(x (x+1) … (x+N−1))
    let mut prod = Rat::ONE;
    for i in 0..shift {
        prod *= x.clone() + Rat::from(Int::from(i));
    }
    let zf = fl_rat(&z, w);
    let half = fl_rat(&Rat::from_parts(Int::ONE, Nat::from(2u8)), w);
    let two_pi = F::pi(w) * fl_int(&Int::from(2), w);
    let mut s = (&zf - &half) * zf.ln() - &zf + two_pi.ln() * &half;
    let z2 = &zf * &zf;
    let mut zpow = zf.clone(); // z^{2k−1}
    let mut last = isize::MAX;
    for k in 1..(w as u64) {
        let b = num::bernoulli(2 * k as usize);
        let c = b / Rat::from(Int::from((2 * k) * (2 * k - 1)));
        let term = fl_rat(&c, w) / &zpow;
        let l = log2_of(&term);
        if l > last {
            break; // 漸近級数が増え始めた（ここまでで誤差は十分小さいはず）
        }
        last = l;
        s = s + &term;
        if l < log2_of(&s) - w as isize - 8 {
            break;
        }
        zpow = zpow * &z2;
    }
    let v = s - fl_rat(&prod, w).ln();
    v.with_precision(p).value()
}

/// $\Gamma(x)$（有理数。0 以下の整数は極で ∞）
pub fn gamma(x: &Rat, p: usize) -> F {
    if let Some(n) = num::as_int(x) {
        if n <= Int::ZERO {
            return F::INFINITY;
        }
        if let Ok(k) = u64::try_from(&(n - Int::ONE)) {
            if k <= 10_000 {
                let budget = super::Budget::unlimited();
                return fl_int(&num::factorial(k, &budget).unwrap(), p);
            }
        }
    }
    if num::sign(x) > 0 {
        return ln_gamma(x, p + 16).exp().with_precision(p).value();
    }
    // 反射公式 Γ(x) Γ(1−x) = π / sin(πx)
    let w = guard(p);
    let one_minus = Rat::ONE - x.clone();
    let g = gamma(&one_minus, w);
    let pix = F::pi(w) * fl_rat(x, w);
    (F::pi(w) / (pix.sin() * g)).with_precision(p).value()
}

/// $\psi(x)$（有理数。0 以下の整数は極）
pub fn digamma(x: &Rat, p: usize) -> F {
    let w = guard(p);
    if let Some(n) = num::as_int(x) {
        if n <= Int::ZERO {
            return F::INFINITY;
        }
    }
    if num::sign(x) <= 0 {
        // ψ(1−x) − ψ(x) = π cot πx
        let one_minus = Rat::ONE - x.clone();
        let pix = F::pi(w) * fl_rat(x, w);
        let cot = pix.cos() / pix.sin();
        return (digamma(&one_minus, w) - F::pi(w) * cot).with_precision(p).value();
    }
    let target = (w as f64 * 0.35).max(10.0);
    let xf = x.to_f64().value();
    let shift = if xf < target { (target - xf).ceil() as u64 } else { 0 };
    let z = x.clone() + Rat::from(Int::from(shift));
    let mut corr = Rat::ZERO; // Σ 1/(x + i)
    for i in 0..shift {
        corr += Rat::ONE / (x.clone() + Rat::from(Int::from(i)));
    }
    let zf = fl_rat(&z, w);
    let mut s = zf.ln() - fl_int(&Int::ONE, w) / (fl_int(&Int::from(2), w) * &zf);
    let z2 = &zf * &zf;
    let mut zpow = z2.clone();
    let mut last = isize::MAX;
    for k in 1..(w as u64) {
        let b = num::bernoulli(2 * k as usize);
        let c = b / Rat::from(Int::from(2 * k));
        let term = fl_rat(&c, w) / &zpow;
        let l = log2_of(&term);
        if l > last {
            break;
        }
        last = l;
        s = s - &term;
        if l < log2_of(&s) - w as isize - 8 {
            break;
        }
        zpow = zpow * &z2;
    }
    (s - fl_rat(&corr, w)).with_precision(p).value()
}

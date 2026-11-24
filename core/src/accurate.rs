//! 桁落ち・途中のあふれを避けて計算する、数の並びの和・積・内積・ノルム（#204）。
//! 展開の段で、項が全部定数になる Σ・Π・内積・トレース・ノルムに使う（`expand.rs`）

/// 和（Shewchuk の方法。重ならない部分和の列を持つので、丸めの 1 回分までぴったり。Python の `math.fsum` と同じ）。10¹⁶ + 1 − 10¹⁶ = 1
pub fn sum(values: &[f64]) -> f64 {
    if values.iter().any(|v| !v.is_finite()) {
        return values.iter().sum();
    }
    let mut partials: Vec<f64> = Vec::new();
    for &v in values {
        let mut x = v;
        let mut i = 0;
        for j in 0..partials.len() {
            let mut y = partials[j];
            if x.abs() < y.abs() {
                std::mem::swap(&mut x, &mut y);
            }
            let hi = x + y;
            let lo = y - (hi - x);
            if lo != 0.0 {
                partials[i] = lo;
                i += 1;
            }
            x = hi;
        }
        partials.truncate(i);
        partials.push(x);
    }
    // 大きいほうから足す（最後の丸めを正しくする）
    let mut hi = 0.0;
    while let Some(x) = partials.pop() {
        let y = hi;
        hi = x + y;
        let lo = y - (hi - x);
        if lo != 0.0 {
            partials.push(lo);
            // 残りを足して終わる
            return hi + partials.iter().rev().sum::<f64>();
        }
    }
    hi
}

/// 2 のべき 2ᵉ（e が f64 の指数の範囲を超えても、2 回に分けて掛ける）
fn scale(v: f64, e: i32) -> f64 {
    let h = e / 2;
    v * 2f64.powi(h) * 2f64.powi(e - h)
}

/// 指数（v = m 2ᵉ、0.5 ≤ |m| < 1 の e）
fn exponent(v: f64) -> i32 {
    if v == 0.0 || !v.is_finite() { 0 } else { v.abs().log2().floor() as i32 + 1 }
}

/// 積。仮数と指数を分けて掛けるので、途中だけあふれる・消える並び（10²⁰⁰ · 10²⁰⁰ · 10⁻²⁰⁰）も、答えが表せれば求まる
pub fn product(values: &[f64]) -> f64 {
    let mut m = 1.0f64;
    let mut e = 0i32;
    for &v in values {
        if v == 0.0 || !v.is_finite() {
            return values.iter().product();
        }
        let k = exponent(v);
        m *= scale(v, -k);
        e += k;
        let j = exponent(m);
        m = scale(m, -j);
        e += j;
    }
    scale(m, e)
}

/// 内積 Σ aᵢ bᵢ。それぞれを 2 のべきでそろえてから、誤差の無い積（fma）と補償和で求める
/// （(10¹⁶, 1, 10¹⁶) · (1, 1, −1) = 1、(10²⁰⁰, 10²⁰⁰, 1) · (10¹¹⁰, −10¹¹⁰, 2) = 2）
pub fn dot(a: &[f64], b: &[f64]) -> f64 {
    if a.iter().chain(b).any(|v| !v.is_finite()) {
        return a.iter().zip(b).map(|(x, y)| x * y).sum();
    }
    let ea = a.iter().map(|&v| exponent(v)).max().unwrap_or(0);
    let eb = b.iter().map(|&v| exponent(v)).max().unwrap_or(0);
    let mut parts = Vec::with_capacity(2 * a.len());
    for (&x, &y) in a.iter().zip(b) {
        let (x, y) = (scale(x, -ea), scale(y, -eb));
        let p = x * y;
        parts.push(p);
        parts.push(x.mul_add(y, -p));
    }
    scale(sum(&parts), ea + eb)
}

/// ユークリッドのノルム √(Σ vᵢ²)。いちばん大きい成分で割ってから 2 乗する（10²⁰⁰・10⁻²⁰⁰ の成分でも、あふれ・消えない）
pub fn norm(values: &[f64]) -> f64 {
    let m = values.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    if m == 0.0 || !m.is_finite() {
        return values.iter().map(|v| v * v).sum::<f64>().sqrt();
    }
    let e = exponent(m);
    let s: Vec<f64> = values.iter().map(|&v| scale(v, -e)).map(|v| v * v).collect();
    scale(sum(&s).sqrt(), e)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_digits() {
        assert_eq!(sum(&[1e16, 1.0, -1e16]), 1.0);
        assert_eq!(product(&[1e200, 1e200, 1e-200]), 1e200);
        assert_eq!(dot(&[1e16, 1.0, 1e16], &[1.0, 1.0, -1.0]), 1.0);
        assert_eq!(dot(&[1e200, 1e200, 1.0], &[1e110, -1e110, 2.0]), 2.0);
        assert!((norm(&[1e200, 1e200]) / (2f64.sqrt() * 1e200) - 1.0).abs() < 1e-15);
        assert!((norm(&[1e-200, 1e-200]) / (2f64.sqrt() * 1e-200) - 1.0).abs() < 1e-15);
        assert_eq!(norm(&[3.0, 4.0]), 5.0);
        assert_eq!(product(&[2.0, 3.0, 4.0]), 24.0);
    }
}

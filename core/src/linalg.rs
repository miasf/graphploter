//! 数の行列の掃き出し（計画 `docs/plan-math-notation.md` の M7）。連立一次方程式の階数・零空間（ker）・列空間（Im）を求める。
//!
//! 成分が式の行列の計算（積・行列式など）は、展開の段の `expand/linalg.rs`。ここは、成分を数にした後の数値の計算

/// 数の行列（行の順）
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    pub rows: usize,
    pub cols: usize,
    pub a: Vec<f64>,
}

/// 成分が有限桁の小数（整数を含む）の正方行列の行列式を、正確に求める（#123）。
/// 成分を 10ᵈ 倍して整数にし（d は小数点以下の桁の最大。15 まで。10ᵈ 倍が 2⁵³ を超えるものは使わない）、Bareiss 法（割り算が割り切れる消去）で i128 のまま計算して、
/// 最後に 10^(dn) で割る（丸めは最後の 1 回だけ）。小数にならない成分（√2 など）・桁あふれは None。
/// 浮動小数点で計算すると、`(10⁸ + 1)(10⁸ − 1) − 10⁸·10⁸` の 10¹⁶ − 1 が丸まって 0 になる
pub fn exact_det(a: &[f64], n: usize) -> Option<f64> {
    if n == 0 || a.len() != n * n {
        return None;
    }
    let decimals = |v: f64| -> Option<i32> {
        let s = format!("{}", v.abs());
        if !v.is_finite() || s.contains('e') {
            return None;
        }
        Some(s.split_once('.').map_or(0, |(_, f)| f.len() as i32))
    };
    let d = a.iter().map(|&v| decimals(v)).collect::<Option<Vec<_>>>()?.into_iter().max()?;
    if d > 15 {
        return None;
    }
    let scale = 10f64.powi(d);
    let mut m: Vec<i128> = Vec::with_capacity(n * n);
    for &v in a {
        let r = (v * scale).round();
        if r.abs() > 9_007_199_254_740_992.0 || r / scale != v {
            return None;
        }
        m.push(r as i128);
    }
    // Bareiss: M[i][j] ← (M[i][j]·M[k][k] − M[i][k]·M[k][j]) / 前のピボット（割り切れる）
    let mut sign = 1i128;
    let mut prev = 1i128;
    for k in 0..n {
        if m[k * n + k] == 0 {
            // この列がこの先 全部 0 なら、行列式は 0
            let Some(p) = (k + 1..n).find(|&i| m[i * n + k] != 0) else { return Some(0.0) };
            for j in 0..n {
                m.swap(k * n + j, p * n + j);
            }
            sign = -sign;
        }
        let pivot = m[k * n + k];
        for i in k + 1..n {
            for j in k + 1..n {
                let v = m[i * n + j].checked_mul(pivot)?.checked_sub(m[i * n + k].checked_mul(m[k * n + j])?)?;
                m[i * n + j] = v / prev;
            }
            m[i * n + k] = 0;
        }
        prev = pivot;
    }
    let det = sign * m[n * n - 1];
    Some(det as f64 / 10f64.powi(d * n as i32))
}

/// 行列式を浮動小数点で計算したときの丸め誤差の上限の見積もり 4nε Σ|a₁σ(1) ⋯ aₙσ(n)|（展開の全部の項の絶対値の和。6 × 6 まで）
pub fn det_rounding(a: &[f64], n: usize) -> f64 {
    fn perm(a: &[f64], n: usize, row: usize, used: u32) -> f64 {
        if row == n {
            return 1.0;
        }
        (0..n).filter(|c| used & (1 << c) == 0).map(|c| a[row * n + c].abs() * perm(a, n, row + 1, used | (1 << c))).sum()
    }
    if n > 6 {
        return f64::NAN;
    }
    4.0 * n as f64 * f64::EPSILON * perm(a, n, 0, 0)
}

/// ker・Im・rank の階数をどう決めたか（#124。計画 `docs/plan-stage3b-kernel.md`）
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RankHow {
    /// 成分が有限桁の小数・分母の小さい分数なので、有理数で正確に掃き出した
    Exact,
    /// 特異値で決めた。0 とみなした特異値は丸め誤差の程度（√2 を含むちょうど特異な行列など）
    Rounding,
    /// 特異値で決め、丸め誤差より大きい特異値も 0 とみなした（0 とみなした中で最大の特異値, 最大の特異値）
    Approx(f64, f64),
}

/// 数値の階数で 0 とみなす特異値の比（最大の特異値に対して）
pub const RANK_TOL: f64 = 1e-9;

/// 掃き出しの結果
#[derive(Debug, Clone)]
pub struct Reduced {
    /// 行の簡約形（上から階数の本数だけが 0 でない行）
    pub rref: Matrix,
    /// ピボットのある列（階数の本数）
    pub pivots: Vec<usize>,
}

impl Matrix {
    pub fn new(rows: usize, cols: usize, a: Vec<f64>) -> Matrix {
        debug_assert_eq!(a.len(), rows * cols);
        Matrix { rows, cols, a }
    }

    pub fn at(&self, i: usize, j: usize) -> f64 {
        self.a[i * self.cols + j]
    }

    fn row(&self, i: usize) -> &[f64] {
        &self.a[i * self.cols..(i + 1) * self.cols]
    }

    pub fn transpose(&self) -> Matrix {
        let mut a = Vec::with_capacity(self.a.len());
        for j in 0..self.cols {
            for i in 0..self.rows {
                a.push(self.at(i, j));
            }
        }
        Matrix::new(self.cols, self.rows, a)
    }

    /// 部分ピボット付きの掃き出し。最初の `lead` 列だけをピボットに使う（残りの列は、拡大係数行列の右辺）。
    /// 0 とみなすしきい値は、成分の最大の絶対値 × 10⁻⁹
    pub fn reduce(&self, lead: usize) -> Reduced {
        self.reduce_tol(lead, 1e-9)
    }

    /// [`Self::reduce`] の、0 とみなすしきい値（成分の最大の絶対値 × rel）を決められるもの。
    /// 固有値に丸め誤差がある A − λI の零空間は、少し緩いしきい値で求める
    pub fn reduce_tol(&self, lead: usize, rel: f64) -> Reduced {
        let (m, n) = (self.rows, self.cols);
        let mut a = self.a.clone();
        let scale = a.iter().fold(0.0f64, |s, v| s.max(v.abs()));
        let tol = scale * rel;
        let mut pivots = Vec::new();
        let mut r = 0;
        for c in 0..lead.min(n) {
            if r >= m {
                break;
            }
            let p = (r..m).max_by(|&i, &j| a[i * n + c].abs().total_cmp(&a[j * n + c].abs())).unwrap_or(r);
            if a[p * n + c].abs() <= tol {
                continue;
            }
            for k in 0..n {
                a.swap(r * n + k, p * n + k);
            }
            let d = a[r * n + c].clone();
            for k in 0..n {
                a[r * n + k] /= d;
            }
            for i in 0..m {
                if i == r {
                    continue;
                }
                let f = a[i * n + c];
                if f != 0.0 {
                    for k in 0..n {
                        a[i * n + k] -= f * a[r * n + k];
                    }
                }
            }
            pivots.push(c);
            r += 1;
        }
        // 丸めの残りを 0 にする
        for v in &mut a {
            if v.abs() <= tol {
                *v = 0.0;
            }
        }
        Reduced { rref: Matrix::new(m, n, a), pivots }
    }

    /// 階数（rank・dim の値。[`Self::reduce_judged`] で決める）
    pub fn rank(&self) -> usize {
        self.reduce_judged().0.pivots.len()
    }

    /// 掃き出した形 `red` から、零空間の基底（自由変数の 1 つを 1、ほかを 0 にした解）
    pub fn null_basis_of(&self, red: &Reduced) -> Vec<Vec<f64>> {
        (0..self.cols)
            .filter(|c| !red.pivots.contains(c))
            .map(|free| {
                let mut v = vec![0.0; self.cols];
                v[free] = 1.0;
                for (r, &p) in red.pivots.iter().enumerate() {
                    v[p] = -red.rref.at(r, free);
                }
                v
            })
            .collect()
    }

    /// 掃き出した形 `red` の 0 でない行
    pub fn rows_of(red: &Reduced) -> Vec<Vec<f64>> {
        (0..red.pivots.len()).map(|r| red.rref.row(r).to_vec()).collect()
    }

    /// 掃き出した形 `red` のピボットの列（元の行列の列そのもの）
    pub fn pivot_columns_of(&self, red: &Reduced) -> Vec<Vec<f64>> {
        red.pivots.iter().map(|&c| (0..self.rows).map(|i| self.at(i, c)).collect()).collect()
    }

    /// 零空間（A x = 0 の解）の正規直交基底
    pub fn null_space(&self) -> Vec<Vec<f64>> {
        orthonormalize(self.null_basis_of(&self.reduce(self.cols)))
    }

    /// 零空間の基底（掃き出した形のまま。自由変数の 1 つを 1、ほかを 0 にした解。式で見せるとき用）
    pub fn null_space_basis(&self) -> Vec<Vec<f64>> {
        self.null_space_basis_tol(1e-9)
    }

    /// [`Self::null_space_basis`] の、しきい値を決められるもの（[`Self::reduce_tol`]）
    pub fn null_space_basis_tol(&self, rel: f64) -> Vec<Vec<f64>> {
        self.null_basis_of(&self.reduce_tol(self.cols, rel))
    }

    /// 掃き出した形の 0 でない行（A x = 0 と同じ連立方程式の、独立な式の係数）
    pub fn reduced_rows(&self) -> Vec<Vec<f64>> {
        self.reduced_rows_tol(1e-9)
    }

    /// [`Self::reduced_rows`] の、しきい値を決められるもの（[`Self::reduce_tol`]）
    pub fn reduced_rows_tol(&self, rel: f64) -> Vec<Vec<f64>> {
        Self::rows_of(&self.reduce_tol(self.cols, rel))
    }

    /// 列空間の基底（ピボットの列そのもの。式で見せるとき用）
    pub fn pivot_columns(&self) -> Vec<Vec<f64>> {
        self.pivot_columns_of(&self.reduce(self.cols))
    }

    /// 列空間（像）の正規直交基底
    pub fn column_space(&self) -> Vec<Vec<f64>> {
        orthonormalize(self.pivot_columns())
    }

    /// 有理数で掃き出す（成分が全部、有限桁の小数か分母の小さい分数のときだけ。係数が大きすぎたら None）。
    /// `1 + 10⁻¹²` も 0 でないピボットになる（#124）
    pub fn reduce_exact(&self) -> Option<Reduced> {
        use crate::groebner::Rat;
        let (m, n) = (self.rows, self.cols);
        let mut a: Vec<Rat> = self.a.iter().map(|&v| Rat::from_f64(v)).collect::<Option<_>>()?;
        let mut pivots = Vec::new();
        let mut r = 0;
        for c in 0..n {
            if r >= m {
                break;
            }
            let Some(p) = (r..m).find(|&i| !a[i * n + c].is_zero()) else { continue };
            for k in 0..n {
                a.swap(r * n + k, p * n + k);
            }
            let d = a[r * n + c].clone();
            for k in 0..n {
                a[r * n + k] = a[r * n + k].div(&d)?;
            }
            for i in 0..m {
                let f = a[i * n + c].clone();
                if i == r || f.is_zero() {
                    continue;
                }
                for k in 0..n {
                    a[i * n + k] = a[i * n + k].add(&f.mul(&a[r * n + k])?.neg())?;
                }
            }
            pivots.push(c);
            r += 1;
        }
        Some(Reduced { rref: Matrix::new(m, n, a.iter().map(|x| x.f()).collect()), pivots })
    }

    /// 特異値（片側ヤコビ法。大きい順に、列の数だけ。列が行より多ければ、余りは 0 に近い値）
    pub fn singular_values(&self) -> Vec<f64> {
        let (m, n) = (self.rows, self.cols);
        let mut u: Vec<Vec<f64>> = (0..n).map(|j| (0..m).map(|i| self.at(i, j)).collect()).collect();
        let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
        for _ in 0..60 {
            let mut rotated = false;
            for p in 0..n {
                for q in p + 1..n {
                    let (alpha, beta, gamma) = (dot(&u[p], &u[p]), dot(&u[q], &u[q]), dot(&u[p], &u[q]));
                    if gamma == 0.0 || gamma.abs() <= 1e-15 * (alpha * beta).sqrt() {
                        continue;
                    }
                    rotated = true;
                    let zeta = (beta - alpha) / (2.0 * gamma);
                    let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                    let c = 1.0 / (1.0 + t * t).sqrt();
                    let s = c * t;
                    for i in 0..m {
                        let (x, y) = (u[p][i], u[q][i]);
                        u[p][i] = c * x - s * y;
                        u[q][i] = s * x + c * y;
                    }
                }
            }
            if !rotated {
                break;
            }
        }
        let mut sv: Vec<f64> = u.iter().map(|v| dot(v, v).sqrt()).collect();
        sv.sort_by(|a, b| b.total_cmp(a));
        sv
    }

    /// ker・Im・rank の掃き出しと、階数の決め方（#124）。成分が有限桁の小数なら有理数で正確に。そうでなければ特異値
    /// σ₁ ≥ σ₂ ≥ … で、σₖ ≤ 10⁻⁹ σ₁ を 0 とみなした階数 r にし、ちょうど r 本のピボットになる掃き出しを使う。
    /// 0 とみなした特異値が丸め誤差（16 max(m, n) ε σ₁）より大きければ [`RankHow::Approx`]
    pub fn reduce_judged(&self) -> (Reduced, RankHow) {
        if let Some(red) = self.reduce_exact() {
            return (red, RankHow::Exact);
        }
        let sv = self.singular_values();
        let top = sv.first().copied().unwrap_or(0.0);
        let k = self.rows.min(self.cols);
        if !(top > 0.0 && top.is_finite()) {
            return (self.reduce(self.cols), RankHow::Rounding);
        }
        let r = sv.iter().take(k).filter(|&&v| v > RANK_TOL * top).count();
        // ちょうど r 本のピボットになるしきい値（σ_r と σ_{r+1} の間の、対数で真ん中に近いもの）
        let hi = if r > 0 { sv[r - 1] / top } else { 1.0 };
        let lo = if r < k { (sv[r] / top).max(f64::EPSILON) } else { f64::EPSILON };
        let mid = (hi * lo).sqrt();
        let mut tries: Vec<f64> = (1..=16).map(|e| 10f64.powi(-e)).collect();
        tries.push(mid);
        tries.sort_by(|a, b| (a.ln() - mid.ln()).abs().total_cmp(&(b.ln() - mid.ln()).abs()));
        let red = tries.iter().map(|&rel| self.reduce_tol(self.cols, rel)).find(|red| red.pivots.len() == r).unwrap_or_else(|| self.reduce(self.cols));
        let r = red.pivots.len();
        let rounding = 16.0 * self.rows.max(self.cols) as f64 * f64::EPSILON * top;
        let how = if r < k && sv[r] > rounding { RankHow::Approx(sv[r], top) } else { RankHow::Rounding };
        (red, how)
    }

    /// 行列 × ベクトル
    pub fn apply(&self, x: &[f64]) -> Vec<f64> {
        (0..self.rows).map(|i| self.row(i).iter().zip(x).map(|(a, b)| a * b).sum()).collect()
    }
}

/// グラム・シュミットで正規直交にする（長さが 0 になったものは捨てる）
pub fn orthonormalize(vs: Vec<Vec<f64>>) -> Vec<Vec<f64>> {
    let mut out: Vec<Vec<f64>> = Vec::new();
    for mut v in vs {
        let len0 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        // 2 回引く（1 回だと、ほとんど平行なベクトルで直交が崩れる）
        for _ in 0..2 {
            for b in &out {
                let d: f64 = v.iter().zip(b).map(|(x, y)| x * y).sum();
                for (x, y) in v.iter_mut().zip(b) {
                    *x -= d * y;
                }
            }
        }
        let len = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        if len <= 1e-9 * len0.max(1e-300) {
            continue;
        }
        v.iter_mut().for_each(|x| *x /= len);
        out.push(v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rng(seed: &mut u64) -> f64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((*seed >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0
    }

    #[test]
    fn rank_plus_nullity_is_columns() {
        let mut seed = 7u64;
        for (m, n, r) in [(3, 3, 3), (3, 3, 2), (2, 5, 2), (5, 3, 1), (4, 6, 3), (6, 6, 4)] {
            // 階数 r の行列: r 本の乱数の行の 1 次結合
            let base: Vec<Vec<f64>> = (0..r).map(|_| (0..n).map(|_| rng(&mut seed)).collect()).collect();
            let mut a = Vec::new();
            for i in 0..m {
                let coef: Vec<f64> = (0..r).map(|k| if i < r { (i == k) as u8 as f64 } else { rng(&mut seed) }).collect();
                for j in 0..n {
                    a.push((0..r).map(|k| coef[k] * base[k][j]).sum());
                }
            }
            let mat = Matrix::new(m, n, a);
            assert_eq!(mat.rank(), r.min(m), "{m}×{n}");
            let ns = mat.null_space();
            assert_eq!(mat.rank() + ns.len(), n);
            for b in &ns {
                let ab = mat.apply(b);
                assert!(ab.iter().map(|x| x * x).sum::<f64>().sqrt() < 1e-9);
                assert!((b.iter().map(|x| x * x).sum::<f64>() - 1.0).abs() < 1e-12);
            }
            // 列空間の基底は、零空間（転置の）と直交する
            let cs = mat.column_space();
            assert_eq!(cs.len(), mat.rank());
            for left in mat.transpose().null_space() {
                for c in &cs {
                    assert!(left.iter().zip(c).map(|(x, y)| x * y).sum::<f64>().abs() < 1e-9);
                }
            }
        }
    }

    #[test]
    fn reduce_augmented() {
        // x + y + z = 1, 2x + 2y + 2z = 2 → 1 本
        let m = Matrix::new(2, 4, vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0]);
        let red = m.reduce(3);
        assert_eq!(red.pivots, vec![0]);
        assert_eq!(&red.rref.a[4..], &[0.0; 4]);
        // x + y = 1, x + y = 2 → 矛盾（係数が 0 で右辺が 0 でない行）
        let m = Matrix::new(2, 4, vec![1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 0.0, 2.0]);
        let red = m.reduce(3);
        assert_eq!(red.pivots.len(), 1);
        assert!(red.rref.at(1, 3).abs() > 0.5);
    }
}

/// 固有値 1 つ（重なりをまとめたもの）。im が 0 なら実数
#[derive(Debug, Clone, PartialEq)]
pub struct Eigenvalue {
    pub re: f64,
    pub im: f64,
    /// 重複度（固有多項式の根として何重か）
    pub mult: usize,
}

impl Matrix {
    /// 固有多項式 det(λI − A) の係数（λ⁰ から λⁿ の順。λⁿ の係数は 1）。ファデーエフ・ルヴェリエの方法
    pub fn char_poly(&self) -> Vec<f64> {
        let n = self.rows;
        let mut c = vec![0.0; n + 1];
        c[n] = 1.0;
        let mut m = vec![0.0; n * n]; // M₀ = 0
        for k in 1..=n {
            // M_k = A M_{k−1} + c_{n−k+1} I
            let mut next = vec![0.0; n * n];
            for i in 0..n {
                for j in 0..n {
                    let mut s = 0.0;
                    for l in 0..n {
                        s += self.at(i, l) * m[l * n + j];
                    }
                    next[i * n + j] = s + if i == j { c[n - k + 1] } else { 0.0 };
                }
            }
            m = next;
            // c_{n−k} = −tr(A M_k) / k
            let mut tr = 0.0;
            for i in 0..n {
                for l in 0..n {
                    tr += self.at(i, l) * m[l * n + i];
                }
            }
            c[n - k] = -tr / k as f64;
        }
        c
    }

    /// 固有値（正方行列だけ）。固有多項式の根をデュラン・ケルナー法で求め、近い根をまとめて重複度にする。
    /// 実数の固有値は小さい順、実数でないものは後ろ（実部の順）
    pub fn eigenvalues(&self) -> Vec<Eigenvalue> {
        use num_complex::Complex64 as C;
        let n = self.rows;
        if n == 0 || self.cols != n {
            return Vec::new();
        }
        let c = self.char_poly();
        let scale = self.a.iter().fold(0.0f64, |s, v| s.max(v.abs())).max(1e-300);
        let eval = |z: C| c.iter().rev().fold(C::new(0.0, 0.0), |acc, &k| acc * z + k);
        // 根の大きさの上限（コーシー）
        let bound = 1.0 + c[..n].iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let mut z: Vec<C> = (0..n).map(|k| C::from_polar(bound * 0.9, 0.4 + 2.0 * std::f64::consts::PI * k as f64 / n as f64)).collect();
        for _ in 0..2000 {
            let mut change = 0.0f64;
            for i in 0..n {
                let mut den = C::new(1.0, 0.0);
                for j in 0..n {
                    if i != j {
                        den *= z[i] - z[j];
                    }
                }
                if den.norm() == 0.0 {
                    den = C::new(1e-300, 0.0);
                }
                let d = eval(z[i]) / den;
                z[i] -= d;
                change = change.max(d.norm());
            }
            if change <= 1e-15 * bound {
                break;
            }
        }
        // 近い根をまとめる（重根は 1e−5 くらいまでしか合わないので、平均をとる）
        let tol = 1e-4 * scale.max(1.0);
        let mut groups: Vec<(C, usize)> = Vec::new();
        for r in z {
            match groups.iter_mut().find(|(g, k)| (*g / *k as f64 - r).norm() < tol) {
                Some((g, k)) => {
                    *g += r;
                    *k += 1;
                }
                None => groups.push((r, 1)),
            }
        }
        let snap = |v: f64| -> f64 {
            // 分母 12 までの分数に近ければ、その分数にする
            for d in 1..=12 {
                let q = (v * d as f64).round() / d as f64;
                if (v - q).abs() < 1e-7 * scale.max(1.0) {
                    return q;
                }
            }
            v
        };
        let mut out: Vec<Eigenvalue> = groups
            .into_iter()
            .map(|(g, k)| {
                let m = g / k as f64;
                let im = if m.im.abs() < 1e-7 * scale.max(1.0) { 0.0 } else { snap(m.im) };
                Eigenvalue { re: snap(m.re), im, mult: k }
            })
            .collect();
        out.sort_by(|a, b| (a.im != 0.0).cmp(&(b.im != 0.0)).then(a.re.total_cmp(&b.re)).then(a.im.total_cmp(&b.im)));
        out
    }

    /// A − λI（正方行列だけ）
    pub fn shifted(&self, lambda: f64) -> Matrix {
        let mut a = self.a.clone();
        for i in 0..self.rows.min(self.cols) {
            a[i * self.cols + i] -= lambda;
        }
        Matrix::new(self.rows, self.cols, a)
    }
}

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
        let (m, n) = (self.rows, self.cols);
        let mut a = self.a.clone();
        let scale = a.iter().fold(0.0f64, |s, v| s.max(v.abs()));
        let tol = scale * 1e-9;
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
            let d = a[r * n + c];
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

    /// 階数
    pub fn rank(&self) -> usize {
        self.reduce(self.cols).pivots.len()
    }

    /// 零空間（A x = 0 の解）の正規直交基底
    pub fn null_space(&self) -> Vec<Vec<f64>> {
        let red = self.reduce(self.cols);
        let n = self.cols;
        let mut basis = Vec::new();
        for free in (0..n).filter(|c| !red.pivots.contains(c)) {
            // 自由変数を 1、ほかの自由変数を 0 にした解
            let mut v = vec![0.0; n];
            v[free] = 1.0;
            for (r, &p) in red.pivots.iter().enumerate() {
                v[p] = -red.rref.at(r, free);
            }
            basis.push(v);
        }
        orthonormalize(basis)
    }

    /// 零空間の基底（掃き出した形のまま。自由変数の 1 つを 1、ほかを 0 にした解。式で見せるとき用）
    pub fn null_space_basis(&self) -> Vec<Vec<f64>> {
        let red = self.reduce(self.cols);
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

    /// 掃き出した形の 0 でない行（A x = 0 と同じ連立方程式の、独立な式の係数）
    pub fn reduced_rows(&self) -> Vec<Vec<f64>> {
        let red = self.reduce(self.cols);
        (0..red.pivots.len()).map(|r| red.rref.row(r).to_vec()).collect()
    }

    /// 列空間の基底（ピボットの列そのもの。式で見せるとき用）
    pub fn pivot_columns(&self) -> Vec<Vec<f64>> {
        let red = self.reduce(self.cols);
        red.pivots.iter().map(|&c| (0..self.rows).map(|i| self.at(i, c)).collect()).collect()
    }

    /// 列空間（像）の正規直交基底
    pub fn column_space(&self) -> Vec<Vec<f64>> {
        let red = self.reduce(self.cols);
        let basis = red.pivots.iter().map(|&c| (0..self.rows).map(|i| self.at(i, c)).collect()).collect();
        orthonormalize(basis)
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

//! ベクトルと行列の計算（計画 `docs/plan-math-notation.md` の M6）。展開の段で、成分ごとの式にする。
//!
//! 組 `(a, b, c)` は列ベクトルで、n × 1 の行列と同じものとして扱う（[`Mat::of`]）。計算の結果が n × 1 なら組、
//! 1 × 1 ならスカラーに戻す（[`Mat::into_expr`]）。成分の式は `diff.rs` の作り方（数どうしはその場で計算する）で組む

use crate::diff::{add, div, mul, neg, sub};
use crate::functions::Func;
use crate::parser::{BinOp, Expr};

/// 行列の大きさの上限（6 × 6）
pub const MAX_SIZE: usize = 6;
/// 行列のべきの上限
pub const MAX_POWER: i64 = 20;

/// 行列（成分は展開した後の式。行の順）
#[derive(Debug, Clone)]
pub struct Mat {
    pub rows: usize,
    pub cols: usize,
    pub items: Vec<Expr>,
}

impl Mat {
    /// 組（列ベクトル）・行列なら、行列として見る。スカラーは None
    pub fn of(e: &Expr) -> Option<Mat> {
        match e {
            Expr::Tuple(items) => Some(Mat { rows: items.len(), cols: 1, items: items.clone() }),
            Expr::Matrix { rows, cols, items } => Some(Mat { rows: *rows, cols: *cols, items: items.clone() }),
            _ => None,
        }
    }

    /// 式に戻す: 1 × 1 はスカラー、n × 1 は組、ほかは行列
    pub fn into_expr(mut self) -> Expr {
        if self.rows == 1 && self.cols == 1 {
            return self.items.pop().unwrap_or(Expr::Num(0.0));
        }
        if self.cols == 1 {
            return Expr::Tuple(self.items);
        }
        Expr::Matrix { rows: self.rows, cols: self.cols, items: self.items }
    }

    pub fn at(&self, i: usize, j: usize) -> &Expr {
        &self.items[i * self.cols + j]
    }

    /// 列ベクトルか（成分が 2 つ以上）
    pub fn is_vector(&self) -> bool {
        self.cols == 1 && self.rows > 1
    }

    /// 大きさの説明（「2×3 の行列」「3 成分のベクトル」）
    pub fn describe(&self) -> String {
        if self.cols == 1 {
            format!("{} 成分のベクトル", self.rows)
        } else {
            format!("{}×{} の行列", self.rows, self.cols)
        }
    }

    fn map(&self, f: impl Fn(&Expr) -> Expr) -> Mat {
        Mat { rows: self.rows, cols: self.cols, items: self.items.iter().map(f).collect() }
    }

    pub fn identity(n: usize) -> Mat {
        let items = (0..n * n).map(|k| Expr::Num(if k / n == k % n { 1.0 } else { 0.0 })).collect();
        Mat { rows: n, cols: n, items }
    }

    pub fn transpose(&self) -> Mat {
        let mut items = Vec::with_capacity(self.items.len());
        for j in 0..self.cols {
            for i in 0..self.rows {
                items.push(self.at(i, j).clone());
            }
        }
        Mat { rows: self.cols, cols: self.rows, items }
    }

    /// 共役転置
    pub fn dagger(&self) -> Mat {
        self.transpose().map(|e| Expr::Call(Func::Conj, vec![e.clone()]))
    }

    pub fn negated(&self) -> Mat {
        self.map(|e| neg(e.clone()))
    }

    /// 行列の積（列の数と行の数が合わなければエラー）
    pub fn times(&self, o: &Mat) -> Result<Mat, String> {
        if self.cols != o.rows {
            return Err(format!("{}と {}は掛けられません", self.describe(), o.describe()));
        }
        let mut items = Vec::with_capacity(self.rows * o.cols);
        for i in 0..self.rows {
            for j in 0..o.cols {
                let mut s = Expr::Num(0.0);
                for k in 0..self.cols {
                    s = add(s, mul(self.at(i, k).clone(), o.at(k, j).clone()));
                }
                items.push(s);
            }
        }
        Ok(Mat { rows: self.rows, cols: o.cols, items })
    }

    fn square(&self, what: &str) -> Result<usize, String> {
        if self.rows != self.cols {
            return Err(format!("{what}は正方行列で使えます（{}です）", self.describe()));
        }
        Ok(self.rows)
    }

    /// 行列式（余因子展開。下の行から、使う列の組ごとに小行列式を覚えておく）
    pub fn det(&self) -> Result<Expr, String> {
        let n = self.square("行列式")?;
        Ok(minors(self, &(0..n).collect::<Vec<_>>(), &(0..n).collect::<Vec<_>>()))
    }

    pub fn trace(&self) -> Result<Expr, String> {
        let n = self.square("トレース")?;
        Ok((0..n).fold(Expr::Num(0.0), |s, k| add(s, self.at(k, k).clone())))
    }

    /// 逆行列（余因子行列の転置を行列式で割る）
    pub fn inverse(&self) -> Result<Mat, String> {
        let n = self.square("逆行列")?;
        let d = self.det()?;
        if matches!(d, Expr::Num(v) if v.abs() < 1e-300) {
            return Err("行列式が 0 なので、逆行列はありません".into());
        }
        if n == 1 {
            return Ok(Mat { rows: 1, cols: 1, items: vec![div(Expr::Num(1.0), d)] });
        }
        let all: Vec<usize> = (0..n).collect();
        let without = |k: usize| all.iter().copied().filter(|&x| x != k).collect::<Vec<_>>();
        let mut items = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                // 逆行列の (i, j) 成分 = (j, i) の余因子 / det
                let c = minors(self, &without(j), &without(i));
                let c = if (i + j) % 2 == 0 { c } else { neg(c) };
                items.push(div(c, d.clone()));
            }
        }
        Ok(Mat { rows: n, cols: n, items })
    }

    /// べき（k は −20〜20 の整数。負は逆行列のべき）。繰り返し 2 乗で掛ける
    pub fn power(&self, k: i64, max_nodes: usize) -> Result<Mat, String> {
        let n = self.square("行列のべき")?;
        if k.abs() > MAX_POWER {
            return Err(format!("行列のべきは {MAX_POWER} 乗までにしてください"));
        }
        let mut base = if k < 0 { self.inverse()? } else { self.clone() };
        let mut out = Mat::identity(n);
        let mut e = k.unsigned_abs();
        while e > 0 {
            if e & 1 == 1 {
                out = out.times(&base)?;
            }
            e >>= 1;
            if e > 0 {
                base = base.times(&base)?;
            }
            if out.node_count() > max_nodes || base.node_count() > max_nodes {
                return Err("行列のべきの式が大きくなりすぎます（べきを小さくするか、成分を数にしてください）".into());
            }
        }
        Ok(out)
    }

    pub fn node_count(&self) -> usize {
        self.items.iter().map(Expr::node_count).sum()
    }
}

/// 行 `rows`・列 `cols` を残した小行列式（余因子展開）
fn minors(m: &Mat, rows: &[usize], cols: &[usize]) -> Expr {
    // 使う列の組（ビットの並び）ごとに、「下から何行分」の小行列式を覚える
    fn go(m: &Mat, rows: &[usize], mask: u32, cols: &[usize], memo: &mut std::collections::HashMap<u32, Expr>) -> Expr {
        let Some((&row, rest)) = rows.split_first() else { return Expr::Num(1.0) };
        if let Some(e) = memo.get(&mask) {
            return e.clone();
        }
        let mut s = Expr::Num(0.0);
        let mut sign = true;
        for (k, &c) in cols.iter().enumerate() {
            if mask & (1 << k) == 0 {
                continue;
            }
            let a = m.at(row, c).clone();
            // 0 の成分は、小行列式を作らない
            if !matches!(a, Expr::Num(v) if v == 0.0) {
                let t = mul(a, go(m, rest, mask & !(1 << k), cols, memo));
                s = if sign { add(s, t) } else { sub(s, t) };
            }
            sign = !sign;
        }
        memo.insert(mask, s.clone());
        s
    }
    let mut memo = std::collections::HashMap::new();
    go(m, rows, (1u32 << cols.len()) - 1, cols, &mut memo)
}

/// ベクトルの大きさ √(Σ cᵢ²)
pub fn norm(items: &[Expr]) -> Expr {
    let s = items.iter().fold(Expr::Num(0.0), |s, c| add(s, mul(c.clone(), c.clone())));
    Expr::Call(Func::Sqrt, vec![s])
}

/// 内積 Σ uᵢ vᵢ
pub fn dot(u: &[Expr], v: &[Expr]) -> Result<Expr, String> {
    if u.len() != v.len() {
        return Err(format!("{} 成分と {} 成分のベクトルの内積は計算できません", u.len(), v.len()));
    }
    Ok(u.iter().zip(v).fold(Expr::Num(0.0), |s, (a, b)| add(s, mul(a.clone(), b.clone()))))
}

/// 外積（3 成分ならベクトル、2 成分ならスカラー u₁v₂ − u₂v₁）
pub fn cross(u: &[Expr], v: &[Expr]) -> Result<Expr, String> {
    let m = |a: &Expr, b: &Expr| mul(a.clone(), b.clone());
    match (u, v) {
        ([a1, a2, a3], [b1, b2, b3]) => Ok(Expr::Tuple(vec![sub(m(a2, b3), m(a3, b2)), sub(m(a3, b1), m(a1, b3)), sub(m(a1, b2), m(a2, b1))])),
        ([a1, a2], [b1, b2]) => Ok(sub(m(a1, b2), m(a2, b1))),
        _ => Err(format!("外積は 3 成分（か 2 成分）のベクトルどうしで使えます（{} 成分と {} 成分）", u.len(), v.len())),
    }
}

/// ベクトル・行列の 2 項の演算（べき以外）。どちらもスカラーなら None（ふつうの演算）。リストとの演算も None（後でリストを開く）
pub fn binary(op: BinOp, a: &Expr, b: &Expr) -> Result<Option<Expr>, String> {
    let bin = |x: Expr, y: Expr| Expr::Bin(op, Box::new(x), Box::new(y));
    Ok(Some(match (Mat::of(a), Mat::of(b)) {
        (None, None) => return Ok(None),
        (Some(u), Some(v)) => match op {
            BinOp::Add | BinOp::Sub => {
                if (u.rows, u.cols) != (v.rows, v.cols) {
                    return Err(format!("{}と {}は、足し算・引き算ができません", u.describe(), v.describe()));
                }
                Mat { rows: u.rows, cols: u.cols, items: u.items.iter().zip(&v.items).map(|(x, y)| bin(x.clone(), y.clone())).collect() }.into_expr()
            }
            // ベクトルどうしを並べて書いたもの・`·` は内積
            BinOp::Mul if u.cols == 1 && v.cols == 1 => {
                if u.rows != v.rows {
                    return Err(format!("{} 成分と {} 成分のベクトルは計算できません", u.rows, v.rows));
                }
                dot(&u.items, &v.items)?
            }
            BinOp::Mul => u.times(&v)?.into_expr(),
            BinOp::Div => return Err("行列・ベクトルでは割れません（逆行列 A^(-1) を掛けてください）".into()),
            BinOp::Pow => return Err("行列・ベクトルのべきの指数は、整数にしてください".into()),
        },
        (Some(u), None) if !matches!(b, Expr::List(_)) => match op {
            BinOp::Mul | BinOp::Div => u.map(|x| bin(x.clone(), b.clone())).into_expr(),
            _ => return Err(format!("{}とスカラーは、掛け算・割り算だけが使えます", u.describe())),
        },
        (None, Some(v)) if !matches!(a, Expr::List(_)) => match op {
            BinOp::Mul => v.map(|x| bin(a.clone(), x.clone())).into_expr(),
            BinOp::Div => return Err("行列・ベクトルでは割れません（逆行列 A^(-1) を掛けてください）".into()),
            _ => return Err(format!("スカラーと{}は、掛け算だけが使えます", v.describe())),
        },
        _ => return Ok(None),
    }))
}

/// 大きさの上限（6 × 6）を確かめる
pub fn check_size(rows: usize, cols: usize) -> Result<(), String> {
    if rows > MAX_SIZE || cols > MAX_SIZE {
        return Err(format!("行列は {MAX_SIZE}×{MAX_SIZE} までにしてください（{rows}×{cols} です）"));
    }
    Ok(())
}

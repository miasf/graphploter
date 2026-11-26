//! 厳密な行列（計画 `docs/plan-exact-areas.md` の LA0）。
//!
//! 成分は [`Term`]（分数・根号・文字）。掃き出しは成分の式のまま行い、ピボットが 0 かは [`ball::is_zero`] で決める
//! （構造で 0 でなく、値で 0 を含まなければ 0 でない）。

use super::ball;
use super::term::{add, mul, neg, pow, Term};
use super::Stop;

/// 行列
#[derive(Debug, Clone, PartialEq)]
pub struct EMat {
    pub rows: usize,
    pub cols: usize,
    pub a: Vec<Term>,
}

impl EMat {
    pub fn at(&self, i: usize, j: usize) -> &Term {
        &self.a[i * self.cols + j]
    }

    pub fn of_term(t: &Term) -> Option<EMat> {
        match t {
            Term::Matrix(r, c, v) => Some(EMat { rows: *r, cols: *c, a: v.clone() }),
            Term::Tuple(v) => Some(EMat { rows: v.len(), cols: 1, a: v.clone() }),
            _ => None,
        }
    }

    /// Term に戻す（n × 1 は組、1 × 1 は数。今の道の `Mat::into_expr` と同じ）
    pub fn into_term(self) -> Term {
        if self.rows == 1 && self.cols == 1 {
            return self.a.into_iter().next().unwrap();
        }
        if self.cols == 1 {
            return Term::Tuple(self.a);
        }
        Term::Matrix(self.rows, self.cols, self.a)
    }

    pub fn identity(n: usize) -> EMat {
        let mut a = vec![Term::zero(); n * n];
        for i in 0..n {
            a[i * n + i] = Term::one();
        }
        EMat { rows: n, cols: n, a }
    }

    pub fn transpose(&self) -> EMat {
        let mut a = Vec::with_capacity(self.a.len());
        for j in 0..self.cols {
            for i in 0..self.rows {
                a.push(self.at(i, j).clone());
            }
        }
        EMat { rows: self.cols, cols: self.rows, a }
    }
}

/// 0 か（構造か、値で）。値が計算できない（文字を含む）なら、構造で 0 でなければ 0 でないとみなす
fn is_zero(t: &Term) -> bool {
    if t.is_zero() {
        return true;
    }
    if !t.is_number() {
        return false;
    }
    let mut rules = Vec::new();
    ball::is_zero(t, &mut rules).unwrap_or(false)
}

pub fn add_m(x: &EMat, y: &EMat) -> Result<EMat, Stop> {
    if x.rows != y.rows || x.cols != y.cols {
        return Err(Stop::NotFound("行列の大きさが違います".into()));
    }
    Ok(EMat { rows: x.rows, cols: x.cols, a: x.a.iter().zip(&y.a).map(|(p, q)| add(vec![p.clone(), q.clone()])).collect() })
}

pub fn scale(c: &Term, x: &EMat) -> EMat {
    EMat { rows: x.rows, cols: x.cols, a: x.a.iter().map(|p| mul(vec![c.clone(), p.clone()])).collect() }
}

pub fn mul_m(x: &EMat, y: &EMat) -> Result<EMat, Stop> {
    if x.cols != y.rows {
        return Err(Stop::NotFound("行列の積の大きさが合いません".into()));
    }
    let mut a = Vec::with_capacity(x.rows * y.cols);
    for i in 0..x.rows {
        for j in 0..y.cols {
            a.push(add((0..x.cols).map(|k| mul(vec![x.at(i, k).clone(), y.at(k, j).clone()])).collect()));
        }
    }
    Ok(EMat { rows: x.rows, cols: y.cols, a })
}

/// 簡約された行列と主成分の列（掃き出し法。ピボットは 0 でない最初の行）
pub fn rref(m: &EMat) -> (EMat, Vec<usize>) {
    let mut a = m.clone();
    let mut pivots = Vec::new();
    let mut r = 0;
    for c in 0..a.cols {
        if r >= a.rows {
            break;
        }
        let Some(p) = (r..a.rows).find(|&i| !is_zero(a.at(i, c))) else { continue };
        if p != r {
            for j in 0..a.cols {
                a.a.swap(p * a.cols + j, r * a.cols + j);
            }
        }
        let inv = pow(a.at(r, c).clone(), Term::int(-1));
        for j in 0..a.cols {
            let v = mul(vec![a.at(r, j).clone(), inv.clone()]);
            a.a[r * a.cols + j] = v;
        }
        for i in 0..a.rows {
            if i == r || is_zero(a.at(i, c)) {
                continue;
            }
            let f = a.at(i, c).clone();
            for j in 0..a.cols {
                let v = add(vec![a.at(i, j).clone(), neg(mul(vec![f.clone(), a.at(r, j).clone()]))]);
                a.a[i * a.cols + j] = v;
            }
        }
        pivots.push(c);
        r += 1;
    }
    (a, pivots)
}

/// 行列式（掃き出しで上三角にして対角の積。文字を含むときは余因子展開（6 × 6 まで））
pub fn det(m: &EMat) -> Result<Term, Stop> {
    if m.rows != m.cols {
        return Err(Stop::NotFound("行列式は正方行列に使ってください".into()));
    }
    let n = m.rows;
    if !m.a.iter().all(Term::is_number) {
        return Ok(cofactor_det(m));
    }
    let mut a = m.clone();
    let mut d = Term::one();
    for c in 0..n {
        let Some(p) = (c..n).find(|&i| !is_zero(a.at(i, c))) else { return Ok(Term::zero()) };
        if p != c {
            for j in 0..n {
                a.a.swap(p * n + j, c * n + j);
            }
            d = neg(d);
        }
        let piv = a.at(c, c).clone();
        d = mul(vec![d, piv.clone()]);
        let inv = pow(piv, Term::int(-1));
        for i in c + 1..n {
            if is_zero(a.at(i, c)) {
                continue;
            }
            let f = mul(vec![a.at(i, c).clone(), inv.clone()]);
            for j in c..n {
                let v = add(vec![a.at(i, j).clone(), neg(mul(vec![f.clone(), a.at(c, j).clone()]))]);
                a.a[i * n + j] = v;
            }
        }
    }
    Ok(d)
}

fn cofactor_det(m: &EMat) -> Term {
    let n = m.rows;
    if n == 1 {
        return m.a[0].clone();
    }
    let mut parts = Vec::new();
    for j in 0..n {
        let mut minor = Vec::with_capacity((n - 1) * (n - 1));
        for i in 1..n {
            for k in 0..n {
                if k != j {
                    minor.push(m.at(i, k).clone());
                }
            }
        }
        let sub = cofactor_det(&EMat { rows: n - 1, cols: n - 1, a: minor });
        let term = mul(vec![m.at(0, j).clone(), sub]);
        parts.push(if j % 2 == 1 { neg(term) } else { term });
    }
    add(parts)
}

/// 逆行列（[A | I] を掃き出す）。無ければ「逆行列はありません」
pub fn inverse(m: &EMat) -> Result<EMat, Stop> {
    if m.rows != m.cols {
        return Err(Stop::NotFound("逆行列は正方行列に使ってください".into()));
    }
    let n = m.rows;
    let mut aug = EMat { rows: n, cols: 2 * n, a: Vec::with_capacity(2 * n * n) };
    for i in 0..n {
        for j in 0..n {
            aug.a.push(m.at(i, j).clone());
        }
        for j in 0..n {
            aug.a.push(if i == j { Term::one() } else { Term::zero() });
        }
    }
    let (r, piv) = rref(&aug);
    if piv.len() < n || piv[n - 1] >= n {
        return Err(Stop::NotExist("逆行列はありません（行列式が 0）".into()));
    }
    let mut a = Vec::with_capacity(n * n);
    for i in 0..n {
        for j in 0..n {
            a.push(r.at(i, n + j).clone());
        }
    }
    Ok(EMat { rows: n, cols: n, a })
}

pub fn rank(m: &EMat) -> usize {
    rref(m).1.len()
}

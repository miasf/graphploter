//! 論理式・集合（計画 `docs/plan-math-notation.md` の M11）を、描ける形（選言標準形: 「かつ」のかたまりを「または」でつないだもの）にする。
//!
//! 1. 集合と ∈ を条件に直す（[`member`]）: {点 | 条件} は点の変数を入れた条件、区間は不等式、ℤ は sin(πp) = 0、名前の集合は定義
//! 2. 否定を内側へ押し込み（ド・モルガン）、XOR・NAND・NOR・⇒・⇔ を「かつ・または・否定」に書き換える
//! 3. 「かつ」を「または」の上に分配して、かたまりの並びにする（16 個まで）
//!
//! 原子は「左 関係 右」（関係は <・≤・=・≠）。原子の否定は、< ↔ ≥、≤ ↔ >、= ↔ ≠

use crate::parser::{Expr, Formula, Rel, SetE};

/// かたまりの数の上限
pub const MAX_CLAUSES: usize = 16;

/// 原子の関係
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Lt,
    Le,
    Eq,
    Ne,
}

/// 原子 `左 関係 右`
#[derive(Debug, Clone, PartialEq)]
pub struct Lit {
    pub l: Expr,
    pub op: Op,
    pub r: Expr,
}

impl Lit {
    fn negate(&self) -> Lit {
        match self.op {
            Op::Lt => Lit { l: self.r.clone(), op: Op::Le, r: self.l.clone() },
            Op::Le => Lit { l: self.r.clone(), op: Op::Lt, r: self.l.clone() },
            Op::Eq => Lit { op: Op::Ne, ..self.clone() },
            Op::Ne => Lit { op: Op::Eq, ..self.clone() },
        }
    }
}

/// 否定を原子まで押し込んだ論理式
#[derive(Debug, Clone, PartialEq)]
enum Nnf {
    True,
    False,
    Lit(Lit),
    And(Vec<Nnf>),
    Or(Vec<Nnf>),
}

/// 集合の名前の定義を引く（`expand.rs` が渡す）
pub type SetLookup<'a> = &'a dyn Fn(&str) -> Option<SetE>;

/// 集合の点の成分の数（名前の集合は定義から。区間・数の集合は 1）
pub fn arity(s: &SetE, sets: SetLookup) -> Result<usize, String> {
    Ok(match s {
        SetE::Builder { point, .. } => match point {
            Expr::Tuple(items) => items.len(),
            _ => 1,
        },
        SetE::Named(n) => arity(&sets(n).ok_or_else(|| undefined_set(n))?, sets)?,
        SetE::Interval { .. } | SetE::Numbers(_) => 1,
        SetE::Finite(items) => match items.first() {
            Some(Expr::Tuple(c)) => c.len(),
            _ => 1,
        },
        SetE::Product(parts) => parts.iter().map(|p| arity(p, sets)).sum::<Result<usize, _>>()?,
        SetE::Union(a, _) | SetE::Inter(a, _) | SetE::Diff(a, _) | SetE::SymDiff(a, _) => arity(a, sets)?,
        SetE::Complement(a) => arity(a, sets)?,
        SetE::Empty => 0,
    })
}

fn undefined_set(n: &str) -> String {
    format!("集合 {n} は定義されていません（{n} = {{(x, y) | …}} の行を足してください）")
}

/// 点 p が集合 s に属するという条件
pub fn member(p: &Expr, s: &SetE, sets: SetLookup) -> Result<Formula, String> {
    let comps = |e: &Expr| -> Vec<Expr> {
        match e {
            Expr::Tuple(items) => items.clone(),
            _ => vec![e.clone()],
        }
    };
    let chain = |exprs: Vec<Expr>, rels: Vec<Rel>| Formula::Chain(crate::parser::Chain { pos: vec![0; exprs.len()], exprs, rels });
    Ok(match s {
        SetE::Builder { point, cond } => {
            let names: Vec<String> = comps(point)
                .iter()
                .map(|q| match q {
                    Expr::Var(v) => Ok(v.clone()),
                    _ => Err("媒介変数の式で書いた集合 {(x(t), y(t)) | …} には、∈ を使えません".to_string()),
                })
                .collect::<Result<_, _>>()?;
            let ps = comps(p);
            if ps.len() != names.len() {
                return Err(format!("点の成分の数（{}）が、集合の点の成分の数（{}）と違います", ps.len(), names.len()));
            }
            substitute(cond, &|v: &str| names.iter().position(|n| n == v).map(|k| ps[k].clone()))
        }
        SetE::Named(n) => member(p, &sets(n).ok_or_else(|| undefined_set(n))?, sets)?,
        // 有限集合: どれかの要素と等しい
        SetE::Finite(items) => {
            let ps = comps(p);
            let mut ors = Vec::new();
            for it in items {
                let qs = comps(it);
                if qs.len() != ps.len() {
                    return Err(format!("点の成分の数（{}）が、集合の要素の成分の数（{}）と違います", ps.len(), qs.len()));
                }
                let eqs: Vec<Formula> = ps.iter().zip(&qs).map(|(a, b)| chain(vec![a.clone(), b.clone()], vec![Rel::Eq])).collect();
                ors.push(if eqs.len() == 1 { eqs.into_iter().next().unwrap_or(Formula::True) } else { Formula::And(eqs) });
            }
            if ors.is_empty() { Formula::False } else { Formula::Or(ors) }
        }
        SetE::Interval { lo, hi, lo_open, hi_open } => {
            if matches!(p, Expr::Tuple(_)) {
                return Err("区間には、数（座標 1 つ）の ∈ だけが使えます".into());
            }
            let infinite = |e: &Expr| match e {
                Expr::Num(v) => v.is_infinite(),
                Expr::Neg(a) => matches!(**a, Expr::Num(v) if v.is_infinite()),
                _ => false,
            };
            let rel = |open: bool| if open { Rel::Lt } else { Rel::Le };
            let mut parts = Vec::new();
            if !infinite(lo) {
                parts.push(chain(vec![lo.clone(), p.clone()], vec![rel(*lo_open)]));
            }
            if !infinite(hi) {
                parts.push(chain(vec![p.clone(), hi.clone()], vec![rel(*hi_open)]));
            }
            if parts.is_empty() { Formula::True } else { Formula::And(parts) }
        }
        SetE::Numbers(c) => {
            if matches!(p, Expr::Tuple(_)) {
                return Err(format!("{c} には、数の ∈ だけが使えます（点なら {c} × {c} と書いてください）"));
            }
            let pi_p = Expr::Bin(crate::parser::BinOp::Mul, Box::new(Expr::Const(crate::parser::Constant::Pi)), Box::new(p.clone()));
            let integer = chain(vec![Expr::Call(crate::functions::Func::Sin, vec![pi_p]), Expr::Num(0.0)], vec![Rel::Eq]);
            match c {
                'ℝ' | 'ℂ' => Formula::True,
                'ℤ' => integer,
                'ℕ' => Formula::And(vec![integer, chain(vec![p.clone(), Expr::Num(1.0)], vec![Rel::Ge])]),
                _ => return Err("有理数の集合 ℚ は描けません（どの区間にも詰まっていて、線や面になりません）".into()),
            }
        }
        SetE::Product(parts) => {
            let ps = comps(p);
            let mut out = Vec::new();
            let mut k = 0;
            for part in parts {
                let n = arity(part, sets)?;
                if k + n > ps.len() {
                    return Err("直積の成分の数が、点の成分の数より多くなっています".into());
                }
                let q = if n == 1 { ps[k].clone() } else { Expr::Tuple(ps[k..k + n].to_vec()) };
                out.push(member(&q, part, sets)?);
                k += n;
            }
            if k != ps.len() {
                return Err(format!("点の成分の数（{}）が、直積の成分の数（{k}）と違います", ps.len()));
            }
            Formula::And(out)
        }
        SetE::Union(a, b) => Formula::Or(vec![member(p, a, sets)?, member(p, b, sets)?]),
        SetE::Inter(a, b) => Formula::And(vec![member(p, a, sets)?, member(p, b, sets)?]),
        SetE::Diff(a, b) => Formula::And(vec![member(p, a, sets)?, Formula::Not(Box::new(member(p, b, sets)?))]),
        SetE::SymDiff(a, b) => Formula::Xor(Box::new(member(p, a, sets)?), Box::new(member(p, b, sets)?)),
        SetE::Complement(a) => Formula::Not(Box::new(member(p, a, sets)?)),
        SetE::Empty => Formula::False,
    })
}

/// 論理式の式に変数の置き換えをする（{点 | 条件} の点の変数は、中で外の同じ名前を隠す）
pub fn substitute(f: &Formula, s: &dyn Fn(&str) -> Option<Expr>) -> Formula {
    let b = |g: &Formula| Box::new(substitute(g, s));
    match f {
        Formula::True | Formula::False => f.clone(),
        Formula::Chain(c) => Formula::Chain(crate::parser::Chain { exprs: c.exprs.iter().map(|e| e.substitute(s)).collect(), ..c.clone() }),
        Formula::Ne(a, c) => Formula::Ne(a.substitute(s), c.substitute(s)),
        Formula::Not(a) => Formula::Not(b(a)),
        Formula::And(v) => Formula::And(v.iter().map(|g| substitute(g, s)).collect()),
        Formula::Or(v) => Formula::Or(v.iter().map(|g| substitute(g, s)).collect()),
        Formula::Xor(x, y) => Formula::Xor(b(x), b(y)),
        Formula::Iff(x, y) => Formula::Iff(b(x), b(y)),
        Formula::Implies(x, y) => Formula::Implies(b(x), b(y)),
        Formula::Nand(x, y) => Formula::Nand(b(x), b(y)),
        Formula::Nor(x, y) => Formula::Nor(b(x), b(y)),
        Formula::In(p, set) => Formula::In(p.substitute(s), substitute_set(set, s)),
        Formula::Set(set) => Formula::Set(substitute_set(set, s)),
        Formula::Big { or, vars, cond, body } => {
            let mut bound = Vec::new();
            vars.collect_vars(&mut bound);
            let inner = |v: &str| if bound.iter().any(|n| n == v) { None } else { s(v) };
            Formula::Big { or: *or, vars: vars.clone(), cond: Box::new(substitute(cond, &inner)), body: Box::new(substitute(body, &inner)) }
        }
    }
}

fn substitute_set(set: &SetE, s: &dyn Fn(&str) -> Option<Expr>) -> SetE {
    let b = |x: &SetE| Box::new(substitute_set(x, s));
    match set {
        SetE::Builder { point, cond } => {
            let mut bound = Vec::new();
            point.collect_vars(&mut bound);
            let inner = |v: &str| if bound.iter().any(|n| n == v) { None } else { s(v) };
            SetE::Builder { point: point.clone(), cond: Box::new(substitute(cond, &inner)) }
        }
        SetE::Interval { lo, hi, lo_open, hi_open } => {
            SetE::Interval { lo: lo.substitute(s), hi: hi.substitute(s), lo_open: *lo_open, hi_open: *hi_open }
        }
        SetE::Product(parts) => SetE::Product(parts.iter().map(|p| substitute_set(p, s)).collect()),
        SetE::Union(x, y) => SetE::Union(b(x), b(y)),
        SetE::Inter(x, y) => SetE::Inter(b(x), b(y)),
        SetE::Diff(x, y) => SetE::Diff(b(x), b(y)),
        SetE::SymDiff(x, y) => SetE::SymDiff(b(x), b(y)),
        SetE::Complement(x) => SetE::Complement(b(x)),
        SetE::Finite(items) => SetE::Finite(items.iter().map(|e| e.substitute(s)).collect()),
        SetE::Named(_) | SetE::Numbers(_) | SetE::Empty => set.clone(),
    }
}

/// 集合と ∈ を、条件に直す。`coords` は、集合そのもの（`S`・{…}）が書かれたときの点（その行の表示中の座標）
fn resolve(f: &Formula, coords: &[String], sets: SetLookup) -> Result<Formula, String> {
    let r = |g: &Formula| resolve(g, coords, sets).map(Box::new);
    Ok(match f {
        Formula::In(p, s) => resolve(&member(p, s, sets)?, coords, sets)?,
        Formula::Set(s) => {
            let n = arity(s, sets)?.clamp(1, coords.len());
            let p = if n == 1 {
                Expr::Var(coords[0].clone())
            } else {
                Expr::Tuple(coords[..n].iter().map(|c| Expr::Var(c.clone())).collect())
            };
            resolve(&member(&p, s, sets)?, coords, sets)?
        }
        Formula::Not(a) => Formula::Not(r(a)?),
        Formula::And(v) => Formula::And(v.iter().map(|g| resolve(g, coords, sets)).collect::<Result<_, _>>()?),
        Formula::Or(v) => Formula::Or(v.iter().map(|g| resolve(g, coords, sets)).collect::<Result<_, _>>()?),
        Formula::Xor(x, y) => Formula::Xor(r(x)?, r(y)?),
        Formula::Iff(x, y) => Formula::Iff(r(x)?, r(y)?),
        Formula::Implies(x, y) => Formula::Implies(r(x)?, r(y)?),
        Formula::Nand(x, y) => Formula::Nand(r(x)?, r(y)?),
        Formula::Nor(x, y) => Formula::Nor(r(x)?, r(y)?),
        Formula::True | Formula::False | Formula::Chain(_) | Formula::Ne(..) => f.clone(),
        Formula::Big { .. } => return Err("内部エラー: ⋃・⋂ が展開されていません".into()),
    })
}

/// 否定を原子まで押し込む（`neg` は外側に否定が奇数個あるか）
fn nnf(f: &Formula, neg: bool) -> Nnf {
    let and_or = |v: Vec<Nnf>, is_and: bool| if is_and { Nnf::And(v) } else { Nnf::Or(v) };
    match f {
        Formula::True => if neg { Nnf::False } else { Nnf::True },
        Formula::False => if neg { Nnf::True } else { Nnf::False },
        Formula::Chain(c) => {
            let mut lits = Vec::new();
            for (k, rel) in c.rels.iter().enumerate() {
                let (a, b) = (c.exprs[k].clone(), c.exprs[k + 1].clone());
                lits.push(match rel {
                    Rel::Lt => Lit { l: a, op: Op::Lt, r: b },
                    Rel::Le => Lit { l: a, op: Op::Le, r: b },
                    Rel::Gt => Lit { l: b, op: Op::Lt, r: a },
                    Rel::Ge => Lit { l: b, op: Op::Le, r: a },
                    Rel::Eq => Lit { l: a, op: Op::Eq, r: b },
                });
            }
            // 連鎖は「かつ」。否定すると、それぞれの否定の「または」
            if neg {
                Nnf::Or(lits.iter().map(|l| Nnf::Lit(l.negate())).collect())
            } else {
                Nnf::And(lits.into_iter().map(Nnf::Lit).collect())
            }
        }
        Formula::Ne(a, b) => {
            let l = Lit { l: a.clone(), op: Op::Ne, r: b.clone() };
            Nnf::Lit(if neg { l.negate() } else { l })
        }
        Formula::Not(a) => nnf(a, !neg),
        Formula::And(v) => and_or(v.iter().map(|g| nnf(g, neg)).collect(), !neg),
        Formula::Or(v) => and_or(v.iter().map(|g| nnf(g, neg)).collect(), neg),
        // A ⊕ B = (A ∧ ¬B) ∨ (¬A ∧ B)、A ⇔ B = (A ∧ B) ∨ (¬A ∧ ¬B)
        Formula::Xor(a, b) | Formula::Iff(a, b) => {
            let xor = matches!(f, Formula::Xor(..)) != neg;
            if xor {
                Nnf::Or(vec![Nnf::And(vec![nnf(a, false), nnf(b, true)]), Nnf::And(vec![nnf(a, true), nnf(b, false)])])
            } else {
                Nnf::Or(vec![Nnf::And(vec![nnf(a, false), nnf(b, false)]), Nnf::And(vec![nnf(a, true), nnf(b, true)])])
            }
        }
        // A ⇒ B = ¬A ∨ B
        Formula::Implies(a, b) => and_or(vec![nnf(a, !neg), nnf(b, neg)], neg),
        // A ⊼ B = ¬(A ∧ B)、A ⊽ B = ¬(A ∨ B)
        Formula::Nand(a, b) => and_or(vec![nnf(a, !neg), nnf(b, !neg)], neg),
        Formula::Nor(a, b) => and_or(vec![nnf(a, !neg), nnf(b, !neg)], !neg),
        Formula::In(..) | Formula::Set(_) | Formula::Big { .. } => Nnf::False, // resolve の後には残らない
    }
}

/// 選言標準形: かたまり（原子の「かつ」）の並び。空の並びは「偽」、空のかたまりは「真」
fn dnf(f: &Nnf) -> Result<Vec<Vec<Lit>>, String> {
    let too_many = || format!("条件が複雑すぎます（「または」で分けると {MAX_CLAUSES} 個を超えます）");
    Ok(match f {
        Nnf::True => vec![Vec::new()],
        Nnf::False => Vec::new(),
        Nnf::Lit(l) => vec![vec![l.clone()]],
        Nnf::Or(v) => {
            let mut out = Vec::new();
            for g in v {
                out.extend(dnf(g)?);
                if out.len() > MAX_CLAUSES {
                    return Err(too_many());
                }
            }
            out
        }
        Nnf::And(v) => {
            let mut acc: Vec<Vec<Lit>> = vec![Vec::new()];
            for g in v {
                let d = dnf(g)?;
                let mut next = Vec::new();
                for a in &acc {
                    for b in &d {
                        let mut c = a.clone();
                        c.extend(b.iter().cloned());
                        next.push(c);
                    }
                }
                if next.len() > MAX_CLAUSES {
                    return Err(too_many());
                }
                acc = next;
            }
            acc
        }
    })
}

/// 論理式を選言標準形にする。`coords` は表示中の座標の名前、`sets` は集合の名前の定義
pub fn clauses(f: &Formula, coords: &[String], sets: SetLookup) -> Result<Vec<Vec<Lit>>, String> {
    let resolved = resolve(f, coords, sets)?;
    dnf(&nnf(&resolved, false))
}

/// 論理式を集合として読む（`S = A ∪ B` の右辺。集合と、その「または」「かつ」「否定」「XOR」だけ）
pub fn as_set(f: &Formula) -> Option<SetE> {
    Some(match f {
        Formula::Set(s) => s.clone(),
        Formula::Or(v) => v.iter().map(as_set).collect::<Option<Vec<_>>>()?.into_iter().reduce(|a, b| SetE::Union(Box::new(a), Box::new(b)))?,
        Formula::And(v) => v.iter().map(as_set).collect::<Option<Vec<_>>>()?.into_iter().reduce(|a, b| SetE::Inter(Box::new(a), Box::new(b)))?,
        Formula::Not(a) => SetE::Complement(Box::new(as_set(a)?)),
        Formula::Xor(a, b) => SetE::SymDiff(Box::new(as_set(a)?), Box::new(as_set(b)?)),
        Formula::False => SetE::Empty,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_formula;

    fn no_sets(_: &str) -> Option<SetE> {
        None
    }

    fn count(src: &str) -> (usize, Vec<usize>) {
        let f = parse_formula(src, &[]).unwrap_or_else(|e| panic!("{src}: {e}"));
        let c = clauses(&f, &["x".into(), "y".into(), "z".into()], &no_sets).unwrap_or_else(|e| panic!("{src}: {e}"));
        (c.len(), c.iter().map(Vec::len).collect())
    }

    #[test]
    fn normal_forms() {
        assert_eq!(count("x < 1 ∧ y < 1"), (1, vec![2]));
        assert_eq!(count("x < 1 ∨ y < 1"), (2, vec![1, 1]));
        assert_eq!(count("¬(x < 1 ∨ y < 1)"), (1, vec![2])); // ド・モルガン
        assert_eq!(count("x < 1 ⊕ y < 1"), (2, vec![2, 2])); // XOR
        assert_eq!(count("x < 1 ⇒ y < 1"), (2, vec![1, 1]));
        assert_eq!(count("x < 1 ⊼ y < 1"), (2, vec![1, 1])); // NAND = ¬A ∨ ¬B
        assert_eq!(count("x < 1 ⊽ y < 1"), (1, vec![2])); // NOR = ¬A ∧ ¬B
        assert_eq!(count("0 ≤ x < 1"), (1, vec![2]));
        assert_eq!(count("¬(0 ≤ x < 1)"), (2, vec![1, 1]));
        assert_eq!(count("(x < 1 ∨ x > 2) ∧ (y < 1 ∨ y > 2)"), (4, vec![2, 2, 2, 2]));
        assert_eq!(count("⊤"), (1, vec![0]));
        assert_eq!(count("⊥ ∨ ∅"), (0, vec![]));
        // 否定の向き: ¬(x < 1) は 1 ≤ x
        let f = parse_formula("¬(x < 1)", &[]).unwrap();
        let c = clauses(&f, &["x".into()], &no_sets).unwrap();
        assert_eq!(c[0][0].op, Op::Le);
        assert_eq!(c[0][0].l, Expr::Num(1.0));
    }

    #[test]
    fn sets_and_membership() {
        assert_eq!(count("⦃(x, y, z) ∣ x < 1, y < 1⦄"), (1, vec![2]));
        assert_eq!(count("(x, y, z) ∈ [0, 1] × [0, 2] × (0, 1]"), (1, vec![6]));
        assert_eq!(count("x ∈ [0, ∞)"), (1, vec![1]));
        assert_eq!(count("x ∈ ℝ"), (1, vec![0]));
        assert_eq!(count("x ∉ [0, 1]"), (2, vec![1, 1]));
        // 点の名前の置き換え（(a, b) を (x, y) に）
        let f = parse_formula("⦃(a, b) ∣ a^2 + b^2 < 1⦄", &[]).unwrap();
        let c = clauses(&f, &["x".into(), "y".into(), "z".into()], &no_sets).unwrap();
        assert!(c[0][0].l.uses_var("x") && c[0][0].l.uses_var("y") && !c[0][0].l.uses_var("a"));
        // 名前の集合と、差・対称差・補集合
        let a = parse_formula("⦃(x, y, z) ∣ x < 0⦄", &[]).unwrap();
        let sets = move |n: &str| if n == "A" { as_set(&a) } else { None };
        let c = |src: &str| clauses(&parse_formula(src, &[]).unwrap(), &["x".into(), "y".into(), "z".into()], &sets).unwrap().len();
        assert_eq!(c("A ∖ ⦃(x, y, z) ∣ y < 0⦄"), 1);
        assert_eq!(c("A △ ⦃(x, y, z) ∣ y < 0⦄"), 2);
        assert_eq!(c("Aᶜ"), 1);
        assert_eq!(c("A^(c) ∪ A"), 2);
        assert!(clauses(&parse_formula("B", &[]).unwrap(), &["x".into()], &sets).is_err());
        assert!(clauses(&parse_formula("x ∈ ℚ", &[]).unwrap(), &["x".into()], &sets).is_err());
    }
}

//! 陰関数 F = 0 を、描きやすい部品に分けるための式の変形。
//!
//! 陰関数の面は「F の符号が変わる所」として探す。F が 0 に触れるだけ（`(x² + y² − 4)² (z − 1)² = 0`）だったり、
//! 値が飛んだり（`mod(x y, z) = 0`）する式は、そのままでは符号が変わらず、格子の上の値から推測するしかない。
//! ここでは式そのものを変形して、符号がふつうに変わるなめらかな式の集まりにする。
//!
//! - **因数に分ける**（[`Program::zero_factors`]）: `A B = 0` は `A = 0` と `B = 0` を合わせたもの。`Aⁿ = 0`・`|A| = 0` は `A = 0`
//! - **段差の枝に分ける**（[`Program::frozen`]・[`Program::branch_conds`]）: `floor`・`mod` などの値を枝の番号 τ に固定した
//!   なめらかな式 F_τ と、「その枝にいる」という条件（段差の元の引数の範囲）の組にする

use super::*;

/// 部品の面のうち、描く部分の条件: `prog` の値 s が `lo`〜`hi` の範囲に入る所
pub struct Cond {
    pub prog: Program,
    pub lo: f64,
    pub hi: f64,
    /// 端の値そのものを範囲に含むか
    pub lo_closed: bool,
    pub hi_closed: bool,
}

impl Cond {
    /// 値 s が範囲に入るか
    pub fn holds(&self, s: f64) -> bool {
        (s > self.lo || (self.lo_closed && s == self.lo)) && (s < self.hi || (self.hi_closed && s == self.hi))
    }
}

impl Program {
    /// 同じ変数で、命令 `root` を結果にした式
    fn sub_program(&self, root: usize) -> Program {
        let mut b = Builder::default();
        let at = b.append(&self.code.ops, &Op::Var);
        Program { code: Code::new(b, vec![at[root]]), vars: self.vars.clone(), used_params: self.used_params.clone() }
    }

    /// 定数 0 と分かる命令か（0 との積も 0 とみなす。枝を固定した式の `b × 0` を消すため）
    fn is_zero_op(&self, i: usize) -> bool {
        match &self.code.ops[i] {
            Op::Const(c) => c.re == 0.0 && c.im == 0.0,
            Op::Neg(a) => self.is_zero_op(*a),
            Op::Bin(BinOp::Mul, a, b) => self.is_zero_op(*a) || self.is_zero_op(*b),
            _ => false,
        }
    }

    /// 命令 i の値が 0 になる所を、因数の命令の「どれかが 0 になる所」に分ける
    fn collect_factors(&self, i: usize, out: &mut Vec<usize>) {
        match &self.code.ops[i] {
            Op::Const(_) => {}
            Op::Neg(a) | Op::Call1(Func::Abs | Func::Sqrt, a) => self.collect_factors(*a, out),
            Op::Bin(BinOp::Mul, a, b) => {
                self.collect_factors(*a, out);
                self.collect_factors(*b, out);
            }
            // 分母が 0 になる所は零点ではない
            Op::Bin(BinOp::Div, a, _) => self.collect_factors(*a, out),
            Op::Bin(BinOp::Pow, a, e) if matches!(self.code.ops[*e], Op::Const(c) if c.im == 0.0 && c.re > 0.0) => {
                self.collect_factors(*a, out)
            }
            Op::Bin(BinOp::Sub | BinOp::Add, a, b) if self.is_zero_op(*b) => self.collect_factors(*a, out),
            Op::Bin(BinOp::Sub | BinOp::Add, a, b) if self.is_zero_op(*a) => self.collect_factors(*b, out),
            _ => {
                if !out.contains(&i) {
                    out.push(i);
                }
            }
        }
    }

    /// F = 0 を、因数ごとの「因数 = 0」に分ける。戻り値は因数の式と、分けて式が変わったか
    /// （変わらなければ、因数は元の式 1 つ）。定数だけの因数は入れない。
    /// 分けた因数の零点のうち、元の式が定義されない所（ほかの因数が定義されない所）は、元の式の零点ではない
    pub fn zero_factors(&self) -> (Vec<Program>, bool) {
        let root = self.code.outs[0];
        let mut ids = Vec::new();
        self.collect_factors(root, &mut ids);
        if ids == [root] {
            return (vec![self.sub_program(root)], false);
        }
        (ids.into_iter().map(|i| self.sub_program(i)).collect(), true)
    }

    /// 変数を使わない式（定数）か
    pub fn is_constant(&self) -> bool {
        !self.code.ops.iter().any(|op| matches!(op, Op::Var(_)))
    }

    /// 枝の番号 `k`（[`Program::eval_branches`] と同じ順）の枝にいる条件。段差の元ごとに 1 つ:
    /// `floor(s)` = τ は τ ≤ s < τ + 1、`ceil(s)` = τ は τ − 1 < s ≤ τ、`round(s)` = τ は s が τ ± 0.5 の間
    /// （ちょうど半分は 0 から遠いほうへ丸める）、`sign(s)` = τ は s の符号、`mod(a, b)` は s = a/b の `floor`
    pub fn branch_conds(&self, k: &[f64]) -> Vec<Cond> {
        let sites = self.branch_site_ops();
        assert_eq!(sites.len(), k.len());
        let mut b = Builder::default();
        let at = b.append(&self.code.ops, &Op::Var);
        let mut roots = Vec::with_capacity(sites.len());
        let mut bounds = Vec::with_capacity(sites.len());
        for (&i, &t) in sites.iter().zip(k) {
            let (root, bound) = match &self.code.ops[i] {
                Op::Call2(Func::Mod, a, d) => (b.push(Op::Bin(BinOp::Div, at[*a], at[*d])), (t, t + 1.0, true, false)),
                Op::Call1(Func::Floor, a) => (at[*a], (t, t + 1.0, true, false)),
                Op::Call1(Func::Ceil, a) => (at[*a], (t - 1.0, t, false, true)),
                Op::Call1(Func::Round, a) => (at[*a], (t - 0.5, t + 0.5, t > 0.0, t < 0.0)),
                Op::Call1(_, a) => (
                    at[*a],
                    if t > 0.0 {
                        (0.0, f64::INFINITY, false, false)
                    } else if t < 0.0 {
                        (f64::NEG_INFINITY, 0.0, false, false)
                    } else {
                        (0.0, 0.0, true, true)
                    },
                ),
                _ => unreachable!("段差の元ではない命令"),
            };
            roots.push(root);
            bounds.push(bound);
        }
        let ops = b.ops;
        roots
            .into_iter()
            .zip(bounds)
            .map(|(root, (lo, hi, lo_closed, hi_closed))| {
                let mut nb = Builder::default();
                let at = nb.append(&ops, &Op::Var);
                let prog =
                    Program { code: Code::new(nb, vec![at[root]]), vars: self.vars.clone(), used_params: self.used_params.clone() };
                Cond { prog, lo, hi, lo_closed, hi_closed }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prog(src: &str) -> Program {
        Program::compile(&crate::parser::parse(src).unwrap(), &["x", "y", "z"]).unwrap()
    }

    fn at(p: &Program, q: [f64; 3]) -> f64 {
        let mut out = [C::new(0.0, 0.0)];
        p.eval_batch(&[&[q[0]], &[q[1]], &[q[2]]], &mut out);
        as_real(out[0])
    }

    #[test]
    fn factors_of_products_and_powers() {
        let (f, changed) = prog("(x^2+y^2-4)^2 (z-1)^2").zero_factors();
        assert!(changed);
        assert_eq!(f.len(), 2);
        assert_eq!(at(&f[0], [1.0, 2.0, 9.0]), 1.0); // x² + y² − 4
        assert_eq!(at(&f[1], [1.0, 2.0, 9.0]), 8.0); // z − 1
        // 和・差は分けない
        let (f, changed) = prog("x y z - 1").zero_factors();
        assert!(!changed);
        assert_eq!(f.len(), 1);
        // abs・正のべき・符号の反転・「− 0」は外す
        let (f, changed) = prog("-abs(sin(x) y)^3 - 0").zero_factors();
        assert!(changed);
        assert_eq!(f.len(), 2);
        // 同じ因数は 1 つにまとめる
        assert_eq!(prog("(x - 1) (x - 1) y").zero_factors().0.len(), 2);
    }

    #[test]
    fn branch_conds_of_mod_and_floor() {
        let p = prog("mod(x y, z) + floor(x)");
        let conds = p.branch_conds(&[2.0, -1.0]);
        assert_eq!(conds.len(), 2);
        // mod(x y, z) の枝 2: 2 ≤ x y / z < 3
        assert_eq!(at(&conds[0].prog, [3.0, 5.0, 6.0]), 2.5);
        assert!(conds[0].holds(2.0) && conds[0].holds(2.5) && !conds[0].holds(3.0));
        // floor(x) の枝 −1: −1 ≤ x < 0
        assert_eq!(at(&conds[1].prog, [-0.25, 0.0, 0.0]), -0.25);
        assert!(conds[1].holds(-1.0) && !conds[1].holds(0.0));
        // 枝 0 の mod(x y, z) は x y − 0 z。0 との積を消して、x と y に分かれる
        let (f, changed) = prog("mod(x y, z)").frozen(&[0.0]).zero_factors();
        assert!(changed);
        assert_eq!(f.len(), 2);
    }
}

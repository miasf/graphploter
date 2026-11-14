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

/// 枝の中での式の値の範囲を、区間演算で調べるための式（[`Program::frozen_relaxed`]）
pub struct Relaxed {
    /// 枝を固定した式の、`mod(a, b)` の値（余り）を新しい変数に置き換えたもの。変数は元の変数の後ろに、余りが出てくる順に並ぶ
    prog: Program,
    /// 余りごとの、割る数 b の式（変数の並びは `prog` と同じ）
    divisors: Vec<Program>,
}

impl Relaxed {
    /// 箱（元の変数ごとの区間）の中の、その枝にいる点で、枝を固定した式が 0 になるかもしれないか。
    ///
    /// 枝を固定した式 a − τ b をそのまま区間演算にかけると、a と b の幅がそのまま出て「あるかもしれない」ばかりになる。
    /// その枝にいる点では、余り a − τ b は 0 と b の間にあるので、余りをその範囲の独立な変数とみなして調べる
    pub fn may_contain_zero(&self, boxes: &[crate::interval::Interval]) -> bool {
        use crate::interval::{Interval, Range};
        let nv = boxes.len();
        let mut all: Vec<Interval> = boxes.to_vec();
        all.resize(nv + self.divisors.len(), Interval::new(f64::NEG_INFINITY, f64::INFINITY));
        for (j, d) in self.divisors.iter().enumerate() {
            match d.eval_interval(&all) {
                Range::Known(i) => all[nv + j] = Interval::new(i.lo.min(0.0), i.hi.max(0.0)),
                Range::Unknown => return true,
            }
        }
        self.prog.eval_interval(&all).may_contain_zero()
    }
}

impl Program {
    /// [`Program::frozen`] と同じ枝の式を、区間演算で「その枝の中に零点があるか」を調べやすい形にしたもの
    pub fn frozen_relaxed(&self, k: &[f64]) -> Relaxed {
        let sites = self.branch_site_ops();
        assert_eq!(sites.len(), k.len());
        let nv = self.vars.len();
        let mut b = Builder::default();
        let mut at: Vec<usize> = Vec::with_capacity(self.code.ops.len());
        let mut div_roots: Vec<usize> = Vec::new();
        for (i, op) in self.code.ops.iter().enumerate() {
            let new = if let Some(s) = sites.iter().position(|&x| x == i) {
                match op {
                    Op::Call2(Func::Mod, _, d) => {
                        div_roots.push(at[*d]);
                        Op::Var(nv + div_roots.len() - 1)
                    }
                    _ => Op::Const(C::new(k[s], 0.0)),
                }
            } else {
                match op {
                    Op::Const(c) => Op::Const(*c),
                    Op::Var(v) => Op::Var(*v),
                    Op::Neg(a) => Op::Neg(at[*a]),
                    Op::Bin(o, a, c) => Op::Bin(*o, at[*a], at[*c]),
                    Op::Call1(f, a) => Op::Call1(*f, at[*a]),
                    Op::Call2(f, a, c) => Op::Call2(*f, at[*a], at[*c]),
                    Op::CallN(f, args) => Op::CallN(*f, args.iter().map(|&a| at[a]).collect()),
                }
            };
            at.push(b.push(new));
        }
        let mut vars = self.vars.clone();
        vars.extend((0..div_roots.len()).map(|j| format!("\u{0}余り{j}")));
        let ops = b.ops;
        let make = |root: usize| {
            let mut nb = Builder::default();
            let at = nb.append(&ops, &Op::Var);
            Program { code: Code::new(nb, vec![at[root]]), vars: vars.clone(), used_params: self.used_params.clone() }
        };
        Relaxed { prog: make(at[self.code.outs[0]]), divisors: div_roots.iter().map(|&r| make(r)).collect() }
    }

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

    /// 実数の定数の命令なら、その値
    fn real_const(&self, i: usize) -> Option<f64> {
        match self.code.ops[i] {
            Op::Const(c) if c.im == 0.0 => Some(c.re),
            _ => None,
        }
    }

    /// 命令 i が、実数ならいつも 0 以上で、0 になるのが「ある式 A = 0」の所だけの形（A^(偶数)・|A|・A·A）なら、その A
    fn nonneg_base(&self, i: usize) -> Option<usize> {
        match &self.code.ops[i] {
            Op::Call1(Func::Abs, a) => Some(*a),
            Op::Bin(BinOp::Mul, a, b) if a == b => Some(*a),
            Op::Bin(BinOp::Pow, a, e) => {
                let e = self.real_const(*e)?;
                (e > 0.0 && e % 2.0 == 0.0).then_some(*a)
            }
            _ => None,
        }
    }

    /// 命令 i を、符号 `sign` を付けた項の和に分けて、どの項も 0 以上の形（[`Program::nonneg_base`]）なら、
    /// (項の A, 符号) を `out` に足す。0 以上の形でない項・0 でない定数の項があれば false
    fn nonneg_terms(&self, i: usize, sign: f64, out: &mut Vec<(usize, f64)>) -> bool {
        match &self.code.ops[i] {
            Op::Bin(BinOp::Add, a, b) => self.nonneg_terms(*a, sign, out) && self.nonneg_terms(*b, sign, out),
            Op::Bin(BinOp::Sub, a, b) => self.nonneg_terms(*a, sign, out) && self.nonneg_terms(*b, -sign, out),
            Op::Neg(a) => self.nonneg_terms(*a, -sign, out),
            Op::Const(c) => c.re == 0.0 && c.im == 0.0,
            Op::Bin(BinOp::Mul, a, b) if self.real_const(*a).is_some_and(|c| c != 0.0) => {
                self.nonneg_terms(*b, sign * self.real_const(*a).unwrap().signum(), out)
            }
            Op::Bin(BinOp::Mul, a, b) if self.real_const(*b).is_some_and(|c| c != 0.0) => {
                self.nonneg_terms(*a, sign * self.real_const(*b).unwrap().signum(), out)
            }
            Op::Bin(BinOp::Div, a, b) if self.real_const(*b).is_some_and(|c| c != 0.0) => {
                self.nonneg_terms(*a, sign * self.real_const(*b).unwrap().signum(), out)
            }
            _ => match self.nonneg_base(i) {
                Some(base) => {
                    out.push((base, sign));
                    true
                }
                None => false,
            },
        }
    }

    /// F = 0 の解が面にならず、いくつかの式が同時に 0 になる所（線・点）だけになる形なら、因数ごとに、同時に 0 になる式の組。
    ///
    /// 因数（[`Program::zero_factors`]）のどれもが「0 以上の項の和」（$A^2 + B^2$・$|A| + |B|$・$-A^4 - 3B^2$ など。符号は全部の項で同じ）で、
    /// 項の A が 2 つ以上あるとき。$A^2 + B^2 = 0 \iff A = B = 0$ なので、式が 3 つの座標で 2 つなら線、3 つ以上なら点になる。
    /// 実数だけで計算できない式（0 以上と言えない）、面になる因数が混ざる式（`(x^2 + y^2)(z - 1) = 0`）は None
    pub fn common_zero_sets(&self) -> Option<Vec<Vec<Program>>> {
        if !self.code.real {
            return None;
        }
        let (factors, _) = self.zero_factors();
        let mut out = Vec::new();
        for f in factors.iter().filter(|f| !f.is_constant()) {
            let mut terms = Vec::new();
            if !f.nonneg_terms(f.code.outs[0], 1.0, &mut terms) || terms.iter().any(|t| t.1 != terms[0].1) {
                return None;
            }
            let mut bases: Vec<usize> = Vec::new();
            for (b, _) in terms {
                if !bases.contains(&b) {
                    bases.push(b);
                }
            }
            let parts: Vec<Program> = bases.into_iter().map(|b| f.sub_program(b)).filter(|p| !p.is_constant()).collect();
            if parts.len() < 2 {
                return None;
            }
            out.push(parts);
        }
        (!out.is_empty()).then_some(out)
    }

    /// F を分数 N/D の形に直す（通分）。F = 0 は「N = 0 で、D ≠ 0」と同じで、N には D の極（符号は変わるが零点でない所）が無い。
    /// `tan` → `sin/cos`、`cot` → `cos/sin`、`sec`・`csc` → `1/cos`・`1/sin`、割り算と負のべきは分母へ、和・差は通分する。
    /// それ以外の関数は、引数の中に割り算があっても、そのまま 1 つの項として扱う。
    /// 分母の無い式（変形しても変わらない式）と、実数だけで計算できない式は None
    pub fn rationalized(&self) -> Option<(Program, Program)> {
        if !self.code.real {
            return None;
        }
        let ops = &self.code.ops;
        let mut b = Builder::default();
        let at = b.append(ops, &Op::Var);
        // 命令ごとの (分子, 分母)。分母が None なら 1
        let mut nd: Vec<(usize, Option<usize>)> = Vec::with_capacity(ops.len());
        fn mul(b: &mut Builder, x: usize, y: Option<usize>) -> usize {
            match y {
                Some(y) => b.push(Op::Bin(BinOp::Mul, x, y)),
                None => x,
            }
        }
        fn mul_opt(b: &mut Builder, x: Option<usize>, y: Option<usize>) -> Option<usize> {
            match (x, y) {
                (Some(x), y) => Some(mul(b, x, y)),
                (None, y) => y,
            }
        }
        for (i, op) in ops.iter().enumerate() {
            let cur = match op {
                Op::Neg(a) => {
                    let (n, d) = nd[*a];
                    (b.push(Op::Neg(n)), d)
                }
                Op::Bin(o @ (BinOp::Add | BinOp::Sub), a, c) => {
                    let ((na, da), (nc, dc)) = (nd[*a], nd[*c]);
                    if da.is_none() && dc.is_none() {
                        (at[i], None)
                    } else if da == dc {
                        (b.push(Op::Bin(*o, na, nc)), da)
                    } else {
                        let (l, r) = (mul(&mut b, na, dc), mul(&mut b, nc, da));
                        (b.push(Op::Bin(*o, l, r)), mul_opt(&mut b, da, dc))
                    }
                }
                Op::Bin(BinOp::Mul, a, c) => {
                    let ((na, da), (nc, dc)) = (nd[*a], nd[*c]);
                    if da.is_none() && dc.is_none() {
                        (at[i], None)
                    } else {
                        (b.push(Op::Bin(BinOp::Mul, na, nc)), mul_opt(&mut b, da, dc))
                    }
                }
                Op::Bin(BinOp::Div, a, c) => {
                    let ((na, da), (nc, dc)) = (nd[*a], nd[*c]);
                    if dc.is_none() && b.const_of(nc).is_some() {
                        // 定数で割るだけなら、分母にしない
                        (b.push(Op::Bin(BinOp::Div, na, nc)), da)
                    } else {
                        (mul(&mut b, na, dc), Some(mul(&mut b, nc, da)))
                    }
                }
                Op::Bin(BinOp::Pow, a, e) => {
                    let (na, da) = nd[*a];
                    match b.const_of(at[*e]) {
                        Some(c) if c.im == 0.0 && c.re < 0.0 => {
                            // a^(−p) = 1 / a^p。整数の p なら、分子と分母を入れ替えてべきにする
                            let p = b.push(Op::Const(C::new(-c.re, 0.0)));
                            let one = b.push(Op::Const(C::new(1.0, 0.0)));
                            if c.re.fract() == 0.0 {
                                let num = match da {
                                    Some(d) => b.push(Op::Bin(BinOp::Pow, d, p)),
                                    None => one,
                                };
                                (num, Some(b.push(Op::Bin(BinOp::Pow, na, p))))
                            } else {
                                (one, Some(b.push(Op::Bin(BinOp::Pow, at[*a], p))))
                            }
                        }
                        Some(c) if c.im == 0.0 && c.re > 0.0 && c.re.fract() == 0.0 && da.is_some() => {
                            let d = da.map(|d| b.push(Op::Bin(BinOp::Pow, d, at[*e])));
                            (b.push(Op::Bin(BinOp::Pow, na, at[*e])), d)
                        }
                        _ => (at[i], None),
                    }
                }
                Op::Call1(f @ (Func::Tan | Func::Cot | Func::Sec | Func::Csc), a) => {
                    let (sin, cos) = (b.push(Op::Call1(Func::Sin, at[*a])), b.push(Op::Call1(Func::Cos, at[*a])));
                    let one = b.push(Op::Const(C::new(1.0, 0.0)));
                    match f {
                        Func::Tan => (sin, Some(cos)),
                        Func::Cot => (cos, Some(sin)),
                        Func::Sec => (one, Some(cos)),
                        _ => (one, Some(sin)),
                    }
                }
                _ => (at[i], None),
            };
            nd.push(cur);
        }
        let (n, d) = nd[self.code.outs[0]];
        let d = d?;
        let ops = b.ops;
        let make = |root: usize| {
            let mut nb = Builder::default();
            let at = nb.append(&ops, &Op::Var);
            Program { code: Code::new(nb, vec![at[root]]), vars: self.vars.clone(), used_params: self.used_params.clone() }
        };
        Some((make(n), make(d)))
    }

    /// 式 F が変数 `v` について 1 次（F = a v + b。a・b は `v` を含まない）なら、F = 0 を解いた v = −b/a と a。
    /// どちらも変数は `v` 以外の変数（元の並び）。a が定数 0 の式（`v` を含まない式）は None。
    /// 1 次かどうかは式の形で決める（`v` を含む関数の引数・`v` どうしの積・`v` での割り算・べきがあれば 1 次でない）
    pub fn solved_for(&self, v: usize) -> Option<(Program, Program)> {
        #[derive(Clone, Copy)]
        enum R {
            /// v を含まない（新しい命令の番号）
            Ind(usize),
            /// a v + b（a・b の新しい命令の番号）
            Aff(usize, usize),
        }
        let remap = |k: usize| if k < v { k } else { k - 1 };
        let mut b = Builder::default();
        let zero = b.push(Op::Const(C::new(0.0, 0.0)));
        let one = b.push(Op::Const(C::new(1.0, 0.0)));
        let mut rep: Vec<R> = Vec::with_capacity(self.code.ops.len());
        for op in &self.code.ops {
            let ind = |r: R| match r {
                R::Ind(i) => Some(i),
                R::Aff(..) => None,
            };
            let aff = |r: R| match r {
                R::Ind(i) => (zero, i),
                R::Aff(a, c) => (a, c),
            };
            let r = match op {
                Op::Const(c) => R::Ind(b.push(Op::Const(*c))),
                Op::Var(k) if *k == v => R::Aff(one, zero),
                Op::Var(k) => R::Ind(b.push(Op::Var(remap(*k)))),
                Op::Neg(x) => match rep[*x] {
                    R::Ind(i) => R::Ind(b.push(Op::Neg(i))),
                    R::Aff(a, c) => R::Aff(b.push(Op::Neg(a)), b.push(Op::Neg(c))),
                },
                Op::Bin(o, x, y) => {
                    let (rx, ry) = (rep[*x], rep[*y]);
                    match (o, ind(rx), ind(ry)) {
                        (_, Some(i), Some(j)) => R::Ind(b.push(Op::Bin(*o, i, j))),
                        (BinOp::Add | BinOp::Sub, _, _) => {
                            let ((a1, c1), (a2, c2)) = (aff(rx), aff(ry));
                            R::Aff(b.push(Op::Bin(*o, a1, a2)), b.push(Op::Bin(*o, c1, c2)))
                        }
                        (BinOp::Mul, Some(i), None) | (BinOp::Mul, None, Some(i)) => {
                            let (a, c) = if ind(rx).is_some() { aff(ry) } else { aff(rx) };
                            R::Aff(b.push(Op::Bin(BinOp::Mul, i, a)), b.push(Op::Bin(BinOp::Mul, i, c)))
                        }
                        (BinOp::Div, None, Some(j)) => {
                            let (a, c) = aff(rx);
                            R::Aff(b.push(Op::Bin(BinOp::Div, a, j)), b.push(Op::Bin(BinOp::Div, c, j)))
                        }
                        _ => return None,
                    }
                }
                Op::Call1(f, x) => R::Ind(b.push(Op::Call1(*f, ind(rep[*x])?))),
                Op::Call2(f, x, y) => R::Ind(b.push(Op::Call2(*f, ind(rep[*x])?, ind(rep[*y])?))),
                Op::CallN(f, args) => {
                    let a: Option<Vec<usize>> = args.iter().map(|&x| ind(rep[x])).collect();
                    R::Ind(b.push(Op::CallN(*f, a?)))
                }
            };
            rep.push(r);
        }
        let R::Aff(a, c) = rep[self.code.outs[0]] else { return None };
        if b.const_of(a).is_some_and(|k| k == C::new(0.0, 0.0)) {
            return None;
        }
        let nc = b.push(Op::Neg(c));
        let g = b.push(Op::Bin(BinOp::Div, nc, a));
        let ops = b.ops;
        let mut vars = self.vars.clone();
        vars.remove(v);
        let make = |root: usize| {
            let mut nb = Builder::default();
            let at = nb.append(&ops, &Op::Var);
            Program { code: Code::new(nb, vec![at[root]]), vars: vars.clone(), used_params: self.used_params.clone() }
        };
        Some((make(g), make(a)))
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
    fn rationalized_removes_poles() {
        // 値は元の式と同じ（N/D）で、分子には極が無い
        for src in ["tan(x) - z y", "1/(x y) - z", "x/(y - 1) + 2/(z + 3) - cot(x)", "(x/y)^2 - sec(z) + y^(-2)", "-(1/x) / (y/z) + x^(-0.5)"] {
            let p = prog(src);
            let (n, d) = p.rationalized().unwrap_or_else(|| panic!("{src}"));
            for q in [[0.7, 1.9, -2.3], [2.1, -0.4, 1.1], [1.3, 3.3, 0.6]] {
                let (f, r) = (at(&p, q), at(&n, q) / at(&d, q));
                assert!((f - r).abs() <= 1e-9 * (1.0 + f.abs()), "{src} {q:?}: {f} {r}");
            }
        }
        let (n, d) = prog("tan(x) - z y").rationalized().unwrap();
        let pole = [std::f64::consts::FRAC_PI_2, 1.0, 1.0];
        assert!((at(&n, pole) - 1.0).abs() < 1e-12 && at(&d, pole).abs() < 1e-12);
        // 分母の無い式・定数で割るだけの式は変えない
        assert!(prog("x^2 + sin(y) z / 3 - 1").rationalized().is_none());
        // 関数の引数の中の割り算は、そのまま
        assert!(prog("sin(1/x) - z").rationalized().is_none());
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
        // 枝の中での値の範囲: floor(x) + floor(y) + z − mod(x y, 3) の枝 (1, 2, 0) は 3 + z − [0, 3]。
        // x・y の箱が広くても、z が −3 〜 0 の外なら零点は無い
        use crate::interval::Interval;
        let r = prog("floor(x) + floor(y) + z - mod(x y, 3)").frozen_relaxed(&[1.0, 2.0, 0.0]);
        let wide = Interval::new(-10.0, 10.0);
        assert!(r.may_contain_zero(&[wide, wide, Interval::new(-1.0, -0.5)]));
        assert!(!r.may_contain_zero(&[wide, wide, Interval::new(0.5, 1.0)]));
        assert!(!r.may_contain_zero(&[wide, wide, Interval::new(-4.0, -3.5)]));
        // 割る数が変数のとき: mod(x y, z) − 1 の余りは 0 と z の間。z が 1 より小さければ零点は無い
        let r = prog("mod(x y, z) - 1").frozen_relaxed(&[5.0]);
        assert!(!r.may_contain_zero(&[wide, wide, Interval::new(0.2, 0.9)]));
        assert!(r.may_contain_zero(&[wide, wide, Interval::new(0.2, 1.5)]));
        // 枝 0 の mod(x y, z) は x y − 0 z。0 との積を消して、x と y に分かれる
        let (f, changed) = prog("mod(x y, z)").frozen(&[0.0]).zero_factors();
        assert!(changed);
        assert_eq!(f.len(), 2);
    }
}

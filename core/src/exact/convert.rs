//! 展開の前の構文木 `Expr` → 厳密値の道の式 [`Term`]（計画 `docs/plan-exact-core.md` の EX2 の 2）。
//!
//! 展開の段（`expand.rs`）は積分・極限・∞ の和を f64 にし、`diff.rs` も数どうしを f64 にまとめるので、厳密値の道は通らない。
//! 解析した直後の木から作る:
//! - 数の字句は、最短の 10 進表記の分数（`0.3` → 3/10）。小数があったら `decimal` の印
//! - 定義した関数の呼び出し `f(2)` は、定義の本体に引数を入れる（再帰は深さ 32 まで）
//! - スライダーの文字は今の値（最短の 10 進表記の分数）、座標と媒介変数は文字 `Sym`、束縛された変数（Σ の添字）はその値
//! - 記法（`Binder`・`Op`）は [`handler`] の表にあるものだけ。表に無い記法は「まだ厳密に計算できません」

use super::num::{self, Int, Rat};
use super::term::{add, call, div, mul, neg, pow, sub, Atom, Term};
use super::{exhausted, Stop};
use crate::eval::Params;
use crate::expand::{Def, Defs};
use crate::parser::{BinOp, Constant, Expr};

/// 束縛の枠。関数の本体に入るときは `barrier` の枠を積み、その下の枠は見えなくする
struct Frame {
    vars: Vec<(String, Term)>,
    barrier: bool,
}

pub struct Converter<'a> {
    defs: &'a Defs,
    params: &'a Params,
    frames: Vec<Frame>,
    depth: usize,
    /// 入力に小数の字句があった（答えも小数で出す。計画の 2.7）
    pub decimal: bool,
    /// 四則と整数の関数以外の規則（極限・積分など）を使った。照らし合わせの相手が無いとき、答えを捨てる（計画の 2.4）
    pub analytic: bool,
    /// 入力に自然対数 `ln` があった（D5: 書き出しは `\ln`）
    pub uses_ln: bool,
    /// 行が不定積分だけなら（中身, 変数）。答えは原始関数で、微分して戻ることで確かめ、書き出しで + C を足す
    pub antiderivative: Option<(Term, String)>,
    /// スライダーの文字を今の値にせず、文字 `Sym` のままにする（式の表示 `f(x+2)` → $a(x+2)^2$。`expression.rs`）
    pub keep_params: bool,
}

/// 記法の厳密な計算（文書 8 の各ステップが 1 つずつ足す）
type Handler = fn(&mut Converter, &str, &[Expr], &Expr) -> Result<Term, Stop>;

/// 束縛する記法（`Binder { op, … }`）の表
fn handler(op: &str) -> Option<Handler> {
    match op {
        "sum" => Some(finite_sum),
        "prod" => Some(finite_prod),
        "diff" => Some(derivative),
        "lim" => Some(limit_handler),
        "int" => Some(integral_handler),
        "seqlim" => Some(seqlimit_handler),
        _ => None,
    }
}

impl<'a> Converter<'a> {
    pub fn new(defs: &'a Defs, params: &'a Params) -> Self {
        Converter { defs, params, frames: Vec::new(), depth: 0, decimal: false, analytic: false, uses_ln: false, antiderivative: None, keep_params: false }
    }

    fn lookup(&self, name: &str) -> Option<Term> {
        for f in self.frames.iter().rev() {
            if let Some((_, t)) = f.vars.iter().rev().find(|(n, _)| n == name) {
                return Some(t.clone());
            }
            if f.barrier {
                break;
            }
        }
        None
    }

    pub fn term(&mut self, e: &Expr) -> Result<Term, Stop> {
        if exhausted() {
            return Err(Stop::TimeUp);
        }
        let t = match e {
            Expr::Num(v) => {
                if v.fract() != 0.0 {
                    self.decimal = true;
                }
                if v.is_infinite() {
                    return Err(Stop::NotFound("∞ を含む式はまだ厳密に計算できません".into()));
                }
                // 2^53 以上の整数の字句は、f64 になった時点で下の桁が丸められているかもしれない（字句の文字は構文木に残らない）
                if v.fract() == 0.0 && v.abs() >= 9007199254740992.0 {
                    return Err(Stop::NotFound("16 桁を超える整数の字句は厳密に扱えません（2^60+1 のように式で書いてください）".into()));
                }
                Term::Num(num::rat_of_literal(*v).ok_or_else(|| Stop::NotFound("数が有限ではありません".into()))?)
            }
            Expr::Const(Constant::Pi) => Term::Atom(Atom::Pi),
            Expr::Const(Constant::E) => Term::Atom(Atom::E),
            Expr::Const(Constant::I) => Term::Atom(Atom::I),
            Expr::Const(Constant::EulerGamma) => Term::Atom(Atom::EulerGamma),
            Expr::Const(Constant::Catalan) => Term::Atom(Atom::Catalan),
            Expr::Var(name) => self.var(name)?,
            Expr::Neg(a) => neg(self.term(a)?),
            Expr::Bin(op, a, b) => {
                let (x, y) = (self.term(a)?, self.term(b)?);
                if let Some(t) = linear_bin(*op, &x, &y)? {
                    return Ok(t);
                }
                match op {
                    BinOp::Add => add(vec![x, y]),
                    BinOp::Sub => sub(x, y),
                    BinOp::Mul => mul(vec![x, y]),
                    BinOp::Div => {
                        if y.is_zero() {
                            return Err(Stop::NotFound("0 で割っています".into()));
                        }
                        div(x, y)
                    }
                    BinOp::Pow => pow(x, y),
                }
            }
            Expr::Call(f, args) => {
                if *f == crate::functions::Func::Ln {
                    self.uses_ln = true;
                }
                let args = args.iter().map(|a| self.term(a)).collect::<Result<Vec<_>, _>>()?;
                call(*f, args)
            }
            Expr::Tuple(items) => Term::Tuple(items.iter().map(|a| self.term(a)).collect::<Result<_, _>>()?),
            Expr::List(items) => Term::List(items.iter().map(|a| self.term(a)).collect::<Result<_, _>>()?),
            Expr::Apply { name, primes, args } => self.apply(name, *primes, args)?,
            Expr::Binder { op, var, args, body } => match handler(op) {
                Some(h) => h(self, var, args, body)?,
                None => return Err(Stop::NotFound(format!("記法 {op} はまだ厳密に計算できません"))),
            },
            Expr::Op { op, args } if op == "at" => self.at(args)?,
            Expr::Op { op, args } if matches!(op.as_str(), "det" | "tr" | "inv" | "transpose" | "rank" | "norm" | "dot") => self.matrix_op(op, args)?,
            Expr::Matrix { rows, cols, items } => {
                let a = items.iter().map(|x| self.term(x)).collect::<Result<Vec<_>, _>>()?;
                super::matrix::EMat { rows: *rows, cols: *cols, a }.into_term()
            }
            Expr::Op { op, args } if matches!(op.as_str(), "grad" | "div" | "rot" | "curl" | "lap") && args.len() == 1 => self.vector_operator(op, &args[0])?,
            Expr::Op { op, .. } => return Err(Stop::NotFound(format!("記法 {op} はまだ厳密に計算できません"))),
            other => return Err(Stop::NotFound(format!("この書き方はまだ厳密に計算できません（{}）", short(other)))),
        };
        if t.size() > 100_000 {
            return Err(Stop::NotFound("式が大きすぎます".into()));
        }
        if t.has_division_by_zero() {
            return Err(Stop::NotFound("0 で割っています".into()));
        }
        if exhausted() {
            return Err(Stop::TimeUp);
        }
        Ok(t)
    }

    fn var(&mut self, name: &str) -> Result<Term, Stop> {
        if let Some(t) = self.lookup(name) {
            return Ok(t);
        }
        // 引数の無い定義（行列・ベクトル `A = ⟦…⟧`）は、その右辺（文書 8 の LA1。今の道の `Def::Value` と同じ）
        if let Some(Def::Value(body)) = self.defs.get(name).cloned() {
            if self.depth >= 32 {
                return Err(Stop::NotFound("定義の呼び出しが深すぎます".into()));
            }
            self.frames.push(Frame { vars: Vec::new(), barrier: true });
            self.depth += 1;
            let r = self.term(&body);
            self.depth -= 1;
            self.frames.pop();
            return r;
        }
        if self.keep_params && self.params.contains_key(name) {
            return Ok(Term::Sym(name.to_string()));
        }
        if let Some(c) = self.params.get(name) {
            let re = num::rat_of_literal(c.re).ok_or_else(|| Stop::NotFound(format!("{name} の値が有限ではありません")))?;
            if c.re.fract() != 0.0 {
                self.decimal = true;
            }
            if c.im == 0.0 {
                return Ok(Term::Num(re));
            }
            let im = num::rat_of_literal(c.im).ok_or_else(|| Stop::NotFound(format!("{name} の値が有限ではありません")))?;
            return Ok(add(vec![Term::Num(re), mul(vec![Term::Num(im), Term::Atom(Atom::I)])]));
        }
        let is_param_var = matches!(name, "t" | "u" | "v") || name.strip_prefix('t').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
        if crate::equation::coord_index(name).is_some() || is_param_var {
            return Ok(Term::Sym(name.to_string()));
        }
        Err(Stop::NotFound(format!("変数 '{name}' はここでは使えません（パラメータとして定義してください）")))
    }

    fn apply(&mut self, name: &str, primes: u8, args: &[Expr]) -> Result<Term, Stop> {
        let Some(Def::Function { args: formal, body }) = self.defs.get(name).cloned() else {
            return Err(Stop::NotFound(format!("{name} はまだ厳密に計算できません")));
        };
        if primes > 0 {
            // f′(a): 本体を仮引数で微分してから入れる（引数 1 つの関数だけ。文書 8 の DF0）
            let ([f0], [a0]) = (formal.as_slice(), args) else { return Err(Stop::NotFound("引数が 1 つでない関数の f′ はまだ厳密に計算できません".into())) };
            if self.depth >= 32 {
                return Err(Stop::NotFound("定義の呼び出しが深すぎます".into()));
            }
            let value = self.term(a0)?;
            let var = "#d";
            self.frames.push(Frame { vars: vec![(f0.clone(), Term::Sym(var.into()))], barrier: true });
            self.depth += 1;
            let b = self.term(&body);
            self.depth -= 1;
            self.frames.pop();
            let mut d = b?;
            for _ in 0..primes {
                d = super::diff::diff(&d, var)?;
            }
            self.analytic = true;
            return super::diff::subst_checked(&d, var, &value).map_err(|()| Stop::NotFound("0 で割っています".into()));
        }
        if formal.len() != args.len() {
            return Err(Stop::NotFound(format!("{name} の引数の数が違います")));
        }
        if self.depth >= 32 {
            return Err(Stop::NotFound("定義の呼び出しが深すぎます".into()));
        }
        let values = args.iter().map(|a| self.term(a)).collect::<Result<Vec<_>, _>>()?;
        self.frames.push(Frame { vars: formal.into_iter().zip(values).collect(), barrier: true });
        self.depth += 1;
        let r = self.term(&body);
        self.depth -= 1;
        self.frames.pop();
        r
    }

    /// 点での値 `@at(式, x, a, y, b, …)`: 変数を文字のまま式を作ってから、値を入れる（`@at(@diff(x) …, x, π/3)` の微分係数）
    fn at(&mut self, args: &[Expr]) -> Result<Term, Stop> {
        if args.len() < 3 || args.len() % 2 == 0 {
            return Err(Stop::NotFound("点での値の書き方が違います".into()));
        }
        let mut pairs = Vec::new();
        for k in (1..args.len()).step_by(2) {
            let Expr::Var(name) = &args[k] else { return Err(Stop::NotFound("点での値の変数が名前ではありません".into())) };
            pairs.push((name.clone(), self.term(&args[k + 1])?));
        }
        self.frames.push(Frame { vars: pairs.iter().map(|(n, _)| (n.clone(), Term::Sym(n.clone()))).collect(), barrier: false });
        let body = self.term(&args[0]);
        self.frames.pop();
        let body = body?;
        let mut t = body.clone();
        let mut divides_by_zero = false;
        for (n, v) in &pairs {
            match super::diff::subst_checked(&t, n, v) {
                Ok(u) => t = u,
                Err(()) => {
                    divides_by_zero = true;
                    break;
                }
            }
        }
        if divides_by_zero && pairs.len() != 1 {
            return Err(Stop::NotFound("0 で割っています".into()));
        }
        // 1 変数で、入れると 0 で割る（|x| の 0 の導関数）なら、左右の極限で決める（文書 8 の DF0）
        if (divides_by_zero || t.has_division_by_zero()) && pairs.len() == 1 {
            use super::limit::{limit, Lim, Point};
            let (n, v) = &pairs[0];
            let right = limit(&body, n, &Point::Finite(v.clone()), 1);
            let left = limit(&body, n, &Point::Finite(v.clone()), -1);
            self.analytic = true;
            return match (left, right) {
                (Ok(Lim::Value(l)), Ok(Lim::Value(r))) if l == r => Ok(l),
                (Ok(l), Ok(r)) => {
                    let show = |x: &Lim| match x {
                        Lim::Value(t) => super::latex::to_latex(t, &super::latex::Style::default()),
                        Lim::PlusInf => "+∞".into(),
                        Lim::MinusInf => "−∞".into(),
                    };
                    Err(Stop::NotExist(format!("{n} = {} で、左からの極限 {} と右からの極限 {} が違います", super::latex::to_latex(v, &super::latex::Style::default()), show(&l), show(&r))))
                }
                _ => Err(Stop::NotFound("0 で割っています".into())),
            };
        }
        Ok(t)
    }

    /// 行列の演算（文書 8 の LA0）
    fn matrix_op(&mut self, op: &str, args: &[Expr]) -> Result<Term, Stop> {
        use super::matrix::{self as m, EMat};
        let vals = args.iter().map(|a| self.term(a)).collect::<Result<Vec<_>, _>>()?;
        let mat = |t: &Term| EMat::of_term(t).ok_or_else(|| Stop::NotFound(format!("@{op} は行列かベクトルに使ってください")));
        Ok(match (op, vals.as_slice()) {
            ("det", [a]) => m::det(&mat(a)?)?,
            ("tr", [a]) => {
                let x = mat(a)?;
                if x.rows != x.cols {
                    return Err(Stop::NotFound("トレースは正方行列に使ってください".into()));
                }
                add((0..x.rows).map(|i| x.at(i, i).clone()).collect())
            }
            ("inv", [a]) => m::inverse(&mat(a)?)?.into_term(),
            ("transpose", [a]) => mat(a)?.transpose().into_term(),
            ("rank", [a]) => Term::int(m::rank(&mat(a)?) as i64),
            ("norm", [a]) => {
                let x = mat(a)?;
                pow(add(x.a.iter().map(|t| mul(vec![t.clone(), t.clone()])).collect()), Term::rat(1, 2))
            }
            ("dot", [a, b]) => {
                let (x, y) = (mat(a)?, mat(b)?);
                if x.a.len() != y.a.len() {
                    return Err(Stop::NotFound("内積の成分の数が違います".into()));
                }
                add(x.a.iter().zip(&y.a).map(|(p, q)| mul(vec![p.clone(), q.clone()])).collect())
            }
            _ => return Err(Stop::NotFound(format!("@{op} の書き方はまだ厳密に計算できません"))),
        })
    }

    /// ∇・∇·・∇×・∇²（座標 x, y, z について。今の道の `vector_operator` と同じ意味。文書 8 の DF0）
    fn vector_operator(&mut self, op: &str, arg: &Expr) -> Result<Term, Stop> {
        use super::diff::diff;
        const XYZ: [&str; 3] = ["x", "y", "z"];
        let v = self.term(arg)?;
        self.analytic = true;
        let sub = |a: Term, b: Term| add(vec![a, neg(b)]);
        Ok(match (op, &v) {
            ("grad", Term::Tuple(_)) | ("lap", Term::Tuple(_)) => return Err(Stop::NotFound("∇・∇² はスカラーの式に使ってください".into())),
            ("grad", _) => Term::Tuple(XYZ.iter().map(|c| diff(&v, c)).collect::<Result<_, _>>()?),
            ("lap", _) => add(XYZ.iter().map(|c| diff(&diff(&v, c)?, c)).collect::<Result<_, _>>()?),
            ("div", Term::Tuple(items)) if items.len() <= 3 => add(items.iter().zip(XYZ).map(|(f, c)| diff(f, c)).collect::<Result<_, _>>()?),
            ("rot" | "curl", Term::Tuple(items)) if items.len() == 2 => sub(diff(&items[1], "x")?, diff(&items[0], "y")?),
            ("rot" | "curl", Term::Tuple(items)) if items.len() == 3 => {
                let (p, q, r) = (&items[0], &items[1], &items[2]);
                Term::Tuple(vec![sub(diff(r, "y")?, diff(q, "z")?), sub(diff(p, "z")?, diff(r, "x")?), sub(diff(q, "x")?, diff(p, "y")?)])
            }
            _ => return Err(Stop::NotFound("∇·・∇× は 2 成分か 3 成分のベクトル場に使ってください".into())),
        })
    }

    /// 束縛した変数に値を入れて中身を変換する
    fn with_bound<T>(&mut self, var: &str, value: Term, f: impl FnOnce(&mut Self) -> Result<T, Stop>) -> Result<T, Stop> {
        self.frames.push(Frame { vars: vec![(var.to_string(), value)], barrier: false });
        let r = f(self);
        self.frames.pop();
        r
    }

    /// 整数になる式（Σ の範囲など）
    fn integer(&mut self, e: &Expr, what: &str) -> Result<Int, Stop> {
        let t = self.term(e)?;
        t.as_rat().and_then(num::as_int).ok_or_else(|| Stop::NotFound(format!("{what}が整数の数になりません")))
    }
}

/// 項の数が上限以下の、範囲が整数の数の Σ（無限の和・文字の範囲は文書 8 の DM1・SR1）
fn finite_sum(c: &mut Converter, var: &str, args: &[Expr], body: &Expr) -> Result<Term, Stop> {
    finite_sum_or_prod(c, var, args, body, true)
}

fn finite_prod(c: &mut Converter, var: &str, args: &[Expr], body: &Expr) -> Result<Term, Stop> {
    finite_sum_or_prod(c, var, args, body, false)
}

const MAX_FINITE_TERMS: i64 = 10_000;

fn finite_sum_or_prod(c: &mut Converter, var: &str, args: &[Expr], body: &Expr, sum: bool) -> Result<Term, Stop> {
    let [lo, hi] = args else { return Err(Stop::NotFound("Σ・Π の範囲の書き方はまだ厳密に計算できません".into())) };
    if matches!(hi, Expr::Num(v) if v.is_infinite() && *v > 0.0) {
        if !sum {
            return Err(Stop::NotFound("∞ までの積はまだ厳密に計算できません".into()));
        }
        // ∞ までの和（文書 8 の SR1）
        let a = c.integer(lo, "Σ の下端")?;
        let t = c.with_bound(var, Term::Sym(var.to_string()), |c| c.term(body))?;
        c.analytic = true;
        return super::infinite_sum::infinite_sum(&t, var, &a);
    }
    let a = c.integer(lo, "Σ・Π の下端")?;
    let b = c.integer(hi, "Σ・Π の上端")?;
    let count = &b - &a + Int::ONE;
    if count > Int::from(MAX_FINITE_TERMS) {
        return Err(Stop::NotFound("項が多すぎます（閉じた形はまだ計算できません）".into()));
    }
    let Ok(count) = i64::try_from(&count) else { return Err(Stop::NotFound("項が多すぎます".into())) };
    let mut parts = Vec::with_capacity(count.max(0) as usize);
    let mut k = a;
    for _ in 0..count.max(0) {
        parts.push(c.with_bound(var, Term::Num(Rat::from(k.clone())), |c| c.term(body))?);
        k += Int::ONE;
    }
    Ok(if sum { add(parts) } else { mul(parts) })
}

/// 積分 `@int(x) 項`（不定積分。IG0）・`@int(x, a, b) 項`（定積分。IG2）
fn integral_handler(c: &mut Converter, var: &str, args: &[Expr], body: &Expr) -> Result<Term, Stop> {
    let f = c.with_bound(var, Term::Sym(var.to_string()), |c| c.term(body))?;
    c.analytic = true;
    match args {
        [] => {
            let g = super::integrate::antiderivative(&f, var)?.ok_or_else(|| Stop::NotFound("原始関数が見つかりません".into()))?;
            c.antiderivative = Some((f, var.to_string()));
            Ok(g)
        }
        [lo, hi] => {
            let bound = |c: &mut Converter, e: &Expr| -> Result<super::limit::Point, Stop> {
                Ok(match e {
                    Expr::Num(v) if v.is_infinite() => if *v > 0.0 { super::limit::Point::PlusInf } else { super::limit::Point::MinusInf },
                    Expr::Neg(a) if matches!(&**a, Expr::Num(v) if v.is_infinite()) => super::limit::Point::MinusInf,
                    other => super::limit::Point::Finite(c.term(other)?),
                })
            };
            let (a, b) = (bound(c, lo)?, bound(c, hi)?);
            super::definite::definite(&f, var, &a, &b)
        }
        _ => Err(Stop::NotFound("積分の書き方はまだ厳密に計算できません".into())),
    }
}

/// 極限 `@lim(x, a) 項`・`@lim(x, a, ±1) 項`（片側）（文書 8 の LM1）
fn limit_handler(c: &mut Converter, var: &str, args: &[Expr], body: &Expr) -> Result<Term, Stop> {
    limit_common(c, var, args, body, false)
}

/// 数列の極限 `@seqlim(n, ∞) 項`。関数の極限があれば数列の極限も同じ（無ければ、数列では分からないので「見つからない」）
fn seqlimit_handler(c: &mut Converter, var: &str, args: &[Expr], body: &Expr) -> Result<Term, Stop> {
    limit_common(c, var, args, body, true)
}

fn limit_common(c: &mut Converter, var: &str, args: &[Expr], body: &Expr, sequence: bool) -> Result<Term, Stop> {
    use super::limit::{limit, Lim, Point};
    let (target, side) = match args {
        [t] => (t, 0),
        [t, s] => (t, if s.is_minus_one() { -1 } else { 1 }),
        _ => return Err(Stop::NotFound("極限の書き方が違います".into())),
    };
    let point = match target {
        Expr::Num(v) if v.is_infinite() => if *v > 0.0 { Point::PlusInf } else { Point::MinusInf },
        Expr::Neg(e) if matches!(&**e, Expr::Num(v) if v.is_infinite()) => Point::MinusInf,
        other => Point::Finite(c.term(other)?),
    };
    let t = c.with_bound(var, Term::Sym(var.to_string()), |c| c.term(body))?;
    c.analytic = true;
    match limit(&t, var, &point, side) {
        Ok(Lim::Value(v)) => Ok(v),
        Ok(Lim::PlusInf) => Err(Stop::NotExist("+∞ に発散します".into())),
        Ok(Lim::MinusInf) => Err(Stop::NotExist("−∞ に発散します".into())),
        Err(Stop::NotExist(r)) if sequence => Err(Stop::NotFound(format!("関数の極限は存在しません（{r}）が、数列の極限は分かりません"))),
        Err(e) => Err(e),
    }
}

/// 微分 `@diff(x) 項`・`@diff(x, k) 項`（文書 8 の DF0）
fn derivative(c: &mut Converter, var: &str, args: &[Expr], body: &Expr) -> Result<Term, Stop> {
    let k = match args {
        [] => 1,
        [k] => {
            let k = c.integer(k, "微分の回数")?;
            u32::try_from(&k).ok().filter(|k| *k <= 8).ok_or_else(|| Stop::NotFound("微分の回数は 0〜8 にしてください".into()))?
        }
        _ => return Err(Stop::NotFound("微分の書き方が違います".into())),
    };
    let t = c.with_bound(var, Term::Sym(var.to_string()), |c| c.term(body))?;
    let mut d = t;
    for _ in 0..k {
        d = super::diff::diff(&d, var)?;
    }
    c.analytic = true;
    Ok(d)
}

/// 行列・ベクトルの四則（どちらかが行列か組のとき。両方スカラーなら None）
fn linear_bin(op: BinOp, x: &Term, y: &Term) -> Result<Option<Term>, Stop> {
    use super::matrix::{self as m, EMat};
    let (mx, my) = (EMat::of_term(x), EMat::of_term(y));
    if mx.is_none() && my.is_none() {
        return Ok(None);
    }
    let r = match (op, mx, my) {
        (BinOp::Add, Some(a), Some(b)) => m::add_m(&a, &b)?,
        (BinOp::Sub, Some(a), Some(b)) => m::add_m(&a, &m::scale(&Term::int(-1), &b))?,
        (BinOp::Mul, Some(a), Some(b)) if a.cols == b.rows => m::mul_m(&a, &b)?,
        (BinOp::Mul, Some(a), None) => m::scale(y, &a),
        (BinOp::Mul, None, Some(b)) => m::scale(x, &b),
        (BinOp::Div, Some(a), None) => m::scale(&pow(y.clone(), Term::int(-1)), &a),
        (BinOp::Pow, Some(a), None) if a.rows == a.cols && a.cols > 1 => {
            let k = y.as_rat().and_then(super::num::as_int).and_then(|k| i64::try_from(&k).ok()).ok_or_else(|| Stop::NotFound("行列のべきは整数にしてください".into()))?;
            if k.abs() > 64 {
                return Err(Stop::NotFound("行列のべきが大きすぎます".into()));
            }
            let base = if k < 0 { m::inverse(&a)? } else { a.clone() };
            let mut acc = EMat::identity(a.rows);
            for _ in 0..k.unsigned_abs() {
                acc = m::mul_m(&acc, &base)?;
            }
            acc
        }
        _ => return Ok(None),
    };
    Ok(Some(r.into_term()))
}

fn short(e: &Expr) -> String {
    let s = format!("{e:?}");
    s.chars().take(40).collect()
}


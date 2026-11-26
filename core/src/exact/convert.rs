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
}

/// 記法の厳密な計算（文書 8 の各ステップが 1 つずつ足す）
type Handler = fn(&mut Converter, &str, &[Expr], &Expr) -> Result<Term, Stop>;

/// 束縛する記法（`Binder { op, … }`）の表
fn handler(op: &str) -> Option<Handler> {
    match op {
        "sum" => Some(finite_sum),
        "prod" => Some(finite_prod),
        "diff" => Some(derivative),
        _ => None,
    }
}

impl<'a> Converter<'a> {
    pub fn new(defs: &'a Defs, params: &'a Params) -> Self {
        Converter { defs, params, frames: Vec::new(), depth: 0, decimal: false, analytic: false, uses_ln: false }
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
            return Ok(super::diff::subst(&d, var, &value));
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
        let mut t = body?;
        for (n, v) in &pairs {
            t = super::diff::subst(&t, n, v);
        }
        Ok(t)
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
    if matches!(hi, Expr::Num(v) if v.is_infinite()) {
        return Err(Stop::NotFound("∞ までの和・積はまだ厳密に計算できません".into()));
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

fn short(e: &Expr) -> String {
    let s = format!("{e:?}");
    s.chars().take(40).collect()
}


//! 数式の文字列を構文木 (AST) に変換する。
//!
//! Desmos のように書きやすくするため、次の書き方を許す:
//! - 暗黙の掛け算: `2x`, `xy`, `2sin(x)`, `(x+1)(x-1)`
//! - 括弧なしの関数呼び出し: `sin x`, `sin^2 x`  (= (sin x)^2)
//! - 定数: `pi` / `π`, `e`, `i`（虚数単位）
//!
//! 数学の記法（計画 `docs/plan-math-notation.md` の 2.2）のための書き方も読む。どれも今までの式では使えなかった文字なので、
//! 今までの式の読み方は変わらない:
//! - `@名前`: 新しい演算子（`@sum(n, 1, 10) 項` のように束縛する変数を持つもの、`@det(A)` のように関数の形のもの）
//! - `{名前}`: 2 文字以上の名前の変数 1 つ（`{alpha}`・`{vec_v}`）
//! - `名前'`: プライム。定義された関数の名前なら導関数の呼び出し `f'(x)`、それ以外は `x'`（常微分方程式）
//! - `名前_(式)`: 数字でない添字（数列の項 `a_(n+1)`）。数字だけの添字 `w_2` は今までどおり変数 `w2`
//! - `⟦a, b; c, d⟧`: 行列（`,` で列、`;` で行を区切る）
//! - `×`: 外積・直積。スカラーどうしなら掛け算（展開の段 `expand.rs` で決める）
//!
//! 定義された関数の名前（[`parse_with`] の `names`）の直後に `(` があれば、掛け算ではなく呼び出し [`Expr::Apply`] にする。
//! 新しい節は、展開の段（`expand.rs`）で今までの節だけの式に直してから、命令列にする

use crate::functions::Func;
use std::fmt;

/// 構文木のノード
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    /// 名前付き定数 (pi, e, i)
    Const(Constant),
    Var(String),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
    /// 組 (a, b, c)。媒介変数表示の点を表す
    Tuple(Vec<Expr>),
    /// リスト [a, b, …]。媒介変数表示の中で使うと、要素ごとに図形をまとめて描く
    List(Vec<Expr>),
    // ---- ここから下は、展開の段（`expand.rs`）で消える節。命令列にはならない ----
    /// 定義された関数の呼び出し `f(a, b)`。`primes` はプライムの数（`f''(x)` なら 2）
    Apply { name: String, primes: u8, args: Vec<Expr> },
    /// 変数を束縛する演算子 `@sum(n, 1, 10) 項`。`args` は束縛する変数の後の引数（範囲など）、`body` は中身
    Binder { op: String, var: String, args: Vec<Expr>, body: Box<Expr> },
    /// 束縛する変数の無い演算子 `@det(A)`・`@grad 項`、外積 `a × b`（`op` は "cross"）
    Op { op: String, args: Vec<Expr> },
    /// 行列 `⟦a, b; c, d⟧`。`items` は行の順に並べた成分（rows × cols 個）
    Matrix { rows: usize, cols: usize, items: Vec<Expr> },
    /// 数字でない添字 `a_(n+1)`
    Index { name: String, index: Box<Expr> },
    /// 呼び出しでないプライム `x'`・`y''`
    Prime { name: String, order: u8 },
    /// 条件（論理式）。集合の上の総和 `@sumset(n; 条件) 項` の条件を持つ
    Cond(Box<Formula>),
}

impl Expr {
    pub(crate) fn is_minus_one(&self) -> bool {
        matches!(self, Expr::Num(n) if *n == -1.0)
            || matches!(self, Expr::Neg(x) if matches!(**x, Expr::Num(n) if n == 1.0))
    }

    /// 式の中で使われている変数名を集める（重複なし、出てきた順）
    pub fn collect_vars(&self, out: &mut Vec<String>) {
        match self {
            Expr::Var(v) => {
                if !out.contains(v) {
                    out.push(v.clone());
                }
            }
            Expr::Num(_) | Expr::Const(_) => {}
            Expr::Neg(a) => a.collect_vars(out),
            Expr::Bin(_, a, b) => {
                a.collect_vars(out);
                b.collect_vars(out);
            }
            Expr::Call(_, args) | Expr::Tuple(args) | Expr::List(args) => {
                args.iter().for_each(|a| a.collect_vars(out))
            }
            Expr::Apply { args, .. } | Expr::Op { args, .. } => args.iter().for_each(|a| a.collect_vars(out)),
            Expr::Matrix { items, .. } => items.iter().for_each(|a| a.collect_vars(out)),
            Expr::Index { index, .. } => index.collect_vars(out),
            // x' は x の導関数で、x そのものではないので、変数として数えない
            Expr::Prime { .. } => {}
            Expr::Cond(f) => f.collect_vars(out),
            Expr::Binder { op, args, body, .. } if !binds(op) => {
                args.iter().for_each(|a| a.collect_vars(out));
                body.collect_vars(out);
            }
            Expr::Binder { var, args, body, .. } => {
                args.iter().for_each(|a| a.collect_vars(out));
                // 束縛する変数は、中身の中では外の同じ名前を隠す（外へは出さない）
                let mut inner = Vec::new();
                body.collect_vars(&mut inner);
                for v in inner {
                    if v != *var && !out.contains(&v) {
                        out.push(v);
                    }
                }
            }
        }
    }

    /// 変数を置き換えた式を作る。`f` が Some を返した変数は、その式に置き換える
    pub fn substitute(&self, f: &dyn Fn(&str) -> Option<Expr>) -> Expr {
        match self {
            Expr::Var(v) => f(v).unwrap_or_else(|| self.clone()),
            Expr::Num(_) | Expr::Const(_) => self.clone(),
            Expr::Neg(a) => Expr::Neg(Box::new(a.substitute(f))),
            Expr::Bin(o, a, b) => Expr::Bin(*o, Box::new(a.substitute(f)), Box::new(b.substitute(f))),
            Expr::Call(g, args) => Expr::Call(*g, args.iter().map(|a| a.substitute(f)).collect()),
            Expr::Tuple(args) => Expr::Tuple(args.iter().map(|a| a.substitute(f)).collect()),
            Expr::List(args) => Expr::List(args.iter().map(|a| a.substitute(f)).collect()),
            Expr::Apply { name, primes, args } => {
                Expr::Apply { name: name.clone(), primes: *primes, args: args.iter().map(|a| a.substitute(f)).collect() }
            }
            Expr::Op { op, args } => Expr::Op { op: op.clone(), args: args.iter().map(|a| a.substitute(f)).collect() },
            Expr::Matrix { rows, cols, items } => {
                Expr::Matrix { rows: *rows, cols: *cols, items: items.iter().map(|a| a.substitute(f)).collect() }
            }
            Expr::Index { name, index } => Expr::Index { name: name.clone(), index: Box::new(index.substitute(f)) },
            Expr::Prime { .. } => self.clone(),
            Expr::Cond(c) => Expr::Cond(Box::new(crate::logic::substitute(c, f))),
            // 微分・差分の変数は束縛しない（結果はその変数の式）。名前の置き換え（変数 → 変数）なら一緒に変える
            Expr::Binder { op, var, args, body } if !binds(op) && !matches!(f(var), Some(Expr::Var(_)) | None) => {
                let inner = |v: &str| if v == var { None } else { f(v) };
                Expr::Binder {
                    op: op.clone(),
                    var: var.clone(),
                    args: args.iter().map(|a| a.substitute(f)).collect(),
                    body: Box::new(body.substitute(&inner)),
                }
            }
            Expr::Binder { op, var, args, body } if !binds(op) => {
                let var = match f(var) {
                    Some(Expr::Var(new)) => new,
                    _ => var.clone(),
                };
                Expr::Binder { op: op.clone(), var, args: args.iter().map(|a| a.substitute(f)).collect(), body: Box::new(body.substitute(f)) }
            }
            Expr::Binder { op, var, args, body } => {
                // 中身の中の束縛する変数は置き換えない（外の同じ名前を隠している）
                let inner = |v: &str| if v == var { None } else { f(v) };
                Expr::Binder {
                    op: op.clone(),
                    var: var.clone(),
                    args: args.iter().map(|a| a.substitute(f)).collect(),
                    body: Box::new(body.substitute(&inner)),
                }
            }
        }
    }

    /// 節を置き換えた式を作る。`f` が Some を返した節は、その式に置き換える（中は見ない）。
    /// 束縛する変数は気にしない（プライム `x'` を変数に置き換えるのに使う）
    pub fn rewrite(&self, f: &dyn Fn(&Expr) -> Option<Expr>) -> Expr {
        if let Some(e) = f(self) {
            return e;
        }
        let all = |v: &[Expr]| v.iter().map(|a| a.rewrite(f)).collect::<Vec<_>>();
        match self {
            Expr::Num(_) | Expr::Const(_) | Expr::Var(_) | Expr::Prime { .. } | Expr::Cond(_) => self.clone(),
            Expr::Neg(a) => Expr::Neg(Box::new(a.rewrite(f))),
            Expr::Bin(o, a, b) => Expr::Bin(*o, Box::new(a.rewrite(f)), Box::new(b.rewrite(f))),
            Expr::Call(g, args) => Expr::Call(*g, all(args)),
            Expr::Tuple(args) => Expr::Tuple(all(args)),
            Expr::List(args) => Expr::List(all(args)),
            Expr::Apply { name, primes, args } => Expr::Apply { name: name.clone(), primes: *primes, args: all(args) },
            Expr::Op { op, args } => Expr::Op { op: op.clone(), args: all(args) },
            Expr::Matrix { rows, cols, items } => Expr::Matrix { rows: *rows, cols: *cols, items: all(items) },
            Expr::Index { name, index } => Expr::Index { name: name.clone(), index: Box::new(index.rewrite(f)) },
            Expr::Binder { op, var, args, body } => {
                Expr::Binder { op: op.clone(), var: var.clone(), args: all(args), body: Box::new(body.rewrite(f)) }
            }
        }
    }

    /// 式の中のプライム `x'`・`y''` の (名前, 階数)
    pub fn primes(&self) -> Vec<(String, u8)> {
        let out = std::cell::RefCell::new(Vec::new());
        self.rewrite(&|e| {
            if let Expr::Prime { name, order } = e {
                out.borrow_mut().push((name.clone(), *order));
            }
            None
        });
        out.into_inner()
    }

    /// 式の中で使われている変数名か調べる
    pub fn uses_var(&self, name: &str) -> bool {
        match self {
            Expr::Var(v) => v == name,
            Expr::Num(_) | Expr::Const(_) => false,
            Expr::Neg(a) => a.uses_var(name),
            Expr::Bin(_, a, b) => a.uses_var(name) || b.uses_var(name),
            Expr::Call(_, args) | Expr::Tuple(args) | Expr::List(args) => args.iter().any(|a| a.uses_var(name)),
            Expr::Apply { args, .. } | Expr::Op { args, .. } => args.iter().any(|a| a.uses_var(name)),
            Expr::Matrix { items, .. } => items.iter().any(|a| a.uses_var(name)),
            Expr::Index { index, .. } => index.uses_var(name),
            Expr::Prime { .. } => false,
            Expr::Cond(f) => {
                let mut vs = Vec::new();
                f.collect_vars(&mut vs);
                vs.iter().any(|v| v == name)
            }
            Expr::Binder { op, var, args, body } if !binds(op) => var == name || args.iter().any(|a| a.uses_var(name)) || body.uses_var(name),
            Expr::Binder { var, args, body, .. } => args.iter().any(|a| a.uses_var(name)) || (var != name && body.uses_var(name)),
        }
    }

    /// 展開の段（`expand.rs`）で消える節（[`Expr::Apply`] など）を含むか
    pub fn has_notation(&self) -> bool {
        match self {
            Expr::Num(_) | Expr::Const(_) | Expr::Var(_) => false,
            Expr::Neg(a) => a.has_notation(),
            Expr::Bin(_, a, b) => a.has_notation() || b.has_notation(),
            Expr::Call(_, args) | Expr::Tuple(args) | Expr::List(args) => args.iter().any(Expr::has_notation),
            Expr::Apply { .. } | Expr::Binder { .. } | Expr::Op { .. } | Expr::Matrix { .. } | Expr::Index { .. } | Expr::Prime { .. } | Expr::Cond(_) => true,
        }
    }

    /// 節の数（展開した式の大きさの上限を調べるのに使う）
    pub fn node_count(&self) -> usize {
        1 + match self {
            Expr::Num(_) | Expr::Const(_) | Expr::Var(_) | Expr::Prime { .. } | Expr::Cond(_) => 0,
            Expr::Neg(a) => a.node_count(),
            Expr::Bin(_, a, b) => a.node_count() + b.node_count(),
            Expr::Call(_, args) | Expr::Tuple(args) | Expr::List(args) | Expr::Apply { args, .. } | Expr::Op { args, .. } => {
                args.iter().map(Expr::node_count).sum()
            }
            Expr::Matrix { items, .. } => items.iter().map(Expr::node_count).sum(),
            Expr::Index { index, .. } => index.node_count(),
            Expr::Binder { args, body, .. } => args.iter().map(Expr::node_count).sum::<usize>() + body.node_count(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constant {
    Pi,
    E,
    I,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// 解析エラー（`pos` は文字単位の位置）
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub pos: usize,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}（{}文字目）", self.message, self.pos + 1)
    }
}

fn err<T>(message: impl Into<String>, pos: usize) -> Result<T, ParseError> {
    Err(ParseError {
        message: message.into(),
        pos,
    })
}

// ---------------------------------------------------------------------------
// 字句解析（文字列 → トークン列）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Var(String),
    Const(Constant),
    Func(Func),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Bang,
    LParen,
    LBracket,
    RBracket,
    RParen,
    Comma,
    /// 関係（`=`・不等号）。括弧の外でだけ使える（[`parse_system`]）
    Rel(Rel),
    /// `@名前`（新しい演算子）
    Op(String),
    /// `'`（プライム）
    Prime,
    /// `_(`（数字でない添字の始まり。閉じ括弧は ')'）
    Sub,
    /// `;`（行列の行の区切り）
    Semi,
    /// `⟦`・`⟧`（行列の括弧）
    LMat,
    RMat,
    /// `×`（外積・直積。スカラーどうしなら掛け算）
    Cross,
    /// `∘`（関数の合成）
    Compose,
    /// `%%`（a mod b。掛け算と同じ強さ。LaTeX の `\bmod`）
    Mod,
    /// `…`（リストの中の、等差の並びの省略）
    Ellipsis,
    /// 論理・集合の記号（∧ ∨ ¬ ⇒ ⇐ ⇔ ⊕ ⊼ ⊽ ∩ ∪ ∖ △ ∈ ∉ ∋ ≠ ⊤ ⊥ ∅ ℝ ℤ ℕ ℂ ℚ ⦃ ∣ ⦄ ᶜ ∁。計画 M11）
    Sym(char),
    /// `±`・`∓`（両方の符号の図形）
    PlusMinus,
    MinusPlus,
    Eof,
}

/// `@名前` の演算子の読み方
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    /// `@sum(n, 1, 10) 項`: 括弧の中の最初が束縛する変数、残りが引数。中身は直後の項
    Binder,
    /// `@grad 項`: 括弧を付けずに、直後の項に掛かる
    Prefix,
    /// `@det(A)`: ふつうの関数と同じ形
    Func,
}

/// `@名前` の演算子が、変数を本当に束縛するか（Σ・Π・積分は束縛する。微分・差分は、結果がその変数の式なので束縛しない）
pub fn binds(op: &str) -> bool {
    !matches!(op, "diff" | "fdiff" | "bdiff" | "cdiff")
}

/// `@名前` の演算子の一覧（名前 → 読み方）。ステップごとに足す（計画 `docs/plan-math-notation.md`）
pub fn op_kind(name: &str) -> Option<OpKind> {
    Some(match name {
        // fdiff・bdiff・cdiff: 前進・後退・中心差分（総和の逆）
        // max・min・sup・inf: 添字つきの最大・最小 `@max(t, a, b) 項`（等分点での近似。計画 M12）
        "sum" | "prod" | "int" | "lim" | "diff" | "fdiff" | "bdiff" | "cdiff" | "max" | "min" | "sup" | "inf" => OpKind::Binder,
        // ∃・∀（行の先頭だけ。equation.rs が行ごと読む。計画 M12）
        "exists" | "forall" => OpKind::Func,
        "cases" | "intset" | "sumset" | "prodset" | "bigcup" | "bigcap" | "bigvee" | "bigwedge" => OpKind::Func,
        // ベクトルと行列（計画 M6・M7）
        "det" | "tr" | "inv" | "transpose" | "norm" | "dot" | "cross" | "angle" | "ker" | "im" | "dim" | "rank" | "eig" => OpKind::Func,
        // テイラー多項式 @taylor(f, x, a, n)
        "taylor" => OpKind::Func,
        // 停留点 @crit f（1 行にそれだけ。equation.rs が行ごと読む）
        "crit" | "cobweb" | "orbit" | "orbitpoints" | "recurrence" | "bifurcation" => OpKind::Func,
        // 点での値 @at(式, x, a, y, b, …)
        "at" => OpKind::Func,
        // ベクトル場の演算子: 勾配・発散・回転・ラプラシアン（直後の項に掛かる）
        "grad" | "div" | "rot" | "curl" | "lap" => OpKind::Prefix,
        _ => return None,
    })
}

/// 関係演算子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rel {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

/// 範囲を共有する略記を、変数ごとの行に分ける。カンマで区切った行のうち、関係の無い「変数 1 つだけ」の行
/// （今までは「行ごとに = か不等号を付けてください」のエラー）を、前の行の最後の変数・次の行の最初の変数と組にする:
///
/// - `−1 ≤ x, y, z ≤ 1` → `−1 ≤ x ≤ 1`, `−1 ≤ y ≤ 1`, `−1 ≤ z ≤ 1`
/// - `0 ≤ x, y, z` → `0 ≤ x`, `0 ≤ y`, `0 ≤ z`、`x, y ≥ 0` → `x ≥ 0`, `y ≥ 0`
///
/// 変数だけの行が無い `0 ≤ x, y ≤ 1` は、今までどおり 2 つの条件（x ≥ 0 と y ≤ 1）。
/// 集合の条件 `{(x, y, z) | −1 ≤ x, y, z ≤ 1}` でも同じ（[`Parser::formula`]）
pub fn expand_shared_bounds(chains: Vec<Chain>) -> Vec<Chain> {
    let bare = |c: &Chain| c.rels.is_empty() && matches!(c.exprs.as_slice(), [Expr::Var(_)]);
    if !chains.iter().any(bare) {
        return chains;
    }
    let mut out: Vec<Chain> = Vec::new();
    let mut i = 0;
    while i < chains.len() {
        if !bare(&chains[i]) {
            out.push(chains[i].clone());
            i += 1;
            continue;
        }
        // 変数だけの行の並び [i, j)
        let mut j = i;
        while j < chains.len() && bare(&chains[j]) {
            j += 1;
        }
        let mut vars: Vec<(Expr, usize)> = chains[i..j].iter().map(|c| (c.exprs[0].clone(), c.pos[0])).collect();
        // 前の行（関係があり、最後が変数）を、範囲の左側として使う
        let left = out.last().filter(|c| !c.rels.is_empty() && matches!(c.exprs.last(), Some(Expr::Var(_)))).cloned();
        if let Some(l) = &left {
            out.pop();
            vars.insert(0, (l.exprs.last().cloned().unwrap_or(Expr::Num(0.0)), *l.pos.last().unwrap_or(&0)));
        }
        // 次の行（関係があり、最初が変数）を、範囲の右側として使う
        let right = chains.get(j).filter(|c| !c.rels.is_empty() && matches!(c.exprs.first(), Some(Expr::Var(_)))).cloned();
        if let Some(r) = &right {
            vars.push((r.exprs[0].clone(), r.pos[0]));
            j += 1;
        }
        if left.is_none() && right.is_none() {
            // 範囲が無い: そのまま（後でエラーになる）
            out.extend(chains[i..j].iter().cloned());
            i = j;
            continue;
        }
        for (v, at) in vars {
            let mut c = Chain { exprs: Vec::new(), rels: Vec::new(), pos: Vec::new() };
            if let Some(l) = &left {
                let n = l.exprs.len() - 1;
                c.exprs.extend(l.exprs[..n].iter().cloned());
                c.pos.extend(l.pos[..n].iter().copied());
                c.rels.extend(l.rels.iter().copied());
            }
            c.exprs.push(v);
            c.pos.push(at);
            if let Some(r) = &right {
                c.exprs.extend(r.exprs[1..].iter().cloned());
                c.pos.extend(r.pos[1..].iter().copied());
                c.rels.extend(r.rels.iter().copied());
            }
            out.push(c);
        }
        i = j;
    }
    out
}


/// 条件の並び（`,` で区切ったもの）の中の、範囲を共有する略記 `−1 ≤ x, y, z ≤ 1` を変数ごとの条件に分ける
/// （[`expand_shared_bounds`]）。変数 1 つだけの条件は集合の名前（`Formula::Set(SetE::Named)`）として読まれているので、
/// 隣に範囲（変数で終わる・始まる関係の連鎖）があるときだけ、その変数の範囲にする
fn shared_bounds_in_formulas(parts: Vec<Formula>) -> Vec<Formula> {
    let bare = |f: &Formula| matches!(f, Formula::Set(SetE::Named(_)));
    if !parts.iter().any(bare) || !parts.iter().any(|f| matches!(f, Formula::Chain(_))) {
        return parts;
    }
    let mut out = Vec::new();
    let mut run: Vec<Chain> = Vec::new();
    let flush = |run: &mut Vec<Chain>, out: &mut Vec<Formula>| {
        for c in expand_shared_bounds(std::mem::take(run)) {
            out.push(match (c.rels.is_empty(), c.exprs.as_slice()) {
                (true, [Expr::Var(n)]) => Formula::Set(SetE::Named(n.clone())),
                _ => Formula::Chain(c),
            });
        }
    };
    for f in parts {
        match f {
            Formula::Chain(c) => run.push(c),
            Formula::Set(SetE::Named(n)) => run.push(Chain { exprs: vec![Expr::Var(n)], rels: Vec::new(), pos: vec![0] }),
            other => {
                flush(&mut run, &mut out);
                out.push(other);
            }
        }
    }
    flush(&mut run, &mut out);
    out
}

/// 関係の連鎖 `a ∘₁ b ∘₂ c …`（`exprs` は `rels` より 1 つ多い。関係が無ければ式 1 つ）
#[derive(Debug, Clone, PartialEq)]
pub struct Chain {
    pub exprs: Vec<Expr>,
    pub rels: Vec<Rel>,
    /// 式ごとの、始まりの位置（文字単位）
    pub pos: Vec<usize>,
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    pos: usize,
}

/// 識別子として認識する名前（関数と定数）。長いものを優先して照合する。
fn named_tokens() -> Vec<(&'static str, Tok)> {
    let mut v: Vec<(&'static str, Tok)> = Func::all()
        .flat_map(|f| f.names().iter().map(move |n| (*n, Tok::Func(f))))
        .collect();
    v.push(("pi", Tok::Const(Constant::Pi)));
    v.push(("π", Tok::Const(Constant::Pi)));
    v.push(("e", Tok::Const(Constant::E)));
    v.push(("i", Tok::Const(Constant::I)));
    // 座標の名前に使うギリシャ文字（計画 9 章の S38）: 1 つの変数として読む（"theta" を t·h·e·t·a にしない）
    for (names, var) in [
        (&["theta", "θ", "ϑ"][..], "theta"),
        (&["phi", "φ", "ϕ"][..], "phi"),
        (&["rho", "ρ"][..], "rho"),
        (&["sigma", "σ"][..], "sigma"),
        (&["tau", "τ"][..], "tau"),
        (&["mu", "μ"][..], "mu"),
        (&["nu", "ν"][..], "nu"),
    ] {
        for n in names {
            v.push((n, Tok::Var(var.into())));
        }
    }
    // 長い名前から順に試す（"sinh" を "sin" + "h" と誤認しないため）
    v.sort_by_key(|t| std::cmp::Reverse(t.0.chars().count()));
    v
}

fn tokenize(src: &str) -> Result<Vec<Token>, ParseError> {
    let chars: Vec<char> = src.chars().collect();
    let names = named_tokens();
    let mut out = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;

        // 数値
        if c.is_ascii_digit() || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            let n: f64 = match s.parse() {
                Ok(n) => n,
                Err(_) => return err(format!("数値 \"{s}\" を読めません"), start),
            };
            out.push(Token { tok: Tok::Num(n), pos: start });
            continue;
        }

        // 英字の並び: 関数名・定数を最長一致で取り出し、残りは1文字ずつ変数にする
        // 例) "xsinx" → x, sin, x
        // ℝ ℤ ℕ ℂ ℚ ᶜ は文字に分類されるが、名前ではなく集合の記号
        let letter = |c: char| c.is_alphabetic() && !"ℝℤℕℂℚᶜᵀ".contains(c);
        if letter(c) {
            while i < chars.len() && letter(chars[i]) {
                // 数字を含む名前（hyp2F1 など）は英字の並びの外まで見て探す
                let full: String = chars[i..].iter().collect();
                if let Some((name, tok)) =
                    names.iter().find(|(n, _)| n.chars().any(|d| d.is_ascii_digit()) && full.starts_with(n))
                {
                    out.push(Token { tok: tok.clone(), pos: i });
                    i += name.chars().count();
                    continue;
                }
                let mut end = i;
                while end < chars.len() && letter(chars[end]) {
                    end += 1;
                }
                let rest: String = chars[i..end].iter().collect();
                if let Some((name, tok)) = names.iter().find(|(n, _)| rest.starts_with(n)) {
                    out.push(Token { tok: tok.clone(), pos: i });
                    i += name.chars().count();
                } else {
                    out.push(Token { tok: Tok::Var(chars[i].to_string()), pos: i });
                    i += 1;
                }
            }
            // 数字でない添字: "a_(n+1)"（直前が変数のときだけ）
            if chars.get(i) == Some(&'_') && chars.get(i + 1) == Some(&'(') && matches!(out.last(), Some(Token { tok: Tok::Var(_), .. })) {
                out.push(Token { tok: Tok::Sub, pos: i });
                out.push(Token { tok: Tok::LParen, pos: i + 1 });
                i += 2;
                continue;
            }
            // 添字: "w_2" は1つの変数 "w2" にする（直前が1文字の変数のときだけ）
            if chars.get(i) == Some(&'_') {
                let us = i;
                i += 1;
                let ds = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let digits: String = chars[ds..i].iter().collect();
                match out.last_mut() {
                    Some(Token { tok: Tok::Var(v), .. }) if !digits.is_empty() && v.chars().count() == 1 => {
                        // w₁ は w と同じ座標（4次元目）
                        if !(v == "w" && digits == "1") {
                            v.push_str(&digits);
                        }
                    }
                    _ => return err("_ の後には数字を書いてください（例: w_2）", us),
                }
            }
            continue;
        }

        // `@名前`（新しい演算子）。名前は英字の並び全部
        if c == '@' {
            let mut end = i + 1;
            while end < chars.len() && chars[end].is_ascii_alphabetic() {
                end += 1;
            }
            if end == i + 1 {
                return err("@ の後には演算子の名前を書いてください", i);
            }
            let name: String = chars[i + 1..end].iter().collect();
            if op_kind(&name).is_none() {
                return err(format!("@{name} という演算子はありません"), i);
            }
            out.push(Token { tok: Tok::Op(name), pos: i });
            i = end;
            continue;
        }
        // `{名前}`: 2 文字以上の名前の変数 1 つ（英字・数字・_ だけ）
        if c == '{' {
            let mut end = i + 1;
            while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            if chars.get(end) != Some(&'}') || end == i + 1 {
                return err("{ } の中には名前（英字・数字・_）だけを書いてください", i);
            }
            let name: String = chars[i + 1..end].iter().collect();
            out.push(Token { tok: Tok::Var(name), pos: i });
            i = end + 1;
            // 添字: `{vec_b}_(k)`・`{vec_b}_2`（成分の取り出し。数字だけでも、名前には足さない）
            if chars.get(i) == Some(&'_') {
                let mut d = i + 1;
                while d < chars.len() && chars[d].is_ascii_digit() {
                    d += 1;
                }
                if chars.get(i + 1) == Some(&'(') {
                    out.push(Token { tok: Tok::Sub, pos: i });
                    i += 1;
                } else if d > i + 1 {
                    let digits: String = chars[i + 1..d].iter().collect();
                    out.push(Token { tok: Tok::Sub, pos: i });
                    out.push(Token { tok: Tok::LParen, pos: i });
                    out.push(Token { tok: Tok::Num(digits.parse().unwrap_or(0.0)), pos: i + 1 });
                    out.push(Token { tok: Tok::RParen, pos: d });
                    i = d;
                }
            }
            continue;
        }

        let tok = match c {
            '+' => Tok::Plus,
            '-' | '−' => Tok::Minus,
            '*' | '·' => Tok::Star,
            '×' => Tok::Cross,
            '∘' => Tok::Compose,
            '…' => Tok::Ellipsis,
            '∧' | '∨' | '¬' | '⇒' | '⇐' | '⇔' | '⊕' | '⊼' | '⊽' | '∩' | '∪' | '∖' | '△' | '∈' | '∉' | '∋' | '≠' | '⊤' | '⊥' | '∅' | 'ℝ'
            | 'ℤ' | 'ℕ' | 'ℂ' | 'ℚ' | '⦃' | '∣' | '⦄' | 'ᶜ' | '∁' => Tok::Sym(c),
            // 無限大（積分の端など）
            '∞' => Tok::Num(f64::INFINITY),
            '±' => Tok::PlusMinus,
            '∓' => Tok::MinusPlus,
            '.' if chars.get(i + 1) == Some(&'.') && chars.get(i + 2) == Some(&'.') => {
                out.push(Token { tok: Tok::Ellipsis, pos: i });
                i += 3;
                continue;
            }
            '\'' | '′' => Tok::Prime,
            '%' if chars.get(i + 1) == Some(&'%') => {
                out.push(Token { tok: Tok::Mod, pos: i });
                i += 2;
                continue;
            }
            // 共役 z^∗（LaTeX の `^{\ast}`。展開の段で conj にする）
            '∗' => Tok::Var("∗".into()),
            // 転置 Aᵀ は A^T と同じ。† は ^† の形で使う（共役転置）
            'ᵀ' => {
                out.push(Token { tok: Tok::Caret, pos: i });
                Tok::Var("T".into())
            }
            '†' => Tok::Var(c.to_string()),
            ';' => Tok::Semi,
            '⟦' => Tok::LMat,
            '⟧' => Tok::RMat,
            '/' | '÷' => Tok::Slash,
            '^' => Tok::Caret,
            '!' => Tok::Bang,
            '(' => Tok::LParen,
            '[' => Tok::LBracket,
            ']' => Tok::RBracket,
            ')' => Tok::RParen,
            ',' => Tok::Comma,
            '=' | '＝' => Tok::Rel(Rel::Eq),
            '≤' | '≦' => Tok::Rel(Rel::Le),
            '≥' | '≧' => Tok::Rel(Rel::Ge),
            '<' | '＜' | '>' | '＞' => {
                let less = matches!(c, '<' | '＜');
                if matches!(chars.get(i + 1), Some('=' | '＝')) {
                    out.push(Token { tok: Tok::Rel(if less { Rel::Le } else { Rel::Ge }), pos: i });
                    i += 2;
                    continue;
                }
                Tok::Rel(if less { Rel::Lt } else { Rel::Gt })
            }
            _ => return err(format!("使えない文字 '{c}' があります"), i),
        };
        out.push(Token { tok, pos: i });
        i += 1;
    }
    out.push(Token { tok: Tok::Eof, pos: chars.len() });
    Ok(out)
}

// ---------------------------------------------------------------------------
// 構文解析（トークン列 → 構文木）
//
// 優先順位（弱い順）:
//   expr  := term (('+' | '-') term)*
//   term  := unary (('*' | '/' | 暗黙の掛け算) unary)*
//   unary := ('-' | '+') unary | power
//   power := primary '!'* ('^' unary)?     ← 右結合: 2^3^2 = 2^(3^2)、x! は階乗
//   primary := 数値 | 変数 | 定数 | 関数呼び出し | '(' expr ')'
// ---------------------------------------------------------------------------

struct Parser<'a> {
    toks: Vec<Token>,
    i: usize,
    /// 定義された関数の名前（直後の `(` を呼び出しとして読む）
    names: &'a [String],
}

impl Parser<'_> {
    fn peek(&self) -> &Tok {
        &self.toks[self.i].tok
    }
    fn pos(&self) -> usize {
        self.toks[self.i].pos
    }
    fn next(&mut self) -> Token {
        let t = self.toks[self.i].clone();
        if self.i < self.toks.len() - 1 {
            self.i += 1;
        }
        t
    }
    fn expect(&mut self, tok: Tok, what: &str) -> Result<(), ParseError> {
        if *self.peek() == tok {
            self.next();
            Ok(())
        } else if matches!(self.peek(), Tok::Rel(_)) {
            err("=・≤ などは、括弧の外に書いてください", self.pos())
        } else {
            err(format!("{what} が必要です"), self.pos())
        }
    }

    fn expr(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.term()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                Tok::PlusMinus | Tok::MinusPlus => {
                    let op = if *self.peek() == Tok::PlusMinus { "pm" } else { "mp" };
                    self.next();
                    let rhs = self.term()?;
                    lhs = Expr::Op { op: op.into(), args: vec![lhs, rhs] };
                    continue;
                }
                _ => return Ok(lhs),
            };
            self.next();
            let rhs = self.term()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
    }

    fn term(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => {
                    self.next();
                    BinOp::Mul
                }
                Tok::Slash => {
                    self.next();
                    BinOp::Div
                }
                Tok::Mod => {
                    self.next();
                    let rhs = self.unary()?;
                    lhs = Expr::Call(Func::Mod, vec![lhs, rhs]);
                    continue;
                }
                Tok::Cross | Tok::Compose => {
                    let op = if *self.peek() == Tok::Cross { "cross" } else { "compose" };
                    self.next();
                    let rhs = self.unary()?;
                    lhs = Expr::Op { op: op.into(), args: vec![lhs, rhs] };
                    continue;
                }
                // 次に値が始まるなら暗黙の掛け算
                Tok::Num(_) | Tok::Var(_) | Tok::Const(_) | Tok::Func(_) | Tok::LParen | Tok::LBracket | Tok::Op(_) | Tok::LMat => {
                    BinOp::Mul
                }
                _ => return Ok(lhs),
            };
            let rhs = self.unary()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Tok::Minus => {
                self.next();
                Ok(Expr::Neg(Box::new(self.unary()?)))
            }
            Tok::Plus => {
                self.next();
                self.unary()
            }
            Tok::PlusMinus | Tok::MinusPlus => {
                let op = if *self.peek() == Tok::PlusMinus { "pm" } else { "mp" };
                self.next();
                Ok(Expr::Op { op: op.into(), args: vec![self.unary()?] })
            }
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<Expr, ParseError> {
        let base = self.primary()?;
        self.after_primary(base)
    }

    fn after_primary(&mut self, mut base: Expr) -> Result<Expr, ParseError> {
        // 階乗 x! = Γ(x + 1)。累乗より先に計算する（3!^2 = (3!)^2）
        while *self.peek() == Tok::Bang {
            self.next();
            let arg = Expr::Bin(BinOp::Add, Box::new(base), Box::new(Expr::Num(1.0)));
            base = Expr::Call(Func::Gamma, vec![arg]);
        }
        if *self.peek() == Tok::Caret {
            self.next();
            let exp = self.unary()?;
            // f^2(x) は呼び出し結果のべき。f^(-1)(x) は既存の逆関数の記法を保つ。
            if *self.peek() == Tok::LParen {
                let callable = match &base {
                    Expr::Var(name) if !exp.is_minus_one() => Some((name, 0)),
                    Expr::Prime { name, order } => Some((name, *order)),
                    _ => None,
                };
                if let Some((name, primes)) = callable.filter(|(name, _)| self.names.contains(name)) {
                    self.next();
                    let args = self.args_until(Tok::RParen, "閉じ括弧 ')'")?;
                    let call = Expr::Apply { name: name.clone(), primes, args };
                    return self.after_primary(Expr::Bin(BinOp::Pow, Box::new(call), Box::new(exp)));
                }
            }
            return Ok(Expr::Bin(BinOp::Pow, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let t = self.next();
        match t.tok {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Var(v) => self.after_name(v),
            Tok::Op(name) => self.op(name, t.pos),
            Tok::LMat => self.matrix(t.pos),
            Tok::Const(c) => Ok(Expr::Const(c)),
            Tok::LBracket => {
                // 要素の代わりに … を書ける（等差の並びの省略。展開の段で並べる）
                let item = |p: &mut Self| -> Result<Expr, ParseError> {
                    if *p.peek() == Tok::Ellipsis {
                        p.next();
                        Ok(Expr::Op { op: "ellipsis".into(), args: Vec::new() })
                    } else {
                        p.expr()
                    }
                };
                let mut items = vec![item(self)?];
                while *self.peek() == Tok::Comma {
                    self.next();
                    items.push(item(self)?);
                }
                self.expect(Tok::RBracket, "閉じ括弧 ']'")?;
                Ok(Expr::List(items))
            }
            Tok::LParen => {
                let e = self.expr()?;
                // カンマが続けば組 (a, b, c)
                if *self.peek() == Tok::Comma {
                    let mut items = vec![e];
                    while *self.peek() == Tok::Comma {
                        self.next();
                        items.push(self.expr()?);
                    }
                    self.expect(Tok::RParen, "閉じ括弧 ')'")?;
                    return Ok(Expr::Tuple(items));
                }
                self.expect(Tok::RParen, "閉じ括弧 ')'")?;
                Ok(e)
            }
            Tok::Func(f) => self.call(f, t.pos),
            Tok::Eof => err("式が途中で終わっています", t.pos),
            _ => err("ここに値が必要です", t.pos),
        }
    }

    /// 変数の名前の後: 定義された関数の呼び出し `f(x)`・`f'(x)`、プライム `x'`、添字 `a_(n)`、ふつうの変数
    fn after_name(&mut self, name: String) -> Result<Expr, ParseError> {
        let mut primes: u8 = 0;
        while *self.peek() == Tok::Prime {
            self.next();
            primes = primes.saturating_add(1);
        }
        if self.names.contains(&name) && *self.peek() == Tok::LParen {
            self.next();
            let args = self.args_until(Tok::RParen, "閉じ括弧 ')'")?;
            return Ok(Expr::Apply { name, primes, args });
        }
        if primes > 0 {
            return Ok(Expr::Prime { name, order: primes });
        }
        if *self.peek() == Tok::Sub {
            self.next();
            self.expect(Tok::LParen, "'('")?;
            // 添字が 2 つなら組（行列の成分 A_(1, 2)）
            let mut ix = self.args_until(Tok::RParen, "閉じ括弧 ')'")?;
            let index = if ix.len() == 1 { ix.remove(0) } else { Expr::Tuple(ix) };
            return Ok(Expr::Index { name, index: Box::new(index) });
        }
        Ok(Expr::Var(name))
    }

    /// `,` で区切った式を、`close` まで読む（`close` も読み進める）
    fn args_until(&mut self, close: Tok, what: &str) -> Result<Vec<Expr>, ParseError> {
        let mut args = vec![self.expr()?];
        while *self.peek() == Tok::Comma {
            self.next();
            args.push(self.expr()?);
        }
        self.expect(close, what)?;
        Ok(args)
    }

    /// 関係の連鎖 `a ≤ b < c` を 1 つ読む（`Expr::Op { op: "rel:≤<", args: [a, b, c] }`。関係が無ければ誤り）
    fn relation_chain(&mut self, what: &str) -> Result<Expr, ParseError> {
        let mut exprs = vec![self.expr()?];
        let mut rels = String::new();
        while let Tok::Rel(r) = *self.peek() {
            self.next();
            rels.push(match r {
                Rel::Eq => '=',
                Rel::Lt => '<',
                Rel::Le => '≤',
                Rel::Gt => '>',
                Rel::Ge => '≥',
            });
            exprs.push(self.expr()?);
        }
        if rels.is_empty() {
            return err(format!("{what}には、0 ≤ x ≤ 1 のように ≤・< などを書いてください"), self.pos());
        }
        Ok(Expr::Op { op: format!("rel:{rels}"), args: exprs })
    }

    /// 添字と条件 `(n; 条件)`（`@名前` は読んだ後）
    fn index_set(&mut self, name: &str) -> Result<(Expr, Formula), ParseError> {
        self.expect(Tok::LParen, &format!("@{name} の後の '('"))?;
        let vars = self.expr()?;
        self.expect(Tok::Semi, "';'")?;
        let cond = self.formula()?;
        self.expect(Tok::RParen, "閉じ括弧 ')'")?;
        Ok((vars, cond))
    }

    /// 領域の上の積分 `@intset((x, y), 条件, 条件, …)(中身)`（`@intset(` は読んだ後）。中身は直後の項。
    /// `Expr::Op { op: "intset", args: [変数の組, 関係の連鎖…, 中身] }`
    fn intset(&mut self) -> Result<Expr, ParseError> {
        let vars = self.expr()?;
        let mut args = vec![vars];
        while *self.peek() == Tok::Comma {
            self.next();
            // ふつうは関係の連鎖。「または」「否定」などの論理式（x² + y² ≤ 1 ∨ (x − 1)² + y² ≤ 1・¬(x² + y² < 1)）なら、
            // 読み直して論理式として持つ
            let start = self.i;
            match self.relation_chain("積分する領域の条件") {
                Ok(chain) if matches!(self.peek(), Tok::Comma | Tok::Semi | Tok::RParen) => args.push(chain),
                first => {
                    self.i = start;
                    match self.equivalence() {
                        Ok(f) if matches!(self.peek(), Tok::Comma | Tok::Semi | Tok::RParen) => args.push(Expr::Cond(Box::new(f))),
                        // 論理式としても読めなければ、関係の連鎖としての誤りを出す
                        _ => {
                            self.i = start;
                            first?;
                            args.push(self.relation_chain("積分する領域の条件")?);
                        }
                    }
                }
            }
        }
        // `; {vec}`: ベクトル場と向きの付いた要素の内積（F · dr、F · dS）。`; {d_s}`・`; {d_x}, {d_y}` は積分の記号の d の後の文字
        // （弧長 ds・面積 dS・dA・体積 dV・座標 dx。範囲の次元と合っているかを確かめる）。`,` で区切って並べられる
        if *self.peek() == Tok::Semi {
            self.next();
            loop {
                match self.next().tok {
                    Tok::Var(m) => args.push(Expr::Op { op: format!("mode:{m}"), args: Vec::new() }),
                    _ => return err("; の後には {vec} か {d_s} のように書いてください", self.pos()),
                }
                if *self.peek() != Tok::Comma {
                    break;
                }
                self.next();
            }
        }
        self.expect(Tok::RParen, "閉じ括弧 ')'")?;
        args.push(self.term()?);
        Ok(Expr::Op { op: "intset".into(), args })
    }

    /// 場合分け `@cases(値, 条件, 条件; 値, 条件; それ以外の値)`（`@cases(` は読んだ後）。
    /// 場合ごとに `Expr::Op { op: "case", args: [値, 関係の連鎖, …] }`、関係の連鎖は `Expr::Op { op: "rel:<≤", args: [式, 式, 式] }`
    /// （`rel:` の後に関係を 1 文字ずつ。`=` `<` `≤` `>` `≥`）
    fn cases(&mut self) -> Result<Expr, ParseError> {
        let mut cases = Vec::new();
        loop {
            let value = self.expr()?;
            let mut args = vec![value];
            while *self.peek() == Tok::Comma {
                self.next();
                let mut exprs = vec![self.expr()?];
                let mut rels = String::new();
                while let Tok::Rel(r) = *self.peek() {
                    self.next();
                    rels.push(match r {
                        Rel::Eq => '=',
                        Rel::Lt => '<',
                        Rel::Le => '≤',
                        Rel::Gt => '>',
                        Rel::Ge => '≥',
                    });
                    exprs.push(self.expr()?);
                }
                if rels.is_empty() {
                    return err("場合分けの条件には、x < 0 のように =・< などを書いてください", self.pos());
                }
                args.push(Expr::Op { op: format!("rel:{rels}"), args: exprs });
            }
            cases.push(Expr::Op { op: "case".into(), args });
            match self.peek() {
                Tok::Semi => {
                    self.next();
                }
                Tok::RParen => {
                    self.next();
                    return Ok(Expr::Op { op: "cases".into(), args: cases });
                }
                _ => return err("場合分けは、場合を ; で区切り、) で閉じてください", self.pos()),
            }
        }
    }

    /// `@名前` の演算子（[`op_kind`]）
    fn op(&mut self, name: String, pos: usize) -> Result<Expr, ParseError> {
        if name == "cases" {
            self.expect(Tok::LParen, "@cases の後の '('")?;
            return self.cases();
        }
        if name == "intset" {
            self.expect(Tok::LParen, "@intset の後の '('")?;
            return self.intset();
        }
        if name == "sumset" || name == "prodset" {
            // 集合の上の総和・総積 `@sumset(n; 条件) 項`（添字は変数か変数の組）
            let (vars, cond) = self.index_set(&name)?;
            let body = self.term()?;
            return Ok(Expr::Op { op: name, args: vec![vars, Expr::Cond(Box::new(cond)), body] });
        }
        if matches!(name.as_str(), "exists" | "forall") {
            return err(format!("@{name}(a, 0, 1) は、行の先頭に書いてください"), pos);
        }
        if matches!(name.as_str(), "bigcup" | "bigcap" | "bigvee" | "bigwedge") {
            return err(format!("@{name} は条件・集合の中で使ってください"), pos);
        }
        match op_kind(&name) {
            Some(OpKind::Binder) => {
                self.expect(Tok::LParen, &format!("@{name} の後の '('"))?;
                let var = match self.next().tok {
                    Tok::Var(v) => v,
                    _ => return err(format!("@{name}( の後には、束縛する変数の名前を書いてください"), pos),
                };
                let args = if *self.peek() == Tok::Comma {
                    self.next();
                    self.args_until(Tok::RParen, "閉じ括弧 ')'")?
                } else {
                    self.expect(Tok::RParen, "閉じ括弧 ')'")?;
                    Vec::new()
                };
                // 中身は直後の項（掛け算・割り算のかたまり）
                let body = self.term()?;
                Ok(Expr::Binder { op: name, var, args, body: Box::new(body) })
            }
            Some(OpKind::Prefix) => Ok(Expr::Op { op: name, args: vec![self.term()?] }),
            // 行列の演算子は、括弧が無ければ直後の 1 つに掛かる（det A・dim ker A。sin x と同じ読み方）
            Some(OpKind::Func) if matches!(name.as_str(), "det" | "tr" | "ker" | "im" | "dim" | "rank" | "eig" | "crit") && *self.peek() != Tok::LParen => {
                Ok(Expr::Op { op: name, args: vec![self.power()?] })
            }
            Some(OpKind::Func) => {
                self.expect(Tok::LParen, &format!("@{name} の後の '('"))?;
                let args = self.args_until(Tok::RParen, "閉じ括弧 ')'")?;
                Ok(Expr::Op { op: name, args })
            }
            None => err(format!("@{name} という演算子はありません"), pos),
        }
    }

    /// 行列 `⟦a, b; c, d⟧`（`⟦` は読んだ後）
    fn matrix(&mut self, pos: usize) -> Result<Expr, ParseError> {
        let mut rows: Vec<Vec<Expr>> = vec![vec![self.expr()?]];
        loop {
            match self.peek() {
                Tok::Comma => {
                    self.next();
                    let e = self.expr()?;
                    if let Some(row) = rows.last_mut() {
                        row.push(e);
                    }
                }
                Tok::Semi => {
                    self.next();
                    rows.push(vec![self.expr()?]);
                }
                Tok::RMat => {
                    self.next();
                    break;
                }
                _ => return err("行列の成分は , で、行は ; で区切り、⟧ で閉じてください", self.pos()),
            }
        }
        let cols = rows[0].len();
        if rows.iter().any(|r| r.len() != cols) {
            return err("行列の行ごとの成分の数がそろっていません", pos);
        }
        Ok(Expr::Matrix { rows: rows.len(), cols, items: rows.into_iter().flatten().collect() })
    }

    /// 関数呼び出し。`sin(x)`, `sin x`, `sin^2(x)` の3通りを受け付ける。
    fn call(&mut self, f: Func, pos: usize) -> Result<Expr, ParseError> {
        // sin^2(x) → (sin(x))^2
        let mut power = None;
        if *self.peek() == Tok::Caret {
            self.next();
            power = Some(self.primary()?);
        }

        let args = if *self.peek() == Tok::LParen {
            self.next();
            let mut args = vec![self.expr()?];
            while *self.peek() == Tok::Comma {
                self.next();
                args.push(self.expr()?);
            }
            self.expect(Tok::RParen, "閉じ括弧 ')'")?;
            args
        } else {
            // 括弧なし: 直後の1項（累乗を含む）だけを引数にする。
            // 例) sin x^2 = sin(x^2)、 sin x y = sin(x)·y
            // ただし数で始まるときは、続く定数（π・e）と文字 1 つまでを係数つきの 1 項とみなす（数学の書き方どおり
            // sin 2t = sin(2t)、sin 2πx = sin(2πx)、cos 3x^2 = cos(3x^2)）。LaTeX の \sin2t がこの形で届く。
            // 前は sin(2)·t と読み、三葉結び目 (sin t + 2 sin 2t, …) が直線に、ローマ曲面が別の形になっていた
            let mut arg = self.power()?;
            if matches!(arg, Expr::Num(_)) {
                while matches!(self.peek(), Tok::Const(_)) {
                    let c = self.power()?;
                    arg = Expr::Bin(BinOp::Mul, Box::new(arg), Box::new(c));
                }
                if matches!(self.peek(), Tok::Var(_)) {
                    let v = self.power()?;
                    arg = Expr::Bin(BinOp::Mul, Box::new(arg), Box::new(v));
                }
            }
            vec![arg]
        };

        // max・min は 3 つ以上の引数も受け付け、2 つずつの入れ子にする（max(a, b, c) = max(a, max(b, c))）
        let args = if matches!(f, crate::functions::Func::Max | crate::functions::Func::Min) && args.len() > 2 {
            let mut it = args.into_iter().rev();
            let last = it.next().unwrap();
            let nested = it.fold(last, |acc, a| Expr::Call(f, vec![a, acc]));
            match nested {
                Expr::Call(_, a) => a,
                _ => unreachable!(),
            }
        } else {
            args
        };
        if !f.arities().contains(&args.len()) {
            let counts: Vec<String> = f.arities().iter().map(|n| n.to_string()).collect();
            return err(
                format!("{} の引数は {} 個です（{} 個あります）", f.names()[0], counts.join(" か "), args.len()),
                pos,
            );
        }
        let call = Expr::Call(f, args);
        Ok(match power {
            Some(p) => Expr::Bin(BinOp::Pow, Box::new(call), Box::new(p)),
            None => call,
        })
    }
}

/// 文字列を構文木に変換する
pub fn parse(src: &str) -> Result<Expr, ParseError> {
    parse_with(src, &[])
}

/// 定義された関数の名前 `names` を知った上で、文字列を構文木に変換する（`f(x)` を呼び出しとして読む）
pub fn parse_with(src: &str, names: &[String]) -> Result<Expr, ParseError> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, i: 0, names };
    if *p.peek() == Tok::Eof {
        return err("式が空です", 0);
    }
    let e = p.expr()?;
    if *p.peek() != Tok::Eof {
        return err("ここで式が終わるはずです（余分な文字があります）", p.pos());
    }
    Ok(e)
}

/// 連立の式を解析する。括弧の外の `,` で行に分け、行ごとに関係の連鎖 `a ∘ b ∘ c …` を読む
/// （`y = x, z = 0`・`x ≤ y ≤ z`・`0 ≤ x = y`）。関係の無い行もそのまま返す
///
/// ```text
/// system := chain (',' chain)*
/// chain  := expr (rel expr)*
/// ```
pub fn parse_system(src: &str) -> Result<Vec<Chain>, ParseError> {
    parse_system_with(src, &[])
}

/// 定義された関数の名前 `names` を知った上で、連立の式を解析する（[`parse_system`]）
pub fn parse_system_with(src: &str, names: &[String]) -> Result<Vec<Chain>, ParseError> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, i: 0, names };
    if *p.peek() == Tok::Eof {
        return err("式が空です", 0);
    }
    let mut out = Vec::new();
    loop {
        if matches!(p.peek(), Tok::Comma | Tok::Eof) {
            return err("`,` の前後に式を書いてください", p.pos());
        }
        let mut chain = Chain { exprs: Vec::new(), rels: Vec::new(), pos: vec![p.pos()] };
        chain.exprs.push(p.expr()?);
        while let Tok::Rel(r) = *p.peek() {
            p.next();
            chain.rels.push(r);
            chain.pos.push(p.pos());
            chain.exprs.push(p.expr()?);
        }
        out.push(chain);
        match p.peek() {
            Tok::Comma => {
                p.next();
            }
            Tok::Eof => return Ok(out),
            _ => return err("ここで式が終わるはずです（余分な文字があります）", p.pos()),
        }
    }
}

// ---------------------------------------------------------------------------
// 論理式と集合（計画 M11）
// ---------------------------------------------------------------------------

/// 論理・集合の記号（この文字を含む式は、論理式として読む）
pub const LOGIC_CHARS: &str = "∧∨¬⇒⇐⇔⊕⊼⊽∩∪∖△∈∉∋≠⊤⊥∅ℝℤℕℂℚ⦃⦄ᶜ∁";

/// 論理式
#[derive(Debug, Clone, PartialEq)]
pub enum Formula {
    True,
    False,
    /// 関係の連鎖 a ≤ b < c
    Chain(Chain),
    /// a ≠ b
    Ne(Expr, Expr),
    Not(Box<Formula>),
    And(Vec<Formula>),
    Or(Vec<Formula>),
    Xor(Box<Formula>, Box<Formula>),
    Iff(Box<Formula>, Box<Formula>),
    Implies(Box<Formula>, Box<Formula>),
    Nand(Box<Formula>, Box<Formula>),
    Nor(Box<Formula>, Box<Formula>),
    /// 点 ∈ 集合
    In(Expr, SetE),
    /// 集合そのもの（その行の座標の点が属するという条件）
    Set(SetE),
    /// 添字で並べた「または」「かつ」 ⋃_{k ∈ S} A_k・⋂（`or` が true なら ⋃）。添字は `vars`（変数か変数の組）
    Big { or: bool, vars: Expr, cond: Box<Formula>, body: Box<Formula> },
}

impl Formula {
    /// 論理式の中の変数（{点 | 条件} の点の変数・⋃ の添字は外へ出さない）
    pub fn collect_vars(&self, out: &mut Vec<String>) {
        let push = |e: &Expr, out: &mut Vec<String>| e.collect_vars(out);
        match self {
            Formula::True | Formula::False => {}
            Formula::Chain(c) => c.exprs.iter().for_each(|e| push(e, out)),
            Formula::Ne(a, b) => {
                push(a, out);
                push(b, out);
            }
            Formula::Not(a) => a.collect_vars(out),
            Formula::And(v) | Formula::Or(v) => v.iter().for_each(|g| g.collect_vars(out)),
            Formula::Xor(a, b) | Formula::Iff(a, b) | Formula::Implies(a, b) | Formula::Nand(a, b) | Formula::Nor(a, b) => {
                a.collect_vars(out);
                b.collect_vars(out);
            }
            Formula::In(p, s) => {
                push(p, out);
                s.collect_vars(out);
            }
            Formula::Set(s) => s.collect_vars(out),
            Formula::Big { vars, cond, body, .. } => {
                let mut bound = Vec::new();
                vars.collect_vars(&mut bound);
                let mut inner = Vec::new();
                cond.collect_vars(&mut inner);
                body.collect_vars(&mut inner);
                for v in inner {
                    if !bound.contains(&v) && !out.contains(&v) {
                        out.push(v);
                    }
                }
            }
        }
    }
}

impl SetE {
    fn collect_vars(&self, out: &mut Vec<String>) {
        match self {
            SetE::Builder { point, cond } => {
                let mut bound = Vec::new();
                point.collect_vars(&mut bound);
                let mut inner = Vec::new();
                cond.collect_vars(&mut inner);
                for v in inner {
                    if !bound.contains(&v) && !out.contains(&v) {
                        out.push(v);
                    }
                }
            }
            SetE::Interval { lo, hi, .. } => {
                lo.collect_vars(out);
                hi.collect_vars(out);
            }
            SetE::Finite(items) => items.iter().for_each(|e| e.collect_vars(out)),
            SetE::Product(parts) => parts.iter().for_each(|p| p.collect_vars(out)),
            SetE::Union(a, b) | SetE::Inter(a, b) | SetE::Diff(a, b) | SetE::SymDiff(a, b) => {
                a.collect_vars(out);
                b.collect_vars(out);
            }
            SetE::Complement(a) => a.collect_vars(out),
            SetE::Named(_) | SetE::Numbers(_) | SetE::Empty => {}
        }
    }
}

/// 集合
#[derive(Debug, Clone, PartialEq)]
pub enum SetE {
    /// 内包的表記 {点 | 条件}
    Builder { point: Expr, cond: Box<Formula> },
    /// 名前を付けた集合（定義 `S = {…}`）
    Named(String),
    /// 要素を書き並べた有限集合 {1, 4, 9}
    Finite(Vec<Expr>),
    /// 区間（端は ±∞ でもよい）
    Interval { lo: Expr, hi: Expr, lo_open: bool, hi_open: bool },
    /// ℝ ℤ ℕ ℂ ℚ
    Numbers(char),
    Product(Vec<SetE>),
    Union(Box<SetE>, Box<SetE>),
    Inter(Box<SetE>, Box<SetE>),
    Diff(Box<SetE>, Box<SetE>),
    SymDiff(Box<SetE>, Box<SetE>),
    Complement(Box<SetE>),
    Empty,
}

impl Parser<'_> {
    fn sym(&self) -> Option<char> {
        match self.peek() {
            Tok::Sym(c) => Some(*c),
            _ => None,
        }
    }

    /// 論理式（弱い順に `,` < ⇔ < ⇒・⇐ < ∨・∪・⊕・△・⊽ < ∧・∩・∖・⊼ < ¬ < 原子）。
    /// `,` は条件の区切り（かつ）で、いちばん弱い: `x < 0 ∨ y < 0, z > 0` は (x < 0 ∨ y < 0) ∧ z > 0
    fn formula(&mut self) -> Result<Formula, ParseError> {
        let mut parts = vec![self.equivalence()?];
        while *self.peek() == Tok::Comma {
            self.next();
            parts.push(self.equivalence()?);
        }
        let mut parts = shared_bounds_in_formulas(parts);
        Ok(if parts.len() == 1 { parts.pop().unwrap_or(Formula::True) } else { Formula::And(parts) })
    }

    fn equivalence(&mut self) -> Result<Formula, ParseError> {
        let mut f = self.implication()?;
        while self.sym() == Some('⇔') {
            self.next();
            let g = self.implication()?;
            f = Formula::Iff(Box::new(f), Box::new(g));
        }
        Ok(f)
    }

    fn implication(&mut self) -> Result<Formula, ParseError> {
        let f = self.disjunction()?;
        match self.sym() {
            Some('⇒') => {
                self.next();
                Ok(Formula::Implies(Box::new(f), Box::new(self.implication()?)))
            }
            Some('⇐') => {
                self.next();
                Ok(Formula::Implies(Box::new(self.implication()?), Box::new(f)))
            }
            _ => Ok(f),
        }
    }

    fn disjunction(&mut self) -> Result<Formula, ParseError> {
        let mut f = self.conjunction(false)?;
        loop {
            let c = match self.sym() {
                Some(c @ ('∨' | '∪' | '⊕' | '△' | '⊽')) => c,
                _ => return Ok(f),
            };
            self.next();
            let g = self.conjunction(false)?;
            f = match c {
                '∨' | '∪' => Formula::Or(vec![f, g]),
                '⊽' => Formula::Nor(Box::new(f), Box::new(g)),
                _ => Formula::Xor(Box::new(f), Box::new(g)),
            };
        }
    }

    /// `comma` が true なら、`,` も「かつ」（集合の括弧・場合分けの中では false にしない。ここでは常に true）
    fn conjunction(&mut self, comma: bool) -> Result<Formula, ParseError> {
        let mut f = self.negation()?;
        loop {
            let c = match (self.peek(), self.sym()) {
                (Tok::Comma, _) if comma => ',',
                (_, Some(c @ ('∧' | '∩' | '∖' | '⊼'))) => c,
                _ => return Ok(f),
            };
            self.next();
            let g = self.negation()?;
            f = match c {
                '∖' => Formula::And(vec![f, Formula::Not(Box::new(g))]),
                '⊼' => Formula::Nand(Box::new(f), Box::new(g)),
                _ => Formula::And(vec![f, g]),
            };
        }
    }

    fn negation(&mut self) -> Result<Formula, ParseError> {
        if self.sym() == Some('¬') {
            self.next();
            return Ok(Formula::Not(Box::new(self.negation()?)));
        }
        let mut f = self.logic_atom()?;
        while self.sym() == Some('ᶜ') {
            self.next();
            f = Formula::Not(Box::new(f));
        }
        Ok(f)
    }

    /// `(` の後が論理式か（対応する `)` までの深さ 1 に、関係・論理・集合の記号があるか）
    fn paren_is_formula(&self) -> bool {
        let mut depth = 0;
        for t in &self.toks[self.i..] {
            match &t.tok {
                Tok::LParen | Tok::LBracket | Tok::LMat => depth += 1,
                Tok::RParen | Tok::RBracket | Tok::RMat => {
                    depth -= 1;
                    if depth == 0 {
                        return false;
                    }
                }
                Tok::Rel(_) | Tok::Sym(_) if depth == 1 => return true,
                Tok::Eof => return false,
                _ => {}
            }
        }
        false
    }

    fn logic_atom(&mut self) -> Result<Formula, ParseError> {
        // ⋃_{k ∈ S} A_k・⋂（⋁・⋀ も）
        if let Tok::Op(name) = self.peek().clone() {
            if matches!(name.as_str(), "bigcup" | "bigcap" | "bigvee" | "bigwedge") {
                self.next();
                let (vars, cond) = self.index_set(&name)?;
                let body = self.negation()?;
                return Ok(Formula::Big { or: name == "bigcup" || name == "bigvee", vars, cond: Box::new(cond), body: Box::new(body) });
            }
        }
        match self.sym() {
            Some('⊤') => {
                self.next();
                return Ok(Formula::True);
            }
            Some('⊥') => {
                self.next();
                return Ok(Formula::False);
            }
            Some('⦃' | '∅' | 'ℝ' | 'ℤ' | 'ℕ' | 'ℂ' | 'ℚ' | '∁') => return Ok(Formula::Set(self.set_factor()?)),
            _ => {}
        }
        if *self.peek() == Tok::LParen && self.paren_is_formula() {
            self.next();
            let f = self.formula()?;
            self.expect(Tok::RParen, "閉じ括弧 ')'")?;
            return Ok(f);
        }
        let pos = self.pos();
        let e = self.expr()?;
        match (self.peek().clone(), self.sym()) {
            (Tok::Rel(r), _) => {
                let mut chain = Chain { exprs: vec![e], rels: vec![], pos: vec![pos] };
                let mut rel = Some(r);
                while let Some(r) = rel {
                    self.next();
                    chain.rels.push(r);
                    chain.pos.push(self.pos());
                    chain.exprs.push(self.expr()?);
                    rel = match self.peek() {
                        Tok::Rel(r) => Some(*r),
                        _ => None,
                    };
                }
                Ok(Formula::Chain(chain))
            }
            (_, Some('≠')) => {
                self.next();
                Ok(Formula::Ne(e, self.expr()?))
            }
            // 割り切る d ∣ n（mod(n, d) = 0 で、1 ≤ d ≤ |n|）
            (_, Some('∣')) => {
                self.next();
                let n = self.expr()?;
                let mk = |exprs: Vec<Expr>, rels: Vec<Rel>| Formula::Chain(Chain { pos: vec![pos; exprs.len()], exprs, rels });
                let modulo = Expr::Call(Func::Mod, vec![n.clone(), e.clone()]);
                let abs = Expr::Call(Func::Abs, vec![n]);
                Ok(Formula::And(vec![mk(vec![modulo, Expr::Num(0.0)], vec![Rel::Eq]), mk(vec![Expr::Num(1.0), e, abs], vec![Rel::Le, Rel::Le])]))
            }
            (_, Some('∈')) => {
                self.next();
                Ok(Formula::In(e, self.set_expr()?))
            }
            (_, Some('∉')) => {
                self.next();
                Ok(Formula::Not(Box::new(Formula::In(e, self.set_expr()?))))
            }
            (_, Some('∋')) => {
                self.next();
                let p = self.expr()?;
                Ok(Formula::In(p, set_of_expr(e, pos)?))
            }
            _ => Ok(Formula::Set(set_of_expr(e, pos)?)),
        }
    }

    /// ∈ の右の集合（弱い順に ∪・∖・△ < ∩ < × < 原子）
    fn set_expr(&mut self) -> Result<SetE, ParseError> {
        let mut s = self.set_inter()?;
        loop {
            let c = match self.sym() {
                Some(c @ ('∪' | '∖' | '△')) => c,
                _ => return Ok(s),
            };
            self.next();
            let t = self.set_inter()?;
            s = match c {
                '∪' => SetE::Union(Box::new(s), Box::new(t)),
                '∖' => SetE::Diff(Box::new(s), Box::new(t)),
                _ => SetE::SymDiff(Box::new(s), Box::new(t)),
            };
        }
    }

    fn set_inter(&mut self) -> Result<SetE, ParseError> {
        let mut s = self.set_product()?;
        while self.sym() == Some('∩') {
            self.next();
            s = SetE::Inter(Box::new(s), Box::new(self.set_product()?));
        }
        Ok(s)
    }

    fn set_product(&mut self) -> Result<SetE, ParseError> {
        let first = self.set_factor()?;
        let mut parts = vec![first];
        while *self.peek() == Tok::Cross {
            self.next();
            parts.push(self.set_factor()?);
        }
        Ok(if parts.len() == 1 { parts.pop().unwrap_or(SetE::Empty) } else { SetE::Product(parts) })
    }

    /// `(` か `[` の後に、深さ 1 のカンマがあるか（区間）
    fn bracket_has_comma(&self) -> bool {
        let mut depth = 0;
        for t in &self.toks[self.i..] {
            match &t.tok {
                Tok::LParen | Tok::LBracket | Tok::LMat => depth += 1,
                Tok::RParen | Tok::RBracket | Tok::RMat => {
                    depth -= 1;
                    if depth == 0 {
                        return false;
                    }
                }
                Tok::Comma if depth == 1 => return true,
                Tok::Eof => return false,
                _ => {}
            }
        }
        false
    }

    fn set_factor(&mut self) -> Result<SetE, ParseError> {
        let pos = self.pos();
        let mut s = match (self.peek().clone(), self.sym()) {
            (_, Some('⦃')) => {
                self.next();
                let point = self.expr()?;
                // 要素を書き並べた有限集合 {1, 4, 9}
                if matches!(self.peek(), Tok::Comma) || self.sym() == Some('⦄') {
                    let mut items = vec![point];
                    while *self.peek() == Tok::Comma {
                        self.next();
                        items.push(self.expr()?);
                    }
                    if self.sym() != Some('⦄') {
                        return err("集合の括弧 } が閉じていません", self.pos());
                    }
                    self.next();
                    return Ok(SetE::Finite(items));
                }
                if self.sym() != Some('∣') {
                    return err("集合は {(x, y) | 条件} か {1, 2, 3} の形で書いてください", self.pos());
                }
                self.next();
                let cond = self.formula()?;
                if self.sym() != Some('⦄') {
                    return err("集合の括弧 } が閉じていません", self.pos());
                }
                self.next();
                SetE::Builder { point, cond: Box::new(cond) }
            }
            (_, Some('∅')) => {
                self.next();
                SetE::Empty
            }
            (_, Some(c @ ('ℝ' | 'ℤ' | 'ℕ' | 'ℂ' | 'ℚ'))) => {
                self.next();
                SetE::Numbers(c)
            }
            (_, Some('∁')) => {
                self.next();
                SetE::Complement(Box::new(self.set_factor()?))
            }
            (Tok::LParen | Tok::LBracket, _) if self.bracket_has_comma() => {
                let lo_open = *self.peek() == Tok::LParen;
                self.next();
                let lo = self.expr()?;
                self.expect(Tok::Comma, "','")?;
                let hi = self.expr()?;
                let hi_open = match self.next().tok {
                    Tok::RParen => true,
                    Tok::RBracket => false,
                    _ => return err("区間は [a, b]・(a, b)・[a, b) の形で書いてください", pos),
                };
                SetE::Interval { lo, hi, lo_open, hi_open }
            }
            (Tok::LParen, _) => {
                self.next();
                let s = self.set_expr()?;
                self.expect(Tok::RParen, "閉じ括弧 ')'")?;
                s
            }
            _ => {
                let e = self.power()?;
                set_of_expr(e, pos)?
            }
        };
        while self.sym() == Some('ᶜ') {
            self.next();
            s = SetE::Complement(Box::new(s));
        }
        Ok(s)
    }
}

/// 式として読んだものを集合にする（名前 `S`、補集合 `S^(c)`）
fn set_of_expr(e: Expr, pos: usize) -> Result<SetE, ParseError> {
    match e {
        Expr::Var(n) => Ok(SetE::Named(n)),
        Expr::Bin(BinOp::Pow, a, c) if matches!(&*c, Expr::Var(v) if v == "c") => Ok(SetE::Complement(Box::new(set_of_expr(*a, pos)?))),
        _ => err("条件には、x < 1 のように =・< などを書いてください（集合なら、名前か {(x, y) | …} を書きます）", pos),
    }
}

/// 論理式を読む（計画 M11）。`names` は定義された関数の名前
pub fn parse_formula(src: &str, names: &[String]) -> Result<Formula, ParseError> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, i: 0, names };
    if *p.peek() == Tok::Eof {
        return err("式が空です", 0);
    }
    let f = p.formula()?;
    if *p.peek() != Tok::Eof {
        return err("ここで式が終わるはずです（余分な文字があります）", p.pos());
    }
    Ok(f)
}

/// 定義の行の左辺の形 `名前(変数, 変数, …) =` なら、(名前, 変数の並び)。字句だけで決める
/// （構文木にすると `f(x)` と `f x` の区別が付かないため）。変数の名前が重なっていれば None
pub fn definition_head(src: &str) -> Option<(String, Vec<String>)> {
    let toks = tokenize(src).ok()?;
    let Tok::Var(name) = &toks.first()?.tok else { return None };
    if toks.get(1)?.tok != Tok::LParen {
        return None;
    }
    let mut args: Vec<String> = Vec::new();
    let mut k = 2;
    loop {
        let Tok::Var(a) = &toks.get(k)?.tok else { return None };
        if args.contains(a) || a == name {
            return None;
        }
        args.push(a.clone());
        match &toks.get(k + 1)?.tok {
            Tok::Comma => k += 2,
            Tok::RParen => break,
            _ => return None,
        }
    }
    (toks.get(k + 2)?.tok == Tok::Rel(Rel::Eq)).then(|| (name.clone(), args))
}

#[cfg(test)]
mod tests {
    #[test]
    fn function_with_numeric_coefficient_argument() {
        // sin 2t = sin(2t)（三葉結び目・ローマ曲面のテスト表の式。LaTeX の \sin2t がこの形で届く）
        assert_eq!(parse("sin 2t").unwrap(), parse("sin(2t)").unwrap());
        assert_eq!(parse("sin 2pi x").unwrap(), parse("sin(2 pi x)").unwrap());
        assert_eq!(parse("cos 3x^2").unwrap(), parse("cos(3 x^2)").unwrap());
        assert_eq!(parse("sin t + 2 sin 2t").unwrap(), parse("sin(t) + 2 sin(2t)").unwrap());
        // 今までどおり: 文字の後の文字は引数に入れない、数だけなら数だけ
        assert_eq!(parse("sin x y").unwrap(), parse("sin(x) y").unwrap());
        assert_eq!(parse("sin 2").unwrap(), parse("sin(2)").unwrap());
        assert_eq!(parse("sin 2 cos x").unwrap(), parse("sin(2) cos(x)").unwrap());
        assert_eq!(parse("sin 2t y").unwrap(), parse("sin(2t) y").unwrap());
    }
    #[test]
    fn max_min_with_many_arguments_nest() {
        // max(a, b, c) = max(a, max(b, c))（計画 12 章の表の S-11: max(|x|, |y|, |z|) = 1）
        assert_eq!(parse("max(a, b, c)").unwrap(), parse("max(a, max(b, c))").unwrap());
        assert_eq!(parse("min(a, b, c, d)").unwrap(), parse("min(a, min(b, min(c, d)))").unwrap());
        assert!(parse("max(a)").is_err());
    }
    use super::*;

    fn var(s: &str) -> Box<Expr> {
        Box::new(Expr::Var(s.into()))
    }

    #[test]
    fn implicit_multiplication() {
        assert_eq!(
            parse("xy").unwrap(),
            Expr::Bin(BinOp::Mul, var("x"), var("y"))
        );
        assert_eq!(
            parse("2x").unwrap(),
            Expr::Bin(BinOp::Mul, Box::new(Expr::Num(2.0)), var("x"))
        );
    }

    #[test]
    fn neg_power_precedence() {
        // -x^2 = -(x^2)
        assert_eq!(
            parse("-x^2").unwrap(),
            Expr::Neg(Box::new(Expr::Bin(BinOp::Pow, var("x"), Box::new(Expr::Num(2.0)))))
        );
    }

    #[test]
    fn function_forms() {
        let a = parse("sin(x)").unwrap();
        assert_eq!(parse("sin x").unwrap(), a);
        assert_eq!(parse("sinx").unwrap(), a);
        assert!(matches!(parse("sinh(x)").unwrap(), Expr::Call(Func::Sinh, _)));
        assert!(matches!(parse("sin^2 x").unwrap(), Expr::Bin(BinOp::Pow, _, _)));
    }

    #[test]
    fn subscripts() {
        assert_eq!(parse("w_2").unwrap(), Expr::Var("w2".into()));
        assert_eq!(
            parse("2t_1x").unwrap(),
            Expr::Bin(
                BinOp::Mul,
                Box::new(Expr::Bin(BinOp::Mul, Box::new(Expr::Num(2.0)), var("t1"))),
                var("x")
            )
        );
        assert_eq!(parse("w_1").unwrap(), Expr::Var("w".into()));
        assert!(parse("x_").is_err());
        assert!(parse("sin_2").is_err());
    }

    #[test]
    fn notation_tokens() {
        // @sum は束縛する変数を持ち、中身は直後の項
        let e = parse("@sum(n, 1, 3) n^2 + 1").unwrap();
        let Expr::Bin(BinOp::Add, a, _) = e else { panic!("{e:?}") };
        assert!(matches!(*a, Expr::Binder { ref op, ref var, ref args, .. } if op == "sum" && var == "n" && args.len() == 2));
        // {名前} は 1 つの変数
        assert_eq!(parse("{alpha} x").unwrap(), Expr::Bin(BinOp::Mul, var("alpha"), var("x")));
        // 数字でない添字
        assert!(matches!(parse("a_(n+1)").unwrap(), Expr::Index { ref name, .. } if name == "a"));
        assert_eq!(parse("a_1").unwrap(), Expr::Var("a1".into()));
        // 行列
        assert!(matches!(parse("⟦1, 2; 3, 4⟧").unwrap(), Expr::Matrix { rows: 2, cols: 2, .. }));
        assert!(parse("⟦1, 2; 3⟧").is_err());
        // プライム
        assert_eq!(parse("x'").unwrap(), Expr::Prime { name: "x".into(), order: 1 });
        // 外積
        assert!(matches!(parse("a × b").unwrap(), Expr::Op { ref op, .. } if op == "cross"));
        assert!(parse("@nothing(x)").is_err());
    }

    #[test]
    fn defined_names_make_calls() {
        let names = vec!["f".to_string()];
        // 定義された名前なら呼び出し。f(x)^2 は呼び出しの 2 乗
        let e = parse_with("f(x)^2", &names).unwrap();
        let Expr::Bin(BinOp::Pow, a, _) = e else { panic!("{e:?}") };
        assert!(matches!(*a, Expr::Apply { ref name, primes: 0, ref args } if name == "f" && args.len() == 1));
        assert!(matches!(parse_with("f''(x)", &names).unwrap(), Expr::Apply { primes: 2, .. }));
        // 定義が無ければ今までどおり掛け算 f·x²
        assert_eq!(
            parse("f(x)^2").unwrap(),
            Expr::Bin(BinOp::Mul, var("f"), Box::new(Expr::Bin(BinOp::Pow, var("x"), Box::new(Expr::Num(2.0)))))
        );
    }

    #[test]
    fn definition_heads() {
        assert_eq!(definition_head("f(x) = x^2"), Some(("f".into(), vec!["x".into()])));
        assert_eq!(definition_head("g(x, y)=x y"), Some(("g".into(), vec!["x".into(), "y".into()])));
        assert_eq!(definition_head("f x = 1"), None);
        assert_eq!(definition_head("f(x, x) = 1"), None);
        assert_eq!(definition_head("f(2) = 1"), None);
        assert_eq!(definition_head("f(x) + 1 = 2"), None);
    }

    #[test]
    fn binder_scoping() {
        let e = parse("@sum(n, 1, 3) n x").unwrap();
        let mut vs = Vec::new();
        e.collect_vars(&mut vs);
        assert_eq!(vs, vec!["x".to_string()]);
        // 置き換えても、中身の束縛する変数は変わらない
        let s = e.substitute(&|v| (v == "n").then(|| Expr::Num(5.0)));
        assert_eq!(s, e);
    }

    #[test]
    fn existing_reads_unchanged() {
        // e^(int) は e^(i·n·t) のまま（int を名前にしない）
        let e = parse("e^(int)").unwrap();
        let Expr::Bin(BinOp::Pow, _, b) = e else { panic!() };
        let mut vs = Vec::new();
        b.collect_vars(&mut vs);
        assert_eq!(vs, vec!["n".to_string(), "t".to_string()]);
        // × は今までどおり（展開の段で掛け算に戻す）。· は掛け算
        assert_eq!(parse("x·y").unwrap(), Expr::Bin(BinOp::Mul, var("x"), var("y")));
    }

    #[test]
    fn errors() {
        assert!(parse("").is_err());
        assert!(parse("(x+1").is_err());
        assert!(parse("x+").is_err());
        assert!(parse("x $ y").is_err());
    }
}

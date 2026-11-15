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
}

impl Expr {
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
            Expr::Apply { .. } | Expr::Binder { .. } | Expr::Op { .. } | Expr::Matrix { .. } | Expr::Index { .. } | Expr::Prime { .. } => true,
        }
    }

    /// 節の数（展開した式の大きさの上限を調べるのに使う）
    pub fn node_count(&self) -> usize {
        1 + match self {
            Expr::Num(_) | Expr::Const(_) | Expr::Var(_) | Expr::Prime { .. } => 0,
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

/// `@名前` の演算子の一覧（名前 → 読み方）。ステップごとに足す（計画 `docs/plan-math-notation.md`）
pub fn op_kind(name: &str) -> Option<OpKind> {
    Some(match name {
        "sum" | "prod" | "int" | "diff" => OpKind::Binder,
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
        if c.is_alphabetic() {
            while i < chars.len() && chars[i].is_alphabetic() {
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
                while end < chars.len() && chars[end].is_alphabetic() {
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
            continue;
        }

        let tok = match c {
            '+' => Tok::Plus,
            '-' | '−' => Tok::Minus,
            '*' | '·' => Tok::Star,
            '×' => Tok::Cross,
            '∘' => Tok::Compose,
            '\'' | '′' => Tok::Prime,
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
            _ => self.power(),
        }
    }

    fn power(&mut self) -> Result<Expr, ParseError> {
        let mut base = self.primary()?;
        // 階乗 x! = Γ(x + 1)。累乗より先に計算する（3!^2 = (3!)^2）
        while *self.peek() == Tok::Bang {
            self.next();
            let arg = Expr::Bin(BinOp::Add, Box::new(base), Box::new(Expr::Num(1.0)));
            base = Expr::Call(Func::Gamma, vec![arg]);
        }
        if *self.peek() == Tok::Caret {
            self.next();
            let exp = self.unary()?;
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
                let mut items = vec![self.expr()?];
                while *self.peek() == Tok::Comma {
                    self.next();
                    items.push(self.expr()?);
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
            let index = self.expr()?;
            self.expect(Tok::RParen, "閉じ括弧 ')'")?;
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

    /// `@名前` の演算子（[`op_kind`]）
    fn op(&mut self, name: String, pos: usize) -> Result<Expr, ParseError> {
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
            vec![self.power()?]
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

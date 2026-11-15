//! 展開の段: 数学の記法（計画 `docs/plan-math-notation.md`）を、今までの式（`Expr` の 8 つの節だけ）に直す。
//!
//! ```text
//! 文字列 → 構文木（新しい節あり） → [この段] → 構文木（今までの節だけ） → 今までの道（陽関数・陰関数・…）
//! ```
//!
//! - 関数の定義 `f(x) = x^2` と、その呼び出し `f(2x)`・合成 `g(f(x))`・関数どうしの演算 `(f + g)(x)`・`(f ∘ g)(x)`（M1）
//! - 総和 `@sum(n, 1, N) 項`・総積 `@prod(…)`、等差のリスト `[1, 3, …, 9]`、両方の符号 `±`・`∓`（M4）
//! - 微分 `@diff(x) 項`・`@diff(x, 2) 項`、導関数 `f'(x)`（M3。記号微分は `diff.rs`）
//! - 場合分け `@cases(値, 条件; 値, 条件; それ以外の値)`（M2。[`crate::functions::Func::Piece`] にする）
//! - 定積分 `@int(t, a, b)(式)`（M5。ガウス・ルジャンドルの 16 点 × 2 区間の重み付きの和にする。端は式でも ∞ でもよい）
//!
//! 定義は、ほかの行（式の一覧）から [`Defs::parse`] で作り、[`install`] で入れておく（`lib.rs` が `plot` の間だけ入れる）。
//! 計算エンジンの式の文法は `parser.rs` の先頭の説明。展開した後の式には、新しい節を残さない（残れば内部エラー）

use crate::eval::Params;
use crate::parser::{definition_head, parse_system_with, BinOp, Expr, Rel};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// 展開した式の節の数の上限（計画 2.5）
pub const MAX_NODES: usize = 200_000;
/// Σ・Π の項の数の上限
const MAX_TERMS: usize = 2_000;
/// `[a, …, c]` の要素の数の上限（リストの組み合わせの上限と同じ）
const MAX_LIST: usize = 256;
/// `±` の符号を表す変数（展開の後、[`crate::equation::compile_with`] が +1 と −1 の 2 つの図形にする。利用者は書けない名前）
pub const PM_SIGN: &str = "#pm";
/// 関数の呼び出しの入れ子の深さの上限
const MAX_DEPTH: usize = 64;

/// 定義 1 つ
#[derive(Debug, Clone)]
pub enum Def {
    /// 関数 `f(x, y) = body`。`args` は仮引数の名前
    Function { args: Vec<String>, body: Expr },
    /// 引数の無い定義（関数の形 `h = f + g`・`h = f ∘ g`）
    Value(Expr),
}

/// 定義の表（名前 → 定義）
#[derive(Debug, Clone, Default)]
pub struct Defs {
    map: BTreeMap<String, Def>,
}

/// 定義にできる名前か（計画 2.4）: 座標（x, y, z, w, w₂, …）・e・i・媒介変数（t, u, v, t₁, …）・座標系の座標の名前は不可
pub fn definable(name: &str) -> bool {
    let is_param_var = matches!(name, "t" | "u" | "v")
        || name.strip_prefix('t').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
    !(crate::equation::coord_index(name).is_some()
        || matches!(name, "e" | "i" | "theta" | "phi" | "rho")
        || is_param_var
        || name.is_empty())
}

impl Defs {
    /// 式の一覧の行（`\n` 区切り）から、定義の行を集める。定義でない行・誤りのある行は無視する
    /// （誤りは、その行を描くときに出る）。`params` にある名前（スライダー）は定義にしない（`a(x+1)` は今までどおり積）
    pub fn parse(src: &str, params: &Params) -> Defs {
        let lines: Vec<&str> = src.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        // 1 回目: 関数の定義の左辺 `f(x) =` から名前を集める
        let mut heads: Vec<(usize, String, Vec<String>)> = Vec::new();
        for (k, line) in lines.iter().enumerate() {
            if let Some((name, args)) = definition_head(line) {
                if definable(&name) && !params.contains_key(&name) && !heads.iter().any(|h| h.1 == name) {
                    heads.push((k, name, args));
                }
            }
        }
        let mut names: Vec<String> = heads.iter().map(|h| h.1.clone()).collect();
        // 引数の無い定義 `h = f + g`: 右辺が関数の名前を（呼び出さずに）使う行。h もほかの行から使えるので、増えなくなるまで繰り返す
        let mut values: Vec<(usize, String)> = Vec::new();
        for _ in 0..8 {
            let before = values.len();
            for (k, line) in lines.iter().enumerate() {
                if heads.iter().any(|h| h.0 == k) || values.iter().any(|v| v.0 == k) {
                    continue;
                }
                let Ok(chains) = parse_system_with(line, &names) else { continue };
                let [chain] = chains.as_slice() else { continue };
                if chain.rels != [Rel::Eq] {
                    continue;
                }
                let Expr::Var(name) = &chain.exprs[0] else { continue };
                if !definable(name) || params.contains_key(name) || names.contains(name) {
                    continue;
                }
                if uses_function_value(&chain.exprs[1], &names) {
                    names.push(name.clone());
                    values.push((k, name.clone()));
                }
            }
            if values.len() == before {
                break;
            }
        }
        let mut map = BTreeMap::new();
        for (k, name, args) in heads {
            let Ok(chains) = parse_system_with(lines[k], &names) else { continue };
            let [chain] = chains.as_slice() else { continue };
            if chain.rels != [Rel::Eq] {
                continue;
            }
            map.insert(name, Def::Function { args, body: chain.exprs[1].clone() });
        }
        for (k, name) in values {
            let Ok(chains) = parse_system_with(lines[k], &names) else { continue };
            map.insert(name, Def::Value(chains[0].exprs[1].clone()));
        }
        Defs { map }
    }

    /// 定義された名前（解析器に渡して `f(x)` を呼び出しとして読ませる）
    pub fn names(&self) -> Vec<String> {
        self.map.keys().cloned().collect()
    }

    pub fn get(&self, name: &str) -> Option<&Def> {
        self.map.get(name)
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// 式が、関数の名前を呼び出さずに（値として）使っているか（`f + g`・`f ∘ g`・`sin(f)`）
fn uses_function_value(e: &Expr, names: &[String]) -> bool {
    match e {
        Expr::Var(v) => names.contains(v),
        Expr::Num(_) | Expr::Const(_) | Expr::Prime { .. } => false,
        Expr::Neg(a) | Expr::Index { index: a, .. } => uses_function_value(a, names),
        Expr::Bin(_, a, b) => uses_function_value(a, names) || uses_function_value(b, names),
        Expr::Call(_, args) | Expr::Tuple(args) | Expr::List(args) | Expr::Op { args, .. } => {
            args.iter().any(|a| uses_function_value(a, names))
        }
        // 呼び出しの引数の中の名前は値（`g(f)` は無いが、`f(g(x))` は呼び出しなので数えない）
        Expr::Apply { args, .. } => args.iter().any(|a| uses_function_value(a, names)),
        Expr::Matrix { items, .. } => items.iter().any(|a| uses_function_value(a, names)),
        Expr::Binder { args, body, .. } => args.iter().any(|a| uses_function_value(a, names)) || uses_function_value(body, names),
    }
}

// ---------------------------------------------------------------------------
// 今の定義（plot の間だけ入れる）
// ---------------------------------------------------------------------------

thread_local! {
    static CURRENT: RefCell<Rc<Defs>> = RefCell::new(Rc::new(Defs::default()));
    /// 展開で使った定義の名前（画面が、定義が変わったときに描き直す式を決めるのに使う）
    static USED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// 展開の途中で値を使ったパラメータ（Σ の範囲の N など。展開した式には残らないので、別に覚えておく）
    static USED_PARAMS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// 直前にコンパイルした行が定義の行なら、その名前と、描いたものの説明（`y = f(x)` など。描かないなら None）
    static DEF_LINE: RefCell<Option<(String, Option<String>)>> = const { RefCell::new(None) };
}

/// [`install`] の戻り値。捨てると、前の定義に戻す
pub struct Installed(Option<Rc<Defs>>);

impl Drop for Installed {
    fn drop(&mut self) {
        if let Some(prev) = self.0.take() {
            CURRENT.with(|c| *c.borrow_mut() = prev);
        }
    }
}

/// 定義を入れる（戻り値を捨てるまで）。使った定義の名前と、定義の行の印も空にする
pub fn install(defs: Defs) -> Installed {
    USED.with(|u| u.borrow_mut().clear());
    USED_PARAMS.with(|u| u.borrow_mut().clear());
    DEF_LINE.with(|d| *d.borrow_mut() = None);
    Installed(Some(CURRENT.with(|c| std::mem::replace(&mut *c.borrow_mut(), Rc::new(defs)))))
}

fn current() -> Rc<Defs> {
    CURRENT.with(|c| c.borrow().clone())
}

/// 今の定義の名前
pub fn names() -> Vec<String> {
    current().names()
}

/// 展開で使った定義の名前（重複なし、出てきた順）
pub fn used() -> Vec<String> {
    USED.with(|u| u.borrow().clone())
}

/// 展開の途中で値を使ったパラメータ（[`install`] の後。Σ の範囲など、展開した式に残らないもの）
pub fn used_params() -> Vec<String> {
    USED_PARAMS.with(|u| u.borrow().clone())
}

/// 直前にコンパイルした行が定義の行なら、その名前
pub fn def_line() -> Option<String> {
    DEF_LINE.with(|d| d.borrow().as_ref().map(|d| d.0.clone()))
}

/// 直前にコンパイルした定義の行の説明（「f の定義（y = f(x)）」）。定義の行でなければ None
pub fn def_line_label() -> Option<String> {
    DEF_LINE.with(|d| {
        d.borrow().as_ref().map(|(name, drawn)| match drawn {
            Some(l) => format!("{name} の定義（{l}）"),
            None => format!("{name} の定義"),
        })
    })
}

/// コンパイルした行が定義の行だったことを記録する（`equation.rs`）。`drawn` は描いたものの説明
pub fn mark_def_line(name: &str, drawn: Option<String>) {
    DEF_LINE.with(|d| *d.borrow_mut() = Some((name.to_string(), drawn)));
}

fn note_used(name: &str) {
    USED.with(|u| {
        let mut u = u.borrow_mut();
        if !u.iter().any(|n| n == name) {
            u.push(name.to_string());
        }
    });
}

// ---------------------------------------------------------------------------
// 展開
// ---------------------------------------------------------------------------

/// 展開の途中の値の形
#[derive(Debug, Clone)]
enum Val {
    /// ふつうの式（スカラー。組・リストもここ）
    S(Expr),
    /// 関数。仮引数は `#0`, `#1`, …（利用者が書けない名前なので、本体の中の外の名前とぶつからない）
    Fn { arity: usize, body: Expr },
}

/// 仮引数の名前
fn arg_name(k: usize) -> String {
    format!("#{k}")
}

/// 関数の本体の仮引数 `#k` に、式 `args[k]` を一度に代入する
fn apply_body(body: &Expr, args: &[Expr]) -> Expr {
    body.substitute(&|v: &str| v.strip_prefix('#').and_then(|k| k.parse::<usize>().ok()).and_then(|k| args.get(k).cloned()))
}

struct Expander<'a> {
    defs: &'a Defs,
    params: &'a Params,
    /// 積分の入れ子の深さ（2 重まで）
    int_depth: usize,
    /// 展開中の定義の名前（自分自身を使う定義を見つける）
    stack: Vec<String>,
}

/// 式を展開して、今までの節だけの式にする。新しい節を使っていない式は、そのまま返す
pub fn expand(e: &Expr, params: &Params) -> Result<Expr, String> {
    if !e.has_notation() && !mentions_defs(e) {
        return Ok(e.clone());
    }
    let defs = current();
    let mut x = Expander { defs: &defs, params, int_depth: 0, stack: Vec::new() };
    let out = match x.val(e)? {
        Val::S(s) => s,
        Val::Fn { .. } => return Err(fn_as_value_error(e)),
    };
    if out.node_count() > MAX_NODES {
        return Err("式が大きくなりすぎます（関数の入れ子・Σ の項の数を減らしてください）".into());
    }
    Ok(out)
}

/// 式が、定義された名前を（呼び出しでなく）使っているか
fn mentions_defs(e: &Expr) -> bool {
    let defs = current();
    if defs.is_empty() {
        return false;
    }
    let mut vs = Vec::new();
    e.collect_vars(&mut vs);
    vs.iter().any(|v| defs.get(v).is_some())
}

fn fn_as_value_error(e: &Expr) -> String {
    let mut vs = Vec::new();
    e.collect_vars(&mut vs);
    let defs = current();
    match vs.iter().find(|v| defs.get(v).is_some()) {
        Some(f) => format!("{f} は関数です。{f}(x) のように引数を書いてください"),
        None => "関数は、f(x) のように引数を書いて使ってください".into(),
    }
}

impl Expander<'_> {
    fn enter(&mut self, name: &str) -> Result<(), String> {
        if self.stack.iter().any(|n| n == name) {
            return Err(format!("{name} の定義が {name} 自身を使っています"));
        }
        if self.stack.len() >= MAX_DEPTH {
            return Err("関数の呼び出しの入れ子が深すぎます".into());
        }
        self.stack.push(name.to_string());
        note_used(name);
        Ok(())
    }

    /// 定義された名前の値（関数）。定義が無ければ None
    fn def_value(&mut self, name: &str) -> Result<Option<Val>, String> {
        let Some(def) = self.defs.get(name).cloned() else { return Ok(None) };
        self.enter(name)?;
        let v = match def {
            Def::Function { args, body } => {
                // 仮引数を #k に置き換え、本体を展開する（本体の中の呼び出しも、ここで展開される）
                let renamed = body.substitute(&|v: &str| args.iter().position(|a| a == v).map(|k| Expr::Var(arg_name(k))));
                match self.val(&renamed)? {
                    Val::S(b) => Val::Fn { arity: args.len(), body: b },
                    Val::Fn { .. } => {
                        self.stack.pop();
                        return Err(format!("{name} の値が関数になっています（{name}(x) = g(x) のように書いてください）"));
                    }
                }
            }
            Def::Value(e) => self.val(&e)?,
        };
        self.stack.pop();
        Ok(Some(v))
    }

    /// スカラーでなければならない所の値
    fn scalar(&mut self, e: &Expr) -> Result<Expr, String> {
        match self.val(e)? {
            Val::S(s) => Ok(s),
            Val::Fn { .. } => Err(fn_as_value_error(e)),
        }
    }

    /// 関数を引数に当てはめる
    fn apply(&self, name: &str, arity: usize, body: &Expr, args: Vec<Expr>) -> Result<Expr, String> {
        // 引数が組 1 つで、関数の引数が 2 つ以上なら、組の成分を引数にする（(f + g)((x, y)) の形）
        let args = match args.as_slice() {
            [Expr::Tuple(items)] if arity > 1 && items.len() == arity => items.clone(),
            _ => args,
        };
        if args.len() != arity {
            return Err(format!("{name} の引数は {arity} 個です（{} 個あります）", args.len()));
        }
        let out = apply_body(body, &args);
        if out.node_count() > MAX_NODES {
            return Err("式が大きくなりすぎます（関数の入れ子を減らしてください）".into());
        }
        Ok(out)
    }

    fn val(&mut self, e: &Expr) -> Result<Val, String> {
        Ok(match e {
            Expr::Num(_) | Expr::Const(_) => Val::S(e.clone()),
            Expr::Var(name) => match self.def_value(name)? {
                Some(v) => v,
                None => Val::S(e.clone()),
            },
            Expr::Apply { name, primes, args } => {
                let Some(f) = self.def_value(name)? else {
                    return Err(format!("{name} は定義されていません"));
                };
                let args = args.iter().map(|a| self.scalar(a)).collect::<Result<Vec<_>, _>>()?;
                match f {
                    Val::Fn { arity, body } => {
                        // f'(u): 本体を仮引数で微分してから当てはめる
                        let body = if *primes > 0 {
                            if arity != 1 {
                                return Err(format!("{name} は引数が {arity} 個なので、' ではなく ∂ で微分してください"));
                            }
                            crate::diff::diff_n(&body, &arg_name(0), *primes as usize)?
                        } else {
                            body
                        };
                        Val::S(self.apply(name, arity, &body, args)?)
                    }
                    Val::S(_) => return Err(format!("{name} は関数ではありません")),
                }
            }
            Expr::Neg(a) => match self.val(a)? {
                Val::S(s) => Val::S(Expr::Neg(Box::new(s))),
                Val::Fn { arity, body } => Val::Fn { arity, body: Expr::Neg(Box::new(body)) },
            },
            Expr::Bin(op, a, b) => {
                let (va, vb) = (self.val(a)?, self.val(b)?);
                self.bin(*op, va, vb, e)?
            }
            Expr::Call(f, args) => {
                let vals = args.iter().map(|a| self.val(a)).collect::<Result<Vec<_>, _>>()?;
                // 1 引数の関数に関数を渡したら合成（sin(f) = sin ∘ f）
                if let [Val::Fn { arity, body }] = vals.as_slice() {
                    return Ok(Val::Fn { arity: *arity, body: Expr::Call(*f, vec![body.clone()]) });
                }
                let mut out = Vec::with_capacity(vals.len());
                for v in vals {
                    match v {
                        Val::S(s) => out.push(s),
                        Val::Fn { .. } => return Err(fn_as_value_error(e)),
                    }
                }
                Val::S(Expr::Call(*f, out))
            }
            Expr::Tuple(items) => Val::S(Expr::Tuple(items.iter().map(|a| self.scalar(a)).collect::<Result<_, _>>()?)),
            Expr::List(items) if items.iter().any(is_ellipsis) => Val::S(self.arithmetic_list(items)?),
            Expr::List(items) => Val::S(Expr::List(items.iter().map(|a| self.scalar(a)).collect::<Result<_, _>>()?)),
            Expr::Op { op, args } if op == "pm" || op == "mp" => {
                // a ± b → a + s b、∓ は a − s b（s は +1 と −1。全部の ± がそろって動く）
                let sign = Expr::Var(PM_SIGN.into());
                let signed = |b: Expr| Expr::Bin(BinOp::Mul, Box::new(sign.clone()), Box::new(b));
                let minus = op == "mp";
                match args.as_slice() {
                    [b] => {
                        let b = signed(self.scalar(b)?);
                        Val::S(if minus { Expr::Neg(Box::new(b)) } else { b })
                    }
                    [a, b] => {
                        let (a, b) = (self.scalar(a)?, signed(self.scalar(b)?));
                        Val::S(Expr::Bin(if minus { BinOp::Sub } else { BinOp::Add }, Box::new(a), Box::new(b)))
                    }
                    _ => return Err("内部エラー: ± の形".into()),
                }
            }
            Expr::Op { op, args } if op == "cases" => Val::S(self.cases(args)?),
            Expr::Op { op, .. } if op == "ellipsis" => return Err("… はリスト [1, 2, …, 10] の中で使ってください".into()),
            Expr::Binder { op, var, args, body } if op == "sum" || op == "prod" => Val::S(self.sum_or_prod(op == "sum", var, args, body)?),
            Expr::Binder { op, var, args, body } if op == "int" => Val::S(self.integral(var, args, body)?),
            Expr::Binder { op, var, args, body } if op == "diff" => {
                let k = match args.as_slice() {
                    [] => 1,
                    [k] => {
                        let k = self.integer(k, "微分の回数")?;
                        if !(0..=8).contains(&k) {
                            return Err("微分の回数は 0〜8 にしてください".into());
                        }
                        k as usize
                    }
                    _ => return Err("微分は d/dx か d²/dx² の形で書いてください".into()),
                };
                match self.val(body)? {
                    Val::S(s) => Val::S(crate::diff::diff_n(&s, var, k)?),
                    // df/dx（f は引数 1 つの関数）は f'(x)
                    Val::Fn { arity: 1, body } => Val::S(apply_body(&crate::diff::diff_n(&body, &arg_name(0), k)?, &[Expr::Var(var.clone())])),
                    Val::Fn { .. } => return Err(fn_as_value_error(body)),
                }
            }
            Expr::Op { op, args } if op == "cross" => {
                // 外積はベクトル・行列の段階で入る。それまではスカラーの掛け算
                let (va, vb) = (self.val(&args[0])?, self.val(&args[1])?);
                self.bin(BinOp::Mul, va, vb, e)?
            }
            Expr::Op { op, args } if op == "compose" => {
                let (f, g) = (self.val(&args[0])?, self.val(&args[1])?);
                match (f, g) {
                    (Val::Fn { arity: 1, body: fb }, Val::Fn { arity, body: gb }) => {
                        Val::Fn { arity, body: apply_body(&fb, &[gb]) }
                    }
                    (Val::Fn { arity, body: fb }, Val::Fn { arity: ga, body: Expr::Tuple(items) }) if items.len() == arity => {
                        Val::Fn { arity: ga, body: apply_body(&fb, &items) }
                    }
                    (Val::Fn { .. }, Val::Fn { .. }) => return Err("合成 f ∘ g では、g の値の数と f の引数の数をそろえてください".into()),
                    _ => return Err("∘ の両側には関数を書いてください".into()),
                }
            }
            Expr::Op { op, .. } => return Err(format!("@{op} はまだ使えません")),
            Expr::Binder { op, .. } => return Err(format!("@{op} はまだ使えません")),
            Expr::Matrix { .. } => return Err("行列はまだ使えません".into()),
            Expr::Index { name, .. } => return Err(format!("{name}_(…) の添字はまだ使えません")),
            Expr::Prime { name, order } => return Err(format!("{name}{} はまだ使えません", "'".repeat(*order as usize))),
        })
    }

    /// 場合分けを [`crate::functions::Func::Piece`] の呼び出しにする。条件 `L < R` の余裕は R − L（厳密）、
    /// `L ≤ R` は R − L、`L = R` は −|L − R|。連鎖 `0 ≤ x < 1` と `,` で並べた条件は全部成り立つとき（かつ）
    fn cases(&mut self, cases: &[Expr]) -> Result<Expr, String> {
        use crate::functions::Func;
        let mut out = Vec::new();
        for (k, case) in cases.iter().enumerate() {
            let Expr::Op { args, .. } = case else { return Err("内部エラー: 場合分けの形".into()) };
            let value = self.scalar(&args[0])?;
            let mut margins = Vec::new();
            let mut strict: u64 = 0;
            for chain in &args[1..] {
                let Expr::Op { op, args: exprs } = chain else { return Err("内部エラー: 条件の形".into()) };
                let rels: Vec<char> = op.trim_start_matches("rel:").chars().collect();
                let exprs = exprs.iter().map(|e| self.scalar(e)).collect::<Result<Vec<_>, _>>()?;
                for (j, r) in rels.iter().enumerate() {
                    let (l, rr) = (exprs[j].clone(), exprs[j + 1].clone());
                    let d = |a: Expr, b: Expr| Expr::Bin(BinOp::Sub, Box::new(a), Box::new(b));
                    let (m, s) = match r {
                        '<' => (d(rr, l), true),
                        '≤' => (d(rr, l), false),
                        '>' => (d(l, rr), true),
                        '≥' => (d(l, rr), false),
                        _ => (Expr::Neg(Box::new(Expr::Call(Func::Abs, vec![d(l, rr)]))), false),
                    };
                    if margins.len() >= 50 {
                        return Err("場合分けの条件が多すぎます".into());
                    }
                    if s {
                        strict |= 1 << margins.len();
                    }
                    margins.push(m);
                }
            }
            if margins.is_empty() && k + 1 != cases.len() {
                return Err("条件の無い場合（それ以外）は、最後に書いてください".into());
            }
            out.push(Expr::Num(margins.len() as f64));
            out.push(Expr::Num(strict as f64));
            out.extend(margins);
            out.push(value);
        }
        Ok(Expr::Call(Func::Piece, out))
    }

    /// 定積分 $\int_a^b g(t)\,dt$ を、区間を 2 つに分けたガウス・ルジャンドルの重み付きの和にする（2 重目は 8 点 × 2）。
    /// 端 a, b は式のまま（x を使ってよい）。∞ の端は変数変換で有限の区間にする:
    /// $[a, \infty)$ は $t = a + \frac{s}{1-s}$、$(-\infty, b]$ は $t = b - \frac{s}{1-s}$（$s \in [0, 1)$）、
    /// $(-\infty, \infty)$ は $(-\infty, 0]$ と $[0, \infty)$ に分ける
    fn integral(&mut self, var: &str, args: &[Expr], body: &Expr) -> Result<Expr, String> {
        let [lo, hi] = args else { return Err("積分の範囲を書いてください（∫_0^1 の形）".into()) };
        if self.int_depth >= 2 {
            return Err("積分の入れ子は 2 重までにしてください".into());
        }
        let (lo, hi) = (self.scalar(lo)?, self.scalar(hi)?);
        let inf = |e: &Expr| match e {
            Expr::Num(v) if v.is_infinite() => Some(v.signum()),
            Expr::Neg(a) => match &**a {
                Expr::Num(v) if v.is_infinite() => Some(-v.signum()),
                _ => None,
            },
            _ => None,
        };
        let num = Expr::Num;
        let b = |op: BinOp, x: Expr, y: Expr| Expr::Bin(op, Box::new(x), Box::new(y));
        let s = Expr::Var("#s".into());
        let one_minus_s = b(BinOp::Sub, num(1.0), s.clone());
        // 置き換え: t = φ(s)、dt = φ'(s) ds。s の範囲 [s0, s1]
        let (phi, dphi, s0, s1) = match (inf(&lo), inf(&hi)) {
            (None, None) => (s.clone(), num(1.0), lo.clone(), hi.clone()),
            (None, Some(1.0)) | (Some(-1.0), None) => {
                // [a, ∞) と (−∞, b]
                let (base, dir) = if inf(&hi).is_some() { (lo.clone(), BinOp::Add) } else { (hi.clone(), BinOp::Sub) };
                let u = b(BinOp::Div, s.clone(), one_minus_s.clone());
                let jac = b(BinOp::Div, num(1.0), b(BinOp::Pow, one_minus_s.clone(), num(2.0)));
                (b(dir, base, u), jac, num(0.0), num(1.0))
            }
            (Some(-1.0), Some(1.0)) => {
                // (−∞, ∞) は (−∞, 0] と [0, ∞) に分ける（1 つの変換 t = s/(1−s²) より、ずっと誤差が小さい）
                let neg_inf = Expr::Neg(Box::new(num(f64::INFINITY)));
                let left = self.integral(var, &[neg_inf, num(0.0)], body)?;
                let right = self.integral(var, &[num(0.0), num(f64::INFINITY)], body)?;
                return Ok(b(BinOp::Add, left, right));
            }
            (Some(1.0), _) | (_, Some(-1.0)) => {
                return Err("積分の端の ∞ の向きを確かめてください（下の端に ∞、上の端に −∞ は書けません）".into())
            }
            _ => return Err("積分の範囲を確かめてください".into()),
        };
        let (nodes, weights) = gauss_legendre(if self.int_depth == 0 { 16 } else { 8 });
        // 区間 [s0, s1] を 2 つに分ける: 幅の半分 h、j 番目の中点 m_j
        let h = b(BinOp::Div, b(BinOp::Sub, s1, s0.clone()), num(2.0));
        let half_h = b(BinOp::Div, h.clone(), num(2.0));
        self.int_depth += 1;
        let mut terms = Vec::new();
        let mut nodes_count = 0;
        for j in 0..2 {
            let mid = b(BinOp::Add, s0.clone(), b(BinOp::Mul, num(j as f64 + 0.5), h.clone()));
            for (xi, w) in nodes.iter().zip(&weights) {
                let sk = b(BinOp::Add, mid.clone(), b(BinOp::Mul, num(*xi), half_h.clone()));
                let at_s = |e: &Expr| e.substitute(&|v: &str| (v == "#s").then(|| sk.clone()));
                let t = at_s(&phi);
                let g = body.substitute(&|v: &str| (v == var).then(|| t.clone()));
                let g = match self.scalar(&g) {
                    Ok(g) => g,
                    Err(e) => {
                        self.int_depth -= 1;
                        return Err(e);
                    }
                };
                let term = b(BinOp::Mul, num(*w), b(BinOp::Mul, g, at_s(&dphi)));
                nodes_count += term.node_count();
                if nodes_count > MAX_NODES {
                    self.int_depth -= 1;
                    return Err("積分の式が大きくなりすぎます（入れ子を減らしてください）".into());
                }
                terms.push(term);
            }
        }
        self.int_depth -= 1;
        Ok(b(BinOp::Mul, half_h, balanced(terms, BinOp::Add)))
    }

    /// 式の値（数）。`what` はエラーの文に入れる名前（「Σ の範囲」など）。座標・媒介変数を含めばエラー
    fn number(&mut self, e: &Expr, what: &str) -> Result<f64, String> {
        let s = self.scalar(e)?;
        let prog = crate::eval::Program::compile_with(&s, &[], self.params).map_err(|err| {
            match err.strip_prefix("変数 '").and_then(|r| r.split_once('\'')) {
                Some((name, _)) if self.params.contains_key(name) || !definable(name) => format!("{what}に {name} は使えません"),
                // 未定義の文字は、画面側の「スライダーを追加」ボタンが読める文言のままにする
                Some(_) => err,
                None => format!("{what}は、数とパラメータだけで書いてください"),
            }
        })?;
        USED_PARAMS.with(|u| {
            let mut u = u.borrow_mut();
            for p in prog.used_params() {
                if !u.contains(p) {
                    u.push(p.clone());
                }
            }
        });
        let mut out = [num_complex::Complex64::new(0.0, 0.0)];
        prog.eval_batch(&[], &mut out);
        let v = crate::eval::as_real(out[0]);
        if !v.is_finite() {
            return Err(format!("{what}が実数の値になりません"));
        }
        Ok(v)
    }

    /// 整数の値（整数から 10⁻⁹ より離れていればエラー）
    fn integer(&mut self, e: &Expr, what: &str) -> Result<i64, String> {
        let v = self.number(e, what)?;
        let r = v.round();
        if (v - r).abs() > 1e-9 {
            return Err(format!("{what}は整数にしてください（今は {v}。スライダーなら、刻みを 1 にしてください）"));
        }
        Ok(r as i64)
    }

    /// Σ・Π: 束縛する変数に a, a + 1, …, b を入れた中身を、平衡のとれた木で足す・掛ける。b < a なら 0（Π は 1）
    fn sum_or_prod(&mut self, sum: bool, var: &str, args: &[Expr], body: &Expr) -> Result<Expr, String> {
        let sym = if sum { "Σ" } else { "Π" };
        let [lo, hi] = args else {
            return Err(format!("{sym} は、範囲を {sym}_{{{var}=1}}^{{10}} の形で書いてください"));
        };
        let what = format!("{sym} の範囲");
        let (a, b) = (self.integer(lo, &what)?, self.integer(hi, &what)?);
        if b < a {
            return Ok(Expr::Num(if sum { 0.0 } else { 1.0 }));
        }
        if (b - a) as u64 >= MAX_TERMS as u64 {
            return Err(format!("{sym} の項が多すぎます（{} 個。{MAX_TERMS} 個まで）", b - a + 1));
        }
        let mut terms = Vec::with_capacity((b - a + 1) as usize);
        let mut nodes = 0;
        for k in a..=b {
            let t = body.substitute(&|v: &str| (v == var).then_some(Expr::Num(k as f64)));
            let t = self.scalar(&t)?;
            nodes += t.node_count();
            if nodes > MAX_NODES {
                return Err(format!("{sym} の式が大きくなりすぎます（項の数を減らしてください）"));
            }
            terms.push(t);
        }
        Ok(balanced(terms, if sum { BinOp::Add } else { BinOp::Mul }))
    }

    /// `[a, …, c]`（公差 1）・`[a, b, …, c]`（公差 b − a）を、数の並びにする
    fn arithmetic_list(&mut self, items: &[Expr]) -> Result<Expr, String> {
        let what = "… のリストの値";
        let (first, step, last) = match items {
            [a, e, c] if is_ellipsis(e) && !is_ellipsis(a) && !is_ellipsis(c) => (self.number(a, what)?, 1.0, self.number(c, what)?),
            [a, b, e, c] if is_ellipsis(e) && ![a, b, c].iter().any(|x| is_ellipsis(x)) => {
                let (a, b) = (self.number(a, what)?, self.number(b, what)?);
                (a, b - a, self.number(c, what)?)
            }
            _ => return Err("… は [1, 2, …, 10] か [1, …, 10] の形で使ってください".into()),
        };
        if step == 0.0 || (last - first) / step < -1e-9 {
            return Err("… のリストが終わりません（並びの向きと最後の値を確かめてください）".into());
        }
        let n = ((last - first) / step + 1e-9).floor() as usize + 1;
        if n > MAX_LIST {
            return Err(format!("リストの要素が多すぎます（{n} 個。{MAX_LIST} 個まで）"));
        }
        Ok(Expr::List((0..n).map(|k| Expr::Num(first + step * k as f64)).collect()))
    }

    /// 2 項の演算。関数どうしは点ごと、関数 × 式は当てはめ（`(f + g)(x)`・`f x`）
    fn bin(&mut self, op: BinOp, a: Val, b: Val, whole: &Expr) -> Result<Val, String> {
        let mk = |a: Expr, b: Expr| Expr::Bin(op, Box::new(a), Box::new(b));
        Ok(match (a, b) {
            (Val::S(a), Val::S(b)) => Val::S(mk(a, b)),
            (Val::Fn { arity, body }, Val::S(arg)) if op == BinOp::Mul => {
                Val::S(self.apply("この関数", arity, &body, vec![arg])?)
            }
            (Val::Fn { arity: n, body: fa }, Val::Fn { arity: m, body: fb }) => {
                if n != m {
                    return Err(format!("引数の数が違う関数どうし（{n} 個と {m} 個）は計算できません"));
                }
                Val::Fn { arity: n, body: mk(fa, fb) }
            }
            (Val::Fn { arity, body }, Val::S(c)) => {
                if op == BinOp::Pow {
                    if matches!(&c, Expr::Neg(x) if matches!(**x, Expr::Num(n) if n == 1.0)) {
                        return Err("逆関数 f⁻¹ は、y = f⁻¹(x) の形（片側がそれだけ）でだけ使えます".into());
                    }
                    return Err("関数のべき乗 f² は使えません（f(x)^2 と書いてください）".into());
                }
                Val::Fn { arity, body: mk(body, c) }
            }
            (Val::S(c), Val::Fn { arity, body }) => {
                if op == BinOp::Pow {
                    return Err(fn_as_value_error(whole));
                }
                Val::Fn { arity, body: mk(c, body) }
            }
        })
    }
}

/// ガウス・ルジャンドルの n 点の節点と重み（区間 [−1, 1]）。ルジャンドル多項式 $P_n$ の根をニュートン法で求める
pub fn gauss_legendre(n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut xs = vec![0.0; n];
    let mut ws = vec![0.0; n];
    for i in 0..n {
        // 根の初期値（チェビシェフの近似）
        let mut x = (std::f64::consts::PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        let mut dp = 0.0;
        for _ in 0..100 {
            // P_n(x) と P_n'(x) を漸化式で
            let (mut p0, mut p1) = (1.0, x);
            for k in 2..=n {
                let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                p0 = p1;
                p1 = p2;
            }
            dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
            let dx = p1 / dp;
            x -= dx;
            if dx.abs() < 1e-16 {
                break;
            }
        }
        xs[i] = x;
        ws[i] = 2.0 / ((1.0 - x * x) * dp * dp);
    }
    (xs, ws)
}

fn is_ellipsis(e: &Expr) -> bool {
    matches!(e, Expr::Op { op, .. } if op == "ellipsis")
}

/// 項を、深さが log₂ になるように組んで足す・掛ける（左に偏った木にしない）
fn balanced(mut terms: Vec<Expr>, op: BinOp) -> Expr {
    while terms.len() > 1 {
        let mut next = Vec::with_capacity(terms.len().div_ceil(2));
        let mut it = terms.into_iter();
        while let Some(a) = it.next() {
            next.push(match it.next() {
                Some(b) => Expr::Bin(op, Box::new(a), Box::new(b)),
                None => a,
            });
        }
        terms = next;
    }
    terms.pop().unwrap_or(Expr::Num(0.0))
}

/// 等式の片側が `f^(-1)(式)`（f は引数 1 つの関数）だけなら、`f(反対側) = 式` に書き換える（逆関数）
pub fn rewrite_inverse(l: Expr, r: Expr) -> (Expr, Expr) {
    let defs = current();
    let inv = |e: &Expr| -> Option<(String, Expr)> {
        let Expr::Bin(BinOp::Mul, a, arg) = e else { return None };
        let Expr::Bin(BinOp::Pow, f, k) = &**a else { return None };
        let Expr::Var(name) = &**f else { return None };
        let minus_one = matches!(&**k, Expr::Neg(x) if matches!(**x, Expr::Num(n) if n == 1.0)) || matches!(**k, Expr::Num(n) if n == -1.0);
        let one_arg = match defs.get(name) {
            Some(Def::Function { args, .. }) => args.len() == 1,
            Some(Def::Value(_)) => true,
            None => false,
        };
        (minus_one && one_arg).then(|| (name.clone(), (**arg).clone()))
    };
    if let Some((f, arg)) = inv(&l) {
        return (Expr::Apply { name: f, primes: 0, args: vec![r] }, arg);
    }
    if let Some((f, arg)) = inv(&r) {
        return (arg, Expr::Apply { name: f, primes: 0, args: vec![l] });
    }
    (l, r)
}

/// 定義の行（左辺が `f(x, y)`（定義と同じ仮引数）か、引数の無い定義の名前）なら、(名前, 引数の数, 値が組か)。
/// 展開できない定義（自分自身を使うなど）は、引数の数だけ返す（描くときに誤りが出る）
pub fn definition_line(l: &Expr) -> Option<(String, usize, bool)> {
    let defs = current();
    let name = match l {
        Expr::Apply { name, primes: 0, args } => match defs.get(name) {
            Some(Def::Function { args: formal, .. })
                if args.len() == formal.len() && args.iter().zip(formal).all(|(a, f)| matches!(a, Expr::Var(v) if v == f)) =>
            {
                name
            }
            _ => return None,
        },
        Expr::Var(name) if matches!(defs.get(name), Some(Def::Value(_))) => name,
        _ => return None,
    };
    let params = Params::new();
    let mut x = Expander { defs: &defs, params: &params, int_depth: 0, stack: Vec::new() };
    Some(match x.def_value(name) {
        Ok(Some(Val::Fn { arity, body })) => (name.clone(), arity, matches!(body, Expr::Tuple(_))),
        _ => (
            name.clone(),
            match defs.get(name) {
                Some(Def::Function { args, .. }) => args.len(),
                _ => 0,
            },
            false,
        ),
    })
}

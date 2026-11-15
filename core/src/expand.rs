//! 展開の段: 数学の記法（計画 `docs/plan-math-notation.md`）を、今までの式（`Expr` の 8 つの節だけ）に直す。
//!
//! ```text
//! 文字列 → 構文木（新しい節あり） → [この段] → 構文木（今までの節だけ） → 今までの道（陽関数・陰関数・…）
//! ```
//!
//! - 関数の定義 `f(x) = x^2` と、その呼び出し `f(2x)`・合成 `g(f(x))`・関数どうしの演算 `(f + g)(x)`・`(f ∘ g)(x)`（M1）
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
    /// 展開中の定義の名前（自分自身を使う定義を見つける）
    stack: Vec<String>,
}

/// 式を展開して、今までの節だけの式にする。新しい節を使っていない式は、そのまま返す
pub fn expand(e: &Expr, _params: &Params) -> Result<Expr, String> {
    if !e.has_notation() && !mentions_defs(e) {
        return Ok(e.clone());
    }
    let defs = current();
    let mut x = Expander { defs: &defs, stack: Vec::new() };
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
                if *primes > 0 {
                    return Err(format!("{name}{} はまだ使えません（微分は次の段階で入ります）", "'".repeat(*primes as usize)));
                }
                let Some(f) = self.def_value(name)? else {
                    return Err(format!("{name} は定義されていません"));
                };
                let args = args.iter().map(|a| self.scalar(a)).collect::<Result<Vec<_>, _>>()?;
                match f {
                    Val::Fn { arity, body } => Val::S(self.apply(name, arity, &body, args)?),
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
            Expr::List(items) => Val::S(Expr::List(items.iter().map(|a| self.scalar(a)).collect::<Result<_, _>>()?)),
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
    let mut x = Expander { defs: &defs, stack: Vec::new() };
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

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
//! - 差分 `@fdiff(x) 項`（前進 Δ）・`@bdiff`（後退 ∇）・`@cdiff`（中心 δ）。`@fdiff(x, h) 項` で幅 h（既定 1。式でよく、x を使ってもよい）、
//!   `@fdiff(x, h, k) 項` で k 階（総和 Σ の逆。M4 の続き）
//! - 定積分 `@int(t, a, b)(式)`（M5。ガウス・ルジャンドルの 16 点 × 2 区間の重み付きの和にする。端は式でも ∞ でもよい）。
//!   3 重まで（入れ子の 2 重目は 8 点、3 重目は 4 点 × 2 区間）
//! - 不定積分 `@int(x)(式)`（積分定数を省いた形。表で扱える形は記号で `integrate.rs`、扱えなければ数値で $\int_0^x$）
//! - ベクトル（組）の計算: 和・差・スカラー倍・内積（`*`）・外積（`×`）・大きさ（`abs`）。勾配 `@grad f`・発散 `@div F`・
//!   回転 `@rot F`（`@curl`）・ラプラシアン `@lap f`（座標は x, y, z）
//! - 線積分・面積分・体積分: 点を媒介変数の式で書いた `@intset((cos(t), sin(t), 0), 0 ≤ t ≤ 2pi)(f)`（弧長 ds・面積 dS・体積 dV）と、
//!   `@intset(…; {vec})(F)`（F · dr・F · dS）。`; {d_s}`・`; {d_x}, {d_y}` は積分の記号の d の後の文字（範囲の次元と合うかを確かめ、
//!   曲線の上の dx は x′(t) を掛ける）。変数の組の範囲の等式 `@intset((x, y, z), x = y = z, 0 ≤ x ≤ 1)(f)` は、解いて曲線・曲面にする。
//!   1 次式に解けない等式 `@intset((x, y), x^2 + y^2 = 1)(f)` は、媒介変数を作らずに点と重みを数値で求める（`implicit.rs`）
//! - 領域の上の重積分 `@intset((x, y), 0 ≤ x ≤ 1, x^2 ≤ y ≤ x)(式)`（条件から累次積分の順番と範囲を見つける。
//!   範囲の形にならない条件は、条件の外を 0 にした近似。変数ごとの範囲が無くても、1 次式の範囲 `−1 ≤ x + y ≤ 1, 0 ≤ x − y ≤ 1` なら
//!   変数変換（ヤコビアン 1/|det|）で累次積分にする）
//!
//! 定義は、ほかの行（式の一覧）から [`Defs::parse`] で作り、[`install`] で入れておく（`lib.rs` が `plot` の間だけ入れる）。
//! 計算エンジンの式の文法は `parser.rs` の先頭の説明。展開した後の式には、新しい節を残さない（残れば内部エラー）

use crate::eval::Params;
use crate::parser::{definition_head, parse_system_with, BinOp, Expr, Rel};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::rc::Rc;

pub mod linalg;
use linalg::{cross, norm, Mat};

/// 展開した式の節の数の上限（計画 2.5）。球座標で書いた球の体積積分（3 重・体積要素 |det J|）が約 132 万節（展開とコンパイルで 0.4 秒ほど）。
/// これより大きい式は、メモリ（WASM で数十 MB）と時間が重くなるので断る
pub const MAX_NODES: usize = 1_500_000;
/// Σ・Π の項の数の上限
const MAX_TERMS: usize = 2_000;
const LIMIT_INDEX_SAMPLE: &str = "数列の添字を標本ごとに評価します";
const LIMIT_INTEGRAL_SAMPLE: &str = "積分を標本ごとに評価します";

fn uses_internal_var(expr: &Expr, except: Option<&str>) -> bool {
    let mut vars = Vec::new();
    expr.collect_vars(&mut vars);
    vars.iter().any(|v| v.starts_with('#') && Some(v.as_str()) != except)
}

fn arithmetic_sample_error(error: &str) -> bool {
    ["左極限と右極限が一致しません", "振動のため", "極限の収束を数値計算で確認できません", "積分が収束しない"]
        .iter().any(|prefix| error.starts_with(prefix))
}
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
    /// 集合の定義 `S = {(x, y) | …}`・`S = A ∪ B`（計画 M11）
    Set(crate::parser::SetE),
}

/// 同じ数列に、偶数項と奇数項など複数の規則を書ける。
#[derive(Debug, Clone, Default)]
pub struct Seq {
    rules: Vec<SequenceRule>,
}

#[derive(Debug, Clone)]
struct SequenceRule {
    index: SequenceIndex,
    body: Expr,
    recursive: bool,
    has_cases: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequenceIndex {
    var: String,
    step: i64,
    shift: i64,
}

impl SequenceIndex {
    fn parameter(&self, k: i64) -> Option<i64> {
        let delta = k.checked_sub(self.shift)?;
        if delta.checked_rem(self.step)? != 0 {
            return None;
        }
        delta.checked_div(self.step)
    }
}

#[derive(Debug)]
enum SequenceInverse {
    Root(f64),
    Exp,
    TenToPower,
    Divide(Expr),
    MultiplyByDivisor(Expr),
}

#[derive(Debug)]
pub(crate) struct SequenceHead {
    pub(crate) name: String,
    index: SequenceIndex,
    inverses: Vec<SequenceInverse>,
}

impl SequenceHead {
    pub(crate) fn argument_at(&self, k: i64, params: &Params) -> Option<i64> {
        if params.contains_key(&format!("{}{k}", self.name)) {
            return None;
        }
        self.index.parameter(k)
    }

    pub(crate) fn uses_principal_root(&self) -> bool {
        self.inverses.iter().any(|inverse| matches!(inverse, SequenceInverse::Root(p) if *p > 1.0))
    }
}

/// 定義の表（名前 → 定義）
#[derive(Debug, Clone, Default)]
pub struct Defs {
    map: BTreeMap<String, Def>,
    /// 数列（名前は座標と同じでもよい: `z_(n+1) = z_(n)^2 + c`。ふつうの定義とは別の表）
    seqs: BTreeMap<String, Seq>,
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
                if uses_function_value(&chain.exprs[1], &names) || has_vector_or_matrix(&chain.exprs[1]) {
                    names.push(name.clone());
                    values.push((k, name.clone()));
                }
            }
            if values.len() == before {
                break;
            }
        }
        let mut map = BTreeMap::new();
        // 集合の定義 `S = …`（右辺に論理・集合の記号がある行）
        for line in &lines {
            let Some((name, rhs)) = line.split_once('=') else { continue };
            let name = name.trim().trim_start_matches('{').trim_end_matches('}');
            let simple = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric());
            if !simple || !definable(name) || params.contains_key(name) || names.iter().any(|n| n == name) {
                continue;
            }
            if !rhs.chars().any(|c| crate::parser::LOGIC_CHARS.contains(c)) {
                continue;
            }
            if let Some(set) = crate::parser::parse_formula(rhs, &names).ok().as_ref().and_then(crate::logic::as_set) {
                map.insert(name.to_string(), Def::Set(set));
            }
        }
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
        // 数列 `a_(n) = 式`・`a_(n+1) = 式`
        let mut seqs = BTreeMap::new();
        for line in &lines {
            let Ok(chains) = parse_system_with(line, &names) else { continue };
            let [chain] = chains.as_slice() else { continue };
            if chain.rels != [Rel::Eq] {
                continue;
            }
            let Some(head) = sequence_head(&chain.exprs[0]) else { continue };
            let mut body = chain.exprs[1].clone();
            for inverse in head.inverses {
                body = match inverse {
                    SequenceInverse::Root(1.0) => body,
                    SequenceInverse::Root(2.0) => Expr::Call(crate::functions::Func::Sqrt, vec![body]),
                    SequenceInverse::Root(p) => power(body, Expr::Num(1.0 / p)),
                    SequenceInverse::Exp => Expr::Call(crate::functions::Func::Exp, vec![body]),
                    SequenceInverse::TenToPower => power(Expr::Num(10.0), body),
                    SequenceInverse::Divide(factor) => Expr::Bin(BinOp::Div, Box::new(body), Box::new(factor)),
                    SequenceInverse::MultiplyByDivisor(divisor) => undo_sequence_division(body, divisor),
                };
            }
            let (recursive, has_cases) = sequence_body_features(&body, &head.name);
            seqs.entry(head.name).or_insert_with(Seq::default).rules.push(SequenceRule { index: head.index, body, recursive, has_cases });
        }
        Defs { map, seqs }
    }

    /// 定義された名前（解析器に渡して `f(x)` を呼び出しとして読ませる）
    pub fn names(&self) -> Vec<String> {
        self.map.keys().cloned().collect()
    }

    pub fn get(&self, name: &str) -> Option<&Def> {
        self.map.get(name)
    }

    pub fn seq(&self, name: &str) -> Option<&Seq> {
        self.seqs.get(name)
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty() && self.seqs.is_empty()
    }
}

fn power(base: Expr, exponent: Expr) -> Expr {
    Expr::Bin(BinOp::Pow, Box::new(base), Box::new(exponent))
}

fn undo_sequence_division(body: Expr, divisor: Expr) -> Expr {
    use crate::functions::Func;
    let product = |divisor| Expr::Bin(BinOp::Mul, Box::new(body), Box::new(divisor));
    if let Expr::Num(value) = divisor {
        return if value != 0.0 && value.is_finite() { product(divisor) } else { Expr::Num(f64::NAN) };
    }
    // Multiplying by the denominator only preserves the equation where it is nonzero.
    let nonzero = Expr::Op { op: "rel:>".into(), args: vec![Expr::Call(Func::Abs, vec![divisor.clone()]), Expr::Num(0.0)] };
    let case = Expr::Op { op: "case".into(), args: vec![product(divisor), nonzero] };
    Expr::Op { op: "sequence_division".into(), args: vec![case] }
}

/// 左辺の添字を、整数係数の 1 次式として読む。
fn sequence_index(e: &Expr, var: &str) -> Option<(i64, i64)> {
    match e {
        Expr::Var(v) if v == var => Some((1, 0)),
        Expr::Num(n) if n.fract() == 0.0 && n.abs() <= (1_i64 << 53) as f64 => Some((0, *n as i64)),
        Expr::Neg(a) => {
            let (m, b) = sequence_index(a, var)?;
            Some((m.checked_neg()?, b.checked_neg()?))
        }
        Expr::Bin(op, a, b) => {
            let (am, ab) = sequence_index(a, var)?;
            let (bm, bb) = sequence_index(b, var)?;
            match op {
                BinOp::Add => Some((am.checked_add(bm)?, ab.checked_add(bb)?)),
                BinOp::Sub => Some((am.checked_sub(bm)?, ab.checked_sub(bb)?)),
                BinOp::Mul if am == 0 => Some((ab.checked_mul(bm)?, ab.checked_mul(bb)?)),
                BinOp::Mul if bm == 0 => Some((bb.checked_mul(am)?, bb.checked_mul(ab)?)),
                BinOp::Div if bm == 0 && am.checked_rem(bb)? == 0 && ab.checked_rem(bb)? == 0 => Some((am.checked_div(bb)?, ab.checked_div(bb)?)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// a_(2n+1)、a_(n+1)^2、log(a_(n+1)) などの左辺。
pub(crate) fn sequence_head(mut e: &Expr) -> Option<SequenceHead> {
    use crate::functions::Func;
    let mut inverses = Vec::new();
    loop {
        match e {
            Expr::Bin(BinOp::Pow, base, exp) => match &**exp {
                Expr::Num(p) if *p > 0.0 && p.fract() == 0.0 => {
                    inverses.push(SequenceInverse::Root(*p));
                    e = base;
                }
                _ => return None,
            },
            Expr::Call(f @ (Func::Ln | Func::Log10), args) => {
                let [arg] = args.as_slice() else { return None };
                inverses.push(if *f == Func::Ln { SequenceInverse::Exp } else { SequenceInverse::TenToPower });
                e = arg;
            }
            Expr::Bin(BinOp::Mul, ..) => {
                let mut head = sequence_product_head(e)?;
                inverses.append(&mut head.inverses);
                head.inverses = inverses;
                return Some(head);
            }
            Expr::Bin(BinOp::Div, numerator, divisor) => {
                inverses.push(SequenceInverse::MultiplyByDivisor(divisor.as_ref().clone()));
                e = numerator;
            }
            Expr::Index { name, index } => {
                let mut vars = Vec::new();
                index.collect_vars(&mut vars);
                let [var] = vars.as_slice() else { return None };
                if crate::equation::coord_index(var).is_some() {
                    return None;
                }
                let (step, shift) = sequence_index(index, var)?;
                return (step != 0).then(|| SequenceHead {
                    name: name.clone(),
                    index: SequenceIndex { var: var.clone(), step, shift },
                    inverses,
                });
            }
            _ => return None,
        }
    }
}

fn sequence_product_head(product: &Expr) -> Option<SequenceHead> {
    let mut pending = vec![product];
    let mut factors = Vec::new();
    let mut target: Option<(usize, SequenceHead)> = None;
    let mut tied = false;
    while let Some(factor) = pending.pop() {
        if let Expr::Bin(BinOp::Mul, left, right) = factor {
            pending.extend([right.as_ref(), left.as_ref()]);
            continue;
        }
        if let Some(head) = sequence_head(factor) {
            if let Some((_, best)) = &target {
                // Parallel indices of the same sequence have a fixed order for every n.
                if head.name != best.name || head.index.var != best.index.var || head.index.step != best.index.step {
                    return None;
                }
                if head.index.shift <= best.index.shift {
                    tied |= head.index.shift == best.index.shift;
                    factors.push(factor);
                    continue;
                }
            }
            target = Some((factors.len(), head));
            tied = false;
        } else if !matches!(factor, Expr::Num(_) | Expr::Const(_) | Expr::Var(_))
            && !matches!(factor, Expr::Index { index, .. } if matches!(**index, Expr::Num(_)))
        {
            return None;
        }
        factors.push(factor);
    }
    let (target, mut head) = target?;
    if tied {
        return None;
    }
    // Divide out the other factors before undoing the target's own power or log.
    head.inverses.splice(0..0, factors.into_iter().enumerate()
        .filter(|(i, _)| *i != target)
        .map(|(_, factor)| SequenceInverse::Divide(factor.clone())));
    Some(head)
}

fn sequence_body_features(body: &Expr, name: &str) -> (bool, bool) {
    let found = std::cell::Cell::new(false);
    let has_cases = std::cell::Cell::new(false);
    let _ = body.rewrite(&|e| {
        let same = match e {
            Expr::Index { name: n, .. } => n == name,
            Expr::Var(n) => split_subscript(n).is_some_and(|(base, _)| base == name),
            _ => false,
        };
        if same {
            found.set(true);
        }
        if matches!(e, Expr::Op { op, .. } if op == "cases") {
            has_cases.set(true);
        }
        None
    });
    (found.get(), has_cases.get())
}

fn initial_index(name: &str, param: &str) -> Option<i64> {
    let suffix = param.strip_prefix(name)?;
    let k: i64 = suffix.parse().ok()?;
    (k >= 0 && suffix == k.to_string()).then_some(k)
}

/// 数列の初項の添字（パラメータ a1 などのうち、いちばん小さい番号）。
pub fn first_term(name: &str, params: &Params) -> Option<i64> {
    params.keys().filter_map(|param| initial_index(name, param)).min()
}

pub(crate) fn sequence_start(head: &SequenceHead, params: &Params) -> i64 {
    let defs = current();
    if let Some(seq) = defs.seq(&head.name) {
        if seq.rules.iter().any(|rule| rule.recursive) {
            return first_term(&head.name, params).unwrap_or_else(|| i64::from(!seq.rules.iter().any(|rule| rule.has_cases)));
        }
    }
    0
}

const MAX_SEQUENCE_PLOT_STEPS: i64 = 512;

fn sequence_plot_start<'a>(
    expander: &mut Expander<'_>,
    start: &'a Expr,
    start_error: &str,
) -> Result<(&'a str, i64), String> {
    let (name, first) = match start {
        Expr::Var(v) => {
            let (name, digits) = split_subscript(v).ok_or(start_error)?;
            (name, digits.parse::<i64>().map_err(|_| "開始項の添字が大きすぎます")?)
        }
        Expr::Index { name, index } => (name.as_str(), expander.integer(index, "開始項の添字")?),
        _ => return Err(start_error.into()),
    };
    if expander.defs.seq(name).is_none() {
        return Err(format!("数列 {name} の定義を追加してください"));
    }
    Ok((name, first))
}

fn sequence_plot_term(expander: &mut Expander<'_>, name: &str, k: i64) -> Result<Option<num_complex::Complex64>, String> {
    let expr = expander.term_value(name, k)?;
    Ok(expander.constant_value(&expr))
}

fn sequence_plot_value(expander: &mut Expander<'_>, name: &str, k: i64, label: &str) -> Result<f64, String> {
    let value = sequence_plot_term(expander, name, k)?.map(crate::eval::as_real);
    value.filter(|v| (*v as f32).is_finite())
        .ok_or_else(|| format!("{label}には有限の実数の項が必要です（{name}_{k}）"))
}

fn sequence_complex_value(expander: &mut Expander<'_>, name: &str, k: i64, label: &str) -> Result<num_complex::Complex64, String> {
    sequence_plot_term(expander, name, k)?
        .filter(|v| (v.re as f32).is_finite() && (v.im as f32).is_finite())
        .ok_or_else(|| format!("{label}には描画できる範囲の有限な項が必要です（{name}_{k}）"))
}

/// 開始項から、階段図に必要な連続した実数の項を求める。
pub(crate) fn cobweb_terms(start: &Expr, steps: &Expr, params: &Params) -> Result<Vec<f64>, String> {
    let defs = current();
    let mut expander = Expander::new(&defs, params);
    let start_error = "階段図の最初の引数は a_1 や a_(N) のように数列の項を書いてください";
    let (name, first) = sequence_plot_start(&mut expander, start, start_error)?;
    let steps = expander.integer(steps, "階段図の反復回数")?;
    if !(1..=MAX_SEQUENCE_PLOT_STEPS).contains(&steps) {
        return Err(format!("階段図の反復回数は 1〜{MAX_SEQUENCE_PLOT_STEPS} にしてください"));
    }
    let last = first.checked_add(steps).ok_or("最後の項の添字が大きすぎます")?;
    (first..=last).map(|k| sequence_plot_value(&mut expander, name, k, "階段図")).collect()
}

/// 各数列の開始項を座標にする。1 列の複素数列は実部と虚部を軸にする。
pub(crate) fn orbit_points(starts: &[Expr], steps: &Expr, dots: bool, params: &Params) -> Result<(Vec<[f64; 3]>, bool), String> {
    let defs = current();
    let mut expander = Expander::new(&defs, params);
    let start_error = "軌道の座標には a_0 や a_(N) のように数列の項を書いてください";
    let starts = starts.iter()
        .map(|start| sequence_plot_start(&mut expander, start, start_error))
        .collect::<Result<Vec<_>, _>>()?;
    let steps = orbit_steps(&mut expander, steps, dots)?;
    if starts.iter().any(|&(_, first)| first.checked_add(steps).is_none()) {
        return Err("最後の項の添字が大きすぎます".into());
    }
    if let [(name, first)] = starts.as_slice() {
        let mut complex = false;
        let points = (0..=steps).map(|offset| {
            let v = sequence_complex_value(&mut expander, name, first + offset, "軌道")?;
            let im = if crate::eval::as_real(v).is_finite() { 0.0 } else { v.im };
            complex |= im != 0.0;
            Ok([v.re, im, 0.0])
        }).collect::<Result<_, String>>()?;
        return Ok((points, complex));
    }
    let points = (0..=steps).map(|offset| {
        let mut point = [0.0; 3];
        for (coordinate, &(name, first)) in point.iter_mut().zip(&starts) {
            *coordinate = sequence_plot_value(&mut expander, name, first + offset, "軌道")?;
        }
        Ok(point)
    }).collect::<Result<_, String>>()?;
    Ok((points, false))
}

fn orbit_steps(expander: &mut Expander<'_>, steps: &Expr, dots: bool) -> Result<i64, String> {
    let steps = expander.integer(steps, "軌道の反復回数")?;
    let max_steps = if dots { 8_192 } else { MAX_SEQUENCE_PLOT_STEPS };
    if !(0..=max_steps).contains(&steps) {
        return Err(format!("軌道の反復回数は 0〜{max_steps} にしてください"));
    }
    Ok(steps)
}

pub(crate) fn matrix_orbit(matrix: &Expr, start: &[Expr], steps: &Expr, dots: bool, params: &Params) -> Result<Vec<[f64; 3]>, String> {
    let defs = current();
    let mut expander = Expander::new(&defs, params);
    let mat = expander.scalar(matrix)?;
    let mat = Mat::of(&mat).filter(|m| m.rows == start.len() && m.cols == start.len())
        .ok_or("軌道の行列は、初期点と同じ次元の 2×2 または 3×3 の正方行列にしてください")?;
    let steps = orbit_steps(&mut expander, steps, dots)?;
    let weights: Vec<_> = mat.items.iter().map(|v| expander.number(v, "軌道の行列の成分")).collect::<Result<_, _>>()?;
    let mut point = [0.0; 3];
    for (p, expr) in point.iter_mut().zip(start) {
        *p = expander.number(expr, "軌道の初期点")?;
    }
    let mut points = Vec::with_capacity(steps as usize + 1);
    for _ in 0..=steps {
        if point.iter().any(|p| !(*p as f32).is_finite()) {
            return Err("軌道に描画できる範囲を超えた項があります（反復回数を減らしてください）".into());
        }
        points.push(point);
        let mut next = [0.0; 3];
        for (p, row) in next.iter_mut().zip(weights.chunks_exact(start.len())) {
            *p = row.iter().zip(&point).map(|(a, x)| a * x).sum();
        }
        point = next;
    }
    Ok(points)
}

/// 各添字に対応する頂点を (添字, 実部, 虚部) に置く。
pub(crate) fn recurrence_nodes(start: &Expr, count: &Expr, distance: &Expr, params: &Params) -> Result<(Vec<[f64; 3]>, f64), String> {
    let defs = current();
    let mut expander = Expander::new(&defs, params);
    let start_error = "リカレンスネットワークの開始項は a_0 などの数列の項を書いてください";
    let (name, first) = sequence_plot_start(&mut expander, start, start_error)?;
    let count = expander.integer(count, "リカレンスネットワークの項数")?;
    if !(1..=128).contains(&count) {
        return Err("リカレンスネットワークの項数は 1〜128 にしてください".into());
    }
    let distance = expander.number(distance, "リカレンスネットワークの距離")?;
    if distance < 0.0 {
        return Err("リカレンスネットワークの距離は 0 以上にしてください".into());
    }
    let last = first.checked_add(count - 1).ok_or("最後の項の添字が大きすぎます")?;
    let nodes = (first..=last).map(|k| {
        let value = sequence_complex_value(&mut expander, name, k, "リカレンスネットワーク")?;
        Ok([k as f64, value.re, value.im])
    }).collect::<Result<Vec<_>, String>>()?;
    Ok((nodes, distance))
}

/// 二変数関数 f(x, r) を、各 r について同じ初期値から反復する。
pub(crate) fn bifurcation_points(map: &str, start: &Expr, lo: &Expr, hi: &Expr, params: &Params) -> Result<Vec<[f64; 3]>, String> {
    const COLUMNS: usize = 401;
    const BURN_IN: usize = 400;
    const SAMPLES: usize = 48;
    let defs = current();
    let mut expander = Expander::new(&defs, params);
    let start = expander.number(start, "分岐図の初期値")?;
    let lo = expander.number(lo, "分岐図のパラメータの下限")?;
    let hi = expander.number(hi, "分岐図のパラメータの上限")?;
    if lo >= hi || !(lo as f32).is_finite() || !(hi as f32).is_finite() {
        return Err("分岐図の範囲は、描画できる値で下限を上限より小さくしてください".into());
    }
    let vars = ["#state", "#parameter"];
    let call = Expr::Apply { name: map.into(), primes: 0, args: vars.iter().map(|&v| Expr::Var(v.into())).collect() };
    let expr = expander.scalar(&call)?;
    if expr.node_count() > 512 {
        return Err("分岐図の写像の式が大きすぎます".into());
    }
    let program = crate::eval::Program::compile_with(&expr, &vars, params)?;
    record_used_params(program.used_params());
    let controls: Vec<_> = (0..COLUMNS).map(|k| lo + (hi - lo) * (k as f64 / (COLUMNS - 1) as f64)).collect();
    let mut states = vec![start; COLUMNS];
    let mut values = vec![num_complex::Complex64::default(); COLUMNS];
    let mut columns = vec![Vec::new(); COLUMNS];
    for step in 0..BURN_IN + SAMPLES {
        program.eval_batch(&[&states, &controls], &mut values);
        for (i, (state, value)) in states.iter_mut().zip(&values).enumerate() {
            if !state.is_finite() {
                continue;
            }
            let value = crate::eval::as_real(*value);
            *state = if (value as f32).is_finite() { value } else { f64::NAN };
            if step >= BURN_IN && state.is_finite() {
                let column = &mut columns[i];
                let value = *state as f32;
                if !column.contains(&value) {
                    column.push(value);
                }
            }
        }
    }
    Ok(controls.into_iter().zip(columns).flat_map(|(r, values)| {
        values.into_iter().map(move |v| [r, f64::from(v), 0.0])
    }).collect())
}

/// この行の規則で決まる項を描く。別の添字を指定する行には点を残す。
pub(crate) fn sequence_terms(head: &SequenceHead, from: i64, to: i64, params: &Params) -> Result<Option<Vec<(i64, num_complex::Complex64)>>, String> {
    let defs = current();
    let Some(seq) = defs.seq(&head.name) else { return Ok(None) };
    let mut x = Expander::new(&defs, params);
    let step = i128::from(head.index.step).abs();
    let start = i128::from(from) + (i128::from(head.index.shift) - i128::from(from)).rem_euclid(step);
    let mut indices: BTreeSet<i64> = (0..MAX_TERMS)
        .map(|i| start + i as i128 * step)
        .take_while(|&k| k <= i128::from(to))
        .map(|k| k as i64)
        .collect();
    indices.extend(params.keys().filter_map(|param| initial_index(&head.name, param)).filter(|k| (from..=to).contains(k)));
    let mut out = Vec::new();
    let mut first_missing = None;
    let mut has_calculated_term = false;
    for k in indices.into_iter().take(MAX_TERMS) {
        let initial = params.contains_key(&format!("{}{k}", head.name));
        let rule = seq.rules.iter().find(|r| r.index.parameter(k).is_some());
        let belongs = match rule {
            Some(r) => r.index == head.index,
            None => seq.rules[0].index == head.index && initial,
        };
        if !belongs {
            continue;
        }
        let e = match x.term_value(&head.name, k) {
            Ok(e) => e,
            Err(e) if e.starts_with("変数 '") && e.contains("（初項 ") => {
                first_missing.get_or_insert(e);
                continue;
            }
            Err(e) => return Err(e),
        };
        let Some(value) = x.numeric_value(&e) else { return Ok(None) };
        out.push((k, value));
        has_calculated_term |= !initial;
    }
    if let Some(error) = first_missing {
        if !has_calculated_term {
            return Err(error);
        }
        if let Some((_, message)) = error.split_once('（') {
            add_note(message.trim_end_matches('）'));
        }
    }
    Ok(Some(out))
}
/// 横の座標の表示範囲（数列の点の添字の範囲に使う。`lib.rs` が入れる）
pub fn set_view_x(lo: f64, hi: f64) {
    VIEW_X.with(|x| x.set((lo, hi)));
}

pub fn view_x() -> (f64, f64) {
    VIEW_X.with(|x| x.get())
}

/// 表示中の 3 つの座標の表示範囲（横・奥・縦の順に下端・上端。停留点を探す範囲に使う。`lib.rs` が入れる）
pub fn set_view_box(b: [f64; 6]) {
    VIEW_BOX.with(|x| x.set(b));
}

pub fn view_box() -> [f64; 6] {
    VIEW_BOX.with(|x| x.get())
}

/// 数字の添字の付いた名前 `a1` が、定義した数列の項の名前か
pub fn is_term_name(name: &str) -> bool {
    split_subscript(name).is_some_and(|(h, _)| current().seq(h).is_some()) && crate::equation::coord_index(name).is_none()
}

/// 今の定義の数列
pub fn seq_def(name: &str) -> Option<Seq> {
    current().seq(name).cloned()
}

/// 式が、関数の名前を呼び出さずに（値として）使っているか（`f + g`・`f ∘ g`・`sin(f)`）
fn uses_function_value(e: &Expr, names: &[String]) -> bool {
    match e {
        Expr::Var(v) => names.contains(v),
        Expr::Num(_) | Expr::Const(_) | Expr::Prime { .. } | Expr::Cond(_) => false,
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

/// 式が、組（ベクトル）か行列を含むか（`{vec_b} = (1, 2, 3)`・`A = ⟦1, 2; 3, 4⟧` を定義として読むため）
fn has_vector_or_matrix(e: &Expr) -> bool {
    match e {
        Expr::Tuple(_) | Expr::Matrix { .. } => true,
        Expr::Neg(a) => has_vector_or_matrix(a),
        Expr::Bin(_, a, b) => has_vector_or_matrix(a) || has_vector_or_matrix(b),
        Expr::Call(_, args) | Expr::Op { args, .. } => args.iter().any(has_vector_or_matrix),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// 今の定義（plot の間だけ入れる）
// ---------------------------------------------------------------------------

thread_local! {
    static CURRENT: RefCell<Rc<Defs>> = RefCell::new(Rc::new(Defs::default()));
    /// 展開で使った定義の名前（画面が、定義が変わったときに描き直す式を決めるのに使う）
    static USED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// 式の説明に足す注意
    static NOTES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// ≠ で除いた所 h = 左 − 右 ≠ 0 の h（表示中の座標の式）。曲線なら h = 0 の点に白丸、面・領域なら h = 0 の線を点線で描く
    static EXCLUDED: RefCell<Vec<crate::eval::Program>> = const { RefCell::new(Vec::new()) };
    /// 集合の条件で決めた媒介変数の範囲
    static PARAM_RANGES: RefCell<Vec<(String, f64, f64)>> = const { RefCell::new(Vec::new()) };
    /// 近似を使った（範囲の形にならない条件で切り取った領域の積分）。画面の説明に「（積分は近似）」と出す
    static APPROX: RefCell<bool> = const { RefCell::new(false) };
    /// 展開の途中で値を使ったパラメータ（Σ の範囲の N など。展開した式には残らないので、別に覚えておく）
    static USED_PARAMS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// 直前にコンパイルした行が定義の行なら、その名前と、描いたものの説明（`y = f(x)` など。描かないなら None）
    static DEF_LINE: RefCell<Option<(String, Option<String>)>> = const { RefCell::new(None) };
    /// 原点から表示範囲の箱のいちばん遠い角までの距離（ker・Im の直線・平面の長さ。`lib.rs` が入れる）
    static REACH: RefCell<f64> = const { RefCell::new(20.0) };
    /// 作った図形を、表示範囲の箱で切るか（ker・Im）
    static TRIM: RefCell<bool> = const { RefCell::new(false) };
    /// 式の説明（「ker A（2 次元。平面）」「det = −2」）。あれば、ふつうの説明の代わりに出す
    static KIND: RefCell<Option<String>> = const { RefCell::new(None) };
    /// 横の座標の表示範囲
    static VIEW_X: std::cell::Cell<(f64, f64)> = const { std::cell::Cell::new((-10.0, 10.0)) };
    static VIEW_BOX: std::cell::Cell<[f64; 6]> = const { std::cell::Cell::new([-10.0, 10.0, -10.0, 10.0, -10.0, 10.0]) };
    /// 常微分方程式の初期値の欄（状態の成分ごとの式。計画 M9。`lib.rs` が入れる）
    static ODE_INIT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    /// 常微分方程式の状態の名前（`x`・`y'` など。初期値の欄の見出しに使う）
    static ODE_STATES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// 常微分方程式の初期値の欄の式を入れる（[`install`] の後）
pub fn set_ode_init(v: Vec<String>) {
    ODE_INIT.with(|x| *x.borrow_mut() = v);
}

pub fn ode_init() -> Vec<String> {
    ODE_INIT.with(|x| x.borrow().clone())
}

pub fn set_ode_states(v: Vec<String>) {
    ODE_STATES.with(|x| *x.borrow_mut() = v);
}

pub fn ode_states() -> Vec<String> {
    ODE_STATES.with(|x| x.borrow().clone())
}

/// 原点から表示範囲の箱のいちばん遠い角までの距離を入れる（[`install`] の後、コンパイルの前）
pub fn set_reach(r: f64) {
    if r.is_finite() && r > 0.0 {
        REACH.with(|x| *x.borrow_mut() = r);
    }
}

pub fn reach() -> f64 {
    REACH.with(|x| *x.borrow())
}

/// 作った図形を、表示範囲の箱で切る
pub fn request_trim() {
    TRIM.with(|x| *x.borrow_mut() = true);
}

pub fn trim_requested() -> bool {
    TRIM.with(|x| *x.borrow())
}

/// 式の説明を決める（ふつうの説明の代わりに出す）
pub fn set_kind(label: String) {
    KIND.with(|x| *x.borrow_mut() = Some(label));
}

pub fn kind_label() -> Option<String> {
    KIND.with(|x| x.borrow().clone())
}

/// 式を展開して、行列・ベクトルなら (行の数, 列の数, 成分の式)。そうでなければ None（計画 M7 の、行列を描く行の判定）
pub fn matrix_value(e: &Expr, params: &Params) -> Result<Option<(usize, usize, Vec<Expr>)>, String> {
    let defs = current();
    let mut x = Expander::new(&defs, params);
    Ok(match x.val(e)? {
        Val::S(v) => Mat::of(&v).map(|m| (m.rows, m.cols, m.items)),
        Val::Fn { .. } => None,
    })
}

/// 式が使っている、座標・媒介変数でない名前を「使ったパラメータ」に数える（まだ定義されていない文字でもよい。
/// 行列の定義の成分の文字が後からスライダーになったときに、その行を描き直すため）
pub fn note_names(items: &[Expr]) {
    let mut vs = Vec::new();
    items.iter().for_each(|e| e.collect_vars(&mut vs));
    USED_PARAMS.with(|u| {
        let mut u = u.borrow_mut();
        for v in vs {
            if definable(&v) && !u.contains(&v) {
                u.push(v);
            }
        }
    });
}

/// 成分が全部、数とパラメータだけの式なら、その値（座標・媒介変数を含めば None）
pub fn numbers(items: &[Expr], params: &Params) -> Option<Vec<f64>> {
    items
        .iter()
        .map(|e| {
            let prog = crate::eval::Program::compile_with(e, &[], params).ok()?;
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
            v.is_finite().then_some(v)
        })
        .collect()
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
    APPROX.with(|a| *a.borrow_mut() = false);
    NOTES.with(|n| n.borrow_mut().clear());
    EXCLUDED.with(|n| n.borrow_mut().clear());
    PARAM_RANGES.with(|r| r.borrow_mut().clear());
    DEF_LINE.with(|d| *d.borrow_mut() = None);
    REACH.with(|x| *x.borrow_mut() = 20.0);
    TRIM.with(|x| *x.borrow_mut() = false);
    KIND.with(|x| *x.borrow_mut() = None);
    ODE_INIT.with(|x| x.borrow_mut().clear());
    ODE_STATES.with(|x| x.borrow_mut().clear());
    Installed(Some(CURRENT.with(|c| std::mem::replace(&mut *c.borrow_mut(), Rc::new(defs)))))
}

fn current() -> Rc<Defs> {
    CURRENT.with(|c| c.borrow().clone())
}

/// 今の定義の名前
pub fn names() -> Vec<String> {
    current().names()
}

/// 集合の定義（計画 M11）。使ったら、使った定義に数える
pub fn set_def(name: &str) -> Option<crate::parser::SetE> {
    match current().get(name) {
        Some(Def::Set(s)) => {
            note_used(name);
            Some(s.clone())
        }
        _ => None,
    }
}

/// 式の説明に足す注意（「≠ は描けないので無視」など）
/// ≠ で除いた所の式 h（[`EXCLUDED`]）を足す
pub fn add_excluded(h: crate::eval::Program) {
    EXCLUDED.with(|n| n.borrow_mut().push(h));
}

/// ≠ で除いた所の式を取り出す（空にする）
pub fn take_excluded() -> Vec<crate::eval::Program> {
    EXCLUDED.with(|n| std::mem::take(&mut *n.borrow_mut()))
}

/// 展開の途中で値を使ったパラメータを覚える（展開した式に残らないもの）
fn record_used_params(names: &[String]) {
    USED_PARAMS.with(|u| {
        let mut u = u.borrow_mut();
        for p in names {
            if !u.contains(p) {
                u.push(p.clone());
            }
        }
    });
}

pub fn add_note(s: &str) {
    NOTES.with(|n| {
        let mut n = n.borrow_mut();
        if !n.iter().any(|x| x == s) {
            n.push(s.to_string());
        }
    });
}

/// 式の説明に足す注意
pub fn notes() -> Vec<String> {
    NOTES.with(|n| n.borrow().clone())
}

/// 媒介変数の範囲を決める（{(cos t, sin t) | 0 ≤ t ≤ π} の条件から。画面の範囲の欄より優先する）
pub fn set_param_range(name: &str, lo: f64, hi: f64) {
    PARAM_RANGES.with(|r| r.borrow_mut().push((name.to_string(), lo, hi)));
}

/// [`set_param_range`] で決めた媒介変数の範囲
pub fn param_range(name: &str) -> Option<(f64, f64)> {
    PARAM_RANGES.with(|r| r.borrow().iter().find(|(n, _, _)| n == name).map(|(_, a, b)| (*a, *b)))
}

/// 展開で使った定義の名前（重複なし、出てきた順）
pub fn used() -> Vec<String> {
    USED.with(|u| u.borrow().clone())
}

/// 近似を使ったか（[`install`] の後。範囲の形にならない条件で切り取った領域の積分）
pub fn approximate() -> bool {
    APPROX.with(|a| *a.borrow())
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
        d.borrow().as_ref().map(|(name, drawn)| {
            // ベクトルの名前 vec_F は「ベクトル F」と出す（矢印を重ねる文字や太字の数学用文字は、スマホの字体によって □ になる）
            let name = name.strip_prefix("vec_").map_or_else(|| name.clone(), |v| format!("ベクトル {v}"));
            match drawn {
                Some(l) => format!("{name} の定義（{l}）"),
                None => format!("{name} の定義"),
            }
        })
    })
}

/// コンパイルした行が定義の行だったことを記録する（`equation.rs`）。`drawn` は描いたものの説明
pub fn mark_def_line(name: &str, drawn: Option<String>) {
    DEF_LINE.with(|d| *d.borrow_mut() = Some((name.to_string(), drawn)));
}

/// パラメータを「使った」に数える（展開した式に残らない初項など）
fn note_param(name: &str) {
    USED_PARAMS.with(|u| {
        let mut u = u.borrow_mut();
        if !u.iter().any(|n| n == name) {
            u.push(name.to_string());
        }
    });
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

type TermKey = (String, i64);

enum TermEvaluation {
    Running,
    Needs(TermKey),
}

struct Expander<'a> {
    defs: &'a Defs,
    params: &'a Params,
    /// 積分の入れ子の深さ（2 重まで）
    int_depth: usize,
    /// 展開中の定義の名前（自分自身を使う定義を見つける）
    stack: Vec<String>,
    /// 漸化式の項（名前, 添字 → 値の式）。同じ展開の中で使い回す
    memo: HashMap<TermKey, Expr>,
    /// 項の展開中に、先に求める必要がある項を記録する。
    term_evaluation: Option<TermEvaluation>,
    /// 極限の中身を先に式へ展開できるか調べている間だけ true。
    probing_limit: bool,
    limit_sample_depth: usize,
    limit_samples_left: usize,
}

/// 式を展開して、今までの節だけの式にする。新しい節を使っていない式は、そのまま返す
pub fn expand(e: &Expr, params: &Params) -> Result<Expr, String> {
    if !e.has_notation() && !mentions_defs(e) && !nested_tuple(e, true) && !e.uses_var("∗") {
        return Ok(e.clone());
    }
    let defs = current();
    let mut x = Expander::new(&defs, params);
    let out = match x.val(e)? {
        Val::S(s) => s,
        Val::Fn { arity, body } => field_at_xyz(arity, &body).ok_or_else(|| fn_as_value_error(e))?,
    };
    if out.node_count() > MAX_NODES {
        return Err("式が大きくなりすぎます（関数の入れ子・Σ の項の数を減らしてください）".into());
    }
    Ok(out)
}

pub(crate) fn limit_line(
    op: &str, var: &str, args: &[Expr], body: &Expr, params: &Params,
) -> Result<(crate::limit::Graph, crate::limit::Report), String> {
    let (graph, report) = Expander::new(&current(), params).limit_expression(op, var, args, body, true)?;
    Ok((graph.expect("極限だけの行にはグラフがある"), report))
}

/// 式の途中（一番外でない所）に組があるか（ベクトルの計算。展開の段で成分の式にする）
fn nested_tuple(e: &Expr, top: bool) -> bool {
    match e {
        Expr::Tuple(items) => !top || items.iter().any(|i| nested_tuple(i, false)),
        Expr::Num(_) | Expr::Const(_) | Expr::Var(_) => false,
        Expr::Neg(a) => nested_tuple(a, false),
        Expr::Bin(_, a, b) => nested_tuple(a, false) || nested_tuple(b, false),
        Expr::Call(_, args) | Expr::List(args) => args.iter().any(|a| nested_tuple(a, false)),
        _ => false,
    }
}

/// 式が、定義された名前を（呼び出しでなく）使っているか
fn mentions_defs(e: &Expr) -> bool {
    let defs = current();
    if defs.is_empty() {
        return false;
    }
    let mut vs = Vec::new();
    e.collect_vars(&mut vs);
    vs.iter().any(|v| defs.get(v).is_some() || split_subscript(v).is_some_and(|(h, _)| defs.get(h).is_some() || defs.seq(h).is_some()))
}

/**
 * 値が組（ベクトル）の関数 F⃗(x, y, z) = (…) を、名前だけで書いた所: 座標 x, y, z に当てはめたベクトル場にする
 * （`F⃗` だけの行は矢印の場、`F⃗ · (1, 0, 0)` は場の成分）。値がスカラーの関数は、今までどおり引数を書いてもらう
 */
fn field_at_xyz(arity: usize, body: &Expr) -> Option<Expr> {
    (arity <= 3 && matches!(body, Expr::Tuple(_))).then(|| apply_body(body, &["x", "y", "z"][..arity].iter().map(|c| Expr::Var((*c).into())).collect::<Vec<_>>()))
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

impl<'a> Expander<'a> {
    fn new(defs: &'a Defs, params: &'a Params) -> Self {
        Self {
            defs, params, int_depth: 0, stack: Vec::new(), memo: HashMap::new(), term_evaluation: None,
            probing_limit: false, limit_sample_depth: 0, limit_samples_left: 12_000,
        }
    }

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
            Def::Set(_) => {
                self.stack.pop();
                return Err(format!("集合 {name} は式の中では使えません（x ∈ {name} や、{name} だけの行で使ってください）"));
            }
        };
        self.stack.pop();
        Ok(Some(v))
    }

    /// スカラーでなければならない所の値
    fn scalar(&mut self, e: &Expr) -> Result<Expr, String> {
        match self.val(e)? {
            Val::S(s) => Ok(s),
            Val::Fn { arity, body } => field_at_xyz(arity, &body).ok_or_else(|| fn_as_value_error(e)),
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
                None => match self.component_var(name)? {
                    Some(c) => Val::S(c),
                    None => Val::S(e.clone()),
                },
            },
            Expr::Apply { name, primes, args } => {
                if self.limit_sample_depth > 0 && *primes == 0 {
                    if let Some(value) = self.apply_at_limit_sample(name, args)? {
                        return Ok(value);
                    }
                }
                let depth = self.stack.len();
                let f = match self.def_value(name) {
                    Err(e) if *primes == 0 && e.contains("'#") => {
                        self.stack.truncate(depth);
                        if let Some(value) = self.apply_at_limit_sample(name, args)? {
                            return Ok(value);
                        }
                        return Err(e);
                    }
                    value => value?,
                };
                let Some(f) = f else {
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
                    // 関数でない定義（行列・ベクトル・数）の直後の括弧は、掛け算（A (x, y, z)）
                    Val::S(s) if *primes == 0 => {
                        let arg = if args.len() == 1 { args.into_iter().next().unwrap_or(Expr::Num(0.0)) } else { Expr::Tuple(args) };
                        self.bin(BinOp::Mul, Val::S(s), Val::S(arg), e)?
                    }
                    Val::S(_) => return Err(format!("{name} は関数ではありません")),
                }
            }
            Expr::Neg(a) => match self.val(a)? {
                Val::S(Expr::Tuple(items)) => Val::S(Expr::Tuple(items.into_iter().map(|c| Expr::Neg(Box::new(c))).collect())),
                Val::S(Expr::Matrix { rows, cols, items }) => {
                    Val::S(Expr::Matrix { rows, cols, items: items.into_iter().map(|c| Expr::Neg(Box::new(c))).collect() })
                }
                Val::S(s) => Val::S(Expr::Neg(Box::new(s))),
                Val::Fn { arity, body } => Val::Fn { arity, body: Expr::Neg(Box::new(body)) },
            },
            Expr::Bin(BinOp::Pow, a, b) => {
                let va = self.val(a)?;
                // 行列・ベクトルのべき: ^T（転置）・^†（共役転置）・整数（負は逆行列）
                if let Some(m) = if let Val::S(base) = &va { Mat::of(base) } else { None } {
                    match &**b {
                        // T がパラメータでも、行列の ^T は転置を優先する
                        Expr::Var(t) if t == "T" => return Ok(Val::S(m.transpose().into_expr())),
                        Expr::Var(t) if t == "†" => return Ok(Val::S(m.dagger().into_expr())),
                        Expr::Var(t) if t == "∗" => {
                            return Ok(Val::S(Mat { rows: m.rows, cols: m.cols, items: m.items.iter().map(|c| Expr::Call(crate::functions::Func::Conj, vec![c.clone()])).collect() }.into_expr()))
                        }
                        _ => {}
                    }
                    if m.cols == 1 {
                        return Err("ベクトルのべきは使えません（内積 v · v か、大きさ |v|^2 と書いてください）".into());
                    }
                    let k = self.integer(b, "行列のべき")?;
                    return Ok(Val::S(m.power(k, MAX_NODES)?.into_expr()));
                }
                // 共役 z^∗
                if matches!(&**b, Expr::Var(t) if t == "∗") {
                    return match va {
                        Val::S(s) => Ok(Val::S(Expr::Call(crate::functions::Func::Conj, vec![s]))),
                        Val::Fn { .. } => Err(fn_as_value_error(a)),
                    };
                }
                let vb = self.val(b)?;
                self.bin(BinOp::Pow, va, vb, e)?
            }
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
                // ベクトルの大きさ |v| = √(v · v)、行列の |A| は行列式。Re・Im・conj は成分ごと。ほかの関数には渡せない
                if let Some(m) = if let [Val::S(v)] = vals.as_slice() { Mat::of(v) } else { None } {
                    use crate::functions::Func;
                    return Ok(Val::S(match f {
                        Func::Abs if m.cols == 1 => norm(&m.items),
                        Func::Abs => m.det()?,
                        Func::Re | Func::Im | Func::Conj => {
                            Mat { rows: m.rows, cols: m.cols, items: m.items.iter().map(|c| Expr::Call(*f, vec![c.clone()])).collect() }.into_expr()
                        }
                        _ => return Err(format!("{} に{}は渡せません", f.names().first().copied().unwrap_or("この関数"), m.describe())),
                    }));
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
            Expr::Op { op, args } if matches!(op.as_str(), "cases" | "sequence_division") => Val::S(self.cases(args)?),
            Expr::Op { op, args } if op == "sumset" || op == "prodset" => Val::S(self.set_sum(op == "sumset", args)?),
            Expr::Cond(_) => return Err("条件は、Σ・Π の下でだけ使えます".into()),
            Expr::Op { op, args } if op == "intset" => Val::S(self.region_integral(args)?),
            Expr::Op { op, .. } if op == "ellipsis" => return Err("… はリスト [1, 2, …, 10] の中で使ってください".into()),
            Expr::Binder { op, var, args, body } if op == "sum" || op == "prod" => Val::S(self.sum_or_prod(op == "sum", var, args, body)?),
            Expr::Binder { op, var, args, body } if op == "int" => Val::S(self.integral(var, args, body)?),
            Expr::Binder { op, var, args, body } if matches!(op.as_str(), "lim" | "seqlim") => {
                let (_, report) = self.limit_expression(op, var, args, body, false)?;
                Val::S(Expr::Num(report.value()?))
            }
            Expr::Binder { op, var, args, body } if matches!(op.as_str(), "max" | "min" | "sup" | "inf") => {
                Val::S(self.extremum(matches!(op.as_str(), "max" | "sup"), var, args, body)?)
            }
            Expr::Binder { op, var, args, body } if op == "fdiff" || op == "bdiff" || op == "cdiff" => {
                Val::S(self.difference(op, var, args, body)?)
            }
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
                // 外積: 両方がベクトル（3 成分ならベクトル、2 成分ならスカラー）。スカラーどうしは掛け算
                match (self.val(&args[0])?, self.val(&args[1])?) {
                    (Val::S(Expr::Tuple(u)), Val::S(Expr::Tuple(v))) => Val::S(cross(&u, &v)?),
                    (Val::S(Expr::Tuple(_)), Val::S(o)) | (Val::S(o), Val::S(Expr::Tuple(_))) if !matches!(o, Expr::Matrix { .. }) => {
                        return Err("× の両側がベクトルなら外積、スカラーなら掛け算です（ベクトルとスカラーなら · か並べて書いてください）".into())
                    }
                    (va, vb) => self.bin(BinOp::Mul, va, vb, e)?,
                }
            }
            Expr::Op { op, args } if matches!(op.as_str(), "grad" | "div" | "rot" | "curl" | "lap") => Val::S(self.vector_operator(op, &args[0])?),
            // 点での値 `@at(式, x, a, y, b, …)`（LaTeX の \left.式\right|_{x=a, y=b}）: 式を展開してから、変数に値を一度に入れる
            // （∇ × F⃗ のように、展開してはじめて x, y, z が出てくる式にも効く）
            Expr::Op { op, args } if op == "at" => {
                if args.len() < 3 || args.len() % 2 == 0 {
                    return Err("点での値は、式|_{x = a, y = b} の形で書いてください".into());
                }
                let mut names = Vec::new();
                let mut values = Vec::new();
                for pair in args[1..].chunks(2) {
                    let Expr::Var(n) = &pair[0] else { return Err("点での値の、= の左には変数の名前を書いてください".into()) };
                    names.push(n.clone());
                    values.push(self.scalar(&pair[1])?);
                }
                let body = match self.val(&args[0])? {
                    Val::S(s) => s,
                    Val::Fn { arity, body } => field_at_xyz(arity, &body).ok_or_else(|| fn_as_value_error(&args[0]))?,
                };
                Val::S(body.substitute(&|v: &str| names.iter().position(|n| n == v).map(|k| values[k].clone())))
            }
            // テイラー多項式 `@taylor(f, x, a, n)`・`@taylor(f, (x, y), (a, b), n)`
            Expr::Op { op, args } if op == "taylor" => Val::S(self.taylor(args)?),
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
            Expr::Op { op, args } if matches!(op.as_str(), "det" | "tr" | "inv" | "transpose" | "norm" | "dot" | "angle") => {
                Val::S(self.matrix_op(op, args)?)
            }
            Expr::Op { op, args } if op == "dim" || op == "rank" => Val::S(Expr::Num(self.dimension(op, args)? as f64)),
            Expr::Op { op, .. } if op == "eig" => {
                return Err("@eig(A)（固有値・固有空間）は、1 行にそれだけを書いてください（決まった固有値 λ の固有空間は @ker(A − λI)）".into())
            }
            Expr::Op { op, .. } if matches!(op.as_str(), "ker" | "im") => {
                return Err(format!("@{op}(A) は、1 行にそれだけを書いてください（式の中では @dim(@{op}(A)) の形で使えます）"))
            }
            Expr::Op { op, .. } => return Err(format!("@{op} はまだ使えません")),
            Expr::Binder { op, .. } => return Err(format!("@{op} はまだ使えません")),
            Expr::Matrix { rows, cols, items } => {
                linalg::check_size(*rows, *cols)?;
                let items = items.iter().map(|a| self.scalar(a)).collect::<Result<Vec<_>, _>>()?;
                if items.iter().any(|c| Mat::of(c).is_some()) {
                    return Err("行列の成分には、数・式を書いてください（ベクトル・行列は入れられません）".into());
                }
                Val::S(Mat { rows: *rows, cols: *cols, items }.into_expr())
            }
            Expr::Index { name, index } if self.defs.seq(name).is_some() => Val::S(self.term(name, index)?),
            Expr::Index { name, index } => match self.def_value(name)? {
                Some(Val::S(v)) if Mat::of(&v).is_some() => Val::S(self.component(name, &v, index)?),
                _ => return Err(format!("{name}_(…) の添字はまだ使えません")),
            },
            Expr::Prime { name, order } => return Err(format!("{name}{} はまだ使えません", "'".repeat(*order as usize))),
        })
    }

    /// ベクトル・行列の値（そうでなければ、演算子の名前を入れたエラー）
    fn mat(&mut self, e: &Expr, what: &str) -> Result<Mat, String> {
        let v = self.scalar(e)?;
        Mat::of(&v).ok_or_else(|| format!("{what} には行列・ベクトルを書いてください"))
    }

    /// 行列・ベクトルの関数の形の演算子（@det・@tr・@inv・@transpose・@norm・@dot・@angle）
    fn matrix_op(&mut self, op: &str, args: &[Expr]) -> Result<Expr, String> {
        let what = format!("@{op}");
        let want = if matches!(op, "dot" | "angle") { 2 } else { 1 };
        if args.len() != want {
            return Err(format!("{what} の引数は {want} 個です（{} 個あります）", args.len()));
        }
        Ok(match op {
            "det" => match self.scalar(&args[0])? {
                // 1 × 1（スカラー）の行列式は、その数
                v => match Mat::of(&v) {
                    Some(m) => m.det()?,
                    None => v,
                },
            },
            "tr" => self.mat(&args[0], &what)?.trace()?,
            "inv" => self.mat(&args[0], &what)?.inverse()?.into_expr(),
            "transpose" => self.mat(&args[0], &what)?.transpose().into_expr(),
            "norm" => {
                let m = self.mat(&args[0], &what)?;
                if m.cols != 1 {
                    return Err("@norm にはベクトルを書いてください".into());
                }
                norm(&m.items)
            }
            _ => {
                let (u, v) = (self.mat(&args[0], &what)?, self.mat(&args[1], &what)?);
                if u.cols != 1 || v.cols != 1 {
                    return Err(format!("{what} にはベクトルを 2 つ書いてください"));
                }
                let d = linalg::dot(&u.items, &v.items)?;
                if op == "dot" {
                    d
                } else {
                    // なす角 arccos(u · v / (|u| |v|))
                    let den = crate::diff::mul(norm(&u.items), norm(&v.items));
                    Expr::Call(crate::functions::Func::Acos, vec![crate::diff::div(d, den)])
                }
            }
        })
    }

    /// 階数 `@rank(A)` と、次元 `@dim(@ker(A))`・`@dim(Im(A))`。成分を数にして、掃き出しで求める
    fn dimension(&mut self, op: &str, args: &[Expr]) -> Result<usize, String> {
        let [arg] = args else { return Err(format!("@{op} の引数は 1 個です")) };
        let (kind, inner) = match (op, arg) {
            ("rank", a) => ("rank", a),
            (_, Expr::Op { op: o, args: a }) if (o == "ker" || o == "im") && a.len() == 1 => (o.as_str(), &a[0]),
            (_, Expr::Call(crate::functions::Func::Im, a)) if a.len() == 1 => ("im", &a[0]),
            // dim A（行列そのもの）: 何の次元かが決まらないので、書き方を案内する
            (_, Expr::Var(n)) => {
                return Err(format!(
                    "dim の中が {n} だけです。零空間の次元なら dim ker {n}、像の次元なら dim Im {n}（= rank {n}）と書いてください"
                ))
            }
            _ => return Err("dim の中を ker A か Im A にしてください（dim ker A・dim Im A の形で使います）".into()),
        };
        let m = self.mat(inner, &format!("@{op}"))?;
        let Some(a) = numbers(&m.items, self.params) else {
            return Err("階数・次元を求める行列の成分は、数とパラメータだけで書いてください".into());
        };
        let rank = crate::linalg::Matrix::new(m.rows, m.cols, a).rank();
        Ok(if kind == "ker" { m.cols - rank } else { rank })
    }

    /// 数列の添字は整数に評価できる式。必要な項と初項だけを参照する。
    fn term(&mut self, name: &str, index: &Expr) -> Result<Expr, String> {
        if self.probing_limit && uses_internal_var(index, None) {
            return Err(LIMIT_INDEX_SAMPLE.into());
        }
        let k = self.integer(index, &format!("{name} の添字"))?;
        self.term_value(name, k)
    }

    fn term_value(&mut self, name: &str, k: i64) -> Result<Expr, String> {
        note_used(name);
        let param = format!("{name}{k}");
        if k >= 0 && self.params.contains_key(&param) {
            note_param(&param);
            return Ok(Expr::Var(param));
        }
        let key = (name.to_owned(), k);
        if let Some(value) = self.memo.get(&key) {
            return Ok(value.clone());
        }
        if let Some(evaluation) = &mut self.term_evaluation {
            // 展開の途中で必要になった項を積み、先にその項の値を求めてから再開する。
            if matches!(evaluation, TermEvaluation::Running) {
                *evaluation = TermEvaluation::Needs(key);
            }
            return Err("数列の項を計算中".into());
        }
        self.resolve_term(key)
    }

    fn term_body(&self, (name, k): &TermKey, root: &TermKey) -> Result<Expr, String> {
        let missing = |i| format!("変数 '{name}{i}' はここでは使えません（初項 {name}_{i} を定義してください）");
        let Some(seq) = self.defs.seq(name) else { return Err(format!("{name} の数列は定義されていません")) };
        let Some((rule, n)) = seq.rules.iter().find_map(|rule| rule.index.parameter(*k).map(|n| (rule, n))) else {
            return Err(if *k >= 0 { missing(*k) } else { format!("{name} の添字 {k} に対応する定義がありません") });
        };
        if rule.recursive {
            if rule.has_cases {
                if *k < 0 {
                    return Err(missing(0));
                }
            } else if let Some(first) = first_term(name, self.params) {
                if *k < 0 || (*k < first && (root.0 != *name || root.1 >= first)) {
                    return Err(missing((*k).max(0)));
                }
            } else if *k < 0 || (*k == 0 && root.0 == *name && root.1 > 0 && rule.index.step.unsigned_abs() == 1) {
                return Err(missing(i64::from(root.0 == *name && root.1 > 0 && rule.index.step.unsigned_abs() == 1)));
            }
        }
        Ok(rule.body.substitute(&|v| (v == rule.index.var).then_some(Expr::Num(n as f64))))
    }

    fn resolve_term(&mut self, root: TermKey) -> Result<Expr, String> {
        const MAX_SEQUENCE_TERMS: usize = 10_000;
        let body = self.term_body(&root, &root)?;
        let mut active = HashSet::from([root.clone()]);
        let mut frames = vec![(root.clone(), body)];
        let mut work = 1;
        let stack_len = self.stack.len();
        let int_depth = self.int_depth;
        while let Some((_, body)) = frames.last() {
            self.term_evaluation = Some(TermEvaluation::Running);
            let result = self.scalar(body);
            let evaluation = self.term_evaluation.take();
            self.stack.truncate(stack_len);
            self.int_depth = int_depth;
            if let Some(TermEvaluation::Needs(key)) = evaluation {
                if !active.insert(key.clone()) {
                    return Err(format!("数列の参照が循環しています（{}_{}）", key.0, key.1));
                }
                if work == MAX_SEQUENCE_TERMS {
                    return Err(format!("数列の計算に必要な項が多すぎます（{MAX_SEQUENCE_TERMS} 項まで）"));
                }
                let body = self.term_body(&key, &root)?;
                frames.push((key, body));
                work += 1;
                continue;
            }
            let e = result?;
            let value = match self.numeric_value(&e) {
                Some(v) if v.im == 0.0 || crate::eval::as_real(v).is_finite() => Expr::Num(v.re),
                Some(v) if v.re.is_finite() && v.im.is_finite() => {
                    let imaginary = Expr::Bin(BinOp::Mul, Box::new(Expr::Num(v.im)), Box::new(Expr::Const(crate::parser::Constant::I)));
                    Expr::Bin(BinOp::Add, Box::new(Expr::Num(v.re)), Box::new(imaginary))
                }
                Some(_) => Expr::Num(f64::NAN),
                None => e,
            };
            if value.node_count() > MAX_NODES {
                return Err(format!("{} の項の式が大きくなりすぎます（添字を小さくしてください）", root.0));
            }
            let (key, _) = frames.pop().unwrap();
            active.remove(&key);
            self.memo.insert(key, value);
        }
        Ok(self.memo[&root].clone())
    }
    /// 成分の取り出し `v_(2)`・`A_(1, 2)`（番号は 1 から）
    fn component(&mut self, name: &str, value: &Expr, index: &Expr) -> Result<Expr, String> {
        let Some(m) = Mat::of(value) else { return Err(format!("{name} はベクトル・行列ではありません")) };
        let what = format!("{name} の添字");
        let (i, j) = match index {
            Expr::Tuple(ix) if ix.len() == 2 => (self.integer(&ix[0], &what)?, self.integer(&ix[1], &what)?),
            Expr::Tuple(_) => return Err(format!("{name} の添字は 1 つか 2 つにしてください")),
            k if m.cols == 1 => (self.integer(k, &what)?, 1),
            k if m.rows == 1 => (1, self.integer(k, &what)?),
            _ => return Err(format!("行列 {name} の成分は {name}_(1, 2) のように、行と列の番号で書いてください")),
        };
        if i < 1 || j < 1 || i as usize > m.rows || j as usize > m.cols {
            return Err(format!("{name} は{}なので、({i}, {j}) 成分はありません", m.describe()));
        }
        Ok(m.at(i as usize - 1, j as usize - 1).clone())
    }

    /// `v_1`（字句では変数 `v1`）: v がベクトルとして定義されていて、`v1` という名前のパラメータ・座標が無ければ、1 つ目の成分
    fn component_var(&mut self, name: &str) -> Result<Option<Expr>, String> {
        let Some((head, digits)) = split_subscript(name) else { return Ok(None) };
        // 数列の項 a₂₀（字句では変数 a20）: 同じ名前のパラメータ（初項）が無ければ、項
        if self.defs.seq(head).is_some() && !self.params.contains_key(name) && crate::equation::coord_index(name).is_none() {
            let k: f64 = digits.parse().map_err(|_| format!("{name} の添字を読めません"))?;
            return self.term(head, &Expr::Num(k)).map(Some);
        }
        if self.params.contains_key(name) || crate::equation::coord_index(name).is_some() || self.defs.get(head).is_none() {
            return Ok(None);
        }
        let Some(Val::S(v)) = self.def_value(head)? else { return Ok(None) };
        if !Mat::of(&v).is_some_and(|m| m.cols == 1 || m.rows == 1) {
            return Ok(None);
        }
        let k: f64 = digits.parse().map_err(|_| format!("{name} の添字を読めません"))?;
        self.component(head, &v, &Expr::Num(k)).map(Some)
    }

    /// 場合分けを [`crate::functions::Func::Piece`] の呼び出しにする。条件 `L < R` の余裕は R − L（厳密）、
    /// `L ≤ R` は R − L、`L = R` は −|L − R|。連鎖 `0 ≤ x < 1` と `,` で並べた条件は全部成り立つとき（かつ）
    fn cases(&mut self, cases: &[Expr]) -> Result<Expr, String> {
        use crate::functions::Func;
        let mut out = Vec::new();
        let mut choose_branch = self.term_evaluation.is_some();
        for (k, case) in cases.iter().enumerate() {
            let Expr::Op { args, .. } = case else { return Err("内部エラー: 場合分けの形".into()) };
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
            if choose_branch {
                // 添字の代入後に条件が決まる数列では、必要な分岐の項だけを求める。
                let mut matches = true;
                for (j, margin) in margins.iter().enumerate() {
                    let Some(v) = self.numeric_value(margin) else {
                        choose_branch = false;
                        break;
                    };
                    matches &= if strict >> j & 1 == 1 { v.re > 0.0 } else { v.re >= 0.0 };
                }
                if choose_branch {
                    if matches {
                        return self.scalar(&args[0]);
                    }
                    continue;
                }
            }
            out.push(Expr::Num(margins.len() as f64));
            out.push(Expr::Num(strict as f64));
            out.extend(margins);
            out.push(self.scalar(&args[0])?);
        }
        if choose_branch {
            return Ok(Expr::Num(f64::NAN));
        }
        Ok(Expr::Call(Func::Piece, out))
    }

    /// 差分（幅 h、k 階）。前進 $\Delta_h f(x) = f(x+h) - f(x)$、後退 $\nabla_h f(x) = f(x) - f(x-h)$、
    /// 中心 $\delta_h f(x) = f(x+\tfrac h2) - f(x-\tfrac h2)$。変数を x + h などに置き換えた式を作る（置き換えは 1 回なので、
    /// h が x を含んでも f(x + h(x)) − f(x) になる）。k 階は k 回くり返す（h が x を含むなら、h も一緒にずれる）
    fn difference(&mut self, op: &str, var: &str, args: &[Expr], body: &Expr) -> Result<Expr, String> {
        let sym = match op {
            "fdiff" => "Δ",
            "bdiff" => "∇",
            _ => "δ",
        };
        let (h, k) = match args {
            [] => (Expr::Num(1.0), 1),
            [h] => (self.scalar(h)?, 1),
            [h, k] => {
                let k = self.integer(k, &format!("{sym} の階数"))?;
                if !(0..=8).contains(&k) {
                    return Err(format!("{sym} の階数は 0〜8 にしてください"));
                }
                (self.scalar(h)?, k as usize)
            }
            _ => return Err(format!("{sym} は {sym}_{{x}}、{sym}_{{x, h}} の形で書いてください")),
        };
        // 中身（df のように関数の名前だけなら、その関数を x に当てはめたもの）
        let mut f = match self.val(body)? {
            Val::S(s) => s,
            Val::Fn { arity: 1, body } => apply_body(&body, &[Expr::Var(var.to_string())]),
            Val::Fn { .. } => return Err(fn_as_value_error(body)),
        };
        let b = |o: BinOp, x: Expr, y: Expr| Expr::Bin(o, Box::new(x), Box::new(y));
        let x = Expr::Var(var.to_string());
        let shifted = |f: &Expr, by: Expr| f.substitute(&|v: &str| (v == var).then(|| b(BinOp::Add, x.clone(), by.clone())));
        for _ in 0..k {
            let half = b(BinOp::Div, h.clone(), Expr::Num(2.0));
            f = match op {
                "fdiff" => b(BinOp::Sub, shifted(&f, h.clone()), f),
                "bdiff" => b(BinOp::Sub, f.clone(), shifted(&f, Expr::Neg(Box::new(h.clone())))),
                _ => b(BinOp::Sub, shifted(&f, half.clone()), shifted(&f, Expr::Neg(Box::new(half)))),
            };
            if f.node_count() > MAX_NODES {
                return Err(format!("{sym} の式が大きくなりすぎます（階数を減らしてください）"));
            }
        }
        Ok(f)
    }

    /// 添字の変数の名前（変数か変数の組）
    fn index_vars(vars: &Expr) -> Result<Vec<String>, String> {
        let msg = || "添字は n か (m, n) のように変数で書いてください".to_string();
        match vars {
            Expr::Var(v) => Ok(vec![v.clone()]),
            Expr::Tuple(items) => items.iter().map(|e| if let Expr::Var(v) = e { Ok(v.clone()) } else { Err(msg()) }).collect(),
            _ => Err(msg()),
        }
    }

    /// 数とパラメータだけの式の値（座標などを含めばエラー）
    fn const_value(&mut self, e: &Expr, what: &str) -> Result<f64, String> {
        self.number(e, what)
    }

    /// 条件を満たす添字の値を全部並べる（計画 M4 の追加: 集合の上の総和）。
    /// 要素を書き並べた集合 `n ∈ {1, 4, 9}` があればその要素、無ければ条件の不等式から整数の範囲を出し、1 つずつ条件を調べる
    fn enumerate(&mut self, vars: &[String], cond: &crate::parser::Formula) -> Result<Vec<Vec<f64>>, String> {
        use crate::logic::{Lit, Op};
        use crate::parser::{Formula, SetE};
        const MAX_CANDIDATES: usize = 20_000;
        let what = "Σ・Π の条件";
        let lookup = |n: &str| set_def(n);
        let point = if vars.len() == 1 { Expr::Var(vars[0].clone()) } else { Expr::Tuple(vars.iter().map(|v| Expr::Var(v.clone())).collect()) };
        // 書き並べた集合
        fn finite_of(f: &Formula, point: &Expr) -> Option<Vec<Expr>> {
            match f {
                Formula::In(p, set) if p == point => match set {
                    SetE::Finite(items) => Some(items.clone()),
                    SetE::Named(n) => match set_def(n) {
                        Some(SetE::Finite(items)) => Some(items),
                        _ => None,
                    },
                    _ => None,
                },
                Formula::And(v) => v.iter().find_map(|g| finite_of(g, point)),
                _ => None,
            }
        }
        let mut candidates: Vec<Vec<f64>> = Vec::new();
        if let Some(items) = finite_of(cond, &point) {
            for it in items {
                let comps = match it {
                    Expr::Tuple(c) => c,
                    e => vec![e],
                };
                if comps.len() != vars.len() {
                    return Err("集合の要素の成分の数が、添字の数と違います".into());
                }
                let mut vals = Vec::new();
                for c in &comps {
                    let e = self.scalar(c)?;
                    vals.push(self.const_value(&e, what)?);
                }
                candidates.push(vals);
            }
        } else {
            // 条件の不等式から、添字ごとの整数の範囲を出す（「または」で分かれていれば、全部を含む範囲）
            let clauses = crate::logic::clauses(cond, vars, &lookup)?;
            let mut lo = vec![f64::INFINITY; vars.len()];
            let mut hi = vec![f64::NEG_INFINITY; vars.len()];
            for c in &clauses {
                // このかたまりでの添字ごとの範囲。別の添字で書いた端（m < n）は、その添字の範囲が分かってから区間演算で絞る
                let n = vars.len();
                let (mut l, mut h) = (vec![f64::NEG_INFINITY; n], vec![f64::INFINITY; n]);
                for _ in 0..=n {
                    for (k, v) in vars.iter().enumerate() {
                        for Lit { l: a, op, r: b } in c {
                            let is_v = |e: &Expr| matches!(e, Expr::Var(name) if name == v);
                            let other = if is_v(a) { b } else if is_v(b) { a } else { continue };
                            if other.uses_var(v) {
                                continue;
                            }
                            // 反対側の値の範囲（ほかの添字を使うなら、その添字の今の範囲で）
                            let used: Vec<usize> = (0..n).filter(|&j| other.uses_var(&vars[j])).collect();
                            let e = self.scalar(other)?;
                            let (xl, xh) = if used.is_empty() {
                                match self.const_value(&e, what) {
                                    Ok(x) => (x, x),
                                    Err(_) => continue,
                                }
                            } else {
                                // まだ分からない端は ±10¹² と置く（上の端を出すには相手の上の端だけが要り、下の端も同じなので、
                                // 要る側の答えが有限なら使える。m < n ≤ 3 から m ≤ 2）
                                const BIG: f64 = 1e12;
                                let names: Vec<&str> = used.iter().map(|&j| vars[j].as_str()).collect();
                                let Ok(p) = crate::eval::Program::compile_with(&e, &names, self.params) else { continue };
                                let boxes: Vec<crate::interval::Interval> =
                                    used.iter().map(|&j| crate::interval::Interval::new(l[j].max(-BIG), h[j].min(BIG))).collect();
                                match p.eval_interval(&boxes) {
                                    crate::interval::Range::Known(i) => (
                                        if i.lo > -BIG / 10.0 { i.lo } else { f64::NEG_INFINITY },
                                        if i.hi < BIG / 10.0 { i.hi } else { f64::INFINITY },
                                    ),
                                    crate::interval::Range::Unknown => continue,
                                }
                            };
                            match (op, is_v(a)) {
                                (Op::Lt, true) => h[k] = h[k].min(xh.ceil() - 1.0),
                                (Op::Le, true) => h[k] = h[k].min(xh.floor()),
                                (Op::Lt, false) => l[k] = l[k].max(xl.floor() + 1.0),
                                (Op::Le, false) => l[k] = l[k].max(xl.ceil()),
                                (Op::Eq, _) => {
                                    l[k] = l[k].max(xl.ceil());
                                    h[k] = h[k].min(xh.floor());
                                }
                                _ => {}
                            }
                        }
                    }
                }
                for (k, v) in vars.iter().enumerate() {
                    if !l[k].is_finite() || !h[k].is_finite() {
                        return Err(format!("Σ・Π の添字 {v} の範囲を、1 ≤ {v} ≤ 10 や {v} ∈ {{1, 2, 3}} のように書いてください"));
                    }
                    lo[k] = lo[k].min(l[k]);
                    hi[k] = hi[k].max(h[k]);
                }
            }
            // 添字の組を、番号から作る（各添字の範囲の大きさを桁にした数え方）
            let sizes: Vec<f64> = (0..vars.len()).map(|k| (hi[k] - lo[k] + 1.0).max(0.0)).collect();
            let total: f64 = sizes.iter().product();
            if total > MAX_CANDIDATES as f64 {
                return Err(format!("Σ・Π の添字の候補が多すぎます（{total} 個。{MAX_CANDIDATES} 個まで）"));
            }
            for idx in 0..total as usize {
                let mut rest = idx;
                let mut vals = vec![0.0; vars.len()];
                for k in (0..vars.len()).rev() {
                    let size = sizes[k] as usize;
                    vals[k] = lo[k] + (rest % size) as f64;
                    rest /= size;
                }
                candidates.push(vals);
            }
        }
        // 条件を 1 つずつ調べる
        let mut out = Vec::new();
        for vals in candidates {
            let f = crate::logic::substitute(cond, &|n: &str| vars.iter().position(|v| v == n).map(|k| Expr::Num(vals[k])));
            let clauses = crate::logic::clauses(&f, vars, &lookup)?;
            let mut ok_any = false;
            for c in &clauses {
                let mut ok = true;
                for Lit { l, op, r } in c {
                    let (a, b) = (self.scalar(l)?, self.scalar(r)?);
                    let (a, b) = (self.const_value(&a, what)?, self.const_value(&b, what)?);
                    let eq = (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0);
                    ok &= match op {
                        Op::Lt => a < b && !eq,
                        Op::Le => a <= b || eq,
                        Op::Eq => eq,
                        Op::Ne => !eq,
                    };
                    if !ok {
                        break;
                    }
                }
                if ok {
                    ok_any = true;
                    break;
                }
            }
            if ok_any {
                out.push(vals);
            }
        }
        Ok(out)
    }

    /// 集合の上の総和・総積 `@sumset(n; 条件) 項`
    fn set_sum(&mut self, sum: bool, args: &[Expr]) -> Result<Expr, String> {
        let sym = if sum { "Σ" } else { "Π" };
        let [vars, Expr::Cond(cond), body] = args else { return Err("内部エラー: 集合の上の総和の形".into()) };
        let names = Self::index_vars(vars)?;
        let values = self.enumerate(&names, cond)?;
        if values.len() > MAX_TERMS {
            return Err(format!("{sym} の項が多すぎます（{} 個。{MAX_TERMS} 個まで）", values.len()));
        }
        if values.is_empty() {
            return Ok(Expr::Num(if sum { 0.0 } else { 1.0 }));
        }
        let symbolic = self.scalar(body).ok();
        let mut terms = Vec::with_capacity(values.len());
        let mut nodes = 0;
        for vals in &values {
            let put = |e: &Expr| e.substitute(&|v: &str| names.iter().position(|n| n == v).map(|k| Expr::Num(vals[k])));
            let t = match &symbolic {
                Some(s) => put(s),
                None => self.scalar(&put(body))?,
            };
            nodes += t.node_count();
            if nodes > MAX_NODES {
                return Err(format!("{sym} の式が大きくなりすぎます（項の数を減らしてください）"));
            }
            terms.push(t);
        }
        Ok(balanced(terms, if sum { BinOp::Add } else { BinOp::Mul }))
    }

    /// 1 変数の実数の極限。近づく先と中身には、スライダーや定義を使える。
    fn limit_expression(
        &mut self,
        op: &str,
        var: &str,
        args: &[Expr],
        body: &Expr,
        graph: bool,
    ) -> Result<(Option<crate::limit::Graph>, crate::limit::Report), String> {
        use crate::{eval::Program, limit::{self, Graph}};
        let (target, side) = match args {
            [target] => (target, 0),
            [target, Expr::Num(1.0)] => (target, 1),
            [target, side] if side.is_minus_one() => (target, -1),
            _ => return Err("極限の近づく先と向きを確かめてください（x → a、x → a⁺、x → a⁻）".into()),
        };
        let target_expr = self.scalar(target)?;
        let infinity = matches!(&target_expr, Expr::Num(v) if v.is_infinite())
            || matches!(&target_expr, Expr::Neg(e) if matches!(&**e, Expr::Num(v) if v.is_infinite()));
        let target = Program::compile_with(&target_expr, &[], self.params)?;
        target.used_params().iter().for_each(|p| note_param(p));
        let mut value = [num_complex::Complex64::default()];
        target.eval_batch(&[], &mut value);
        let point = value[0].re;
        if value[0].im != 0.0 || (!point.is_finite() && !(infinity && point.is_infinite() && side == 0)) {
            return Err("極限の近づく先には、実数または向きの付かない ±∞ を書いてください".into());
        }
        if op == "seqlim" && point != f64::INFINITY {
            return Err("自然数の添字の極限は n → ∞ の形で書いてください".into());
        }

        // 中身の変数は、同じ名前の定義・パラメータを隠す。入れ子の外側の変数とも区別する。
        let mut local = "#limit".to_string();
        while body.uses_var(&local) {
            local.push('#');
        }
        let body = body.substitute(&|name| (name == var).then(|| Expr::Var(local.clone())));
        let sequence = point == f64::INFINITY && (op == "seqlim" || var == "n");
        let start = match &body {
            Expr::Index { name, index } if matches!(&**index, Expr::Var(v) if v == &local)
                && self.defs.seq(name).is_some_and(|seq| seq.rules.iter().any(|r| r.recursive)) =>
            {
                first_term(name, self.params).unwrap_or(1).max(1) as f64
            }
            _ => 1.0,
        };
        let stack_len = self.stack.len();
        let int_depth = self.int_depth;
        let previous_probe = std::mem::replace(&mut self.probing_limit, true);
        let symbolic = self.scalar(&body).and_then(|expanded| {
            let prog = Program::compile_with(&expanded, &[&local], self.params)?;
            Ok((expanded, prog))
        });
        self.probing_limit = previous_probe;
        self.stack.truncate(stack_len);
        self.int_depth = int_depth;
        if let Ok((expanded, prog)) = symbolic {
            prog.used_params().iter().for_each(|p| note_param(p));
            let mut sample = |xs: &[f64]| Ok(limit::program_samples(&prog, xs));
            if sequence {
                return limit::sequence(&mut sample, var, graph, start);
            }
            let removable = limit::removable_quotient(&expanded, &local, point, self.params);
            let report = limit::analyze(&mut sample, var, point, side, removable, graph)?;
            return Ok((graph.then_some(Graph::Function(prog)), report));
        }
        let error = symbolic.err().unwrap();
        let indexed = error == LIMIT_INDEX_SAMPLE;
        if !indexed && error != LIMIT_INTEGRAL_SAMPLE && !error.contains(&local)
            && !(error.contains("'#") && body.uses_var(&local))
        {
            return Err(error);
        }
        let mut sample = |xs: &[f64]| self.limit_samples(&body, &local, xs);
        if sequence || (indexed && point == f64::INFINITY) {
            return limit::sequence(&mut sample, var, graph, start);
        }
        let report = limit::analyze(&mut sample, var, point, side, None, graph)?;
        let curve = if graph {
            let (lo, hi) = view_x();
            let xs: Vec<_> = (0..=128).map(|i| lo + (hi - lo) * f64::from(i) / 128.0).collect();
            let pairs = sample(&xs)?.into_iter().zip(xs).map(|(y, x)| [x, y]).collect();
            Some(Graph::Sampled(pairs))
        } else {
            None
        };
        Ok((curve, report))
    }

    fn limit_samples(&mut self, body: &Expr, var: &str, xs: &[f64]) -> Result<Vec<f64>, String> {
        self.limit_samples_left = self.limit_samples_left.checked_sub(xs.len())
            .ok_or("極限の標本の計算量が上限に達しました")?;
        let previous_probe = std::mem::take(&mut self.probing_limit);
        self.limit_sample_depth += 1;
        let stack_len = self.stack.len();
        let int_depth = self.int_depth;
        let values = xs.iter().map(|&x| {
            if !x.is_finite() { return Ok(f64::NAN); }
            let at_x = |name: &str| (name == var).then_some(Expr::Num(x));
            let result = self.scalar(&body.substitute(&at_x));
            self.stack.truncate(stack_len);
            self.int_depth = int_depth;
            let value = match result {
                Ok(value) => value.substitute(&at_x),
                Err(e) if arithmetic_sample_error(&e) => return Ok(f64::NAN),
                Err(e) => return Err(e),
            };
            let p = crate::eval::Program::compile_with(&value, &[], self.params)?;
            p.used_params().iter().for_each(|p| note_param(p));
            let mut out = [num_complex::Complex64::default()];
            p.eval_batch(&[], &mut out);
            Ok(crate::limit::real_sample(out[0]))
        }).collect();
        self.limit_sample_depth -= 1;
        self.probing_limit = previous_probe;
        values
    }

    fn apply_at_limit_sample(&mut self, name: &str, actual: &[Expr]) -> Result<Option<Val>, String> {
        let Some(Def::Function { args, body }) = self.defs.get(name).cloned() else { return Ok(None) };
        if args.len() != actual.len() { return Ok(None); }
        let mut values = Vec::with_capacity(actual.len());
        for arg in actual {
            let arg = self.scalar(arg)?;
            let Some(v) = self.constant_value(&arg).filter(|v| v.im == 0.0) else { return Ok(None) };
            values.push(Expr::Num(v.re));
        }
        self.enter(name)?;
        let stack_len = self.stack.len() - 1;
        let expr = body.substitute(&|v| args.iter().position(|a| a == v).map(|i| values[i].clone()));
        let result = self.val(&expr);
        self.stack.truncate(stack_len);
        result.map(Some)
    }

    /// 定積分 $\int_a^b g(t)\,dt$ を、区間を 2 つに分けたガウス・ルジャンドルの重み付きの和にする（2 重目は 8 点 × 2）。
    /// 端 a, b は式のまま（x を使ってよい）。∞ の端は変数変換で有限の区間にする:
    /// $[a, \infty)$ は $t = a + \frac{s}{1-s}$、$(-\infty, b]$ は $t = b - \frac{s}{1-s}$（$s \in [0, 1)$）、
    /// $(-\infty, \infty)$ は $(-\infty, 0]$ と $[0, \infty)$ に分ける
    fn integral(&mut self, var: &str, args: &[Expr], body: &Expr) -> Result<Expr, String> {
        if args.is_empty() {
            return self.indefinite(var, body);
        }
        let [lo, hi] = args else { return Err("積分の範囲を書いてください（∫_0^1 の形）".into()) };
        if self.probing_limit
            && (uses_internal_var(lo, None) || uses_internal_var(hi, None) || uses_internal_var(body, Some(var)))
        {
            return Err(LIMIT_INTEGRAL_SAMPLE.into());
        }
        if self.int_depth >= 3 {
            return Err("積分の入れ子は 3 重までにしてください".into());
        }
        let (lo, hi) = (self.scalar(lo)?, self.scalar(hi)?);
        if self.limit_sample_depth > 0 {
            if let Some(value) = self.wide_integral_primitive(var, &lo, &hi, body) {
                return Ok(Expr::Num(value));
            }
        }
        // 値が定数になる積分は、適応型の積分で 1 つの数にする（決まった点の和より、ずっと正確）
        if let Some(v) = self.closed_integral(var, &lo, &hi, body)? {
            return Ok(Expr::Num(v));
        }
        let inf = |e: &Expr| match e {
            Expr::Num(v) if v.is_infinite() => Some(v.signum()),
            Expr::Neg(a) => match &**a {
                Expr::Num(v) if v.is_infinite() => Some(-v.signum()),
                _ => None,
            },
            _ => None,
        };
        let num = Expr::Num;
        // 節を作るときに定数をその場で計算する（端が数なら、積分の点の位置が数 1 つになる。3 重積分の式の大きさを抑える）
        let b = |op: BinOp, x: Expr, y: Expr| match op {
            BinOp::Add => crate::diff::add(x, y),
            BinOp::Sub => crate::diff::sub(x, y),
            BinOp::Mul => crate::diff::mul(x, y),
            BinOp::Div => crate::diff::div(x, y),
            BinOp::Pow => crate::diff::pow(x, y),
        };
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
        // 入れ子の深さごとの点の数（2 区間それぞれ）。3 重積分で被積分関数を 32 × 16 × 8 = 4,096 回計算する
        let (nodes, weights) = gauss_legendre([16, 8, 4][self.int_depth.min(2)]);
        // 区間 [s0, s1] を 2 つに分ける: 幅の半分 h、j 番目の中点 m_j
        let h = b(BinOp::Div, b(BinOp::Sub, s1, s0.clone()), num(2.0));
        let half_h = b(BinOp::Div, h.clone(), num(2.0));
        self.int_depth += 1;
        // 先に中身を記号のまま展開する（中の微分・差分は、積分の変数の式になる）。できなければ点ごとに展開する
        let symbolic = self.scalar(body).ok();
        let mut terms = Vec::new();
        let mut nodes_count = 0;
        for j in 0..2 {
            let mid = b(BinOp::Add, s0.clone(), b(BinOp::Mul, num(j as f64 + 0.5), h.clone()));
            for (xi, w) in nodes.iter().zip(&weights) {
                let sk = b(BinOp::Add, mid.clone(), b(BinOp::Mul, num(*xi), half_h.clone()));
                let at_s = |e: &Expr| e.substitute(&|v: &str| (v == "#s").then(|| sk.clone()));
                let t = at_s(&phi);
                let put = |e: &Expr| e.substitute(&|v: &str| (v == var).then(|| t.clone()));
                let g = match &symbolic {
                    Some(s) => put(s),
                    None => match self.scalar(&put(body)) {
                        Ok(g) => g,
                        Err(e) => {
                            self.int_depth -= 1;
                            return Err(e);
                        }
                    },
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

    fn wide_integral_primitive(&mut self, var: &str, lo: &Expr, hi: &Expr, body: &Expr) -> Option<f64> {
        let a = self.constant_value(lo)?;
        let b = self.constant_value(hi)?;
        if a.im != 0.0 || b.im != 0.0 || (b.re - a.re).abs() < 32.0 {
            return None;
        }
        let mut local = "#integral".to_string();
        while body.uses_var(&local) { local.push('#'); }
        let depth = self.stack.len();
        let integral_depth = self.int_depth;
        self.int_depth += 1;
        let expanded = self.scalar(&body.substitute(&|v| (v == var).then(|| Expr::Var(local.clone()))));
        self.stack.truncate(depth);
        self.int_depth = integral_depth;
        let primitive = crate::integrate::regular_antiderivative(&expanded.ok()?, &local)?;
        let prog = crate::eval::Program::compile_with(&primitive, &[&local], self.params).ok()?;
        prog.used_params().iter().for_each(|p| note_param(p));
        let mut out = [num_complex::Complex64::default(); 2];
        prog.eval_batch(&[&[a.re, b.re]], &mut out);
        let value = out[1] - out[0];
        (value.im == 0.0 && value.re.is_finite()).then_some(value.re)
    }

    /// 値が定数になる積分（端と中身が、積分の変数とパラメータだけの式）を、適応型の積分（`quad.rs`）で数にする。
    /// すぐ内側に続く積分 ∫∫∫（内側の端は外側の積分の変数の式でよい）も、まとめて入れ子のまま計算する
    /// （内側を決まった点の和にすると、内側の誤差がそのまま残る）。定数にならない・計算が多すぎるときは None
    /// （今までの、点の和の式にする道に任せる）
    fn closed_integral(&mut self, var: &str, lo: &Expr, hi: &Expr, body: &Expr) -> Result<Option<f64>, String> {
        // 端に ∞ がある積分で誤差が目標に届かないのは、ほとんど発散しているとき（∫₀^∞ 1 dt など）
        match self.closed_integral_value(var, lo, hi, body) {
            Some((_, true, true)) => Err("積分が収束しないようです（∞ までの積分が発散する・とても遅く 0 に近づく・範囲の中に中身が定義されない所がある）".to_string()),
            Some((v, _, _)) => Ok(Some(v)),
            None => Ok(None),
        }
    }

    /// [`Self::closed_integral`] の本体。(値, 誤差が目標に届かなかったか, 端に ∞ があるか)
    fn closed_integral_value(&mut self, var: &str, lo: &Expr, hi: &Expr, body: &Expr) -> Option<(f64, bool, bool)> {
        use crate::eval::Program;
        let mut levels: Vec<(String, Expr, Expr)> = vec![(var.to_string(), lo.clone(), hi.clone())];
        let mut inner = body;
        while let Expr::Binder { op, var: v, args, body: b } = inner {
            if op != "int" || args.len() != 2 || levels.len() >= 3 || levels.iter().any(|l| &l.0 == v) {
                break;
            }
            let (Ok(l), Ok(h)) = (self.scalar(&args[0]), self.scalar(&args[1])) else { break };
            levels.push((v.clone(), l, h));
            inner = b;
        }
        let names: Vec<&str> = levels.iter().map(|l| l.0.as_str()).collect();
        // 端の式（±∞ はコンパイルできないので、数のまま持つ）
        let bound = |e: &Expr, i: usize| -> Option<Bound> {
            match e {
                Expr::Num(v) if v.is_infinite() => Some(Bound::Inf(*v)),
                Expr::Neg(a) if matches!(&**a, Expr::Num(v) if v.is_infinite()) => {
                    let Expr::Num(v) = &**a else { return None };
                    Some(Bound::Inf(-*v))
                }
                _ => Program::compile_with(e, &names[..i], self.params).ok().map(Bound::Prog),
            }
        };
        let mut bounds = Vec::with_capacity(levels.len());
        for (i, (_, l, h)) in levels.iter().enumerate() {
            bounds.push((bound(l, i)?, bound(h, i)?));
        }
        self.int_depth += levels.len();
        let g = self.scalar(inner);
        self.int_depth -= levels.len();
        let f = Program::compile_with(&g.ok()?, &names, self.params).ok()?;
        // パラメータを使っていれば、スライダーが動いたら計算し直す
        USED_PARAMS.with(|u| {
            let mut u = u.borrow_mut();
            let progs = bounds.iter().flat_map(|(a, b)| [a, b]).filter_map(|b| if let Bound::Prog(p) = b { Some(p) } else { None });
            for p in progs.flat_map(|p| p.used_params()).chain(f.used_params()) {
                if !u.contains(p) {
                    u.push(p.clone());
                }
            }
        });
        let limit_sample = self.limit_sample_depth > 0;
        let budget = crate::quad::Budget::new(if limit_sample { 24_000 } else { CLOSED_BUDGET });
        let v = nested_integral(&bounds, &f, 0, [0.0; 3], &budget, limit_sample);
        let infinite = bounds.iter().any(|(a, b)| matches!(a, Bound::Inf(_)) || matches!(b, Bound::Inf(_)));
        if limit_sample && (budget.exhausted() || budget.rough.get() || !v.is_finite()) {
            return Some((f64::NAN, true, infinite));
        }
        if budget.exhausted() {
            return None;
        }
        if !v.is_finite() {
            // 端に ∞ があって有限の値にならない: 発散している（今までの点の和にすると、意味の無い数になる）
            return infinite.then_some((v, true, true));
        }
        let rough = budget.rough.get();
        if rough {
            add_note("積分の誤差の見積もりが目標（10⁻¹⁰）に届いていません（端で発散する・激しく振動するなど）");
        }
        Some((v, rough, infinite))
    }

    /// 添字つきの最大・最小 $\max_{t \in [a, b]} g(t)$（計画 M12）: 範囲を 64 等分した 65 点での値の max・min（近似）
    /// `max_{t ∈ [a, b]} g`・`min` で、a・b・g が t とパラメータだけの式のとき（計画 P1-7。A69 + 区間演算）。
    /// 区間演算が使える式は分枝限定法: 区間の上限（最大なら）の大きい順に半分に分け、真ん中の値で今までの最大を更新し、
    /// 上限が今までの最大 + 許容（1e-12 倍）以下の区間は捨てる。残りが無くなれば、その値は 1e-12 の精度で保証つき。
    /// 区間演算が使えない式（特殊関数）は、4,097 点で探して、いちばん良い点の周りを黄金分割で詰める（近似。説明に注意を出す）。
    /// 定数にならなければ None（今までの 65 点の近似の式にする）
    fn closed_extremum(&mut self, max: bool, var: &str, lo: &Expr, hi: &Expr, g: &Expr) -> Option<f64> {
        use crate::eval::Program;
        use crate::interval::{Interval, Range};
        use num_complex::Complex64 as C;
        let num = |e: &Expr| -> Option<f64> {
            let p = Program::compile_with(e, &[], self.params).ok()?;
            let mut out = [C::new(0.0, 0.0)];
            p.eval_batch(&[], &mut out);
            Some(crate::eval::as_real(out[0])).filter(|v| v.is_finite())
        };
        let (a, b) = (num(lo)?, num(hi)?);
        let f = Program::compile_with(g, &[var], self.params).ok()?;
        USED_PARAMS.with(|u| {
            let mut u = u.borrow_mut();
            for p in f.used_params() {
                if !u.contains(p) {
                    u.push(p.clone());
                }
            }
        });
        let (a, b) = (a.min(b), a.max(b));
        // 最大を求める形にそろえる（最小は −g の最大）
        let sign = if max { 1.0 } else { -1.0 };
        let vals = |ts: &[f64]| -> Vec<f64> {
            let mut out = vec![C::new(0.0, 0.0); ts.len()];
            f.eval_batch(&[ts], &mut out);
            out.iter().map(|v| sign * crate::eval::as_real(*v)).collect()
        };
        let pick = |ts: &[f64], vs: &[f64]| ts.iter().zip(vs).filter(|(_, v)| v.is_finite()).fold((f64::NAN, f64::NEG_INFINITY), |m, (&t, &v)| if v > m.1 { (t, v) } else { m });
        let ts: Vec<f64> = (0..=4096).map(|k| a + (b - a) * k as f64 / 4096.0).collect();
        let (mut best_t, mut best) = pick(&ts, &vals(&ts));
        if best == f64::NEG_INFINITY {
            return None; // どこでも値が実数にならない
        }
        if f.supports_interval() && a < b {
            // 分枝限定法（上限の大きい順。BinaryHeap は最大を先に出す）
            let upper = |lo: f64, hi: f64| -> f64 {
                match f.eval_interval(&[Interval::new(lo, hi)]) {
                    Range::Known(iv) => if max { iv.hi } else { -iv.lo },
                    Range::Unknown => f64::INFINITY,
                }
            };
            let mut heap = std::collections::BinaryHeap::new();
            let key = |u: f64| (u.to_bits() as i64 ^ (((u.to_bits() as i64) >> 63) & i64::MAX)) as i128;
            heap.push((key(upper(a, b)), a.to_bits(), b.to_bits()));
            let mut steps = 0;
            let mut proved = false;
            while let Some((_, lb, hb)) = heap.pop() {
                let (l, h) = (f64::from_bits(lb), f64::from_bits(hb));
                let u = upper(l, h);
                // 上限の大きい順に出てくるので、これが今までの最大 + 許容以下なら、残り全部も以下（証明が済んだ）
                if u <= best + 1e-12 * best.abs().max(1e-300) {
                    proved = true;
                    break;
                }
                steps += 1;
                if steps > 20_000 {
                    break;
                }
                let m = 0.5 * (l + h);
                if m <= l || m >= h {
                    continue; // 浮動小数点の幅の下限（この区間の値は端の点で見ている）
                }
                let v = vals(&[m])[0];
                if v.is_finite() && v > best {
                    (best_t, best) = (m, v);
                }
                for (p, q) in [(l, m), (m, h)] {
                    let uq = upper(p, q);
                    if uq > best + 1e-12 * best.abs().max(1e-300) {
                        heap.push((key(uq), p.to_bits(), q.to_bits()));
                    }
                }
            }
            if proved || heap.is_empty() {
                return Some(sign * best);
            }
        }
        // 区間演算が使えない・分枝限定が終わらない: いちばん良い点の周りを黄金分割で詰める（近似）
        let step = (b - a) / 4096.0;
        let (mut lo_t, mut hi_t) = ((best_t - step).max(a), (best_t + step).min(b));
        let r = 0.618_033_988_749_895;
        for _ in 0..80 {
            let (c, d) = (hi_t - r * (hi_t - lo_t), lo_t + r * (hi_t - lo_t));
            let v = vals(&[c, d]);
            if v[0] >= v[1] { hi_t = d } else { lo_t = c }
            for vv in v {
                if vv.is_finite() && vv > best {
                    best = vv;
                }
            }
        }
        add_note("max・min は 4,097 点と黄金分割での近似（区間演算が使えない式）");
        Some(sign * best)
    }

    fn extremum(&mut self, max: bool, var: &str, args: &[Expr], body: &Expr) -> Result<Expr, String> {
        const N: usize = 64;
        let [lo, hi] = args else { return Err("max・min は、max_{t ∈ [0, 1]} の形で範囲を書いてください".into()) };
        let (lo, hi) = (self.scalar(lo)?, self.scalar(hi)?);
        let g = self.scalar(body)?;
        // 値が定数になるもの（範囲と中身が t とパラメータだけ）は、分枝限定法で保証つきの最大・最小にする
        if let Some(v) = self.closed_extremum(max, var, &lo, &hi, &g) {
            return Ok(Expr::Num(v));
        }
        let f = if max { crate::functions::Func::Max } else { crate::functions::Func::Min };
        let mut terms = Vec::with_capacity(N + 1);
        for k in 0..=N {
            let s = k as f64 / N as f64;
            let t = crate::diff::add(crate::diff::mul(Expr::Num(1.0 - s), lo.clone()), crate::diff::mul(Expr::Num(s), hi.clone()));
            terms.push(g.substitute(&|v| (v == var).then(|| t.clone())));
        }
        // 平衡のとれた木で max・min をとる
        while terms.len() > 1 {
            let mut next = Vec::with_capacity(terms.len().div_ceil(2));
            let mut it = terms.into_iter();
            while let Some(a) = it.next() {
                next.push(match it.next() {
                    Some(b) => Expr::Call(f, vec![a, b]),
                    None => a,
                });
            }
            terms = next;
        }
        add_note("max・min は 64 等分の点での近似");
        Ok(terms.pop().unwrap_or(Expr::Num(0.0)))
    }

    /// 不定積分（積分定数を省いた形）: 表で扱える形は記号で（`integrate.rs`）、扱えなければ数値で $\int_0^x f(t)\,dt$
    /// （そのときの積分定数は F(0) = 0 になるもの）
    fn indefinite(&mut self, var: &str, body: &Expr) -> Result<Expr, String> {
        let f = match self.val(body)? {
            Val::S(s) => s,
            Val::Fn { arity: 1, body } => apply_body(&body, &[Expr::Var(var.to_string())]),
            Val::Fn { .. } => return Err(fn_as_value_error(body)),
        };
        if let Some(big_f) = crate::integrate::antiderivative(&f, var) {
            return Ok(big_f);
        }
        // 数値: 0 から x まで（中身はもう展開済みなので、そのまま積分の変数を入れる）
        self.integral(var, &[Expr::Num(0.0), Expr::Var(var.to_string())], &f)
    }

    /// 領域の上の重積分 `@intset((x, y), 条件…)(中身)`。条件を「変数 ≤ 式」の形の範囲に分け、
    /// 内側の変数の範囲が外側の変数だけで決まる順番を探して、累次積分にする。範囲の形にならない条件（x² + y² ≤ 1 など）は、
    /// 中身に「条件の外では 0」を掛けた近似にする（境目で値が飛ぶので、誤差は 10⁻² ほど）
    /// 勾配・発散・回転・ラプラシアン（座標は x, y, z）。中身が関数の名前なら、座標に当てはめる
    fn vector_operator(&mut self, op: &str, arg: &Expr) -> Result<Expr, String> {
        use crate::diff::{add, diff, sub};
        const XYZ: [&str; 3] = ["x", "y", "z"];
        let v = match self.val(arg)? {
            Val::S(s) => s,
            Val::Fn { arity, body } if arity <= 3 => {
                apply_body(&body, &XYZ[..arity].iter().map(|c| Expr::Var((*c).into())).collect::<Vec<_>>())
            }
            Val::Fn { .. } => return Err(fn_as_value_error(arg)),
        };
        let name = match op {
            "grad" => "∇（勾配）",
            "div" => "∇·（発散）",
            "rot" | "curl" => "∇×（回転）",
            _ => "∇²（ラプラシアン）",
        };
        let d = |e: &Expr, c: &str| diff(e, c);
        Ok(match (op, &v) {
            ("grad", Expr::Tuple(_)) | ("lap", Expr::Tuple(_)) => return Err(format!("{name} はスカラーの式に使ってください")),
            ("grad", _) => Expr::Tuple(XYZ.iter().map(|c| d(&v, c)).collect::<Result<_, _>>()?),
            ("lap", _) => {
                let mut s = Expr::Num(0.0);
                for c in XYZ {
                    s = add(s, d(&d(&v, c)?, c)?);
                }
                s
            }
            ("div", Expr::Tuple(items)) if items.len() <= 3 => {
                let mut s = Expr::Num(0.0);
                for (f, c) in items.iter().zip(XYZ) {
                    s = add(s, d(f, c)?);
                }
                s
            }
            ("rot" | "curl", Expr::Tuple(items)) if items.len() == 2 => sub(d(&items[1], "x")?, d(&items[0], "y")?),
            ("rot" | "curl", Expr::Tuple(items)) if items.len() == 3 => {
                let (p, q, r) = (&items[0], &items[1], &items[2]);
                Expr::Tuple(vec![sub(d(r, "y")?, d(q, "z")?), sub(d(p, "z")?, d(r, "x")?), sub(d(q, "x")?, d(p, "y")?)])
            }
            _ => return Err(format!("{name} は 2 成分か 3 成分のベクトル場 (P, Q, R) に使ってください")),
        })
    }

    /// 点を媒介変数の式で書いた集合の上の積分（線積分・面積分・体積分）。被積分関数の座標 `coords`（ふつうは x, y, z）に点の成分を入れ、
    /// 弧長 $|\mathbf r'|$・面積要素 $|\mathbf r_u \times \mathbf r_v|$・体積要素 $|\det J|$（`vec` なら $\mathbf F \cdot \mathbf r'$・
    /// $\mathbf F \cdot (\mathbf r_u \times \mathbf r_v)$）を掛けて、媒介変数の領域の上の積分にする。
    /// 媒介変数 `params` を渡さなければ、条件に出てくる名前のうち点の式に出てくるもの（出てくる順。面の向き $\mathbf r_u \times \mathbf r_v$ もこの順）。
    /// 条件の等式（`y = x^2`・`x = y = z`）は、媒介変数のどれかについて解いて点の式に入れる（[`Self::eliminate`]）。
    /// `diffs` は積分の記号の d の後の文字（ds・dS・dV・dx …）。範囲の次元と合わなければ誤り（[`Self::measure_of`]）
    #[allow(clippy::too_many_arguments)]
    fn parametric_integral(
        &mut self,
        point: &Expr,
        chains: &[Expr],
        vec_mode: bool,
        diffs: &[String],
        params: Option<Vec<String>>,
        coords: &[String],
        body: &Expr,
    ) -> Result<Expr, String> {
        use crate::diff::{add, diff, mul, neg, sub};
        let mut comps: Vec<Expr> = match self.scalar(point)? {
            Expr::Tuple(items) => items,
            e => vec![e],
        };
        let dim = comps.len();
        if dim > 3 {
            return Err("曲線・曲面の点は 3 成分までにしてください".into());
        }
        let mut params = match params {
            Some(p) => p,
            None => {
                // 条件に出てくる名前（パラメータ・定義の名前は除く）のうち、点の式に出てくるもの
                let mut in_cond: Vec<String> = Vec::new();
                for chain in chains {
                    match chain {
                        Expr::Op { args, .. } => args.iter().for_each(|e| e.collect_vars(&mut in_cond)),
                        Expr::Cond(f) => f.collect_vars(&mut in_cond),
                        e => e.collect_vars(&mut in_cond),
                    }
                }
                let mut ps: Vec<String> = Vec::new();
                for c in &comps {
                    let mut vs = Vec::new();
                    c.collect_vars(&mut vs);
                    for v in vs {
                        if in_cond.contains(&v) && !self.params.contains_key(&v) && !self.defs.map.contains_key(&v) && !ps.contains(&v) {
                            ps.push(v);
                        }
                    }
                }
                ps
            }
        };
        // 等式の条件を解いて、点の式に入れる。残りの条件は媒介変数の範囲
        let (eqs, mut chains) = split_equalities(chains);
        if !eqs.is_empty() {
            let (solved, rest, unsolved) = self.eliminate(&params, &eqs)?;
            let back = |v: &str| solved.iter().find(|(w, _)| w == v).map(|(_, e)| e.clone());
            comps = comps.iter().map(|c| c.substitute(&back)).collect();
            chains = chains.iter().map(|c| subst_chain(c, &back)).collect();
            params = rest;
            // 1 次式に解けない等式（x² + y² = 1・x² + y² + z² = 1）が残れば、点を数値で探す
            if !unsolved.is_empty() {
                return self.implicit_integral(&comps, &chains, &unsolved, &params, vec_mode, diffs, coords, body);
            }
        }
        let k = params.len();
        if k == 0 {
            return Err(if eqs.is_empty() {
                "曲線・曲面の媒介変数の範囲を、0 ≤ t ≤ 2π のように書いてください".into()
            } else {
                "等式で範囲が 1 点になります（積分は 0）。曲線なら、等式を 1 つ減らしてください".into()
            });
        }
        let measure = self.measure_of(k, dim, diffs, vec_mode, &params, coords)?;
        let jac: Vec<Vec<Expr>> = params.iter().map(|p| comps.iter().map(|c| diff(c, p)).collect::<Result<_, _>>()).collect::<Result<_, _>>()?;
        // 被積分関数（関数の名前なら座標に当てはめる）の座標に点の成分を入れる
        let coord_vars: Vec<Expr> = coords.iter().map(|c| Expr::Var(c.clone())).collect();
        let g = match self.val(body)? {
            Val::S(s) => s,
            Val::Fn { arity, body } if arity <= coords.len() => apply_body(&body, &coord_vars[..arity]),
            Val::Fn { .. } => return Err(fn_as_value_error(body)),
        };
        let g = g.substitute(&|v: &str| coords[..dim.min(coords.len())].iter().position(|c| c == v).map(|i| comps[i].clone()));
        let dot = |u: &[Expr], v: &[Expr]| u.iter().zip(v).fold(Expr::Num(0.0), |s, (a, b)| add(s, mul(a.clone(), b.clone())));
        if vec_mode {
            let Expr::Tuple(f) = &g else { return Err("F · dr・F · dS の F はベクトル場 (P, Q, R) にしてください".into()) };
            if f.len() != dim {
                return Err(format!("ベクトル場の成分の数（{}）を、点の成分の数（{dim}）にそろえてください", f.len()));
            }
        } else if matches!(g, Expr::Tuple(_)) {
            return Err("ベクトル場を積分するなら F · dr・F · dS の形で書いてください".into());
        }
        let normal = || match cross(&jac[0], &jac[1]) {
            Ok(Expr::Tuple(n)) => Ok(n),
            Ok(_) => Err("内部エラー: 法線".to_string()),
            Err(e) => Err(e),
        };
        let integrand = match measure {
            Measure::Natural if vec_mode => {
                let Expr::Tuple(f) = &g else { return Err("F · dr・F · dS の F はベクトル場 (P, Q, R) にしてください".into()) };
                match k {
                    1 => dot(f, &jac[0]),
                    2 if dim == 3 => dot(f, &normal()?),
                    _ => return Err("F · dr は曲線（媒介変数 1 つ）、F · dS は 3 次元の曲面（媒介変数 2 つ）で使えます".into()),
                }
            }
            Measure::Natural => {
                let m = match (k, dim) {
                    (1, _) => norm(&jac[0]),
                    (2, 3) => norm(&normal()?),
                    (2, 2) => Expr::Call(crate::functions::Func::Abs, vec![sub(mul(jac[0][0].clone(), jac[1][1].clone()), mul(jac[0][1].clone(), jac[1][0].clone()))]),
                    (3, 3) => Expr::Call(crate::functions::Func::Abs, vec![det3(&jac)]),
                    _ => return Err(format!("媒介変数 {k} つで {dim} 成分の点の積分はできません（曲線は 1 つ、曲面は 2 つ、立体は 3 つ）")),
                };
                mul(g, m)
            }
            Measure::Plain => g,
            Measure::Coord(i) => mul(g, jac[0][i].clone()),
            Measure::Form(i, sign) => {
                let n = normal()?;
                let c = n[i].clone();
                mul(g, if sign { c } else { neg(c) })
            }
        };
        let mut args = vec![Expr::Tuple(params.iter().map(|p| Expr::Var(p.clone())).collect())];
        args.extend(chains);
        args.push(integrand);
        self.region_integral(&args)
    }

    /// 1 次式に解けない等式で書いた曲線・曲面（`{(x, y) | x² + y² = 1}`・`{(x, y, z) | x² + y² + z² = 1, z ≥ 0}`）の上の積分。
    /// 点と重みは `implicit.rs` が数値で求める（変数 `params` の空間の曲線・曲面の、弧長・面積の重みと、単位接線・枠）。
    /// 点の成分 `comps`（`params` の式。ふつうは変数そのもの）で写した先の弧長・面積・F · dr・F · dS にする。
    /// `eqs` は解けずに残った等式の左辺 − 右辺、`chains` は残りの条件（不等式）。
    /// 向きは、平面の曲線なら {F < 0} を左に見る向き（F = 左辺 − 右辺。円 x² + y² = 1 なら反時計回り）、
    /// 空間の曲線（等式 2 つ）なら ∇F₁ × ∇F₂、曲面なら ∇F の向き（{F < 0} の外向き。球面 x² + y² + z² = 1 なら外向き）
    #[allow(clippy::too_many_arguments)]
    fn implicit_integral(
        &mut self,
        comps: &[Expr],
        chains: &[Expr],
        eqs: &[Expr],
        params: &[String],
        vec_mode: bool,
        diffs: &[String],
        coords: &[String],
        body: &Expr,
    ) -> Result<Expr, String> {
        use crate::diff::{diff, sub};
        use crate::eval::{as_real, Program};
        let (p, q, dim) = (params.len(), eqs.len(), comps.len());
        if p > 3 {
            return Err("等式で書いた曲線・曲面の変数は 3 つまでにしてください".into());
        }
        if q >= p {
            return Err("等式の数が変数の数と同じなので、範囲が点になります（積分は 0）。曲線なら、等式を 1 つ減らしてください".into());
        }
        let k = p - q;
        let measure = self.measure_of(k, dim, diffs, vec_mode, &[], coords)?;
        if vec_mode && !(k == 1 || (k == 2 && dim == 3)) {
            return Err("F · dr は曲線、F · dS は 3 次元の曲面で使えます".into());
        }
        let names: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
        let consts = self.params;
        let mut used: Vec<String> = Vec::new();
        let mut compile = |e: &Expr, what: &str| -> Result<Program, String> {
            let prog = Program::compile_with(e, &names, consts)
                .map_err(|_| format!("等式で書いた曲線・曲面の{what}は、変数 {} とパラメータだけの式にしてください", params.join(", ")))?;
            used.extend(prog.used_params().iter().cloned());
            Ok(prog)
        };
        let mut eq_progs = Vec::new();
        let mut grads = Vec::new();
        for f in eqs {
            eq_progs.push(compile(f, "等式")?);
            let mut row = Vec::new();
            for v in params {
                row.push(compile(&diff(f, v)?, "等式")?);
            }
            grads.push(row);
        }
        let mut jac: Vec<Vec<Program>> = Vec::new();
        for c in comps {
            let mut row = Vec::new();
            for v in params {
                row.push(compile(&diff(c, v)?, "点")?);
            }
            jac.push(row);
        }
        let pairs = self.condition_pairs(params, chains)?;
        let free = |e: &Expr| !params.iter().any(|v| e.uses_var(v));
        let mut known: Vec<(Option<f64>, Option<f64>)> = vec![(None, None); 3];
        let mut ineqs = Vec::new();
        for (l, _, r) in &pairs {
            ineqs.push(compile(&sub(r.clone(), l.clone()), "条件")?);
            for (i, v) in params.iter().enumerate() {
                let is_v = |e: &Expr| matches!(e, Expr::Var(name) if name == v);
                if is_v(r) && free(l) {
                    if let Some(a) = self.numeric(l) {
                        known[i].0 = Some(known[i].0.map_or(a, |b: f64| b.max(a)));
                    }
                } else if is_v(l) && free(r) {
                    if let Some(a) = self.numeric(r) {
                        known[i].1 = Some(known[i].1.map_or(a, |b: f64| b.min(a)));
                    }
                }
            }
        }
        record_used_params(&used);
        let manifold = crate::implicit::Manifold { p, eqs: eq_progs, grads, ineqs, known };
        let nodes = crate::implicit::nodes(&manifold)?;
        // 被積分関数（関数の名前なら座標に当てはめる）の座標に点の成分を入れる
        let coord_vars: Vec<Expr> = coords.iter().map(|c| Expr::Var(c.clone())).collect();
        let g = match self.val(body)? {
            Val::S(s) => s,
            Val::Fn { arity, body } if arity <= coords.len() => apply_body(&body, &coord_vars[..arity]),
            Val::Fn { .. } => return Err(fn_as_value_error(body)),
        };
        let g = g.substitute(&|v: &str| coords[..dim.min(coords.len())].iter().position(|c| c == v).map(|i| comps[i].clone()));
        let fields: Vec<Expr> = match (&g, vec_mode) {
            (Expr::Tuple(f), true) if f.len() == dim => f.clone(),
            (Expr::Tuple(f), true) => return Err(format!("ベクトル場の成分の数（{}）を、点の成分の数（{dim}）にそろえてください", f.len())),
            (_, true) => return Err("F · dr・F · dS の F はベクトル場 (P, Q, R) にしてください".into()),
            (Expr::Tuple(_), false) => return Err("ベクトル場を積分するなら F · dr・F · dS の形で書いてください".into()),
            (_, false) => vec![g.clone()],
        };
        let pts: Vec<[f64; 3]> = nodes.iter().map(|n| n.u).collect();
        let at = |prog: &Program| -> Vec<f64> {
            let cols: Vec<Vec<f64>> = (0..p).map(|l| pts.iter().map(|q| q[l]).collect()).collect();
            let inputs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
            let mut out = vec![num_complex::Complex64::new(0.0, 0.0); pts.len()];
            prog.eval_batch(&inputs, &mut out);
            out.iter().map(|v| as_real(*v)).collect()
        };
        // 点ごとに、写した先の接ベクトル（曲線）・法線 r_u × r_v にあたるもの（曲面）から、場の成分ごとに掛ける係数を作る
        let dr: Vec<Vec<Vec<f64>>> = jac.iter().map(|row| row.iter().map(&at).collect()).collect();
        let mut coefs: Vec<Vec<f64>> = vec![Vec::with_capacity(nodes.len()); fields.len()];
        for (i, n) in nodes.iter().enumerate() {
            let tangent = |j: usize| -> Vec<f64> { (0..dim).map(|c| (0..p).map(|l| dr[c][l][i] * n.frame[j][l]).sum()).collect() };
            let t0 = tangent(0);
            let normal = || -> [f64; 3] {
                let t1 = tangent(1);
                if dim < 3 {
                    return [0.0; 3];
                }
                [t0[1] * t1[2] - t0[2] * t1[1], t0[2] * t1[0] - t0[0] * t1[2], t0[0] * t1[1] - t0[1] * t1[0]]
            };
            let len = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
            if vec_mode {
                let v: Vec<f64> = if k == 1 { t0.clone() } else { normal().to_vec() };
                for (c, cv) in coefs.iter_mut().enumerate() {
                    cv.push(n.w * v[c]);
                }
                continue;
            }
            let c = match measure {
                Measure::Natural | Measure::Plain => match (k, dim) {
                    (1, _) => len(&t0),
                    (2, 3) => len(&normal()),
                    (2, 2) => {
                        let t1 = tangent(1);
                        (t0[0] * t1[1] - t0[1] * t1[0]).abs()
                    }
                    _ => return Err(format!("{k} 次元の範囲を {dim} 成分の点に写した積分はできません")),
                },
                Measure::Coord(j) => t0[j],
                Measure::Form(j, sign) => {
                    let v = normal()[j];
                    if sign {
                        v
                    } else {
                        -v
                    }
                }
            };
            coefs[0].push(n.w * c);
        }
        // 被積分関数が変数とパラメータだけの式なら、点で計算して足す（数になる）。そうでなければ Σ 係数 · f(点)
        let progs: Option<Vec<Program>> = fields.iter().map(|f| Program::compile_with(f, &names, self.params).ok()).collect();
        if let Some(progs) = progs {
            let mut total = 0.0;
            for (prog, cs) in progs.iter().zip(&coefs) {
                record_used_params(prog.used_params());
                total += at(prog).iter().zip(cs).map(|(f, c)| f * c).sum::<f64>();
            }
            return Ok(Expr::Num(total));
        }
        let mut terms = Vec::new();
        let mut count = 0;
        for (f, cs) in fields.iter().zip(&coefs) {
            for (n, c) in nodes.iter().zip(cs) {
                let put = f.substitute(&|v: &str| params.iter().position(|w| w == v).map(|l| Expr::Num(n.u[l])));
                let term = Expr::Bin(BinOp::Mul, Box::new(Expr::Num(*c)), Box::new(put));
                count += term.node_count();
                if count > MAX_NODES {
                    return Err("積分の式が大きくなりすぎます（等式で書いた曲線・曲面の上の、ほかの変数を含む積分）".to_string());
                }
                terms.push(term);
            }
        }
        Ok(if terms.is_empty() { Expr::Num(0.0) } else { balanced(terms, BinOp::Add) })
    }

    /// 積分の記号の d の後の文字 `diffs` から、媒介変数 `params`（k 個）・点の成分 dim 個の範囲の上で掛けるもの（測度）を決める。
    /// - 無し: 範囲の次元から（曲線は弧長、曲面は面積、立体は体積）
    /// - ds: 曲線の弧長、dS・dA・dσ: 曲面・平面の領域の面積、dV: 立体の体積（範囲の次元と違えば誤り。ds の範囲が曲面など）
    /// - 媒介変数そのもの（dt・du dv）: 何も掛けない
    /// - 座標（`coords`）: 曲線の上の dx は $x'(t)$、曲面の上の dx dy は法線の z 成分（向きの付いた面積）。
    ///   座標の数が範囲の次元と同じ（平面の領域の dx dy）なら、面積・体積
    /// - `vec`（F · dr・F · dS）: dr は曲線、dS は曲面
    fn measure_of(&self, k: usize, dim: usize, diffs: &[String], vec_mode: bool, params: &[String], coords: &[String]) -> Result<Measure, String> {
        let shape = shape_name(k, dim);
        let need = |want: usize, d: &str| -> Result<Measure, String> {
            if want == k {
                return Ok(Measure::Natural);
            }
            let hint = match k {
                1 => "曲線なら ds ",
                2 if dim == 2 => "平面の領域なら dA（dx dy）",
                2 => "曲面なら dS ",
                _ => "立体なら dV ",
            };
            Err(format!("d{d} は{}の上の積分に使う記号ですが、この範囲は{shape}です（{hint}と書いてください）", shape_name(want, dim.max(want))))
        };
        if vec_mode {
            return match diffs {
                [] => Ok(Measure::Natural),
                [d] if d == "r" || d == "s" => need(1, d).map_err(|_| format!("F · dr は曲線の上の積分ですが、この範囲は{shape}です（曲面なら F · dS と書いてください）")),
                [d] if d == "S" || d == "A" || d == "σ" || d == "sigma" => {
                    if k == 2 && dim == 3 {
                        Ok(Measure::Natural)
                    } else {
                        Err(format!("F · dS は 3 次元の曲面の上の積分ですが、この範囲は{shape}です（曲線なら F · dr と書いてください）"))
                    }
                }
                _ => Err(format!("ベクトル場の積分は F · dr か F · dS の形で書いてください（d{} は使えません）", diffs.join(" d"))),
            };
        }
        if diffs.is_empty() {
            return Ok(Measure::Natural);
        }
        // 弧長 ds・面積 dS・dA・dσ・体積 dV（ds はいつも弧長。ほかは、範囲の変数の名前なら、その変数の d）
        if let [d] = diffs {
            let own = params.iter().any(|p| p == d);
            match d.as_str() {
                "s" => return need(1, d),
                "S" | "A" | "σ" | "sigma" if !own => return need(2, if d == "sigma" { "σ" } else { d }),
                "V" if !own => return need(3, d),
                _ => {}
            }
        }
        // 媒介変数そのもの（並びは問わない）
        if diffs.len() == k && diffs.iter().all(|d| params.contains(d)) && (1..k).all(|i| !diffs[..i].contains(&diffs[i])) {
            // 平面の領域 {(x, y) | …} の dx dy もここ（点が変数の組なら、何も掛けないのが面積）
            return Ok(Measure::Plain);
        }
        // 座標の d（dx・dy・dz）
        let idx: Option<Vec<usize>> = diffs.iter().map(|d| coords[..dim.min(coords.len())].iter().position(|c| c == d)).collect();
        if let Some(idx) = idx {
            if (1..idx.len()).any(|i| idx[..i].contains(&idx[i])) {
                return Err(format!("積分の d{} が重なっています", diffs.join(" d")));
            }
            match (idx.len(), k, dim) {
                // 範囲と同じ次元の座標を全部並べた dx dy・dx dy dz は、面積・体積
                (n, _, _) if n == k && n == dim => return Ok(Measure::Natural),
                (1, 1, _) => return Ok(Measure::Coord(idx[0])),
                (2, 2, 3) => {
                    // dy dz → 法線の x 成分、dz dx → y 成分、dx dy → z 成分（逆の順なら向きが逆）
                    let missing = 3 - idx[0] - idx[1];
                    let sign = (idx[0] + 1) % 3 == idx[1];
                    return Ok(Measure::Form(missing, sign));
                }
                _ => {}
            }
            let ds = diffs.iter().map(|d| format!("d{d}")).collect::<Vec<_>>().join(" ");
            let want = match k {
                1 => "曲線なら ds か dx 1 つ".to_string(),
                2 if dim == 2 => "平面の領域なら dA か dx dy".to_string(),
                2 => "曲面なら dS か dx dy（向きの付いた面積）".to_string(),
                _ => "立体なら dV か dx dy dz".to_string(),
            };
            return Err(format!("{ds} は、この範囲の積分に合いません（範囲は{shape}。{want} と書いてください）"));
        }
        let ds = diffs.iter().map(|d| format!("d{d}")).collect::<Vec<_>>().join(" ");
        Err(format!(
            "積分の {ds} の文字が、範囲の変数（{}）にも、ds・dS・dA・dV にもなっていません",
            params.iter().chain(coords[..dim.min(coords.len())].iter()).cloned().collect::<Vec<_>>().join(", ")
        ))
    }

    /// 等式の条件 `eqs`（左, 右）を、変数 `vars` のどれかについて解く（その変数の 1 次式になっている等式だけ）。
    /// 解いた変数と式（残りの変数だけの式）の組と、解かずに残った変数（媒介変数）、1 次式に解けずに残った等式を返す。
    /// 後ろの変数から解く（x = y = z なら y = x, z = x で、x が媒介変数）。係数が数の変数を先に選ぶ（0 で割らない）
    fn eliminate(&mut self, vars: &[String], eqs: &[(Expr, Expr)]) -> Result<Solved, String> {
        use crate::diff::{diff, div, neg, sub};
        let mut solved: Vec<(String, Expr)> = Vec::new();
        let mut pending: Vec<Expr> = Vec::new();
        for (l, r) in eqs {
            pending.push(sub(self.scalar(l)?, self.scalar(r)?));
        }
        while !pending.is_empty() {
            let mut progress = false;
            let mut k = 0;
            while k < pending.len() {
                let back = |v: &str| solved.iter().find(|(w, _)| w == v).map(|(_, e)| e.clone());
                let g = pending[k].substitute(&back);
                let free: Vec<&String> = vars.iter().filter(|v| g.uses_var(v) && !solved.iter().any(|(w, _)| w == *v)).collect();
                if free.is_empty() {
                    // 変数の無い等式（1 = 1）は落とす。成り立たない等式（0 = 1）なら範囲が空
                    if self.numeric(&g).is_some_and(|c| c.abs() > 1e-12) {
                        return Err("等式の条件が成り立たないので、範囲が空です".into());
                    }
                    pending.remove(k);
                    progress = true;
                    continue;
                }
                // 1 次式になっている変数（係数が数のものを先に）
                let mut best: Option<(bool, &String, Expr)> = None;
                for v in free.iter().rev() {
                    let Ok(a) = diff(&g, v) else { continue };
                    if a.uses_var(v) {
                        continue;
                    }
                    let constant = match self.numeric(&a) {
                        Some(c) if c.abs() <= 1e-12 => continue,
                        Some(_) => true,
                        None => false,
                    };
                    if best.as_ref().is_none_or(|b| constant && !b.0) {
                        best = Some((constant, v, a));
                    }
                }
                let Some((_, v, a)) = best else {
                    k += 1;
                    continue;
                };
                let v = v.clone();
                let zero = |w: &str| (w == v).then_some(Expr::Num(0.0));
                let value = neg(div(g.substitute(&zero), a));
                let only = |w: &str| (w == v).then(|| value.clone());
                for (_, e) in &mut solved {
                    *e = e.substitute(&only);
                }
                solved.push((v, value));
                pending.remove(k);
                progress = true;
            }
            if !progress {
                // 1 次式に解けない等式（x² + y² = 1 など）は、残して返す（数値で点を探す: implicit.rs）
                break;
            }
        }
        let back = |v: &str| solved.iter().find(|(w, _)| w == v).map(|(_, e)| e.clone());
        let unsolved = pending.iter().map(|g| g.substitute(&back)).collect();
        let rest = vars.iter().filter(|v| !solved.iter().any(|(w, _)| w == *v)).cloned().collect();
        Ok((solved, rest, unsolved))
    }

    /// 変数を含まない式の値（パラメータは今の値）。変数があれば None。誤りを記録しない
    fn numeric(&self, e: &Expr) -> Option<f64> {
        let p = crate::eval::Program::compile_with(e, &[], self.params).ok()?;
        let mut out = [num_complex::Complex64::new(0.0, 0.0)];
        p.eval_batch(&[], &mut out);
        Some(crate::eval::as_real(out[0])).filter(|v| v.is_finite())
    }

    /// 1 次式の範囲で書いた領域（平行四辺形・平行六面体。`−1 ≤ x + y ≤ 1, 0 ≤ x − y ≤ 1`）の上の積分を、変数変換で累次積分にする。
    /// 下と上の両方の端がある 1 次式 $g_i = \sum_j a_{ij} x_j + b_i$ を変数の数だけ選び、新しい変数 $u_i = g_i$ に取り替える。
    /// $x = A^{-1}(u - b)$ を中身と残りの条件に入れ、ヤコビアン $1 / |\det A|$ を掛けて、$u$ の箱の上の積分にする。
    /// そういう 1 次式が足りない（または互いに独立でない）なら `None`
    fn linear_change(&mut self, vars: &[String], pairs: &[(Expr, bool, Expr)], body: &Expr) -> Result<Option<Expr>, String> {
        use crate::diff::{add, diff, div, mul, sub};
        let n = vars.len();
        let free = |e: &Expr| !vars.iter().any(|v| e.uses_var(v));
        // 条件ごとに、「数 ≤ 1 次式」（下の端）か「1 次式 ≤ 数」（上の端）かを調べる
        struct Form {
            g: Expr,
            coef: Vec<Expr>,
            lo: bool,
            hi: bool,
        }
        let mut forms: Vec<Form> = Vec::new();
        let mut role: Vec<Option<(usize, bool)>> = Vec::new();
        for (l, _, r) in pairs {
            let (g, lower) = match (free(l), free(r)) {
                (true, false) => (r, true),
                (false, true) => (l, false),
                _ => {
                    role.push(None);
                    continue;
                }
            };
            // 1 次式: どの変数で微分しても、変数が残らない
            let coef: Option<Vec<Expr>> = vars.iter().map(|v| diff(g, v).ok().filter(|d| free(d))).collect();
            let Some(coef) = coef else {
                role.push(None);
                continue;
            };
            let i = forms.iter().position(|f| f.g == *g).unwrap_or_else(|| {
                forms.push(Form { g: g.clone(), coef, lo: false, hi: false });
                forms.len() - 1
            });
            if lower {
                forms[i].lo = true;
            } else {
                forms[i].hi = true;
            }
            role.push(Some((i, lower)));
        }
        let chosen: Vec<usize> = (0..forms.len()).filter(|&i| forms[i].lo && forms[i].hi).take(n).collect();
        if chosen.len() < n {
            return Ok(None);
        }
        // 行列式（行 = 選んだ 1 次式の係数。列 col を w に取り替えたものも作る: クラメルの公式）
        let m = |a: &Expr, b: &Expr| mul(a.clone(), b.clone());
        let det = |rows: &[Vec<Expr>]| -> Expr {
            match rows.len() {
                1 => rows[0][0].clone(),
                2 => sub(m(&rows[0][0], &rows[1][1]), m(&rows[0][1], &rows[1][0])),
                _ => {
                    let minor = |j: usize, k: usize| sub(m(&rows[1][j], &rows[2][k]), m(&rows[1][k], &rows[2][j]));
                    add(sub(m(&rows[0][0], &minor(1, 2)), m(&rows[0][1], &minor(0, 2))), m(&rows[0][2], &minor(0, 1)))
                }
            }
        };
        let a: Vec<Vec<Expr>> = chosen.iter().map(|&i| forms[i].coef.clone()).collect();
        let d = det(&a);
        // 係数が数なら、独立かを確かめる（x + y と 2x + 2y では変換できない）
        if let Ok(v) = self.number(&d, "領域の条件") {
            if v.abs() < 1e-12 {
                return Ok(None);
            }
        }
        let zero = |v: &str| vars.iter().any(|w| w == v).then_some(Expr::Num(0.0));
        let names: Vec<String> = (0..n).map(|i| format!("⟨u{i}⟩")).collect();
        // w = u − b
        let w: Vec<Expr> = chosen.iter().zip(&names).map(|(&i, u)| sub(Expr::Var(u.clone()), forms[i].g.substitute(&zero))).collect();
        let xs: Vec<Expr> = (0..n)
            .map(|col| {
                let mut rows = a.clone();
                for (row, wi) in rows.iter_mut().zip(&w) {
                    row[col] = wi.clone();
                }
                div(det(&rows), d.clone())
            })
            .collect();
        let back = |v: &str| vars.iter().position(|w| w == v).map(|j| xs[j].clone());
        let body = self.scalar(body)?.substitute(&back);
        let body = div(body, Expr::Call(crate::functions::Func::Abs, vec![d]));
        let mut args = vec![Expr::Tuple(names.iter().map(|u| Expr::Var(u.clone())).collect())];
        for ((l, strict, r), role) in pairs.iter().zip(&role) {
            let op = if *strict { "rel:<" } else { "rel:≤" }.to_string();
            let u = |i: usize| chosen.iter().position(|&c| c == i).map(|k| Expr::Var(names[k].clone()));
            let sides = match role.and_then(|(i, lower)| u(i).map(|u| (u, lower))) {
                Some((u, true)) => vec![l.clone(), u],
                Some((u, false)) => vec![u, r.clone()],
                None => vec![l.substitute(&back), r.substitute(&back)],
            };
            args.push(Expr::Op { op, args: sides });
        }
        args.push(body);
        self.region_integral(&args).map(Some)
    }

    /// 範囲の条件（不等式の鎖・論理式）を「左 関係 右」の組にする（関係は '<' なら true、'≤' なら false。'>'・'≥' は左右を入れ替える）。
    /// 論理式（∨・¬ など）は、選言標準形にして 1 本の不等式 max_k min_j (右 − 左) ≥ 0 にまとめる
    fn condition_pairs(&mut self, vars: &[String], chains: &[Expr]) -> Result<Vec<(Expr, bool, Expr)>, String> {
        let mut pairs: Vec<(Expr, bool, Expr)> = Vec::new();
        for chain in chains {
            // 論理式の条件（∨・¬ など）: 選言標準形にして、1 本の不等式 max_k min_j (右 − 左) ≥ 0 にまとめる
            // （「かつ」のかたまり k のどれかを全部満たす）。等式は面積・体積が 0、≠ は面積・体積を変えないので落とす
            if let Expr::Cond(f) = chain {
                use crate::functions::Func;
                use crate::logic::Op as L;
                let lookup = |n: &str| set_def(n);
                let clauses = crate::logic::clauses(f, vars, &lookup)?;
                let mut alts = Vec::new();
                for clause in &clauses {
                    let mut gs = Vec::new();
                    for lit in clause {
                        match lit.op {
                            L::Lt | L::Le => gs.push(crate::diff::sub(self.scalar(&lit.r)?, self.scalar(&lit.l)?)),
                            L::Ne => {}
                            L::Eq => {} // 面積・体積 0 の部分（そのかたまりは落とす）
                        }
                    }
                    if clause.iter().any(|lit| lit.op == L::Eq) {
                        continue;
                    }
                    alts.push(gs.into_iter().reduce(|a, b| Expr::Call(Func::Min, vec![a, b])).unwrap_or(Expr::Num(1.0)));
                }
                let g = alts.into_iter().reduce(|a, b| Expr::Call(Func::Max, vec![a, b])).unwrap_or(Expr::Num(-1.0));
                pairs.push((Expr::Num(0.0), false, g));
                continue;
            }
            let Expr::Op { op, args: exprs } = chain else { return Err("内部エラー: 条件の形".into()) };
            let rels: Vec<char> = op.trim_start_matches("rel:").chars().collect();
            let exprs = exprs.iter().map(|e| self.scalar(e)).collect::<Result<Vec<_>, _>>()?;
            for (j, r) in rels.iter().enumerate() {
                let (l, rr) = (exprs[j].clone(), exprs[j + 1].clone());
                match r {
                    '<' => pairs.push((l, true, rr)),
                    '≤' => pairs.push((l, false, rr)),
                    '>' => pairs.push((rr, true, l)),
                    '≥' => pairs.push((rr, false, l)),
                    _ => return Err("積分する領域の条件に = は使えません（面積・体積が 0 になります）".into()),
                }
            }
        }
        Ok(pairs)
    }

    fn region_integral(&mut self, args: &[Expr]) -> Result<Expr, String> {
        use crate::functions::Func;
        let (Some(vars_e), Some(body)) = (args.first(), args.last()) else { return Err("内部エラー: 領域の積分の形".into()) };
        // 点が変数の組でなければ（媒介変数の式）、線積分・面積分・体積分。`; {vec}`・`; {d_s}` の印も、ここで取り出す
        let mut vec_mode = false;
        let mut diffs: Vec<String> = Vec::new();
        for a in &args[1..args.len() - 1] {
            let Expr::Op { op, .. } = a else { continue };
            let Some(m) = op.strip_prefix("mode:") else { continue };
            if m == "vec" {
                vec_mode = true;
            } else if let Some(d) = m.strip_prefix("d_") {
                diffs.push(d.to_string());
            } else {
                return Err(format!("; の後の {m} は使えません（{{vec}} か {{d_s}} のように書いてください）"));
            }
        }
        let chains: Vec<Expr> = args[1..args.len() - 1].iter().filter(|a| !matches!(a, Expr::Op { op, .. } if op.starts_with("mode:"))).cloned().collect();
        let plain_vars = |e: &Expr| -> bool {
            let names: Vec<&String> = match e {
                Expr::Var(v) => vec![v],
                Expr::Tuple(items) => items.iter().filter_map(|i| if let Expr::Var(v) = i { Some(v) } else { None }).collect(),
                _ => return false,
            };
            let n = if let Expr::Tuple(items) = e { items.len() } else { 1 };
            names.len() == n && (1..names.len()).all(|i| !names[..i].contains(&names[i]))
        };
        const XYZ: [&str; 3] = ["x", "y", "z"];
        if vec_mode || !plain_vars(vars_e) {
            let coords: Vec<String> = XYZ.iter().map(|c| c.to_string()).collect();
            return self.parametric_integral(vars_e, &chains, vec_mode, &diffs, None, &coords, body);
        }
        // 変数の組 (x, y, z) の範囲: 等式の条件（y = x², x = y = z）があれば、解いて曲線・曲面の積分にする。
        // 無ければ、積分の d の文字が範囲の次元と合うかだけ確かめて、今までの重積分
        let names: Vec<String> = match vars_e {
            Expr::Var(v) => vec![v.clone()],
            Expr::Tuple(items) => items.iter().filter_map(|i| if let Expr::Var(v) = i { Some(v.clone()) } else { None }).collect(),
            _ => Vec::new(),
        };
        if !split_equalities(&chains).0.is_empty() {
            return self.parametric_integral(vars_e, &chains, false, &diffs, Some(names.clone()), &names, body);
        }
        if !diffs.is_empty() {
            match self.measure_of(names.len(), names.len(), &diffs, false, &names, &names)? {
                Measure::Natural | Measure::Plain => {}
                _ => return Err("内部エラー: 領域の積分の測度".into()),
            }
        }
        let args: Vec<Expr> = std::iter::once(vars_e.clone()).chain(chains).chain(std::iter::once(body.clone())).collect();
        let args = args.as_slice();
        let vars: Vec<String> = match vars_e {
            Expr::Var(v) => vec![v.clone()],
            Expr::Tuple(items) => items
                .iter()
                .map(|e| match e {
                    Expr::Var(v) => Ok(v.clone()),
                    _ => Err("積分する領域の点は、(x, y) のように変数の組で書いてください".to_string()),
                })
                .collect::<Result<_, _>>()?,
            _ => return Err("積分する領域の点は、(x, y) のように変数の組で書いてください".into()),
        };
        if vars.len() > 3 {
            return Err("領域の積分は 3 重までにしてください".into());
        }
        // 条件を「左 関係 右」の組に分ける（関係は '<' か '≤'。'>'・'≥' は左右を入れ替える）
        let pairs = self.condition_pairs(&vars, &args[1..args.len() - 1])?;
        // 順番（外側から内側）ごとに、範囲に使える組を内側の変数から順に割り当てる
        let is_var = |e: &Expr, v: &str| matches!(e, Expr::Var(n) if n == v);
        let mut best: Option<(Vec<String>, Vec<(Vec<Expr>, Vec<Expr>)>, Vec<usize>)> = None;
        for order in permutations(&vars) {
            let mut used = vec![false; pairs.len()];
            let mut bounds: Vec<(Vec<Expr>, Vec<Expr>)> = vec![(Vec::new(), Vec::new()); order.len()];
            // 内側の変数の範囲「下の端 ≤ 上の端」から決まる、外側の変数の条件（1 ≤ x ≤ y なら 1 ≤ y）。
            // 外側の変数に範囲の端が書かれていないとき（1 ≤ x ≤ y, y ≤ 3 の y の下の端）だけ使う
            let mut implied: Vec<(Expr, Expr)> = Vec::new();
            for i in (0..order.len()).rev() {
                let v = &order[i];
                let inner = &order[i..];
                // 反対側が v と内側の変数を使わない
                let free = |e: &Expr| !inner.iter().any(|w| e.uses_var(w));
                for (k, (l, _, r)) in pairs.iter().enumerate() {
                    if used[k] {
                        continue;
                    }
                    if is_var(r, v) && free(l) {
                        bounds[i].0.push(l.clone());
                        used[k] = true;
                    } else if is_var(l, v) && free(r) {
                        bounds[i].1.push(r.clone());
                        used[k] = true;
                    }
                }
                if bounds[i].0.is_empty() {
                    bounds[i].0.extend(implied.iter().filter(|(l, r)| is_var(r, v) && free(l)).map(|(l, _)| l.clone()));
                }
                if bounds[i].1.is_empty() {
                    bounds[i].1.extend(implied.iter().filter(|(l, r)| is_var(l, v) && free(r)).map(|(_, r)| r.clone()));
                }
                for lo in &bounds[i].0 {
                    for hi in &bounds[i].1 {
                        implied.push((lo.clone(), hi.clone()));
                    }
                }
            }
            if bounds.iter().any(|(lo, hi)| lo.is_empty() || hi.is_empty()) {
                continue;
            }
            let rest: Vec<usize> = (0..pairs.len()).filter(|&k| !used[k]).collect();
            if best.as_ref().is_none_or(|b| rest.len() < b.2.len()) {
                best = Some((order, bounds, rest));
            }
        }
        let Some((order, bounds, rest)) = best else {
            // 変数ごとの範囲が無くても、条件が 1 次式の範囲（−1 ≤ x + y ≤ 1, 0 ≤ x − y ≤ 1）なら、変数変換で累次積分にする
            if let Some(e) = self.linear_change(&vars, &pairs, body)? {
                return Ok(e);
            }
            // 累次積分の形に書けない領域（x² + y² ≤ 1・x² ≤ y ≤ x + 2）: 領域の境目を数値で探して、点を置く
            if let Some(e) = self.general_region(&vars, &pairs, body)? {
                return Ok(e);
            }
            return Err(format!(
                "積分する領域は、それぞれの変数の範囲を書いてください（例: 0 ≤ {0} ≤ 1, 0 ≤ {1} ≤ {0}）",
                vars[0],
                vars.get(1).cloned().unwrap_or_else(|| "y".into())
            ));
        };
        // 範囲の形にならない条件: 条件の外で 0 になる因子を掛ける（近似）
        let mut f = body.clone();
        if !rest.is_empty() {
            // 境目を数値で探せる領域なら、そうする（条件の外を 0 にする近似より、ずっと正確）
            if let Ok(Some(e)) = self.general_region(&vars, &pairs, body) {
                return Ok(e);
            }
            APPROX.with(|a| *a.borrow_mut() = true);
            let mut piece = vec![Expr::Num(rest.len() as f64), Expr::Num(0.0)];
            let mut strict: u64 = 0;
            for (j, &k) in rest.iter().enumerate() {
                let (l, s, r) = &pairs[k];
                if *s {
                    strict |= 1 << j;
                }
                piece.push(Expr::Bin(BinOp::Sub, Box::new(r.clone()), Box::new(l.clone())));
            }
            piece[1] = Expr::Num(strict as f64);
            piece.extend([Expr::Num(1.0), Expr::Num(0.0), Expr::Num(0.0), Expr::Num(0.0)]);
            f = Expr::Bin(BinOp::Mul, Box::new(f), Box::new(Expr::Call(Func::Piece, piece)));
        }
        // 内側から積分で包む。下の端は max、上の端は min
        let fold = |es: &[Expr], g: Func| {
            es[1..].iter().fold(es[0].clone(), |acc, e| Expr::Call(g, vec![acc, e.clone()]))
        };
        for (v, (lo, hi)) in order.iter().zip(&bounds).rev() {
            let (lo, mut hi) = (fold(lo, Func::Max), fold(hi, Func::Min));
            // 端が外側の変数の式なら、下の端 > 上の端になる所（0 ≤ y ≤ sin x の sin x < 0 の所）は空。逆向きの積分（負の値）にしない
            if order.iter().any(|w| lo.uses_var(w) || hi.uses_var(w)) {
                hi = Expr::Call(Func::Max, vec![lo.clone(), hi]);
            }
            f = Expr::Binder { op: "int".into(), var: v.clone(), args: vec![lo, hi], body: Box::new(f) };
        }
        self.scalar(&f)
    }

    /// 累次積分の形に書けない領域の上の積分（`region.rs`）。条件が積分の変数とパラメータだけの式でなければ None
    /// （領域がほかの座標で変わるときは、点を置く場所を前もって決められない）。結果は Σ 重み · 中身(点)
    /// g を v の 2 次以下の式と見たときの係数（`solve.rs`）
    fn as_quadratic(&self, g: &Expr, v: &str, names: &[&str]) -> Option<Quadratic> {
        crate::solve::quadratic(g, v, names, self.params)
    }

    /// 3 重積分の領域で、ある変数 v の条件が全部「v の範囲」に直せるとき（v について 2 次以下の不等式で、v² の係数が負の数か 0）、
    /// v の積分を内側の 1 次元の積分 $\int_{\max_i l_i}^{\min_j h_j} f\,dv$ にし、残り 2 変数の領域（v を含まない条件と、
    /// 2 次式の判別式 ≥ 0、すべての l_i ≤ h_j）の上で 2 重の領域の積分（`region.rs`）にする。
    /// 球 x² + y² + z² ≤ 1 は −√(1 − x² − y²) ≤ z ≤ √(1 − x² − y²) と円板に、2 本の円柱 x² + z² ≤ 1, y² + z² ≤ 1 は
    /// z の範囲と正方形になる。3 次元の切り口を探すより、ずっと速く正確（2 次元の切り口だけ探せばよい）。直せなければ None
    fn reduced_region(&mut self, vars: &[String], pairs: &[(Expr, bool, Expr)], f: &crate::eval::Program, budget: &crate::quad::Budget) -> Option<f64> {
        use crate::diff::{add, div, mul, sub};
        use crate::eval::Program;
        use num_complex::Complex64 as C;
        let names: Vec<&str> = vars.iter().map(|s| s.as_str()).collect();
        'var: for k in (0..3).rev() {
            let v = vars[k].as_str();
            let outer: Vec<String> = vars.iter().enumerate().filter(|(i, _)| *i != k).map(|(_, s)| s.clone()).collect();
            let (mut lows, mut highs, mut rest) = (Vec::new(), Vec::new(), Vec::new());
            for (l, s, r) in pairs {
                if !l.uses_var(v) && !r.uses_var(v) {
                    rest.push((l.clone(), *s, r.clone()));
                    continue;
                }
                let g = sub(r.clone(), l.clone()); // g ≥ 0
                // 1. v の 2 次以下の式なら、根の公式で
                let quadratic = match self.as_quadratic(&g, v, &names) {
                    Some((_, b, c, Some(av), b_const)) => {
                        // 1 次式（a = 0）は、丸め誤差で a が 0 にならないこともあるので幅を持たせる
                        let av = if av.abs() <= 1e-12 { 0.0 } else { av };
                        if av < 0.0 {
                            // a v² + b v + c ≥ 0（a < 0）: 2 つの根の間。判別式が負なら空
                            let disc = sub(mul(b.clone(), b.clone()), mul(Expr::Num(4.0 * av), c));
                            let root = Expr::Call(crate::functions::Func::Sqrt, vec![disc.clone()]);
                            let two_a = Expr::Num(2.0 * av);
                            lows.push(div(add(Expr::Neg(Box::new(b.clone())), root.clone()), two_a.clone()));
                            highs.push(div(sub(Expr::Neg(Box::new(b)), root), two_a));
                            rest.push((Expr::Num(0.0), false, disc));
                            true
                        } else if av == 0.0 {
                            match b_const {
                                Some(bv) if bv > 0.0 => lows.push(div(Expr::Neg(Box::new(c)), Expr::Num(bv))),
                                Some(bv) if bv < 0.0 => highs.push(div(Expr::Neg(Box::new(c)), Expr::Num(bv))),
                                Some(_) => rest.push((Expr::Num(0.0), false, c)),
                                None => {}
                            }
                            b_const.is_some()
                        } else {
                            false // v² の係数が正: 2 つの根の外側（区間 1 つにならない）
                        }
                    }
                    _ => false,
                };
                // 2. v が 1 か所にしか出てこなければ、孤立法で（√x + √y + √z ≤ 1・|x| + |y| + |z| ≤ 1）
                if !quadratic {
                    let Some((lo, hi, conds)) = isolate(&g, v) else { continue 'var };
                    lows.extend(lo);
                    highs.extend(hi);
                    rest.extend(conds.into_iter().map(|c| (Expr::Num(0.0), false, c)));
                }
            }
            for lo in &lows {
                for hi in &highs {
                    rest.push((lo.clone(), false, hi.clone()));
                }
            }
            let onames: Vec<&str> = outer.iter().map(|s| s.as_str()).collect();
            let compile = |es: &[Expr]| es.iter().map(|e| Program::compile_with(e, &onames, self.params).ok()).collect::<Option<Vec<_>>>();
            let (Some(low_p), Some(high_p)) = (compile(&lows), compile(&highs)) else { continue };
            let Some((progs, known)) = self.region_programs(&outer, &rest) else { continue };
            // 中身が定数（体積など）なら、内側の積分は (上の端 − 下の端) · 定数（ばらばらな 3 点で値がぴったり同じなら定数とみなす）
            let fconst = {
                let mut out = [C::new(0.0, 0.0); 3];
                let probe = [[0.31, -0.47, 0.83], [1.7, 2.3, -1.1], [-2.9, 0.61, 3.7]];
                let cols: Vec<Vec<f64>> = (0..3).map(|i| probe.iter().map(|p| p[i]).collect()).collect();
                let inputs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
                f.eval_batch(&inputs, &mut out);
                (out[0] == out[1] && out[1] == out[2] && out[0].im == 0.0 && out[0].re.is_finite()).then_some(out[0].re)
            };
            // 範囲の端の式を、点の並びにまとめて計算する（1 点ずつ計算すると、呼び出しの手間が計算より大きい）
            let ends = |progs: &[Program], inputs: &[&[f64]], init: f64, pick: fn(f64, f64) -> f64| -> Vec<f64> {
                let len = inputs[0].len();
                let mut acc = vec![init; len];
                let mut vals = vec![C::new(0.0, 0.0); len];
                for p in progs {
                    p.eval_batch(inputs, &mut vals);
                    for (a, v) in acc.iter_mut().zip(&vals) {
                        *a = pick(*a, crate::eval::as_real(*v));
                    }
                }
                acc
            };
            let integrand = |inputs: &[&[f64]], out: &mut [f64]| {
                let los = ends(&low_p, inputs, f64::NEG_INFINITY, f64::max);
                let his = ends(&high_p, inputs, f64::INFINITY, f64::min);
                for (j, o) in out.iter_mut().enumerate() {
                    let pt = [inputs[0][j], inputs[1][j]];
                    let (lo, hi) = (los[j], his[j]);
                    if hi <= lo || hi.is_nan() || lo.is_nan() {
                        *o = 0.0;
                        continue;
                    }
                    if let Some(c) = fconst {
                        *o = c * (hi - lo);
                        continue;
                    }
                    let mut g = |ts: &[f64], vals: &mut [f64]| {
                        let cols: Vec<Vec<f64>> = (0..3)
                            .map(|i| if i == k { ts.to_vec() } else { vec![pt[if i < k { i } else { i - 1 }]; ts.len()] })
                            .collect();
                        let ins: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
                        let mut res = vec![C::new(0.0, 0.0); ts.len()];
                        f.eval_batch(&ins, &mut res);
                        for (o, r) in vals.iter_mut().zip(&res) {
                            *o = crate::eval::as_real(*r);
                        }
                    };
                    *o = crate::quad::integrate(&mut g, lo, hi, crate::quad::Tol { rel: 1e-11, panels: 100, ends: true }, budget);
                }
            };
            return crate::region::integrate(progs, 2, &known, &integrand, budget);
        }
        None
    }

    /// 条件の組（左 ≤ 右）を、領域の条件の式（右 − 左 ≥ 0）と、数で書かれた変数ごとの端にする。式にできなければ None
    fn region_programs(&mut self, vars: &[String], pairs: &[(Expr, bool, Expr)]) -> Option<RegionPrograms> {
        let names: Vec<&str> = vars.iter().map(|v| v.as_str()).collect();
        let free = |e: &Expr| !vars.iter().any(|v| e.uses_var(v));
        let mut progs = Vec::new();
        let mut known: Vec<(Option<f64>, Option<f64>)> = vec![(None, None); vars.len()];
        for (l, _, r) in pairs {
            let g = Expr::Bin(BinOp::Sub, Box::new(r.clone()), Box::new(l.clone()));
            progs.push(crate::eval::Program::compile_with(&g, &names, self.params).ok()?);
            for (i, v) in vars.iter().enumerate() {
                let is_v = |e: &Expr| matches!(e, Expr::Var(name) if name == v);
                if is_v(r) && free(l) {
                    if let Ok(a) = self.number(l, "領域の条件") {
                        known[i].0 = Some(known[i].0.map_or(a, |b: f64| b.max(a)));
                    }
                } else if is_v(l) && free(r) {
                    if let Ok(a) = self.number(r, "領域の条件") {
                        known[i].1 = Some(known[i].1.map_or(a, |b: f64| b.min(a)));
                    }
                }
            }
        }
        Some((progs, known))
    }

    fn general_region(&mut self, vars: &[String], pairs: &[(Expr, bool, Expr)], body: &Expr) -> Result<Option<Expr>, String> {
        let n = vars.len();
        let names: Vec<&str> = vars.iter().map(|v| v.as_str()).collect();
        let free = |e: &Expr| !vars.iter().any(|v| e.uses_var(v));
        let mut progs = Vec::new();
        let mut known: Vec<(Option<f64>, Option<f64>)> = vec![(None, None); n];
        let mut used: Vec<String> = Vec::new();
        for (l, _, r) in pairs {
            let g = Expr::Bin(BinOp::Sub, Box::new(r.clone()), Box::new(l.clone()));
            let Ok(prog) = crate::eval::Program::compile_with(&g, &names, self.params) else { return Ok(None) };
            used.extend(prog.used_params().iter().cloned());
            progs.push(prog);
            // 数で書かれた端（0 ≤ x・x ≤ 1）は、領域を探す箱にそのまま使う
            for (i, v) in vars.iter().enumerate() {
                let is_v = |e: &Expr| matches!(e, Expr::Var(name) if name == v);
                if is_v(r) && free(l) {
                    if let Ok(a) = self.number(l, "領域の条件") {
                        known[i].0 = Some(known[i].0.map_or(a, |b: f64| b.max(a)));
                    }
                } else if is_v(l) && free(r) {
                    if let Ok(a) = self.number(r, "領域の条件") {
                        known[i].1 = Some(known[i].1.map_or(a, |b: f64| b.min(a)));
                    }
                }
            }
        }
        USED_PARAMS.with(|u| {
            let mut u = u.borrow_mut();
            for p in used {
                if !u.contains(&p) {
                    u.push(p);
                }
            }
        });
        // 値が定数になる積分（中身が積分の変数とパラメータだけの式）は、適応型の積分で数にする
        self.int_depth += n;
        let closed = self.scalar(body).ok().and_then(|g| crate::eval::Program::compile_with(&g, &names, self.params).ok());
        self.int_depth -= n;
        if let Some(f) = closed {
            USED_PARAMS.with(|u| {
                let mut u = u.borrow_mut();
                for p in f.used_params() {
                    if !u.contains(p) {
                        u.push(p.clone());
                    }
                }
            });
            let budget = crate::quad::Budget::new(CLOSED_BUDGET);
            // 3 重積分で、ある変数の条件が全部その変数の範囲に直せるなら、その変数を内側の 1 次元の積分にする
            let reduced = if n == 3 { self.reduced_region(vars, pairs, &f, &budget) } else { None };
            let rv = reduced.or_else(|| crate::region::integrate(progs.clone(), n, &known, &crate::region::program_integrand(&f), &budget));
            if let Some(v) = rv {
                if budget.rough.get() {
                    add_note("積分の誤差の見積もりが目標（10⁻¹⁰）に届いていません（端で発散する・激しく振動するなど）");
                }
                return Ok(Some(Expr::Num(v)));
            }
        }
        let points = crate::region::quadrature(progs, n, &known)?;
        self.int_depth += n;
        let result = (|| {
            let symbolic = self.scalar(body).ok();
            let mut terms = Vec::with_capacity(points.len());
            let mut nodes_count = 0;
            for (p, w) in &points {
                let put = |e: &Expr| e.substitute(&|v: &str| vars.iter().position(|name| name == v).map(|k| Expr::Num(p[k])));
                let g = match &symbolic {
                    Some(s) => put(s),
                    None => self.scalar(&put(body))?,
                };
                let term = Expr::Bin(BinOp::Mul, Box::new(Expr::Num(*w)), Box::new(g));
                nodes_count += term.node_count();
                if nodes_count > MAX_NODES {
                    return Err("積分の式が大きくなりすぎます（入れ子を減らしてください）".to_string());
                }
                terms.push(term);
            }
            Ok(if terms.is_empty() { Expr::Num(0.0) } else { balanced(terms, BinOp::Add) })
        })();
        self.int_depth -= n;
        result.map(Some)
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
        if v.is_infinite() {
            // Σ_{k=1}^{∞} など（Σ は項を全部書き並べて計算するので、無限には足せない）。
            // 前は「実数の値になりません」と出て、何がいけないのか分からなかった
            return Err(format!("{what}に ∞ は使えません（有限の値にしてください）"));
        }
        if !v.is_finite() {
            return Err(format!("{what}が実数の値になりません"));
        }
        Ok(v)
    }

    /// 整数の値（整数から 10⁻⁹ より離れていればエラー）
    /// テイラー多項式 $T_n(\mathbf x) = \sum_{|\alpha| \le n} \frac{\partial^\alpha f(\mathbf a)}{\alpha!} (\mathbf x - \mathbf a)^\alpha$。
    /// 偏導関数は記号微分（[`crate::diff::diff`]）で作り、中心 $\mathbf a$ を入れる。係数が数（とパラメータ）だけなら数にし、
    /// 説明に多項式を出す（`T₅(x) = x − x³/6 + x⁵/120`）
    fn taylor(&mut self, args: &[Expr]) -> Result<Expr, String> {
        let [f, vars, centers, order] = args else {
            return Err("テイラー展開は Taylor(f, x, a, n)（多変数なら Taylor(f, (x, y), (a, b), n)）の形で書いてください".into());
        };
        let names: Vec<String> = match vars {
            Expr::Var(v) => vec![v.clone()],
            Expr::Tuple(items) => items
                .iter()
                .map(|v| match v {
                    Expr::Var(v) => Ok(v.clone()),
                    _ => Err("テイラー展開の 2 つ目には、変数の名前（x か (x, y)）を書いてください".to_string()),
                })
                .collect::<Result<_, _>>()?,
            _ => return Err("テイラー展開の 2 つ目には、変数の名前（x か (x, y)）を書いてください".into()),
        };
        let k = names.len();
        if !(1..=3).contains(&k) || names.iter().enumerate().any(|(i, n)| names[..i].contains(n)) {
            return Err("テイラー展開の変数は、違う名前を 1〜3 個にしてください".into());
        }
        let centers: Vec<Expr> = match (centers, k) {
            (Expr::Tuple(items), _) if items.len() == k => items.iter().map(|c| self.scalar(c)).collect::<Result<_, _>>()?,
            (Expr::Tuple(_), 1) => return Err("1 変数のテイラー展開の中心は、数 1 つにしてください".into()),
            (c, 1) => vec![self.scalar(c)?],
            _ => return Err(format!("テイラー展開の中心は、変数と同じ {k} 個の組 (a, b{}) にしてください", if k == 3 { ", c" } else { "" })),
        };
        if centers.iter().any(|c| names.iter().any(|v| c.uses_var(v))) {
            return Err("テイラー展開の中心に、展開する変数は使えません".into());
        }
        let n = self.integer(order, "テイラー展開の次数")?;
        let max = [20, 10, 6][k - 1];
        if !(0..=max).contains(&n) {
            return Err(format!("{k} 変数のテイラー展開の次数は 0〜{max} にしてください"));
        }
        let n = n as usize;
        // f は式か、引数が変数の数と同じ関数（関数の名前だけでもよい）
        let body = match self.val(f)? {
            Val::S(s) => s,
            Val::Fn { arity, body } if arity == k => apply_body(&body, &names.iter().map(|v| Expr::Var(v.clone())).collect::<Vec<_>>()),
            Val::Fn { .. } => return Err(format!("テイラー展開する関数の引数の数を、変数の数（{k}）にそろえてください")),
        };
        if matches!(body, Expr::Tuple(_) | Expr::List(_)) || Mat::of(&body).is_some() {
            return Err("テイラー展開できるのは、値が数になる式です（ベクトル・リストは成分ごとに書いてください）".into());
        }
        // 1 変数で中心が数なら、べき級数の計算で係数を直接求める（記号微分で 20 階まで微分すると式が大きくなりすぎる。[`crate::series`]）
        if k == 1 {
            if let Some(a) = self.constant_value(&centers[0]) {
                let value = |e: &Expr| self.constant_value(e);
                if let Some(coefs) = crate::series::taylor(&body, &names[0], a, n, &value) {
                    let real = coefs.iter().all(|c| c.im.abs() <= 1e-12 * c.re.abs().max(1.0)) && a.im == 0.0;
                    let mut terms = Vec::new();
                    for (j, c) in coefs.iter().enumerate() {
                        if c.norm() < 1e-14 {
                            continue;
                        }
                        let coef = if c.im.abs() <= 1e-12 * c.re.abs().max(1.0) {
                            Expr::Num(c.re)
                        } else {
                            crate::diff::add(Expr::Num(c.re), crate::diff::mul(Expr::Num(c.im), Expr::Const(crate::parser::Constant::I)))
                        };
                        let shift = crate::diff::sub(Expr::Var(names[0].clone()), centers[0].clone());
                        terms.push(if j == 0 { coef } else { crate::diff::mul(coef, crate::diff::pow(shift, Expr::Num(j as f64))) });
                    }
                    if real {
                        let shown: Vec<(Vec<usize>, num_complex::Complex64)> = coefs.iter().enumerate().map(|(j, c)| (vec![j], num_complex::Complex64::new(c.re, 0.0))).collect();
                        add_note(&taylor_text(n, &names, &[a.re], &shown));
                    }
                    return Ok(balanced(terms, BinOp::Add));
                }
            }
        }
        // 多重指数 α を次数の順に。∂^α f は、α から 1 つ減らした指数の偏導関数をもう 1 回微分して作る
        let mut indices: Vec<Vec<usize>> = vec![vec![0; k]];
        let mut derivs: Vec<Expr> = vec![body];
        let mut terms: Vec<Expr> = Vec::new();
        let mut shown: Vec<(Vec<usize>, num_complex::Complex64)> = Vec::new();
        let mut numeric = true;
        let at = |e: &Expr| e.substitute(&|v: &str| names.iter().position(|n| n == v).map(|i| centers[i].clone()));
        let mut start = 0;
        for degree in 0..=n {
            if degree > 0 {
                let end = indices.len();
                for p in start..end {
                    // 最後に 0 でない成分以降だけを増やす（同じ指数を 2 回作らない）
                    let last = indices[p].iter().rposition(|&a| a > 0).unwrap_or(0);
                    for j in last..k {
                        let mut a = indices[p].clone();
                        a[j] += 1;
                        let d = crate::diff::diff(&derivs[p], &names[j])?;
                        if d.node_count() > 200_000 {
                            return Err("テイラー展開の導関数が大きくなりすぎました（次数を下げてください）".into());
                        }
                        indices.push(a);
                        derivs.push(d);
                    }
                }
                start = end;
            } else {
                start = 0;
            }
            for p in start..indices.len() {
                let alpha = &indices[p];
                let fact: f64 = alpha.iter().map(|&a| (1..=a).map(|i| i as f64).product::<f64>()).product();
                let coef = crate::diff::div(at(&derivs[p]), Expr::Num(fact));
                // 数とパラメータだけの係数は、数にする（式が短くなり、説明にも出せる）
                let coef = match self.constant_value(&coef) {
                    Some(c) if c.im.abs() <= 1e-12 * c.re.abs().max(1.0) => {
                        shown.push((alpha.clone(), num_complex::Complex64::new(c.re, 0.0)));
                        Expr::Num(c.re)
                    }
                    _ => {
                        numeric = false;
                        coef
                    }
                };
                if matches!(coef, Expr::Num(v) if v.abs() < 1e-14) {
                    continue;
                }
                let mut term = coef;
                for (i, &a) in alpha.iter().enumerate() {
                    if a > 0 {
                        let shift = crate::diff::sub(Expr::Var(names[i].clone()), centers[i].clone());
                        term = crate::diff::mul(term, crate::diff::pow(shift, Expr::Num(a as f64)));
                    }
                }
                terms.push(term);
            }
        }
        // 中心が数なら、説明に多項式を出す
        let center_values: Option<Vec<f64>> = centers.iter().map(|c| self.constant_value(c).filter(|c| c.im == 0.0).map(|c| c.re)).collect();
        if let (true, Some(cv)) = (numeric, center_values) {
            add_note(&taylor_text(n, &names, &cv, &shown));
        }
        Ok(balanced(terms, BinOp::Add))
    }

    /// 数とパラメータだけの式の値（変数を含めば None）。使ったパラメータを覚える
    fn constant_value(&self, e: &Expr) -> Option<num_complex::Complex64> {
        self.numeric_value(e).filter(|v| v.re.is_finite() && v.im.is_finite())
    }

    fn numeric_value(&self, e: &Expr) -> Option<num_complex::Complex64> {
        // 数列で既に数にした項や初期値を、表と描画のために再コンパイルしない。
        if let Expr::Num(n) = e {
            return Some(num_complex::Complex64::new(*n, 0.0));
        }
        if let Expr::Var(name) = e {
            let value = self.params.get(name)?;
            note_param(name);
            return Some(*value);
        }
        let prog = crate::eval::Program::compile_with(e, &[], self.params).ok()?;
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
        Some(out[0])
    }

    fn integer(&mut self, e: &Expr, what: &str) -> Result<i64, String> {
        let v = self.number(e, what)?;
        let r = v.round();
        if (v - r).abs() > 1e-9 {
            return Err(format!("{what}は整数にしてください（今は {v}。スライダーなら、刻みを 1 にしてください）"));
        }
        Ok(r as i64)
    }

    /// Σ・Π: 束縛する変数に a, a + 1, …, b を入れた中身を、平衡のとれた木で足す・掛ける。b < a なら 0（Π は 1）
    /// `Σ_{var=a}^{∞} body`（値が定数になるもの）。項の式をコンパイルして [`sum_to_infinity`] で計算する
    fn infinite_sum(&mut self, var: &str, a: i64, body: &Expr) -> Result<f64, String> {
        let g = self.scalar(body)?;
        let f = crate::eval::Program::compile_with(&g, &[var], self.params)
            .map_err(|_| "∞ までの総和は、値が数になる式（座標を含まない式）だけ計算できます".to_string())?;
        USED_PARAMS.with(|u| {
            let mut u = u.borrow_mut();
            for p in f.used_params() {
                if !u.contains(p) {
                    u.push(p.clone());
                }
            }
        });
        sum_to_infinity(&f, a)
    }

    /// `Σ_{var=a}^{∞} body` で、中身に座標などの変数があるもの。項の大きさを表示範囲の点（決まった 257 点）で調べ、
    /// 項の最大が、それまでの和の大きさの 10⁻⁹ 倍より小さい項が 4 つ続いたら、そこまでの和にする（説明に打ち切った所を出す）。
    /// 400 項までに小さくならなければ、400 項で打ち切る（収束しない・遅い級数。説明に出す）
    fn truncated_sum(&mut self, var: &str, a: i64, g: &Expr) -> Result<Expr, String> {
        const CAP: i64 = 400;
        let mut vars: Vec<String> = Vec::new();
        g.collect_vars(&mut vars);
        vars.retain(|v| v != var && !self.params.contains_key(v));
        vars.dedup();
        let mut sorted = vars.clone();
        sorted.sort();
        sorted.dedup();
        let vars = sorted;
        let names: Vec<&str> = vars.iter().map(|s| s.as_str()).collect();
        // 変数ごとの範囲: 表示中の座標は表示範囲、そのほかは [−10, 10]
        let vb = view_box();
        let range = |v: &str| -> (f64, f64) {
            match crate::equation::coord_index(v) {
                Some(0) => (vb[0], vb[1]),
                Some(1) => (vb[2], vb[3]),
                Some(2) => (vb[4], vb[5]),
                _ => (-10.0, 10.0),
            }
        };
        // 決まった点（ファン・デル・コルプト列。変数ごとに別の素数の基数）と、範囲の両端・真ん中
        const PRIMES: [u32; 8] = [2, 3, 5, 7, 11, 13, 17, 19];
        let radical = |mut i: u32, b: u32| {
            let (mut f, mut r) = (1.0, 0.0);
            while i > 0 {
                f /= b as f64;
                r += f * (i % b) as f64;
                i /= b;
            }
            r
        };
        let m = 257usize;
        let samples: Vec<Vec<f64>> = vars
            .iter()
            .enumerate()
            .map(|(j, v)| {
                let (lo, hi) = range(v);
                (0..m).map(|i| lo + (hi - lo) * if i == 0 { 0.5 } else { radical(i as u32, PRIMES[j % 8]) }).collect()
            })
            .collect();
        let inputs: Vec<&[f64]> = samples.iter().map(|s| s.as_slice()).collect();
        let mut terms = Vec::new();
        let mut partial = vec![num_complex::Complex64::new(0.0, 0.0); m];
        let mut small = 0;
        let mut nodes = 0;
        let mut n = a;
        let mut converged = false;
        while n - a < CAP {
            let t = g.substitute(&|v: &str| (v == var).then_some(Expr::Num(n as f64)));
            let prog = crate::eval::Program::compile_with(&t, &names, self.params)
                .map_err(|e| format!("∞ までの総和の項を計算できません（{e}）"))?;
            USED_PARAMS.with(|u| {
                let mut u = u.borrow_mut();
                for p in prog.used_params() {
                    if !u.contains(p) {
                        u.push(p.clone());
                    }
                }
            });
            let mut out = vec![num_complex::Complex64::new(0.0, 0.0); m];
            prog.eval_batch(&inputs, &mut out);
            let mut biggest = 0.0f64;
            let mut size = 0.0f64;
            for (s, o) in partial.iter_mut().zip(&out) {
                if o.re.is_finite() && o.im.is_finite() {
                    *s += *o;
                    biggest = biggest.max(o.norm());
                }
                if s.re.is_finite() && s.im.is_finite() {
                    size = size.max(s.norm());
                }
            }
            nodes += t.node_count();
            if nodes > MAX_NODES {
                return Err("∞ までの総和の式が大きくなりすぎます".into());
            }
            terms.push(t);
            n += 1;
            if biggest <= 1e-9 * size.max(1e-300) || biggest == 0.0 {
                small += 1;
                if small >= 4 {
                    converged = true;
                    break;
                }
            } else {
                small = 0;
            }
        }
        add_note(&if converged {
            format!("Σ の ∞ は {var} = {} までの和。その先の項は表示範囲で和の 10⁻⁹ 倍より小さい", n - 1)
        } else {
            format!("Σ の ∞ は {var} = {} までで打ち切り（項がまだ小さくならない）", n - 1)
        });
        Ok(balanced(terms, BinOp::Add))
    }

    fn sum_or_prod(&mut self, sum: bool, var: &str, args: &[Expr], body: &Expr) -> Result<Expr, String> {
        let sym = if sum { "Σ" } else { "Π" };
        let [lo, hi] = args else {
            return Err(format!("{sym} は、範囲を {sym}_{{{var}=1}}^{{10}} の形で書いてください"));
        };
        let what = format!("{sym} の範囲");
        // 上の端が ∞: 値が定数になる総和だけ、級数の加速で計算する
        if matches!(self.scalar(hi)?, Expr::Num(v) if v == f64::INFINITY) {
            if !sum {
                return Err("Π の範囲に ∞ は使えません（有限の範囲にしてください）".into());
            }
            let a = self.integer(lo, &what)?;
            // 中身に座標など（値が 1 つの数にならない変数）があれば、項が表示範囲で十分小さくなる所で打ち切った和にする
            // （ワイエルシュトラウス関数 Σ aⁿ cos(bⁿ π x) など）
            let g = self.scalar(body)?;
            if crate::eval::Program::compile_with(&g, &[var], self.params).is_err() {
                return self.truncated_sum(var, a, &g);
            }
            return self.infinite_sum(var, a, body).map(Expr::Num);
        }
        let (a, b) = (self.integer(lo, &what)?, self.integer(hi, &what)?);
        if b < a {
            return Ok(Expr::Num(if sum { 0.0 } else { 1.0 }));
        }
        if (b - a) as u64 >= MAX_TERMS as u64 {
            return Err(format!("{sym} の項が多すぎます（{} 個。{MAX_TERMS} 個まで）", b - a + 1));
        }
        let mut terms = Vec::with_capacity((b - a + 1) as usize);
        let mut nodes = 0;
        // 先に中身を記号のまま 1 回展開して、数を入れる（中の微分・差分は、その変数の式になってから数が入る）。
        // 展開できなければ（範囲が束縛する変数で決まる入れ子の Σ など）、項ごとに数を入れてから展開する
        let symbolic = self.scalar(body).ok();
        for k in a..=b {
            let put = |e: &Expr| e.substitute(&|v: &str| (v == var).then_some(Expr::Num(k as f64)));
            let t = match &symbolic {
                Some(s) => put(s),
                None => self.scalar(&put(body))?,
            };
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
            (Val::S(a), Val::S(b)) => match linalg::binary(op, &a, &b)? {
                Some(v) => Val::S(v),
                None => Val::S(mk(a, b)),
            },
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
                    if c.is_minus_one() {
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
/// 不等式 g ≥ 0 を、変数 v の範囲（下の端の並び・上の端の並び）と、v を含まない条件（≥ 0 になるべき式の並び）に解く。
/// v が g の中に 1 か所しか出てこないときの「孤立法」: g = A ± u（u だけが v を含む）から u の範囲を出し、
/// u の外側の演算から順に逆の演算を当てて v の範囲にする（符号・定数倍・定数の足し引き・√・|·|・2 乗・exp・ln）。
/// 解けなければ None
fn isolate(g: &Expr, v: &str) -> Option<(Vec<Expr>, Vec<Expr>, Vec<Expr>)> {
    let (mut lows, mut highs, mut conds) = (Vec::new(), Vec::new(), Vec::new());
    // g ≥ 0 ⇔ 0 ≤ g: 「u ≥ 0」を解く
    bound_of(g, Expr::Num(0.0), false, v, &mut lows, &mut highs, &mut conds)?;
    Some((lows, highs, conds))
}

/// 和・差を (符号, 項) に分ける
fn add_terms(e: &Expr, sign: f64, out: &mut Vec<(f64, Expr)>) {
    match e {
        Expr::Bin(BinOp::Add, a, b) => {
            add_terms(a, sign, out);
            add_terms(b, sign, out);
        }
        Expr::Bin(BinOp::Sub, a, b) => {
            add_terms(a, sign, out);
            add_terms(b, -sign, out);
        }
        Expr::Neg(a) => add_terms(a, -sign, out),
        _ => out.push((sign, e.clone())),
    }
}

/// u ≤ b（upper）・u ≥ b（!upper）を、v の範囲と条件に解いて足す。解けなければ None
#[allow(clippy::too_many_arguments)]
fn bound_of(u: &Expr, b: Expr, upper: bool, v: &str, lows: &mut Vec<Expr>, highs: &mut Vec<Expr>, conds: &mut Vec<Expr>) -> Option<()> {
    use crate::diff::{add, div, mul, sub};
    use crate::functions::Func;
    let neg = |e: Expr| Expr::Neg(Box::new(e));
    let num = |e: &Expr| match e {
        Expr::Num(k) => Some(*k),
        Expr::Neg(a) => match &**a {
            Expr::Num(k) => Some(-*k),
            _ => None,
        },
        _ => None,
    };
    if matches!(u, Expr::Var(n) if n == v) {
        if upper { highs.push(b) } else { lows.push(b) }
        return Some(());
    }
    if !u.uses_var(v) {
        // v を含まない: 条件そのもの（u ≤ b なら b − u ≥ 0）
        conds.push(if upper { sub(b, u.clone()) } else { sub(u.clone(), b) });
        return Some(());
    }
    match u {
        Expr::Bin(BinOp::Add | BinOp::Sub, ..) | Expr::Neg(_) => {
            // u = A + σ w（w だけが v を含む）: σ w ≤ b − A
            let mut terms = Vec::new();
            add_terms(u, 1.0, &mut terms);
            let with: Vec<&(f64, Expr)> = terms.iter().filter(|t| t.1.uses_var(v)).collect();
            if with.len() != 1 {
                return None;
            }
            let (sigma, w) = with[0].clone();
            let mut rest = b;
            for (s, t) in terms.iter().filter(|t| !t.1.uses_var(v)) {
                rest = if *s > 0.0 { sub(rest, t.clone()) } else { add(rest, t.clone()) };
            }
            if sigma > 0.0 {
                bound_of(&w, rest, upper, v, lows, highs, conds)
            } else {
                bound_of(&w, neg(rest), !upper, v, lows, highs, conds)
            }
        }
        // 定数倍 k w・w k・w / k
        Expr::Bin(BinOp::Mul, a, c) if num(a).is_some() || num(c).is_some() => {
            let (k, w) = match num(a) {
                Some(k) => (k, &**c),
                None => (num(c)?, &**a),
            };
            if k == 0.0 {
                return None;
            }
            bound_of(w, div(b, Expr::Num(k)), if k > 0.0 { upper } else { !upper }, v, lows, highs, conds)
        }
        Expr::Bin(BinOp::Div, w, c) if num(c).is_some() => {
            let k = num(c)?;
            if k == 0.0 {
                return None;
            }
            bound_of(w, mul(b, Expr::Num(k)), if k > 0.0 { upper } else { !upper }, v, lows, highs, conds)
        }
        // √w ≤ b ⇔ b ≥ 0 かつ 0 ≤ w ≤ b²、√w ≥ b ⇔ w ≥ max(b, 0)²
        Expr::Call(Func::Sqrt, args) if args.len() == 1 => {
            let w = &args[0];
            if upper {
                conds.push(b.clone());
                bound_of(w, Expr::Num(0.0), false, v, lows, highs, conds)?;
                bound_of(w, mul(b.clone(), b), true, v, lows, highs, conds)
            } else {
                let m = Expr::Call(Func::Max, vec![b, Expr::Num(0.0)]);
                bound_of(w, mul(m.clone(), m), false, v, lows, highs, conds)
            }
        }
        // |w| ≤ b ⇔ −b ≤ w ≤ b（|w| ≥ b は区間 2 つになるので解かない）
        Expr::Call(Func::Abs, args) if args.len() == 1 && upper => {
            bound_of(&args[0], b.clone(), true, v, lows, highs, conds)?;
            bound_of(&args[0], neg(b), false, v, lows, highs, conds)
        }
        // w² ≤ b ⇔ b ≥ 0 かつ −√b ≤ w ≤ √b
        Expr::Bin(BinOp::Pow, w, e) if upper && num(e) == Some(2.0) => {
            conds.push(b.clone());
            let r = Expr::Call(Func::Sqrt, vec![b]);
            bound_of(w, r.clone(), true, v, lows, highs, conds)?;
            bound_of(w, neg(r), false, v, lows, highs, conds)
        }
        // e^w ≤ b ⇔ b > 0 かつ w ≤ ln b（e^w ≥ b は b ≤ 0 で全部になるので解かない）
        Expr::Call(Func::Exp, args) if args.len() == 1 && upper => {
            conds.push(b.clone());
            bound_of(&args[0], Expr::Call(Func::Ln, vec![b]), true, v, lows, highs, conds)
        }
        // ln w ≤ b ⇔ 0 ≤ w ≤ e^b、ln w ≥ b ⇔ w ≥ e^b
        Expr::Call(Func::Ln, args) if args.len() == 1 => {
            let eb = Expr::Call(Func::Exp, vec![b]);
            if upper {
                bound_of(&args[0], Expr::Num(0.0), false, v, lows, highs, conds)?;
            }
            bound_of(&args[0], eb, upper, v, lows, highs, conds)
        }
        _ => None,
    }
}

/// `Σ_{k=a}^{∞} f(k)`（計画 `docs/plan-render-algorithms.md` の P1-1。A33）。次の順に試し、2 通りの計算が合ったものを返す:
/// 1. そのまま足す（項が和の 10⁻¹⁷ 倍より小さくなれば終わり。13 万項まで）
/// 2. 交代級数（項の符号が交互で、絶対値が減っていく）: Cohen・Villegas・Zagier の加速（項 n 個で誤差 5.8⁻ⁿ ほど）。n = 30 と 40 で比べる
/// 3. オイラー・マクローリンの和公式: Σ_{k=a}^{N−1} f(k) + ∫_N^∞ f(x) dx + f(N)/2 − f′(N)/12（f は整数でない x でも実数になること）。
///    N を 2 通りに変えて比べる。∫ が収束しなければ発散
///
/// どれも合わなければ、発散するか収束が遅すぎるとして誤りにする
fn sum_to_infinity(f: &crate::eval::Program, a: i64) -> Result<f64, String> {
    use num_complex::Complex64 as C;
    let eval = |xs: &[f64]| -> Vec<f64> {
        let mut out = vec![C::new(0.0, 0.0); xs.len()];
        f.eval_batch(&[xs], &mut out);
        out.iter().map(|v| crate::eval::as_real(*v)).collect()
    };
    let terms = |from: i64, n: usize| eval(&(0..n).map(|k| (from + k as i64) as f64).collect::<Vec<_>>());
    let close = |x: f64, y: f64| (x - y).abs() <= 1e-11 * x.abs().max(y.abs()).max(1e-300) || (x - y).abs() <= 1e-300;
    let first = terms(a, 64);
    if first.iter().any(|t| !t.is_finite()) {
        return Err("Σ の項が実数の値になりません".into());
    }
    // 1. そのまま足す（Neumaier の補償つき和）
    {
        let (mut s, mut comp) = (0.0f64, 0.0f64);
        let mut small = 0;
        let mut k = a;
        while k - a < 131_072 {
            let ts = terms(k, 1024);
            for t in ts {
                if !t.is_finite() {
                    return Err("Σ の項が実数の値になりません".into());
                }
                let n = s + t;
                comp += if s.abs() >= t.abs() { (s - n) + t } else { (t - n) + s };
                s = n;
                small = if t.abs() <= 1e-17 * (s + comp).abs() { small + 1 } else { 0 };
                if small >= 8 {
                    return Ok(s + comp);
                }
            }
            k += 1024;
        }
    }
    // 2. 交代級数
    let alternating = first.windows(2).all(|w| w[0] * w[1] < 0.0) && first.windows(2).skip(8).all(|w| w[1].abs() <= w[0].abs() * (1.0 + 1e-12));
    // 発散する交代級数（1 − 1 + 1 − …）にも「自然な和」を返してしまうので、遠くの項が十分小さいことを先に確かめる
    let far = eval(&[(a + 10_000) as f64])[0];
    if alternating && far.is_finite() && far.abs() < 0.05 * first[0].abs() {
        let cvz = |n: usize| {
            let ak: Vec<f64> = first.iter().take(n).map(|t| t.abs()).collect();
            let mut d = (3.0 + 8f64.sqrt()).powi(n as i32);
            d = (d + 1.0 / d) / 2.0;
            let (mut b, mut c, mut s) = (-1.0f64, -d, 0.0f64);
            for (k, &v) in ak.iter().enumerate() {
                c = b - c;
                s += c * v;
                let (kf, nf) = (k as f64, n as f64);
                b *= (kf + nf) * (kf - nf) / ((kf + 0.5) * (kf + 1.0));
            }
            first[0].signum() * s / d
        };
        let (s30, s40) = (cvz(30), cvz(40));
        if close(s30, s40) {
            return Ok(s40);
        }
    }
    // 3. オイラー・マクローリン（f′ と f‴ の補正まで。f‴ は中心差分）
    let half = eval(&[a as f64 + 0.5]);
    if half[0].is_finite() {
        let mut rough = false;
        let mut em = |n: i64| -> f64 {
            let budget = crate::quad::Budget::new(300_000);
            let head: f64 = terms(a, (n - a) as usize).iter().sum();
            let mut g = |xs: &[f64], out: &mut [f64]| out.copy_from_slice(&eval(xs));
            let tail = crate::quad::integrate(&mut g, n as f64, f64::INFINITY, crate::quad::Tol { rel: 1e-12, panels: 1000, ends: false }, &budget);
            rough |= budget.rough.get();
            let (x, h) = (n as f64, 0.05);
            let v = eval(&[x, x + h, x - h, x + 2.0 * h, x - 2.0 * h]);
            let d1 = (v[1] - v[2]) / (2.0 * h);
            let d3 = (v[3] - 2.0 * v[1] + 2.0 * v[2] - v[4]) / (2.0 * h * h * h);
            head + tail + v[0] / 2.0 - d1 / 12.0 + d3 / 720.0
        };
        let (s1, s2) = (em(a + 1024), em(a + 4096));
        // 和公式の残りは N が大きいほど小さいので、差は大きい N の値の誤差の上限としては大きめに出る（sin(k)/k² で
        // 差 4e-11、大きい N の値の誤差 4e-12）。差が 1e-9 以内なら大きい N の値を返す
        if s1.is_finite() && s2.is_finite() && !rough && (s1 - s2).abs() <= 1e-9 * s2.abs().max(1e-300) {
            return Ok(s2);
        }
    }
    Err("Σ が収束しないようです（発散する・収束がとても遅い）".into())
}

/// 値が定数になる積分 1 つで、中身を計算してよい回数の上限（3 重積分でも 0.5 秒ほどで終わる数）
const CLOSED_BUDGET: u64 = 6_000_000;

/// 入れ子の積分 ∫_{a₀}^{b₀} ∫_{a₁(t₀)}^{b₁(t₀)} … f dt… の、i 段目から内側（`p` は外側の積分の変数の値）
use crate::solve::Quadratic;

/// 領域の条件の式（≥ 0）と、変数ごとの数で書かれた端（[`Expander::region_programs`]）
type RegionPrograms = (Vec<crate::eval::Program>, Vec<(Option<f64>, Option<f64>)>);

/// 入れ子の積分の端: 外側の積分の変数の式か、±∞
enum Bound {
    Prog(crate::eval::Program),
    Inf(f64),
}

fn nested_integral(bounds: &[(Bound, Bound)], f: &crate::eval::Program, i: usize, p: [f64; 3], budget: &crate::quad::Budget, wide: bool) -> f64 {
    use num_complex::Complex64 as C;
    let n = bounds.len();
    let at = |b: &Bound| {
        let prog = match b {
            Bound::Inf(v) => return *v,
            Bound::Prog(prog) => prog,
        };
        let cols: Vec<[f64; 1]> = (0..i).map(|k| [p[k]]).collect();
        let inputs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
        let mut out = [C::new(0.0, 0.0)];
        prog.eval_batch(&inputs, &mut out);
        crate::eval::as_real(out[0])
    };
    let (a, b) = (at(&bounds[i].0), at(&bounds[i].1));
    // 外側ほど目標を厳しくする（内側の積分の誤差は、外側の値の誤差として足される）
    let tol = crate::quad::Tol { rel: if n == 1 { 1e-10 } else if i == 0 { 1e-9 } else { 1e-11 }, panels: [[20_000, 0, 0], [400, 200, 0], [200, 100, 100]][n - 1][i], ends: true };
    let integrate = if wide { crate::quad::integrate_wide } else { crate::quad::integrate };
    if i + 1 == n {
        let mut g = |ts: &[f64], out: &mut [f64]| {
            let cols: Vec<Vec<f64>> = (0..n).map(|k| if k == i { ts.to_vec() } else { vec![p[k]; ts.len()] }).collect();
            let inputs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
            let mut vals = vec![C::new(0.0, 0.0); ts.len()];
            f.eval_batch(&inputs, &mut vals);
            for (o, v) in out.iter_mut().zip(&vals) {
                *o = crate::eval::as_real(*v);
            }
        };
        integrate(&mut g, a, b, tol, budget)
    } else {
        let mut g = |ts: &[f64], out: &mut [f64]| {
            for (o, &t) in out.iter_mut().zip(ts) {
                let mut q = p;
                q[i] = t;
                *o = nested_integral(bounds, f, i + 1, q, budget, wide);
            }
        };
        integrate(&mut g, a, b, tol, budget)
    }
}

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

/// 等式を解いた結果: (解いた変数, その式) の並びと、残った変数（[`Expander::eliminate`]）
/// 解いた変数と式、残った変数（媒介変数）、1 次式に解けずに残った等式（左辺 − 右辺。残った変数の式）
type Solved = (Vec<(String, Expr)>, Vec<String>, Vec<Expr>);

/// 範囲の上の積分で、媒介変数の領域の上の中身に掛けるもの（[`Expander::measure_of`]）
#[derive(Debug, Clone, Copy, PartialEq)]
enum Measure {
    /// 範囲の次元の大きさ（弧長・面積・体積。`vec` なら F · r′・F · (r_u × r_v)）
    Natural,
    /// 何も掛けない（媒介変数そのものの d）
    Plain,
    /// 曲線の上の dx: 点の i 番目の成分の微分
    Coord(usize),
    /// 曲面の上の dx dy: 法線 r_u × r_v の i 番目の成分（false なら向きが逆）
    Form(usize, bool),
}

/// 媒介変数 k 個・点の成分 dim 個の範囲の呼び名
fn shape_name(k: usize, dim: usize) -> &'static str {
    match (k, dim) {
        (0, _) => "点",
        (1, 1) => "区間",
        (1, _) => "曲線（媒介変数 1 つ）",
        (2, 2) => "平面の領域",
        (2, _) => "曲面（媒介変数 2 つ）",
        _ => "立体（媒介変数 3 つ）",
    }
}

/// 範囲の条件を、等式（左, 右）とそれ以外に分ける。`x = y = z` は x = y と y = z、`0 ≤ x = y` は 0 ≤ x と x = y
fn split_equalities(chains: &[Expr]) -> (Vec<(Expr, Expr)>, Vec<Expr>) {
    let mut eqs = Vec::new();
    let mut rest = Vec::new();
    for c in chains {
        match c {
            Expr::Op { op, args } if op.starts_with("rel:") && op.contains('=') => {
                for (j, r) in op.trim_start_matches("rel:").chars().enumerate() {
                    let (l, rr) = (args[j].clone(), args[j + 1].clone());
                    if r == '=' {
                        eqs.push((l, rr));
                    } else {
                        rest.push(Expr::Op { op: format!("rel:{r}"), args: vec![l, rr] });
                    }
                }
            }
            _ => rest.push(c.clone()),
        }
    }
    (eqs, rest)
}

/// 範囲の条件の変数に式を入れる
fn subst_chain(c: &Expr, s: &dyn Fn(&str) -> Option<Expr>) -> Expr {
    match c {
        Expr::Op { op, args } => Expr::Op { op: op.clone(), args: args.iter().map(|e| e.substitute(s)).collect() },
        Expr::Cond(f) => Expr::Cond(Box::new(crate::logic::substitute(f, s))),
        e => e.substitute(s),
    }
}

/// 3 × 3 の行列式（列 cols[j][i]）
fn det3(cols: &[Vec<Expr>]) -> Expr {
    use crate::diff::{add, mul, sub};
    let a = |i: usize, j: usize| cols[j][i].clone();
    let minor = |i: usize, j: usize, k: usize, l: usize| sub(mul(a(i, k), a(j, l)), mul(a(i, l), a(j, k)));
    add(sub(mul(a(0, 0), minor(1, 2, 1, 2)), mul(a(0, 1), minor(1, 2, 0, 2))), mul(a(0, 2), minor(1, 2, 0, 1)))
}

/// 並び替えの全部（3 つまで）
fn permutations(v: &[String]) -> Vec<Vec<String>> {
    if v.len() <= 1 {
        return vec![v.to_vec()];
    }
    let mut out = Vec::new();
    for i in 0..v.len() {
        let mut rest = v.to_vec();
        let first = rest.remove(i);
        for mut p in permutations(&rest) {
            p.insert(0, first.clone());
            out.push(p);
        }
    }
    out
}

/// 数字の添字の付いた名前 `v1` を、(v, 1) に分ける（頭は 1 文字）
fn split_subscript(name: &str) -> Option<(&str, &str)> {
    let first = name.chars().next()?;
    let (head, digits) = name.split_at(first.len_utf8());
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) && first.is_alphabetic()).then_some((head, digits))
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

/// 論理式の中の ⋃・⋂（[`crate::parser::Formula::Big`]）を、添字ごとの「または」「かつ」に展開する（計画 M12 の一部）
pub fn expand_big(f: &crate::parser::Formula, params: &Params) -> Result<crate::parser::Formula, String> {
    use crate::parser::Formula as F;
    let defs = current();
    let mut x = Expander::new(&defs, params);
    fn go(f: &F, x: &mut Expander) -> Result<F, String> {
        let b = |g: &F, x: &mut Expander| go(g, x).map(Box::new);
        Ok(match f {
            F::Big { or, vars, cond, body } => {
                let names = Expander::index_vars(vars)?;
                let values = x.enumerate(&names, cond)?;
                if values.len() > crate::logic::MAX_CLAUSES * 4 {
                    return Err(format!("⋃・⋂ の項が多すぎます（{} 個）", values.len()));
                }
                let mut parts = Vec::new();
                for vals in values {
                    let g = crate::logic::substitute(body, &|v: &str| names.iter().position(|n| n == v).map(|k| Expr::Num(vals[k])));
                    parts.push(go(&g, x)?);
                }
                if parts.is_empty() {
                    if *or { F::False } else { F::True }
                } else if *or {
                    F::Or(parts)
                } else {
                    F::And(parts)
                }
            }
            F::Not(a) => F::Not(b(a, x)?),
            F::And(v) => F::And(v.iter().map(|g| go(g, x)).collect::<Result<_, _>>()?),
            F::Or(v) => F::Or(v.iter().map(|g| go(g, x)).collect::<Result<_, _>>()?),
            F::Xor(p, q) => F::Xor(b(p, x)?, b(q, x)?),
            F::Iff(p, q) => F::Iff(b(p, x)?, b(q, x)?),
            F::Implies(p, q) => F::Implies(b(p, x)?, b(q, x)?),
            F::Nand(p, q) => F::Nand(b(p, x)?, b(q, x)?),
            F::Nor(p, q) => F::Nor(b(p, x)?, b(q, x)?),
            _ => f.clone(),
        })
    }
    go(f, &mut x)
}

/// 等式の片側が `f^(-1)(式)`（f は引数 1 つの関数）だけなら、`f(反対側) = 式` に書き換える（逆関数）
pub fn rewrite_inverse(l: Expr, r: Expr) -> (Expr, Expr) {
    let defs = current();
    let inv = |e: &Expr| -> Option<(String, Expr)> {
        let Expr::Bin(BinOp::Mul, a, arg) = e else { return None };
        let Expr::Bin(BinOp::Pow, f, k) = &**a else { return None };
        let Expr::Var(name) = &**f else { return None };
        let one_arg = match defs.get(name) {
            Some(Def::Function { args, .. }) => args.len() == 1,
            Some(Def::Value(_)) => {
                let params = Params::new();
                let mut x = Expander::new(&defs, &params);
                matches!(x.def_value(name), Ok(Some(Val::Fn { arity: 1, .. })))
            }
            Some(Def::Set(_)) | None => false,
        };
        (k.is_minus_one() && one_arg).then(|| (name.clone(), (**arg).clone()))
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
    let mut x = Expander::new(&defs, &params);
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

/// 係数を、小さい分母の分数で書けるなら分数で（1/6・−3/2）。書けなければ有効 6 桁
fn coefficient_text(v: f64) -> (bool, String) {
    let neg = v < 0.0;
    let a = v.abs();
    // 連分数で、値が浮動小数点の誤差の範囲で一致する分数を探す（e・π のような数は分数にしない）
    let (mut h0, mut h1, mut k0, mut k1) = (0.0f64, 1.0f64, 1.0f64, 0.0f64);
    let mut x = a;
    let mut frac = None;
    for _ in 0..30 {
        let q = x.floor();
        let (h2, k2) = (q * h1 + h0, q * k1 + k0);
        // 分母は 10⁷ まで。分子が 1000 以下なら分母は 10¹⁸ まで（1/20! のような階乗の逆数も分数で出す）。
        // 分子も分母も大きい分数（4551459/808579796）は、無理数をたまたま近似しただけなので使わない
        if k2 > 1e18 || h2 > 1e7 || (k2 > 1e7 && h2 > 1000.0) {
            break;
        }
        (h0, h1, k0, k1) = (h1, h2, k1, k2);
        if (h1 / k1 - a).abs() <= 4e-16 * a.max(1e-300) {
            frac = Some((h1, k1));
            break;
        }
        let r = x - q;
        if r < 1e-15 {
            break;
        }
        x = 1.0 / r;
    }
    let text = match frac {
        Some((h, k)) if k == 1.0 => format!("{h}"),
        Some((h, k)) => format!("{h}/{k}"),
        // 小さすぎる・大きすぎる数は、有効 6 桁の指数表記（2.50521×10⁻⁸）
        None if a != 0.0 && (a < 1e-3 || a >= 1e7) => {
            let e = a.log10().floor() as i32;
            let m = format!("{:.5}", a / 10f64.powi(e));
            let m = m.trim_end_matches('0').trim_end_matches('.');
            let sup: String = e.to_string().chars().map(|c| if c == '-' { '⁻' } else { "⁰¹²³⁴⁵⁶⁷⁸⁹".chars().nth(c.to_digit(10).unwrap() as usize).unwrap() }).collect();
            format!("{m}×10{sup}")
        }
        None => {
            let s = format!("{a:.6}");
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
    };
    (neg, text)
}

/// 上付きの数字（² ³ …）
fn superscript(n: usize) -> String {
    if n == 1 {
        return String::new();
    }
    n.to_string().chars().map(|c| "⁰¹²³⁴⁵⁶⁷⁸⁹".chars().nth(c.to_digit(10).unwrap() as usize).unwrap()).collect()
}

/// 説明に出すテイラー多項式（`T₃(x) = x − x³/6`、`T₂(x, y) = 1 + (x − 1) + …`）
fn taylor_text(n: usize, names: &[String], centers: &[f64], terms: &[(Vec<usize>, num_complex::Complex64)]) -> String {
    let sub: String = n.to_string().chars().map(|c| "₀₁₂₃₄₅₆₇₈₉".chars().nth(c.to_digit(10).unwrap() as usize).unwrap()).collect();
    let shift = |i: usize, wrap: bool| -> String {
        let c = centers[i];
        if c == 0.0 {
            names[i].clone()
        } else {
            let (neg, t) = coefficient_text(c);
            let s = format!("{} {} {t}", names[i], if neg { "+" } else { "−" });
            if wrap { format!("({s})") } else { s }
        }
    };
    let mut out = String::new();
    for (alpha, c) in terms {
        let v = c.re;
        if v.abs() < 1e-14 {
            continue;
        }
        let factors: Vec<usize> = (0..alpha.len()).filter(|&i| alpha[i] > 0).collect();
        let mono: String = factors
            .iter()
            .map(|&i| {
                format!("{}{}", shift(i, true), superscript(alpha[i]))
            })
            .collect();
        let (neg, t) = coefficient_text(v);
        let body = if mono.is_empty() {
            t
        } else if t == "1" {
            mono
        } else if let Some((p, q)) = t.split_once('/') {
            if p == "1" { format!("{mono}/{q}") } else { format!("{p}{mono}/{q}") }
        } else {
            format!("{t}{mono}")
        };
        if out.is_empty() {
            out = if neg { format!("−{body}") } else { body };
        } else {
            out.push_str(if neg { " − " } else { " + " });
            out.push_str(&body);
        }
    }
    if out.is_empty() {
        out = "0".into();
    }
    format!("T{sub}({}) = {out}", names.join(", "))
}

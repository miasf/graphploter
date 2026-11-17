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
//!   `@intset(…; {vec})(F)`（F · dr・F · dS）
//! - 領域の上の重積分 `@intset((x, y), 0 ≤ x ≤ 1, x^2 ≤ y ≤ x)(式)`（条件から累次積分の順番と範囲を見つける。
//!   範囲の形にならない条件は、条件の外を 0 にした近似。変数ごとの範囲が無くても、1 次式の範囲 `−1 ≤ x + y ≤ 1, 0 ≤ x − y ≤ 1` なら
//!   変数変換（ヤコビアン 1/|det|）で累次積分にする）
//!
//! 定義は、ほかの行（式の一覧）から [`Defs::parse`] で作り、[`install`] で入れておく（`lib.rs` が `plot` の間だけ入れる）。
//! 計算エンジンの式の文法は `parser.rs` の先頭の説明。展開した後の式には、新しい節を残さない（残れば内部エラー）

use crate::eval::Params;
use crate::parser::{definition_head, parse_system_with, BinOp, Expr, Rel};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub mod linalg;
use linalg::{cross, norm, Mat};

/// 展開した式の節の数の上限（計画 2.5）。球座標で書いた球の体積積分（3 重・体積要素 |det J|）が約 132 万節（展開とコンパイルで 0.4 秒ほど）。
/// これより大きい式は、メモリ（WASM で数十 MB）と時間が重くなるので断る
pub const MAX_NODES: usize = 1_500_000;
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
    /// 集合の定義 `S = {(x, y) | …}`・`S = A ∪ B`（計画 M11）
    Set(crate::parser::SetE),
}

/// 数列の定義 `a_(n) = 式`（計画 M10）。`body` は添字の変数 `var` の式（左辺の添字のずれは、もう直してある）。
/// 右辺に同じ数列の項があれば漸化式
#[derive(Debug, Clone)]
pub struct Seq {
    pub var: String,
    pub body: Expr,
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
            let Some((name, var, shift)) = sequence_head(&chain.exprs[0]) else { continue };
            if seqs.contains_key(&name) {
                continue;
            }
            // a_(n + k) = f(n) → a_(m) = f(m − k)
            let body = chain.exprs[1].substitute(&|v| {
                (v == var && shift != 0).then(|| Expr::Bin(BinOp::Sub, Box::new(Expr::Var(var.clone())), Box::new(Expr::Num(shift as f64))))
            });
            seqs.insert(name, Seq { var, body });
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

/// 数列の定義の左辺 `a_(n)`・`a_(n + k)`・`a_(n − k)` なら (名前, 添字の変数, k)
pub fn sequence_head(e: &Expr) -> Option<(String, String, i64)> {
    let Expr::Index { name, index } = e else { return None };
    let int = |e: &Expr| match e {
        Expr::Num(v) if v.fract() == 0.0 && v.abs() < 1e4 => Some(*v as i64),
        _ => None,
    };
    let (var, k) = match &**index {
        Expr::Var(v) => (v.clone(), 0),
        Expr::Bin(BinOp::Add, a, b) => match (&**a, int(b)) {
            (Expr::Var(v), Some(k)) => (v.clone(), k),
            _ => return None,
        },
        Expr::Bin(BinOp::Sub, a, b) => match (&**a, int(b)) {
            (Expr::Var(v), Some(k)) => (v.clone(), -k),
            _ => return None,
        },
        _ => return None,
    };
    (crate::equation::coord_index(&var).is_none()).then(|| (name.clone(), var, k))
}

/// 数列の項 a_from, …, a_to の値（複素数）。項が数にならない（座標を含む）なら None。初項が足りないなどはエラー
pub fn sequence_terms(name: &str, from: i64, to: i64, params: &Params) -> Result<Option<Vec<(i64, num_complex::Complex64)>>, String> {
    let defs = current();
    let mut x = Expander { defs: &defs, params, int_depth: 0, stack: Vec::new(), memo: Default::default() };
    let mut out = Vec::new();
    for k in from..=to {
        let e = x.term(name, &Expr::Num(k as f64))?;
        let Ok(p) = crate::eval::Program::compile_with(&e, &[], params) else { return Ok(None) };
        let mut v = [num_complex::Complex64::new(0.0, 0.0)];
        p.eval_batch(&[], &mut v);
        out.push((k, v[0]));
    }
    Ok(Some(out))
}

/// 数列の初項の添字（パラメータ `a{k}` のうち、いちばん小さい k。無ければ None）
pub fn first_term(name: &str, params: &Params) -> Option<i64> {
    (0..=10_000).find(|k| params.contains_key(&format!("{name}{k}")))
}

/// 漸化式か（右辺に同じ数列の項がある）
pub fn is_recurrence(name: &str) -> bool {
    current().seq(name).is_some_and(|s| {
        let mut vs = Vec::new();
        s.body.rewrite(&|e| matches!(e, Expr::Index { name: n, .. } if n == name).then(|| Expr::Var("#self".into()))).collect_vars(&mut vs);
        vs.iter().any(|v| v == "#self")
    })
}

/// 横の座標の表示範囲（数列の点の添字の範囲に使う。`lib.rs` が入れる）
pub fn set_view_x(lo: f64, hi: f64) {
    VIEW_X.with(|x| x.set((lo, hi)));
}

pub fn view_x() -> (f64, f64) {
    VIEW_X.with(|x| x.get())
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
    let mut x = Expander { defs: &defs, params, int_depth: 0, stack: Vec::new(), memo: Default::default() };
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

struct Expander<'a> {
    defs: &'a Defs,
    params: &'a Params,
    /// 積分の入れ子の深さ（2 重まで）
    int_depth: usize,
    /// 展開中の定義の名前（自分自身を使う定義を見つける）
    stack: Vec<String>,
    /// 漸化式の項（名前, 添字 → 値の式）。同じ展開の中で使い回す
    memo: std::collections::HashMap<(String, i64), Expr>,
}

/// 式を展開して、今までの節だけの式にする。新しい節を使っていない式は、そのまま返す
pub fn expand(e: &Expr, params: &Params) -> Result<Expr, String> {
    if !e.has_notation() && !mentions_defs(e) && !nested_tuple(e, true) && !e.uses_var("∗") {
        return Ok(e.clone());
    }
    let defs = current();
    let mut x = Expander { defs: &defs, params, int_depth: 0, stack: Vec::new(), memo: Default::default() };
    let out = match x.val(e)? {
        Val::S(s) => s,
        Val::Fn { arity, body } => field_at_xyz(arity, &body).ok_or_else(|| fn_as_value_error(e))?,
    };
    if out.node_count() > MAX_NODES {
        return Err("式が大きくなりすぎます（関数の入れ子・Σ の項の数を減らしてください）".into());
    }
    Ok(out)
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
            Expr::Op { op, args } if op == "cases" => Val::S(self.cases(args)?),
            Expr::Op { op, args } if op == "sumset" || op == "prodset" => Val::S(self.set_sum(op == "sumset", args)?),
            Expr::Cond(_) => return Err("条件は、Σ・Π の下でだけ使えます".into()),
            Expr::Op { op, args } if op == "intset" => Val::S(self.region_integral(args)?),
            Expr::Op { op, .. } if op == "ellipsis" => return Err("… はリスト [1, 2, …, 10] の中で使ってください".into()),
            Expr::Binder { op, var, args, body } if op == "sum" || op == "prod" => Val::S(self.sum_or_prod(op == "sum", var, args, body)?),
            Expr::Binder { op, var, args, body } if op == "int" => Val::S(self.integral(var, args, body)?),
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
            _ => return Err("@dim は、dim ker A・dim Im A の形で使ってください".into()),
        };
        let m = self.mat(inner, &format!("@{op}"))?;
        let Some(a) = numbers(&m.items, self.params) else {
            return Err("階数・次元を求める行列の成分は、数とパラメータだけで書いてください".into());
        };
        let rank = crate::linalg::Matrix::new(m.rows, m.cols, a).rank();
        Ok(if kind == "ker" { m.cols - rank } else { rank })
    }

    /// 数列の項 `a_(式)`（計画 M10）。添字は整数（数とパラメータの式）。
    /// パラメータ `a{k}`（初項 `a_1 = 1` の行）があればその値。一般項なら添字を代入した式。漸化式なら、初項から順に作る
    /// （項が数とパラメータだけなら数にしてから次へ進む。フィボナッチの a₈₀ も一瞬）
    fn term(&mut self, name: &str, index: &Expr) -> Result<Expr, String> {
        const MAX_SPAN: i64 = 10_000;
        let k = self.integer(index, &format!("{name} の添字"))?;
        let Some(seq) = self.defs.seq(name).cloned() else { return Err(format!("{name} の数列は定義されていません")) };
        let param = |k: i64| format!("{name}{k}");
        let initial = |x: &Self, k: i64| -> Option<Expr> {
            (k >= 0 && x.params.contains_key(&param(k))).then(|| {
                note_param(&param(k));
                Expr::Var(param(k))
            })
        };
        if let Some(v) = initial(self, k) {
            return Ok(v);
        }
        if let Some(v) = self.memo.get(&(name.to_string(), k)) {
            return Ok(v.clone());
        }
        let recursive = {
            let mut vs = Vec::new();
            seq.body.rewrite(&|e| matches!(e, Expr::Index { name: n, .. } if n == name).then(|| Expr::Var("#self".into()))).collect_vars(&mut vs);
            vs.iter().any(|v| v == "#self")
        };
        let at = |x: &mut Self, k: i64| -> Result<Expr, String> {
            let body = seq.body.substitute(&|v| (v == seq.var).then_some(Expr::Num(k as f64)));
            let e = x.scalar(&body)?;
            // 数とパラメータだけなら数にする（式が育たないように）
            Ok(match numbers(std::slice::from_ref(&e), x.params) {
                Some(v) => Expr::Num(v[0]),
                None => e,
            })
        };
        if !recursive {
            if self.stack.len() >= MAX_DEPTH {
                return Err("数列の入れ子が深すぎます".into());
            }
            note_used(name);
            return at(self, k);
        }
        // 漸化式: 添字 k 以下で、いちばん近い初項（パラメータ）から順に作る
        let Some(lo) = (0..=k.max(0)).rev().take(MAX_SPAN as usize + 1).find(|&j| self.params.contains_key(&param(j))) else {
            let first = if k >= 1 { 1 } else { 0 };
            return Err(format!("変数 '{}' はここでは使えません（初項 {name}_{first} を定義してください）", param(first)));
        };
        // 初項より前の項が要る漸化式（2 つ前を使うなど）のために、続けて並んだ初項を全部入れる
        let mut start = lo;
        while start > 0 && self.params.contains_key(&param(start - 1)) {
            start -= 1;
        }
        if k - start > MAX_SPAN {
            return Err(format!("{name} の添字が初項から離れすぎています（{MAX_SPAN} 項まで）"));
        }
        note_used(name);
        for j in start..=k {
            if self.memo.contains_key(&(name.to_string(), j)) {
                continue;
            }
            let v = match initial(self, j) {
                Some(v) => v,
                None => {
                    // 項 j の式の中の同じ数列の項は、もう作った項（無ければ、初項が足りない）
                    let missing = std::cell::RefCell::new(None);
                    let memo = &self.memo;
                    let body = seq.body.substitute(&|v| (v == seq.var).then_some(Expr::Num(j as f64)));
                    let body = body.rewrite(&|e| match e {
                        Expr::Index { name: n, index } if n == name => {
                            let idx = crate::eval::Program::compile_with(index, &[], self.params).ok().map(|p| {
                                let mut out = [num_complex::Complex64::new(0.0, 0.0)];
                                p.eval_batch(&[], &mut out);
                                crate::eval::as_real(out[0])
                            });
                            match idx {
                                Some(i) if i.fract() == 0.0 => match memo.get(&(name.to_string(), i as i64)) {
                                    Some(v) => Some(v.clone()),
                                    None if self.params.contains_key(&param(i as i64)) => Some(Expr::Var(param(i as i64))),
                                    None => {
                                        missing.borrow_mut().get_or_insert(i as i64);
                                        Some(Expr::Num(0.0))
                                    }
                                },
                                _ => None,
                            }
                        }
                        _ => None,
                    });
                    if let Some(i) = missing.into_inner() {
                        return Err(format!("変数 '{}' はここでは使えません（初項 {name}_{i} を定義してください）", param(i.max(0))));
                    }
                    for p in (start..j).map(param) {
                        if body.uses_var(&p) {
                            note_param(&p);
                        }
                    }
                    let e = self.scalar(&body)?;
                    let e = match numbers(std::slice::from_ref(&e), self.params) {
                        Some(v) => Expr::Num(v[0]),
                        None => e,
                    };
                    if e.node_count() > MAX_NODES {
                        return Err(format!("{name} の項の式が大きくなりすぎます（添字を小さくしてください）"));
                    }
                    e
                }
            };
            self.memo.insert((name.to_string(), j), v);
        }
        Ok(self.memo[&(name.to_string(), k)].clone())
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

    /// 定積分 $\int_a^b g(t)\,dt$ を、区間を 2 つに分けたガウス・ルジャンドルの重み付きの和にする（2 重目は 8 点 × 2）。
    /// 端 a, b は式のまま（x を使ってよい）。∞ の端は変数変換で有限の区間にする:
    /// $[a, \infty)$ は $t = a + \frac{s}{1-s}$、$(-\infty, b]$ は $t = b - \frac{s}{1-s}$（$s \in [0, 1)$）、
    /// $(-\infty, \infty)$ は $(-\infty, 0]$ と $[0, \infty)$ に分ける
    fn integral(&mut self, var: &str, args: &[Expr], body: &Expr) -> Result<Expr, String> {
        if args.is_empty() {
            return self.indefinite(var, body);
        }
        let [lo, hi] = args else { return Err("積分の範囲を書いてください（∫_0^1 の形）".into()) };
        if self.int_depth >= 3 {
            return Err("積分の入れ子は 3 重までにしてください".into());
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

    /// 添字つきの最大・最小 $\max_{t \in [a, b]} g(t)$（計画 M12）: 範囲を 64 等分した 65 点での値の max・min（近似）
    fn extremum(&mut self, max: bool, var: &str, args: &[Expr], body: &Expr) -> Result<Expr, String> {
        const N: usize = 64;
        let [lo, hi] = args else { return Err("max・min は、max_{t ∈ [0, 1]} の形で範囲を書いてください".into()) };
        let (lo, hi) = (self.scalar(lo)?, self.scalar(hi)?);
        let g = self.scalar(body)?;
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

    /// 点を媒介変数の式で書いた集合の上の積分（線積分・面積分・体積分）。被積分関数の x, y, z に点の成分を入れ、
    /// 弧長 $|\mathbf r'|$・面積要素 $|\mathbf r_u \times \mathbf r_v|$・体積要素 $|\det J|$（`vec` なら $\mathbf F \cdot \mathbf r'$・
    /// $\mathbf F \cdot (\mathbf r_u \times \mathbf r_v)$）を掛けて、媒介変数の領域の上の積分にする。
    /// 媒介変数は、条件で範囲を書いた名前のうち点の式に出てくるもの（出てくる順。面の向き $\mathbf r_u \times \mathbf r_v$ もこの順）
    fn parametric_integral(&mut self, point: &Expr, chains: &[Expr], vec_mode: bool, body: &Expr) -> Result<Expr, String> {
        use crate::diff::{add, diff, mul, sub};
        const XYZ: [&str; 3] = ["x", "y", "z"];
        let comps: Vec<Expr> = match self.scalar(point)? {
            Expr::Tuple(items) => items,
            e => vec![e],
        };
        let dim = comps.len();
        if dim > 3 {
            return Err("曲線・曲面の点は 3 成分までにしてください".into());
        }
        // 条件で範囲を書いた名前
        let mut bounded: Vec<String> = Vec::new();
        for chain in chains {
            if let Expr::Op { args: exprs, .. } = chain {
                for e in exprs {
                    if let Expr::Var(v) = e {
                        bounded.push(v.clone());
                    }
                }
            }
        }
        let mut params: Vec<String> = Vec::new();
        for c in &comps {
            let mut vs = Vec::new();
            c.collect_vars(&mut vs);
            for v in vs {
                if bounded.contains(&v) && !params.contains(&v) {
                    params.push(v);
                }
            }
        }
        let k = params.len();
        if k == 0 {
            return Err("曲線・曲面の媒介変数の範囲を、0 ≤ t ≤ 2π のように書いてください".into());
        }
        let jac: Vec<Vec<Expr>> = params.iter().map(|p| comps.iter().map(|c| diff(c, p)).collect::<Result<_, _>>()).collect::<Result<_, _>>()?;
        // 被積分関数（関数の名前なら座標に当てはめる）の x, y, z に点の成分を入れる
        let g = match self.val(body)? {
            Val::S(s) => s,
            Val::Fn { arity, body } if arity <= 3 => {
                apply_body(&body, &XYZ[..arity].iter().map(|c| Expr::Var((*c).into())).collect::<Vec<_>>())
            }
            Val::Fn { .. } => return Err(fn_as_value_error(body)),
        };
        let g = g.substitute(&|v: &str| XYZ[..dim].iter().position(|c| *c == v).map(|i| comps[i].clone()));
        let dot = |u: &[Expr], v: &[Expr]| u.iter().zip(v).fold(Expr::Num(0.0), |s, (a, b)| add(s, mul(a.clone(), b.clone())));
        let integrand = if vec_mode {
            let Expr::Tuple(f) = &g else { return Err("F · dr・F · dS の F はベクトル場 (P, Q, R) にしてください".into()) };
            if f.len() != dim {
                return Err(format!("ベクトル場の成分の数（{}）を、点の成分の数（{dim}）にそろえてください", f.len()));
            }
            match k {
                1 => dot(f, &jac[0]),
                2 if dim == 3 => match cross(&jac[0], &jac[1])? {
                    Expr::Tuple(n) => dot(f, &n),
                    _ => unreachable!(),
                },
                _ => return Err("F · dr は曲線（媒介変数 1 つ）、F · dS は 3 次元の曲面（媒介変数 2 つ）で使えます".into()),
            }
        } else {
            if matches!(g, Expr::Tuple(_)) {
                return Err("ベクトル場を積分するなら F · dr・F · dS の形で書いてください".into());
            }
            let measure = match (k, dim) {
                (1, _) => norm(&jac[0]),
                (2, 3) => match cross(&jac[0], &jac[1])? {
                    Expr::Tuple(n) => norm(&n),
                    _ => unreachable!(),
                },
                (2, 2) => Expr::Call(crate::functions::Func::Abs, vec![sub(mul(jac[0][0].clone(), jac[1][1].clone()), mul(jac[0][1].clone(), jac[1][0].clone()))]),
                (3, 3) => Expr::Call(crate::functions::Func::Abs, vec![det3(&jac)]),
                _ => return Err(format!("媒介変数 {k} つで {dim} 成分の点の積分はできません（曲線は 1 つ、曲面は 2 つ、立体は 3 つ）")),
            };
            mul(g, measure)
        };
        let mut args = vec![Expr::Tuple(params.iter().map(|p| Expr::Var(p.clone())).collect())];
        args.extend(chains.iter().cloned());
        args.push(integrand);
        self.region_integral(&args)
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

    fn region_integral(&mut self, args: &[Expr]) -> Result<Expr, String> {
        use crate::functions::Func;
        let (Some(vars_e), Some(body)) = (args.first(), args.last()) else { return Err("内部エラー: 領域の積分の形".into()) };
        // 点が変数の組でなければ（媒介変数の式）、線積分・面積分・体積分。`; vec` の印も、ここで取り出す
        let vec_mode = args.iter().any(|a| matches!(a, Expr::Op { op, .. } if op == "mode:vec"));
        if let Some(bad) = args.iter().find_map(|a| match a {
            Expr::Op { op, .. } if op.starts_with("mode:") && op != "mode:vec" => Some(op.trim_start_matches("mode:").to_string()),
            _ => None,
        }) {
            return Err(format!("; の後の {bad} は使えません（{{vec}} と書いてください）"));
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
        if vec_mode || !plain_vars(vars_e) {
            return self.parametric_integral(vars_e, &chains, vec_mode, body);
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
        let mut pairs: Vec<(Expr, bool, Expr)> = Vec::new();
        for chain in &args[1..args.len() - 1] {
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
    let mut x = Expander { defs: &defs, params, int_depth: 0, stack: Vec::new(), memo: Default::default() };
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
        let minus_one = matches!(&**k, Expr::Neg(x) if matches!(**x, Expr::Num(n) if n == 1.0)) || matches!(**k, Expr::Num(n) if n == -1.0);
        let one_arg = match defs.get(name) {
            Some(Def::Function { args, .. }) => args.len() == 1,
            Some(Def::Value(_)) => {
                let params = Params::new();
                let mut x = Expander { defs: &defs, params: &params, int_depth: 0, stack: Vec::new(), memo: Default::default() };
                matches!(x.def_value(name), Ok(Some(Val::Fn { arity: 1, .. })))
            }
            Some(Def::Set(_)) | None => false,
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
    let mut x = Expander { defs: &defs, params: &params, int_depth: 0, stack: Vec::new(), memo: Default::default() };
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

//! 厳密値の道（計画 `docs/plan-exact-core.md`・`docs/plan-exact-areas.md`）。
//!
//! 今までの近似値の道（展開の段 → 命令列 → f64）とは別に、式の値を厳密に求める。
//! 結果は必ず別の道の値と照らし合わせてから出す（計画の 2.4）。画面では別の Worker で動く（EX5）。
//!
//! - [`num`]: 大きな整数・分数（dashu）と、整数の計算（EX1）
//! - [`term`]: 式の形と自動の正規化、[`convert`]: 展開の前の構文木からの変換（EX2）
//! - [`ball`]: 誤差の幅つきの高い精度の値・0 の判定、[`consts`]: 定数と特殊関数の値（EX3）

pub mod algebraic;
pub mod ball;
pub mod check;
pub mod consts;
pub mod convert;
pub mod diff;
pub mod infinite_sum;
pub mod latex;
pub mod limit;
pub mod matrix;
pub mod num;
pub mod poly;
pub mod series;
pub mod term;
pub mod values;

use crate::eval::Params;
use std::cell::Cell;
use term::Term;

/// 厳密値の計算を途中でやめる理由
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// 見つからない（扱えない記法・式が大きすぎる など。理由つき）
    NotFound(String),
    /// 存在しないと示せた（理由つき）
    NotExist(String),
    /// 時間切れ
    TimeUp,
}

thread_local! {
    /// 式の正規化で数える仕事の量（使った量、上限）。上限を超えたら [`exhausted`] が真になり、呼び出し側が時間切れにする
    static WORK: Cell<(u64, u64)> = const { Cell::new((0, u64::MAX)) };
}

/// 正規化の仕事を `n` だけ数える（式の関数は Result を返さないので、超えたら印だけ付ける）
pub(crate) fn spend(n: u64) {
    WORK.with(|w| {
        let (u, l) = w.get();
        w.set((u.saturating_add(n), l));
    });
}

/// 仕事の量が上限を超えたか
pub(crate) fn exhausted() -> bool {
    WORK.with(|w| {
        let (u, l) = w.get();
        u > l
    })
}

/// 上限 `limit` で仕事を数えながら `f` を動かす（入れ子にしない）
pub fn with_work_limit<T>(limit: u64, f: impl FnOnce() -> T) -> (T, u64) {
    let before = WORK.with(|w| w.replace((0, limit)));
    let r = f();
    let used = WORK.with(|w| w.replace(before)).0;
    (r, used)
}

/// 既定の仕事の量の上限（計画の D7）。EX5 で測った: ネイティブで 1 秒に約 500 万（数の展開）〜 90 万（大きな分数の和）。
/// WASM はその 1/2〜1/3 なので、500 万でふつうの PC の WASM で約 2 秒
pub const DEFAULT_WORK: u64 = 5_000_000;

/// 変換の結果と、その途中で分かったこと
#[derive(Debug, Clone)]
pub struct Converted {
    pub term: Term,
    pub decimal: bool,
    pub analytic: bool,
    pub uses_ln: bool,
}

/// 式の文字列を、厳密値の道の式にする（照らし合わせ・書き出しの前の段階）。定義は今入っているもの（`expand::install`）
pub fn convert_source(source: &str, params: &Params) -> (Result<Converted, Stop>, u64) {
    convert_source_with(source, params, DEFAULT_WORK)
}

/// [`convert_source`] の仕事の量の上限を決めた版
pub fn convert_source_with(source: &str, params: &Params, work: u64) -> (Result<Converted, Stop>, u64) {
    let defs = crate::expand::current_defs();
    let names = defs.names();
    let e = match crate::parser::parse_with(source, &names) {
        Ok(e) => e,
        Err(e) => return (Err(Stop::NotFound(format!("式を読めません: {e}"))), 0),
    };
    with_work_limit(work, || {
        let mut c = convert::Converter::new(&defs, params);
        let t = c.term(&e)?;
        if exhausted() {
            return Err(Stop::TimeUp);
        }
        Ok(Converted { term: t, decimal: c.decimal, analytic: c.analytic, uses_ln: c.uses_ln })
    })
}

/// [`convert_source`] の式と、入力に小数があったか
pub fn evaluate_term(source: &str, params: &Params) -> Result<(Term, bool), Stop> {
    convert_source(source, params).0.map(|c| (c.term, c.decimal))
}

/// 厳密値の道の結果（計画の 2.3）
#[derive(Debug, Clone)]
pub enum Outcome {
    /// 見つかった（照らし合わせに通った）
    Found(Term),
    /// 推定（EX11）
    Guess(Term),
    /// 見つからない（理由つき）
    NotFound(String),
    /// 照らし合わせで食い違ったので捨てた（理由つき。画面では「見つからない」と同じ）
    Rejected(String),
    /// 存在しないと示せた（理由つき）
    NotExist(String),
}

/// 厳密値の道の答え
#[derive(Debug, Clone)]
pub struct Report {
    pub outcome: Outcome,
    /// 答えの LaTeX（見つかったとき・推定のとき）
    pub latex: String,
    /// 答えの、行として読める LaTeX（定数は `\\operatorname{eulergamma}` など。「行に追加」に使う）
    pub input_latex: String,
    /// 答えのエンジンの文字列
    pub engine: String,
    /// 答えの値を f64 に丸めたもの（実部, 虚部）
    pub approx: Option<(f64, f64)>,
    /// 答えの値の 10 進 50 桁（D9）
    pub digits: Option<String>,
    /// 入力に小数があった（答えも小数で出す）
    pub decimal: bool,
    /// 入力に自然対数 `ln` があった
    pub uses_ln: bool,
    /// 使った規則の名前（将来の途中式のため。D4）
    pub rules: Vec<&'static str>,
    /// 使った仕事の量
    pub work: u64,
}

impl Report {
    fn new(outcome: Outcome) -> Report {
        Report { outcome, latex: String::new(), input_latex: String::new(), engine: String::new(), approx: None, digits: None, decimal: false, uses_ln: false, rules: Vec::new(), work: 0 }
    }
}

/// 式の文字列の厳密値を求め、照らし合わせてから返す（計画の 2.4）
pub fn evaluate(source: &str, params: &Params) -> Report {
    evaluate_with(source, params, DEFAULT_WORK)
}

/// [`evaluate`] の仕事の量の上限を決めた版
pub fn evaluate_with(source: &str, params: &Params, work_limit: u64) -> Report {
    let (r, work) = convert_source_with(source, params, work_limit);
    let c = match r {
        Ok(c) => c,
        Err(Stop::NotFound(s)) => return Report { work, ..Report::new(Outcome::NotFound(s)) },
        Err(Stop::NotExist(s)) => return Report { work, ..Report::new(Outcome::NotExist(s)) },
        Err(Stop::TimeUp) => return Report { work, ..Report::new(Outcome::NotFound("時間切れ".into())) },
    };
    let mut rep = Report { decimal: c.decimal, uses_ln: c.uses_ln, work, ..Report::new(Outcome::NotFound(String::new())) };
    let t = c.term;
    // 組・リスト（成分が全部数）は成分ごとに照らし合わせる
    let items: Option<Vec<Term>> = match &t {
        Term::Tuple(v) | Term::List(v) | Term::Matrix(_, _, v) if v.iter().all(Term::is_number) => Some(v.clone()),
        _ => None,
    };
    if items.is_none() && !t.is_number() {
        rep.outcome = Outcome::NotFound("座標を含む式の厳密値はまだ出しません".into());
        return rep;
    }
    let tol = if c.analytic { check::F64_TOLERANCE } else { check::F64_TOLERANCE_ARITHMETIC };
    let verdict = check::with_tolerance(tol, || match &items {
        Some(v) => check::items_against_f64(v, source, params),
        None => check::against_f64(&t, source, params),
    });
    match verdict {
        check::Check::Agree => {}
        check::Check::Disagree { exact, other } => {
            rep.rules.push("照らし合わせで食い違った");
            rep.outcome = Outcome::Rejected(format!("照らし合わせで食い違いました（厳密値の道 {exact:?}、今の道 {other:?}）"));
            return rep;
        }
        check::Check::NoPartner(e) => {
            if c.analytic {
                rep.outcome = Outcome::NotFound(format!("照らし合わせの相手がありません（{e}）"));
                return rep;
            }
            rep.rules.push("四則と整数の関数だけなので、相手なしで採った");
        }
        check::Check::NoValue(e) => {
            rep.outcome = Outcome::NotFound(format!("値を確かめられません（{e}）"));
            return rep;
        }
    }
    if items.is_some() {
        rep.latex = latex::to_latex(&t, &latex::Style { decimal: c.decimal, input: false });
        rep.input_latex = latex::to_latex(&t, &latex::Style { decimal: c.decimal, input: true });
        rep.engine = latex::to_engine(&t);
        rep.outcome = Outcome::Found(t);
        return rep;
    }
    if let Ok(b) = ball::value(&t, 64) {
        rep.approx = Some(b.to_f64());
    }
    if let Ok(b) = ball::value(&t, 180) {
        let re = ball::to_decimal(&b.re, 50);
        rep.digits = Some(if b.is_real() { re } else { format!("{re} + ({}) i", ball::to_decimal(&b.im, 50)) });
    }
    rep.latex = latex::to_latex(&t, &latex::Style { decimal: c.decimal, input: false });
    rep.input_latex = latex::to_latex(&t, &latex::Style { decimal: c.decimal, input: true });
    rep.engine = latex::to_engine(&t);
    rep.outcome = Outcome::Found(t);
    rep
}

/// 時間切れ（仕事の量が予算を超えた）。同じ入力なら同じ所で止まる（計画の 2.5）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeUp;

/// 仕事の量の予算。大きな整数の掛け算・規則の適用などで [`Budget::charge`] を呼んで数える
#[derive(Debug)]
pub struct Budget {
    limit: u64,
    used: Cell<u64>,
}

impl Budget {
    pub fn new(limit: u64) -> Self {
        Budget { limit, used: Cell::new(0) }
    }

    /// 予算の上限なし（テスト用）
    pub fn unlimited() -> Self {
        Budget::new(u64::MAX)
    }

    /// 仕事を `n` だけ数える。上限を超えたら [`TimeUp`]
    pub fn charge(&self, n: u64) -> Result<(), TimeUp> {
        let u = self.used.get().saturating_add(n);
        self.used.set(u);
        if u > self.limit {
            Err(TimeUp)
        } else {
            Ok(())
        }
    }

    pub fn used(&self) -> u64 {
        self.used.get()
    }
}

//! 厳密値の道（計画 `docs/plan-exact-core.md`・`docs/plan-exact-areas.md`）。
//!
//! 今までの近似値の道（展開の段 → 命令列 → f64）とは別に、式の値を厳密に求める。
//! 結果は必ず別の道の値と照らし合わせてから出す（計画の 2.4）。画面では別の Worker で動く（EX5）。
//!
//! - [`num`]: 大きな整数・分数（dashu）と、整数の計算（EX1）
//! - [`term`]: 式の形と自動の正規化、[`convert`]: 展開の前の構文木からの変換（EX2）

pub mod convert;
pub mod num;
pub mod term;

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

/// 既定の仕事の量の上限（EX5 で、ふつうの PC で約 2 秒になる数に決め直す。計画の D7）
pub const DEFAULT_WORK: u64 = 50_000_000;

/// 式の文字列を、厳密値の道の式にする（照らし合わせ・書き出しの前の段階）。定義は今入っているもの（`expand::install`）。
/// 戻り値の 2 つ目は、入力に小数があったか
pub fn evaluate_term(source: &str, params: &Params) -> Result<(Term, bool), Stop> {
    let defs = crate::expand::current_defs();
    let names = defs.names();
    let e = crate::parser::parse_with(source, &names).map_err(|e| Stop::NotFound(format!("式を読めません: {e}")))?;
    let (r, _) = with_work_limit(DEFAULT_WORK, || {
        let mut c = convert::Converter::new(&defs, params);
        let t = c.term(&e)?;
        if exhausted() {
            return Err(Stop::TimeUp);
        }
        Ok((t, c.decimal))
    });
    r
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

//! 厳密値の道（計画 `docs/plan-exact-core.md`・`docs/plan-exact-areas.md`）。
//!
//! 今までの近似値の道（展開の段 → 命令列 → f64）とは別に、式の値を厳密に求める。
//! 結果は必ず別の道の値と照らし合わせてから出す（計画の 2.4）。画面では別の Worker で動く（EX5）。
//!
//! - [`num`]: 大きな整数・分数（dashu）と、整数の計算（EX1）

pub mod num;

use std::cell::Cell;

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

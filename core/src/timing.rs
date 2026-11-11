//! 計算時間の計測（ベンチマークと、画面側の内訳の表示に使う）。
//!
//! 式の計算（`eval` の命令列の実行）にかかった時間と点の数を、スレッドごとに足し合わせておく。
//! メッシュを作る関数は `reset` してから呼び、終わったら `take` で取り出す。

use std::cell::Cell;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod clock {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        // Worker でも画面でも使える `self.performance.now()`
        #[wasm_bindgen(js_namespace = performance, js_name = now)]
        fn performance_now() -> f64;
    }

    pub fn now_ms() -> f64 {
        performance_now()
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod clock {
    use std::sync::OnceLock;
    use std::time::Instant;

    static START: OnceLock<Instant> = OnceLock::new();

    pub fn now_ms() -> f64 {
        START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1e3
    }
}

/// 今の時刻（ミリ秒。差だけに意味がある）
pub fn now_ms() -> f64 {
    clock::now_ms()
}

thread_local! {
    static EVAL_MS: Cell<f64> = const { Cell::new(0.0) };
    static POINTS: Cell<f64> = const { Cell::new(0.0) };
}

/// 式の計算 1 回分（`points` 個の点で `ms` ミリ秒）を記録する
pub(crate) fn record_eval(ms: f64, points: usize) {
    EVAL_MS.with(|c| c.set(c.get() + ms));
    POINTS.with(|c| c.set(c.get() + points as f64));
}

/// 記録を 0 に戻す
pub fn reset() {
    EVAL_MS.with(|c| c.set(0.0));
    POINTS.with(|c| c.set(0.0));
}

/// `reset` してからの (式の計算の時間 [ms], 計算した点の数) を取り出して、0 に戻す
pub fn take() -> (f64, f64) {
    let r = (EVAL_MS.with(|c| c.get()), POINTS.with(|c| c.get()));
    reset();
    r
}

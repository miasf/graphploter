//! 教科書・貼り付けた LaTeX の書き方（第 4 段階 4a: #163 #164）。式は `web/src/lib/latex.ts` が出す形

use graph_core::equation::eval_constant;

fn value(src: &str) -> f64 {
    eval_constant(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn close(src: &str, want: f64) {
    let v = value(src);
    assert!((v - want).abs() <= 1e-12 * want.abs().max(1.0), "{src}: {v}（正しくは {want}）");
}

/// Σ・Π の添字、積分の変数の i は、その中だけ変数（外の i は虚数単位のまま。#163）
#[test]
fn index_i_is_bound() {
    close("@sum(i,1,5) i", 15.0);
    close("@prod(i,1,5) i", 120.0);
    close("@int(i,0,1)(i)", 0.5);
    close("@sum(i,1,3) i^(2)", 14.0);
    // (Σ i) + i = 15 + i
    close("Re(( @sum(i,1,5) i)+i)", 15.0);
    close("Im(( @sum(i,1,5) i)+i)", 1.0);
    // 入れ子: 内側の範囲の i も外側の添字。添字でない Σ の中の i は虚数単位
    close("@sum(i,1,3) @sum(j,1,i) (i j)", 25.0);
    close("Im(@sum(n,1,2) i)", 2.0);
}

/// 底のある対数 \log_b x（#164。逆三角関数は latex.ts で arcsin などにする）
#[test]
fn logarithm_with_base() {
    close("@logbase(2) 8", 3.0);
    close("@logbase(2) (((1)/(8)))", -3.0);
    close("@logbase(10) 1000 + 1", 4.0);
    close("2 @logbase(3) 9", 4.0);
    close("1/@logbase(2) 8", 1.0 / 3.0);
    close("arcsin (((1)/(2)))", std::f64::consts::PI / 6.0);
    close("arccos (1/2)", std::f64::consts::PI / 3.0);
}

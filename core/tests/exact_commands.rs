//! 形を指定する命令（計画 `docs/plan-exact-areas.md` の SC2）。答えの形の表は `exact_cases/commands.tsv`（`exact.rs`）

use graph_core::equation::{eval_batch_check, eval_constant};

#[test]
fn current_path_uses_the_inner_value() {
    // 今の道（グラフ・近似値）では、命令は中身そのもの
    assert_eq!(eval_constant("@factor(2^32 + 1)").unwrap(), 4294967297.0);
    assert_eq!(eval_constant("@expand((1 + 2)^3)").unwrap(), 27.0);
    for (cmd, inner) in [("@factor(x^4 - 1)", "x^4 - 1"), ("@expand((x + 1)^3)", "(x + 1)^3"), ("@apart(x/(x^2 - 3x + 2))", "x/(x^2 - 3x + 2)")] {
        let xs = [-1.5, 0.25, 3.0, 7.5];
        assert_eq!(eval_batch_check(cmd, &xs), eval_batch_check(inner, &xs), "{cmd}");
    }
}

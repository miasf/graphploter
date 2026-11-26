//! 根号の整理（計画 `docs/plan-exact-core.md` の EX7 のうち、有理化と入れ子の根号）

use graph_core::eval::Params;
use graph_core::exact::{evaluate, Outcome};
use graph_core::expand;

fn latex(src: &str) -> String {
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse("", &params));
    let r = evaluate(src, &params);
    match r.outcome {
        Outcome::Found(_) => r.latex,
        other => panic!("{src}: {other:?}"),
    }
}

#[test]
fn radicals() {
    assert_eq!(latex("sqrt(3+2sqrt(2))"), r"1+\sqrt{2}");
    assert_eq!(latex("sqrt(7-4sqrt(3))"), r"2-\sqrt{3}");
    assert_eq!(latex("sqrt(5+2sqrt(6))"), r"\sqrt{2}+\sqrt{3}");
    assert_eq!(latex("1/(sqrt(3)-1)"), r"\frac{1+\sqrt{3}}{2}");
    assert_eq!(latex("1/(1+sqrt(2)+sqrt(3))"), r"\frac{2+\sqrt{2}-\sqrt{6}}{4}");
    assert_eq!(latex("2/(sqrt(5)+sqrt(3))"), r"\sqrt{5}-\sqrt{3}");
    assert_eq!(latex("1/(sqrt(2)+sqrt(3)+sqrt(5))"), r"\frac{3\sqrt{2}+2\sqrt{3}-\sqrt{30}}{12}");
    // (√2 + √3)² − 5 − 2√6 = 0（数値に頼らず式の形で）
    assert_eq!(latex("(sqrt(2)+sqrt(3))^2-5-2sqrt(6)"), "0");
    // 外せない入れ子はそのまま
    assert_eq!(latex("sqrt(2+sqrt(2))"), r"\sqrt{2+\sqrt{2}}");
}

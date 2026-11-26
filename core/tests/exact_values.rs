//! 関数の特別な値（計画 `docs/plan-exact-areas.md` の SC0 の完了の条件）。書き出した LaTeX で比べる

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
fn special_values() {
    assert_eq!(latex("sin(π/12)"), r"\frac{\sqrt{6}-\sqrt{2}}{4}");
    assert_eq!(latex("cos(2π/5)"), r"\frac{\sqrt{5}-1}{4}");
    assert_eq!(latex("sin(7π/6)"), r"-\frac{1}{2}");
    assert_eq!(latex("tan(π/8)"), r"\sqrt{2}-1");
    assert_eq!(latex("tan(π/10)"), r"\frac{\sqrt{25-10\sqrt{5}}}{5}");
    assert_eq!(latex("@logbase(4) 8"), r"\frac{3}{2}");
    assert_eq!(latex("log(2)+log(5)"), "1");
    assert_eq!(latex("gamma(1/2)"), r"\sqrt{\pi}");
    assert_eq!(latex("gamma(5/2)"), r"\frac{3\sqrt{\pi}}{4}");
    assert_eq!(latex("gamma(-1/2)"), r"-2\sqrt{\pi}");
    assert_eq!(latex("zeta(-1)"), r"-\frac{1}{12}");
    assert_eq!(latex("zeta(2)"), r"\frac{\pi^{2}}{6}");
    assert_eq!(latex("zeta(4)"), r"\frac{\pi^{4}}{90}");
    assert_eq!(latex("zeta(3)"), r"\zeta(3)");
    assert_eq!(latex("digamma(1/2)"), r"-2\ln 2-\gamma");
    assert_eq!(latex("digamma(4)"), r"\frac{11}{6}-\gamma");
    assert_eq!(latex("arctan(2-sqrt(3))"), r"\frac{\pi}{12}");
    assert_eq!(latex("arccos(-sqrt(3)/2)"), r"\frac{5\pi}{6}");
    assert_eq!(latex("arg(1+i)"), r"\frac{\pi}{4}");
    assert_eq!(latex("arg(-1-i)"), r"-\frac{3\pi}{4}");
    assert_eq!(latex("e^(i π/5)"), r"\frac{1+\sqrt{5}}{4}+\frac{\sqrt{10-2\sqrt{5}}}{4}i");
    // 表に無い角はそのまま（近似値つき）
    assert_eq!(latex("sin(π/7)"), r"\sin\left(\frac{\pi}{7}\right)");
}

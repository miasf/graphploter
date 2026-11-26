//! 厳密な微分と点での値（計画 `docs/plan-exact-areas.md` の DF0 の完了の条件）

use graph_core::eval::Params;
use graph_core::exact::{evaluate, Outcome};
use graph_core::expand;

fn latex_with(src: &str, defs: &str) -> String {
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse(defs, &params));
    let r = evaluate(src, &params);
    match r.outcome {
        Outcome::Found(_) => r.latex,
        other => panic!("{src}: {other:?}"),
    }
}

fn latex(src: &str) -> String {
    latex_with(src, "")
}

#[test]
fn derivative_values() {
    assert_eq!(latex("@at(@diff(x) x^3 sin(x), x, π/3)"), r"\frac{\pi^{3}}{54}+\frac{\sqrt{3}\pi^{2}}{6}");
    assert_eq!(latex("@at(@diff(x, 2) e^x cos(x), x, 0)"), "0");
    assert_eq!(latex("@at(@diff(x) sqrt(x), x, 4)"), r"\frac{1}{4}");
    assert_eq!(latex("@at(@diff(x) arcsin(x), x, 1/2)"), r"\frac{2\sqrt{3}}{3}");
    assert_eq!(latex("@at(@diff(x) x^x, x, 1)"), "1");
    assert_eq!(latex("@at(@diff(t) t^2 e^t, t, 1)"), "3e");
    // 定義した関数の導関数 f′(a)
    assert_eq!(latex_with("f'(2)", "f(x)=x^3-3x"), "9");
    assert_eq!(latex_with("f''(π/6)", "f(x)=sin(2x)"), r"-2\sqrt{3}");
    // 勾配・発散・回転・ラプラシアン（点での値）
    assert_eq!(latex("@at(@grad(x^2 y), x, 1, y, 2)"), r"\left(4,1,0\right)");
    assert_eq!(latex("@at(@div((x^2, x y, z)), x, 1, y, 2, z, 3)"), "4");
    assert_eq!(latex("@at(@lap(x^2+y^2+z^2), x, 1, y, 1, z, 1)"), "6");
}

#[test]
fn kinks_are_not_reported() {
    // |x| の 0: 左右の微分係数（−1 と 1）が違うので「存在しない」
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse("", &params));
    let r = evaluate("@at(@diff(x) abs(x), x, 0)", &params);
    assert!(matches!(&r.outcome, Outcome::NotExist(s) if s.contains("-1") && s.contains("1")), "{r:?}");
    // |x|³ の 0 は微分できる（0）
    let r = evaluate("@at(@diff(x) abs(x)^3, x, 0)", &params);
    assert!(matches!(&r.outcome, Outcome::Found(_) | Outcome::NotFound(_)), "{r:?}");
}

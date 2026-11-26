//! 式の表示: 定義した関数を使った式を、代入した形で右下に出す（`f(x+2)` → $(x+2)^2$。`core/src/exact/expression.rs`）

use graph_core::eval::Params;
use graph_core::exact::{evaluate, Outcome, Report};
use graph_core::expand;

fn run(defs: &str, sliders: &[(&str, f64)], src: &str) -> Report {
    let mut params = Params::new();
    for (n, v) in sliders {
        params.insert(n.to_string(), num_complex::Complex64::new(*v, 0.0));
    }
    let _d = expand::install(expand::Defs::parse(defs, &params));
    evaluate(src, &params)
}

fn shown(defs: &str, sliders: &[(&str, f64)], src: &str) -> String {
    let r = run(defs, sliders, src);
    assert!(!r.equation, "{src}: 式の答えは「=」を前に付けて出す");
    match r.outcome {
        Outcome::Found(_) => r.latex,
        other => panic!("{src}: {other:?}"),
    }
}

#[test]
fn substituted_forms() {
    let f = "f(x) = x^2\ng(x) = x^3";
    assert_eq!(shown(f, &[], "f(x+2)"), r"\left(x+2\right)^{2}");
    assert_eq!(shown(f, &[], "f(sin(x))"), r"\sin^{2} x");
    assert_eq!(shown(f, &[], "f(g(x)+2y)"), r"\left(x^{3}+2y\right)^{2}");
    assert_eq!(shown("f(x,y) = x^2+y^2", &[], "f(x+1, y-1)"), r"\left(x+1\right)^{2}+\left(y-1\right)^{2}");
    // 数どうしは正規化でまとまる
    assert_eq!(shown("f(x) = 1/(x-1)", &[], "f(x+1)"), r"\frac{1}{x}");
    // 定義域を変える変形はしない（√(x²) は x でない）
    assert_eq!(shown("f(x) = sqrt(x)", &[], "f(x^2)"), r"\sqrt{x^{2}}");
    // 微分した関数も
    assert_eq!(shown(f, &[], "f'(x)"), "2x");
    // 小数の定義は小数で
    assert_eq!(shown("f(x) = 0.5x^2", &[], "f(x+2)"), r"0.5\left(x+2\right)^{2}");
}

#[test]
fn explicit_lines_and_adding_rows() {
    // y = f(x+2) の行: 右下は右辺、「行に追加」は y = … の行
    let r = run("f(x) = x^2", &[], "y = f(x+2)");
    assert!(matches!(r.outcome, Outcome::Found(_)));
    assert_eq!(r.latex, r"\left(x+2\right)^{2}");
    assert_eq!(r.input_latex, r"y=\left(x+2\right)^{2}");
    assert!(r.engine.starts_with("y="), "{}", r.engine);
    assert_eq!(shown("f(x) = x^2", &[], "z = f(x+2y)"), r"\left(x+2y\right)^{2}");
}

#[test]
fn sliders_stay_letters() {
    // スライダーは今の値にせず文字のまま
    assert_eq!(shown("f(x) = a x^2", &[("a", 3.0)], "f(x+2)"), r"a\left(x+2\right)^{2}");
    // 座標が残らない式は、今までどおり数の値（スライダーの今の値で）
    let r = run("f(x) = a x^2", &[("a", 3.0)], "f(2)");
    assert!(matches!(r.outcome, Outcome::Found(_)) && r.latex == "12", "{:?} {}", r.outcome, r.latex);
}

#[test]
fn other_routes_are_unchanged() {
    // 数の値
    assert_eq!(run("f(x) = x^2", &[], "f(2)").latex, "4");
    // 不定積分は原始関数の道（+ C）
    assert_eq!(run("f(x) = x^2", &[], "@int(x) f(x)").latex, r"\frac{x^{3}}{3}+C");
    // 方程式は解の道
    let r = run("f(x) = x^2", &[], "f(x+2) = 0");
    assert!(r.equation && r.latex == "x=-2", "{}", r.latex);
    // 定義した関数を使わない式は出さない（x² の行に「= x²」と出さない）
    assert!(matches!(run("f(x) = x^2", &[], "x^2+1").outcome, Outcome::NotFound(_)));
}

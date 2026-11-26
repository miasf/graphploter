//! 方程式の厳密な解（計画 `docs/plan-exact-areas.md` の SC1）の完了の条件の例。表の全体は `exact_cases/equations.tsv`（`exact.rs`）

use graph_core::eval::Params;
use graph_core::exact::{evaluate, Outcome};

fn solve(src: &str) -> String {
    let r = evaluate(src, &Params::new());
    match r.outcome {
        Outcome::Found(_) => r.latex,
        other => format!("{other:?}"),
    }
}

#[test]
fn completion_examples() {
    let cases = [
        ("x^2 = 2", "x=\\pm \\sqrt{2}"),
        ("x^3 - 6x^2 + 11x - 6 = 0", "x=1,\\ 2,\\ 3"),
        ("x^4 - 10x^2 + 1 = 0", "x=\\pm \\left(\\sqrt{3}-\\sqrt{2}\\right),\\ \\pm \\left(\\sqrt{2}+\\sqrt{3}\\right)"),
        ("x^3 - 3x + 1 = 0", "x=-2\\cos\\left(\\frac{\\pi}{9}\\right),\\ 2\\cos\\left(\\frac{4\\pi}{9}\\right),\\ 2\\cos\\left(\\frac{2\\pi}{9}\\right)"),
        ("sqrt(x + 2) = x", "x=2"),
        ("4^x - 3·2^x + 2 = 0", "x=0,\\ 1"),
        ("sin(x) = 1/2", "x=\\frac{\\pi}{6}+2\\pi n,\\ \\frac{5\\pi}{6}+2\\pi n\\quad\\left(n\\in\\mathbb{Z}\\right)"),
        ("x^2 + y^2 = 1, y = x", "\\left(x,y\\right)=\\left(-\\frac{\\sqrt{2}}{2},-\\frac{\\sqrt{2}}{2}\\right),\\ \\left(\\frac{\\sqrt{2}}{2},\\frac{\\sqrt{2}}{2}\\right)"),
        ("x + y = 3, x y = 2", "\\left(x,y\\right)=\\left(1,2\\right),\\ \\left(2,1\\right)"),
    ];
    let mut bad = Vec::new();
    for (src, want) in cases {
        let got = solve(src);
        println!("{src}  →  {got}");
        if !want.is_empty() && got != want {
            bad.push(format!("{src}: {got} （期待 {want}）"));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn equation_rows_that_are_not_solved() {
    // 曲線（文字が 2 つ）・縦の直線そのもの（x = 3）は、方程式の解を出さない（今までどおり何も出さない）
    for src in ["y = x^2", "x = 3", "x^2 + y^2 = 1"] {
        let r = evaluate(src, &Params::new());
        assert!(!r.equation || !matches!(r.outcome, Outcome::Found(_)), "{src}: {:?}", r.outcome);
    }
    // 方程式でない行は今までどおり
    let r = evaluate("1/3 + 1/6", &Params::new());
    assert!(!r.equation && r.latex == "\\frac{1}{2}", "{}", r.latex);
}

#[test]
fn domain_of_the_original_expression() {
    // 正規化で (x − 1)²/(x − 1) = x − 1 にまとまっても、元の式は x = 1 で 0 で割る（SymPy も式を作った時点でまとめるので、表には入れない）
    assert_eq!(solve("(x - 1)^2/(x - 1) = 0"), "\\text{実数の解はありません}");
    assert_eq!(solve("(x^2 - 1)/(x - 1) = 2"), "\\text{実数の解はありません}");
}

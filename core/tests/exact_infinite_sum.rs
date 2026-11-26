//! 無限級数の値（計画 `docs/plan-exact-areas.md` の SR1 の完了の条件）

use graph_core::eval::Params;
use graph_core::exact::{evaluate, Outcome};
use graph_core::expand;

fn run(src: &str) -> (Outcome, String) {
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse("", &params));
    let r = evaluate(src, &params);
    (r.outcome, r.latex)
}

fn latex(src: &str) -> String {
    match run(src) {
        (Outcome::Found(_), l) => l,
        (other, _) => panic!("{src}: {other:?}"),
    }
}

#[test]
fn values() {
    assert_eq!(latex("@sum(k,1,∞) 1/(k(k+1))"), "1");
    assert_eq!(latex("@sum(k,0,∞) k/2^k"), "2");
    assert_eq!(latex("@sum(k,0,∞) 1/k!"), "e");
    assert_eq!(latex("@sum(k,1,∞) 1/k^2"), r"\frac{\pi^{2}}{6}");
    assert_eq!(latex("@sum(k,1,∞) (-1)^(k+1)/k"), r"\ln 2");
    assert_eq!(latex("@sum(k,1,∞) 1/(2k-1)^2"), r"\frac{\pi^{2}}{8}");
    assert_eq!(latex("@sum(k,0,∞) (-1)^k/(2k+1)"), r"\frac{\pi}{4}");
    assert_eq!(latex("@sum(k,0,∞) (-1)^k/(2k+1)^2"), "G");
    assert_eq!(latex("@sum(k,1,∞) 1/k^3"), r"\zeta(3)");
    assert_eq!(latex("@sum(k,0,∞) k^2/3^k"), r"\frac{3}{2}");
    assert_eq!(latex("@sum(k,1,∞) 1/(k(k+2))"), r"\frac{3}{4}");
    assert_eq!(latex("@sum(k,1,∞) 1/(2^k k)"), r"\ln 2");
    assert_eq!(latex("@sum(k,0,∞) 1/(2k+1)!"), r"\frac{e}{2}-\frac{1}{2e}");
    assert_eq!(latex("@sum(k,1,∞) k/(k+1)!"), "1");
    // 1/3・1/6 のずれ（ガウスのディガンマ関数の値）: Σ 1/((3k+1)(3k+2)) = π/(3√3)
    assert_eq!(latex("@sum(k,0,∞) 1/((3k+1)(3k+2))"), r"\frac{\sqrt{3}\pi}{9}");
}

#[test]
fn divergence() {
    let not_exist = |src: &str| matches!(run(src).0, Outcome::NotExist(_));
    assert!(not_exist("@sum(k,1,∞) 1/k"));
    assert!(not_exist("@sum(k,1,∞) k/(k^2+1)"));
    assert!(not_exist("@sum(k,0,∞) (-1)^k"));
    assert!(not_exist("@sum(k,0,∞) 2^k/k^5"));
    // 判定できない形は「存在しない」と言わない
    assert!(matches!(run("@sum(k,2,∞) 1/(k ln(k))").0, Outcome::NotFound(_)));
}

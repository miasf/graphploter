//! 極限（計画 `docs/plan-exact-areas.md` の LM1 の完了の条件）

use graph_core::eval::Params;
use graph_core::exact::{evaluate, Outcome};
use graph_core::expand;

fn run(src: &str) -> Outcome {
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse("", &params));
    evaluate(src, &params).outcome
}

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
fn limits() {
    assert_eq!(latex("@lim(x,0) (1-cos(x))/x^2"), r"\frac{1}{2}");
    assert_eq!(latex("@lim(x,0) (tan(x)-sin(x))/x^3"), r"\frac{1}{2}");
    assert_eq!(latex("@lim(x,∞) (sqrt(x^2+x)-x)"), r"\frac{1}{2}");
    assert_eq!(latex("@lim(x,0,1) x^x"), "1");
    assert_eq!(latex("@lim(x,∞) (1+1/x)^x"), "e");
    assert_eq!(latex("@lim(x,∞) (1-1/x)^x"), r"\frac{1}{e}");
    assert_eq!(latex("@lim(x,0) (e^x-1-x)/x^2"), r"\frac{1}{2}");
    assert_eq!(latex("@lim(x,0,1) x ln(x)"), "0");
    assert_eq!(latex("@lim(x,0) (2^x-1)/x"), r"\ln 2");
    assert_eq!(latex("@lim(x,2) (x^3-8)/(x-2)"), "12");
    assert_eq!(latex("@lim(x,∞) x^(1/x)"), "1");
    // 片側だけ定義される: √x の 0 は右からの値（両側の書き方では、今の道が数値で確かめられないので照らし合わせの相手が無く、出さない）
    assert_eq!(latex("@lim(x,0,1) sqrt(x)"), "0");
    assert!(matches!(run("@lim(x,0) sqrt(x)"), Outcome::NotFound(_)));
    // 極限の中の式に組み込める
    assert_eq!(latex("1+@lim(x,0) sin(x)/x"), "2");
}

#[test]
fn not_exist() {
    let reason = |src: &str| match run(src) {
        Outcome::NotExist(r) => r,
        other => panic!("{src}: {other:?}"),
    };
    assert!(reason("@lim(x,0) abs(x)/x").contains("左からは -1、右からは 1"));
    assert!(reason("@lim(x,0) 1/x").contains("左からは −∞、右からは +∞"));
    assert!(reason("@lim(x,0) 1/x^2").contains("+∞ に発散"));
    assert!(reason("@lim(x,0) sin(1/x)").contains("振動"));
    // x sin(1/x) は振動しない（0 に近づく）が、まだ展開できないので「見つからない」（「存在しない」と言わない）
    assert!(matches!(run("@lim(x,0) x sin(1/x)"), Outcome::NotFound(_) | Outcome::Found(_)));
}

//! 積分（計画 `docs/plan-exact-areas.md` の IG0〜IG2 の完了の条件）

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
fn antiderivatives() {
    assert_eq!(latex("@int(x) x^2 e^x"), r"\left(x^{2}-2x+2\right)e^{x}+C");
    assert_eq!(latex("@int(x) x ln(x)"), r"\frac{x^{2}\ln x}{2}-\frac{x^{2}}{4}+C");
    assert_eq!(latex("@int(x) sin(x)^3"), r"\frac{\cos^{3} x}{3}-\cos x+C");
    assert_eq!(latex("@int(x) e^x sin(x)"), r"\frac{e^{x}\left(\sin x-\cos x\right)}{2}+C");
    assert_eq!(latex("@int(x) 1/(x ln(x))"), r"\ln\left|\ln x\right|+C");
    assert_eq!(latex("@int(x) x/(x^2-3x+2)"), r"2\ln\left|x-2\right|-\ln\left|x-1\right|+C");
    // 有理関数（IG1）: 1/(x³ + 1) は ln と arctan
    let l = latex("@int(x) 1/(x^3+1)");
    assert!(l.contains(r"\arctan") && l.contains(r"\ln\left|x+1\right|"), "{l}");
    // 1/(x⁴ + 1) は有理数の上で分解できない 4 次なので、まだ見つからない（値を間違えない）
    assert!(matches!(run("@int(x) 1/(x^4+1)").0, Outcome::NotFound(_)));
}

#[test]
fn definite() {
    assert_eq!(latex("@int(x,0,π) sin(x)^2"), r"\frac{\pi}{2}");
    assert_eq!(latex("@int(x,1,2) ln(x)"), r"2\ln 2-1");
    assert_eq!(latex("@int(x,0,∞) e^(-x^2)"), r"\frac{\sqrt{\pi}}{2}");
    assert_eq!(latex("@int(x,0,∞) x^3 e^(-x)"), "6");
    assert_eq!(latex("@int(x,-∞,∞) 1/(x^2+1)"), r"\pi");
    assert_eq!(latex("@int(x,0,1) x^2(1-x)^3"), r"\frac{1}{60}");
    assert_eq!(latex("@int(x,0,∞) 1/(1+x^4)"), r"\frac{\sqrt{2}\pi}{4}");
    assert_eq!(latex("@int(x,0,∞) e^(-x) ln(x)"), r"-\gamma");
    assert_eq!(latex("@int(x,0,π/2) sin(x)^4 cos(x)^2"), r"\frac{\pi}{32}");
    assert_eq!(latex("@int(x,0,1) 1/sqrt(x)"), "2");
    assert_eq!(latex("@int(x,0,2π) 1/(2+cos(x))"), r"\frac{2\sqrt{3}\pi}{3}");
    assert_eq!(latex("@int(x,0,∞) sin(x)/x"), r"\frac{\pi}{2}");
    assert_eq!(latex("@int(x,0,1) arctan(x)/x"), "G");
    assert_eq!(latex("@int(x,0,2) abs(x-1)"), "1");
    assert_eq!(latex("@int(x,-1,1) x^5 cos(x)"), "0");
    assert!(matches!(run("@int(x,-1,1) 1/x").0, Outcome::NotExist(_)));
    assert!(matches!(run("@int(x,1,∞) 1/x").0, Outcome::NotExist(_)));
}

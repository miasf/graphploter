//! 式の級数展開（計画 `docs/plan-exact-areas.md` の LM0 の完了の条件）

use graph_core::eval::Params;
use graph_core::exact::evaluate_term;
use graph_core::exact::num::{Int, Nat, Rat};
use graph_core::exact::series::{expand, Map, Ser};
use graph_core::exact::term::Term;
use graph_core::expand;

fn term(src: &str) -> Term {
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse("", &params));
    evaluate_term(src, &params).unwrap().0
}

fn q(n: i64, d: u64) -> Rat {
    Rat::from_parts(Int::from(n), Nat::from(d))
}

fn at0(src: &str, cap: i64) -> Ser {
    expand(&term(src), "x", &Map::Finite(Term::zero(), 1), &Rat::from(Int::from(cap))).unwrap()
}

/// (指数, 係数) の並びが期待どおり（係数は有理数）
fn check(s: &Ser, want: &[(Rat, Rat)], order: Rat) {
    let got: Vec<(Rat, Rat)> = s.terms.iter().map(|(e, c)| (e.clone(), c.as_rat().unwrap_or_else(|| panic!("係数が有理数でない: {c:?}")).clone())).collect();
    assert_eq!(got, want, "{s:?}");
    assert_eq!(s.order, order, "{s:?}");
}

fn i(n: i64) -> Rat {
    q(n, 1)
}

#[test]
fn expansions() {
    check(&at0("tan(x)", 9), &[(i(1), i(1)), (i(3), q(1, 3)), (i(5), q(2, 15)), (i(7), q(17, 315))], i(9));
    check(&at0("1/(1-x)-e^x", 4), &[(i(2), q(1, 2)), (i(3), q(5, 6))], i(4));
    check(&at0("1/sin(x)", 5), &[(i(-1), i(1)), (i(1), q(1, 6)), (i(3), q(7, 360))], i(5));
    check(&at0("sqrt(x+x^2)", 3), &[(q(1, 2), i(1)), (q(3, 2), q(1, 2)), (q(5, 2), q(-1, 8))], i(3));
    // 相殺: sin x − x + x³/6 の先頭は x⁵/120
    let s = at0("sin(x)-x+x^3/6", 8);
    assert_eq!(s.lead().map(|(e, c)| (e, c)), Some((i(5), Term::Num(q(1, 120)))));
    // ∞ での展開（x = 1/h）: √(x² + x) − x = 1/2 − h/8 + …
    let s = expand(&term("sqrt(x^2+x)-x"), "x", &Map::Infinite(1), &i(3)).unwrap();
    assert_eq!(s.lead(), Some((i(0), Term::Num(q(1, 2)))));
}

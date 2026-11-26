//! 厳密値の道の式の形と自動の正規化（計画 `docs/plan-exact-core.md` の EX2 の完了の条件）。
//! `Term` を直接作って比べる（書き出しは EX4）

use graph_core::eval::Params;
use graph_core::exact::num::{Int, Nat, Rat};
use graph_core::exact::term::{add, mul, pow, Atom, Term};
use graph_core::exact::{evaluate_term, Stop};
use graph_core::expand;

fn q(n: i64, d: u64) -> Term {
    Term::Num(Rat::from_parts(Int::from(n), Nat::from(d)))
}

fn n(v: i64) -> Term {
    q(v, 1)
}

fn sqrt(t: Term) -> Term {
    pow(t, q(1, 2))
}

fn x() -> Term {
    Term::Sym("x".into())
}

fn i() -> Term {
    Term::Atom(Atom::I)
}

fn pi() -> Term {
    Term::Atom(Atom::Pi)
}

fn ln(t: Term) -> Term {
    graph_core::exact::term::call(graph_core::functions::Func::Ln, vec![t])
}

fn ev(src: &str) -> Term {
    ev_with(src, "")
}

fn ev_with(src: &str, defs: &str) -> Term {
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse(defs, &params));
    match evaluate_term(src, &params) {
        Ok((t, _)) => t,
        Err(Stop::NotFound(r)) => panic!("{src}: 見つからない（{r}）"),
        Err(e) => panic!("{src}: {e:?}"),
    }
}

#[test]
fn numbers_stay_exact() {
    assert_eq!(ev("1/3+1/6"), q(1, 2));
    assert_eq!(ev("0.1+0.2"), q(3, 10));
    assert_eq!(ev("sqrt(2)^2-2"), n(0));
    assert_eq!(ev("sqrt(8)"), mul(vec![n(2), sqrt(n(2))]));
    assert_eq!(ev("sqrt(12) sqrt(3)"), n(6));
    assert_eq!(ev("sqrt(2) sqrt(3)"), sqrt(n(6)));
    assert_eq!(ev("sqrt(3/4)"), mul(vec![q(1, 2), sqrt(n(3))]));
    assert_eq!(ev("1/sqrt(2)"), mul(vec![q(1, 2), sqrt(n(2))]));
    assert_eq!(ev("8^(2/3)"), n(4));
    assert_eq!(ev("4^(1/6)"), pow(n(2), q(1, 3)));
    assert_eq!(ev("16^(3/4)"), n(8));
    assert_eq!(ev("(4/9)^(3/2)"), q(8, 27));
    assert_eq!(ev("2^(-3)"), q(1, 8));
    assert_eq!(ev("(-8)^(1/3)"), add(vec![n(1), mul(vec![sqrt(n(3)), i()])]));
    assert_eq!(ev("sqrt(-4)"), mul(vec![n(2), i()]));
}

#[test]
fn numeric_sums_are_expanded() {
    assert_eq!(ev("(1+sqrt(2))^2"), add(vec![n(3), mul(vec![n(2), sqrt(n(2))])]));
    assert_eq!(ev("(sqrt(3)+sqrt(2))(sqrt(3)-sqrt(2))"), n(1));
    assert_eq!(ev("(2+sqrt(3))^3"), add(vec![n(26), mul(vec![n(15), sqrt(n(3))])]));
    assert_eq!(ev("1/(sqrt(3)-1)"), add(vec![q(1, 2), mul(vec![q(1, 2), sqrt(n(3))])]));
    assert_eq!(ev("1/(2-sqrt(3))"), add(vec![n(2), sqrt(n(3))]));
    assert_eq!(ev("(1+i)^2"), mul(vec![n(2), i()]));
    assert_eq!(ev("(1+i)^10"), mul(vec![n(32), i()]));
    assert_eq!(ev("1/(1+i)"), add(vec![q(1, 2), mul(vec![q(-1, 2), i()])]));
    assert_eq!(ev("i^2"), n(-1));
    assert_eq!(ev("(1+sqrt(3)i)^6"), n(64));
}

#[test]
fn logs_and_exponentials() {
    assert_eq!(ev("ln(e^3)"), n(3));
    assert_eq!(ev("e^(2ln(3))"), n(9));
    assert_eq!(ev("e^(ln(2)+ln(3))"), n(6));
    assert_eq!(ev("ln(8)"), mul(vec![n(3), ln(n(2))]));
    assert_eq!(ev("ln(-1)"), mul(vec![pi(), i()]));
    assert_eq!(ev("e^(i π)+1"), n(0));
    assert_eq!(ev("e^(i π/3)"), add(vec![q(1, 2), mul(vec![q(1, 2), sqrt(n(3)), i()])]));
    assert_eq!(ev("log(1000)"), n(3));
    assert_eq!(ev("log(0.01)"), n(-2));
}

#[test]
fn integers() {
    assert_eq!(ev("25!"), Term::Num(Rat::from("15511210043330985984000000".parse::<Int>().unwrap())));
    assert_eq!(ev("nCr(60,30)"), Term::Num(Rat::from("118264581564861424".parse::<Int>().unwrap())));
    assert_eq!(ev("gcd(2^60+1, 3^40)"), n(1));
    assert_eq!(ev("mod(2^61-1, 1000)"), n(951));
    assert_eq!(ev("12!/(5! 7!)"), n(792));
    assert_eq!(ev("@sum(k,1,100) k^2"), n(338350));
    assert_eq!(ev("@prod(k,1,5) (2k-1)"), n(945));
    assert_eq!(ev("@sum(k,1,10) 1/(k(k+1))"), q(10, 11));
}

#[test]
fn symbols_are_not_rewritten_unsafely() {
    assert_eq!(add(vec![x(), x()]), mul(vec![n(2), x()]));
    assert_eq!(mul(vec![x(), pow(x(), n(2))]), pow(x(), n(3)));
    let sq = pow(add(vec![x(), n(1)]), n(2));
    assert!(matches!(sq, Term::Pow(..)), "(x+1)^2 は展開しない: {sq:?}");
    let r = sqrt(pow(x(), n(2)));
    assert_eq!(r, Term::Pow(Box::new(pow(x(), n(2))), Box::new(q(1, 2))), "√(x²) は x にしない");
    assert_eq!(mul(vec![n(2), add(vec![x(), n(1)])]), add(vec![mul(vec![n(2), x()]), n(2)]));
    assert_eq!(add(vec![x(), mul(vec![n(-1), x()])]), n(0));
}

#[test]
fn definitions_are_applied() {
    assert_eq!(ev_with("f(1/2)", "f(x)=x^2+1"), q(5, 4));
    assert_eq!(ev_with("g(2)", "f(x)=x^2+1\ng(x)=f(x)+f(1)"), n(7));
}

/// 和の項の並べ方が入力の順に関係しない
#[test]
fn order_does_not_matter() {
    let parts = vec![
        x(),
        Term::Sym("y".into()),
        n(2),
        pow(x(), n(2)),
        graph_core::exact::term::call(graph_core::functions::Func::Sin, vec![x()]),
        pi(),
        sqrt(n(2)),
        mul(vec![n(3), x(), Term::Sym("y".into())]),
    ];
    let want = add(parts.clone());
    let mut s: u64 = 12345;
    for _ in 0..1000 {
        let mut p = parts.clone();
        for k in (1..p.len()).rev() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            p.swap(k, (s >> 33) as usize % (k + 1));
        }
        // 2 つずつ足していっても同じ
        let mut acc = n(0);
        for t in p.clone() {
            acc = add(vec![acc, t]);
        }
        assert_eq!(acc, want);
        assert_eq!(add(p), want);
    }
}

#[test]
fn not_found_reasons() {
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse("", &params));
    assert!(matches!(evaluate_term("@max(x,0,1) x^2", &params), Err(Stop::NotFound(_))));
    assert!(matches!(evaluate_term("a+1", &params), Err(Stop::NotFound(_))));
    assert!(matches!(evaluate_term("1/0", &params), Err(Stop::NotFound(_))));
}

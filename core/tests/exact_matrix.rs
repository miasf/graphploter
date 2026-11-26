//! 厳密な行列（計画 `docs/plan-exact-areas.md` の LA0 の完了の条件）

use graph_core::eval::Params;
use graph_core::exact::term::{add, mul, pow, Term};
use graph_core::exact::{evaluate, evaluate_term, Outcome};
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
fn exact_matrices() {
    assert_eq!(latex("@inv(⟦1,2,3;4,5,6;7,8,10⟧)"), r"\begin{pmatrix}-\frac{2}{3} & -\frac{4}{3} & 1\\-\frac{2}{3} & \frac{11}{3} & -2\\1 & -2 & 1\end{pmatrix}");
    assert_eq!(latex("@rank(⟦1,2;2,4⟧)"), "1");
    assert_eq!(latex("@det(⟦sqrt(2),1;1,sqrt(2)⟧)"), "1");
    assert_eq!(latex("⟦1,2;3,4⟧^2"), r"\begin{pmatrix}7 & 10\\15 & 22\end{pmatrix}");
    assert_eq!(latex("⟦2,1;1,1⟧^(-1)"), r"\begin{pmatrix}1 & -1\\-1 & 2\end{pmatrix}");
    // ヒルベルト行列 6 × 6 の逆行列は整数（f64 では誤差が出る）。左上は 36、右下は 11·C(10,5)² = 698544
    let h: Vec<String> = (1..=6).map(|i| (1..=6).map(|j| format!("1/{}", i + j - 1)).collect::<Vec<_>>().join(",")).collect();
    let src = format!("@inv(⟦{}⟧)", h.join(";"));
    let l = latex(&src);
    assert!(l.starts_with(r"\begin{pmatrix}36 & -630"), "{l}");
    assert!(l.ends_with(r"698544\end{pmatrix}"), "{l}");
    assert!(!l.contains("frac"), "{l}");
    // 逆行列が無い
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse("", &params));
    assert!(matches!(evaluate("@inv(⟦1,2;2,4⟧)", &params).outcome, Outcome::NotExist(_) | Outcome::NotFound(_)));
    // 文字を含む行列式（式の形で）: det ⟦x,1;1,x⟧ = x² − 1
    let (t, _) = evaluate_term("@det(⟦x,1;1,x⟧)", &params).unwrap();
    assert_eq!(t, add(vec![pow(Term::Sym("x".into()), Term::int(2)), Term::int(-1)]));
    let (t, _) = evaluate_term("@det(⟦x,y;y,x⟧)", &params).unwrap();
    assert_eq!(t, add(vec![pow(Term::Sym("x".into()), Term::int(2)), mul(vec![Term::int(-1), pow(Term::Sym("y".into()), Term::int(2))])]));
}

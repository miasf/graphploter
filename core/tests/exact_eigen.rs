//! 固有値・対角化・ジョルダン標準形・グラム・シュミット（計画 `docs/plan-exact-areas.md` の LA1 の完了の条件）。
//! 例題の表（`exact_cases/eigen.tsv`）に入らない形（3 次の固有値・根号で表せない固有値・定義した行列・大きさの上限）もここで確かめる

use graph_core::eval::Params;
use graph_core::exact::{evaluate, Outcome};
use graph_core::expand;

fn answer(defs: &str, src: &str) -> Outcome {
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse(defs, &params));
    evaluate(src, &params).outcome
}

fn latex(defs: &str, src: &str) -> String {
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse(defs, &params));
    let r = evaluate(src, &params);
    assert!(r.equation, "{src}: 答えは文（方程式の解と同じ出し方）");
    match r.outcome {
        Outcome::Found(_) => r.latex,
        other => panic!("{src}: {other:?}"),
    }
}

#[test]
fn completion_examples() {
    // ⟦2,1;1,2⟧ → 1, 3
    let l = latex("", "@eig(⟦2,1;1,2⟧)");
    assert!(l.contains(r"\lambda=1:\ \begin{pmatrix}-1\\1\end{pmatrix}") && l.contains(r"\lambda=3:\ \begin{pmatrix}1\\1\end{pmatrix}"), "{l}");
    // ⟦4,1;2,3⟧ → P = ⟦−1,1;2,1⟧、D = diag(2, 5)
    assert_eq!(
        latex("", "@diagonalize(⟦4,1;2,3⟧)"),
        r"P=\begin{pmatrix}-1 & 1\\2 & 1\end{pmatrix},\ D=\begin{pmatrix}2 & 0\\0 & 5\end{pmatrix}\quad\left(P^{-1}AP=D\right)"
    );
    // ⟦0,1,0;0,0,1;2,−5,4⟧ の固有多項式 (λ − 1)²(λ − 2) とジョルダン標準形 J₂(1) ⊕ (2)
    assert!(latex("", "@eig(⟦0,1,0;0,0,1;2,-5,4⟧)").starts_with(r"\text{固有多項式 }\left(\lambda-1\right)^{2}\left(\lambda-2\right)"));
    assert!(latex("", "@jordan(⟦0,1,0;0,0,1;2,-5,4⟧)").contains(r"J=\begin{pmatrix}1 & 1 & 0\\0 & 1 & 0\\0 & 0 & 2\end{pmatrix}"));
    // (1,1,0), (1,0,1), (0,1,1) のグラム・シュミット
    assert_eq!(
        latex("", "@gramschmidt((1,1,0),(1,0,1),(0,1,1))"),
        r"\mathbf{u}_{1}=\frac{1}{\sqrt{2}}\begin{pmatrix}1\\1\\0\end{pmatrix},\ \mathbf{u}_{2}=\frac{1}{\sqrt{6}}\begin{pmatrix}1\\-1\\2\end{pmatrix},\ \mathbf{u}_{3}=\frac{1}{\sqrt{3}}\begin{pmatrix}-1\\1\\1\end{pmatrix}"
    );
}

#[test]
fn not_diagonalizable_and_defined_matrices() {
    assert_eq!(latex("", "@diagonalize(⟦3,1;-1,1⟧)"), r"\text{対角化できません（固有値 }\lambda=2\text{ の重複度は 2、固有ベクトルは 1 本）}");
    // 定義した行列（A = ⟦…⟧ の行）を使える。LA0 の行列式・べきも
    assert!(latex("A = ⟦2,1;1,2⟧", "@eig(A)").contains(r"\lambda=3"));
    assert!(latex("A = ⟦2,1;1,2⟧", "@eig A").contains(r"\lambda=1"));
    let params = Params::new();
    let _d = expand::install(expand::Defs::parse("A = ⟦2,1;1,2⟧", &params));
    let r = evaluate("@det(A)", &params);
    assert!(matches!(r.outcome, Outcome::Found(_)) && r.latex == "3", "{:?}", r.outcome);
}

#[test]
fn cubic_and_unexpressed_eigenvalues() {
    // 既約な 3 次の固有多項式（実根が 3 つ）: 根は三角関数の形、固有ベクトルは λ の式
    let l = latex("", "@eig(⟦1,2,3;4,5,6;7,8,10⟧)");
    assert!(l.contains(r"\lambda^{3}-16\lambda^{2}-12\lambda+3=0\text{ の根}"), "{l}");
    assert!(l.contains(r"\begin{pmatrix}-8\lambda^{2}+134\lambda+12\\7\lambda^{2}-109\lambda-93\\66\end{pmatrix}"), "{l}");
    // 根号で表せない 5 次（λ⁵ − λ − 1 の同伴行列）: 「の根」のまま、固有ベクトルは λ の式
    let l = latex("", "@eig(⟦0,0,0,0,1;1,0,0,0,1;0,1,0,0,0;0,0,1,0,0;0,0,0,1,0⟧)");
    assert!(l.contains(r"\lambda^{5}-\lambda-1=0\text{ の根 }\lambda"), "{l}");
    // P は出さない（固有値を根号で表せない・3 次の根を入れると式が大きくなる）
    assert!(matches!(answer("", "@diagonalize(⟦1,2,3;4,5,6;7,8,10⟧)"), Outcome::NotFound(_)));
    // 実根が 1 つの 3 次（カルダノ）: 複素数の 2 つの根も出す
    let l = latex("", "@eig(⟦0,0,1;1,0,1;0,1,0⟧)");
    assert!(l.contains(r"\lambda^{3}-\lambda-1=0\text{ の根}") && l.matches(r"\sqrt[3]").count() >= 2, "{l}");
}

#[test]
fn jordan_chains() {
    // 3 重の固有値（細胞 1 つ）: J₃(2)
    let l = latex("", "@jordan(⟦2,1,1;0,2,1;0,0,2⟧)");
    assert!(l.contains(r"J=\begin{pmatrix}2 & 1 & 0\\0 & 2 & 1\\0 & 0 & 2\end{pmatrix}"), "{l}");
    // 4 重の固有値で細胞が 2 つと 1 つと 1 つ: J₂(1) ⊕ (1) ⊕ (1)
    let l = latex("", "@jordan(⟦1,1,0,0;0,1,0,0;0,0,1,0;0,0,0,1⟧)");
    assert!(l.contains(r"J=\begin{pmatrix}1 & 1 & 0 & 0\\0 & 1 & 0 & 0\\0 & 0 & 1 & 0\\0 & 0 & 0 & 1\end{pmatrix}"), "{l}");
    // 根号の固有値の重なり（(λ² − 2)²）: Q(√2) の中で鎖を作る
    let l = latex("", "@jordan(⟦0,1,1,0;2,0,0,1;0,0,0,1;0,0,2,0⟧)");
    assert!(l.contains(r"J=\begin{pmatrix}-\sqrt{2} & 1 & 0 & 0\\0 & -\sqrt{2} & 0 & 0\\0 & 0 & \sqrt{2} & 1\\0 & 0 & 0 & \sqrt{2}\end{pmatrix}"), "{l}");
    // 大きさの上限（D4）: ジョルダン標準形は 4 × 4 まで
    assert!(matches!(answer("", "@jordan(⟦1,0,0,0,0;0,1,0,0,0;0,0,1,0,0;0,0,0,1,0;0,0,0,0,1⟧)"), Outcome::NotFound(_)));
}

#[test]
fn gram_schmidt_edge_cases() {
    // 1 次結合のベクトルは除く（今の道と同じ）
    let l = latex("", "@gramschmidt((1,1,1),(2,2,2),(1,0,0))");
    assert!(l.ends_with(r"\text{（2 本目は前のベクトルの 1 次結合なので除いた）}"), "{l}");
    // 行列 1 つなら列ベクトル
    assert_eq!(latex("", "@gramschmidt(⟦1,1;0,1⟧)"), r"\mathbf{u}_{1}=\begin{pmatrix}1\\0\end{pmatrix},\ \mathbf{u}_{2}=\begin{pmatrix}0\\1\end{pmatrix}");
    // 根号の成分
    assert!(latex("", "@gramschmidt((1,sqrt(2)),(0,1))").contains(r"\frac{\sqrt{3}}{3}"));
    // 5 本は上限（D4）を超える
    assert!(matches!(answer("", "@gramschmidt((1,0,0,0,0),(0,1,0,0,0),(0,0,1,0,0),(0,0,0,1,0),(0,0,0,0,1))"), Outcome::NotFound(_)));
}

#[test]
fn not_yet() {
    // 文字・根号の成分の行列はまだ（0 章）
    assert!(matches!(answer("", "@eig(⟦x,1;1,x⟧)"), Outcome::NotFound(_)));
    assert!(matches!(answer("", "@eig(⟦sqrt(2),1;1,sqrt(2)⟧)"), Outcome::NotFound(_)));
    assert!(matches!(answer("", "@eig(⟦1,2,3;4,5,6⟧)"), Outcome::NotFound(_)));
}

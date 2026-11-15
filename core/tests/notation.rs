//! 数学の記法（計画 `docs/plan-math-notation.md`）。画面と同じ入口（`graph_core::plot` と `definitions`）で確かめる

use graph_core::{definitions, plot, MeshResult};

/// 3 次元・既定に近い範囲で描く。`defs` は式の一覧のほかの行（`\n` 区切り）、`params` は (名前, 値) の並び
fn run(src: &str, defs: &str, params: &[(&str, f64)]) -> Result<MeshResult, String> {
    definitions(defs);
    let names: Vec<&str> = params.iter().map(|p| p.0).collect();
    let values: Vec<f64> = params.iter().flat_map(|p| [p.1, 0.0]).collect();
    plot(
        src,
        &[-3.0, 3.0, -3.0, 3.0, -3.0, 3.0],
        40,
        24,
        "t",
        &[0.0, 5.0],
        &names.join(","),
        &values,
        "",
        3,
        &[0, 1, 2],
        &[0.0; 3],
        &[f64::NAN; 6],
        "",
        &[],
        16,
        &[0.0; 10],
        &[0.0; 5],
        0.0,
        0,
        false,
        false,
        1,
        false,
        0,
    )
    .map_err(|_| format!("{src} が描けない"))
}

/// 頂点と三角形
fn mesh(src: &str, defs: &str, params: &[(&str, f64)]) -> (Vec<f32>, Vec<u32>) {
    let m = run(src, defs, params).unwrap();
    (m.positions(), m.indices())
}

/// 2 つの式が同じメッシュ（頂点がビットまで同じ・三角形が同じ）になること
fn same(a: &str, b: &str, defs: &str) {
    let (pa, ia) = mesh(a, defs, &[]);
    let (pb, ib) = mesh(b, "", &[]);
    assert!(!pa.is_empty(), "{a} が空");
    assert_eq!(pa.len(), pb.len(), "{a} と {b} の頂点の数");
    assert!(pa.iter().zip(&pb).all(|(x, y)| x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan())), "{a} と {b} の頂点");
    assert_eq!(ia, ib, "{a} と {b} の三角形");
}

/// エラーの文（plot のエラーは JsError なので、ネイティブのテストでは中身を読めない。同じ文を返す compile で確かめる）
fn error_of(src: &str, defs: &str) -> String {
    let _d = graph_core::expand::install(graph_core::expand::Defs::parse(defs, &Default::default()));
    match graph_core::equation::compile(src) {
        Ok(_) => String::new(),
        Err(e) => e,
    }
}

const FG: &str = "f(x) = x^2\ng(x) = sin(x)";

#[test]
fn function_calls_expand_to_the_same_mesh() {
    same("z = f(2x)", "z = (2x)^2", FG);
    same("z = g(f(x))", "z = sin(x^2)", FG);
    same("z = (f+g)(x)", "z = x^2 + sin(x)", FG);
    same("z = (f∘g)(y)", "z = sin(y)^2", FG);
    same("z = f(x) g(y)", "z = x^2 sin(y)", FG);
    same("z = f(x)^2 + 1", "z = (x^2)^2 + 1", FG);
}

#[test]
fn two_argument_functions() {
    same("z = h(y, x + 1)", "z = y (x + 1)", "h(x, y) = x y");
    // 定義の中で別の定義を使う
    same("z = k(x, y)", "z = (x^2) + y", "f(x) = x^2\nk(a, b) = f(a) + b");
}

#[test]
fn function_values() {
    // 引数の無い定義 h = f + g・q = f ∘ g と、sin(f)
    same("z = h(x)", "z = x^2 + sin(x)", "f(x) = x^2\ng(x) = sin(x)\nh = f + g");
    same("z = q(y)", "z = sin(y)^2", "f(x) = x^2\ng(x) = sin(x)\nq = f∘g");
    same("z = s(x)", "z = sin(x^2)", "f(x) = x^2\ns = sin(f)");
}

#[test]
fn parameter_names_stay_products() {
    // a がパラメータなら a(x) = 1 は今までどおり a·x = 1（陰関数）
    let (p1, i1) = mesh("a(x) = 1", "a(x) = 1", &[("a", 2.0)]);
    definitions("");
    let (p2, i2) = mesh("a x = 1", "", &[("a", 2.0)]);
    assert_eq!((p1, i1), (p2, i2));
}

#[test]
fn definition_line_is_drawn() {
    // 引数 1 つの定義の行は y = f(x)、2 つは z = f(x, y)
    same("f(x) = x^2", "y = x^2", "f(x) = x^2");
    same("h(x, y) = x y", "z = x y", "h(x, y) = x y");
    let m = run("f(x) = x^2", "f(x) = x^2", &[]).unwrap();
    assert_eq!(m.def_name(), "f");
    assert_eq!(m.kind(), "f の定義（y = f(x)）");
    // 値が組の関数は媒介変数表示
    same("c(s) = (cos(s), sin(s), s)", "(cos(t), sin(t), t)", "c(s) = (cos(s), sin(s), s)");
    // 引数 3 つの定義は描かない（エラーにもしない）
    let m = run("p(x, y, z) = x y z", "p(x, y, z) = x y z", &[]).unwrap();
    assert!(m.positions().is_empty());
    assert_eq!(m.kind(), "p の定義");
}

#[test]
fn used_definitions_are_reported() {
    let m = run("z = g(f(x)) + y", FG, &[]).unwrap();
    let mut used: Vec<String> = m.used_defs().split(',').map(String::from).collect();
    used.sort();
    assert_eq!(used, vec!["f", "g"]);
    let m = run("z = x y", FG, &[]).unwrap();
    assert_eq!(m.used_defs(), "");
}

#[test]
fn inverse_function() {
    same("y = f^(-1)(x)", "y^2 = x", "f(x) = x^2");
    assert!(error_of("z = f^(-1)(x) + 1", "f(x) = x^2").contains("逆関数"));
}

#[test]
fn errors_name_the_function() {
    assert!(error_of("z = f(x)", "f(x) = f(x) + 1").contains("f の定義が f 自身"), "{}", error_of("z = f(x)", "f(x) = f(x) + 1"));
    assert!(error_of("z = h(x)", "h(x, y) = x y").contains("h の引数は 2 個"));
    assert!(error_of("z = f + 1", "f(x) = x^2").contains("f は関数です"));
    // 定義が無ければ、今までどおり f·x の積として読まれ、未定義の文字の誤り（画面の「スライダーを追加」に使う文言）
    assert!(error_of("z = f(x)", "").contains("変数 'f'"));
}

#[test]
fn existing_expressions_do_not_change() {
    // 定義があっても、使っていない式は今までと同じ
    for src in ["z = x^2 - y^2", "x^2 + y^2 + z^2 = 4", "(cos t, sin t, t/4)", "z = (x+1)(x-1)"] {
        same(src, src, FG);
    }
}

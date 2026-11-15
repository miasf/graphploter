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

// ---- M4: Σ・Π・等差のリスト・± ----

/// パラメータの無い式の値
fn value(src: &str) -> f64 {
    graph_core::equation::eval_constant(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

#[test]
fn sums_and_products() {
    same("z = @sum(n,1,3) n x", "z = 1 x + 2 x + 3 x", "");
    same("z = @prod(k,1,3) (x-k)", "z = (x-1)(x-2)(x-3)", "");
    assert_eq!(value("@sum(n,1,2) @sum(m,1,n) m"), 4.0);
    assert_eq!(value("@sum(n,1,10) n"), 55.0);
    assert_eq!(value("@prod(n,1,5) n"), 120.0);
    // 範囲が逆なら 0（Π は 1）
    assert_eq!(value("@sum(n,3,1) n"), 0.0);
    assert_eq!(value("@prod(n,3,1) n"), 1.0);
    // 中身は直後の項
    assert_eq!(value("@sum(n,1,3) n + 1"), 7.0);
}

#[test]
fn sum_range_errors() {
    let e = error_of("z = @sum(n,1,x) n", "");
    assert!(e.contains("Σ の範囲に x は使えません"), "{e}");
    let mut ps = std::collections::BTreeMap::new();
    ps.insert("N".to_string(), num_complex::Complex64::new(2.5, 0.0));
    let e = graph_core::equation::compile_with("z = @sum(n,1,N) n", &ps, &graph_core::equation::Space::standard()).err().unwrap_or_default();
    assert!(e.contains("整数"), "{e}");
    assert!(error_of("z = @sum(n,1,5000) n x", "").contains("項が多すぎます"));
}

#[test]
fn sum_range_parameter_is_reported() {
    // 範囲だけに使ったパラメータ N も、使ったパラメータとして返す（スライダーを動かしたら描き直す）
    let m = run("z = @sum(n,1,N) sin(n x)/n", "", &[("N", 3.0)]).unwrap();
    assert!(m.used_params().split(',').any(|p| p == "N"), "{}", m.used_params());
    // 項の数がパラメータで変わる
    let (p3, _) = mesh("z = @sum(n,1,N) sin(n x)/n", "", &[("N", 3.0)]);
    let (p1, _) = mesh("z = @sum(n,1,N) sin(n x)/n", "", &[("N", 1.0)]);
    assert_ne!(p3, p1);
}

#[test]
fn sums_inside_functions() {
    same("z = s(x)", "z = 1 sin(1 x) / 1 + 2 sin(2 x) / 2", "s(x) = @sum(k,1,2) k sin(k x) / k");
}

#[test]
fn arithmetic_lists() {
    let a = mesh("z = [1, 3, …, 9] x", "", &[]);
    let b = mesh("z = [1, 3, 5, 7, 9] x", "", &[]);
    assert_eq!(a, b);
    assert_eq!(mesh("z = [1, …, 3] x", "", &[]), mesh("z = [1, 2, 3] x", "", &[]));
    assert_eq!(mesh("z = [1, ..., 3] x", "", &[]), mesh("z = [1, 2, 3] x", "", &[]));
    assert!(error_of("z = [1, 0, …, 3] x", "").contains("終わりません"));
}

#[test]
fn plus_minus() {
    assert_eq!(mesh("y = ±sqrt(x)", "", &[]), mesh("y = [1, -1] sqrt(x)", "", &[]));
    // 全部の ± はそろって動く（4 通りではなく 2 通り）
    let m = run("z = ±x ± y", "", &[]).unwrap();
    assert!(m.kind().ends_with("× 2"), "{}", m.kind());
    assert_eq!(mesh("z = x ∓ y", "", &[]).0.len(), mesh("z = [x - y, x + y]", "", &[]).0.len());
}

#[test]
fn latex_outputs_are_readable() {
    // web/src/lib/latex.test.ts の変換結果を、計算エンジンがそのまま読めること
    assert_eq!(value("@sum(n,1,10) n^(2)"), 385.0);
    let mut ps = std::collections::BTreeMap::new();
    ps.insert("N".to_string(), num_complex::Complex64::new(3.0, 0.0));
    let sp = graph_core::equation::Space::standard();
    assert!(graph_core::equation::compile_with("z=@sum(k,1,N) ((sin (kx))/(k))+1", &ps, &sp).is_ok());
    assert!(graph_core::equation::compile_with("z=@prod(k,1,5) (x-k)", &ps, &sp).is_ok());
    assert!(graph_core::equation::compile_with("y=± sqrt(x)", &ps, &sp).is_ok());
    assert!(graph_core::equation::compile_with("z=[1,3,…,9] x", &ps, &sp).is_ok());
    assert!((value("30*(pi/180)") - std::f64::consts::PI / 6.0).abs() < 1e-15);
    assert!((value("@int(u,-∞,∞)(((1)/(1+u^(2))))") - std::f64::consts::PI).abs() < 1e-6);
    assert!((value("@int(t,0,1)(t) +1") - 1.5).abs() < 1e-14);
    assert!(run("f(x)= @cases(x^(2), x<0; x)", "f(x)= @cases(x^(2), x<0; x)", &[]).is_ok());
    assert_eq!(value("@cases(1, 0<=0.5<1; 0, 0.5>=1; -1)"), 1.0);
}

// ---- M3: 記号微分 ----

/// 2 つの式の頂点の差の最大
fn max_diff(a: &str, b: &str, defs: &str) -> f64 {
    let (pa, ia) = mesh(a, defs, &[]);
    let (pb, ib) = mesh(b, "", &[]);
    assert_eq!(ia, ib, "{a} と {b} の三角形");
    pa.iter().zip(&pb).filter(|(x, y)| x.is_finite() && y.is_finite()).map(|(x, y)| (x - y).abs() as f64).fold(0.0, f64::max)
}

#[test]
fn derivatives_match_hand_written() {
    assert!(max_diff("z = @diff(x) sin(x y)", "z = y cos(x y)", "") < 1e-5);
    assert!(max_diff("y = @diff(x, 2) exp(-x^2)", "y = (4x^2 - 2) exp(-x^2)", "") < 1e-5);
    assert!(max_diff("z = @diff(x) @diff(y) sin(x y)", "z = cos(x y) - x y sin(x y)", "") < 1e-5);
    // 導関数 f'、f''
    assert!(max_diff("y = f'(x)", "y = 3x^2", "f(x) = x^3") < 1e-5);
    assert!(max_diff("y = f''(2x)", "y = 12x", "f(x) = x^3") < 1e-5);
    // df/dx の形
    assert!(max_diff("y = @diff(x) f", "y = cos(x)", "f(t) = sin(t)") < 1e-5);
}

#[test]
fn derivative_values() {
    // 差分に落ちる関数: ζ'(2) = −0.9375482543…
    let mut ps = std::collections::BTreeMap::new();
    ps.insert("s".to_string(), num_complex::Complex64::new(2.0, 0.0));
    let (d, _) = graph_core::equation::eval_constant_with("@diff(s) zeta(s)", &ps).unwrap();
    assert!((d + 0.937_548_254_3).abs() < 1e-6, "{d}");
    assert!(error_of("z = h'(x, y)", "h(x, y) = x y").contains("∂"));
}

// ---- M2: 場合分け ----

#[test]
fn piecewise_values() {
    let m = run("z = @cases(x^2, x < 0; x)", "", &[]).unwrap();
    let p = m.positions();
    let mut checked = 0;
    for v in p.chunks_exact(3) {
        let (x, z) = (v[0] as f64, v[2] as f64);
        if !x.is_finite() || !z.is_finite() || x.abs() < 0.2 {
            continue;
        }
        let want = if x < 0.0 { x * x } else { x };
        if want.abs() <= 3.0 {
            assert!((z - want).abs() < 1e-5, "x = {x}: {z} と {want}");
            checked += 1;
        }
    }
    assert!(checked > 100);
    // 選ばれなかった値が NaN でも、結果に影響しない
    assert_eq!(value("@cases(sqrt(0 - 4), 1 < 0; 7)"), 7.0);
    // どの場合も成り立たなければ描かない
    assert!(graph_core::equation::eval_constant("@cases(1, 2 < 1)").is_err());
    // 連鎖と、, で並べた条件（かつ）
    assert_eq!(value("@cases(1, 0 ≤ 0.5 < 1; 2)"), 1.0);
    assert_eq!(value("@cases(1, 0 ≤ 1 < 1; 2)"), 2.0);
    assert_eq!(value("@cases(1, 0 < 1, 2 > 3; 2)"), 2.0);
    assert_eq!(value("@cases(1, 2 = 2; 2)"), 1.0);
}

#[test]
fn piecewise_in_functions_and_derivatives() {
    // 定義の中の場合分けと、その微分（値だけを微分する）
    assert!(max_diff("y = f'(x)", "y = @cases(2x, x < 0; 1)", "f(x) = @cases(x^2, x < 0; x)") < 1e-5);
}

#[test]
fn piecewise_step_is_cut_like_sign() {
    // 段差の位置は sign(x) と同じように切り分けられる: z = 1 の面の、x = 0 に最も近い頂点が、格子の 1/1000 以内
    let nearest = |src: &str| {
        let m = run(src, "", &[]).unwrap();
        m.positions().chunks_exact(3).filter(|v| (v[2] - 1.0).abs() < 1e-6).map(|v| v[0] as f64).fold(f64::INFINITY, f64::min)
    };
    let h = 6.0 / 40.0;
    let (c, s) = (nearest("z = @cases(1, x > 0; -1)"), nearest("z = sign(x)"));
    assert!(c.abs() < h / 1000.0 + 1e-6, "場合分け {c}");
    assert!((c - s).abs() < 1e-6, "場合分け {c}, sign {s}");
}

// ---- M5: 定積分 ----

fn close(src: &str, want: f64, tol: f64) {
    let v = value(src);
    assert!((v - want).abs() <= tol, "{src} = {v}（{want} から {} 離れている）", (v - want).abs());
}

#[test]
fn integral_values() {
    close("@int(t,0,1)(t^2)", 1.0 / 3.0, 1e-14);
    close("@int(t,0,pi)(sin(t))", 2.0, 1e-13);
    close("@int(t,0,∞)(exp(-t))", 1.0, 1e-6);
    close("@int(t,-∞,∞)(exp(-t^2))", std::f64::consts::PI.sqrt(), 1e-6);
    close("@int(t,-∞,0)(exp(t))", 1.0, 1e-6);
    // 入れ子
    close("@int(s,0,1)(@int(t,0,s)(1))", 0.5, 1e-13);
    // 中身は直後の項。+1 は積分の外
    close("@int(t,0,1)(t) + 1", 1.5, 1e-14);
    assert!(error_of("z = @int(a,0,1)(@int(b,0,1)(@int(c,0,1)(x)))", "").contains("2 重まで"));
}

#[test]
fn integral_with_variable_end() {
    // ∫₀ˣ e^{−t²} dt = (√π/2) erf(x)
    let m = run("y = @int(t,0,x)(exp(-t^2))", "", &[]).unwrap();
    let mut n = 0;
    for v in m.positions().chunks_exact(3) {
        let (x, y) = (v[0] as f64, v[1] as f64);
        if x.is_finite() && y.is_finite() {
            let want = std::f64::consts::PI.sqrt() / 2.0 * libm_erf(x);
            assert!((y - want).abs() < 1e-5, "x = {x}: {y} と {want}");
            n += 1;
        }
    }
    assert!(n > 10);
}

/// erf（f32 の頂点と比べるので、計算エンジンの erf で十分）
fn libm_erf(x: f64) -> f64 {
    value(&format!("erf({x})"))
}

#[test]
fn integral_against_bessel() {
    // ∫₀^{2π} cos(x sin t − y t) dt = 2π J_y(x)（y が整数の所）
    for (x, y) in [(1.5, 0.0), (2.5, 1.0), (0.7, 2.0)] {
        let v = value(&format!("@int(t,0,2pi)(cos({x} sin(t) - {y} t))"));
        let j = value(&format!("2 pi besselJ({y}, {x})"));
        assert!((v - j).abs() < 1e-8, "x = {x}, y = {y}: {v} と {j}");
    }
}

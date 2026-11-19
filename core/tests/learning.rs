//! 大学数学の学習向けの記法（テイラー多項式・停留点の分類）。画面と同じ入口（`graph_core::plot` と `definitions`）で確かめる

use graph_core::{definitions, plot, MeshResult};

fn run(src: &str, defs: &str, params: &[(&str, f64)]) -> Result<MeshResult, String> {
    definitions(defs);
    let names: Vec<&str> = params.iter().map(|p| p.0).collect();
    let values: Vec<f64> = params.iter().flat_map(|p| [p.1, 0.0]).collect();
    plot(
        src, &[-3.0, 3.0, -3.0, 3.0, -3.0, 3.0], 40, 24, "t", &[0.0, 5.0], &names.join(","), &values, "", 3, &[0, 1, 2], &[0.0; 3],
        &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0,
    )
    .map_err(|_| format!("{src} が描けない"))
}

fn kind(src: &str, defs: &str) -> String {
    run(src, defs, &[]).unwrap().kind().to_string()
}

fn error_of(src: &str, defs: &str) -> String {
    let _d = graph_core::expand::install(graph_core::expand::Defs::parse(defs, &Default::default()));
    match graph_core::equation::compile(src) {
        Ok(_) => String::new(),
        Err(e) => e,
    }
}

fn value(src: &str) -> f64 {
    graph_core::equation::eval_constant(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

/// 頂点がビットまで同じ
fn same(a: &str, b: &str, defs: &str) {
    let pa = run(a, defs, &[]).unwrap().positions();
    let pb = run(b, "", &[]).unwrap().positions();
    assert!(!pa.is_empty(), "{a} が空");
    assert_eq!(pa.len(), pb.len(), "{a} と {b} の頂点の数");
    let worst = pa.iter().zip(&pb).map(|(x, y)| (x - y).abs()).fold(0.0f32, f32::max);
    assert!(worst < 1e-5, "{a} と {b} の頂点のずれ {worst}");
}

#[test]
fn taylor_one_variable() {
    assert!(kind("y = @taylor(sin(x), x, 0, 5)", "").contains("T₅(x) = x − x³/6 + x⁵/120"), "{}", kind("y = @taylor(sin(x), x, 0, 5)", ""));
    assert!(kind("y = @taylor(e^(x), x, 1, 2)", "").contains("T₂(x) = 2.718282 + 2.718282(x − 1) + 1.359141(x − 1)²"), "{}", kind("y = @taylor(e^(x), x, 1, 2)", ""));
    assert!(kind("y = @taylor(ln(x), x, 1, 3)", "").contains("T₃(x) = (x − 1) − (x − 1)²/2 + (x − 1)³/3"), "{}", kind("y = @taylor(ln(x), x, 1, 3)", ""));
    // 値: cos の 6 次の多項式の 1 での値
    let want = 1.0 - 0.5 + 1.0 / 24.0 - 1.0 / 720.0;
    assert!((value("@at(@taylor(cos(x), x, 0, 6), x, 1)") - want).abs() < 1e-12);
    // 関数の名前だけでもよい
    assert!(kind("y = @taylor(f, x, 0, 3)", "f(x) = 1/(1-x)").contains("T₃(x) = 1 + x + x² + x³"));
    // 0 次は定数
    assert!(kind("y = @taylor(cos(x), x, 0, 0)", "").contains("T₀(x) = 1"));
}

#[test]
fn taylor_degree_from_slider() {
    let a = run("y = @taylor(sin(x), x, 0, N)", "", &[("N", 3.0)]).unwrap();
    assert!(a.kind().contains("T₃(x) = x − x³/6"), "{}", a.kind());
    assert!(a.used_params().contains(&"N".to_string()));
    assert!(error_of("y = @taylor(sin(x), x, 0, 1.5)", "").contains("整数"));
    assert!(error_of("y = @taylor(sin(x), x, 0, 30)", "").contains("0〜20"));
}

#[test]
fn taylor_several_variables() {
    // 接平面: f(x, y) = x² + y² の (1, 1) での 1 次
    same("z = @taylor(x^(2)+y^(2), (x,y), (1,1), 1)", "z = 2 + 2(x-1) + 2(y-1)", "");
    assert!(kind("z = @taylor(x^(2)+y^(2), (x,y), (1,1), 1)", "").contains("T₁(x, y) = 2 + 2(x − 1) + 2(y − 1)"));
    // 2 次は元の多項式そのもの
    same("z = @taylor(x y + y^(2), (x,y), (1,-1), 2)", "z = x y + y^(2)", "");
    let k = kind("z = @taylor(e^(x) cos(y), (x,y), (0,0), 2)", "");
    assert!(k.contains("T₂(x, y) = 1 + x + x²/2 − y²/2"), "{k}");
    assert!(error_of("z = @taylor(x y, (x,y), 1, 2)", "").contains("組"));
}

#[test]
fn taylor_keeps_other_variables() {
    // 係数に別の変数が残る（x について展開した sin(x y)）。説明は出さず、式として描く
    same("z = @taylor(sin(x y), x, 0, 3)", "z = y x - y^(3) x^(3) / 6", "");
}

#[test]
fn critical_points_two_variables() {
    // f = x³ − 3x + y²: (1, 0) は極小、(−1, 0) は鞍点
    let m = run("@crit(x^(3)-3x+y^(2))", "", &[]).unwrap();
    let k = m.kind().to_string();
    assert!(k.contains("2 個"), "{k}");
    assert!(k.contains("(−1, 0): 鞍点 f = 2（det H = −12 < 0）"), "{k}");
    assert!(k.contains("(1, 0): 極小 f = −2（det H = 12 > 0, ∂²f/∂x² = 6 > 0）"), "{k}");
    // 点は曲面の上 (x, y, f)
    let p = m.positions();
    assert_eq!(p.is_empty(), false);
    // sin x cos y の、表示範囲 [−3, 3]² の中の停留点: (±π/2, 0) は極値、(0, ±π/2) は鞍点。ほかに (±π/2, ±π)? 範囲外。全部で 5 個（原点は鞍点でなく... 確かめる）
    let k = kind("@crit(sin(x) cos(y))", "");
    assert!(k.contains("(−1.570796, 0): 極小 f = −1"), "{k}");
    assert!(k.contains("(1.570796, 0): 極大 f = 1"), "{k}");
    assert!(k.contains("(0, −1.570796): 鞍点"), "{k}");
    assert!(k.contains("(0, 1.570796): 鞍点"), "{k}");
    assert!(k.contains("4 個"), "{k}");
}

#[test]
fn critical_points_by_function_name() {
    let k = kind("@crit f", "f(x,y) = x^(2) + x y + y^(2) - 3x");
    assert!(k.contains("(2, −1): 極小 f = −3"), "{k}");
}

#[test]
fn critical_points_one_variable_higher_derivatives() {
    let k = kind("@crit(x^(3))", "");
    assert!(k.contains("x = 0: 極値でない f = 0（f″ = 0, f‴ = 6 ≠ 0。奇数階なので極値でない）"), "{k}");
    let k = kind("@crit(x^(4) - 1)", "");
    assert!(k.contains("x = 0: 極小 f = −1（f″ = f‴ = 0, f⁽⁴⁾ = 24 > 0）"), "{k}");
    let k = kind("@crit(x^(3) - 3x)", "");
    assert!(k.contains("x = −1: 極大 f = 2（f″ = −6 < 0）") && k.contains("x = 1: 極小 f = −2（f″ = 6 > 0）"), "{k}");
}

#[test]
fn critical_points_three_variables_and_degenerate() {
    let k = kind("@crit(x^(2) + 2y^(2) + 3z^(2) - x y)", "");
    assert!(k.contains("(0, 0, 0): 極小 f = 0（主小行列式"), "{k}");
    let k = kind("@crit(x^(2) - y^(2) + z^(2))", "");
    assert!(k.contains("(0, 0, 0): 鞍点"), "{k}");
    let k = kind("@crit(x^(4) + y^(4))", "");
    assert!(k.contains("(0, 0): 判定できない"), "{k}");
    // 線に沿って続く停留点 (x − y)²
    let k = kind("@crit((x - y)^(2))", "");
    assert!(k.contains("線・面に沿って続いている可能性"), "{k}");
    // 停留点が無い
    let k = kind("@crit(x + y)", "");
    assert!(k.contains("0 個"), "{k}");
    assert!(error_of("@crit((x, y))", "").contains("ベクトル"));
    assert!(error_of("@crit(5)", "").contains("座標"));
}

#[test]
#[ignore]
fn critical_speed() {
    for src in ["@crit(sin(x) cos(y))", "@crit(sin(3x) cos(3y) + x/5)", "@crit(sin(x) sin(y) sin(z))", "@crit(gamma(x))", "@crit(besselJ(0, x) + y^(2))"] {
        let t = std::time::Instant::now();
        let r = run(src, "", &[]);
        println!("{src}: {:?} {}", t.elapsed(), r.map(|m| m.kind().to_string()).unwrap_or_else(|e| e));
    }
}

#[test]
#[ignore]
fn taylor_speed() {
    for src in ["y = @taylor(tan(x), x, 0, 20)", "y = @taylor(1/(1+x^(2)), x, 0, 20)", "y = @taylor(gamma(x+2), x, 0, 8)", "z = @taylor(e^(x) cos(y), (x,y), (0,0), 10)", "z = @taylor(sin(x y), (x,y), (1,1), 10)", "y = @taylor(sin(x+z), (x,z), (0,0), 6)"] {
        let t = std::time::Instant::now();
        let e = error_of(src, "");
        if !e.is_empty() {
            println!("{src}: {:?} エラー {e}", t.elapsed());
            continue;
        }
        let r = run(src, "", &[]);
        println!("{src}: {:?} {}", t.elapsed(), r.map(|m| m.kind().chars().take(120).collect::<String>()).unwrap_or_else(|e| e));
    }
}

#[test]
fn taylor_high_degree_by_series() {
    // tan x の 9 次の係数は 62/2835
    let k = kind("y = @taylor(tan(x), x, 0, 20)", "");
    assert!(k.contains("62x⁹/2835"), "{k}");
    // 1/(1 + x²) = 1 − x² + x⁴ − …
    let k = kind("y = @taylor(1/(1+x^(2)), x, 0, 20)", "");
    assert!(k.contains("T₂₀(x) = 1 − x² + x⁴ − x⁶ + x⁸"), "{k}");
    // arctan x = x − x³/3 + x⁵/5
    assert!(kind("y = @taylor(arctan(x), x, 0, 5)", "").contains("T₅(x) = x − x³/3 + x⁵/5"));
    // √x の 1 のまわり
    assert!(kind("y = @taylor(sqrt(x), x, 1, 2)", "").contains("T₂(x) = 1 + (x − 1)/2 − (x − 1)²/8"));
    // x^x（指数が変数）: 1 のまわり 1 + (x−1) + (x−1)²
    assert!(kind("y = @taylor(x^(x), x, 1, 2)", "").contains("T₂(x) = 1 + (x − 1) + (x − 1)²"));
    // 特殊関数は記号微分の道（説明は出る）
    assert!(kind("y = @taylor(gamma(x+2), x, 0, 3)", "").contains("T₃(x) = 1 + 0.42278"));
    // 値が複素数の係数（e^(i x)）も描ける
    let v = graph_core::equation::eval_constant("Re(@at(@taylor(e^(i x), x, 0, 12), x, 1))").unwrap();
    assert!((v - 1f64.cos()).abs() < 1e-8, "{v}");
}

#[test]
fn taylor_small_coefficients_are_readable() {
    let k = kind("y = @taylor(sin(x), x, 0, 13)", "");
    assert!(k.contains("x¹¹/39916800 + x¹³/6227020800"), "{k}");
    let k = kind("y = @taylor(e^(x/1000), x, 0, 3)", "");
    assert!(k.contains("x³/6000000000"), "{k}");
    let k = kind("y = @taylor(e^(x/(3π)), x, 0, 3)", "");
    assert!(k.contains("×10⁻⁴x³"), "{k}");
}

//! 数学の記法（計画 `docs/plan-math-notation.md`）。画面と同じ入口（`graph_core::plot` と `definitions`）で確かめる

use graph_core::{definitions, ode_init, plot, MeshResult};

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
    // 不定積分・重積分（latex.test.ts の変換結果）
    assert!(graph_core::equation::compile_with("y= @int(x)(x^(2))", &ps, &sp).is_ok());
    assert!((value("@int(x,0,1)(@int(y,0,x)(xy))") - 0.125).abs() < 1e-12);
    assert!((value("@intset((x,y),0<=x<=1,0<=y<=x)(xy)") - 0.125).abs() < 1e-12);
    assert!((value("@intset((x,y),0<=x<=1,0<=y<=2)(xy)") - 1.0).abs() < 1e-12);
    assert!((value("@intset((x,y,z),0<=x<=1,0<=y<=1,0<=z<=1)(1)") - 1.0).abs() < 1e-12);
    // ベクトル場・線積分（latex.test.ts の変換結果）
    for src in ["z= @div (x^(2),y^(2),0)", "z= @rot (-y,x)", "z= @lap x^(2)y", "z= @lap (x^(2)y)"] {
        assert!(graph_core::equation::compile_with(src, &ps, &sp).is_ok(), "{src}");
    }
    assert_eq!(value("(1,2,3)×(4,5,6)(1,0,0)"), -3.0);
    assert!((value("@intset(( cos t, sin t),0<=t<=2 pi; {vec})((-y,x))") - 2.0 * std::f64::consts::PI).abs() < 1e-9);
    assert!((value("@intset(( cos t, sin t),0<=t<=2 pi)(x^(2))") - std::f64::consts::PI).abs() < 1e-9);
    // 集合と論理（latex.test.ts の変換結果）
    for src in ["⦃(x,y,z)∣x^(2)+y^(2)<1,0<=z<=2⦄", "x^(2)+y^(2)+z^(2)<=4∧¬(z>1)", "x<0⊕y<0", "x<0⊼y<0", "x<0⊽y<0", "x∈[0,1)", "x∈(0,1]", "(x,y,z)∈[0,1]×[0,2]×[0,1]", "x∈ℤ", "x∉ℕ", "x≠0", "∅∨⊤"] {
        assert!(run(src, "", &[]).is_ok(), "{src}");
    }
    let defs = "A= ⦃(x,y,z)∣x<0⦄\nB= ⦃(x,y,z)∣y<0⦄\nC= ⦃(x,y,z)∣z<0⦄";
    for src in ["A∪B∖C", "A△B", "A^(c)∩Bᶜ"] {
        assert!(run(src, defs, &[]).is_ok(), "{src}");
    }
    // 集合の上の総和・⋃（latex.test.ts の変換結果）
    assert_eq!(value("@sumset(n; n∈⦃1,4,9⦄) n"), 14.0);
    assert_eq!(value("@sumset(n; 1<=n<=10, mod (n,2)=1) n"), 25.0);
    assert_eq!(value("@sumset(d; d∣12) d"), 28.0);
    assert_eq!(value("@prodset((m,n); (m,n)∈[1,3]×[1,3]) mn"), 46656.0);
    for src in ["@bigcup(k; 1<=k<=3) ⦃(x,y)∣x<k⦄", "x∈⦃1,2,3⦄"] {
        assert!(run(src, "", &[]).is_ok(), "{src}");
    }
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
    assert!(error_of("z = @int(a,0,1)(@int(b,0,1)(@int(c,0,1)(@int(d,0,1)(x))))", "").contains("3 重まで"));
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

// ---- 差分（総和の逆） ----

#[test]
fn differences() {
    // 幅 1: Δx² = 2x + 1、∇x² = 2x − 1、δx² = 2x
    assert!(max_diff("y = @fdiff(x) x^2", "y = 2x + 1", "") < 1e-4);
    assert!(max_diff("y = @bdiff(x) x^2", "y = 2x - 1", "") < 1e-4);
    assert!(max_diff("y = @cdiff(x) x^2", "y = 2x", "") < 1e-4);
    // 幅を変える: Δ_{0.5} x² = x + 0.25
    assert!(max_diff("y = @fdiff(x, 0.5) x^2", "y = x + 0.25", "") < 1e-4);
    // 幅に変数: h = x/10 なら (1.1x)² − x² = 0.21x²
    assert!(max_diff("y = @fdiff(x, x/10) x^2", "y = 0.21 x^2", "") < 1e-4);
    // 2 階: Δ²x³ = 6x + 6
    assert!(max_diff("y = @fdiff(x, 1, 2) x^3", "y = 6x + 6", "") < 1e-3);
    // 関数の名前だけ
    assert!(max_diff("y = @fdiff(x) f", "y = 2x + 1", "f(t) = t^2") < 1e-4);
    // 2 変数の式で、y の差分
    assert!(max_diff("z = @fdiff(y) x y^2", "z = x (2y + 1)", "") < 1e-3);
}

#[test]
fn differences_and_sums_are_inverse() {
    // Σ_{n=0}^{N−1} Δf(n) = f(N) − f(0)（和分と差分）
    assert_eq!(value("@sum(n,0,4) @fdiff(n) n^2"), 25.0);
    assert_eq!(value("@sum(n,1,5) @bdiff(n) n^3"), 125.0);
    // 幅はパラメータでもよく、使ったパラメータとして返す
    let m = run("y = @fdiff(x, h) sin(x)", "", &[("h", 0.5)]).unwrap();
    assert!(m.used_params().split(',').any(|p| p == "h"));
    assert!(error_of("y = @fdiff(x, 1, 9) x", "").contains("0〜8"));
}

#[test]
fn derivatives_inside_definitions_and_sums() {
    // 定義の中の微分・差分（仮引数の名前を置き換えるときに、微分の変数も一緒に変わる）
    assert!(max_diff("y = g(2x)", "y = 3 (2x)^2", "g(t) = @diff(t) t^3") < 1e-3);
    assert!(max_diff("y = q(x)", "y = 2x + 1", "q(t) = @fdiff(t) t^2") < 1e-4);
    // Σ・積分の中の微分
    assert_eq!(value("@sum(n,1,3) @diff(n) n^2"), 12.0);
    assert!((value("@int(t,0,1)(@diff(t) t^3)") - 1.0).abs() < 1e-12);
}

// ---- 不定積分・重積分 ----

#[test]
fn indefinite_integrals() {
    // 表で扱える形は記号で（積分定数を省いた形）
    assert!(max_diff("y = @int(x)(x^2)", "y = x^3/3", "") < 1e-4);
    assert!(max_diff("y = @int(x)(cos(2x + 1))", "y = sin(2x + 1)/2", "") < 1e-4);
    assert!(max_diff("y = @int(x)(1/x)", "y = ln(abs(x))", "") < 1e-4);
    assert!(max_diff("y = @int(x)(f)", "y = x^3/3", "f(t) = t^2") < 1e-4);
    // 2 変数の式の x での不定積分（y は定数として扱う）
    assert!(max_diff("z = @int(x)(x y)", "z = x^2/2 y", "") < 1e-3);
    // 表に無い形は数値で ∫₀ˣ（F(0) = 0）: ∫ e^{−x²} dx = (√π/2) erf(x)
    assert!(max_diff("y = @int(x)(exp(-x^2))", "y = sqrt(pi)/2 erf(x)", "") < 1e-5);
    // 微分すると元に戻る
    assert!(max_diff("y = @diff(x) @int(x)(x sin(x))", "y = x sin(x)", "") < 1e-4);
}

#[test]
fn region_integrals() {
    // 範囲の形の条件は累次積分（順番は自動）: 三角形 0 ≤ x ≤ 1, 0 ≤ y ≤ x の面積 1/2
    close("@intset((x, y), 0 ≤ x ≤ 1, 0 ≤ y ≤ x)(1)", 0.5, 1e-12);
    // 条件の書く順番・向きによらない
    close("@intset((x, y), y ≤ x, x ≤ 1, y ≥ 0, 0 ≤ x)(1)", 0.5, 1e-12);
    // 累次積分の形に書けない領域: 境目を数値で探す
    let pi = std::f64::consts::PI;
    close("@intset((x, y), x^2 + y^2 ≤ 1)(1)", pi, 1e-6);
    close("@intset((x, y), x^2 + y^2 ≤ 4)(x^2 + y^2)", 8.0 * pi, 1e-5);
    close("@intset((x, y), (x - 100)^2 + (y + 50)^2 ≤ 1)(1)", pi, 1e-6);
    close("@intset((x, y), x^2 ≤ y ≤ x + 2)(1)", 4.5, 1e-6);
    // 穴の端・角（切り口の長さが折れる所）で分ける
    close("@intset((x, y), 1 ≤ x^2 + y^2 ≤ 4)(1)", 3.0 * pi, 1e-6);
    close("@intset((x, y), abs(x) + abs(y) ≤ 1)(1)", 2.0, 1e-6);
    close("@intset((x, y), (x^2 + y^2)^2 ≤ 2 (x^2 - y^2))(1)", 2.0, 1e-6);
    close("@intset((x, y, z), 1 ≤ x^2 + y^2 + z^2 ≤ 4)(1)", 28.0 * pi / 3.0, 1e-4);
    // 下の端 > 上の端になる所（sin x < 0）は空
    close("@intset((x, y), 0 ≤ y ≤ sin(x), 0 ≤ x ≤ 2 pi)(1)", 2.0, 2e-2);
    close("@intset((x, y), x^2 / 9 + y^2 / 4 ≤ 1, y ≥ 0)(y)", 8.0, 1e-5);
    close("@intset((x, y, z), x^2 + y^2 + z^2 ≤ 1)(1)", 4.0 * pi / 3.0, 1e-4);
    close("@intset((x, y, z), x^2 + y^2 ≤ z ≤ 1)(1)", pi / 2.0, 1e-4);
    close("@intset((x, y), -1 ≤ x ≤ 1, -1 ≤ y ≤ 1, x^2 + y^2 ≤ 1)(1)", pi, 1e-6);
    assert!(graph_core::equation::eval_constant("@intset((x, y), y ≥ x^2)(1)").unwrap_err().contains("どこまでも続いて"));
    // 外側の変数の端が、内側の変数の範囲から決まる（1 ≤ x ≤ y なら 1 ≤ y）
    close("@intset((x, y), 1 ≤ x ≤ y, y ≤ 3)(1)", 2.0, 1e-12);
    close("@intset((x, y), 1 ≤ x ≤ y ≤ 3)(x y)", 8.0, 1e-10);
    close("@intset((x, y, z), 0 ≤ x ≤ y ≤ z ≤ 1)(1)", 1.0 / 6.0, 1e-12);
    // 長方形の上の x y: ∫₀¹∫₀² x y dy dx = 1
    close("@intset((x, y), 0 ≤ x ≤ 1, 0 ≤ y ≤ 2)(x y)", 1.0, 1e-12);
    // 円の面積（範囲の形で書いたもの）: 端で √ の傾きが無限大なので、精度は 10⁻³ ほど
    close("@intset((x, y), -1 ≤ x ≤ 1, -sqrt(1 - x^2) ≤ y ≤ sqrt(1 - x^2))(1)", std::f64::consts::PI, 2e-3);
    // 3 重: 四面体の体積 1/6、立方体の上の x y z = 1/8
    close("@intset((x, y, z), 0 ≤ x ≤ 1, 0 ≤ y ≤ 1 - x, 0 ≤ z ≤ 1 - x - y)(1)", 1.0 / 6.0, 1e-12);
    close("@intset((x, y, z), 0 ≤ x ≤ 1, 0 ≤ y ≤ 1, 0 ≤ z ≤ 1)(x y z)", 0.125, 1e-12);
    // 範囲の形にならない条件は、条件の外で 0 にした近似（円の面積 π を 5 % 以内）
    close("@intset((x, y), -1 ≤ x ≤ 1, -1 ≤ y ≤ 1, x^2 + y^2 ≤ 1)(1)", std::f64::consts::PI, 0.05 * std::f64::consts::PI);
    // 1 次式の範囲（平行四辺形）は、変数変換（ヤコビアン 1/|det|）で累次積分にする: −1 ≤ x + y ≤ 1, 0 ≤ x − y ≤ 1 の面積は 2 · 1 / 2 = 1
    close("@intset((x, y), -1 ≤ x + y ≤ 1, 0 ≤ x - y ≤ 1)(1)", 1.0, 1e-12);
    // 中身も変換する: u = x + y, v = x − y で ∫∫ u² v / 2 du dv = (2/3)(1/2)/2 = 1/6
    close("@intset((x, y), -1 ≤ x + y ≤ 1, 0 ≤ x - y ≤ 1)((x + y)^2 (x - y))", 1.0 / 6.0, 1e-12);
    close("@intset((x, y), -1 ≤ x + y ≤ 1, 0 ≤ x - y ≤ 1)(x)", 0.25, 1e-12);
    // 片方がふつうの範囲でもよい・定数項つき・3 変数
    close("@intset((x, y), 0 ≤ x ≤ 1, 0 ≤ 2x + y - 1 ≤ 3)(1)", 3.0, 1e-12);
    close("@intset((x, y, z), 0 ≤ x + y ≤ 1, 0 ≤ y + z ≤ 1, 0 ≤ z + x ≤ 1)(1)", 0.5, 1e-12);
    // 残りの条件は、変換した後で近似（ひし形 |x| + |y| ≤ 1 の x ≥ 0 の半分。面積 1）
    close("@intset((x, y), -1 ≤ x + y ≤ 1, -1 ≤ x - y ≤ 1, x ≥ 0)(1)", 1.0, 0.05);
    // 独立でない 1 次式では変換できない
    assert!(error_of("z = @intset((s, t), 0 ≤ s + t ≤ 1, 0 ≤ 2s + 2t ≤ 1)(1)", "").contains("範囲"));
    // 範囲が書かれていない変数があれば誤り
    assert!(error_of("z = @intset((s, t), 0 ≤ s ≤ 1, s + t ≤ 1)(1)", "").contains("範囲"));
}

#[test]
fn region_integrals_with_outer_variables() {
    // 外の変数を範囲に使う: ∬_{0≤s≤x, 0≤t≤y} 1 = x y（x, y > 0）
    let m = run("z = @intset((s, t), 0 ≤ s ≤ x, 0 ≤ t ≤ y)(1)", "", &[]).unwrap();
    let mut n = 0;
    for v in m.positions().chunks_exact(3) {
        let (x, y, z) = (v[0] as f64, v[1] as f64, v[2] as f64);
        if x.is_finite() && x > 0.0 && y > 0.0 && z.is_finite() {
            assert!((z - x * y).abs() < 1e-4, "({x}, {y}): {z}");
            n += 1;
        }
    }
    assert!(n > 50);
    // 近似を使ったら説明に出す
    // （領域が外の変数で変わるとき。変わらなければ、境目を数値で探すので近似ではない）
    let m = run("z = @intset((s, t), -1 ≤ s ≤ 1, -1 ≤ t ≤ 1, s^2 + t^2 ≤ 1 + x^2)(1)", "", &[]).unwrap();
    assert!(m.kind().contains("積分は近似"), "{}", m.kind());
    let m = run("z = @intset((s, t), -1 ≤ s ≤ 1, -1 ≤ t ≤ 1, s^2 + t^2 ≤ 1)(1) + x", "", &[]).unwrap();
    assert!(!m.kind().contains("積分は近似"), "{}", m.kind());
}

// ---- ベクトル・grad・div・rot・線積分・面積分・体積分 ----

#[test]
fn vector_algebra() {
    assert_eq!(value("(1,2,3)(4,5,6)"), 32.0);
    assert_eq!(value("abs((3,4))"), 5.0);
    assert!((value("abs((1,2,3)×(4,5,6))") - 54f64.sqrt()).abs() < 1e-12);
    assert_eq!(value("(1,2)×(3,4)"), -2.0);
    assert_eq!(value("((1,2,3) + (1,1,1))(1,0,0)"), 2.0);
    assert_eq!(value("(2 (1,2,3))(0,0,1)"), 6.0);
    // 直線 (1, 2, 3) + t (0, 1, 1)
    assert_eq!(mesh("(1, 2, 3) + t (0, 1, 1)", "", &[]), mesh("(1, 2 + t, 3 + t)", "", &[]));
    assert!(error_of("z = (1,2) + x", "").contains("掛け算・割り算"));
    assert!(error_of("z = sin((x, y))", "").contains("ベクトル"));
}

#[test]
fn grad_div_rot() {
    assert!(max_diff("z = @div((x^2, y^2, 0))", "z = 2x + 2y", "") < 1e-4);
    assert!(max_diff("z = (@grad(x^2 + y^2))(1, 1, 0)", "z = 2x + 2y", "") < 1e-4);
    assert!(max_diff("z = @rot((-y, x)) + 0 x", "z = 2 + 0 x", "") < 1e-6);
    assert!(max_diff("z = abs(@rot((-y, x, 0))) + 0 x", "z = 2 + 0 x", "") < 1e-6);
    assert!(max_diff("z = @lap(x^2 y)", "z = 2y", "") < 1e-4);
    // div rot = 0、rot grad = 0
    assert!(max_diff("z = @div(@rot((x y, y z, z x))) + x", "z = x", "") < 1e-6);
    assert!(max_diff("z = abs(@rot(@grad(sin(x y) + x^3))) + x", "z = x", "") < 1e-6);
    // 定義したベクトル場・関数の名前に
    assert!(max_diff("z = @div F", "z = 2x + 1", "F(x, y, z) = (x^2, y, 0)") < 1e-4);
    assert!(max_diff("z = (@grad f)(1, 0, 0)", "z = 2x", "f(x, y, z) = x^2 + y") < 1e-4);
}

#[test]
fn line_surface_volume_integrals() {
    use std::f64::consts::PI;
    // 円周の長さ 2π、らせんの長さ 2π√2
    close("@intset((cos(t), sin(t)), 0 ≤ t ≤ 2pi)(1)", 2.0 * PI, 1e-9);
    close("@intset((cos(t), sin(t), t), 0 ≤ t ≤ 2pi)(1)", 2.0 * PI * 2f64.sqrt(), 1e-9);
    // スカラーの線積分 ∫_C x² ds（単位円）= π
    close("@intset((cos(t), sin(t)), 0 ≤ t ≤ 2pi)(x^2)", PI, 1e-9);
    // 仕事 ∮ (−y, x) · dr = 2π
    close("@intset((cos(t), sin(t)), 0 ≤ t ≤ 2pi; {vec})((-y, x))", 2.0 * PI, 1e-9);
    // 球面の面積 4π、流束 ∯ (x, y, z) · dS = 4π（外向き）
    let sphere = "(sin(u) cos(v), sin(u) sin(v), cos(u)), 0 ≤ u ≤ pi, 0 ≤ v ≤ 2pi";
    close(&format!("@intset({sphere})(1)"), 4.0 * PI, 1e-6);
    close(&format!("@intset({sphere}; {{vec}})((x, y, z))"), 4.0 * PI, 1e-6);
    // 流束 ∯ (x³, y³, z³) · dS = 12π/5（ガウスの発散定理で ∭ div F dV と同じ値）
    close(&format!("@intset({sphere}; {{vec}})((x^3, y^3, z^3))"), 12.0 * PI / 5.0, 1e-4);
    // 体積の側（立方体 [0, 1]³ の上の ∭ div F = 3）
    close("@intset((x, y, z), 0 ≤ x ≤ 1, 0 ≤ y ≤ 1, 0 ≤ z ≤ 1)(@div((x^3, y^3, z^3)))", 3.0, 1e-12);
    // 球の体積（球座標で）4π/3
    close("@intset((p sin(u) cos(v), p sin(u) sin(v), p cos(u)), 0 ≤ p ≤ 1, 0 ≤ u ≤ pi, 0 ≤ v ≤ 2pi)(1)", 4.0 * PI / 3.0, 1e-6);
    // 平面の領域の変数変換（極座標）: 円の面積 π
    close("@intset((r cos(t), r sin(t)), 0 ≤ r ≤ 1, 0 ≤ t ≤ 2pi)(1)", PI, 1e-9);
    assert!(error_of("z = @intset((cos(t), sin(t)), 0 ≤ t ≤ 1; {vec})(x)", "").contains("ベクトル場"));
}

// ---- M11: 集合と論理 ----

#[test]
fn set_builder_and_logic_match_systems() {
    assert_eq!(mesh("⦃(x, y, z) ∣ x^2+y^2<1, 0≤z≤2⦄", "", &[]), mesh("x^2+y^2<1, 0≤z≤2", "", &[]));
    assert_eq!(mesh("x^2+y^2+z^2 ≤ 4 ∧ ¬(z > 1)", "", &[]), mesh("x^2+y^2+z^2 ≤ 4, z ≤ 1", "", &[]));
    assert_eq!(mesh("(x, y, z) ∈ [0, 1] × [0, 2] × [0, 1]", "", &[]), mesh("0 ≤ x ≤ 1, 0 ≤ y ≤ 2, 0 ≤ z ≤ 1", "", &[]));
    assert_eq!(mesh("x ∈ ℤ", "", &[]), mesh("sin(pi x) = 0", "", &[]));
    // 点の名前は自由（(a, b, c) を表示中の座標に置き換える）
    assert_eq!(mesh("⦃(a, b, c) ∣ a^2+b^2+c^2 ≤ 4⦄", "", &[]), mesh("x^2+y^2+z^2 ≤ 4", "", &[]));
}

#[test]
fn named_sets_and_difference() {
    let defs = "A = ⦃(x,y,z) ∣ x^2+y^2+z^2 ≤ 4⦄\nB = ⦃(x,y,z) ∣ z > 0⦄";
    assert_eq!(mesh("A ∖ B", defs, &[]), mesh("x^2+y^2+z^2 ≤ 4, z ≤ 0", "", &[]));
    assert_eq!(mesh("A ∩ Bᶜ", defs, &[]), mesh("x^2+y^2+z^2 ≤ 4, z ≤ 0", "", &[]));
    assert_eq!(mesh("(x, y, z) ∈ A ∖ B", defs, &[]), mesh("x^2+y^2+z^2 ≤ 4, z ≤ 0", "", &[]));
    // 定義の行そのものも描く
    let m = run("A = ⦃(x,y,z) ∣ x^2+y^2+z^2 ≤ 4⦄", defs, &[]).unwrap();
    assert_eq!(m.def_name(), "A");
    assert!(!m.positions().is_empty());
    // 使った集合の名前を返す（定義を変えたら描き直す）
    let m = run("A ∖ B", defs, &[]).unwrap();
    let mut used: Vec<String> = m.used_defs().split(',').map(String::from).collect();
    used.sort();
    assert_eq!(used, vec!["A", "B"]);
    assert!(error_of("C ∪ A", defs).contains("集合 C は定義されていません"));
}

#[test]
fn unions_of_regions() {
    // 2 つの球の和集合: 面の頂点は、もう一方の球の内側（中心から 1 − 2h より内）に無い
    let m = run("x^2+y^2+z^2 ≤ 1 ∨ (x-1)^2+y^2+z^2 ≤ 1", "", &[]).unwrap();
    let p = m.positions();
    assert!(p.len() > 300);
    let h = 6.0 / 24.0;
    for v in p.chunks_exact(3) {
        let (x, y, z) = (v[0] as f64, v[1] as f64, v[2] as f64);
        if !x.is_finite() {
            continue;
        }
        let r0 = (x * x + y * y + z * z).sqrt();
        let r1 = ((x - 1.0) * (x - 1.0) + y * y + z * z).sqrt();
        assert!(r0 > 1.0 - 2.0 * h || r1 > 1.0 - 2.0 * h, "({x}, {y}, {z}) が内側");
    }
    // XOR（対称差）・NAND・NOR も描ける
    for src in ["x^2+y^2+z^2 ≤ 4 ⊕ z ≤ 0", "x < 0 ⊼ y < 0", "x < 0 ⊽ y < 0", "(x < 0) ⇒ (y < 0)", "(x < 0) ⇔ (y < 0)"] {
        assert!(!mesh(src, "", &[]).0.is_empty(), "{src}");
    }
    // 等式どうしの「または」は、両方を重ねて描く
    let m = run("x^2 + y^2 = 1 ∨ z = 0", "", &[]).unwrap();
    assert!(m.kind().contains("× 2"), "{}", m.kind());
}

#[test]
fn not_equal_empty_and_parametric_sets() {
    let m = run("x ≠ 0, x^2+y^2+z^2 ≤ 1", "", &[]).unwrap();
    assert!(m.kind().contains("≠"), "{}", m.kind());
    assert_eq!(m.positions(), mesh("x^2+y^2+z^2 ≤ 1", "", &[]).0);
    let m = run("⊥ ∨ ∅", "", &[]).unwrap();
    assert!(m.positions().is_empty() && m.kind().contains("空集合"), "{}", m.kind());
    // 媒介変数で書いた集合: 条件が範囲になる（画面の範囲の欄 [0, 5] より優先。半円なので y ≥ 0）
    let m = run("⦃(cos(t), sin(t), 0) ∣ 0 ≤ t ≤ pi⦄", "", &[]).unwrap();
    let p = m.positions();
    assert!(p.len() > 30);
    for v in p.chunks_exact(3) {
        if v[1].is_finite() {
            assert!(v[1] >= -1e-6, "{v:?}");
        }
    }
    assert!(error_of("⦃(cos(t), sin(t), 0) ∣ 0 ≤ t⦄", "").contains("上の端"));
    assert!(error_of("x ∈ ℚ", "").contains("有理数"));
}

// ---- 集合の上の総和・総積、有限集合、⋃・⋂ ----

#[test]
fn sums_over_sets() {
    assert_eq!(value("@sumset(n; n ∈ ⦃1, 4, 9⦄) n"), 14.0);
    assert_eq!(value("@prodset(n; n ∈ ⦃2, 3, 5⦄) n"), 30.0);
    // 範囲と条件（1 から 10 までの奇数の和 = 25）
    assert_eq!(value("@sumset(n; 1 ≤ n ≤ 10, mod(n, 2) = 1) n"), 25.0);
    assert_eq!(value("@sumset(n; n ∈ [1, 10], mod(n, 2) = 1) n"), 25.0);
    // 約数の和 σ(12) = 28、個数 6
    assert_eq!(value("@sumset(d; d ∣ 12) d"), 28.0);
    assert_eq!(value("@sumset(d; d ∣ 12) 1"), 6.0);
    // 2 つの添字: Σ_{1≤m≤3, 1≤n≤3} m n = 36、m < n だけなら 11
    assert_eq!(value("@sumset((m, n); 1 ≤ m ≤ 3, 1 ≤ n ≤ 3) m n"), 36.0);
    assert_eq!(value("@sumset((m, n); 1 ≤ m < n ≤ 3) m n"), 11.0);
    // 「または」で分かれた範囲、空の集合
    assert_eq!(value("@sumset(n; n ≤ -8 ∨ n ≥ 8, -9 ≤ n ≤ 9) 1"), 4.0);
    assert_eq!(value("@sumset(n; 3 ≤ n ≤ 1) n"), 0.0);
    // 項は式（x を使ってよい）。範囲にパラメータ
    let mut ps = std::collections::BTreeMap::new();
    ps.insert("N".to_string(), num_complex::Complex64::new(5.0, 0.0));
    let sp = graph_core::equation::Space::standard();
    assert!(graph_core::equation::compile_with("z = @sumset(n; 1 ≤ n ≤ N, mod(n, 2) = 1) sin(n x)/n", &ps, &sp).is_ok());
    // 名前を付けた有限集合
    let defs = "S = ⦃1, 2, 3⦄";
    let _d = graph_core::expand::install(graph_core::expand::Defs::parse(defs, &Default::default()));
    assert_eq!(graph_core::equation::eval_constant("@sumset(k; k ∈ S) k^2").unwrap(), 14.0);
    drop(_d);
    assert!(error_of("z = @sumset(n; n ≥ 1) n", "").contains("範囲"));
    assert!(error_of("z = @sumset(n; 1 ≤ n ≤ 3, n < x) n", "").contains("x"));
}

#[test]
fn finite_sets_and_big_unions() {
    // x ∈ {1, 2, 3} は 3 枚の平面
    assert_eq!(mesh("x ∈ ⦃-1, 0, 1⦄", "", &[]).0.len(), mesh("x = -1 ∨ x = 0 ∨ x = 1", "", &[]).0.len());
    // ⋃_{k=1}^{3} {(x,y,z) | (x − 2k + 4)² + y² + z² ≤ 0.5}: 3 つの球
    let m = run("@bigcup(k; 1 ≤ k ≤ 3) ⦃(x, y, z) ∣ (x - k + 2)^2 + y^2 + z^2 ≤ 0.2⦄", "", &[]).unwrap();
    let p = m.positions();
    let centers: std::collections::BTreeSet<i32> = p.chunks_exact(3).filter(|v| v[0].is_finite()).map(|v| v[0].round() as i32).collect();
    assert!(centers.contains(&-1) && centers.contains(&0) && centers.contains(&1), "{centers:?}");
    // ⋂: 3 つの半空間の共通部分
    assert_eq!(mesh("@bigcap(k; k ∈ ⦃0, 1⦄) (x + k ≥ 0)", "", &[]), mesh("x + 0 ≥ 0 ∧ x + 1 ≥ 0", "", &[]));
}

// ---- M6: ベクトルと行列の計算 ----

/// 定義を入れたうえでの、パラメータの無い式の値
fn value_with(src: &str, defs: &str) -> f64 {
    let _d = graph_core::expand::install(graph_core::expand::Defs::parse(defs, &Default::default()));
    graph_core::equation::eval_constant(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

#[test]
fn matrix_products_and_determinants() {
    // ⟦1,2;3,4⟧ ⟦5,6;7,8⟧ = ⟦19,22;43,50⟧
    let ab = "(⟦1,2;3,4⟧ ⟦5,6;7,8⟧)";
    let want = [[19.0, 22.0], [43.0, 50.0]];
    for (i, row) in want.iter().enumerate() {
        for (j, w) in row.iter().enumerate() {
            let e_i = if i == 0 { "⟦1,0⟧" } else { "⟦0,1⟧" };
            let e_j = if j == 0 { "(1,0)" } else { "(0,1)" };
            assert_eq!(value(&format!("{e_i} {ab} {e_j}")), *w);
        }
    }
    assert_eq!(value("@det(⟦1,2;3,4⟧)"), -2.0);
    assert_eq!(value("abs(⟦1,2;3,4⟧)"), -2.0);
    assert_eq!(value("@tr(⟦1,2;3,4⟧)"), 5.0);
    // 逆行列 ⟦-2, 1; 1.5, -0.5⟧
    assert_eq!(value("⟦1,0⟧ ⟦1,2;3,4⟧^(-1) (1,0)"), -2.0);
    assert_eq!(value("⟦0,1⟧ ⟦1,2;3,4⟧^(-1) (1,0)"), 1.5);
    assert_eq!(value("⟦0,1⟧ @inv(⟦1,2;3,4⟧) (0,1)"), -0.5);
    // 外積・内積・ノルム・なす角
    assert_eq!(value("((1,2,3)×(4,5,6))(1,0,0)"), -3.0);
    assert_eq!(value("@cross((1,2,3),(4,5,6)) (0,1,0)"), 6.0);
    assert_eq!(value("@cross((1,2,3),(4,5,6)) (0,0,1)"), -3.0);
    assert_eq!(value("(1,2,3)(4,5,6)"), 32.0);
    assert_eq!(value("@dot((1,2,3),(4,5,6))"), 32.0);
    assert_eq!(value("@norm((3,4))"), 5.0);
    assert!((value("@angle((1,0),(0,2))") - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
    // 転置・べき
    assert_eq!(value("(1,2,3)^T (4,5,6)"), 32.0);
    assert_eq!(value("⟦1,0⟧ ⟦1,2;3,4⟧ᵀ (0,1)"), 3.0);
    assert_eq!(value("@tr(⟦1,2;3,4⟧^3)"), value("@tr(⟦1,2;3,4⟧ ⟦1,2;3,4⟧ ⟦1,2;3,4⟧)"));
    assert_eq!(value("@det(⟦1,2;3,4⟧^0)"), 1.0);
    assert_eq!(value("@det(⟦2,i;0,1⟧^†)"), 2.0);
}

#[test]
fn determinant_5x5_matches_lu() {
    // 決まった乱数の 5×5 の行列で、余因子展開と数値の LU を比べる
    let mut seed = 12345u64;
    let mut next = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as f64 / (1u64 << 31) as f64) * 4.0 - 2.0
    };
    let n = 5;
    let a: Vec<Vec<f64>> = (0..n).map(|_| (0..n).map(|_| (next() * 100.0).round() / 100.0).collect()).collect();
    let src = format!("@det(⟦{}⟧)", a.iter().map(|r| r.iter().map(|v| format!("({v})")).collect::<Vec<_>>().join(",")).collect::<Vec<_>>().join(";"));
    let mut m = a.clone();
    let mut det = 1.0;
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| m[i][c].abs().total_cmp(&m[j][c].abs())).unwrap();
        if p != c {
            m.swap(p, c);
            det = -det;
        }
        det *= m[c][c];
        for r in c + 1..n {
            let f = m[r][c] / m[c][c];
            for k in c..n {
                m[r][k] -= f * m[c][k];
            }
        }
    }
    assert!((value(&src) - det).abs() < 1e-9, "{} と {det}", value(&src));
}

#[test]
fn matrices_draw_the_same_mesh() {
    assert_eq!(
        mesh("⟦cos(a), -sin(a); sin(a), cos(a)⟧ (cos(t), 2 sin(t))", "", &[("a", 0.5)]),
        mesh("(cos(a) cos(t) + (-sin(a)) (2 sin(t)), sin(a) cos(t) + cos(a) (2 sin(t)))", "", &[("a", 0.5)])
    );
    assert!(max_diff("z = @det(⟦x, y; y, x⟧)", "z = x^2 - y^2", "") < 1e-6);
    // ベクトルの等式は、成分ごとの連立
    let bits = |src: &str| mesh(src, "", &[]).0.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits("(x, y, z) = (1, 2, 3)"), bits("x = 1, y = 2, z = 3"));
    // 定義したベクトル・行列
    let defs = "{vec_b} = (1, 2, 3)\nA = ⟦2, 0; 0, 3⟧\nB = A^T A";
    assert_eq!(mesh("{vec_b} + t (0, 1, 1)", defs, &[]), mesh("(1, 2 + t, 3 + t)", "", &[]));
    assert_eq!(value_with("@det(A)", defs), 6.0);
    assert_eq!(value_with("@tr(B)", defs), 13.0);
    assert_eq!(value_with("{vec_b}_2 + {vec_b}_(3)", defs), 5.0);
    assert_eq!(value_with("A_(2,2)", defs), 3.0);
    assert_eq!(value_with("p_1 + p_2", "p = (10, 20)"), 30.0);
    assert_eq!(mesh("A (cos(t), sin(t))", defs, &[]), mesh("(2 cos(t), 3 sin(t))", "", &[]));
    let m = run("A (cos(t), sin(t))", defs, &[]).unwrap();
    assert_eq!(m.used_defs(), "A");
}

#[test]
fn matrix_errors_and_sizes() {
    assert!(error_of("z = @det(⟦1,2,3;4,5,6⟧ ⟦1,2;3,4⟧) + x", "").contains("2×3 の行列と 2×2 の行列は掛けられません"));
    assert!(error_of("z = @det(⟦1,2,3;4,5,6⟧) + x", "").contains("正方行列"));
    // 行が 2・3 の行列だけの行は、正方でなくても列ベクトルを描く。行が 4 以上なら描けない
    assert_eq!(error_of("⟦1,2,3;4,5,6⟧", ""), "");
    assert!(error_of("⟦1,2;3,4;5,6;7,8⟧", "").contains("行列だけの行"));
    assert_eq!(error_of("⟦1,2;3,4⟧", ""), "");
    assert!(error_of("z = @tr(⟦1,2;2,4⟧^(-1)) + x", "").contains("逆行列はありません"));
    assert!(error_of("z = A_(3,1) + x", "A = ⟦1,2;3,4⟧").contains("成分はありません"));
    // 6×6 の行列式（成分が全部違う式）が、節の数の上限の中
    let names = ["x", "y", "z"];
    let rows: Vec<String> = (0..6)
        .map(|i| (0..6).map(|j| format!("({}+{})", names[(i + j) % 3], i * 6 + j + 1)).collect::<Vec<_>>().join(","))
        .collect();
    let src = format!("z = @det(⟦{}⟧)", rows.join(";"));
    let chains = graph_core::parser::parse_system(&src).unwrap();
    let e = graph_core::expand::expand(&chains[0].exprs[1], &Default::default()).unwrap();
    assert!(e.node_count() < 200_000, "{}", e.node_count());
}

// ---- M7: 行列の描き方 ----

/// 有限の頂点
fn points(src: &str, defs: &str) -> Vec<[f32; 3]> {
    let m = run(src, defs, &[]).unwrap_or_else(|e| panic!("{e}: {}", error_of(src, defs)));
    m.positions().chunks_exact(3).filter(|v| v.iter().all(|c| c.is_finite())).map(|v| [v[0], v[1], v[2]]).collect()
}

#[test]
fn linear_systems_are_reduced() {
    // 重なった平面は、平面 1 枚（連立方程式の曲線にしない）
    let m = run("x + y + z = 1, 2x + 2y + 2z = 2", "", &[]).unwrap();
    assert_eq!(m.kind(), run("x + y + z = 1", "", &[]).unwrap().kind());
    let p = points("x + y + z = 1, 2x + 2y + 2z = 2", "");
    assert!(p.len() > 100 && p.iter().all(|v| (v[0] + v[1] + v[2] - 1.0).abs() < 1e-4));
    assert!(error_of("x + y = 1, x + y = 2", "").contains("解がありません"));
    // 4 本・階数 3 は点 (1/2, 1/2, 0)
    let p = points("x + y + z = 1, x - y = 0, z = 0, 2x + z = 1", "");
    assert!(!p.is_empty() && p.iter().all(|v| (v[0] - 0.5).abs() < 1e-6 && (v[1] - 0.5).abs() < 1e-6 && v[2].abs() < 1e-6), "{p:?}");
    // ベクトルの等式 A x = b
    let p = points("⟦1,1,1; 2,2,2⟧ (x, y, z) = (1, 2)", "");
    assert!(p.len() > 100 && p.iter().all(|v| (v[0] + v[1] + v[2] - 1.0).abs() < 1e-4));
    // 独立な 1 次式・1 次式でない連立は、今までの道のまま（掃き出さない）
    let sp = graph_core::equation::Space::standard();
    let ps = Default::default();
    for src in ["x + y + z = 1, x - y = 0", "x^2 + y^2 = 1, z = 0"] {
        assert!(matches!(graph_core::equation::compile_with(src, &ps, &sp), Ok(graph_core::equation::Plot::System { ref eqs, .. }) if eqs.len() == 2), "{src}");
    }
}

#[test]
fn kernel_and_image() {
    // ker ⟦1, 1, 1⟧ は平面 x + y + z = 0（表示範囲の箱の中だけ）
    let m = run("@ker(⟦1, 1, 1⟧)", "", &[]).unwrap();
    assert!(m.kind().contains("x + y + z = 0（2 次元。平面）"), "{}", m.kind());
    let p = points("@ker(⟦1, 1, 1⟧)", "");
    assert!(p.len() > 100);
    assert!(p.iter().all(|v| (v[0] + v[1] + v[2]).abs() < 1e-5 && v.iter().all(|c| c.abs() <= 3.0 + 1e-5)));
    // 箱の角の近くまで届いている
    assert!(p.iter().any(|v| v[0] > 2.9) && p.iter().any(|v| v[0] < -2.9));
    // ker ⟦1,0,0; 0,1,0⟧ は z 軸
    let p = points("@ker(⟦1,0,0; 0,1,0⟧)", "");
    assert!(p.len() > 10 && p.iter().all(|v| v[0].abs() < 1e-6 && v[1].abs() < 1e-6 && v[2].abs() <= 3.0 + 1e-5));
    // Im ⟦1,0; 0,1; 0,0⟧ は平面 z = 0
    let p = points("Im(⟦1,0; 0,1; 0,0⟧)", "");
    assert!(p.len() > 100 && p.iter().all(|v| v[2].abs() < 1e-6));
    assert_eq!(points("@im(A)", "A = ⟦1,0; 0,1; 0,0⟧").len(), p.len());
    // 引数がスカラーの Im は、今までどおり虚部
    assert!(max_diff("z = Im((x + i y)^2)", "z = 2 x y", "") < 1e-6);
    // 2 列の行列の ker は、xy 平面の中の直線（2D 表示でも同じ）
    let p = points("@ker(⟦1, 1⟧)", "");
    assert!(p.len() > 10 && p.iter().all(|v| (v[0] + v[1]).abs() < 1e-5 && v[2] == 0.0));
    assert!(error_of("@ker(⟦1, 1, 1, 1⟧)", "").contains("列を 3 つ以下にするか、次元を 4 に"));
    assert!(error_of("@ker(⟦x, 1, 1⟧)", "").contains("数とパラメータ"));
    // 次元・階数は数
    assert_eq!(value("@dim(@ker(⟦1, 1, 1⟧))"), 2.0);
    assert_eq!(value("@dim @ker ⟦1,0,0; 0,1,0⟧ + @rank(⟦1,2;2,4⟧)"), 2.0);
    assert_eq!(value("@dim(Im(⟦1,2;2,4⟧))"), 1.0);
}

#[test]
fn determinant_line_and_columns() {
    // 行列式だけの行: 列ベクトルが張る箱 [0,2] × [0,1] × [0,3]
    let m = run("@det(⟦2,0,0; 0,1,0; 0,0,3⟧)", "", &[]).unwrap();
    assert!(m.kind().contains("det = 6"), "{}", m.kind());
    let p = points("@det(⟦2,0,0; 0,1,0; 0,0,3⟧)", "");
    let (lo, hi) = p.iter().fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), v| ([0, 1, 2].map(|k| lo[k].min(v[k])), [0, 1, 2].map(|k| hi[k].max(v[k]))));
    assert_eq!((lo, hi), ([0.0, 0.0, 0.0], [2.0, 1.0, 3.0]));
    // 成分に座標があれば、今までどおり「縦 = 式」
    assert!(max_diff("@det(⟦x, y; y, x⟧)", "z = x^2 - y^2", "") < 1e-6);
    // 行列の定義の行は、列ベクトルを原点からの線分で描く
    let m = run("A = ⟦1, 0; 1, 2⟧", "A = ⟦1, 0; 1, 2⟧", &[]).unwrap();
    assert!(m.kind().contains("A の定義（列ベクトル）"), "{}", m.kind());
    let p = points("A = ⟦1, 0; 1, 2⟧", "A = ⟦1, 0; 1, 2⟧");
    // 矢印の先（矢じりの頂点）と、軸の線分の始点（原点）
    assert!(p.iter().any(|v| *v == [1.0, 1.0, 0.0]) && p.iter().any(|v| *v == [0.0, 2.0, 0.0]));
    // 軸の尾（太さのある軸の尾のふた）は原点のすぐそば
    assert!(p.iter().any(|v| v.iter().map(|c| c * c).sum::<f32>().sqrt() < 0.2));
    // ベクトルの定義は、そのベクトル
    let p = points("{vec_b} = (1, 2, 3)", "{vec_b} = (1, 2, 3)");
    assert!(p.iter().any(|v| *v == [1.0, 2.0, 3.0]));
    // 成分が式の行列の定義は、図形なし
    assert!(points("A = ⟦x, 0; 1, 2⟧", "A = ⟦x, 0; 1, 2⟧").is_empty());
}

#[test]
fn matrix_as_coordinate_system() {
    // 行列 ⟦1, a; 0, 1⟧ を座標系にすると、頂点の移した先は (x + a y, y, z)。成分のパラメータ・定義の名前を「使った」と報告する
    graph_core::coord_system("(A_(1,1) x + A_(1,2) y, A_(2,1) x + A_(2,2) y, z)", "");
    let mut m = run("x^2 + y^2 + z^2 = 4", "A = ⟦1, a; 0, 1⟧", &[("a", 0.5)]).unwrap();
    let (p, t) = (m.positions(), m.take_target());
    assert!(p.len() > 300 && p.len() == t.len());
    for (q, w) in p.chunks_exact(3).zip(t.chunks_exact(3)) {
        if q[0].is_finite() {
            assert!((w[0] - (q[0] + 0.5 * q[1])).abs() < 1e-5 && (w[1] - q[1]).abs() < 1e-6 && (w[2] - q[2]).abs() < 1e-6);
        }
    }
    assert!(m.used_params().split(',').any(|n| n == "a"), "{}", m.used_params());
    assert!(m.used_defs().split(',').any(|n| n == "A"), "{}", m.used_defs());
}

// ---- M8: ベクトル場 ----

#[test]
fn vector_fields() {
    let mut m = run("(-y, x, 0)", "", &[]).unwrap();
    assert!(m.kind().starts_with("ベクトル場"), "{}", m.kind());
    // 9³ − 9（z 軸の上は大きさ 0）。矢印は全部メッシュ（軸も筒）で、流線を描かなければ線は無い
    assert!(m.kind().contains(&format!("{} 本の矢印", 9 * 9 * 9 - 9)), "{}", m.kind());
    assert!(m.take_outline().is_empty());
    assert_eq!(m.value_range().len(), 2);
    // ベクトル場は、鮮やかな色の対応表で塗る
    assert!(m.vivid());
    // 勾配・回転は、手で書いた場と同じ矢印
    let arrows = |src: &str, defs: &str| {
        let mut m = run(src, defs, &[]).unwrap();
        (m.positions(), m.take_outline())
    };
    let near = |a: (Vec<f32>, Vec<f32>), b: (Vec<f32>, Vec<f32>), what: &str| {
        assert_eq!((a.0.len(), a.1.len()), (b.0.len(), b.1.len()), "{what}");
        let close = |x: &f32, y: &f32| (x.is_nan() && y.is_nan()) || (x - y).abs() < 1e-5;
        assert!(a.0.iter().zip(&b.0).all(|(x, y)| close(x, y)) && a.1.iter().zip(&b.1).all(|(x, y)| close(x, y)), "{what}");
    };
    near(arrows("@grad(x^2 + y^2 - z)", ""), arrows("(2x, 2y, -1)", ""), "勾配");
    near(arrows("@rot((-y, x, 0)) + (0 x, 0, 0)", ""), arrows("(0 x, 0, 2)", ""), "回転");
    // 成分が 2 つなら、縦の成分は 0
    near(arrows("(-y, x)", ""), arrows("(-y, x, 0)", ""), "2 成分");
    // 関数の定義の行 F(x, y, z) = (…) も、ベクトル場として描く
    let m = run("F(x, y, z) = (-y, x, 0)", "F(x, y, z) = (-y, x, 0)", &[]).unwrap();
    assert!(m.kind().contains("F の定義（ベクトル場）"), "{}", m.kind());
    // 媒介変数表示は今までどおり。座標と媒介変数が混ざった組は、今までと同じエラー
    assert!(run("(cos(t), sin(t), t)", "", &[]).unwrap().kind().contains("媒介変数表示"));
    assert!(!error_of("(x, t, 0)", "").is_empty());
}

// ---- M9: 常微分方程式 ----

/// 常微分方程式を描く（t の範囲 [t0, t1]、初期値の欄 `init`）。点（NaN で切った列）を返す
fn ode_points(src: &str, t: (f64, f64), init: &[&str], params: &[(&str, f64)]) -> (MeshResult, Vec<[f32; 3]>) {
    ode_points_in(src, 3.0, t, init, params)
}

/// 表示範囲 [−half, half]³ で
fn ode_points_in(src: &str, half: f64, t: (f64, f64), init: &[&str], params: &[(&str, f64)]) -> (MeshResult, Vec<[f32; 3]>) {
    definitions("");
    ode_init(&init.join("\n"));
    let names: Vec<&str> = params.iter().map(|p| p.0).collect();
    let values: Vec<f64> = params.iter().flat_map(|p| [p.1, 0.0]).collect();
    let m = plot(
        src, &[-half, half, -half, half, -half, half], 40, 24, "t", &[t.0, t.1], &names.join(","), &values, "", 3, &[0, 1, 2], &[0.0; 3],
        &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0,
    )
    .unwrap_or_else(|_| panic!("{src}: {}", error_of(src, "")));
    let p = m.positions().chunks_exact(3).map(|v| [v[0], v[1], v[2]]).collect();
    (m, p)
}

#[test]
fn ode_systems() {
    // 円 x' = −y, y' = x（(1, 0) から）
    let (m, p) = ode_points("x' = -y, y' = x", (0.0, 20.0 * std::f64::consts::PI), &["1", "0"], &[]);
    assert!(m.kind().contains("常微分方程式"), "{}", m.kind());
    assert_eq!(m.ode_states(), "x,y");
    let fin: Vec<_> = p.iter().filter(|v| v[0].is_finite()).collect();
    assert!(fin.len() > 100 && fin.iter().all(|v| (v[0] * v[0] + v[1] * v[1] - 1.0).abs() < 1e-4 && v[2] == 0.0));
    // 2 階: x'' = −x は同じ円（x(0) = 1, x'(0) = 0 → x = cos t）。プライムの無い y は一定
    let (m, _) = ode_points("x'' = -x - x'/5", (0.0, 10.0), &[], &[]);
    assert_eq!(m.ode_states(), "x,x'");
    // ローレンツ（t ∈ [0, 40]）: 点が 3 万個以下
    let start = std::time::Instant::now();
    let lorenz = "x' = 10(y - x), y' = x(28 - z) - y, z' = x y - 8z/3";
    let (m, p) = ode_points_in(lorenz, 30.0, (0.0, 40.0), &[], &[]);
    // t = 40 まで解いた（箱の 8 倍を出ていない）: 最後の点は、アトラクターの上
    assert!(p.len() > 3_000 && p.last().is_some_and(|v| v[2] > 0.0 && v[2] < 60.0));
    let ms = start.elapsed().as_secs_f64() * 1e3;
    assert!(p.len() <= 30_000, "{} 点", p.len());
    assert!(!m.kind().contains("打ち切り"));
    eprintln!("ローレンツ: {ms:.1} ms, {} 点", p.len());
    // 同じ入力なら同じ点
    let again = ode_points_in(lorenz, 30.0, (0.0, 40.0), &[], &[]).1;
    assert!(p.iter().zip(&again).all(|(a, b)| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())));
    // 初期値のリストは、組み合わせの数だけ（パラメータも使える）
    let (m, _) = ode_points("x' = -y, y' = x", (0.0, 6.0), &["[1, 2]", "a"], &[("a", 0.5)]);
    assert!(m.kind().contains("2 本"), "{}", m.kind());
    assert!(m.used_params().split(',').any(|n| n == "a"));
    // \frac{dx}{dt} の形も同じ
    assert_eq!(ode_points("@diff(t) x = -y, @diff(t) y = x", (0.0, 6.0), &[], &[]).1.len(), ode_points("x' = -y, y' = x", (0.0, 6.0), &[], &[]).1.len());
    assert!(error_of("x' = y, x' = 1", "").contains("2 回"));
}

#[test]
fn ode_single_equations() {
    // y'' + y = 0（y(0) = 0, y'(0) = 1）は y = sin x
    let (m, p) = ode_points("y'' + y = 0", (0.0, 1.0), &["0", "1"], &[]);
    assert_eq!(m.ode_states(), "y,y'");
    let curve: Vec<_> = p.iter().take_while(|v| v[0].is_finite()).collect();
    assert!(curve.len() > 50 && curve.iter().all(|v| (v[1] as f64 - (v[0] as f64).sin()).abs() < 1e-4));
    assert!((curve[0][0] + 3.0).abs() < 1e-6 && (curve.last().unwrap()[0] - 3.0).abs() < 1e-6);
    // 2 y' + 4 y = 8（解くと y' = (8 − 4y)/2）は y = 2 + C e^{−2x}（y(0) = 3 なら C = 1）。1 階なので傾きの場もある
    let (m, p) = ode_points("2 y' + 4 y = 8", (0.0, 1.0), &["3"], &[]);
    assert!(m.kind().contains("傾きの場"), "{}", m.kind());
    let curve: Vec<_> = p.iter().take_while(|v| v[0].is_finite()).filter(|v| v[0] > -0.5).collect();
    assert!(curve.iter().all(|v| (v[1] as f64 - (2.0 + (-2.0 * v[0] as f64).exp())).abs() < 1e-4));
    // 解いた形にできない
    assert!(error_of("y' y' = 1", "").contains("解いた形"));
    // y' = y²、y(0) = 1（x = 1 で発散）は、箱を出た所で止まり、エラーにならない
    let (_, p) = ode_points("y' = y^2", (0.0, 1.0), &["1"], &[]);
    let curve: Vec<_> = p.iter().take_while(|v| v[0].is_finite()).collect();
    assert!(curve.last().unwrap()[0] < 1.0);
    // 定義した関数の導関数 f'(x) は、今までどおり
    assert!(max_diff("y = f'(x)", "y = 2x", "f(x) = x^2") < 1e-9);
}

// ---- M10: 数列 ----

fn ps(list: &[(&str, f64)]) -> graph_core::eval::Params {
    list.iter().map(|(n, v)| (n.to_string(), num_complex::Complex64::new(*v, 0.0))).collect()
}

/// 定義とパラメータを入れたうえでの値
fn seq_value(src: &str, defs: &str, params: &[(&str, f64)]) -> Result<f64, String> {
    let p = ps(params);
    let _d = graph_core::expand::install(graph_core::expand::Defs::parse(defs, &p));
    graph_core::equation::eval_constant_with(src, &p).map(|v| v.0)
}

#[test]
fn sequences() {
    assert!((seq_value("a_(3)", "a_(n) = 1/n^2", &[]).unwrap() - 1.0 / 9.0).abs() < 1e-15);
    assert_eq!(seq_value("a_(10)", "a_(n+1) = a_(n) + 2", &[("a1", 1.0)]).unwrap(), 19.0);
    // 数字の添字 a_10（字句では a10）も項
    assert_eq!(seq_value("a_10 + a_1", "a_(n+1) = a_(n) + 2", &[("a1", 1.0)]).unwrap(), 20.0);
    // フィボナッチの a₈₀
    let start = std::time::Instant::now();
    let f80 = seq_value("a_(80)", "a_(n) = a_(n-1) + a_(n-2)", &[("a1", 1.0), ("a2", 1.0)]).unwrap();
    assert_eq!(f80, 23416728348467685.0);
    eprintln!("フィボナッチ a₈₀: {:.3} ms", start.elapsed().as_secs_f64() * 1e3);
    // 部分和は Σ と組み合わせる
    assert!((seq_value("s_(4)", "a_(n) = 1/n\ns_(n) = @sum(k,1,n) a_(k)", &[]).unwrap() - 25.0 / 12.0).abs() < 1e-14);
    // 初項が無ければ、スライダーを足せる形のエラー
    let e = seq_value("a_(5)", "a_(n+1) = 2 a_(n)", &[]).unwrap_err();
    assert!(e.contains("変数 'a1' はここでは使えません"), "{e}");
    // 添字が数字の名前は今までどおり（パラメータ a1・座標 w₂）
    assert_eq!(seq_value("a_1 + 1", "", &[("a1", 2.0)]).unwrap(), 3.0);
}

#[test]
fn sequence_terms_with_coordinates() {
    // z = a_(3)（a_(n) = x^n）と z = x^3 が同じメッシュ
    same("z = a_(3)", "z = x^3", "a_(n) = x^n");
    // マンデルブロ: z_(n+1) = z_(n)^2 + x + i y、z0 = 0 のもとで |z₈| = 2 を、手で 8 回入れ子にした式と比べる
    let defs = "z_(n+1) = z_(n)^2 + x + i y";
    let mut hand = "0".to_string();
    for _ in 0..8 {
        hand = format!("({hand})^2 + x + i y");
    }
    let (a, b) = (mesh_p(&format!("abs(z_(8)) = 2 + 0 z"), defs, &[("z0", 0.0)]), mesh_p(&format!("abs({hand}) = 2 + 0 z"), "", &[("z0", 0.0)]));
    assert!(!a.0.is_empty());
    assert_eq!(a, b);
}

fn mesh_p(src: &str, defs: &str, params: &[(&str, f64)]) -> (Vec<f32>, Vec<u32>) {
    mesh(src, defs, params)
}

#[test]
fn sequence_lines_draw_points() {
    // a_(n) = 1/n の点 (n, 1/n, 0)（n は横の表示範囲 [−3, 3] の中の 0 以上の整数。n = 0 は無限大なので描かない）
    let m = run("a_(n) = 1/n", "a_(n) = 1/n", &[]).unwrap();
    assert!(m.kind().contains("a の定義（数列の点）"), "{}", m.kind());
    let p: Vec<[f32; 3]> = m.positions().chunks_exact(3).filter(|v| v[0].is_finite()).map(|v| [v[0], v[1], v[2]]).collect();
    for n in [1, 2, 3] {
        assert!(p.iter().any(|v| *v == [n as f32, 1.0 / n as f32, 0.0]), "{n}: {p:?}");
    }
    // 初項の行 a_1 = 8 そのものは、今までどおりパラメータの定義
    let m = run("a_1 = 8", "a_(n+1) = a_(n) / 2\na_1 = 8", &[]).unwrap();
    assert_eq!(m.param_name(), "a1");
    // 漸化式は初項の添字から
    let m = run("a_(n+1) = a_(n) / 2", "a_(n+1) = a_(n) / 2", &[("a1", 2.0)]).unwrap();
    let p: Vec<f32> = m.positions().chunks_exact(3).filter(|v| v[0].is_finite()).map(|v| v[0]).collect();
    assert!(p.iter().all(|n| *n >= 1.0) && p.contains(&3.0));
    assert!(m.used_params().split(',').any(|n| n == "a1"), "{}", m.used_params());
    // 複素数の数列は、複素平面の軌道
    let m = run("w_(n+1) = w_(n) (0.8 + 0.5 i)", "w_(n+1) = w_(n) (0.8 + 0.5 i)", &[("w0", 1.0)]).unwrap();
    assert!(m.kind().contains("軌道"), "{}", m.kind());
}

// ---- M13: 名前に使える文字と、小さな記号 ----

#[test]
fn names_and_small_symbols() {
    // 名前 {alpha} のスライダー、関数 beta と名前 {beta} が同じ式の中で両方使える
    assert_eq!(mesh("z = {alpha} x", "", &[("alpha", 2.0)]), mesh("z = 2 x", "", &[]));
    // 2 文字以上の名前の値の行は、パラメータの定義
    assert_eq!(run("{alpha} = 2", "", &[]).unwrap().param_name(), "alpha");
    let p = ps(&[("alpha", 2.0), ("beta", 3.0)]);
    let _d = graph_core::expand::install(graph_core::expand::Defs::parse("", &p));
    let (v, _) = graph_core::equation::eval_constant_with("beta(2, 3) + {beta}", &p).unwrap();
    assert!((v - (1.0 / 12.0 + 3.0)).abs() < 1e-12, "{v}");
    drop(_d);
    // a ≡ b (mod n) は latex.ts が mod(a − b, n) = 0 にする。a mod b（%%）
    assert_eq!(value("7 %% 3"), 1.0);
    assert_eq!(value("2 + 7 %% 3 * 2"), 4.0);
    assert_eq!(value("gcd(12, 18) + lcm(4, 6)"), 18.0);
    // 直交・平行
    let p = points("(x, y, z) ⟂ (1, 1, 1)", "");
    assert!(p.len() > 100 && p.iter().all(|v| (v[0] + v[1] + v[2]).abs() < 1e-4));
    let p = points("(x, y, z) ∥ (1, 1, 1)", "");
    assert!(!p.is_empty() && p.iter().all(|v| (v[0] - v[1]).abs() < 1e-3 && (v[1] - v[2]).abs() < 1e-3), "{:?}", &p[..p.len().min(4)]);
    // 共役 z^∗
    assert!(max_diff("z = Re((x + i y)^∗ (x + i y))", "z = x^2 + y^2", "") < 1e-9);
}

// ---- M12: ∃・∀・添字つきの max・min ----

#[test]
fn quantifiers() {
    // ∃a ∈ [0, 1]: 範囲で重ねるのと同じメッシュ
    let stacked = {
        definitions("");
        plot(
            "x^2+y^2+(z-a)^2 = 1", &[-3.0, 3.0, -3.0, 3.0, -3.0, 3.0], 40, 24, "t", &[0.0, 5.0], "", &[], "", 3, &[0, 1, 2], &[0.0; 3],
            &[f64::NAN; 6], "a", &[0.0, 1.0], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0,
        )
        .unwrap_or_else(|_| panic!("stack"))
    };
    let m = run("@exists(a,0,1) x^2+y^2+(z-a)^2 = 1", "", &[]).unwrap();
    assert_eq!((m.positions(), m.indices()), (stacked.positions(), stacked.indices()));
    // ∃a ∈ [0, 2π]: x = cos a, y = sin a, z = 0 は円
    let (pa, _) = mesh("@exists(a,0,2pi) x = cos(a), y = sin(a), z = 0", "", &[]);
    let circle = {
        definitions("");
        plot(
            "(cos(t), sin(t), 0)", &[-3.0, 3.0, -3.0, 3.0, -3.0, 3.0], 40, 24, "t", &[0.0, std::f64::consts::TAU], "", &[], "", 3, &[0, 1, 2],
            &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0,
        )
        .unwrap_or_else(|_| panic!("circle"))
    };
    assert_eq!(pa, circle.positions());
    // ∀a ∈ [−1, 1]: z ≥ a x は z ≥ |x|（近似）。頂点が z ≥ |x| − 少し の上
    let p = points("@forall(a,-1,1) z ≥ a x", "");
    assert!(p.len() > 100 && p.iter().all(|v| v[2] >= v[0].abs() - 0.1));
    let m = run("@forall(a,-1,1) z ≥ a x", "", &[]).unwrap();
    assert!(m.kind().contains("近似"), "{}", m.kind());
    // ⋃（M11 で入れた書き方）: 3 つの球
    let p = points("@bigcup(k; 1 ≤ k ≤ 3) ⦃(x, y, z) ∣ (x - k + 2)^2 + y^2 + z^2 ≤ 0.2⦄", "");
    assert!(p.iter().any(|v| v[0] > 0.7) && p.iter().any(|v| v[0] < -0.7) && p.iter().any(|v| v[0].abs() < 0.3));
    assert!(error_of("@forall(a,0,1) x = a", "").contains("不等式だけ"));
}

#[test]
fn indexed_max() {
    // max_{t ∈ [0, 1]} t x = max(0, x)（端の点も等分点に入る）
    assert!(max_diff("z = @max(t, 0, 1) (t x)", "z = max(0, x)", "") < 1e-12);
    assert!(max_diff("z = @min(t, 0, 1) (t x)", "z = min(0, x)", "") < 1e-12);
    let m = run("z = @max(t, 0, 1) sin(x t) y", "", &[]).unwrap();
    assert!(m.kind().contains("近似"), "{}", m.kind());
    // ふつうの max(a, b) は今までどおり
    assert!(max_diff("z = max(x, y)", "z = (x + y + abs(x - y)) / 2", "") < 1e-9);
}

/// tr A・det A だけの行は、値を説明に出す（図形は高さがその値の平面）
#[test]
fn trace_line_shows_value() {
    let defs = "A=⟦1,2;3,4⟧";
    assert!(run("@tr(A)", defs, &[]).unwrap().kind().starts_with("tr = 5"));
    assert!(run("@rank(A)", defs, &[]).unwrap().kind().starts_with("rank = 2"));
}

/// ベクトルの名前 F⃗（`{vec_F}`）でベクトル場を定義して、線積分・面積分・体積分・発散に使う
#[test]
fn vector_named_field_in_integrals() {
    let defs = "{vec_F} (x,y,z)=(-y,x,0)\n{vec_G} (x,y,z)=(x,y,z)";
    let pi = std::f64::consts::PI;
    // 仕事: 単位円を 1 周 → 2π（点を入れる形と、名前だけの形）
    assert!((value_with("@intset(( cos t, sin t,0),0<=t<=2 pi; {vec})({vec_F})", defs) - 2.0 * pi).abs() < 1e-9);
    assert!((value_with("@intset(( cos t, sin t,0),0<=t<=2 pi; {vec})({vec_F} (x,y,z))", defs) - 2.0 * pi).abs() < 1e-9);
    // 流束: 単位球面を通る G = r の流束は 4π
    let sphere = "@intset(( sin u cos v, sin u sin v, cos u),0<=u<=pi,0<=v<=2 pi; {vec})({vec_G})";
    assert!((value_with(sphere, defs) - 4.0 * pi).abs() < 1e-6, "{}", value_with(sphere, defs));
    // 体積分: ∭ ∇·G dV（単位球）= 3 · 4π/3 = 4π（ガウスの定理と同じ値）
    let ball = "@intset((r sin u cos v,r sin u sin v,r cos u),0<=r<=1,0<=u<=pi,0<=v<=2 pi)(@div {vec_G})";
    assert!((value_with(ball, defs) - 4.0 * pi).abs() < 1e-6, "{}", value_with(ball, defs));
    // 定義の行と名前だけの行は、矢印の場
    assert_eq!(error_of("{vec_F}", defs), "");
    assert_eq!(error_of("{vec_F} (x,y,z)", defs), "");
}

/// 点での値 @at(式, x, a, …): ∇ × F⃗ のように、展開してから値を入れる
#[test]
fn value_at_point() {
    let defs = "{vec_F} (x,y,z)=(-y,x,0)";
    assert_eq!(value_with("@at(x^2 y, x, 2, y, 3)", ""), 12.0);
    // ∇ × (−y, x, 0) = (0, 0, 2)
    assert_eq!(value_with("@at(@rot {vec_F}, x, 1, y, 2, z, 3) · (0,0,1)", defs), 2.0);
    // F⃗ を点で: (−2, 1, 0)
    assert_eq!(value_with("@at({vec_F}, x, 1, y, 2, z, 0) · (1,0,0)", defs), -2.0);
}

/// dim の書き方: 括弧があってもなくてもよい。dim A だけは、書き方を案内する
#[test]
fn dim_forms() {
    let defs = "A=⟦1,2;2,4⟧";
    for s in ["@dim @ker A", "@dim ( @ker (A))", "@dim(@ker(A))", "@dim ( @ker A)"] {
        assert_eq!(value_with(s, defs), 1.0, "{s}");
    }
    assert_eq!(value_with("@dim(Im(A))", defs), 1.0);
    assert!(error_of("@dim(A)", defs).contains("dim ker A"), "{}", error_of("@dim(A)", defs));
}

// ---- 集合・積分の範囲の書き方の自由度: 媒介変数の名前・等式で書いた曲線と曲面・積分の d の文字 ----

#[test]
fn parametric_sets_with_any_parameter_name() {
    // 点の式の媒介変数は t・u・v でなくてもよい（座標と同じ名前 x でも）
    same("⦃(x,x^(2))∣0<=x<=1⦄", "⦃(t,t^(2))∣0<=t<=1⦄", "");
    same("⦃(s, sin(s))∣0<=s<=6⦄", "⦃(t, sin(t))∣0<=t<=6⦄", "");
    same("⦃(x,y,x^(2)+y^(2))∣-1<=x<=1,-1<=y<=1⦄", "⦃(u,v,u^(2)+v^(2))∣-1<=u<=1,-1<=v<=1⦄", "");
    // 範囲の無い t・u・v を、付け替えた名前と混ぜない
    assert!(error_of("⦃(s, t, s)∣0<=s<=1⦄", "").contains("t の範囲"));
}

#[test]
fn regions_written_with_equations() {
    let sqrt3 = 3f64.sqrt();
    // 曲線 y = x²（弧長）・x = y = z（直線）
    let arc = (5f64.sqrt() / 2.0) + (2.0 + 5f64.sqrt()).ln() / 4.0;
    close("@intset((x,y),y=x^2,0<=x<=1; {d_s})(1)", arc, 1e-9);
    close("@intset((x,y),y=x^2,0<=x<=1)(1)", arc, 1e-9);
    close("@intset((x,y,z),x=y=z,0<=x<=1; {d_s})(x+y+z)", 1.5 * sqrt3, 1e-12);
    close("@intset((x,y), x = 1, 0<=y<=2; {d_s})(y)", 2.0, 1e-12);
    // 曲面 z = x² + y²（円板の上）: 面積 π(5√5 − 1)/6
    close("@intset((x,y,z),z=x^2+y^2,x^2+y^2<=1; {d_S})(1)", std::f64::consts::PI * (5.0 * 5f64.sqrt() - 1.0) / 6.0, 1e-5);
    // 1 次式にならない等式は、媒介変数で書くように言う
    assert!(graph_core::equation::eval_constant("@intset((x,y),x^2+y^2=1; {d_s})(1)").unwrap_err().contains("媒介変数"));
}

#[test]
fn differentials_must_match_the_region() {
    let err = |s: &str| graph_core::equation::eval_constant(s).unwrap_err();
    // ds の範囲が曲面・平面の領域、dA の範囲が曲線、F · dS の範囲が曲線
    assert!(err("@intset((u, v, u v),0<=u<=1, 0<=v<=1; {d_s})(1)").contains("曲面なら dS"));
    assert!(err("@intset((x,y),x^2+y^2<=1; {d_s})(1)").contains("平面の領域なら dA"));
    assert!(err("@intset((x,y,z),z=x^2+y^2,x^2+y^2<=1; {d_s})(1)").contains("ds は曲線"));
    assert!(err("@intset((x,y),y=x^2,0<=x<=1; {d_A})(1)").contains("曲線なら ds"));
    assert!(err("@intset((cos t, sin t),0<=t<=2pi; {vec},{d_S})((-y,x))").contains("F · dr"));
    assert!(err("@intset((x,y),x^2+y^2<=1; {d_x})(1)").contains("dx dy"));
    // 合っていれば今までどおり
    close("@intset((x,y),x^2+y^2<=1; {d_A})(1)", std::f64::consts::PI, 1e-6);
    close("@intset((x,y),x^2+y^2<=1; {d_x},{d_y})(1)", std::f64::consts::PI, 1e-6);
    close("@intset((u, v, u v),0<=u<=1, 0<=v<=1; {d_S})(1)", value("@intset((u, v, u v),0<=u<=1, 0<=v<=1)(1)"), 1e-12);
    close("@intset((cos t, sin t),0<=t<=2pi; {vec},{d_r})((-y,x))", 2.0 * std::f64::consts::PI, 1e-9);
    close("@intset((r cos(theta), r sin(theta)),0<=r<=1,0<=theta<=2pi; {d_A})(1)", std::f64::consts::PI, 1e-9);
}

#[test]
fn coordinate_and_parameter_differentials() {
    let pi = std::f64::consts::PI;
    // 曲線の上の P dx + Q dy（向きの付いた線積分）: ∮ −y dx + x dy = 2 × 面積
    close("@intset((cos t, sin t),0<=t<=2pi; {d_x})(-y) + @intset((cos t, sin t),0<=t<=2pi; {d_y})(x)", 2.0 * pi, 1e-9);
    // 媒介変数そのものの d は何も掛けない（極座標の r dr dθ）
    close("@intset((r cos theta, r sin theta),0<=r<=1,0<=theta<=2pi; {d_r},{d_theta})(r)", pi, 1e-9);
    close("@intset((cos t, sin t),0<=t<=2pi; {d_t})(1)", 2.0 * pi, 1e-12);
    // 曲面の上の dx dy は法線の z 成分（平面 x + y + z = 1 の三角形を xy 平面に映した面積 1/2）
    close("@intset((u,v,1-u-v),0<=u<=1,0<=v<=1-u; {d_x},{d_y})(1)", 0.5, 1e-12);
    close("@intset((u,v,1-u-v),0<=u<=1,0<=v<=1-u; {d_y},{d_x})(1)", -0.5, 1e-12);
}

#[test]
fn parametric_set_conditions() {
    // t ∈ [0, 1] は 0 ≤ t ≤ 1、≠ は描き方が変わらない
    same("⦃(t,t^(2))∣t∈[0,1]⦄", "⦃(t,t^(2))∣0<=t<=1⦄", "");
    same("⦃(cos(t),sin(t))∣0<=t<=2pi,t≠pi⦄", "⦃(cos(t),sin(t))∣0<=t<=2pi⦄", "");
    // 端がほかの媒介変数の式（扇形の三角形 0 ≤ t ≤ s）も描ける
    let (p, _) = mesh("⦃(s cos(t),s sin(t))∣0<=s<=1,0<=t<=s⦄", "", &[]);
    assert!(!p.is_empty());
    // 媒介変数で書いた集合どうしの ∪（折れ線の経路）は、それぞれ描いて重ねる
    let (p1, _) = mesh("⦃(t,t^(2))∣-1<=t<=1⦄", "", &[]);
    let (p2, _) = mesh("⦃(t,t^(2))∣-1<=t<=1⦄ ∪ ⦃(t,1)∣-1<=t<=1⦄", "", &[]);
    assert!(p2.len() > p1.len());
}

#[test]
fn integrals_over_unions_and_set_operations() {
    let pi = std::f64::consts::PI;
    // 点が同じ変数の組の集合どうしの ∩・∖・∪ は、条件の (x, y) ∈ A ∩ B
    close("@intset((x,y),(x,y)∈⦃(x,y)∣x^(2)+y^(2)<=1⦄ ∩ ⦃(x,y)∣y>=0⦄; {d_A})(1)", pi / 2.0, 1e-6);
    close("@intset((x,y),(x,y)∈⦃(x,y)∣x^(2)+y^(2)<=1⦄ ∖ ⦃(x,y)∣y>=0⦄; {d_A})(1)", pi / 2.0, 1e-6);
    // 平面全体（端が ∞）
    close("@intset((x,y),-∞<=x<=∞,-∞<=y<=∞; {d_A})(e^(-x^(2)-y^(2)))", pi, 1e-9);
}

#[test]
fn kernel_and_image_formulas() {
    let kind = |defs: &str, src: &str| run(src, defs, &[]).unwrap().kind().to_string();
    // 張るベクトル・媒介変数表示・方程式を、小さい整数で出す。名前は書いた行列の名前
    assert_eq!(kind("A= ⟦1,2;2,4⟧", "@ker A"), "ker A = span{(2, −1)}、(x, y) = t(2, −1)、x + 2y = 0（1 次元。直線）");
    assert_eq!(kind("A= ⟦1,2;2,4⟧", "Im A"), "Im A = span{(1, 2)}、(x, y) = t(1, 2)、2x − y = 0（1 次元。直線）");
    assert_eq!(kind("B= ⟦1,2,3;0,1,1⟧", "@ker B"), "ker B = span{(1, 1, −1)}、(x, y, z) = t(1, 1, −1)、x + z = 0, y + z = 0（1 次元。直線）");
    assert_eq!(kind("B= ⟦1,2,3;0,1,1⟧", "Im B"), "Im B = ℝ²（2 次元。平面）");
    assert_eq!(kind("D= ⟦1,1,1;1,1,1;1,1,1⟧", "@ker D"), "ker D = span{(1, −1, 0), (1, 0, −1)}、(x, y, z) = s(1, −1, 0) + t(1, 0, −1)、x + y + z = 0（2 次元。平面）");
    assert_eq!(kind("E= ⟦1,0;0,1⟧", "@ker E"), "ker E = {0}（0 次元。原点）");
    // 描けない次元でも、式は誤りの文に出す
    assert!(error_of("@ker F", "F= ⟦1,2,3,4;0,1,1,1⟧").starts_with("ker F = span{(1, 1, −1, 0), (2, 1, 0, −1)}"));
}

#[test]
fn non_square_matrix_definitions_draw_columns() {
    // 行が 2・3 なら、正方でなくても列ベクトルを描く
    let m = run("B= ⟦1,2,3;0,1,1⟧", "B= ⟦1,2,3;0,1,1⟧", &[]).unwrap();
    assert!(!m.positions().is_empty() || m.kind().contains("列ベクトル"));
    let m = run("H= ⟦1,2;0,1;1,1⟧", "H= ⟦1,2;0,1;1,1⟧", &[]).unwrap();
    assert!(!m.positions().is_empty() || m.kind().contains("列ベクトル"));
}

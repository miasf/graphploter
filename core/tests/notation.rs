//! 数学の記法（計画 `docs/plan-math-notation.md`）。画面と同じ入口（`graph_core::plot` と `definitions`）で確かめる

use graph_core::{definitions, ode_init, plot, MeshResult};

/// 3 次元・既定に近い範囲で描く。`defs` は式の一覧のほかの行（`\n` 区切り）、`params` は (名前, 値) の並び
fn run(src: &str, defs: &str, params: &[(&str, f64)]) -> Result<MeshResult, String> {
    run_flags(src, defs, params, 0)
}

fn run_flags(src: &str, defs: &str, params: &[(&str, f64)], flags: u32) -> Result<MeshResult, String> {
    run_in_view(src, defs, params, flags, false)
}

fn run_in_view(src: &str, defs: &str, params: &[(&str, f64)], flags: u32, flat: bool) -> Result<MeshResult, String> {
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
        flat,
        1,
        false,
        flags,
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
fn powers_of_function_calls() {
    same("z = sin^(2)(x) + cos^(3)(y)", "z = sin(x)^2 + cos(y)^3", "");
    same("z = erf^(-2)(x+2)", "z = erf(x+2)^(-2)", "");
    same("z = max^(2)(x,y)", "z = max(x,y)^2", "");
    same("z = f^(x+1)(y)^2", "z = ((y+2)^(x+1))^2", "f(t) = t+2");
    same("z = g^(3)(x,y)", "z = (x+y)^3", "g(s,t) = s+t");
    same("z = x^2(y)", "z = x^2*y", "");
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
fn limit_values_and_binding() {
    close("@lim(x,0) sin(x)/x", 1.0, 1e-9);
    close("@lim(x,0) (1-cos(x))/x^2", 0.5, 1e-12);
    close("@lim(x,0) (exp(x)-1-x)/x^2", 0.5, 1e-12);
    close("@lim(x,∞) (1+1/x)^x", std::f64::consts::E, 1e-8);
    close("@lim(x,1) (x^2-1)/(x-1)", 2.0, 1e-8);
    close("@lim(x,∞) (2x^2+1)/(x^2-1)", 2.0, 1e-8);
    close("@lim(x,-∞) exp(x)", 0.0, 1e-9);
    close("@lim(x,0,1) sign(x)", 1.0, 1e-9);
    close("@lim(x,0,-1) sign(x)", -1.0, 1e-9);
    close("@lim(x,0,1) sqrt(x)", 0.0, 1e-9);
    close("@lim(t,0) (@lim(t,0) cos(t) + t)", 1.0, 1e-9);
    close("@lim(t,0) (@lim(u,0) (t+u))", 0.0, 1e-9);

    let mut m = run("@lim(a,b) f(a)", "a = 99\nf(t) = t + b", &[("a", 99.0), ("b", 2.0)]).unwrap();
    assert_eq!(m.limit_variable(), "a");
    let result = m.limit_values();
    assert_eq!([result[0], result[1], result[2], result[4], result[6]], [2.0, 0.0, 1.0, 1.0, 4.0]);
    assert!([result[3], result[5], result[7]].iter().all(|v| (v - 4.0).abs() < 1e-9));
    assert_eq!(m.used_params(), "b");
    assert!(!m.take_limit_source().is_empty());
    same("z = @lim(t,0) sin(t)/t + x", "z = 1 + x", "");
}

#[test]
fn limits_compose_with_integrals_derivatives_and_definitions() {
    close("@lim(x,0) (@int(t,0,x) cos(t))/x", 1.0, 1e-8);
    close("@lim(x,∞) @int(t,0,x) exp(-t)", 1.0, 1e-8);
    close("@lim(x,∞) @int(t,0,x) exp(-t^2)", std::f64::consts::PI.sqrt() / 2.0, 1e-8);
    close("@lim(x,∞) @int(t,1,x) exp(-(t-x)^2)", std::f64::consts::PI.sqrt() / 2.0, 1e-8);
    close("@lim(x,∞) (@int(t,0,x) sin(t))/x", 0.0, 1e-8);
    let improper = run("@lim(x,0,1) @int(t,x,1) 1/t", "", &[]).unwrap();
    assert_eq!(improper.limit_values()[7], f64::INFINITY);
    close("@lim(x,1) @diff(x) (x^3)", 3.0, 1e-8);
    close("@lim(h,0) (@lim(x,h) x^2)", 0.0, 1e-8);
    let mut m = run("@lim(x,0) f'(x+a)", "f(x) = x^3", &[("a", 2.0)]).unwrap();
    assert!((m.limit_values()[7] - 12.0).abs() < 1e-8);
    assert_eq!(m.used_params(), "a");
    let samples = m.take_limit_samples();
    assert!(samples.len() >= 100 && samples.chunks_exact(2).all(|p| p[0] != 0.0));
    let integral = run("@lim(x,∞) f(x)", "f(x) = @int(t,0,x) exp(-t)", &[]).unwrap();
    assert!((integral.limit_values()[7] - 1.0).abs() < 1e-8);
    let nested = run("@lim(x,0) g(x+a)", "g(t) = @lim(u,t) sin(u)", &[("a", 0.3)]).unwrap();
    assert!((nested.limit_values()[7] - 0.3_f64.sin()).abs() < 1e-8);
}

#[test]
fn sequence_limits_use_integers_and_distinguish_persistent_oscillation() {
    close("@lim(n,∞) (-1)^n/n", 0.0, 1e-8);
    close("@lim(n,∞) 1/(n+1)", 0.0, 1e-8);
    close("@lim(n,∞) (1+1/n)^n", std::f64::consts::E, 1e-8);
    close("@sum(k,1,512) 1/2^k", 1.0, 1e-10);
    close("@lim(n,∞) @sum(k,1,n) 1/2^k", 1.0, 1e-8);
    let mut m = run("@lim(k,∞) (1+b_(k+1))", "b_(n) = 1/n^2", &[]).unwrap();
    assert!(m.limit_sequence());
    assert!((m.limit_values()[7] - 1.0).abs() < 1e-7);
    let samples = m.take_limit_samples();
    assert!(samples.chunks_exact(2).all(|p| p[0].fract() == 0.0));
    assert!(samples.chunks_exact(2).any(|p| p[0] == 4_096.0));
    let recur = run("@lim(n,∞) a_(n)", "a_(n+1) = (a_(n)+2)/2", &[("a1", 0.0)]).unwrap();
    assert!((recur.limit_values()[7] - 2.0).abs() < 1e-8);
    assert!(recur.used_params().contains("a1"));
    let later = run("@lim(n,∞) a_(n)", "a_(n+1) = (a_(n)+2)/2", &[("a20", 0.0)]).unwrap();
    assert!((later.limit_values()[7] - 2.0).abs() < 1e-8);
    let alias = run("@lim(k,∞) f(k)", "f(k) = a_(k)\na_(n) = 1/n", &[]).unwrap();
    assert!(alias.limit_sequence());
    assert!(alias.limit_values()[7].abs() < 1e-8);
    let oscillation = run("@seqlim(k,∞) (-1)^k", "", &[]).unwrap();
    assert_eq!(oscillation.limit_values()[4], 3.0);
    let divergent = run("@lim(n,∞) n", "", &[]).unwrap();
    assert_eq!(divergent.limit_values()[7], f64::INFINITY);
    let overflow = run("@lim(n,∞) 2^n", "", &[]).unwrap();
    assert_eq!(overflow.limit_values()[7], f64::INFINITY);
}

#[test]
fn limit_reports_divergence_and_uncertainty() {
    let result = |src| run(src, "", &[]).unwrap().limit_values();
    assert_eq!(&result("@lim(x,0) 1/x^2")[2..6], &[2.0, f64::INFINITY, 2.0, f64::INFINITY]);
    assert_eq!(&result("@lim(x,0) 1/x")[2..6], &[2.0, f64::NEG_INFINITY, 2.0, f64::INFINITY]);
    assert_eq!(result("@lim(x,0,-1) 1/x")[7], f64::NEG_INFINITY);
    assert_eq!(result("@lim(x,0,1) ln(x)")[7], f64::NEG_INFINITY);
    assert_eq!(result("@lim(x,∞) exp(x)")[7], f64::INFINITY);
    let oscillation = result("@lim(x,0) sin(1/x)");
    assert_eq!([oscillation[2], oscillation[4]], [3.0, 3.0]);
    assert!(oscillation[7].is_nan());
    assert_eq!(result("@lim(x,0) x sin(1/x)")[2], 1.0);
    assert_eq!(result("@lim(x,0) ((1-cos(x))/x^2+2)")[2], 0.0);
    assert_eq!(result("@lim(x,0,-1) sqrt(x)")[2], 0.0);
    assert!(error_of("z = @lim(x,0) 1/x", "").contains("一致しません"));
    assert!(error_of("z = @lim(x,0) sin(1/x)", "").contains("振動"));
    assert!(error_of("@lim(x,∞,1) x", "").contains("近づく先"));
    assert!(error_of("@lim(x,1/0) x", "").contains("近づく先"));
}

#[test]
fn standalone_limits_plot_the_finite_value() {
    let mut plane = run("@lim(x,1) 2x", "", &[]).unwrap();
    let integral = run("@int(t,0,2) 1", "", &[]).unwrap();
    assert!(!integral.positions().is_empty());
    assert_eq!(plane.positions(), integral.positions());
    assert_eq!(plane.indices(), integral.indices());
    assert!(plane.take_limit_source().chunks_exact(3).any(|p| p[0].abs() > 0.1 && p[2] == 0.0));

    for (src, y) in [("@lim(x,1) 2x", 2.0), ("@lim(x,0,1) abs(x)/x", 1.0), ("@lim(n,∞) (1+1/n)", 1.0)] {
        let m = run_in_view(src, "", &[], 0, true).unwrap();
        let ps = m.positions();
        assert_eq!(&ps, &[-3.0, y, 0.0, 3.0, y, 0.0], "{src}");
    }
    let mut sequence = run("@lim(n,∞) (1+1/n)", "", &[]).unwrap();
    assert!(!sequence.indices().is_empty());
    assert_eq!(&sequence.take_limit_source()[..3], &[1.0, 2.0, 0.0]);
    assert!(run("@lim(x,0) 1/x^2", "", &[]).unwrap().indices().is_empty());
}

#[test]
fn limit_graph_marks_both_sides_and_the_defined_value() {
    let mut hole = run_in_view("@lim(x,0) abs(x)/x", "", &[], 0, true).unwrap();
    assert_eq!(&hole.limit_values()[2..6], &[1.0, -1.0, 1.0, 1.0]);
    assert!(hole.limit_values()[6..].iter().all(|x| x.is_nan()));
    assert_eq!(hole.take_holes(), vec![0.0, -1.0, 0.0, 0.0, 1.0, 0.0]);
    assert!(hole.take_limit_point().is_empty());
    assert!(hole.positions().is_empty());
    let positions = hole.take_limit_source();
    assert!(positions.chunks_exact(3).any(|p| p[0] < 0.0 && p[1] == -1.0));
    assert!(positions.chunks_exact(3).any(|p| p[0] > 0.0 && p[1] == 1.0));
    assert!(positions.chunks_exact(3).collect::<Vec<_>>().windows(2).all(|p| {
        p[0][0].is_nan() || p[1][0].is_nan() || p[0][1] == p[1][1]
    }));

    let mut filled = run("@lim(x,0) @cases(-1, x < 0; 1, x > 0; 0)", "", &[]).unwrap();
    assert_eq!(filled.limit_values()[6], 0.0);
    assert_eq!(filled.take_holes(), vec![0.0, -1.0, 0.0, 0.0, 1.0, 0.0]);
    assert_eq!(filled.take_limit_point(), vec![0.0, 0.0, 0.0]);

    let mut endpoint = run("@lim(x,0) @cases(-1, x < 0; 1)", "", &[]).unwrap();
    assert_eq!(endpoint.take_holes(), vec![0.0, -1.0, 0.0]);
    assert_eq!(endpoint.take_limit_point(), vec![0.0, 1.0, 0.0]);
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
    // ベクトル場の線積分 ∮ (−y, x) · dr = 2π
    close("@intset((cos(t), sin(t)), 0 ≤ t ≤ 2pi; {vec})((-y, x))", 2.0 * PI, 1e-9);
    // 球面の面積 4π、ベクトル場の面積分 ∯ (x, y, z) · dS = 4π（外向きの法線）
    let sphere = "(sin(u) cos(v), sin(u) sin(v), cos(u)), 0 ≤ u ≤ pi, 0 ≤ v ≤ 2pi";
    close(&format!("@intset({sphere})(1)"), 4.0 * PI, 1e-6);
    close(&format!("@intset({sphere}; {{vec}})((x, y, z))"), 4.0 * PI, 1e-6);
    // ベクトル場の面積分 ∯ (x³, y³, z³) · dS = 12π/5（ガウスの発散定理で ∭ div F dV と同じ値）
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
fn two_component_sets_live_in_the_plane() {
    let defs = "D = ⦃(x,y)∣x^2+y^2<=1⦄\nE = ⦃(s,t)∣0<=s<=1,-1<=t<=1⦄";
    for (source, defs) in [
        ("D", defs),
        ("D ∖ E", defs),
        ("∅ ∪ D", defs),
        ("Eᶜ", defs),
        ("⦃(a,b)∣a^2+b^2<=1⦄", ""),
        ("⦃(z,x)∣z^2+x^2<=1⦄", ""),
        ("⦃(x,y)∣x^2+y^2<=1,z=0⦄", ""),
        ("@bigcup(k; 1<=k<=2) ⦃(x,y)∣(x-k)^2+y^2<=0.2⦄", ""),
    ] {
        for flat in [false, true] {
            let result = run_in_view(source, defs, &[], 0, flat).unwrap();
            assert!(!result.indices().is_empty(), "{source}");
            assert!(result.positions().chunks_exact(3).all(|p| p[2] == 0.0), "{source}");
        }
    }
    let disk = "⦃(x,y)∣x^2+y^2<=1⦄";
    let volume = "⦃(x,y,z)∣x^2+y^2<=1⦄";
    for source in [volume.to_string(), format!("{disk} ∪ {volume}")] {
        let result = run(&source, "", &[]).unwrap();
        assert!(result.positions().chunks_exact(3).any(|p| p[2].abs() > 2.5), "{source}");
    }
    assert!(run("⦃(x,y)∣x^2+y^2<=1,z>0⦄", "", &[]).unwrap().indices().is_empty());
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
    // ≠ は形を変えず、除いた所（球の中の x = 0 の円板の縁ではなく、領域の面の上の x = 0 の線）を点線で示す
    let mut m = run("x ≠ 0, x^2+y^2+z^2 ≤ 1", "", &[]).unwrap();
    assert!(!m.kind().contains("無視"), "{}", m.kind());
    assert_eq!(m.positions(), mesh("x^2+y^2+z^2 ≤ 1", "", &[]).0);
    assert!(m.take_outline_dashed().iter().filter(|v| v.is_finite()).count() > 30, "除いた線が無い");
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
    let (p, _) = mesh("⦃(t, t^2)∣t>=0⦄", "", &[]);
    assert!(p.chunks_exact(3).any(|v| v[0] > 1.0));
    assert!(p.chunks_exact(3).filter(|v| v[0].is_finite()).all(|v| v[0] >= -1e-5));
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
    let mut m = run("A = ⟦1, 0; 1, 2⟧", "A = ⟦1, 0; 1, 2⟧", &[]).unwrap();
    assert!(m.kind().contains("A の定義（列ベクトル）"), "{}", m.kind());
    assert!(m.vector_arrows());
    let p: Vec<_> = m.positions().chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
    // 矢じりだけがメッシュ、軸は原点から矢じりの底面までの線分
    assert!(p.iter().any(|v| *v == [1.0, 1.0, 0.0]) && p.iter().any(|v| *v == [0.0, 2.0, 0.0]));
    assert!(p.iter().all(|v| v[1] > 0.75 && v[2].abs() < 0.1));
    let shafts = m.take_outline();
    assert_eq!(shafts.len(), 18);
    assert_eq!(&shafts[..3], &[0.0; 3]);
    assert_eq!(&shafts[9..12], &[0.0; 3]);
    assert_eq!(&shafts[12..15], &[0.0, 1.79, 0.0]);
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
fn powered_sequence_terms() {
    let recurrence = "a_(n+1)^(2) = a_(n)+1";
    let init = [("a1", 3.0)];
    assert_eq!(seq_value("a_(2)", recurrence, &init).unwrap(), 2.0);
    let m = run(recurrence, recurrence, &init).unwrap();
    assert!(m.positions().chunks_exact(3).any(|p| p == [2.0, 2.0, 0.0]));
    assert!(m.kind().contains("根の主値"));
    let third = seq_value("b_(3)", "b_(n+2)^3 = b_(n) + n", &[("b1", 7.0), ("b2", 0.0)]).unwrap();
    assert!((third - 2.0).abs() < 1e-14);
}

#[test]
fn sequence_term_products() {
    let recurrence = "(a_(n+1))(a_(n))=6";
    let initial = [("a1", 2.0)];
    assert_eq!(seq_value("a_2", recurrence, &initial).unwrap(), 3.0);
    assert_eq!(seq_value("a_3", recurrence, &initial).unwrap(), 2.0);
    let plotted = run(recurrence, recurrence, &initial).unwrap();
    assert!(plotted.positions().chunks_exact(3).any(|p| p == [2.0, 3.0, 0.0]));
    assert!(seq_value("a_2", recurrence, &[("a1", 0.0)]).is_err());
    assert!(seq_value("a_2", "(a_(n+1))(a_(n))=0", &[("a1", 0.0)]).is_err());

    let reversed = "a_(n)*a_(n+1)=6";
    assert_eq!(seq_value("a_2", reversed, &initial).unwrap(), 3.0);
    let sparse = "a_(2n)*a_(2n+2)^2*a_2=36";
    assert_eq!(seq_value("a_4", sparse, &[("a2", 2.0)]).unwrap(), 3.0);
    let derived = "a_(n)=n+1\nb_(n)=(a_(n+1))(a_(n))";
    assert_eq!(seq_value("b_2", derived, &[]).unwrap(), 12.0);
    let product_recurrence = "a_(n+2)=(a_(n+1))(a_(n))";
    assert_eq!(seq_value("a_4", product_recurrence, &[("a1", 2.0), ("a2", 3.0)]).unwrap(), 18.0);
}

#[test]
fn divided_sequence_terms() {
    let recurrence = "((a_(n+1))/(2))=sin a_(n)+1";
    let initial = [("a1", 0.0)];
    assert_eq!(seq_value("a_2", recurrence, &initial).unwrap(), 2.0);
    assert!((seq_value("a_3", recurrence, &initial).unwrap() - 2.0 * (2.0_f64.sin() + 1.0)).abs() < 1e-12);
    let plot = run(recurrence, recurrence, &initial).unwrap();
    assert!(plot.positions().chunks_exact(3).any(|p| p == [2.0, 2.0, 0.0]));

    let previous = "a_(n+1)/a_(n)=3";
    assert_eq!(seq_value("a_3", previous, &[("a1", 2.0)]).unwrap(), 18.0);
    assert!(seq_value("a_2", previous, &initial).is_err());
    assert!(seq_value("a_2", "a_(n+1)/0=1", &initial).is_err());
    assert_eq!(seq_value("a_4", "a_(n+1)/(n-2)=1", &[]).unwrap(), 1.0);
    assert!(seq_value("a_3", "a_(n+1)/(n-2)=1", &[]).is_err());
    assert_eq!(seq_value("a_2", "a_(n+1)^2/2=8", &initial).unwrap(), 4.0);
}

#[test]
fn recurrence_operations_and_initial_values() {
    let given = [("a1", 2.0)];
    assert_eq!(seq_value("a_3", "a_(n+1) = a_(n)^2 + a_1", &given).unwrap(), 38.0);
    assert!((seq_value("a_2", "a_(n+1) = log(a_(n)) + a_1", &given).unwrap() - (2.0_f64.log10() + 2.0)).abs() < 1e-12);
    let given = [("a1", 0.5)];
    assert_eq!(seq_value("a_2", "log(a_(n+1)) = a_(n) + a_1", &given).unwrap(), 10.0);
    assert!((seq_value("a_2", "ln(a_(n+1)^2) = a_(n) + a_1", &given).unwrap() - 0.5_f64.exp()).abs() < 1e-12);
    // 多くの項をたどっても、呼び出しの深さには依存しない。
    assert_eq!(seq_value("a_1500", "a_(n+1) = a_(n) + a_1", &[("a1", 2.0)]).unwrap(), 3000.0);
    let overflow = seq_value("a_100", "a_(n+1) = a_(n)^2 + a_(n)", &[("a1", 2.0)]).unwrap_err();
    assert_eq!(overflow, "実数の値になりません");
}

#[test]
fn recurrence_cases_skip_unused_terms() {
    let rule = "a_(n) = @cases(1, n=0; a_(n-1)+2)";
    assert_eq!(seq_value("a_5", rule, &[]).unwrap(), 11.0);
    assert_eq!(seq_value("a_0", "a_(n) = @cases(2, n=0; a_(n))", &[]).unwrap(), 2.0);
    let plot = run(rule, rule, &[]).unwrap();
    assert!(plot.positions().chunks_exact(3).any(|p| p == [0.0, 1.0, 0.0]));
}

#[test]
fn sequence_table_uses_the_same_values_as_the_plot() {
    let rule = "a_(2n) = a_(n) + a_1";
    let params = [("a1", 3.0)];
    let plotted = run(rule, rule, &params).unwrap();
    assert!(graph_core::take_sequence_table().is_empty());
    let inspected = run_flags(rule, rule, &params, 32).unwrap();
    assert!(plotted.positions().iter().map(|x| x.to_bits()).eq(inspected.positions().iter().map(|x| x.to_bits())));
    let rows = graph_core::take_sequence_table();
    assert_eq!(&rows[..3], &[1.0, 3.0, 0.0]);
    assert!(rows[3].is_nan());
    assert_eq!(&rows[4..], &[2.0, 6.0, 0.0, 1.0]);

    let complex = "b_(n) = n + 2i";
    run_flags(complex, complex, &[], 32).unwrap();
    assert_eq!(&graph_core::take_sequence_table()[4..8], &[1.0, 1.0, 2.0, 1.0]);
}

#[test]
fn sequence_cobweb_uses_consecutive_terms() {
    let rule = "a_(n+1)=1+r(a_(n)-1)";
    let params = [("a1", 2.0), ("r", -0.5), ("N", 2.0)];
    let m = run("@cobweb(a_1,N)", rule, &params).unwrap();
    assert!(m.kind().contains("階段図（2 回の反復）"));
    assert_eq!(m.positions(), vec![2.0, 2.0, 0.0, 2.0, 0.5, 0.0, 0.5, 0.5, 0.0, 0.5, 1.25, 0.0, 1.25, 1.25, 0.0]);
    for param in ["a1", "r", "N"] {
        assert!(m.used_params().split(',').any(|p| p == param));
    }
    let fixed = run("@cobweb(a_(0),512)", "a_(n)=1", &[]).unwrap();
    assert_eq!(&fixed.positions()[..3], &[1.0, 1.0, 0.0]);
    assert!(error_of("@cobweb(a_1,513)", rule).contains("1〜512"));
    assert!(error_of("@cobweb(a_1,2)", "a_(n)=i").contains("有限の実数"));
    assert!(error_of("@cobweb(a_1,2)", "").contains("数列 a の定義"));
}

#[test]
fn sequence_orbit_uses_the_same_step_for_each_coordinate() {
    let rules = "a_(n+1)=-b_(n)\nb_(n+1)=a_(n)\nc_(n)=n";
    let params = [("a0", 2.0), ("b0", 1.0), ("N", 2.0)];
    let m = run("@orbit(a_0,b_0,c_(1),N)", rules, &params).unwrap();
    assert!(m.kind().contains("軌道（3 次元・2 回の反復）"));
    let points = m.positions();
    assert_eq!(&points[..9], &[2.0, 1.0, 1.0, -1.0, 2.0, 2.0, -2.0, -1.0, 3.0]);
    assert!(points[9..12].iter().all(|p| p.is_nan()));
    assert_eq!(&points[12..], &[-2.0, -1.0, 3.0]);
    for param in ["a0", "b0", "N"] {
        assert!(m.used_params().split(',').any(|p| p == param));
    }
    for name in ["a", "b", "c"] {
        assert!(m.used_defs().split(',').any(|d| d == name));
    }
    let start = run("@orbit(a_0,b_0,0)", rules, &params).unwrap();
    assert_eq!(&start.positions()[..3], &[2.0, 1.0, 0.0]);
    let lag = run("@orbit(c_0,c_1,1)", rules, &[]).unwrap();
    assert_eq!(&lag.positions()[..6], &[0.0, 1.0, 0.0, 1.0, 2.0, 0.0]);
    assert!(error_of("@orbit(a_0,b_0,c_0,d_0,2)", rules).contains("1〜3 個"));
    assert!(error_of("@orbit(a_0,b_0,513)", rules).contains("0〜512"));
    assert!(error_of("@orbit(a_0,b_0,1)", "a_(n)=i\nb_(n)=n").contains("有限の実数"));
}

#[test]
fn sequence_orbit_can_iterate_a_matrix_from_an_initial_point() {
    let m = run("@orbit(A,(s,2),2)", "A=⟦0,-1;1,0⟧", &[("s", 1.0)]).unwrap();
    assert_eq!(&m.positions()[..9], &[1.0, 2.0, 0.0, -2.0, 1.0, 0.0, -1.0, -2.0, 0.0]);
    assert_eq!(m.used_params(), "s");
    assert_eq!(m.used_defs(), "A");
    assert!(error_of("@orbit(A,(1,2,3),2)", "A=⟦0,-1;1,0⟧").contains("正方行列"));
}

#[test]
fn sequence_orbit_number_line_and_scatter() {
    let rules = "a_(n+1)=-a_(n)/2";
    let params = [("a0", 2.0)];
    let hops = run("@orbit(a_0,2)", rules, &params).unwrap().positions();
    let hops: Vec<_> = hops.chunks_exact(3).collect();
    assert_eq!(hops[0], &[2.0, 0.0, 0.0]);
    assert!(hops[6][1] < 0.0);
    assert_eq!(hops[12], &[-1.0, 0.0, 0.0]);
    assert!(hops[20][1] > 0.0);
    assert_eq!(hops.last().unwrap(), &&[0.5, 0.0, 0.0]);

    let dots = run("@orbitpoints(a_0,a_1,2)", rules, &params).unwrap().positions();
    assert_eq!(&dots[..6], &[2.0, -1.0, 0.0, 2.0, -1.0, 0.0]);
    assert!(dots[6..9].iter().all(|p| p.is_nan()));
    assert_eq!(&dots[9..12], &[-1.0, 0.5, 0.0]);
    assert!(error_of("@orbitpoints(a_0,8193)", rules).contains("8192"));
}

#[test]
fn complex_sequence_orbit_uses_real_and_imaginary_coordinates() {
    let rules = "Z_(n+1)=i*Z_(n)";
    let init = run("Z_0=2+i", rules, &[]).unwrap();
    assert_eq!(init.param_name(), "Z0");
    assert_eq!(init.param_value(), vec![2.0, 1.0]);
    let params = [("Z0", 2.0), ("N", 2.0)];
    let orbit = run_in_view("@orbit(Z_0,N)", rules, &params, 0, true).unwrap();
    assert!(orbit.kind().contains("複素平面・2 回"));
    let positions = orbit.positions();
    assert_eq!(&positions[..9], &[2.0, 0.0, 0.0, 0.0, 2.0, 0.0, -2.0, 0.0, 0.0]);
    assert!(positions[9..11].iter().all(|p| p.is_nan()));
    assert_eq!(&positions[12..], &[-2.0, 0.0, 0.0]);
    assert!(orbit.used_params().split(',').any(|p| p == "Z0"));
    let dots = run("@orbitpoints(Z_0,N)", rules, &params).unwrap().positions();
    assert_eq!(&dots[9..12], &[0.0, 2.0, 0.0]);
    assert_eq!(&dots[18..21], &[-2.0, 0.0, 0.0]);
}

#[test]
fn recurrence_network_joins_similar_terms_without_joining_different_groups() {
    let rules = "a_(n)=mod(n,2)";
    let m = run("@recurrence(a_1,4,d)", rules, &[("d", 0.0)]).unwrap();
    assert!(m.kind().contains("4 頂点・2 辺"));
    assert_eq!(m.used_params(), "d");
    let points = m.positions();
    let runs: Vec<_> = points.chunks_exact(3).collect::<Vec<_>>()
        .split(|p| p[0].is_nan()).filter(|run| !run.is_empty()).map(|run| run.to_vec()).collect();
    let edges: Vec<_> = runs.iter().filter(|run| run.len() > 1)
        .map(|run| (run[0], *run.last().unwrap())).collect();
    assert_eq!(edges, vec![(&[1.0, 1.0, 0.0][..], &[3.0, 1.0, 0.0][..]), (&[2.0, 0.0, 0.0][..], &[4.0, 0.0, 0.0][..])]);
    assert!(error_of("@recurrence(a_1,4,-1)", rules).contains("0 以上"));
}

#[test]
fn complex_recurrence_network_uses_both_parts_for_distance() {
    let rules = "a_(n)=i*mod(n,2)";
    let graph = run("@recurrence(a_0,4,0.5)", rules, &[]).unwrap();
    assert!(graph.kind().contains("4 頂点・2 辺"));
    assert!(graph.positions().chunks_exact(3).any(|p| p == [1.0, 0.0, 1.0]));
}

#[test]
fn bifurcation_restarts_each_column_then_plots_the_remaining_orbit() {
    let rules = "F(x,r)=r*x*(1-x)+a";
    let m = run("@bifurcation(F,0.2,2.8,3.2)", rules, &[("a", 0.0)]).unwrap();
    assert!(m.kind().contains("分岐図"));
    assert_eq!(m.used_params(), "a");
    assert_eq!(m.used_defs(), "F");
    let points = m.positions();
    let samples: Vec<_> = points.chunks_exact(9).map(|p| &p[..3]).collect();
    let left: Vec<_> = samples.iter().filter(|p| p[0] == 2.8).collect();
    let right: Vec<_> = samples.iter().filter(|p| p[0] == 3.2).collect();
    assert_eq!(left.len(), 1);
    assert!((left[0][1] - (1.0 - 1.0 / 2.8)).abs() < 1e-6);
    assert_eq!(right.len(), 2);
    assert!(right.iter().all(|p| p[2] == 0.0));
    assert!(error_of("@bifurcation(F,0.2,4,3)", rules).contains("下限"));
}

#[test]
fn recurrence_index_rules_and_dependencies() {
    let even = "a_(2n) = a_(n) + a_1";
    let odd = "a_(2n+1) = a_(n)^2 + a_1";
    let both = format!("{even}\n{odd}");
    let given = [("a1", 2.0)];
    assert_eq!(seq_value("a_6", &both, &given).unwrap(), 8.0);
    assert_eq!(seq_value("a_(a_2)", &both, &given).unwrap(), 6.0);
    assert_eq!(seq_value("b_125", "b_(2(2n+1)+3) = n", &[]).unwrap(), 30.0);
    assert_eq!(seq_value("a_4", even, &given).unwrap(), 6.0);
    assert_eq!(seq_value("a_2", "a_(n) = a_(2n) + 1", &[("a8", 3.0)]).unwrap(), 5.0);
    assert_eq!(seq_value("a_7", "a_(n+1) = a_(floor(n/2)) + a_1", &[("a0", 1.0), ("a1", 2.0)]).unwrap(), 6.0);
    let missing = seq_value("a_6", even, &given).unwrap_err();
    assert!(missing.contains("初項 a_3"), "{missing}");
    assert!(seq_value("a_(3/2)", &both, &given).unwrap_err().contains("整数"));
    let plot = run(even, even, &given).unwrap();
    assert!(plot.positions().chunks_exact(3).any(|p| p == [2.0, 4.0, 0.0]));
    let runaway = seq_value("a_4", "a_(n) = a_(n+1) + a_(n-1)", &given).unwrap_err();
    assert!(runaway.contains("必要な項が多すぎ"), "{runaway}");
    assert!(seq_value("a_4", "a_(n) = a_(n)", &given).unwrap_err().contains("循環"));
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
    // ベクトル場の線積分: 単位円を 1 周 → 2π（点を入れる形と、名前だけの形）
    assert!((value_with("@intset(( cos t, sin t,0),0<=t<=2 pi; {vec})({vec_F})", defs) - 2.0 * pi).abs() < 1e-9);
    assert!((value_with("@intset(( cos t, sin t,0),0<=t<=2 pi; {vec})({vec_F} (x,y,z))", defs) - 2.0 * pi).abs() < 1e-9);
    // ベクトル場の面積分: 単位球面の上の ∯ G · dS（G = r、外向き）は 4π
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
    // この名前を明示的に入力変数として宣言すれば、表示範囲に応じて動かせる。
    same("⦃(s, t, s)∣0<=s<=1⦄", "⦃(a, b, a)∣0<=a<=1,b∈ℝ⦄", "");
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
    // 1 次式にならない等式（円・球面）も、点を数値で探して積分する（implicit_sets_without_parameters）
    close("@intset((x,y),x^2+y^2=1; {d_s})(1)", 2.0 * std::f64::consts::PI, 1e-9);
}

/// 1 次式に解けない等式で書いた曲線・曲面（集合の書き方 {(x, y) | x² + y² = 1}）の上の積分。媒介変数で書いたものと同じ値
#[test]
fn implicit_sets_without_parameters() {
    let pi = std::f64::consts::PI;
    // 平面の曲線: 円周・楕円の周（媒介変数で書いた値）・∫ x² ds・向き（{F < 0} を左に見る = 反時計回り）
    close("@intset((x,y),x^2+y^2=1; {d_s})(x^2)", pi, 1e-9);
    close("@intset((x,y),x^2/4+y^2=1; {d_s})(1)", value("@intset((2cos(t), sin(t)),0<=t<=2pi; {d_s})(1)"), 1e-9);
    close("@intset((x,y),x^2+y^2=1; {vec},{d_r})((-y,x))", 2.0 * pi, 1e-9);
    close("@intset((x,y),1=x^2+y^2; {vec},{d_r})((-y,x))", -2.0 * pi, 1e-9);
    close("@intset((x,y),x^2+y^2=1; {d_x})(-y)", pi, 1e-9);
    // 不等式で切った弧（座標の端・斜めの端）
    close("@intset((x,y),x^2+y^2=1, y>=0; {d_s})(1)", pi, 1e-9);
    close("@intset((x,y),x^2+y^2=1, x+y>=1; {d_s})(1)", pi / 2.0, 1e-7);
    // 曲面: 球面の面積・外向きの法線での F · dS（ガウスの定理の値 ∭ div F dV）・半球・円柱の側面（斜めに切る）
    close("@intset((x,y,z),x^2+y^2+z^2=1; {d_S})(1)", 4.0 * pi, 1e-7);
    close("@intset((x,y,z),x^2+y^2+z^2=1; {vec},{d_S})((x^3,y^3,z^3))", 12.0 * pi / 5.0, 1e-7);
    close("@intset((x,y,z),x^2+y^2+z^2=1, z>=0; {d_S})(1)", 2.0 * pi, 1e-7);
    close("@intset((x,y,z),x^2+y^2=1, 0<=z<=2-x; {d_S})(1)", 4.0 * pi, 1e-6);
    // トーラス（R = 2, r = 1）の面積 4π²Rr
    close("@intset((x,y,z),(x^2+y^2+z^2+3)^2=16(x^2+y^2); {d_S})(1)", 8.0 * pi * pi, 1e-4);
    // 頂点（特異点）のある円錐の側面 π√2
    close("@intset((x,y,z),x^2+y^2=z^2, 0<=z<=1; {d_S})(1)", pi * 2f64.sqrt(), 1e-3);
    // 空間の曲線: 1 次式に解ける等式が 1 つ（z = 1）・解けない等式が 2 つ（球面と円柱の交わり = 2 つの円）
    close("@intset((x,y,z),x^2+y^2=1, z=1; {d_s})(1)", 2.0 * pi, 1e-9);
    close("@intset((x,y,z),x^2+y^2+z^2=2, x^2+y^2=1; {d_s})(1)", 4.0 * pi, 1e-7);
    // 被積分関数にほかの変数（外側の積分の変数）があっても、点ごとの和にする: ∫₀¹ ∮ a x² ds da = π/2
    close("@int(a,0,1)(@intset((x,y),x^2+y^2=1; {d_s})(a x^2))", pi / 2.0, 1e-9);
    // 限りなく続く曲線・見つからない曲線・点になる範囲
    let err = |s: &str| graph_core::equation::eval_constant(s).unwrap_err();
    assert!(err("@intset((x,y),x^2-y^2=1; {d_s})(1)").contains("限りなく"));
    assert!(err("@intset((x,y),x^2+y^2=-1; {d_s})(1)").contains("見つかりません"));
    assert!(err("@intset((x,y),x^2+y^2=1, x^2-y^2=0; {d_s})(1)").contains("点"));
    assert!(err("@intset((x,y,z),x^2+y^2+z^2=1; {d_s})(1)").contains("dS"));
}

/// 等式で書いた曲線・曲面の上の積分の図（画面側の latex.ts の implicitView が作る集合）が、図形になること
#[test]
fn implicit_integral_drawings() {
    // 平面の曲線 x² + y² = 1 の上の幕、球面を法線の向きに厚くした殻、球面と円柱の交わり（空間の曲線）
    for src in [
        "⦃(x,y,z)∣x^(2)+y^(2)=1, min (0,x^(2))<=z<= max (0,x^(2))⦄",
        "⦃(x,y,z)∣min (0,1)<=(((x^(2)+y^(2)+z^(2))-(1))/(abs(( @diff(x) ((x^(2)+y^(2)+z^(2))-(1)), @diff(y) ((x^(2)+y^(2)+z^(2))-(1)), @diff(z) ((x^(2)+y^(2)+z^(2))-(1))))))<= max (0,1)⦄",
        "⦃(x,y,z)∣x^(2)+y^(2)+z^(2)=2,x^(2)+y^(2)=1⦄",
    ] {
        let (p, _) = mesh(src, "", &[]);
        assert!(!p.is_empty(), "{src} が空");
    }
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
    // t ∈ [0, 1] は 0 ≤ t ≤ 1。≠ で切った枝も描ける。
    same("⦃(t,t^(2))∣t∈[0,1]⦄", "⦃(t,t^(2))∣0<=t<=1⦄", "");
    let (p, _) = mesh("⦃(t,t^(2))∣-1<=t<=1,t≠0⦄", "", &[]);
    assert!(p.chunks_exact(3).any(|v| v[0] < -0.5));
    assert!(p.chunks_exact(3).any(|v| v[0] > 0.5));
    // 端がほかの媒介変数の式（扇形の三角形 0 ≤ t ≤ s）も描ける
    let (p, _) = mesh("⦃(s cos(t),s sin(t))∣0<=s<=1,0<=t<=s⦄", "", &[]);
    assert!(!p.is_empty());
    // 媒介変数で書いた集合どうしの ∪（折れ線の経路）は、それぞれ描いて重ねる
    let (p1, _) = mesh("⦃(t,t^(2))∣-1<=t<=1⦄", "", &[]);
    let (p2, _) = mesh("⦃(t,t^(2))∣-1<=t<=1⦄ ∪ ⦃(t,1)∣-1<=t<=1⦄", "", &[]);
    assert!(p2.len() > p1.len());
}

#[test]
fn set_images_map_the_condition_domain() {
    let radius = (3.0_f64.sqrt() / 2.0).sqrt() as f32;
    let extrema = |positions: &[f32], axis: usize| {
        positions.chunks_exact(3).map(|p| p[axis]).filter(|v| v.is_finite())
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)))
    };
    let definition = "D = ⦃(2x,y)∣x^2+y^2<=sqrt(3)/2⦄";
    let hemisphere = "D = ⦃(x,y,sqrt(1-x^2-y^2))∣x^2+y^2<=sqrt(3)/2⦄";
    for (source, defs) in [(definition, ""), ("D", definition), (definition, hemisphere)] {
        for flat in [false, true] {
            let result = run_in_view(source, defs, &[], 0, flat).unwrap();
            let p = result.positions();
            assert!(!result.indices().is_empty());
            let (x_min, x_max) = extrema(&p, 0);
            let (y_min, y_max) = extrema(&p, 1);
            assert!((x_min + 2.0 * radius).abs() < 0.03 && (x_max - 2.0 * radius).abs() < 0.03, "{x_min}, {x_max}");
            assert!((y_min + radius).abs() < 0.03 && (y_max - radius).abs() < 0.03, "{y_min}, {y_max}");
            assert!(p.chunks_exact(3).all(|p| p[2] == 0.0));
        }
    }
    let dome = run(hemisphere, definition, &[]).unwrap();
    let positions = dome.positions();
    assert!(!dome.indices().is_empty());
    let (lo, hi) = extrema(&positions, 2);
    let rim = (1.0 - radius * radius).sqrt();
    assert!((lo - rim).abs() < 0.02 && (hi - 1.0).abs() < 0.02, "{lo}, {hi}");
    assert!(positions.chunks_exact(3).all(|p| (p[0]*p[0] + p[1]*p[1] + p[2]*p[2] - 1.0).abs() < 1e-5));

    let p = mesh("D = ⦃sqrt(1-x^2-y^2)∣x^2+y^2<=sqrt(3)/2⦄", "", &[]).0;
    let (lo, hi) = extrema(&p, 0);
    assert!((lo - (1.0 - radius * radius).sqrt()).abs() < 0.02 && (hi - 1.0).abs() < 0.02, "{lo}, {hi}");
    assert!(p.chunks_exact(3).filter(|p| p[0].is_finite()).all(|p| p[1] == 0.0 && p[2] == 0.0));

    let p = mesh("⦃2s∣s∈[-2,-1]∨s∈[1,2]⦄", "", &[]).0;
    assert!(p.iter().any(|v| v.is_nan()) && p[0] < -3.9);
    assert!(p.chunks_exact(3).filter(|p| p[0].is_finite()).all(|p| p[0].abs() >= 1.99));
    assert!(p.chunks_exact(3).any(|p| p[0] > 3.9));

    let p = mesh("⦃(x,y,x^2-y^2)∣x^2+y^2=1,y>=0⦄", "", &[]).0;
    assert!(p.len() > 30);
    assert!(p.chunks_exact(3).filter(|p| p[0].is_finite()).all(|p| {
        p[1] >= -0.01 && (p[0]*p[0] + p[1]*p[1] - 1.0).abs() < 0.01
            && (p[2] - p[0]*p[0] + p[1]*p[1]).abs() < 1e-5
    }));

    let member = "(x,y)∈⦃(2s+1,t)∣s^2+t^2<=1⦄";
    let p = run_in_view(member, "", &[], 0, true).unwrap().positions();
    let (lo, hi) = extrema(&p, 0);
    assert!((lo + 1.0).abs() < 0.03 && (hi - 3.0).abs() < 0.03, "{lo}, {hi}");
}

#[test]
fn image_sets_share_one_path_for_ranges_logic_and_discontinuities() {
    let (p, _) = mesh("⦃(s,t,s-t)∣0<=s<=60,0<=2t<=s⦄", "", &[]);
    assert!(p.chunks_exact(3).any(|q| q[1] > 29.0));

    let collapsed = run("⦃(s,s,2s)∣s∈ℝ,s^2+t^2<=1⦄", "", &[]).unwrap();
    let p = collapsed.positions();
    assert!(collapsed.indices().is_empty());
    assert!(p.chunks_exact(3).any(|q| q[2] > 1.9));
    assert!(p.chunks_exact(3).any(|q| q[2] < -1.9));

    let (p, _) = mesh("⦃(s, abs(s), s^3)∣s∈[-2,2], s^2>=1, s<1.8⦄", "", &[]);
    assert!(p.chunks_exact(3).any(|q| q[0] < -1.5));
    assert!(p.chunks_exact(3).any(|q| q[0] > 1.5));
    assert!(p.chunks_exact(3).filter(|q| q[0].is_finite()).all(|q| {
        q[0].abs() >= 0.998 && q[0] <= 1.801
            && (q[1] - q[0].abs()).abs() < 1e-5 && (q[2] - q[0].powi(3)).abs() < 1e-5
    }));

    for (source, gap) in [("⦃1/s∣-1<=s<=1⦄", 0.99), ("⦃floor(s)∣-1.5<=s<=1.5⦄", 0.0)] {
        let (p, _) = mesh(source, "", &[]);
        assert!(p.chunks_exact(3).any(|q| q[0] < -gap));
        assert!(p.chunks_exact(3).any(|q| q[0] > gap));
        for segment in p.chunks_exact(3).collect::<Vec<_>>().windows(2) {
            let (a, b) = (segment[0][0], segment[1][0]);
            if a.is_finite() && b.is_finite() {
                if gap > 0.0 {
                    assert!(a * b > 0.0 && a.abs() >= gap && b.abs() >= gap, "{source}: {a}, {b}");
                } else {
                    assert_eq!(a, b, "{source}: two isolated values are joined");
                }
            }
        }
    }

    let mut both = run("⦃(s,s^2,0)∣-1<=s<=1⦄ ∪ ⦃(u,v,2+u^2-v^2)∣u^2+v^2<=1⦄", "", &[]).unwrap();
    assert!(!both.indices().is_empty());
    // 曲線の部分は、面と一緒に描く添えの線（#121）
    assert!(both.take_extra().chunks_exact(3).any(|q| q[0].is_finite() && q[2] == 0.0));
}

#[test]
fn image_sets_offer_sliders_for_free_parameters() {
    for (source, name) in [
        ("D=⦃(ax,y)∣x^2+y^2<=1⦄", "a"),
        ("D=⦃(2x,y)∣x^2+y^2<=a⦄", "a"),
        ("D=⦃(x,a)∣-1<=x<=1⦄", "a"),
        ("D=⦃(s,a)∣s∈[-1,1]⦄", "a"),
        ("D=⦃(s,a s^2)∣0<=s<=1,a>0⦄", "a"),
        ("D=⦃(s,s^2)∣0<=s<={alpha}⦄", "alpha"),
    ] {
        assert!(error_of(source, "").starts_with(&format!("変数 '{name}' はここでは使えません")), "{source}");
        let result = run(source, "", &[(name, 2.0)]).unwrap();
        assert!(result.used_params().split(',').any(|n| n == name), "{source}");
    }
    let (p, _) = mesh("D=⦃(x,a)∣-1<=x<=1⦄", "", &[("a", 2.0)]);
    assert!(p.chunks_exact(3).any(|q| q[0].is_finite()));
    assert!(p.chunks_exact(3).filter(|q| q[0].is_finite()).all(|q| q[1] == 2.0));
}

#[test]
fn image_sets_keep_smooth_boundaries_and_normals() {
    let dome = "⦃(x,y,sqrt(1-x^2-y^2))∣x^2+y^2<=0.8⦄";
    let boundary_error = |m: &MeshResult| {
        let mut edges = std::collections::HashMap::<(usize, usize), usize>::new();
        for t in m.indices().chunks_exact(3) {
            for i in 0..3 {
                let (a, b) = (t[i] as usize, t[(i + 1) % 3] as usize);
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let p = m.positions();
        edges.into_iter().filter(|(_, count)| *count == 1).map(|((a, b), _)| {
            let x = 0.5 * (p[3*a] + p[3*b]);
            let y = 0.5 * (p[3*a+1] + p[3*b+1]);
            (x*x + y*y - 0.8).abs()
        }).fold(0.0, f32::max)
    };
    let fine = run(dome, "", &[]).unwrap();
    let coarse = run_flags(dome, "", &[], 1).unwrap();
    assert!(boundary_error(&fine) < 0.0015, "{}", boundary_error(&fine));
    assert!(boundary_error(&fine) < 0.4 * boundary_error(&coarse));
    assert!(2 * fine.indices().len() > 3 * fine.positions().len());
    for (p, n) in fine.positions().chunks_exact(3).zip(fine.normals().chunks_exact(3)) {
        assert!(p.iter().zip(n).map(|(a, b)| a * b).sum::<f32>() > 0.98, "{p:?}, {n:?}");
    }

    let surface = "⦃(x,y,sin(30x) cos(20y))∣-1<=x<=1,-1<=y<=1⦄";
    let fine = run(surface, "", &[]).unwrap();
    let coarse = run_flags(surface, "", &[], 1).unwrap();
    assert!(fine.indices().len() > coarse.indices().len() * 2);
    let curve = "⦃(t,sin(25t))∣-1<=t<=1⦄";
    let fine = run(curve, "", &[]).unwrap();
    let coarse = run_flags(curve, "", &[], 1).unwrap();
    assert!(fine.positions().len() > coarse.positions().len());
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

#[test]
fn im_is_image_only_for_real_matrices() {
    // 行全体が Im(実数の行列（列が 2 つ以上）) なら像。ベクトル・複素数の成分・式の中は虚部（成分ごと）
    assert!(run("Im A", "A= ⟦1,2;2,4⟧", &[]).unwrap().kind().starts_with("Im A = span{(1, 2)}"));
    same("Im {vec_b}", "(1, 0, 3)", "{vec_b}=(1+i,2,3i)");
    same("z = Im((x + i y)^(2))", "z = 2 x y", "");
}

#[test]
fn eigenvalues_and_eigenspaces() {
    let kind = |defs: &str, src: &str| run(src, defs, &[]).unwrap().kind().to_string();
    // 固有値ごとの固有空間（重複度・対角化できないこと・実数でない固有値も）
    assert_eq!(
        kind("A= ⟦2,1;1,2⟧", "@eig A"),
        "A の固有値 ／ λ = 1: E(1) = span{(1, −1)}、(x, y) = t(1, −1)、x + y = 0 ／ λ = 3: E(3) = span{(1, 1)}、(x, y) = t(1, 1)、x − y = 0"
    );
    assert!(kind("B= ⟦1,1;0,1⟧", "@eig B").contains("λ = 1（2 重）: E(1) = span{(1, 0)}") && kind("B= ⟦1,1;0,1⟧", "@eig B").contains("対角化できない"));
    assert!(kind("C= ⟦0,-1;1,0⟧", "@eig C").contains("λ = ±i（実数でないので固有空間は描かない）"));
    assert!(kind("D= ⟦2,0,0;0,2,0;0,0,3⟧", "@eig D").contains("λ = 2（2 重）: E(2) = span{(1, 0, 0), (0, 1, 0)}"));
    assert!(error_of("@eig J", "J= ⟦1,2,3;4,5,6⟧").contains("正方行列"));
    // 決まった固有値の固有空間 ker(A − λI)。単位行列は I か E
    assert_eq!(kind("A= ⟦2,1;1,2⟧", "@ker(A-3I)"), "ker (A − 3I) = span{(1, 1)}、(x, y) = t(1, 1)、x − y = 0（1 次元。直線）");
    assert_eq!(kind("A= ⟦2,1;1,2⟧", "@ker(1E - A)"), "ker (1E − A) = span{(1, −1)}、(x, y) = t(1, −1)、x + y = 0（1 次元。直線）");
    assert!(error_of("@ker(A - 3)", "A= ⟦2,1;1,2⟧").contains("単位行列は I か E"));
    // 対角化・ジョルダン標準形（文書 8 の LA1）: 今の道では固有値と固有空間（形は厳密値の道が出す）
    assert_eq!(kind("A= ⟦2,1;1,2⟧", "@diagonalize A"), kind("A= ⟦2,1;1,2⟧", "@eig A"));
    assert_eq!(kind("B= ⟦1,1;0,1⟧", "@jordan(B)"), kind("B= ⟦1,1;0,1⟧", "@eig B"));
    // グラム・シュミット: 正規直交にしたベクトル（1 次結合のベクトルは除く）
    assert_eq!(kind("", "@gramschmidt((3,4),(1,0))"), "グラム・シュミットの正規直交化 ／ u₁ = (0.6, 0.8) ／ u₂ = (0.8, −0.6)");
    assert!(kind("", "@gramschmidt((1,1,1),(2,2,2),(1,0,0))").ends_with("1 本は前のベクトルの 1 次結合なので除いた"));
    assert!(kind("", "@gramschmidt(⟦1,1;0,1⟧)").contains("u₂ = (0, 1)"));
    assert!(error_of("@jordan(A) + 1", "A= ⟦2,1;1,2⟧").contains("1 行にそれだけ"));
}

#[test]
fn flat_eigenvectors_have_triangle_heads_and_line_shafts() {
    let mut m = run_in_view("@eig(A)", "A=⟦2,0;0,3⟧", &[], 0, true).unwrap();
    assert!(m.vector_arrows());
    assert!(m.take_complex_values().is_empty());
    assert_eq!(m.indices().len(), 6);
    assert!(m.positions().chunks_exact(3).all(|p| p[2] == 0.0 && p[0].hypot(p[1]) > 1.0));
    let shafts = m.take_outline();
    assert_eq!(shafts.len(), 18);
    assert_eq!(&shafts[..3], &[0.0; 3]);
    assert_eq!(&shafts[9..12], &[0.0; 3]);
}

/// 積分する領域の条件の ∧・∨ は、行全体を論理式にしない（値の平面として描く。#114）
#[test]
fn logic_inside_integral_regions() {
    let pi = std::f64::consts::PI;
    for (src, want) in [
        ("@intset((x,y),x^(2)+y^(2)<=1∧y>=0; {d_A})(1)", pi / 2.0),
        ("@intset((x,y),x^(2)+y^(2)<=1∨(x-3)^(2)+y^(2)<=1; {d_A})(1)", 2.0 * pi),
        ("@intset((x,y),x^(2)+y^(2)<=4,¬(x^(2)+y^(2)<1); {d_A})(1)", 3.0 * pi),
    ] {
        let comma = src.replace('∧', ",");
        assert!((value(src) - want).abs() < 1e-6, "{src}");
        // 画面と同じ入口で描ける（値が表示範囲 z ≤ 3 の外なら、平面は範囲の外で見えないだけ）
        let m = run(src, "", &[]).unwrap_or_else(|e| panic!("{src}: {e}"));
        if want < 3.0 {
            assert!(!m.indices().is_empty(), "{src}: 平面が描けない");
        }
        if src.contains('∧') {
            let c = run(&comma, "", &[]).unwrap();
            assert_eq!(m.positions(), c.positions(), "{src}: コンマで書いたものと同じ平面");
        }
    }
    // 積分の外の論理式は、今までどおり論理式として描く
    assert!(run("x^(2)+y^(2)+z^(2)<=4∧¬(z>1)", "", &[]).is_ok());
}

//! 第 5 段階: 黙って間違う・止まる計算（#160 #169 #131 #132 #204 #205）

use graph_core::plot;

/// 注目点（極値など）を求めても、記号微分の式が大きくなりすぎて止まらない（#160。√x/√x は 25 秒以上かかっていた）
#[test]
fn points_of_interest_finish() {
    for src in ["y=((sqrt(x))/(sqrt(x)))", "y=x/x"] {
        graph_core::definitions("");
        let t = std::time::Instant::now();
        let m = plot(src, &[-6.0, 6.0, -6.0, 6.0, -6.0, 6.0], 40, 24, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, true, 1, false, 16);
        assert!(m.is_ok(), "{src}");
        let ms = t.elapsed().as_millis();
        assert!(ms < 3000, "{src}: {ms} ms");
    }
}

fn ode(src: &str, init: &str, d: [f64; 6]) -> (String, usize) {
    let (kind, points, _) = ode_from(src, init, "", false, d);
    (kind, points.len())
}

/// 初期時刻の欄 `start`・前の保存データか `legacy` を付けて解く。(説明, 点 (x, y) の列, 解いた初期時刻)
fn ode_from(src: &str, init: &str, start: &str, legacy: bool, d: [f64; 6]) -> (String, Vec<[f32; 2]>, f64) {
    graph_core::definitions("");
    graph_core::ode_init(init);
    graph_core::ode_start(start, legacy);
    let m = plot(src, &d, 40, 24, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, true, 1, false, 0)
        .unwrap_or_else(|_| panic!("{src} が描けない"));
    let points = m.positions().chunks_exact(3).map(|p| [p[0], p[1]]).collect();
    (m.kind().to_string(), points, m.ode_t0())
}

/// 解曲線の、x にいちばん近い点の y
fn y_at(points: &[[f32; 2]], x: f32) -> f32 {
    points.iter().filter(|p| p[0].is_finite()).min_by(|a, b| (a[0] - x).abs().total_cmp(&(b[0] - x).abs())).map(|p| p[1]).unwrap()
}

/// 初期時刻に右辺が定義されない常微分方程式は、解曲線を描いたように見せずに理由を出す（#132）
#[test]
fn ode_undefined_at_start() {
    let d = [-6.0, 6.0, -6.0, 6.0, -6.0, 6.0];
    let why = "（初期時刻 x = 0 で右辺が定義されないので、解曲線は描けません）";
    // y′ = 1/x: 傾きの場だけ
    let (k, n) = ode("y'=1/x", "", d);
    assert_eq!(k, format!("常微分方程式の傾きの場{why}"));
    assert!(n > 0);
    assert_eq!(ode("y'=1/x", "[1,2]", d).0, format!("常微分方程式の傾きの場{why}"));
    // y″ = 1/x: 何も描かない（初期点 (0, 1) だけを返していた）
    assert_eq!(ode("y''=1/x", "", d), (format!("常微分方程式{why}"), 0));
    // 初期時刻を 0 でない所にすれば解ける（#131 からは、範囲に 0 が無くても初期時刻は 0）。ふつうの式は今までどおり
    assert_eq!(ode_from("y'=1/x", "", "1", false, [0.5, 6.0, -6.0, 6.0, -6.0, 6.0]).0, "常微分方程式の解曲線と傾きの場");
    assert_eq!(ode("y'=x-y", "", d).0, "常微分方程式の解曲線と傾きの場");
    assert_eq!(ode("y''+y=0", "", d).0, "常微分方程式の解曲線");
}

fn value(src: &str) -> f64 {
    graph_core::equation::eval_constant(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn near(src: &str, want: f64, rel: f64) {
    let v = value(src);
    assert!((v - want).abs() <= rel * want.abs(), "{src}: {v}（正しくは {want}）");
}

/// 定数の和・積・内積・トレース・ノルム・階乗の比・多項式の積分を、桁を保って計算する（#204）
#[test]
fn constants_keep_their_digits() {
    // 和・内積・トレースの桁落ち（0 と出ていた）
    assert_eq!(value("@sum(k,1,3) @cases(10^(16), k = 1; 1, k = 2; -10^(16))"), 1.0);
    assert_eq!(value("@dot((10^(16),1,10^(16)),(1,1,-1))"), 1.0);
    assert_eq!(value("@tr(⟦10^(16),0,0;0,1,0;0,0,-10^(16)⟧)"), 1.0);
    // 途中だけあふれる積・内積・ノルム（実数の値にならなかった・0 と出ていた）
    near("@prod(n,1,3) @cases(10^(200), n = 1; 10^(200), n = 2; 10^(-200))", 1e200, 1e-13);
    assert_eq!(value("@dot((10^(200),10^(200),1),(10^(110),-10^(110),2))"), 2.0);
    near("@norm((10^(200),10^(200)))", 2f64.sqrt() * 1e200, 1e-13);
    near("@norm((10^(-200),10^(-200)))", 2f64.sqrt() * 1e-200, 1e-13);
    assert_eq!(value("abs((3,4))"), 5.0);
    // 2⁵³ を超える整数の行列の階数（1 と出ていた）
    assert_eq!(value("@rank(⟦10^(16),0;0,1⟧)"), 2.0);
    // 階乗の比（どちらもあふれる。整数どうしは正確に）
    assert_eq!(value("172!/171!"), 172.0);
    assert_eq!(value("200!/198!"), 39800.0);
    assert_eq!(value("5!/3!"), 20.0);
    // 多項式の定積分は有理数で正確に（被積分関数の値が丸めで 1・t を失っていた）
    assert_eq!(value("@int(t,-1,1)(10^(16) t+1)"), 2.0);
    assert_eq!(value("@int(t,0,1)((t+10^(16))-10^(16))"), 0.5);
    assert_eq!(value("@int(t,0,3)(t^(2))"), 9.0);
    // ふつうの値は今までどおり
    near("@sum(k,1,100) 1/k^2", 1.6349839001848923, 1e-15);
    assert_eq!(value("@prod(k,1,10) k"), 3628800.0);
    near("@int(t,0,1)(e^(t))", std::f64::consts::E - 1.0, 1e-12);
}

/// 微分・極限・差分の合成で、同じ値の式や尺度によって結果が変わっていた（#205）
#[test]
fn compositions_agree() {
    // 偶数乗の |u| は u の偶数乗（折れ目が無い）
    assert_eq!(value("@at(@diff(x, 2) abs(x)^2, x, 0)"), 2.0);
    assert_eq!(value("@at(@diff(x, 4) abs(x)^4, x, 0)"), 24.0);
    assert_eq!(value("@at(@diff(x, 2) sqrt(x^2)^2, x, 0)"), 2.0);
    assert_eq!(value("@at(@diff(x, 2) abs(sin(x))^2, x, 0)"), 2.0);
    // 段差 sign(x) の微分に掛けた式が 0 なら、導関数は存在する（|x|³″ = 6|x|、(x² sign x)″ = 6|x|）
    assert_eq!(value("@at(@diff(x, 2) abs(x)^3, x, 0)"), 0.0);
    assert_eq!(value("@at(@diff(x, 2) sign(x) x^3, x, 0)"), 0.0);
    // 折れ目で左右の値が違えば、点での値も「存在しない」（前は sign(0) = 0 の片側の値 0 を出していた）
    for src in ["@at(@diff(x, 1) abs(x), x, 0)", "@at(@diff(x, 3) abs(x)^3, x, 0)", "@at(@diff(x, 2) x abs(x), x, 0)", "@at(@diff(x, 1) x floor(x), x, 0)"] {
        let e = graph_core::equation::eval_constant(src).expect_err(src);
        assert!(e.contains("導関数は存在しません"), "{src}: {e}");
    }
    assert_eq!(value("@at(@diff(x, 1) abs(x), x, 1)"), 1.0);
    assert_eq!(value("@at(@diff(x, 1) max(x^2, 0), x, 0)"), 0.0);
    // 大きい点の極限: a ± h が丸めなしで表せる h で近づく
    assert_eq!(value("@lim(x,10^(16)) (x-10^(16))"), 0.0);
    assert_eq!(value("@lim(x,10^(16)) abs(x-10^(16))"), 0.0);
    assert_eq!(value("@lim(x,10^(16)) (x-10^(16)+0.5)"), 0.5);
    assert_eq!(value("@lim(x,10^(16)) 5"), 5.0);
    // ゆっくり 0 に近づくべき
    assert_eq!(value("@lim(x,∞) 1/x^(0.01)"), 0.0);
    assert_eq!(value("@lim(x,0,1) x^(0.01)"), 0.0);
    // 収束しないもの・遅すぎるものは、今までどおり確かめられない
    for src in ["@lim(x,∞) ln(ln(x))", "@lim(x,∞) 1/ln(x)", "@lim(x,∞) x^(0.01)", "@lim(x,∞) sin(ln(x))"] {
        assert!(graph_core::equation::eval_constant(src).is_err(), "{src}");
    }
    // 振動（#115 の追加の例。標本と周期がそろう）
    let e = graph_core::equation::eval_constant("@lim(x,0) sin(2π/x^2)").unwrap_err();
    assert!(e.contains("振動"), "{e}");
    // x に比べて小さい幅の差分: 多項式は引き算をしない。そうでなければ、幅が丸めで消える所は値にしない
    assert_eq!(value("@at(@fdiff(x, 10^(-20)) x, x, 1)/10^(-20)"), 1.0);
    assert_eq!(value("@at(@fdiff(x, 10^(-20)) x^2, x, 1)/10^(-20)"), 2.0);
    near("@at(@cdiff(x, 10^(-20)) x^3, x, 1)/10^(-20)", 3.0, 1e-15);
    assert!(graph_core::equation::eval_constant("@at(@fdiff(x, 10^(-20)) sin(x), x, 1)").is_err());
    near("@at(@fdiff(x, 10^(-10)) sin(x), x, 1)/10^(-10)", 1f64.cos(), 1e-6);
    assert_eq!(value("@at(@fdiff(x, 1, 2) x^3, x, 1)"), 12.0);
    assert_eq!(value("@at(@fdiff(x, 1, 2) x floor(x), x, 1)"), 2.0);
}

/// 初期時刻は表示範囲によらない（#131）。範囲を動かしても同じ解の切り取り
#[test]
fn ode_initial_time_is_fixed() {
    let wide = [-1.0, 2.0, -6.0, 6.0, -6.0, 6.0];
    let narrow = [1.0, 2.0, -6.0, 6.0, -6.0, 6.0];
    let cos2 = 2f32.cos();
    // y″ + y = 0、y(0) = 1、y′(0) = 0 は y = cos x。範囲が [1, 2] でも（前は cos(x − 1)）
    for d in [wide, narrow, [100.0, 101.0, -6.0, 6.0, -6.0, 6.0]] {
        let (kind, points, t0) = ode_from("y''+y=0", "", "", false, d);
        assert_eq!((kind.as_str(), t0), ("常微分方程式の解曲線", 0.0), "{d:?}");
        let x = d[1] as f32;
        assert!((y_at(&points, x) - x.cos()).abs() < 2e-3, "{d:?}: {}", y_at(&points, x));
        // 範囲の外の点は、端の外側の 1 点ずつまで
        assert!(points.iter().filter(|p| p[0] < d[0] as f32 || p[0] > d[1] as f32).count() <= 2, "{d:?}");
    }
    assert!((y_at(&ode_from("y''+y=0", "", "", false, narrow).1, 2.0) - cos2).abs() < 2e-3);
    // 初期時刻の欄: x₀ = 1 なら y = cos(x − 1)
    let (_, points, t0) = ode_from("y''+y=0", "", "1", false, wide);
    assert_eq!(t0, 1.0);
    assert!((y_at(&points, 2.0) - 1f32.cos()).abs() < 2e-3);
    assert!((y_at(&points, -1.0) - (-2f32).cos()).abs() < 2e-3);
    // 前の保存データ（初期時刻の欄が無い）で欄が空なら、前の規則（範囲に 0 が無ければ範囲の始まり）
    let (_, points, t0) = ode_from("y''+y=0", "", "", true, narrow);
    assert_eq!(t0, 1.0);
    assert!((y_at(&points, 2.0) - 1f32.cos()).abs() < 2e-3);
    assert_eq!(ode_from("y''+y=0", "", "", true, wide).2, 0.0);
    // 範囲に届く前に発散したら、説明に出す（y′ = y²、y(0) = 1 は x = 1 で発散）
    let (kind, points, _) = ode_from("y'=y^2", "", "", false, [2.0, 3.0, -6.0, 6.0, -6.0, 6.0]);
    assert!(kind.starts_with("常微分方程式の傾きの場（初期時刻 x = 0 から解く途中、x = "), "{kind}");
    assert!(kind.ends_with("で止まり、表示範囲に届かないので、解曲線は描けません）"), "{kind}");
    assert!(points.iter().all(|p| !p[0].is_finite() || p[0] >= 1.9), "傾きの場だけ");
    // 連立（独立変数 t）も同じ
    let (kind, _, t0) = ode_from("x'=-y, y'=x", "", "", false, wide);
    assert_eq!((kind.as_str(), t0), ("常微分方程式の解曲線", 0.0));
}

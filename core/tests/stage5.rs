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
    graph_core::definitions("");
    graph_core::ode_init(init);
    let m = plot(src, &d, 40, 24, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, true, 1, false, 0)
        .unwrap_or_else(|_| panic!("{src} が描けない"));
    (m.kind().to_string(), m.positions().len() / 3)
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
    // 範囲に 0 が無ければ範囲の始まりから解ける。ふつうの式は今までどおり
    assert_eq!(ode("y'=1/x", "", [0.5, 6.0, -6.0, 6.0, -6.0, 6.0]).0, "常微分方程式の解曲線と傾きの場");
    assert_eq!(ode("y'=x-y", "", d).0, "常微分方程式の解曲線と傾きの場");
    assert_eq!(ode("y''+y=0", "", d).0, "常微分方程式の解曲線");
}

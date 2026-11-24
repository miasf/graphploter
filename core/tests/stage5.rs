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

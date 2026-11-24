//! 数値計算の判定で、黙って間違った結果を出していたもの（第 3 段階 3b）。
//! 式は利用者が打つ LaTeX を `web/src/lib/latex.ts` で変換したもの

use graph_core::equation::eval_constant;

fn close(src: &str, want: f64, rel: f64) {
    match eval_constant(src) {
        Ok(v) => assert!((v - want).abs() <= rel * want.abs().max(f64::MIN_POSITIVE), "{src}: {v} （正しくは {want}）"),
        Err(e) => panic!("{src}: {e}（正しくは {want}）"),
    }
}

/// ゼータ関数の自明な零点（負の偶数）はちょうど 0。近くでは零点をまたいで符号が変わる（#149）
#[test]
fn zeta_trivial_zeros() {
    for s in ["zeta(-2)", "zeta(-40)", "zeta(-60)", "zeta(-200)", "dirichletEta(-40)", "dirichletEta(-60)"] {
        assert_eq!(eval_constant(s), Ok(0.0), "{s}");
    }
    // ζ'(−40) = (20)! … = 40! ζ(41) / (2 (2π)⁴⁰) ≈ 4.8241448354828e15。2 次の項（ζ''(−40)/2 · 10⁻¹² ≈ −9×10³）まで含めて相対 10⁻⁵
    let d = 4.824_144_835_482_831e15;
    close("zeta(-40.000001)", -d * 1e-6, 1e-5);
    close("zeta(-39.999999)", d * 1e-6, 1e-5);
    // 負の奇数 ζ(1 − 2n) = −B₂ₙ/(2n) は今までどおり
    close("zeta(-1)", -1.0 / 12.0, 1e-13);
    close("zeta(-3)", 1.0 / 120.0, 1e-13);
    close("zeta(-21)", -281.460_144_927_536_25, 1e-12);
    close("zeta(-41)", -2.004_031_065_651_625_2e16, 1e-12);
}

fn crit(src: &str, r: f64) -> String {
    graph_core::definitions("");
    let m = graph_core::plot(
        src, &[-r, r, -r, r, -r, r], 40, 24, "t", &[0.0, 5.0], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10],
        &[0.0; 5], 0.0, 0, false, false, 1, false, 0,
    )
    .unwrap_or_else(|_| panic!("{src} が描けない"));
    m.kind().to_string()
}

/// 平坦な停留点（f′ の重根）で、0 でない最初の導関数の階数の偶奇を取り違えない（#117）。
/// ニュートン法が 1 回に 0.8 倍（x⁶）しか近づかず、−1.5×10⁻⁸ に残った点で f⁽⁵⁾ = 720x ≠ 0 と見ていた
#[test]
fn flat_critical_points() {
    for r in [3.0, 6.0] {
        let k = crit("@crit(x^(6))", r);
        assert!(k.contains("1 個") && k.contains("x = 0: 極小 f = 0（f″ = f‴ = f⁽⁴⁾ = f⁽⁵⁾ = 0, f⁽⁶⁾ = 720 > 0）"), "{k}");
        let k = crit("@crit(-x^(6))", r);
        assert!(k.contains("x = 0: 極大 f = 0（f″ = f‴ = f⁽⁴⁾ = f⁽⁵⁾ = 0, f⁽⁶⁾ = −720 < 0）"), "{k}");
        let k = crit("@crit(x^(8))", r);
        assert!(k.contains("x = 0: 極小 f = 0（f″ = f‴ = f⁽⁴⁾ = f⁽⁵⁾ = f⁽⁶⁾ = f⁽⁷⁾ = 0, f⁽⁸⁾ = 40320 > 0）"), "{k}");
        // x⁷ は極大と出ていた（f⁽⁶⁾ = 5040x < 0）
        let k = crit("@crit(x^(7))", r);
        assert!(k.contains("x = 0: 極値でない f = 0（f″ = f‴ = f⁽⁴⁾ = f⁽⁵⁾ = f⁽⁶⁾ = 0, f⁽⁷⁾ = 5040 ≠ 0"), "{k}");
        let k = crit("@crit((x-0.3)^(6))", r);
        assert!(k.contains("x = 0.3: 極小"), "{k}");
        // 展開した (x − 0.3)⁶: f′ の丸め誤差で、0.2〜0.4 に 50 個の停留点が散らばっていた
        let k = crit("@crit(x^(6)-1.8x^(5)+1.35x^(4)-0.54x^(3)+0.1215x^(2)-0.01458x+0.000729)", r);
        assert!(k.contains("1 個") && k.contains("x = 0.3: 極小"), "{k}");
        // 今までどおり
        assert!(crit("@crit(x^(4))", r).contains("x = 0: 極小 f = 0（f″ = f‴ = 0, f⁽⁴⁾ = 24 > 0）"));
        assert!(crit("@crit(x^(3))", r).contains("x = 0: 極値でない"));
        assert!(crit("@crit(x^(5))", r).contains("x = 0: 極値でない"));
        let k = crit("@crit(x^(6)+x^(7))", r);
        assert!(k.contains("x = −0.857143: 極大") && k.contains("x = 0: 極小"), "{k}");
        // 近い 2 つの単根の停留点（±10⁻⁴）は、間の f″ = 0 の点へ動かさない
        let k = crit("@crit(x^(3)-3*10^(-8) x)", r);
        assert!(k.contains("x = −0.0001: 極大"), "{k}");
    }
    // 2 変数: 探すときの減衰で (±0.016, 0) に止まり、「極小」が 2 つ出ていた。原点の 1 点（ヘッセ行列が退化しているので判定できない）
    for src in ["@crit(x^(6)+y^(2))", "@crit(x^(4)+y^(2))"] {
        let k = crit(src, 3.0);
        assert!(k.contains("1 個") && k.contains("(0, 0): 判定できない"), "{k}");
    }
    // 座標軸に沿って続く停留点は、今までどおり続いていると知らせる
    assert!(crit("@crit(sin(x) sin(y) sin(z))", 3.0).contains("線・面に沿って続いている可能性"));
}

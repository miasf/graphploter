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

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

/// 描けたら説明（式の行に出す文）、描けなければエラーの文
fn row(src: &str, defs: &str) -> Result<String, String> {
    let _d = graph_core::expand::install(graph_core::expand::Defs::parse(defs, &Default::default()));
    graph_core::equation::compile(src)?;
    drop(_d);
    graph_core::definitions(defs);
    let m = graph_core::plot(
        src, &[-3.0, 3.0, -3.0, 3.0, -3.0, 3.0], 40, 24, "t", &[0.0, 5.0], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16,
        &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0,
    )
    .map_err(|_| format!("{src} が描けない"))?;
    Ok(m.kind().to_string())
}

/// 展開点で関数が定義されていない・要求した次数まで微分できないテイラー多項式を作らない（#116）
#[test]
fn taylor_needs_derivatives() {
    let fails = |src: &str, defs: &str, why: &str| match row(src, defs) {
        Ok(k) => panic!("{src}: {k} を描いた（「{why}」と出すはず）"),
        Err(e) => assert!(e.contains(why), "{src}: 「{e}」（「{why}」と出すはず）"),
    };
    let poly = |src: &str, defs: &str, want: &str| match row(src, defs) {
        Ok(k) => assert!(k.contains(want), "{src}: {k}（{want} のはず）"),
        Err(e) => panic!("{src}: {e}（{want} のはず）"),
    };
    // |x| の原点: f′ は左右で −1 と 1（T₃ = 0 と出ていた）。0 次は作れる
    fails("y = @taylor(abs(x), x, 0, 3)", "", "x = 0 で f′ の左の極限 −1 と右の極限 1 が違うので、f′ は存在しません。1 次以上のテイラー多項式は作れません（0 次までなら作れます）");
    poly("y = @taylor(abs(x), x, 0, 0)", "", "T₀(x) = 0");
    poly("y = @taylor(abs(x), x, 1, 3)", "", "T₃(x) = 1 + (x − 1)");
    // √x・ln x の原点（説明のない空の図だった）
    fails("y = @taylor(sqrt(x), x, 0, 3)", "", "x = 0 に近づくと f′ が発散するので、f′ は存在しません");
    fails("y = @taylor(ln(x), x, 0, 3)", "", "x = 0 で f の値がありません");
    fails("y = @taylor(1/x, x, 0, 2)", "", "x = 0 で f の値がありません");
    fails("y = @taylor(sin(x)/x, x, 0, 4)", "", "極限は 1 なので、場合分けで値を決めれば展開できます");
    // 何階まで微分できるか
    fails("y = @taylor(x abs(x), x, 0, 3)", "", "f″ は存在しません。2 次以上");
    poly("y = @taylor(x abs(x), x, 0, 1)", "", "T₁(x) = 0");
    fails("y = @taylor(abs(x)^(3), x, 0, 3)", "", "f‴ は存在しません。3 次以上");
    fails("y = @taylor(max(x,0), x, 0, 2)", "", "f′ は存在しません");
    fails("y = @taylor(f, x, 0, 3)", "f(x) = @cases(x^(2), x < 0; x^(3))", "f″ の左の極限 2 と右の極限 0 が違う");
    // 値が飛ぶ所
    fails("y = @taylor(floor(x), x, 0, 2)", "", "f は連続でなく、1 次以上");
    fails("y = @taylor(sign(x), x, 0, 1)", "", "f は連続でなく");
    poly("y = @taylor(floor(x), x, 0.5, 2)", "", "T₂(x) = 0");
    // 極に丸めの分だけ近い中心
    fails("y = @taylor(tan(x), x, pi/2, 2)", "", "発散する");
    // 1 点だけ値を決めた場合分け: 決めた枝の導関数（0）でなく、まわりからの極限を使う（T₄ = 1 と出ていた）
    poly("y = @taylor(f, x, 0, 4)", "f(x) = @cases(1, x = 0; sin(x)/x)", "T₄(x) = 1 − 0.166667x² + 0.008333x⁴");
    // 多変数
    fails("z = @taylor(abs(x)+y^(2), (x,y), (0,0), 2)", "", "∂f/∂x の極限が、近づく向きによって違う（−1 と 1）");
    poly("z = @taylor(abs(x)+y^(2), (x,y), (1,0), 2)", "", "T₂(x, y) = 1 + (x − 1) + y²");
    // なめらかな関数は今までどおり
    poly("y = @taylor(sin(x), x, 0, 5)", "", "T₅(x) = x − x³/6 + x⁵/120");
    poly("y = @taylor(besselJ(0,x), x, 0, 4)", "", "T₄(x) = 1 − x²/4 + x⁴/64");
    poly("y = @taylor(gamma(x), x, 1, 3)", "", "T₃(x) = 1 − 0.577216(x − 1) + 0.989056(x − 1)² − 0.907479(x − 1)³");
}

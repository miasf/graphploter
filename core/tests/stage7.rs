//! 陰関数・陽関数の描き方で、解を落とす・無い解を描く（第7段階の前半: #140 #142 #158 #175 #206、#119・#120 の 2D）

use graph_core::plot;

struct Out {
    kind: String,
    /// 曲線の頂点（NaN の区切りを除く）
    verts: Vec<[f32; 2]>,
    /// 長さの無い曲線（描画側が点を描く）
    dots: Vec<[f32; 2]>,
    holes: Vec<[f32; 2]>,
    tris: usize,
    curve: bool,
}

fn draw(src: &str, d: f64, ires: u32) -> Out {
    graph_core::definitions("");
    let mut m = plot(src, &[-d, d, -d, d, -d, d], 64, ires, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, true, 1, false, 0)
        .unwrap_or_else(|_| panic!("{src} が描けない"));
    // 添えの線・点（点・段差の飛ぶ線の上の解）も合わせて見る
    let mut pos = m.positions();
    let extra = m.take_extra();
    if !extra.is_empty() {
        pos.extend_from_slice(&[f32::NAN; 3]);
        pos.extend(extra);
    }
    let v: Vec<&[f32]> = pos.chunks_exact(3).collect();
    let dots = (0..v.len().saturating_sub(1))
        .filter(|&k| v[k][0].is_finite() && v[k] == v[k + 1] && (k == 0 || v[k - 1][0].is_nan()) && v.get(k + 2).is_none_or(|n| n[0].is_nan()))
        .map(|k| [v[k][0], v[k][1]])
        .collect();
    let verts = v.iter().filter(|p| p[0].is_finite()).map(|p| [p[0], p[1]]).collect();
    let holes = m.take_holes().chunks_exact(3).map(|p| [p[0], p[1]]).collect();
    Out { kind: m.kind(), verts, dots, holes, tris: m.indices().len() / 3, curve: m.curve() }
}

fn near(ps: &[[f32; 2]], q: [f32; 2], tol: f32) -> bool {
    ps.iter().any(|p| (p[0] - q[0]).abs() <= tol && (p[1] - q[1]).abs() <= tol)
}

/// 単位円からの距離の最大（頂点が全部、円の上にあるか）
fn off_circle(ps: &[[f32; 2]], c: [f32; 2], r: f32) -> f32 {
    ps.iter().map(|p| ((p[0] - c[0]).hypot(p[1] - c[1]) - r).abs()).fold(0.0, f32::max)
}

/// |F|・√|F|・Fⁿ・定数倍・積の因数は、F = 0 と同じ曲線（#140・#120）
#[test]
fn factors_keep_the_same_curve() {
    let base = draw("x^2+y^2=1", 6.0, 48).verts.len();
    for src in ["abs(x^2+y^2-1)=0", "sqrt(abs(x^2+y^2-1))=0", "(x^2+y^2-1)^2=0", "3(x^2+y^2-1)=0", "sqrt(x^2+y^2-1)=0"] {
        let o = draw(src, 6.0, 48);
        assert_eq!(o.verts.len(), base, "{src}");
        assert!(off_circle(&o.verts, [0.0, 0.0], 1.0) < 1e-4, "{src}");
    }
    // ずらした円と、成分が 2 つある式
    let o = draw("abs((x-1.3)^2+(y+0.7)^2-2)=0", 6.0, 48);
    assert!(o.verts.len() > 100 && off_circle(&o.verts, [1.3, -0.7], 2f32.sqrt()) < 1e-4);
    let o = draw("sqrt(abs(((x-2)^2+y^2-1)((x+2)^2+y^2-1)))=0", 6.0, 48);
    assert!(near(&o.verts, [3.0, 0.0], 1e-3) && near(&o.verts, [-3.0, 0.0], 1e-3) && near(&o.verts, [1.0, 0.0], 1e-3));
    // 積の因数: 直線 y = 1 と、原点の点（#120 の 2D）
    let o = draw("(x^2+y^2)(y-1)=0", 6.0, 48);
    assert!(o.dots.iter().any(|p| p[0].abs() < 1e-3 && p[1].abs() < 1e-3), "{:?}", o.dots);
    assert!(o.verts.iter().filter(|p| p[1] != 0.0 || p[0] != 0.0).all(|p| (p[1] - 1.0).abs() < 1e-4 || (p[0].abs() < 1e-3 && p[1].abs() < 1e-3)));
    // 分母が 0 になる所は解ではない
    let o = draw("(x^2-1)/(x-1)=0", 6.0, 48);
    assert!(o.verts.iter().all(|p| (p[0] + 1.0).abs() < 1e-4), "{:?}", &o.verts[..4.min(o.verts.len())]);
}

/// 0 にならない正の極小（(x² + y² − 1)² + 10⁻⁷ など）を曲線・点にしない。本当に 0 になる点は正しい所に（#206）
#[test]
fn positive_minima_are_not_zeros() {
    for src in ["(x^2+y^2-1)^2+10^(-7)=0", "(x^2+y^2-1)^4+10^(-7)=0", "(x^2+y^2-4)^2+10^(-7)=0", "(x^2+4y^2-1)^2+10^(-7)=0", "x^2+y^2+10^(-9)=0"] {
        assert!(draw(src, 5.0, 40).verts.is_empty(), "{src}");
    }
    let o = draw("(x-.001)^2+(y-.002)^2=0", 5.0, 40);
    assert_eq!(o.dots.len(), 1);
    assert!((o.dots[0][0] - 0.001).abs() < 1e-5 && (o.dots[0][1] - 0.002).abs() < 1e-5, "{:?}", o.dots);
    // 今までどおり描くもの
    assert!(!draw("(x^2+y^2-4)^2=0", 5.0, 40).verts.is_empty());
    assert_eq!(draw("x^2+y^2=0", 5.0, 40).dots.len(), 1);
}

/// 格子より小さい円（#142）
#[test]
fn small_loops_are_found() {
    for (r2, r) in [(1e-4, 0.01f32), (1e-6, 0.001)] {
        let o = draw(&format!("(x-1.234)^2+(y-0.123)^2={r2}"), 6.0, 48);
        assert!(o.verts.len() >= 32, "半径 {r}: {} 頂点", o.verts.len());
        assert!(off_circle(&o.verts, [1.234, 0.123], r) < 0.02 * r, "半径 {r}");
    }
}

/// 恒等式は、定義される所の全体（#206）
#[test]
fn identities_fill_their_domain() {
    for src in ["x=x", "(x-1)/(x-1)=1", "sqrt(x)=sqrt(x)", "x<=x"] {
        let o = draw(src, 5.0, 40);
        assert!(!o.curve && o.tris > 1000, "{src}: {}", o.kind);
    }
    // 厳密な不等式の x < x は空のまま
    assert_eq!(draw("x<x", 5.0, 40).tris, 0);
}

/// 等号つきの不等式の境目に、曲線が接するだけの点（#206）。接する曲線どうしの交点（#119 の 2D）
#[test]
fn touching_points_are_kept() {
    // 格子の細かさ・表示範囲によらず 1 点（格子によっては、切り取りで境目のすぐ脇に切れ端が残っていた）
    for (d, ires) in [(5.0, 40), (3.0, 24), (3.0, 32), (3.0, 80), (3.0, 120), (6.0, 48)] {
        for (src, p) in [("x^2+y^2=1, x>=1", [1.0, 0.0]), ("x^2+y^2=1, y>=1", [0.0, 1.0]), ("x^2+y^2=1, x<=-1", [-1.0, 0.0]), ("y=cos(x), y=1", [0.0, 1.0])] {
            let o = draw(src, d, ires);
            assert_eq!(o.dots.len(), 1, "±{d} {ires} {src}: {:?}", o.dots);
            assert!(near(&o.dots, p, 1e-4) && o.verts.len() == 2, "±{d} {ires} {src}: {:?}", o.verts);
        }
    }
    // 厳密な不等式・わずかに離れた曲線は空。ふつうに切り取った弧の端には点を足さない
    for src in ["x^2+y^2=1, x>1", "y=cos(x), y=1+10^(-6)", "x^2+y^2=1, x>=1, y>0.5"] {
        assert!(draw(src, 5.0, 40).verts.is_empty(), "{src}");
    }
    assert!(draw("x^2+y^2=1, x>=0.5", 5.0, 40).dots.is_empty());
    assert_eq!(draw("y=sin(x), y=1", 5.0, 40).dots.len(), 2);
}

/// 段差の飛ぶ線の上だけで成り立つ解（#175）。格子が整数に乗らない範囲でも
#[test]
fn jump_lines_are_drawn() {
    for d in [5.3, 6.0] {
        for src in ["floor(x)+floor(-x)=0", "floor(x)=x"] {
            let o = draw(src, d, 40);
            for k in -5..=5 {
                assert!(o.verts.iter().any(|p| (p[0] - k as f32).abs() < 1e-5), "±{d} {src}: x = {k}");
            }
            assert!(o.verts.iter().all(|p| (p[0] - p[0].round()).abs() < 1e-4), "±{d} {src}: 整数でない x");
        }
    }
    // 飛ぶ所で 0 にならない式には線を足さない
    assert!(draw("floor(x)-0.5=0", 5.3, 40).verts.is_empty());
}

/// 場合分けで 1 点だけ定義した値は黒丸、左右の極限は白丸（#158）
#[test]
fn single_point_cases() {
    let o = draw("y=@cases(2, x=0.025; 1, x<0.025; 3, x>0.025)", 6.0, 48);
    assert!(near(&o.dots, [0.025, 2.0], 1e-6), "{:?}", o.dots);
    assert!(near(&o.holes, [0.025, 1.0], 1e-6) && near(&o.holes, [0.025, 3.0], 1e-6), "{:?}", o.holes);
    assert!(near(&draw("y=@cases(2, x=0.025)", 6.0, 48).dots, [0.025, 2.0], 1e-6));
    let o = draw("y=@cases(5, x=1; x^2)", 6.0, 48);
    assert!(near(&o.dots, [1.0, 5.0], 1e-6) && near(&o.holes, [1.0, 1.0], 1e-6));
    // ふつうの段差・連続な場合分け・√x の端には付けない
    for src in ["y=@cases(1, x<0; 3)", "y=sqrt(x)", "y=@cases(x, x<1; 2-x)"] {
        let o = draw(src, 6.0, 48);
        assert!(o.dots.is_empty(), "{src}: {:?}", o.dots);
    }
}

/// 標本の間に隠れた細い山（#206）
#[test]
fn narrow_peaks_are_kept() {
    let o = draw("y=1/(1+10^(20)(x-.234567)^2)", 5.0, 40);
    assert!(near(&o.verts, [0.234567, 1.0], 1e-5), "最大 {:?}", o.verts.iter().map(|p| p[1]).fold(f32::MIN, f32::max));
    let o = draw("y=-1/(1+10^(16)(x+1.5)^2)", 5.0, 40);
    assert!(near(&o.verts, [-1.5, -1.0], 1e-5));
}

/// 3D・4D の描画の結果（頂点・三角形・添えの線と点・説明・GPU を使わない理由）
struct Out3 {
    kind: String,
    verts: Vec<[f32; 3]>,
    tris: usize,
    extra: Vec<[f32; 3]>,
    extra_dots: Vec<[f32; 3]>,
    gpu_reason: String,
}

fn draw3(src: &str, d: f64, ires: u32, dims: u32, slice: &[f64], flat: bool) -> Out3 {
    graph_core::definitions("");
    let coord = vec![f64::NAN; 2 * dims as usize];
    let mut m = plot(src, &[-d, d, -d, d, -d, d], 64, ires, "", &[], "", &[], "", dims, &[0, 1, 2], slice, &coord, "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, flat, 1, false, 0)
        .unwrap_or_else(|_| panic!("{src} が描けない"));
    let verts = m.positions().chunks_exact(3).filter(|p| p[0].is_finite()).map(|p| [p[0], p[1], p[2]]).collect();
    let ex = m.take_extra();
    let v: Vec<&[f32]> = ex.chunks_exact(3).collect();
    let extra_dots = (0..v.len().saturating_sub(1))
        .filter(|&k| v[k][0].is_finite() && v[k] == v[k + 1] && (k == 0 || v[k - 1][0].is_nan()) && v.get(k + 2).is_none_or(|n| n[0].is_nan()))
        .map(|k| [v[k][0], v[k][1], v[k][2]])
        .collect();
    let extra = v.iter().filter(|p| p[0].is_finite()).map(|p| [p[0], p[1], p[2]]).collect();
    Out3 { kind: m.kind(), verts, tris: m.indices().len() / 3, extra, extra_dots, gpu_reason: m.gpu_reason() }
}

fn near3(ps: &[[f32; 3]], q: [f32; 3], tol: f32) -> bool {
    ps.iter().any(|p| (0..3).all(|c| (p[c] - q[c]).abs() <= tol))
}

/// 4 次元の超球の断面が 1 点に接する（#134）: 球面 → 点 → 空
#[test]
fn hypersphere_slice_point() {
    let o = draw3("x^2+y^2+z^2+w^2=1", 6.0, 48, 4, &[0.0, 0.0, 0.0, 1.0], false);
    assert_eq!(o.verts.len(), 2, "{}", o.kind);
    assert!(near3(&o.verts, [0.0; 3], 1e-5));
    assert!(draw3("x^2+y^2+z^2+w^2=1", 6.0, 48, 4, &[0.0, 0.0, 0.0, 1.01], false).verts.is_empty());
    assert!(draw3("x^2+y^2+z^2+w^2=1", 6.0, 48, 4, &[0.0, 0.0, 0.0, 0.99], false).tris > 1000);
}

/// 面と線・点が 1 つの式・集合に混ざる（#120・#121）: 線・点は添えの線・点
#[test]
fn mixed_dimensions_are_kept() {
    let o = draw3("(x^2+y^2)(z-1)=0", 6.0, 48, 3, &[0.0; 3], false);
    assert!(o.tris > 1000 && o.verts.iter().all(|p| (p[2] - 1.0).abs() < 1e-4), "平面 z = 1");
    assert!(o.extra.len() > 10 && o.extra.iter().all(|p| p[0].abs() < 1e-4 && p[1].abs() < 1e-4), "z 軸");
    assert!(o.extra.iter().any(|p| p[2] < -5.0) && o.extra.iter().any(|p| p[2] > 5.0));
    // 集合の和: 円板と、点 (3, 0)・縦線 x = 3・横線 y = 0。写像 (x², y) の像では縦線が x = 9
    for flat in [true, false] {
        let o = draw3("⦃(x,y)∣x^(2)+y^(2)<=1∨(x-3)^(2)+y^(2)=0⦄", 6.0, 48, 3, &[0.0; 3], flat);
        assert!(o.tris > 1000 && near3(&o.extra_dots, [3.0, 0.0, 0.0], 1e-4), "点 (3, 0): {:?}", o.extra_dots);
        let o = draw3("⦃(x,y)∣x^(2)+y^(2)<=1∨x=3⦄", 6.0, 48, 3, &[0.0; 3], flat);
        assert!(o.tris > 1000 && o.extra.len() > 10 && o.extra.iter().all(|p| (p[0] - 3.0).abs() < 1e-4));
        let o = draw3("⦃(x^(2),y)∣x^(2)+y^(2)<=1∨x=3⦄", 9.5, 48, 3, &[0.0; 3], flat);
        assert!(o.extra.len() > 10 && o.extra.iter().all(|p| (p[0] - 9.0).abs() < 1e-3));
    }
}

/// 複素数の陰関数で Re F と Im F が同じ面（曲線）になる（#122）
#[test]
fn dependent_complex_parts() {
    for src in ["i(x^2+y^2+z^2-1)=0", "(x^2+y^2+z^2-1)+i(x^2+y^2+z^2-1)=0", "(x^2+y^2+z^2-1)+i(x^2+y^2+z^2-1)^2=0"] {
        let o = draw3(src, 5.0, 40, 3, &[0.0; 3], false);
        assert!(o.tris > 1000, "{src}: {}", o.kind);
        assert!(o.verts.iter().all(|p| ((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 1.0).abs() < 0.01), "{src}");
    }
    // 今までどおり: 交わりの曲線
    let o = draw3("(x^2+y^2+z^2-1)+i z=0", 5.0, 40, 3, &[0.0; 3], false);
    assert!(o.tris == 0 && o.verts.iter().all(|p| p[2].abs() < 1e-4));
    // 2D
    let o = draw3("i(x^2+y^2-1)=0", 5.0, 40, 3, &[0.0; 3], true);
    assert!(o.verts.len() > 100 && o.verts.iter().all(|p| (p[0].hypot(p[1]) - 1.0).abs() < 1e-3));
}

/// 格子より薄い領域の境目は 2 枚とも描き、厚さを説明に出す（#143）
#[test]
fn thin_shells_keep_both_sides() {
    let one = draw3("x^2+y^2+z^2<=1", 5.0, 48, 3, &[0.0; 3], false).tris;
    let o = draw3("1<=x^2+y^2+z^2<=1.0001", 5.0, 48, 3, &[0.0; 3], false);
    assert!(o.tris >= 2 * one - 10, "{} 三角形（球 1 枚は {one}）", o.tris);
    assert!(o.kind.contains("厚さ約 0.00005"), "{}", o.kind);
    // 同じ面の不等式 2 本は 1 枚・説明なし
    let o = draw3("x^2+y^2+z^2<=1, 2x^2+2y^2+2z^2<=2", 5.0, 48, 3, &[0.0; 3], false);
    assert!(o.tris <= one + 10 && !o.kind.contains("厚さ"), "{}", o.kind);
}

/// 3D の連立方程式: 接する点・接する線・同じ面・恒等式・不等式の境目に接する点（#119・#206）
#[test]
fn touching_systems_in_3d() {
    let o = draw3("x^2+y^2+z^2=1, (x-2)^2+y^2+z^2=1", 5.0, 40, 3, &[0.0; 3], false);
    assert_eq!(o.verts.len(), 2);
    assert!(near3(&o.verts, [1.0, 0.0, 0.0], 1e-4), "{:?}", o.verts);
    assert!(draw3("x^2+y^2+z^2=1, (x-2.00001)^2+y^2+z^2=1", 5.0, 40, 3, &[0.0; 3], false).verts.is_empty());
    // 球と円柱は赤道の円で接する
    let o = draw3("x^2+y^2+z^2=1, x^2+y^2=1", 5.0, 40, 3, &[0.0; 3], false);
    assert!(o.verts.len() > 50 && o.verts.iter().all(|p| p[2].abs() < 1e-3 && (p[0].hypot(p[1]) - 1.0).abs() < 1e-3));
    for src in ["x^2+y^2+z^2=1, 2x^2+2y^2+2z^2=2", "x^2+y^2+z^2=1, z=z"] {
        let o = draw3(src, 5.0, 40, 3, &[0.0; 3], false);
        assert!(o.tris > 1000, "{src}: {}", o.kind);
    }
    let o = draw3("x^2+y^2+z^2=1, z>=1", 5.0, 40, 3, &[0.0; 3], false);
    assert!(near3(&o.extra_dots, [0.0, 0.0, 1.0], 1e-4) && o.extra_dots.len() == 1, "{:?}", o.extra_dots);
    assert!(draw3("x^2+y^2+z^2=1, z>1", 5.0, 40, 3, &[0.0; 3], false).extra.is_empty());
    // ふつうに交わる式は今までどおり
    let o = draw3("x^2+y^2+z^2=1, (x-1)^2+y^2+z^2=1", 5.0, 40, 3, &[0.0; 3], false);
    assert!(o.verts.len() > 100 && o.verts.iter().all(|p| (p[0] - 0.5).abs() < 1e-3));
}

/// 段差の枝の中で恒等的に成り立つ解は塗る。3D の飛ぶ面の上の解は面（GPU は使わない）。2D の飛ぶ線は添えの線（GPU でも重ねる）
#[test]
fn step_function_solutions() {
    let o = draw3("floor(x)=2", 5.0, 40, 3, &[0.0; 3], true);
    assert!(o.tris > 100 && o.verts.iter().all(|p| p[0] >= 2.0 - 1e-5 && p[0] <= 3.0 + 1e-5), "{}", o.kind);
    let o = draw3("floor(x)+floor(y)=0", 5.0, 40, 3, &[0.0; 3], true);
    assert!(o.tris > 100 && o.verts.iter().all(|p| p[0].floor() + p[1].floor() == 0.0 || (p[0] - p[0].round()).abs() < 1e-4 || (p[1] - p[1].round()).abs() < 1e-4));
    let o = draw3("floor(x)+floor(-x)=0", 5.3, 48, 3, &[0.0; 3], false);
    for k in -5..=5 {
        assert!(o.verts.iter().any(|p| (p[0] - k as f32).abs() < 1e-4), "平面 x = {k}");
    }
    assert!(!o.gpu_reason.is_empty() && !o.kind.contains("細かすぎて"), "{} / {}", o.kind, o.gpu_reason);
    // 体積のある解（floor(x) + floor(y) + floor(z) = 0 の単位立方体 73 個）: メッシュでも描き（スマートフォンの「自動」はメッシュ）、
    // 光線（GPU）も使える（飛ぶ面の解があるからと止めない）
    let o = draw3("floor(x)+floor(y)+floor(z)=0", 5.0, 48, 3, &[0.0; 3], false);
    assert!(o.gpu_reason.is_empty(), "{}", o.gpu_reason);
    assert!(o.tris > 0 && o.verts.iter().all(|p| (0..3).any(|a| (p[a] - p[a].round()).abs() < 1e-4)), "{}", o.kind);
    let o = draw3("floor(x)+floor(-x)=0", 5.3, 40, 3, &[0.0; 3], true);
    for k in -5..=5 {
        assert!(o.extra.iter().any(|p| (p[0] - k as f32).abs() < 1e-4), "2D の添えの線 x = {k}");
    }
}

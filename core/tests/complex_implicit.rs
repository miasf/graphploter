//! 複素数の陰関数（値が複素数の F = 0 を、Re F = Im F = 0 の線として描く）。画面と同じ入口（`graph_core::plot`）で確かめる

use graph_core::{equation, plot, MeshResult};

const R: f64 = 3.0;

fn run(src: &str, flat: bool) -> MeshResult {
    run_mode(src, flat, 0)
}

/// 複素数の色分け `mode`（0: しない, 1: |F| = 1, 2: Re F = 0, 3: Im F = 0）を付けて描く
fn run_mode(src: &str, flat: bool, mode: u32) -> MeshResult {
    let rd = equation::rot_dims(3);
    plot(
        src, &[-R, R, -R, R, -R, R], 48, 40, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16,
        &vec![0.0; rd * (rd - 1) / 2], &vec![0.0; rd], 0.0, mode, false, flat, 1, flat, 0,
    )
    .unwrap_or_else(|_| panic!("{src} が計算できない"))
}

fn pts(m: &MeshResult) -> Vec<[f64; 3]> {
    m.positions().chunks_exact(3).filter(|q| q[0].is_finite()).map(|q| [q[0] as f64, q[1] as f64, q[2] as f64]).collect()
}

#[test]
fn helix() {
    // e^{ix} = y + iz: y = cos x, z = sin x のらせん（x の向きに 3 周ほど）
    let m = run("e^(i x) = y + i z", false);
    assert_eq!(m.kind(), "複素数の陰関数（Re F = Im F = 0 の曲線）");
    assert!(m.curve());
    let p = pts(&m);
    assert!(p.len() > 100, "{}", p.len());
    for q in &p {
        assert!((q[1] - q[0].cos()).abs() < 1e-4 && (q[2] - q[0].sin()).abs() < 1e-4, "{q:?}");
    }
    let (lo, hi) = p.iter().fold((f64::MAX, f64::MIN), |(a, b), q| (a.min(q[0]), b.max(q[0])));
    assert!(lo < -2.9 && hi > 2.9, "{lo} {hi}");
}

#[test]
fn square_gives_two_parabolas() {
    // (x + iy)² = z: x² − y² = z かつ 2xy = 0 → x = 0 の z = −y² と、y = 0 の z = x²
    let m = run("(x + i y)^2 = z", false);
    // 右辺が座標 1 つなので、今までどおり陽関数（複素数の色分けで描く）。陰関数の形にして確かめる
    assert!(!m.kind().starts_with("複素数の陰関数"), "{}", m.kind());
    let m = run("(x + i y)^2 - z = 0", false);
    assert_eq!(m.kind(), "複素数の陰関数（Re F = Im F = 0 の曲線）");
    let p = pts(&m);
    assert!(!p.is_empty());
    for q in &p {
        let on_a = q[0].abs() < 1e-4 && (q[2] + q[1] * q[1]).abs() < 1e-3;
        let on_b = q[1].abs() < 1e-4 && (q[2] - q[0] * q[0]).abs() < 1e-3;
        assert!(on_a || on_b, "{q:?}");
    }
    assert!(p.iter().any(|q| q[0].abs() > 1.0) && p.iter().any(|q| q[1].abs() > 1.0));
}

#[test]
fn real_valued_with_i_stays_a_surface() {
    // i を含んでも値が実数の式は、ふつうの陰関数（曲面）
    let m = run("abs(x + i y) = z^2 + 1", false);
    assert_eq!(m.kind(), "陰関数");
    assert!(!m.curve() && !m.indices().is_empty());
    // i を含まない式は、定義域の外で複素数になっても、ふつうの陰関数
    let m = run("sqrt(x) + y^2 + z^2 = 2", false);
    assert_eq!(m.kind(), "陰関数");
}

#[test]
fn flat_points() {
    // 2D（切り口 z = 0）: (x + iy)² = 1 → x² − y² = 1, xy = 0 → (±1, 0)
    let m = run("(x + i y)^2 = 1 + z", true);
    assert_eq!(m.kind(), "複素数の陰関数（Re F = Im F = 0 の点、2 個）");
    let p = pts(&m);
    for want in [1.0, -1.0] {
        assert!(p.iter().any(|q| (q[0] - want).abs() < 1e-6 && q[1].abs() < 1e-6), "{p:?}");
    }
}

#[test]
fn colored_surfaces() {
    let src = "e^(i x) = y + i z";
    // |F| = 1 の曲面: 頂点で |e^{ix} − y − iz| = 1、色分けの値が頂点ごとにある
    let m = run_mode(src, false, 1);
    assert_eq!(m.kind(), "複素数の陰関数（|F| = 1 の曲面、色は arg F）");
    assert!(!m.curve() && !m.indices().is_empty());
    let (p, c) = (m.positions(), m.complex_values());
    assert_eq!(c.len(), p.len());
    for q in p.chunks_exact(3) {
        let (x, y, z) = (q[0] as f64, q[1] as f64, q[2] as f64);
        let r = ((x.cos() - y).powi(2) + (x.sin() - z).powi(2)).sqrt();
        assert!((r - 1.0).abs() < 0.05, "{q:?} {r}");
    }
    // Re F = 0 の曲面: y = cos x。色の値の 1 つ目（Re F / |F|）はほぼ 0
    let m = run_mode(src, false, 2);
    assert_eq!(m.kind(), "複素数の陰関数（Re F = 0 の曲面、色は F）");
    for (q, v) in m.positions().chunks_exact(3).zip(m.complex_values().chunks_exact(3)) {
        assert!(((q[0] as f64).cos() - q[1] as f64).abs() < 0.02, "{q:?}");
        assert!(v[0].abs() < 0.2 || v[2] < -3.0, "{v:?}");
    }
}

#[test]
fn complex_coordinates() {
    use graph_core::complex_coords;
    let rd = equation::rot_dims(6);
    let run6 = |axes: [u32; 3]| {
        complex_coords(true);
        plot(
            "x^2 + y^2 + z^2 = 1", &[-2.0, 2.0, -2.0, 2.0, -2.0, 2.0], 48, 40, "", &[], "", &[], "", 6, &axes, &[0.0; 6], &[f64::NAN; 12], "",
            &[], 16, &vec![0.0; rd * (rd - 1) / 2], &vec![0.0; rd], 0.0, 0, false, false, 1, false, 0,
        )
        .ok()
        .unwrap()
    };
    // x, y, z を表示（虚部は全部 0）: 実数の部分 = 球面
    let m = run6([0, 1, 2]);
    assert_eq!(m.kind(), "陰関数");
    assert!(!m.curve());
    // (x, w, y) を表示（z = w₂ = w₃ = 0）: (x + iw)² + y² = 1 → w = 0 の円 x² + y² = 1 と、x = 0 の双曲線 y² − w² = 1
    let m = run6([0, 3, 1]);
    assert_eq!(m.kind(), "複素数の陰関数（Re F = Im F = 0 の曲線）");
    let p = pts(&m);
    assert!(p.len() > 50);
    for q in &p {
        let (x, w, y) = (q[0], q[1], q[2]);
        let circle = w.abs() < 1e-4 && (x * x + y * y - 1.0).abs() < 1e-3;
        let hyperbola = x.abs() < 1e-4 && (y * y - w * w - 1.0).abs() < 1e-3;
        assert!(circle || hyperbola, "{q:?}");
    }
    // 使っている名前に、虚部の座標が入る（w の見せ方の対象になる）
    assert!(m.used_params().split(',').any(|n| n == "w2"), "{}", m.used_params());
    // 次の plot では元に戻る（1 回だけ）
    let m = plot(
        "x^2 + y^2 + z^2 = 1", &[-2.0, 2.0, -2.0, 2.0, -2.0, 2.0], 48, 40, "", &[], "", &[], "", 6, &[0, 3, 1], &[0.0; 6], &[f64::NAN; 12], "",
        &[], 16, &vec![0.0; rd * (rd - 1) / 2], &vec![0.0; rd], 0.0, 0, false, false, 1, false, 0,
    )
    .ok()
    .unwrap();
    assert_eq!(m.kind(), "陰関数");
    // 次元が足りない・虚部の座標を使っている式はエラー
    let mut sp = equation::Space::basic(4, [0, 1, 2], vec![0.0; 4]);
    sp.complex_coords = true;
    assert!(equation::compile_with("x^2 + y^2 = 1", &Default::default(), &sp).err().unwrap().contains("次元を 6 以上"));
    let mut sp = equation::Space::basic(6, [0, 1, 2], vec![0.0; 6]);
    sp.complex_coords = true;
    assert!(equation::compile_with("x + w = 1", &Default::default(), &sp).is_err());
}

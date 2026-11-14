//! いくつかの式が同時に 0 になる所（線・点だけの解）を、画面と同じ入口（`graph_core::plot`）で描く

use graph_core::{equation, plot, MeshResult};

/// 表示範囲 ±3 の 3 次元（`dims` 次元。4 次元なら座標 0 と 3 の組を `xw` だけ回す）
fn run(src: &str, dims: u32, xw: f64) -> MeshResult {
    let n = dims as usize;
    let rd = equation::rot_dims(n);
    let mut rotation = vec![0.0; rd * (rd - 1) / 2];
    if n > 3 {
        rotation[2] = xw;
    }
    let r = 3.0;
    plot(
        src, &[-r, r, -r, r, -r, r], 64, 40, "", &[], "", &[], "", dims, &[0, 1, 2], &vec![0.0; n],
        &vec![f64::NAN; 2 * n], "", &[], 16, &rotation, &vec![0.0; rd], 0.0, 0, false, false, 1, false, 0,
    )
    .unwrap_or_else(|_| panic!("{src} が計算できない"))
}

/// 線分の端（NaN で区切った点）
fn ends(m: &MeshResult) -> Vec<[f32; 3]> {
    m.positions().chunks_exact(3).filter(|p| p[0].is_finite()).map(|p| [p[0], p[1], p[2]]).collect()
}

#[test]
fn lines_and_points_are_drawn_as_curves() {
    // z 軸
    let m = run("x^2 + y^2 = 0", 3, 0.0);
    assert!(m.curve());
    assert_eq!(m.kind(), "陰関数（線だけの解）");
    assert!(!m.local_available());
    let pts = ends(&m);
    assert!(pts.len() > 20 && pts.iter().all(|p| p[0].abs() < 1e-5 && p[1].abs() < 1e-5), "{}", pts.len());
    let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[2]), b.max(p[2])));
    assert!(lo < -2.9 && hi > 2.9, "{lo} {hi}");
    // 原点 1 つ
    let m = run("x^2 + y^2 + z^2 = 0", 3, 0.0);
    assert_eq!(m.kind(), "陰関数（点だけの解）");
    assert_eq!(ends(&m).len(), 2);
    // 面になる式は今までどおり
    let m = run("x^2 + y^2 = 1", 3, 0.0);
    assert!(!m.curve());
    assert_eq!(m.kind(), "陰関数");
}

#[test]
fn rotated_line_in_four_dimensions() {
    // w = 0 の断面で、xw に 0.6 rad 回した x² + y² = 0 も z 軸（回した後の x は cos(0.6) x + … で、w = 0）
    let m = run("x^2 + y^2 = 0", 4, 0.6);
    assert!(m.curve());
    let pts = ends(&m);
    assert!(pts.len() > 20 && pts.iter().all(|p| p[0].abs() < 1e-4 && p[1].abs() < 1e-4), "{}", pts.len());
    // y = x, z = w の線（w の断面は 0）: (y − x)² + (z − w)² = 0
    let m = run("(y - x)^2 + (z - w)^2 = 0", 4, 0.0);
    let pts = ends(&m);
    assert!(pts.len() > 20 && pts.iter().all(|p| (p[1] - p[0]).abs() < 1e-4 && p[2].abs() < 1e-4), "{}", pts.len());
}

#[test]
#[ignore]
fn line_report() {
    for src in [
        "x^2 + y^2 = 0",
        "(x^2 + y^2 - 4)^2 + z^2 = 0",
        "(x^2 + y^2 - 4)^2 + (z - x)^2 = 0",
        "(x^2 + y^2 + z^2 - 4)^2 + (x^2 + y^2 - 2 x)^2 = 0",
        "(sin(x) - cos(y))^2 + (cos(y) - tan(z))^2 = 0",
        "x^2 + y^2 + z^2 = 0",
    ] {
        let t = std::time::Instant::now();
        let m = run(src, 3, 0.0);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        println!("{src}\t{}\t{} 点\t{ms:.1} ms", m.kind(), ends(&m).len());
    }
}

/// `tools/visual/cases.tsv` の式のうち、線・点だけの解として描くもの（一覧を確かめるために手で回す）
#[test]
#[ignore]
fn cases_taking_the_line_path() {
    let list = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../tools/visual/cases.tsv")).unwrap();
    for line in list.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let src = line.splitn(3, '\t').nth(2).unwrap_or("");
        if let Ok(m) = std::panic::catch_unwind(|| run(src, 3, 0.0)) {
            if m.kind().contains("だけの解") {
                println!("{src}\t{}", m.kind());
            }
        }
    }
}

//! 2D 表示の投影で、縦に立った面（真上から見ると面積が無い）が線として残ること

use graph_core::plot;

fn outline(src: &str) -> (Vec<f32>, usize) {
    let mut m = plot(src, &[-2.0, 2.0, -2.0, 2.0, -2.0, 2.0], 60, 40, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, true, 1, false, 0)
        .unwrap_or_else(|_| panic!("{src}"));
    let tris = m.indices().len() / 3;
    (m.take_outline(), tris)
}

#[test]
fn vertical_surfaces_leave_lines() {
    // y = x² を z の方向に伸ばした面・y = x の平面・円柱。どれも真上から見ると線になる
    for (src, on) in [
        ("y = x^2 + 0 z", &(|x: f32, y: f32| (y - x * x).abs()) as &dyn Fn(f32, f32) -> f32),
        ("y - x + 0 z = 0", &|x: f32, y: f32| (y - x).abs()),
        ("x^2 + y^2 + 0 z = 1", &|x: f32, y: f32| ((x * x + y * y).sqrt() - 1.0).abs()),
    ] {
        let (lines, _) = outline(src);
        let pts: Vec<&[f32]> = lines.chunks_exact(3).filter(|p| p[0].is_finite()).collect();
        assert!(pts.len() > 50, "{src}: 線の点が {} 個", pts.len());
        for p in &pts {
            assert!(p[2] == 0.0 && on(p[0], p[1]) < 0.05, "{src}: 線の点 {p:?} が曲線の上にない");
        }
        // 線が曲線全体を覆う: x = −0.9 … 0.9 のどこにも近くに線の点がある
        for k in 0..=18 {
            let x = -0.9 + 0.1 * k as f32;
            assert!(pts.iter().any(|p| (p[0] - x).abs() < 0.1), "{src}: x = {x} の近くに線がない");
        }
    }
}

#[test]
fn lying_surfaces_add_no_lines() {
    for src in ["z - x^2 - y^2 = 0", "y = 0.01 z"] {
        let (lines, tris) = outline(src);
        assert!(tris > 0 && lines.is_empty(), "{src}: {} points on helper lines", lines.len() / 3);
    }
}

#[test]
fn rotating_a_projected_surface_does_not_add_a_helper_outline() {
    let domain = [-12.0, 12.0, -12.0, 12.0, -12.0, 12.0];
    let center = [1.0, 0.0, 0.0, 0.0, 0.0];
    let mut rotation = [0.0; 10];
    rotation[4] = 27.0_f64.to_radians();
    let mut m = plot(
        "z = x^2+y^2, z ≠ 0", &domain, 150, 80, "", &[], "", &[], "", 3,
        &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 24, &rotation, &center,
        40.0, 0, false, true, 1, false, 0,
    ).ok().unwrap();
    let check = |result: &mut graph_core::MeshResult| {
        assert!(!result.indices().is_empty());
        assert!(result.take_outline().is_empty());
        assert_eq!(result.take_holes(), [0.0, 0.0, 0.0]);
    };
    check(&mut m);
    let shape = m.take_shape().unwrap();
    rotation[4] = -35.0_f64.to_radians();
    let mut projected = shape.project(&domain, 3, &[0, 1, 2], &rotation, &center, 40.0).ok().unwrap();
    check(&mut projected);
}

/// 3D: 縦に立った面は、表示範囲の箱で切れた縁が線になる（真上から見ると面が真横を向いて見えなくなるため）。
/// 閉じた面（球）には線を足さない
#[test]
fn box_edges_in_3d() {
    let lines = |src: &str| {
        let mut m = plot(src, &[-10.0, 10.0, -10.0, 10.0, -5.0, 5.0], 60, 40, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0)
            .unwrap_or_else(|_| panic!("{src}"));
        m.take_outline()
    };
    for src in ["y = 0", "y = x^2", "x = 0"] {
        let l = lines(src);
        let top: Vec<&[f32]> = l.chunks_exact(3).filter(|p| p[0].is_finite() && (p[2] - 5.0).abs() < 1e-3).collect();
        assert!(top.len() > 20, "{src}: 上の面の縁の線が {} 点", top.len());
    }
    assert!(lines("x^2 + y^2 + z^2 = 4").is_empty(), "球に線がある");
}

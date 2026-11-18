//! ≠ で除いた所を、白丸（曲線）・点線（面・領域）で示す

use graph_core::plot;

fn run(src: &str, flat: bool) -> (Vec<[f32; 3]>, usize, String) {
    let mut m = plot(src, &[-2.0, 2.0, -2.0, 2.0, -2.0, 2.0], 60, 40, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, flat, 1, flat, 0)
        .unwrap_or_else(|_| panic!("{src} がエラー"));
    let holes = m.take_holes().chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect();
    let dashed = m.take_outline_dashed().chunks_exact(3).filter(|p| p[0].is_finite()).count();
    (holes, dashed, m.kind())
}

fn has(holes: &[[f32; 3]], q: [f32; 2]) -> bool {
    holes.iter().any(|p| (p[0] - q[0]).abs() < 1e-5 && (p[1] - q[1]).abs() < 1e-5)
}

#[test]
fn excluded_points_and_lines() {
    // 2D: 直線 y = x から原点を除く → 原点に白丸
    let (h, _, kind) = run("y = x, y ≠ 0", true);
    assert!(has(&h, [0.0, 0.0]), "{kind}: {h:?}");
    assert_eq!(h.len(), 1, "{h:?}");
    assert!(!kind.contains("無視"), "{kind}");
    // 円から x = 0 の 2 点を除く
    let (h, _, _) = run("x^2 + y^2 = 1, x ≠ 0", true);
    assert!(has(&h, [0.0, 1.0]) && has(&h, [0.0, -1.0]) && h.len() == 2, "{h:?}");
    // 3D: 平面 y = x から y = 0 の線を除く → 点線
    let (h, dashed, _) = run("y = x, y ≠ 0", false);
    assert!(h.is_empty() && dashed > 10, "白丸 {h:?}・点線の点 {dashed}");
}

//! ≠ で除いた所を、白丸（曲線）・点線（面・領域）で示す

use graph_core::plot;

fn run(src: &str, flat: bool) -> (Vec<[f32; 3]>, usize, String) {
    run_view(src, flat, flat)
}

fn run_view(src: &str, flat: bool, slice: bool) -> (Vec<[f32; 3]>, usize, String) {
    let mut m = plot(src, &[-2.0, 2.0, -2.0, 2.0, -2.0, 2.0], 60, 40, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, flat, 1, slice, 0)
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

#[test]
fn isolated_surface_exclusion_in_3d_projection_and_slice() {
    for (flat, slice) in [(false, false), (true, false), (true, true)] {
        let (marks, dashed, _) = run_view("z = (x - 0.3)^2 + (y + 0.7)^2, z ≠ 0", flat, slice);
        assert_eq!(marks, [[0.3, -0.7, 0.0]], "{flat} {slice}");
        assert_eq!(dashed, 0);
    }
    let (marks, _, _) = run("z = x^2 + y^2, z ≠ 0", true);
    assert!(has(&marks, [0.0, 0.0]));
    let (marks, _, _) = run("z = x^2 + y^2 + 0.00000000000001, z ≠ 0", false);
    assert!(marks.is_empty(), "{marks:?}");
}

#[test]
fn tangent_curve_exclusions_are_visible_in_both_2d_modes() {
    for slice in [false, true] {
        let (marks, _, _) = run_view("y = (x - 0.31)^2, y ≠ 0", true, slice);
        assert_eq!(marks, [[0.31, 0.0, 0.0]]);
        let (marks, _, _) = run_view("y = x, y ≠ 0", true, slice);
        assert_eq!(marks, [[0.0, 0.0, 0.0]]);
    }
}

#[test]
fn ordinary_curves_mark_finite_limits_at_undefined_points() {
    for (expr, expected) in [
        ("y = sin(x)/x", vec![[0.0, 1.0]]),
        ("y = tan(x)/x", vec![[0.0, 1.0]]),
        ("y = (1-cos(x))/x^2", vec![[0.0, 0.5]]),
        ("y = abs(x-0.31)/(x-0.31)", vec![[0.31, -1.0], [0.31, 1.0]]),
        ("y = (x-0.31)^2/(x-0.31)^2", vec![[0.31, 1.0]]),
        ("y = sin(3x)/sin(3x)", vec![[-std::f32::consts::PI / 3.0, 1.0], [0.0, 1.0], [std::f32::consts::PI / 3.0, 1.0]]),
        ("x = sin(y-0.31)/(y-0.31)", vec![[1.0, 0.31]]),
        ("y = @cases(-1, x < 0.31; 1, x > 0.31)", vec![[0.31, -1.0], [0.31, 1.0]]),
        ("y = @cases(2, x > 0.31)", vec![[0.31, 2.0]]),
        ("y = @cases(2, x ≥ 0.31)", vec![]),
        ("y = @cases(2, x = 0; sin(x)/x)", vec![]),
        ("y = 1/x", vec![]),
        ("y = sin(1/x)", vec![]),
    ] {
        let (marks, _, _) = run(expr, true);
        assert_eq!(marks.len(), expected.len(), "{expr}: {marks:?}");
        assert!(expected.into_iter().all(|p| has(&marks, p)), "{expr}: {marks:?}");
    }
}

#[test]
fn ordinary_surface_marks_only_isolated_holes_with_a_single_limit() {
    let (marks, _, _) = run("z = (x^2+y^2)/(x^2+y^2)", false);
    assert_eq!(marks, vec![[0.0, 0.0, 1.0]]);
    let (marks, _, _) = run("z = x^2/(x^2+y^2)", false);
    assert!(marks.is_empty(), "{marks:?}");
    let (marks, _, _) = run("z = (x+y)/(x+y)", false);
    assert!(marks.is_empty(), "{marks:?}");
}

/// 2D の切り口（z = 0）で、z ≠ 0 の曲線は全部除かれる（前は y = x がそのまま描かれていた）
#[test]
fn slice_fully_excluded() {
    let mut m = plot("y = x, z ≠ 0", &[-2.0, 2.0, -2.0, 2.0, -2.0, 2.0], 60, 40, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, true, 1, true, 0).ok().unwrap();
    assert!(m.positions().is_empty() && m.kind().contains("全部除かれる"), "{}", m.kind());
    assert!(m.take_holes().is_empty());
}

/// 接するだけの 2 本の等式（y = x², y = 0）も、3D で交わる線（z 軸）を描く（前は 0 個の点）
#[test]
fn tangent_surfaces_meet_on_a_line() {
    let m = plot("y = x^2, y = 0", &[-2.0, 2.0, -2.0, 2.0, -2.0, 2.0], 60, 40, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0).ok().unwrap();
    let p: Vec<f32> = m.positions();
    let pts: Vec<&[f32]> = p.chunks_exact(3).filter(|v| v[0].is_finite()).collect();
    assert!(pts.len() > 20, "{}", pts.len());
    assert!(pts.iter().all(|v| v[0].abs() < 1e-6 && v[1].abs() < 1e-6), "z 軸の上にない");
    let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v[2]), b.max(v[2])));
    assert!(lo <= -1.99 && hi >= 1.99, "{lo}..{hi}");
}

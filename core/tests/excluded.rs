//! ≠ で除いた所を、白丸（曲線）・点線（面・領域）で示す

use graph_core::plot;

fn run(src: &str, flat: bool) -> (Vec<[f32; 3]>, usize, String) {
    run_view(src, flat, flat)
}

fn graph(src: &str, flat: bool, slice: bool) -> graph_core::MeshResult {
    graph_rotated(src, flat, slice, &[0.0; 10])
}

const DOMAIN: [f64; 6] = [-2.0, 2.0, -2.0, 2.0, -2.0, 2.0];

fn graph_rotated(src: &str, flat: bool, slice: bool, rotation: &[f64; 10]) -> graph_core::MeshResult {
    plot(src, &DOMAIN, 60, 40, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, rotation, &[0.0; 5], 0.0, 0, false, flat, 1, slice, 0)
        .unwrap_or_else(|_| panic!("{src} がエラー"))
}

fn run_view(src: &str, flat: bool, slice: bool) -> (Vec<[f32; 3]>, usize, String) {
    let mut m = graph(src, flat, slice);
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

#[test]
fn white_marks_rotate_with_the_graph_and_survive_reprojection() {
    for (src, flat, slice, plane, forward, backward) in [
        ("z = (x - 0.3)^2 + (y + 0.7)^2, z ≠ 0", false, false, 1, [0.0, -0.7, 0.3], [0.0, -0.7, -0.3]),
        ("z = (x - 0.3)^2 + (y + 0.7)^2, z ≠ 0", true, false, 1, [0.0, -0.7, 0.0], [0.0, -0.7, 0.0]),
        ("z = (x^2+y^2)/(x^2+y^2)", false, false, 1, [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ("x = (y^2+z^2)/(y^2+z^2)", false, false, 0, [0.0, 1.0, 0.0], [0.0, -1.0, 0.0]),
        ("y = (x^2+z^2)/(x^2+z^2)", false, false, 0, [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ("y = tan(x)/x", true, false, 0, [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ("y = tan(x)/x", true, true, 0, [-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]),
        ("x = tan(y)/y", true, false, 0, [0.0, 1.0, 0.0], [0.0, -1.0, 0.0]),
        ("x = tan(y)/y", true, true, 0, [0.0, 1.0, 0.0], [0.0, -1.0, 0.0]),
        ("y = x, x ≠ 0.31", true, false, 0, [-0.31, 0.31, 0.0], [0.31, -0.31, 0.0]),
        ("y = (x - 0.31)^2, y ≠ 0", true, false, 0, [0.0, 0.31, 0.0], [0.0, -0.31, 0.0]),
    ] {
        let mut rotation = [0.0; 10];
        rotation[plane] = std::f64::consts::FRAC_PI_2;
        let mut initial = graph_rotated(src, flat, slice, &rotation);
        let assert_mark = |mark: Vec<f32>, expected: [f32; 3]| {
            assert_eq!(mark.len(), 3, "{src}: {mark:?}");
            assert!(mark.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-5), "{src}: {mark:?} != {expected:?}");
        };
        assert_mark(initial.take_holes(), forward);
        let shape = initial.take_shape().unwrap_or_else(|| panic!("{src}: no shape to reproject"));
        rotation[plane] = -rotation[plane];
        let mut next = shape.project(&DOMAIN, 3, &[0, 1, 2], &rotation, &[0.0; 5], 0.0).ok().unwrap();
        assert_mark(next.take_holes(), backward);
    }
}

#[test]
fn white_surface_marks_use_each_dependent_axis_in_projection_and_slice() {
    for (src, expected) in [
        ("x = (y-0.3)^2+z^2, x ≠ 0", [0.0, 0.3, 0.0]),
        ("y = (x-0.3)^2+z^2, y ≠ 0", [0.3, 0.0, 0.0]),
        ("z = (x-0.3)^2+y^2, z ≠ 0", [0.3, 0.0, 0.0]),
    ] {
        for slice in [false, true] {
            let mut g = graph(src, true, slice);
            assert_eq!(g.take_holes(), expected, "{src}: slice={slice}");

            let mut rotation = [0.0; 10];
            rotation[0] = std::f64::consts::FRAC_PI_6;
            let [x, y, _] = expected;
            let (sin, cos) = (rotation[0] as f32).sin_cos();
            let moved = [x * cos - y * sin, x * sin + y * cos, 0.0];
            let mut g = graph_rotated(src, true, slice, &rotation);
            let marks = g.take_holes();
            assert_eq!(marks.len(), 3, "{src}: slice={slice}, {marks:?}");
            assert!(marks.iter().zip(moved).all(|(a, b)| (a - b).abs() < 1e-5), "{src}: {marks:?}");
            if !slice {
                let shape = g.take_shape().unwrap();
                rotation[0] *= -1.0;
                let mut reprojection = shape.project(&DOMAIN, 3, &[0, 1, 2], &rotation, &[0.0; 5], 0.0).ok().unwrap();
                let marks = reprojection.take_holes();
                let back = [x * cos + y * sin, -x * sin + y * cos, 0.0];
                assert_eq!(marks.len(), 3, "{src}: {marks:?}");
                assert!(marks.iter().zip(back).all(|(a, b)| (a - b).abs() < 1e-5), "{src}: {marks:?}");
            }
        }
    }

    // Rotating the excluded point out of the slice removes it from that view.
    let mut rotation = [0.0; 10];
    rotation[1] = std::f64::consts::FRAC_PI_6;
    let mut g = graph_rotated("y = (x-0.3)^2+z^2, y ≠ 0", true, true, &rotation);
    assert!(g.take_holes().is_empty());
}

#[test]
fn rotated_paraboloid_keeps_the_excluded_origin_in_the_slice() {
    let domain = [-12.0, 12.0, -12.0, 12.0, -12.0, 12.0];
    for degrees in [47.0_f64, -71.0] {
        let mut rotation = [0.0; 10];
        rotation[1] = degrees.to_radians();
        let mut g = plot(
            "z = x^2+y^2, z ≠ 0", &domain, 150, 80, "", &[], "", &[], "", 3,
            &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 24, &rotation, &[0.0; 5],
            40.0, 0, false, true, 1, true, 0,
        ).unwrap_or_else(|_| panic!("xz {degrees}°"));
        assert!(!g.positions().is_empty(), "xz {degrees}°");
        assert_eq!(g.take_holes(), [0.0, 0.0, 0.0], "xz {degrees}°");
    }
}

#[test]
fn extruded_finite_holes_are_dashed_along_the_free_axis() {
    for (src, flat, free, fixed) in [
        ("y = tan(x)/x", false, 2, [0.0, 1.0, 0.0]),
        ("z = tan(x)/x", false, 1, [0.0, 0.0, 1.0]),
        ("z = tan(x)/x", true, 1, [0.0, 0.0, 0.0]),
        ("x = sin(z-0.31)/(z-0.31)", false, 1, [1.0, 0.0, 0.31]),
        ("y = (1-cos(x))/x^2", false, 2, [0.0, 0.5, 0.0]),
        ("y = @cases(1, x > 0.31)", false, 2, [0.31, 1.0, 0.0]),
    ] {
        let mut m = graph(src, flat, false);
        let dashed = m.take_outline_dashed();
        let Some((line, separator)) = dashed.split_last_chunk::<3>() else { panic!("{src}: no dashed line") };
        assert!(separator.iter().all(|v| v.is_nan()), "{src}");
        let points: Vec<_> = line.chunks_exact(3).collect();
        assert!(points.iter().all(|p| (0..3).all(|i| i == free || (p[i] - fixed[i]).abs() < 1e-5)), "{src}");
        assert_eq!(points.first().unwrap()[free], -2.0, "{src}");
        assert_eq!(points.last().unwrap()[free], 2.0, "{src}");
        assert!(m.take_holes().is_empty(), "{src}");
    }
}

#[test]
fn extruded_holes_separate_distinct_limits_and_skip_poles() {
    let dashed = graph("y = abs(x)/x", false, false).take_outline_dashed();
    let pts: Vec<_> = dashed.chunks_exact(3).collect();
    let branches: Vec<_> = pts.split(|p| p[0].is_nan()).filter(|line| !line.is_empty()).collect();
    assert_eq!(branches.len(), 2);
    for (line, y) in branches.into_iter().zip([-1.0, 1.0]) {
        assert!(line.iter().all(|p| p[0] == 0.0 && p[1] == y));
    }
    for src in ["y = 1/x", "y = tan(x)", "y = sin(1/x)", "y = x", "y = @cases(2, x = 0; sin(x)/x)"] {
        assert!(graph(src, false, false).take_outline_dashed().is_empty(), "{src}");
    }
    let (marks, dashed, _) = run("y = tan(x)/x", true);
    assert_eq!(marks, [[0.0, 1.0, 0.0]]);
    assert_eq!(dashed, 0);

    let dashed = graph("y = tan(x)/x, z ≥ 0.37", false, false).take_outline_dashed();
    let segments: Vec<_> = dashed.chunks_exact(3).filter(|p| p[0].is_finite()).collect();
    assert!((segments.first().unwrap()[2] - 0.37).abs() < 1e-4);
    assert_eq!(segments.last().unwrap()[2], 2.0);
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

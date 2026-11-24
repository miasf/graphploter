//! 集合と 2D の切り口（第 3 段階: #146 #147 #162 #144）。画面と同じ入口（`graph_core::plot`）で確かめる

use graph_core::{definitions, plot, MeshResult};

fn draw(src: &str, domain: [f64; 6], flat: bool, flat_slice: bool) -> Result<MeshResult, String> {
    definitions("");
    plot(src, &domain, 150, 80, "t", &[0.0, 5.0], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, flat, 1, flat_slice, 0)
        .map_err(|_| format!("{src} が描けない"))
}

/// 2D の点の集合（頂点の重なりを除いた (x, y)）
fn points(src: &str) -> Vec<(i64, i64)> {
    let m = draw(src, [-6.0, 6.0, -6.0, 12.0, -6.0, 6.0], true, false).unwrap();
    let mut v: Vec<(i64, i64)> = m
        .positions()
        .chunks(3)
        .filter(|c| c[0].is_finite())
        .map(|c| ((c[0] as f64 * 1e6).round() as i64, (c[1] as f64 * 1e6).round() as i64))
        .collect();
    v.sort();
    v.dedup();
    v.into_iter().map(|(x, y)| (x / 1_000_000, y / 1_000_000)).collect()
}

#[test]
fn integer_sets_respect_strict_bounds() {
    let sq = |xs: &[i64]| xs.iter().map(|&x| (x, x * x)).collect::<Vec<_>>();
    assert_eq!(points("⦃(x,y)∣x∈ℤ,y=x^(2),-3<x<3⦄"), sq(&[-2, -1, 0, 1, 2]));
    assert_eq!(points("⦃(x,y)∣x∈ℤ,y=x^(2),-3<=x<=3⦄"), sq(&[-3, -2, -1, 0, 1, 2, 3]));
    assert_eq!(points("⦃(x,y)∣x∈ℤ,y=x^(2),-3<x<=3⦄"), sq(&[-2, -1, 0, 1, 2, 3]));
    assert_eq!(points("⦃(x,y)∣x∈ℤ,y=x^(2),-3<=x<3⦄"), sq(&[-3, -2, -1, 0, 1, 2]));
    assert_eq!(points("⦃(x,y)∣x∈ℤ,y=x^(2),-3.1<=x<=3.1⦄"), sq(&[-3, -2, -1, 0, 1, 2, 3]));
}

#[test]
fn integer_points_on_the_axis() {
    let axis = |xs: std::ops::RangeInclusive<i64>| xs.map(|x| (x, 0)).collect::<Vec<_>>();
    assert_eq!(points("⦃(x,y)∣x∈ℤ,y=0,-3<=x<=3⦄"), axis(-3..=3));
    assert_eq!(points("⦃(x,0)∣x∈ℤ,-3<=x<=3⦄"), axis(-3..=3));
    assert_eq!(points("⦃(x,y)∣x∈ℕ,y=0,1<=x<=3⦄"), axis(1..=3));
    assert_eq!(points("⦃(x,y)∣x∈ℕ,y=0,x<=3⦄"), axis(1..=3));
    // 格子点
    let grid: Vec<(i64, i64)> = (-1..=1).flat_map(|x| (-1..=1).map(move |y| (x, y))).collect();
    assert_eq!(points("⦃(x,y)∣x∈ℤ,y∈ℤ,-1<=x<=1,-1<=y<=1⦄"), grid);
}

/// 2D の切り口（縦の座標 = 0）で、z = 0 と重なる図形が消えない（#162）
#[test]
fn slices_on_the_plane() {
    let d = [-3.0, 3.0, -3.0, 3.0, -3.0, 3.0];
    let slice = |src: &str| draw(src, d, true, true);
    let tris = |src: &str| slice(src).map(|m| m.indices().len() / 3).unwrap_or(0);
    let verts = |src: &str| slice(src).map(|m| m.positions().chunks(3).filter(|c| c[0].is_finite()).count()).unwrap_or(0);
    // z = 0: 平面全体（2D の画面いっぱいの領域）
    assert!(tris("z=0") > 0, "z=0 の切り口は平面全体");
    // z = 0 の上の単位円板・単位円
    assert!(tris("z=0,x^(2)+y^(2)<=1") > 0, "単位円板");
    let circle = slice("z=0,x^(2)+y^(2)=1").unwrap();
    let on = circle.positions().chunks(3).filter(|c| c[0].is_finite()).all(|c| ((c[0] * c[0] + c[1] * c[1]) as f64 - 1.0).abs() < 1e-2);
    assert!(verts("z=0,x^(2)+y^(2)=1") > 20 && on, "単位円周");
    // z = 1: 空
    assert_eq!(verts("z=1"), 0);
    assert_eq!(tris("z=1,x^(2)+y^(2)<=1"), 0);
    // 今までどおり: 球の切り口は円、z を使わない式はそのまま
    let ball = slice("x^(2)+y^(2)+z^(2)=4").unwrap();
    assert!(ball.positions().chunks(3).filter(|c| c[0].is_finite()).all(|c| ((c[0] * c[0] + c[1] * c[1]) as f64 - 4.0).abs() < 5e-2));
    assert!(verts("y=x^(2)") > 20);
    assert!(verts("z=x^(2)+y^(2)-1") > 20, "放物面の切り口は円");
}

/// 座標系の作る範囲の端ちょうど（φ = π。f32 では π より少し大きい）の面を、範囲の外として落とさない（#144）
#[test]
fn range_end_in_f32_is_inside() {
    use graph_core::coords::trim_to_domain;
    use graph_core::mesh::{Domain, Mesh};
    let pi = std::f64::consts::PI;
    for z in [0.0f32, pi as f32] {
        let mut m = Mesh { positions: vec![0.0, 0.0, z, 1.0, 0.0, z, 0.0, 1.0, z], indices: vec![0, 1, 2], ..Default::default() };
        trim_to_domain(&mut m, Domain { lo: [0.0, 0.0, 0.0], hi: [5.0, 2.0 * pi, pi] }, false, false);
        assert_eq!(m.indices.len(), 3, "φ = {z} の面が落ちた");
    }
}

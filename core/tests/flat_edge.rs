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
    // 寝ている面（z = x² + y² を陰関数で）は、真上から見ても面積があるので、縁（縦に近い所）だけ
    let (lines, _) = outline("z - x^2 - y^2 = 0");
    let pts = lines.chunks_exact(3).filter(|p| p[0].is_finite()).count();
    assert!(pts < 20, "寝ている面に線が {pts} 点");
}

//! 範囲を共有する略記（−1 ≤ x, y, z ≤ 1 など）

use graph_core::plot;

/// (説明, 頂点の外接箱)
fn shape(src: &str) -> (String, [f32; 3], [f32; 3]) {
    let m = plot(src, &[-2.0, 2.0, -2.0, 2.0, -2.0, 2.0], 60, 40, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0)
        .unwrap_or_else(|_| panic!("{src} がエラー"));
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in m.positions().chunks_exact(3) {
        for a in 0..3 {
            lo[a] = lo[a].min(v[a]);
            hi[a] = hi[a].max(v[a]);
        }
    }
    (m.kind(), lo, hi)
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 1e-3)
}

#[test]
fn shared_bounds() {
    for src in ["-1 <= x, y, z <= 1", "⦃(x,y,z)∣-1<=x,y,z<=1⦄"] {
        let (kind, lo, hi) = shape(src);
        assert!(kind.contains("不等式の領域（6 本）"), "{src}: {kind}");
        assert!(close(lo, [-1.0; 3]) && close(hi, [1.0; 3]), "{src}: {lo:?}..{hi:?}");
    }
    // 片側だけ: x, y, z ≥ 0（と x + y + z ≤ 1 で 4 面体）
    let (_, lo, hi) = shape("x, y, z >= 0, x + y + z <= 1");
    assert!(close(lo, [0.0; 3]) && close(hi, [1.0; 3]), "{lo:?}..{hi:?}");
}

//! 重積分だけの行は、積分する領域を描き、値を説明に出す

use graph_core::plot;

fn run(src: &str) -> (String, [f32; 3], [f32; 3]) {
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
    (0..3).all(|k| (a[k] - b[k]).abs() < 2e-3)
}

#[test]
fn integral_regions_are_drawn() {
    // ∬_{0 ≤ y ≤ x ≤ 1} xy dA = 1/8: 平面 z = 0 の上の三角形
    for src in ["@intset((x,y),0<=x<=1,0<=y<=x)(x y)", "@int(x,0,1)@int(y,0,x)(x y)"] {
        let (kind, lo, hi) = run(src);
        assert_eq!(kind, "2 重積分の領域（値 0.125）", "{src}");
        assert!(close(lo, [0.0; 3]) && close(hi, [1.0, 1.0, 0.0]), "{src}: {lo:?}..{hi:?}");
    }
    // ∭ 円柱 1 dV = π
    let (kind, lo, hi) = run("@intset((x,y,z),x^2+y^2<=1,0<=z<=1)(1)");
    assert_eq!(kind, "3 重積分の領域（値 3.1415926536）");
    assert!(close(lo, [-1.0, -1.0, 0.0]) && close(hi, [1.0; 3]), "{lo:?}..{hi:?}");
    // 1 重の積分・線積分は、今までどおり
    assert!(!run("@int(x,0,1)(x^2)").0.contains("重積分"));
}

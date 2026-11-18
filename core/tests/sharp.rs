//! 陰関数の折れ目（鋭い辺・角）を立てる（計画 `docs/plan-render-algorithms.md` の P2-1。テスト表 S-11）。
//! `max(|x|, |y|, |z|) = 1` の立方体で、角の点・辺の上の点から、いちばん近い頂点までの距離を測る

use graph_core::plot;

fn positions(src: &str, r: f64, ires: u32) -> Vec<f32> {
    let m = plot(src, &[-r, r, -r, r, -r, r], 60, ires, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0)
        .unwrap_or_else(|_| panic!("{src}"));
    m.positions()
}

fn nearest(p: &[f32], q: [f64; 3]) -> f64 {
    p.chunks_exact(3)
        .map(|v| ((v[0] as f64 - q[0]).powi(2) + (v[1] as f64 - q[1]).powi(2) + (v[2] as f64 - q[2]).powi(2)).sqrt())
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn cube_corners_and_edges_are_sharp() {
    for ires in [40u32, 80] {
        let p = positions("max(abs(x), abs(y), abs(z)) = 1", 2.0, ires);
        let h = 4.0 / ires as f64;
        // 8 つの角: 頂点がちょうど角に乗る（前は 0.7〜1.15 マス内側に丸まっていた）
        for c in 0..8 {
            let q = [0, 1, 2].map(|k| if c >> k & 1 == 1 { 1.0 } else { -1.0 });
            let d = nearest(&p, q);
            assert!(d < 0.01 * h, "格子 {ires}: 角 {q:?} から {d}");
        }
        // 12 本の辺: 辺の上の点からの距離は、辺に沿った頂点の間隔（1 マス）の半分ほど（前は平均 0.37〜0.7 マス）
        let mut sum = 0.0;
        let mut count = 0;
        for a in 0..3 {
            for s in [[-1.0, -1.0], [-1.0, 1.0], [1.0, -1.0], [1.0, 1.0]] {
                for k in 0..21 {
                    let mut q = [0.0; 3];
                    q[a] = -1.0 + 2.0 * k as f64 / 20.0;
                    q[(a + 1) % 3] = s[0];
                    q[(a + 2) % 3] = s[1];
                    let d = nearest(&p, q);
                    assert!(d <= 0.55 * h, "格子 {ires}: 辺の点 {q:?} から {d}");
                    sum += d;
                    count += 1;
                }
            }
        }
        assert!(sum / count as f64 <= 0.35 * h, "格子 {ires}: 辺の平均 {}", sum / count as f64);
    }
}

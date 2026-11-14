//! 表示していない座標（w 以降）の見せ方（計画 11 章の H1）。画面と同じ入口（`graph_core::plot`）で確かめる

use graph_core::{equation, hidden_style, plot, MeshResult};

const R: f64 = 3.0;

/// `dims` 次元で x, y, z を表示（断面の値は全部 0）。`style` を [`hidden_style`] で渡してから描く
fn run(src: &str, dims: u32, style: &[f64]) -> Result<MeshResult, ()> {
    let n = dims as usize;
    let rd = equation::rot_dims(n);
    hidden_style(style.to_vec());
    plot(
        src, &[-R, R, -R, R, -R, R], 48, 40, "", &[], "", &[], "", dims, &[0, 1, 2], &vec![0.0; n], &vec![f64::NAN; 2 * n], "",
        &[], 16, &vec![0.0; rd * (rd - 1) / 2], &vec![0.0; rd], 0.0, 0, false, false, 1, false, 0,
    )
    .map_err(|_| ())
}

/// 見せ方・枚数・座標ごとの範囲（指定しない座標は NaN）
fn style(mode: f64, count: f64, dims: usize, ranges: &[(usize, f64, f64)]) -> Vec<f64> {
    let mut v = vec![mode, count];
    v.extend(std::iter::repeat_n(f64::NAN, 2 * dims));
    for &(k, a, b) in ranges {
        v[2 + 2 * k] = a;
        v[3 + 2 * k] = b;
    }
    v
}

#[test]
fn layers_of_level_sets() {
    let m = run("w = x^2 + y^2 + z^2", 4, &style(1.0, 5.0, 4, &[(3, 1.0, 5.0)])).unwrap();
    assert_eq!(m.kind(), "陰関数（w の値ごとに 5 枚）");
    assert_eq!(m.value_range(), vec![1.0, 5.0]);
    let (p, c) = (m.positions(), m.complex_values());
    assert_eq!(c.len(), p.len());
    let mut seen = std::collections::BTreeSet::new();
    for (q, v) in p.chunks_exact(3).zip(c.chunks_exact(3)) {
        let w = v[0] as f64;
        seen.insert((w * 10.0).round() as i64);
        let r2 = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]) as f64;
        assert!((r2 - w).abs() < 0.05 * w.max(1.0), "{q:?} {w}");
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), vec![10, 20, 30, 40, 50]);
    // 2 つの座標を使う式（5 次元）: 全部の組み合わせ
    let m = run("w = x + w_2 z", 5, &style(1.0, 3.0, 5, &[(3, -1.0, 1.0), (4, 0.0, 2.0)])).unwrap();
    assert!(m.kind().ends_with("（w の値ごとに 9 枚）"), "{}", m.kind());
    // 表示していない座標を使わない式は、今までと同じ
    let a = run("x^2 + y^2 + z^2 = 4", 4, &style(1.0, 5.0, 4, &[(3, 1.0, 5.0)])).unwrap();
    let b = run("x^2 + y^2 + z^2 = 4", 4, &[]).unwrap();
    assert_eq!(a.positions(), b.positions());
    assert!(a.value_range().is_empty());
}

#[test]
fn planes_by_value() {
    let m = run("w = x + 2y - z", 4, &style(2.0, 3.0, 4, &[(3, -5.0, 5.0)])).unwrap();
    assert_eq!(m.kind(), "w の値で断面を塗り分け（9 枚）");
    let (p, c) = (m.positions(), m.complex_values());
    assert!(!m.indices().is_empty());
    for (q, v) in p.chunks_exact(3).zip(c.chunks_exact(3)) {
        assert!((v[0] - (q[0] + 2.0 * q[1] - q[2])).abs() < 1e-4);
    }
    // 形が違う式はエラー（エラーの JsError はネイティブでは作れないので、判定の関数を直接呼ぶ）
    let sp = equation::Space::basic(4, [0, 1, 2], vec![0.0; 4]);
    let field = |src: &str| equation::hidden_field(src, &Default::default(), &sp).map(|(_, k)| k);
    assert_eq!(field("w = x + 2y - z"), Ok(3));
    assert_eq!(field("x y = w"), Ok(3));
    for src in ["x^2 + w^2 = 1", "w = w x", "z = x + w", "w = [1, 2] x", "w = x, z = 0"] {
        assert!(field(src).is_err(), "{src}");
    }
}

#[test]
fn volume_values() {
    let mut m = run("w = x y z", 4, &style(3.0, 5.0, 4, &[(3, -1.0, 1.0)])).unwrap();
    let n = m.volume_n() as usize;
    assert_eq!(n, 40);
    assert_eq!(m.kind(), "w の値の伝達関数（40³）");
    let v = m.take_volume();
    assert_eq!(v.len(), n * n * n);
    let at = |i: usize| -R + 2.0 * R * i as f64 / (n - 1) as f64;
    for (x, y, z) in [(0, 0, 0), (5, 17, 39), (39, 2, 11)] {
        let want = at(x) * at(y) * at(z);
        assert!((v[(z * n + y) * n + x] as f64 - want).abs() < 1e-4, "{x} {y} {z}");
    }
    // 呼ぶのは 1 回だけ: 次の plot はふつうの断面
    let m = run("w = x y z", 4, &[]).unwrap();
    assert_eq!(m.volume_n(), 0);
}

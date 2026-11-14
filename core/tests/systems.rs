//! いくつかの式が同時に 0 になる所（線・点だけの解）を、画面と同じ入口（`graph_core::plot`）で描く

use graph_core::{equation, plot, MeshResult};

/// 表示範囲 ±3 の 3 次元（`dims` 次元。4 次元なら座標 0 と 3 の組を `xw` だけ回す）
fn run(src: &str, dims: u32, xw: f64) -> MeshResult {
    run_with(src, 3.0, 40, dims, 0.0, xw)
}

/// 表示範囲 ±r、陰関数の細かさ `ires`。4 次元以上なら、表示していない座標の断面の値は `w`、座標 0 と 3 の組を `xw` だけ回す
fn run_with(src: &str, r: f64, ires: u32, dims: u32, w: f64, xw: f64) -> MeshResult {
    let rot = [(2, xw)];
    run_full(src, r, ires, dims, w, if dims > 3 { &rot } else { &[] }, 0)
}

/// [`run_with`] で、回転を (座標の組の番号, 角度) で決め（組は (0,1), (0,2), (0,3), … の順）、複素数の色分け `complex` を付ける版
fn run_full(src: &str, r: f64, ires: u32, dims: u32, w: f64, rot: &[(usize, f64)], complex: u32) -> MeshResult {
    let n = dims as usize;
    let rd = equation::rot_dims(n);
    let mut rotation = vec![0.0; rd * (rd - 1) / 2];
    for &(k, a) in rot {
        rotation[k] = a;
    }
    let mut slice = vec![0.0; n];
    for v in slice.iter_mut().skip(3) {
        *v = w;
    }
    plot(
        src, &[-r, r, -r, r, -r, r], 64, ires, "", &[], "", &[], "", dims, &[0, 1, 2], &slice,
        &vec![f64::NAN; 2 * n], "", &[], 16, &rotation, &vec![0.0; rd], 0.0, complex, false, false, 1, false, 0,
    )
    .unwrap_or_else(|e| panic!("{src} が計算できない: {:?}", e))
}

/// 線分の端（NaN で区切った点）
fn ends(m: &MeshResult) -> Vec<[f32; 3]> {
    m.positions().chunks_exact(3).filter(|p| p[0].is_finite()).map(|p| [p[0], p[1], p[2]]).collect()
}

#[test]
fn lines_and_points_are_drawn_as_curves() {
    // z 軸
    let m = run("x^2 + y^2 = 0", 3, 0.0);
    assert!(m.curve());
    assert_eq!(m.kind(), "陰関数（線だけの解）");
    assert!(!m.local_available());
    let pts = ends(&m);
    assert!(pts.len() > 20 && pts.iter().all(|p| p[0].abs() < 1e-5 && p[1].abs() < 1e-5), "{}", pts.len());
    let (lo, hi) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p[2]), b.max(p[2])));
    assert!(lo < -2.9 && hi > 2.9, "{lo} {hi}");
    // 原点 1 つ
    let m = run("x^2 + y^2 + z^2 = 0", 3, 0.0);
    assert_eq!(m.kind(), "陰関数（点だけの解）");
    assert_eq!(ends(&m).len(), 2);
    // 面になる式は今までどおり
    let m = run("x^2 + y^2 = 1", 3, 0.0);
    assert!(!m.curve());
    assert_eq!(m.kind(), "陰関数");
}

#[test]
fn rotated_line_in_four_dimensions() {
    // w = 0 の断面で、xw に 0.6 rad 回した x² + y² = 0 も z 軸（回した後の x は cos(0.6) x + … で、w = 0）
    let m = run("x^2 + y^2 = 0", 4, 0.6);
    assert!(m.curve());
    let pts = ends(&m);
    assert!(pts.len() > 20 && pts.iter().all(|p| p[0].abs() < 1e-4 && p[1].abs() < 1e-4), "{}", pts.len());
    // y = x, z = w の線（w の断面は 0）: (y − x)² + (z − w)² = 0
    let m = run("(y - x)^2 + (z - w)^2 = 0", 4, 0.0);
    let pts = ends(&m);
    assert!(pts.len() > 20 && pts.iter().all(|p| (p[1] - p[0]).abs() < 1e-4 && p[2].abs() < 1e-4), "{}", pts.len());
}

#[test]
#[ignore]
fn line_report() {
    for src in [
        "x^2 + y^2 = 0",
        "(x^2 + y^2 - 4)^2 + z^2 = 0",
        "(x^2 + y^2 - 4)^2 + (z - x)^2 = 0",
        "(x^2 + y^2 + z^2 - 4)^2 + (x^2 + y^2 - 2 x)^2 = 0",
        "(sin(x) - cos(y))^2 + (cos(y) - tan(z))^2 = 0",
        "x^2 + y^2 + z^2 = 0",
    ] {
        let t = std::time::Instant::now();
        let m = run(src, 3, 0.0);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        println!("{src}\t{}\t{} 点\t{ms:.1} ms", m.kind(), ends(&m).len());
    }
}

/// `tools/visual/cases.tsv` の式のうち、線・点だけの解として描くもの（一覧を確かめるために手で回す）
#[test]
#[ignore]
fn cases_taking_the_line_path() {
    let list = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../tools/visual/cases.tsv")).unwrap();
    for line in list.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let src = line.splitn(3, '\t').nth(2).unwrap_or("");
        if let Ok(m) = std::panic::catch_unwind(|| run(src, 3, 0.0)) {
            if m.kind().contains("だけの解") {
                println!("{src}\t{}", m.kind());
            }
        }
    }
}


// ---- 線の物差し（計画 8.1 の測り方。表示範囲 ±5、陰関数の細かさ 80、h = 0.125） ----

const R: f64 = 5.0;
const N: u32 = 80;
const H: f64 = 2.0 * R / N as f64;

/// 線の物差しの値
#[derive(Debug)]
struct CurveMetrics {
    /// 正解の点のうち、描いた線から 0.25h より遠いものの割合（%）
    missing: f64,
    /// その距離の最大（h の倍数。2h より遠い所は 2h とする）
    missing_max: f64,
    /// 描いた線から正解までの距離の最大（h の倍数。同上）
    offset_max: f64,
    /// 描いた線のうち、正解から 0.25h より遠い部分の長さ
    false_len: f64,
    /// 線分の端のうち、ほかの線分とつながっていないもの（表示範囲の端から 1.5h 以内は除く）
    breaks: usize,
    /// 描いた線の長さ
    length: f64,
}

/// 点の集まりの、格子（幅 h）で引ける表
struct Grid {
    cells: std::collections::HashMap<[i64; 3], Vec<usize>>,
    pts: Vec<[f64; 3]>,
}

impl Grid {
    fn new(pts: Vec<[f64; 3]>) -> Grid {
        let mut cells: std::collections::HashMap<[i64; 3], Vec<usize>> = Default::default();
        for (i, p) in pts.iter().enumerate() {
            cells.entry(p.map(|v| (v / H).floor() as i64)).or_default().push(i);
        }
        Grid { cells, pts }
    }

    /// いちばん近い点までの距離（2h より遠ければ 2h）
    fn nearest(&self, p: [f64; 3]) -> f64 {
        let c = p.map(|v| (v / H).floor() as i64);
        let mut best = 2.0 * H;
        for dx in -2..=2 {
            for dy in -2..=2 {
                for dz in -2..=2 {
                    if let Some(ids) = self.cells.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) {
                        for &i in ids {
                            let q = self.pts[i];
                            best = best.min(((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt());
                        }
                    }
                }
            }
        }
        best
    }
}

/// 媒介変数で書いた正解の線を、0.002 以下の間隔の点の列にする（表示範囲の外の点は捨てる）
fn truth(f: impl Fn(f64) -> [f64; 3], t0: f64, t1: f64) -> Vec<[f64; 3]> {
    let mut out = Vec::new();
    let mut t = t0;
    let mut p = f(t);
    while t <= t1 {
        if p.iter().all(|v| v.abs() <= R) {
            out.push(p);
        }
        // 次の点まで 0.002 以下になる刻み
        let mut dt = 1e-3;
        loop {
            let q = f(t + dt);
            let d = ((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2) + (q[2] - p[2]).powi(2)).sqrt();
            if d <= 0.002 || dt < 1e-9 {
                t += dt;
                p = q;
                break;
            }
            dt *= 0.5;
        }
    }
    out
}

fn curve_metrics(positions: &[f32], truth: &[[f64; 3]]) -> CurveMetrics {
    let segs: Vec<([f32; 3], [f32; 3])> = positions
        .chunks_exact(9)
        .filter(|s| s[..6].iter().all(|v| v.is_finite()))
        .map(|s| ([s[0], s[1], s[2]], [s[3], s[4], s[5]]))
        .collect();
    let f64s = |a: [f32; 3]| a.map(|v| v as f64);
    // 描いた線の上の点（0.004 以下の間隔）と、その点が代表する長さ
    let mut drawn = Vec::new();
    let mut weight = Vec::new();
    let mut length = 0.0;
    for &(a, b) in &segs {
        let (a, b) = (f64s(a), f64s(b));
        let len = ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2) + (b[2] - a[2]).powi(2)).sqrt();
        length += len;
        let k = (len / 0.004).ceil().max(1.0) as usize;
        for i in 0..=k {
            let s = i as f64 / k as f64;
            let p = [0, 1, 2].map(|c| a[c] + s * (b[c] - a[c]));
            // 表示範囲の外は描画側で切り落とすので、測らない
            if p.iter().any(|v| v.abs() > R) {
                continue;
            }
            drawn.push(p);
            weight.push(if i == 0 || i == k { 0.5 } else { 1.0 } * len / k as f64);
        }
    }
    let dg = Grid::new(drawn.clone());
    let tg = Grid::new(truth.to_vec());
    let tol = 0.25 * H;
    let mut missing = 0;
    let mut missing_max: f64 = 0.0;
    for p in truth {
        let d = dg.nearest(*p);
        if d > tol {
            missing += 1;
            missing_max = missing_max.max(d);
        }
    }
    let mut offset_max: f64 = 0.0;
    let mut false_len = 0.0;
    for (p, w) in drawn.iter().zip(&weight) {
        let d = tg.nearest(*p);
        offset_max = offset_max.max(d);
        if d > tol {
            false_len += w;
        }
    }
    // 途切れ: 端の点（f32 のビット）が、ほかの線分の端と同じでないもの
    let mut count: std::collections::HashMap<[u32; 3], usize> = Default::default();
    for &(a, b) in &segs {
        *count.entry(a.map(f32::to_bits)).or_default() += 1;
        *count.entry(b.map(f32::to_bits)).or_default() += 1;
    }
    let breaks = count
        .iter()
        .filter(|(k, &n)| n == 1 && k.iter().all(|b| (R - f32::from_bits(*b).abs() as f64) > 1.5 * H))
        .count();
    CurveMetrics {
        missing: 100.0 * missing as f64 / truth.len().max(1) as f64,
        missing_max: missing_max / H,
        offset_max: offset_max / H,
        false_len,
        breaks,
        length,
    }
}

/// 物差しにかける式（両方の順番）と、正解の線
fn curve_cases() -> Vec<(&'static str, &'static str, Vec<[f64; 3]>)> {
    use std::f64::consts::PI;
    let line = |f: fn(f64) -> [f64; 3]| truth(f, -6.0, 6.0);
    let mut sct = Vec::new();
    for k in -2..=2 {
        for j in -2..=2 {
            for s in [-1.0, 1.0] {
                sct.extend(truth(
                    |t| [t, s * (PI / 2.0 - t) + 2.0 * PI * k as f64, t.sin().atan() + PI * j as f64],
                    -R,
                    R,
                ));
            }
        }
    }
    let r198 = (4.0f64 - 1.98 * 1.98).sqrt();
    vec![
        ("y = x", "z = 0", line(|t| [t, t, 0.0])),
        ("y = x", "x = z", line(|t| [t, t, t])),
        ("x^2+y^2 = 4", "z = x", truth(|t| [2.0 * t.cos(), 2.0 * t.sin(), 2.0 * t.cos()], 0.0, 2.0 * PI)),
        ("z = 0", "x^2+y^2 = 4", truth(|t| [2.0 * t.cos(), 2.0 * t.sin(), 0.0], 0.0, 2.0 * PI)),
        ("x^2+y^2+z^2 = 4", "x^2+y^2 = 2x", truth(|t| [1.0 + t.cos(), t.sin(), 2.0 * (t / 2.0).sin()], 0.0, 4.0 * PI)),
        ("x^2+y^2 = 1", "x^2+z^2 = 1", {
            let mut v = truth(|t| [t.cos(), t.sin(), t.sin()], 0.0, 2.0 * PI);
            v.extend(truth(|t| [t.cos(), t.sin(), -t.sin()], 0.0, 2.0 * PI));
            v
        }),
        ("sin(x) = cos(y)", "cos(y) = tan(z)", sct),
        ("x^2+y^2+z^2 = 4", "z = 1.98", truth(move |t| [r198 * t.cos(), r198 * t.sin(), 1.98], 0.0, 2.0 * PI)),
        ("z = 0.03x", "z = 0", line(|t| [0.0, t, 0.0])),
    ]
}

/// 線の物差しの表（両方の順番）。`cargo test --release --test systems curve_report -- --ignored --nocapture`
#[test]
#[ignore]
fn curve_report() {
    println!("式\t欠け %\t欠けの最大 h\tずれの最大 h\t偽の線\t途切れ\t長さ\t時間 ms\tF1 だけ ms");
    for (a, b, t) in curve_cases() {
        for (p, q) in [(a, b), (b, a)] {
            let src = format!("{p}, {q}");
            let mut best = f64::INFINITY;
            let mut m = None;
            for _ in 0..3 {
                let t0 = std::time::Instant::now();
                let r = run_with(&src, R, N, 3, 0.0, 0.0);
                best = best.min(t0.elapsed().as_secs_f64() * 1e3);
                m = Some(r);
            }
            let mut only = f64::INFINITY;
            for _ in 0..3 {
                let t0 = std::time::Instant::now();
                // 陽関数にならないよう、陰関数の形にして描く（連立の 1 本目は、いつも陰関数の道で描くので）
                let (l, r) = p.split_once('=').unwrap();
                run_with(&format!("{l} - ({r}) = 0"), R, N, 3, 0.0, 0.0);
                only = only.min(t0.elapsed().as_secs_f64() * 1e3);
            }
            let c = curve_metrics(&m.unwrap().positions(), &t);
            println!(
                "{src}\t{:.2}\t{:.2}\t{:.3}\t{:.3}\t{}\t{:.3}\t{best:.1}\t{only:.1}",
                c.missing, c.missing_max, c.offset_max, c.false_len, c.breaks, c.length
            );
        }
    }
}

/// 交わる線の物差しの回帰（計画 S29 の完了の条件の一部）
#[test]
fn curve_regression() {
    for (a, b, t) in curve_cases() {
        let exact = matches!(a, "y = x" | "x^2+y^2 = 4" | "z = 0" | "z = 0.03x");
        let rough = matches!(a, "x^2+y^2 = 1") || (a == "x^2+y^2+z^2 = 4" && b != "z = 1.98");
        let sct = a.starts_with("sin");
        if !(exact || rough || sct) {
            continue;
        }
        for (p, q) in [(a, b), (b, a)] {
            let src = format!("{p}, {q}");
            let m = run_with(&src, R, N, 3, 0.0, 0.0);
            assert_eq!(m.kind(), "連立方程式の曲線");
            let c = curve_metrics(&m.positions(), &t);
            assert_eq!(c.breaks, 0, "{src}: {c:?}");
            if exact {
                assert!(c.missing == 0.0 && c.offset_max <= 0.05 && c.false_len == 0.0, "{src}: {c:?}");
            } else if rough {
                assert!(c.missing <= 3.0 && c.missing_max <= 0.6 && c.false_len <= 0.2, "{src}: {c:?}");
            } else {
                assert!(c.missing <= 0.5 && c.false_len <= 0.1, "{src}: {c:?}");
            }
        }
    }
}

#[test]
fn systems_in_four_dimensions_and_lists() {
    // y = x, z = w（w の断面の値 1）は、3 次元の y = x, z = 1 と同じ線
    let a = run_with("y = x, z = w", R, N, 4, 1.0, 0.0);
    let b = run_with("y = x, z = 1", R, N, 3, 0.0, 0.0);
    let t = truth(|t| [t, t, 1.0], -6.0, 6.0);
    for m in [&a, &b] {
        let c = curve_metrics(&m.positions(), &t);
        assert!(c.missing == 0.0 && c.false_len == 0.0 && c.breaks == 0, "{c:?}");
    }
    // xw に 0.6 rad 回した y = x, z = 0 も、空にならず、途切れない
    let m = run_with("y = x, z = 0", R, N, 4, 0.0, 0.6);
    let c = curve_metrics(&m.positions(), &[]);
    assert!(c.length > 1.0 && c.breaks == 0, "{c:?}");
    // リスト: 2 本の線
    let m = run_with("y = [1, 2] x, z = 0", R, N, 3, 0.0, 0.0);
    assert_eq!(m.kind(), "連立方程式の曲線 × 2");
    assert!(m.curve());
}


// ---- 切り取り（計画 S30） ----

/// 辺ごとの三角形の数
fn edge_counts(idx: &[u32]) -> std::collections::HashMap<(u32, u32), usize> {
    let mut out: std::collections::HashMap<(u32, u32), usize> = Default::default();
    for t in idx.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            *out.entry((a.min(b), a.max(b))).or_default() += 1;
        }
    }
    out
}

fn vert(p: &[f32], i: u32) -> [f64; 3] {
    let i = i as usize;
    [p[3 * i] as f64, p[3 * i + 1] as f64, p[3 * i + 2] as f64]
}

fn area(p: &[f32], idx: &[u32]) -> f64 {
    idx.chunks_exact(3)
        .map(|t| {
            let (a, b, c) = (vert(p, t[0]), vert(p, t[1]), vert(p, t[2]));
            let (u, v) = ([0, 1, 2].map(|k| b[k] - a[k]), [0, 1, 2].map(|k| c[k] - a[k]));
            let x = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            0.5 * (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt()
        })
        .sum()
}

/// 縁の辺（三角形が 1 つだけの辺）の頂点
fn border_vertices(idx: &[u32]) -> Vec<u32> {
    let mut v: Vec<u32> = edge_counts(idx).into_iter().filter(|(_, n)| *n == 1).flat_map(|((a, b), _)| [a, b]).collect();
    v.sort_unstable();
    v.dedup();
    v
}

#[test]
fn hemisphere_is_cut_at_the_plane() {
    let m = run_with("x^2 + y^2 + z^2 = 4, z ≥ 0", R, N, 3, 0.0, 0.0);
    assert_eq!(m.kind(), "陰関数（不等式で切り取り）");
    let (p, idx) = (m.positions(), m.indices());
    assert!(p.chunks_exact(3).all(|q| q[2] >= -1e-3));
    let border = border_vertices(&idx);
    assert!(!border.is_empty());
    for v in border {
        let q = vert(&p, v);
        assert!(q[2].abs() <= 1e-3 && ((q[0] * q[0] + q[1] * q[1]).sqrt() - 2.0).abs() <= 0.02, "{q:?}");
    }
    let a = area(&p, &idx);
    assert!((a / (8.0 * std::f64::consts::PI) - 1.0).abs() <= 0.01, "{a}");
    assert!(edge_counts(&idx).values().all(|&n| n <= 2));
}

#[test]
fn explicit_surface_is_trimmed() {
    let m = run_with("z = x^2 + y^2, z ≤ 4", R, N, 3, 0.0, 0.0);
    assert_eq!(m.kind(), "z = f(x, y)（不等式で切り取り）");
    let p = m.positions();
    assert!(p.chunks_exact(3).filter(|q| q.iter().all(|v| v.abs() <= R as f32)).all(|q| q[2] <= 4.0 + 1e-3));
    assert_eq!(m.normals().len(), p.len());
    let g = m.grid_coords();
    assert!(g.is_empty() || g.len() == p.len());
    // 複素数の色分け（高さ |f|）
    let m = run_full("z = (x + i y)^2 / 10, x ≥ 0", R, N, 3, 0.0, &[], 1);
    let p = m.positions();
    assert_eq!(m.complex_values().len(), p.len());
    assert!(p.chunks_exact(3).all(|q| q[0] >= -1e-3));
}

#[test]
fn trim_is_conforming() {
    let m = run_with("x^2 + y^2 + z^2 = 9, x + y + z ≤ 1, x ≥ -1", R, N, 3, 0.0, 0.0);
    let (p, idx) = (m.positions(), m.indices());
    let border = border_vertices(&idx);
    assert!(!border.is_empty());
    let s3 = 3f64.sqrt();
    for v in border {
        let q = vert(&p, v);
        let d1 = ((q[0] + q[1] + q[2] - 1.0) / s3).abs();
        let d2 = (q[0] + 1.0).abs();
        assert!(d1.min(d2) <= 1e-3, "{q:?}");
    }
}

#[test]
fn line_segment_by_inequalities() {
    let m = run_with("y = x, z = 0, 0 ≤ x ≤ 1", R, N, 3, 0.0, 0.0);
    assert_eq!(m.kind(), "連立方程式の曲線（不等式で切り取り）");
    let c = curve_metrics(&m.positions(), &[]);
    assert!((c.length - 2f64.sqrt()).abs() <= 1e-3, "{c:?}");
    let xs: Vec<f32> = m.positions().chunks_exact(3).filter(|q| q[0].is_finite()).map(|q| q[0]).collect();
    let (lo, hi) = xs.iter().fold((f32::MAX, f32::MIN), |(a, b), &x| (a.min(x), b.max(x)));
    assert!(lo >= -1e-3 && hi <= 1.0 + 1e-3 && lo <= 1e-3 && hi >= 1.0 - 1e-3, "{lo} {hi}");
}

#[test]
fn rotated_explicit_is_trimmed() {
    // xy に 0.5 rad 回した z = x^2, y ≥ 0。回した後の座標 q で、元の y = −sin(0.5) q_x + cos(0.5) q_y ≥ 0
    let mut m = run_full("z = x^2, y ≥ 0", R, N, 3, 0.0, &[(0, 0.5)], 0);
    let p = m.positions();
    assert!(!p.is_empty());
    for q in p.chunks_exact(3) {
        let (qx, qy) = (q[0] as f64, q[1] as f64);
        let y = -(0.5f64).sin() * qx + (0.5f64).cos() * qy;
        assert!(y >= -1e-3, "{q:?}");
    }
    assert!(m.take_shape().is_none());
}

#[test]
#[ignore]
fn trim_time() {
    for src in ["x^2 + y^2 + z^2 = 4", "x^2 + y^2 + z^2 = 4, z ≥ 0"] {
        let mut best = f64::INFINITY;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            run_with(src, R, N, 3, 0.0, 0.0);
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
        }
        println!("{src}\t{best:.1} ms");
    }
}

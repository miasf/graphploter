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
    .unwrap_or_else(|_| panic!("{src} が計算できない"))
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

// ---- 不等式の領域（計画 S31） ----

/// 点 p から三角形 abc までの距離（Ericson の最近点）
fn dist_tri(p: [f64; 3], a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let sub = |u: [f64; 3], v: [f64; 3]| [u[0] - v[0], u[1] - v[1], u[2] - v[2]];
    let dot = |u: [f64; 3], v: [f64; 3]| u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
    let at = |s: f64, t: f64| [0, 1, 2].map(|k| a[k] + s * (b[k] - a[k]) + t * (c[k] - a[k]));
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    let q = if d1 <= 0.0 && d2 <= 0.0 {
        a
    } else {
        let bp = sub(p, b);
        let (d3, d4) = (dot(ab, bp), dot(ac, bp));
        if d3 >= 0.0 && d4 <= d3 {
            b
        } else {
            let vc = d1 * d4 - d3 * d2;
            if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
                at(d1 / (d1 - d3), 0.0)
            } else {
                let cp = sub(p, c);
                let (d5, d6) = (dot(ab, cp), dot(ac, cp));
                if d6 >= 0.0 && d5 <= d6 {
                    c
                } else {
                    let vb = d5 * d2 - d1 * d6;
                    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
                        at(0.0, d2 / (d2 - d6))
                    } else {
                        let va = d3 * d6 - d5 * d4;
                        if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
                            let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
                            [0, 1, 2].map(|k| b[k] + w * (c[k] - b[k]))
                        } else {
                            let den = 1.0 / (va + vb + vc);
                            at(vb * den, vc * den)
                        }
                    }
                }
            }
        }
    };
    let d = sub(p, q);
    dot(d, d).sqrt()
}

/// 点の列から、メッシュ（三角形）までの距離（H より遠ければ H）
fn dist_to_mesh(pts: &[[f64; 3]], p: &[f32], idx: &[u32]) -> Vec<f64> {
    let cell = |v: f64| (v / H).floor() as i64;
    let mut grid: std::collections::HashMap<[i64; 3], Vec<usize>> = Default::default();
    for (k, t) in idx.chunks_exact(3).enumerate() {
        let vs = [vert(p, t[0]), vert(p, t[1]), vert(p, t[2])];
        let lo = [0, 1, 2].map(|c| cell(vs.iter().map(|v| v[c]).fold(f64::INFINITY, f64::min)));
        let hi = [0, 1, 2].map(|c| cell(vs.iter().map(|v| v[c]).fold(f64::NEG_INFINITY, f64::max)));
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    grid.entry([x, y, z]).or_default().push(k);
                }
            }
        }
    }
    pts.iter()
        .map(|&q| {
            let c = q.map(cell);
            let mut best = H;
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        for &k in grid.get(&[c[0] + dx, c[1] + dy, c[2] + dz]).into_iter().flatten() {
                            let t = &idx[3 * k..3 * k + 3];
                            best = best.min(dist_tri(q, vert(p, t[0]), vert(p, t[1]), vert(p, t[2])));
                        }
                    }
                }
            }
            best
        })
        .collect()
}

/// 符号付きの体積（外向きの三角形なら正）
fn volume(p: &[f32], idx: &[u32]) -> f64 {
    idx.chunks_exact(3)
        .map(|t| {
            let (a, b, c) = (vert(p, t[0]), vert(p, t[1]), vert(p, t[2]));
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum()
}

/// 面を表示範囲で切り取ったもの（陰関数の面は、表示範囲の少し外まで作る）と、表示範囲の端のふたを合わせた (点, 三角形)
fn closed_region(src: &str) -> (Vec<f32>, Vec<u32>) {
    use graph_core::{mesh::Mesh, system};
    let mut m = run_with(src, R, N, 3, 0.0, 0.0);
    let mut faces = Mesh { positions: m.positions(), indices: m.indices(), ..Default::default() };
    // ふたは面から対角線の 1e-4 倍だけ内側に作るので、同じ所で切る
    let lim = R - 2.0 * R * 3f64.sqrt() * 1e-4;
    for a in 0..3 {
        for s in [-1.0, 1.0] {
            system::trim_mesh(&mut faces, &mut |pts: &[[f64; 3]]| pts.iter().map(|q| s * q[a] - lim).collect(), 30);
        }
    }
    let caps = m.take_field().unwrap().caps(-1, 0.0, false, 0, N).unwrap();
    let base = faces.positions.len() as u32 / 3;
    let mut p = faces.positions;
    p.extend(caps.positions());
    let mut idx = faces.indices;
    idx.extend(caps.indices().iter().map(|k| k + base));
    (p, idx)
}

/// 正解の角の線の点から、メッシュまでの距離の (最大, 平均)（h の倍数）
fn corner_dist(src: &str, line: &[[f64; 3]]) -> (f64, f64) {
    let m = run_with(src, R, N, 3, 0.0, 0.0);
    let d = dist_to_mesh(line, &m.positions(), &m.indices());
    let max = d.iter().cloned().fold(0.0, f64::max);
    let out = (max / H, d.iter().sum::<f64>() / d.len() as f64 / H);
    println!("角 {src}: 最大 {:.3}h 平均 {:.3}h", out.0, out.1);
    out
}

#[test]
fn cube_has_sharp_corners() {
    let src = "abs(x) ≤ 2, abs(y) ≤ 2, abs(z) ≤ 2";
    let m = run_with(src, R, N, 3, 0.0, 0.0);
    assert_eq!(m.kind(), "不等式の領域（3 本）");
    let (p, idx) = (m.positions(), m.indices());
    for c in 0..8 {
        let corner = [0, 1, 2].map(|k| if c >> k & 1 == 1 { 2.0 } else { -2.0 });
        let near = p.chunks_exact(3).any(|q| (0..3).all(|k| (q[k] as f64 - corner[k]).abs() <= 0.01));
        assert!(near, "{corner:?}");
    }
    let mut edges = Vec::new();
    for a in 0..3 {
        for s in [[-2.0, -2.0], [-2.0, 2.0], [2.0, -2.0], [2.0, 2.0]] {
            edges.extend(truth(
                move |t| {
                    let mut q = [0.0; 3];
                    q[a] = t;
                    q[(a + 1) % 3] = s[0];
                    q[(a + 2) % 3] = s[1];
                    q
                },
                -2.0,
                2.0,
            ));
        }
    }
    let (max, _) = corner_dist(src, &edges);
    assert!(max <= 0.1, "{max}");
    let v = volume(&p, &idx);
    assert!((v / 64.0 - 1.0).abs() <= 0.01, "{v}");
}

#[test]
fn wedge_region() {
    let src = "x ≤ y ≤ z";
    let line = truth(|t| [t, t, t], -R, R);
    let (max, _) = corner_dist(src, &line);
    assert!(max <= 0.1, "{max}");
    let (p, idx) = closed_region(src);
    let v = volume(&p, &idx);
    assert!((v / (1000.0 / 6.0) - 1.0).abs() <= 0.01, "{v}");
}

#[test]
fn steinmetz_region() {
    let src = "x^2 + y^2 ≤ 9, x^2 + z^2 ≤ 9";
    let mut line = truth(|t| [3.0 * t.cos(), 3.0 * t.sin(), 3.0 * t.sin()], 0.0, 2.0 * std::f64::consts::PI);
    line.extend(truth(|t| [3.0 * t.cos(), 3.0 * t.sin(), -3.0 * t.sin()], 0.0, 2.0 * std::f64::consts::PI));
    let (max, _) = corner_dist(src, &line);
    assert!(max <= 0.1, "{max}");
    let m = run_with(src, R, N, 3, 0.0, 0.0);
    let v = volume(&m.positions(), &m.indices());
    assert!((v / 144.0 - 1.0).abs() <= 0.01, "{v}");
}

#[test]
fn thin_region_keeps_its_edge() {
    use std::f64::consts::PI;
    let mut line = Vec::new();
    for k in -2..=2 {
        let x = k as f64 * PI;
        if x.abs() < R {
            line.extend(truth(move |t| [x, t, 0.0], -R, R));
        }
        let y = PI / 2.0 + k as f64 * PI;
        if y.abs() < R {
            line.extend(truth(move |t| [t, y, 0.0], -R, R));
        }
    }
    let (max, mean) = corner_dist("0 ≤ z ≤ sin(x) cos(y)", &line);
    assert!(mean <= 0.05 && max <= 0.6, "{max} {mean}");
}

#[test]
fn single_inequality() {
    let a = run_with("x^2 + y^2 + z^2 < 4", R, N, 3, 0.0, 0.0);
    let b = run_with("x^2 + y^2 + z^2 = 4", R, N, 3, 0.0, 0.0);
    assert_eq!(a.kind(), "不等式の領域");
    assert_eq!(a.positions(), b.positions());
    assert_eq!(a.indices(), b.indices());
    // x ≤ y: 端のふたは x = −5・y = 5 で全体、z = ±5 で半分、x = 5・y = −5 には無い
    let mut m = run_with("x ≤ y", R, N, 3, 0.0, 0.0);
    let caps = m.take_field().unwrap().caps(-1, 0.0, false, 0, N).unwrap();
    let (p, idx) = (caps.positions(), caps.indices());
    let face_area = |axis: usize, at: f64| {
        let tris: Vec<u32> = idx
            .chunks_exact(3)
            .filter(|t| t.iter().all(|&k| (vert(&p, k)[axis] - at).abs() < 0.01))
            .flatten()
            .copied()
            .collect();
        area(&p, &tris)
    };
    for (axis, at, want) in [(0, -R, 100.0), (1, R, 100.0), (2, R, 50.0), (2, -R, 50.0), (0, R, 0.0), (1, -R, 0.0)] {
        let a = face_area(axis, at);
        assert!((a - want).abs() <= 1.0, "{axis} {at}: {a}");
    }
}

#[test]
fn region_cap_has_corners() {
    let mut m = run_with("abs(x) ≤ 2, abs(y) ≤ 2, abs(z) ≤ 2", R, N, 3, 0.0, 0.0);
    let cap = m.take_field().unwrap().cap(2, 0.3, true, N).unwrap();
    let (p, idx) = (cap.positions(), cap.indices());
    let a = area(&p, &idx);
    assert!((a / 16.0 - 1.0).abs() <= 0.005, "{a}");
    for (sx, sy) in [(-2.0, -2.0), (-2.0, 2.0), (2.0, -2.0), (2.0, 2.0)] {
        let near = p.chunks_exact(3).any(|q| (q[0] as f64 - sx).abs() <= 0.02 && (q[1] as f64 - sy).abs() <= 0.02);
        assert!(near, "{sx} {sy}");
    }
}

#[test]
#[ignore]
fn region_time() {
    let best = |src: &str| {
        (0..3)
            .map(|_| {
                let t = std::time::Instant::now();
                run_with(src, R, N, 3, 0.0, 0.0);
                t.elapsed().as_secs_f64() * 1e3
            })
            .fold(f64::INFINITY, f64::min)
    };
    for (src, faces) in [
        ("abs(x) ≤ 2, abs(y) ≤ 2, abs(z) ≤ 2", vec!["abs(x) - 2 = 0", "abs(y) - 2 = 0", "abs(z) - 2 = 0"]),
        ("x ≤ y ≤ z", vec!["x - y = 0", "y - z = 0"]),
        ("x^2 + y^2 ≤ 9, x^2 + z^2 ≤ 9", vec!["x^2 + y^2 - 9 = 0", "x^2 + z^2 - 9 = 0"]),
        ("0 ≤ z ≤ sin(x) cos(y)", vec!["0 - z = 0", "z - sin(x) cos(y) = 0"]),
    ] {
        let all = best(src);
        let sum: f64 = faces.iter().map(|f| best(f)).sum();
        println!("{src}\t{all:.1} ms\t面ごとの合計 {sum:.1} ms\t{:.2} 倍", all / sum);
    }
}

// ---- 点と 2D 表示（計画 S32） ----

/// 2D 表示（`slice` なら切り口、そうでなければ投影）。表示範囲 ±5
fn run_flat(src: &str, slice: bool) -> MeshResult {
    let rd = equation::rot_dims(3);
    plot(
        src, &[-R, R, -R, R, -R, R], 64, N, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16,
        &vec![0.0; rd * (rd - 1) / 2], &vec![0.0; rd], 0.0, 0, false, true, 1, slice, 0,
    )
    .unwrap_or_else(|_| panic!("{src} が計算できない"))
}

/// 点のメッシュ（同じ点を 2 つと NaN）の点
fn dots(m: &MeshResult) -> Vec<[f64; 3]> {
    m.positions().chunks_exact(9).map(|s| [s[0] as f64, s[1] as f64, s[2] as f64]).collect()
}

fn close(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() <= tol)
}

#[test]
fn points_of_three_equations() {
    let m = run_with("x = y = z = 1", R, N, 3, 0.0, 0.0);
    assert_eq!(m.kind(), "連立方程式の点（1 個）");
    let p = dots(&m);
    assert!(p.len() == 1 && close(p[0], [1.0; 3], 1e-6), "{p:?}");
    let s2 = 2f64.sqrt();
    let p = dots(&run_with("x^2 + y^2 + z^2 = 4, x = y, z = 0", R, N, 3, 0.0, 0.0));
    assert_eq!(p.len(), 2, "{p:?}");
    for want in [[s2, s2, 0.0], [-s2, -s2, 0.0]] {
        assert!(p.iter().any(|q| close(*q, want, 1e-6)), "{p:?}");
    }
    // 式の順番を入れ替えても、同じ点
    let orders = ["sin(x) = cos(y), cos(y) = tan(z), tan(z) = 0.5", "tan(z) = 0.5, sin(x) = cos(y), cos(y) = tan(z)", "cos(y) = tan(z), tan(z) = 0.5, sin(x) = cos(y)"];
    let sets: Vec<Vec<[f64; 3]>> = orders.iter().map(|s| dots(&run_with(s, R, N, 3, 0.0, 0.0))).collect();
    for (src, p) in orders.iter().zip(&sets) {
        assert_eq!(p.len(), 18, "{src}");
        for q in p {
            assert!(sets[0].iter().any(|r| close(*q, *r, 1e-6)), "{src}: {q:?}");
        }
    }
}

#[test]
fn points_respect_inequalities() {
    let p = dots(&run_with("x^2 + y^2 + z^2 = 4, x = y, z = 0, x ≥ 0", R, N, 3, 0.0, 0.0));
    assert_eq!(p.len(), 1, "{p:?}");
}

#[test]
fn flat_intersection_points() {
    let m = run_flat("y = x, y = x^2", true);
    assert_eq!(m.kind(), "連立方程式の点（2 個）");
    let p = dots(&m);
    for want in [[0.0, 0.0, 0.0], [1.0, 1.0, 0.0]] {
        assert!(p.iter().any(|q| close(*q, want, 1e-6)), "{p:?}");
    }
}

#[test]
fn flat_region_area() {
    let m = run_flat("y ≤ x^2", true);
    assert_eq!(m.kind(), "不等式の領域（2D）");
    let a = area(&m.positions(), &m.indices());
    let want = 100.0 - 20.0 * 5f64.sqrt() / 3.0;
    assert!((a / want - 1.0).abs() <= 0.01, "{a}");
    let m = run_flat("x^2 + y^2 ≤ 9, y ≥ 0", true);
    let p = m.positions();
    let a = area(&p, &m.indices());
    assert!((a / (4.5 * std::f64::consts::PI) - 1.0).abs() <= 0.01, "{a}");
    for sx in [-3.0, 3.0] {
        assert!(p.chunks_exact(3).any(|q| (q[0] as f64 - sx).abs() <= 0.02 && (q[1] as f64).abs() <= 0.02), "{sx}");
    }
}

#[test]
fn flat_curve_is_clipped() {
    let m = run_flat("x^2 + y^2 = 4, y ≥ 0", true);
    let c = curve_metrics(&m.positions(), &[]);
    assert!((c.length / (2.0 * std::f64::consts::PI) - 1.0).abs() <= 0.005, "{c:?}");
}

#[test]
fn flat_projection_of_ball() {
    let m = run_flat("x^2 + y^2 + z^2 ≤ 4", false);
    let p = m.positions();
    assert!(!p.is_empty());
    let rmax = p.chunks_exact(3).map(|q| ((q[0] * q[0] + q[1] * q[1]) as f64).sqrt()).fold(0.0, f64::max);
    assert!((rmax - 2.0).abs() <= 0.02, "{rmax}");
    assert!(p.chunks_exact(3).all(|q| q[2] == 0.0));
}

/// 切った線に、長さの無い線分（描画側が点とみなす）が残らない
#[test]
fn clipped_lines_have_no_dots() {
    for (src, flat) in [("x^2 + y^2 = 4, y ≥ 0", true), ("y = x, z = 0, 0 ≤ x ≤ 1", false), ("x^2 + y^2 = 4, z = 0, y ≥ 0", false)] {
        let m = if flat { run_flat(src, true) } else { run_with(src, R, N, 3, 0.0, 0.0) };
        for s in m.positions().chunks_exact(9) {
            let d: f32 = (0..3).map(|c| (s[c] - s[3 + c]).abs()).sum();
            assert!(d > 1e-5, "{src}: {s:?}");
        }
    }
}

// ---- 媒介変数表示の切り取り（計画 S33） ----

/// 媒介変数の範囲（"t,u" と [t0, t1, u0, u1, …]）を付けて、3 次元・回転なし・表示範囲 ±5 で描く
fn run_param(src: &str, names: &str, ranges: &[f64]) -> MeshResult {
    let rd = equation::rot_dims(3);
    plot(
        src, &[-R, R, -R, R, -R, R], 64, N, names, ranges, "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16,
        &vec![0.0; rd * (rd - 1) / 2], &vec![0.0; rd], 0.0, 0, false, false, 1, false, 0,
    )
    .unwrap_or_else(|_| panic!("{src} が計算できない"))
}

#[test]
fn parametric_curve_is_clipped() {
    use std::f64::consts::PI;
    let r = [-2.0 * PI, 2.0 * PI];
    let mut whole = run_param("(cos t, sin t, t/4)", "t", &r);
    assert!(whole.take_shape().is_some());
    let mut cut = run_param("(cos t, sin t, t/4), z ≥ 0", "t", &r);
    assert_eq!(cut.kind(), "媒介変数表示の曲線（不等式で切り取り）");
    assert!(cut.take_shape().is_none());
    assert!(cut.positions().chunks_exact(3).filter(|q| q[2].is_finite()).all(|q| q[2] >= -1e-3));
    // 曲線の点は線分の列ではなく点の列（NaN で切れ目）なので、つながった点どうしの長さを足す
    let len = |m: &MeshResult| {
        let p = m.positions();
        p.chunks_exact(3)
            .zip(p.chunks_exact(3).skip(1))
            .filter(|(a, b)| a.iter().chain(b.iter()).all(|v| v.is_finite()))
            .map(|(a, b)| (0..3).map(|k| ((a[k] - b[k]) as f64).powi(2)).sum::<f64>().sqrt())
            .sum::<f64>()
    };
    let (lw, lc) = (len(&whole), len(&cut));
    assert!((lc / (lw / 2.0) - 1.0).abs() <= 0.01, "{lw} {lc}");
}

#[test]
fn parametric_surface_is_trimmed() {
    use std::f64::consts::PI;
    let m = run_param("((3 + cos v) cos u, (3 + cos v) sin u, sin v), x ≥ 0", "u,v", &[0.0, 2.0 * PI, 0.0, 2.0 * PI]);
    assert_eq!(m.kind(), "媒介変数表示の曲面（不等式で切り取り）");
    let p = m.positions();
    assert!(p.chunks_exact(3).all(|q| q[0] >= -1e-3));
    let a = area(&p, &m.indices());
    assert!((a / (6.0 * PI * PI) - 1.0).abs() <= 0.01, "{a}");
    assert_eq!(m.grid_coords().len(), p.len());
}

// ---- 2D の領域の境目の線（計画 S34） ----

#[test]
fn flat_region_outline() {
    let mut m = run_flat("y < x^2, y ≥ -3", true);
    let (solid, dashed) = (m.take_outline(), m.take_outline_dashed());
    let pts = |v: &[f32]| -> Vec<[f32; 3]> { v.chunks_exact(3).filter(|q| q[0].is_finite()).map(|q| [q[0], q[1], q[2]]).collect() };
    let (s, d) = (pts(&solid), pts(&dashed));
    assert!(!s.is_empty() && !d.is_empty());
    assert!(d.iter().all(|q| (q[1] - q[0] * q[0]).abs() <= 1e-2 && q[1] >= -3.0 - 1e-3), "点線");
    assert!(s.iter().all(|q| (q[1] + 3.0).abs() <= 1e-2), "実線");
    // y = −3 の線は全体が残る（x が −5 〜 5）
    let (lo, hi) = s.iter().fold((f32::MAX, f32::MIN), |(a, b), q| (a.min(q[0]), b.max(q[0])));
    assert!(lo <= -4.9 && hi >= 4.9, "{lo} {hi}");
    // 不等式でない式には線が無い
    let mut m = run_flat("x^2 + y^2 = 4", true);
    assert!(m.take_outline().is_empty() && m.take_outline_dashed().is_empty());
}

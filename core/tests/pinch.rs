//! ピンチ点（交わる角度が 0 になる所）と、面にならない特異点の線（計画 P1-6。テスト表 S-05）

use graph_core::plot;

fn positions(src: &str, r: f64, ires: u32) -> Vec<[f64; 3]> {
    let m = plot(src, &[-r, r, -r, r, -r, r], 60, ires, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0)
        .unwrap_or_else(|_| panic!("{src}"));
    m.positions().chunks_exact(3).map(|v| [v[0] as f64, v[1] as f64, v[2] as f64]).collect()
}

fn nearest(p: &[[f64; 3]], q: [f64; 3]) -> f64 {
    p.iter().map(|v| ((v[0] - q[0]).powi(2) + (v[1] - q[1]).powi(2) + (v[2] - q[2]).powi(2)).sqrt()).fold(f64::INFINITY, f64::min)
}

/// 線 a → b の上の点から、いちばん近い頂点までの距離（マス）の最大と、最大の所
fn along(p: &[[f64; 3]], a: [f64; 3], b: [f64; 3], h: f64) -> (f64, [f64; 3]) {
    let mut worst = (0.0, a);
    for k in 0..=100 {
        let t = k as f64 / 100.0;
        let q = [0, 1, 2].map(|c| a[c] + t * (b[c] - a[c]));
        let d = nearest(p, q) / h;
        if d > worst.0 {
            worst = (d, q);
        }
    }
    worst
}

#[test]
#[ignore]
fn report() {
    let ires = 80;
    let h = 2.0 / ires as f64;
    let roman = positions("x^2 y^2 + y^2 z^2 + z^2 x^2 = x y z", 1.0, ires);
    println!("ローマ曲面: 頂点 {}", roman.len());
    println!("  x 軸の二重線 [0, 0.5]: 最大 {:?}", along(&roman, [0.0; 3], [0.5, 0.0, 0.0], h));
    println!("  ピンチ点 (0.5, 0, 0): {:.2} マス", nearest(&roman, [0.5, 0.0, 0.0]) / h);
    println!("  ひげ [0.5, 1]: 最大 {:?}", along(&roman, [0.5, 0.0, 0.0], [1.0, 0.0, 0.0], h));
    let w = positions("x^2 = y^2 z", 1.0, ires);
    println!("Whitney の傘: 頂点 {}", w.len());
    println!("  z 軸の二重線 [0, 1]: 最大 {:?}", along(&w, [0.0; 3], [0.0, 0.0, 1.0], h));
    println!("  ピンチ点 (0, 0, 0): {:.2} マス", nearest(&w, [0.0; 3]) / h);
    println!("  柄 [−1, 0]: 最大 {:?}", along(&w, [0.0, 0.0, -1.0], [0.0; 3], h));
}

/// ローマ曲面 x²y² + y²z² + z²x² = xyz の媒介変数表示（θ, φ ∈ [0, 2π)）
fn roman_param(t: f64, f: f64) -> [f64; 3] {
    let (c, sn) = (f.cos(), f.sin());
    [t.cos() * c * sn, t.sin() * c * sn, t.cos() * t.sin() * c * c]
}

/// 面の点の集まり（2 方向に n 等分）
fn samples(n: usize) -> Vec<[f64; 3]> {
    let tau = std::f64::consts::TAU;
    (0..n).flat_map(|i| (0..n).map(move |j| roman_param(tau * i as f64 / n as f64, tau * j as f64 / n as f64))).collect()
}

/// 点の集まりの、格子で引く表（近い点を速く探す）
struct Lookup {
    cell: f64,
    map: std::collections::HashMap<[i64; 3], Vec<[f64; 3]>>,
}
impl Lookup {
    fn new(pts: &[[f64; 3]], cell: f64) -> Self {
        let mut map: std::collections::HashMap<[i64; 3], Vec<[f64; 3]>> = Default::default();
        for p in pts {
            map.entry(p.map(|v| (v / cell).floor() as i64)).or_default().push(*p);
        }
        Lookup { cell, map }
    }
    /// いちばん近い点までの距離（`cell` の 3 倍まで。それより遠ければ 3 cell）
    fn dist(&self, q: [f64; 3]) -> f64 {
        let k = q.map(|v| (v / self.cell).floor() as i64);
        let mut best = 3.0 * self.cell;
        for dx in -3..=3 {
            for dy in -3..=3 {
                for dz in -3..=3 {
                    for p in self.map.get(&[k[0] + dx, k[1] + dy, k[2] + dz]).into_iter().flatten() {
                        best = best.min(((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt());
                    }
                }
            }
        }
        best
    }
}

/// 点 p から三角形までの距離（Ericson の最近点）
fn tri_dist(p: [f64; 3], t: &[[f64; 3]; 3]) -> f64 {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let (a, b, c) = (t[0], t[1], t[2]);
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let lerp = |o: [f64; 3], d: [f64; 3], s: f64| [o[0] + s * d[0], o[1] + s * d[1], o[2] + s * d[2]];
    let dist = |q: [f64; 3]| dot(sub(p, q), sub(p, q)).sqrt();
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return dist(a);
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return dist(b);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return dist(lerp(a, ab, d1 / (d1 - d3)));
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return dist(c);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return dist(lerp(a, ac, d2 / (d2 - d6)));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return dist(lerp(b, sub(c, b), (d4 - d3) / ((d4 - d3) + (d5 - d6))));
    }
    let den = 1.0 / (va + vb + vc);
    let (v, w) = (vb * den, vc * den);
    dist([a[0] + ab[0] * v + ac[0] * w, a[1] + ab[1] * v + ac[1] * w, a[2] + ab[2] * v + ac[2] * w])
}

/// ローマ曲面の、頂点 → 面（とげ）と、面 → 頂点（穴）の距離（マス）。(とげの最大, とげ 0.5 マス超の頂点・重心の数, 穴の最大, 穴 0.5 マス超の点の数)
fn roman_errors(ires: u32) -> (f64, usize, f64, usize) {
    let r = 0.6;
    let h = 2.0 * r / ires as f64;
    let m0 = plot("x^2 y^2 + y^2 z^2 + z^2 x^2 = x y z", &[-r, r, -r, r, -r, r], 60, ires, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0).ok().unwrap();
    let pos: Vec<[f64; 3]> = m0.positions().chunks_exact(3).map(|v| [v[0] as f64, v[1] as f64, v[2] as f64]).collect();
    // 頂点と、三角形の重心（2 枚の面をまたぐ三角形は、頂点が面の上でも重心が面から離れる）
    let mut mesh = pos.clone();
    for t in m0.indices().chunks_exact(3) {
        let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
        mesh.push([0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0));
    }
    let surf = samples(1200);
    let s = Lookup::new(&surf, h);
    let m = Lookup::new(&mesh, h);
    let _ = &m;
    let spikes: Vec<f64> = mesh.iter().map(|&q| s.dist(q) / h).collect();
    // 面の点から、いちばん近い三角形までの距離（三角形は重心の格子で引く）
    let tris: Vec<[[f64; 3]; 3]> = m0.indices().chunks_exact(3).map(|t| [pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]]).collect();
    let mut grid: std::collections::HashMap<[i64; 3], Vec<usize>> = Default::default();
    for (i, t) in tris.iter().enumerate() {
        let c = [0, 1, 2].map(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.0);
        grid.entry(c.map(|v| (v / h).floor() as i64)).or_default().push(i);
    }
    let holes: Vec<f64> = surf
        .iter()
        .step_by(7)
        .map(|&q| {
            let k = q.map(|v| (v / h).floor() as i64);
            let mut best = 4.0 * h;
            for dx in -4..=4 {
                for dy in -4..=4 {
                    for dz in -4..=4 {
                        for &i in grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]).into_iter().flatten() {
                            best = best.min(tri_dist(q, &tris[i]));
                        }
                    }
                }
            }
            best / h
        })
        .collect();
    if std::env::var_os("PINCH_WHERE").is_some() {
        // 穴の点が、どの軸からどれだけ離れているか、軸に沿ってどこか
        let mut bins = [0usize; 12];
        let mut far = 0;
        for (q, d) in surf.iter().step_by(7).zip(&holes) {
            if *d <= 0.5 {
                continue;
            }
            let (ax, dist_ax) = (0..3).map(|a| (a, ((q[(a + 1) % 3]).powi(2) + (q[(a + 2) % 3]).powi(2)).sqrt())).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
            if dist_ax > 4.0 * h {
                far += 1;
                continue;
            }
            bins[((q[ax].abs() / 0.05) as usize).min(11)] += 1;
        }
        println!("  格子 {ires}: 軸から 4 マス以内の穴を、軸に沿って 0.05 ごと: {bins:?}、軸から遠い穴 {far}");
    }
    (
        spikes.iter().cloned().fold(0.0, f64::max),
        spikes.iter().filter(|&&d| d > 0.5).count(),
        holes.iter().cloned().fold(0.0, f64::max),
        holes.iter().filter(|&&d| d > 0.5).count(),
    )
}

#[test]
#[ignore]
fn roman_report() {
    for ires in [40, 80] {
        println!("ローマ曲面 格子 {ires}: {:?}", roman_errors(ires));
    }
}

/// ローマ曲面のピンチ点（各軸の ±1/2。交わる角度が 0 になる所）の近くで、面がくしの歯のように欠けない。
/// 前（捨てる半径 1.5 マスで、1 枚の三角形で特異点へ結んでいた）は、格子 80 で 0.5 マスより深い穴の点が 2,899、最大 1.34 マス
#[test]
fn roman_pinch_points() {
    let (spike, _, hole, holes) = roman_errors(80);
    assert!(spike <= 0.7, "とげ {spike} マス");
    assert!(hole <= 0.9, "穴 {hole} マス");
    assert!(holes <= 2000, "0.5 マスより深い穴の点 {holes}");
    let (_, _, hole, _) = roman_errors(40);
    assert!(hole <= 0.95, "格子 40: 穴 {hole} マス");
}

/// 線の点（outline の実線）のうち、線 a → b から 0.3 マス以内の点で、a → b の何割を覆うか（0.05 ごとの区間のうち、点のある区間の割合）
fn line_coverage(src: &str, a: [f64; 3], b: [f64; 3]) -> f64 {
    let ires = 80;
    let r = 1.0;
    let h = 2.0 * r / ires as f64;
    let mut m = plot(src, &[-r, r, -r, r, -r, r], 60, ires, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0).ok().unwrap();
    let pts: Vec<[f64; 3]> = m.take_outline().chunks_exact(3).filter(|p| p[0].is_finite()).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect();
    let d = [0, 1, 2].map(|c| b[c] - a[c]);
    let len2 = d.iter().map(|v| v * v).sum::<f64>();
    let mut bins = [false; 20];
    for p in &pts {
        let t = ([0, 1, 2].map(|c| (p[c] - a[c]) * d[c]).iter().sum::<f64>() / len2).clamp(0.0, 1.0);
        let q = [0, 1, 2].map(|c| a[c] + t * d[c]);
        let dist = ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt();
        if dist <= 0.3 * h {
            bins[((t * 20.0) as usize).min(19)] = true;
        }
    }
    bins.iter().filter(|&&b| b).count() as f64 / 20.0
}

#[test]
#[ignore]
fn whisker_report() {
    println!("ローマ曲面のひげ x ∈ [0.5, 1]: {}", line_coverage("x^2 y^2 + y^2 z^2 + z^2 x^2 = x y z", [0.52, 0.0, 0.0], [1.0, 0.0, 0.0]));
    println!("ローマ曲面のひげ z ∈ [−1, −0.5]: {}", line_coverage("x^2 y^2 + y^2 z^2 + z^2 x^2 = x y z", [0.0, 0.0, -1.0], [0.0, 0.0, -0.52]));
    println!("Whitney の傘の柄 z ∈ [−1, 0]: {}", line_coverage("x^2 = y^2 z", [0.0, 0.0, -1.0], [0.0, 0.0, -0.02]));
    println!("二重線（線を引かない）z ∈ [0.1, 1]: {}", line_coverage("x^2 = y^2 z", [0.0, 0.0, 0.1], [0.0, 0.0, 1.0]));
}

/// 面にならない特異点の線（ローマ曲面のひげ、ホイットニーの傘の柄）は線として描き、二重線（面が描く）には線を足さない
#[test]
fn whiskers_and_handles_are_lines() {
    let roman = "x^2 y^2 + y^2 z^2 + z^2 x^2 = x y z";
    assert!(line_coverage(roman, [0.52, 0.0, 0.0], [1.0, 0.0, 0.0]) >= 0.95);
    assert!(line_coverage(roman, [0.0, 0.0, -1.0], [0.0, 0.0, -0.52]) >= 0.95);
    assert!(line_coverage("x^2 = y^2 z", [0.0, 0.0, -1.0], [0.0, 0.0, -0.02]) >= 0.95);
    assert_eq!(line_coverage("x^2 = y^2 z", [0.0, 0.0, 0.1], [0.0, 0.0, 1.0]), 0.0);
    // 尖った点（円錐の先）・なめらかな面には線を足さない
    assert_eq!(line_coverage("x^2 + y^2 = z^2", [0.0, 0.0, -1.0], [0.0, 0.0, 1.0]), 0.0);
}

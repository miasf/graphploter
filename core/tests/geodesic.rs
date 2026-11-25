//! 曲面の上の測地線（計画 N3。`docs/plan-n3-geodesics.md`）: plot の前に geodesic_spec で欄を渡す

use graph_core::{geodesic_spec, plot};

struct Out {
    kind: String,
    /// 測地線ごとの点（面の両側に浮かせた 2 本の、平均 = 面の上の点）
    paths: Vec<Vec<[f64; 3]>>,
    vars: String,
    used: Vec<String>,
}

fn draw_with(src: &str, ranges: (&str, &[f64]), params: (&str, &[f64]), spec: &str) -> Out {
    graph_core::definitions("");
    geodesic_spec(spec);
    let d = 5.0;
    let mut m = plot(src, &[-d, d, -d, d, -d, d], 64, 48, ranges.0, ranges.1, params.0, params.1, "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, 0)
        .unwrap_or_else(|_| panic!("{src} が描けない"));
    let g = m.take_geodesic();
    let mut lines: Vec<Vec<[f64; 3]>> = vec![Vec::new()];
    for p in g.chunks_exact(3) {
        if p[0].is_nan() {
            lines.push(Vec::new());
        } else {
            lines.last_mut().unwrap().push([p[0] as f64, p[1] as f64, p[2] as f64]);
        }
    }
    lines.retain(|l| !l.is_empty());
    assert!(lines.len() % 2 == 0, "線は 2 本ずつ");
    let paths = lines
        .chunks_exact(2)
        .map(|pair| {
            assert_eq!(pair[0].len(), pair[1].len());
            pair[0].iter().zip(&pair[1]).map(|(a, b)| [0, 1, 2].map(|k| (a[k] + b[k]) / 2.0)).collect()
        })
        .collect();
    Out { kind: m.kind(), paths, vars: m.geodesic_vars(), used: m.used_params().split(',').map(String::from).collect() }
}

fn draw(src: &str, spec: &str) -> Out {
    draw_with(src, ("", &[]), ("", &[]), spec)
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// 大円からのずれ（原点を通る平面からの距離と、半径 2 からの差の大きい方）
fn off_great_circle(p: &[[f64; 3]]) -> f64 {
    let c = cross(p[0], p[p.len() / 7]);
    let l = dot(c, c).sqrt();
    let n = c.map(|v| v / l);
    p.iter().map(|q| dot(*q, n).abs().max((dot(*q, *q).sqrt() - 2.0).abs())).fold(0.0, f64::max)
}

/// 球の測地線は大円（媒介変数・陰関数の両方）。f32 で返すので、誤差は 10⁻⁵ 程度まで
#[test]
fn sphere_geodesics_are_great_circles() {
    let pi = std::f64::consts::PI;
    let o = draw_with("(2sin(u)cos(v),2sin(u)sin(v),2cos(u))", ("u,v", &[0.0, pi, 0.0, 2.0 * pi]), ("", &[]), "pi/2\n0.3\npi/4\n4pi");
    assert_eq!(o.vars, "u,v");
    assert_eq!(o.paths.len(), 1);
    assert!(off_great_circle(&o.paths[0]) < 1e-4, "{}", off_great_circle(&o.paths[0]));
    // 1 周（4π）で始点に戻る
    let (a, b) = (o.paths[0][0], *o.paths[0].last().unwrap());
    assert!(dot([a[0] - b[0], a[1] - b[1], a[2] - b[2]], [a[0] - b[0], a[1] - b[1], a[2] - b[2]]).sqrt() < 1e-4);

    let o = draw("x^2+y^2+z^2=4", "2\n0.5\n0.3\n0.7\n4pi");
    assert_eq!(o.vars, "x,y,z");
    assert_eq!(o.paths.len(), 1);
    assert!(off_great_circle(&o.paths[0]) < 1e-4, "{}", off_great_circle(&o.paths[0]));
    // 始点の欄が空なら、表示範囲の中心に一番近い頂点から
    let o = draw("x^2+y^2+z^2=4", "\n\n\n\n");
    assert_eq!(o.paths.len(), 1);
    assert!(off_great_circle(&o.paths[0]) < 1e-4);
}

/// 陽関数の平面は直線で、表示範囲の端で止まる
#[test]
fn plane_geodesic_is_straight() {
    let o = draw("z=2x+3y", "0\n0\n1\n100");
    assert_eq!(o.vars, "x,y");
    let p = &o.paths[0];
    let (a, b) = (p[0], *p.last().unwrap());
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let l = dot(d, d).sqrt();
    let worst = p.iter().map(|q| {
        let c = cross([q[0] - a[0], q[1] - a[1], q[2] - a[2]], d);
        dot(c, c).sqrt() / l
    }).fold(0.0, f64::max);
    assert!(worst < 1e-4, "{worst}");
    assert!((b[0].abs() - 5.0).abs() < 1e-4 || (b[1].abs() - 5.0).abs() < 1e-4, "{b:?}");
}

/// トーラスの外側の赤道を出た測地線は、媒介変数の範囲の端（v = 2π）で切れずに続く。赤道そのものも測地線
#[test]
fn torus_geodesic_wraps_around() {
    let pi = std::f64::consts::PI;
    let src = "((3+cos(v))cos(u),(3+cos(v))sin(u),sin(v))";
    let r = ("u,v", &[0.0, 2.0 * pi, 0.0, 2.0 * pi][..]);
    // 外側の赤道（v = 0）を u の向きへ: 半径 4 の円を 1 周以上
    let o = draw_with(src, r, ("", &[]), "0\n0\n0\n30");
    let p = &o.paths[0];
    assert!(p.iter().all(|q| q[2].abs() < 1e-4 && (q[0].hypot(q[1]) - 4.0).abs() < 1e-4));
    let arc: f64 = p.windows(2).map(|w| dot([w[1][0] - w[0][0], w[1][1] - w[0][1], w[1][2] - w[0][2]], [w[1][0] - w[0][0], w[1][1] - w[0][1], w[1][2] - w[0][2]]).sqrt()).sum();
    assert!(arc > 29.0, "{arc}");
    // 斜めに出た測地線も、v の範囲の端をまたいで続く（長さ 30 まで）
    let o = draw_with(src, r, ("", &[]), "1\n0.2\n1\n30");
    let p = &o.paths[0];
    let arc: f64 = p.windows(2).map(|w| dot([w[1][0] - w[0][0], w[1][1] - w[0][1], w[1][2] - w[0][2]], [w[1][0] - w[0][0], w[1][1] - w[0][1], w[1][2] - w[0][2]]).sqrt()).sum();
    assert!(arc > 29.0, "{arc}");
    assert!(!o.kind.contains("測地線"), "{}", o.kind);
}

/// 欄のリストで何本も描く。パラメータを使える（使ったパラメータを結果に返す）
#[test]
fn lists_and_parameters() {
    let o = draw("z=x^2-y^2", "0\n0\n[0, pi/4, pi/2]\n3");
    assert_eq!(o.paths.len(), 3);
    let o = draw_with("z=x^2-y^2", ("", &[]), ("a", &[0.5, 0.0]), "a\n0\na\n2");
    assert_eq!(o.paths.len(), 1);
    assert!(o.used.iter().any(|n| n == "a"), "{:?}", o.used);
    assert!((o.paths[0][0][0] - 0.5).abs() < 1e-4);
}

/// 球の極（座標の特異点）・面の無い所・欄の誤り・使えない式
#[test]
fn stops_and_errors() {
    let pi = std::f64::consts::PI;
    let o = draw_with("(2sin(u)cos(v),2sin(u)sin(v),2cos(u))", ("u,v", &[0.0, pi, 0.0, 2.0 * pi]), ("", &[]), "1\n0.5\npi\n20");
    assert!(o.kind.contains("特異点") || o.paths[0].last().unwrap()[2] > 1.99, "{}", o.kind);
    // 中心は勾配が 0 で、面の上へ寄せられない
    let o = draw("x^2+y^2+z^2=4", "0\n0\n0\n0\n5");
    assert!(o.kind.contains("始点の近くに面がありません"), "{}", o.kind);
    let o = draw("z=x^2", "0\n0\nq\n3");
    assert!(o.kind.contains("測地線の向き θ"), "{}", o.kind);
    assert!(o.paths.is_empty());
    // 曲線では使えない（欄の名前も返さない）
    let o = draw("(cos(t),sin(t),t)", "0\n0\n1");
    assert!(o.paths.is_empty() && o.vars.is_empty());
}

/// 欄が空の文字列なら描かない
#[test]
fn off_by_default() {
    let o = draw("x^2+y^2+z^2=4", "");
    assert!(o.paths.is_empty());
    assert_eq!(o.vars, "x,y,z");
}

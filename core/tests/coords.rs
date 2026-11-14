//! 座標系（計画 9 章の S36）: 頂点を座標系で移した先（`take_target`）と、作る範囲で切ること。画面と同じ入口（`graph_core::plot`）で確かめる

use graph_core::{coord_system, equation, plot, MeshResult};
use std::f64::consts::{PI, TAU};

const SPHERE: &str = "(x sin(y) cos(z), x sin(y) sin(z), x cos(y))";
const CYLINDER: &str = "(x cos(y), x sin(y), z)";

/// 3 次元で、作る範囲 `dom`（[x の最小, 最大, y …, z …]）・座標系 `map`（空なら無し）で描く。媒介変数 t の範囲は [0, 5]
fn run(src: &str, map: &str, dom: [f64; 6], flat: bool) -> MeshResult {
    run_with(src, map, dom, flat, 100, 60)
}

fn run_with(src: &str, map: &str, dom: [f64; 6], flat: bool, res: u32, ires: u32) -> MeshResult {
    coord_system(map);
    plot(
        src, &dom, res, ires, "t", &[0.0, 5.0], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, &[0.0; 10],
        &[0.0; 5], 0.0, 0, false, flat, 1, false, 0,
    )
    .unwrap_or_else(|_| panic!("{src} が描けない"))
}

fn tri_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let w = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    0.5 * (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt()
}

fn pt(v: &[f32], i: u32) -> [f64; 3] {
    let i = 3 * i as usize;
    [v[i] as f64, v[i + 1] as f64, v[i + 2] as f64]
}

fn norm(p: [f64; 3]) -> f64 {
    (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt()
}

#[test]
fn target_of_sphere() {
    let mut m = run("x = 2", SPHERE, [0.0, 5.0, 0.0, PI, 0.0, TAU], false);
    let (t, idx) = (m.take_target(), m.take_indices());
    assert!(!idx.is_empty());
    assert_eq!(t.len(), m.positions().len());
    for q in t.chunks_exact(3) {
        let r = norm([q[0] as f64, q[1] as f64, q[2] as f64]);
        assert!((r - 2.0).abs() < 1e-4, "{q:?}");
    }
    let mut area = 0.0;
    let mut worst: f64 = 0.0;
    for tri in idx.chunks_exact(3) {
        let (a, b, c) = (pt(&t, tri[0]), pt(&t, tri[1]), pt(&t, tri[2]));
        area += tri_area(a, b, c);
        let g = [0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0);
        worst = worst.max((norm(g) - 2.0).abs());
    }
    assert!((area / (16.0 * PI) - 1.0).abs() < 0.01, "面積 {area}");
    assert!(worst <= 0.01, "重心のずれ {worst}");
}

#[test]
fn target_of_line() {
    let mut m = run("(t, t, t)", SPHERE, [0.0, 5.0, 0.0, PI, 0.0, TAU], false);
    assert!(m.curve());
    let (p, t) = (m.take_positions(), m.take_target());
    assert_eq!(p.len(), t.len());
    let mut count = 0;
    for (q, v) in p.chunks_exact(3).zip(t.chunks_exact(3)) {
        if !q[0].is_finite() {
            assert!(!v[0].is_finite());
            continue;
        }
        let s = q[0] as f64;
        let want = [s * s.sin() * s.cos(), s * s.sin() * s.sin(), s * s.cos()];
        let d = norm([0, 1, 2].map(|k| v[k] as f64 - want[k]));
        assert!(d < 1e-3, "t = {s}: {v:?}");
        count += 1;
    }
    assert!(count > 50, "{count}");
}

#[test]
fn target_of_implicit() {
    let dom = [0.0, 5.0, 0.0, TAU, -5.0, 5.0];
    let mut m = run("x^2 + z^2 = 4", CYLINDER, dom, false);
    let (p, t, idx) = (m.take_positions(), m.take_target(), m.take_indices());
    assert!(!idx.is_empty());
    let used: std::collections::BTreeSet<u32> = idx.iter().copied().collect();
    for &i in &used {
        let q = pt(&p, i);
        for a in 0..3 {
            assert!(q[a] >= dom[2 * a] - 1e-3 && q[a] <= dom[2 * a + 1] + 1e-3, "作る範囲の外: {q:?}");
        }
        let r = norm(pt(&t, i));
        assert!((r - 2.0).abs() < 0.02, "{r}");
    }
    // 粗い版（自分の頂点を持つもの）にも移した先がある
    if m.has_lod_mesh() {
        let (lp, lt) = (m.take_lod_positions(), m.take_lod_target());
        assert_eq!(lp.len(), lt.len());
    }
}

#[test]
fn undefined_target_is_dropped() {
    let mut m = run("z = 0", "(1/x, y, z)", [-5.0, 5.0, -5.0, 5.0, -5.0, 5.0], false);
    let (t, idx, lod) = (m.take_target(), m.take_indices(), m.take_lod_indices());
    assert!(!idx.is_empty());
    assert!(t.iter().all(|v| v.is_finite()));
    let n = (t.len() / 3) as u32;
    assert!(idx.iter().chain(lod.iter()).all(|&i| i < n));
}

#[test]
fn no_map_no_target() {
    let a = run("x^2 + y^2 + z^2 = 4", "", [-5.0, 5.0, -5.0, 5.0, -5.0, 5.0], false);
    assert!(a.has_lod_mesh() || !a.indices().is_empty());
    let mut a = a;
    assert!(a.take_target().is_empty());
}

#[test]
fn flat_target_is_flat() {
    // 2D 表示の極座標のバラ曲線 r = cos(2θ)（座標の値の空間では x = cos(2y) の曲線）
    let mut m = run("x = cos(2y)", CYLINDER, [0.0, 5.0, 0.0, TAU, -5.0, 5.0], true);
    let t = m.take_target();
    assert!(t.iter().any(|v| v.is_finite()));
    for q in t.chunks_exact(3) {
        if q[0].is_finite() {
            assert_eq!(q[2], 0.0);
        }
    }
}

#[test]
fn coord_map_errors() {
    let sp = equation::Space::standard();
    let f = |src: &str| equation::compile_coord_map(src, &Default::default(), &sp).map(|_| ());
    assert!(f(SPHERE).is_ok());
    for src in ["x", "(x, y)", "(x, y, z, x)", "(x, y, w)", "(x, y, a)", "([1, 2], y, z)"] {
        assert!(f(src).is_err(), "{src}");
    }
    let mut ps = graph_core::eval::Params::new();
    ps.insert("a".into(), num_complex::Complex64::new(2.0, 0.0));
    assert!(equation::compile_coord_map("(a x, y, z)", &ps, &sp).is_ok());
}

/// 目で見るための書き出し（`DUMP_DIR` を付けて `--ignored` で回す）。移した先を頂点にした OBJ（線は `l`）
#[test]
#[ignore]
fn dump_obj() {
    let Ok(dir) = std::env::var("DUMP_DIR") else { return };
    let cases: [(&str, &str, &str, [f64; 6]); 4] = [
        ("sphere_x2", "x = 2", SPHERE, [0.0, 5.0, 0.0, PI, 0.0, TAU]),
        ("sphere_line", "(t, t, t)", SPHERE, [0.0, 5.0, 0.0, PI, 0.0, TAU]),
        ("cyl_implicit", "x^2 + z^2 = 4", CYLINDER, [0.0, 5.0, 0.0, TAU, -5.0, 5.0]),
        ("cyl_explicit", "x = 1 + cos(3y)", CYLINDER, [0.0, 5.0, 0.0, TAU, -5.0, 5.0]),
    ];
    for (name, src, map, dom) in cases {
        let mut m = run(src, map, dom, false);
        let curve = m.curve();
        let (t, idx) = (m.take_target(), m.take_indices());
        let mut s = String::new();
        for q in t.chunks_exact(3) {
            s += &format!("v {} {} {}\n", q[0], q[1], q[2]);
        }
        if curve {
            for k in 0..t.len() / 3 - 1 {
                if t[3 * k].is_finite() && t[3 * k + 3].is_finite() {
                    s += &format!("l {} {}\n", k + 1, k + 2);
                }
            }
        } else {
            for tri in idx.chunks_exact(3) {
                s += &format!("f {} {} {}\n", tri[0] + 1, tri[1] + 1, tri[2] + 1);
            }
        }
        std::fs::write(format!("{dir}/{name}.obj"), s).unwrap();
    }
}

/// 時間（計画の完了の条件: 座標系ありの `x = 2` が、無しの 1.3 倍以下）。`--ignored --nocapture` で回して数字を見る
#[test]
#[ignore]
fn timing() {
    let dom = [0.0, 5.0, 0.0, PI, 0.0, TAU];
    let best = |map: &str, src: &str| {
        (0..7)
            .map(|_| {
                let s = std::time::Instant::now();
                std::hint::black_box(run_with(src, map, dom, false, 150, 80));
                s.elapsed().as_secs_f64() * 1e3
            })
            .fold(f64::INFINITY, f64::min)
    };
    for src in ["x = 2", "x^2 + z^2 = 4", "(t, t, t)"] {
        let (a, b) = (best("", src), best(SPHERE, src));
        println!("{src}: 座標系なし {a:.2} ms, あり {b:.2} ms（{:.2} 倍）", b / a);
    }
}

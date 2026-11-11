//! ベンチマーク（手で回す。CI では動かさない）
//!
//! ```text
//! cargo test --release --test bench -- --ignored --nocapture
//! ```
//!
//! 画面と同じ入口（`graph_core::plot`）で式ごとにメッシュを作り、時間の内訳（3 回のうち最も速いもの）と、
//! 頂点・三角形の数、形の傷の数を表にして出す。値は表に出すだけで、確かめはしない。

use graph_core::{equation, plot, MeshResult};

/// 表示範囲 ±r（3 軸とも）
fn cube(r: f64) -> [f64; 6] {
    [-r, r, -r, r, -r, r]
}

/// 計算の設定
#[derive(Clone, Copy)]
struct Setup {
    domain: [f64; 6],
    /// 陽関数・媒介変数表示の細かさ
    res: u32,
    /// 陰関数の細かさ
    ires: u32,
    dims: u32,
    /// 回転（座標 0 と 3 の組、xw 回転）の角度
    xw: f64,
}

impl Setup {
    fn new(r: f64, res: u32, ires: u32) -> Setup {
        Setup { domain: cube(r), res, ires, dims: 3, xw: 0.0 }
    }
}

fn run(src: &str, s: Setup) -> MeshResult {
    let n = s.dims as usize;
    let rd = equation::rot_dims(n);
    let mut rotation = vec![0.0; rd * (rd - 1) / 2];
    // 座標の組は (0,1), (0,2), (0,3), … の順なので、(0,3) は 3 番目
    rotation[2] = s.xw;
    plot(
        src,
        &s.domain,
        s.res,
        s.ires,
        "",
        &[],
        "",
        &[],
        "",
        s.dims,
        &[0, 1, 2],
        &vec![0.0; n],
        &vec![f64::NAN; 2 * n],
        "",
        &[],
        16,
        &rotation,
        &vec![0.0; rd],
        0.0,
        0,
        false,
        false,
        1,
        false,
    )
    .unwrap_or_else(|_| panic!("{src} が計算できない"))
}

/// 3 回計算して、全体の時間が最も短かった回の結果
fn best(src: &str, s: Setup) -> MeshResult {
    let mut keep: Option<(f64, MeshResult)> = None;
    for _ in 0..3 {
        let r = run(src, s);
        let st = r.stats();
        let total = st[0] + st[1] + st[2];
        if keep.as_ref().is_none_or(|(t, _)| total < *t) {
            keep = Some((total, r));
        }
    }
    keep.unwrap().1
}

/// 三角形が1つだけの辺（縁）の数と、3つ以上の三角形が集まる辺（重なり）の数
fn bad_edges(indices: &[u32]) -> (usize, usize) {
    let mut count = std::collections::HashMap::new();
    for t in indices.chunks_exact(3) {
        for e in 0..3 {
            let (u, v) = (t[e], t[(e + 1) % 3]);
            *count.entry((u.min(v), u.max(v))).or_insert(0usize) += 1;
        }
    }
    (count.values().filter(|&&c| c == 1).count(), count.values().filter(|&&c| c >= 3).count())
}

fn header() {
    println!(
        "| {:<40} | {:>4} | {:>9} | {:>9} | {:>9} | {:>9} | {:>9} | {:>8} | {:>8} |",
        "式", "細かさ", "式の計算", "メッシュ", "粗い版", "全体", "頂点", "三角形", "縁/重なり"
    );
    println!("|{}|", ["---"; 9].join("|"));
}

fn row(label: &str, res: u32, r: &MeshResult) {
    let st = r.stats();
    let (b, o) = bad_edges(&r.indices());
    println!(
        "| {:<40} | {:>4} | {:>9.2} | {:>9.2} | {:>9.2} | {:>9.2} | {:>9} | {:>8} | {:>8} |",
        label,
        res,
        st[0],
        st[1],
        st[2],
        st[0] + st[1] + st[2],
        r.positions().len() / 3,
        r.indices().len() / 3,
        format!("{b}/{o}"),
    );
}

const GYROID: &str = "sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x) = 0";
const TORUS: &str = "(sqrt(x^2+y^2)-3)^2+z^2-0.04 = 0";

#[test]
#[ignore]
fn bench_report() {
    println!("\n## 陽関数（範囲 ±5、細かさ 150）。時間は ms\n");
    header();
    for f in [
        "sin(x) cos(y)",
        "x^2 - y^2",
        "1/(x^2+y^2)",
        "tan(x) y",
        "floor(x) + floor(y)",
        "abs(gamma(x + i y))",
        "abs(zeta(x + i y))",
        "sqrt(4 - x^2 - y^2)",
    ] {
        row(&format!("z = {f}"), 150, &best(&format!("z = {f}"), Setup::new(5.0, 150, 80)));
    }

    println!("\n## 陰関数（範囲 ±5。小さい球は ±10）\n");
    header();
    let implicit: Vec<(String, f64, u32)> = vec![
        ("x^2+y^2+z^2-4 = 0".into(), 5.0, 80),
        ("x^2+y^2+z^2-4.01 = 0".into(), 5.0, 80),
        ("x^2+y^2+z^2-4.01 = 0".into(), 5.0, 200),
        (GYROID.into(), 5.0, 80),
        (GYROID.into(), 5.0, 200),
        ("z (x^2+y^2) - 1 = 0".into(), 5.0, 80),
        ("abs(gamma(x + i y)) - z = 0".into(), 5.0, 80),
        ("tan(x) - z = 0".into(), 5.0, 80),
        (TORUS.into(), 5.0, 200),
        ("(x-0.12)^2+(y-0.12)^2+(z-0.12)^2 = 0.01".into(), 10.0, 80),
        ("(x-0.12)^2+(y-0.12)^2+(z-0.12)^2 = 0.04".into(), 10.0, 80),
        ("(x-0.12)^2+(y-0.12)^2+(z-0.12)^2 = 0.09".into(), 10.0, 80),
    ];
    for (f, r, n) in &implicit {
        row(f, *n, &best(f, Setup::new(*r, 150, *n)));
    }

    println!("\n## 媒介変数表示（「メッシュ」は n 次元の点を計算した後の射影を含む。射影だけの時間は別の列）\n");
    println!("| {:<40} | {:>4} | {:>9} | {:>9} | {:>9} | {:>8} |", "図形", "細かさ", "点の計算", "射影だけ", "粗い版", "三角形");
    println!("|{}|", ["---"; 6].join("|"));
    let param = [
        ("曲面（4 次元）", "(cos u, sin u, cos v, sin v)", 4, 150),
        ("曲面（4 次元）", "(cos u, sin u, cos v, sin v)", 4, 300),
        ("立体", "(u cos v, u sin v, t)", 3, 150),
        ("多変数 k=4", "(t_1, t_2, t_3, t_4)", 4, 150),
    ];
    for (label, src, dims, res) in param {
        let s = Setup { domain: cube(8.0), res, ires: 80, dims, xw: 0.5 };
        let mut r = best(src, s);
        let st = r.stats();
        let shape = r.take_shape().expect("媒介変数表示の形");
        let rd = equation::rot_dims(dims as usize);
        let mut rotation = vec![0.0; rd * (rd - 1) / 2];
        rotation[2] = 0.7;
        // 射影だけ（回転を変えて描き直すとき）の時間。3 回の最小
        let proj = (0..3)
            .map(|_| {
                let p = shape.project(&s.domain, dims, &[0, 1, 2], &rotation, &vec![0.0; rd], 0.0).unwrap();
                p.stats()[1]
            })
            .fold(f64::INFINITY, f64::min);
        println!(
            "| {:<40} | {:>4} | {:>9.2} | {:>9.2} | {:>9.2} | {:>8} |",
            format!("{label} {src}"),
            res,
            st[0],
            proj,
            st[2],
            r.indices().len() / 3
        );
    }

    println!("\n## 形の傷\n");
    // tan(x) = z の極の平面 x = ±π/2, ±3π/2 の上の、偽の頂点（|tan x − z| が大きい）
    // 1/x の極 x = 0 は、範囲 ±5 では格子の点になる（値が有限でないので面は張られない）。
    // 格子の点からずらすため、x の範囲を 0.05 だけずらす
    for (f, pole) in [("tan(x) - z = 0", true), ("1/x - z = 0", false)] {
        let mut s = Setup::new(5.0, 150, 80);
        if !pole {
            s.domain[0] += 0.05;
            s.domain[1] += 0.05;
        }
        let r = run(f, s);
        let h = 10.0 / 80.0;
        let poles: Vec<f64> = if pole {
            [-1.5, -0.5, 0.5, 1.5].iter().map(|k| k * std::f64::consts::PI).collect()
        } else {
            vec![0.0]
        };
        let pos = r.positions();
        let fake = pos
            .chunks_exact(3)
            .filter(|q| {
                let (x, z) = (q[0] as f64, q[2] as f64);
                let g = if pole { x.tan() } else { 1.0 / x };
                poles.iter().any(|c| (x - c).abs() < h) && (g - z).abs() > 2.0
            })
            .count();
        println!("- {f}（細かさ 80、x の範囲 {:.2}〜{:.2}）: 極の平面の上の偽の頂点 {fake} / {}", s.domain[0], s.domain[1], pos.len() / 3);
    }
    // |Γ(x + iy)| の極 x = 0, −1, …, −4 の周り（半径 0.3）で、高さが表示範囲の上限 5 以上の頂点の数
    let r = run("z = abs(gamma(x + i y))", Setup::new(5.0, 150, 80));
    let pos = r.positions();
    let counts: Vec<String> = (0..5)
        .map(|k| {
            let c = -(k as f64);
            let n = pos
                .chunks_exact(3)
                .filter(|q| ((q[0] as f64 - c).powi(2) + (q[1] as f64).powi(2)).sqrt() < 0.3 && q[2] >= 5.0)
                .count();
            format!("x = {}: {n}", -(k as i32))
        })
        .collect();
    println!("- z = abs(gamma(x + i y))（細かさ 150）の極の周りで z ≥ 5 の頂点: {}", counts.join(", "));
}

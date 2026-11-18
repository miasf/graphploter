//! 集合（不等式の領域）を回しても形が崩れないこと、表示範囲を広げても粗くならないこと

use graph_core::plot;

struct Mesh {
    p: Vec<[f64; 3]>,
    t: Vec<[usize; 3]>,
}

fn mesh(src: &str, r: f64, rot: &[f64; 10]) -> Mesh {
    let m = plot(src, &[-r, r, -r, r, -r, r], 150, 80, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, rot, &[0.0; 5], 0.0, 0, false, false, 1, false, 0)
        .unwrap_or_else(|e| panic!("{src}: {e:?}"));
    let pos = m.positions();
    let tr = m.transform();
    let p = pos
        .chunks_exact(3)
        .map(|v| {
            let v = [v[0] as f64, v[1] as f64, v[2] as f64];
            if tr.len() == 12 {
                [0, 1, 2].map(|i| tr[4 * i] * v[0] + tr[4 * i + 1] * v[1] + tr[4 * i + 2] * v[2] + tr[4 * i + 3])
            } else {
                v
            }
        })
        .collect();
    let t = m.indices().chunks_exact(3).map(|c| [c[0] as usize, c[1] as usize, c[2] as usize]).collect();
    Mesh { p, t }
}

/// 回転行列（Space::rotation_matrix と同じ。5 次元の組 (0,1),(0,2),(0,3),(0,4),(1,2),…）
fn matrix(rot: &[f64; 10]) -> [[f64; 3]; 3] {
    let n = 5;
    let mut m = vec![vec![0.0; n]; n];
    for i in 0..n {
        m[i][i] = 1.0;
    }
    let mut k = 0;
    for i in 0..n {
        for j in i + 1..n {
            let a = rot[k];
            k += 1;
            let (c, s) = (a.cos(), a.sin());
            let (ri, rj) = (m[i].clone(), m[j].clone());
            for col in 0..n {
                m[i][col] = c * ri[col] - s * rj[col];
                m[j][col] = s * ri[col] + c * rj[col];
            }
        }
    }
    [0, 1, 2].map(|i| [0, 1, 2].map(|j| m[i][j]))
}

fn area(m: &Mesh) -> f64 {
    m.t.iter()
        .map(|&[a, b, c]| {
            let (p, q, r) = (m.p[a], m.p[b], m.p[c]);
            let u = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
            let v = [r[0] - p[0], r[1] - p[1], r[2] - p[2]];
            let w = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            0.5 * (w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt()
        })
        .sum()
}

/// 1 つの三角形にしか使われていない辺の数（位置で頂点をまとめる）。閉じた立体なら 0
fn open(m: &Mesh) -> usize {
    use std::collections::HashMap;
    let key = |v: [f64; 3]| v.map(|x| (x * 1e5).round() as i64);
    let mut id = HashMap::new();
    let ids: Vec<usize> = m.p.iter().map(|&v| { let n = id.len(); *id.entry(key(v)).or_insert(n) }).collect();
    let mut edges: HashMap<(usize, usize), i32> = HashMap::new();
    for t in &m.t {
        let t = t.map(|i| ids[i]);
        if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
            continue;
        }
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    edges.values().filter(|&&c| c == 1).count()
}

/// 頂点を回す前の座標に戻して、領域の外へのはみ出し（各不等式の値の最大）を測る
fn worst(m: &Mesh, rot: &[f64; 10], g: &dyn Fn([f64; 3]) -> f64) -> (f64, [f64; 3]) {
    let r = matrix(rot);
    let mut out = (0.0, [0.0; 3]);
    for &q in &m.p {
        // p_k = Σ_j R[j][k] q_j
        let p = [0, 1, 2].map(|k| (0..3).map(|j| r[j][k] * q[j]).sum::<f64>());
        let v = g(p);
        if v > out.0 {
            out = (v, p);
        }
    }
    out
}

#[test]
#[ignore]
fn report() {
    let rot = {
        let mut r = [0.0; 10];
        r[0] = 0.5;
        r[1] = 0.35;
        r[4] = 0.17;
        r
    };
    let cases: Vec<(&str, Box<dyn Fn([f64; 3]) -> f64>)> = vec![
        (
            "⦃(x,y,z)∣x^2+y^2<=1, 0<=z<=x^3+y^3⦄",
            Box::new(|p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] - 1.0).max(-p[2]).max(p[2] - p[0].powi(3) - p[1].powi(3))),
        ),
        ("⦃(x,y,z)∣abs(x)<=1, abs(y)<=0.5, 0<=z<=1⦄", Box::new(|p: [f64; 3]| (p[0].abs() - 1.0).max(p[1].abs() - 0.5).max(-p[2]).max(p[2] - 1.0))),
        ("⦃(x,y,z)∣x^2+y^2+z^2<=1⦄", Box::new(|p: [f64; 3]| p[0] * p[0] + p[1] * p[1] + p[2] * p[2] - 1.0)),
        ("⦃(x,y,z)∣x^2+y^2<=1, abs(z)<=0.5⦄", Box::new(|p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] - 1.0).max(p[2].abs() - 0.5))),
        ("⦃(x,y,z)∣x+y+z<=1, x>=0, y>=0, z>=0⦄", Box::new(|p: [f64; 3]| (p[0] + p[1] + p[2] - 1.0).max(-p[0]).max(-p[1]).max(-p[2]))),
        ("⦃(x,y,z)∣x^2+y^2<=z^2, 0<=z<=1⦄", Box::new(|p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] - p[2] * p[2]).max(-p[2]).max(p[2] - 1.0))),
        ("⦃(x,y,z)∣x^2+y^2<=z, z<=1⦄", Box::new(|p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] - p[2]).max(p[2] - 1.0))),
        ("⦃(x,y,z)∣x^2+y^2+z^2<=1, z>=x^2+y^2⦄", Box::new(|p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2] - 1.0).max(p[0] * p[0] + p[1] * p[1] - p[2]))),
        ("⦃(x,y,z)∣x^2+y^2=1, 0<=z<=1⦄", Box::new(|p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] - 1.0).abs().max(-p[2]).max(p[2] - 1.0))),
        ("⦃(x,y,z)∣x^2+y^2<=1, abs(y)<=z<=1⦄", Box::new(|p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] - 1.0).max(p[1].abs() - p[2]).max(p[2] - 1.0))),
    ];
    for (src, g) in &cases {
        for half in [1.5, 6.0] {
            let t0 = std::time::Instant::now();
            let a = mesh(src, half, &[0.0; 10]);
            let t1 = t0.elapsed().as_secs_f64() * 1e3;
            let b = mesh(src, half, &rot);
            let t2 = t0.elapsed().as_secs_f64() * 1e3 - t1;
            println!("  {t1:.0} ms / {t2:.0} ms");
            let (wa, _) = worst(&a, &[0.0; 10], g.as_ref());
            let (wb, pb) = worst(&b, &rot, g.as_ref());
            println!(
                "{src} half={half}: tri {} / {}  area {:.4} / {:.4}  open {} / {}  worst {:.2e} / {:.2e} at {:?}",
                a.t.len(),
                b.t.len(),
                area(&a),
                area(&b),
                open(&a),
                open(&b),
                wa,
                wb,
                pb
            );
        }
    }
}

/// 表示範囲を広げても粗くならない（図形の周りだけの格子で作り直す。lib.rs の fit_to_shape）。
/// 前は表示範囲 ±6 で、±1.5 のときの 1/13 ほどの三角形になっていた
#[test]
fn wide_domain_keeps_detail() {
    for src in ["⦃(x,y,z)∣x^2+y^2<=1, 0<=z<=x^3+y^3⦄", "⦃(x,y,z)∣x+y+z<=1, x>=0, y>=0, z>=0⦄", "x^2+y^2+z^2=1", "max(abs(x), abs(y), abs(z)) = 0.5", "⦃(x,y,z)∣x^2+y^2=1, 0<=z<=1⦄"] {
        let near = mesh(src, 1.5, &[0.0; 10]).t.len();
        let far = mesh(src, 6.0, &[0.0; 10]).t.len();
        assert!(far as f64 >= 0.5 * near as f64, "{src}: ±6 で {far} 個（±1.5 で {near} 個）");
    }
}

/// 集合を回しても形が変わらない（面積が同じで、頂点が回す前の不等式を満たす）
#[test]
fn rotated_sets_keep_shape() {
    let mut rot = [0.0; 10];
    rot[0] = 0.5;
    rot[1] = 0.35;
    rot[4] = 0.17;
    let cases: Vec<(&str, Box<dyn Fn([f64; 3]) -> f64>)> = vec![
        ("⦃(x,y,z)∣x+y+z<=1, x>=0, y>=0, z>=0⦄", Box::new(|p: [f64; 3]| (p[0] + p[1] + p[2] - 1.0).max(-p[0]).max(-p[1]).max(-p[2]))),
        ("⦃(x,y,z)∣x^2+y^2<=1, abs(z)<=0.5⦄", Box::new(|p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] - 1.0).max(p[2].abs() - 0.5))),
        ("⦃(x,y,z)∣x^2+y^2<=z^2, 0<=z<=1⦄", Box::new(|p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] - p[2] * p[2]).max(-p[2]).max(p[2] - 1.0))),
    ];
    for (src, g) in &cases {
        let a = mesh(src, 1.5, &[0.0; 10]);
        let b = mesh(src, 1.5, &rot);
        let (sa, sb) = (area(&a), area(&b));
        assert!((sa - sb).abs() <= 0.01 * sa, "{src}: 面積 {sa} → {sb}");
        let (w, p) = worst(&b, &rot, g.as_ref());
        assert!(w <= 1e-3, "{src}: 頂点 {p:?} が領域の外（{w}）");
    }
}

#[test]
#[ignore]
fn axis_angles() {
    let g = |p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] - 1.0).max(-p[2]).max(p[2] - p[0].powi(3) - p[1].powi(3));
    let src = "⦃(x,y,z)∣x^2+y^2<=1, 0<=z<=x^3+y^3⦄";
    let a0 = area(&mesh(src, 1.5, &[0.0; 10]));
    let d = mesh("⦃(x,y,z)∣x^2+z^2<=1, 0<=-y<=x^3+z^3⦄", 1.5, &[0.0; 10]);
    println!("plane direct: tri {} area {:.4} open {}", d.t.len(), area(&d), open(&d));
    for idx in [4usize, 1] {
        for deg in [30.0f64, 60.0, 90.0, 135.0] {
            let mut rot = [0.0; 10];
            rot[idx] = deg.to_radians();
            let m = mesh(src, 1.5, &rot);
            let (w, p) = worst(&m, &rot, &g);
            println!("plane {idx} {deg}°: tri {} area {:.4} (0°: {a0:.4}) open {} worst {w:.2e} at {p:?}", m.t.len(), area(&m), open(&m));
        }
    }
}

/// 境目の面を陽関数にするとき、根号の無い解の座標を選ぶ（0 ≤ −y ≤ x³ + z³ は z = ∛(−y − x³) でなく y = −x³ − z³）。
/// 前は ∛ の面が z ≈ 0 で垂直になって縞と隙間が出て、面積が 0.6% 多かった（x 軸の周りに 90° 回したときも同じ）
#[test]
fn faces_avoid_radicals() {
    let a0 = area(&mesh("⦃(x,y,z)∣x^2+y^2<=1, 0<=z<=x^3+y^3⦄", 1.5, &[0.0; 10]));
    let d = area(&mesh("⦃(x,y,z)∣x^2+z^2<=1, 0<=-y<=x^3+z^3⦄", 1.5, &[0.0; 10]));
    assert!((d - a0).abs() <= 1e-3 * a0, "面積 {d}（回す前の形で {a0}）");
}

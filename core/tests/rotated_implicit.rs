//! 回転が表示中の座標どうしだけの陰関数を、回す前の形で作って回す道（`mesh::LocalMesh`）のテスト。
//! （plot の flags のビット 3）。回転だけ変えたとき、F を計算し直さずに（`Shape::project`）、その回転で式から作ったものと同じ面・切り口になるか

use graph_core::{equation, eval, plot, MeshResult};
use num_complex::Complex64 as C;

const DOMAIN: [f64; 6] = [-10.0, 10.0, -10.0, 10.0, -6.0, 6.0];

fn angles(xy: f64, xz: f64) -> Vec<f64> {
    let mut a = vec![0.0; 10];
    a[0] = xy;
    a[1] = xz;
    a
}

/// `local` なら、回す前の形で作る（flags のビット 3）。そうでなければ今までの道（回した式で表示範囲に作る）
fn mesh(src: &str, rot: &[f64], ires: u32, local: bool) -> MeshResult {
    plot(
        src, &DOMAIN, 150, ires, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, rot, &[0.0; 5],
        0.0, 0, false, false, 1, false, if local { 8 } else { 0 },
    )
    .unwrap_or_else(|_| panic!("{src} が計算できない"))
}

/// その回転で、表示中の座標の F（回した式）
fn rotated_prog(src: &str, rot: &[f64]) -> eval::Program {
    let mut sp = equation::Space::basic(3, [0, 1, 2], vec![0.0; 3]);
    sp.rotation = rot.to_vec();
    match equation::compile_with(src, &eval::Params::new(), &sp).unwrap() {
        equation::Plot::Implicit { prog, .. } => prog,
        _ => panic!("{src} が陰関数にならない"),
    }
}

fn values(prog: &eval::Program, pos: &[f32]) -> Vec<f64> {
    let cols: Vec<Vec<f64>> = (0..3).map(|a| pos.chunks_exact(3).map(|p| p[a] as f64).collect()).collect();
    let mut out = vec![C::new(0.0, 0.0); cols[0].len()];
    prog.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
    out.iter().map(|v| v.re).collect()
}

fn inside(p: &[f32]) -> bool {
    (0..3).all(|a| p[a] as f64 >= DOMAIN[2 * a] && p[a] as f64 <= DOMAIN[2 * a + 1])
}

#[test]
fn reprojected_surface_matches_the_rotation() {
    // 中心からずれた球（回すと動く）。表示範囲の外にはみ出す大きさにする
    let src = "(x - 6)^2 + y^2 + (z - 2)^2 = 25";
    let mut r = mesh(src, &angles(0.4, 0.2), 60, true);
    assert!(r.kind().starts_with("陰関数"), "{}", r.kind());
    let shape = r.take_shape().expect("回す前の形を持っていない");
    assert!(shape.view_dependent());
    for (xy, xz) in [(0.4, 0.2), (1.3, -0.5), (2.9, 0.8), (0.0, 0.0)] {
        let rot = angles(xy, xz);
        let mut m = shape
            .project(&DOMAIN, 3, &[0, 1, 2], &rot, &[0.0; 5], 0.0)
            .unwrap_or_else(|_| panic!("{xy}, {xz} で射影できない"));
        let pos = m.positions();
        let prog = rotated_prog(src, &rot);
        let f = values(&prog, &pos);
        let mut shown = 0;
        for (p, v) in pos.chunks_exact(3).zip(&f) {
            if inside(p) {
                shown += 1;
                // F = |p − c|² − 25 で、格子の 1 マス（約 0.5）ずれると |F| は 5 くらい
                assert!(v.abs() < 3.0, "{xy}, {xz}: 頂点 {p:?} が面から離れている（F = {v}）");
            }
        }
        assert!(shown > 500, "{xy}, {xz}: 表示範囲の中の頂点が少ない（{shown}）");
        // 表示範囲を覆っている: 回した式から作ったものと、表示範囲の中の三角形の数がだいたい同じ
        let direct = {
            let d = mesh(src, &rot, 60, false);
            let (p, ix) = (d.positions(), d.indices());
            ix.chunks_exact(3).filter(|t| t.iter().all(|&k| inside(&p[k as usize * 3..k as usize * 3 + 3]))).count()
        };
        let ix = m.indices();
        let ours = ix.chunks_exact(3).filter(|t| t.iter().all(|&k| inside(&pos[k as usize * 3..k as usize * 3 + 3]))).count();
        if xy != 0.0 {
            // 回す前の形は、マスの数を表示範囲と同じにして、表示範囲より広い立方体に作る（マスが 1.5〜2.5 倍大きい）ので、三角形は少ない
            let ratio = ours as f64 / direct as f64;
            assert!((0.15..1.5).contains(&ratio), "{xy}, {xz}: 三角形の数が {ours}（式から作ると {direct}）");
        }
        // 切り口のふたも、その回転のもの（ふたの点は球の中）
        let caps = m.take_field().expect("ふたの元が無い");
        let cap = caps.cap(2, 1.0, true, 80).unwrap();
        let cp = cap.positions();
        assert!(!cp.is_empty(), "{xy}, {xz}: ふたが無い");
        let cf = values(&prog, &cp);
        assert!(cf.iter().all(|&v| v < 1.0), "{xy}, {xz}: ふたが球の外にある");
    }
}

#[test]
fn plain_requests_keep_the_old_path() {
    // 印（flags のビット 3）の無い依頼は、回す前の形を作らない（今までどおり、回した式で作る）
    let mut plain = mesh("x^2 + y^2 + z^2 = 25", &angles(0.4, 0.2), 30, false);
    assert!(plain.take_shape().is_none());
    // 表示する面だけを作り、回す前の形を作れることを知らせる（Worker は面を先に返してから、印を付けてもう一度呼ぶ）
    assert!(plain.local_available());
    assert!(!plain.indices().is_empty());
    // 印の付いた依頼は、回す前の形だけを作る（表示する面は作らない）
    let mut only = mesh("x^2 + y^2 + z^2 = 25", &angles(0.4, 0.2), 30, true);
    assert!(only.indices().is_empty());
    assert!(only.take_shape().is_some_and(|s| s.rigid()));
    // 陽関数（回すと形の点を使い回す、別の道）は、回す前の陰関数の形を作らない
    assert!(!mesh("z = x y / 10", &angles(0.4, 0.2), 30, false).local_available());
    // 回転が無ければ、印があっても今までどおり
    assert!(mesh("x^2 + y^2 + z^2 = 25", &angles(0.0, 0.0), 30, true).take_shape().is_none());
}

#[test]
fn rotation_with_hidden_coords_recomputes() {
    // 4 次元で、式が使う w が絡む回転（xw）は、回す前の形を回しても描けない（作り直しの合図を返す）
    let src = "x^2 + y^2 + z^2 + w^2 = 9";
    let mut rot = vec![0.0; 10];
    rot[0] = 0.5;
    let mut r = plot(
        src, &DOMAIN, 150, 40, "", &[], "", &[], "", 4, &[0, 1, 2], &[0.0; 4], &[f64::NAN; 8], "", &[], 16, &rot, &[0.0; 5],
        0.0, 0, false, false, 1, false, 8,
    )
    .unwrap_or_else(|_| panic!("計算できない"));
    let shape = r.take_shape().expect("回す前の形を持っていない");
    // 表示中の座標どうし（xy）だけなら回せる
    rot[0] = 1.1;
    assert!(shape.project(&DOMAIN, 4, &[0, 1, 2], &rot, &[0.0; 5], 0.0).is_ok());
    // エラー（JsError）は wasm でしか作れないので、ここでは作ろうとして止まることで確かめる
    rot[2] = 0.3; // xw
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        shape.project(&DOMAIN, 4, &[0, 1, 2], &rot, &[0.0; 5], 0.0).is_ok()
    }));
    assert!(!matches!(res, Ok(true)), "w が絡む回転なのに、回す前の形を回して描いた");
}

/// 時間の比べ（`cargo test --release --test rotated_implicit -- --ignored --nocapture`）:
/// 今までの道（回転のたびに、回した式で作り直す）と、回す前の形を 1 回作って回す道。
/// 再生中と同じ軽い計算（flags = 1）の細かさ 40・24 と、止まったときの 80
#[test]
#[ignore]
fn timing_report() {
    use std::time::Instant;
    let cases = [
        "sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x) = 0",
        "x^2 + y^2 + z^2 = 25",
        "(sqrt(x^2 + y^2) - 5)^2 + z^2 = 4",
        "mod(x, 2) + mod(y, 2) + mod(z, 2) = 2",
        "tan(x) = z y",
    ];
    println!("| 式 | 細かさ | 回す前の形を作る ms | 回すだけ ms/コマ | 今まで（作り直し）ms/コマ |");
    println!("|---|---|---|---|---|");
    for src in cases {
        for (ires, flags) in [(80u32, 0u32), (40, 1), (24, 1)] {
            let call = |rot: &[f64], flags: u32| {
                plot(
                    src, &DOMAIN, 150, ires, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, rot,
                    &[0.0; 5], 0.0, 0, false, false, 1, false, flags | 8,
                )
                .unwrap_or_else(|_| panic!("{src}"))
            };
            let t = Instant::now();
            let mut r = call(&angles(0.25, 0.17), flags);
            let build = t.elapsed().as_secs_f64() * 1e3;
            let Some(shape) = r.take_shape() else {
                println!("| `{src}` | {ires} | （段差のある式なので今までの道）| | {build:.0} |");
                continue;
            };
            let frames = 12;
            let t = Instant::now();
            for k in 0..frames {
                let mut m = shape.project(&DOMAIN, 3, &[0, 1, 2], &angles(0.25 + 0.05 * k as f64, 0.17), &[0.0; 5], 0.0).unwrap();
                std::hint::black_box(m.take_positions());
            }
            let reuse = t.elapsed().as_secs_f64() * 1e3 / frames as f64;
            // 今までの道: 回した式を作り、表示範囲の格子で作り直す（xy の回転を毎コマ変える）
            let t = Instant::now();
            for k in 0..3 {
                let rot = angles(0.25 + 0.05 * k as f64, 0.17);
                let mut sp = equation::Space::basic(3, [0, 1, 2], vec![0.0; 3]);
                sp.rotation = rot.clone();
                let prog = rotated_prog(src, &rot);
                let dom = graph_core::mesh::Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
                let refine = if flags == 1 { graph_core::mesh::Refine::Light } else { graph_core::mesh::Refine::Normal };
                let m = graph_core::mesh::implicit_solid_opt(&prog, dom, ires as usize, &[], 0, refine);
                std::hint::black_box(m.indices.len());
            }
            let old = t.elapsed().as_secs_f64() * 1e3 / 3.0;
            println!("| `{src}` | {ires}{} | {build:.0} | {reuse:.1} | {old:.0} |", if flags == 1 { "（再生中）" } else { "" });
        }
    }
}

/// 法線のずれ（`cargo test --release --test rotated_implicit normal_report -- --ignored --nocapture`）:
/// 頂点の法線と、本当の面の向き（式の勾配を細かい刻みで求めたもの）の角度（度）。止まったとき（80）と再生中（24）
#[test]
#[ignore]
fn normal_report() {
    let cases = [
        "x^2 + y^2 + z^2 = 25",
        "(sqrt(x^2 + y^2) - 6)^2 + z^2 = 4",
        "sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x) = 0",
        "cos(x) + cos(y) + cos(z) = 0",
        "((x/4)^2 + 9/4 (y/4)^2 + (z/4)^2 - 1)^3 - (x/4)^2 (z/4)^3 - 9/80 (y/4)^2 (z/4)^3 = 0",
        "(x/4)^2 (y/4)^2 + (y/4)^2 (z/4)^2 + (z/4)^2 (x/4)^2 - (x/4) (y/4) (z/4) = 0",
        "x^2 + y^2 = z^2",
        "abs(x) + abs(y) + abs(z) = 6",
        "x^8 + y^8 + z^8 = 5^8",
    ];
    println!("| 式 | 細かさ | 平均 ° | 95% ° | 最大 ° |");
    println!("|---|---|---|---|---|");
    for src in cases {
        let prog = rotated_prog_none(src);
        for (ires, flags) in [(80u32, 0u32), (24, 1)] {
            let r = plot(
                src, &DOMAIN, 150, ires, "", &[], "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16,
                &[0.0; 10], &[0.0; 5], 0.0, 0, false, false, 1, false, flags,
            )
            .unwrap_or_else(|_| panic!("{src}"));
            let (pos, nrm) = (r.positions(), r.normals());
            let mut errs: Vec<f64> = Vec::new();
            let h = 1e-5;
            for (p, n) in pos.chunks_exact(3).zip(nrm.chunks_exact(3)) {
                if !inside(p) {
                    continue;
                }
                let at = |d: [f64; 3]| {
                    let mut o = [C::new(0.0, 0.0)];
                    prog.eval_batch(&[&[p[0] as f64 + d[0]], &[p[1] as f64 + d[1]], &[p[2] as f64 + d[2]]], &mut o);
                    o[0].re
                };
                let g = [0, 1, 2].map(|a| {
                    let mut e = [0.0; 3];
                    e[a] = h;
                    (at(e) - at(e.map(|v| -v))) / (2.0 * h)
                });
                let len = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
                if !(len.is_finite() && len > 1e-9) {
                    continue;
                }
                let d = ((g[0] * n[0] as f64 + g[1] * n[1] as f64 + g[2] * n[2] as f64) / len).abs().min(1.0);
                errs.push(d.acos().to_degrees());
            }
            errs.sort_by(|a, b| a.total_cmp(b));
            let mean = errs.iter().sum::<f64>() / errs.len().max(1) as f64;
            let p95 = errs.get(errs.len() * 95 / 100).copied().unwrap_or(0.0);
            let max = errs.last().copied().unwrap_or(0.0);
            println!("| `{src}` | {ires} | {mean:.2} | {p95:.2} | {max:.1} |");
        }
    }
}

/// 回さない、表示中の座標の F
fn rotated_prog_none(src: &str) -> eval::Program {
    rotated_prog(src, &[0.0; 10])
}

#[test]
fn transform_matches_projection() {
    // 回す前のメッシュに 1 次式を掛けたもの（描画側で 3D オブジェクトを回す）が、射影し直したメッシュと同じ
    let src = "(x - 6)^2 + y^2 + (z - 2)^2 = 25";
    let mut r = mesh(src, &angles(0.4, 0.2), 40, true);
    let shape = r.take_shape().unwrap();
    assert!(shape.rigid());
    let local = shape.local_mesh().unwrap().positions();
    for (xy, xz) in [(0.4, 0.2), (1.7, -0.6)] {
        let rot = angles(xy, xz);
        let t = shape.transform(&DOMAIN, 3, &[0, 1, 2], &rot, &[0.0; 5]).unwrap().transform();
        assert_eq!(t.len(), 12);
        let projected = shape.project(&DOMAIN, 3, &[0, 1, 2], &rot, &[0.0; 5], 0.0).unwrap().positions();
        assert_eq!(local.len(), projected.len());
        for (p, q) in local.chunks_exact(3).zip(projected.chunks_exact(3)) {
            for a in 0..3 {
                let v = t[4 * a] * p[0] as f64 + t[4 * a + 1] * p[1] as f64 + t[4 * a + 2] * p[2] as f64 + t[4 * a + 3];
                assert!((v - q[a] as f64).abs() < 1e-3, "{xy}, {xz}: {v} と {}", q[a]);
            }
        }
    }
}

#[test]
fn stepped_surfaces_keep_their_branches_when_rotated() {
    // 段差のある式も回す前の形で作る。枝は、回す範囲（立方体）の角まで調べると部品の数の上限を超えて、表示範囲の中の枝まで
    // 落ちていた（この式で 2 割）。内接する球の中だけで調べるので、表示範囲の中は全部描く
    let src = "mod(x, 2) + mod(y, 2) + mod(z, 2) = 2";
    let rot = angles(0.25, 0.17);
    let mut r = mesh(src, &rot, 80, true);
    let shape = r.take_shape().expect("回す前の形を持っていない");
    let m = shape.project(&DOMAIN, 3, &[0, 1, 2], &rot, &[0.0; 5], 0.0).unwrap_or_else(|_| panic!("射影できない"));
    // 表示範囲を覆っている: 回した式から表示範囲に作った面の頂点の近く（0.6 以内の箱とその隣）に、回す前の形の頂点がある
    let key = |p: &[f32]| [0, 1, 2].map(|a| (p[a] as f64 / 0.6).floor() as i64);
    let ours: std::collections::HashSet<[i64; 3]> = m.positions().chunks_exact(3).map(key).collect();
    let d = mesh(src, &rot, 80, false);
    let dp = d.positions();
    let pts: Vec<&[f32]> = dp.chunks_exact(3).filter(|p| inside(p)).collect();
    let near = |k: [i64; 3]| (0..27).any(|d| ours.contains(&[k[0] + d % 3 - 1, k[1] + d / 3 % 3 - 1, k[2] + d / 9 - 1]));
    let missing = pts.iter().filter(|p| !near(key(p))).count();
    assert!(pts.len() > 1000, "式から作った面が小さい（{}）", pts.len());
    assert!((missing as f64) < 0.01 * pts.len() as f64, "回す前の形で欠けている頂点が {missing} / {}", pts.len());
}

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
            // 回した道の格子の間隔は、表示範囲の一番狭い座標（z）に合わせるので、x・y の向きには細かい
            let ratio = ours as f64 / direct as f64;
            assert!((0.5..4.0).contains(&ratio), "{xy}, {xz}: 三角形の数が {ours}（式から作ると {direct}）");
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
fn steps_and_plain_requests_keep_the_old_path() {
    // 段差のある式と、印（flags のビット 3）の無い依頼は、回す前の形を作らない（今までどおり、回した式で作る）
    assert!(mesh("mod(x, 2) + mod(y, 2) + mod(z, 2) = 2", &angles(0.4, 0.2), 30, true).take_shape().is_none());
    assert!(mesh("x^2 + y^2 + z^2 = 25", &angles(0.4, 0.2), 30, false).take_shape().is_none());
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

//! 画素ごとの判定（計画 12 章の G2）の物差し。決まったカメラから光線を飛ばし、
//! 正解（繰り返しの上限 1 万回・1/16 画素まで詰める）と、(a) ふつうの設定の光線、(b) 今のメッシュ（画面に描き込んだ深さ）を比べる。
//!
//! 表を出す: `cargo test --release --test pixel -- --ignored --nocapture pixel_report`（式を絞るなら環境変数 PIXEL_ONLY）

use graph_core::eval::Program;
use graph_core::parser::{parse_system, Rel};
use graph_core::pixel::{ray_box, ray_hit, RayHit, RaySettings};
use graph_core::plot;
use std::time::Instant;

/// 7.1（`shape_report`）と同じ表示範囲
const LO: [f64; 3] = [-10.0, -10.0, -6.4];
const HI: [f64; 3] = [10.0, 10.0, 6.4];
const FOV_DEG: f64 = 40.0;

/// `core/tests/shape.rs` の陰関数 16 式と同じ
const IMPLICIT: [&str; 16] = [
    "sqrt(x)+sqrt(y)+sqrt(z)=3",
    "max(abs(x),max(abs(y),abs(z))) = 4",
    "abs(x)+abs(y)+abs(z)=6",
    "x^2+y^2 = z^2",
    "x y z = 1",
    "sin(x) sin(y) sin(z) = 0.001",
    "x y = 0.0001",
    "x^2 y^2 + y^2 z^2 + z^2 x^2 = 1",
    "ln(x) + ln(y) + ln(z) = 1",
    "tan(x) = z y",
    "arcsin(x/5) + arcsin(y/5) = z",
    "sin(40/(x^2+y^2+z^2+1)) = 0.5",
    "x^(2/3) + y^(2/3) + z^(2/3) = 4",
    "z^2 = sin(x) sin(y)",
    "sqrt(36 - x^2 - y^2) - z = 0",
    "1/(x y) - z = 0",
];

/// 追加で見る式（接する面・細い球・段差・ジャイロイド）
const EXTRA: [&str; 5] = [
    "(x^2+y^2+z^2-16)^2 = 0",
    "x^2+y^2+z^2 = 0.0001",
    "floor(x) = z",
    "sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x) = 0",
    "x^2+y^2+z^2 = 16",
];

/// カメラ: 原点を見る。`yaw`・`pitch` はラジアン
struct Camera {
    eye: [f64; 3],
    fwd: [f64; 3],
    right: [f64; 3],
    up: [f64; 3],
    w: usize,
    h: usize,
    tan_half: f64,
}

fn norm(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / l, v[1] / l, v[2] / l]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl Camera {
    fn new(w: usize, h: usize) -> Camera {
        let (yaw, pitch, dist): (f64, f64, f64) = (-0.9, 0.45, 34.0);
        let eye = [dist * pitch.cos() * yaw.cos(), dist * pitch.cos() * yaw.sin(), dist * pitch.sin()];
        let fwd = norm([-eye[0], -eye[1], -eye[2]]);
        let right = norm([fwd[1], -fwd[0], 0.0]);
        let up = [right[1] * fwd[2] - right[2] * fwd[1], right[2] * fwd[0] - right[0] * fwd[2], right[0] * fwd[1] - right[1] * fwd[0]];
        Camera { eye, fwd, right, up, w, h, tan_half: (FOV_DEG.to_radians() / 2.0).tan() }
    }

    /// 画素 (i, j) の光線の向き（長さ 1）
    fn dir(&self, i: usize, j: usize) -> [f64; 3] {
        let aspect = self.w as f64 / self.h as f64;
        let x = ((i as f64 + 0.5) / self.w as f64 * 2.0 - 1.0) * self.tan_half * aspect;
        let y = (1.0 - (j as f64 + 0.5) / self.h as f64 * 2.0) * self.tan_half;
        norm([0, 1, 2].map(|k| self.fwd[k] + x * self.right[k] + y * self.up[k]))
    }

    /// 距離 t での 1 画素の大きさの係数
    fn pixel(&self) -> f64 {
        2.0 * self.tan_half / self.h as f64
    }
}

/// 陰関数の F（左辺 − 右辺）
fn field(src: &str) -> Program {
    let chains = parse_system(src).unwrap();
    let c = &chains[0];
    let e = if c.rels == [Rel::Eq] {
        graph_core::parser::Expr::Bin(graph_core::parser::BinOp::Sub, Box::new(c.exprs[0].clone()), Box::new(c.exprs[1].clone()))
    } else {
        c.exprs[0].clone()
    };
    Program::compile(&e, &["x", "y", "z"]).unwrap()
}

/// 全部の画素の光線の結果（当たれば t、当たらなければ None、打ち切りは Some(NaN)）
fn rays(prog: &Program, cam: &Camera, s: RaySettings) -> Vec<Option<f64>> {
    let mut out = Vec::with_capacity(cam.w * cam.h);
    for j in 0..cam.h {
        for i in 0..cam.w {
            let d = cam.dir(i, j);
            let r = match ray_box(cam.eye, d, LO, HI) {
                Some((t0, t1)) => ray_hit(prog, cam.eye, d, t0, t1, s),
                None => RayHit::Miss,
            };
            out.push(match r {
                RayHit::Hit { t, .. } => Some(t),
                RayHit::Miss => None,
                RayHit::Aborted => Some(f64::NAN),
            });
        }
    }
    out
}

/// 今のメッシュ（アプリと同じ plot の結果）を画面に描き込み、画素ごとの最も手前の t を求める
fn mesh_depths(src: &str, cam: &Camera) -> Vec<Option<f64>> {
    let m = plot(
        src,
        &[LO[0], HI[0], LO[1], HI[1], LO[2], HI[2]],
        150,
        80,
        "t",
        &[0.0, 1.0],
        "",
        &[],
        "",
        3,
        &[0, 1, 2],
        &[0.0; 3],
        &[f64::NAN; 6],
        "",
        &[],
        16,
        &[0.0; 10],
        &[0.0; 5],
        0.0,
        0,
        false,
        false,
        1,
        false,
        0,
    )
    .unwrap_or_else(|_| panic!("{src} のメッシュが作れない"));
    let (pos, idx) = (m.positions(), m.indices());
    let mut depth: Vec<Option<f64>> = vec![None; cam.w * cam.h];
    let aspect = cam.w as f64 / cam.h as f64;
    // 頂点を画面へ: (画素の x, 画素の y, カメラの奥行き z)
    let proj = |k: u32| -> Option<[f64; 3]> {
        let p = [pos[3 * k as usize] as f64, pos[3 * k as usize + 1] as f64, pos[3 * k as usize + 2] as f64];
        if !p.iter().all(|v| v.is_finite()) {
            return None;
        }
        let v = [p[0] - cam.eye[0], p[1] - cam.eye[1], p[2] - cam.eye[2]];
        let z = dot(v, cam.fwd);
        if z <= 1e-6 {
            return None;
        }
        let sx = dot(v, cam.right) / z / (cam.tan_half * aspect);
        let sy = dot(v, cam.up) / z / cam.tan_half;
        Some([(sx + 1.0) / 2.0 * cam.w as f64 - 0.5, (1.0 - sy) / 2.0 * cam.h as f64 - 0.5, z])
    };
    for tri in idx.chunks_exact(3) {
        let (Some(a), Some(b), Some(c)) = (proj(tri[0]), proj(tri[1]), proj(tri[2])) else { continue };
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        if area.abs() < 1e-12 {
            continue;
        }
        let x0 = a[0].min(b[0]).min(c[0]).ceil().max(0.0) as usize;
        let x1 = (a[0].max(b[0]).max(c[0]).floor() as i64).min(cam.w as i64 - 1);
        let y0 = a[1].min(b[1]).min(c[1]).ceil().max(0.0) as usize;
        let y1 = (a[1].max(b[1]).max(c[1]).floor() as i64).min(cam.h as i64 - 1);
        if x1 < 0 || y1 < 0 {
            continue;
        }
        for py in y0..=y1 as usize {
            for px in x0..=x1 as usize {
                let (x, y) = (px as f64, py as f64);
                let w0 = ((b[0] - x) * (c[1] - y) - (b[1] - y) * (c[0] - x)) / area;
                let w1 = ((c[0] - x) * (a[1] - y) - (c[1] - y) * (a[0] - x)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                // 奥行きは 1/z を画面の上で線形に補間する（透視投影）
                let z = 1.0 / (w0 / a[2] + w1 / b[2] + w2 / c[2]);
                let t = z / dot(cam.dir(px, py), cam.fwd);
                let d = &mut depth[py * cam.w + px];
                if d.is_none_or(|old| t < old) {
                    *d = Some(t);
                }
            }
        }
    }
    depth
}

/// 正解と比べた数字: (違う画素の割合 %, 深さの差の 95 パーセンタイル（画素の大きさの何倍か）, 打ち切った画素の割合 %)
fn compare(truth: &[Option<f64>], got: &[Option<f64>], pixel: f64) -> (f64, f64, f64) {
    let (mut n, mut wrong, mut aborted) = (0usize, 0usize, 0usize);
    let mut diffs = Vec::new();
    for (t, g) in truth.iter().zip(got) {
        if t.is_some_and(f64::is_nan) {
            continue; // 正解が打ち切った画素は数えない
        }
        n += 1;
        if g.is_some_and(f64::is_nan) {
            aborted += 1;
            continue;
        }
        match (t, g) {
            (Some(t), Some(g)) => diffs.push((t - g).abs() / (pixel * t)),
            (None, None) => {}
            _ => wrong += 1,
        }
    }
    diffs.sort_by(f64::total_cmp);
    let p95 = if diffs.is_empty() { 0.0 } else { diffs[(diffs.len() * 95 / 100).min(diffs.len() - 1)] };
    (100.0 * wrong as f64 / n.max(1) as f64, p95, 100.0 * aborted as f64 / n.max(1) as f64)
}

fn settings(cam: &Camera) -> (RaySettings, RaySettings) {
    (RaySettings { pixel: cam.pixel(), max_iter: 512 }, RaySettings { pixel: cam.pixel() / 16.0, max_iter: 10_000 })
}

#[test]
fn small_report_sphere_and_touching() {
    // 小さい画面（40 × 30）で: 球は正解と全部一致、接する球も当たる
    let cam = Camera::new(40, 30);
    let (s, truth_s) = settings(&cam);
    for src in ["x^2+y^2+z^2 = 16", "(x^2+y^2+z^2-16)^2 = 0"] {
        let p = field(src);
        let truth = rays(&p, &cam, truth_s);
        let got = rays(&p, &cam, s);
        let (wrong, p95, aborted) = compare(&truth, &got, cam.pixel());
        assert_eq!(wrong, 0.0, "{src}");
        assert_eq!(aborted, 0.0, "{src}");
        assert!(p95 < 1.0, "{src}: {p95}");
        // 半径 4 の球は、この画面で直径 10 画素ほど
        assert!(got.iter().filter(|g| g.is_some()).count() > 40, "{src}");
    }
}

#[test]
#[ignore]
fn pixel_report() {
    let only = std::env::var("PIXEL_ONLY").unwrap_or_default();
    let cam = Camera::new(160, 120);
    let (s, truth_s) = settings(&cam);
    println!("| 式 | (a) 違う % | (a) 深さ p95 | (a) 打ち切り % | (a) ms | (b) 違う % | (b) 深さ p95 | 正解 ms | 正解の打ち切り % |");
    println!("|---|---|---|---|---|---|---|---|---|");
    let mut worst: (f64, f64) = (0.0, 0.0);
    for src in IMPLICIT.iter().chain(EXTRA.iter()) {
        if !src.contains(only.as_str()) {
            continue;
        }
        let p = field(src);
        let t0 = Instant::now();
        let truth = rays(&p, &cam, truth_s);
        let truth_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = Instant::now();
        let a = rays(&p, &cam, s);
        let a_ms = t1.elapsed().as_secs_f64() * 1000.0;
        let b = mesh_depths(src, &cam);
        let (aw, ap, aa) = compare(&truth, &a, cam.pixel());
        let (bw, bp, _) = compare(&truth, &b, cam.pixel());
        let truth_abort = 100.0 * truth.iter().filter(|t| t.is_some_and(f64::is_nan)).count() as f64 / truth.len() as f64;
        println!("| `{src}` | {aw:.2} | {ap:.2} | {aa:.2} | {a_ms:.0} | {bw:.2} | {bp:.1} | {truth_ms:.0} | {truth_abort:.2} |");
        if IMPLICIT.contains(src) {
            worst = (worst.0.max(aw), worst.1.max(aa));
        }
    }
    println!("陰関数 16 式の最大: (a) 違う {:.2} %、打ち切り {:.2} %（完了の条件: 0.5 % 以下・0.1 % 以下）", worst.0, worst.1);
}

//! 3D の光線の描き方（計画 12 章の G5）の検査（`tools/gpu-g5/check.mjs`）用: 式ごとの GLSL と、
//! G2 の物差し（`core/tests/pixel.rs` の `pixel_report`）と同じカメラ・160 × 120 本の光線の基準の結果（`ray_hit`、上限 512 回・1 画素）を、
//! JSON にして標準出力に書く。
//!
//! ```text
//! cargo run --release --example ray_fixture
//! ```
//!
//! 光線の結果は、当たれば t、当たらなければ null、打ち切りは "A"

use graph_core::eval::{GlslOptions, Program};
use graph_core::parser::{parse_system, BinOp, Expr, Rel};
use graph_core::pixel::{ray_box, ray_hit, RayHit, RaySettings};
use std::fmt::Write;

/// `pixel_report` と同じ表示範囲・視野角
const LO: [f64; 3] = [-10.0, -10.0, -6.4];
const HI: [f64; 3] = [10.0, 10.0, 6.4];
const FOV_DEG: f64 = 40.0;

/// 基準（Rust の 64 ビット）は元の式で、GPU には同じ解の別の式（計算エンジンが光線の描き方のために言い換えた形。
/// `equation::gpu_friendly`）を渡す組。基準の実装が元の式で面を見つけられるときに、言い換えの正確さを画素ごとに確かめる。
/// `mod(x y, z) = 0` → `sin(pi x y / z) = 0` は入れない: 元の式は解が値の飛ぶ所にしか無く、基準の実装でも面を 1 つも見つけられない
/// （当たった画素 0 %。言い換えた式は基準も GPU も 59.7 % で、画素ごとに一致する）。言い換えた式は CASES で確かめる
const REWRITES: &[(&str, &str)] = &[];

/// `pixel_report` の 21 式と、G0 の式（`pixel_report` に無いもの）
const CASES: &[&str] = &[
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
    "(x^2+y^2+z^2-16)^2 = 0",
    "x^2+y^2+z^2 = 0.0001",
    "floor(x) = z",
    "sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x) = 0",
    "x^2+y^2+z^2 = 16",
    // G0 の式
    "x^2+y^2+z^2 = 4",
    "sin(x) = cos(y) z",
    "floor(x y) = z floor(z)",
    "mod(x y, z) = 1",
    // 値がいつも 0 以上で、値が飛ぶ所でちょうど 0 になる（面が無数に重なる）
    "mod(x y, z) = 0",
    // 上の言い換え（基準も GPU もこの式）
    "sin(pi x y / z) = 0",
];

fn norm(v: [f64; 3]) -> [f64; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / l, v[1] / l, v[2] / l]
}

fn field(src: &str) -> Program {
    let chains = parse_system(src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
    let c = &chains[0];
    let e = if c.rels == [Rel::Eq] { Expr::Bin(BinOp::Sub, Box::new(c.exprs[0].clone()), Box::new(c.exprs[1].clone())) } else { c.exprs[0].clone() };
    Program::compile(&e, &["x", "y", "z"]).unwrap()
}

fn main() {
    // 画面の大きさ（既定は pixel_report と同じ 160 × 120。RAY_W・RAY_H で変えられる）
    let env = |k: &str, d: usize| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let (w, h) = (env("RAY_W", 160), env("RAY_H", 120));
    // pixel_report の Camera と同じ
    let (yaw, pitch, dist): (f64, f64, f64) = (-0.9, 0.45, 34.0);
    let eye = [dist * pitch.cos() * yaw.cos(), dist * pitch.cos() * yaw.sin(), dist * pitch.sin()];
    let fwd = norm([-eye[0], -eye[1], -eye[2]]);
    let right = norm([fwd[1], -fwd[0], 0.0]);
    let up = [right[1] * fwd[2] - right[2] * fwd[1], right[2] * fwd[0] - right[0] * fwd[2], right[0] * fwd[1] - right[1] * fwd[0]];
    let tan_half = (FOV_DEG.to_radians() / 2.0).tan();
    let pixel = 2.0 * tan_half / h as f64;
    let dir = |i: usize, j: usize| {
        let aspect = w as f64 / h as f64;
        let x = ((i as f64 + 0.5) / w as f64 * 2.0 - 1.0) * tan_half * aspect;
        let y = (1.0 - (j as f64 + 0.5) / h as f64 * 2.0) * tan_half;
        norm([0, 1, 2].map(|k| fwd[k] + x * right[k] + y * up[k]))
    };
    let s = RaySettings { pixel, max_iter: 512 };
    let v3 = |v: [f64; 3]| format!("[{:e},{:e},{:e}]", v[0], v[1], v[2]);
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"w\":{w},\"h\":{h},\"eye\":{},\"fwd\":{},\"right\":{},\"up\":{},\"tanHalf\":{tan_half:e},\"pixel\":{pixel:e},\"lo\":{},\"hi\":{},\"cases\":[\n",
        v3(eye),
        v3(fwd),
        v3(right),
        v3(up),
        v3(LO),
        v3(HI)
    );
    let all: Vec<(String, &str, &str)> = CASES
        .iter()
        .map(|s| (s.to_string(), *s, *s))
        .chain(REWRITES.iter().map(|(a, b)| (format!("{a}（GPU は {b}）"), *a, *b)))
        .collect();
    for (ci, (label, src, gsrc)) in all.iter().enumerate() {
        let p = field(src);
        let g = field(gsrc).to_glsl(&GlslOptions { name: "gR".into(), ..Default::default() }).unwrap_or_else(|e| panic!("{src}: {e}"));
        let mut rays = String::new();
        for j in 0..h {
            for i in 0..w {
                let d = dir(i, j);
                let r = match ray_box(eye, d, LO, HI) {
                    Some((t0, t1)) => ray_hit(&p, eye, d, t0, t1, s),
                    None => RayHit::Miss,
                };
                if !rays.is_empty() {
                    rays.push(',');
                }
                match r {
                    RayHit::Hit { t, .. } => {
                        let _ = write!(rays, "{t:.6}");
                    }
                    RayHit::Miss => rays.push_str("null"),
                    RayHit::Aborted => rays.push_str("\"A\""),
                }
            }
        }
        let _ = write!(out, "{{\"src\":{label:?},\"glsl\":{:?},\"rays\":[{rays}]}}{}\n", g.source(), if ci + 1 < all.len() { "," } else { "" });
    }
    out.push_str("]}\n");
    print!("{out}");
}

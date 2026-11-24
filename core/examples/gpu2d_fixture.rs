//! 2D の画素ごとの描き方（計画 12 章の G4）の画像の検査（`tools/gpu-g4/check.mjs`）用: 式ごとの GLSL を JSON にして標準出力に書く。
//!
//! ```text
//! cargo run --release --example gpu2d_fixture
//! ```
//!
//! 式は `x, y, z` の陰関数か、カンマで区切った不等式（連立）。等式は 左辺 − 右辺、`≤`・`<` は 左辺 − 右辺 ≤ 0、`≥`・`>` は 右辺 − 左辺 ≤ 0

use graph_core::eval::Program;
use graph_core::gpu2d::{scene, scene_affine};
use graph_core::parser::{parse_system, BinOp, Expr, Rel};

const CASES: &[&str] = &[
    "1/x = y",
    "floor(x) = y",
    "(x^2+y^2-1)^2 = 0",
    "sin(1/x) = y",
    "x^2+y^2 = 4",
    "x^2+y^2 <= 4",
    "x^2+y^2 <= 9, y > x",
    "x^2+y^2 = 9, y >= 0",
    // 空集合（#139）と、対照の直線
    "x > 0, x <= 0",
    "x >= 0, x <= 0",
    "x^2+y^2 < 0",
    "y > x^2, y < x^2",
    "x > 0.3, x <= 0.3",
];

fn sub(a: &Expr, b: &Expr) -> Expr {
    Expr::Bin(BinOp::Sub, Box::new(a.clone()), Box::new(b.clone()))
}

fn main() {
    let mut out = String::from("[\n");
    for (i, src) in CASES.iter().enumerate() {
        let mut eq = None;
        let mut cuts = Vec::new();
        let mut strict = Vec::new();
        for c in parse_system(src).unwrap_or_else(|e| panic!("{src}: {e:?}")) {
            let (a, b) = (&c.exprs[0], &c.exprs[1]);
            let compile = |e: &Expr| Program::compile(e, &["x", "y", "z"]).unwrap();
            match c.rels[0] {
                Rel::Eq => eq = Some(compile(&sub(a, b))),
                Rel::Le | Rel::Lt => {
                    cuts.push(compile(&sub(a, b)));
                    strict.push(c.rels[0] == Rel::Lt);
                }
                Rel::Ge | Rel::Gt => {
                    cuts.push(compile(&sub(b, a)));
                    strict.push(c.rels[0] == Rel::Gt);
                }
            }
        }
        // 式に畳み込んだ形（scene）と、画素の座標から式の座標への 1 次式を uniform で受け取る形（scene_affine。ここでは恒等）の両方
        let s = scene(eq.as_ref(), &cuts, &strict).unwrap_or_else(|e| panic!("{src}: {e}"));
        let t = scene_affine(eq.as_ref(), &cuts, &strict, 3, 0).unwrap_or_else(|e| panic!("{src}: {e}"));
        for (k, (label, g, affine)) in [(src.to_string(), &s, false), (format!("{src}（1 次式）"), &t, true)].into_iter().enumerate() {
            out.push_str(&format!(
                "{{\"src\":{:?},\"glsl\":{:?},\"eq\":{},\"cuts\":{},\"lines\":{},\"affine\":{}}}{}\n",
                label,
                g.glsl,
                g.eq,
                g.cuts,
                g.lines,
                affine,
                if i + 1 < CASES.len() || k == 0 { "," } else { "" }
            ));
        }
    }
    out.push_str("]\n");
    print!("{out}");
}

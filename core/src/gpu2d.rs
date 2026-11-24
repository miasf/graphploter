//! 2D 表示の陰関数の曲線と不等式の領域を、画素ごとに描くための GLSL（計画 `docs/plan-hard-tasks.md` 12 章の G4）。
//!
//! 描く物は「関数」の並びで表す。関数 0 から `eq` 個（0 か 1）が等式 F = 0、その後の `cuts` 個が不等式 G ≤ 0。
//! - 等式があれば曲線 F = 0 を描く（不等式があれば、それを全部満たす所だけ）
//! - 等式が無ければ、不等式を全部満たす所を塗り、`lines` のビットが立っている不等式の境目 G_k = 0 を実線で引く
//!   （厳密な不等式 `<` の点線は、弧の長さが画素ごとには分からないので、今までのメッシュの線のまま）
//!
//! 画面側（`web/src/lib/gpu2dMaterial.ts`）は、ここで作った GLSL の決まった名前の関数を呼ぶ:
//! `float gF(int f, vec2 q)`（値）、`vec4 gFD(int f, vec2 q)`（値と、横・奥の座標についての勾配）、
//! `Div gFI(int f, vec2 lo, vec2 hi)`（矩形の上の飾り付きの区間）。`q` は表示中の (横, 奥) の座標で、縦の座標は 0。
//! 関数の数は `#define G_EQ`・`G_CUTS`・`G_LINES`（境目の線を引く不等式）・`G_STRICT`（厳密な不等式）で渡す。

use crate::eval::{glsl_bundle, Program};
use std::fmt::Write;

/// 画素ごとに描く 2D の図形
#[derive(Debug, Clone, PartialEq)]
pub struct Scene2d {
    /// GLSL（`#version`・`precision` は付けない）
    pub glsl: String,
    /// 等式の数（0 か 1）
    pub eq: usize,
    /// 不等式の数
    pub cuts: usize,
    /// 境目を実線で引く不等式（ビット k が不等式 k）
    pub lines: u32,
    /// [`scene_affine`] で作ったとき: 画素の座標 q = (横, 奥, 0) から式の座標への 1 次式（座標ごとに [A_k0, A_k1, A_k2, b_k]）と、
    /// uniform にしたパラメータの名前・値。[`scene`] で作ったとき（パラメータを畳み込んだ式）は空
    pub affine: Vec<f64>,
    pub params: Vec<String>,
    pub values: Vec<f64>,
}

/// 厳密な不等式（G < 0）の番号のビット（`#define G_STRICT`。境目の上を中身に入れない。#139）
fn strict_mask(strict: &[bool]) -> u32 {
    strict.iter().enumerate().filter(|(_, s)| **s).fold(0u32, |m, (k, _)| m | (1 << k))
}

/// 等式の上限（0 か 1）と不等式の上限。多いと、画素ごとの計算が重い
pub const MAX_CUTS: usize = 8;

/// 2D の図形の GLSL を作る。`eq` は等式 F = 0（無ければ None）、`cuts` は不等式 G ≤ 0、`strict` は不等式ごとに厳密か。
/// 式の変数は表示中の座標（2 つか 3 つ。3 つ目の縦の座標は 0 にする）。GPU に移せない式なら Err
pub fn scene(eq: Option<&Program>, cuts: &[Program], strict: &[bool]) -> Result<Scene2d, String> {
    if eq.is_none() && cuts.is_empty() {
        return Err("描く物がありません".into());
    }
    if cuts.len() > MAX_CUTS {
        return Err(format!("不等式が {MAX_CUTS} 本を超える式は GPU の描き方では使えません"));
    }
    let mut items: Vec<(&Program, String)> = Vec::new();
    if let Some(e) = eq {
        items.push((e, "gE0".into()));
    }
    for (k, c) in cuts.iter().enumerate() {
        items.push((c, format!("gC{k}")));
    }
    for (p, _) in &items {
        if p.vars().len() > 3 {
            return Err("内部エラー: 2D の式の変数が多すぎます".into());
        }
    }
    let refs: Vec<(&Program, &str)> = items.iter().map(|(p, n)| (*p, n.as_str())).collect();
    let (body, lens) = glsl_bundle(&refs, &[])?;
    let lines = if eq.is_none() {
        strict.iter().enumerate().filter(|(_, s)| !**s).fold(0u32, |m, (k, _)| m | (1 << k))
    } else {
        0
    };
    let mut s = format!("#define G_EQ {}\n#define G_CUTS {}\n#define G_LINES {lines}\n#define G_FUNCS {}\n", eq.is_some() as usize, cuts.len(), items.len());
    let _ = writeln!(s, "#define G_STRICT {}", strict_mask(strict));
    s.push_str(&body);
    // 引数の配列を作る（横 q.x、奥 q.y、縦 0）
    let fill = |out: &mut String, n: usize, ty: &str, vals: [&str; 3]| {
        let _ = writeln!(out, "    {ty} p[{n}];");
        for (k, v) in vals.iter().enumerate().take(n) {
            let _ = writeln!(out, "    p[{k}] = {v};");
        }
    };
    let mut val = String::from("float gF(int f, vec2 q) {\n");
    let mut dual = String::from("vec4 gFD(int f, vec2 q) {\n");
    let mut inter = String::from("Div gFI(int f, vec2 lo, vec2 hi) {\n");
    for (k, ((_, name), &n)) in items.iter().zip(&lens).enumerate() {
        let _ = writeln!(val, "  if (f == {k}) {{");
        fill(&mut val, n, "float", ["q.x", "q.y", "0.0"]);
        let _ = writeln!(val, "    return {name}(p);\n  }}");
        let _ = writeln!(dual, "  if (f == {k}) {{");
        fill(&mut dual, n, "vec4", ["vec4(q.x, 1.0, 0.0, 0.0)", "vec4(q.y, 0.0, 1.0, 0.0)", "vec4(0.0)"]);
        let _ = writeln!(dual, "    return {name}d(p);\n  }}");
        let _ = writeln!(inter, "  if (f == {k}) {{");
        fill(&mut inter, n, "vec2", ["vec2(lo.x, hi.x)", "vec2(lo.y, hi.y)", "vec2(0.0)"]);
        let _ = writeln!(inter, "    return {name}i(p);\n  }}");
    }
    val.push_str("  return 0.0;\n}\n");
    dual.push_str("  return vec4(0.0);\n}\n");
    inter.push_str("  return gi_empty();\n}\n");
    s.push_str(&val);
    s.push_str(&dual);
    s.push_str(&inter);
    Ok(Scene2d { glsl: s, eq: eq.is_some() as usize, cuts: cuts.len(), lines, affine: Vec::new(), params: Vec::new(), values: Vec::new() })
}

/// [`scene`] の、回転・断面・パラメータを uniform で渡す版（計画 12 章の G7。3D の光線の描き方と同じ考え方）。
/// 式は元の座標 x, y, z, w, …（`dims` 個）の式で、その後の `n_params` 個の変数がスライダーのパラメータ（`uParam`）。
/// 画素の座標 q = (横, 奥, 0) から元の座標への 1 次式 P_k = uPA[k] · q + uPb[k] も uniform。
/// 回転・断面・スライダーを動かしても GLSL を作り直さない（[`Scene2d`] の `affine`・`values` だけが変わる）
pub fn scene_affine(eq: Option<&Program>, cuts: &[Program], strict: &[bool], dims: usize, n_params: usize) -> Result<Scene2d, String> {
    if eq.is_none() && cuts.is_empty() {
        return Err("描く物がありません".into());
    }
    if cuts.len() > MAX_CUTS {
        return Err(format!("不等式が {MAX_CUTS} 本を超える式は GPU の描き方では使えません"));
    }
    let mut items: Vec<(&Program, String)> = Vec::new();
    if let Some(e) = eq {
        items.push((e, "gE0".into()));
    }
    for (k, c) in cuts.iter().enumerate() {
        items.push((c, format!("gC{k}")));
    }
    for (p, _) in &items {
        if p.vars().len() != dims + n_params {
            return Err("内部エラー: 2D の式の変数の数が合いません".into());
        }
    }
    let idx: Vec<usize> = (dims..dims + n_params).collect();
    let refs: Vec<(&Program, &str)> = items.iter().map(|(p, n)| (*p, n.as_str())).collect();
    let (body, _) = glsl_bundle(&refs, &idx)?;
    let lines = if eq.is_none() { strict.iter().enumerate().filter(|(_, s)| !**s).fold(0u32, |m, (k, _)| m | (1 << k)) } else { 0 };
    let mut s = format!("#define G_EQ {}\n#define G_CUTS {}\n#define G_LINES {lines}\n#define G_FUNCS {}\n", eq.is_some() as usize, cuts.len(), items.len());
    let _ = writeln!(s, "#define G_STRICT {}", strict_mask(strict));
    let _ = writeln!(s, "#define G_N {dims}\nuniform vec3 uPA[G_N];\nuniform float uPb[G_N];");
    if n_params > 0 {
        let _ = writeln!(s, "uniform float uParam[{n_params}];");
    }
    s.push_str(&body);
    let mut val = String::from("float gF(int f, vec2 q) {\n  float p[G_N];\n  for (int k = 0; k < G_N; k++) p[k] = dot(uPA[k], vec3(q, 0.0)) + uPb[k];\n");
    let mut dual = String::from("vec4 gFD(int f, vec2 q) {\n  vec4 p[G_N];\n  for (int k = 0; k < G_N; k++) p[k] = vec4(dot(uPA[k], vec3(q, 0.0)) + uPb[k], uPA[k]);\n");
    // 矩形の上の箱: 1 次式の横・奥の項の範囲を足す
    let mut inter = String::from(
        "Div gFI(int f, vec2 lo, vec2 hi) {\n  vec2 p[G_N];\n  for (int k = 0; k < G_N; k++) {\n    vec2 ax = uPA[k].x * vec2(lo.x, hi.x);\n    vec2 ay = uPA[k].y * vec2(lo.y, hi.y);\n    p[k] = vec2(min(ax.x, ax.y) + min(ay.x, ay.y), max(ax.x, ax.y) + max(ay.x, ay.y)) + uPb[k];\n  }\n",
    );
    for (k, (_, name)) in items.iter().enumerate() {
        let _ = writeln!(val, "  if (f == {k}) return {name}(p);");
        let _ = writeln!(dual, "  if (f == {k}) return {name}d(p);");
        let _ = writeln!(inter, "  if (f == {k}) return {name}i(p);");
    }
    val.push_str("  return 0.0;\n}\n");
    dual.push_str("  return vec4(0.0);\n}\n");
    inter.push_str("  return gi_empty();\n}\n");
    s.push_str(&val);
    s.push_str(&dual);
    s.push_str(&inter);
    Ok(Scene2d { glsl: s, eq: eq.is_some() as usize, cuts: cuts.len(), lines, affine: Vec::new(), params: Vec::new(), values: Vec::new() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn prog(src: &str, vars: &[&str]) -> Program {
        Program::compile(&parse(src).unwrap(), vars).unwrap()
    }

    #[test]
    fn curve_scene() {
        let s = scene(Some(&prog("x^2 + y^2 - 4", &["x", "y", "z"])), &[], &[]).unwrap();
        assert_eq!((s.eq, s.cuts, s.lines), (1, 0, 0));
        assert!(s.glsl.starts_with("#define G_EQ 1\n#define G_CUTS 0\n#define G_LINES 0\n#define G_FUNCS 1\n"), "{}", s.glsl);
        assert!(s.glsl.contains("float gE0(float p[3])"));
        assert!(s.glsl.contains("    p[2] = 0.0;\n    return gE0(p);"), "{}", s.glsl);
        assert!(s.glsl.contains("Div gFI(int f, vec2 lo, vec2 hi)"));
    }

    #[test]
    fn region_scene_lines_only_for_non_strict() {
        let cuts = [prog("x^2 + y^2 - 4", &["x", "y", "z"]), prog("y - x", &["x", "y", "z"]), prog("x - 3", &["x", "y", "z"])];
        let s = scene(None, &cuts, &[false, true, false]).unwrap();
        assert_eq!((s.eq, s.cuts, s.lines), (0, 3, 0b101));
        assert!(s.glsl.contains("#define G_LINES 5\n"));
        assert!(s.glsl.contains("if (f == 2) {"));
        // 2 つの変数の式（z = f(x, y) の切り口）は、配列の長さ 2
        let s = scene(Some(&prog("sin(x) - y", &["x", "y"])), &[], &[]).unwrap();
        assert!(s.glsl.contains("float p[2];") && !s.glsl.contains("p[2] ="), "{}", s.glsl);
    }

    #[test]
    fn unsupported_is_error() {
        assert!(scene(Some(&prog("gamma(x) - y", &["x", "y", "z"])), &[], &[]).is_err());
        assert!(scene(None, &[], &[]).is_err());
    }
}

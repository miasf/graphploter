//! グラフソフトの計算エンジン。
//!
//! - `parser`    : 数式の文字列 → 構文木
//! - `eval`      : 構文木 → 命令列、多数の点でまとめて計算（複素数）
//! - `functions` : 組み込み関数の一覧
//! - `special`   : 特殊関数の実装
//! - `equation`  : 式の種類（陽関数・陰関数）の判定
//! - `mesh`      : 曲面のメッシュ生成
//!
//! このファイルは、JavaScript から呼び出す関数（WASM の入口）だけを置く。

pub mod equation;
pub mod eval;
pub mod functions;
pub mod mesh;
pub mod parser;
pub mod special;

use wasm_bindgen::prelude::*;

/// JavaScript に返すメッシュ
#[wasm_bindgen]
pub struct MeshResult {
    inner: mesh::Mesh,
    kind: String,
    params: &'static str,
    curve: bool,
}

#[wasm_bindgen]
impl MeshResult {
    /// 頂点座標（Float32Array）
    #[wasm_bindgen(getter)]
    pub fn positions(&self) -> Vec<f32> {
        self.inner.positions.clone()
    }
    /// 頂点の法線（Float32Array）
    #[wasm_bindgen(getter)]
    pub fn normals(&self) -> Vec<f32> {
        self.inner.normals.clone()
    }
    /// グラフの種類の説明（例: "z = f(x, y)", "陰関数"）
    #[wasm_bindgen(getter)]
    pub fn kind(&self) -> String {
        self.kind.clone()
    }
    /// 媒介変数の名前: "t"（曲線）, "uv"（曲面）, ""（媒介変数表示でない）
    #[wasm_bindgen(getter)]
    pub fn params(&self) -> String {
        self.params.into()
    }
    /// 曲線なら true。このとき positions は線の点の列（途切れる所に NaN）で、indices は空。
    #[wasm_bindgen(getter)]
    pub fn curve(&self) -> bool {
        self.curve
    }
    /// 格子線用の座標（Float32Array、空なら頂点座標を使う）
    #[wasm_bindgen(getter)]
    pub fn grid_coords(&self) -> Vec<f32> {
        self.inner.grid_coords.clone()
    }
    /// 三角形の頂点番号（Uint32Array）
    #[wasm_bindgen(getter)]
    pub fn indices(&self) -> Vec<u32> {
        self.inner.indices.clone()
    }
}

/// 式に誤りがないかだけを調べる。誤りがあればその説明を投げる。
#[wasm_bindgen]
pub fn check_expression(source: &str) -> Result<(), JsError> {
    equation::compile(source).map(|_| ()).map_err(|e| JsError::new(&e))
}

/// 変数を含まない式の値を計算する（媒介変数の範囲の入力用）。例: "2pi" → 6.283...
#[wasm_bindgen]
pub fn eval_constant(source: &str) -> Result<f64, JsError> {
    equation::eval_constant(source).map_err(|e| JsError::new(&e))
}

/// 式を解析して、曲面（または曲線）を作る。
///
/// - `domain`: 表示範囲 [xmin, xmax, ymin, ymax, zmin, zmax]
/// - `resolution`: 陽関数・媒介変数表示の曲面の格子の細かさ（1辺のマス数）
/// - `implicit_resolution`: 陰関数の格子の細かさ（立方体の1辺のマス数。計算量はこの3乗）
/// - `params`: 媒介変数の範囲 [t0, t1, u0, u1, v0, v1]
#[wasm_bindgen]
pub fn plot(
    source: &str,
    domain: &[f64],
    resolution: u32,
    implicit_resolution: u32,
    params: &[f64],
) -> Result<MeshResult, JsError> {
    if domain.len() != 6 || params.len() != 6 {
        return Err(JsError::new("内部エラー: 範囲の数が違います"));
    }
    let plot = equation::compile(source).map_err(|e| JsError::new(&e))?;
    let dom = mesh::Domain { lo: [domain[0], domain[2], domain[4]], hi: [domain[1], domain[3], domain[5]] };
    let size = (0..3).map(|a| (dom.hi[a] - dom.lo[a]).powi(2)).sum::<f64>().sqrt();
    let inner = match &plot {
        equation::Plot::Explicit { axis, prog } => {
            mesh::explicit_surface(prog, dom, resolution.clamp(2, 1000) as usize, *axis)
        }
        equation::Plot::Implicit { prog } => {
            mesh::implicit_surface(prog, dom, implicit_resolution.clamp(2, 300) as usize)
        }
        equation::Plot::Surface { progs } => mesh::parametric_surface(
            progs,
            dom,
            (params[2], params[3]),
            (params[4], params[5]),
            resolution.clamp(2, 1000) as usize,
        ),
        equation::Plot::Curve { progs } => mesh::Mesh {
            positions: mesh::parametric_curve(progs, (params[0], params[1]), 400, size * 2e-4),
            ..Default::default()
        },
    };
    Ok(MeshResult {
        inner,
        kind: plot.kind_label(),
        params: plot.params(),
        curve: matches!(plot, equation::Plot::Curve { .. }),
    })
}

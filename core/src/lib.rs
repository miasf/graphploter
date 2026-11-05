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

/// 式を解析して、曲面のメッシュを作る。
///
/// - `resolution`: 陽関数の格子の細かさ（1辺のマス数）
/// - `implicit_resolution`: 陰関数の格子の細かさ（立方体の1辺のマス数。計算量はこの3乗）
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn plot(
    source: &str,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    zmin: f64,
    zmax: f64,
    resolution: u32,
    implicit_resolution: u32,
) -> Result<MeshResult, JsError> {
    let plot = equation::compile(source).map_err(|e| JsError::new(&e))?;
    let dom = mesh::Domain { lo: [xmin, ymin, zmin], hi: [xmax, ymax, zmax] };
    let inner = match &plot {
        equation::Plot::Explicit { axis, prog } => {
            mesh::explicit_surface(prog, dom, resolution.clamp(2, 1000) as usize, *axis)
        }
        equation::Plot::Implicit { prog } => {
            mesh::implicit_surface(prog, dom, implicit_resolution.clamp(2, 300) as usize)
        }
    };
    Ok(MeshResult { inner, kind: plot.kind_label() })
}

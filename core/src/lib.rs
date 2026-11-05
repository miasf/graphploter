//! グラフソフトの計算エンジン。
//!
//! - `parser`    : 数式の文字列 → 構文木
//! - `eval`      : 構文木 → 命令列、多数の点でまとめて計算（複素数）
//! - `functions` : 組み込み関数の一覧
//! - `special`   : 特殊関数の実装
//! - `mesh`      : 曲面のメッシュ生成
//!
//! このファイルは、JavaScript から呼び出す関数（WASM の入口）だけを置く。

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
    /// 三角形の頂点番号（Uint32Array）
    #[wasm_bindgen(getter)]
    pub fn indices(&self) -> Vec<u32> {
        self.inner.indices.clone()
    }
}

/// 入力の左辺 `z =` を取り除き、右辺だけを返す。
/// 現時点では z = f(x, y) の形（または右辺だけ）に対応。
fn explicit_rhs(source: &str) -> Result<&str, String> {
    match source.split_once('=') {
        None => Ok(source),
        Some((lhs, rhs)) => {
            if lhs.trim() == "z" && !rhs.contains('=') {
                Ok(rhs)
            } else {
                Err("今のところ「z = (x と y の式)」の形だけに対応しています".into())
            }
        }
    }
}

fn compile_explicit(source: &str) -> Result<eval::Program, String> {
    let rhs = explicit_rhs(source)?;
    let expr = parser::parse(rhs).map_err(|e| e.to_string())?;
    eval::Program::compile(&expr, &["x", "y"])
}

/// 式に誤りがないかだけを調べる。誤りがあればその説明を投げる。
#[wasm_bindgen]
pub fn check_expression(source: &str) -> Result<(), JsError> {
    compile_explicit(source).map(|_| ()).map_err(|e| JsError::new(&e))
}

/// 陽関数 z = f(x, y) の曲面を作る。
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn plot_explicit(
    source: &str,
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    zmin: f64,
    zmax: f64,
    resolution: u32,
) -> Result<MeshResult, JsError> {
    let prog = compile_explicit(source).map_err(|e| JsError::new(&e))?;
    let dom = mesh::Domain { x: (xmin, xmax), y: (ymin, ymax), z: (zmin, zmax) };
    Ok(MeshResult {
        inner: mesh::explicit_surface(&prog, dom, resolution.clamp(2, 1000) as usize),
    })
}

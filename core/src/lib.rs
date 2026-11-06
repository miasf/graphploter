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
    params: Vec<String>,
    curve: bool,
    used_params: Vec<String>,
    /// パラメータの定義なら (名前, 値)
    param: Option<(String, num_complex::Complex64)>,
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
    /// 媒介変数の名前（カンマ区切り）。例: "t"（曲線）, "u,v"（曲面）, ""（媒介変数表示でない）
    #[wasm_bindgen(getter)]
    pub fn params(&self) -> String {
        self.params.join(",")
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
    /// この式で使われたパラメータの名前（カンマ区切り）
    #[wasm_bindgen(getter)]
    pub fn used_params(&self) -> String {
        self.used_params.join(",")
    }
    /// パラメータの定義なら、その名前（そうでなければ空）
    #[wasm_bindgen(getter)]
    pub fn param_name(&self) -> String {
        self.param.as_ref().map(|p| p.0.clone()).unwrap_or_default()
    }
    /// パラメータの値 [実部, 虚部]
    #[wasm_bindgen(getter)]
    pub fn param_value(&self) -> Vec<f64> {
        self.param.as_ref().map(|p| vec![p.1.re, p.1.im]).unwrap_or_default()
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
/// - `domain`: 表示範囲 [横の最小, 横の最大, 奥の最小, 奥の最大, 縦の最小, 縦の最大]
/// - `resolution`: 陽関数・媒介変数表示の曲面の格子の細かさ（1辺のマス数）
/// - `implicit_resolution`: 陰関数の格子の細かさ（立方体の1辺のマス数。計算量はこの3乗）
/// - `range_names`, `ranges`: 媒介変数の範囲。例: "t,u,v" と [t0, t1, u0, u1, v0, v1]。
///   無い媒介変数は 0〜2π
/// - `param_names`, `param_values`: パラメータ（スライダー）。例: "a,b" と [a の実部, a の虚部, b の実部, …]
/// - `dims`: 次元、`axes`: 画面の横・奥・縦に表示する座標の番号、`slice`: 各座標の断面の値
/// - `coord_ranges`: 各座標の「範囲で重ねる」範囲 [x の最小, x の最大, y の最小, …]（重ねない座標は NaN）
/// - `stack_names`, `stack_ranges`: 範囲で重ねるパラメータの名前（カンマ区切り）と [最小, 最大, …]
/// - `samples`: 範囲を何等分して重ねるか
/// - `rotation`: 回転角（ラジアン、座標の組 (0,1), (0,2), …, (1,2), … の順）、`center`: 回転の中心
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn plot(
    source: &str,
    domain: &[f64],
    resolution: u32,
    implicit_resolution: u32,
    range_names: &str,
    ranges: &[f64],
    param_names: &str,
    param_values: &[f64],
    dims: u32,
    axes: &[u32],
    slice: &[f64],
    coord_ranges: &[f64],
    stack_names: &str,
    stack_ranges: &[f64],
    samples: u32,
    rotation: &[f64],
    center: &[f64],
) -> Result<MeshResult, JsError> {
    let err = |m: &str| JsError::new(m);
    let n = dims as usize;
    if domain.len() != 6
        || axes.len() != 3
        || dims < 3
        || slice.len() != n
        || coord_ranges.len() != 2 * n
        || rotation.len() != equation::rot_dims(n) * (equation::rot_dims(n) - 1) / 2
        || center.len() != equation::rot_dims(n)
    {
        return Err(err("内部エラー: 範囲や次元の指定が合っていません"));
    }
    let split = |s: &str| -> Vec<String> { s.split(',').filter(|s| !s.is_empty()).map(String::from).collect() };
    let pairs = |names: Vec<String>, vals: &[f64]| -> Result<Vec<(String, f64, f64)>, JsError> {
        if vals.len() != names.len() * 2 {
            return Err(err("内部エラー: 名前と値の数が合っていません"));
        }
        Ok(names.into_iter().enumerate().map(|(k, n)| (n, vals[2 * k], vals[2 * k + 1])).collect())
    };
    let range_of = pairs(split(range_names), ranges)?;
    let consts: eval::Params = pairs(split(param_names), param_values)?
        .into_iter()
        .map(|(n, re, im)| (n, num_complex::Complex64::new(re, im)))
        .collect();
    let space = equation::Space {
        dims: n,
        axes: [axes[0] as usize, axes[1] as usize, axes[2] as usize],
        slice: slice.to_vec(),
        ranges: coord_ranges
            .chunks_exact(2)
            .map(|r| (r[0].is_finite() && r[1].is_finite()).then_some((r[0], r[1])))
            .collect(),
        param_ranges: pairs(split(stack_names), stack_ranges)?,
        samples: samples.clamp(2, 200) as usize,
        rotation: rotation.to_vec(),
        center: center.to_vec(),
    };
    if space.axes.iter().any(|&a| a >= space.dims) {
        return Err(err("内部エラー: 表示する座標の番号が次元を超えています"));
    }

    let plot = equation::compile_with(source, &consts, &space).map_err(|e| JsError::new(&e))?;
    let dom = mesh::Domain { lo: [domain[0], domain[2], domain[4]], hi: [domain[1], domain[3], domain[5]] };
    let size = (0..3).map(|a| (dom.hi[a] - dom.lo[a]).powi(2)).sum::<f64>().sqrt();
    // 媒介変数の範囲（範囲で重ねるパラメータならその範囲。指定が無ければ 0〜2π）
    let range = |name: &str| {
        range_of
            .iter()
            .chain(space.param_ranges.iter())
            .find(|(n, _, _)| n == name)
            .map(|(_, a, b)| (*a, *b))
            .unwrap_or((0.0, std::f64::consts::TAU))
    };
    let inner = match &plot {
        equation::Plot::Explicit { axis, prog, .. } => {
            mesh::explicit_surface(prog, dom, resolution.clamp(2, 1000) as usize, *axis)
        }
        equation::Plot::Implicit { prog } => {
            mesh::implicit_surface(prog, dom, implicit_resolution.clamp(2, 300) as usize)
        }
        equation::Plot::Solid { prog, ranges, .. } => mesh::implicit_solid(
            prog,
            dom,
            implicit_resolution.clamp(2, 300) as usize,
            ranges,
            space.samples,
        ),
        equation::Plot::Surface { progs, vars } => mesh::parametric_surface(
            progs,
            dom,
            range(&vars[0]),
            range(&vars[1]),
            resolution.clamp(2, 1000) as usize,
        ),
        equation::Plot::Curve { progs, vars } => mesh::Mesh {
            positions: mesh::parametric_curve(progs, range(&vars[0]), 400, size * 2e-4),
            ..Default::default()
        },
        equation::Plot::Param { .. } => mesh::Mesh::default(),
    };
    Ok(MeshResult {
        inner,
        kind: plot.kind_label(),
        params: plot.param_vars(),
        curve: matches!(plot, equation::Plot::Curve { .. }),
        used_params: plot.used_params(),
        param: match &plot {
            equation::Plot::Param { name, value, .. } => Some((name.clone(), *value)),
            _ => None,
        },
    })
}

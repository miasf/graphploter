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
    /// 媒介変数表示なら、回転・射影する前の n 次元の点（回転だけ変わったときに使い回す）
    shape: Option<mesh::NdShape>,
    /// 回転した陽関数なら、式が使っている名前（回転が条件を満たすか確かめるため）
    explicit_used: Option<Vec<String>>,
    /// 陰関数・範囲で重ねた立体なら、その中身（切り口のふたを作るため）
    field: Option<(mesh::Field, mesh::Domain)>,
    /// 媒介変数表示の立体（閉じた面）なら true。切り口のふたは描画側で作る
    solid: bool,
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
    /// 媒介変数表示なら、回転・射影する前の形を取り出す（取り出せるのは1回だけ）。
    /// 回転・表示する軸・表示範囲だけが変わったときは、これの `project` で描き直せる
    pub fn take_shape(&mut self) -> Option<Shape> {
        let inner = self.shape.take()?;
        Some(Shape {
            inner,
            explicit_used: self.explicit_used.clone(),
            kind: self.kind.clone(),
            params: self.params.clone(),
            curve: self.curve,
            used_params: self.used_params.clone(),
        })
    }
    /// 媒介変数表示の立体（閉じた面で、外向きにそろっている）なら true
    #[wasm_bindgen(getter)]
    pub fn solid(&self) -> bool {
        self.solid
    }
    /// 陰関数・範囲で重ねた立体なら、切り口のふたを作るための中身を取り出す（取り出せるのは1回だけ）
    pub fn take_field(&mut self) -> Option<CapSource> {
        self.field.take().map(|(inner, dom)| CapSource { inner, dom })
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

/// パラメータを使ってよい式の値（媒介変数の範囲 0 ≤ t ≤ b など）。
/// 戻り値は [値]。使ったパラメータの名前は `last_used_params` で取り出す
#[wasm_bindgen]
pub fn eval_range(source: &str, param_names: &str, param_values: &[f64]) -> Result<f64, JsError> {
    let names: Vec<&str> = param_names.split(',').filter(|s| !s.is_empty()).collect();
    if param_values.len() != names.len() * 2 {
        return Err(JsError::new("内部エラー: パラメータの数が違います"));
    }
    let consts: eval::Params = names
        .iter()
        .enumerate()
        .map(|(k, n)| (n.to_string(), num_complex::Complex64::new(param_values[2 * k], param_values[2 * k + 1])))
        .collect();
    let (v, used) = equation::eval_constant_with(source, &consts).map_err(|e| JsError::new(&e))?;
    LAST_USED.with(|u| *u.borrow_mut() = used.join(","));
    Ok(v)
}

thread_local! {
    static LAST_USED: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// 直前の `eval_range` で使ったパラメータの名前（カンマ区切り）
#[wasm_bindgen]
pub fn last_used_params() -> String {
    LAST_USED.with(|u| u.borrow().clone())
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
/// - `perspective`: 4次元の遠近法の視点までの距離（0 なら遠近法なし）。射影で描く図形（媒介変数表示・回した陽関数）だけに効く
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
    perspective: f64,
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
    // 媒介変数の範囲（範囲で重ねるパラメータならスライダーの範囲。指定が無ければ 0〜2π）
    let range = |name: &str| {
        space
            .param_ranges
            .iter()
            .chain(range_of.iter())
            .find(|(n, _, _)| n == name)
            .map(|(_, a, b)| (*a, *b))
            .unwrap_or((0.0, std::f64::consts::TAU))
    };
    let field = match &plot {
        equation::Plot::Implicit { prog } => Some((mesh::Field::new(prog, &[], 0, dom), dom)),
        equation::Plot::Solid { prog, ranges, .. } => Some((mesh::Field::new(prog, ranges, space.samples, dom), dom)),
        _ => None,
    };
    let inner = match &plot {
        equation::Plot::Explicit { rotated: true, .. } => mesh::Mesh::default(), // 下で作る
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
        equation::Plot::Surface { .. } | equation::Plot::Curve { .. } | equation::Plot::Volume { .. } => {
            mesh::Mesh::default() // 下で作る
        }
        equation::Plot::Param { .. } => mesh::Mesh::default(),
    };
    // 媒介変数表示: n 次元の点を計算してから、回転・射影する
    let shape = match &plot {
        equation::Plot::Surface { sets, vars } => Some(mesh::parametric_surface_nd(
            sets,
            range(&vars[0]),
            range(&vars[1]),
            resolution.clamp(2, 1000) as usize,
        )),
        // 立体は面が6枚になるので、1枚あたりを少し粗くする
        equation::Plot::Volume { sets, vars } => Some(mesh::parametric_solid_nd(
            sets,
            [range(&vars[0]), range(&vars[1]), range(&vars[2])],
            (resolution * 2 / 3).clamp(2, 600) as usize,
        )),
        equation::Plot::Curve { sets, vars } => {
            Some(mesh::parametric_curve_nd(sets, range(&vars[0]), 400, size * 2e-4))
        }
        // 回転した陽関数: 回転の中心から表示範囲の角までの距離 ρ の範囲で計算しておく。
        // こうすると、どの角度に回しても表示範囲が欠けない（角度が変わっても計算し直さなくてよい）
        equation::Plot::Explicit { axis, prog, rotated: true, .. } => {
            let c: Vec<f64> = space.axes.iter().map(|&a| space.center.get(a).copied().unwrap_or(0.0)).collect();
            let rho = (0..3)
                .map(|a| (dom.lo[a] - c[a]).abs().max((dom.hi[a] - c[a]).abs()).powi(2))
                .sum::<f64>()
                .sqrt();
            let (ua, va) = equation::others(*axis);
            Some(mesh::explicit_nd(
                prog,
                *axis,
                space.axes,
                &space.slice,
                (c[ua] - rho, c[ua] + rho),
                (c[va] - rho, c[va] + rho),
                resolution.clamp(2, 1000) as usize,
            ))
        }
        _ => None,
    };
    // 回転した陽関数は、表示する軸・断面・中心が変わると形が変わる。回転がその条件を満たすかも確かめる
    let explicit_used = match &plot {
        equation::Plot::Explicit { rotated: true, .. } => Some(plot.used_params()),
        _ => None,
    };
    let inner = match &shape {
        Some(sh) => mesh::project(sh, &space.view(perspective), dom),
        None => inner,
    };
    Ok(MeshResult {
        inner,
        solid: shape.as_ref().is_some_and(|s| s.solid),
        shape,
        field,
        explicit_used,
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

/// 媒介変数表示の、回転・射影する前の形（n 次元の点）。Worker が式ごとに持っておく
#[wasm_bindgen]
pub struct Shape {
    inner: mesh::NdShape,
    explicit_used: Option<Vec<String>>,
    kind: String,
    params: Vec<String>,
    curve: bool,
    used_params: Vec<String>,
}

#[wasm_bindgen]
impl Shape {
    /// 回転した陽関数から作った形か。そうなら、表示する軸・断面の値・回転の中心が変わると使い回せない
    #[wasm_bindgen(getter)]
    pub fn view_dependent(&self) -> bool {
        self.explicit_used.is_some()
    }

    /// 回転・表示する軸・表示範囲を変えて、もう一度3次元に射影する（式は計算し直さない）。
    /// 引数の意味は [`plot`] と同じ。
    /// 回転した陽関数で、新しい回転が「表示中の座標どうし」でなくなったときは
    /// "RECOMPUTE" というエラーを返す（陰関数として計算し直す必要がある）
    pub fn project(
        &self,
        domain: &[f64],
        dims: u32,
        axes: &[u32],
        rotation: &[f64],
        center: &[f64],
        perspective: f64,
    ) -> Result<MeshResult, JsError> {
        let n = dims as usize;
        let rn = equation::rot_dims(n);
        if domain.len() != 6 || axes.len() != 3 || rotation.len() != rn * (rn - 1) / 2 || n != self.inner.dims {
            return Err(JsError::new("内部エラー: 射影の指定が合っていません"));
        }
        let mut space = equation::Space::basic(n, [axes[0] as usize, axes[1] as usize, axes[2] as usize], vec![0.0; n]);
        space.rotation = rotation.to_vec();
        space.center = center.to_vec();
        if let Some(used) = &self.explicit_used {
            if space.rotation.iter().all(|&a| a == 0.0) || !space.rotation_keeps_shown(used) {
                return Err(JsError::new("RECOMPUTE"));
            }
        }
        let dom = mesh::Domain { lo: [domain[0], domain[2], domain[4]], hi: [domain[1], domain[3], domain[5]] };
        Ok(MeshResult {
            inner: mesh::project(&self.inner, &space.view(perspective), dom),
            solid: self.inner.solid,
            field: None,
            shape: None,
            explicit_used: None,
            kind: self.kind.clone(),
            params: self.params.clone(),
            curve: self.curve,
            used_params: self.used_params.clone(),
            param: None,
        })
    }
}

/// 陰関数・範囲で重ねた立体の中身。切り口の位置だけが変わったときは、これで切り口のふただけを作り直す
#[wasm_bindgen]
pub struct CapSource {
    inner: mesh::Field,
    dom: mesh::Domain,
}

#[wasm_bindgen]
impl CapSource {
    /// 平面「画面の `axis` 番目（0: 横, 1: 奥, 2: 縦）の座標 = `pos`」での切り口のふた。
    /// `keep_below` なら座標が `pos` 以下の側を残す（ふたは反対側を向く）。`n` は格子の1辺のマス数
    pub fn cap(&self, axis: u32, pos: f64, keep_below: bool, n: u32) -> Result<MeshResult, JsError> {
        if axis > 2 {
            return Err(JsError::new("内部エラー: 切り口の軸の番号が違います"));
        }
        Ok(MeshResult {
            inner: self.inner.cap(self.dom, axis as usize, pos, keep_below, n.clamp(2, 1000) as usize),
            kind: String::new(),
            params: Vec::new(),
            curve: false,
            used_params: Vec::new(),
            param: None,
            shape: None,
            explicit_used: None,
            field: None,
            solid: false,
        })
    }
}

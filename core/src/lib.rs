//! グラフソフトの計算エンジン。
//!
//! - `parser`    : 数式の文字列 → 構文木
//! - `eval`      : 構文木 → 命令列、多数の点でまとめて計算（複素数）
//! - `interval`  : 区間演算（箱の上で式の値が入る範囲。曲面の無い所を飛ばすのに使う）
//! - `functions` : 組み込み関数の一覧
//! - `special`   : 特殊関数の実装
//! - `equation`  : 式の種類（陽関数・陰関数）の判定
//! - `mesh`      : 曲面のメッシュ生成
//!
//! このファイルは、JavaScript から呼び出す関数（WASM の入口）だけを置く。

pub mod adaptive;
pub mod equation;
pub mod eval;
pub mod functions;
pub mod interval;
pub mod mesh;
pub mod parser;
pub mod special;
pub mod timing;

use wasm_bindgen::prelude::*;

/// JavaScript に返すメッシュ
#[wasm_bindgen]
pub struct MeshResult {
    inner: mesh::Mesh,
    kind: String,
    params: Vec<String>,
    curve: bool,
    /// 射影する前の形が曲線か（2D の切り口では、曲面も射影した後で線になるので `curve` は true になる。
    /// 射影し直すときは、元の種類で切り口の線にするか決める）
    shape_curve: bool,
    used_params: Vec<String>,
    /// パラメータの定義なら (名前, 値)
    param: Option<(String, num_complex::Complex64)>,
    /// 媒介変数表示なら、回転・射影する前の n 次元の点（回転だけ変わったときに使い回す）
    shape: Option<mesh::NdShape>,
    /// 回転した陽関数なら、式が使っている名前（回転が条件を満たすか確かめるため）
    explicit_used: Option<Vec<String>>,
    /// 陰関数・範囲で重ねた立体なら、その中身（切り口のふたを作るため。リストなら複数）
    field: Option<(Vec<mesh::Field>, mesh::Domain)>,
    /// 媒介変数表示の立体（閉じた面）なら true。切り口のふたは描画側で作る
    solid: bool,
    /// 2D 表示で作ったもの（射影し直すときも平らにする）
    flat: bool,
    /// 2D 表示の切り口（射影し直すときも、平面との交わりの線にする）
    flat_slice: bool,
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
            flat: self.flat,
            flat_slice: self.flat_slice,
            explicit_used: self.explicit_used.clone(),
            kind: self.kind.clone(),
            params: self.params.clone(),
            curve: self.shape_curve,
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
    /// 複素数の色分け: 頂点ごとの (Re w / |w|, Im w / |w|, log₂|w|)（Float32Array、空なら単色）
    #[wasm_bindgen(getter)]
    pub fn complex_values(&self) -> Vec<f32> {
        self.inner.complex.clone()
    }
    /// 三角形の頂点番号（Uint32Array）
    #[wasm_bindgen(getter)]
    pub fn indices(&self) -> Vec<u32> {
        self.inner.indices.clone()
    }
    /// 粗い版の三角形の頂点番号（Uint32Array）。視点を動かしている間に描く。空なら、そのまま描く
    #[wasm_bindgen(getter)]
    pub fn lod_indices(&self) -> Vec<u32> {
        self.inner.lod.clone()
    }
    // ---- 配列を取り出す（取り出せるのは1回だけ。取り出した後の getter は空を返す）----
    // getter は Rust 側で clone() してから JavaScript の配列に写すのでコピーが2回になる。
    // こちらは Rust 側の配列をそのまま渡すので、コピーは JavaScript の配列に写す1回だけ

    /// 自分の頂点を持つ粗い版（陰関数）があるか
    #[wasm_bindgen(getter)]
    pub fn has_lod_mesh(&self) -> bool {
        self.inner.lod_mesh.is_some()
    }
    /// 自分の頂点を持つ粗い版の頂点座標を取り出す（無ければ空）
    pub fn take_lod_positions(&mut self) -> Vec<f32> {
        self.inner.lod_mesh.as_mut().map(|m| std::mem::take(&mut m.positions)).unwrap_or_default()
    }
    /// 自分の頂点を持つ粗い版の法線を取り出す（無ければ空）
    pub fn take_lod_normals(&mut self) -> Vec<f32> {
        self.inner.lod_mesh.as_mut().map(|m| std::mem::take(&mut m.normals)).unwrap_or_default()
    }
    /// 自分の頂点を持つ粗い版の三角形を取り出す（無ければ空）
    pub fn take_lod_mesh_indices(&mut self) -> Vec<u32> {
        self.inner.lod_mesh.as_mut().map(|m| std::mem::take(&mut m.indices)).unwrap_or_default()
    }

    /// 頂点座標を取り出す（Float32Array）
    pub fn take_positions(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.inner.positions)
    }
    /// 頂点の法線を取り出す（Float32Array）
    pub fn take_normals(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.inner.normals)
    }
    /// 三角形の頂点番号を取り出す（Uint32Array）
    pub fn take_indices(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.inner.indices)
    }
    /// 粗い版の三角形の頂点番号を取り出す（Uint32Array）
    pub fn take_lod_indices(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.inner.lod)
    }
    /// 格子線用の座標を取り出す（Float32Array）
    pub fn take_grid_coords(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.inner.grid_coords)
    }
    /// 複素数の色分けの値を取り出す（Float32Array）
    pub fn take_complex_values(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.inner.complex)
    }
    /// かかった時間の内訳 [式の計算 ms, メッシュ ms, 粗い版 ms, 計算した点の数,
    /// 陰関数なら部分ごとの時間 × 4]（`mesh::Stats` を参照）
    #[wasm_bindgen(getter)]
    pub fn stats(&self) -> Vec<f64> {
        let s = self.inner.stats;
        let mut v = vec![s.eval_ms, s.mesh_ms, s.lod_ms, s.points];
        v.extend_from_slice(&s.phases);
        v
    }
}

/// `start` から計った時間の内訳を `mesh.stats` に入れる。`lod_start` 〜 `end` が粗い版を作った時間。
/// 式の計算の時間は、`start` の時点で `timing::reset` してから集めたもの
fn record_stats(mesh: &mut mesh::Mesh, start: f64, lod_start: f64, end: f64) {
    let (eval_ms, points) = timing::take();
    let phases = mesh.stats.phases;
    mesh.stats = mesh::Stats { eval_ms, mesh_ms: (lod_start - start - eval_ms).max(0.0), lod_ms: end - lod_start, points, phases };
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

/// [`plot`] の `flags` のビット 0: 形が急に変わる所を細かくしない
const FLAG_NO_REFINE: u32 = 1;
/// [`plot`] の `flags` のビット 2: 媒介変数が 3 つ以上の図形を、辺だけで描く
const FLAG_EDGES: u32 = 4;

/// 式を解析して、曲面（または曲線）を作る。
///
/// - `domain`: 表示範囲 [横の最小, 横の最大, 奥の最小, 奥の最大, 縦の最小, 縦の最大]
/// - `resolution`: 陽関数・媒介変数表示の曲面の格子の細かさ（1辺のマス数）
/// - `implicit_resolution`: 陰関数の格子の細かさ（立方体の1辺のマス数。計算量はこの3乗）
/// - `range_names`, `ranges`: 媒介変数の範囲。例: "t,u,v" と [t0, t1, u0, u1, v0, v1]。
///   無い媒介変数は 0〜2π
/// - `param_names`, `param_values`: パラメータ（スライダー）。例: "a,b" と [a の実部, a の虚部, b の実部, …]
/// - `complex_params`: 複素数のスライダーにしたパラメータの名前（カンマ区切り）。これを使った定数の式は複素平面の点になる
/// - `dims`: 次元、`axes`: 画面の横・奥・縦に表示する座標の番号、`slice`: 各座標の断面の値
/// - `coord_ranges`: 各座標の「範囲で重ねる」範囲 [x の最小, x の最大, y の最小, …]（重ねない座標は NaN）
/// - `stack_names`, `stack_ranges`: 範囲で重ねるパラメータの名前（カンマ区切り）と [最小, 最大, …]
/// - `samples`: 範囲を何等分して重ねるか
/// - `rotation`: 回転角（ラジアン、座標の組 (0,1), (0,2), …, (1,2), … の順）、`center`: 回転の中心
/// - `perspective`: 4次元の遠近法の視点までの距離（0 なら遠近法なし）。射影で描く図形（媒介変数表示・回した陽関数）だけに効く
/// - `complex_mode`: 陽関数の複素数の色分け（0: しない, 1: 高さ |f|, 2: 高さ Re f, 3: 高さ Im f, 4: 高さ arg f。色は偏角）。
///   複素平面の図形では縦の軸に描くもの（0: 0, 1: |f|, 2: Re f, 3: Im f, 4: arg f, 5: 媒介変数）
/// - `walls`: 陽関数の値が飛ぶ所（floor の段差など）に縦の壁を描くか
/// - `flat`: 2D 表示。縦の軸の座標を 0 にして、横・奥の平面に描く。
///   縦の座標 = f(横, 奥) は平面の塗り分け、陰関数・ほかの陽関数は曲線、範囲で重ねた立体は塗った領域になる
/// - `flat_slice`: 2D 表示の描き方。true なら「切り口」（縦の座標 = 0 の平面との交わり）、
///   false なら「投影」（3次元の図形を真上から平面に押しつぶした形）
/// - `layers`: 媒介変数が4つ以上の図形で、固定する媒介変数の範囲を何等分した所に面を描くか（1 なら両端だけ）
/// - `flags`: そのほかの設定（ビットごと）。ビット 0: 形が急に変わる所を細かくしない（再生・ドラッグ中。速さを優先する）。
///   ビット 2: 媒介変数が 3 つ以上の図形を、面ではなく辺だけで描く
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
    complex_params: &str,
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
    complex_mode: u32,
    walls: bool,
    flat: bool,
    layers: u32,
    flat_slice: bool,
    flags: u32,
) -> Result<MeshResult, JsError> {
    let start = timing::now_ms();
    timing::reset();
    let err = |m: &str| JsError::new(m);
    let mut complex_mode = mesh::ComplexMode::from_u32(complex_mode);
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
        complex_height: match complex_mode {
            mesh::ComplexMode::Off | mesh::ComplexMode::Param => None,
            mesh::ComplexMode::Abs => Some(functions::Func::Abs),
            mesh::ComplexMode::Re => Some(functions::Func::Re),
            mesh::ComplexMode::Im => Some(functions::Func::Im),
            mesh::ComplexMode::Arg => Some(functions::Func::Arg),
        },
        lift_param: complex_mode == mesh::ComplexMode::Param,
        complex_params: split(complex_params),
    };
    if space.axes.iter().any(|&a| a >= space.dims) {
        return Err(err("内部エラー: 表示する座標の番号が次元を超えています"));
    }

    let plot = equation::compile_with(source, &consts, &space).map_err(|e| JsError::new(&e))?;
    // 「縦を媒介変数にする」は複素平面の図形だけの設定。陽関数では色分けしない
    if complex_mode == mesh::ComplexMode::Param {
        complex_mode = mesh::ComplexMode::Off;
    }
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
    let ctx = Ctx {
        space: &space,
        dom,
        size,
        resolution: resolution.clamp(2, 1000) as usize,
        implicit_resolution: implicit_resolution.clamp(2, 300) as usize,
        complex_mode,
        walls,
        flat,
        flat_slice: flat && flat_slice,
        layers: layers.clamp(1, 12) as usize,
        refine: flags & FLAG_NO_REFINE == 0,
        edges: flags & FLAG_EDGES != 0,
        range: &range,
        view: space.view(perspective),
    };
    let built = build(&plot, &ctx).map_err(|e| JsError::new(&e))?;
    let mut inner = match &built.shape {
        Some(sh) => {
            let mut m = mesh::project(sh, &ctx.view, dom);
            if flat {
                m = flat_finish(m, built.curve, ctx.flat_slice);
            }
            m
        }
        None => built.inner,
    };
    // 2D の切り口で、射影してから線にした媒介変数表示の曲面
    let curve = built.curve || (ctx.flat_slice && built.shape.is_some());
    let lod_start = timing::now_ms();
    if !curve {
        mesh::add_cluster_lod(&mut inner);
    }
    record_stats(&mut inner, start, lod_start, timing::now_ms());
    // 回転した陽関数は、表示する軸・断面・中心が変わると形が変わる。回転がその条件を満たすかも確かめる
    let explicit_used = (built.shape.is_some() && built.rotated_explicit).then(|| plot.used_params());
    Ok(MeshResult {
        inner,
        flat,
        flat_slice: ctx.flat_slice,
        solid: built.shape.as_ref().is_some_and(|s| s.solid) && !flat,
        shape: built.shape,
        field: (!built.fields.is_empty()).then_some((built.fields, dom)),
        explicit_used,
        kind: built.kind,
        params: plot.param_vars(),
        curve,
        shape_curve: built.curve,
        used_params: plot.used_params(),
        param: match &plot {
            equation::Plot::Param { name, value, .. } => Some((name.clone(), *value)),
            _ => None,
        },
    })
}

// ---- 重い陰関数の F の計算を、いくつかの Worker に分ける ----
// 使い方（Worker 側）: field_mode_plan() → plot(…) → field_chunks() でかたまりの数を知る。
// 手伝う Worker は field_mode_chunks(a, b) → plot(…) → take_field_values()。
// 担当の Worker は全部の値をつなげて field_mode_given(値) → plot(…) でメッシュを作る。
// 計画・一部の計算のときの plot の結果（空のメッシュ）は捨てる

/// 次の [`plot`] では、F の計算の計画（かたまりの数）だけを作る
#[wasm_bindgen]
pub fn field_mode_plan() {
    mesh::set_field_mode(mesh::FieldMode::Plan);
}

/// 次の [`plot`] では、F のかたまり `from..to` の値だけを計算する
#[wasm_bindgen]
pub fn field_mode_chunks(from: u32, to: u32) {
    mesh::set_field_mode(mesh::FieldMode::Chunks(from as usize, to as usize));
}

/// 次の [`plot`] では、F の値 `values`（全部のかたまりをつなげたもの）を使ってメッシュを作る
#[wasm_bindgen]
pub fn field_mode_given(values: Vec<f32>) {
    mesh::set_field_mode(mesh::FieldMode::Given(values));
}

/// 計画の結果: かたまりの数。分けられない式（陰関数でない・陰関数が 2 つ以上）なら 0。F の計算の仕方はふつうに戻す
#[wasm_bindgen]
pub fn field_chunks() -> u32 {
    let out = mesh::take_field_out();
    if out.calls == 1 { out.chunks as u32 } else { 0 }
}

/// 一部の計算の結果（Float32Array）。F の計算の仕方はふつうに戻す
#[wasm_bindgen]
pub fn take_field_values() -> Vec<f32> {
    mesh::take_field_out().values
}

/// F の計算の仕方をふつうに戻す（途中で失敗したとき）
#[wasm_bindgen]
pub fn field_mode_reset() {
    mesh::take_field_out();
}

/// 2D 表示で、3次元に射影した形を平面に描く形にする。
/// 「切り口」なら曲面と平面（縦の座標 = 0）の交わりの線、「投影」なら縦の座標を 0 にした形
fn flat_finish(mut m: mesh::Mesh, curve: bool, slice: bool) -> mesh::Mesh {
    if slice && !curve {
        return mesh::slice_mesh(&m, 0.0);
    }
    mesh::flatten(&mut m, curve);
    m
}

/// 図形を作るのに使う設定
#[derive(Clone)]
struct Ctx<'a> {
    space: &'a equation::Space,
    dom: mesh::Domain,
    /// 表示範囲の対角線の長さ
    size: f64,
    resolution: usize,
    implicit_resolution: usize,
    complex_mode: mesh::ComplexMode,
    walls: bool,
    /// 2D 表示（[`plot`] の `flat` を参照）
    flat: bool,
    /// 2D 表示で「切り口」を描くか（[`plot`] の `flat_slice` を参照）
    flat_slice: bool,
    /// 媒介変数が4つ以上の図形の、面を描く所の分割数（[`plot`] の `layers` を参照）
    layers: usize,
    /// 形が急に変わる所を細かくするか（[`plot`] の `flags` を参照）
    refine: bool,
    /// 媒介変数が 3 つ以上の図形を、辺だけで描くか（[`plot`] の `flags` を参照）
    edges: bool,
    range: &'a dyn Fn(&str) -> (f64, f64),
    /// 回転・射影（4次元の遠近法を含む）
    view: mesh::View,
}

/// 作った図形。媒介変数表示などは、回転・射影する前の n 次元の点（`shape`）で持つ
#[derive(Default)]
struct Built {
    inner: mesh::Mesh,
    shape: Option<mesh::NdShape>,
    /// 陰関数・範囲で重ねた立体の中身（切り口のふた用）
    fields: Vec<mesh::Field>,
    curve: bool,
    kind: String,
    /// 回転した陽関数を点で計算したもの（表示する軸などが変わると使い回せない）
    rotated_explicit: bool,
}

/// 式の種類ごとに図形を作る。リストで複数の図形になったもの（[`equation::Plot::Multi`]）は、まとめて1つにする
fn build(plot: &equation::Plot, ctx: &Ctx) -> Result<Built, String> {
    use equation::Plot as P;
    // 複素平面の図形は、中身の媒介変数表示として作る（説明だけ差し替える）
    if let P::ComplexPlane { inner, label } = plot {
        let mut b = build(inner, ctx)?;
        b.kind = if ctx.flat { format!("{label}（2D）") } else { label.clone() };
        return Ok(b);
    }
    if let P::Multi(ps) = plot {
        let parts = ps.iter().map(|p| build(p, ctx)).collect::<Result<Vec<_>, _>>()?;
        // 2D の切り口では、媒介変数表示の曲面も（射影した後で）線になる
        let line = |b: &Built| b.curve || (ctx.flat_slice && b.shape.is_some());
        let curve = parts.first().is_some_and(line);
        if parts.iter().any(|b| line(b) != curve) {
            return Err("リストで、曲線と曲面が混ざる式はまとめて描けません".into());
        }
        let kind = format!("{} × {}", parts.first().map_or(String::new(), |b| b.kind.clone()), parts.len());
        let rotated_explicit = parts.iter().any(|b| b.rotated_explicit);
        let parts_curve_raw = parts.first().is_some_and(|b| b.curve);
        let fields = parts.iter().flat_map(|b| b.fields.iter().cloned()).collect();
        // 全部が n 次元の点なら、点のままつなげる（回転だけ変わったとき使い回せる）
        if parts.iter().all(|b| b.shape.is_some()) {
            let shapes: Vec<mesh::NdShape> = parts.into_iter().filter_map(|b| b.shape).collect();
            if let Some(shape) = mesh::merge_shapes(shapes.clone()) {
                // 切り口の線にするのは射影した後（plot・Shape::project）なので、ここでは元の種類のままにする
                let curve = parts_curve_raw;
                return Ok(Built { shape: Some(shape), fields, curve, kind, rotated_explicit, ..Default::default() });
            }
            // つなげられない（点の並べ方が違う）ときは、射影してからつなげる
            let meshes = shapes
                .iter()
                .map(|sh| {
                    let m = mesh::project(sh, &ctx.view, ctx.dom);
                    if ctx.flat {
                        flat_finish(m, curve, ctx.flat_slice)
                    } else {
                        m
                    }
                })
                .collect();
            return Ok(Built { inner: mesh::merge(meshes, curve), fields, curve, kind, ..Default::default() });
        }
        let meshes = parts
            .into_iter()
            .map(|b| match &b.shape {
                Some(sh) => {
                    let m = mesh::project(sh, &ctx.view, ctx.dom);
                    if ctx.flat {
                        flat_finish(m, curve, ctx.flat_slice)
                    } else {
                        m
                    }
                }
                None => b.inner,
            })
            .collect();
        return Ok(Built { inner: mesh::merge(meshes, curve), fields, curve, kind, ..Default::default() });
    }
    if ctx.flat {
        return build_2d(plot, ctx);
    }
    let (space, dom) = (ctx.space, ctx.dom);
    let mut b = Built { kind: plot.kind_label(), curve: matches!(plot, P::Curve { .. }), ..Default::default() };
    match plot {
        // 回転した陽関数: 回転の中心から表示範囲の角までの距離 ρ の範囲で計算しておく。
        // こうすると、どの角度に回しても表示範囲が欠けない（角度が変わっても計算し直さなくてよい）。
        // 複素数の色分けをするときは、高さを |f|・Re f・Im f にして回す（色は偏角）
        P::Explicit { axis, prog, rotated: true, .. } => {
            let ext = rotated_domain(space, dom);
            // 広げた範囲でも、表示範囲と同じくらいの格子の間隔にする（同じ分割数のままだと粗くなり、段差の縁が崩れる）。
            // 重くなりすぎないよう、2 倍までにする
            let (ua, va) = equation::others(*axis);
            let width = (dom.hi[ua] - dom.lo[ua]).max(dom.hi[va] - dom.lo[va]);
            let k = ((ext.hi[ua] - ext.lo[ua]) / width).clamp(1.0, 2.0);
            b.shape = Some(mesh::explicit_nd(
                prog,
                *axis,
                space.axes,
                &space.slice,
                ext,
                (ctx.resolution as f64 * k).round() as usize,
                ctx.complex_mode,
                ctx.walls,
                ctx.refine,
                dom.hi[*axis] - dom.lo[*axis],
            ));
            b.rotated_explicit = true;
        }
        P::Explicit { axis, prog, .. } => {
            b.inner =
                mesh::explicit_surface_opt(prog, dom, ctx.resolution, *axis, ctx.complex_mode, ctx.walls, ctx.refine);
        }
        P::Implicit { prog, .. } => {
            b.inner = mesh::implicit_surface(prog, dom, ctx.implicit_resolution);
            b.fields.push(mesh::Field::new(prog, &[], 0, dom));
        }
        P::Solid { prog, ranges, .. } => {
            b.inner = mesh::implicit_solid(prog, dom, ctx.implicit_resolution, ranges, space.samples);
            b.fields.push(mesh::Field::new(prog, ranges, space.samples, dom));
        }
        // 媒介変数表示: n 次元の点を計算してから、回転・射影する
        P::Surface { sets, vars } => {
            let tol = ctx.refine.then_some(ctx.size * 1e-3);
            let (u, v) = ((ctx.range)(&vars[0]), (ctx.range)(&vars[1]));
            b.shape = Some(mesh::parametric_surface_nd_opt(sets, u, v, ctx.resolution, tol));
        }
        // 立体は面が6枚になるので、1枚あたりを少し粗くする
        // 辺だけ描く: 媒介変数を 1 つだけ動かした曲線（媒介変数の直方体の辺の像）
        P::Volume { sets, vars } | P::Sheets { sets, vars } if ctx.edges => {
            let r: Vec<(f64, f64)> = vars.iter().map(|v| (ctx.range)(v)).collect();
            b.curve = true;
            b.kind = format!("{} · 辺 {} 本", b.kind, vars.len() << (vars.len() - 1));
            b.shape = Some(mesh::parametric_edges_nd(sets, &r, 400, ctx.size * 2e-4));
        }
        P::Volume { sets, vars } => {
            let r = [(ctx.range)(&vars[0]), (ctx.range)(&vars[1]), (ctx.range)(&vars[2])];
            b.shape = Some(mesh::parametric_solid_nd(sets, r, (ctx.resolution * 2 / 3).max(2)));
        }
        P::Sheets { sets, vars } => b.shape = Some(sheets(sets, vars, ctx, &mut b.kind)),
        P::Curve { sets, vars } => {
            b.shape = Some(mesh::parametric_curve_nd(sets, (ctx.range)(&vars[0]), 400, ctx.size * 2e-4));
        }
        P::Param { .. } | P::Multi(_) | P::ComplexPlane { .. } => {}
    }
    Ok(b)
}

/// 媒介変数が4つ以上の図形（面の集まり）を作り、説明に面の枚数を足す
fn sheets(sets: &[Vec<eval::Program>], vars: &[String], ctx: &Ctx, kind: &mut String) -> mesh::NdShape {
    let ranges: Vec<(f64, f64)> = vars.iter().map(|v| (ctx.range)(v)).collect();
    let (_, count, n) = mesh::sheet_layout(vars.len(), ctx.layers, ctx.resolution);
    *kind = format!("{kind} · 面 {count} 枚（{n} × {n}）");
    mesh::parametric_sheets_nd(sets, &ranges, ctx.layers, ctx.resolution)
}

/// 2D 表示の図形を作る（[`plot`] の `flat` を参照）
fn build_2d(plot: &equation::Plot, ctx: &Ctx) -> Result<Built, String> {
    use equation::Plot as P;
    let (space, dom) = (ctx.space, ctx.dom);
    // 縦の座標を使っているか（使っていなければ、投影も切り口も同じ形になる）
    let uses_vertical = match plot {
        P::Explicit { axis: 2, .. } => true,
        // y = f(x, z) などの f の変数は、残り2つの表示中の座標（2つ目が縦）
        P::Explicit { prog, .. } => prog.uses_var_index(1),
        P::Implicit { prog, .. } | P::Solid { prog, .. } => prog.uses_var_index(2),
        _ => false,
    };
    // 回した陽関数: 回す前の形を n 次元の点で作り、回して射影してから平面に描く（plot・Shape::project）
    if let P::Explicit { axis, prog, names, rotated: true } = plot {
        if *axis == 2 || uses_vertical {
            // 曲面を作り、射影した後で切り口の線か、押しつぶした形にする。
            // z = f(x, y) を押しつぶした形は、回した後の高さで塗り分ける（格子線用の座標の3つ目が回した後の縦の座標）
            let ctx3 = Ctx { flat: false, flat_slice: false, ..ctx.clone() };
            let mut b = build(plot, &ctx3)?;
            b.kind = if ctx.flat_slice {
                format!("{}の切り口", b.kind)
            } else if *axis == 2 {
                format!("{} の値で塗り分け", names[2])
            } else {
                format!("{}の投影", b.kind)
            };
            return Ok(b);
        }
        let other = if *axis == 0 { &names[1] } else { &names[0] };
        let ext = rotated_domain(space, dom);
        return Ok(Built {
            kind: format!("{} = f({})", names[*axis], other),
            curve: true,
            shape: Some(mesh::explicit_curve_nd(prog, *axis, space.axes, &space.slice, ext, ctx.size * 1e-4)),
            rotated_explicit: true,
            ..Default::default()
        });
    }
    // 投影: 3次元の図形を作ってから、縦の座標を 0 にする（z = f(x, y) は値で塗り分ける）
    if !ctx.flat_slice && uses_vertical && !matches!(plot, P::Explicit { axis: 2, .. }) {
        let ctx3 = Ctx { flat: false, flat_slice: false, ..ctx.clone() };
        let mut b = build(plot, &ctx3)?;
        b.kind = format!("{}の投影", b.kind);
        b.fields.clear();
        if b.shape.is_none() {
            mesh::flatten(&mut b.inner, b.curve);
        }
        return Ok(b);
    }
    // 平面の曲線は、立体の格子よりずっと細かくしても軽い
    let res2 = (ctx.implicit_resolution * 4).clamp(8, 1200);
    let mut b = Built { kind: plot.kind_label(), ..Default::default() };
    match plot {
        // 切り口: z = f(x, y) と平面 z = 0 の交わりは、曲線 f(x, y) = 0
        P::Explicit { axis: 2, prog, names, .. } if ctx.flat_slice => {
            b.curve = true;
            b.kind = format!("{} = f({}, {}) の切り口（f = 0）", names[2], names[0], names[1]);
            b.inner = mesh::implicit_curve_2d(prog, dom, res2);
        }
        P::Explicit { axis: 2, prog, names, .. } => {
            b.kind = format!("{} の値で塗り分け", names[2]);
            b.inner = mesh::flat_map(prog, dom, ctx.resolution, ctx.complex_mode);
        }
        P::Explicit { axis, prog, names, .. } => {
            b.curve = true;
            let other = if *axis == 0 { &names[1] } else { &names[0] };
            b.kind = format!("{} = f({})", names[*axis], other);
            b.inner = mesh::explicit_curve_2d(prog, *axis, dom, ctx.size * 1e-4);
        }
        P::Implicit { prog, .. } => {
            b.curve = true;
            b.kind = "陰関数の曲線".into();
            b.inner = mesh::implicit_curve_2d(prog, dom, res2);
        }
        P::Solid { prog, ranges, .. } => {
            b.kind = "範囲で重ねた領域".into();
            b.inner = mesh::Field::new(prog, ranges, space.samples, dom).cap(dom, 2, 0.0, true, res2 / 2);
        }
        P::Surface { sets, vars } => {
            let tol = ctx.refine.then_some(ctx.size * 1e-3);
            let (u, v) = ((ctx.range)(&vars[0]), (ctx.range)(&vars[1]));
            b.shape = Some(mesh::parametric_surface_nd_opt(sets, u, v, ctx.resolution, tol));
        }
        // 辺だけ描く: 媒介変数を 1 つだけ動かした曲線（媒介変数の直方体の辺の像）
        P::Volume { sets, vars } | P::Sheets { sets, vars } if ctx.edges => {
            let r: Vec<(f64, f64)> = vars.iter().map(|v| (ctx.range)(v)).collect();
            b.curve = true;
            b.kind = format!("{} · 辺 {} 本", b.kind, vars.len() << (vars.len() - 1));
            b.shape = Some(mesh::parametric_edges_nd(sets, &r, 400, ctx.size * 2e-4));
        }
        P::Volume { sets, vars } => {
            let r = [(ctx.range)(&vars[0]), (ctx.range)(&vars[1]), (ctx.range)(&vars[2])];
            b.shape = Some(mesh::parametric_solid_nd(sets, r, (ctx.resolution * 2 / 3).max(2)));
        }
        P::Sheets { sets, vars } => b.shape = Some(sheets(sets, vars, ctx, &mut b.kind)),
        P::Curve { sets, vars } => {
            b.curve = true;
            b.shape = Some(mesh::parametric_curve_nd(sets, (ctx.range)(&vars[0]), 400, ctx.size * 1e-4));
        }
        P::Param { .. } | P::Multi(_) | P::ComplexPlane { .. } => {}
    }
    Ok(b)
}

/// 回した陽関数を計算する範囲（表示中の座標で）: 回転の中心から表示範囲の角までの距離 ρ の、中心の周りの立方体。
/// 回転は中心からの距離を変えないので、どの角度に回しても表示範囲が欠けない（角度が変わっても計算し直さなくてよい）
fn rotated_domain(space: &equation::Space, dom: mesh::Domain) -> mesh::Domain {
    let c: Vec<f64> = space.axes.iter().map(|&a| space.center.get(a).copied().unwrap_or(0.0)).collect();
    let rho = (0..3)
        .map(|a| (dom.lo[a] - c[a]).abs().max((dom.hi[a] - c[a]).abs()).powi(2))
        .sum::<f64>()
        .sqrt();
    mesh::Domain { lo: [c[0] - rho, c[1] - rho, c[2] - rho], hi: [c[0] + rho, c[1] + rho, c[2] + rho] }
}

/// 媒介変数表示の、回転・射影する前の形（n 次元の点）。Worker が式ごとに持っておく
#[wasm_bindgen]
pub struct Shape {
    inner: mesh::NdShape,
    /// 2D 表示（射影した後で縦の座標を 0 にする）
    flat: bool,
    /// 2D 表示の切り口（射影した後で、平面との交わりの線にする）
    flat_slice: bool,
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
        let start = timing::now_ms();
        timing::reset();
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
        let mut inner = mesh::project(&self.inner, &space.view(perspective), dom);
        if self.flat {
            inner = flat_finish(inner, self.curve, self.flat_slice);
        }
        let end = timing::now_ms();
        record_stats(&mut inner, start, end, end);
        Ok(MeshResult {
            inner,
            flat: self.flat,
            flat_slice: self.flat_slice,
            solid: self.inner.solid && !self.flat,
            field: None,
            shape: None,
            explicit_used: None,
            kind: self.kind.clone(),
            params: self.params.clone(),
            curve: self.curve || self.flat_slice,
            shape_curve: self.curve,
            used_params: self.used_params.clone(),
            param: None,
        })
    }
}

/// 陰関数・範囲で重ねた立体の中身。切り口の位置だけが変わったときは、これで切り口のふただけを作り直す
#[wasm_bindgen]
pub struct CapSource {
    inner: Vec<mesh::Field>,
    dom: mesh::Domain,
}

#[wasm_bindgen]
impl CapSource {
    /// 平面「画面の `axis` 番目（0: 横, 1: 奥, 2: 縦）の座標 = `pos`」での切り口のふた。
    /// `keep_below` なら座標が `pos` 以下の側を残す（ふたは反対側を向く）。`n` は格子の1辺のマス数
    pub fn cap(&self, axis: u32, pos: f64, keep_below: bool, n: u32) -> Result<MeshResult, JsError> {
        self.caps(axis as i32, pos, keep_below, n, 0)
    }

    /// 切り口のふた（`axis` が負なら切り口なし）と、範囲で重ねた立体の表示範囲の端のふた（`box_n` が 0 なら作らない。
    /// 細かさは `box_n`）をまとめたもの。端のふたは、切り口で切り落とされる側を描かない
    pub fn caps(&self, axis: i32, pos: f64, keep_below: bool, n: u32, box_n: u32) -> Result<MeshResult, JsError> {
        if axis > 2 {
            return Err(JsError::new("内部エラー: 切り口の軸の番号が違います"));
        }
        let start = timing::now_ms();
        timing::reset();
        let cut = (axis >= 0).then_some((axis as usize, pos, keep_below));
        let mut parts = Vec::new();
        for f in &self.inner {
            if let Some((a, p, k)) = cut {
                parts.push(f.cap(self.dom, a, p, k, n.clamp(2, 1000) as usize));
            }
            if box_n > 0 && f.is_stacked() {
                parts.push(f.box_caps(self.dom, box_n.clamp(2, 1000) as usize, cut));
            }
        }
        let mut inner = mesh::merge(parts, false);
        let end = timing::now_ms();
        record_stats(&mut inner, start, end, end);
        Ok(MeshResult {
            inner,
            kind: String::new(),
            params: Vec::new(),
            curve: false,
            shape_curve: false,
            used_params: Vec::new(),
            param: None,
            shape: None,
            explicit_used: None,
            field: None,
            solid: false,
            flat: false,
            flat_slice: false,
        })
    }
}

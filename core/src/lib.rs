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
pub mod system;
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
    /// 回す前の形で作った陰関数（回転だけ変わったときに使い回す）と、式が使っている名前
    local: Option<(mesh::LocalMesh, Vec<String>)>,
    /// 回す前の形を作れる陰関数か（[`plot`] の `flags` のビット 3 で作れる）
    local_available: bool,
    /// 頂点に掛ける 1 次式 q = M p + t（[M の 1 行目, t₀, 2 行目, t₁, 3 行目, t₂] の 12 個）。
    /// 回す前の形のメッシュを返したときだけ（[`Shape::local_mesh`]・[`Shape::transform`]）。描画側で 3D オブジェクトに掛ける
    transform: Option<[f64; 12]>,
    /// 陰関数・範囲で重ねた立体なら、その中身（切り口のふたを作るため。リストなら複数）
    field: Option<(Vec<mesh::Field>, mesh::Domain)>,
    /// 媒介変数表示の立体（閉じた面）なら true。切り口のふたは描画側で作る
    solid: bool,
    /// 2D 表示で作ったもの（射影し直すときも平らにする）
    flat: bool,
    /// 2D 表示の切り口（射影し直すときも、平面との交わりの線にする）
    flat_slice: bool,
    /// 2D 表示の不等式の領域の境目の線（実線・点線。線分ごとに 2 点と NaN、または点の列）。ふつうは空
    outline: (Vec<f32>, Vec<f32>),
    /// 表示していない座標の値で色を付けるなら、その値の範囲（[`hidden_style`]）。値は頂点ごとの complex の 1 つ目
    value_range: Option<(f64, f64)>,
    /// 伝達関数で描くなら、格子（1 辺 n 点。x が最も速く変わる順）での値と n（[`hidden_style`]）
    volume: Option<(Vec<f32>, usize)>,
}

#[wasm_bindgen]
impl MeshResult {
    /// 表示していない座標の値で色を付けるなら [最小, 最大]（値は complex_values の 1 つ目）。付けないなら空
    pub fn value_range(&self) -> Vec<f64> {
        self.value_range.map_or(Vec::new(), |(a, b)| vec![a, b])
    }
    /// 伝達関数の格子の 1 辺の点の数（伝達関数でなければ 0）
    pub fn volume_n(&self) -> u32 {
        self.volume.as_ref().map_or(0, |v| v.1 as u32)
    }
    /// 伝達関数の格子での値（Float32Array。x が最も速く変わり、次に y、z。定義されない所は NaN）
    pub fn take_volume(&mut self) -> Vec<f32> {
        self.volume.as_mut().map_or(Vec::new(), |v| std::mem::take(&mut v.0))
    }
    /// 2D 表示の不等式の領域の境目の線のうち、実線（`≤`・`≥`）の分を取り出す（Float32Array。無ければ空）
    pub fn take_outline(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.outline.0)
    }
    /// 同じく、点線（`<`・`>`）の分
    pub fn take_outline_dashed(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.outline.1)
    }
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
    /// 頂点に掛ける 1 次式（12 個。[`MeshResult`] の transform を参照）。無ければ空
    #[wasm_bindgen(getter)]
    pub fn transform(&self) -> Vec<f64> {
        self.transform.map_or_else(Vec::new, |t| t.to_vec())
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
    /// 回す前の形を作れる陰関数か（回転が表示中の座標どうしだけで、2D 表示でない）。そうなら、`flags` のビット 3 を付けて
    /// もう一度 [`plot`] を呼ぶと、回す前の形だけを作る（`take_shape`）
    #[wasm_bindgen(getter)]
    pub fn local_available(&self) -> bool {
        self.local_available
    }
    /// 媒介変数表示なら、回転・射影する前の形を取り出す（取り出せるのは1回だけ）。
    /// 回転・表示する軸・表示範囲だけが変わったときは、これの `project` で描き直せる
    pub fn take_shape(&mut self) -> Option<Shape> {
        let (inner, local) = match (self.shape.take(), self.local.take()) {
            (Some(sh), _) => (Some(sh), None),
            (None, Some(l)) => (None, Some(l)),
            (None, None) => return None,
        };
        let explicit_used = match &local {
            Some((_, used)) => Some(used.clone()),
            None => self.explicit_used.clone(),
        };
        Some(Shape {
            inner,
            local: local.map(|l| l.0),
            flat: self.flat,
            flat_slice: self.flat_slice,
            explicit_used,
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
/// [`plot`] の `flags` のビット 3: 陰関数の、回す前の形だけを作る
const FLAG_LOCAL: u32 = 8;
/// [`plot`] の `flags` のビット 1: 陰関数で、格子より小さい図形を念入りに探す（遅くなる）
const FLAG_THOROUGH: u32 = 2;
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
/// - `flags`: そのほかの設定（ビットごと）。ビット 0: 形が急に変わる所を細かくしない（再生・ドラッグ中。速さを優先する。
///   陰関数の、格子より小さい図形を細かく作り直すこともしない）。
///   ビット 1: 陰関数で、格子より小さい図形を念入りに探す（遅くなる）。
///   ビット 2: 媒介変数が 3 つ以上の図形を、面ではなく辺だけで描く。
///   ビット 3: 回転が表示中の座標どうしだけの陰関数（結果の `local_available`）を、回す前の形で（表示範囲より広く）作る。
///   表示する面は作らない（結果のメッシュは空）。結果の `take_shape` を回せば、回転が変わっても F を計算し直さない。
///   表示する面とは別の呼び出しにしてあるのは、止まったときに表示する面を先に返し、回す前の形は後から作るため
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
    let hidden = HIDDEN_STYLE.with(|h| std::mem::take(&mut *h.borrow_mut()));
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
        flat_slice: flat && flat_slice,
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
        implicit_refine: if flags & FLAG_NO_REFINE != 0 {
            mesh::Refine::Light
        } else if flags & FLAG_THOROUGH != 0 {
            mesh::Refine::Thorough
        } else {
            mesh::Refine::Normal
        },
        edges: flags & FLAG_EDGES != 0,
        local: flags & FLAG_LOCAL != 0,
        range: &range,
        view: space.view(perspective),
    };
    // 表示していない座標の見せ方（2D 表示では、今までどおり断面）
    let hidden_on = !flat && hidden.first().is_some_and(|&m| m >= 1.0);
    let built = if hidden_on {
        build_hidden(&hidden, source, &consts, &plot, &ctx)
    } else {
        build(&plot, &ctx)
    }
    .map_err(|e| JsError::new(&e))?;
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
        outline: built.outline,
        value_range: built.value_range,
        volume: built.volume,
        solid: built.shape.as_ref().is_some_and(|s| s.solid) && !flat,
        shape: built.shape,
        local: built.local,
        transform: None,
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
        local_available: !flat && !hidden_on && matches!(&plot, equation::Plot::Implicit { local: Some(_), .. }),
    })
}

// ---- 表示していない座標（w 以降）の見せ方 ----

thread_local! {
    /// 次の [`plot`] だけに効く、表示していない座標の見せ方（[`hidden_style`]）
    static HIDDEN_STYLE: std::cell::RefCell<Vec<f64>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// 次の [`plot`] で、表示していない座標（w 以降）を使う式をどう見せるかを決める（その 1 回だけ。呼ばなければ断面）。
/// `values` = [見せ方, 枚数, 座標 0 の範囲の最小, 最大, 座標 1 の …]（範囲は全部の座標の分。表示していない座標の分だけ使う。
/// NaN なら断面の値だけ）。見せ方は 0: 断面（今まで）、1: 等値面を何枚も（範囲を `枚数` 等分した値ごとの図形を重ね、値で色を付ける）、
/// 2: 断面の塗り分け（各軸に `枚数` 枚の平面を、値で塗る）、3: 伝達関数（格子での値を返す。描画側で光線を進めて塗る）。
/// 4: 範囲で重ねる（式ごとの範囲で、通る領域を描く）。2・3 は「表示していない座標 = 式」の形のときだけ。
/// 断面以外では、式が使っている表示していない座標について、次元の設定の「範囲で重ねる」より、この見せ方を優先する
#[wasm_bindgen]
pub fn hidden_style(values: Vec<f64>) {
    HIDDEN_STYLE.with(|h| *h.borrow_mut() = values);
}

/// 等値面を何枚も重ねるときの、図形の数の上限
const MAX_HIDDEN_LAYERS: usize = 64;

/// [`hidden_style`] の見せ方で図形を作る
fn build_hidden(h: &[f64], source: &str, params: &eval::Params, plot: &equation::Plot, ctx: &Ctx) -> Result<Built, String> {
    let space = ctx.space;
    let mode = h[0] as u32;
    let count = (h.get(1).copied().unwrap_or(5.0) as usize).clamp(1, 16);
    // 座標 k の範囲（指定が無ければ断面の値だけ）
    let range_of = |k: usize| match (h.get(2 + 2 * k), h.get(3 + 2 * k)) {
        (Some(&a), Some(&b)) if a.is_finite() && b.is_finite() => (a.min(b), a.max(b)),
        _ => {
            let v = space.slice.get(k).copied().unwrap_or(0.0);
            (v, v)
        }
    };
    let dom = ctx.dom;
    // 式が使っている、表示していない座標
    let used = plot.used_params();
    let hidden: Vec<usize> =
        (0..space.dims).filter(|k| !space.axes.contains(k) && used.contains(&equation::coord_name(*k))).collect();
    // 式ごとの見せ方を、次元の設定の「範囲で重ねる」より優先する（その座標は断面の値として扱う。重ねると、値を変えても同じ立体になる）
    let mut own = space.clone();
    for &k in &hidden {
        if let Some(r) = own.ranges.get_mut(k) {
            *r = None;
        }
    }
    match mode {
        // 範囲で重ねる: 式ごとの範囲で、通る領域（立体）を描く。範囲が 1 点の座標は断面
        4 => {
            for &k in &hidden {
                let (a, b) = range_of(k);
                if a < b {
                    own.ranges[k] = Some((a, b));
                } else {
                    own.slice[k] = a;
                }
            }
            let p = equation::compile_with(source, params, &own)?;
            build(&p, &Ctx { space: &own, ..ctx.clone() })
        }
        1 => {
            if hidden.is_empty() {
                return build(plot, ctx);
            }
            // 座標ごとの値の数（全部の組み合わせが上限を超えないように減らす。範囲が 1 点の座標は動かないので数えない）
            let moving = hidden.iter().filter(|&&k| range_of(k).0 < range_of(k).1).count().max(1);
            let mut per = count.max(2);
            while per > 2 && per.pow(moving as u32) > MAX_HIDDEN_LAYERS {
                per -= 1;
            }
            let values: Vec<Vec<f64>> = hidden
                .iter()
                .map(|&k| {
                    let (a, b) = range_of(k);
                    if a == b {
                        vec![a]
                    } else {
                        (0..per).map(|i| a + (b - a) * i as f64 / (per - 1) as f64).collect()
                    }
                })
                .collect();
            let total: usize = values.iter().map(Vec::len).product();
            if total > MAX_HIDDEN_LAYERS {
                return Err(format!("等値面が多すぎます（{MAX_HIDDEN_LAYERS} 枚まで）"));
            }
            let mut meshes = Vec::with_capacity(total);
            let mut curve = None;
            let mut kind = String::new();
            for idx in 0..total {
                let mut sp = own.clone();
                let mut rest = idx;
                for (j, &k) in hidden.iter().enumerate() {
                    sp.slice[k] = values[j][rest % values[j].len()];
                    rest /= values[j].len();
                }
                let first = sp.slice[hidden[0]];
                let p = equation::compile_with(source, params, &sp)?;
                let ctx2 = Ctx { space: &sp, ..ctx.clone() };
                let b = build(&p, &ctx2)?.settle();
                if *curve.get_or_insert(b.curve) != b.curve {
                    return Err("等値面を何枚も重ねると、曲線と曲面が混ざります".into());
                }
                kind = b.kind.clone();
                let mut m = match &b.shape {
                    Some(sh) => mesh::project(sh, &ctx.view, dom),
                    None => b.inner,
                };
                let verts = m.positions.len() / 3;
                m.complex = (0..verts).flat_map(|_| [first as f32, 0.0, 0.0]).collect();
                meshes.push(m);
            }
            let curve = curve.unwrap_or(false);
            let name = equation::coord_name(hidden[0]);
            Ok(Built {
                inner: mesh::merge(meshes, curve),
                curve,
                kind: format!("{kind}（{name} の値ごとに {total} 枚）"),
                value_range: Some(range_of(hidden[0])),
                ..Default::default()
            })
        }
        2 | 3 => {
            let (f, k) = equation::hidden_field(source, params, space)?;
            let name = equation::coord_name(k);
            if mode == 2 {
                let n = ctx.resolution.clamp(8, 200);
                let mut m = mesh::Mesh::default();
                for axis in 0..3 {
                    let (ua, va) = equation::others(axis);
                    for i in 0..count {
                        let pos = dom.lo[axis] + (dom.hi[axis] - dom.lo[axis]) * (i + 1) as f64 / (count + 1) as f64;
                        plane_by_value(&mut m, &f, dom, axis, ua, va, pos, n);
                    }
                }
                Ok(Built {
                    inner: m,
                    kind: format!("{name} の値で断面を塗り分け（{} 枚）", 3 * count),
                    value_range: Some(range_of(k)),
                    ..Default::default()
                })
            } else {
                let n = ctx.implicit_resolution.clamp(24, 96);
                let at = |a: usize, i: usize| dom.lo[a] + (dom.hi[a] - dom.lo[a]) * i as f64 / (n - 1) as f64;
                let mut cols: [Vec<f64>; 3] = Default::default();
                for z in 0..n {
                    for y in 0..n {
                        for x in 0..n {
                            cols[0].push(at(0, x));
                            cols[1].push(at(1, y));
                            cols[2].push(at(2, z));
                        }
                    }
                }
                let mut out = vec![num_complex::Complex64::new(0.0, 0.0); n * n * n];
                f.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
                let vals = out.into_iter().map(|v| eval::as_real(v) as f32).collect();
                Ok(Built {
                    kind: format!("{name} の値の伝達関数（{n}³）"),
                    value_range: Some(range_of(k)),
                    volume: Some((vals, n)),
                    ..Default::default()
                })
            }
        }
        _ => build(plot, ctx),
    }
}

/// 平面「`axis` 番目の座標 = `pos`」を `n × n` マスに分け、頂点ごとの式 `f` の値を complex の 1 つ目に入れて `m` に足す。
/// 値が有限でない頂点のあるマスは描かない
#[allow(clippy::too_many_arguments)]
fn plane_by_value(m: &mut mesh::Mesh, f: &eval::Program, dom: mesh::Domain, axis: usize, ua: usize, va: usize, pos: f64, n: usize) {
    let w = n + 1;
    let mut cols: [Vec<f64>; 3] = [vec![pos; w * w], vec![pos; w * w], vec![pos; w * w]];
    for j in 0..w {
        for i in 0..w {
            cols[ua][j * w + i] = dom.lo[ua] + (dom.hi[ua] - dom.lo[ua]) * i as f64 / n as f64;
            cols[va][j * w + i] = dom.lo[va] + (dom.hi[va] - dom.lo[va]) * j as f64 / n as f64;
        }
    }
    let mut out = vec![num_complex::Complex64::new(0.0, 0.0); w * w];
    f.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
    let vals: Vec<f64> = out.into_iter().map(eval::as_real).collect();
    let base = (m.positions.len() / 3) as u32;
    let mut nrm = [0.0f32; 3];
    nrm[axis] = 1.0;
    for k in 0..w * w {
        m.positions.extend([cols[0][k] as f32, cols[1][k] as f32, cols[2][k] as f32]);
        // 平面に垂直な向きの格子線は引かない（平面が格子線の位置にあると、面全体が点々の線になる）
        let mut g = [cols[0][k] as f32, cols[1][k] as f32, cols[2][k] as f32];
        g[axis] = mesh::NO_GRID;
        m.grid_coords.extend(g);
        m.normals.extend(nrm);
        m.complex.extend([vals[k] as f32, 0.0, 0.0]);
    }
    for j in 0..n {
        for i in 0..n {
            let (a, b, c, d) = (j * w + i, j * w + i + 1, (j + 1) * w + i, (j + 1) * w + i + 1);
            if [a, b, c, d].iter().all(|&q| vals[q].is_finite()) {
                let id = |q: usize| base + q as u32;
                m.indices.extend([id(a), id(b), id(d), id(a), id(d), id(c)]);
            }
        }
    }
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
    /// 陰関数で、格子より小さい図形を細かく作り直すか（[`plot`] の `flags` を参照）
    implicit_refine: mesh::Refine,
    /// 媒介変数が 3 つ以上の図形を、辺だけで描くか（[`plot`] の `flags` を参照）
    edges: bool,
    /// 回転が表示中の座標どうしだけの陰関数の、回す前の形だけを作るか（[`plot`] の `flags` を参照）
    local: bool,
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
    /// 回す前の形で作った陰関数と、式が使っている名前（[`mesh::LocalMesh`]）
    local: Option<(mesh::LocalMesh, Vec<String>)>,
    /// 2D 表示の不等式の領域の境目の線（実線, 点線）
    outline: (Vec<f32>, Vec<f32>),
    /// 表示していない座標の値で色を付けるなら、その範囲
    value_range: Option<(f64, f64)>,
    /// 伝達関数の格子の値と、1 辺の点の数
    volume: Option<(Vec<f32>, usize)>,
}

impl Built {
    /// 回す前の形を捨てる（リスト・2D 表示では使い回さない。表示するメッシュは別に作ってある）
    fn settle(mut self) -> Built {
        self.local = None;
        self
    }
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
        // 回す前の形で作った陰関数は、リストでは使い回さない（今の回転で回したメッシュにする）
        let parts = ps.iter().map(|p| build(p, ctx).map(Built::settle)).collect::<Result<Vec<_>, _>>()?;
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
        let outline = (
            parts.iter().flat_map(|b| b.outline.0.iter().copied()).collect(),
            parts.iter().flat_map(|b| b.outline.1.iter().copied()).collect(),
        );
        // 全部が n 次元の点なら、点のままつなげる（回転だけ変わったとき使い回せる）
        if parts.iter().all(|b| b.shape.is_some()) {
            let shapes: Vec<mesh::NdShape> = parts.into_iter().filter_map(|b| b.shape).collect();
            if let Some(shape) = mesh::merge_shapes(shapes.clone()) {
                // 切り口の線にするのは射影した後（plot・Shape::project）なので、ここでは元の種類のままにする
                let curve = parts_curve_raw;
                return Ok(Built { shape: Some(shape), fields, curve, kind, rotated_explicit, outline, ..Default::default() });
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
            return Ok(Built { inner: mesh::merge(meshes, curve), fields, curve, kind, outline, ..Default::default() });
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
        return Ok(Built { inner: mesh::merge(meshes, curve), fields, curve, kind, outline, ..Default::default() });
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
        // 表示していない座標が絡む回転をした陽関数: 傾いた座標系の陽関数として作る。
        // 回転が変わるたびに計算し直す（点は使い回さない）。切り口のふたの元は作らない（回さない陽関数と同じ）
        P::Tilted { prog, color, frame, grid, .. } => {
            b.inner = mesh::explicit_tilted(
                prog,
                color.as_ref(),
                *frame,
                *grid,
                dom,
                ctx.resolution,
                ctx.walls,
                ctx.refine,
            );
        }
        // 回転が表示中の座標どうしだけの陰関数の、回す前の形だけを作る依頼（FLAG_LOCAL）: 回す前の F で、回転の中心から表示範囲の角までの
        // 距離 ρ の立方体にメッシュを作る（[`mesh::LocalMesh`]）。表示する面（回した F で表示範囲に作るもの）は作らない（Worker が
        // 先に別の呼び出しで作って返し、その後でこれを呼ぶ）。次に回転を再生・ドラッグするときは、これを描画側で 3D オブジェクトとして
        // 回すだけで描ける。
        // 動いている間だけ使うので、格子のマスの数は表示範囲と同じにする（マスは表示範囲より 1.5〜2.5 倍大きい。法線は式の勾配から
        // 求めるので、光の当たり方は止まったときとそろう）。細かくすると、止まったときの計算と三角形の数が何倍にもなる
        // （ジャイロイドで 0.1 → 1 秒、17 万 → 125 万個）。
        // 段差のある式（floor・mod など）の枝は、立方体に内接する球（どの向きに回しても表示範囲が入る所）の中だけで調べる
        // （[`mesh::in_inscribed_ball`]）。角まで調べると、枝の数が部品の数の上限を超えて、表示範囲の中の枝まで描かなくなる
        // （mod(x, 2) + mod(y, 2) + mod(z, 2) = 2 で 2 割欠けた）
        P::Implicit { local: Some(lp), .. } if ctx.local => {
            let ext = rotated_domain(space, dom);
            let mut m = mesh::in_inscribed_ball(|| {
                mesh::implicit_solid_opt(lp, ext, ctx.implicit_resolution, &[], 0, mesh::Refine::Light)
            });
            mesh::add_cluster_lod(&mut m);
            let local = mesh::LocalMesh { mesh: m, prog: lp.clone(), dims: space.dims, axes: space.axes, slice: space.slice.clone() };
            b.local = Some((local, lp.used_params().to_vec()));
        }
        // 複素数の陰関数（値が複素数の F = 0）: Re F = Im F = 0 の線として、連立方程式の交わる線と同じ道で描く
        // 複素数の色分けをするなら、曲面（|F| = 1・Re F = 0・Im F = 0）を作って、頂点の F の値で色を付ける（陽関数の色分けと同じ色）
        P::Implicit { prog, .. } if system::is_complex_valued(prog, dom) => {
            use functions::Func;
            let surface = match ctx.complex_mode {
                mesh::ComplexMode::Abs | mesh::ComplexMode::Arg => Some((prog.level_of(Func::Abs, 1.0), "|F| = 1 の曲面、色は arg F")),
                mesh::ComplexMode::Re => Some((prog.level_of(Func::Re, 0.0), "Re F = 0 の曲面、色は F")),
                mesh::ComplexMode::Im => Some((prog.level_of(Func::Im, 0.0), "Im F = 0 の曲面、色は F")),
                _ => None,
            };
            if let Some((g, label)) = surface {
                b.kind = format!("複素数の陰関数（{label}）");
                b.inner = mesh::implicit_solid_opt(&g, dom, ctx.implicit_resolution, &[], 0, ctx.implicit_refine);
                mesh::color_by_complex(&mut b.inner, prog);
            } else {
                b.curve = true;
                b.kind = "複素数の陰関数（Re F = Im F = 0 の曲線）".into();
                let (re, im) = (prog.part(Func::Re), prog.part(Func::Im));
                b.inner = system::intersection_curve(&re, &im, dom, ctx.implicit_resolution, ctx.implicit_refine);
            }
        }
        // 面にならず、線・点だけになる式（A² + B² = 0 は A = B = 0 の線）: 交わる線・点として描く
        P::Implicit { prog, .. } if prog.common_zero_sets().is_some() => {
            let sets = prog.common_zero_sets().unwrap_or_default();
            let (m, kind) = system::common_zero_sets(&sets, dom, ctx.implicit_resolution, ctx.implicit_refine);
            b.inner = m;
            b.kind = kind;
            b.curve = true;
        }
        P::Implicit { prog, .. } => {
            b.inner = mesh::implicit_solid_opt(prog, dom, ctx.implicit_resolution, &[], 0, ctx.implicit_refine);
            b.kind.push_str(&b.inner.note);
            b.fields.push(mesh::Field::new(prog, &[], 0, dom));
        }
        P::Solid { prog, ranges, .. } => {
            b.inner = mesh::implicit_solid(prog, dom, ctx.implicit_resolution, ranges, space.samples);
            b.kind.push_str(&b.inner.note);
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
        // 等式 1 本と不等式: 等式の行を今までの道で描き、できたメッシュを不等式ごとに切り取る（system::trim_mesh）。
        // 回した陽関数の点（shape）は射影してメッシュにする（回転が変わるたびに計算し直す）。切り口のふたは作らない
        P::System { base: Some(base), eqs, cuts, .. } if eqs.len() == 1 => {
            let inner = build(base, ctx)?.settle();
            let mut m = match &inner.shape {
                Some(sh) => mesh::project(sh, &ctx.view, dom),
                None => inner.inner,
            };
            for g in cuts {
                system::trim_mesh(&mut m, &mut system::values_of(g), 8);
            }
            b.inner = m;
            b.curve = inner.curve;
            b.kind = if cuts.is_empty() { inner.kind } else { format!("{}（不等式で切り取り）", inner.kind) };
        }
        // 交わる線: 書いた順で 1 本目の等式の面の上で、2 本目の等式の線を取り出し、ニュートン法で寄せる（system.rs）。
        // 不等式があれば、線分を切る
        P::System { eqs, cuts, .. } if eqs.len() == 2 => {
            b.curve = true;
            b.inner = system::intersection_curve(&eqs[0], &eqs[1], dom, ctx.implicit_resolution, ctx.implicit_refine);
            for g in cuts {
                system::clip_segments(&mut b.inner.positions, &mut system::values_of(g), 12);
            }
        }
        // 等式 3 本の点: 2 本の交わる線の上で 3 本目の符号が変わる所から寄せる。不等式を満たす点だけ残す
        P::System { eqs, cuts, .. } if eqs.len() == 3 => {
            let pts = system::points_on_curve(eqs, dom, ctx.implicit_resolution, ctx.implicit_refine);
            let pts = system::points_inside(pts, cuts);
            b.curve = true;
            b.kind = format!("連立方程式の点（{} 個）", pts.len());
            b.inner = system::points_mesh(&pts);
        }
        // 不等式の領域: 面 G_i = 0 ごとに作り、ほかの不等式で切り取る。切り口と表示範囲の端には、ふたをする
        // 媒介変数表示を不等式で切り取る: 射影してから、曲面なら三角形を、曲線なら線分を切る（回転が変わるたびに計算し直す）
        P::System { base: Some(base), eqs, cuts, .. } if eqs.is_empty() => {
            let inner = build(base, ctx)?.settle();
            let mut m = match &inner.shape {
                Some(sh) => mesh::project(sh, &ctx.view, dom),
                None => inner.inner,
            };
            for g in cuts {
                if inner.curve {
                    system::clip_segments(&mut m.positions, &mut system::values_of(g), 12);
                } else {
                    system::trim_mesh(&mut m, &mut system::values_of(g), 8);
                }
            }
            b.inner = m;
            b.curve = inner.curve;
            b.kind = format!("{}（不等式で切り取り）", inner.kind);
        }
        P::System { eqs, cuts, .. } if eqs.is_empty() => {
            b.inner = system::region_faces(cuts, dom, ctx.implicit_resolution, ctx.implicit_refine);
            b.fields.push(mesh::Field::region(cuts));
        }
        P::System { .. } => return Err("連立方程式・不等式は、まだ描けません".into()),
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
        // 傾いた座標系の陽関数は、3 次元の曲面を作ってから押しつぶす（2D の切り口では、この道にならない）
        P::Tilted { .. } => true,
        P::System { eqs, cuts, .. } => eqs.iter().chain(cuts).any(|p| p.uses_var_index(2)),
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
        let mut b = build(plot, &ctx3)?.settle();
        b.kind = format!("{}の投影", b.kind);
        // 不等式の領域は、境目の面と表示範囲の端のふたをまとめて平らにする（端で切れた所も塗る）
        if let P::System { eqs, cuts, .. } = plot {
            if eqs.is_empty() {
                let caps = mesh::Field::region(cuts).box_caps(dom, ctx.implicit_resolution.max(8), None);
                b.inner = mesh::merge(vec![std::mem::take(&mut b.inner), caps], false);
            }
        }
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
        // 複素数の陰関数の 2D 表示: Re F = Im F = 0 の交点
        P::Implicit { prog, .. } if system::is_complex_valued(prog, dom) => {
            let (re, im) = (prog.part(functions::Func::Re), prog.part(functions::Func::Im));
            let pts = system::flat_points(&re, &im, dom, res2);
            b.curve = true;
            b.kind = format!("複素数の陰関数（Re F = Im F = 0 の点、{} 個）", pts.len());
            b.inner = system::points_mesh(&pts);
        }
        P::Implicit { prog, from, .. } => {
            b.curve = true;
            b.kind = match from {
                // 回した陽関数（2D の切り口）: 縦の座標 = 0 の曲線
                Some(f) => format!("{f} を回したものの切り口"),
                None => "陰関数の曲線".into(),
            };
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
        // 傾いた座標系の陽関数は、上の投影の道で作る
        P::Param { .. } | P::Multi(_) | P::ComplexPlane { .. } | P::Tilted { .. } => {}
        // 媒介変数表示を切り取ったもの: 3D で作ってから、切り口の線か、押しつぶした形にする
        P::System { base: Some(base), eqs, .. } if eqs.is_empty() && !base.param_vars().is_empty() => {
            let ctx3 = Ctx { flat: false, flat_slice: false, ..ctx.clone() };
            let inner = build(plot, &ctx3)?.settle();
            b.kind = inner.kind;
            b.curve = inner.curve || ctx.flat_slice;
            b.inner = flat_finish(inner.inner, inner.curve, ctx.flat_slice);
        }
        // 縦の座標 = 0 の 2 次元の問題として描く（切り口、または縦の座標を使わない式）
        P::System { eqs, cuts, strict, .. } => match eqs.len() {
            0 => {
                b.kind = "不等式の領域（2D）".into();
                b.inner = mesh::Field::region(cuts).cap(dom, 2, 0.0, true, res2 / 2);
                // 境目の線: 不等式ごとに G_i = 0 の線を、ほかの不等式で切る。厳密な不等式（< ・ >）の分は点線
                for (i, g) in cuts.iter().enumerate() {
                    let mut line = mesh::implicit_curve_2d(g, dom, res2).positions;
                    for (j, other) in cuts.iter().enumerate() {
                        if j != i {
                            system::clip_segments(&mut line, &mut system::values_of(other), 12);
                        }
                    }
                    if strict[i] { &mut b.outline.1 } else { &mut b.outline.0 }.extend(line);
                }
            }
            1 => {
                b.curve = true;
                b.kind = "陰関数の曲線（不等式で切り取り）".into();
                b.inner = mesh::implicit_curve_2d(&eqs[0], dom, res2);
                for g in cuts {
                    system::clip_segments(&mut b.inner.positions, &mut system::values_of(g), 12);
                }
            }
            2 => {
                let pts = system::points_inside(system::flat_points(&eqs[0], &eqs[1], dom, res2), cuts);
                b.curve = true;
                b.kind = format!("連立方程式の点（{} 個）", pts.len());
                b.inner = system::points_mesh(&pts);
            }
            _ => return Err("2D 表示では、等式は 2 本までにしてください".into()),
        },
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
    /// n 次元の点（媒介変数表示・回した陽関数）。回す前の形で作った陰関数なら None で、`local` を使う
    inner: Option<mesh::NdShape>,
    local: Option<mesh::LocalMesh>,
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

    /// 回す前の形で作った陰関数か。そうなら、陰関数の細かさ・念入りに探すかでも形が変わる
    #[wasm_bindgen(getter)]
    pub fn implicit(&self) -> bool {
        self.local.is_some()
    }

    /// 回転・表示する軸・表示範囲を変えて、もう一度3次元に射影する（式は計算し直さない）。
    /// 引数の意味は [`plot`] と同じ。
    /// 回転した陽関数で、新しい回転が「表示中の座標どうし」でなくなったときは
    /// "RECOMPUTE" というエラーを返す（傾いた座標系の陽関数・陰関数として計算し直す必要がある）
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
        let (view, dom) = self.view_for(domain, dims, axes, rotation, center, perspective)?;
        let (mut inner, field, solid) = match (&self.inner, &self.local) {
            (Some(sh), _) => (mesh::project(sh, &view, dom), None, sh.solid),
            (None, Some(l)) => {
                let field = (!self.flat).then(|| (vec![l.field(&view, dom)], dom));
                (l.project(&view), field, false)
            }
            (None, None) => return Err(JsError::new("RECOMPUTE")),
        };
        if self.flat {
            inner = flat_finish(inner, self.curve, self.flat_slice);
        }
        let end = timing::now_ms();
        record_stats(&mut inner, start, end, end);
        Ok(self.result(inner, field, solid && !self.flat, None))
    }

    /// 回す前の形で作った陰関数で、2D 表示でないなら true。メッシュを作り直さず、描画側で
    /// [`Shape::local_mesh`] のメッシュに [`Shape::transform`] の 1 次式を掛けて描ける
    #[wasm_bindgen(getter)]
    pub fn rigid(&self) -> bool {
        self.local.is_some() && !self.flat
    }

    /// 回す前の形のメッシュ（[`Shape::rigid`] のときだけ）
    pub fn local_mesh(&self) -> Option<MeshResult> {
        let l = self.local.as_ref().filter(|_| !self.flat)?;
        let space = equation::Space::basic(l.dims, l.axes, vec![0.0; l.dims]);
        Some(self.result(l.project(&space.view(0.0)), None, false, None))
    }

    /// 回転だけを変えたときの、頂点に掛ける 1 次式（結果の `transform`）と切り口のふたの元（`take_field`）。メッシュは空。
    /// 引数の意味は [`Shape::project`] と同じ（遠近法は使わない）。回せないときは "RECOMPUTE" のエラー
    pub fn transform(&self, domain: &[f64], dims: u32, axes: &[u32], rotation: &[f64], center: &[f64]) -> Result<MeshResult, JsError> {
        let start = timing::now_ms();
        timing::reset();
        let Some(l) = self.local.as_ref().filter(|_| !self.flat) else {
            return Err(JsError::new("RECOMPUTE"));
        };
        let (view, dom) = self.view_for(domain, dims, axes, rotation, center, 0.0)?;
        let (m, t) = l.affine(&view);
        let tr = [m[0][0], m[0][1], m[0][2], t[0], m[1][0], m[1][1], m[1][2], t[1], m[2][0], m[2][1], m[2][2], t[2]];
        let mut inner = mesh::Mesh::default();
        let end = timing::now_ms();
        record_stats(&mut inner, start, end, end);
        Ok(self.result(inner, Some((vec![l.field(&view, dom)], dom)), false, Some(tr)))
    }

    /// 結果の入れ物（式の説明などは、この形のものを使う）
    fn result(
        &self,
        inner: mesh::Mesh,
        field: Option<(Vec<mesh::Field>, mesh::Domain)>,
        solid: bool,
        transform: Option<[f64; 12]>,
    ) -> MeshResult {
        MeshResult {
            inner,
            flat: self.flat,
            flat_slice: self.flat_slice,
            outline: Default::default(),
            value_range: None,
            volume: None,
            solid,
            field,
            shape: None,
            local: None,
            transform,
            explicit_used: None,
            kind: self.kind.clone(),
            params: self.params.clone(),
            curve: self.curve || self.flat_slice,
            shape_curve: self.curve,
            used_params: self.used_params.clone(),
            param: None,
            local_available: false,
        }
    }
}

impl Shape {
    /// 回転・射影の設定と表示範囲。回した陽関数・回す前の形で作った陰関数で、回転が条件を外れたら "RECOMPUTE" のエラー
    fn view_for(
        &self,
        domain: &[f64],
        dims: u32,
        axes: &[u32],
        rotation: &[f64],
        center: &[f64],
        perspective: f64,
    ) -> Result<(mesh::View, mesh::Domain), JsError> {
        let n = dims as usize;
        let rn = equation::rot_dims(n);
        let dims_of = self.inner.as_ref().map_or_else(|| self.local.as_ref().map_or(0, |l| l.dims), |s| s.dims);
        if domain.len() != 6 || axes.len() != 3 || rotation.len() != rn * (rn - 1) / 2 || n != dims_of {
            return Err(JsError::new("内部エラー: 射影の指定が合っていません"));
        }
        let mut space = equation::Space::basic(n, [axes[0] as usize, axes[1] as usize, axes[2] as usize], vec![0.0; n]);
        space.rotation = rotation.to_vec();
        space.center = center.to_vec();
        if let Some(used) = &self.explicit_used {
            // 回した陽関数は、回転が無くなったら回さない陽関数として作り直す（陰関数は、回転が 0 でも回す前の形のまま描ける）
            let zero = self.inner.is_some() && space.rotation.iter().all(|&a| a == 0.0);
            if zero || !space.rotation_keeps_shown(used) {
                return Err(JsError::new("RECOMPUTE"));
            }
        }
        let dom = mesh::Domain { lo: [domain[0], domain[2], domain[4]], hi: [domain[1], domain[3], domain[5]] };
        Ok((space.view(perspective), dom))
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
            local_available: false,
            shape: None,
            explicit_used: None,
            local: None,
            transform: None,
            field: None,
            solid: false,
            flat: false,
            flat_slice: false,
            outline: Default::default(),
            value_range: None,
            volume: None,
        })
    }
}

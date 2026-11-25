//! グラフソフトの計算エンジン。
//!
//! - `parser`    : 数式の文字列 → 構文木
//! - `eval`      : 構文木 → 命令列、多数の点でまとめて計算（複素数）
//! - `interval`  : 区間演算（箱の上で式の値が入る範囲。曲面の無い所を飛ばすのに使う）
//! - `functions` : 組み込み関数の一覧
//! - `special`   : 特殊関数の実装
//! - `expand`    : 数学の記法（関数の定義・Σ・微分・行列など）を今までの式に直す（展開の段）
//! - `equation`  : 式の種類（陽関数・陰関数）の判定
//! - `mesh`      : 曲面のメッシュ生成
//!
//! このファイルは、JavaScript から呼び出す関数（WASM の入口）だけを置く。

pub mod adaptive;
pub mod coords;
pub mod accurate;
pub mod critical;
pub mod diff;
pub mod equation;
pub mod eval;
pub mod expand;
pub mod field;
pub mod integrate;
pub mod logic;
pub mod functions;
pub mod gpu2d;
pub mod interval;
pub mod linalg;
pub mod mesh;
pub mod ode;
pub mod pixel;
pub mod groebner;
pub mod quad;
pub mod breakpoints;
pub mod series;
mod holes;
mod limit;
pub mod solve;
mod region;
pub mod implicit;
pub mod parser;
pub mod special;
pub mod system;
mod set_image;
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
    /// 実線の点ごとの値（ベクトル場の流線の大きさ）。ふつうは空
    outline_values: Vec<f32>,
    /// 行列の列ベクトルや固有ベクトル。実線は矢印の軸。
    vector_arrows: bool,
    /// ベクトル場なら true（描画側で鮮やかな色の対応表を使う）
    vivid: bool,
    /// 表示していない座標の値で色を付けるなら、その値の範囲（[`hidden_style`]）。値は頂点ごとの complex の 1 つ目
    value_range: Option<(f64, f64)>,
    /// 伝達関数で描くなら、格子（1 辺 n 点。x が最も速く変わる順）での値と n（[`hidden_style`]）。
    /// value_range が無ければ、不等式の領域の霧（値は G = max G_i。G ≤ 0 が中身）
    volume: Option<(Vec<f32>, usize)>,
    /// 座標系（[`coord_system`]）。回転だけ変わったときの射影し直し（[`Shape::project`]）でも同じ写像をかける
    coord: Option<coords::CoordMap>,
    /// 座標系があるとき、`outline` の 2 本の線を移した先
    outline_target: (Vec<f32>, Vec<f32>),
    /// 添えの線・点（面・領域・曲線と一緒に描く、次元の低い部分。#120 #121 #175）。線分ごとに 2 点と NaN、点は同じ点 2 つと NaN。
    /// GPU で画素ごとに描くときも、メッシュで重ねて描く
    extra: Vec<f32>,
    extra_target: Vec<f32>,
    /// 展開で使った定義の名前と、この行が定義の行ならその名前（[`definitions`]）
    defs: (Vec<String>, Option<String>),
    /// 2D 表示の曲線・不等式の領域を、画素ごとに描くための GLSL（計画 12 章の G4。GPU に移せない式・座標系を使う式では None）
    gpu2d: Option<gpu2d::Scene2d>,
    /// 3D の陰関数を光線で描くためのもの（計画 12 章の G5・G7）。GPU に移せない式では None
    gpu3d: Option<Gpu3d>,
    /// GPU の描き方を使えない理由（画面の ⚙ に出す）。使えるなら空
    gpu_reason: String,
    /// 常微分方程式なら、状態の成分の名前（`x`・`y'`。初期値の欄の見出し）
    ode_states: Vec<String>,
    /// 常微分方程式なら、独立変数の名前と、解いた初期時刻（#131。初期時刻の欄の見出し・前の保存データの読み込み）
    ode_var: String,
    ode_t0: f64,
    /// 除外点と有限の極限の位置（点ごとに 3 つ）。描画側で白丸を描く。
    holes: Vec<f32>,
    holes_target: Vec<f32>,
    /// 回転前の白丸。再射影時に図形と一緒に動かす。
    source_holes: Vec<f64>,
    source_dashed: Vec<f64>,
    /// 極限の行の数値結果と、近づく点での定義値を示す点の座標
    limit: Option<LimitMesh>,
}

struct LimitMesh {
    report: limit::Report,
    source: Vec<f32>,
    source_target: Vec<f32>,
    point: Vec<f32>,
    point_target: Vec<f32>,
}

/// 3D の陰関数を光線で描くための GLSL と、uniform の値
#[derive(Clone)]
struct Gpu3d {
    /// gR・gRd・gRi と下請けの関数。スライダーのパラメータは uniform の配列 uParam
    glsl: String,
    /// uParam の順のパラメータの名前と、今の値
    params: Vec<String>,
    values: Vec<f64>,
    /// 表示中の座標 q から F の座標（元の座標 x, y, z, w, …）P への 1 次式 P = A q + b（座標ごとに [A_k0, A_k1, A_k2, b_k]）
    affine: Vec<f64>,
    /// 表示中の 3 つの座標の番号（回転だけが変わった結果の 1 次式 q = M p + t で、その行を書き換えるため）
    axes: [usize; 3],
}

#[wasm_bindgen]
impl MeshResult {
    /// 3D の陰関数を光線で描くための GLSL（[`MeshResult`] の `gpu3d`）。無ければ空
    pub fn gpu3d_glsl(&self) -> String {
        self.gpu3d.as_ref().map_or(String::new(), |g| g.glsl.clone())
    }
    /// 同じく、uniform の配列 uParam の順のパラメータの名前（カンマ区切り）
    pub fn gpu3d_params(&self) -> String {
        self.gpu3d.as_ref().map_or(String::new(), |g| g.params.join(","))
    }
    /// 同じく、パラメータの今の値
    pub fn gpu3d_values(&self) -> Vec<f64> {
        self.gpu3d.as_ref().map_or(Vec::new(), |g| g.values.clone())
    }
    /// 同じく、表示中の座標から F の座標への 1 次式（座標ごとに [A_k0, A_k1, A_k2, b_k]。座標の数 × 4 個）
    pub fn gpu3d_affine(&self) -> Vec<f64> {
        self.gpu3d.as_ref().map_or(Vec::new(), |g| g.affine.clone())
    }
    /// 同じく、表示中の 3 つの座標の番号
    pub fn gpu3d_axes(&self) -> Vec<u32> {
        self.gpu3d.as_ref().map_or(Vec::new(), |g| g.axes.map(|a| a as u32).to_vec())
    }
    /// GPU の描き方を使えない理由。使えるなら空
    #[wasm_bindgen(getter)]
    pub fn gpu_reason(&self) -> String {
        self.gpu_reason.clone()
    }
    /// 画素ごとに描く 2D の図形の GLSL（[`gpu2d`]）。無ければ空
    pub fn gpu2d_glsl(&self) -> String {
        self.gpu2d.as_ref().map_or(String::new(), |g| g.glsl.clone())
    }
    /// 同じく、画素の座標から式の座標への 1 次式（座標ごとに [A_k0, A_k1, A_k2, b_k]。パラメータを畳み込んだ式なら空）
    pub fn gpu2d_affine(&self) -> Vec<f64> {
        self.gpu2d.as_ref().map_or(Vec::new(), |g| g.affine.clone())
    }
    /// 同じく、uniform にしたパラメータの名前（カンマ区切り）と、今の値
    pub fn gpu2d_params(&self) -> String {
        self.gpu2d.as_ref().map_or(String::new(), |g| g.params.join(","))
    }
    pub fn gpu2d_values(&self) -> Vec<f64> {
        self.gpu2d.as_ref().map_or(Vec::new(), |g| g.values.clone())
    }
    /// 同じく [等式の数, 不等式の数, 実線を引く不等式のビット]。無ければ空
    pub fn gpu2d_info(&self) -> Vec<u32> {
        self.gpu2d.as_ref().map_or(Vec::new(), |g| vec![g.eq as u32, g.cuts as u32, g.lines])
    }
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
    /// 座標系で移した先 T(q)（頂点ごとに 3 つ。Float32Array）を取り出す。座標系が無ければ空
    pub fn take_target(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.inner.target)
    }
    /// 自分の頂点を持つ粗い版の、座標系で移した先を取り出す（無ければ空）
    pub fn take_lod_target(&mut self) -> Vec<f32> {
        self.inner.lod_mesh.as_mut().map(|m| std::mem::take(&mut m.target)).unwrap_or_default()
    }
    /// 2D 表示の不等式の領域の境目の線（実線）を、座標系で移した先（座標系が無ければ空）
    pub fn take_outline_target(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.outline_target.0)
    }
    /// 同じく点線の分
    pub fn take_outline_dashed_target(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.outline_target.1)
    }
    /// 添えの線・点（Float32Array。無ければ空）。面・曲線と同じ色で、GPU で描くときも重ねて描く
    pub fn take_extra(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.extra)
    }
    /// 同じく、座標系で移した先（座標系が無ければ空）
    pub fn take_extra_target(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.extra_target)
    }
    /// 実線の点ごとの値（Float32Array。ベクトル場の流線の大きさ。無ければ空）
    pub fn take_outline_values(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.outline_values)
    }
    #[wasm_bindgen(getter)]
    pub fn vector_arrows(&self) -> bool {
        self.vector_arrows
    }
    /// ≠ で除いた点（Float32Array。点ごとに 3 つ。無ければ空）。描画側で白丸を描く
    pub fn take_holes(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.holes)
    }

    pub fn limit_variable(&self) -> String {
        self.limit.as_ref().map_or_else(String::new, |m| m.report.variable.clone())
    }

    pub fn limit_sequence(&self) -> bool {
        self.limit.as_ref().is_some_and(|m| m.report.sequence)
    }

    pub fn take_limit_samples(&mut self) -> Vec<f64> {
        self.limit.as_mut().map_or_else(Vec::new, |m| std::mem::take(&mut m.report.lesson).into_iter().flatten().collect())
    }

    /// [近づく先, 向き, 左の状態と値, 右の状態と値, 点での値, 指定した向きの極限値]
    pub fn limit_values(&self) -> Vec<f64> {
        let Some(LimitMesh { report: r, .. }) = &self.limit else { return Vec::new() };
        let [ls, lv] = r.left.pair();
        let [rs, rv] = r.right.pair();
        vec![r.target, f64::from(r.side), ls, lv, rs, rv, r.defined.unwrap_or(f64::NAN), r.value().unwrap_or(f64::NAN)]
    }

    pub fn take_limit_point(&mut self) -> Vec<f32> {
        self.limit.as_mut().map_or_else(Vec::new, |m| std::mem::take(&mut m.point))
    }

    pub fn take_limit_source(&mut self) -> Vec<f32> {
        self.limit.as_mut().map_or_else(Vec::new, |m| std::mem::take(&mut m.source))
    }

    pub fn take_limit_source_target(&mut self) -> Vec<f32> {
        self.limit.as_mut().map_or_else(Vec::new, |m| std::mem::take(&mut m.source_target))
    }

    pub fn take_holes_target(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.holes_target)
    }

    pub fn take_limit_holes_target(&mut self) -> Vec<f32> {
        if self.limit.is_some() { self.take_holes_target() } else { Vec::new() }
    }

    pub fn take_limit_point_target(&mut self) -> Vec<f32> {
        self.limit.as_mut().map_or_else(Vec::new, |m| std::mem::take(&mut m.point_target))
    }
    /// ベクトル場か（矢印・流線を鮮やかな色の対応表で塗る）
    #[wasm_bindgen(getter)]
    pub fn vivid(&self) -> bool {
        self.vivid
    }
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
    /// この式が使った定義（関数 f など。[`definitions`]）の名前（カンマ区切り）
    /// 常微分方程式の状態の成分の名前（カンマ区切り。常微分方程式でなければ空）
    #[wasm_bindgen(getter)]
    pub fn ode_states(&self) -> String {
        self.ode_states.join(",")
    }

    /// 常微分方程式の独立変数の名前（常微分方程式でなければ空）
    #[wasm_bindgen(getter)]
    pub fn ode_var(&self) -> String {
        self.ode_var.clone()
    }

    /// 常微分方程式を解いた初期時刻（常微分方程式でなければ NaN）
    #[wasm_bindgen(getter)]
    pub fn ode_t0(&self) -> f64 {
        self.ode_t0
    }

    #[wasm_bindgen(getter)]
    pub fn used_defs(&self) -> String {
        self.defs.0.join(",")
    }
    /// 定義の行（`f(x) = …`）なら、その名前（そうでなければ空）
    #[wasm_bindgen(getter)]
    pub fn def_name(&self) -> String {
        self.defs.1.clone().unwrap_or_default()
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
            source_holes: std::mem::take(&mut self.source_holes),
            source_dashed: std::mem::take(&mut self.source_dashed),
            local: local.map(|l| l.0),
            flat: self.flat,
            flat_slice: self.flat_slice,
            explicit_used,
            kind: self.kind.clone(),
            params: self.params.clone(),
            curve: self.shape_curve,
            used_params: self.used_params.clone(),
            coord: self.coord.clone(),
            gpu3d: self.gpu3d.clone(),
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
    /// 自分の頂点を持つ粗い版の、頂点ごとの色分けの値（complex）を取り出す（無い・色分けしないなら空）
    pub fn take_lod_complex_values(&mut self) -> Vec<f32> {
        self.inner.lod_mesh.as_mut().map(|m| std::mem::take(&mut m.complex)).unwrap_or_default()
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

/// 座標系の格子（座標曲線。計画 9 章の S39）。`map`・`names` は [`coord_system`] と同じ、`ranges` は作る範囲
/// [x の最小, 最大, y …, z …]、`param_names`・`param_values` は [`plot`] と同じパラメータ。`lines` は範囲を何等分した所に線を引くか、
/// `faces` は線を引く面（ビット a: 座標 a が範囲の最小の面）、`flat` は 2D 表示（縦の座標 0 の平面に引く）。
/// 結果は曲線: `take_positions` が元の位置（座標の値）、`take_target` が座標系で移した先。描画側が移行の値で混ぜる
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn coord_grid(
    map: &str,
    names: &str,
    ranges: &[f64],
    param_names: &str,
    param_values: &[f64],
    lines: u32,
    faces: u32,
    flat: bool,
) -> Result<MeshResult, JsError> {
    let names_v: Vec<String> = names.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let pn: Vec<&str> = param_names.split(',').filter(|s| !s.is_empty()).collect();
    if ranges.len() != 6 || param_values.len() != 2 * pn.len() {
        return Err(JsError::new("内部エラー: 座標系の格子の指定が合っていません"));
    }
    let consts: eval::Params =
        pn.iter().enumerate().map(|(k, n)| (n.to_string(), num_complex::Complex64::new(param_values[2 * k], param_values[2 * k + 1]))).collect();
    let mut space = equation::Space::standard();
    space.coord_names = <[String; 3]>::try_from(names_v).ok();
    // 座標系の式は、定義した行列・関数を使える（[`definitions`]）
    let _defs = take_definitions(&consts);
    let c = equation::compile_coord_map(map, &consts, &space).map_err(|e| JsError::new(&e))?;
    let dom = mesh::Domain { lo: [ranges[0], ranges[2], ranges[4]], hi: [ranges[1], ranges[3], ranges[5]] };
    let mut positions = coords::grid_lines(dom, lines as usize, faces, flat);
    let target = c.apply_line(&mut positions, flat);
    let inner = mesh::Mesh { positions, target, ..Default::default() };
    Ok(MeshResult {
        inner,
        kind: String::new(),
        params: Vec::new(),
        curve: true,
        shape_curve: true,
        used_params: Vec::new(),
        param: None,
        local_available: false,
        shape: None,
        explicit_used: None,
        local: None,
        transform: None,
        field: None,
        solid: false,
        flat,
        flat_slice: false,
        outline: Default::default(),
        outline_values: Vec::new(),
        vector_arrows: false,
        vivid: false,
        outline_target: Default::default(),
        extra: Vec::new(),
        extra_target: Vec::new(),
        coord: None,
        value_range: None,
        volume: None,
        defs: Default::default(),
        gpu2d: None,
        gpu3d: None,
        gpu_reason: String::new(),
        ode_states: Vec::new(),
        ode_var: String::new(),
        ode_t0: f64::NAN,
        holes: Vec::new(),
        holes_target: Vec::new(),
        source_holes: Vec::new(),
        source_dashed: Vec::new(),
        limit: None,
    })
}

/// 式に誤りがないかだけを調べる。誤りがあればその説明を投げる。
#[wasm_bindgen]
pub fn check_expression(source: &str) -> Result<(), JsError> {
    let _defs = take_definitions(&eval::Params::new());
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
    let _defs = take_definitions(&consts);
    let (v, mut used) = equation::eval_constant_with(source, &consts).map_err(|e| JsError::new(&e))?;
    for p in expand::used_params() {
        if !used.contains(&p) {
            used.push(p);
        }
    }
    LAST_USED.with(|u| *u.borrow_mut() = used.join(","));
    LAST_USED_DEFS.with(|u| *u.borrow_mut() = expand::used().join(","));
    Ok(v)
}

thread_local! {
    static LAST_USED: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    /// 直前の [`eval_range`] で使った定義の名前（[`last_used_defs`]）
    static LAST_USED_DEFS: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    /// 次の [`plot`]・[`check_expression`]・[`eval_range`] だけに効く、定義の行（[`definitions`]）
    static DEFINITIONS: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// 次の [`plot`]・[`check_expression`]・[`eval_range`] で使う定義（その 1 回だけ。計画 `docs/plan-math-notation.md` の M0）。
/// `src` は式の一覧の行（計算エンジン用の文字列）を `\n` でつないだもの。定義の行（`f(x) = …`・`h = f + g`）だけが使われ、
/// ほかの行は無視される。呼ばなければ定義なし
#[wasm_bindgen]
pub fn definitions(src: &str) {
    DEFINITIONS.with(|d| *d.borrow_mut() = src.to_string());
}

/// [`definitions`] で渡された定義を取り出して入れる（戻り値を捨てるまで）
fn take_definitions(params: &eval::Params) -> expand::Installed {
    let src = DEFINITIONS.with(|d| std::mem::take(&mut *d.borrow_mut()));
    expand::install(expand::Defs::parse(&src, params))
}

/// 直前の `eval_range` で使った定義の名前（カンマ区切り。範囲の式 [0, f(2)] の f。定義が変わったら描き直すため。#191 #208）
#[wasm_bindgen]
pub fn last_used_defs() -> String {
    LAST_USED_DEFS.with(|u| u.borrow().clone())
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
const FLAG_SEQUENCE_TABLE: u32 = 32;

thread_local! {
    /// 最後の [`plot`] で求めた注目する点（[`last_poi`]）
    static LAST_POI: std::cell::RefCell<Vec<f64>> = const { std::cell::RefCell::new(Vec::new()) };
    static LAST_SEQUENCE_TABLE: std::cell::RefCell<Vec<f64>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// 最後の [`plot`] で求めた、陽関数の注目する点（極値・零点・縦軸との交点・停留点）。
/// [点の 3 つの座標, 種類, …]。種類は 0: 極大, 1: 極小, 2: 鞍点, 3: 零点, 4: 縦軸との交点, 5: そのほかの停留点。
/// `plot` の `flags` のビット 4 を立てたときだけ求める（無ければ空）
#[wasm_bindgen]
pub fn last_poi() -> Vec<f64> {
    LAST_POI.with(|p| p.borrow().clone())
}

/// 最後の描画で要求した数列表を、項ごとの [添字, 実部, 虚部, 規則へ渡した添字] として返す。
#[wasm_bindgen]
pub fn take_sequence_table() -> Vec<f64> {
    LAST_SEQUENCE_TABLE.with(|p| std::mem::take(&mut *p.borrow_mut()))
}

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
    let _field_style = FieldStyleReset;
    let complex_coords = COMPLEX_COORDS.with(|c| c.replace(false));
    let (coord_src, coord_names) = COORD_MAP.with(|c| std::mem::take(&mut *c.borrow_mut()));
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
        complex_coords,
        coord_names: None,
    };
    if space.axes.iter().any(|&a| a >= space.dims) {
        return Err(err("内部エラー: 表示する座標の番号が次元を超えています"));
    }
    let mut space = space;
    let names: Vec<String> = coord_names.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    if let Ok(names) = <[String; 3]>::try_from(names) {
        space.coord_names = Some(names);
    }

    let _defs = take_definitions(&consts);
    LAST_POI.with(|p| p.borrow_mut().clear());
    LAST_SEQUENCE_TABLE.with(|p| p.borrow_mut().clear());
    let _ = equation::take_last_explicit();
    expand::set_ode_init(ODE_INIT_SRC.with(|s| std::mem::take(&mut *s.borrow_mut())).split('\n').map(String::from).collect());
    expand::set_ode_start_from(ODE_START_SRC.with(|s| std::mem::take(&mut *s.borrow_mut())));
    // 原点を通る直線・平面（ker・Im）を、表示範囲の箱を覆う長さで作るための、原点から箱のいちばん遠い角までの距離
    expand::set_view_x(domain[0], domain[1]);
    expand::set_view_box([domain[0], domain[1], domain[2], domain[3], domain[4], domain[5]]);
    expand::set_reach((0..3).map(|a| domain[2 * a].abs().max(domain[2 * a + 1].abs()).powi(2)).sum::<f64>().sqrt());
    let plot = equation::compile_with(source, &consts, &space).map_err(|e| JsError::new(&e))?;
    if flags & FLAG_SEQUENCE_TABLE != 0 {
        if let equation::Plot::Points { sequence: Some(sequence), .. } = &plot {
            LAST_SEQUENCE_TABLE.with(|p| *p.borrow_mut() = sequence.table(&consts));
        }
    }
    // 注目する点（陽関数の極値・零点など。点を押すと値が出る機能）: 画面側が頼んだとき（ビット 4。動かしている間は頼まない）
    if flags & 16 != 0 && coord_src.trim().is_empty() {
        if let (equation::Plot::Explicit { rotated: false, .. }, Some((axis, e, shown))) = (&plot, equation::take_last_explicit()) {
            let d = [domain[0], domain[1], domain[2], domain[3], domain[4], domain[5]];
            let poi = critical::points_of_interest(axis, &e, &shown, &d, &consts, flat);
            LAST_POI.with(|p| *p.borrow_mut() = poi);
        }
    }
    if !coord_src.trim().is_empty() && matches!(plot, equation::Plot::Field { .. }) {
        return Err(err("座標系を選んだ式では、ベクトル場は描けません（成分をどの向きで読むかが決まらないため）"));
    }
    let coord = if coord_src.trim().is_empty() {
        None
    } else {
        Some(equation::compile_coord_map(&coord_src, &consts, &space).map_err(|e| JsError::new(&e))?)
    };
    // 「縦を媒介変数にする」は複素平面の図形だけの設定。陽関数では色分けしない
    if complex_mode == mesh::ComplexMode::Param {
        complex_mode = mesh::ComplexMode::Off;
    }
    let dom = mesh::Domain { lo: [domain[0], domain[2], domain[4]], hi: [domain[1], domain[3], domain[5]] };
    let size = (0..3).map(|a| (dom.hi[a] - dom.lo[a]).powi(2)).sum::<f64>().sqrt();
    // 媒介変数の範囲（範囲で重ねるパラメータならスライダーの範囲。指定が無ければ 0〜2π）
    let range = |name: &str| {
        // 集合の条件で決めた媒介変数の範囲（{(cos t, sin t) | 0 ≤ t ≤ π}）が最優先
        if let Some(r) = expand::param_range(name) {
            return r;
        }
        space
            .param_ranges
            .iter()
            .chain(range_of.iter())
            .find(|(n, _, _)| n == name)
            .map(|(_, a, b)| (*a, *b))
            .unwrap_or((0.0, std::f64::consts::TAU))
    };
    let ctx = Ctx {
        params: &consts,
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
        region_fog: !flat && hidden.first().is_some_and(|&m| m == 6.0),
    };
    // 表示していない座標の見せ方（2D 表示では、今までどおり断面）
    let hidden_on = !flat && hidden.first().is_some_and(|&m| m >= 1.0);
    if coord.is_some() && hidden_on && hidden.first().is_some_and(|&m| m == 3.0 || m == 5.0 || m == 6.0) {
        return Err(err("座標系を使う式では、伝達関数・霧の見せ方は使えません"));
    }
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
    // 表示範囲の箱の中だけを残す図形（ker・Im の直線・平面。計画 M7）。回転が変わったら作り直すので、射影する前の形は返さない
    let trimmed = expand::trim_requested();
    if trimmed && coord.is_none() {
        coords::trim_to_domain(&mut inner, dom, curve, flat);
    }
    let mut outline = built.outline;
    let vector_arrows = matches!(plot, equation::Plot::Arrows { .. });
    // 2D の投影で、縦に立った面の代わりに引く線（mesh::flatten）
    outline.0.append(&mut inner.edge_lines);
    // ≠ で除いた所: 曲線なら白丸の点、面・領域なら点線（mark_excluded）
    let excluded = expand::take_excluded();
    let mut holes = built.holes;
    let mut source_holes = built.source_holes;
    let mut source_dashed = built.source_dashed;
    let explicit_surface = match (&plot, &built.shape) {
        (equation::Plot::Explicit { names, rotated: true, .. }, Some(shape)) if !curve => Some((names, shape)),
        _ => None,
    };
    let mut fully_excluded = false;
    if !excluded.is_empty() && coord.is_none() {
        if let Some((names, shape)) = explicit_surface {
            // 回転前の曲面で除外線を求め、再射影でも同じ線を使う。
            let params = space.with_hidden(&consts);
            let names = names.each_ref().map(String::as_str);
            let guards: Vec<_> = excluded.iter()
                .filter_map(|(expr, _)| eval::Program::compile_with(expr, &names, &params).ok()).collect();
            let local = mesh::Mesh {
                positions: shape.points.chunks_exact(n).flat_map(|p| space.axes.map(|a| p[a] as f32)).collect(),
                indices: shape.tris.as_ref().map_or_else(Vec::new, |(_, tris)| tris.clone()),
                ..Default::default()
            };
            let (_, dashed, _) = mark_excluded(&local, false, guards.iter());
            source_dashed.extend(line_in_axes(&dashed, &space));
        } else {
            // 投影後の点に、投影前の高さを使う条件は適用しない。
            let direct = excluded.iter().map(|(_, p)| p).filter(|p| {
                !flat || ctx.flat_slice || (curve && !p.uses_var_index(2))
            });
            let (h, dashed, all) = mark_excluded(&inner, curve, direct);
            holes.extend(h);
            outline.1.extend(dashed);
            if all {
                // 曲線全体が除かれた（y = x, z ≠ 0 の 2D の切り口 z = 0）: 何も描かない
                inner = mesh::Mesh::default();
                holes.clear();
                fully_excluded = true;
                expand::add_note("≠ で全部除かれる");
            }
        }
    }
    if let equation::Plot::Explicit { axis, names, prog, expr, rotated } = &plot {
        if !hidden_on {
            let params = space.with_hidden(&consts);
            let marks = if *rotated && built.curve {
                let ext = rotated_domain(&space, dom);
                rotated_curve_exclusions(expr, prog, names, *axis, &params, ext, &excluded, built.shape.as_ref(), &space, ctx.flat_slice)
            } else if *rotated {
                holes::surface(expr, prog, names, *axis, &params, rotated_domain(&space, dom), &excluded)
            } else if !built.curve || *axis == 2 {
                holes::surface(expr, prog, names, *axis, &params, dom, &excluded)
            } else if !fully_excluded && (ctx.flat_slice || !prog.uses_var_index(1)) {
                holes::curve_exclusions(expr, prog, names, *axis, &params, dom, &excluded, ctx.flat_slice)
            } else {
                Vec::new()
            };
            if *rotated {
                source_holes.extend(mesh::points_in_axes(marks, space.axes, &space.slice));
            } else {
                append_visible_holes(&mut holes, marks, dom, flat, ctx.flat_slice, ctx.size);
            }
        }
    } else if let equation::Plot::Implicit { slice_source: Some(source), .. } = &plot {
        if let equation::Plot::Explicit { axis, names, prog, expr, .. } = source.as_ref() {
            let params = space.with_hidden(&consts);
            let ext = rotated_domain(&space, dom);
            let marks = holes::surface(expr, prog, names, *axis, &params, ext, &excluded);
            let local = mesh::points_in_axes(marks, space.axes, &space.slice);
            let mut exact = Vec::new();
            append_visible_holes(&mut exact, local.chunks_exact(n).map(|p| ctx.view.apply(p)), dom, flat, true, ctx.size);
            if ctx.complex_mode == mesh::ComplexMode::Off {
                let lines = holes::extruded_lines(expr, prog, names, *axis, &params, ext, false);
                let positions = line_in_axes(&lines, &space);
                let projected: Vec<_> = positions.chunks_exact(n).map(|p| ctx.view.apply(p)).collect();
                let intersections = projected.windows(2).filter_map(|p| {
                    let [a, b] = [p[0], p[1]];
                    if !a.iter().chain(&b).all(|v| v.is_finite())
                        || a[2].abs().max(b[2].abs()) <= ctx.size * 1e-9
                        || (a[2] < 0.0 && b[2] < 0.0) || (a[2] > 0.0 && b[2] > 0.0)
                    {
                        return None;
                    }
                    let t = a[2] / (a[2] - b[2]);
                    Some([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1]), 0.0])
                });
                append_visible_holes(&mut exact, intersections, dom, flat, true, ctx.size);
            }
            if !exact.is_empty() {
                // A tessellated slice can have crossings on both sides of the same tangent point.
                let tolerance = (ctx.size * 1e-6) as f32;
                for p in holes.chunks_exact(3) {
                    if !exact.chunks_exact(3).any(|q| (0..3).all(|i| (p[i] - q[i]).abs() <= tolerance)) {
                        exact.extend_from_slice(p);
                    }
                }
                holes = exact;
            }
        }
    }
    if !fully_excluded {
        let marks = source_holes.chunks_exact(n).map(|p| ctx.view.apply(p));
        append_visible_holes(&mut holes, marks, dom, flat, ctx.flat_slice && !built.curve, ctx.size);
        if !curve {
            outline.1.extend(project_dashed(&source_dashed, n, &ctx.view, dom, flat));
        }
    }
    let limit = if let equation::Plot::Limit { report, .. } = &plot {
        let (open, mut point) = report.marks();
        let mut source = built.limit_source;
        let source_target = coord.as_ref().map_or_else(Vec::new, |c| c.apply_line(&mut source, flat));
        let point_target = coord.as_ref().map_or_else(Vec::new, |c| c.apply_line(&mut point, flat));
        holes.extend(open);
        Some(LimitMesh { report: report.clone(), source, source_target, point, point_target })
    } else {
        None
    };
    // 3D の面: 表示範囲の箱で切れた縁を線にする（真横を向いた面も見える。mesh::box_edge_lines）。
    // 流線（点ごとの値つきの線）・座標系があるときは足さない
    if !flat && !curve && !vector_arrows && coord.is_none() && built.outline_values.is_empty() {
        outline.0.extend(mesh::box_edge_lines(&inner, dom));
    }
    let mut outline_target = (Vec::new(), Vec::new());
    let mut holes_target = Vec::new();
    let mut extra = built.extra;
    let has_extra = !extra.is_empty();
    if flat {
        for p in extra.chunks_exact_mut(3).filter(|p| p[0].is_finite()) {
            p[2] = 0.0;
        }
    }
    let mut extra_target = Vec::new();
    if let Some(c) = &coord {
        extra_target = c.apply_line(&mut extra, flat);
        // 作る範囲からはみ出した所を落とす（媒介変数表示は、範囲が媒介変数で決まるので切らない）
        let parametric = built.shape.is_some() && !built.rotated_explicit;
        if !parametric {
            coords::trim_to_domain(&mut inner, dom, curve, flat);
        }
        c.apply(&mut inner, curve, flat);
        outline_target = (c.apply_line(&mut outline.0, flat), c.apply_line(&mut outline.1, flat));
        holes_target = c.apply_line(&mut holes, flat);
    }
    // 3D の陰関数（回した陽関数を陰関数にしたものも）を光線で描くための GLSL と、使えないならその理由
    let gpu3d = match &built.gpu3d_off {
        Some(reason) => (None, reason.clone()),
        None => gpu3d_of(&plot, &consts, &space, flat, hidden_on, coord.is_some(), built.gpu2d.is_some()),
    };
    let lod_start = timing::now_ms();
    if !curve {
        mesh::add_cluster_lod(&mut inner);
    }
    record_stats(&mut inner, start, lod_start, timing::now_ms());
    // 回転した陽関数は、表示する軸・断面・中心が変わると形が変わる。回転がその条件を満たすかも確かめる
    let explicit_used = (built.shape.is_some() && built.rotated_explicit).then(|| plot.used_params());
    // 表示中の形だけから求めた除外印は、元の形を使い回すときも計算し直す。
    let uncached_exclusion = coord.is_none() && !excluded.is_empty()
        && !matches!(plot, equation::Plot::Explicit { rotated: true, .. });
    Ok(MeshResult {
        inner,
        flat,
        flat_slice: ctx.flat_slice,
        outline,
        outline_target,
        extra,
        extra_target,
        outline_values: built.outline_values,
        vector_arrows,
        vivid: built.vivid,
        value_range: built.value_range,
        volume: built.volume,
        // 座標系を使う式では、切り口のふたの元と立体の印を返さない（座標の値の空間の平らなふたは、移した後の切り口に合わない）
        solid: built.shape.as_ref().is_some_and(|s| s.solid) && !flat && coord.is_none(),
        // 添えの線・点のある式は、回転だけ変わったときも作り直す（射影し直す形を返さない）
        shape: if trimmed || uncached_exclusion || fully_excluded || has_extra {
            None
        } else {
            built.shape
        },
        local: if coord.is_some() || uncached_exclusion { None } else { built.local },
        transform: None,
        field: (!built.fields.is_empty() && coord.is_none()).then_some((built.fields, dom)),
        explicit_used,
        kind: {
            let mut k = expand::kind_label()
                .or_else(expand::def_line_label)
                .unwrap_or_else(|| equation::rename_kind(&built.kind, &space));
            if expand::approximate() {
                k.push_str("（積分は近似）");
            }
            for n in expand::notes() {
                k.push_str(&format!("（{n}）"));
            }
            k
        },
        // 範囲を式の側で決めた媒介変数（集合の条件・ker・行列式の立体）は、範囲の欄を出さない
        params: plot.param_vars().into_iter().filter(|n| expand::param_range(n).is_none()).collect(),
        curve,
        shape_curve: built.curve,
        // Σ の範囲などで使ったパラメータは、展開した式に残らないので足す（値が変わったら計算し直すため）
        used_params: {
            let mut u = plot.used_params();
            // 座標系の式が使うパラメータ（行列を座標系にしたときの、成分のスライダーなど）
            let exclusion_params = excluded.iter().flat_map(|(_, p)| p.used_params().iter().cloned());
            for p in expand::used_params().into_iter().chain(coord.iter().flat_map(|c| c.used_params())).chain(exclusion_params) {
                if !u.contains(&p) {
                    u.push(p);
                }
            }
            u
        },
        param: match &plot {
            equation::Plot::Param { name, value, .. } => Some((name.clone(), *value)),
            _ => None,
        },
        local_available: !flat
            && !hidden_on
            && !uncached_exclusion
            && coord.is_none()
            && (matches!(&plot, equation::Plot::Implicit { local: Some(_), .. }) || matches!(&plot, equation::Plot::System { eqs, local: Some(_), .. } if eqs.is_empty())),
        gpu2d: if coord.is_some() || fully_excluded {
            None
        } else {
            // uniform にしたパラメータの今の値
            built.gpu2d.map(|mut s| {
                s.values = s.params.iter().map(|n| consts.get(n).map_or(0.0, |c| c.re)).collect();
                s
            })
        },
        gpu3d: gpu3d.0,
        gpu_reason: gpu3d.1,
        coord,
        defs: (expand::used(), expand::def_line()),
        ode_states: expand::ode_states(),
        ode_var: expand::ode_solved().0,
        ode_t0: expand::ode_solved().1,
        holes,
        holes_target,
        source_holes,
        source_dashed,
        limit,
    })
}

thread_local! {
    /// 次の [`plot`] だけに効く、座標系の式（[`coord_system`]）
    static COORD_MAP: std::cell::RefCell<(String, String)> = const { std::cell::RefCell::new((String::new(), String::new())) };
}

/// 次の [`plot`] で使う座標系（計画 9 章の S36。その 1 回だけ。呼ばなければ直交座標）。
/// `map` は組 `(X, Y, Z)` の形の式で、変数は表示中の 3 つの座標（例: 球座標 `(x sin(y) cos(z), x sin(y) sin(z), x cos(y))`）、
/// 定数は [`plot`] のパラメータ。空なら座標系なし。
/// 座標系があると、`plot` の `domain` は「作る範囲」（座標の値の範囲）として使い、結果の `take_target` に頂点ごとの T(q) が入る。
/// 陽関数・陰関数・連立の図形は作る範囲の箱で切る（媒介変数表示は切らない）。切り口のふたの元・立体の印は返さない
/// `names` は座標の名前 3 つ（カンマ区切り。例 `r,theta,phi`。計画 S38）。式と座標系の式の中のこの名前を、表示中の 3 つの座標として読む
/// （同じ名前のパラメータより優先する）。空なら名前なし。ギリシャ文字は解析器が `theta`・`phi`・`rho` と読む
#[wasm_bindgen]
pub fn coord_system(map: &str, names: &str) {
    COORD_MAP.with(|c| *c.borrow_mut() = (map.to_string(), names.to_string()));
}

// ---- 常微分方程式の初期値（計画 M9） ----

thread_local! {
    static ODE_INIT_SRC: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static ODE_START_SRC: std::cell::RefCell<(String, bool)> = const { std::cell::RefCell::new((String::new(), false)) };
}

/// 次の [`plot`] だけに効く、常微分方程式の初期時刻の欄（式。空なら 0）。`legacy` は初期時刻の欄が無かった前の保存データで、
/// 欄が空なら前の規則（範囲に 0 があれば 0、無ければ範囲の始まり）で決める（#131）
#[wasm_bindgen]
pub fn ode_start(src: &str, legacy: bool) {
    ODE_START_SRC.with(|s| *s.borrow_mut() = (src.to_string(), legacy));
}

/// 次の [`plot`] だけに効く、常微分方程式の初期値の欄（状態の成分ごとの式を `\n` でつないだもの。空の行は既定の値）
#[wasm_bindgen]
pub fn ode_init(src: &str) {
    ODE_INIT_SRC.with(|s| *s.borrow_mut() = src.to_string());
}

// ---- ベクトル場の矢印（計画 M8） ----

thread_local! {
    /// 次の [`plot`] だけに効く、ベクトル場の矢印の数と、長さをそろえるか（[`field_style`]）
    static FIELD_STYLE: std::cell::Cell<(u32, bool, u32)> = const { std::cell::Cell::new((0, false, 0)) };
}

/// 次の [`plot`] で、ベクトル場の矢印をどう描くかを決める（その 1 回だけ）。`count` は各軸の矢印の数（0 なら既定: 3D で 9、2D で 18）、
/// `unit` なら長さをそろえる。`stream` は流線の始点の各軸の数（0 なら流線なし。計画 M9）
#[wasm_bindgen]
pub fn field_style(count: u32, unit: bool, stream: u32) {
    FIELD_STYLE.with(|f| f.set((count, unit, stream)));
}

/// 1 つの点での右辺（状態と独立変数 → 状態の導関数）
fn ode_rhs(rhs: &[eval::Program]) -> impl FnMut(f64, &[f64], &mut [f64]) + '_ {
    let mut out = [num_complex::Complex64::new(0.0, 0.0)];
    move |t, y, dy| {
        let mut cols: Vec<[f64; 1]> = y.iter().map(|v| [*v]).collect();
        cols.push([t]);
        let inputs: Vec<&[f64]> = cols.iter().map(|c| &c[..]).collect();
        for (k, p) in rhs.iter().enumerate() {
            let used: Vec<&[f64]> = inputs[..p.vars().len()].to_vec();
            p.eval_batch(&used, &mut out);
            dy[k] = eval::as_real(out[0]);
        }
    }
}

/// 常微分方程式の解曲線（計画 M9）。初期値の組ごとに、初期時刻から前と後ろへ解いて、点の列（n 次元の曲線）にする。
/// 1 本の 1 階なら、傾きの場（同じ長さの短い線分）も足す
fn ode_built(o: &equation::Ode, ctx: &Ctx) -> Built {
    const TOTAL_STEPS: usize = 1_000_000;
    let space = ctx.space;
    let dims = space.dims;
    let dom = ctx.dom;
    let reach = (0..3).map(|a| dom.lo[a].abs().max(dom.hi[a].abs())).fold(0.0, f64::max);
    // 独立変数の範囲と初期時刻。初期時刻は欄の値（空なら 0）で、範囲によらない（#131。範囲を動かしても同じ解の切り取り）。
    // 初期時刻の欄が無かった前の保存データで欄が空なら、前の規則（範囲に 0 があれば 0、無ければ範囲の始まり）
    let (lo, hi) = match o.one {
        Some(_) => (dom.lo[0], dom.hi[0]),
        None => (ctx.range)("t"),
    };
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let t0 = match o.t0 {
        Some(v) => v,
        None if o.legacy && !(lo <= 0.0 && 0.0 <= hi) => lo,
        None => 0.0,
    };
    let var = match o.one {
        Some((x, _)) => crate::equation::coord_name(x),
        None => "t".to_string(),
    };
    expand::set_ode_solved(var.clone(), t0);
    let opt = ode::Options { limit: 8.0 * reach.max(1.0), chord: ctx.size / 400.0, max_arc: f64::INFINITY, arc_dims: o.rhs.len(), max_steps: ode::MAX_STEPS };
    let base: Vec<f64> = (0..dims).map(|k| space.slice.get(k).copied().unwrap_or(0.0)).collect();
    let mut points: Vec<f64> = Vec::new();
    let mut steps = 0;
    let mut truncated = false;
    // 初期時刻に右辺が定義されない初期値（y′ = 1/x の x = 0。#132）は、解き始められないので描かない
    let mut undefined = 0;
    // 範囲の外の初期時刻から解いて、範囲に届く前に止まった（発散・歩数の上限）解曲線の数と、止まった時刻
    let mut unreached = 0;
    let mut stopped = f64::NAN;
    for init in &o.inits {
        if steps >= TOTAL_STEPS {
            truncated = true;
            break;
        }
        let mut f = ode_rhs(&o.rhs);
        let mut d0 = vec![0.0; init.len()];
        f(t0, init, &mut d0);
        if init.iter().chain(&d0).any(|v| !v.is_finite()) {
            undefined += 1;
            continue;
        }
        let left = TOTAL_STEPS - steps;
        let mut solve_to = |t1: f64| {
            if (t1 - t0).abs() > 0.0 {
                let s = ode::solve(&mut f, t0, init, t1, ode::Options { max_steps: opt.max_steps.min(left), ..opt });
                truncated |= s.truncated;
                s.points
            } else {
                vec![(t0, init.clone())]
            }
        };
        // 初期時刻から範囲の両端へ（初期時刻が範囲の外なら、範囲を通って遠い端まで）
        let back = if t0 > lo { solve_to(lo) } else { vec![(t0, init.clone())] };
        let fwd = if t0 < hi { solve_to(hi) } else { vec![(t0, init.clone())] };
        steps += back.len() + fwd.len();
        let seq: Vec<&(f64, Vec<f64>)> = back.iter().rev().chain(fwd.iter().skip(1)).collect();
        // 範囲の中の点と、その外側の 1 点ずつ（曲線が範囲の端まで届くように）だけを描く
        let first = seq.iter().position(|(t, _)| *t >= lo);
        let last = seq.iter().rposition(|(t, _)| *t <= hi);
        let (Some(first), Some(last)) = (first, last) else {
            unreached += 1;
            let end = if t0 < lo { seq.last() } else { seq.first() };
            stopped = end.map_or(f64::NAN, |(t, _)| *t);
            continue;
        };
        if first > last {
            unreached += 1;
            continue;
        }
        if !points.is_empty() {
            points.extend(std::iter::repeat_n(f64::NAN, dims));
        }
        for (t, y) in seq[first.saturating_sub(1)..(last + 2).min(seq.len())].iter().map(|p| (&p.0, &p.1)) {
            let mut p = base.clone();
            for (k, c) in o.coords.iter().enumerate() {
                if let Some(c) = c {
                    p[*c] = y[k];
                }
            }
            if let Some((x, _)) = o.one {
                p[x] = *t;
            }
            points.extend(p);
        }
    }
    // 傾きの場（1 本の 1 階）: 格子の点に、向き (1, f) の同じ長さの線分
    if let (Some((x, u)), 1) = (o.one, o.rhs.len()) {
        let ax = |c: usize| space.axes.iter().position(|&a| a == c).unwrap_or(0);
        let (ix, iu) = (ax(x), ax(u));
        let m = 20;
        let (wx, wu) = ((dom.hi[ix] - dom.lo[ix]) / m as f64, (dom.hi[iu] - dom.lo[iu]) / m as f64);
        let half = 0.35 * wx.min(wu);
        let mut f = ode_rhs(&o.rhs);
        let mut d = [0.0];
        for i in 0..m {
            for j in 0..m {
                let (px, pu) = (dom.lo[ix] + (i as f64 + 0.5) * wx, dom.lo[iu] + (j as f64 + 0.5) * wu);
                f(px, &[pu], &mut d);
                let slope = d[0];
                if !slope.is_finite() {
                    continue;
                }
                // 画面の上の長さをそろえる（縦横の目盛りの比は気にしない）
                let l = (1.0 + slope * slope).sqrt();
                let (dx, du) = (half / l, half * slope / l);
                if !points.is_empty() {
                    points.extend(std::iter::repeat_n(f64::NAN, dims));
                }
                for sgn in [-1.0, 1.0] {
                    let mut p = base.clone();
                    p[x] = px + sgn * dx;
                    p[u] = pu + sgn * du;
                    points.extend(p);
                }
            }
        }
    }
    let n = o.inits.len() - undefined - unreached;
    let slope_field = o.one.is_some() && o.rhs.len() == 1;
    let mut kind = match (n, slope_field) {
        (0, true) => "常微分方程式の傾きの場".to_string(),
        (0, false) => "常微分方程式".to_string(),
        (_, true) => "常微分方程式の解曲線と傾きの場".to_string(),
        (_, false) => "常微分方程式の解曲線".to_string(),
    };
    if n > 1 {
        kind.push_str(&format!("（{n} 本）"));
    }
    if unreached > 0 {
        let t0_text = crate::critical::num_text(t0);
        let at = if stopped.is_finite() { format!("{var} = {} で止まり", crate::critical::num_text(stopped)) } else { "止まり".to_string() };
        kind.push_str(&if n == 0 && undefined == 0 {
            format!("（初期時刻 {var} = {t0_text} から解く途中、{at}、表示範囲に届かないので、解曲線は描けません）")
        } else {
            format!("（初期時刻 {var} = {t0_text} から解く途中、{at}、表示範囲に届かない解曲線が {unreached} 本あります）")
        });
    }
    if undefined > 0 {
        let t0_text = crate::critical::num_text(t0);
        kind.push_str(&if n == 0 {
            format!("（初期時刻 {var} = {t0_text} で右辺が定義されないので、解曲線は描けません）")
        } else {
            format!("（初期時刻 {var} = {t0_text} で右辺が定義されない初期値が {undefined} 本あり、その解曲線は描きません）")
        });
    }
    if truncated {
        kind.push_str("（途中で打ち切り）");
    }
    let shape = mesh::NdShape { dims, points, grid: None, solid: false, complex: Vec::new(), tris: None, keep_full: false, keep_far: true };
    Built { shape: Some(shape), curve: true, kind, ..Default::default() }
}

/// [`plot`] の終わりに、矢印の設定を既定に戻す（その 1 回だけ効くように）
struct FieldStyleReset;
impl Drop for FieldStyleReset {
    fn drop(&mut self) {
        FIELD_STYLE.with(|f| f.set((0, false, 0)));
    }
}

/// 矢印を作り、`Built` にする（矢じりはメッシュ、軸は境目の線と同じ道で返す）
fn arrows_built(a: field::Arrows, kind: String) -> Built {
    Built { inner: a.mesh, value_range: a.range, kind, ..Default::default() }
}

fn field_built(plot: &equation::Plot, ctx: &Ctx) -> Option<Built> {
    match plot {
        equation::Plot::Field { progs, rot } => {
            let (count, unit, stream) = FIELD_STYLE.with(|f| f.get());
            let count = if count == 0 { if ctx.flat { 18 } else { 9 } } else { count.clamp(4, 24) as usize };
            let a = field::field(progs, *rot, ctx.dom, field::Style { count, unit, flat: ctx.flat });
            let n = a.count;
            let mut b = arrows_built(a, format!("ベクトル場 · {n} 本の矢印"));
            b.vivid = true;
            if stream > 0 {
                let (lines, mags) = field::streamlines(progs, *rot, ctx.dom, stream as usize, ctx.flat);
                if !lines.is_empty() {
                    b.outline.0 = lines;
                    b.outline_values = mags;
                    b.kind.push_str("・流線");
                }
            }
            Some(b)
        }
        equation::Plot::Arrows { vectors } => {
            let axes = if ctx.flat { 2 } else { 3 };
            let span = (0..axes).map(|i| ctx.dom.hi[i] - ctx.dom.lo[i]).fold(0.0, f64::max);
            Some(Built { inner: field::vectors(vectors, ctx.flat, span), kind: plot.kind_label(), ..Default::default() })
        }
        equation::Plot::Ode(o) => Some(ode_built(o, ctx)),
        equation::Plot::Limit { graph, report } => Some(Built {
            inner: match graph {
                limit::Graph::Function(prog) => mesh::limit_curve(prog, report, ctx.dom, ctx.size * 1e-4),
                limit::Graph::Sampled(points) => mesh::sampled_limit_curve(points, report),
                limit::Graph::Sequence(points) => {
                    let visible: Vec<_> = points.iter().filter(|p| p[1].is_finite() && (ctx.dom.lo[0]..=ctx.dom.hi[0]).contains(&p[0]))
                        .map(|p| [p[0], p[1], 0.0]).collect();
                    system::points_mesh(&visible)
                }
            },
            kind: plot.kind_label(),
            curve: true,
            ..Default::default()
        }),
        equation::Plot::Points { points, line, .. } => {
            let pts: Vec<[f64; 3]> = points.iter().map(|p| if ctx.flat { [p[0], p[1], 0.0] } else { *p }).collect();
            let inner = if *line {
                mesh::Mesh { positions: pts.iter().flat_map(|p| p.map(|v| v as f32)).collect(), ..Default::default() }
            } else {
                system::points_mesh(&pts)
            };
            Some(Built { inner, curve: true, kind: plot.kind_label(), ..Default::default() })
        }
        _ => None,
    }
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
/// 6: 霧（不等式の領域の中身を霧で塗る。境目までの距離の格子を返す。ほかの式は断面と同じ）。
/// 5: 陰関数の F の値の伝達関数（陰関数 F(x, y, z) = 0 の F の値を格子で返す。表示していない座標を使わない式にも使える）。
/// 断面以外では、式が使っている表示していない座標について、次元の設定の「範囲で重ねる」より、この見せ方を優先する
#[wasm_bindgen]
pub fn hidden_style(values: Vec<f64>) {
    HIDDEN_STYLE.with(|h| *h.borrow_mut() = values);
}

thread_local! {
    /// 次の [`plot`] だけに効く「座標を複素数にする」（[`complex_coords`]）
    static COMPLEX_COORDS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 次の [`plot`] で、座標を複素数にする（その 1 回だけ）: 式の x, y, z を x + i w, y + i w₂, z + i w₃ に置き換えて描く。
/// 6 次元以上で使う。実数の座標 x, y, z と虚部 w, w₂, w₃ の 6 つから 3 つを表示し、残りは断面（や w の見せ方）で決める
#[wasm_bindgen]
pub fn complex_coords(on: bool) {
    COMPLEX_COORDS.with(|c| c.set(on));
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
                // 自分の頂点を持つ粗い版（陰関数。動かしている間に描く）にも同じ値を付ける（付けないと、動かすと色が変わる）
                if let Some(l) = m.lod_mesh.as_deref_mut() {
                    l.complex = (0..l.positions.len() / 3).flat_map(|_| [first as f32, 0.0, 0.0]).collect();
                }
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
                // 平面は平らなので、陽関数の細かさの半分で足りる（色は頂点の間でなめらかに補間される）。
                // 陽関数の細かさ（既定 150）のままだと、各軸 3 枚で三角形が 40 万個を超えて重かった
                let n = (ctx.resolution / 2).clamp(16, 100);
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
                // 値が複素数になる式（w = (x + i y)² + z など）は、実部・虚部の組で返し、色を偏角にする
                if system::is_complex_valued(&f, dom) {
                    let (vals, q) = complex_volume(&f, dom, n);
                    return Ok(Built {
                        kind: format!("{name} の値の伝達関数（複素数。色は偏角、{n}³）"),
                        value_range: Some((0.0, q)),
                        volume: Some((vals, n)),
                        ..Default::default()
                    });
                }
                let vals = grid_values(&f, dom, n);
                Ok(Built {
                    kind: format!("{name} の値の伝達関数（{n}³）"),
                    value_range: Some(range_of(k)),
                    volume: Some((vals, n)),
                    ..Default::default()
                })
            }
        }
        // 陰関数の F の値の伝達関数: F(x, y, z) = 0 の F（左辺 − 右辺）の値を格子で返す。表示していない座標は断面の値。
        // 色の範囲は |F| の 9 割の点が入る値 q での [−q, q]（F の大きさは式によって桁が違うので、表示範囲の中の分布から決める。
        // 最大にすると、遠くの大きな値に引っぱられて 0 の近くが 1 色になる）
        5 => {
            // 表示していない座標は断面の値（次元の設定の「範囲で重ねる」より優先。重ねた立体の G ではなく F を見たいので）
            let compiled = equation::compile_with(source, params, &own)?;
            let equation::Plot::Implicit { prog, .. } = &compiled else {
                return Err("F の値の伝達関数は、陰関数 F(x, y, z) = 0 のときだけ使えます".into());
            };
            let n = ctx.implicit_resolution.clamp(24, 128);
            // 値が複素数の陰関数: 実部・虚部の組で返し、色を偏角、不透明度を |F| で決める（|F| の範囲は [0, q]）
            if system::is_complex_valued(prog, dom) {
                let (vals, q) = complex_volume(prog, dom, n);
                return Ok(Built {
                    kind: format!("複素数の陰関数の F の値の伝達関数（色は偏角、{n}³）"),
                    value_range: Some((0.0, q)),
                    volume: Some((vals, n)),
                    ..Default::default()
                });
            }
            let vals = grid_values(prog, dom, n);
            let mut mags: Vec<f32> = vals.iter().filter(|v| v.is_finite()).map(|v| v.abs()).collect();
            let q = if mags.is_empty() {
                1.0
            } else {
                let i = (mags.len() * 9 / 10).min(mags.len() - 1);
                let (_, q, _) = mags.select_nth_unstable_by(i, |a, b| a.total_cmp(b));
                if *q > 0.0 { *q as f64 } else { 1.0 }
            };
            Ok(Built {
                kind: format!("陰関数の F の値の伝達関数（{n}³）"),
                value_range: Some((-q, q)),
                volume: Some((vals, n)),
                ..Default::default()
            })
        }
        _ => build(plot, ctx),
    }
}

/// 表示範囲の格子での `f` の値を、実部・虚部の組（長さ 2n³。描画側はこの長さで複素数と見分ける）と、
/// |f| の 9 割の点が入る値 q（不透明度の目安。値の大きさは式によって桁が違うので、分布から決める）にする
fn complex_volume(f: &eval::Program, dom: mesh::Domain, n: usize) -> (Vec<f32>, f64) {
    let vals = grid_complex(f, dom, n);
    let mut mags: Vec<f64> = vals.iter().filter(|v| v.re.is_finite() && v.im.is_finite()).map(|v| v.norm()).collect();
    let q = if mags.is_empty() {
        1.0
    } else {
        let i = (mags.len() * 9 / 10).min(mags.len() - 1);
        let (_, q, _) = mags.select_nth_unstable_by(i, |a, b| a.total_cmp(b));
        if *q > 0.0 { *q } else { 1.0 }
    };
    (vals.into_iter().flat_map(|v| [v.re as f32, v.im as f32]).collect(), q)
}

/// 不等式の領域の霧の格子の値: G を、境目 G = 0 までの距離の見積もり G / |∇G|（表示範囲の座標の長さ）にする。
/// 描画側は、この値の符号で中身か外か、大きさで境目に近いかが分かる（G の大きさは式によって桁が違うので、そのままでは境目の幅を決められない。
/// 描画側で勾配を求めると、歩くたびにテクスチャを 6 回多く読むので重い）。勾配は中心差分（端では片側）。
/// 勾配が 0 の所は、境目から遠いとみなす（符号だけ残す）
fn region_distance(g: &[f32], dom: mesh::Domain, n: usize) -> Vec<f32> {
    let h: Vec<f64> = (0..3).map(|a| (dom.hi[a] - dom.lo[a]) / (n - 1) as f64).collect();
    let idx = |x: usize, y: usize, z: usize| (z * n + y) * n + x;
    let mut out = vec![f32::NAN; g.len()];
    for z in 0..n {
        for y in 0..n {
            for x in 0..n {
                let v = g[idx(x, y, z)];
                if v.is_nan() {
                    continue;
                }
                let p = [x, y, z];
                let mut grad2 = 0.0f64;
                for a in 0..3 {
                    let (mut lo, mut hi) = (p, p);
                    lo[a] = p[a].saturating_sub(1);
                    hi[a] = (p[a] + 1).min(n - 1);
                    let (gl, gh) = (g[idx(lo[0], lo[1], lo[2])], g[idx(hi[0], hi[1], hi[2])]);
                    // 隣が定義されない所は、自分の値で代える（片側の差分）
                    let gl = if gl.is_nan() { v } else { gl } as f64;
                    let gh = if gh.is_nan() { v } else { gh } as f64;
                    let d = (gh - gl) / (h[a] * (hi[a] - lo[a]).max(1) as f64);
                    grad2 += d * d;
                }
                let gl = grad2.sqrt();
                out[idx(x, y, z)] = if gl > 0.0 && gl.is_finite() {
                    (v as f64 / gl) as f32
                } else if v <= 0.0 {
                    -1e4
                } else {
                    1e4
                };
            }
        }
    }
    out
}

/// 表示範囲の格子（1 辺 `n` 点。x が最も速く変わる）での `f` の値（実部）
fn grid_values(f: &eval::Program, dom: mesh::Domain, n: usize) -> Vec<f32> {
    grid_complex(f, dom, n).into_iter().map(|v| eval::as_real(v) as f32).collect()
}

/// 表示範囲の格子（1 辺 `n` 点。x が最も速く変わる）での `f` の値
fn grid_complex(f: &eval::Program, dom: mesh::Domain, n: usize) -> Vec<num_complex::Complex64> {
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
    out
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

fn append_visible_holes(
    holes: &mut Vec<f32>, marks: impl IntoIterator<Item = [f64; 3]>,
    dom: mesh::Domain, flat: bool, slice: bool, size: f64,
) {
    for mut p in marks {
        if !p.iter().all(|v| v.is_finite()) || (slice && p[2].abs() > size * 1e-9) {
            continue;
        }
        if flat {
            p[2] = 0.0;
        }
        if (0..3).all(|i| (flat && i == 2) || (dom.lo[i]..=dom.hi[i]).contains(&p[i])) {
            let p = p.map(|v| v as f32);
            if !holes.chunks_exact(3).any(|q| (0..3).all(|i| (p[i] - q[i]).abs() <= (size * 1e-7) as f32)) {
                holes.extend(p);
            }
        }
    }
}

fn line_in_axes(positions: &[f32], space: &equation::Space) -> Vec<f64> {
    let points = positions.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]);
    mesh::points_in_axes(points, space.axes, &space.slice)
}

fn project_dashed(
    positions: &[f64], dims: usize, view: &mesh::View, dom: mesh::Domain, flat: bool,
) -> Vec<f32> {
    if positions.is_empty() {
        return Vec::new();
    }
    let points = positions.chunks_exact(dims).flat_map(|p| {
        let mut q = view.apply(p).map(|v| v as f32);
        if flat && q.iter().all(|v| v.is_finite()) {
            q[2] = 0.0;
        }
        q
    }).collect();
    let mut line = mesh::Mesh { positions: points, ..Default::default() };
    coords::trim_to_domain(&mut line, dom, true, flat);
    line.positions
}

fn explicit_curve_breaks(
    expr: &parser::Expr, prog: &eval::Program, names: &[String; 3], axis: usize, params: &eval::Params, dom: mesh::Domain,
) -> Vec<holes::Break> {
    let free = usize::from(axis == 0);
    let section = expr.substitute(&|name| (name == names[2]).then_some(parser::Expr::Num(0.0)));
    holes::curve(&section, &names[free], params, (dom.lo[free], dom.hi[free]), |xs| holes::curve_samples(prog, xs))
}

fn rotated_curve_exclusions(
    expr: &parser::Expr, prog: &eval::Program, names: &[String; 3], axis: usize, params: &eval::Params,
    dom: mesh::Domain, exclusions: &[(parser::Expr, eval::Program)], shape: Option<&mesh::NdShape>,
    space: &equation::Space, slice: bool,
) -> Vec<[f64; 3]> {
    let Some(shape) = shape.filter(|_| !exclusions.is_empty()) else { return Vec::new() };
    let names_ref = names.each_ref().map(String::as_str);
    let local: Vec<_> = exclusions.iter().filter_map(|(e, _)| {
        let p = eval::Program::compile_with(e, &names_ref, params).ok()?;
        (slice || !p.uses_var_index(2)).then(|| (e.clone(), p))
    }).collect();
    let positions = shape.points.chunks_exact(shape.dims)
        .flat_map(|p| space.axes.map(|a| p[a] as f32)).collect();
    let local_mesh = mesh::Mesh { positions, ..Default::default() };
    let (crossings, _, all) = mark_excluded(&local_mesh, true, local.iter().map(|(_, p)| p));
    if all {
        return Vec::new();
    }
    let mut marks = holes::curve_exclusions(expr, prog, names, axis, params, dom, &local, slice);
    marks.extend(crossings.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]));
    marks
}

/// 図形を作るのに使う設定
#[derive(Clone)]
struct Ctx<'a> {
    params: &'a eval::Params,
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
    /// 不等式の領域を霧（G = max G_i の格子。描画側で G ≤ 0 の中身を塗る）で描くか。3D で、見せ方が霧（6）のときだけ。
    /// 標準は今までどおり境目の面とふた（霧は光線を箱の中で何百歩も進めるので、回している間が十数倍重い）
    region_fog: bool,
}

/// 作った図形。媒介変数表示などは、回転・射影する前の n 次元の点（`shape`）で持つ
#[derive(Default)]
struct Built {
    inner: mesh::Mesh,
    holes: Vec<f32>,
    source_holes: Vec<f64>,
    source_dashed: Vec<f64>,
    /// 極限値から独立して表示する、元の曲線・数列の点。
    limit_source: Vec<f32>,
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
    /// 添えの線・点（[`MeshResult`] の `extra`）
    extra: Vec<f32>,
    /// 光線（GPU）で描けない理由（あれば、GPU の描き方を使わない）
    gpu3d_off: Option<String>,
    /// 実線の点ごとの値（流線の大きさ ‖F‖。value_range の色の対応表で塗る）。ふつうは空
    outline_values: Vec<f32>,
    /// ベクトル場（矢印・流線を、鮮やかな色の対応表で塗る）
    vivid: bool,
    /// 表示していない座標の値で色を付けるなら、その範囲
    value_range: Option<(f64, f64)>,
    /// 伝達関数の格子の値と、1 辺の点の数
    volume: Option<(Vec<f32>, usize)>,
    /// 2D 表示で、画素ごとに描くための GLSL
    gpu2d: Option<gpu2d::Scene2d>,
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
    if let P::SetImage(image) = plot {
        return set_image::build(image, ctx);
    }
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
            if ps.iter().any(|p| matches!(p, P::SetImage(_))) {
                return Ok(set_image::merge_union(parts, ctx));
            }
            return Err("曲線と曲面（領域）が混ざる式は、1 つの行ではまとめて描けません（リスト・∪ の部分ごとに行を分けてください）".into());
        }
        let kind = format!("{} × {}", parts.first().map_or(String::new(), |b| b.kind.clone()), parts.len());
        let rotated_explicit = parts.iter().any(|b| b.rotated_explicit);
        let parts_curve_raw = parts.first().is_some_and(|b| b.curve);
        let fields = parts.iter().flat_map(|b| b.fields.iter().cloned()).collect();
        let outline = (
            parts.iter().flat_map(|b| b.outline.0.iter().copied()).collect(),
            parts.iter().flat_map(|b| b.outline.1.iter().copied()).collect(),
        );
        let extra: Vec<f32> = parts.iter().flat_map(|b| b.extra.iter().copied()).collect();
        let mut holes = parts.iter().flat_map(|b| b.holes.iter().copied()).collect();
        let source_holes: Vec<_> = parts.iter().flat_map(|b| b.source_holes.iter().copied()).collect();
        let source_dashed: Vec<_> = parts.iter().flat_map(|b| b.source_dashed.iter().copied()).collect();
        // 全部が n 次元の点なら、点のままつなげる（回転だけ変わったとき使い回せる）
        if parts.iter().all(|b| b.shape.is_some()) {
            let shapes: Vec<mesh::NdShape> = parts.into_iter().filter_map(|b| b.shape).collect();
            if let Some(shape) = mesh::merge_shapes(shapes.clone()) {
                // 切り口の線にするのは射影した後（plot・Shape::project）なので、ここでは元の種類のままにする
                let curve = parts_curve_raw;
                return Ok(Built { shape: Some(shape), source_holes, source_dashed, fields, curve, kind, rotated_explicit, outline, extra, ..Default::default() });
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
            let marks = source_holes.chunks_exact(ctx.space.dims).map(|p| ctx.view.apply(p));
            append_visible_holes(&mut holes, marks, ctx.dom, ctx.flat, ctx.flat_slice && !parts_curve_raw, ctx.size);
            return Ok(Built { inner: mesh::merge(meshes, curve), holes, source_dashed, fields, curve, kind, outline, extra, ..Default::default() });
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
        let marks = source_holes.chunks_exact(ctx.space.dims).map(|p| ctx.view.apply(p));
        append_visible_holes(&mut holes, marks, ctx.dom, ctx.flat, ctx.flat_slice && !parts_curve_raw, ctx.size);
        return Ok(Built { inner: mesh::merge(meshes, curve), holes, source_dashed, fields, curve, kind, outline, extra, ..Default::default() });
    }
    if let Some(mut b) = field_built(plot, ctx) {
        if let P::Limit { report, .. } = plot {
            let source = std::mem::take(&mut b.inner.positions);
            if let Some(value) = report.finite_value() {
                if ctx.flat {
                    let y = value as f32;
                    let [lo, hi] = [ctx.dom.lo[0] as f32, ctx.dom.hi[0] as f32];
                    b.inner.positions.extend([lo, y, 0.0, hi, y, 0.0]);
                } else {
                    let mut level = build(&equation::constant_plot(value, ctx.space)?, ctx)?;
                    level.kind = b.kind;
                    b = level;
                }
            }
            b.limit_source = source;
        }
        return Ok(b);
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
        P::Explicit { axis, prog, names, expr, rotated: true } => {
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
            if ctx.complex_mode == mesh::ComplexMode::Off {
                let dashed = holes::extruded_lines(expr, prog, names, *axis, &space.with_hidden(ctx.params), ext, false);
                b.source_dashed = line_in_axes(&dashed, space);
            }
            b.rotated_explicit = true;
        }
        P::Explicit { axis, prog, names, expr, .. } => {
            b.inner =
                mesh::explicit_surface_opt(prog, dom, ctx.resolution, *axis, ctx.complex_mode, ctx.walls, ctx.refine);
            if ctx.complex_mode == mesh::ComplexMode::Off {
                b.outline.1 = holes::extruded_lines(expr, prog, names, *axis, &space.with_hidden(ctx.params), dom, false);
            }
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
        // 不等式の領域の回す前の形（陰関数の上と同じ）: 回す前の不等式で、回転の中心から表示範囲の角までの距離 ρ の立方体に作る。
        // 小さい領域は fit_to_shape で領域の周りだけの格子にするので、回転の再生・ドラッグ中も止まったときと同じ細かさで回る
        // （前は再生のたびに格子 16 ほどで作り直して、境目がぎざぎざに崩れて見えた）
        P::System { eqs, local: Some(lr), .. } if ctx.local && eqs.is_empty() => {
            let ext = rotated_domain(space, dom);
            let mut m = fit_to_shape(ext, ctx.implicit_resolution, |d| {
                system::region_faces(&lr.cuts, &lr.faces, d, ctx.implicit_resolution, ctx.resolution, ctx.implicit_refine, ctx.refine)
            });
            mesh::add_cluster_lod(&mut m);
            let local = mesh::LocalMesh { mesh: m, prog: lr.max.clone(), dims: space.dims, axes: space.axes, slice: space.slice.clone() };
            b.local = Some((local, lr.max.used_params().to_vec()));
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
                let (re, im) = (prog.part(Func::Re), prog.part(Func::Im));
                // Re F と Im F が同じ面になる式（i F = 0、F + i F = 0、F + i F² = 0。#122）: 片方の面の上で、もう片方がどこでもほぼ 0 なら、
                // その面が解（交わりは線でなく面）
                let n = ctx.implicit_resolution;
                let mut done = false;
                for (first, second) in [(&re, &im), (&im, &re)] {
                    let surface = mesh::implicit_solid_opt(first, dom, n, &[], 0, ctx.implicit_refine);
                    if !surface.indices.is_empty() && system::vanishes_on(first, second, &surface, dom) {
                        b.kind = "複素数の陰関数（Re F と Im F が同じ面になるので、F = 0 の面。数値で判定）".into();
                        b.inner = surface;
                        b.fields.push(mesh::Field::new(first, &[], 0, dom));
                        done = true;
                        break;
                    }
                }
                if !done {
                    b.curve = true;
                    b.kind = "複素数の陰関数（Re F = Im F = 0 の曲線）".into();
                    b.inner = system::intersection_curve(&re, &im, dom, n, ctx.implicit_refine);
                }
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
            // 段差の枝の中で恒等的に成り立つ解（体積がある）は、下で枝の範囲から作るので、ふつうの面は作らない
            let volumes = mesh::branch_volumes_3d(prog, dom);
            if volumes.is_empty() {
                b.inner = fit_to_shape(dom, ctx.implicit_resolution, |d| mesh::implicit_solid_opt(prog, d, ctx.implicit_resolution, &[], 0, ctx.implicit_refine));
                b.kind.push_str(&b.inner.note);
                b.fields.push(mesh::Field::new(prog, &[], 0, dom));
            }
            // 段差の枝の中で恒等的に成り立つ解（floor(x) + floor(y) + floor(z) = 0 の単位立方体の並び。体積がある）: 枝の範囲の境目の面を、
            // 枝ごとに小さい箱の中で作る（メッシュでも体積の解を描く。スマートフォンの「自動」はメッシュ）。光線（GPU）はそのまま使える
            if !volumes.is_empty() {
                // 枝の範囲は、段差の元の引数が 1 次式なら凸多面体（平面の多角形を切るだけで正確）。そうでなければ領域の面の道
                let parts: Vec<mesh::Mesh> = volumes
                    .iter()
                    .map(|(cuts, _strict, bx)| match cuts.iter().map(mesh::affine_of).collect::<Option<Vec<_>>>() {
                        Some(planes) => mesh::convex_faces(&planes, *bx),
                        None => system::region_faces(cuts, &[], *bx, 8, 8, mesh::Refine::Light, false),
                    })
                    .collect();
                b.inner = mesh::merge(parts, false);
                b.kind = "陰関数（体積のある解）".into();
                b.fields.clear();
            }
            // 段差の飛ぶ面の上だけの解（floor(x) + floor(-x) = 0 の整数の平面。#175）。光線（GPU）では厚さ 0 の面は描けないので、メッシュで描く。
            // 体積のある解の式では、飛ぶ面はその境目なので足さない
            let jumps = if volumes.is_empty() { mesh::jump_surfaces(prog, dom, ctx.implicit_resolution) } else { mesh::Mesh::default() };
            if !jumps.indices.is_empty() {
                b.inner = mesh::merge(vec![std::mem::take(&mut b.inner), jumps], false);
                b.gpu3d_off = Some("段差の飛ぶ面の上だけで成り立つ解があるので、メッシュで描きます".into());
                // 細い枝として描けなかった所は、飛ぶ面として描いた
                b.kind = b.kind.replace("（細かすぎて描けない部分あり）", "");
            }
            // 面になる因数と混ざった、線・点だけの因数（(x² + y²)(z − 1) = 0 の z 軸。#120）: 添えの線・点として描く
            if let Some(sets) = prog.lower_dim_parts() {
                let (m, _) = system::common_zero_sets(&sets, dom, ctx.implicit_resolution, ctx.implicit_refine);
                b.extra = m.positions;
                if !b.extra.is_empty() {
                    b.kind.push_str("（線・点の部分を含む）");
                }
            }
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
        P::Param { .. } | P::Multi(_) | P::ComplexPlane { .. } | P::Field { .. } | P::Arrows { .. } | P::Ode(_) | P::Points { .. } | P::Limit { .. } => {}
        // 等式 1 本と不等式: 等式の行を今までの道で描き、できたメッシュを不等式ごとに切り取る（system::trim_mesh）。
        // 回した陽関数の点（shape）は射影してメッシュにする（回転が変わるたびに計算し直す）。切り口のふたは作らない
        // 切り取った後の面が表示範囲に比べて小さければ（x² + y² = 1, 0 ≤ z ≤ 1 を ±6 で）、その周りだけで作り直す（fit_to_shape）
        P::System { base: Some(base), eqs, cuts, strict, .. } if eqs.len() == 1 => {
            let made = std::cell::RefCell::new(None);
            let trimmed = |d: mesh::Domain| -> mesh::Mesh {
                let c = Ctx { dom: d, ..ctx.clone() };
                let inner = match build(base, &c) {
                    Ok(x) => x.settle(),
                    Err(e) => {
                        made.borrow_mut().get_or_insert(Err(e));
                        return mesh::Mesh::default();
                    }
                };
                let mut m = match &inner.shape {
                    Some(sh) => mesh::project(sh, &ctx.view, d),
                    None => inner.inner,
                };
                for g in cuts {
                    system::trim_mesh(&mut m, &mut system::values_of(g), 8);
                }
                let mut first = made.borrow_mut();
                if first.is_none() {
                    let mut dashed = inner.outline.1;
                    dashed.extend(project_dashed(&inner.source_dashed, ctx.space.dims, &ctx.view, d, false));
                    *first = Some(Ok((inner.curve, inner.kind, dashed)));
                }
                m
            };
            let m = if cuts.is_empty() { trimmed(dom) } else { fit_to_shape(dom, ctx.implicit_resolution, trimmed) };
            let (curve, kind, mut dashed) = made.into_inner().unwrap_or_else(|| Err("内部エラー".into()))?;
            for g in cuts {
                system::clip_segments(&mut dashed, &mut system::values_of(g), 12);
            }
            // 等号つきの不等式の境目に面が接するだけの点（x² + y² + z² = 1, z ≥ 1 の (0, 0, 1)。#206）: 切り取ると消えるので、添えの点にする
            if !curve && !cuts.is_empty() && !ctx.flat {
                if let P::Implicit { prog, .. } = &**base {
                    let n = ctx.implicit_resolution;
                    let surface = mesh::implicit_solid_opt(prog, dom, n, &[], 0, ctx.implicit_refine);
                    let h = (0..3).map(|a| (dom.hi[a] - dom.lo[a]) / n.max(2) as f64).fold(f64::INFINITY, f64::min);
                    let kept: Vec<[f64; 3]> = m.positions.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect();
                    let mut pts = Vec::new();
                    for (i, g) in cuts.iter().enumerate() {
                        if strict.get(i) == Some(&true) {
                            continue;
                        }
                        for p in system::touching_points(prog, g, &surface, dom, n) {
                            let inside = cuts.iter().zip(strict.iter()).all(|(c, &st)| {
                                let v = system::values_of(c)(&[p])[0];
                                let eps = 1e-3 * strict_margin(c, dom);
                                if st { v < -eps } else { v <= eps }
                            });
                            let near = kept.iter().any(|q| (0..3).map(|c| (q[c] - p[c]).powi(2)).sum::<f64>() < (2.0 * h).powi(2));
                            if inside && !near {
                                pts.push(p);
                            }
                        }
                    }
                    b.extra = system::points_mesh(&pts).positions;
                }
            }
            b.inner = m;
            b.outline.1 = dashed;
            b.curve = curve;
            b.kind = if cuts.is_empty() { kind } else { format!("{kind}（不等式で切り取り）") };
        }
        // 交わる線: 書いた順で 1 本目の等式の面の上で、2 本目の等式の線を取り出し、ニュートン法で寄せる（system.rs）。
        // 不等式があれば、線分を切る
        P::System { eqs, cuts, exact, .. } if eqs.len() == 2 => {
            let n = ctx.implicit_resolution;
            // 1 本目と同じ面になる 2 本目（定数倍・恒等式。#206）: 面そのものが解
            let surface = mesh::implicit_solid_opt(&eqs[0], dom, n, &[], 0, ctx.implicit_refine);
            for (first, second) in [(0, 1), (1, 0)] {
                let own = (first == 1).then(|| mesh::implicit_solid_opt(&eqs[1], dom, n, &[], 0, ctx.implicit_refine));
                let s = own.as_ref().unwrap_or(&surface);
                if system::vanishes_on(&eqs[first], &eqs[second], s, dom) {
                    let mut m = own.unwrap_or_else(|| mesh::implicit_solid_opt(&eqs[0], dom, n, &[], 0, ctx.implicit_refine));
                    for g in cuts {
                        system::trim_mesh(&mut m, &mut system::values_of(g), 8);
                    }
                    b.inner = m;
                    b.kind = "連立方程式の面（2 本の式が同じ面）".into();
                    b.fields.push(mesh::Field::new(&eqs[first], &[], 0, dom));
                    return Ok(b);
                }
            }
            b.curve = true;
            b.inner = system::intersection_curve(&eqs[0], &eqs[1], dom, n, ctx.implicit_refine);
            // 面どうしが接するだけの所（球 2 つが 1 点で、球と円柱が赤道の円で接する。#119・#206）
            let touch = system::touching_points(&eqs[0], &eqs[1], &surface, dom, n);
            if !touch.is_empty() {
                let h = (0..3).map(|a| (dom.hi[a] - dom.lo[a]) / n.max(2) as f64).fold(f64::INFINITY, f64::min);
                b.inner = mesh::merge(vec![std::mem::take(&mut b.inner), system::touching_mesh(&touch, h)], true);
            }
            // 縦の座標を使わない 2 本の等式で、符号の変わる所が無い（接するだけ: y = x², y = 0 は x = y = 0 の直線）なら、
            // グレブナー基底で求めた平面の解（exact[1]）を縦にのばした直線にする
            if b.inner.positions.is_empty() && !eqs.iter().any(|e| e.uses_var_index(2)) {
                if let Some(pts) = &exact[1] {
                    let mut pos = Vec::new();
                    for p in pts.iter().filter(|p| (0..2).all(|a| p[a] >= dom.lo[a] && p[a] <= dom.hi[a])) {
                        if !pos.is_empty() {
                            pos.extend([f32::NAN; 3]);
                        }
                        let n = 64;
                        for k in 0..=n {
                            let z = dom.lo[2] + (dom.hi[2] - dom.lo[2]) * k as f64 / n as f64;
                            pos.extend([p[0] as f32, p[1] as f32, z as f32]);
                        }
                    }
                    b.inner.positions = pos;
                }
            }
            for g in cuts {
                system::clip_segments(&mut b.inner.positions, &mut system::values_of(g), 12);
            }
        }
        // 等式 3 本の点: 2 本の交わる線の上で 3 本目の符号が変わる所から寄せる。不等式を満たす点だけ残す
        P::System { eqs, cuts, exact, .. } if eqs.len() == 3 => {
            let pts = system::points_on_curve(eqs, dom, ctx.implicit_resolution, ctx.implicit_refine);
            // 多項式なら、グレブナー基底で求めた解（接するだけの交点も）を合わせる
            let pts = system::with_exact(pts, exact[0].as_deref(), dom, 3);
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
        // 霧: G = max G_i（全部の不等式を満たす所が G ≤ 0）を格子で返す。面を作らないので、境目の面を作るより軽い
        // （面は G_i ごとに格子で計算した上で、細分・切り取り・ふたを作る）
        P::System { eqs, cuts, .. } if eqs.is_empty() && ctx.region_fog => {
            let n = ctx.implicit_resolution.clamp(24, 128);
            let mut g = grid_values(&cuts[0], dom, n);
            for c in &cuts[1..] {
                for (a, v) in g.iter_mut().zip(grid_values(c, dom, n)) {
                    // どちらかが定義されない所は、領域の外（NaN）
                    *a = if a.is_nan() || v.is_nan() { f32::NAN } else { a.max(v) };
                }
            }
            b.kind.push_str(&format!("（霧、{n}³）"));
            b.volume = Some((region_distance(&g, dom, n), n));
        }
        P::System { eqs, cuts, faces, strict, .. } if eqs.is_empty() => {
            b.inner = fit_to_shape(dom, ctx.implicit_resolution, |d| system::region_faces(cuts, faces, d, ctx.implicit_resolution, ctx.resolution, ctx.implicit_refine, ctx.refine));
            b.fields.push(mesh::Field::region_strict(cuts, strict));
            let h = (0..3).map(|a| (dom.hi[a] - dom.lo[a]) / ctx.implicit_resolution.max(2) as f64).fold(f64::INFINITY, f64::min);
            if let Some(t) = system::thin_region(cuts, &b.inner, h) {
                b.kind = format!("{}（厚さ約 {}。格子より薄い）", b.kind, critical::num_text(t));
            }
        }
        P::SetImage(_) => unreachable!("set images are built before the 2D/3D split"),
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
    if let P::Explicit { axis, prog, names, expr, rotated: true } = plot {
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
        let breaks = explicit_curve_breaks(expr, prog, names, *axis, &space.with_hidden(ctx.params), ext);
        let (shape, source_holes) =
            mesh::explicit_curve_nd_with_holes(prog, *axis, space.axes, &space.slice, ext, ctx.size * 1e-4, &breaks);
        return Ok(Built {
            kind: format!("{} = f({})", names[*axis], other),
            curve: true,
            shape: Some(shape),
            source_holes,
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
        if let P::System { eqs, cuts, strict, .. } = plot {
            if eqs.is_empty() {
                let caps = mesh::Field::region_strict(cuts, strict).box_caps(dom, ctx.implicit_resolution.max(8), None);
                b.inner = mesh::merge(vec![std::mem::take(&mut b.inner), caps], false);
            }
        }
        b.fields.clear();
        if b.shape.is_none() {
            mesh::flatten(&mut b.inner, b.curve);
            for point in b.outline.1.chunks_exact_mut(3).filter(|p| p[0].is_finite()) {
                point[2] = 0.0;
            }
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
            b.gpu2d = gpu2d::scene(Some(prog), &[], &[]).ok();
        }
        P::Explicit { axis: 2, prog, names, expr, .. } => {
            b.kind = format!("{} の値で塗り分け", names[2]);
            b.inner = mesh::flat_map(prog, dom, ctx.resolution, ctx.complex_mode);
            if ctx.complex_mode == mesh::ComplexMode::Off {
                b.outline.1 = holes::extruded_lines(expr, prog, names, 2, &space.with_hidden(ctx.params), dom, true);
            }
        }
        P::Explicit { axis, prog, names, expr, .. } => {
            b.curve = true;
            let other = if *axis == 0 { &names[1] } else { &names[0] };
            b.kind = format!("{} = f({})", names[*axis], other);
            let breaks = explicit_curve_breaks(expr, prog, names, *axis, &space.with_hidden(ctx.params), dom);
            (b.inner, b.holes) = mesh::explicit_curve_with_holes(prog, *axis, dom, ctx.size * 1e-4, &breaks);
            add_narrow_extrema(&mut b.inner.positions, expr, prog, names, *axis, &space.with_hidden(ctx.params), dom, ctx.size * 1e-4);
        }
        // 複素数の陰関数の 2D 表示: Re F = Im F = 0 の交点
        P::Implicit { prog, .. } if system::is_complex_valued(prog, dom) => {
            let (re, im) = (prog.part(functions::Func::Re), prog.part(functions::Func::Im));
            b.curve = true;
            // Re F と Im F が同じ曲線になる式（#122 の 2D）: その曲線が解
            for (first, second) in [(&re, &im), (&im, &re)] {
                let line = mesh::implicit_curve_2d(first, dom, res2);
                if !line.positions.is_empty() && system::vanishes_on(first, second, &line, dom) {
                    b.kind = "複素数の陰関数（Re F と Im F が同じ曲線になるので、F = 0 の曲線。数値で判定）".into();
                    b.inner = line;
                    return Ok(b);
                }
            }
            let pts = system::flat_points(&re, &im, dom, res2);
            b.kind = format!("複素数の陰関数（Re F = Im F = 0 の点、{} 個）", pts.len());
            b.inner = system::points_mesh(&pts);
        }
        // 段差の枝の中で恒等的に成り立つ解（floor(x) + floor(y) = 0 の正方形の並び。面積がある）: その範囲を塗り、曲線の部分は添えの線にする
        P::Implicit { prog, from: None, .. } if mesh::branch_regions_2d(prog, dom, res2 / 2).is_some() => {
            b.kind = "陰関数（面積のある解を含む）".into();
            b.inner = mesh::branch_regions_2d(prog, dom, res2 / 2).unwrap_or_default();
            b.extra = mesh::implicit_curve_2d(prog, dom, res2).positions;
        }
        // 恒等式（√x = √x・(x − 1)/(x − 1) = 1）: 定義される所の全体が解なので、領域として塗る（#206）
        P::Implicit { prog, from: None, .. } if identically_zero_2d(prog, dom) => {
            b.kind = "陰関数（恒等式: 定義される所の全体）".into();
            b.inner = mesh::Field::region_strict(std::slice::from_ref(prog), &[false]).cap(dom, 2, 0.0, true, res2 / 2);
        }
        P::Implicit { prog, from, gpu, .. } => {
            b.curve = true;
            b.kind = match from {
                // 回した陽関数（2D の切り口）: 縦の座標 = 0 の曲線
                Some(f) => format!("{f} を回したものの切り口"),
                None => "陰関数の曲線".into(),
            };
            (b.inner, b.extra) = mesh::implicit_curve_2d_parts(prog, dom, res2);
            // 回転・断面・パラメータを uniform で渡せる形（元の座標の式）があればそれ、無ければ畳み込んだ式
            b.gpu2d = match gpu {
                Some(g) => gpu2d::scene_affine(Some(&g.prog), &[], &[], g.dims, g.params.len()).ok().map(|s| with_affine(s, space, &g.params)),
                None => gpu2d::scene(Some(prog), &[], &[]).ok(),
            };
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
        P::Param { .. } | P::Multi(_) | P::SetImage(_) | P::ComplexPlane { .. } | P::Tilted { .. } | P::Field { .. } | P::Arrows { .. } | P::Ode(_) | P::Points { .. } | P::Limit { .. } => {}
        // 媒介変数表示を切り取ったもの: 3D で作ってから、切り口の線か、押しつぶした形にする
        P::System { base: Some(base), eqs, .. } if eqs.is_empty() && !base.param_vars().is_empty() => {
            let ctx3 = Ctx { flat: false, flat_slice: false, ..ctx.clone() };
            let inner = build(plot, &ctx3)?.settle();
            b.kind = inner.kind;
            b.curve = inner.curve || ctx.flat_slice;
            b.inner = flat_finish(inner.inner, inner.curve, ctx.flat_slice);
        }
        // 縦の座標 = 0 の 2 次元の問題として描く（切り口、または縦の座標を使わない式）
        P::System { eqs, cuts, strict, gpu, exact, .. } => match eqs.len() {
            0 => {
                b.kind = "不等式の領域（2D）".into();
                b.inner = mesh::Field::region_strict(cuts, strict).cap(dom, 2, 0.0, true, res2 / 2);
                b.gpu2d = match gpu {
                    Some(g) => gpu2d::scene_affine(None, &g.cuts, strict, g.dims, g.params.len()).ok().map(|s| with_affine(s, space, &g.params)),
                    None => gpu2d::scene(None, cuts, strict).ok(),
                };
                // 境目の線: 不等式ごとに G_i = 0 の線を、ほかの不等式で切る。厳密な不等式（< ・ >）の分は点線。
                // 厳密な不等式 G < 0 は G ≤ −ε で切る（x > 0.3, x ≤ 0.3 の x = 0.3 の線を残さない。#139）
                let margins: Vec<f64> = cuts.iter().zip(strict).map(|(g, &s)| if s { strict_margin(g, dom) } else { 0.0 }).collect();
                for (i, g) in cuts.iter().enumerate() {
                    let mut line = mesh::implicit_curve_2d(g, dom, res2).positions;
                    for (j, other) in cuts.iter().enumerate() {
                        if j != i {
                            let eps = margins[j];
                            let mut value = system::values_of(other);
                            system::clip_segments(&mut line, &mut |p: &[[f64; 3]]| value(p).into_iter().map(|v| v + eps).collect(), 12);
                        }
                    }
                    // 厳密な不等式の境目は、すぐ近くに領域があるときだけ（x² + y² < 0 の原点・y > x², y < x² の放物線は、空集合の境目）
                    if strict[i] {
                        keep_region_edges(&mut line, cuts, strict, dom);
                    }
                    if strict[i] { &mut b.outline.1 } else { &mut b.outline.0 }.extend(line);
                }
            }
            1 => {
                b.curve = true;
                b.kind = "陰関数の曲線（不等式で切り取り）".into();
                (b.inner, b.extra) = mesh::implicit_curve_2d_parts(&eqs[0], dom, res2);
                b.gpu2d = match gpu {
                    Some(g) => gpu2d::scene_affine(g.eqs.first(), &g.cuts, strict, g.dims, g.params.len()).ok().map(|s| with_affine(s, space, &g.params)),
                    None => gpu2d::scene(Some(&eqs[0]), cuts, strict).ok(),
                };
                let whole = b.inner.positions.clone();
                for g in cuts {
                    system::clip_segments(&mut b.inner.positions, &mut system::values_of(g), 12);
                    system::clip_segments(&mut b.extra, &mut system::values_of(g), 12);
                }
                // 切り取りで残った、マスの 1/100 より短い切れ端（接する点のすぐ脇で、折れ線が境目を少しはみ出した所）は捨てる
                let h = (0..2).map(|a| (dom.hi[a] - dom.lo[a]) / res2.max(2) as f64).fold(f64::INFINITY, f64::min);
                drop_slivers(&mut b.inner.positions, 1e-2 * h);
                // 等号つきの不等式の境目に、曲線が接するだけの点（x² + y² = 1, x ≥ 1 の (1, 0)。#206）: 切り取ると長さが無くなって消える。
                // 境目の線と接する点のうち、残った曲線から離れていて、ほかの不等式も満たすものを点として足す
                let touches = inequality_touches_2d(&eqs[0], cuts, strict, &whole, &b.inner.positions, dom, res2);
                if !touches.is_empty() {
                    if !b.extra.is_empty() {
                        b.extra.extend_from_slice(&[f32::NAN; 3]);
                    }
                    b.extra.extend(system::points_mesh(&touches).positions);
                }
            }
            2 => {
                // 2 本の式が同じ曲線になる（定数倍・恒等式）: その曲線が解
                for (first, second) in [(0, 1), (1, 0)] {
                    let (mut line, mut extra) = mesh::implicit_curve_2d_parts(&eqs[first], dom, res2);
                    let whole = mesh::Mesh { positions: [line.positions.clone(), extra.clone()].concat(), ..Default::default() };
                    if !whole.positions.is_empty() && system::vanishes_on(&eqs[first], &eqs[second], &whole, dom) {
                        for g in cuts {
                            system::clip_segments(&mut line.positions, &mut system::values_of(g), 12);
                            system::clip_segments(&mut extra, &mut system::values_of(g), 12);
                        }
                        b.curve = true;
                        b.kind = "連立方程式の曲線（2 本の式が同じ曲線）".into();
                        (b.inner, b.extra) = (line, extra);
                        return Ok(b);
                    }
                }
                let pts = system::with_exact(system::flat_points(&eqs[0], &eqs[1], dom, res2), exact[1].as_deref(), dom, 2);
                let pts = system::points_inside(pts, cuts);
                b.curve = true;
                b.kind = format!("連立方程式の点（{} 個）", pts.len());
                b.inner = system::points_mesh(&pts);
            }
            _ => return Err("2D 表示では、等式は 2 本までにしてください".into()),
        },
    }
    Ok(b)
}

/// 標本の間に隠れた細い山・谷（y = 1/(1 + 10²⁰ (x − 0.234567)²) の高さ 1 の山。幅 10⁻¹⁰ で、どの標本にも乗らない。#206）を曲線に足す。
/// 導関数 f′ を記号微分して 1,000 点で調べ（符号の変わる所が 200 を超える波打つ式は調べない）、符号が変わる所を二分法で詰めて極値の点 x* を求める。そこでの f(x*) が、
/// 曲線（両隣の頂点を結んだ線分）から `tol` より離れていれば、頂点 (x*, f(x*)) を曲線に差し込む
fn add_narrow_extrema(positions: &mut Vec<f32>, expr: &parser::Expr, prog: &eval::Program, names: &[String; 3], axis: usize, params: &eval::Params, dom: mesh::Domain, tol: f64) {
    let free = usize::from(axis == 0);
    let var = &names[free];
    let section = expr.substitute(&|name| (name == names[2]).then_some(parser::Expr::Num(0.0)));
    let Ok(d) = diff::diff(&section, var) else { return };
    let Ok(dp) = eval::Program::compile_with(&d, &[var.as_str()], params) else { return };
    let (lo, hi) = (dom.lo[free], dom.hi[free]);
    let n = 1000;
    let xs: Vec<f64> = (0..=n).map(|k| lo + (hi - lo) * k as f64 / n as f64).collect();
    let eval_d = |pts: &[f64]| {
        let mut out = vec![num_complex::Complex64::new(0.0, 0.0); pts.len()];
        dp.eval_batch(&[pts], &mut out);
        out.iter().map(|v| eval::as_real(*v)).collect::<Vec<f64>>()
    };
    let ds = eval_d(&xs);
    // 符号が変わる所（多すぎる式は、細い山を探すまでもなく波打っているので調べない）
    let brackets: Vec<(f64, f64, bool)> = (0..n)
        .filter(|&k| ds[k].is_finite() && ds[k + 1].is_finite() && ds[k] != 0.0 && ds[k + 1] != 0.0 && (ds[k] < 0.0) != (ds[k + 1] < 0.0))
        .map(|k| (xs[k], xs[k + 1], ds[k] < 0.0))
        .collect();
    if brackets.is_empty() || brackets.len() > 200 {
        return;
    }
    // 二分法（まとめて計算する）
    let (mut l, mut r): (Vec<f64>, Vec<f64>) = brackets.iter().map(|b| (b.0, b.1)).unzip();
    for _ in 0..60 {
        let mids: Vec<f64> = l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect();
        let dm = eval_d(&mids);
        for (k, (&m, v)) in mids.iter().zip(dm).enumerate() {
            if !v.is_finite() {
                continue;
            }
            if (v < 0.0) == brackets[k].2 {
                l[k] = m;
            } else {
                r[k] = m;
            }
        }
    }
    let extrema: Vec<f64> = l.iter().zip(&r).map(|(a, b)| 0.5 * (a + b)).collect();
    let ys = holes::curve_samples(prog, &extrema);
    for (x, y) in extrema.into_iter().zip(ys) {
        if !y.is_finite() || y < dom.lo[axis] - (dom.hi[axis] - dom.lo[axis]) || y > dom.hi[axis] + (dom.hi[axis] - dom.lo[axis]) {
            continue;
        }
        // 曲線の頂点の列（NaN で区切る）のうち、x を挟む隣り合った頂点
        let count = positions.len() / 3;
        let at = |k: usize, c: usize| positions[3 * k + c] as f64;
        let Some(k) = (0..count.saturating_sub(1)).find(|&k| {
            at(k, 0).is_finite() && at(k + 1, 0).is_finite() && at(k, free) <= x && x <= at(k + 1, free) && at(k, free) < at(k + 1, free)
        }) else {
            continue;
        };
        let t = (x - at(k, free)) / (at(k + 1, free) - at(k, free));
        let line = at(k, axis) + t * (at(k + 1, axis) - at(k, axis));
        if (y - line).abs() <= tol {
            continue;
        }
        let mut p = [0.0f32; 3];
        p[free] = x as f32;
        p[axis] = y as f32;
        positions.splice(3 * (k + 1)..3 * (k + 1), p);
    }
}

/// 曲線（2 点と NaN の組の列）のうち、長さが `min` より短い線分を捨てる（長さ 0 の点はそのまま）
fn drop_slivers(pos: &mut Vec<f32>, min: f64) {
    let mut out = Vec::with_capacity(pos.len());
    for seg in pos.chunks(9) {
        if seg.len() == 9 && seg[..6].iter().all(|v| v.is_finite()) {
            let len = ((seg[3] - seg[0]) as f64).hypot((seg[4] - seg[1]) as f64);
            if len > 0.0 && len < min {
                continue;
            }
        }
        out.extend_from_slice(seg);
    }
    *pos = out;
}

/// 2D の式が、表示範囲の格子（41 × 41 点と、ずらした 40 × 40 点）の定義される点の全部で 0（|F| ≤ 10⁻¹³）か（恒等式）
fn identically_zero_2d(prog: &eval::Program, dom: mesh::Domain) -> bool {
    let n = 41;
    let mut pts = Vec::with_capacity(2 * n * n);
    for k in 0..n * n {
        let (i, j) = ((k % n) as f64, (k / n) as f64);
        let at = |a: usize, t: f64| dom.lo[a] + (dom.hi[a] - dom.lo[a]) * t;
        pts.push([at(0, i / (n - 1) as f64), at(1, j / (n - 1) as f64), 0.0]);
        pts.push([at(0, (i + 0.37) / n as f64), at(1, (j + 0.61) / n as f64), 0.0]);
    }
    let vals = system::values_of(prog)(&pts);
    let defined: Vec<f64> = vals.into_iter().filter(|v| v.is_finite()).collect();
    !defined.is_empty() && defined.iter().all(|v| v.abs() <= 1e-13)
}

/// 2D の曲線 F = 0（切り取る前の線 `whole`、切り取った後の線 `kept`）が、等号つきの不等式 G ≤ 0 の境目 G = 0 に接するだけの点
/// （[`system::tangent_points`]）のうち、ほかの不等式も満たし、残った曲線から 2 マスより離れたもの
fn inequality_touches_2d(eq: &eval::Program, cuts: &[eval::Program], strict: &[bool], whole: &[f32], kept: &[f32], dom: mesh::Domain, n: usize) -> Vec<[f64; 3]> {
    let points: Vec<[f64; 3]> = whole.chunks_exact(3).filter(|p| p[0].is_finite()).map(|p| [p[0] as f64, p[1] as f64, 0.0]).collect();
    if points.is_empty() {
        return Vec::new();
    }
    let h = (0..2).map(|a| (dom.hi[a] - dom.lo[a]) / n.max(2) as f64).fold(f64::INFINITY, f64::min);
    let rest: Vec<[f64; 2]> = kept.chunks_exact(3).filter(|p| p[0].is_finite()).map(|p| [p[0] as f64, p[1] as f64]).collect();
    let mut out: Vec<[f64; 3]> = Vec::new();
    for (g, &s) in cuts.iter().zip(strict) {
        if s {
            continue;
        }
        let gv = system::values_of(g)(&points);
        for p in system::tangent_points(eq, g, &points, &gv, h) {
            let inside = cuts.iter().zip(strict).all(|(c, &cs)| {
                let v = system::values_of(c)(&[p])[0];
                let eps = 1e-3 * strict_margin(c, dom);
                if cs { v < -eps } else { v <= eps }
            });
            let near = rest.iter().any(|q| (q[0] - p[0]).hypot(q[1] - p[1]) < 2.0 * h);
            if inside && !near && !out.iter().any(|q| (q[0] - p[0]).hypot(q[1] - p[1]) < 0.25 * h) {
                out.push(p);
            }
        }
    }
    out
}

/// 境目の線を切るとき、厳密な不等式 G < 0 を G ≤ −ε として扱う ε（表示範囲の z = 0 の 33 × 33 点での |G| の最大の 10⁻⁶ 倍。#139）。
/// 線の頂点は f32 なので、10⁻⁷ ほどずれる（0.3 は 0.30000001）。それより大きく、見えないほど小さく
fn strict_margin(g: &eval::Program, dom: mesh::Domain) -> f64 {
    let n = 33;
    let at = |a: usize, k: usize| dom.lo[a] + (dom.hi[a] - dom.lo[a]) * k as f64 / (n - 1) as f64;
    let pts: Vec<[f64; 3]> = (0..n * n).map(|k| [at(0, k % n), at(1, k / n), 0.0]).collect();
    let max = system::values_of(g)(&pts).into_iter().filter(|v| v.is_finite()).fold(0.0f64, |m, v| m.max(v.abs()));
    (1e-6 * max).max(1e-300)
}

/// 厳密な不等式の境目の線から、まわり（表示範囲の対角線の 10⁻⁴ 倍の 8 方向）に領域の点が無い頂点を除く。
/// 領域は全部の不等式 G_j < 0（厳密）・G_j ≤ 0。まわりの点は境目から離れているので、余裕 ε は使わない
fn keep_region_edges(line: &mut [f32], cuts: &[eval::Program], strict: &[bool], dom: mesh::Domain) {
    let size = ((dom.hi[0] - dom.lo[0]).powi(2) + (dom.hi[1] - dom.lo[1]).powi(2)).sqrt();
    let r = 1e-4 * size;
    let verts: Vec<usize> = (0..line.len() / 3).filter(|&k| line[3 * k].is_finite() && line[3 * k + 1].is_finite()).collect();
    let mut pts = Vec::with_capacity(verts.len() * 8);
    for &k in &verts {
        for d in 0..8 {
            let a = std::f64::consts::TAU * d as f64 / 8.0;
            pts.push([line[3 * k] as f64 + r * a.cos(), line[3 * k + 1] as f64 + r * a.sin(), 0.0]);
        }
    }
    let mut inside = vec![true; pts.len()];
    for (g, &s) in cuts.iter().zip(strict) {
        for (ok, v) in inside.iter_mut().zip(system::values_of(g)(&pts)) {
            *ok &= if s { v < 0.0 } else { v <= 0.0 };
        }
    }
    for (n, &k) in verts.iter().enumerate() {
        if !inside[8 * n..8 * n + 8].iter().any(|&b| b) {
            line[3 * k..3 * k + 3].fill(f32::NAN);
        }
    }
}

/// 3D の陰関数を光線で描くためのもの（[`Gpu3d`]）と、使えないときの理由（2D 表示では、画素ごとの描き方を使えない理由）。
/// 陰関数・不等式の領域でない式（陽関数・媒介変数表示など）は、もともと GPU の描き方の対象ではないので、理由も空
#[allow(clippy::too_many_arguments)]
fn gpu3d_of(
    plot: &equation::Plot,
    consts: &eval::Params,
    space: &equation::Space,
    flat: bool,
    hidden_on: bool,
    coord: bool,
    has_gpu2d: bool,
) -> (Option<Gpu3d>, String) {
    if coord {
        let implicit = matches!(plot, equation::Plot::Implicit { .. } | equation::Plot::System { .. });
        return (None, if implicit { "座標系を使う式は、GPU の描き方では使えません".into() } else { String::new() });
    }
    if flat {
        let reason = if has_gpu2d {
            String::new()
        } else {
            match plot {
                equation::Plot::Implicit { prog, .. } => prog_reason(prog),
                equation::Plot::System { eqs, cuts, .. } => eqs.iter().chain(cuts).map(prog_reason).find(|r| !r.is_empty()).unwrap_or_else(|| "この式は 2D の画素ごとの描き方では使えません".into()),
                _ => String::new(),
            }
        };
        return (None, reason);
    }
    if hidden_on {
        let implicit = matches!(plot, equation::Plot::Implicit { .. });
        return (None, if implicit { "表示していない座標の見せ方を使う式は、GPU の描き方では使えません".into() } else { String::new() });
    }
    let equation::Plot::Implicit { prog, gpu, .. } = plot else {
        return (None, String::new());
    };
    let Some(g) = gpu else { return (None, prog_reason(prog)) };
    let idx: Vec<usize> = (g.dims..g.dims + g.params.len()).collect();
    match g.prog.to_glsl(&eval::GlslOptions { name: "gR".into(), params: idx, ..Default::default() }) {
        Ok(code) => {
            let values = g.params.iter().map(|n| consts.get(n).map_or(0.0, |c| c.re)).collect();
            (Some(Gpu3d { glsl: code.source(), params: g.params.clone(), values, affine: space.gpu_affine(), axes: space.axes }), String::new())
        }
        Err(e) => (None, e),
    }
}

/// 式が GPU の描き方に移せない理由（移せるなら空）
fn prog_reason(prog: &eval::Program) -> String {
    prog.to_glsl(&eval::GlslOptions::default()).err().unwrap_or_default()
}

/// [`gpu2d::scene_affine`] の図形に、画素の座標から式の座標への 1 次式とパラメータの名前を入れる（値は [`plot`] で入れる）
fn with_affine(mut s: gpu2d::Scene2d, space: &equation::Space, params: &[String]) -> gpu2d::Scene2d {
    s.affine = space.gpu_affine();
    s.params = params.to_vec();
    s
}

/// ≠ で除いた所 h = 0（`hs` のどれか）を示す。曲線なら、線分が h = 0 を横切る点（はさみうち法で寄せる）を白丸の点にし、
/// 面・領域なら、三角形ごとに h = 0 の線分を求めて点線にする。曲線が h = 0 の上を走っている（頂点の半分以上で h = 0）なら、
/// 曲線全体が除かれる（3 つ目の値が true）。戻り値は（白丸の点, 点線の線分, 曲線全体が除かれるか）
fn mark_excluded<'a>(mesh: &mesh::Mesh, curve: bool, hs: impl Iterator<Item = &'a eval::Program>) -> (Vec<f32>, Vec<f32>, bool) {
    let mut hs = hs.peekable();
    if hs.peek().is_none() {
        return (Vec::new(), Vec::new(), false);
    }
    let pts: Vec<[f64; 3]> = mesh.positions.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect();
    let eval_at = |h: &eval::Program, q: &[[f64; 3]]| -> Vec<f64> {
        let cols: [Vec<f64>; 3] = [0, 1, 2].map(|c| q.iter().map(|p| p[c]).collect());
        let mut out = vec![num_complex::Complex64::new(0.0, 0.0); q.len()];
        h.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
        out.iter().map(|v| if v.im.abs() <= 1e-9 * v.re.abs().max(1.0) { v.re } else { f64::NAN }).collect()
    };
    let lerp = |a: [f64; 3], b: [f64; 3], t: f64| [0, 1, 2].map(|c| a[c] + t * (b[c] - a[c]));
    let (mut holes, mut dashed) = (Vec::new(), Vec::new());
    for h in hs {
        let v = eval_at(h, &pts);
        if curve {
            let zeros = v.iter().filter(|x| **x == 0.0).count();
            if zeros * 2 > v.iter().filter(|x| x.is_finite()).count() {
                return (Vec::new(), Vec::new(), true);
            }
            // 符号が変わる線分（はさみうち法で 30 回寄せる）と、ちょうど 0 の頂点
            let mut found: Vec<[f64; 3]> = Vec::new();
            let mut segs: Vec<([f64; 3], [f64; 3], f64, f64)> = Vec::new();
            for k in 0..pts.len().saturating_sub(1) {
                let (a, b) = (v[k], v[k + 1]);
                if !(a.is_finite() && b.is_finite()) {
                    continue;
                }
                if a == 0.0 {
                    found.push(pts[k]);
                } else if a * b < 0.0 {
                    segs.push((pts[k], pts[k + 1], a, b));
                }
            }
            if let Some(&last) = v.last() {
                if last == 0.0 {
                    found.push(pts[pts.len() - 1]);
                }
            }
            for _ in 0..30 {
                if segs.is_empty() {
                    break;
                }
                let mids: Vec<[f64; 3]> = segs.iter().map(|&(a, b, fa, fb)| lerp(a, b, fa / (fa - fb))).collect();
                let fm = eval_at(h, &mids);
                for (s, (m, f)) in segs.iter_mut().zip(mids.iter().zip(fm)) {
                    if !f.is_finite() || f == 0.0 {
                        if f == 0.0 {
                            found.push(*m);
                        }
                        s.0 = *m;
                        s.1 = *m;
                        s.2 = 0.0;
                        s.3 = 0.0;
                    } else if f * s.2 < 0.0 {
                        (s.1, s.3) = (*m, f);
                    } else {
                        (s.0, s.2) = (*m, f);
                    }
                }
                segs.retain(|s| s.2 != 0.0 || s.0 != s.1);
                // 寄り切った（端の値がとても小さい）線分は、点を決める
                segs.retain(|&(a, b, fa, fb)| {
                    let done = (0..3).all(|c| (a[c] - b[c]).abs() <= 1e-12 * (1.0 + a[c].abs()));
                    if done {
                        found.push(lerp(a, b, fa / (fa - fb)));
                    }
                    !done
                });
            }
            for (a, b, fa, fb) in segs {
                found.push(if fa == fb { a } else { lerp(a, b, fa / (fa - fb)) });
            }
            for p in found.into_iter().take(500) {
                holes.extend(p.map(|x| x as f32));
            }
        } else {
            for t in mesh.indices.chunks_exact(3) {
                let ids = [t[0] as usize, t[1] as usize, t[2] as usize];
                let f = ids.map(|i| v[i]);
                if !f.iter().all(|x| x.is_finite()) {
                    continue;
                }
                let mut cross = Vec::with_capacity(2);
                for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                    if (f[i] < 0.0) != (f[j] < 0.0) {
                        cross.push(lerp(pts[ids[i]], pts[ids[j]], f[i] / (f[i] - f[j])));
                    }
                }
                if let [a, b] = cross[..] {
                    dashed.extend([a[0] as f32, a[1] as f32, a[2] as f32, b[0] as f32, b[1] as f32, b[2] as f32, f32::NAN, f32::NAN, f32::NAN]);
                }
            }
        }
    }
    (holes, dashed, false)
}

/// 表示範囲に比べて小さい図形を、図形の周りだけの格子で作り直す（表示範囲を広げると粗くなるのを防ぐ）。
///
/// 格子のマスの数は表示範囲の 1 辺あたり `n` なので、表示範囲を 4 倍に広げると、半径 1 の球は 1 辺 4 マスほどで粗くなっていた。
/// まず表示範囲の格子で作り、できた面の外接箱（1.5 マスの余白つき）の 1 辺が表示範囲の 2/3 以下なら、その箱を立方体に広げた所
/// （表示範囲に収まるように寄せる）に同じマスの数の格子を置いて作り直す。作り直した面が箱の端（表示範囲の端でない所）に触れたら、
/// 粗い格子で見落とした部分が箱で切れたとみなして、表示範囲の格子の面のままにする
fn fit_to_shape(dom: mesh::Domain, n: usize, make: impl Fn(mesh::Domain) -> mesh::Mesh) -> mesh::Mesh {
    let n = n.max(2) as f64;
    let side = |d: &mesh::Domain| (0..3).map(|a| d.hi[a] - d.lo[a]).fold(0.0, f64::max);
    let (mut best, mut box_) = (make(dom), dom);
    let first_lines = best.edge_lines.clone();
    // 1 回目は表示範囲の 2/3 以下になるとき。粗い格子の余白（1.5 マス）が大きいので、作り直した面でもう一度だけ詰める（8 割以下になるとき）
    for shrink in [2.0 / 3.0, 0.8] {
        let Some((lo, hi)) = bounds(&best.positions) else { break };
        let pad = 1.5 * side(&box_) / n;
        let ext = (0..3).map(|a| (hi[a] - lo[a]) + 2.0 * pad).fold(0.0, f64::max);
        if !(ext > 0.0 && ext <= side(&box_) * shrink) {
            break;
        }
        let mut fit = dom;
        for a in 0..3 {
            if ext < dom.hi[a] - dom.lo[a] {
                let upper = (dom.hi[a] - ext).max(dom.lo[a]);
                let l = (0.5 * (lo[a] + hi[a]) - 0.5 * ext).clamp(dom.lo[a], upper);
                fit.lo[a] = l;
                fit.hi[a] = l + ext;
            }
        }
        let next = make(fit);
        let Some((lo2, hi2)) = bounds(&next.positions) else { break };
        let tol = 0.5 * ext / n;
        let touches = (0..3).any(|a| (lo2[a] - fit.lo[a] < tol && fit.lo[a] > dom.lo[a] + tol) || (fit.hi[a] - hi2[a] < tol && fit.hi[a] < dom.hi[a] - tol));
        if touches {
            break;
        }
        (best, box_) = (next, fit);
    }
    // 面にならない特異点の線（mesh::singular::trace_singular_lines）は、表示範囲の格子で作った 1 回目のものを使う
    // （ローマ曲面のひげは面の外へ伸びるので、面の周りだけの箱では途中で切れる。線はニュートン法で寄せるので格子の粗さによらない）
    best.edge_lines = first_lines;
    best
}

/// 頂点の外接箱（有限な頂点だけ）。頂点が無ければ None
fn bounds(positions: &[f32]) -> Option<([f64; 3], [f64; 3])> {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in positions.chunks_exact(3) {
        if p.iter().all(|v| v.is_finite()) {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a] as f64);
                hi[a] = hi[a].max(p[a] as f64);
            }
        }
    }
    lo[0].is_finite().then_some((lo, hi))
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
    source_holes: Vec<f64>,
    source_dashed: Vec<f64>,
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
    /// 座標系（[`coord_system`]）。射影し直した後に同じ写像をかける
    coord: Option<coords::CoordMap>,
    /// 3D の陰関数を光線で描くためのもの（回す前の形のメッシュの結果にも載せる。載せないと描画側が光線の箱を捨てる）
    gpu3d: Option<Gpu3d>,
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
        let curve = self.curve || self.flat_slice;
        let (field, solid) = if let Some(c) = &self.coord {
            // 回した陽関数は作る範囲からはみ出して作るので切る（媒介変数表示は切らない。[`plot`] と同じ）
            if self.explicit_used.is_some() {
                coords::trim_to_domain(&mut inner, dom, curve, self.flat);
            }
            c.apply(&mut inner, curve, self.flat);
            (None, false)
        } else {
            (field, solid)
        };
        if !self.flat && !curve && self.coord.is_none() {
            inner.edge_lines = mesh::box_edge_lines(&inner, dom);
        }
        let end = timing::now_ms();
        record_stats(&mut inner, start, end, end);
        let mut result = self.result(inner, field, solid && !self.flat, None);
        let marks = self.source_holes.chunks_exact(dims as usize).map(|p| view.apply(p));
        let size = (0..3).map(|a| (dom.hi[a] - dom.lo[a]).powi(2)).sum::<f64>().sqrt();
        append_visible_holes(&mut result.holes, marks, dom, self.flat, self.flat_slice && !self.curve, size);
        if !curve {
            result.outline.1 = project_dashed(&self.source_dashed, dims as usize, &view, dom, self.flat);
        }
        if let Some(c) = &self.coord {
            result.holes_target = c.apply_line(&mut result.holes, self.flat);
            result.outline_target.1 = c.apply_line(&mut result.outline.1, self.flat);
        }
        Ok(result)
    }

    /// 回す前の形で作った陰関数で、2D 表示でないなら true。メッシュを作り直さず、描画側で
    /// [`Shape::local_mesh`] のメッシュに [`Shape::transform`] の 1 次式を掛けて描ける
    #[wasm_bindgen(getter)]
    pub fn rigid(&self) -> bool {
        self.local.is_some() && !self.flat && self.coord.is_none()
    }

    /// 回す前の形のメッシュ（[`Shape::rigid`] のときだけ）
    pub fn local_mesh(&self) -> Option<MeshResult> {
        let l = self.local.as_ref().filter(|_| !self.flat && self.coord.is_none())?;
        let space = equation::Space::basic(l.dims, l.axes, vec![0.0; l.dims]);
        Some(self.result(l.project(&space.view(0.0)), None, false, None))
    }

    /// 回転だけを変えたときの、頂点に掛ける 1 次式（結果の `transform`）と切り口のふたの元（`take_field`）。メッシュは空。
    /// 引数の意味は [`Shape::project`] と同じ（遠近法は使わない）。回せないときは "RECOMPUTE" のエラー
    pub fn transform(&self, domain: &[f64], dims: u32, axes: &[u32], rotation: &[f64], center: &[f64]) -> Result<MeshResult, JsError> {
        let start = timing::now_ms();
        timing::reset();
        let Some(l) = self.local.as_ref().filter(|_| !self.flat && self.coord.is_none()) else {
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
        mut inner: mesh::Mesh,
        field: Option<(Vec<mesh::Field>, mesh::Domain)>,
        solid: bool,
        transform: Option<[f64; 12]>,
    ) -> MeshResult {
        let lines = std::mem::take(&mut inner.edge_lines);
        MeshResult {
            inner,
            flat: self.flat,
            flat_slice: self.flat_slice,
            outline: (lines, Vec::new()),
            outline_values: Vec::new(),
            vector_arrows: false,
            vivid: false,
            outline_target: Default::default(),
            extra: Vec::new(),
            extra_target: Vec::new(),
            coord: None,
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
            defs: Default::default(),
            gpu2d: None,
            // 光線の 1 次式は、回転だけが変わったら描画側が書き換える（setPlotTransform）。ここでは形を作ったときのまま
            gpu3d: self.gpu3d.clone(),
            gpu_reason: String::new(),
            ode_states: Vec::new(),
            ode_var: String::new(),
            ode_t0: f64::NAN,
            holes: Vec::new(),
            holes_target: Vec::new(),
            source_holes: Vec::new(),
            source_dashed: Vec::new(),
            limit: None,
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
            outline_values: Vec::new(),
            vector_arrows: false,
            vivid: false,
            outline_target: Default::default(),
            extra: Vec::new(),
            extra_target: Vec::new(),
            coord: None,
            value_range: None,
            volume: None,
            defs: Default::default(),
            gpu2d: None,
            gpu3d: None,
            gpu_reason: String::new(),
            ode_states: Vec::new(),
            ode_var: String::new(),
            ode_t0: f64::NAN,
            holes: Vec::new(),
            holes_target: Vec::new(),
            source_holes: Vec::new(),
            source_dashed: Vec::new(),
            limit: None,
        })
    }
}

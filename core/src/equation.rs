//! 入力された式が「どの種類のグラフか」を判定して、コンパイルする。
//!
//! 座標は x, y, z, w（= w₁）, w₂, w₃, …（4次元目が w、5次元目が w₂）。
//! そのうち3つを画面の横・奥・縦の軸に表示し、残りの座標は値を固定する（断面）。
//! 以下の「表示中の座標」は、画面の軸に割り当てた3つの座標のこと（ふつうは x, y, z）。
//!
//! - `z = f(x, y)` のように、片側が表示中の座標1つだけ          → 陽関数の曲面
//!   （`=` が無ければ「縦の軸の座標 = 式」とみなす）
//! - それ以外の `左辺 = 右辺`                                 → 陰関数 左辺 − 右辺 = 0
//! - `(x(t), y(t), z(t), …)`                                  → 媒介変数表示の曲線
//! - `(x(u,v), y(u,v), z(u,v), …)`                            → 媒介変数表示の曲面
//! - `(x(u,v,t), y(u,v,t), z(u,v,t), …)`                      → 媒介変数表示の立体（媒介変数の直方体の6つの面を描く）
//! - `(x(t₁,…,t₄), y(t₁,…,t₄), …)`（媒介変数が4つ以上）       → 媒介変数表示の多変数の図形
//!   （媒介変数を2つずつ動かした面をたくさん描く。[`crate::mesh::parametric_sheets_nd`]）
//!   （媒介変数は t / u, v / u, v, t / t₁, t₂, t₃, t₄, …。成分が次元より少なければ残りは 0）
//! - `a = 1`（座標以外の名前 = 数）                            → パラメータ（スライダー）の定義
//!
//! 表示していない座標と、パラメータは、式の中では定数として扱う。
//! t, u, v も、組 (…, …) の外ではふつうのパラメータとして使える。
//!
//! **範囲で重ねる**: 表示していない座標やパラメータに範囲を付けると、その範囲で動かしたときに
//! 図形が通る領域を描く（曲面 → 立体、曲線 → 曲面）。
//!
//! **回転**: 中心 c と、座標の組 (i, j) ごとの回転角で回転 R を決め、図形上の点 p を
//! q = R (p − c) + c に動かしてから、断面・射影をとる。式は p = c + Rᵀ (q − c) を代入して q の式にする。

use crate::eval::{Params, Program};
use crate::parser::{parse_system_with, parse_with, BinOp, Expr, ParseError, Rel};
use num_complex::Complex64 as C;

/// 座標空間の設定
#[derive(Debug, Clone)]
pub struct Space {
    /// 次元（3 以上）
    pub dims: usize,
    /// 画面の横・奥・縦の軸に表示する座標の番号（0: x, 1: y, 2: z, 3: w, 4: w₂, …）
    pub axes: [usize; 3],
    /// 各座標の断面の値（表示していない座標だけが使われる）
    pub slice: Vec<f64>,
    /// 各座標の「範囲で重ねる」範囲（表示していない座標だけが使われる。None なら断面）
    pub ranges: Vec<Option<(f64, f64)>>,
    /// パラメータの「範囲で重ねる」範囲
    pub param_ranges: Vec<(String, f64, f64)>,
    /// 範囲を何等分して重ねるか
    pub samples: usize,
    /// 回転角（ラジアン）。座標の組 (0,1), (0,2), …, (0,m−1), (1,2), … の順に mC2 個。
    /// m = max(次元, 5)（次元が5未満でも、w, w₂ を使った5次元の回転ができる）
    pub rotation: Vec<f64>,
    /// 回転の中心（m 個）
    pub center: Vec<f64>,
    /// 陽関数の複素数の色分けで、高さにするもの（|f| なら Abs、Re f なら Re、Im f なら Im。しないなら None）。
    /// 表示していない座標（w など）が絡む回転で陽関数を陰関数として計算し直すとき、f の代わりにこれを使う
    pub complex_height: Option<crate::functions::Func>,
    /// 複素平面の図形（[`complex_plane`]）で、縦の軸を媒介変数にするか（`complex_height` より優先）
    pub lift_param: bool,
    /// 複素数のパラメータ（スライダーを複素平面にしたもの）の名前。これを使った定数の式は複素平面の点になる
    pub complex_params: Vec<String>,
    /// 2D 表示の切り口を描くか。そうなら、縦の座標を使う陽関数（z = f(x, y) など）を回したときは、
    /// 点を計算して回すのではなく、陰関数として計算し直す（式から直接、縦の座標 = 0 の曲線を求める。
    /// 0 に接するだけの所も見つけられ、格子点の誤差でできる余計な線も出ない）
    pub flat_slice: bool,
    /// 座標を複素数にする: 式の x, y, z を x + i w, y + i w₂, z + i w₃ に置き換える（6 次元以上で使う。[`complexify_coords`]）
    pub complex_coords: bool,
    /// 座標系の座標の名前（計画 9 章の S38。例: r, theta, phi）。式の中のこの名前を、表示中の 3 つの座標に置き換える（[`rename_coords`]）。
    /// 同じ名前のパラメータより優先する
    pub coord_names: Option<[String; 3]>,
}

impl Space {
    /// ふつうの3次元（x, y, z をそのまま表示）
    pub fn standard() -> Space {
        Space::basic(3, [0, 1, 2], vec![0.0; 3])
    }

    /// 範囲・回転なしの空間
    pub fn basic(dims: usize, axes: [usize; 3], slice: Vec<f64>) -> Space {
        Space {
            dims,
            axes,
            slice,
            ranges: vec![None; dims],
            param_ranges: Vec::new(),
            samples: 24,
            rotation: vec![0.0; rot_dims(dims) * (rot_dims(dims) - 1) / 2],
            center: vec![0.0; rot_dims(dims)],
            complex_height: None,
            lift_param: false,
            complex_params: Vec::new(),
            flat_slice: false,
            complex_coords: false,
            coord_names: None,
        }
    }

    /// 媒介変数表示の点 p（次元の数の成分）を、画面の横・奥・縦の座標に移す1次式。
    /// 回転していれば q_a = c_a + Σ_k R[a][k] (p_k − c_k)（次元より先の成分は 0）、していなければ q_a = p_a
    pub fn view_rows(&self) -> [crate::mesh::ViewRow; 3] {
        let m = self.rotation_matrix();
        self.axes.map(|a| self.view_row(a, &m))
    }

    /// 回転後の座標 q_a を、点 p の1次式で表したもの（次元より先の成分 p は 0）
    fn view_row(&self, a: usize, m: &Option<Vec<Vec<f64>>>) -> crate::mesh::ViewRow {
        let n = self.dims;
        let rn = rot_dims(n);
        let c: Vec<f64> = (0..rn).map(|j| self.center.get(j).copied().unwrap_or(0.0)).collect();
        match m {
            None => (0.0, (0..n).map(|k| (k == a) as u8 as f64).collect()),
            Some(m) => {
                let c0 = c[a] - (0..rn).map(|k| m[a][k] * c[k]).sum::<f64>();
                (c0, (0..n).map(|k| m[a][k]).collect())
            }
        }
    }

    /// 射影の設定。`dist` は4次元の遠近法の視点までの距離（0 なら遠近法なし＝平行な射影）。
    /// 遠近法では、表示していない各座標 h の方向に、中心から距離 dist の所に視点を置き、
    /// 点を D / (D − (q_h − c_h)) 倍に縮める（視点から遠い＝奥にある点ほど小さく描く）
    pub fn view(&self, dist: f64) -> crate::mesh::View {
        let m = self.rotation_matrix();
        let rn = rot_dims(self.dims);
        let center = |k: usize| self.center.get(k).copied().unwrap_or(0.0);
        let hidden = if dist > 0.0 {
            (0..rn).filter(|k| !self.axes.contains(k)).map(|k| (self.view_row(k, &m), center(k))).collect()
        } else {
            Vec::new()
        };
        crate::mesh::View {
            rows: self.axes.map(|a| self.view_row(a, &m)),
            center: self.axes.map(center),
            hidden,
            dist,
        }
    }

    /// 回転が「表示中の座標どうし」と「式が使っていない、表示していない座標どうし」だけか。
    /// そうなら、表示中の3つの座標の空間は回転で自分自身に移るので、図形をそのまま回して描ける。
    /// `used` は式が使っている名前。
    pub fn rotation_keeps_shown(&self, used: &[String]) -> bool {
        let n = rot_dims(self.dims);
        let mut k = 0;
        for i in 0..n {
            for j in i + 1..n {
                let angle = self.rotation.get(k).copied().unwrap_or(0.0);
                k += 1;
                if angle == 0.0 {
                    continue;
                }
                let (si, sj) = (self.axes.contains(&i), self.axes.contains(&j));
                let unused = |c: usize| !used.contains(&coord_name(c));
                if !(si && sj || !si && !sj && unused(i) && unused(j)) {
                    return false;
                }
            }
        }
        true
    }

    /// 範囲で重ねる名前と範囲（表示していない座標とパラメータ）
    fn ranged(&self) -> Vec<(String, f64, f64)> {
        let mut out: Vec<(String, f64, f64)> = (0..self.dims)
            .filter(|k| !self.axes.contains(k))
            .filter_map(|k| self.ranges.get(k).copied().flatten().map(|(a, b)| (coord_name(k), a, b)))
            .collect();
        out.extend(self.param_ranges.iter().cloned());
        out
    }

    /// 回転していれば、回転行列 R（n × n）。回転していなければ None
    pub fn rotation_matrix(&self) -> Option<Vec<Vec<f64>>> {
        if self.rotation.iter().all(|&a| a == 0.0) {
            return None;
        }
        let n = rot_dims(self.dims);
        let mut m: Vec<Vec<f64>> = (0..n).map(|i| (0..n).map(|j| (i == j) as u8 as f64).collect()).collect();
        let mut k = 0;
        for i in 0..n {
            for j in i + 1..n {
                let a = self.rotation.get(k).copied().unwrap_or(0.0);
                k += 1;
                if a == 0.0 {
                    continue;
                }
                // 組 (i, j) の平面での回転を、これまでの回転の後にかける
                let (c, s) = (a.cos(), a.sin());
                let (ri, rj) = (m[i].clone(), m[j].clone());
                for (col, (pi, pj)) in ri.iter().zip(&rj).enumerate() {
                    m[i][col] = c * pi - s * pj;
                    m[j][col] = s * pi + c * pj;
                }
            }
        }
        Some(m)
    }

    /// 表示中の座標 q（3 つ）から元の座標 P（`dims` 個）への 1 次式 P = A q + b（GPU の光線の描き方。[`GpuImplicit`]）。
    /// [`rotate_coords`] と同じ置き換え P_k = c_k − Σ_j m[j][k] c_j + Σ_j m[j][k] Q_j で、Q_j は表示中の座標なら q、
    /// 表示していない座標なら断面の値（次元より先の、回転のときだけ使う座標は 0）。戻り値は k ごとに [A_k0, A_k1, A_k2, b_k]
    pub fn gpu_affine(&self) -> Vec<f64> {
        let n = self.dims;
        let rn = rot_dims(n);
        let m = self.rotation_matrix().unwrap_or_else(|| (0..rn).map(|i| (0..rn).map(|j| (i == j) as u8 as f64).collect()).collect());
        let c: Vec<f64> = (0..rn).map(|j| self.center.get(j).copied().unwrap_or(0.0)).collect();
        let sigma = |j: usize| if j < n { self.slice.get(j).copied().unwrap_or(0.0) } else { 0.0 };
        let mut out = Vec::with_capacity(4 * n);
        for k in 0..n {
            let mut b = c[k] - (0..rn).map(|j| m[j][k] * c[j]).sum::<f64>();
            for j in 0..rn {
                if !self.axes.contains(&j) {
                    b += m[j][k] * sigma(j);
                }
            }
            out.extend(self.axes.map(|a| m[a][k]));
            out.push(b);
        }
        out
    }

    /// 表示中の座標の名前（横・奥・縦の順）
    pub(crate) fn shown(&self) -> [String; 3] {
        self.axes.map(coord_name)
    }

    /// 表示していない座標を、断面の値の定数としてパラメータに足す（範囲で重ねるものは除く）。
    /// 次元より先の、回転のときだけ使う座標（3次元のときの w, w₂ など）の値は 0
    pub(crate) fn with_hidden(&self, params: &Params) -> Params {
        let mut ps = params.clone();
        for k in self.dims..rot_dims(self.dims) {
            ps.insert(coord_name(k), C::new(0.0, 0.0));
        }
        for k in 0..self.dims {
            if !self.axes.contains(&k) && self.ranges.get(k).copied().flatten().is_none() {
                ps.insert(coord_name(k), C::new(self.slice.get(k).copied().unwrap_or(0.0), 0.0));
            }
        }
        for (n, _, _) in &self.param_ranges {
            ps.remove(n);
        }
        ps
    }
}

/// 回転に使う次元（次元が5未満でも、5次元の回転ができるようにする）
pub fn rot_dims(dims: usize) -> usize {
    dims.max(5)
}

/// k 番目の座標の名前（0: x, 1: y, 2: z, 3: w, 4: w2, 5: w3, …）
pub fn coord_name(k: usize) -> String {
    match k {
        0 => "x".into(),
        1 => "y".into(),
        2 => "z".into(),
        3 => "w".into(),
        _ => format!("w{}", k - 2),
    }
}

/// 座標の名前 → 番号（座標でなければ None）。w1 は w と同じ
pub fn coord_index(name: &str) -> Option<usize> {
    match name {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        "w" | "w1" => Some(3),
        _ => name.strip_prefix('w')?.parse::<usize>().ok().filter(|&n| n >= 2).map(|n| n + 2),
    }
}

/// 媒介変数（と、範囲で重ねるパラメータ）の数の上限
pub const MAX_PARAM_VARS: usize = 8;

/// 媒介変数として使える名前か（t, u, v, t1, t2, …）
pub(crate) fn is_param_var(name: &str) -> bool {
    matches!(name, "t" | "u" | "v")
        || name.strip_prefix('t').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

/// コンパイル済みのグラフ
pub enum Plot {
    /// 陽関数。表示中の `axis` 番目の座標（0: 横, 1: 奥, 2: 縦）が、残り2つの関数になっている。
    /// `prog` の変数は、残り2つの表示中の座標（軸の順）。
    /// `rotated` が true なら、表示中の座標どうしの回転だけをしているので、
    /// 媒介変数表示 (u, v, f(u, v)) と同じように点を計算してから回転・射影する（[`Space::view_rows`]）。
    Explicit { axis: usize, names: [String; 3], prog: Program, expr: Box<Expr>, rotated: bool },
    /// 陰関数 F = 0。`prog` の変数は表示中の3つの座標。
    /// `from` は、陽関数（z = f(x, y) など）を回して陰関数として計算し直したときの、元の陽関数の形
    /// `local` は、回転が表示中の座標どうしだけのときの、回す前の F（変数は表示中の 3 つの座標）。
    /// これでメッシュを作って回せば、角度が変わっても計算し直さずに済む（lib.rs の build）
    /// `gpu` は、GPU の光線の描き方（計画 12 章の G5・G7）に渡す F（[`GpuImplicit`]）
    Implicit {
        prog: Program, from: Option<String>, local: Option<Program>, gpu: Option<Box<GpuImplicit>>,
        /// 回転した 2D 断面の白丸を、断面に切る前の陽関数から求める。
        slice_source: Option<Box<Plot>>,
    },
    /// 表示していない座標が絡む回転をした陽関数を、傾いた直交座標系 (u, v, t) の陽関数 t = g(u, v) として描くもの（[`tilted`]）。
    /// 表示中の点は q = u e₁ + v e₂ + t e₃（`frame` = [e₁, e₂, e₃]。正規直交・右手系）。
    /// `prog` の変数は (u, v)（利用者が打てない名前 [`TILT_U`]・[`TILT_V`]）。
    /// `color` は複素数の色分け用の f（色分けしないなら None）。
    /// `grid` は格子線用の 2 つの 1 次式 c₀ + c_u u + c_v v（f が使う座標がちょうど 2 つなら、その元の座標の値。None なら (u, v)）。
    /// `label` は元の陽関数の説明（z = f(x, y) など）
    Tilted { prog: Program, color: Option<Program>, frame: [[f64; 3]; 3], grid: Option<[[f64; 3]; 2]>, label: String },
    /// 範囲で重ねた立体。`prog` の変数は表示中の3つの座標と、範囲で動かす名前（`ranges` の順）。
    Solid { prog: Program, ranges: Vec<(f64, f64)>, names: Vec<String> },
    /// 媒介変数表示の曲線。`sets` の1つ1つが1本の曲線で、すべての座標の成分（次元の数）の式。
    /// 変数は `vars`（1つ）。リスト [a, b] を使うと、組み合わせの数だけ曲線ができる。
    /// 回転・射影は、点を計算した後で [`Space::view`] を使って行う（式を計算し直さなくてよい）。
    Curve { sets: Vec<Vec<Program>>, vars: Vec<String> },
    /// 媒介変数表示の曲面。`sets` の1つ1つが1枚の曲面。変数は `vars`（2つ）。
    Surface { sets: Vec<Vec<Program>>, vars: Vec<String> },
    /// 媒介変数表示の立体。`sets` の1つ1つが1つの立体。変数は `vars`（3つ）。
    /// 媒介変数の範囲の直方体を写した形として、直方体の6つの面の像を描く
    Volume { sets: Vec<Vec<Program>>, vars: Vec<String> },
    /// 媒介変数が4つ以上の媒介変数表示。変数は `vars`（4つ以上）。
    /// 媒介変数を2つずつ動かした面（残りは固定）をたくさん描いて表す
    Sheets { sets: Vec<Vec<Program>>, vars: Vec<String> },
    /// ベクトル場（計画 M8）。`progs` は 3 つの成分（変数は表示中の 3 つの座標。回転していれば、回した後の座標の式）。
    /// `rot` は、場の向きに掛ける行列（表示中の座標どうしの回転）
    Field { progs: Vec<Program>, rot: Option<[[f64; 3]; 3]> },
    /// 原点からのベクトルの矢印（行列の列ベクトル・ベクトルの定義。計画 M7・M8）
    Arrows { vectors: Vec<[f64; 3]> },
    /// 点（数列の点 (n, aₙ)。`line` なら順に線で結ぶ: 複素数の数列の軌道）。表示中の座標の空間の点
    Points { points: Vec<[f64; 3]>, line: bool, sequence: Option<SequencePoints> },
    /// 極限だけの行。数値判定と、横を入力・縦を値とする関数のグラフ。
    Limit { graph: crate::limit::Graph, report: crate::limit::Report },
    /// 常微分方程式（計画 M9）。状態 $\mathbf y$ の右辺 `rhs`（変数は状態の名前と独立変数）、初期値の組（組み合わせの数だけ解く）
    Ode(Box<Ode>),
    /// パラメータの定義 `name = value`。`uses` は右辺で使われたほかのパラメータ。
    Param { name: String, value: C, uses: Vec<String> },
    /// リストを含む陽関数・陰関数（例: `z = [1, 2, 3] x`）。リストの要素ごとの図形（組み合わせの数だけ）
    Multi(Vec<Plot>),
    /// 条件を満たす入力点に式を適用した像集合。
    SetImage(Box<crate::set_image::Image>),
    /// 複素平面の図形（[`complex_plane`]）。中身は媒介変数表示 (Re f, Im f, 縦の値) で、`label` は画面に出す説明
    ComplexPlane { inner: Box<Plot>, label: String },
    /// 連立方程式・不等式（[`system`]）: 等式 F_i = 0 と不等式 G_j ≤ 0 を全部満たす所。
    /// 3 次元では、等式の数 m が 0 なら領域、1 なら曲面、2 なら曲線、3 なら点
    System {
        /// 等式が 1 本のとき: その行を今までの道でコンパイルしたもの（陽関数・回した陽関数・陰関数）
        base: Option<Box<Plot>>,
        /// 等式 F_i = 0（変数は表示中の 3 つの座標）
        eqs: Vec<Program>,
        /// 不等式 G_j ≤ 0（変数は表示中の 3 つの座標）
        cuts: Vec<Program>,
        /// 不等式ごとに、厳密（`<`・`>`）か
        strict: Vec<bool>,
        /// 不等式ごとの、境目の面 G_j = 0 を座標の 1 つについて解いた陽関数（軸, 式）の並び（解けなければ空。`solve.rs`）。
        /// 解けた面は、陰関数（格子）ではなく陽関数の道で細かく作る（[`crate::system::region_faces`]）
        faces: Vec<Vec<(usize, Program)>>,
        /// 等式が多項式で、解が有限個のときの、グレブナー基底で求めた解（`groebner.rs`）。
        /// [0] は 3 本の等式の 3D の点、[1] は 2 本の等式の 2D 表示の点（縦の座標を 0 にしたもの）。
        /// 符号が変わらない解（接するだけの交点）も入る。解けなければ None
        exact: [Option<Vec<[f64; 3]>>; 2],
        /// GPU の画素ごとの描き方（2D）に渡す、回転・断面を入れる前の元の座標の式（[`GpuSystem`]）
        gpu: Option<Box<GpuSystem>>,
        /// 不等式だけの領域で、回転が表示中の座標どうしだけなら、回す前の形（[`LocalRegion`]）。
        /// 回転の再生・ドラッグ中は、これで作った面を回すだけで描く（陰関数の `local` と同じ）
        local: Option<Box<LocalRegion>>,
    },
}

/// 回す前の座標の不等式の領域（[`Plot::System`] の `local`）。`cuts`・`faces` は回す前の座標の式、
/// `max` は max G_j（領域が ≤ 0。切り口のふたに使う）
pub struct LocalRegion {
    pub cuts: Vec<Program>,
    pub faces: Vec<Vec<(usize, Program)>>,
    pub max: Program,
}

/// GPU の画素ごとの描き方に渡す連立方程式・不等式（[`GpuImplicit`] と同じ形。等式・不等式で変数の並びをそろえる）。
/// 変数は元の座標 x, y, z, w, …（`dims` 個）の後に、どれかの式が使うスライダーの実数のパラメータ（`params` の順）
#[derive(Debug, Clone)]
pub struct GpuSystem {
    /// 等式 F_i = 0 と不等式 G_j ≤ 0（[`Plot::System`] の `eqs`・`cuts` と同じ順）
    pub eqs: Vec<Program>,
    pub cuts: Vec<Program>,
    pub params: Vec<String>,
    pub dims: usize,
}

/// GPU の光線の描き方に渡す陰関数の F（計画 12 章の G7）。回転も断面も入れる前の、元の座標の式で、
/// 変数は元の座標 x, y, z, w, …（`dims` 個。表示していない座標も変数）の後に、スライダーの実数のパラメータ（`params` の順）。
/// 表示中の座標 q から元の座標 P への 1 次式 P = A q + b（回転・中心・断面の値で決まる。[`Space::gpu_affine`]）と
/// パラメータの値を uniform にして、回転（表示していない座標が絡むものも）・断面・スライダーを動かしても GLSL を作り直さない
#[derive(Debug, Clone)]
pub struct GpuImplicit {
    pub prog: Program,
    pub params: Vec<String>,
    pub dims: usize,
}

impl Plot {
    /// 画面に出す説明
    pub fn kind_label(&self) -> String {
        match self {
            Plot::Explicit { axis, names, .. } => {
                let (a, b) = others(*axis);
                format!("{} = f({}, {})", names[*axis], names[a], names[b])
            }
            Plot::Implicit { from: Some(f), .. } => format!("陰関数（{f} を回したもの）"),
            Plot::Implicit { .. } => "陰関数".into(),
            Plot::Tilted { label, .. } => format!("{label}（回したもの）"),
            Plot::Solid { .. } => "範囲で重ねた立体".into(),
            Plot::Curve { .. } => "媒介変数表示の曲線".into(),
            Plot::Surface { .. } => "媒介変数表示の曲面".into(),
            Plot::Volume { .. } => "媒介変数表示の立体".into(),
            Plot::Sheets { vars, .. } => format!("媒介変数表示の多変数の図形（{} 変数）", vars.len()),
            Plot::Param { .. } => "パラメータ".into(),
            Plot::Field { .. } => "ベクトル場".into(),
            Plot::Ode(o) if o.one.is_some() => "常微分方程式の解曲線".into(),
            Plot::Ode(_) => "常微分方程式の解曲線".into(),
            Plot::Arrows { .. } => "ベクトル".into(),
            Plot::Points { line: true, .. } => "数列の軌道（複素平面）".into(),
            Plot::Points { .. } => "数列の点".into(),
            Plot::Limit { .. } => "極限".into(),
            Plot::Multi(ps) => format!("{} × {}", ps.first().map_or(String::new(), |p| p.kind_label()), ps.len()),
            Plot::SetImage(image) => image.label().into(),
            Plot::ComplexPlane { label, .. } => label.clone(),
            Plot::System { base, eqs, cuts, .. } => {
                let cut = if cuts.is_empty() { "" } else { "（不等式で切り取り）" };
                match (eqs.len(), base) {
                    (0, Some(b)) => format!("{}{cut}", b.kind_label()),
                    (0, _) if cuts.len() > 1 => format!("不等式の領域（{} 本）", cuts.len()),
                    (0, _) => "不等式の領域".into(),
                    (1, Some(b)) => format!("{}{cut}", b.kind_label()),
                    (1, None) => format!("陰関数{cut}"),
                    (2, _) => format!("連立方程式の曲線{cut}"),
                    _ => "連立方程式の点".into(),
                }
            }
        }
    }

    /// 媒介変数の名前（媒介変数表示でなければ空）
    pub fn param_vars(&self) -> Vec<String> {
        match self {
            Plot::Ode(o) if o.one.is_none() => vec!["t".into()],
            Plot::Curve { vars, .. }
            | Plot::Surface { vars, .. }
            | Plot::Volume { vars, .. }
            | Plot::Sheets { vars, .. } => vars.clone(),
            Plot::Multi(ps) => ps.first().map_or(Vec::new(), |p| p.param_vars()),
            // 点（媒介変数を使わない）なら、範囲の欄を出さない
            Plot::ComplexPlane { label, .. } if label.starts_with("複素平面の点") => Vec::new(),
            Plot::ComplexPlane { inner, .. } => inner.param_vars(),
            Plot::System { base: Some(b), .. } => b.param_vars(),
            _ => Vec::new(),
        }
    }

    /// 使われたパラメータ（と、断面で値を固定した座標）の名前
    pub fn used_params(&self) -> Vec<String> {
        let progs: Vec<&Program> = match self {
            Plot::Explicit { prog, .. } | Plot::Implicit { prog, .. } | Plot::Limit { graph: crate::limit::Graph::Function(prog), .. } => vec![prog],
            Plot::Limit { .. } => return Vec::new(),
            Plot::Tilted { prog, color, .. } => std::iter::once(prog).chain(color.as_ref()).collect(),
            Plot::Solid { prog, names, .. } => {
                // 範囲で重ねている名前も「使ったもの」に含める（範囲が変わったら計算し直すため）
                let mut out = names.clone();
                out.extend(prog.used_params().iter().filter(|n| !names.contains(n)).cloned());
                return out;
            }
            Plot::Curve { sets, .. }
            | Plot::Surface { sets, .. }
            | Plot::Volume { sets, .. }
            | Plot::Sheets { sets, .. } => sets.iter().flatten().collect(),
            Plot::Param { uses, .. } => return uses.clone(),
            Plot::Field { progs, .. } => progs.iter().collect(),
            Plot::Ode(o) => {
                let mut out = o.uses.clone();
                for n in o.rhs.iter().flat_map(|p| p.used_params()) {
                    if !out.contains(n) {
                        out.push(n.clone());
                    }
                }
                return out;
            }
            Plot::Arrows { .. } | Plot::Points { .. } => return Vec::new(),
            Plot::ComplexPlane { inner, .. } => return inner.used_params(),
            Plot::SetImage(image) => return image.used_params(),
            Plot::System { base, eqs, cuts, .. } => {
                let mut out: Vec<String> = base.as_ref().map_or(Vec::new(), |b| b.used_params());
                for n in eqs.iter().chain(cuts).flat_map(|p| p.used_params()) {
                    if !out.contains(n) {
                        out.push(n.clone());
                    }
                }
                return out;
            }
            Plot::Multi(ps) => {
                let mut out: Vec<String> = Vec::new();
                for n in ps.iter().flat_map(|p| p.used_params()) {
                    if !out.contains(&n) {
                        out.push(n);
                    }
                }
                return out;
            }
        };
        let mut out: Vec<String> = Vec::new();
        for p in progs {
            for n in p.used_params() {
                if !out.contains(n) {
                    out.push(n.clone());
                }
            }
        }
        out
    }
}

/// axis 以外の2つの軸（小さい順）
pub fn others(axis: usize) -> (usize, usize) {
    match axis {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    }
}

/// `offset` 文字目から始まる部分を解析する（エラーの位置を全体の位置に直す）
fn parse_at(src: &str, offset: usize, params: &Params) -> Result<Expr, String> {
    let e = parse_with(src, &crate::expand::names()).map_err(|e| ParseError { pos: e.pos + offset, ..e }.to_string())?;
    crate::expand::expand(&e, params)
}

/// 式を解析して、数学の記法を展開する（`expand.rs`）。定義の行（`f(x) = …`）は展開せずに、その印を返す
/// （定義の行は、[`compile_with`] が「縦 = f(横, 奥)」などの形にしてから描く）
fn parse_expanded(source: &str, params: &Params) -> Result<Vec<crate::parser::Chain>, String> {
    let mut chains = parse_system_with(source, &crate::expand::names()).map_err(|e| e.to_string())?;
    for chain in &mut chains {
        // 逆関数 y = f⁻¹(x) → f(y) = x
        if chain.rels == [Rel::Eq] {
            let (l, r) = (chain.exprs[0].clone(), chain.exprs[1].clone());
            let (l, r) = crate::expand::rewrite_inverse(l, r);
            chain.exprs = vec![l, r];
        }
        // 初項の行 `a_1 = 8`（字句では a1 = 8）の左辺は、数列の項ではなくパラメータの名前のまま
        let first_term = chain.rels == [Rel::Eq] && matches!(&chain.exprs[0], Expr::Var(v) if crate::expand::is_term_name(v));
        for (k, e) in chain.exprs.iter_mut().enumerate() {
            if k == 0 && first_term {
                continue;
            }
            *e = crate::expand::expand(e, params)?;
        }
    }
    Ok(chains)
}

/// 定義の行（`f(x) = …`・`h = f + g`）を、描く形の式に置き換える。定義の行でなければ None。
/// 引数 1 つは「奥 = f(横)」、2 つは「縦 = f(横, 奥)」。値が組の関数は媒介変数表示（引数を t / u, v / u, v, t にする）。
/// 描けない定義（引数 3 つ以上・引数なし）は Some(None)
fn definition_source(source: &str, space: &Space) -> Option<Option<Vec<crate::parser::Chain>>> {
    let names = crate::expand::names();
    if names.is_empty() {
        return None;
    }
    let chains = parse_system_with(source, &names).ok()?;
    let [chain] = chains.as_slice() else { return None };
    if chain.rels != [Rel::Eq] {
        return None;
    }
    let (name, arity, tuple) = crate::expand::definition_line(&chain.exprs[0])?;
    let shown = space.shown();
    // 値が組で、仮引数が全部座標の名前（F(x, y, z) = (…)）なら、ベクトル場（計画 M8）
    if let (true, Expr::Apply { args, .. }) = (tuple, &chain.exprs[0]) {
        let coords = args.iter().all(|a| matches!(a, Expr::Var(v) if coord_index(v).is_some_and(|k| k < space.dims)));
        if coords && (2..=3).contains(&arity) {
            crate::expand::mark_def_line(&name, Some("ベクトル場".into()));
            return Some(Some(vec![crate::parser::Chain { exprs: vec![chain.exprs[0].clone()], rels: vec![], pos: vec![0] }]));
        }
    }
    let drawn = match (arity, tuple) {
        (1, true) => Some(format!("曲線 {name}(t)")),
        (2, true) => Some(format!("曲面 {name}(u, v)")),
        (3, true) => Some(format!("立体 {name}(u, v, t)")),
        (1, false) => Some(format!("{} = {name}({})", shown[1], shown[0])),
        (2, false) => Some(format!("{} = {name}({}, {})", shown[2], shown[0], shown[1])),
        _ => None,
    };
    crate::expand::mark_def_line(&name, drawn);
    let call = |args: Vec<&str>| Expr::Apply { name: name.clone(), primes: 0, args: args.into_iter().map(|a| Expr::Var(a.into())).collect() };
    let one = |e: Expr| crate::parser::Chain { exprs: vec![e], rels: vec![], pos: vec![0] };
    let eq = |l: Expr, r: Expr| crate::parser::Chain { exprs: vec![l, r], rels: vec![Rel::Eq], pos: vec![0, 0] };
    Some(match (arity, tuple) {
        (1, true) => Some(vec![one(call(vec!["t"]))]),
        (2, true) => Some(vec![one(call(vec!["u", "v"]))]),
        (3, true) => Some(vec![one(call(vec!["u", "v", "t"]))]),
        (1, false) => Some(vec![eq(Expr::Var(shown[1].clone()), call(vec![&shown[0]]))]),
        (2, false) => Some(vec![eq(Expr::Var(shown[2].clone()), call(vec![&shown[0], &shown[1]]))]),
        _ => None,
    })
}

/// 次元より先の座標（3次元なのに w を使った など）を使っていたらエラーにする
fn check_dims(e: &Expr, space: &Space) -> Result<(), String> {
    let mut vars = Vec::new();
    e.collect_vars(&mut vars);
    for v in vars {
        if let Some(k) = coord_index(&v) {
            if k >= space.dims {
                return Err(format!("{v} を使うには、次元を {} 以上にしてください", k + 1));
            }
        }
    }
    Ok(())
}

/// パラメータなし・ふつうの3次元でコンパイルする
pub fn compile(source: &str) -> Result<Plot, String> {
    compile_with(source, &Params::new(), &Space::standard())
}

/// 式をコンパイルする。`params` はほかの行で定義されたパラメータ。
pub fn compile_with(source: &str, params: &Params, space: &Space) -> Result<Plot, String> {
    if let Some(plot) = limit_line(source, params)? {
        return Ok(plot);
    }
    // 論理・集合の記号があれば、論理式として描く（計画 M11）
    if source.chars().any(|c| crate::parser::LOGIC_CHARS.contains(c)) {
        return compile_logic(source, params, space);
    }
    let name = source.trim().trim_start_matches('{').trim_end_matches('}');
    if let Some(set) = crate::expand::set_def(name) {
        return compile_formula(&crate::parser::Formula::Set(set), params, space);
    }
    // ∃・∀（計画 M12）
    if let Some(p) = quantified(source, params, space)? {
        return Ok(p);
    }
    // 直交 u ⟂ v（u · v = 0）・平行 u ∥ v（u × v = 0）。計画 M13
    if let Some(p) = perp_line(source, params, space)? {
        return Ok(p);
    }
    if source.trim_start().starts_with("@cobweb") {
        return cobweb_line(source, params);
    }
    if let Some((head, _)) = source.split_once('(') {
        match head.trim() {
            "@orbit" | "@orbitpoints" => return orbit_line(source, params),
            "@recurrence" => return recurrence_line(source, params),
            "@bifurcation" => return bifurcation_line(source, params),
            _ => {}
        }
    }
    // 数列の定義の行（計画 M10）
    if let Some(p) = sequence_line(source, params)? {
        return Ok(p);
    }
    // 常微分方程式（計画 M9）
    if let Some(p) = ode_line(source, params, space)? {
        return Ok(p);
    }
    // 停留点 `@crit f` だけの行（計画 N4 の前半）
    if let Some(p) = critical_line(source, params, space)? {
        return Ok(p);
    }
    // 行列を描く行（ker・Im・行列式だけの行・行列だけの行・行列やベクトルの定義。計画 M7）
    if let Some(p) = matrix_line(source, params, space)? {
        return Ok(p);
    }
    let chains = match definition_source(source, space) {
        // 定義の行: 描く形の式に置き換えて展開する（描けない定義は、図形なし）
        Some(Some(mut chains)) => {
            for chain in &mut chains {
                for e in &mut chain.exprs {
                    *e = crate::expand::expand(e, params)?;
                }
            }
            chains
        }
        Some(None) => return Ok(Plot::Multi(Vec::new())),
        None => parse_expanded(source, params)?,
    };
    // ± があれば、符号を +1 と −1 にした 2 つの図形（全部の ± がそろって動く）
    let pm = crate::expand::PM_SIGN;
    if chains.iter().any(|c| c.exprs.iter().any(|e| e.uses_var(pm))) {
        let mut plots = Vec::with_capacity(2);
        for s in [1.0, -1.0] {
            let mut cs = chains.clone();
            for c in &mut cs {
                for e in &mut c.exprs {
                    *e = e.substitute(&|v: &str| (v == pm).then_some(Expr::Num(s)));
                }
            }
            plots.push(compile_chains(cs, params, space)?);
        }
        if plots.iter().any(|p| matches!(p, Plot::Param { .. })) {
            return Err("± はパラメータの定義には使えません".into());
        }
        return Ok(Plot::Multi(plots));
    }
    compile_chains(chains, params, space)
}

/// 極限だけの行には、極限の判定と関数のグラフを出す。
fn limit_line(source: &str, params: &Params) -> Result<Option<Plot>, String> {
    let start = source.trim_start_matches(|c: char| c.is_whitespace() || c == '(');
    if !start.starts_with("@lim") && !start.starts_with("@seqlim") {
        return Ok(None);
    }
    let Ok(chains) = parse_system_with(source, &crate::expand::names()) else { return Ok(None) };
    let [chain] = chains.as_slice() else { return Ok(None) };
    let ([], [Expr::Binder { op, var, args, body }]) = (chain.rels.as_slice(), chain.exprs.as_slice()) else { return Ok(None) };
    if !matches!(op.as_str(), "lim" | "seqlim") {
        return Ok(None);
    }
    let (graph, report) = crate::expand::limit_line(op, var, args, body, params)?;
    Ok(Some(Plot::Limit { graph, report }))
}

/// 値だけの式（定積分など）と同じ高さの面。
pub(crate) fn constant_plot(value: f64, space: &Space) -> Result<Plot, String> {
    let chain = crate::parser::Chain { exprs: vec![Expr::Num(value)], rels: Vec::new(), pos: vec![0] };
    compile_chains(vec![chain], &Params::new(), space)
}

/// 停留点 `@crit f` だけの行（計画 N4 の前半。[`crate::critical`]）。f が使う表示中の座標（1〜3 個）について
/// $\nabla f = \mathbf 0$ の点を表示範囲の中で探し、極大・極小・鞍点に分けて説明に出す。図には点を描く:
/// 1 変数 f(x) は (x, f(x))、2 変数 f(x, y) は曲面 z = f(x, y) の上の点 (x, y, f)、3 変数はその点
fn critical_line(source: &str, params: &Params, space: &Space) -> Result<Option<Plot>, String> {
    if !source.contains("@crit") {
        return Ok(None);
    }
    let Ok(chains) = parse_system_with(source, &crate::expand::names()) else { return Ok(None) };
    let [chain] = chains.as_slice() else { return Err("@crit f は、1 行にそれだけを書いてください".into()) };
    let (rels, exprs) = (&chain.rels, &chain.exprs);
    let arg = match (rels.as_slice(), exprs.as_slice()) {
        ([], [Expr::Op { op, args }]) if op == "crit" && args.len() == 1 => &args[0],
        _ => return Err("@crit f は、1 行にそれだけを書いてください".into()),
    };
    // 関数の名前だけ（`@crit f`）なら、表示中の座標を引数にして呼ぶ（f(x)・f(x, y)・f(x, y, z) の、引数の数が合うもの）
    let f = match (arg, crate::expand::expand(arg, params)) {
        (_, Ok(f)) => f,
        (Expr::Var(name), Err(e)) => (1..=3)
            .find_map(|n| {
                let args = (0..n).map(|a| Expr::Var(coord_name(space.axes[a]))).collect();
                crate::expand::expand(&Expr::Apply { name: name.clone(), primes: 0, args }, params).ok()
            })
            .ok_or(e)?,
        (_, Err(e)) => return Err(e),
    };
    if matches!(f, Expr::Tuple(_) | Expr::List(_) | Expr::Matrix { .. }) {
        return Err("停留点は、値が数になる関数で求めます（ベクトル・リストは使えません）".into());
    }
    if space.rotation_matrix().is_some() || space.complex_coords || space.coord_names.is_some() {
        return Err("停留点は、回転・座標系・座標の複素数化をしていないときに求めます".into());
    }
    let mut used = Vec::new();
    f.collect_vars(&mut used);
    let mut axes: Vec<usize> = Vec::new();
    for v in &used {
        if let Some(c) = coord_index(v) {
            match space.axes.iter().position(|&a| a == c) {
                Some(a) if !axes.contains(&a) => axes.push(a),
                Some(_) => {}
                None => return Err(format!("停留点は、表示中の座標の関数で求めます（{v} は表示していません）")),
            }
        }
    }
    axes.sort();
    if axes.is_empty() {
        return Err("停留点を求める関数に、座標（x, y, z）がありません".into());
    }
    let vars: Vec<String> = axes.iter().map(|&a| coord_name(space.axes[a])).collect();
    let vb = crate::expand::view_box();
    let boxes: Vec<(f64, f64)> = axes.iter().map(|&a| (vb[2 * a].min(vb[2 * a + 1]), vb[2 * a].max(vb[2 * a + 1]))).collect();
    let found = crate::critical::find(&f, &vars, &boxes, params)?;
    // 値を置く軸（1 変数は、横なら奥（2D の縦）、それ以外は横。2 変数は残りの軸。3 変数は無し）
    let value_axis = match axes.len() {
        1 => Some(if axes[0] == 1 { 0 } else { 1 }),
        2 => (0..3).find(|a| !axes.contains(a)),
        _ => None,
    };
    let points: Vec<[f64; 3]> = found
        .points
        .iter()
        .filter(|c| c.value.is_finite())
        .map(|c| {
            let mut p = [0.0; 3];
            for (j, &a) in axes.iter().enumerate() {
                p[a] = c.point[j];
            }
            if let Some(a) = value_axis {
                p[a] = c.value;
            }
            p
        })
        .collect();
    let at = |c: &crate::critical::Critical| {
        let xs: Vec<String> = c.point.iter().map(|v| crate::critical::num_text(*v)).collect();
        if xs.len() == 1 { format!("{} = {}", vars[0], xs[0]) } else { format!("({})", xs.join(", ")) }
    };
    // 判定できない点がたくさんあれば、停留点は孤立していない（(x − y)² の直線・sin x sin y sin z の座標軸など）
    let degenerate = found.points.iter().filter(|c| c.class == crate::critical::Class::Degenerate).count();
    let spread = if degenerate >= 5 && !found.more { "。判定できない点が多いので、線・面に沿って続いている可能性" } else { "" };
    let mut parts = vec![format!(
        "停留点 ∇f = 0（{} の範囲で {} 個{}{spread}。種のマスより狭く固まった点は見落とすことがある）",
        boxes.iter().zip(&vars).map(|((lo, hi), v)| format!("{} ≤ {v} ≤ {}", crate::critical::num_text(*lo), crate::critical::num_text(*hi))).collect::<Vec<_>>().join(", "),
        found.points.len(),
        if found.more { " 以上（多すぎるので打ち切り。線・面に沿って続いている可能性もある）" } else { "" }
    )];
    for c in &found.points {
        parts.push(format!("{}: {} f = {}（{}）", at(c), crate::critical::class_name(c.class), crate::critical::num_text(c.value), c.reason));
    }
    crate::expand::set_kind(parts.join(" ／ "));
    Ok(Some(Plot::Points { points, line: false, sequence: None }))
}

/// 行列を描く行（計画 M7 の 3.4）。そうでなければ None（今までの道へ）
/// - `@ker(A)`・`Im(A)`: 零空間・像（原点を通る点・直線・平面・全体）。成分は数とパラメータ
/// - `@det(A)` だけの行: 列ベクトルが張る平行六面体（2×2 は平行四辺形）。成分に座標があれば、今までどおり「縦 = 式」
/// - 行列だけの行と、行列・ベクトルの定義 `A = …`: 列ベクトル（ベクトル）を原点からの線分で描く
fn matrix_line(source: &str, params: &Params, space: &Space) -> Result<Option<Plot>, String> {
    use crate::functions::Func;
    use crate::linalg::Matrix;
    let Ok(chains) = parse_system_with(source, &crate::expand::names()) else { return Ok(None) };
    let [chain] = chains.as_slice() else { return Ok(None) };
    let n = space.dims;
    let dim_word = |k: usize| match k {
        0 => "0 次元。原点".to_string(),
        1 => "1 次元。直線".to_string(),
        2 => "2 次元。平面".to_string(),
        k => format!("{k} 次元"),
    };
    // 定義の行 `A = …`（行列・ベクトル）: 右辺を描く
    let (expr, def) = match (chain.rels.as_slice(), chain.exprs.as_slice()) {
        ([], [e]) => (e, None),
        ([Rel::Eq], [Expr::Var(name), _]) if matches!(crate::expand::definition_line(&chain.exprs[0]), Some((_, 0, _))) => {
            (&chain.exprs[0], Some(name.clone()))
        }
        _ => return Ok(None),
    };
    // tr A・det A・rank A などだけの行: 値を説明に出す（図形は今までどおり、高さがその値の平面）
    if let Expr::Op { op, args } = expr {
        if def.is_none() && args.len() == 1 && matches!(op.as_str(), "tr" | "det" | "rank" | "dim" | "norm" | "angle" | "dot") {
            if let Ok(Expr::Num(v)) = crate::expand::expand(expr, params) {
                let name = match op.as_str() {
                    "norm" => "‖ ‖".to_string(),
                    "angle" => "角".to_string(),
                    "dot" => "内積".to_string(),
                    o => o.to_string(),
                };
                crate::expand::set_kind(format!("{name} = {}（高さがこの値の平面）", format_number(v)));
            }
        }
    }
    let numeric = |e: &Expr| -> Result<Option<Matrix>, String> {
        Ok(match crate::expand::matrix_value(e, params)? {
            Some((rows, cols, items)) => {
                crate::expand::note_names(&items);
                crate::expand::numbers(&items, params).map(|a| Matrix::new(rows, cols, a))
            }
            None => None,
        })
    };
    match expr {
        Expr::Op { op, args } if op == "eig" && args.len() == 1 => {
            let label = matrix_label(&args[0]);
            let Some(m) = numeric(&args[0])? else {
                return Err(format!("eig {label} の行列の成分は、数とパラメータだけで書いてください"));
            };
            eigen(&label, &m, params, space).map(Some)
        }
        Expr::Op { op, args } if (op == "ker" || op == "im") && args.len() == 1 => {
            let name = format!("{} {}", if op == "ker" { "ker" } else { "Im" }, matrix_label(&args[0]));
            let Some(m) = numeric(&args[0])? else {
                return Err(format!("{name} の行列の成分は、数とパラメータだけで書いてください"));
            };
            subspace(&name, &m, op == "ker", params, space, &dim_word).map(Some)
        }
        // 行全体が Im(行列) なら像。像にするのは、列が 2 つ以上で、成分が全部実数の行列だけ。
        // スカラー・ベクトル（列が 1 つ）・複素数の成分がある行列は、今までどおり虚部（成分ごと）
        Expr::Call(Func::Im, args) if args.len() == 1 => match crate::expand::matrix_value(&args[0], params) {
            Ok(Some((rows, cols, items))) if cols > 1 => match crate::expand::numbers(&items, params) {
                Some(a) => subspace(&format!("Im {}", matrix_label(&args[0])), &Matrix::new(rows, cols, a), false, params, space, &dim_word).map(Some),
                None => Ok(None),
            },
            _ => Ok(None),
        },
        Expr::Op { op, args } if op == "det" && args.len() == 1 => {
            let Some(m) = numeric(&args[0])? else { return Ok(None) };
            if m.rows != m.cols || !(2..=3).contains(&m.rows) || m.rows > n {
                return Ok(None);
            }
            // 列ベクトル a_k の u a₁ + v a₂ + t a₃（0 ≤ u, v, t ≤ 1）
            let names = ["u", "v", "t"];
            let items: Vec<Expr> = (0..m.rows).map(|i| linear(0.0, (0..m.cols).map(|j| (m.at(i, j), Expr::Var(names[j].into()))).collect())).collect();
            for v in &names[..m.cols] {
                crate::expand::set_param_range(v, 0.0, 1.0);
            }
            let det = match crate::expand::expand(expr, params)? {
                Expr::Num(v) => v,
                e => crate::expand::numbers(&[e], params).and_then(|v| v.first().copied()).unwrap_or(f64::NAN),
            };
            let shape = if m.rows == 3 { "平行六面体。体積は |det|" } else { "平行四辺形。面積は |det|" };
            crate::expand::set_kind(format!("det = {}（列ベクトルが張る{shape}）", format_number(det)));
            parametric(items, params, space).map(Some)
        }
        _ => {
            let Some(m) = numeric(expr)? else { return Ok(None) };
            // 行列: 正方の 2×2・3×3 の列ベクトル。ベクトル: 2・3 成分
            // 名前の無い数の組 (1, 2, 3) は、今までどおり点（媒介変数表示）
            if m.cols == 1 && def.is_none() {
                return Ok(None);
            }
            let vectors: Vec<Vec<f64>> = if m.cols == 1 && (2..=3).contains(&m.rows) {
                vec![m.a.clone()]
            } else if (2..=3).contains(&m.rows) && m.cols >= 2 {
                // 2×2・3×3 に限らず、行が 2 か 3 なら列ベクトルを描く（2×3 の行列は平面の 3 本のベクトル）
                (0..m.cols).map(|j| (0..m.rows).map(|i| m.at(i, j)).collect()).collect()
            } else if let Some(name) = def {
                // そのほかの大きさは、図形なし
                crate::expand::mark_def_line(&name, None);
                return Ok(Some(Plot::Multi(Vec::new())));
            } else {
                return Ok(None);
            };
            if m.rows > n {
                return Ok(None);
            }
            let what = if m.cols == 1 { "ベクトル" } else { "列ベクトル" };
            match def {
                Some(name) => crate::expand::mark_def_line(&name, Some(what.into())),
                None => crate::expand::set_kind(format!("行列の{what}")),
            }
            // ふつうの 3 次元・回転なしなら矢印。それ以外（4 次元以上・回転あり）は、射影できる線分のまま
            if n == 3 && space.axes == [0, 1, 2] && space.rotation_matrix().is_none() && !space.complex_coords {
                let vectors = vectors.iter().map(|v| [v[0], v[1], v.get(2).copied().unwrap_or(0.0)]).collect();
                return Ok(Some(Plot::Arrows { vectors }));
            }
            segments(&vectors, params, space).map(Some)
        }
    }
}

/// 数を、説明に出す短い文字列にする（整数はそのまま、ほかは有効 6 桁）
fn format_number(v: f64) -> String {
    if v.is_finite() && (v - v.round()).abs() < 1e-9 * v.abs().max(1.0) && v.abs() < 1e15 {
        format!("{}", v.round() as i64).replace('-', "−")
    } else {
        let s = format!("{v:.6}");
        s.trim_end_matches('0').trim_end_matches('.').replace('-', "−")
    }
}

/// 原点からのベクトルを、線分（媒介変数 t ∈ [0, 1] の曲線）で描く
fn segments(vectors: &[Vec<f64>], params: &Params, space: &Space) -> Result<Plot, String> {
    crate::expand::set_param_range("t", 0.0, 1.0);
    let mut all = Vec::new();
    for v in vectors {
        let items = v.iter().map(|c| linear(0.0, vec![(*c, Expr::Var("t".into()))])).collect();
        match parametric(items, params, space)? {
            Plot::Curve { sets, .. } => all.extend(sets),
            _ => return Err("内部エラー: ベクトルの線分".into()),
        }
    }
    Ok(Plot::Curve { sets: all, vars: vec!["t".into()] })
}

/// 固有値と固有空間（`@eig(A)` だけの行）。説明に固有値ごとの重複度・固有空間の式を出し、図には実数の固有値の固有ベクトル
/// （固有空間の基底。長さは表示範囲の 1/3）を矢印で描く。実数でない固有値は説明だけ
fn eigen(label: &str, m: &crate::linalg::Matrix, params: &Params, space: &Space) -> Result<Plot, String> {
    if m.rows != m.cols {
        return Err(format!("固有値は正方行列で求めます（{label} は {}×{}）", m.rows, m.cols));
    }
    let n = m.rows;
    let mut parts: Vec<String> = Vec::new();
    let mut vectors: Vec<Vec<f64>> = Vec::new();
    for ev in m.eigenvalues() {
        let mult = if ev.mult > 1 { format!("（{} 重）", ev.mult) } else { String::new() };
        if ev.im != 0.0 {
            // 共役の組は、虚部が正のほうだけ書く
            if ev.im > 0.0 {
                let im = if ev.im == 1.0 { "i".to_string() } else { format!("{}i", format_number(ev.im)) };
                let value = if ev.re == 0.0 { format!("±{im}") } else { format!("{} ± {im}", format_number(ev.re)) };
                parts.push(format!("λ = {value}{mult}（実数でないので固有空間は描かない）"));
            }
            continue;
        }
        let shifted = m.shifted(ev.re);
        let basis = shifted.null_space_basis_tol(1e-7);
        let eqs = shifted.reduced_rows_tol(1e-7);
        let geo = basis.len();
        let short = if geo < ev.mult { format!("（固有空間は {geo} 次元で、重複度より小さい。対角化できない）") } else { String::new() };
        parts.push(format!("λ = {}{mult}: {}{short}", format_number(ev.re), span_formula(&format!("E({})", format_number(ev.re)), n, &basis, &eqs)));
        for b in &basis {
            let len = b.iter().map(|x| x * x).sum::<f64>().sqrt();
            if len > 0.0 {
                vectors.push(b.iter().map(|x| x / len).collect());
            }
        }
    }
    crate::expand::set_kind(format!("{label} の固有値 ／ {}", parts.join(" ／ ")));
    if n > space.dims || vectors.is_empty() {
        return Ok(Plot::Multi(Vec::new()));
    }
    // 長さは表示範囲の 1/3（単位ベクトルでは小さすぎて見えない）
    let reach = crate::expand::reach() / 3.0;
    let vectors: Vec<Vec<f64>> = vectors.iter().map(|v| v.iter().map(|x| x * reach).collect()).collect();
    if space.dims == 3 && space.axes == [0, 1, 2] && space.rotation_matrix().is_none() && !space.complex_coords {
        let vectors = vectors.iter().map(|v| [v[0], v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)]).collect();
        return Ok(Plot::Arrows { vectors });
    }
    let vectors: Vec<Vec<f64>> = vectors.iter().map(|v| (0..space.dims).map(|i| v.get(i).copied().unwrap_or(0.0)).collect()).collect();
    segments(&vectors, params, space)
}

/// 行列の引数の呼び名（名前ならその名前、A − 3I のような式なら括弧で囲んだ式、そのほかは A）
fn matrix_label(e: &Expr) -> String {
    fn text(e: &Expr) -> Option<String> {
        Some(match e {
            Expr::Var(n) => n.strip_prefix("vec_").unwrap_or(n).to_string(),
            Expr::Num(v) => format_number(*v),
            Expr::Const(_) => return None,
            Expr::Neg(x) => format!("−{}", text(x)?),
            Expr::Bin(BinOp::Add, a, b) => format!("{} + {}", text(a)?, text(b)?),
            Expr::Bin(BinOp::Sub, a, b) => format!("{} − {}", text(a)?, text(b)?),
            Expr::Bin(BinOp::Mul, a, b) => format!("{}{}", text(a)?, text(b)?),
            Expr::Bin(BinOp::Pow, a, b) => format!("{}^{}", text(a)?, text(b)?),
            _ => return None,
        })
    }
    match e {
        Expr::Var(_) => text(e).unwrap_or_else(|| "A".into()),
        e => text(e).map(|t| format!("({t})")).unwrap_or_else(|| "A".into()),
    }
}

/// ベクトルを見やすくする: 成分が小さい整数の比なら、その整数にする（公約数で割る）。そうでなければ、絶対値の最大の成分を 1 にする。
/// 向きは、最初の 0 でない成分が正になるようにそろえる
fn nice_vector(v: &[f64]) -> Vec<f64> {
    let max = v.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    if max == 0.0 {
        return v.to_vec();
    }
    let mut u: Vec<f64> = v.iter().map(|x| x / max).collect();
    if let Some(first) = u.iter().find(|x| x.abs() > 1e-9) {
        if *first < 0.0 {
            u.iter_mut().for_each(|x| *x = -*x);
        }
    }
    for d in 1..=24 {
        let k: Vec<f64> = u.iter().map(|x| x * d as f64).collect();
        if k.iter().all(|x| (x - x.round()).abs() < 1e-7) {
            let ints: Vec<i64> = k.iter().map(|x| x.round() as i64).collect();
            let g = ints.iter().fold(0i64, |g, &x| gcd(g, x.abs())).max(1);
            return ints.iter().map(|&x| (x / g) as f64).collect();
        }
    }
    u
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// ベクトルの見せ方 (−2, 1)
fn vector_text(v: &[f64]) -> String {
    format!("({})", v.iter().map(|x| format_number(*x)).collect::<Vec<_>>().join(", "))
}

/// 1 次式 = 0 の見せ方（x + 2y = 0・−2x + z = 0）。names は座標の名前
fn equation_text(c: &[f64], names: &[String]) -> String {
    let mut s = String::new();
    for (x, n) in c.iter().zip(names) {
        if x.abs() < 1e-12 {
            continue;
        }
        let mag = format_number(x.abs());
        let coef = if mag == "1" { String::new() } else { mag };
        if s.is_empty() {
            s = format!("{}{coef}{n}", if *x < 0.0 { "−" } else { "" });
        } else {
            s += &format!(" {} {coef}{n}", if *x < 0.0 { "−" } else { "+" });
        }
    }
    format!("{} = 0", if s.is_empty() { "0".into() } else { s })
}

/// 零空間・像の式（説明に出す）: 張るベクトル・媒介変数表示・方程式。
/// ker A は、掃き出した A の独立な行が方程式、自由変数ごとの解が張るベクトル。Im A は、ピボットの列が張るベクトル、
/// ker Aᵀ のベクトル n ごとの n · y = 0 が方程式
fn subspace_formula(name: &str, m: &crate::linalg::Matrix, ker: bool) -> String {
    let ambient = if ker { m.cols } else { m.rows };
    let basis = if ker { m.null_space_basis() } else { m.pivot_columns() };
    let eqs = if ker { m.reduced_rows() } else { m.transpose().null_space_basis() };
    span_formula(name, ambient, &basis, &eqs)
}

/// 部分空間の式: 張るベクトル basis・方程式の係数 eqs（どちらも見やすくして出す）
fn span_formula(name: &str, ambient: usize, basis: &[Vec<f64>], eqs: &[Vec<f64>]) -> String {
    let names: Vec<String> = (0..ambient).map(coord_name).collect();
    let basis: Vec<Vec<f64>> = basis.iter().map(|v| nice_vector(v)).collect();
    let eqs: Vec<Vec<f64>> = eqs.iter().map(|v| nice_vector(v)).collect();
    let k = basis.len();
    let space_name = format!("ℝ{}", if ambient <= 9 { ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'][ambient].to_string() } else { format!("^{ambient}") });
    if k == 0 {
        return format!("{name} = {{0}}");
    }
    if k == ambient {
        return format!("{name} = {space_name}");
    }
    let ps: &[&str] = match k {
        1 => &["t"],
        2 => &["s", "t"],
        3 => &["r", "s", "t"],
        _ => &[],
    };
    let point = format!("({})", names.join(", "));
    let span = format!("span{{{}}}", basis.iter().map(|v| vector_text(v)).collect::<Vec<_>>().join(", "));
    let param = if ps.is_empty() {
        String::new()
    } else {
        format!("、{point} = {}", basis.iter().zip(ps).map(|(v, p)| format!("{p}{}", vector_text(v))).collect::<Vec<_>>().join(" + "))
    };
    let equations = eqs.iter().map(|c| equation_text(c, &names)).collect::<Vec<_>>().join(", ");
    format!("{name} = {span}{param}、{equations}")
}

/// 零空間（`ker`）・像（`Im`）を、原点を通る点・直線・平面（表示範囲の箱で切る）・全体として描く。説明には式も出す
fn subspace(name: &str, m: &crate::linalg::Matrix, ker: bool, params: &Params, space: &Space, dim_word: &dyn Fn(usize) -> String) -> Result<Plot, String> {
    let n = space.dims;
    let ambient = if ker { m.cols } else { m.rows };
    let formula = subspace_formula(name, m, ker);
    // 次元より小さい空間の集合（2 列の行列の ker など）は、最初の座標の平面・直線の中に描く（2D 表示の平面の中の直線など）
    if ambient > n {
        let side = if ker { "列" } else { "行" };
        let label = name.split(' ').nth(1).unwrap_or("A");
        return Err(format!(
            "{formula}。{ambient} 次元の空間の中の集合なので描けません（{label} の{side}を {n} つ以下にするか、次元を {ambient} にしてください）"
        ));
    }
    let mut basis = if ker { m.null_space() } else { m.column_space() };
    basis.iter_mut().for_each(|b| b.resize(n, 0.0));
    let k = basis.len();
    crate::expand::set_kind(format!("{formula}（{}）", dim_word(k)));
    let reach = crate::expand::reach();
    let names = ["u", "v", "t"];
    // 表示中の 3 つの座標を全部含む（全体）なら、表示範囲の箱
    if k >= 3 && n == 3 {
        let x = Expr::Var(space.shown()[0].clone());
        let minus_one = Expr::Bin(BinOp::Sub, Box::new(Expr::Bin(BinOp::Sub, Box::new(x.clone()), Box::new(x))), Box::new(Expr::Num(1.0)));
        let chain = crate::parser::Chain { exprs: vec![minus_one, Expr::Num(0.0)], rels: vec![Rel::Le], pos: vec![0, 0] };
        return compile_chains(vec![chain], params, space);
    }
    if k > 3 {
        return Err(format!("{formula}。{k} 次元なので描けません（3 次元まで）"));
    }
    let used: Vec<&str> = match k {
        0 => vec![],
        1 => vec!["t"],
        2 => vec!["u", "v"],
        _ => names.to_vec(),
    };
    for v in &used {
        crate::expand::set_param_range(v, -reach, reach);
    }
    let items: Vec<Expr> = (0..n).map(|i| linear(0.0, basis.iter().zip(&used).map(|(b, v)| (b[i], Expr::Var((*v).into()))).collect())).collect();
    if k > 0 {
        crate::expand::request_trim();
    }
    parametric(items, params, space)
}

/// 論理式・集合（計画 M11）を描く。選言標準形にして、かたまりが 1 つなら今の連立方程式・不等式の道で描く。
/// 領域（等式の無いかたまり）どうしの「または」は、1 本の不等式 $\min_k \max_j g_{kj} \le 0$ にまとめる
/// （重なった面を切り取らなくてよい。和集合の角は、格子の細かさの分だけ丸まる）。等式のあるかたまりは、それぞれ描いて重ねる。
/// ≠ は「かつ」の中では形を変えないので、除いた所（h = 0）を白丸（曲線）・点線（面・領域）で示す
fn compile_logic(source: &str, params: &Params, space: &Space) -> Result<Plot, String> {
    let names = crate::expand::names();
    // 同じ集合名が別の行にあっても、定義の行はその行の右辺を描く。
    if let Some((lhs, rhs)) = source.split_once('=') {
        let name = lhs.trim().trim_start_matches('{').trim_end_matches('}');
        if crate::expand::definable(name) && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            if let Ok(f) = crate::parser::parse_formula(rhs, &names) {
                if crate::logic::as_set(&f).is_some() {
                    if !params.contains_key(name) {
                        crate::expand::mark_def_line(name, None);
                    }
                    return compile_formula(&f, params, space);
                }
            }
        }
    }
    let formula = crate::parser::parse_formula(source, &names).map_err(|e| e.to_string())?;
    compile_formula(&formula, params, space)
}

/// 像として描く集合の要素式と条件。座標名だけの組は通常の座標条件として扱う。
fn image_builder<'a>(f: &'a crate::parser::Formula, params: &Params) -> Option<(&'a Expr, &'a crate::parser::Formula)> {
    use crate::parser::{Formula, SetE};
    let Formula::Set(SetE::Builder { point, cond }) = f else { return None };
    let Expr::Tuple(comps) = point else { return Some((point, cond)); };
    let mut seen: Vec<&str> = Vec::new();
    let plain = comps.iter().all(|c| match c {
        Expr::Var(v) if !seen.contains(&v.as_str()) => {
            seen.push(v);
            true
        }
        _ => false,
    });
    let has_slider = seen.iter().any(|n| params.contains_key(*n));
    let standard = seen.iter().filter(|n| coord_index(n).is_some() || is_param_var(n)).count();
    if !plain || has_slider || (standard > 0 && standard < seen.len()) {
        return Some((point, cond));
    }
    if standard == 0 {
        let mut declared = Vec::new();
        crate::set_image::declared_inputs(cond, params, &mut declared);
        let explicit = seen.iter().filter(|n| declared.iter().any(|name| name == **n)).count();
        if explicit > 0 && explicit < seen.len() {
            return Some((point, cond));
        }
    }
    None
}

/// 「または」（∪）を平らに並べる
fn flatten_or(f: &crate::parser::Formula, out: &mut Vec<crate::parser::Formula>, active: &mut Vec<String>) -> Result<(), String> {
    use crate::parser::{Formula, SetE};
    match f {
        Formula::Or(v) => v.iter().try_for_each(|g| flatten_or(g, out, active)),
        Formula::Set(SetE::Union(a, b)) => {
            flatten_or(&Formula::Set((**a).clone()), out, active)?;
            flatten_or(&Formula::Set((**b).clone()), out, active)
        }
        Formula::Set(SetE::Named(name)) => {
            if active.contains(name) {
                return Err(format!("集合 {name} の定義が循環しています"));
            }
            if let Some(set) = crate::expand::set_def(name) {
                active.push(name.clone());
                flatten_or(&Formula::Set(set), out, active)?;
                active.pop();
            } else {
                out.push(f.clone());
            }
            Ok(())
        }
        g => {
            out.push(g.clone());
            Ok(())
        }
    }
}

/// 二成分の集合を、座標条件から xy 平面上の像として描く。
fn planar_set(set: &crate::parser::SetE, params: &Params, space: &Space) -> Result<Plot, String> {
    let names = [" set planar first", " set planar second"];
    let at = Expr::Tuple(names.iter().map(|n| Expr::Var((*n).into())).collect());
    let condition = crate::logic::member(&at, set, &crate::expand::set_def)?;
    // 二成分目までの束縛名を置換してから、条件に残った高さなどを平面の値にする。
    // 例えば元の要素が (z,x) でも、束縛された z と外側の高さを混同しない。
    let condition = crate::logic::substitute(&condition, &|name| {
        if let Some(i) = names.iter().position(|n| *n == name) {
            Some(Expr::Var(coord_name(i)))
        } else {
            coord_index(name).filter(|&i| i >= 2).map(|_| Expr::Num(0.0))
        }
    });
    let point = Expr::Tuple((0..2).map(|i| Expr::Var(coord_name(i))).collect());
    crate::set_image::compile(&point, &condition, params, space)
}

/// 論理式・集合を描く（[`compile_logic`] の本体）
fn compile_formula(formula: &crate::parser::Formula, params: &Params, space: &Space) -> Result<Plot, String> {
    use crate::logic::{Lit, Op};
    use crate::parser::{Chain, Formula};
    let lookup = crate::expand::set_def;
    // ⋃・⋂ の各項にも、通常の集合と同じ次元の扱いを適用する。
    let formula = crate::expand::expand_big(formula, params)?;
    // 和と集合名を開き、像集合を含むときは枝ごとに描く。
    let mut parts = Vec::new();
    flatten_or(&formula, &mut parts, &mut Vec::new())?;
    if let [single] = parts.as_slice() {
        if let Some((point, cond)) = image_builder(single, params) {
            return crate::set_image::compile(point, cond, params, space);
        }
    }
    let images = parts.iter().any(|p| image_builder(p, params).is_some());
    let arities: Vec<_> = parts.iter().map(|p| {
        crate::logic::as_set(p).and_then(|s| crate::logic::arity(&s, &lookup).ok())
    }).collect();
    let has_plane = arities.contains(&Some(2));
    if !images && has_plane && arities.iter().all(|a| matches!(a, Some(0 | 2))) {
        if let Some(set) = crate::logic::as_set(&Formula::Or(parts.clone())) {
            return planar_set(&set, params, space);
        }
    }
    if parts.len() > 1 && (images || has_plane) {
        let plots = parts.iter().map(|part| compile_formula(part, params, space)).collect::<Result<_, _>>()?;
        return Ok(Plot::Multi(plots));
    }
    let shown: Vec<String> = space.shown().to_vec();
    let clauses = crate::logic::clauses(&formula, &shown, &lookup)?;
    // 原子の式を展開する（関数・Σ など）
    let mut expanded: Vec<Vec<Lit>> = Vec::with_capacity(clauses.len());
    for c in clauses {
        let mut out = Vec::new();
        for l in c {
            if l.op == Op::Ne {
                // ≠ は面積・長さの無い所を除くだけなので、形は変えずに、除いた所を白丸・点線で示す（lib.rs の mark_excluded）
                let h = Expr::Bin(BinOp::Sub, Box::new(crate::expand::expand(&l.l, params)?), Box::new(crate::expand::expand(&l.r, params)?));
                let rotated = match space.rotation_matrix() {
                    Some(m) => rotate_coords(&h, &m, space),
                    None => h.clone(),
                };
                let refs: [&str; 3] = [&shown[0], &shown[1], &shown[2]];
                match Program::compile_with(&rotated, &refs, &space.with_hidden(params)) {
                    Ok(p) if p.vars().len() == 3 => crate::expand::add_excluded(h, p),
                    _ => crate::expand::add_note("≠ は描き方が変わらないので無視"),
                }
                continue;
            }
            out.push(Lit { l: crate::expand::expand(&l.l, params)?, op: l.op, r: crate::expand::expand(&l.r, params)? });
        }
        expanded.push(out);
    }
    if expanded.is_empty() {
        crate::expand::add_note("空集合");
        return Ok(Plot::Multi(Vec::new()));
    }
    let chain_of = |l: &Lit| Chain {
        exprs: vec![l.l.clone(), l.r.clone()],
        rels: vec![match l.op {
            Op::Lt => Rel::Lt,
            Op::Le => Rel::Le,
            _ => Rel::Eq,
        }],
        pos: vec![0, 0],
    };
    // 全体（条件の無いかたまり）は、表示範囲の箱全体の領域。座標を使わない条件は連立の道が受け付けないので、
    // 形の上だけ座標を含めた x − x − 1（どこでも −1）にする
    let x = Expr::Var(shown[0].clone());
    let minus_one = Expr::Bin(BinOp::Sub, Box::new(Expr::Bin(BinOp::Sub, Box::new(x.clone()), Box::new(x))), Box::new(Expr::Num(1.0)));
    let everything = || Chain { exprs: vec![minus_one.clone(), Expr::Num(0.0)], rels: vec![Rel::Le], pos: vec![0, 0] };
    if expanded.len() == 1 {
        let c = &expanded[0];
        let chains = if c.is_empty() { vec![everything()] } else { c.iter().map(chain_of).collect() };
        return compile_chains(chains, params, space);
    }
    let (regions, surfaces): (Vec<&Vec<Lit>>, Vec<&Vec<Lit>>) = expanded.iter().partition(|c| c.iter().all(|l| l.op != Op::Eq));
    let mut plots = Vec::new();
    if !regions.is_empty() {
        let sub = |l: &Lit| Expr::Bin(BinOp::Sub, Box::new(l.l.clone()), Box::new(l.r.clone()));
        let fold = |es: Vec<Expr>, f: crate::functions::Func| es.into_iter().reduce(|a, b| Expr::Call(f, vec![a, b]));
        let parts: Vec<Expr> = regions
            .iter()
            .map(|c| fold(c.iter().map(sub).collect(), crate::functions::Func::Max).unwrap_or(minus_one.clone()))
            .collect();
        let strict = regions.iter().all(|c| !c.is_empty() && c.iter().all(|l| l.op == Op::Lt));
        let g = fold(parts, crate::functions::Func::Min).unwrap_or(minus_one.clone());
        let chain = Chain { exprs: vec![g, Expr::Num(0.0)], rels: vec![if strict { Rel::Lt } else { Rel::Le }], pos: vec![0, 0] };
        plots.push(compile_chains(vec![chain], params, space)?);
    }
    for c in surfaces {
        plots.push(compile_chains(c.iter().map(chain_of).collect(), params, space)?);
    }
    Ok(if plots.len() == 1 { plots.pop().unwrap_or(Plot::Multi(Vec::new())) } else { Plot::Multi(plots) })
}

/// 解析・展開した式（[`compile_with`]）をコンパイルする
pub(crate) fn compile_chains(mut chains: Vec<crate::parser::Chain>, params: &Params, space: &Space) -> Result<Plot, String> {
    let shown = space.shown();
    for chain in &mut chains {
        for e in &mut chain.exprs {
            *e = rename_coords(e, space);
        }
    }
    if space.complex_coords {
        for chain in &mut chains {
            for e in &mut chain.exprs {
                *e = complexify_coords(e, space)?;
            }
        }
    }
    // 略記 −1 ≤ x, y, z ≤ 1（x・y・z がどれも −1 以上 1 以下）・x, y ≥ 0・0 ≤ x, y, z を、変数ごとの行に分ける
    chains = crate::parser::expand_shared_bounds(chains);
    // ベクトル = ベクトルは、成分ごとの等式の連立（(x, y, z) = (1, 2, 3) は x = 1, y = 2, z = 3）。計画 M6
    chains = split_vector_relations(chains)?;
    // 関係が「= 1 つだけ」か「無し」の 1 行の式は、今までの道（陽関数・陰関数・媒介変数表示・パラメータの定義）。それ以外は連立
    let single = chains.len() == 1 && (chains[0].rels.is_empty() || chains[0].rels == [Rel::Eq]);
    if !single {
        return system(chains, params, space);
    }
    let chain = chains.remove(0);
    let (l, r) = match chain.exprs.len() {
        1 => {
            let e = chain.exprs.into_iter().next().unwrap_or(Expr::Num(0.0));
            check_dims(&e, space)?;
            if let Expr::Tuple(items) = e {
                if is_field(&items, space) {
                    return field(items, params, space);
                }
                return parametric(items, params, space);
            }
            if let Expr::Matrix { rows, cols, .. } = &e {
                return Err(format!("{rows}×{cols} の行列だけの行は描けません（A = … と名前を付けるか、ベクトルに掛けてください）"));
            }
            if let Some(p) = complex_plane(&e, params, space)? {
                return Ok(p);
            }
            // = が無い式は「縦の軸の座標 = 式」
            if e.uses_var(&shown[2]) {
                return Err(format!(
                    "{} を含む式は「… = 0」のように = を付けてください（陰関数になります）",
                    shown[2]
                ));
            }
            (Expr::Var(shown[2].clone()), e)
        }
        _ => {
            let mut it = chain.exprs.into_iter();
            let (l, r) = (it.next().unwrap_or(Expr::Num(0.0)), it.next().unwrap_or(Expr::Num(0.0)));
            check_dims(&l, space)?;
            check_dims(&r, space)?;
            if matches!(l, Expr::Tuple(_)) || matches!(r, Expr::Tuple(_)) {
                return Err("媒介変数表示は = を付けずに (x(t), y(t), z(t)) の形で書いてください".into());
            }
            // 左辺が座標以外の名前で、右辺に座標が無ければパラメータの定義
            if let Expr::Var(name) = &l {
                let mut rv = Vec::new();
                r.collect_vars(&mut rv);
                let uses_coord = rv.iter().any(|v| coord_index(v).is_some());
                if coord_index(name).is_none() && !uses_coord {
                    if first_list(&r).is_some() {
                        return Err(format!("{name} の値にリストは使えません"));
                    }
                    if r.uses_var(name) {
                        return Err(format!("{name} の定義の右辺に {name} 自身は使えません"));
                    }
                    return param(name, &r, params);
                }
            }
            (l, r)
        }
    };

    // リストがあれば、要素ごと（組み合わせの数だけ）の式にして、それぞれを描く
    if first_list(&l).is_some() || first_list(&r).is_some() {
        let variants = expand_lists(&[l, r])?;
        let plots = variants
            .into_iter()
            .map(|v| {
                let mut it = v.into_iter();
                let (l, r) = (it.next().unwrap_or(Expr::Num(0.0)), it.next().unwrap_or(Expr::Num(0.0)));
                compile_pair(l, r, params, space)
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Plot::Multi(plots));
    }
    compile_pair(l, r, params, space)
}

/// 関係の両側が全部ベクトル（組）の行を、成分ごとの行に分ける。ベクトルとスカラーが混ざっていればエラー。
/// 関係の無い行（媒介変数表示の組）と、ベクトルの無い行はそのまま
fn split_vector_relations(chains: Vec<crate::parser::Chain>) -> Result<Vec<crate::parser::Chain>, String> {
    let mut out = Vec::with_capacity(chains.len());
    for chain in chains {
        if let Some(Expr::Matrix { rows, cols, .. }) = chain.exprs.iter().find(|e| matches!(e, Expr::Matrix { .. })) {
            if !chain.rels.is_empty() {
                return Err(format!("{rows}×{cols} の行列どうしの等式は描けません（成分ごとに書いてください）"));
            }
        }
        let tuples = chain.exprs.iter().filter(|e| matches!(e, Expr::Tuple(_))).count();
        if chain.rels.is_empty() || tuples == 0 {
            out.push(chain);
            continue;
        }
        let len = |e: &Expr| if let Expr::Tuple(items) = e { items.len() } else { 0 };
        let n = len(&chain.exprs[0]);
        if tuples != chain.exprs.len() || chain.exprs.iter().any(|e| len(e) != n) {
            return Err("ベクトルの等式・不等式は、両側を同じ成分の数のベクトルにしてください".into());
        }
        for k in 0..n {
            let exprs = chain.exprs.iter().map(|e| if let Expr::Tuple(items) = e { items[k].clone() } else { e.clone() }).collect();
            out.push(crate::parser::Chain { exprs, rels: chain.rels.clone(), pos: chain.pos.clone() });
        }
    }
    Ok(out)
}

/// 座標を複素数にする（[`Space::complex_coords`]）: 式の x, y, z を、x + i w, y + i w₂, z + i w₃ に置き換える。
/// 虚部の座標（w, w₂, w₃）を式の中で使っていたらエラー。媒介変数表示の組（成分が座標そのもの）は、そのまま
pub fn complexify_coords(e: &Expr, space: &Space) -> Result<Expr, String> {
    if matches!(e, Expr::Tuple(_)) {
        return Ok(e.clone());
    }
    if space.dims < 6 {
        return Err("座標を複素数にするには、次元を 6 以上にしてください（虚部に w, w₂, w₃ を使います）".into());
    }
    for k in 3..6 {
        if e.uses_var(&coord_name(k)) || (k == 3 && e.uses_var("w1")) {
            return Err("座標を複素数にしている間は、w・w₂・w₃ は使えません（x・y・z の虚部に使います）".into());
        }
    }
    Ok(e.substitute(&|name: &str| {
        let k = coord_index(name).filter(|&k| k < 3)?;
        let imag = Expr::Bin(BinOp::Mul, Box::new(Expr::Const(crate::parser::Constant::I)), Box::new(Expr::Var(coord_name(k + 3))));
        Some(Expr::Bin(BinOp::Add, Box::new(Expr::Var(name.to_string())), Box::new(imag)))
    }))
}

/// 座標系の座標の名前（[`Space::coord_names`]）を、表示中の 3 つの座標に置き換える。名前が無ければそのまま
pub fn rename_coords(e: &Expr, space: &Space) -> Expr {
    let Some(names) = &space.coord_names else { return e.clone() };
    let shown = space.shown();
    e.substitute(&|v: &str| names.iter().position(|n| n == v).map(|k| Expr::Var(shown[k].clone())))
}

/// 式の種類の説明（`x = f(y, z)` など）の、表示中の座標の名前を座標系の名前にする（`r = f(θ, φ)`）。名前が無ければそのまま
pub fn rename_kind(kind: &str, space: &Space) -> String {
    let Some(names) = &space.coord_names else { return kind.to_string() };
    let shown = space.shown();
    let pretty = |n: &str| match n {
        "theta" => "θ".to_string(),
        "phi" => "φ".to_string(),
        "rho" => "ρ".to_string(),
        "sigma" => "σ".to_string(),
        "tau" => "τ".to_string(),
        "mu" => "μ".to_string(),
        "nu" => "ν".to_string(),
        _ => n.to_string(),
    };
    // 英数字の並びごとに、表示中の座標の名前と同じなら置き換える
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        match shown.iter().position(|s| s == word) {
            Some(k) => out.push_str(&pretty(&names[k])),
            None => out.push_str(word),
        }
        word.clear();
    };
    for c in kind.chars() {
        if c.is_ascii_alphanumeric() {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

/// `@exists(a, lo, hi) 中身`・`@forall(a, lo, hi) 中身` の行（計画 M12）。そうでなければ None
/// - ∃ で、中身が等式 1 本（と a を含まない不等式）: a を範囲で重ねる（今の「範囲で重ねる」と同じ）
/// - ∃ で、中身が「座標 = a の式」で表示中の座標が全部決まる: a を媒介変数にした曲線
/// - 中身が不等式だけ: ∃ は $\min_k G(\mathbf p, a_k) \le 0$、∀ は $\max_k G(\mathbf p, a_k) \le 0$（範囲を 32 等分。近似）
fn quantified(source: &str, params: &Params, space: &Space) -> Result<Option<Plot>, String> {
    let src = source.trim_start();
    let (exists, rest) = if let Some(r) = src.strip_prefix("@exists") {
        (true, r)
    } else if let Some(r) = src.strip_prefix("@forall") {
        (false, r)
    } else {
        return Ok(None);
    };
    let sym = if exists { "∃" } else { "∀" };
    let rest = rest.trim_start();
    // 括弧の中 (a, lo, hi)
    let Some(inner) = rest.strip_prefix('(') else { return Err(format!("{sym} の後に (a, 0, 1) の形で、変数と範囲を書いてください")) };
    let mut depth = 1;
    let close = inner.char_indices().find(|&(_, c)| {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            _ => {}
        }
        depth == 0
    });
    let Some((close, _)) = close else { return Err(format!("{sym} の括弧が閉じていません")) };
    let head = parse_at(&format!("({})", &inner[..close]), 0, params)?;
    let Expr::Tuple(h) = head else { return Err(format!("{sym} の範囲を書いてください（{sym}a ∈ [0, 1], …）")) };
    let [Expr::Var(var), lo, hi] = h.as_slice() else { return Err(format!("{sym} の範囲を書いてください（{sym}a ∈ [0, 1], …）")) };
    let value = |e: &Expr| -> Result<f64, String> {
        crate::expand::numbers(std::slice::from_ref(e), params)
            .map(|v| v[0])
            .ok_or_else(|| format!("{sym} の範囲は、数とパラメータで書いてください"))
    };
    let (lo, hi) = (value(lo)?, value(hi)?);
    let body = inner[close + 1..].trim_start().trim_start_matches(',').trim();
    if body.is_empty() {
        return Err(format!("{sym} の後に、条件を書いてください"));
    }
    if coord_index(var).is_some() {
        return Err(format!("{sym} の変数に座標 {var} は使えません"));
    }
    let mut ps = params.clone();
    ps.remove(var);
    let chains = parse_expanded(body, &ps)?;
    let rels: Vec<(Expr, Rel, Expr)> = chains.iter().flat_map(|c| (0..c.rels.len()).map(move |k| (c.exprs[k].clone(), c.rels[k], c.exprs[k + 1].clone()))).collect();
    if rels.len() != chains.iter().map(|c| c.rels.len()).sum::<usize>() || rels.is_empty() {
        return Err(format!("{sym} の中身には、等式・不等式を書いてください"));
    }
    let eqs: Vec<&(Expr, Rel, Expr)> = rels.iter().filter(|r| r.1 == Rel::Eq).collect();
    let shown = space.shown();
    if exists && !eqs.is_empty() {
        // 座標 = a の式 で、表示中の座標が全部決まる → 媒介変数の曲線
        let mut items: Vec<Option<Expr>> = vec![None; space.dims];
        let all_coord = eqs.len() == rels.len()
            && eqs.iter().all(|(l, _, r)| match l {
                Expr::Var(c) if coord_index(c).is_some_and(|k| k < space.dims) && !r.uses_var(c) && items[coord_index(c).unwrap_or(0)].is_none() => {
                    items[coord_index(c).unwrap_or(0)] = Some(r.clone());
                    true
                }
                _ => false,
            });
        if all_coord && shown.iter().all(|c| items[coord_index(c).unwrap_or(0)].is_some()) {
            crate::expand::set_param_range("t", lo, hi);
            let items: Vec<Expr> = items
                .into_iter()
                .enumerate()
                .map(|(k, e)| e.unwrap_or(Expr::Num(space.slice.get(k).copied().unwrap_or(0.0))).substitute(&|v| (v == var).then(|| Expr::Var("t".into()))))
                .collect();
            return parametric(items, &ps, space).map(Some);
        }
        // 等式 1 本: a を範囲で重ねる
        if eqs.len() == 1 && rels.iter().filter(|r| r.1 != Rel::Eq).all(|(l, _, r)| !l.uses_var(var) && !r.uses_var(var)) {
            let mut sp = space.clone();
            sp.param_ranges.push((var.clone(), lo, hi));
            crate::expand::set_param_range(var, lo, hi);
            let cs: Vec<crate::parser::Chain> = chains.into_iter().map(|c| crate::parser::Chain { exprs: c.exprs.iter().map(|e| rename_coords(e, space)).collect(), ..c }).collect();
            return compile_chains(cs, &ps, &sp).map(Some);
        }
        return Err("∃ の中身は、等式 1 本（と a を含まない不等式）か、座標 = a の式・不等式だけにしてください".into());
    }
    if !eqs.is_empty() {
        return Err("∀ の中身は、不等式だけにしてください".into());
    }
    // 不等式だけ: G_j = 左 − 右 ≤ 0 の「かつ」を、a の 33 点で ∃ なら min、∀ なら max
    const N: usize = 32;
    let gs: Vec<Expr> = rels
        .iter()
        .map(|(l, r, rr)| match r {
            Rel::Lt | Rel::Le => crate::diff::sub(l.clone(), rr.clone()),
            _ => crate::diff::sub(rr.clone(), l.clone()),
        })
        .collect();
    let strict = rels.iter().all(|r| matches!(r.1, Rel::Lt | Rel::Gt));
    let fold = |es: Vec<Expr>, f: crate::functions::Func| es.into_iter().reduce(|a, b| Expr::Call(f, vec![a, b])).unwrap_or(Expr::Num(0.0));
    let samples: Vec<Expr> = (0..=N)
        .map(|k| {
            let a = lo + (hi - lo) * k as f64 / N as f64;
            let put = |e: &Expr| e.substitute(&|v| (v == var).then_some(Expr::Num(a)));
            fold(gs.iter().map(put).collect(), crate::functions::Func::Max)
        })
        .collect();
    let g = fold(samples, if exists { crate::functions::Func::Min } else { crate::functions::Func::Max });
    crate::expand::add_note(&format!("{sym} は {var} の範囲を {N} 等分した点での近似"));
    let chain = crate::parser::Chain { exprs: vec![g, Expr::Num(0.0)], rels: vec![if strict { Rel::Lt } else { Rel::Le }], pos: vec![0, 0] };
    compile_chains(vec![chain], &ps, space).map(Some)
}

/// `u ⟂ v`・`u ∥ v` の行を、`u · v = 0`・`u × v = 0`（3 成分なら成分ごとの連立）にして描く。そうでなければ None
fn perp_line(source: &str, params: &Params, space: &Space) -> Result<Option<Plot>, String> {
    let (op, at) = match (source.find('⟂'), source.find('∥')) {
        (Some(k), None) => ('⟂', k),
        (None, Some(k)) => ('∥', k),
        (None, None) => return Ok(None),
        _ => return Err("⟂ と ∥ は、1 行に 1 つだけ書いてください".into()),
    };
    let (l, r) = (&source[..at], &source[at + op.len_utf8()..]);
    let offset = l.chars().count() + 1;
    let (u, v) = (parse_at(l, 0, params)?, parse_at(r, offset, params)?);
    let lhs = if op == '⟂' {
        crate::expand::expand(&Expr::Op { op: "dot".into(), args: vec![u, v] }, params)?
    } else {
        crate::expand::expand(&Expr::Op { op: "cross".into(), args: vec![u, v] }, params)?
    };
    let rhs = match &lhs {
        Expr::Tuple(items) => Expr::Tuple(vec![Expr::Num(0.0); items.len()]),
        _ => Expr::Num(0.0),
    };
    let chain = crate::parser::Chain { exprs: vec![lhs, rhs], rels: vec![Rel::Eq], pos: vec![0, offset] };
    compile_chains(vec![chain], params, space).map(Some)
}

/// 数列の定義の行 `a_(n) = …` を、点 (n, aₙ) にする（n は横の表示範囲の中の整数で、初項の添字以上。2,000 点まで）。
/// 値が実数でない項があれば、複素平面の点 (Re aₙ, Im aₙ) を順に結んだ軌道。項が座標を含めば、図形なし
fn sequence_line(source: &str, params: &Params) -> Result<Option<Plot>, String> {
    if !source.contains("_(") {
        return Ok(None);
    }
    let Ok(chains) = parse_system_with(source, &crate::expand::names()) else { return Ok(None) };
    let [chain] = chains.as_slice() else { return Ok(None) };
    let Some(head) = crate::expand::sequence_head(&chain.exprs[0]) else { return Ok(None) };
    let name = &head.name;
    if chain.rels != [Rel::Eq] || crate::expand::seq_def(name).is_none() {
        return Ok(None);
    }
    if head.uses_principal_root() {
        crate::expand::add_note("各項には根の主値を使用");
    }
    let (lo, hi) = crate::expand::view_x();
    let first = crate::expand::sequence_start(&head, params);
    let from = (lo.ceil() as i64).max(first);
    let to = hi.floor() as i64;
    crate::expand::mark_def_line(name, Some("数列の点".into()));
    if to < from {
        return Ok(Some(Plot::Multi(Vec::new())));
    }
    let Some(terms) = crate::expand::sequence_terms(&head, from, to, params)? else {
        // 項が座標を含む（関数の列）: 点は描かない
        crate::expand::mark_def_line(name, None);
        return Ok(Some(Plot::Multi(Vec::new())));
    };
    let complex = terms.iter().any(|(_, v)| v.im.abs() > 1e-12 * v.re.abs().max(1.0) && v.im.is_finite());
    let points = terms
        .iter()
        .filter(|(_, v)| v.re.is_finite() && v.im.is_finite())
        .map(|(k, v)| if complex { [v.re, v.im, 0.0] } else { [*k as f64, v.re, 0.0] })
        .collect();
    if complex {
        crate::expand::mark_def_line(name, Some("数列の軌道（複素平面）".into()));
    }
    Ok(Some(Plot::Points { points, line: complex, sequence: Some(SequencePoints { head, terms }) }))
}

/// 数列の値を描画座標に変換する前の添字と値。
pub struct SequencePoints {
    head: crate::expand::SequenceHead,
    terms: Vec<(i64, C)>,
}

impl SequencePoints {
    pub(crate) fn table(&self, params: &Params) -> Vec<f64> {
        self.terms.iter().flat_map(|(k, v)| {
            let argument = self.head.argument_at(*k, params).map_or(f64::NAN, |n| n as f64);
            [*k as f64, v.re, v.im, argument]
        }).collect()
    }
}

fn cobweb_line(source: &str, params: &Params) -> Result<Plot, String> {
    let expr = parse_with(source, &crate::expand::names()).map_err(|e| e.to_string())?;
    let Expr::Op { op, args } = expr else { return Err("階段図は単独の式の行に書いてください".into()) };
    let [start, steps] = args.as_slice() else { return Err("階段図は cobweb(a_1, 12) のように開始項と反復回数を書いてください".into()) };
    if op != "cobweb" {
        return Err("階段図は cobweb(a_1, 12) の形で書いてください".into());
    }
    let terms = crate::expand::cobweb_terms(start, steps, params)?;
    let mut points = Vec::with_capacity(2 * terms.len() - 1);
    points.push([terms[0], terms[0], 0.0]);
    for pair in terms.windows(2) {
        if pair[0] != pair[1] {
            points.push([pair[0], pair[1], 0.0]);
            points.push([pair[1], pair[1], 0.0]);
        }
    }
    let line = points.len() > 1;
    crate::expand::set_kind(format!("数列の階段図（{} 回の反復）", terms.len() - 1));
    Ok(Plot::Points { points, line, sequence: None })
}

fn orbit_line(source: &str, params: &Params) -> Result<Plot, String> {
    let usage = "軌道は orbit(a_0, 24) や orbit(a_0, b_0, 24) のように、1〜3 個の開始項と反復回数を書いてください";
    let expr = parse_with(source, &crate::expand::names()).map_err(|e| e.to_string())?;
    let Expr::Op { op, args } = expr else { return Err(usage.into()) };
    let Some((steps, starts)) = args.split_last() else { return Err(usage.into()) };
    let dots = op == "orbitpoints";
    let (mut points, dims, complex) = match starts {
        [matrix, Expr::Tuple(initial)] if (2..=3).contains(&initial.len()) => {
            (crate::expand::matrix_orbit(matrix, initial, steps, dots, params)?, initial.len(), false)
        }
        starts if (1..=3).contains(&starts.len()) => {
            let (points, complex) = crate::expand::orbit_points(starts, steps, dots, params)?;
            (points, starts.len(), complex)
        }
        _ => return Err(usage.into()),
    };
    let steps = points.len() - 1;
    let label = if dots { "数列の軌道の点" } else { "数列の軌道" };
    let plane = if complex { "複素平面".into() } else { format!("{dims} 次元") };
    crate::expand::set_kind(format!("{label}（{plane}・{steps} 回の反復）"));
    if dots {
        return Ok(Plot::Points { line: false, points, sequence: None });
    }
    points.dedup();
    if points.len() == 1 {
        return Ok(Plot::Points { line: false, points, sequence: None });
    }
    let last = *points.last().unwrap();
    if dims == 1 && !complex {
        // 数直線上の移動を、行きは上、戻りは下の弧で表示する。
        points = points.windows(2).flat_map(|pair| {
            let [a, b] = [pair[0][0], pair[1][0]];
            (0..=12).map(move |j| {
                let t = f64::from(j) / 12.0;
                [a + (b - a) * t, (b - a) * t * (1.0 - t), 0.0]
            }).chain(std::iter::once([f64::NAN; 3]))
        }).collect();
    } else {
        points.push([f64::NAN; 3]);
    }
    points.push(last);
    Ok(Plot::Points { line: true, points, sequence: None })
}

fn recurrence_line(source: &str, params: &Params) -> Result<Plot, String> {
    let usage = "リカレンスネットワークは recurrence(a_0, 32, 0.1) のように、開始項・項数・距離を書いてください";
    let expr = parse_with(source, &crate::expand::names()).map_err(|e| e.to_string())?;
    let Expr::Op { args, .. } = expr else { return Err(usage.into()) };
    let [start, count, distance] = args.as_slice() else { return Err(usage.into()) };
    let (nodes, distance) = crate::expand::recurrence_nodes(start, count, distance, params)?;
    let mut points = Vec::new();
    let mut edges = 0;
    for (i, &a) in nodes.iter().enumerate() {
        points.extend([a, [f64::NAN; 3]]);
        for &b in &nodes[i + 1..] {
            if (a[1] - b[1]).hypot(a[2] - b[2]) <= distance {
                // 同じ値に戻る辺も重ならないよう、添字の間隔に応じた弧にする。
                points.extend((0..=8).map(|j| {
                    let t = f64::from(j) / 8.0;
                    let dx = b[0] - a[0];
                    [a[0] + dx * t, a[1] + (b[1] - a[1]) * t + 0.6 * dx * t * (1.0 - t), a[2] + (b[2] - a[2]) * t]
                }));
                points.push([f64::NAN; 3]);
                edges += 1;
            }
        }
    }
    crate::expand::set_kind(format!("リカレンスネットワーク（{} 頂点・{edges} 辺）", nodes.len()));
    Ok(Plot::Points { line: true, points, sequence: None })
}

fn bifurcation_line(source: &str, params: &Params) -> Result<Plot, String> {
    let usage = "分岐図は bifurcation(f, 0.2, 2.5, 4) のように、二変数関数 f(x,r)・初期値・パラメータの下限と上限を書いてください";
    let expr = parse_with(source, &crate::expand::names()).map_err(|e| e.to_string())?;
    let Expr::Op { args, .. } = expr else { return Err(usage.into()) };
    let [Expr::Var(map), start, lo, hi] = args.as_slice() else { return Err(usage.into()) };
    let points = crate::expand::bifurcation_points(map, start, lo, hi, params)?;
    crate::expand::set_kind(format!("分岐図（{} 点）", points.len()));
    Ok(Plot::Points { line: false, points, sequence: None })
}

/// 常微分方程式（[`Plot::Ode`]）
#[derive(Debug, Clone)]
pub struct Ode {
    /// 状態の各成分の右辺（変数は `vars`）
    pub rhs: Vec<Program>,
    /// 状態の名前と、最後に独立変数（`t` か、1 本の形では横の座標）
    pub vars: Vec<String>,
    /// 状態の成分ごとに、点のどの座標に入れるか（導関数の成分は None）
    pub coords: Vec<Option<usize>>,
    /// 初期値の組
    pub inits: Vec<Vec<f64>>,
    /// 1 本の形なら (独立変数の座標, 未知関数の座標)
    pub one: Option<(usize, usize)>,
    /// 初期値で使ったパラメータ
    pub uses: Vec<String>,
}

/// 状態の名前: 0 階は座標の名前そのもの、j 階の導関数は `#x1` のような書けない名前
fn state_name(c: &str, j: u8) -> String {
    if j == 0 {
        c.to_string()
    } else {
        format!("#{c}{j}")
    }
}

/// 常微分方程式の行（計画 M9）。そうでなければ None
/// - 連立: 全部の行が「座標′ = 式」（独立変数は t）
/// - 1 本: 行が 1 つで、プライムの付いた座標が 1 つ（横の座標でない）。独立変数は横の座標。最も高い階の導関数について 1 次式なら解く
fn ode_line(source: &str, params: &Params, space: &Space) -> Result<Option<Plot>, String> {
    if !source.contains('\'') && !source.contains('′') && !source.contains("@diff") {
        return Ok(None);
    }
    let Ok(chains) = parse_system_with(source, &crate::expand::names()) else { return Ok(None) };
    let is_coord = |v: &str| coord_index(v).is_some_and(|k| k < space.dims);
    // d/dt x（\frac{dx}{dt}）は x′ と同じ。d/dx y も、y′（1 本の形）
    let to_prime = |e: &Expr| -> Expr {
        e.rewrite(&|n| match n {
            Expr::Binder { op, var, args, body } if op == "diff" => match (&**body, args.as_slice()) {
                (Expr::Var(c), a) if is_coord(c) && var != c && (var == "t" || is_coord(var)) => {
                    let order = match a {
                        [] => 1,
                        [Expr::Num(k)] if *k >= 1.0 && *k <= 8.0 && k.fract() == 0.0 => *k as u8,
                        _ => return None,
                    };
                    Some(Expr::Prime { name: c.clone(), order })
                }
                _ => None,
            },
            _ => None,
        })
    };
    let chains: Vec<crate::parser::Chain> = chains
        .into_iter()
        .map(|c| crate::parser::Chain { exprs: c.exprs.iter().map(&to_prime).collect(), ..c })
        .collect();
    let mut primed: Vec<(String, u8)> = Vec::new();
    for c in &chains {
        for e in &c.exprs {
            for (n, k) in e.primes() {
                if !is_coord(&n) {
                    continue;
                }
                match primed.iter_mut().find(|p| p.0 == n) {
                    Some(p) => p.1 = p.1.max(k),
                    None => primed.push((n, k)),
                }
            }
        }
    }
    if primed.is_empty() {
        return Ok(None);
    }
    let shown = space.shown();
    let one_form = chains.len() == 1 && primed.len() == 1 && primed[0].0 != shown[0];
    // 定数: パラメータと、状態でない座標（断面の値）
    let mut consts = space.with_hidden(params);
    let ranged = space.ranged();
    for k in 0..space.dims {
        if space.axes.contains(&k) {
            consts.insert(coord_name(k), C::new(space.slice.get(k).copied().unwrap_or(0.0), 0.0));
        }
    }
    for (n, _, _) in &ranged {
        consts.remove(n);
    }
    // プライムを状態の名前にする（階数が上限を超えるプライムは誤り）
    let replace = |e: &Expr, max: &dyn Fn(&str) -> Option<u8>| -> Result<Expr, String> {
        let bad = std::cell::RefCell::new(None);
        let out = e.rewrite(&|n| match n {
            Expr::Prime { name, order } => match max(name) {
                Some(m) if *order <= m => Some(Expr::Var(state_name(name, *order))),
                _ => {
                    bad.borrow_mut().get_or_insert(format!("{name}{} はここでは使えません", "'".repeat(*order as usize)));
                    Some(Expr::Num(0.0))
                }
            },
            _ => None,
        });
        match bad.into_inner() {
            Some(m) => Err(m),
            None => Ok(out),
        }
    };
    let (states, rhs_exprs, coords, indep, one): (Vec<String>, Vec<Expr>, Vec<Option<usize>>, String, Option<(usize, usize)>) = if one_form {
        let (u, k) = primed[0].clone();
        if k > 4 {
            return Err("1 本の常微分方程式は 4 階までにしてください".into());
        }
        let ui = coord_index(&u).unwrap_or(0);
        if !space.axes.contains(&ui) {
            return Err(format!("{u} を表示してください（未知関数 {u} の解曲線を、横の座標との平面に描きます）"));
        }
        let c = &chains[0];
        if c.rels != [Rel::Eq] {
            return Err("常微分方程式は = で書いてください".into());
        }
        let f = Expr::Bin(BinOp::Sub, Box::new(c.exprs[0].clone()), Box::new(c.exprs[1].clone()));
        let f = replace(&f, &|n| (n == u).then_some(k))?;
        let f = crate::expand::expand(&f, params)?;
        let s = state_name(&u, k);
        let ds = crate::diff::diff(&f, &s)?;
        if ds.uses_var(&s) {
            return Err(format!("最も高い階の導関数 {u}{} について解いた形で書いてください", "'".repeat(k as usize)));
        }
        let f0 = f.substitute(&|v| (v == s).then_some(Expr::Num(0.0)));
        let top = crate::diff::neg(crate::diff::div(f0, ds));
        let mut rhs: Vec<Expr> = (1..k).map(|j| Expr::Var(state_name(&u, j))).collect();
        rhs.push(top);
        let states = (0..k).map(|j| state_name(&u, j)).collect();
        let mut coords = vec![None; k as usize];
        coords[0] = Some(ui);
        let xi = space.axes[0];
        consts.remove(&shown[0]);
        (states, rhs, coords, shown[0].clone(), Some((xi, ui)))
    } else {
        let mut lines: Vec<(String, u8, Expr)> = Vec::new();
        for c in &chains {
            let (Expr::Prime { name, order }, [Rel::Eq]) = (&c.exprs[0], c.rels.as_slice()) else {
                return Err("連立の常微分方程式は、行ごとに x' = 式 の形で書いてください".into());
            };
            if !is_coord(name) {
                return Err(format!("{name}' の {name} は座標ではありません（x, y, z, w, … の導関数を書いてください）"));
            }
            if lines.iter().any(|l| l.0 == *name) {
                return Err(format!("{name} の導関数が 2 回書かれています"));
            }
            lines.push((name.clone(), *order, c.exprs[1].clone()));
        }
        let total: usize = lines.iter().map(|l| l.1 as usize).sum();
        if total > 12 {
            return Err("常微分方程式の状態の成分は 12 個までにしてください".into());
        }
        let orders: Vec<(String, u8)> = lines.iter().map(|l| (l.0.clone(), l.1)).collect();
        let max = |n: &str| orders.iter().find(|o| o.0 == n).map(|o| o.1 - 1);
        let (mut states, mut rhs, mut coords) = (Vec::new(), Vec::new(), Vec::new());
        for (c, k, e) in &lines {
            for j in 0..*k {
                states.push(state_name(c, j));
                coords.push((j == 0).then(|| coord_index(c).unwrap_or(0)));
                if j + 1 < *k {
                    rhs.push(Expr::Var(state_name(c, j + 1)));
                } else {
                    rhs.push(crate::expand::expand(&replace(e, &max)?, params)?);
                }
            }
        }
        for (c, _, _) in &lines {
            consts.remove(c);
        }
        if rhs.iter().any(|e| first_list(e).is_some()) {
            return Err("常微分方程式の右辺には、リストは使えません（初期値の欄に書いてください）".into());
        }
        (states, rhs, coords, "t".into(), None)
    };
    consts.remove(&indep);
    let mut vars = states.clone();
    vars.push(indep);
    let refs: Vec<&str> = vars.iter().map(String::as_str).collect();
    let rhs = rhs_exprs.iter().map(|e| Program::compile_with(e, &refs, &consts)).collect::<Result<Vec<_>, _>>()?;
    // 初期値（欄が無ければ、状態は 1、導関数は 0。リストは組み合わせ）
    let given = crate::expand::ode_init();
    let mut uses: Vec<String> = Vec::new();
    let mut choices: Vec<Vec<f64>> = Vec::new();
    for (k, st) in states.iter().enumerate() {
        let label = st.strip_prefix('#').map_or(st.clone(), |r| {
            let (c, j) = r.split_at(r.find(|ch: char| ch.is_ascii_digit()).unwrap_or(r.len()));
            format!("{c}{}", "'".repeat(j.parse().unwrap_or(0)))
        });
        let src = given.get(k).map(|s| s.trim()).filter(|s| !s.is_empty());
        let vals = match src {
            None => vec![if coords[k].is_some() { 1.0 } else { 0.0 }],
            Some(src) => {
                let e = parse_at(src, 0, params).map_err(|e| format!("{label} の初期値: {e}"))?;
                let items = match e {
                    Expr::List(items) => items,
                    e => vec![e],
                };
                let mut vs = Vec::new();
                for it in &items {
                    let p = Program::compile_with(it, &[], params).map_err(|e| format!("{label} の初期値: {e}"))?;
                    uses.extend(p.used_params().iter().filter(|n| !uses.contains(n)).cloned().collect::<Vec<_>>());
                    let mut out = [C::new(0.0, 0.0)];
                    p.eval_batch(&[], &mut out);
                    let v = crate::eval::as_real(out[0]);
                    if !v.is_finite() {
                        return Err(format!("{label} の初期値が実数になりません"));
                    }
                    vs.push(v);
                }
                vs
            }
        };
        choices.push(vals);
    }
    let count: usize = choices.iter().map(|c| c.len()).product();
    if count > 64 {
        return Err(format!("初期値の組み合わせが多すぎます（{count} 通り。64 通りまで）"));
    }
    let mut inits: Vec<Vec<f64>> = vec![Vec::new()];
    for c in &choices {
        inits = inits.into_iter().flat_map(|p| c.iter().map(move |v| [p.clone(), vec![*v]].concat())).collect();
    }
    let labels: Vec<String> = states
        .iter()
        .map(|st| match st.strip_prefix('#') {
            None => st.clone(),
            Some(r) => {
                let i = r.find(|ch: char| ch.is_ascii_digit()).unwrap_or(r.len());
                format!("{}{}", &r[..i], "'".repeat(r[i..].parse().unwrap_or(0)))
            }
        })
        .collect();
    crate::expand::set_ode_states(labels);
    Ok(Some(Plot::Ode(Box::new(Ode { rhs, vars, coords, inits, one, uses }))))
}

/// 座標系の式（計画 9 章の S36）。組 `(X, Y, Z)` の 3 つの成分を、表示中の 3 つの座標の式としてコンパイルする。
/// 定数はパラメータ（表示していない座標は使えない）。成分が 3 つでない・ほかの変数を使うときはエラー
pub fn compile_coord_map(source: &str, params: &Params, space: &Space) -> Result<crate::coords::CoordMap, String> {
    const MSG: &str = "座標系の式は、x, y, z の式を 3 つ書いてください";
    let shown = space.shown();
    let shown_refs: Vec<&str> = shown.iter().map(String::as_str).collect();
    let msg = || {
        if shown == ["x", "y", "z"] {
            MSG.to_string()
        } else {
            format!("座標系の式は、{} の式を 3 つ書いてください", shown.join(", "))
        }
    };
    let e = rename_coords(&parse_at(source, 0, params)?, space);
    let Expr::Tuple(items) = e else { return Err(msg()) };
    if items.len() != 3 {
        return Err(msg());
    }
    let mut progs = Vec::with_capacity(3);
    for item in &items {
        let mut vars = Vec::new();
        item.collect_vars(&mut vars);
        if vars.iter().any(|v| !shown.contains(v) && !params.contains_key(v)) || matches!(item, Expr::Tuple(_) | Expr::List(_)) {
            return Err(msg());
        }
        progs.push(Program::compile_with(item, &shown_refs, params)?);
    }
    let [a, b, c]: [Program; 3] = progs.try_into().map_err(|_| msg())?;
    Ok(crate::coords::CoordMap::new([a, b, c]))
}

/// 「表示していない座標 h = h を使わない式 f」の形の式なら、(f, h の座標の番号)。f の変数は表示中の 3 つの座標
/// （ほかの表示していない座標は断面の値、回転は代入済み）。断面の塗り分け・伝達関数（[`crate::plot`] の `hidden`）に使う。
/// この形でない式・h が絡む回転をしている式はエラー
pub fn hidden_field(source: &str, params: &Params, space: &Space) -> Result<(Program, usize), String> {
    let shape_err = || "断面の塗り分け・伝達関数は、w = f(x, y, z) の形（表示していない座標 = 式）で使えます".to_string();
    let mut chains = parse_expanded(source, params)?;
    for chain in &mut chains {
        for e in &mut chain.exprs {
            *e = rename_coords(e, space);
        }
    }
    let [chain] = chains.as_slice() else { return Err(shape_err()) };
    if chain.rels != [Rel::Eq] {
        return Err(shape_err());
    }
    let hidden_of = |e: &Expr| match e {
        Expr::Var(name) => coord_index(name).filter(|&k| k < space.dims && !space.axes.contains(&k)).map(|k| (k, name.clone())),
        _ => None,
    };
    let (l, r) = (&chain.exprs[0], &chain.exprs[1]);
    let ((k, name), f) = match (hidden_of(l), hidden_of(r)) {
        (Some(h), _) if !r.uses_var(&h.1) => (h, r),
        (_, Some(h)) if !l.uses_var(&h.1) => (h, l),
        _ => return Err(shape_err()),
    };
    check_dims(f, space)?;
    if first_list(f).is_some() {
        return Err("断面の塗り分け・伝達関数では、リストは使えません".into());
    }
    let mut used = vec![name];
    f.collect_vars(&mut used);
    let f = match space.rotation_matrix() {
        Some(_) if !space.rotation_keeps_shown(&used) => {
            return Err("断面の塗り分け・伝達関数は、表示していない座標が絡む回転をしていないときだけ使えます".into())
        }
        Some(m) => rotate_coords(f, &m, space),
        None => f.clone(),
    };
    let shown = space.shown();
    let shown_refs: [&str; 3] = [&shown[0], &shown[1], &shown[2]];
    Ok((Program::compile_with(&f, &shown_refs, &space.with_hidden(params))?, k))
}

/// 等式の上限（表示中の座標は 3 つなので、4 本以上は一般に空）
const MAX_EQUATIONS: usize = 3;
/// 不等式の上限
const MAX_INEQUALITIES: usize = 8;

/// 連立方程式・不等式（[`Plot::System`]）。行ごとの関係の連鎖を、隣どうしの関係 (左, 関係, 右) に分けてコンパイルする
fn system(chains: Vec<crate::parser::Chain>, params: &Params, space: &Space) -> Result<Plot, String> {
    let shown = space.shown();
    let mut rels: Vec<(Expr, Rel, Expr)> = Vec::new();
    // 形の行（媒介変数表示の組）。不等式で切り取る
    let mut shape: Option<Vec<Expr>> = None;
    for chain in chains {
        if chain.rels.is_empty() {
            match &chain.exprs[0] {
                Expr::Tuple(items) if shape.is_none() => {
                    check_dims(&chain.exprs[0], space)?;
                    shape = Some(items.clone());
                    continue;
                }
                Expr::Tuple(_) => return Err("媒介変数表示は 1 行だけにしてください".into()),
                _ => return Err("連立の式は、行ごとに = か不等号を付けてください".into()),
            }
        }
        for e in &chain.exprs {
            check_dims(e, space)?;
            if matches!(e, Expr::Tuple(_)) {
                return Err("媒介変数表示は = を付けずに (x(t), y(t), z(t)) の形で書いてください".into());
            }
        }
        for (k, rel) in chain.rels.iter().enumerate() {
            rels.push((chain.exprs[k].clone(), *rel, chain.exprs[k + 1].clone()));
        }
    }
    // 連立一次方程式（等式が 2 本以上で、全部が座標の 1 次式）は、先に掃き出して独立な等式だけにする（計画 M7）
    if shape.is_none() {
        if let Some(reduced) = reduce_linear(&rels, params, space)? {
            let only_eq = reduced.len() == 1 && reduced[0].1 == Rel::Eq;
            if reduced.is_empty() {
                crate::expand::add_note("どの点も解（0 = 0）");
                return Ok(Plot::Multi(Vec::new()));
            }
            if only_eq {
                let (l, _, r) = reduced.into_iter().next().unwrap_or((Expr::Num(0.0), Rel::Eq, Expr::Num(0.0)));
                return compile_pair(l, r, params, space);
            }
            rels = reduced;
        }
    }
    let m = rels.iter().filter(|(_, r, _)| *r == Rel::Eq).count();
    if shape.is_some() && m > 0 {
        return Err("媒介変数表示と等式は、一緒に使えません（不等式で切り取ることはできます）".into());
    }
    if m > MAX_EQUATIONS {
        return Err(format!("等式が多すぎます（{MAX_EQUATIONS} 本まで）"));
    }
    if rels.len() - m > MAX_INEQUALITIES {
        return Err(format!("不等式は {MAX_INEQUALITIES} 本までにしてください"));
    }
    let ranged = space.ranged();
    for (l, _, r) in &rels {
        if !shown.iter().any(|v| l.uses_var(v) || r.uses_var(v)) {
            return Err(match l {
                Expr::Var(name) if coord_index(name).is_none() => "パラメータの定義は、1 行に 1 つだけ書いてください".into(),
                _ => format!("表示中の座標（{}）のどれかを使った式にしてください", shown.join(", ")),
            });
        }
        if ranged.iter().any(|(n, _, _)| l.uses_var(n) || r.uses_var(n)) {
            return Err("連立方程式・不等式では、範囲で重ねられません（断面にしてください）".into());
        }
    }
    let sides: Vec<Expr> = rels.iter().flat_map(|(l, _, r)| [l.clone(), r.clone()]).collect();
    // 媒介変数表示を不等式で切り取る: 不等式は、射影した後の表示中の座標の式
    if let Some(items) = shape {
        if sides.iter().any(|e| first_list(e).is_some()) {
            return Err("媒介変数表示を切り取る不等式には、リストは使えません".into());
        }
        let mut sys = system_of(rels, params, space)?;
        if let Plot::System { base, .. } = &mut sys {
            *base = Some(Box::new(parametric(items, params, space)?));
        }
        return Ok(sys);
    }
    // リストがあれば、組み合わせごとの連立にする
    if sides.iter().any(|e| first_list(e).is_some()) {
        let plots = expand_lists(&sides)?
            .into_iter()
            .map(|v| {
                let rs = rels.iter().zip(v.chunks(2)).map(|((_, rel, _), lr)| (lr[0].clone(), *rel, lr[1].clone())).collect();
                system_of(rs, params, space)
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Plot::Multi(plots));
    }
    system_of(rels, params, space)
}

/// 連立一次方程式の掃き出し。等式が 2 本以上あり、全部が全座標の 1 次式（係数は数とパラメータ）で、階数が等式の数より小さいとき、
/// 行の簡約形の独立な等式に置き換えた関係の列を返す（不等式はそのまま後ろに付ける）。矛盾していればエラー。
/// 1 次式でない・もう独立・回転している・リストや範囲で重ねる座標を使う、なら None（今までの道のまま）
fn reduce_linear(rels: &[(Expr, Rel, Expr)], params: &Params, space: &Space) -> Result<Option<Vec<(Expr, Rel, Expr)>>, String> {
    let eqs: Vec<&(Expr, Rel, Expr)> = rels.iter().filter(|r| r.1 == Rel::Eq).collect();
    if eqs.len() < 2 || space.rotation_matrix().is_some() || space.complex_coords {
        return Ok(None);
    }
    let n = space.dims;
    let coords: Vec<String> = (0..n).map(coord_name).collect();
    let ranged = space.ranged();
    let value = |e: &Expr| -> Option<f64> {
        let mut vars = Vec::new();
        e.collect_vars(&mut vars);
        if vars.iter().any(|v| coord_index(v).is_some()) {
            return None;
        }
        crate::expand::numbers(std::slice::from_ref(e), params).and_then(|v| v.first().copied())
    };
    // 行ごとに [表示中の 3 つの座標の係数, 右辺]
    let mut a = Vec::with_capacity(eqs.len() * 4);
    for (l, _, r) in &eqs {
        if first_list(l).is_some() || first_list(r).is_some() || ranged.iter().any(|(name, _, _)| l.uses_var(name) || r.uses_var(name)) {
            return Ok(None);
        }
        let e = Expr::Bin(BinOp::Sub, Box::new(l.clone()), Box::new(r.clone()));
        let mut coef = vec![0.0; n];
        for (k, name) in coords.iter().enumerate() {
            if !e.uses_var(name) {
                continue;
            }
            let Ok(d) = crate::diff::diff(&e, name) else { return Ok(None) };
            let Some(c) = value(&d) else { return Ok(None) };
            coef[k] = c;
        }
        let Some(c0) = value(&e.substitute(&|v: &str| coord_index(v).filter(|&k| k < n).map(|_| Expr::Num(0.0)))) else { return Ok(None) };
        // 表示していない座標は、断面の値を入れて定数の側へ移す
        let hidden: f64 = (0..n).filter(|k| !space.axes.contains(k)).map(|k| coef[k] * space.slice[k]).sum();
        a.extend(space.axes.iter().map(|&k| coef[k]));
        a.push(-(c0 + hidden));
    }
    let red = crate::linalg::Matrix::new(eqs.len(), 4, a).reduce(3);
    let rank = red.pivots.len();
    if (rank..eqs.len()).any(|i| red.rref.at(i, 3) != 0.0) {
        return Err("解がありません（連立一次方程式が矛盾しています）".into());
    }
    if rank == eqs.len() {
        return Ok(None);
    }
    let shown = space.shown();
    let mut out: Vec<(Expr, Rel, Expr)> = (0..rank)
        .map(|i| {
            let terms = (0..3).filter(|&j| red.rref.at(i, j) != 0.0).map(|j| (red.rref.at(i, j), Expr::Var(shown[j].clone()))).collect();
            (linear(0.0, terms), Rel::Eq, Expr::Num(red.rref.at(i, 3)))
        })
        .collect();
    out.extend(rels.iter().filter(|r| r.1 != Rel::Eq).cloned());
    Ok(Some(out))
}

/// 式が根号（√・整数でない指数の累乗）を含むか
fn has_radical(e: &Expr) -> bool {
    match e {
        Expr::Call(crate::functions::Func::Sqrt, _) => true,
        Expr::Bin(BinOp::Pow, a, b) => !matches!(**b, Expr::Num(k) if k.fract() == 0.0) || has_radical(a) || has_radical(b),
        Expr::Neg(a) => has_radical(a),
        Expr::Bin(_, a, b) => has_radical(a) || has_radical(b),
        Expr::Call(_, args) => args.iter().any(has_radical),
        _ => false,
    }
}

/// 関係の列（リストを含まない）を [`Plot::System`] にする
fn system_of(rels: Vec<(Expr, Rel, Expr)>, params: &Params, space: &Space) -> Result<Plot, String> {
    let shown = space.shown();
    let shown_refs: [&str; 3] = [&shown[0], &shown[1], &shown[2]];
    let consts = space.with_hidden(params);
    let rot = space.rotation_matrix();
    // 左辺 − 右辺（回転していれば、座標を回した後の座標の式に置き換えてから引く。`rot` が None なら回す前の式）
    let diff_with = |a: &Expr, b: &Expr, rot: &Option<Vec<Vec<f64>>>| -> Result<Program, String> {
        let (a, b) = match rot {
            Some(m) => (rotate_coords(a, m, space), rotate_coords(b, m, space)),
            None => (a.clone(), b.clone()),
        };
        Program::compile_with(&Expr::Bin(BinOp::Sub, Box::new(a), Box::new(b)), &shown_refs, &consts)
    };
    let diff = |a: &Expr, b: &Expr| diff_with(a, b, &rot);
    let (mut eqs, mut cuts, mut strict) = (Vec::new(), Vec::new(), Vec::new());
    let mut faces = Vec::new();
    let mut eq_rows = Vec::new();
    // 境目の面 G = 0 を、表示中の座標の 1 つについて解く（解が 1 つになる座標を先に。同じなら z を先に）
    let solve_face_with = |a: &Expr, b: &Expr, rot: &Option<Vec<Vec<f64>>>| -> Vec<(usize, Program)> {
        let (a, b) = match rot {
            Some(m) => (rotate_coords(a, m, space), rotate_coords(b, m, space)),
            None => (a.clone(), b.clone()),
        };
        let g = Expr::Bin(BinOp::Sub, Box::new(a), Box::new(b));
        let mut best: Vec<(usize, Program)> = Vec::new();
        let mut best_score = (usize::MAX, usize::MAX);
        for axis in [2, 0, 1] {
            let Some(rs) = crate::solve::roots(&g, &shown[axis], &shown_refs, &consts) else { continue };
            let (u, v) = others(axis);
            let Some(progs) = rs.iter().map(|r| Program::compile_with(r, &[shown_refs[u], shown_refs[v]], &consts).ok()).collect::<Option<Vec<_>>>() else {
                continue;
            };
            // 解の数が少ない座標を選ぶ。同じなら、根号（√・奇数乗根）を含まない解（その座標の 1 次式）を選ぶ。
            // 根号の解は、中身が 0 になる所で面が垂直になり、陽関数の格子では細長い三角形と縞になる
            // （0 ≤ −y ≤ x³ + z³ の z = ∛(−y − x³) より、y = −x³ − z³）
            let score = (progs.len(), rs.iter().any(has_radical) as usize);
            if best.is_empty() || score < best_score {
                best = progs.into_iter().map(|p| (axis, p)).collect();
                best_score = score;
            }
        }
        best
    };
    let solve_face = |a: &Expr, b: &Expr| solve_face_with(a, b, &rot);
    for (l, rel, r) in &rels {
        match rel {
            Rel::Eq => {
                eqs.push(diff(l, r)?);
                eq_rows.push((l.clone(), r.clone()));
            }
            Rel::Le | Rel::Lt => {
                cuts.push(diff(l, r)?);
                faces.push(solve_face(l, r));
            }
            Rel::Ge | Rel::Gt => {
                cuts.push(diff(r, l)?);
                faces.push(solve_face(r, l));
            }
        }
        if *rel != Rel::Eq {
            strict.push(matches!(rel, Rel::Lt | Rel::Gt));
        }
    }
    let base = match eq_rows.as_slice() {
        // 回す前の形（Plot::Implicit の local）は使わない（切り取った面は、回転が変わるたびに作り直す）
        [(l, r)] => Some(Box::new(match compile_pair(l.clone(), r.clone(), params, space)? {
            Plot::Implicit { prog, from, .. } => Plot::Implicit { prog, from, local: None, gpu: None, slice_source: None },
            p => p,
        })),
        _ => None,
    };
    let gpu = gpu_system(&rels, params, space);
    // 等式（左 − 右。回転した後の、表示中の座標の式）をグレブナー基底で解く
    let eq_exprs: Vec<Expr> = eq_rows
        .iter()
        .map(|(l, r)| {
            let (l, r) = match &rot {
                Some(m) => (rotate_coords(l, m, space), rotate_coords(r, m, space)),
                None => (l.clone(), r.clone()),
            };
            Expr::Bin(BinOp::Sub, Box::new(l), Box::new(r))
        })
        .collect();
    let exact3 = if eq_exprs.len() == 3 { crate::groebner::solve(&eq_exprs, &shown_refs, &consts) } else { None };
    let exact2 = if eq_exprs.len() == 2 {
        let flat: Vec<Expr> = eq_exprs.iter().map(|e| e.substitute(&|v: &str| (v == shown[2]).then_some(Expr::Num(0.0)))).collect();
        crate::groebner::solve(&flat, &shown_refs[..2], &consts)
    } else {
        None
    };
    // 回す前の形: 不等式だけで、回転が表示中の座標どうしだけ（使う座標が全部表示中）で、範囲で重ねる名前を使わないとき。
    // 回転していないときも作る（初めて回すときから、回すだけで描けるように）
    let local = if eqs.is_empty() && !space.flat_slice {
        let mut used = Vec::new();
        for (l, _, r) in &rels {
            l.collect_vars(&mut used);
            r.collect_vars(&mut used);
        }
        let ranged = space.ranged().iter().any(|(n, _, _)| used.contains(n));
        if !ranged && space.rotation_keeps_shown(&used) {
            let none = None;
            let mut lc = Vec::new();
            let mut lf = Vec::new();
            let mut sides = Vec::new();
            for (l, rel, r) in &rels {
                let (a, b) = if matches!(rel, Rel::Ge | Rel::Gt) { (r, l) } else { (l, r) };
                lc.push(diff_with(a, b, &none)?);
                lf.push(solve_face_with(a, b, &none));
                sides.push(Expr::Bin(BinOp::Sub, Box::new(a.clone()), Box::new(b.clone())));
            }
            let max = sides.into_iter().reduce(|a, b| Expr::Call(crate::functions::Func::Max, vec![a, b]));
            match max.map(|m| Program::compile_with(&m, &shown_refs, &consts)) {
                Some(Ok(max)) => Some(Box::new(LocalRegion { cuts: lc, faces: lf, max })),
                _ => None,
            }
        } else {
            None
        }
    } else {
        None
    };
    Ok(Plot::System { base, eqs, cuts, strict, faces, exact: [exact3, exact2], gpu, local })
}

/// 連立方程式・不等式の、GPU の描き方の式（[`GpuSystem`]）。どれかの式がコンパイルできなければ None（今までどおり、式に畳み込んだ形で描く）
fn gpu_system(rels: &[(Expr, Rel, Expr)], params: &Params, space: &Space) -> Option<Box<GpuSystem>> {
    let sides: Vec<(bool, Expr)> = rels
        .iter()
        .map(|(l, rel, r)| match rel {
            Rel::Eq | Rel::Le | Rel::Lt => (*rel == Rel::Eq, Expr::Bin(BinOp::Sub, Box::new(l.clone()), Box::new(r.clone()))),
            Rel::Ge | Rel::Gt => (false, Expr::Bin(BinOp::Sub, Box::new(r.clone()), Box::new(l.clone()))),
        })
        .collect();
    let coords: Vec<String> = (0..space.dims).map(coord_name).collect();
    let names: Vec<String> = params
        .iter()
        .filter(|(n, v)| v.im == 0.0 && !coords.contains(n) && sides.iter().any(|(_, e)| e.uses_var(n)))
        .map(|(n, _)| n.clone())
        .collect();
    let mut cs = params.clone();
    for n in names.iter().chain(&coords) {
        cs.remove(n);
    }
    let mut vars: Vec<&str> = coords.iter().map(String::as_str).collect();
    vars.extend(names.iter().map(String::as_str));
    let (mut eqs, mut cuts) = (Vec::new(), Vec::new());
    for (is_eq, e) in &sides {
        let p = Program::compile_with(e, &vars, &cs).ok()?;
        if *is_eq { eqs.push(p) } else { cuts.push(p) }
    }
    Some(Box::new(GpuSystem { eqs, cuts, params: names, dims: space.dims }))
}

/// 左辺 = 右辺（リストを含まない）の式をコンパイルする（陽関数・陰関数・範囲で重ねた立体）
fn compile_pair(l: Expr, r: Expr, params: &Params, space: &Space) -> Result<Plot, String> {
    let shown = space.shown();
    let shown_refs: [&str; 3] = [&shown[0], &shown[1], &shown[2]];
    let consts = space.with_hidden(params);
    let rot = space.rotation_matrix();
    let mut slice_source = None;

    // 陽関数で、回転が表示中の座標どうしだけなら、曲面をそのまま回せばよい（媒介変数表示と同じ扱い）
    if let Some(rot) = rot.as_ref().filter(|_| !space.ranged().iter().any(|(n, _, _)| l.uses_var(n) || r.uses_var(n))) {
        let mut used = Vec::new();
        l.collect_vars(&mut used);
        r.collect_vars(&mut used);
        if space.rotation_keeps_shown(&used) {
            if let Some(p) = find_explicit(&l, &r, &shown, &consts)? {
                // 2D の切り口で縦の座標を使う陽関数は、陰関数として計算し直す（Space::flat_slice を参照）
                let vertical = matches!(&p, Plot::Explicit { axis, prog, .. } if *axis == 2 || prog.uses_var_index(1));
                let tilted = space.axes[..2].iter().any(|&a| rot[space.axes[2]][a].abs() > 1e-12);
                if !(space.flat_slice && (vertical || tilted)) {
                    return Ok(match p {
                        Plot::Explicit { axis, names, prog, expr, .. } => Plot::Explicit { axis, names, prog, expr, rotated: true },
                        other => other,
                    });
                }
                slice_source = Some(Box::new(p));
            }
        }
    }

    // 表示していない座標が絡む回転でも、傾いた座標系の陽関数になるなら、陽関数のまま描く（段差・壁・色分けが残る）。
    // 2D の切り口（Space::flat_slice）と、範囲で重ねる名前を使う式は、今までどおり陰関数・立体にする
    if let Some(m) = &rot {
        if !space.flat_slice {
            let ranged = space.ranged();
            let (rl, rr) = (rotate_coords(&l, m, space), rotate_coords(&r, m, space));
            if !ranged.iter().any(|(n, _, _)| rl.uses_var(n) || rr.uses_var(n)) {
                if let Some(p) = tilted(&l, &r, m, &consts, space)? {
                    return Ok(p);
                }
            }
        }
    }

    // 陰関数で、回転が表示中の座標どうしだけなら、回す前の F も作っておく（形を回すだけで描ける）。
    // 回転が無いときも作る（式を入れて初めて回すときから、回す前の形を回すだけで描けるように。Worker が面を返した後に作る）。
    // 陽関数は上の道で描く。2D の切り口・範囲で重ねる名前を使う式は、今までどおり回した F だけ
    // 範囲で重ねる名前は、陽関数かどうかを調べる前に除く（重ねるパラメータは、陽関数の判定では使えない変数としてエラーになる）
    let ranged = space.ranged().iter().any(|(n, _, _)| l.uses_var(n) || r.uses_var(n));
    let local = if !space.flat_slice && !ranged && find_explicit(&l, &r, &shown, &consts)?.is_none() {
        let mut used = Vec::new();
        l.collect_vars(&mut used);
        r.collect_vars(&mut used);
        if space.rotation_keeps_shown(&used) && shown.iter().any(|v| l.uses_var(v) || r.uses_var(v)) {
            let f = Expr::Bin(BinOp::Sub, Box::new(l.clone()), Box::new(r.clone()));
            Some((Program::compile_with(&f, &shown_refs, &consts)?, f))
        } else {
            None
        }
    } else {
        None
    };
    let local = local.map(|(p, _)| p);

    // 陽関数（z = f(x, y) など）を回して陰関数として計算し直すときは、その形を覚えておく。
    // 複素数の色分けをしているなら、f の代わりに高さ（|f|・Re f・Im f）を使う（f のままだと実数にならない）
    let mut from = None;
    let (l, r) = if rot.is_some() {
        match find_explicit(&l, &r, &shown, &consts)? {
            Some(p @ Plot::Explicit { .. }) => {
                from = Some(p.kind_label());
                match space.complex_height {
                    Some(h) => wrap_explicit_side(l, r, &shown, h),
                    None => (l, r),
                }
            }
            _ => (l, r),
        }
    } else {
        (l, r)
    };

    // GPU の光線の描き方の F: 回転・断面を入れる前の元の座標の式（[`GpuImplicit`]）。光線の描き方が苦手な形は、同じ解の得意な形に言い換える
    let unrotated = gpu_friendly(&l, &r);
    // 回転していれば、元の座標 p を回転後の座標 q の式に置き換える
    let (l, r) = match &rot {
        Some(m) => (rotate_coords(&l, m, space), rotate_coords(&r, m, space)),
        None => (l, r),
    };

    // 範囲で重ねる名前を使っていれば、立体
    let ranged: Vec<(String, f64, f64)> =
        space.ranged().into_iter().filter(|(n, _, _)| l.uses_var(n) || r.uses_var(n)).collect();
    if !ranged.is_empty() {
        let mut vars: Vec<&str> = shown_refs.to_vec();
        vars.extend(ranged.iter().map(|(n, _, _)| n.as_str()));
        let f = Expr::Bin(BinOp::Sub, Box::new(l), Box::new(r));
        let prog = Program::compile_with(&f, &vars, &consts)?;
        return Ok(Plot::Solid {
            prog,
            ranges: ranged.iter().map(|&(_, a, b)| (a, b)).collect(),
            names: ranged.into_iter().map(|(n, _, _)| n).collect(),
        });
    }

    // 片方が表示中の座標1つだけで、もう片方にその座標が無ければ陽関数（回転すると陰関数になる）
    if let Some(p) = find_explicit(&l, &r, &shown, &consts)? {
        return Ok(p);
    }
    if !shown.iter().any(|v| l.uses_var(v) || r.uses_var(v)) {
        return Err(format!("表示中の座標（{}）のどれかを使った式にしてください", shown.join(", ")));
    }
    let f = Expr::Bin(BinOp::Sub, Box::new(l), Box::new(r));
    let prog = Program::compile_with(&f, &shown_refs, &consts)?;
    // 線・点だけの解として描く式・複素数の陰関数（i を含む式）は、回す前の形（面のメッシュを 3D オブジェクトとして回す）を使わない
    let local = local.filter(|_| prog.common_zero_sets().is_none() && !prog.has_complex_const());
    // GPU の描き方の F: 回転・断面を入れる前の、元の座標 x, y, z, w, … の式。実数のスライダーのパラメータも変数として残す
    // （複素数のパラメータは定数のまま）。範囲で重ねる名前を使う式は、上で立体になっているので、ここには来ない
    let gpu = {
        let coords: Vec<String> = (0..space.dims).map(coord_name).collect();
        let names: Vec<String> =
            params.iter().filter(|(n, v)| v.im == 0.0 && unrotated.uses_var(n) && !coords.contains(n)).map(|(n, _)| n.clone()).collect();
        let mut cs = params.clone();
        for n in names.iter().chain(&coords) {
            cs.remove(n);
        }
        let mut vars: Vec<&str> = coords.iter().map(String::as_str).collect();
        vars.extend(names.iter().map(String::as_str));
        Program::compile_with(&unrotated, &vars, &cs).ok().map(|p| Box::new(GpuImplicit { prog: p, params: names, dims: space.dims }))
    };
    Ok(Plot::Implicit { prog, from, local, gpu, slice_source })
}

/// 傾いた座標系の陽関数の変数 u, v の名前（利用者が打てない名前にして、パラメータ u・v と衝突しないようにする）
pub const TILT_U: &str = "#u";
pub const TILT_V: &str = "#v";

/// 回転した陽関数 p_a = f(p_k : k ∈ U) を、傾いた直交座標系の陽関数 t = g(u, v) にする（[`Plot::Tilted`]）。
///
/// 元の座標 p_k は表示中の点 q の 1 次式 p_k = β_k + L_k · q。f が使う座標の L_k が張る空間 W に垂直な向き e₃ を「高さ」にとると、
/// q = u e₁ + v e₂ + t e₃（e₁, e₂ は e₃ に垂直）で p_k（k ∈ U）は t によらないので、
/// t = (f(p_k(u, v)) − β_a − (L_a·e₁) u − (L_a·e₂) v) / (L_a·e₃)。
/// 原点は 0 にとる（どこにとっても同じ形になるので、表示範囲を知らなくてよい）。
///
/// 使えない場合は None（今までどおり陰関数にする）: 陽関数でない、W の次元が 3（f が 3 つ以上の独立な向きを使う）、
/// L_a の W に垂直な成分の長さが 0.05 未満（面が「高さ」の向きにほぼ平行で、傾きすぎる）
fn tilted(l: &Expr, r: &Expr, m: &[Vec<f64>], consts: &Params, space: &Space) -> Result<Option<Plot>, String> {
    let shown = space.shown();
    let Some(explicit) = find_explicit(l, r, &shown, consts)? else {
        return Ok(None);
    };
    let label = explicit.kind_label();
    let Plot::Explicit { axis, .. } = explicit else {
        return Ok(None);
    };
    // 左辺の座標 a と、f の式（find_explicit と同じく左辺を優先）
    let name = &shown[axis];
    let f = if matches!(l, Expr::Var(v) if v == name) && !r.uses_var(name) { r } else { l };
    let a = space.axes[axis];

    let n = space.dims;
    let rn = rot_dims(n);
    let c = |j: usize| space.center.get(j).copied().unwrap_or(0.0);
    let s = space.axes;
    // p_k = β_k + L_k · q
    let lk = |k: usize| -> [f64; 3] { [m[s[0]][k], m[s[1]][k], m[s[2]][k]] };
    let beta = |k: usize| -> f64 {
        let mut b = c(k) - (0..3).map(|i| m[s[i]][k] * c(s[i])).sum::<f64>();
        for j in (0..rn).filter(|j| !s.contains(j)) {
            let sigma = if j < n { space.slice.get(j).copied().unwrap_or(0.0) } else { 0.0 };
            b += m[j][k] * (sigma - c(j));
        }
        b
    };

    // f が使う座標
    let mut vars = Vec::new();
    f.collect_vars(&mut vars);
    let mut used: Vec<usize> = vars.iter().filter_map(|v| coord_index(v)).filter(|&k| k < n).collect();
    used.sort_unstable();
    used.dedup();

    let Some(frame) = tilted_frame(&used.iter().map(|&k| lk(k)).collect::<Vec<_>>(), lk(a)) else {
        return Ok(None);
    };
    let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    // 元の座標 p_k を (u, v) の 1 次式 [定数, u の係数, v の係数] にする
    let coef = |k: usize| [beta(k), dot(lk(k), frame[0]), dot(lk(k), frame[1])];
    let as_expr = |cf: [f64; 3]| linear(cf[0], vec![(cf[1], Expr::Var(TILT_U.into())), (cf[2], Expr::Var(TILT_V.into()))]);
    let in_uv = |e: &Expr| {
        e.substitute(&|v: &str| {
            let k = coord_index(v).filter(|&k| k < n)?;
            Some(as_expr(coef(k)))
        })
    };
    // 複素数の色分けをしているなら、高さは h(f)、色は f
    let fh = match space.complex_height {
        Some(h) => Expr::Call(h, vec![f.clone()]),
        None => f.clone(),
    };
    let ca = coef(a);
    let g = Expr::Bin(
        BinOp::Div,
        Box::new(Expr::Bin(BinOp::Sub, Box::new(in_uv(&fh)), Box::new(as_expr(ca)))),
        Box::new(Expr::Num(dot(lk(a), frame[2]))),
    );
    let uv = [TILT_U, TILT_V];
    let prog = Program::compile_with(&g, &uv, consts)?;
    let color = match space.complex_height {
        Some(_) => Some(Program::compile_with(&in_uv(f), &uv, consts)?),
        None => None,
    };
    let grid = (used.len() == 2).then(|| [coef(used[0]), coef(used[1])]);
    Ok(Some(Plot::Tilted { prog, color, frame, grid, label }))
}

/// 傾いた座標系の向き [e₁, e₂, e₃]（[`tilted`]）。`ls` は f が使う座標の L_k、`la` は左辺の座標の L_a。
/// e₃ は L_a の、ls が張る空間 W に垂直な成分の向き。e₁, e₂ は W の基底を含むようにとり、右手系にする。
/// W の次元が 3、または垂直な成分の長さが 0.05 未満なら None
pub fn tilted_frame(ls: &[[f64; 3]], la: [f64; 3]) -> Option<[[f64; 3]; 3]> {
    let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    // x から、正規直交な basis の成分を除いたもの
    let reject = |x: [f64; 3], basis: &[[f64; 3]]| {
        let mut r = x;
        for b in basis {
            let d = dot(r, *b);
            r = [r[0] - d * b[0], r[1] - d * b[1], r[2] - d * b[2]];
        }
        r
    };
    let unit = |x: [f64; 3]| {
        let len = dot(x, x).sqrt();
        [x[0] / len, x[1] / len, x[2] / len]
    };
    // グラム・シュミット（長さが 1e-9 未満になったものは捨てる）
    let mut w: Vec<[f64; 3]> = Vec::new();
    for &x in ls {
        let r = reject(x, &w);
        if dot(r, r).sqrt() >= 1e-9 {
            w.push(unit(r));
        }
    }
    if w.len() >= 3 {
        return None;
    }
    let g = reject(la, &w);
    if dot(g, g).sqrt() < 0.05 {
        return None;
    }
    let e3 = unit(g);
    // e₃ に垂直な平面の基底: W の基底から始め、足りない分は座標軸の向きで補う
    let mut plane = w.clone();
    for cand in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
        if plane.len() == 2 {
            break;
        }
        let mut basis = plane.clone();
        basis.push(e3);
        let r = reject(cand, &basis);
        if dot(r, r).sqrt() >= 1e-6 {
            plane.push(unit(r));
        }
    }
    let (e1, mut e2) = (plane[0], plane[1]);
    let cross = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
    if dot(cross, e3) < 0.0 {
        e2 = [-e2[0], -e2[1], -e2[2]];
    }
    Some([e1, e2, e3])
}

/// 陽関数 `座標 = f` の f の側を h(f) にする（h は abs・Re・Im）
fn wrap_explicit_side(l: Expr, r: Expr, shown: &[String; 3], h: crate::functions::Func) -> (Expr, Expr) {
    let is_axis = |e: &Expr, other: &Expr| matches!(e, Expr::Var(v) if shown.contains(v) && !other.uses_var(v));
    if is_axis(&l, &r) {
        (l, Expr::Call(h, vec![r]))
    } else if is_axis(&r, &l) {
        (Expr::Call(h, vec![l]), r)
    } else {
        (l, r)
    }
}

/// 1次式 c₀ + Σ cⱼ·eⱼ を作る（係数がほぼ 0 の項は省く）
fn linear(c0: f64, terms: Vec<(f64, Expr)>) -> Expr {
    let mut e = Expr::Num(c0);
    for (c, t) in terms {
        if c.abs() < 1e-14 {
            continue;
        }
        let term = if (c - 1.0).abs() < 1e-14 { t } else { Expr::Bin(BinOp::Mul, Box::new(Expr::Num(c)), Box::new(t)) };
        e = Expr::Bin(BinOp::Add, Box::new(e), Box::new(term));
    }
    e
}

/// 式の中の座標 p_k を、回転後の座標 q で表した p_k = c_k + Σ_j R[j][k] (q_j − c_j) に置き換える。
/// 次元より先の座標（回転のときだけ使う w など）は、式には出てこない（その方向にはまっすぐ伸びた形とみなす）
fn rotate_coords(e: &Expr, m: &[Vec<f64>], space: &Space) -> Expr {
    let n = space.dims;
    let rn = rot_dims(n);
    let c: Vec<f64> = (0..rn).map(|j| space.center.get(j).copied().unwrap_or(0.0)).collect();
    e.substitute(&|name: &str| {
        let k = coord_index(name).filter(|&k| k < n)?;
        let c0 = c[k] - (0..rn).map(|j| m[j][k] * c[j]).sum::<f64>();
        Some(linear(c0, (0..rn).map(|j| (m[j][k], Expr::Var(coord_name(j)))).collect()))
    })
}

/// GPU の光線の描き方（区間演算で、連続な関数の符号の変化を手がかりに面を探す）に渡す F = l − r。
/// 苦手な形を、同じ解を持つ得意な形に言い換える（メッシュ・画面に出す式はそのまま）:
///
/// - `mod(A, B) = 0` → `sin(π A / B) = 0`。どちらも「A / B が整数」で、B = 0 ではどちらも定義されない。
///   `mod` の解は値が飛ぶ所にあり、符号が変わらない（B > 0 なら 0 以上）ので、光線は飛ぶ所ごとに 1/16 画素まで詰めて
///   「0 に触れているか」を調べるしかなく、解が密な所（`mod(x y, z) = 0` の z ≈ 0）で回数を使い切っていた。
///   `sin` は解の所で符号が変わるので、1 画素の区間の両端の符号ですぐ分かる。
///   32 ビットの GPU では A / B が大きい所（10⁶ を超える）で sin の角度の誤差が大きくなるが、そこは解の間隔が 1 画素よりずっと細かい所
fn gpu_friendly(l: &Expr, r: &Expr) -> Expr {
    let zero = |e: &Expr| matches!(e, Expr::Num(v) if *v == 0.0);
    let modulo = |e: &Expr| match e {
        Expr::Call(crate::functions::Func::Mod, args) if args.len() == 2 => Some((args[0].clone(), args[1].clone())),
        _ => None,
    };
    let pair = if zero(r) { modulo(l) } else if zero(l) { modulo(r) } else { None };
    if let Some((a, b)) = pair {
        let arg = Expr::Bin(
            BinOp::Div,
            Box::new(Expr::Bin(BinOp::Mul, Box::new(Expr::Const(crate::parser::Constant::Pi)), Box::new(a))),
            Box::new(b),
        );
        return Expr::Call(crate::functions::Func::Sin, vec![arg]);
    }
    Expr::Bin(BinOp::Sub, Box::new(l.clone()), Box::new(r.clone()))
}
/// パラメータの定義 name = rhs。右辺の値を計算する
fn param(name: &str, rhs: &Expr, params: &Params) -> Result<Plot, String> {
    // 自分自身は使えないので、ほかのパラメータだけを渡す
    let mut others = params.clone();
    others.remove(name);
    let prog = Program::compile_with(rhs, &[], &others)?;
    let mut out = [C::new(0.0, 0.0)];
    prog.eval_batch(&[], &mut out);
    if !(out[0].re.is_finite() && out[0].im.is_finite()) {
        return Err(format!("{name} の値が計算できません"));
    }
    Ok(Plot::Param { name: name.to_string(), value: out[0], uses: prog.used_params().to_vec() })
}

/// 組が、ベクトル場か（成分が座標を使い、媒介変数 t, u, v, t₁, … を使わない）。座標と媒介変数が混ざった組は、
/// 今までどおり媒介変数表示の道でエラーにする
fn is_field(items: &[Expr], space: &Space) -> bool {
    let mut vars = Vec::new();
    items.iter().for_each(|e| e.collect_vars(&mut vars));
    let coord = vars.iter().any(|v| coord_index(v).is_some_and(|k| k < space.dims));
    coord && !vars.iter().any(|v| is_param_var(v)) && items.len() <= 3
}

/// ベクトル場（[`Plot::Field`]）。成分は x, y, z の順（2 つなら、縦の成分は 0）。表示していない座標は断面の値
fn field(mut items: Vec<Expr>, params: &Params, space: &Space) -> Result<Plot, String> {
    if space.axes != [0, 1, 2] {
        return Err("ベクトル場は、x, y, z を表示しているときに描けます（表示する座標を x, y, z にしてください）".into());
    }
    if space.complex_coords {
        return Err("座標を複素数にしている間は、ベクトル場は描けません".into());
    }
    if items.iter().any(|e| first_list(e).is_some()) {
        return Err("ベクトル場の成分には、リストは使えません".into());
    }
    let ranged = space.ranged();
    if ranged.iter().any(|(n, _, _)| items.iter().any(|e| e.uses_var(n))) {
        return Err("ベクトル場では、範囲で重ねられません（断面にしてください）".into());
    }
    while items.len() < 3 {
        items.push(Expr::Num(0.0));
    }
    let shown = space.shown();
    let shown_refs: [&str; 3] = [&shown[0], &shown[1], &shown[2]];
    let consts = space.with_hidden(params);
    // 回転: 表示中の座標どうしだけなら、点と向きを回す
    let mut rot = None;
    if let Some(m) = space.rotation_matrix() {
        let n = rot_dims(space.dims);
        let only_shown = (0..n).all(|i| (0..n).all(|j| i == j || (i < 3 && j < 3) || m[i][j].abs() < 1e-12));
        if !only_shown {
            return Err("この回転ではベクトル場を描けません（表示中の座標どうしの回転だけにしてください）".into());
        }
        items = items.iter().map(|e| rotate_coords(e, &m, space)).collect();
        rot = Some([0, 1, 2].map(|i| [0, 1, 2].map(|j| m[i][j])));
    }
    let progs = items.iter().map(|e| Program::compile_with(e, &shown_refs, &consts)).collect::<Result<Vec<_>, _>>()?;
    Ok(Plot::Field { progs, rot })
}

/// 組 (a, b, c, …) を媒介変数表示としてコンパイルする。
/// 成分は座標の順（x, y, z, w, …）で、すべての成分を計算する。表示する3つの座標への回転・射影は後で行う。
fn parametric(mut items: Vec<Expr>, params: &Params, space: &Space) -> Result<Plot, String> {
    if items.len() > space.dims {
        return Err(format!(
            "成分が {n} 個あります。{n} 次元の図形を描くには、次元を {n} にしてください",
            n = items.len()
        ));
    }
    while items.len() < space.dims {
        items.push(Expr::Num(0.0));
    }

    // 使われている媒介変数を調べる（組の中では t, u, v, t₁, t₂ は変数）
    let mut all = Vec::new();
    for e in &items {
        e.collect_vars(&mut all);
    }
    let mut vars: Vec<String> = all.into_iter().filter(|v| is_param_var(v)).collect();
    vars.sort_by_key(|v| match v.as_str() {
        "t" => 0,
        "u" => 1,
        "v" => 2,
        s => 10 + s[1..].parse::<usize>().unwrap_or(0),
    });
    let named = vars.iter().filter(|v| matches!(v.as_str(), "t" | "u" | "v")).count();
    if named > 0 && named < vars.len() {
        return Err("媒介変数は t / u, v / u, v, t / t₁, t₂, t₃, … のどれか1組だけを使ってください".into());
    }
    if named > 0 {
        // t だけなら曲線、u・v だけなら曲面（u だけ・v だけでも曲面）、t と u・v を両方使えば立体
        let has_t = vars.iter().any(|v| v == "t");
        let has_uv = vars.iter().any(|v| v == "u" || v == "v");
        vars = match (has_t, has_uv) {
            (true, true) => vec!["u".into(), "v".into(), "t".into()],
            (false, true) => vec!["u".into(), "v".into()],
            _ => vec!["t".into()],
        };
    }
    if vars.is_empty() {
        vars.push("t".into()); // 媒介変数を使っていない（点）
    }
    vars.dedup();

    // 範囲で重ねるパラメータを使っていれば、それも媒介変数として動かす（曲線 → 曲面）
    let ranged: Vec<String> = space
        .param_ranges
        .iter()
        .map(|(n, _, _)| n.clone())
        .filter(|n| !vars.contains(n) && items.iter().any(|e| e.uses_var(n)))
        .collect();
    vars.extend(ranged);
    if vars.len() > MAX_PARAM_VARS {
        return Err(format!(
            "媒介変数（と、範囲で重ねるパラメータ）は、合わせて {MAX_PARAM_VARS} 個までにしてください"
        ));
    }
    let mut consts = params.clone();
    for (n, _, _) in &space.param_ranges {
        consts.remove(n);
    }

    let var_refs: Vec<&str> = vars.iter().map(|s| s.as_str()).collect();
    let mut sets = Vec::new();
    for variant in expand_lists(&items)? {
        let mut progs = Vec::with_capacity(variant.len());
        for e in &variant {
            // 組の中の t, u, v は媒介変数なので、同じ名前のパラメータより優先される
            progs.push(Program::compile_with(e, &var_refs, &consts)?);
        }
        sets.push(progs);
    }
    // 回転・射影は点を計算した後で行う（Space::view）
    Ok(match vars.len() {
        1 => Plot::Curve { sets, vars },
        2 => Plot::Surface { sets, vars },
        3 => Plot::Volume { sets, vars },
        _ => Plot::Sheets { sets, vars },
    })
}

/// 複素平面の図形。`=` の無い式 f が、座標（x, y, z, …）を使わず、媒介変数（t / u, v / t₁, t₂, …）だけを
/// 変数として使っていれば（同じ名前のパラメータが定義されていなければ）、f の値を複素平面の点として描く。
/// 複素平面は x 座標が実部・y 座標が虚部の平面で、ほかの図形と同じ空間に置く。縦（z 座標）には
/// 「+α」として、0（平面の上）・媒介変数・|f|・arg f などを選んで描ける（[`Space::complex_height`]・[`Space::lift_param`]）。
/// 例) `e^(i t)` → 単位円（縦を t にすると、らせん）、`(u + i v)^2` → 長方形の像（格子線は等角写像の像）
fn complex_plane(e: &Expr, params: &Params, space: &Space) -> Result<Option<Plot>, String> {
    let mut used = Vec::new();
    e.collect_vars(&mut used);
    if used.iter().any(|v| coord_index(v).is_some()) {
        return Ok(None);
    }
    let vars: Vec<&String> = used.iter().filter(|v| is_param_var(v) && !params.contains_key(*v)).collect();
    // 媒介変数が無ければ、値は1つの複素数。i を書いたか、複素数のパラメータを使ったか、値が実数でなければ点にする
    // （実数の定数 `2` や `a` は、今までどおり平面 z = 2 など）
    let point = vars.is_empty();
    if point {
        let complex = uses_i(e)
            || used.iter().any(|v| space.complex_params.contains(v))
            || used.iter().any(|v| params.get(v).is_some_and(|c| c.im != 0.0));
        if !complex || used.iter().any(|v| !params.contains_key(v)) {
            return Ok(None);
        }
    }
    let call = |f: crate::functions::Func| Expr::Call(f, vec![e.clone()]);
    use crate::functions::Func;
    let mut items = vec![call(Func::Re), call(Func::Im), Expr::Num(0.0)];
    let mut height = None;
    if space.lift_param && !point {
        // 媒介変数が1つ（曲線）のときだけ。曲面のときは平面のまま
        let p = parametric(items.clone(), params, space)?;
        if let [v] = p.param_vars().as_slice() {
            items[2] = Expr::Var(v.clone());
            height = Some(prettify(v));
        }
    } else if let Some(h) = space.complex_height {
        items[2] = call(h);
        height = Some(match h {
            Func::Abs => "|f|".into(),
            Func::Arg => "arg f".into(),
            Func::Re => "Re f".into(),
            Func::Im => "Im f".into(),
            _ => String::new(),
        });
    }
    let inner = parametric(items, params, space)?;
    let shape = match &inner {
        Plot::Curve { .. } if point => "点",
        Plot::Curve { .. } => "曲線",
        Plot::Surface { .. } => "曲面",
        Plot::Volume { .. } => "立体",
        _ => "図形",
    };
    let label = match height {
        Some(h) => format!("複素平面の{shape}（横 Re f, 奥 Im f, 縦 {h}）"),
        None => format!("複素平面の{shape}（横 Re f, 奥 Im f）"),
    };
    Ok(Some(Plot::ComplexPlane { inner: Box::new(inner), label }))
}

/// 式が虚数単位 i を使っているか
fn uses_i(e: &Expr) -> bool {
    use crate::parser::Constant;
    match e {
        Expr::Const(Constant::I) => true,
        Expr::Num(_) | Expr::Const(_) | Expr::Var(_) => false,
        Expr::Neg(a) => uses_i(a),
        Expr::Bin(_, a, b) => uses_i(a) || uses_i(b),
        Expr::Call(_, args) | Expr::Tuple(args) | Expr::List(args) => args.iter().any(uses_i),
        // 展開の段（expand.rs）で消える節は、ここには来ない
        Expr::Apply { .. } | Expr::Binder { .. } | Expr::Op { .. } | Expr::Matrix { .. } | Expr::Index { .. } | Expr::Prime { .. } | Expr::Cond(_) => false,
    }
}

/// 媒介変数の名前を画面に出す形にする（t1 → t₁）
fn prettify(v: &str) -> String {
    const SUB: [char; 10] = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];
    v.chars().map(|c| c.to_digit(10).map_or(c, |d| SUB[d as usize])).collect()
}

/// リストを含む式の中で、最初に出てくるリストの要素を返す（リストが無ければ None）
fn first_list(e: &Expr) -> Option<&Vec<Expr>> {
    match e {
        Expr::List(items) => Some(items),
        Expr::Num(_) | Expr::Const(_) | Expr::Var(_) => None,
        Expr::Neg(a) => first_list(a),
        Expr::Bin(_, a, b) => first_list(a).or_else(|| first_list(b)),
        Expr::Call(_, args) | Expr::Tuple(args) => args.iter().find_map(first_list),
        Expr::Apply { .. } | Expr::Binder { .. } | Expr::Op { .. } | Expr::Matrix { .. } | Expr::Index { .. } | Expr::Prime { .. } | Expr::Cond(_) => None,
    }
}

/// 最初に出てくるリストを `with` に置き換えた式。置き換えたら `done` を true にする
fn replace_first_list(e: &Expr, with: &Expr, done: &mut bool) -> Expr {
    if *done {
        return e.clone();
    }
    match e {
        Expr::List(_) => {
            *done = true;
            with.clone()
        }
        Expr::Num(_) | Expr::Const(_) | Expr::Var(_) => e.clone(),
        Expr::Neg(a) => Expr::Neg(Box::new(replace_first_list(a, with, done))),
        Expr::Bin(o, a, b) => {
            let a = replace_first_list(a, with, done);
            let b = replace_first_list(b, with, done);
            Expr::Bin(*o, Box::new(a), Box::new(b))
        }
        Expr::Call(f, args) => Expr::Call(*f, args.iter().map(|a| replace_first_list(a, with, done)).collect()),
        Expr::Tuple(args) => Expr::Tuple(args.iter().map(|a| replace_first_list(a, with, done)).collect()),
        Expr::Apply { .. } | Expr::Binder { .. } | Expr::Op { .. } | Expr::Matrix { .. } | Expr::Index { .. } | Expr::Prime { .. } | Expr::Cond(_) => e.clone(),
    }
}

/// 組の成分に含まれるリストを展開する。リストが複数あれば、すべての組み合わせ（直積）を作る。
/// 例) (t, [−1, 1], [−1, 1]) → (t, −1, −1), (t, −1, 1), (t, 1, −1), (t, 1, 1)
fn expand_lists(items: &[Expr]) -> Result<Vec<Vec<Expr>>, String> {
    const MAX: usize = 256;
    let mut done: Vec<Vec<Expr>> = Vec::new();
    let mut todo: Vec<Vec<Expr>> = vec![items.to_vec()];
    while let Some(variant) = todo.pop() {
        let Some(list) = variant.iter().find_map(first_list).cloned() else {
            done.push(variant);
            continue;
        };
        for elem in &list {
            if first_list(elem).is_some() {
                return Err("リストの中にリストは書けません".into());
            }
            let mut replaced = false;
            todo.push(variant.iter().map(|e| replace_first_list(e, elem, &mut replaced)).collect());
        }
        if done.len() + todo.len() > MAX {
            return Err(format!("リストの組み合わせが多すぎます（{MAX} 個まで）"));
        }
    }
    done.reverse(); // 書いた順に並べる
    Ok(done)
}

/// 変数を含まない式（範囲の指定など）の値を計算する。例: "2pi" → 6.283...
pub fn eval_constant(source: &str) -> Result<f64, String> {
    eval_constant_with(source, &Params::new()).map(|(v, _)| v)
}

/// パラメータを使ってよい式の値を計算する（媒介変数の範囲 0 ≤ t ≤ b など）。
/// 値と、使ったパラメータの名前を返す
pub fn eval_constant_with(source: &str, params: &Params) -> Result<(f64, Vec<String>), String> {
    let e = parse_at(source, 0, params)?;
    let prog = Program::compile_with(&e, &[], params).map_err(|err| {
        // 未定義の文字なら、その名前を残す（画面側で「a のスライダーを追加」ボタンを出すのに使う）
        match err.strip_prefix("変数 '").and_then(|r| r.split_once('\'')) {
            Some((name, _)) => format!("変数 '{name}' はここでは使えません（パラメータとして定義してください）"),
            None => "数値と、pi などの定数・パラメータだけで書いてください".to_string(),
        }
    })?;
    let mut out = [C::new(0.0, 0.0)];
    prog.eval_batch(&[], &mut out);
    let v = crate::eval::as_real(out[0]);
    if v.is_finite() {
        Ok((v, prog.used_params().to_vec()))
    } else {
        Err("実数の値になりません".into())
    }
}

thread_local! {
    /// 最後に作った陽関数の (縦の軸, 式, 表示中の座標の名前)。点を押したときの「注目する点」（極値・零点）を求めるため（[`take_last_explicit`]）
    static LAST_EXPLICIT: std::cell::RefCell<Option<(usize, Expr, [String; 3])>> = const { std::cell::RefCell::new(None) };
}

/// 最後に作った陽関数の式を取り出す（`lib.rs` の [`crate::plot`] が、計算の前に空にしてから使う）
pub fn take_last_explicit() -> Option<(usize, Expr, [String; 3])> {
    LAST_EXPLICIT.with(|l| l.borrow_mut().take())
}

fn explicit(axis: usize, e: &Expr, shown: &[String; 3], consts: &Params) -> Result<Plot, String> {
    let (a, b) = others(axis);
    LAST_EXPLICIT.with(|l| *l.borrow_mut() = Some((axis, e.clone(), shown.clone())));
    let prog = Program::compile_with(e, &[&shown[a], &shown[b]], consts)?;
    Ok(Plot::Explicit { axis, names: shown.clone(), prog, expr: Box::new(e.clone()), rotated: false })
}

/// 片方が表示中の座標1つだけで、もう片方にその座標が無ければ陽関数。左辺を優先して調べる（z = x なら「z = f(x, y)」）
fn find_explicit(l: &Expr, r: &Expr, shown: &[String; 3], consts: &Params) -> Result<Option<Plot>, String> {
    for (one, other) in [(l, r), (r, l)] {
        for (axis, name) in shown.iter().enumerate() {
            if matches!(one, Expr::Var(v) if v == name) && !other.uses_var(name) {
                return explicit(axis, other, shown, consts).map(Some);
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(s: &str) -> String {
        match compile(s) {
            Ok(p) => p.kind_label(),
            Err(e) => format!("error: {e}"),
        }
    }

    fn space4(axes: [usize; 3], w: f64) -> Space {
        Space::basic(4, axes, vec![0.0, 0.0, 0.0, w])
    }

    /// 関係が「= 1 つだけ」か「無し」の式は、連立の文法を入れる前と同じ種類になる
    #[test]
    fn single_relation_is_unchanged() {
        for (src, want) in [
            ("z = x^2", "z = f(x, y)"),
            ("x^2 + y^2 = z", "z = f(x, y)"),
            ("y = x z", "y = f(x, z)"),
            ("x = cos(y)", "x = f(y, z)"),
            ("x^2 + y^2 + z^2 = 1", "陰関数"),
            ("z = z^2 + x", "陰関数"),
            ("sin(x) y", "z = f(x, y)"),
            ("a = 1", "パラメータ"),
            ("z = [1, 2] x", "z = f(x, y) × 2"),
            ("(cos t, sin t, t)", "媒介変数表示の曲線"),
            ("e^(i t)", "複素平面の曲線（横 Re f, 奥 Im f）"),
            ("mod(x, y) = 1", "陰関数"),
        ] {
            assert_eq!(kind(src), want, "{src}");
        }
    }

    /// 連立方程式・不等式の種類
    #[test]
    fn systems_are_classified() {
        for (src, want) in [
            ("y = x, z = 0", "連立方程式の曲線"),
            ("y = x = z", "連立方程式の曲線"),
            ("x = y = z = 1", "連立方程式の点"),
            ("x ≤ y", "不等式の領域"),
            ("x^2 + y^2 + z^2 < 4", "不等式の領域"),
            ("x ≤ y ≤ z", "不等式の領域（2 本）"),
            ("0 ≤ z ≤ sin(x) cos(y)", "不等式の領域（2 本）"),
            ("x^2 + y^2 + z^2 = 4, z ≥ 0", "陰関数（不等式で切り取り）"),
            ("z = x^2 + y^2, z ≤ 4", "z = f(x, y)（不等式で切り取り）"),
            ("y = [1, 2] x, z = 0", "連立方程式の曲線 × 2"),
        ] {
            assert_eq!(kind(src), want, "{src}");
        }
        let Ok(Plot::System { eqs, cuts, strict, .. }) = compile("y = x, 0 ≤ x ≤ 1") else { panic!() };
        assert_eq!((eqs.len(), cuts.len(), strict), (1, 2, vec![false, false]));
    }

    /// 不等号の書き方は、どれも同じ G ≤ 0 になる（`<` は厳密の印だけが違う）
    #[test]
    fn relations_are_equivalent() {
        let g = |src: &str| match compile(src) {
            Ok(Plot::System { cuts, strict, .. }) => (cuts.into_iter().next().unwrap(), strict[0]),
            _ => panic!("{src}"),
        };
        let pts: Vec<[f64; 3]> =
            (0..100).map(|k| [(k as f64 * 0.37).sin() * 3.0, (k as f64 * 0.71).cos() * 3.0, k as f64 * 0.05]).collect();
        let vals = |p: &Program| {
            let cols: [Vec<f64>; 3] = [0, 1, 2].map(|c| pts.iter().map(|q| q[c]).collect());
            let mut out = vec![C::new(0.0, 0.0); pts.len()];
            p.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
            out
        };
        let (base, _) = g("x <= y");
        for src in ["x ≤ y", "x ≦ y", "y >= x", "y ≥ x", "x＜=y", "x < y"] {
            let (p, strict) = g(src);
            assert_eq!(vals(&p), vals(&base), "{src}");
            assert_eq!(strict, src == "x < y", "{src}");
        }
    }

    #[test]
    fn system_errors() {
        for src in ["(x = y)", "x <", "x = y,", ", x = y", "a = 1, b = 2", "sin(x ≤ 1)"] {
            assert!(kind(src).starts_with("error"), "{src}: {}", kind(src));
        }
        // 範囲を共有する略記: x, y = 1 は x = 1 かつ y = 1（parser::expand_shared_bounds）
        assert_eq!(kind("x, y = 1"), "連立方程式の曲線");
        assert_eq!(kind("(x = y)"), "error: =・≤ などは、括弧の外に書いてください（4文字目）");
        assert!(kind("x = y,").contains("`,` の前後に式を書いてください"));
        assert!(kind("a = 1, b = 2").contains("パラメータの定義は、1 行に 1 つだけ"));
        let four = Space::basic(4, [0, 1, 2], vec![0.0; 4]);
        let e = compile_with("x^2 = y = z = w = 1", &Params::new(), &four).err().unwrap();
        assert!(e.contains("等式が多すぎます"), "{e}");
        // 全部が 1 次式なら、先に掃き出す（計画 M7）。断面 w = 0 では w = 1 が成り立たないので、解が無い
        let e = compile_with("x = y = z = w = 1", &Params::new(), &four).err().unwrap();
        assert!(e.contains("解がありません"), "{e}");
        // エラーの位置は、式全体での位置
        let e = kind("y = x, z = (");
        assert!(e.ends_with("（13文字目）"), "{e}");
    }

    #[test]
    fn classify() {
        assert_eq!(kind("sin(x) y"), "z = f(x, y)");
        assert_eq!(kind("z = x^2"), "z = f(x, y)");
        assert_eq!(kind("x^2 + y^2 = z"), "z = f(x, y)");
        assert_eq!(kind("y = x z"), "y = f(x, z)");
        assert_eq!(kind("x = cos(y)"), "x = f(y, z)");
        assert_eq!(kind("x^2 + y^2 + z^2 = 1"), "陰関数");
        assert_eq!(kind("z = z^2 + x"), "陰関数");
        assert_eq!(kind("x = y = z"), "連立方程式の曲線");
        assert!(kind("x + z").starts_with("error"));
        assert_eq!(kind("(cos t, sin t, t)"), "媒介変数表示の曲線");
        assert_eq!(kind("(cos t, sin t)"), "媒介変数表示の曲線");
        assert_eq!(kind("(u, v, u v)"), "媒介変数表示の曲面");
        assert_eq!(kind("(1, 2, 3)"), "媒介変数表示の曲線");
        assert_eq!(kind("(u, v, t)"), "媒介変数表示の立体");
        assert_eq!(compile("(u, t, 0)").unwrap().param_vars(), vec!["u", "v", "t"]);
        assert!(kind("(u, t_1, 0)").starts_with("error"));
        assert!(kind("(t, x, 0)").starts_with("error"));
        assert!(kind("(1, 2, 3, 4)").starts_with("error"));
        // 括弧の掛け算は組ではない
        assert_eq!(kind("(x+1)(y-1)"), "z = f(x, y)");
    }

    #[test]
    fn complex_plane() {
        assert_eq!(kind("e^(i t)"), "複素平面の曲線（横 Re f, 奥 Im f）");
        assert_eq!(kind("e^(it)"), "複素平面の曲線（横 Re f, 奥 Im f）");
        assert_eq!(kind("gamma (1+it)"), "複素平面の曲線（横 Re f, 奥 Im f）");
        assert_eq!(kind("(u + i v)^2"), "複素平面の曲面（横 Re f, 奥 Im f）");
        // 座標を使えば今までどおり陽関数
        assert_eq!(kind("e^(i x)"), "z = f(x, y)");
        // t がパラメータとして定義されていれば、t はパラメータ（e^(i t) は1つの複素数なので点、t だけなら平面）
        let mut ps = Params::new();
        ps.insert("t".into(), C::new(2.0, 0.0));
        assert_eq!(compile_with("e^(i t)", &ps, &Space::standard()).unwrap().kind_label(), "複素平面の点（横 Re f, 奥 Im f）");
        assert_eq!(compile_with("2t", &ps, &Space::standard()).unwrap().kind_label(), "z = f(x, y)");
        // 点を計算すると単位円（縦を t にすると、らせん）
        let mut sp = Space::standard();
        sp.lift_param = true;
        let p = compile_with("e^(i t)", &Params::new(), &sp).unwrap();
        assert_eq!(p.kind_label(), "複素平面の曲線（横 Re f, 奥 Im f, 縦 t）");
        let Plot::ComplexPlane { inner, .. } = p else { panic!() };
        let Plot::Curve { sets, .. } = *inner else { panic!() };
        let t = [0.5];
        let at = |k: usize| {
            let mut out = [C::new(0.0, 0.0)];
            sets[0][k].eval_batch(&[&t], &mut out);
            out[0].re
        };
        assert!((at(0) - 0.5f64.cos()).abs() < 1e-12);
        assert!((at(1) - 0.5f64.sin()).abs() < 1e-12);
        assert!((at(2) - 0.5).abs() < 1e-12);
        sp.lift_param = false;
        sp.complex_height = Some(crate::functions::Func::Abs);
        assert_eq!(compile_with("2e^(i t)", &Params::new(), &sp).unwrap().kind_label(), "複素平面の曲線（横 Re f, 奥 Im f, 縦 |f|）");
        // リストも使える
        assert!(kind("[1, 2] e^(i t)").starts_with("複素平面の曲線"));
    }

    #[test]
    fn complex_point() {
        assert_eq!(kind("1 + 2i"), "複素平面の点（横 Re f, 奥 Im f）");
        assert_eq!(kind("[1, 2] + i"), "複素平面の点（横 Re f, 奥 Im f）");
        // 実数の定数は今までどおり平面
        assert_eq!(kind("2"), "z = f(x, y)");
        let mut ps = Params::new();
        ps.insert("a".into(), C::new(2.0, 0.0));
        ps.insert("c".into(), C::new(1.0, 1.0));
        let sp = Space::standard();
        assert_eq!(compile_with("a", &ps, &sp).unwrap().kind_label(), "z = f(x, y)");
        assert_eq!(compile_with("c^2", &ps, &sp).unwrap().kind_label(), "複素平面の点（横 Re f, 奥 Im f）");
        // 複素数のスライダーなら、値がたまたま実数でも点のまま
        let mut sp = Space::standard();
        sp.complex_params = vec!["a".into()];
        assert_eq!(compile_with("a", &ps, &sp).unwrap().kind_label(), "複素平面の点（横 Re f, 奥 Im f）");
        // 点の位置
        let p = compile_with("c^2", &ps, &Space::standard()).unwrap();
        let Plot::ComplexPlane { inner, .. } = p else { panic!() };
        let Plot::Curve { sets, .. } = *inner else { panic!() };
        let at = |k: usize| {
            let mut out = [C::new(0.0, 0.0)];
            sets[0][k].eval_batch(&[&[0.0]], &mut out);
            out[0].re
        };
        assert!(at(0).abs() < 1e-12 && (at(1) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn params() {
        assert_eq!(kind("a = 2"), "パラメータ");
        assert_eq!(kind("k = 2pi + i"), "パラメータ");
        assert_eq!(kind("a = x + 1"), "error: 変数 'a' はここでは使えません（使える変数: x, y, z）");
        // t, u, v も、組の外ではパラメータとして使える
        assert_eq!(kind("t = 1"), "パラメータ");
        let mut ps = Params::new();
        ps.insert("a".into(), C::new(3.0, 0.0));
        ps.insert("t".into(), C::new(2.0, 0.0));
        let sp = Space::standard();
        // a を使う式
        let p = compile_with("z = a x", &ps, &sp).unwrap();
        assert_eq!(p.used_params(), vec!["a".to_string()]);
        // 組の外の t はパラメータ、組の中の t は媒介変数
        assert_eq!(compile_with("z = t x", &ps, &sp).unwrap().used_params(), vec!["t".to_string()]);
        assert!(compile_with("(t, t, t)", &ps, &sp).unwrap().used_params().is_empty());
        // ほかのパラメータを使うパラメータ
        match compile_with("b = 2a + i", &ps, &sp).unwrap() {
            Plot::Param { name, value, uses } => {
                assert_eq!(name, "b");
                assert_eq!(value, C::new(6.0, 1.0));
                assert_eq!(uses, vec!["a".to_string()]);
            }
            _ => panic!(),
        }
        // 自分自身は使えない
        assert!(compile_with("a = a + 1", &ps, &sp).is_err());
    }

    #[test]
    fn names() {
        assert_eq!(coord_name(3), "w");
        assert_eq!(coord_name(4), "w2");
        assert_eq!(coord_index("w1"), Some(3));
        assert_eq!(coord_index("w2"), Some(4));
        assert_eq!(coord_index("a"), None);
        assert_eq!(coord_index("w0"), None);
        assert!(is_param_var("t3") && is_param_var("u") && !is_param_var("w2") && !is_param_var("ta"));
    }

    #[test]
    fn four_dims() {
        let ps = Params::new();
        // 3次元のときに w を使うとエラー
        assert!(kind("x + w = 1").contains("次元を 4 以上"));
        // 4次元の超球 x² + y² + z² + w² = 4 の w = 1 での断面は、陰関数（半径 √3 の球）
        let sp = space4([0, 1, 2], 1.0);
        let p = compile_with("x^2 + y^2 + z^2 + w^2 = 4", &ps, &sp).unwrap();
        assert_eq!(p.kind_label(), "陰関数");
        assert_eq!(p.used_params(), vec!["w".to_string()]); // w の断面の値を使っている
        // 縦の軸に w を表示すると、w = f(x, y) は陽関数
        let sp = space4([0, 1, 3], 0.0);
        assert_eq!(compile_with("w = x y", &ps, &sp).unwrap().kind_label(), "w = f(x, y)");
        // = が無い式は「縦の軸の座標 = 式」
        assert_eq!(compile_with("x + z", &ps, &sp).unwrap().kind_label(), "w = f(x, y)");
        // 表示していない座標 w = 1 だけの式は描けない
        assert!(compile_with("w = 1", &ps, &space4([0, 1, 2], 0.0)).is_err());
        // 4次元の媒介変数表示
        let sp = space4([0, 1, 2], 0.0);
        assert_eq!(
            compile_with("(cos u, sin u, cos v, sin v)", &ps, &sp).unwrap().kind_label(),
            "媒介変数表示の曲面"
        );
        assert_eq!(compile_with("(t_1, t_2, 0, t_1 t_2)", &ps, &sp).unwrap().param_vars(), vec!["t1", "t2"]);
        assert_eq!(compile_with("(t_1, t_2, t_3, 0)", &ps, &sp).unwrap().kind_label(), "媒介変数表示の立体");
        // 媒介変数が4つ以上でも描ける（面をたくさん描いて表す）
        let p = compile_with("(t_1, t_2, t_3, t_4)", &ps, &sp).unwrap();
        assert_eq!(p.kind_label(), "媒介変数表示の多変数の図形（4 変数）");
        assert_eq!(p.param_vars(), vec!["t1", "t2", "t3", "t4"]);
        // 番号は飛んでいてもよい（小さい順に並べる）。同じ媒介変数を何度使っても1つ
        assert_eq!(compile_with("(t_7 t_2, t_2, t_10 + t_7, t_4)", &ps, &sp).unwrap().param_vars(), vec!["t2", "t4", "t7", "t10"]);
        let many = "(t_1+t_2+t_3+t_4+t_5, t_6+t_7+t_8+t_9, 0, 0)";
        assert!(compile_with(many, &ps, &sp).err().unwrap().contains("8 個まで"));
        assert!(compile_with("(1, 2, 3, 4, 5)", &ps, &sp).err().unwrap().contains("次元を 5"));
        // 5次元目は w₂
        let sp5 = Space::basic(5, [0, 3, 4], vec![0.5; 5]);
        assert_eq!(compile_with("w_2 = x + w", &ps, &sp5).unwrap().kind_label(), "w2 = f(x, w)");
    }

    #[test]
    fn ranges_make_solids() {
        let ps = Params::new();
        // w を範囲で重ねる: 超球の断面（球）を重ねた立体
        let mut sp = space4([0, 1, 2], 0.0);
        sp.ranges[3] = Some((-1.0, 1.0));
        let p = compile_with("x^2 + y^2 + z^2 + w^2 = 4", &ps, &sp).unwrap();
        assert_eq!(p.kind_label(), "範囲で重ねた立体");
        // w を使わない式は、ふつうの曲面のまま
        assert_eq!(compile_with("z = x", &ps, &sp).unwrap().kind_label(), "z = f(x, y)");
        // パラメータを範囲で重ねる: 陽関数 → 立体、曲線 → 曲面
        let mut sp = Space::standard();
        sp.param_ranges.push(("a".into(), 0.0, 2.0));
        assert_eq!(compile_with("z = a x^2", &ps, &sp).unwrap().kind_label(), "範囲で重ねた立体");
        let p = compile_with("(cos t, sin t, a)", &ps, &sp).unwrap();
        assert_eq!(p.kind_label(), "媒介変数表示の曲面");
        assert_eq!(p.param_vars(), vec!["t", "a"]);
        // 曲面を範囲で重ねると立体
        let p = compile_with("(u, v, a)", &ps, &sp).unwrap();
        assert_eq!(p.kind_label(), "媒介変数表示の立体");
        assert_eq!(p.param_vars(), vec!["u", "v", "a"]);
        // 立体をさらに範囲で重ねると、多変数の図形
        sp.param_ranges.push(("b".into(), 0.0, 1.0));
        let p = compile_with("(u, v, a b)", &ps, &sp).unwrap();
        assert_eq!(p.param_vars(), vec!["u", "v", "a", "b"]);
        assert!(matches!(p, Plot::Sheets { .. }));
    }

    #[test]
    fn rotation() {
        use crate::eval::Program;
        let ps = Params::new();
        // xy 平面で 90° 回すと、点 (1, 0, 0) は (0, 1, 0) に移る
        let mut sp = Space::standard();
        sp.rotation[0] = std::f64::consts::FRAC_PI_2;
        let m = sp.rotation_matrix().unwrap();
        assert!((m[0][0]).abs() < 1e-12 && (m[1][0] - 1.0).abs() < 1e-12);
        // 平面 x − 1 = 0 は、回すと平面 y = 1 になる（陽関数の形でないので、陰関数として式に代入して回す）
        let p = compile_with("x - 1 = 0", &ps, &sp).unwrap();
        let prog: &Program = match &p {
            Plot::Implicit { prog, .. } => prog,
            _ => panic!("回転すると陰関数になる"),
        };
        let mut out = [C::new(0.0, 0.0); 2];
        // (5, 1, 0) は y = 1 の上、(1, 5, 0) は上にない
        prog.eval_batch(&[&[5.0, 1.0], &[1.0, 5.0], &[0.0, 0.0]], &mut out);
        assert!(out[0].norm() < 1e-12 && out[1].norm() > 1.0);
        // 中心 (1, 0, 0) の周りで 90° 回すと、平面 x = 1 は中心を通るので y = 0 になる
        sp.center = vec![1.0, 0.0, 0.0, 0.0, 0.0];
        let p = compile_with("x - 1 = 0", &ps, &sp).unwrap();
        if let Plot::Implicit { prog, .. } = &p {
            prog.eval_batch(&[&[7.0, 0.0], &[0.0, 0.0], &[0.0, 0.0]], &mut out);
            assert!(out[0].norm() < 1e-12);
        }
        // 媒介変数表示も回る: 点 (2, 0, 0) を xy で 90° 回すと (0, 2, 0)
        let mut sp = Space::standard();
        sp.rotation[0] = std::f64::consts::FRAC_PI_2;
        let view = |sp: &Space, p: &[f64]| sp.view_rows().map(|(c0, cs)| c0 + cs.iter().zip(p).map(|(c, x)| c * x).sum::<f64>());
        let q = view(&sp, &[2.0, 0.0, 0.0]);
        assert!(q[0].abs() < 1e-12 && (q[1] - 2.0).abs() < 1e-12);
        // 次元が5未満でも、回転は5次元（5C2 = 10 通り）。6次元なら 6C2 = 15 通り
        assert_eq!(Space::basic(4, [0, 1, 2], vec![0.0; 4]).rotation.len(), 10);
        assert_eq!(Space::basic(6, [0, 1, 2], vec![0.0; 6]).rotation.len(), 15);
        // 3次元でも xw 平面で回せる。点 (2, 0, 0) を xw で 60° 回すと、x 成分は 2·cos60° = 1 に縮む
        let mut sp = Space::standard();
        sp.rotation[2] = std::f64::consts::FRAC_PI_3; // 組 (0,3) = xw は3番目
        assert!((view(&sp, &[2.0, 0.0, 0.0])[0] - 1.0).abs() < 1e-12);
        // x = 1 は w 方向に伸びた形なので、xw で 60° 回して w = 0 で見ると x = 1/cos60° = 2。
        // 傾いた座標系の陽関数になる（高さの向き e₃ が x 軸）
        if let Plot::Tilted { prog, frame, .. } = compile_with("x = 1", &ps, &sp).unwrap() {
            let mut o = [C::new(0.0, 0.0)];
            prog.eval_batch(&[&[0.0], &[0.0]], &mut o);
            assert!((frame[2][0].abs() - 1.0).abs() < 1e-12);
            assert!((o[0].re * frame[2][0] - 2.0).abs() < 1e-12);
        } else {
            panic!()
        }
        // 2D の切り口では、陰関数として計算し直す
        sp.flat_slice = true;
        if let Plot::Implicit { prog, .. } = compile_with("x = 1", &ps, &sp).unwrap() {
            let mut o = [C::new(0.0, 0.0)];
            prog.eval_batch(&[&[2.0], &[0.0], &[0.0]], &mut o);
            assert!(o[0].norm() < 1e-12);
        } else {
            panic!()
        }
    }

    #[test]
    fn explicit_rotation_points() {
        use crate::mesh::{explicit_nd, project, Domain};
        // z = 1（高さ 1 の平面）を yz で 90° 回すと、y = −1 の平面になる
        let ps = Params::new();
        let mut sp = Space::standard();
        sp.rotation[4] = std::f64::consts::FRAC_PI_2; // 組 (1,2) = yz
        let Plot::Explicit { axis, prog, rotated: true, .. } = compile_with("z = 1", &ps, &sp).unwrap() else {
            panic!("陽関数のまま回す")
        };
        let dom = Domain { lo: [-3.0; 3], hi: [3.0; 3] };
        let shape = explicit_nd(&prog, axis, sp.axes, &sp.slice, dom, 6, crate::mesh::ComplexMode::Off, false, false, 6.0);
        let m = project(&shape, &sp.view(0.0), Domain { lo: [-3.0; 3], hi: [3.0; 3] });
        assert!(!m.positions.is_empty());
        for q in m.positions.chunks_exact(3) {
            assert!((q[1] + 1.0).abs() < 1e-6, "y = {}", q[1]);
        }
    }

    #[test]
    fn rotated_explicit_keeps_jumps() {
        use crate::mesh::{explicit_nd, project, Domain};
        // 階段 z = floor(x) を xy で回しても（z は変わらない）、段差をまたぐ三角形を作らない
        let ps = Params::new();
        let mut sp = Space::standard();
        sp.rotation[0] = 0.4; // 組 (0,1) = xy
        let Plot::Explicit { axis, prog, rotated: true, .. } = compile_with("z = floor(x)", &ps, &sp).unwrap() else {
            panic!("陽関数のまま回す")
        };
        let dom = Domain { lo: [-6.0; 3], hi: [6.0; 3] };
        let shape = explicit_nd(&prog, axis, sp.axes, &sp.slice, dom, 40, crate::mesh::ComplexMode::Off, false, true, 6.0);
        assert!(shape.keep_full);
        let m = project(&shape, &sp.view(0.0), Domain { lo: [-3.0; 3], hi: [3.0; 3] });
        assert!(m.keep_full);
        assert!(!m.indices.is_empty());
        for t in m.indices.chunks_exact(3) {
            let z = [t[0], t[1], t[2]].map(|k| m.positions[k as usize * 3 + 2]);
            let spread = z.iter().cloned().fold(f32::MIN, f32::max) - z.iter().cloned().fold(f32::MAX, f32::min);
            assert!(spread < 0.5, "段差をまたぐ三角形: {z:?}");
        }
    }

    #[test]
    fn rotated_explicit_flat_slice_is_implicit() {
        // 2D の切り口では、回した z = f(x, y) を陰関数として計算し直す（0 に接するだけの所も見つけられる）
        let ps = Params::new();
        let mut sp = Space::standard();
        sp.rotation[0] = 0.4; // xy
        sp.flat_slice = true;
        let Plot::Implicit { prog, from: Some(_), .. } = compile_with("z = x^2 + y^2", &ps, &sp).unwrap() else {
            panic!("陰関数として計算し直す")
        };
        let d = crate::mesh::Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
        let m = crate::mesh::implicit_curve_2d(&prog, d, 320);
        // 原点の1点だけ
        assert_eq!(m.positions.len(), 9, "{:?}", m.positions);
        assert!(m.positions[0].abs() < 1e-3 && m.positions[1].abs() < 1e-3);
        // 回した階段: 線は段の縁（回す前の座標 x' か y' が整数）の上にきっちり乗る（格子に対して斜めでも、ぎざぎざにならない）
        let Plot::Implicit { prog, .. } = compile_with("z = floor(x)/2 + floor(y)/4", &ps, &sp).unwrap() else { panic!() };
        let m = crate::mesh::implicit_curve_2d(&prog, d, 320);
        assert!(m.positions.len() > 100);
        let (c, s) = (0.4f64.cos(), 0.4f64.sin());
        for q in m.positions.chunks_exact(3).filter(|q| q[0].is_finite()) {
            let (x, y) = (q[0] as f64, q[1] as f64);
            // 回す前の座標（xy で 0.4 回したものを戻す）
            let (u, v) = (c * x + s * y, -s * x + c * y);
            let off = |a: f64| (a - a.round()).abs();
            assert!(off(u).min(off(v)) < 2e-3, "段の縁から外れた点 ({x}, {y}) → ({u}, {v})");
        }
        // y = f(x)（縦の座標を使わない）は、今までどおり陽関数のまま回す
        assert!(matches!(compile_with("y = x^2", &ps, &sp).unwrap(), Plot::Explicit { rotated: true, .. }));
        // 切り口でなければ、陽関数のまま回す
        sp.flat_slice = false;
        assert!(matches!(compile_with("z = x^2 + y^2", &ps, &sp).unwrap(), Plot::Explicit { rotated: true, .. }));
    }

    #[test]
    fn explicit_rotation_as_surface() {
        let ps = Params::new();
        let mut sp = Space::standard();
        // xz で回すだけなら、陽関数のまま（点を計算してから回す）
        sp.rotation[1] = 0.3;
        assert!(matches!(compile_with("z = x y", &ps, &sp).unwrap(), Plot::Explicit { rotated: true, .. }));
        // xw（表示していない w の方向）が絡むと、傾いた座標系の陽関数になる（2D の切り口では陰関数）
        sp.rotation[2] = 0.3;
        assert!(matches!(compile_with("z = x y", &ps, &sp).unwrap(), Plot::Tilted { .. }));
        sp.flat_slice = true;
        assert!(matches!(compile_with("z = x y", &ps, &sp).unwrap(), Plot::Implicit { .. }));
        // 式が使っていない座標どうし（3次元のときの w と w₂）の回転は、影響しない
        let mut sp = Space::standard();
        sp.rotation[9] = 0.5; // 組 (3,4) = ww₂
        assert!(sp.rotation_keeps_shown(&["x".into(), "y".into(), "z".into()]));
        // 4次元で、断面の値として w を使う式に zw の回転をかけると、陰関数
        let mut sp = space4([0, 1, 2], 1.0);
        sp.rotation[7] = 0.2; // 組 (2,3) = zw
        assert!(!sp.rotation_keeps_shown(&["z".into(), "w".into()]));
    }

    #[test]
    fn rotated_complex_explicit() {
        // xw で回すと、傾いた座標系の陽関数になる。複素数の色分けをしているなら、高さは |f| で、色は f
        let ps = Params::new();
        let mut sp = space4([0, 1, 2], 0.0);
        sp.rotation[2] = 0.3; // xw
        sp.complex_height = Some(crate::functions::Func::Abs);
        let p = compile_with("z = (x + i y)^2", &ps, &sp).unwrap();
        assert_eq!(p.kind_label(), "z = f(x, y)（回したもの）");
        let Plot::Tilted { prog, color: Some(color), .. } = p else { panic!() };
        let mut out = [C::new(0.0, 0.0); 3];
        let (us, vs) = ([0.3, -0.7, 1.1], [0.5, 0.2, -0.4]);
        prog.eval_batch(&[&us, &vs], &mut out);
        assert!(out.iter().all(|z| z.im.abs() < 1e-12));
        color.eval_batch(&[&us, &vs], &mut out);
        assert!(out.iter().any(|z| z.im.abs() > 1e-3));
        // 2D の切り口では陰関数として計算し直す。f の代わりに |f| を使うので、
        // F = z − |(x + iy)²| は実数になる（f のままだと複素数になり、曲面が描けない）
        sp.flat_slice = true;
        let p = compile_with("z = (x + i y)^2", &ps, &sp).unwrap();
        assert_eq!(p.kind_label(), "陰関数（z = f(x, y) を回したもの）");
        let Plot::Implicit { prog, .. } = p else { panic!() };
        let mut out = [C::new(0.0, 0.0); 3];
        prog.eval_batch(&[&[0.3, -0.7, 1.1], &[0.5, 0.2, -0.4], &[0.1, 0.9, 0.0]], &mut out);
        assert!(out.iter().all(|z| z.im.abs() < 1e-12));
    }

    #[test]
    fn lists() {
        let ps = Params::new();
        let sp = space4([0, 1, 2], 0.0);
        // 正八胞体の x 方向の辺 8 本
        match compile_with("(t, [-1, 1], [-1, 1], [-1, 1])", &ps, &sp).unwrap() {
            Plot::Curve { sets, .. } => assert_eq!(sets.len(), 8),
            _ => panic!(),
        }
        // 正八胞体の xy 方向の面 4 枚
        match compile_with("(u, v, [-1, 1], [-1, 1])", &ps, &sp).unwrap() {
            Plot::Surface { sets, .. } => assert_eq!(sets.len(), 4),
            _ => panic!(),
        }
        // 式の中のリストも展開される: (t, 2[1, 3]) → (t, 2), (t, 6)
        let Plot::Curve { sets, .. } = compile_with("(t, 2[1, 3], 0)", &ps, &Space::standard()).unwrap() else {
            panic!()
        };
        let mut o = [C::new(0.0, 0.0)];
        sets[1][1].eval_batch(&[&[0.0]], &mut o);
        assert_eq!(o[0], C::new(6.0, 0.0));
        // 陽関数・陰関数でも使える: 要素の数だけ図形ができる
        match compile("z = [1, 2] x").unwrap() {
            Plot::Multi(ps) => {
                assert_eq!(ps.len(), 2);
                assert!(ps.iter().all(|p| p.kind_label() == "z = f(x, y)"));
            }
            _ => panic!(),
        }
        let p = compile("x^2 + y^2 + z^2 = [1, 4, 9]").unwrap();
        assert_eq!(p.kind_label(), "陰関数 × 3");
        // 2つのリストは組み合わせ（2 × 2 = 4 個）
        assert_eq!(compile("z = [1, 2] x + [0, 1]").unwrap().kind_label(), "z = f(x, y) × 4");
        // パラメータの値には使えない
        assert!(compile("a = [1, 2]").is_err());
        assert!(compile_with("(t, [[1, 2], 3], 0)", &ps, &Space::standard()).is_err());
    }

    #[test]
    fn constants_with_params() {
        let mut ps = Params::new();
        ps.insert("b".into(), C::new(3.0, 0.0));
        assert_eq!(eval_constant_with("2b", &ps).unwrap(), (6.0, vec!["b".to_string()]));
        assert_eq!(
            eval_constant_with("2c", &ps).unwrap_err(),
            "変数 'c' はここでは使えません（パラメータとして定義してください）"
        );
    }

    #[test]
    fn constants() {
        assert!((eval_constant("2pi").unwrap() - std::f64::consts::TAU).abs() < 1e-15);
        assert_eq!(eval_constant("-3").unwrap(), -3.0);
        assert!(eval_constant("x").is_err());
        assert!(eval_constant("sqrt(-1)").is_err());
    }

    #[test]
    fn error_position_on_right_side() {
        // 右辺の誤りの位置が、式全体での位置になっているか
        let e = match compile("z = x +") {
            Err(e) => e,
            Ok(_) => panic!(),
        };
        assert!(e.contains("8文字目"), "{e}");
    }
    #[test]
    fn gpu_implicit_matches_rotated_field() {
        // 4 次元・表示していない w が絡む回転・中心のずれ・w の断面 0.5・パラメータ a で、
        // GPU の F（元の座標の式）を P = A q + b で計算した値が、回転を入れた F（表示中の座標の式）と同じ
        let mut sp = Space::basic(4, [0, 1, 2], vec![0.0, 0.0, 0.0, 0.5]);
        sp.rotation = vec![0.3, 0.2, 0.7, 0.0, -0.4, 0.0, 0.0, 0.5, 0.0, 0.0];
        sp.center = vec![0.2, -0.1, 0.0, 0.3, 0.0];
        let mut ps = Params::new();
        ps.insert("a".into(), C::new(1.3, 0.0));
        let Plot::Implicit { prog, gpu: Some(g), .. } = compile_with("x^2 + a y^2 + z w + w^3 = 4", &ps, &sp).unwrap() else {
            panic!("陰関数で、GPU の F がある")
        };
        assert_eq!((g.dims, g.params.clone()), (4, vec!["a".to_string()]));
        let aff = sp.gpu_affine();
        assert_eq!(aff.len(), 16);
        let val = |p: &crate::eval::Program, xs: &[f64]| {
            let cols: Vec<[f64; 1]> = xs.iter().map(|&v| [v]).collect();
            let refs: Vec<&[f64]> = cols.iter().map(|c| &c[..]).collect();
            let mut out = [C::new(0.0, 0.0)];
            p.eval_batch(&refs, &mut out);
            out[0].re
        };
        for q in [[0.3, -1.2, 0.8], [2.0, 0.5, -1.5], [-0.7, 0.1, 0.0]] {
            let want = val(&prog, &q);
            let mut pp: Vec<f64> = (0..4).map(|k| aff[4 * k] * q[0] + aff[4 * k + 1] * q[1] + aff[4 * k + 2] * q[2] + aff[4 * k + 3]).collect();
            pp.push(1.3);
            let got = val(&g.prog, &pp);
            assert!((got - want).abs() < 1e-9 * (1.0 + want.abs()), "q = {q:?}: {got} と {want}");
        }
    }
}

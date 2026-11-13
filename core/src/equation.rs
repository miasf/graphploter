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
use crate::parser::{parse, BinOp, Expr, ParseError};
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

    /// 表示中の座標の名前（横・奥・縦の順）
    fn shown(&self) -> [String; 3] {
        self.axes.map(coord_name)
    }

    /// 表示していない座標を、断面の値の定数としてパラメータに足す（範囲で重ねるものは除く）。
    /// 次元より先の、回転のときだけ使う座標（3次元のときの w, w₂ など）の値は 0
    fn with_hidden(&self, params: &Params) -> Params {
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
fn is_param_var(name: &str) -> bool {
    matches!(name, "t" | "u" | "v")
        || name.strip_prefix('t').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

/// コンパイル済みのグラフ
pub enum Plot {
    /// 陽関数。表示中の `axis` 番目の座標（0: 横, 1: 奥, 2: 縦）が、残り2つの関数になっている。
    /// `prog` の変数は、残り2つの表示中の座標（軸の順）。
    /// `rotated` が true なら、表示中の座標どうしの回転だけをしているので、
    /// 媒介変数表示 (u, v, f(u, v)) と同じように点を計算してから回転・射影する（[`Space::view_rows`]）。
    Explicit { axis: usize, names: [String; 3], prog: Program, rotated: bool },
    /// 陰関数 F = 0。`prog` の変数は表示中の3つの座標。
    /// `from` は、陽関数（z = f(x, y) など）を回して陰関数として計算し直したときの、元の陽関数の形
    /// `local` は、回転が表示中の座標どうしだけのときの、回す前の F（変数は表示中の 3 つの座標）。
    /// これでメッシュを作って回せば、角度が変わっても計算し直さずに済む（lib.rs の build）
    Implicit { prog: Program, from: Option<String>, local: Option<Program> },
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
    /// パラメータの定義 `name = value`。`uses` は右辺で使われたほかのパラメータ。
    Param { name: String, value: C, uses: Vec<String> },
    /// リストを含む陽関数・陰関数（例: `z = [1, 2, 3] x`）。リストの要素ごとの図形（組み合わせの数だけ）
    Multi(Vec<Plot>),
    /// 複素平面の図形（[`complex_plane`]）。中身は媒介変数表示 (Re f, Im f, 縦の値) で、`label` は画面に出す説明
    ComplexPlane { inner: Box<Plot>, label: String },
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
            Plot::Multi(ps) => format!("{} × {}", ps.first().map_or(String::new(), |p| p.kind_label()), ps.len()),
            Plot::ComplexPlane { label, .. } => label.clone(),
        }
    }

    /// 媒介変数の名前（媒介変数表示でなければ空）
    pub fn param_vars(&self) -> Vec<String> {
        match self {
            Plot::Curve { vars, .. }
            | Plot::Surface { vars, .. }
            | Plot::Volume { vars, .. }
            | Plot::Sheets { vars, .. } => vars.clone(),
            Plot::Multi(ps) => ps.first().map_or(Vec::new(), |p| p.param_vars()),
            // 点（媒介変数を使わない）なら、範囲の欄を出さない
            Plot::ComplexPlane { label, .. } if label.starts_with("複素平面の点") => Vec::new(),
            Plot::ComplexPlane { inner, .. } => inner.param_vars(),
            _ => Vec::new(),
        }
    }

    /// 使われたパラメータ（と、断面で値を固定した座標）の名前
    pub fn used_params(&self) -> Vec<String> {
        let progs: Vec<&Program> = match self {
            Plot::Explicit { prog, .. } | Plot::Implicit { prog, .. } => vec![prog],
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
            Plot::ComplexPlane { inner, .. } => return inner.used_params(),
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
fn parse_at(src: &str, offset: usize) -> Result<Expr, String> {
    parse(src).map_err(|e| ParseError { pos: e.pos + offset, ..e }.to_string())
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
    let shown = space.shown();

    let parts: Vec<&str> = source.split('=').collect();
    let (l, r) = match parts.as_slice() {
        [rhs] => {
            let e = parse_at(rhs, 0)?;
            check_dims(&e, space)?;
            if let Expr::Tuple(items) = e {
                return parametric(items, params, space);
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
        [lhs, rhs] => {
            let l = parse_at(lhs, 0)?;
            let r = parse_at(rhs, lhs.chars().count() + 1)?;
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
        _ => return Err("= は1つだけにしてください".into()),
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

/// 左辺 = 右辺（リストを含まない）の式をコンパイルする（陽関数・陰関数・範囲で重ねた立体）
fn compile_pair(l: Expr, r: Expr, params: &Params, space: &Space) -> Result<Plot, String> {
    let shown = space.shown();
    let shown_refs: [&str; 3] = [&shown[0], &shown[1], &shown[2]];
    let consts = space.with_hidden(params);
    let rot = space.rotation_matrix();

    // 陽関数で、回転が表示中の座標どうしだけなら、曲面をそのまま回せばよい（媒介変数表示と同じ扱い）
    if rot.is_some() && !space.ranged().iter().any(|(n, _, _)| l.uses_var(n) || r.uses_var(n)) {
        let mut used = Vec::new();
        l.collect_vars(&mut used);
        r.collect_vars(&mut used);
        if space.rotation_keeps_shown(&used) {
            if let Some(p) = find_explicit(&l, &r, &shown, &consts)? {
                // 2D の切り口で縦の座標を使う陽関数は、陰関数として計算し直す（Space::flat_slice を参照）
                let vertical = matches!(&p, Plot::Explicit { axis, prog, .. } if *axis == 2 || prog.uses_var_index(1));
                if !(space.flat_slice && vertical) {
                    return Ok(match p {
                        Plot::Explicit { axis, names, prog, .. } => Plot::Explicit { axis, names, prog, rotated: true },
                        other => other,
                    });
                }
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
    // 陽関数は上の道で描く。2D の切り口・範囲で重ねる名前を使う式は、今までどおり回した F だけ
    let local = if rot.is_some() && !space.flat_slice && find_explicit(&l, &r, &shown, &consts)?.is_none() {
        let mut used = Vec::new();
        l.collect_vars(&mut used);
        r.collect_vars(&mut used);
        let ranged = space.ranged().iter().any(|(n, _, _)| l.uses_var(n) || r.uses_var(n));
        if !ranged && space.rotation_keeps_shown(&used) && shown.iter().any(|v| l.uses_var(v) || r.uses_var(v)) {
            let f = Expr::Bin(BinOp::Sub, Box::new(l.clone()), Box::new(r.clone()));
            Some(Program::compile_with(&f, &shown_refs, &consts)?)
        } else {
            None
        }
    } else {
        None
    };

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
    Ok(Plot::Implicit { prog: Program::compile_with(&f, &shown_refs, &consts)?, from, local })
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
    let e = parse_at(source, 0)?;
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

fn explicit(axis: usize, e: &Expr, shown: &[String; 3], consts: &Params) -> Result<Plot, String> {
    let (a, b) = others(axis);
    let prog = Program::compile_with(e, &[&shown[a], &shown[b]], consts)?;
    Ok(Plot::Explicit { axis, names: shown.clone(), prog, rotated: false })
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

    #[test]
    fn classify() {
        assert_eq!(kind("sin(x) y"), "z = f(x, y)");
        assert_eq!(kind("z = x^2"), "z = f(x, y)");
        assert_eq!(kind("x^2 + y^2 = z"), "z = f(x, y)");
        assert_eq!(kind("y = x z"), "y = f(x, z)");
        assert_eq!(kind("x = cos(y)"), "x = f(y, z)");
        assert_eq!(kind("x^2 + y^2 + z^2 = 1"), "陰関数");
        assert_eq!(kind("z = z^2 + x"), "陰関数");
        assert!(kind("x = y = z").starts_with("error"));
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
}

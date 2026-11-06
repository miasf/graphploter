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
//!   （媒介変数は t / u, v / t₁, t₂。成分が次元より少なければ残りは 0）
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
    /// 回転角（ラジアン）。座標の組 (0,1), (0,2), …, (0,n−1), (1,2), … の順に nC2 個
    pub rotation: Vec<f64>,
    /// 回転の中心
    pub center: Vec<f64>,
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
            rotation: vec![0.0; dims * (dims - 1) / 2],
            center: vec![0.0; dims],
        }
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
    fn rotation_matrix(&self) -> Option<Vec<Vec<f64>>> {
        if self.rotation.iter().all(|&a| a == 0.0) {
            return None;
        }
        let n = self.dims;
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

    /// 表示していない座標を、断面の値の定数としてパラメータに足す（範囲で重ねるものは除く）
    fn with_hidden(&self, params: &Params) -> Params {
        let mut ps = params.clone();
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

/// 媒介変数として使える名前か（t, u, v, t1, t2, …）
fn is_param_var(name: &str) -> bool {
    matches!(name, "t" | "u" | "v")
        || name.strip_prefix('t').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
}

/// コンパイル済みのグラフ
pub enum Plot {
    /// 陽関数。表示中の `axis` 番目の座標（0: 横, 1: 奥, 2: 縦）が、残り2つの関数になっている。
    /// `prog` の変数は、残り2つの表示中の座標（軸の順）。
    Explicit { axis: usize, names: [String; 3], prog: Program },
    /// 陰関数 F = 0。`prog` の変数は表示中の3つの座標。
    Implicit { prog: Program },
    /// 範囲で重ねた立体。`prog` の変数は表示中の3つの座標と、範囲で動かす名前（`ranges` の順）。
    Solid { prog: Program, ranges: Vec<(f64, f64)>, names: Vec<String> },
    /// 媒介変数表示の曲線。`progs` は表示中の3つの座標の成分で、変数は `vars`（1つ）。
    Curve { progs: [Program; 3], vars: Vec<String> },
    /// 媒介変数表示の曲面。`progs` は表示中の3つの座標の成分で、変数は `vars`（2つ）。
    Surface { progs: [Program; 3], vars: Vec<String> },
    /// パラメータの定義 `name = value`。`uses` は右辺で使われたほかのパラメータ。
    Param { name: String, value: C, uses: Vec<String> },
}

impl Plot {
    /// 画面に出す説明
    pub fn kind_label(&self) -> String {
        match self {
            Plot::Explicit { axis, names, .. } => {
                let (a, b) = others(*axis);
                format!("{} = f({}, {})", names[*axis], names[a], names[b])
            }
            Plot::Implicit { .. } => "陰関数".into(),
            Plot::Solid { .. } => "範囲で重ねた立体".into(),
            Plot::Curve { .. } => "媒介変数表示の曲線".into(),
            Plot::Surface { .. } => "媒介変数表示の曲面".into(),
            Plot::Param { .. } => "パラメータ".into(),
        }
    }

    /// 媒介変数の名前（媒介変数表示でなければ空）
    pub fn param_vars(&self) -> Vec<String> {
        match self {
            Plot::Curve { vars, .. } | Plot::Surface { vars, .. } => vars.clone(),
            _ => Vec::new(),
        }
    }

    /// 使われたパラメータ（と、断面で値を固定した座標）の名前
    pub fn used_params(&self) -> Vec<String> {
        let progs: Vec<&Program> = match self {
            Plot::Explicit { prog, .. } | Plot::Implicit { prog } => vec![prog],
            Plot::Solid { prog, names, .. } => {
                // 範囲で重ねている名前も「使ったもの」に含める（範囲が変わったら計算し直すため）
                let mut out = names.clone();
                out.extend(prog.used_params().iter().filter(|n| !names.contains(n)).cloned());
                return out;
            }
            Plot::Curve { progs, .. } | Plot::Surface { progs, .. } => progs.iter().collect(),
            Plot::Param { uses, .. } => return uses.clone(),
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
    let shown_refs: [&str; 3] = [&shown[0], &shown[1], &shown[2]];
    let consts = space.with_hidden(params);
    let rot = space.rotation_matrix();

    let parts: Vec<&str> = source.split('=').collect();
    let (l, r) = match parts.as_slice() {
        [rhs] => {
            let e = parse_at(rhs, 0)?;
            check_dims(&e, space)?;
            if let Expr::Tuple(items) = e {
                return parametric(items, params, space);
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
    // 左辺を優先して調べる（z = x なら「z = f(x, y)」）
    for (one, other) in [(&l, &r), (&r, &l)] {
        for (axis, name) in shown.iter().enumerate() {
            if matches!(one, Expr::Var(v) if v == name) && !other.uses_var(name) {
                return explicit(axis, other, &shown, &consts);
            }
        }
    }
    if !shown.iter().any(|v| l.uses_var(v) || r.uses_var(v)) {
        return Err(format!("表示中の座標（{}）のどれかを使った式にしてください", shown.join(", ")));
    }
    let f = Expr::Bin(BinOp::Sub, Box::new(l), Box::new(r));
    Ok(Plot::Implicit { prog: Program::compile_with(&f, &shown_refs, &consts)? })
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

/// 式の中の座標 p_k を、回転後の座標 q で表した p_k = c_k + Σ_j R[j][k] (q_j − c_j) に置き換える
fn rotate_coords(e: &Expr, m: &[Vec<f64>], space: &Space) -> Expr {
    let n = space.dims;
    let c = &space.center;
    e.substitute(&|name: &str| {
        let k = coord_index(name).filter(|&k| k < n)?;
        let c0 = c[k] - (0..n).map(|j| m[j][k] * c[j]).sum::<f64>();
        Some(linear(c0, (0..n).map(|j| (m[j][k], Expr::Var(coord_name(j)))).collect()))
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
/// 成分は座標の順（x, y, z, w, …）。表示中の3つの座標の成分だけを描く（残りは射影で落とす）。
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
        return Err("媒介変数は t / u, v / t₁, t₂ のどれか1組だけを使ってください".into());
    }
    if vars.iter().any(|v| v == "t") && vars.len() > 1 {
        return Err("曲線は t だけ、曲面は u と v を使ってください".into());
    }
    // u だけ・v だけでも曲面として扱う
    if vars.iter().any(|v| v == "u" || v == "v") {
        vars = vec!["u".into(), "v".into()];
    }
    if vars.is_empty() {
        vars.push("t".into()); // 媒介変数を使っていない（点）
    }
    if vars.len() > 2 {
        return Err("媒介変数が3つ以上の図形（立体）はまだ描けません".into());
    }

    // 範囲で重ねるパラメータを使っていれば、それも媒介変数として動かす（曲線 → 曲面）
    let ranged: Vec<String> = space
        .param_ranges
        .iter()
        .map(|(n, _, _)| n.clone())
        .filter(|n| !vars.contains(n) && items.iter().any(|e| e.uses_var(n)))
        .collect();
    if vars.len() + ranged.len() > 2 {
        return Err("曲面を範囲で重ねた立体はまだ描けません".into());
    }
    vars.extend(ranged);
    let mut consts = params.clone();
    for (n, _, _) in &space.param_ranges {
        consts.remove(n);
    }

    // 表示する3つの座標の成分を作る（回転していれば q = R (p − c) + c）
    let rot = space.rotation_matrix();
    let shown_exprs: Vec<Expr> = space
        .axes
        .iter()
        .map(|&a| match &rot {
            None => items[a].clone(),
            Some(m) => {
                let c = &space.center;
                let n = space.dims;
                let c0 = c[a] - (0..n).map(|k| m[a][k] * c[k]).sum::<f64>();
                linear(c0, (0..n).map(|k| (m[a][k], items[k].clone())).collect())
            }
        })
        .collect();

    let var_refs: Vec<&str> = vars.iter().map(|s| s.as_str()).collect();
    for e in &items {
        // 表示しない成分も、誤りが無いかだけ確かめる
        Program::compile_with(e, &var_refs, &consts)?;
    }
    let mut progs = Vec::with_capacity(3);
    for e in &shown_exprs {
        // 組の中の t, u, v は媒介変数なので、同じ名前のパラメータより優先される
        progs.push(Program::compile_with(e, &var_refs, &consts)?);
    }
    let shown: [Program; 3] = progs.try_into().map_err(|_| "内部エラー".to_string())?;
    Ok(if vars.len() == 2 {
        Plot::Surface { progs: shown, vars }
    } else {
        Plot::Curve { progs: shown, vars }
    })
}

/// 変数を含まない式（範囲の指定など）の値を計算する。例: "2pi" → 6.283...
pub fn eval_constant(source: &str) -> Result<f64, String> {
    let e = parse_at(source, 0)?;
    let prog = Program::compile(&e, &[]).map_err(|_| "数値と、pi などの定数だけで書いてください".to_string())?;
    let mut out = [C::new(0.0, 0.0)];
    prog.eval_batch(&[], &mut out);
    let v = crate::eval::as_real(out[0]);
    if v.is_finite() {
        Ok(v)
    } else {
        Err("実数の値になりません".into())
    }
}

fn explicit(axis: usize, e: &Expr, shown: &[String; 3], consts: &Params) -> Result<Plot, String> {
    let (a, b) = others(axis);
    let prog = Program::compile_with(e, &[&shown[a], &shown[b]], consts)?;
    Ok(Plot::Explicit { axis, names: shown.clone(), prog })
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
        assert!(kind("(u, t, 0)").starts_with("error"));
        assert!(kind("(t, x, 0)").starts_with("error"));
        assert!(kind("(1, 2, 3, 4)").starts_with("error"));
        // 括弧の掛け算は組ではない
        assert_eq!(kind("(x+1)(y-1)"), "z = f(x, y)");
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
        assert!(compile_with("(t_1, t_2, t_3, 0)", &ps, &sp).err().unwrap().contains("3つ以上"));
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
        assert!(compile_with("(u, v, a)", &ps, &sp).err().unwrap().contains("まだ"));
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
        // 平面 x = 1 は、回すと平面 y = 1 になる（陰関数として描かれる）
        let p = compile_with("x = 1", &ps, &sp).unwrap();
        let prog: &Program = match &p {
            Plot::Implicit { prog } => prog,
            _ => panic!("回転すると陰関数になる"),
        };
        let mut out = [C::new(0.0, 0.0); 2];
        // (5, 1, 0) は y = 1 の上、(1, 5, 0) は上にない
        prog.eval_batch(&[&[5.0, 1.0], &[1.0, 5.0], &[0.0, 0.0]], &mut out);
        assert!(out[0].norm() < 1e-12 && out[1].norm() > 1.0);
        // 中心 (1, 0, 0) の周りで 90° 回すと、平面 x = 1 は中心を通るので y = 0 になる
        sp.center = vec![1.0, 0.0, 0.0];
        let p = compile_with("x = 1", &ps, &sp).unwrap();
        if let Plot::Implicit { prog } = &p {
            prog.eval_batch(&[&[7.0, 0.0], &[0.0, 0.0], &[0.0, 0.0]], &mut out);
            assert!(out[0].norm() < 1e-12);
        }
        // 媒介変数表示も回る: (t, 0, 0) を xy で 90° 回すと (0, t, 0)
        let mut sp = Space::standard();
        sp.rotation[0] = std::f64::consts::FRAC_PI_2;
        if let Plot::Curve { progs, .. } = compile_with("(t, 0, 0)", &ps, &sp).unwrap() {
            let mut o = [C::new(0.0, 0.0)];
            progs[0].eval_batch(&[&[2.0]], &mut o);
            assert!(o[0].norm() < 1e-12);
            progs[1].eval_batch(&[&[2.0]], &mut o);
            assert!((o[0].re - 2.0).abs() < 1e-12);
        } else {
            panic!()
        }
        // 4次元: 回転の数は 4C2 = 6
        assert_eq!(Space::basic(4, [0, 1, 2], vec![0.0; 4]).rotation.len(), 6);
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

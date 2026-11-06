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
}

impl Space {
    /// ふつうの3次元（x, y, z をそのまま表示）
    pub fn standard() -> Space {
        Space { dims: 3, axes: [0, 1, 2], slice: vec![0.0; 3] }
    }

    /// 表示中の座標の名前（横・奥・縦の順）
    fn shown(&self) -> [String; 3] {
        self.axes.map(coord_name)
    }

    /// 表示していない座標を、断面の値の定数としてパラメータに足す
    fn with_hidden(&self, params: &Params) -> Params {
        let mut ps = params.clone();
        for k in 0..self.dims {
            if !self.axes.contains(&k) {
                ps.insert(coord_name(k), C::new(self.slice.get(k).copied().unwrap_or(0.0), 0.0));
            }
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

    let parts: Vec<&str> = source.split('=').collect();
    match parts.as_slice() {
        [rhs] => {
            let e = parse_at(rhs, 0)?;
            check_dims(&e, space)?;
            if let Expr::Tuple(items) = e {
                return parametric(items, params, space);
            }
            if e.uses_var(&shown[2]) {
                return Err(format!(
                    "{} を含む式は「… = 0」のように = を付けてください（陰関数になります）",
                    shown[2]
                ));
            }
            explicit(2, &e, &shown, &consts)
        }
        [lhs, rhs] => {
            let l = parse_at(lhs, 0)?;
            let r = parse_at(rhs, lhs.chars().count() + 1)?;
            check_dims(&l, space)?;
            check_dims(&r, space)?;
            if matches!(l, Expr::Tuple(_)) || matches!(r, Expr::Tuple(_)) {
                return Err("媒介変数表示は = を付けずに (x(t), y(t), z(t)) の形で書いてください".into());
            }
            // 片方が表示中の座標1つだけで、もう片方にその座標が無ければ陽関数
            for (axis, name) in shown.iter().enumerate() {
                let single = |e: &Expr| matches!(e, Expr::Var(v) if v == name);
                if single(&l) && !r.uses_var(name) {
                    return explicit(axis, &r, &shown, &consts);
                }
                if single(&r) && !l.uses_var(name) {
                    return explicit(axis, &l, &shown, &consts);
                }
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
            if !shown.iter().any(|v| l.uses_var(v) || r.uses_var(v)) {
                return Err(format!("表示中の座標（{}）のどれかを使った式にしてください", shown.join(", ")));
            }
            let f = Expr::Bin(BinOp::Sub, Box::new(l), Box::new(r));
            Ok(Plot::Implicit { prog: Program::compile_with(&f, &shown_refs, &consts)? })
        }
        _ => Err("= は1つだけにしてください".into()),
    }
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

    let var_refs: Vec<&str> = vars.iter().map(|s| s.as_str()).collect();
    let mut progs = Vec::with_capacity(items.len());
    for e in &items {
        // 組の中の t, u, v は媒介変数なので、同じ名前のパラメータより優先される
        progs.push(Program::compile_with(e, &var_refs, params)?);
    }
    let shown: [Program; 3] = space.axes.map(|k| progs[k].clone());
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
        Space { dims: 4, axes, slice: vec![0.0, 0.0, 0.0, w] }
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
        let sp5 = Space { dims: 5, axes: [0, 3, 4], slice: vec![0.5; 5] };
        assert_eq!(compile_with("w_2 = x + w", &ps, &sp5).unwrap().kind_label(), "w2 = f(x, w)");
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

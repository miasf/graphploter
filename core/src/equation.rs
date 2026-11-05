//! 入力された式が「どの種類のグラフか」を判定して、コンパイルする。
//!
//! - `z = f(x, y)` / `y = f(x, z)` / `x = f(y, z)`   → 陽関数の曲面
//!   （左右が逆の `f(x, y) = z` でもよい。`=` が無ければ `z =` を補う）
//! - それ以外の `左辺 = 右辺`                         → 陰関数 F(x, y, z) = 左辺 − 右辺 = 0
//! - `(x(t), y(t), z(t))`                              → 媒介変数表示の曲線
//! - `(x(u,v), y(u,v), z(u,v))`                        → 媒介変数表示の曲面
//!   （成分が2つなら z = 0 の平面上）

use crate::eval::Program;
use crate::parser::{parse, BinOp, Expr, ParseError};

pub const AXES: [&str; 3] = ["x", "y", "z"];

/// コンパイル済みのグラフ
pub enum Plot {
    /// 陽関数。`axis` 番目の座標（0:x, 1:y, 2:z）が、残り2つの関数になっている。
    /// `prog` の変数は残り2つを x, y, z の順に並べたもの。
    Explicit { axis: usize, prog: Program },
    /// 陰関数 F(x, y, z) = 0。`prog` の変数は x, y, z。
    Implicit { prog: Program },
    /// 媒介変数表示の曲線。`progs` は x, y, z 成分で、変数は t。
    Curve { progs: [Program; 3] },
    /// 媒介変数表示の曲面。`progs` は x, y, z 成分で、変数は u, v。
    Surface { progs: [Program; 3] },
}

impl Plot {
    /// 画面に出す説明
    pub fn kind_label(&self) -> String {
        match self {
            Plot::Explicit { axis, .. } => {
                let (a, b) = others(*axis);
                format!("{} = f({}, {})", AXES[*axis], AXES[a], AXES[b])
            }
            Plot::Implicit { .. } => "陰関数".into(),
            Plot::Curve { .. } => "媒介変数表示の曲線".into(),
            Plot::Surface { .. } => "媒介変数表示の曲面".into(),
        }
    }

    /// 媒介変数の名前（媒介変数表示でなければ空）
    pub fn params(&self) -> &'static str {
        match self {
            Plot::Curve { .. } => "t",
            Plot::Surface { .. } => "uv",
            _ => "",
        }
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

pub fn compile(source: &str) -> Result<Plot, String> {
    let parts: Vec<&str> = source.split('=').collect();
    match parts.as_slice() {
        [rhs] => {
            let e = parse_at(rhs, 0)?;
            if let Expr::Tuple(items) = e {
                return parametric(items);
            }
            if e.uses_var("z") {
                return Err("z を含む式は「… = 0」のように = を付けてください（陰関数になります）".into());
            }
            explicit(2, &e)
        }
        [lhs, rhs] => {
            let l = parse_at(lhs, 0)?;
            let r = parse_at(rhs, lhs.chars().count() + 1)?;
            if matches!(l, Expr::Tuple(_)) || matches!(r, Expr::Tuple(_)) {
                return Err("媒介変数表示は = を付けずに (x(t), y(t), z(t)) の形で書いてください".into());
            }
            // 片方が x / y / z だけで、もう片方にその変数が無ければ陽関数
            for (axis, name) in AXES.iter().enumerate() {
                let single = |e: &Expr| matches!(e, Expr::Var(v) if v == name);
                if single(&l) && !r.uses_var(name) {
                    return explicit(axis, &r);
                }
                if single(&r) && !l.uses_var(name) {
                    return explicit(axis, &l);
                }
            }
            let f = Expr::Bin(BinOp::Sub, Box::new(l), Box::new(r));
            Ok(Plot::Implicit { prog: Program::compile(&f, &AXES)? })
        }
        _ => Err("= は1つだけにしてください".into()),
    }
}

/// 組 (a, b, c) を媒介変数表示としてコンパイルする
fn parametric(mut items: Vec<Expr>) -> Result<Plot, String> {
    match items.len() {
        2 => items.push(Expr::Num(0.0)),
        3 => {}
        n => return Err(format!("媒介変数表示の成分は 2 つか 3 つにしてください（{n} 個あります）")),
    }
    let uses = |name: &str| items.iter().any(|e| e.uses_var(name));
    let surface = uses("u") || uses("v");
    if surface && uses("t") {
        return Err("曲線は t だけ、曲面は u と v だけを使ってください".into());
    }
    let vars: &[&str] = if surface { &["u", "v"] } else { &["t"] };
    let mut progs = Vec::with_capacity(3);
    for e in &items {
        progs.push(Program::compile(e, vars)?);
    }
    let progs: [Program; 3] = progs.try_into().map_err(|_| "内部エラー".to_string())?;
    Ok(if surface { Plot::Surface { progs } } else { Plot::Curve { progs } })
}

/// 変数を含まない式（範囲の指定など）の値を計算する。例: "2pi" → 6.283...
pub fn eval_constant(source: &str) -> Result<f64, String> {
    let e = parse_at(source, 0)?;
    let prog = Program::compile(&e, &[]).map_err(|_| "数値と、pi などの定数だけで書いてください".to_string())?;
    let mut out = [num_complex::Complex64::new(0.0, 0.0)];
    prog.eval_batch(&[], &mut out);
    let v = crate::eval::as_real(out[0]);
    if v.is_finite() {
        Ok(v)
    } else {
        Err("実数の値になりません".into())
    }
}

fn explicit(axis: usize, e: &Expr) -> Result<Plot, String> {
    let (a, b) = others(axis);
    let prog = Program::compile(e, &[AXES[a], AXES[b]])?;
    Ok(Plot::Explicit { axis, prog })
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

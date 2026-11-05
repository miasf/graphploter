//! 入力された式が「どの種類のグラフか」を判定して、コンパイルする。
//!
//! - `z = f(x, y)` / `y = f(x, z)` / `x = f(y, z)`   → 陽関数の曲面
//!   （左右が逆の `f(x, y) = z` でもよい。`=` が無ければ `z =` を補う）
//! - それ以外の `左辺 = 右辺`                         → 陰関数 F(x, y, z) = 左辺 − 右辺 = 0

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
            if e.uses_var("z") {
                return Err("z を含む式は「… = 0」のように = を付けてください（陰関数になります）".into());
            }
            explicit(2, &e)
        }
        [lhs, rhs] => {
            let l = parse_at(lhs, 0)?;
            let r = parse_at(rhs, lhs.chars().count() + 1)?;
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

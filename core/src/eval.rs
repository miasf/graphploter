//! 構文木を「命令列」に変換し、たくさんの点でまとめて計算する。
//!
//! 1点ずつ構文木をたどると遅いので、
//!   1. 構文木を命令の列（`Op` の並び）に変換しておき
//!   2. 各命令を CHUNK 個の点に対してまとめて実行する
//!
//! という方式にしている。命令の解釈にかかる手間が点の数で薄まるので速い。
//!
//! 値はすべて複素数 (Complex64) で計算する。

use crate::functions::Func;
use crate::parser::{BinOp, Constant, Expr};
use num_complex::Complex64 as C;
use std::f64::consts::{E, PI};

/// 一度にまとめて計算する点の数
const CHUNK: usize = 256;

/// 命令。引数の `usize` は「何番目の命令の結果を使うか」。
#[derive(Debug, Clone)]
enum Op {
    Const(C),
    Var(usize),
    Neg(usize),
    Bin(BinOp, usize, usize),
    Call1(Func, usize),
}

/// コンパイル済みの式
#[derive(Debug, Clone)]
pub struct Program {
    ops: Vec<Op>,
    /// 変数名（`Var(k)` の k 番目の名前）
    vars: Vec<String>,
}

impl Program {
    /// 構文木をコンパイルする。`vars` は使ってよい変数名の一覧。
    pub fn compile(expr: &Expr, vars: &[&str]) -> Result<Program, String> {
        let mut p = Program {
            ops: Vec::new(),
            vars: vars.iter().map(|s| s.to_string()).collect(),
        };
        p.emit(expr)?;
        Ok(p)
    }

    pub fn vars(&self) -> &[String] {
        &self.vars
    }

    /// 命令を1つ追加して、その番号を返す。引数がすべて定数なら、その場で計算して定数にする。
    fn push(&mut self, op: Op) -> usize {
        let folded = match &op {
            Op::Neg(a) => self.const_of(*a).map(|a| -a),
            Op::Bin(o, a, b) => match (self.const_of(*a), self.const_of(*b)) {
                (Some(a), Some(b)) => Some(binop(*o, a, b)),
                _ => None,
            },
            Op::Call1(f, a) => self.const_of(*a).map(|a| f.eval1(a)),
            _ => None,
        };
        self.ops.push(match folded {
            Some(c) => Op::Const(c),
            None => op,
        });
        self.ops.len() - 1
    }

    fn const_of(&self, i: usize) -> Option<C> {
        match self.ops[i] {
            Op::Const(c) => Some(c),
            _ => None,
        }
    }

    fn emit(&mut self, e: &Expr) -> Result<usize, String> {
        Ok(match e {
            Expr::Num(n) => self.push(Op::Const(C::new(*n, 0.0))),
            Expr::Const(c) => self.push(Op::Const(match c {
                Constant::Pi => C::new(PI, 0.0),
                Constant::E => C::new(E, 0.0),
                Constant::I => C::new(0.0, 1.0),
            })),
            Expr::Var(name) => match self.vars.iter().position(|v| v == name) {
                Some(k) => self.push(Op::Var(k)),
                None => {
                    return Err(format!(
                        "変数 '{name}' はここでは使えません（使える変数: {}）",
                        self.vars.join(", ")
                    ))
                }
            },
            Expr::Neg(a) => {
                let a = self.emit(a)?;
                self.push(Op::Neg(a))
            }
            Expr::Bin(o, a, b) => {
                let a = self.emit(a)?;
                let b = self.emit(b)?;
                self.push(Op::Bin(*o, a, b))
            }
            Expr::Call(f, args) => {
                let a = self.emit(&args[0])?;
                self.push(Op::Call1(*f, a))
            }
        })
    }

    /// 多数の点でまとめて計算する。
    ///
    /// `inputs[k]` は k 番目の変数の値の列（すべて同じ長さ）。結果は `out` に書き込む。
    pub fn eval_batch(&self, inputs: &[&[f64]], out: &mut [C]) {
        assert_eq!(inputs.len(), self.vars.len());
        let n = out.len();
        for inp in inputs {
            assert_eq!(inp.len(), n);
        }

        // 命令ごとの作業領域（CHUNK 個ずつ）
        let mut regs = vec![[C::new(0.0, 0.0); CHUNK]; self.ops.len()];
        let last = self.ops.len() - 1;

        let mut start = 0;
        while start < n {
            let len = CHUNK.min(n - start);
            for (i, op) in self.ops.iter().enumerate() {
                // 自分より前の命令の結果だけを読むので、split_at_mut で安全に分けられる
                let (done, rest) = regs.split_at_mut(i);
                let dst = &mut rest[0][..len];
                match op {
                    Op::Const(c) => dst.fill(*c),
                    Op::Var(k) => {
                        for (d, x) in dst.iter_mut().zip(&inputs[*k][start..start + len]) {
                            *d = C::new(*x, 0.0);
                        }
                    }
                    Op::Neg(a) => {
                        for (d, a) in dst.iter_mut().zip(&done[*a][..len]) {
                            *d = -a;
                        }
                    }
                    Op::Bin(o, a, b) => {
                        let (a, b) = (&done[*a][..len], &done[*b][..len]);
                        for j in 0..len {
                            dst[j] = binop(*o, a[j], b[j]);
                        }
                    }
                    Op::Call1(f, a) => {
                        for (d, a) in dst.iter_mut().zip(&done[*a][..len]) {
                            *d = f.eval1(*a);
                        }
                    }
                }
            }
            out[start..start + len].copy_from_slice(&regs[last][..len]);
            start += len;
        }
    }
}

fn binop(o: BinOp, a: C, b: C) -> C {
    match o {
        BinOp::Add => a + b,
        BinOp::Sub => a - b,
        BinOp::Mul => a * b,
        BinOp::Div => a / b,
        BinOp::Pow => pow(a, b),
    }
}

/// 累乗。整数乗や「正の実数の実数乗」は専用の計算にして、誤差と虚部のゴミを防ぐ。
fn pow(a: C, b: C) -> C {
    if b.im == 0.0 {
        if b.re.fract() == 0.0 && b.re.abs() <= 1024.0 {
            return a.powi(b.re as i32);
        }
        if a.im == 0.0 && a.re >= 0.0 {
            return C::new(a.re.powf(b.re), 0.0);
        }
    }
    if a == C::new(0.0, 0.0) {
        return if b.re > 0.0 { a } else { C::new(f64::NAN, f64::NAN) };
    }
    a.powc(b)
}

/// 複素数の結果を実数として扱う。虚部が無視できないほど大きければ「定義されない」(NaN)。
pub fn as_real(z: C) -> f64 {
    if z.im.abs() <= 1e-9 * z.re.abs().max(1.0) {
        z.re
    } else {
        f64::NAN
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn eval_xy(src: &str, x: f64, y: f64) -> C {
        let p = Program::compile(&parse(src).unwrap(), &["x", "y"]).unwrap();
        let mut out = [C::new(0.0, 0.0)];
        p.eval_batch(&[&[x], &[y]], &mut out);
        out[0]
    }

    #[test]
    fn basic() {
        assert_eq!(eval_xy("x^2 + y", 3.0, 1.0), C::new(10.0, 0.0));
        assert_eq!(eval_xy("2xy", 3.0, 4.0), C::new(24.0, 0.0));
        assert!((eval_xy("sin(pi/2)", 0.0, 0.0).re - 1.0).abs() < 1e-15);
    }

    #[test]
    fn complex_values() {
        // i^2 = -1
        assert_eq!(eval_xy("i^2", 0.0, 0.0), C::new(-1.0, 0.0));
        // sqrt(-4) = 2i → 実数としては定義されない
        let z = eval_xy("sqrt(x)", -4.0, 0.0);
        assert!((z - C::new(0.0, 2.0)).norm() < 1e-12);
        assert!(as_real(z).is_nan());
        // (-2)^2 = 4（虚部のゴミが出ない）
        assert_eq!(eval_xy("x^2", -2.0, 0.0), C::new(4.0, 0.0));
    }

    #[test]
    fn many_points() {
        // CHUNK をまたぐ長さでも正しく計算できるか
        let n = 1000;
        let xs: Vec<f64> = (0..n).map(|k| k as f64).collect();
        let ys = vec![1.0; n];
        let p = Program::compile(&parse("x + y").unwrap(), &["x", "y"]).unwrap();
        let mut out = vec![C::new(0.0, 0.0); n];
        p.eval_batch(&[&xs, &ys], &mut out);
        for k in 0..n {
            assert_eq!(out[k].re, k as f64 + 1.0);
        }
    }

    #[test]
    fn unknown_variable() {
        assert!(Program::compile(&parse("x + a").unwrap(), &["x", "y"]).is_err());
    }
}

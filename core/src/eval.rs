//! 構文木を「命令列」に変換し、たくさんの点でまとめて計算する。
//!
//! 1点ずつ構文木をたどると遅いので、
//!   1. 構文木を命令の列（`Op` の並び）に変換しておき
//!   2. 各命令を CHUNK 個の点に対してまとめて実行する
//!
//! という方式にしている。命令の解釈にかかる手間が点の数で薄まるので速い。
//!
//! 速くするための工夫:
//! - **同じ計算の共有**: 同じ命令（例: 2か所に出てくる `sin(u)`）は1回だけ計算する。
//!   媒介変数表示の成分どうしでも共有する（[`ProgramSet`]）。
//! - **定数の畳み込み**: 引数がすべて定数の命令は、コンパイルのときに計算しておく。
//!   媒介変数の一部を固定した式（[`ProgramSet::bind`]）では、固定した変数だけで決まる部分がまとめて定数になる。
//! - **実数だけの計算**: 値は複素数 (Complex64) で計算するのが基本だが、式の定数がすべて実数で、
//!   使っている関数が初等関数だけなら、まず実数 (f64) だけで計算する。
//!   途中で実数に収まらない所（`sqrt` の中が負など）や、有限でない値が出た CHUNK だけ、複素数で計算し直す。
//!   だから結果は複素数で計算したときと（丸め誤差を除いて）同じになる。

use crate::interval::{Interval, Range};
use crate::functions::Func;
use crate::parser::{BinOp, Constant, Expr};
use num_complex::Complex64 as C;
use std::collections::HashMap;
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
    Call2(Func, usize, usize),
    /// 引数が3つ以上の関数
    CallN(Func, Vec<usize>),
}

/// パラメータ（スライダーで値を決める文字）の名前と値
pub type Params = std::collections::BTreeMap<String, C>;

/// 命令の列を組み立てる。定数の畳み込みと、同じ命令の共有をする
#[derive(Default)]
struct Builder {
    ops: Vec<Op>,
    /// 命令（の文字列での表現）→ 番号。同じ命令は1つにまとめる
    seen: HashMap<String, usize>,
}

impl Builder {
    /// 命令を1つ追加して、その番号を返す。引数がすべて定数なら、その場で計算して定数にする。
    /// 同じ命令がすでにあれば、その番号を返す
    fn push(&mut self, op: Op) -> usize {
        let folded = match &op {
            Op::Neg(a) => self.const_of(*a).map(|a| -a),
            Op::Bin(o, a, b) => match (self.const_of(*a), self.const_of(*b)) {
                (Some(a), Some(b)) => Some(binop(*o, a, b)),
                _ => None,
            },
            Op::Call1(f, a) => self.const_of(*a).map(|a| f.eval1(a)),
            Op::Call2(f, a, b) => match (self.const_of(*a), self.const_of(*b)) {
                (Some(a), Some(b)) => Some(f.eval2(a, b)),
                _ => None,
            },
            Op::CallN(f, args) => {
                let vals: Option<Vec<C>> = args.iter().map(|&a| self.const_of(a)).collect();
                vals.map(|v| f.call(&v))
            }
            _ => None,
        };
        let op = match folded {
            Some(c) => Op::Const(c),
            None => op,
        };
        // 定数は、ビットまで同じ値だけを同じとみなす（0.0 と −0.0、NaN どうしを取り違えない）
        let key = match &op {
            Op::Const(c) => format!("C{:x},{:x}", c.re.to_bits(), c.im.to_bits()),
            other => format!("{other:?}"),
        };
        if let Some(&k) = self.seen.get(&key) {
            return k;
        }
        self.ops.push(op);
        self.seen.insert(key, self.ops.len() - 1);
        self.ops.len() - 1
    }

    fn const_of(&self, i: usize) -> Option<C> {
        match self.ops[i] {
            Op::Const(c) => Some(c),
            _ => None,
        }
    }

    /// 別の命令列 `ops` を足す。`var` は変数の命令の置き換え（固定する変数なら定数を返す）。
    /// 戻り値は、`ops` の各命令が、足した後の何番目になったか
    fn append(&mut self, ops: &[Op], var: &dyn Fn(usize) -> Op) -> Vec<usize> {
        let mut at: Vec<usize> = Vec::with_capacity(ops.len());
        for op in ops {
            let new = match op {
                Op::Const(c) => Op::Const(*c),
                Op::Var(k) => var(*k),
                Op::Neg(a) => Op::Neg(at[*a]),
                Op::Bin(o, a, b) => Op::Bin(*o, at[*a], at[*b]),
                Op::Call1(f, a) => Op::Call1(*f, at[*a]),
                Op::Call2(f, a, b) => Op::Call2(*f, at[*a], at[*b]),
                Op::CallN(f, args) => Op::CallN(*f, args.iter().map(|&a| at[a]).collect()),
            };
            at.push(self.push(new));
        }
        at
    }
}

/// 実数だけの計算（[`Code::chunk_real`]）で扱える1引数の関数か
fn real_fn1(f: Func) -> bool {
    use Func::*;
    matches!(
        f,
        Sin | Cos | Tan | Sec | Csc | Cot | Asin | Acos | Atan | Sinh | Cosh | Tanh | Asinh | Acosh | Atanh | Exp
            | Ln | Log10 | Sqrt | Abs | Sinc | Floor | Ceil | Round | Sign | Re | Im | Conj
    )
}

/// 実数だけの計算で扱える2引数の関数か
fn real_fn2(f: Func) -> bool {
    matches!(f, Func::Min | Func::Max | Func::Mod)
}

/// 計算の結果の受け取り先。CHUNK ごとに、結果ごとの値の列を受け取る
trait Sink {
    /// `out` 番目の結果の、`start` 番目の点からの値（実数だけで計算できたとき）
    fn real(&mut self, out: usize, start: usize, vals: &[f64]);
    /// 同じく、複素数で計算したとき
    fn complex(&mut self, out: usize, start: usize, vals: &[C]);
}

/// 命令の列と、結果にする命令の番号（いくつあってもよい）
#[derive(Debug, Clone)]
struct Code {
    ops: Vec<Op>,
    outs: Vec<usize>,
    /// まず実数だけで計算してみるか（定数がすべて実数で、関数が初等関数だけ）
    real: bool,
}

impl Code {
    /// 結果に使われない命令（畳み込まれた定数の引数など）を捨てて、命令列を仕上げる
    fn new(b: Builder, outs: Vec<usize>) -> Code {
        let ops = b.ops;
        let mut used = vec![false; ops.len()];
        for &o in &outs {
            used[o] = true;
        }
        for i in (0..ops.len()).rev() {
            if !used[i] {
                continue;
            }
            match &ops[i] {
                Op::Const(_) | Op::Var(_) => {}
                Op::Neg(a) | Op::Call1(_, a) => used[*a] = true,
                Op::Bin(_, a, b) | Op::Call2(_, a, b) => {
                    used[*a] = true;
                    used[*b] = true;
                }
                Op::CallN(_, args) => args.iter().for_each(|&a| used[a] = true),
            }
        }
        let mut at = vec![usize::MAX; ops.len()];
        let mut kept = Vec::with_capacity(ops.len());
        for (i, op) in ops.into_iter().enumerate() {
            if !used[i] {
                continue;
            }
            at[i] = kept.len();
            kept.push(match op {
                Op::Neg(a) => Op::Neg(at[a]),
                Op::Bin(o, a, b) => Op::Bin(o, at[a], at[b]),
                Op::Call1(f, a) => Op::Call1(f, at[a]),
                Op::Call2(f, a, b) => Op::Call2(f, at[a], at[b]),
                Op::CallN(f, args) => Op::CallN(f, args.iter().map(|&a| at[a]).collect()),
                leaf => leaf,
            });
        }
        let real = kept.iter().all(|op| match op {
            Op::Const(c) => c.im == 0.0,
            Op::Var(_) | Op::Neg(_) | Op::Bin(..) => true,
            Op::Call1(f, _) => real_fn1(*f),
            Op::Call2(f, _, _) => real_fn2(*f),
            Op::CallN(..) => false,
        });
        Code { ops: kept, outs: outs.into_iter().map(|o| at[o]).collect(), real }
    }

    /// `n` 個の点でまとめて計算して、結果を `sink` に渡す。`inputs[k]` は k 番目の変数の値の列
    /// （式に出てこない変数の列は空でもよい）
    /// `run_inner` を呼び、かかった時間と点の数を記録する（[`crate::timing`]）
    fn run<S: Sink>(&self, inputs: &[&[f64]], n: usize, sink: &mut S) {
        let t = crate::timing::now_ms();
        self.run_inner(inputs, n, sink);
        crate::timing::record_eval(crate::timing::now_ms() - t, n);
    }

    fn run_inner<S: Sink>(&self, inputs: &[&[f64]], n: usize, sink: &mut S) {
        let m = self.ops.len();
        // 命令ごとの作業領域（CHUNK 個ずつ）。定数は最初に1回だけ書き込む
        let mut re: Vec<f64> = Vec::new();
        let mut cx: Vec<C> = Vec::new();
        if self.real {
            re = vec![0.0; m * CHUNK];
            for (i, op) in self.ops.iter().enumerate() {
                if let Op::Const(c) = op {
                    re[i * CHUNK..(i + 1) * CHUNK].fill(c.re);
                }
            }
        }
        let mut start = 0;
        while start < n {
            let len = CHUNK.min(n - start);
            if self.real && self.chunk_real(inputs, start, len, &mut re) {
                for (j, &o) in self.outs.iter().enumerate() {
                    sink.real(j, start, &re[o * CHUNK..o * CHUNK + len]);
                }
            } else {
                if cx.is_empty() {
                    cx = vec![C::new(0.0, 0.0); m * CHUNK];
                    for (i, op) in self.ops.iter().enumerate() {
                        if let Op::Const(c) = op {
                            cx[i * CHUNK..(i + 1) * CHUNK].fill(*c);
                        }
                    }
                }
                self.chunk_complex(inputs, start, len, &mut cx);
                for (j, &o) in self.outs.iter().enumerate() {
                    sink.complex(j, start, &cx[o * CHUNK..o * CHUNK + len]);
                }
            }
            start += len;
        }
    }

    /// 1つの CHUNK を複素数で計算する
    fn chunk_complex(&self, inputs: &[&[f64]], start: usize, len: usize, regs: &mut [C]) {
        for (i, op) in self.ops.iter().enumerate() {
            // 自分より前の命令の結果だけを読むので、split_at_mut で安全に分けられる
            let (done, rest) = regs.split_at_mut(i * CHUNK);
            let dst = &mut rest[..len];
            let arg = |a: usize| &done[a * CHUNK..a * CHUNK + len];
            match op {
                Op::Const(_) => {}
                Op::Var(k) => {
                    for (d, x) in dst.iter_mut().zip(&inputs[*k][start..start + len]) {
                        *d = C::new(*x, 0.0);
                    }
                }
                Op::Neg(a) => {
                    for (d, a) in dst.iter_mut().zip(arg(*a)) {
                        *d = -a;
                    }
                }
                Op::Bin(o, a, b) => {
                    let (a, b) = (arg(*a), arg(*b));
                    for j in 0..len {
                        dst[j] = binop(*o, a[j], b[j]);
                    }
                }
                Op::Call1(f, a) => {
                    for (d, a) in dst.iter_mut().zip(arg(*a)) {
                        *d = f.eval1(*a);
                    }
                }
                Op::Call2(f, a, b) => {
                    let (a, b) = (arg(*a), arg(*b));
                    for j in 0..len {
                        dst[j] = f.eval2(a[j], b[j]);
                    }
                }
                Op::CallN(f, args) => {
                    let mut vals = vec![C::new(0.0, 0.0); args.len()];
                    for (j, d) in dst.iter_mut().enumerate() {
                        for (v, &a) in vals.iter_mut().zip(args) {
                            *v = done[a * CHUNK + j];
                        }
                        *d = f.call(&vals);
                    }
                }
            }
        }
    }

    /// 1つの CHUNK を実数だけで計算する。実数に収まらない所（`sqrt` の中が負など）や、
    /// 有限でない値が出たら false を返す（呼び出し側が複素数で計算し直す）
    /// 区間演算（[`Program::eval_interval`]）。命令は [`Code::chunk_real`] と同じものを扱う。分からなければ None
    fn interval(&self, boxes: &[Interval]) -> Option<Interval> {
        use crate::interval as iv;
        let mut regs: Vec<Interval> = Vec::with_capacity(self.ops.len());
        for op in &self.ops {
            let r = |k: usize| regs[k];
            let v = match op {
                Op::Const(c) => {
                    if c.im != 0.0 || !c.re.is_finite() {
                        return None;
                    }
                    Interval::point(c.re)
                }
                Op::Var(k) => boxes[*k],
                Op::Neg(a) => iv::neg(r(*a))?,
                Op::Bin(BinOp::Add, a, b) => iv::add(r(*a), r(*b))?,
                Op::Bin(BinOp::Sub, a, b) => iv::sub(r(*a), r(*b))?,
                Op::Bin(BinOp::Mul, a, b) => iv::mul(r(*a), r(*b))?,
                Op::Bin(BinOp::Div, a, b) => iv::div(r(*a), r(*b))?,
                Op::Bin(BinOp::Pow, a, b) => iv::pow(r(*a), r(*b))?,
                Op::Call1(f, a) => {
                    let x = r(*a);
                    match f {
                        Func::Sin => iv::sin(x)?,
                        Func::Cos => iv::cos(x)?,
                        Func::Tan => iv::tan(x)?,
                        Func::Sec => iv::sec(x)?,
                        Func::Csc => iv::csc(x)?,
                        Func::Cot => iv::cot(x)?,
                        Func::Asin => iv::asin(x)?,
                        Func::Acos => iv::acos(x)?,
                        Func::Atan => iv::atan(x)?,
                        Func::Sinh => iv::sinh(x)?,
                        Func::Cosh => iv::cosh(x)?,
                        Func::Tanh => iv::tanh(x)?,
                        Func::Asinh => iv::asinh(x)?,
                        Func::Acosh => iv::acosh(x)?,
                        Func::Atanh => iv::atanh(x)?,
                        Func::Exp => iv::exp(x)?,
                        Func::Ln => iv::ln(x)?,
                        Func::Log10 => iv::log10(x)?,
                        Func::Sqrt => iv::sqrt(x)?,
                        Func::Abs => iv::abs(x)?,
                        Func::Sinc => iv::sinc(x)?,
                        Func::Floor => iv::floor(x)?,
                        Func::Ceil => iv::ceil(x)?,
                        Func::Round => iv::round(x)?,
                        Func::Sign => iv::sign(x)?,
                        Func::Re | Func::Conj => x,
                        Func::Im => Interval::point(0.0),
                        _ => return None,
                    }
                }
                Op::Call2(f, a, b) => match f {
                    Func::Min => iv::min(r(*a), r(*b))?,
                    Func::Max => iv::max(r(*a), r(*b))?,
                    // mod は値が飛ぶ所が多いので、最初は扱わない
                    _ => return None,
                },
                Op::CallN(..) => return None,
            };
            regs.push(v);
        }
        Some(regs[self.outs[0]])
    }

    fn chunk_real(&self, inputs: &[&[f64]], start: usize, len: usize, regs: &mut [f64]) -> bool {
        for (i, op) in self.ops.iter().enumerate() {
            let (done, rest) = regs.split_at_mut(i * CHUNK);
            let dst = &mut rest[..len];
            let arg = |a: usize| &done[a * CHUNK..a * CHUNK + len];
            // 1引数: dst[j] = f(a[j])
            macro_rules! map1 {
                ($a:expr, $f:expr) => {
                    for (d, &x) in dst.iter_mut().zip(arg($a)) {
                        *d = $f(x);
                    }
                };
            }
            // 1引数で、使える範囲が決まっているもの（範囲の外が1つでもあれば、複素数で計算し直す）
            macro_rules! map1_if {
                ($a:expr, $ok:expr, $f:expr) => {{
                    let mut ok = true;
                    for (d, &x) in dst.iter_mut().zip(arg($a)) {
                        ok &= $ok(x);
                        *d = $f(x);
                    }
                    if !ok {
                        return false;
                    }
                }};
            }
            macro_rules! map2 {
                ($a:expr, $b:expr, $f:expr) => {{
                    let (a, b) = (arg($a), arg($b));
                    for j in 0..len {
                        dst[j] = $f(a[j], b[j]);
                    }
                }};
            }
            match op {
                Op::Const(_) => continue,
                Op::Var(k) => dst.copy_from_slice(&inputs[*k][start..start + len]),
                Op::Neg(a) => map1!(*a, |x: f64| -x),
                Op::Bin(BinOp::Add, a, b) => map2!(*a, *b, |x: f64, y: f64| x + y),
                Op::Bin(BinOp::Sub, a, b) => map2!(*a, *b, |x: f64, y: f64| x - y),
                Op::Bin(BinOp::Mul, a, b) => map2!(*a, *b, |x: f64, y: f64| x * y),
                Op::Bin(BinOp::Div, a, b) => map2!(*a, *b, |x: f64, y: f64| x / y),
                // 累乗は複素数の pow と同じ場合分けにする（整数乗 / 0 以上の数の実数乗。それ以外は複素数になる）
                Op::Bin(BinOp::Pow, a, b) => match self.ops[*b] {
                    Op::Const(e) if e.re.fract() == 0.0 && e.re.abs() <= 1024.0 => {
                        let e = e.re as i32;
                        map1!(*a, |x: f64| x.powi(e))
                    }
                    _ => {
                        let (a, b) = (arg(*a), arg(*b));
                        let mut ok = true;
                        for j in 0..len {
                            let (x, y) = (a[j], b[j]);
                            dst[j] = if y.fract() == 0.0 && y.abs() <= 1024.0 {
                                x.powi(y as i32)
                            } else {
                                ok &= x >= 0.0;
                                x.powf(y)
                            };
                        }
                        if !ok {
                            return false;
                        }
                    }
                },
                Op::Call1(f, a) => match f {
                    Func::Sin => map1!(*a, f64::sin),
                    Func::Cos => map1!(*a, f64::cos),
                    Func::Tan => map1!(*a, f64::tan),
                    Func::Sec => map1!(*a, |x: f64| 1.0 / x.cos()),
                    Func::Csc => map1!(*a, |x: f64| 1.0 / x.sin()),
                    Func::Cot => map1!(*a, |x: f64| x.cos() / x.sin()),
                    Func::Asin => map1_if!(*a, |x: f64| x.abs() <= 1.0, f64::asin),
                    Func::Acos => map1_if!(*a, |x: f64| x.abs() <= 1.0, f64::acos),
                    Func::Atan => map1!(*a, f64::atan),
                    Func::Sinh => map1!(*a, f64::sinh),
                    Func::Cosh => map1!(*a, f64::cosh),
                    Func::Tanh => map1!(*a, f64::tanh),
                    Func::Asinh => map1!(*a, f64::asinh),
                    Func::Acosh => map1_if!(*a, |x: f64| x >= 1.0, f64::acosh),
                    Func::Atanh => map1_if!(*a, |x: f64| x.abs() <= 1.0, f64::atanh),
                    Func::Exp => map1!(*a, f64::exp),
                    Func::Ln => map1_if!(*a, |x: f64| x >= 0.0, f64::ln),
                    Func::Log10 => map1_if!(*a, |x: f64| x >= 0.0, f64::log10),
                    Func::Sqrt => map1_if!(*a, |x: f64| x >= 0.0, f64::sqrt),
                    Func::Abs => map1!(*a, f64::abs),
                    Func::Sinc => map1!(*a, |x: f64| if x.abs() < 1e-8 { 1.0 - x * x / 6.0 } else { x.sin() / x }),
                    Func::Floor => map1!(*a, f64::floor),
                    Func::Ceil => map1!(*a, f64::ceil),
                    Func::Round => map1!(*a, f64::round),
                    Func::Sign => map1!(*a, |x: f64| if x == 0.0 { x } else { x / x.abs() }),
                    Func::Re | Func::Conj => map1!(*a, |x: f64| x),
                    Func::Im => dst.fill(0.0),
                    _ => return false,
                },
                Op::Call2(f, a, b) => match f {
                    Func::Min => map2!(*a, *b, |x: f64, y: f64| if y < x { y } else { x }),
                    Func::Max => map2!(*a, *b, |x: f64, y: f64| if y > x { y } else { x }),
                    Func::Mod => map2!(*a, *b, |x: f64, y: f64| x - y * (x / y).floor()),
                    _ => return false,
                },
                Op::CallN(..) => return false,
            }
            // 有限でない値（0 での割り算・桁あふれなど）が1つでもあれば、v·0 の和が NaN になる
            let mut check = 0.0;
            for &v in dst.iter() {
                check += v * 0.0;
            }
            if check.is_nan() {
                return false;
            }
        }
        true
    }
}

/// コンパイル済みの式
#[derive(Debug, Clone)]
pub struct Program {
    code: Code,
    /// 変数名（`Var(k)` の k 番目の名前）
    vars: Vec<String>,
    /// 実際に使われたパラメータの名前
    used_params: Vec<String>,
}

/// 構文木を命令列にする途中の状態
struct Compiler<'a> {
    b: Builder,
    vars: &'a [&'a str],
    /// 使ってよいパラメータ（式の中では定数として扱う）
    params: &'a Params,
    used_params: Vec<String>,
}

impl Program {
    /// 構文木をコンパイルする。`vars` は使ってよい変数名の一覧。
    pub fn compile(expr: &Expr, vars: &[&str]) -> Result<Program, String> {
        Self::compile_with(expr, vars, &Params::new())
    }

    /// パラメータ付きでコンパイルする。`params` にある文字は、その値の定数として扱う。
    pub fn compile_with(expr: &Expr, vars: &[&str], params: &Params) -> Result<Program, String> {
        let mut c = Compiler { b: Builder::default(), vars, params, used_params: Vec::new() };
        let root = c.emit(expr)?;
        Ok(Program {
            code: Code::new(c.b, vec![root]),
            vars: vars.iter().map(|s| s.to_string()).collect(),
            used_params: c.used_params,
        })
    }

    pub fn vars(&self) -> &[String] {
        &self.vars
    }

    /// k 番目の変数を使っているか（定数に畳み込まれて消えた変数は使っていない）
    pub fn uses_var_index(&self, k: usize) -> bool {
        self.code.ops.iter().any(|op| matches!(op, Op::Var(j) if *j == k))
    }

    /// この式で使われたパラメータの名前
    pub fn used_params(&self) -> &[String] {
        &self.used_params
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
        struct Out<'a>(&'a mut [C]);
        impl Sink for Out<'_> {
            fn real(&mut self, _: usize, start: usize, vals: &[f64]) {
                for (d, v) in self.0[start..start + vals.len()].iter_mut().zip(vals) {
                    *d = C::new(*v, 0.0);
                }
            }
            fn complex(&mut self, _: usize, start: usize, vals: &[C]) {
                self.0[start..start + vals.len()].copy_from_slice(vals);
            }
        }
        self.code.run(inputs, n, &mut Out(out));
    }

    /// 区間演算が使える式か（定数がすべて実数で、関数が初等関数だけ。特殊関数・虚数の定数を使う式は使えない）
    pub fn supports_interval(&self) -> bool {
        self.code.real
    }

    /// 区間演算: `boxes[k]`（k 番目の変数の区間）の上で、式の値が入る区間（[`crate::interval`] を参照）。
    /// 区間演算が使えない式は、すぐ `Unknown` を返す
    pub fn eval_interval(&self, boxes: &[Interval]) -> Range {
        assert_eq!(boxes.len(), self.vars.len());
        if !self.code.real {
            return Range::Unknown;
        }
        match self.code.interval(boxes) {
            Some(i) => Range::Known(i),
            None => Range::Unknown,
        }
    }

    /// 実数だけの計算を使わない式にしたもの（テストで、結果を比べるのに使う）
    #[cfg(test)]
    fn complex_only(&self) -> Program {
        let mut p = self.clone();
        p.code.real = false;
        p
    }
}

impl Compiler<'_> {
    fn emit(&mut self, e: &Expr) -> Result<usize, String> {
        Ok(match e {
            Expr::Num(n) => self.b.push(Op::Const(C::new(*n, 0.0))),
            Expr::Const(c) => self.b.push(Op::Const(match c {
                Constant::Pi => C::new(PI, 0.0),
                Constant::E => C::new(E, 0.0),
                Constant::I => C::new(0.0, 1.0),
            })),
            Expr::Var(name) => {
                if let Some(k) = self.vars.iter().position(|v| v == name) {
                    self.b.push(Op::Var(k))
                } else if let Some(&c) = self.params.get(name) {
                    if !self.used_params.contains(name) {
                        self.used_params.push(name.clone());
                    }
                    self.b.push(Op::Const(c))
                } else {
                    // この文言は画面側で「スライダーを追加」ボタンを出すのに使っている（変えるときは注意）
                    return Err(format!(
                        "変数 '{name}' はここでは使えません（使える変数: {}）",
                        self.vars.join(", ")
                    ));
                }
            }
            Expr::Neg(a) => {
                let a = self.emit(a)?;
                self.b.push(Op::Neg(a))
            }
            Expr::Bin(o, a, b) => {
                let a = self.emit(a)?;
                let b = self.emit(b)?;
                self.b.push(Op::Bin(*o, a, b))
            }
            Expr::Call(f, args) => {
                let mut ids = Vec::with_capacity(args.len());
                for a in args {
                    ids.push(self.emit(a)?);
                }
                match ids.as_slice() {
                    [a] => self.b.push(Op::Call1(*f, *a)),
                    [a, b] => self.b.push(Op::Call2(*f, *a, *b)),
                    _ => self.b.push(Op::CallN(*f, ids)),
                }
            }
            Expr::Tuple(_) => return Err("組 (a, b, c) は式の中では使えません".into()),
            Expr::List(_) => {
                return Err("リスト [a, b] は、今は媒介変数表示 ( … ) の中でだけ使えます".into())
            }
        })
    }
}

/// いくつかの式（媒介変数表示の成分。変数は共通）を、1つの命令列にまとめたもの。
/// 成分どうしで同じ計算（`sin(u)` など）を共有するので、成分ごとに計算するより速い
#[derive(Debug, Clone)]
pub struct ProgramSet {
    code: Code,
    /// 変数の数
    nvars: usize,
}

impl ProgramSet {
    /// `progs`（変数の並びが同じ式）を1つにまとめる
    pub fn new(progs: &[Program]) -> ProgramSet {
        let mut b = Builder::default();
        let outs = progs.iter().map(|p| b.append(&p.code.ops, &Op::Var)[p.code.outs[0]]).collect();
        ProgramSet { code: Code::new(b, outs), nvars: progs.first().map_or(0, |p| p.vars.len()) }
    }

    /// 式の数（点の次元）
    pub fn len(&self) -> usize {
        self.code.outs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.code.outs.is_empty()
    }

    /// いくつかの変数を値に固定した式にする（`fixed` は (変数の番号, 値)）。
    /// 固定した変数だけで決まる部分は、ここで計算して定数にする
    pub fn bind(&self, fixed: &[(usize, f64)]) -> ProgramSet {
        let mut b = Builder::default();
        let at = b.append(&self.code.ops, &|k| match fixed.iter().find(|(v, _)| *v == k) {
            Some(&(_, x)) => Op::Const(C::new(x, 0.0)),
            None => Op::Var(k),
        });
        let outs = self.code.outs.iter().map(|&o| at[o]).collect();
        ProgramSet { code: Code::new(b, outs), nvars: self.nvars }
    }

    /// `n` 個の点で計算して、点ごとに成分を並べたもの（点ごとに `len()` 個）を `out` の後ろに足す。
    /// 実数にならない成分は NaN。`inputs[k]` は k 番目の変数の値の列（固定した変数の列は空でよい）
    pub fn eval_points(&self, inputs: &[&[f64]], n: usize, out: &mut Vec<f64>) {
        assert_eq!(inputs.len(), self.nvars);
        let d = self.len();
        let base = out.len();
        out.resize(base + n * d, 0.0);
        struct Points<'a> {
            pts: &'a mut [f64],
            d: usize,
        }
        impl Sink for Points<'_> {
            fn real(&mut self, out: usize, start: usize, vals: &[f64]) {
                for (k, v) in vals.iter().enumerate() {
                    self.pts[(start + k) * self.d + out] = *v;
                }
            }
            fn complex(&mut self, out: usize, start: usize, vals: &[C]) {
                for (k, v) in vals.iter().enumerate() {
                    self.pts[(start + k) * self.d + out] = as_real(*v);
                }
            }
        }
        self.code.run(inputs, n, &mut Points { pts: &mut out[base..], d });
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
        for (k, v) in out.iter().enumerate() {
            assert_eq!(v.re, k as f64 + 1.0);
        }
    }

    #[test]
    fn new_functions() {
        assert_eq!(eval_xy("mod(x, 3)", 7.0, 0.0), C::new(1.0, 0.0));
        assert_eq!(eval_xy("nCr(x, y)", 5.0, 2.0), C::new(10.0, 0.0));
        assert_eq!(eval_xy("nPr(5, 2) + floor(x)", 1.7, 0.0), C::new(21.0, 0.0));
        assert_eq!(eval_xy("max(x, y) - min(x, y)", 1.0, 4.0), C::new(3.0, 0.0));
        // 階乗
        assert!((eval_xy("x!", 5.0, 0.0).re - 120.0).abs() < 1e-9);
        assert!((eval_xy("3!^2", 0.0, 0.0).re - 36.0).abs() < 1e-9);
        assert!((eval_xy("-x!", 3.0, 0.0).re + 6.0).abs() < 1e-9);
    }

    #[test]
    fn params_are_constants() {
        let mut ps = Params::new();
        ps.insert("a".into(), C::new(2.0, 0.0));
        ps.insert("b".into(), C::new(0.0, 1.0));
        let p = Program::compile_with(&parse("a x + b").unwrap(), &["x"], &ps).unwrap();
        let mut out = [C::new(0.0, 0.0)];
        p.eval_batch(&[&[3.0]], &mut out);
        assert_eq!(out[0], C::new(6.0, 1.0));
        assert_eq!(p.used_params(), &["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn special_functions_in_expressions() {
        let v = |src: &str| eval_xy(src, 2.0, 0.0);
        assert!((v("zeta(x)").re - std::f64::consts::PI.powi(2) / 6.0).abs() < 1e-12);
        assert!((v("erf(0)").re).abs() < 1e-15);
        assert!((v("beta(x, 3)").re - 1.0 / 12.0).abs() < 1e-12);
        assert!((v("besselJ(0, 1)").re - 0.765_197_686_557_966_6).abs() < 1e-12);
        assert!((v("psi(1)").re + 0.577_215_664_901_532_9).abs() < 1e-12);
        assert!((v("arcsinh(sinh(x))").re - 2.0).abs() < 1e-12);
        assert!((v("cot(x) tan(x)").re - 1.0).abs() < 1e-12);
    }

    #[test]
    fn unknown_variable() {
        assert!(Program::compile(&parse("x + a").unwrap(), &["x", "y"]).is_err());
    }

    /// 実数だけの計算は、複素数での計算と同じ結果になる（実数に収まらない所は複素数で計算し直す）
    #[test]
    fn real_path_matches_complex() {
        let sources = [
            "x^2 + y^2 - 1",
            "sin(x) cos(y) + sin(x)",
            "sqrt(1 - x^2 - y^2)",
            "ln(x) + log(y)",
            "x^y",
            "x^(1/3) + y^-2",
            "1 / (x y)",
            "tan(x) + sec(y) + csc(x) + cot(y)",
            "arcsin(x) + arccos(y) + arctan(x y)",
            "sinh(x) + cosh(y) tanh(x) + arcsinh(x) + arccosh(y) + arctanh(x)",
            "exp(x) / (1 + exp(-y))",
            "abs(x) + floor(y) + ceil(x) + round(y) + sign(x)",
            "mod(x, y) + min(x, y) max(x, y)",
            "sinc(x) + Re(x) + Im(y) + conj(x)",
            "exp(900 x)",
            "x! + gamma(y)",
            "(x + i y)^2",
            "sqrt(x) sqrt(x)",
        ];
        // 0・負の数・定義域の境目・格子の点にちょうど乗る値を含める
        let vals = [-3.0, -1.0, -0.5, 0.0, 0.25, 0.5, 1.0, 1.5, 2.0, 7.3];
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for &x in &vals {
            for &y in &vals {
                xs.push(x);
                ys.push(y);
            }
        }
        // CHUNK をまたぐ長さにする（途中の CHUNK だけが複素数になる場合も確かめる）
        let (xs, ys) = (xs.repeat(7), ys.repeat(7));
        for src in sources {
            let p = Program::compile(&parse(src).unwrap(), &["x", "y"]).unwrap();
            let mut fast = vec![C::new(0.0, 0.0); xs.len()];
            let mut slow = fast.clone();
            p.eval_batch(&[&xs, &ys], &mut fast);
            p.complex_only().eval_batch(&[&xs, &ys], &mut slow);
            for k in 0..xs.len() {
                let (a, b) = (fast[k], slow[k]);
                let same = |u: f64, v: f64| (u.is_nan() && v.is_nan()) || u == v || (u - v).abs() <= 1e-9 * v.abs().max(1.0);
                assert!(same(a.re, b.re) && same(a.im, b.im), "{src} at ({}, {}): {a} vs {b}", xs[k], ys[k]);
            }
        }
        // 初等関数だけの式は実数で、特殊関数・虚数単位を含む式は複素数で計算する
        let real = |src: &str| Program::compile(&parse(src).unwrap(), &["x", "y"]).unwrap().code.real;
        assert!(real("sin(x) sqrt(y) + 2^x"));
        assert!(!real("gamma(x)") && !real("x + i y") && !real("arg(x)"));
    }

    #[test]
    fn shared_and_folded() {
        let prog = |src: &str| Program::compile(&parse(src).unwrap(), &["x", "y"]).unwrap();
        // sin(x) は1回だけ計算する: x, sin, y, cos, *, + の6個
        assert_eq!(prog("sin(x) cos(y) + sin(x)").code.ops.len(), 6);
        // 定数だけの部分は畳み込んで、元の定数は捨てる: x, 2π, * の3個
        assert_eq!(prog("2 pi x").code.ops.len(), 3);

        // 成分どうしでも共有する: (cos x cos y, sin x cos y) の cos y は1つ
        let set = ProgramSet::new(&[prog("cos(x) cos(y)"), prog("sin(x) cos(y)"), prog("sin(y)")]);
        assert_eq!(set.len(), 3);
        assert_eq!(set.code.ops.len(), 8); // x, cos x, y, cos y, *, sin x, *, sin y
        let mut pts = Vec::new();
        set.eval_points(&[&[0.0, 1.0], &[0.5, 2.0]], 2, &mut pts);
        assert_eq!(pts.len(), 6);
        assert!((pts[3] - 1f64.cos() * 2f64.cos()).abs() < 1e-15 && (pts[5] - 2f64.sin()).abs() < 1e-15);

        // y を固定すると、y だけで決まる部分が定数になる。固定した変数の列は空でよい
        let bound = set.bind(&[(1, 2.0)]);
        assert_eq!(bound.code.ops.len(), 7); // x, cos x, 定数 cos 2, *, sin x, *, 定数 sin 2
        let mut q = Vec::new();
        bound.eval_points(&[&[1.0], &[]], 1, &mut q);
        assert_eq!(q, pts[3..].to_vec());
        // 全部固定すると定数だけになる
        let all = set.bind(&[(0, 1.0), (1, 2.0)]);
        let mut q = Vec::new();
        all.eval_points(&[&[], &[]], 3, &mut q);
        assert_eq!(q, pts[3..].repeat(3));
    }
}

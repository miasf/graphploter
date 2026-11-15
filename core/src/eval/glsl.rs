//! 命令列から GLSL を作る（計画 `docs/plan-hard-tasks.md` 12 章の G3）。
//!
//! 命令列（[`Op`]）を 1 命令 1 行の GLSL（WebGL2 の GLSL ES 3.00）に直す。同じ命令の共有・定数の畳み込みは、
//! 命令列を作るときに済んでいるので、そのまま効く。作る関数は 3 つ:
//!
//! - `float F(float p[N])`: 値。実数だけの速い道（`chunk_real`）と同じ計算。実数にならない所（`sqrt` の中が負など）は NaN
//! - `vec4 Fd(vec4 p[N])`: 二重数（前進型の自動微分）。`x` が値、`yzw` が勾配。入力の `yzw` に「その座標を表示中の 3 つの座標で
//!   微分したもの」（回転の行列の列）を入れて呼べば、表示中の座標についての勾配が出る
//! - `Div Fi(vec2 p[N])`: 飾り付きの区間（G1 の [`Code::interval_dec`] と同じ決まり）。入力は区間（`x` が下、`y` が上）。
//!   `Div` は `{ float lo; float hi; int d; }` で、`d` は飾り（0: Trv、1: Def、2: Dac、3: Com）、`lo > hi` は空
//!
//! パラメータ（スライダーの文字）は uniform の配列（既定 `uParam`）から読み、定数に畳み込まない。
//! 呼び出し側は、パラメータも変数としてコンパイルし（`Program::compile(&e, &["x", "y", "z", "a"])`）、
//! [`GlslOptions::params`] にその変数の番号を渡す。スライダーを動かしても GLSL を作り直さなくてよい。
//!
//! 区間の丸め: 32 ビットなので、命令ごとに外側へ $2^{-20}(\lvert x\rvert + 1)$ 広げる（Rust の $10^{-12}$ と同じ考え）。
//! `sin`・`exp` など GPU の誤差が大きい関数と累乗は、さらに $2^{-16}(\lvert x\rvert + 1)$ 広げる。
//! 極（`tan` の $\pi/2 + k\pi$）・`sin` の山の判定も、同じ幅だけ「含む」側に倒す（見落としより、区間が広がるほうを選ぶ）。

use super::{Code, Op, Program};
use crate::functions::Func;
use crate::parser::BinOp;
use std::fmt::Write;

/// GLSL の作り方
#[derive(Debug, Clone)]
pub struct GlslOptions {
    /// 関数の名前（値は `name`、二重数は `name` + `d`、区間は `name` + `i`）。1 つのシェーダーに式をいくつも入れるときに分ける
    pub name: String,
    /// uniform にする変数（パラメータ）の番号。ここに無い変数が、関数の引数の配列 `p` に順に並ぶ
    pub params: Vec<usize>,
    /// パラメータの uniform の配列の名前
    pub param_uniform: String,
}

impl Default for GlslOptions {
    fn default() -> Self {
        GlslOptions { name: "F".into(), params: Vec::new(), param_uniform: "uParam".into() }
    }
}

/// 作った GLSL
#[derive(Debug, Clone, PartialEq)]
pub struct GlslCode {
    /// 下請けの関数（使うものだけ）。同じシェーダーに式をいくつも入れるときは、[`glsl_prelude_all`] を 1 回だけ入れる
    pub prelude: String,
    /// パラメータの uniform の宣言（パラメータが無ければ空）
    pub uniforms: String,
    /// `float F(float p[N])`
    pub value: String,
    /// `vec4 Fd(vec4 p[N])`
    pub dual: String,
    /// `Div Fi(vec2 p[N])`
    pub interval: String,
    /// 引数の配列の長さ N（パラメータを除いた変数の数。0 個のときも配列は長さ 1）
    pub inputs: usize,
    /// パラメータの数（uniform の配列の長さ）
    pub params: usize,
}

impl GlslCode {
    /// 全部をつないだもの（`#version` と `precision` は付けない）
    pub fn source(&self) -> String {
        format!("{}{}{}{}{}", self.prelude, self.uniforms, self.value, self.dual, self.interval)
    }
}

/// 全部の下請けの関数（式をいくつも 1 つのシェーダーに入れるとき用）
pub fn glsl_prelude_all() -> String {
    let all: Vec<&str> = snippets().iter().map(|s| s.name).collect();
    prelude_for(&all)
}

/// 下請けの関数 1 つ。`deps` は先に置く関数
struct Snip {
    name: &'static str,
    deps: &'static [&'static str],
    code: String,
}

/// GPU の描き方で使える 1 引数の関数か
fn glsl_fn1(f: Func) -> bool {
    use Func::*;
    matches!(
        f,
        Sin | Cos | Tan | Sec | Csc | Cot | Asin | Acos | Atan | Sinh | Cosh | Tanh | Asinh | Acosh | Atanh | Exp
            | Ln | Log10 | Sqrt | Abs | Sinc | Floor | Ceil | Round | Sign | Re | Im | Conj
    )
}

/// 関数の、GLSL の下請けの名前の後ろの部分（`gf_sin`・`gd_sin`・`gi_sin` の `sin`）
fn fn_key(f: Func) -> &'static str {
    use Func::*;
    match f {
        Sin => "sin",
        Cos => "cos",
        Tan => "tan",
        Sec => "sec",
        Csc => "csc",
        Cot => "cot",
        Asin => "asin",
        Acos => "acos",
        Atan => "atan",
        Sinh => "sinh",
        Cosh => "cosh",
        Tanh => "tanh",
        Asinh => "asinh",
        Acosh => "acosh",
        Atanh => "atanh",
        Exp => "exp",
        Ln => "ln",
        Log10 => "log10",
        Sqrt => "sqrt",
        Abs => "abs",
        Sinc => "sinc",
        Floor => "floor",
        Ceil => "ceil",
        Round => "round",
        Sign => "sign",
        Re | Conj => "re",
        Im => "im",
        Min => "min",
        Max => "max",
        Mod => "mod",
        _ => "",
    }
}

/// f32 の定数の書き方（GLSL の float の字句。有限でなければ下請けの関数）
fn lit(v: f64) -> String {
    if v.is_nan() {
        return "gp_nan()".into();
    }
    let f = v as f32;
    if f.is_infinite() {
        return if f > 0.0 { "gp_inf()".into() } else { "(-gp_inf())".into() };
    }
    // Rust の {:?} は f32 を戻せる最短の桁で書く（1e-5、0.1、3.0 など）
    let s = format!("{f:?}");
    let s = if s.contains('.') || s.contains('e') || s.contains("inf") { s } else { format!("{s}.0") };
    if f < 0.0 {
        format!("({s})")
    } else {
        s
    }
}

/// いくつかの式を 1 つのシェーダーに入れる GLSL（下請けの関数は 1 回だけ）。`items` は (式, 関数の名前)。
/// パラメータは uniform にしない（式に畳み込まれた定数のまま）。戻り値は GLSL と、式ごとの引数の配列の長さ
pub fn glsl_bundle(items: &[(&Program, &str)]) -> Result<(String, Vec<usize>), String> {
    let mut used: Vec<&'static str> = Vec::new();
    let mut body = String::new();
    let mut lens = Vec::with_capacity(items.len());
    for (p, name) in items {
        let (g, u) = to_glsl_used(&p.code, p.vars.len(), &GlslOptions { name: name.to_string(), ..Default::default() })?;
        for n in u {
            if !used.contains(&n) {
                used.push(n);
            }
        }
        body.push_str(&g.value);
        body.push_str(&g.dual);
        body.push_str(&g.interval);
        lens.push(g.inputs.max(1));
    }
    Ok((format!("{}{body}", prelude_for(&used)), lens))
}

impl Program {
    /// GLSL にする（計画 12 章の G3）。実数だけで計算できない式（複素数・特殊関数）と、
    /// GPU に移していない関数（場合分けなど）を使う式は `Err` を返す（呼び出し側はメッシュの道に戻る）
    pub fn to_glsl(&self, opts: &GlslOptions) -> Result<GlslCode, String> {
        to_glsl(&self.code, self.vars.len(), opts)
    }
}

/// GLSL にする命令の数の上限。これより大きい式（積分を展開した式など）は、シェーダーのコンパイルが重すぎる（計画 12.6 の 4）
pub const MAX_GLSL_OPS: usize = 2000;

fn to_glsl(code: &Code, nvars: usize, opts: &GlslOptions) -> Result<GlslCode, String> {
    to_glsl_used(code, nvars, opts).map(|(g, _)| g)
}

/// [`to_glsl`] と、使った下請けの名前
fn to_glsl_used(code: &Code, nvars: usize, opts: &GlslOptions) -> Result<(GlslCode, Vec<&'static str>), String> {
    if !code.real {
        // 使えない関数の名前を出す
        for op in &code.ops {
            let f = match op {
                Op::Call1(f, _) | Op::Call2(f, _, _) | Op::CallN(f, _) => *f,
                _ => continue,
            };
            let ok = match op {
                Op::Call1(..) => glsl_fn1(f),
                Op::Call2(..) => matches!(f, Func::Min | Func::Max | Func::Mod),
                _ => false,
            };
            if !ok {
                return Err(unsupported(f));
            }
        }
        return Err("複素数を使う式は GPU の描き方では使えません".into());
    }
    for op in &code.ops {
        match op {
            Op::Call1(f, _) if !glsl_fn1(*f) => return Err(unsupported(*f)),
            Op::Call2(f, _, _) if !matches!(f, Func::Min | Func::Max | Func::Mod) => return Err(unsupported(*f)),
            Op::CallN(f, _) => return Err(unsupported(*f)),
            _ => {}
        }
    }
    if code.ops.len() > MAX_GLSL_OPS {
        return Err(format!("式が大きすぎて GPU の描き方では使えません（命令 {} 個。上限 {MAX_GLSL_OPS}）", code.ops.len()));
    }
    // 変数 → 引数の配列の何番目か、uniform の何番目か
    let mut slot: Vec<Slot> = Vec::with_capacity(nvars);
    let mut n_in = 0;
    for k in 0..nvars {
        if let Some(j) = opts.params.iter().position(|&p| p == k) {
            slot.push(Slot::Param(j));
        } else {
            slot.push(Slot::Input(n_in));
            n_in += 1;
        }
    }
    let n_par = opts.params.len();
    let arr = n_in.max(1);
    let u = &opts.param_uniform;
    let name = &opts.name;
    let out = code.outs[0];

    let mut used: Vec<&'static str> = vec!["gp_inf", "gp_nan"];
    let mut need = |n: &'static str| {
        if !used.contains(&n) {
            used.push(n);
        }
    };
    let const_of = |i: usize| match code.ops[i] {
        Op::Const(c) => Some(c.re),
        _ => None,
    };
    // 整数の定数の指数（実数だけの道と同じ条件: 小数部が 0 で、絶対値が 1024 以下）
    let int_exp = |i: usize| const_of(i).filter(|e| e.fract() == 0.0 && e.abs() <= 1024.0).map(|e| e as i32);

    // --- 値 ---
    let mut val = format!("float {name}(float p[{arr}]) {{\n");
    for (i, op) in code.ops.iter().enumerate() {
        let e = match op {
            Op::Const(c) => lit(c.re),
            Op::Var(k) => match slot[*k] {
                Slot::Input(j) => format!("p[{j}]"),
                Slot::Param(j) => format!("{u}[{j}]"),
            },
            Op::Neg(a) => format!("-v{a}"),
            Op::Bin(BinOp::Add, a, b) => format!("v{a} + v{b}"),
            Op::Bin(BinOp::Sub, a, b) => format!("v{a} - v{b}"),
            Op::Bin(BinOp::Mul, a, b) => format!("v{a} * v{b}"),
            Op::Bin(BinOp::Div, a, b) => format!("v{a} / v{b}"),
            Op::Bin(BinOp::Pow, a, b) => {
                if let Some(e) = int_exp(*b) {
                    need("gp_powi");
                    format!("gp_powi(v{a}, {e})")
                } else if let Some(e) = const_of(*b) {
                    need("gf_powc");
                    format!("gf_powc(v{a}, {})", lit(e))
                } else {
                    need("gf_pow");
                    format!("gf_pow(v{a}, v{b})")
                }
            }
            Op::Call1(f, a) => {
                let s = match f {
                    Func::Atan | Func::Sinh | Func::Cosh | Func::Asinh | Func::Exp
                    | Func::Abs | Func::Floor | Func::Ceil | Func::Sign => builtin(*f),
                    _ => {
                        need(snip_name("gf_", *f));
                        format!("gf_{}", fn_key(*f))
                    }
                };
                format!("{s}(v{a})")
            }
            Op::Call2(Func::Min, a, b) => format!("(v{b} < v{a} ? v{b} : v{a})"),
            Op::Call2(Func::Max, a, b) => format!("(v{b} > v{a} ? v{b} : v{a})"),
            Op::Call2(_, a, b) => format!("v{a} - v{b} * floor(v{a} / v{b})"),
            Op::CallN(..) => unreachable!(),
        };
        let _ = writeln!(val, "  float v{i} = {e};");
    }
    let _ = writeln!(val, "  return v{out};\n}}");

    // --- 二重数 ---
    let mut dual = format!("vec4 {name}d(vec4 p[{arr}]) {{\n");
    for (i, op) in code.ops.iter().enumerate() {
        let e = match op {
            Op::Const(c) => format!("vec4({}, 0.0, 0.0, 0.0)", lit(c.re)),
            Op::Var(k) => match slot[*k] {
                Slot::Input(j) => format!("p[{j}]"),
                Slot::Param(j) => format!("vec4({u}[{j}], 0.0, 0.0, 0.0)"),
            },
            Op::Neg(a) => format!("-d{a}"),
            Op::Bin(BinOp::Add, a, b) => format!("d{a} + d{b}"),
            Op::Bin(BinOp::Sub, a, b) => format!("d{a} - d{b}"),
            Op::Bin(BinOp::Mul, a, b) => {
                need("gd_mul");
                format!("gd_mul(d{a}, d{b})")
            }
            Op::Bin(BinOp::Div, a, b) => {
                need("gd_div");
                format!("gd_div(d{a}, d{b})")
            }
            Op::Bin(BinOp::Pow, a, b) => {
                if let Some(e) = int_exp(*b) {
                    need("gd_powi");
                    format!("gd_powi(d{a}, {e})")
                } else if let Some(e) = const_of(*b) {
                    need("gd_powc");
                    format!("gd_powc(d{a}, {})", lit(e))
                } else {
                    need("gd_pow");
                    format!("gd_pow(d{a}, d{b})")
                }
            }
            Op::Call1(f, a) => {
                need(snip_name("gd_", *f));
                format!("gd_{}(d{a})", fn_key(*f))
            }
            Op::Call2(Func::Min, a, b) => format!("(d{b}.x < d{a}.x ? d{b} : d{a})"),
            Op::Call2(Func::Max, a, b) => format!("(d{b}.x > d{a}.x ? d{b} : d{a})"),
            Op::Call2(_, a, b) => {
                need("gd_mod");
                format!("gd_mod(d{a}, d{b})")
            }
            Op::CallN(..) => unreachable!(),
        };
        let _ = writeln!(dual, "  vec4 d{i} = {e};");
    }
    let _ = writeln!(dual, "  return d{out};\n}}");

    // --- 飾り付きの区間 ---
    need("gi_core");
    let mut inter = format!("Div {name}i(vec2 p[{arr}]) {{\n");
    for (i, op) in code.ops.iter().enumerate() {
        let e = match op {
            Op::Const(c) => {
                if c.re.is_finite() && (c.re as f32).is_finite() {
                    // 定数は f32 に丸めた分だけ広げる（点のままにすると、0.1 などが本当の値を含まない）
                    let f = c.re as f32;
                    if f as f64 == c.re {
                        format!("Div({0}, {0}, 3)", lit(c.re))
                    } else {
                        let (lo, hi) = if (f as f64) < c.re { (f, next_up(f)) } else { (next_down(f), f) };
                        format!("Div({}, {}, 3)", lit(lo as f64), lit(hi as f64))
                    }
                } else {
                    "gi_entire(0)".into()
                }
            }
            Op::Var(k) => match slot[*k] {
                Slot::Input(j) => format!("Div(p[{j}].x, p[{j}].y, 3)"),
                Slot::Param(j) => format!("Div({u}[{j}], {u}[{j}], 3)"),
            },
            Op::Neg(a) => {
                need("gi_neg");
                format!("gi_neg(r{a})")
            }
            Op::Bin(o, a, b) => {
                let n = match o {
                    BinOp::Add => "gi_add",
                    BinOp::Sub => "gi_sub",
                    BinOp::Mul => "gi_mul",
                    BinOp::Div => "gi_div",
                    BinOp::Pow => "gi_pow",
                };
                need(n);
                format!("{n}(r{a}, r{b})")
            }
            Op::Call1(f, a) => {
                need(snip_name("gi_", *f));
                format!("gi_{}(r{a})", fn_key(*f))
            }
            Op::Call2(f, a, b) => {
                need(snip_name("gi_", *f));
                format!("gi_{}(r{a}, r{b})", fn_key(*f))
            }
            Op::CallN(..) => unreachable!(),
        };
        let _ = writeln!(inter, "  Div r{i} = {e};");
    }
    let _ = writeln!(inter, "  return r{out};\n}}");

    let uniforms = if n_par > 0 { format!("uniform float {u}[{n_par}];\n") } else { String::new() };
    let prelude = prelude_for(&used);
    Ok((GlslCode { prelude, uniforms, value: val, dual, interval: inter, inputs: n_in, params: n_par }, used))
}

#[derive(Clone, Copy)]
enum Slot {
    Input(usize),
    Param(usize),
}

fn unsupported(f: Func) -> String {
    let n = f.names().first().copied().unwrap_or("場合分け");
    format!("{n} は GPU の描き方では使えません")
}

/// GLSL の組み込みの関数をそのまま使う関数（値の道。定義域の外が無く、精度の問題も無いもの）
fn builtin(f: Func) -> String {
    match f {
        Func::Atan => "atan",
        Func::Sinh => "sinh",
        Func::Cosh => "cosh",
        Func::Asinh => "asinh",
        Func::Exp => "exp",
        Func::Abs => "abs",
        Func::Floor => "floor",
        Func::Ceil => "ceil",
        Func::Sign => "sign",
        _ => unreachable!(),
    }
    .into()
}

/// 下請けの名前（`gf_` + 関数）。表に無ければ（呼ぶ前に確かめているので）落とす
fn snip_name(prefix: &str, f: Func) -> &'static str {
    let want = format!("{prefix}{}", fn_key(f));
    snippets().iter().find(|s| s.name == want).map(|s| s.name).unwrap_or_else(|| panic!("GLSL の下請け {want} が無い"))
}

fn next_up(f: f32) -> f32 {
    if f == 0.0 {
        f32::from_bits(1)
    } else if f > 0.0 {
        f32::from_bits(f.to_bits() + 1)
    } else {
        f32::from_bits(f.to_bits() - 1)
    }
}

fn next_down(f: f32) -> f32 {
    -next_up(-f)
}

/// 使う下請けと、その前提を、表の順に並べる
fn prelude_for(names: &[&str]) -> String {
    let all = snippets();
    let mut want = vec![false; all.len()];
    let mut stack: Vec<&str> = names.to_vec();
    while let Some(n) = stack.pop() {
        let Some(k) = all.iter().position(|s| s.name == n) else { panic!("GLSL の下請け {n} が無い") };
        if !want[k] {
            want[k] = true;
            stack.extend(all[k].deps.iter().copied());
        }
    }
    let mut s = String::new();
    for (k, sn) in all.iter().enumerate() {
        if want[k] {
            s.push_str(&sn.code);
            if !sn.code.ends_with('\n') {
                s.push('\n');
            }
        }
    }
    s
}

fn snip(name: &'static str, deps: &'static [&'static str], code: &str) -> Snip {
    Snip { name, deps, code: code.trim_start_matches('\n').to_string() }
}

/// 飾り付きの区間の 1 引数の関数。`unb` は引数の区間が有界でないときの結果、`body` は `vec2 x` から結果の `Div` を返す文
fn dec1(name: &'static str, deps: &'static [&'static str], unb: &str, body: &str) -> Snip {
    let code = format!(
        "Div {name}(Div a) {{\n  if (gi_none(a)) return gi_empty();\n  if (!gi_bd(a)) return gi_wk({unb}, a.d);\n  vec2 x = vec2(a.lo, a.hi);\n{body}\n}}\n"
    );
    Snip { name, deps, code }
}

/// 2 引数の版（`vec2 x`・`vec2 y`）
fn dec2(name: &'static str, deps: &'static [&'static str], unb: &str, body: &str) -> Snip {
    let code = format!(
        "Div {name}(Div a, Div b) {{\n  if (gi_none(a) || gi_none(b)) return gi_empty();\n  int inh = min(a.d, b.d);\n  if (!gi_bd(a) || !gi_bd(b)) return gi_wk({unb}, inh);\n  vec2 x = vec2(a.lo, a.hi);\n  vec2 y = vec2(b.lo, b.hi);\n{body}\n}}\n"
    );
    Snip { name, deps, code }
}

/// 下請けの関数の表（前提が先に来る順）。
/// `gp_`: 共通、`gf_`: 値、`gd_`: 二重数、`gw_`: 飾りの無い区間（Rust の `interval::*` と同じ。`ok` が false は「分からない」）、
/// `gi_`: 飾り付きの区間（Rust の `Code::interval_dec` の命令ごとの場合分けと同じ）
fn snippets() -> &'static [Snip] {
    static ALL: std::sync::OnceLock<Vec<Snip>> = std::sync::OnceLock::new();
    ALL.get_or_init(build_snippets)
}

fn build_snippets() -> Vec<Snip> {
    // 1 引数の、値と導関数（二重数）。(名前, 値の式, 導関数の式)。x は引数の値
    const D1: &[(&str, &str, &str)] = &[
        ("sin", "gp_sin(x)", "gp_cos(x)"),
        ("cos", "gp_cos(x)", "-gp_sin(x)"),
        ("tan", "gp_tan(x)", "1.0 + gp_tan(x) * gp_tan(x)"),
        ("sec", "1.0 / gp_cos(x)", "gp_sin(x) / (gp_cos(x) * gp_cos(x))"),
        ("csc", "1.0 / gp_sin(x)", "-gp_cos(x) / (gp_sin(x) * gp_sin(x))"),
        ("cot", "gp_cos(x) / gp_sin(x)", "-1.0 / (gp_sin(x) * gp_sin(x))"),
        ("asin", "gf_asin(x)", "1.0 / sqrt(1.0 - x * x)"),
        ("acos", "gf_acos(x)", "-1.0 / sqrt(1.0 - x * x)"),
        ("atan", "atan(x)", "1.0 / (1.0 + x * x)"),
        ("sinh", "sinh(x)", "cosh(x)"),
        ("cosh", "cosh(x)", "sinh(x)"),
        ("tanh", "gf_tanh(x)", "1.0 - gf_tanh(x) * gf_tanh(x)"),
        ("asinh", "asinh(x)", "1.0 / sqrt(x * x + 1.0)"),
        ("acosh", "gf_acosh(x)", "1.0 / sqrt(x * x - 1.0)"),
        ("atanh", "gf_atanh(x)", "1.0 / (1.0 - x * x)"),
        ("exp", "exp(x)", "exp(x)"),
        ("ln", "gf_ln(x)", "1.0 / x"),
        ("log10", "gf_log10(x)", "0.4342944819032518 / x"),
        ("sqrt", "gf_sqrt(x)", "0.5 / sqrt(x)"),
        ("abs", "abs(x)", "sign(x)"),
        ("sinc", "gf_sinc(x)", "(abs(x) < 0.5 ? x * (x * x * (1.0 / 30.0 - x * x / 840.0) - 1.0 / 3.0) : (gp_cos(x) - gp_sin(x) / x) / x)"),
        ("floor", "floor(x)", "0.0"),
        ("ceil", "ceil(x)", "0.0"),
        ("round", "gf_round(x)", "0.0"),
        ("sign", "sign(x)", "0.0"),
        ("re", "x", "1.0"),
        ("im", "0.0", "0.0"),
    ];
    let mut v = vec![
        // --- 共通 ---
        snip("gp_inf", &[], "float gp_inf() { return uintBitsToFloat(0x7F800000u); }"),
        snip("gp_nan", &[], "float gp_nan() { return uintBitsToFloat(0x7FC00000u); }"),
        // 有限か（NaN・無限大で false。isnan は最適化で消える実装があるので、比べて決める）
        snip("gp_fin", &["gp_inf"], "bool gp_fin(float x) { return abs(x) < gp_inf(); }"),
        // 整数乗（2 進の繰り返し 2 乗。指数は 1024 以下）
        snip(
            "gp_powi",
            &[],
            r#"
float gp_powi(float x, int n) {
  int m = n < 0 ? -n : n;
  float r = 1.0;
  float b = x;
  for (int i = 0; i < 11; i++) {
    if ((m & 1) == 1) r *= b;
    m >>= 1;
    if (m == 0) break;
    b *= b;
  }
  return n < 0 ? 1.0 / r : r;
}"#,
        ),
        // sin・cos（組み込みは GPU しだいで誤差が大きい。SwiftShader では絶対誤差 4e-4）。Cephes の sinf と同じ:
        // π/4 の偶数倍に寄せて 3 つに分けた π/4 で引き（|x| < 5 万くらいまで正確）、[−π/4, π/4] の多項式で計算する
        snip(
            "gp_sc",
            &[],
            r#"
vec2 gp_sc(float x) {
  float y = abs(x);
  float j = floor(y * 1.2732395447351628);
  j += mod(j, 2.0);
  float r = ((y - j * 0.78515625) - j * 2.4187564849853515625e-4) - j * 3.77489497744594108e-8;
  float z = r * r;
  float s = ((-1.9515295891e-4 * z + 8.3321608736e-3) * z - 1.6666654611e-1) * z * r + r;
  float c = ((2.443315711809948e-5 * z - 1.388731625493765e-3) * z + 4.166664568298827e-2) * z * z - 0.5 * z + 1.0;
  int n = int(j * 0.5) & 3;
  vec2 o = n == 0 ? vec2(s, c) : n == 1 ? vec2(c, -s) : n == 2 ? vec2(-s, -c) : vec2(-c, s);
  if (x < 0.0) o.x = -o.x;
  return o;
}
float gp_sin(float x) { return gp_sc(x).x; }
float gp_cos(float x) { return gp_sc(x).y; }
float gp_tan(float x) { vec2 o = gp_sc(x); return o.x / o.y; }"#,
        ),
        // --- 値（実数だけの道と同じ: 実数にならない所は NaN） ---
        snip("gf_asin", &["gp_nan"], "float gf_asin(float x) { return abs(x) <= 1.0 ? asin(x) : gp_nan(); }"),
        snip("gf_acos", &["gp_nan"], "float gf_acos(float x) { return abs(x) <= 1.0 ? acos(x) : gp_nan(); }"),
        snip("gf_acosh", &["gp_nan"], "float gf_acosh(float x) { return x >= 1.0 ? acosh(x) : gp_nan(); }"),
        snip(
            "gf_atanh",
            &["gp_nan", "gp_inf"],
            "float gf_atanh(float x) { return abs(x) < 1.0 ? atanh(x) : (abs(x) == 1.0 ? x * gp_inf() : gp_nan()); }",
        ),
        // tanh は組み込みだと大きい x で NaN になる実装があるので、exp(−2|x|) で書く
        snip("gf_tanh", &[], "float gf_tanh(float x) { float e = exp(-2.0 * abs(x)); return sign(x) * (1.0 - e) / (1.0 + e); }"),
        snip("gf_ln", &["gp_nan", "gp_inf"], "float gf_ln(float x) { return x > 0.0 ? log(x) : (x == 0.0 ? -gp_inf() : gp_nan()); }"),
        snip("gf_log10", &["gf_ln"], "float gf_log10(float x) { return gf_ln(x) * 0.4342944819032518; }"),
        snip("gf_sqrt", &["gp_nan"], "float gf_sqrt(float x) { return x >= 0.0 ? sqrt(x) : gp_nan(); }"),
        snip("gf_sinc", &["gp_sc"], "float gf_sinc(float x) { return abs(x) < 1e-4 ? 1.0 - x * x / 6.0 : gp_sin(x) / x; }"),
        // 0.5 は 0 から遠い側へ（Rust の round と同じ。組み込みの round は実装しだい）
        snip(
            "gf_round",
            &[],
            "float gf_round(float x) { float a = abs(x); float t = floor(a); if (a - t >= 0.5) t += 1.0; return sign(x) * t; }",
        ),
        snip("gf_sin", &["gp_sc"], "float gf_sin(float x) { return gp_sin(x); }"),
        snip("gf_cos", &["gp_sc"], "float gf_cos(float x) { return gp_cos(x); }"),
        snip("gf_tan", &["gp_sc"], "float gf_tan(float x) { return gp_tan(x); }"),
        snip("gf_sec", &["gp_sc"], "float gf_sec(float x) { return 1.0 / gp_cos(x); }"),
        snip("gf_csc", &["gp_sc"], "float gf_csc(float x) { return 1.0 / gp_sin(x); }"),
        snip("gf_cot", &["gp_sc"], "float gf_cot(float x) { vec2 o = gp_sc(x); return o.y / o.x; }"),
        snip("gf_re", &[], "float gf_re(float x) { return x; }"),
        snip("gf_im", &[], "float gf_im(float x) { return 0.0; }"),
        // 実数乗（0 以上の底だけ。0 の負の数乗は定義しない）
        snip(
            "gf_powc",
            &["gp_nan"],
            "float gf_powc(float x, float e) { return x > 0.0 ? pow(x, e) : (x == 0.0 && e > 0.0 ? 0.0 : gp_nan()); }",
        ),
        // 指数も動く累乗（整数乗か、0 以上の底の実数乗）
        snip(
            "gf_pow",
            &["gp_powi", "gf_powc"],
            "float gf_pow(float x, float y) { return (fract(y) == 0.0 && abs(y) <= 1024.0) ? gp_powi(x, int(y)) : gf_powc(x, y); }",
        ),
        // --- 二重数 ---
        snip("gd_mul", &[], "vec4 gd_mul(vec4 a, vec4 b) { return vec4(a.x * b.x, a.x * b.yzw + b.x * a.yzw); }"),
        snip("gd_div", &[], "vec4 gd_div(vec4 a, vec4 b) { return vec4(a.x / b.x, (a.yzw * b.x - a.x * b.yzw) / (b.x * b.x)); }"),
        snip(
            "gd_powi",
            &["gp_powi"],
            "vec4 gd_powi(vec4 a, int e) { return vec4(gp_powi(a.x, e), e == 0 ? vec3(0.0) : float(e) * gp_powi(a.x, e - 1) * a.yzw); }",
        ),
        snip("gd_powc", &["gf_powc"], "vec4 gd_powc(vec4 a, float e) { return vec4(gf_powc(a.x, e), e * gf_powc(a.x, e - 1.0) * a.yzw); }"),
        snip(
            "gd_pow",
            &["gf_pow"],
            r#"
vec4 gd_pow(vec4 a, vec4 b) {
  float v = gf_pow(a.x, b.x);
  vec3 g = b.x * gf_pow(a.x, b.x - 1.0) * a.yzw;
  if (b.yzw != vec3(0.0)) g += v * log(abs(a.x)) * b.yzw;
  return vec4(v, g);
}"#,
        ),
        snip("gd_mod", &[], "vec4 gd_mod(vec4 a, vec4 b) { float k = floor(a.x / b.x); return vec4(a.x - b.x * k, a.yzw - k * b.yzw); }"),
        // --- 飾りの無い区間（Rust の interval.rs と同じ。ok が false は「分からない」） ---
        snip(
            "gw_core",
            &["gp_fin"],
            r#"
struct Gv { float lo; float hi; bool ok; };
const float GP_P = 9.5367431640625e-7;
const float GP_PT = 1.621246337890625e-5;
const float GP_PI = 3.141592653589793;
const float GP_TAU = 6.283185307179586;
Gv gw_no() { return Gv(0.0, 0.0, false); }
Gv gw_w(float lo, float hi, float pad) {
  if (!(gp_fin(lo) && gp_fin(hi)) || lo > hi) return gw_no();
  return Gv(lo - pad * (abs(lo) + 1.0), hi + pad * (abs(hi) + 1.0), true);
}
bool gw_has(vec2 a, float x) { return a.x <= x && x <= a.y; }
vec2 gw_v(Gv r) { return vec2(r.lo, r.hi); }
Gv gw_nonneg(Gv r) { r.lo = max(r.lo, 0.0); return r; }
Gv gw_add(vec2 a, vec2 b) { return gw_w(a.x + b.x, a.y + b.y, GP_P); }
Gv gw_sub(vec2 a, vec2 b) { return gw_w(a.x - b.y, a.y - b.x, GP_P); }
Gv gw_mul(vec2 a, vec2 b) {
  vec4 q = vec4(a.x * b.x, a.x * b.y, a.y * b.x, a.y * b.y);
  return gw_w(min(min(q.x, q.y), min(q.z, q.w)), max(max(q.x, q.y), max(q.z, q.w)), GP_P);
}
Gv gw_recip(vec2 b) { if (gw_has(b, 0.0)) return gw_no(); return gw_w(1.0 / b.y, 1.0 / b.x, GP_P); }
Gv gw_div(vec2 a, vec2 b) { Gv r = gw_recip(b); if (!r.ok) return r; return gw_mul(a, gw_v(r)); }"#,
        ),
        snip(
            "gw_pow",
            &["gw_core", "gp_powi"],
            r#"
Gv gw_powp(vec2 a, int e) {
  float l = gp_powi(a.x, e);
  float h = gp_powi(a.y, e);
  if ((e & 1) == 1) return gw_w(l, h, GP_PT);
  if (gw_has(a, 0.0)) return gw_nonneg(gw_w(0.0, max(l, h), GP_PT));
  return gw_nonneg(gw_w(min(l, h), max(l, h), GP_PT));
}
Gv gw_powi(vec2 a, int e) {
  if (e == 0) return Gv(1.0, 1.0, true);
  if (e > 0) return gw_powp(a, e);
  Gv r = gw_powp(a, -e);
  if (!r.ok) return r;
  return gw_recip(gw_v(r));
}
Gv gw_pow(vec2 a, vec2 b) {
  if (b.x == b.y) {
    float e = b.x;
    if (fract(e) == 0.0 && abs(e) <= 1024.0) return gw_powi(a, int(e));
    if (a.x < 0.0 || (e < 0.0 && a.x <= 0.0)) return gw_no();
    if (e > 0.0) return gw_w(pow(a.x, e), pow(a.y, e), GP_PT);
    return gw_w(pow(a.y, e), pow(a.x, e), GP_PT);
  }
  if (a.x <= 0.0) return gw_no();
  Gv l = gw_w(log(a.x), log(a.y), GP_PT);
  if (!l.ok) return l;
  Gv m = gw_mul(b, gw_v(l));
  if (!m.ok) return m;
  return gw_w(exp(m.lo), exp(m.hi), GP_PT);
}"#,
        ),
        snip(
            "gw_trig",
            &["gw_core", "gp_sc"],
            r#"
bool gw_hasc(vec2 a, float c) { return ceil((a.x - c) / GP_TAU) * GP_TAU + c <= a.y + GP_PT * (abs(a.y) + 1.0); }
Gv gw_sin(vec2 a) {
  if (a.y - a.x >= GP_TAU) return Gv(-1.0, 1.0, true);
  float s0 = gp_sin(a.x);
  float s1 = gp_sin(a.y);
  float hi = gw_hasc(a, 0.5 * GP_PI) ? 1.0 : max(s0, s1);
  float lo = gw_hasc(a, -0.5 * GP_PI) ? -1.0 : min(s0, s1);
  return gw_w(lo, hi, GP_PT);
}
Gv gw_cos(vec2 a) {
  if (a.y - a.x >= GP_TAU) return Gv(-1.0, 1.0, true);
  float c0 = gp_cos(a.x);
  float c1 = gp_cos(a.y);
  float hi = gw_hasc(a, 0.0) ? 1.0 : max(c0, c1);
  float lo = gw_hasc(a, GP_PI) ? -1.0 : min(c0, c1);
  return gw_w(lo, hi, GP_PT);
}
bool gw_pole(vec2 a, float c) {
  float k = ceil((a.x - c) / GP_PI);
  return k * GP_PI + c <= a.y + GP_PT * (abs(a.y) + 1.0) || (k - 1.0) * GP_PI + c >= a.x - GP_PT * (abs(a.x) + 1.0);
}
Gv gw_tan(vec2 a) { if (a.y - a.x >= GP_PI || gw_pole(a, 0.5 * GP_PI)) return gw_no(); return gw_w(gp_tan(a.x), gp_tan(a.y), GP_PT); }
Gv gw_cot(vec2 a) { if (a.y - a.x >= GP_PI || gw_pole(a, 0.0)) return gw_no(); vec2 l = gp_sc(a.x); vec2 h = gp_sc(a.y); return gw_w(h.y / h.x, l.y / l.x, GP_PT); }
Gv gw_sec(vec2 a) { Gv c = gw_cos(a); if (!c.ok) return c; return gw_recip(gw_v(c)); }
Gv gw_csc(vec2 a) { Gv s = gw_sin(a); if (!s.ok) return s; return gw_recip(gw_v(s)); }"#,
        ),
        // --- 飾り付きの区間 ---
        snip(
            "gi_core",
            &["gw_core", "gp_inf"],
            r#"
struct Div { float lo; float hi; int d; };
Div gi_entire(int d) { return Div(-gp_inf(), gp_inf(), min(d, 2)); }
Div gi_empty() { return Div(gp_inf(), -gp_inf(), 0); }
bool gi_none(Div a) { return !(a.lo <= a.hi); }
bool gi_bd(Div a) { return gp_fin(a.lo) && gp_fin(a.hi); }
Div gi_wk(Div r, int d) { r.d = min(r.d, d); return r; }
Div gi_of(Gv r, int d, int dn) { if (!r.ok) return gi_entire(dn); return Div(r.lo, r.hi, d); }"#,
        ),
        snip(
            "gi_neg",
            &["gi_core"],
            r#"
Div gi_neg(Div a) {
  if (gi_none(a)) return gi_empty();
  if (!gi_bd(a)) return gi_wk(gi_entire(2), a.d);
  return gi_wk(gi_of(gw_w(-a.hi, -a.lo, GP_P), 3, 2), a.d);
}"#,
        ),
        dec2("gi_add", &["gi_core"], "gi_entire(2)", "  return gi_wk(gi_of(gw_add(x, y), 3, 2), inh);"),
        dec2("gi_sub", &["gi_core"], "gi_entire(2)", "  return gi_wk(gi_of(gw_sub(x, y), 3, 2), inh);"),
        dec2("gi_mul", &["gi_core"], "gi_entire(2)", "  return gi_wk(gi_of(gw_mul(x, y), 3, 2), inh);"),
        dec2("gi_div", &["gi_core"], "gi_entire(0)", "  if (gw_has(y, 0.0)) return gi_wk(gi_entire(0), inh);\n  return gi_wk(gi_of(gw_div(x, y), 3, 2), inh);"),
        dec2(
            "gi_pow",
            &["gi_core", "gw_pow"],
            "gi_entire(0)",
            r#"  Div r;
  if (y.x == y.y && fract(y.x) == 0.0 && abs(y.x) <= 1024.0) {
    if (y.x < 0.0 && gw_has(x, 0.0)) r = gi_entire(0); else r = gi_of(gw_pow(x, y), 3, 2);
  } else if (y.x == y.y) {
    float e = y.x;
    if (x.y < 0.0 || (e < 0.0 && x.y <= 0.0)) r = gi_empty();
    else if (x.x < 0.0 || (e < 0.0 && x.x <= 0.0)) { if (e > 0.0) r = gi_of(gw_pow(vec2(0.0, x.y), y), 0, 0); else r = gi_entire(0); }
    else r = gi_of(gw_pow(x, y), 3, 2);
  } else if (x.y <= 0.0) {
    r = gi_empty();
  } else if (x.x <= 0.0) {
    r = gi_entire(0);
  } else {
    r = gi_of(gw_pow(x, y), 3, 2);
  }
  return gi_wk(r, inh);"#,
        ),
        dec2("gi_min", &["gi_core"], "gi_entire(2)", "  return gi_wk(gi_of(gw_w(min(x.x, y.x), min(x.y, y.y), GP_P), 3, 2), inh);"),
        dec2("gi_max", &["gi_core"], "gi_entire(2)", "  return gi_wk(gi_of(gw_w(max(x.x, y.x), max(x.y, y.y), GP_P), 3, 2), inh);"),
        dec2(
            "gi_mod",
            &["gi_core"],
            "gi_entire(0)",
            r#"  if (gw_has(y, 0.0)) return gi_wk(gi_entire(0), inh);
  Gv q = gw_div(x, y);
  if (!q.ok) return gi_wk(gi_entire(0), inh);
  float k0 = floor(q.lo);
  float k1 = floor(q.hi);
  if (k0 == k1) {
    Gv kb = gw_mul(y, vec2(k0));
    if (kb.ok) kb = gw_sub(x, gw_v(kb));
    return gi_wk(gi_of(kb, 3, 2), inh);
  }
  Gv i;
  if (y.x > 0.0) i = gw_w(0.0, y.y, GP_P); else i = gw_w(y.x, 0.0, GP_P);
  return gi_wk(gi_of(i, 1, 1), inh);"#,
        ),
        dec1("gi_sin", &["gi_core", "gw_trig"], "Div(-1.0, 1.0, 3)", "  return gi_wk(gi_of(gw_sin(x), 3, 3), a.d);"),
        dec1("gi_cos", &["gi_core", "gw_trig"], "Div(-1.0, 1.0, 3)", "  return gi_wk(gi_of(gw_cos(x), 3, 3), a.d);"),
        dec1("gi_tan", &["gi_core", "gw_trig"], "gi_entire(0)", "  return gi_wk(gi_of(gw_tan(x), 3, 0), a.d);"),
        dec1("gi_cot", &["gi_core", "gw_trig"], "gi_entire(0)", "  return gi_wk(gi_of(gw_cot(x), 3, 0), a.d);"),
        dec1("gi_sec", &["gi_core", "gw_trig"], "gi_entire(0)", "  return gi_wk(gi_of(gw_sec(x), 3, 0), a.d);"),
        dec1("gi_csc", &["gi_core", "gw_trig"], "gi_entire(0)", "  return gi_wk(gi_of(gw_csc(x), 3, 0), a.d);"),
        dec1("gi_atan", &["gi_core"], "Div(-1.5707963267948966, 1.5707963267948966, 3)", "  return gi_wk(gi_of(gw_w(atan(x.x), atan(x.y), GP_PT), 3, 2), a.d);"),
        dec1(
            "gi_tanh",
            &["gi_core", "gf_tanh"],
            "Div(-1.5707963267948966, 1.5707963267948966, 3)",
            "  return gi_wk(gi_of(gw_w(gf_tanh(x.x), gf_tanh(x.y), GP_PT), 3, 2), a.d);",
        ),
        dec1("gi_sinh", &["gi_core"], "gi_entire(2)", "  return gi_wk(gi_of(gw_w(sinh(x.x), sinh(x.y), GP_PT), 3, 2), a.d);"),
        dec1("gi_asinh", &["gi_core"], "gi_entire(2)", "  return gi_wk(gi_of(gw_w(asinh(x.x), asinh(x.y), GP_PT), 3, 2), a.d);"),
        dec1("gi_exp", &["gi_core"], "gi_entire(2)", "  return gi_wk(gi_of(gw_w(exp(x.x), exp(x.y), GP_PT), 3, 2), a.d);"),
        dec1(
            "gi_cosh",
            &["gi_core"],
            "gi_entire(2)",
            r#"  float l = cosh(x.x);
  float h = cosh(x.y);
  Gv r;
  if (gw_has(x, 0.0)) r = gw_w(1.0, max(l, h), GP_PT); else r = gw_w(min(l, h), max(l, h), GP_PT);
  return gi_wk(gi_of(r, 3, 2), a.d);"#,
        ),
        dec1(
            "gi_abs",
            &["gi_core"],
            "gi_entire(2)",
            r#"  float l = abs(x.x);
  float h = abs(x.y);
  Gv r;
  if (gw_has(x, 0.0)) r = gw_w(0.0, max(l, h), GP_P); else r = gw_w(min(l, h), max(l, h), GP_P);
  r = gw_nonneg(r);
  return gi_wk(gi_of(r, 3, 2), a.d);"#,
        ),
        dec1("gi_sinc", &["gi_core"], "gi_entire(2)", "  return gi_wk(Div(-0.22, 1.0, 3), a.d);"),
        dec1(
            "gi_asin",
            &["gi_core"],
            "gi_entire(0)",
            r#"  if (x.y < -1.0 || x.x > 1.0) return gi_empty();
  bool part = x.x < -1.0 || x.y > 1.0;
  vec2 c = clamp(x, -1.0, 1.0);
  return gi_wk(gi_of(gw_w(asin(c.x), asin(c.y), GP_PT), part ? 0 : 3, part ? 0 : 2), a.d);"#,
        ),
        dec1(
            "gi_acos",
            &["gi_core"],
            "gi_entire(0)",
            r#"  if (x.y < -1.0 || x.x > 1.0) return gi_empty();
  bool part = x.x < -1.0 || x.y > 1.0;
  vec2 c = clamp(x, -1.0, 1.0);
  return gi_wk(gi_of(gw_w(acos(c.y), acos(c.x), GP_PT), part ? 0 : 3, part ? 0 : 2), a.d);"#,
        ),
        dec1(
            "gi_acosh",
            &["gi_core"],
            "gi_entire(0)",
            r#"  if (x.y < 1.0) return gi_empty();
  bool part = x.x < 1.0;
  return gi_wk(gi_of(gw_w(acosh(max(x.x, 1.0)), acosh(x.y), GP_PT), part ? 0 : 3, part ? 0 : 2), a.d);"#,
        ),
        dec1(
            "gi_atanh",
            &["gi_core"],
            "gi_entire(0)",
            r#"  if (x.y <= -1.0 || x.x >= 1.0) return gi_empty();
  if (x.x <= -1.0 || x.y >= 1.0) return gi_wk(gi_entire(0), a.d);
  return gi_wk(gi_of(gw_w(atanh(x.x), atanh(x.y), GP_PT), 3, 2), a.d);"#,
        ),
        dec1(
            "gi_ln",
            &["gi_core"],
            "gi_entire(0)",
            r#"  if (x.y <= 0.0) return gi_empty();
  if (x.x <= 0.0) { Gv t = gw_w(log(x.y), log(x.y), GP_PT); return gi_wk(Div(-gp_inf(), t.ok ? t.hi : gp_inf(), 0), a.d); }
  return gi_wk(gi_of(gw_w(log(x.x), log(x.y), GP_PT), 3, 2), a.d);"#,
        ),
        dec1(
            "gi_log10",
            &["gi_core"],
            "gi_entire(0)",
            r#"  const float L = 0.4342944819032518;
  if (x.y <= 0.0) return gi_empty();
  if (x.x <= 0.0) { Gv t = gw_w(L * log(x.y), L * log(x.y), GP_PT); return gi_wk(Div(-gp_inf(), t.ok ? t.hi : gp_inf(), 0), a.d); }
  return gi_wk(gi_of(gw_w(L * log(x.x), L * log(x.y), GP_PT), 3, 2), a.d);"#,
        ),
        // sqrt は 0 で定義される。丸めで少しだけ負になった下の端（命令ごとに広げた分）は 0 とみなす
        dec1(
            "gi_sqrt",
            &["gi_core"],
            "gi_entire(0)",
            r#"  float tol = 16.0 * GP_P * (abs(x.y) + 1.0);
  if (x.y < -tol) return gi_empty();
  Gv r = gw_w(sqrt(max(x.x, 0.0)), sqrt(max(x.y, 0.0)), GP_PT);
  if (x.x < -tol) return gi_wk(gi_of(r, 0, 0), a.d);
  return gi_wk(gi_of(r, 3, 2), a.d);"#,
        ),
        dec1(
            "gi_floor",
            &["gi_core"],
            "gi_entire(1)",
            r#"  float l = floor(x.x);
  float h = floor(x.y);
  return gi_wk(gi_of(gw_w(l, h, GP_P), l == h ? 3 : 1, 1), a.d);"#,
        ),
        dec1(
            "gi_ceil",
            &["gi_core"],
            "gi_entire(1)",
            r#"  float l = ceil(x.x);
  float h = ceil(x.y);
  return gi_wk(gi_of(gw_w(l, h, GP_P), l == h ? 3 : 1, 1), a.d);"#,
        ),
        dec1(
            "gi_round",
            &["gi_core", "gf_round"],
            "gi_entire(1)",
            r#"  float l = gf_round(x.x);
  float h = gf_round(x.y);
  return gi_wk(gi_of(gw_w(l, h, GP_P), l == h ? 3 : 1, 1), a.d);"#,
        ),
        dec1(
            "gi_sign",
            &["gi_core"],
            "gi_entire(1)",
            r#"  bool cont = x.x > 0.0 || x.y < 0.0;
  return gi_wk(gi_of(gw_w(sign(x.x), sign(x.y), GP_P), cont ? 3 : 1, 1), a.d);"#,
        ),
        dec1("gi_re", &["gi_core"], "gi_entire(2)", "  return gi_wk(Div(x.x, x.y, 3), a.d);"),
        dec1("gi_im", &["gi_core"], "gi_entire(2)", "  return gi_wk(Div(0.0, 0.0, 3), a.d);"),
    ];
    // 二重数の 1 引数の関数（値の式が gf_ を使うものは、その前提を付ける）
    for &(n, val, der) in D1 {
        let name: &'static str = Box::leak(format!("gd_{n}").into_boxed_str());
        let mut deps: Vec<&'static str> = Vec::new();
        if val.contains("gp_") || der.contains("gp_") {
            deps.push("gp_sc");
        }
        for s in ["gf_asin", "gf_acos", "gf_tanh", "gf_acosh", "gf_atanh", "gf_ln", "gf_log10", "gf_sqrt", "gf_sinc", "gf_round"] {
            if val.contains(&format!("{s}(")) || der.contains(&format!("{s}(")) {
                deps.push(s);
            }
        }
        let deps: &'static [&'static str] = Box::leak(deps.into_boxed_slice());
        v.push(Snip { name, deps, code: format!("vec4 {name}(vec4 a) {{ float x = a.x; return vec4({val}, ({der}) * a.yzw); }}\n") });
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn prog(src: &str, vars: &[&str]) -> Program {
        Program::compile(&parse(src).unwrap(), vars).unwrap()
    }

    /// 呼んでいる下請けの関数（`gp_`・`gf_`・`gd_`・`gw_`・`gi_` で始まる名前）が、全部どこかで定義されているか
    fn assert_defined(src: &str) {
        let bytes = src.as_bytes();
        let mut i = 0;
        while i + 3 < bytes.len() {
            let at_word = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
            if at_word && bytes[i] == b'g' && b"pfdwi".contains(&bytes[i + 1]) && bytes[i + 2] == b'_' {
                let end = (i..bytes.len()).find(|&j| !(bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_')).unwrap_or(bytes.len());
                let name = &src[i..end];
                if bytes.get(end) == Some(&b'(') {
                    let defined = ["float", "vec2", "vec4", "bool", "Gv", "Div"].iter().any(|t| src.contains(&format!("{t} {name}(")));
                    assert!(defined, "{name} が定義されていない");
                }
                i = end;
            } else {
                i += 1;
            }
        }
    }

    #[test]
    fn value_function_shape() {
        let g = prog("x^2 + y^2 - 1", &["x", "y"]).to_glsl(&GlslOptions::default()).unwrap();
        assert_eq!(
            g.value,
            "float F(float p[2]) {\n  float v0 = p[0];\n  float v1 = 2.0;\n  float v2 = gp_powi(v0, 2);\n  float v3 = p[1];\n  float v4 = gp_powi(v3, 2);\n  float v5 = v2 + v4;\n  float v6 = 1.0;\n  float v7 = v5 - v6;\n  return v7;\n}\n"
        );
        assert!(g.dual.starts_with("vec4 Fd(vec4 p[2]) {\n"), "{}", g.dual);
        assert!(g.dual.contains("vec4 d2 = gd_powi(d0, 2);"), "{}", g.dual);
        assert!(g.interval.starts_with("Div Fi(vec2 p[2]) {\n"), "{}", g.interval);
        assert!(g.interval.contains("Div r0 = Div(p[0].x, p[0].y, 3);"), "{}", g.interval);
        assert!(g.interval.contains("Div r7 = gi_sub(r5, r6);"), "{}", g.interval);
        assert_eq!((g.inputs, g.params), (2, 0));
        assert_eq!(g.uniforms, "");
        // 使わない下請けは入れない
        assert!(!g.prelude.contains("gi_sin") && !g.prelude.contains("gp_sc"));
        assert_defined(&g.source());
    }

    #[test]
    fn params_become_uniforms() {
        // a は uniform。定数に畳み込まれないので、a の値を変えても GLSL は同じ
        let p = prog("a x^2 + sin(b y) - z", &["x", "y", "z", "a", "b"]);
        let opts = GlslOptions { params: vec![3, 4], ..Default::default() };
        let g = p.to_glsl(&opts).unwrap();
        assert_eq!(g.uniforms, "uniform float uParam[2];\n");
        assert_eq!((g.inputs, g.params), (3, 2));
        assert!(g.value.contains("= uParam[0];") && g.value.contains("= uParam[1];"), "{}", g.value);
        assert!(g.dual.contains("vec4(uParam[0], 0.0, 0.0, 0.0)"), "{}", g.dual);
        assert!(g.interval.contains("Div(uParam[1], uParam[1], 3)"), "{}", g.interval);
        assert!(g.value.starts_with("float F(float p[3])"));
        // 名前を変えられる（1 つのシェーダーに式をいくつも入れるとき）
        let h = p.to_glsl(&GlslOptions { name: "G2".into(), param_uniform: "uP".into(), ..opts }).unwrap();
        assert!(h.value.starts_with("float G2(") && h.dual.starts_with("vec4 G2d(") && h.interval.starts_with("Div G2i("));
        assert_eq!(h.uniforms, "uniform float uP[2];\n");
        assert_defined(&h.source());
    }

    #[test]
    fn constants_are_f32_literals_and_intervals_contain_them() {
        let g = prog("x - 0.1", &["x"]).to_glsl(&GlslOptions::default()).unwrap();
        assert!(g.value.contains("float v1 = 0.1;"), "{}", g.value);
        // 0.1 は f32 で表せないので、区間は f32 の隣どうしの 2 つの数で挟む
        let lo = 0.1f32;
        let hi = next_up(lo);
        assert!((lo as f64) < 0.1 || (hi as f64) > 0.1);
        assert!(g.interval.contains("Div r1 = Div("), "{}", g.interval);
        let g = prog("x + 10^400", &["x"]).to_glsl(&GlslOptions::default()).unwrap();
        assert!(g.value.contains("gp_inf()"), "{}", g.value);
        assert!(g.interval.contains("gi_entire(0)"), "{}", g.interval);
        assert_eq!(lit(-2.5), "(-2.5)");
        assert_eq!(lit(3.0), "3.0");
        assert_eq!(lit(1e-30), "1e-30");
        assert_eq!(lit(1e300), "gp_inf()");
    }

    #[test]
    fn exponents_choose_the_same_branch_as_the_cpu() {
        let g = prog("x^3 + y^0.5 + x^y", &["x", "y"]).to_glsl(&GlslOptions::default()).unwrap();
        assert!(g.value.contains("gp_powi(v0, 3)"), "{}", g.value);
        assert!(g.value.contains("gf_powc(v3, 0.5)"), "{}", g.value);
        assert!(g.value.contains("gf_pow(v0, v3)"), "{}", g.value);
        assert_defined(&g.source());
    }

    #[test]
    fn unsupported_functions_are_errors() {
        let e = prog("gamma(x) - y", &["x", "y"]).to_glsl(&GlslOptions::default()).unwrap_err();
        assert_eq!(e, "gamma は GPU の描き方では使えません");
        let e = prog("besselJ(0, x) - y", &["x", "y"]).to_glsl(&GlslOptions::default()).unwrap_err();
        assert_eq!(e, "besselJ は GPU の描き方では使えません");
        let e = prog("x + i y", &["x", "y"]).to_glsl(&GlslOptions::default()).unwrap_err();
        assert!(e.contains("複素数"), "{e}");
        let e = prog("arg(x)", &["x"]).to_glsl(&GlslOptions::default()).unwrap_err();
        assert_eq!(e, "arg は GPU の描き方では使えません");
    }

    #[test]
    fn every_function_in_the_table_compiles_to_defined_helpers() {
        let srcs = [
            "sin(x) + cos(x) + tan(x) + sec(x) + csc(x) + cot(x)",
            "asin(x) + acos(x) + atan(x) + sinh(x) + cosh(x) + tanh(x)",
            "asinh(x) + acosh(x) + atanh(x) + exp(x) + ln(x) + log(x) + sqrt(x)",
            "abs(x) + sinc(x) + floor(x) + ceil(x) + round(x) + sign(x)",
            "Re(x) + Im(x) + conj(x) + min(x, y) + max(x, y) + mod(x, y)",
            "x / y - x * y + x^y + x^2.5 + x^(-3) - (-x)",
        ];
        for s in srcs {
            let g = prog(s, &["x", "y"]).to_glsl(&GlslOptions::default()).unwrap_or_else(|e| panic!("{s}: {e}"));
            assert_defined(&g.source());
        }
        let all = glsl_prelude_all();
        assert_defined(&all);
        // 下請けの名前は重ならない
        let names: Vec<&str> = snippets().iter().map(|s| s.name).collect();
        for (k, n) in names.iter().enumerate() {
            assert!(!names[k + 1..].contains(n), "{n} が 2 つある");
        }
    }

    #[test]
    fn huge_expressions_are_errors() {
        let src: Vec<String> = (1..=700).map(|k| format!("sin({k} x)")).collect();
        let e = prog(&src.join(" + "), &["x"]).to_glsl(&GlslOptions::default()).unwrap_err();
        assert!(e.starts_with("式が大きすぎて"), "{e}");
    }

    #[test]
    fn no_variables_still_has_an_array_argument() {
        // GLSL は長さ 0 の配列を書けない
        let g = prog("a + 1", &["a"]).to_glsl(&GlslOptions { params: vec![0], ..Default::default() }).unwrap();
        assert!(g.value.starts_with("float F(float p[1])"), "{}", g.value);
        assert_eq!(g.inputs, 0);
    }
}

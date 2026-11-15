//! GLSL の比べるテスト（計画 12 章の G3。`npm run test:gpu`）用: 式ごとに、作った GLSL と、乱数の点・箱での Rust の結果を JSON に書き出す。
//!
//! ```text
//! cargo run --release --example glsl_fixture -- <出力先.json>   （GLSL_ONE="式" で 1 式の GLSL だけを標準出力に）
//! ```
//!
//! - 点（1000 個。成分は ±5）: 値と、記号微分（[`graph_core::diff::diff`]）で求めた勾配。二重数の答え合わせを、別の作り方で行う
//! - 箱（1000 個。中心は ±5、幅は 0〜2。端は f32 で表せる数）: 箱の中の 24 点（角 8 つ + 乱数）の値の最小・最大、定義されない点があるか、
//!   Rust の飾り付きの区間の飾り。GLSL の区間がこの最小・最大を含むこと、飾りが Def 以上なら定義されない点が無いことを確かめる
//! - パラメータ `a` を使う式は、`a` を uniform にした GLSL と、その値（`a = 1.3`）

use graph_core::diff::diff;
use graph_core::eval::{GlslOptions, Program};
use graph_core::interval::{Dec, Interval};
use graph_core::parser::{parse_system, BinOp, Expr, Rel};
use num_complex::Complex64 as C;
use std::fmt::Write;

/// 比べる式。(式, 精度の確かめに使うか)。false は「32 ビットで誤差が大きくなる」のを見るための式（表に出すだけで、落とさない）
const CASES: &[(&str, bool)] = &[
    // interval.rs のテストの式（命令を全部使う）
    ("x^2+y^2+z^2-4", true),
    ("sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)", true),
    ("(sqrt(x^2+y^2)-3)^2+z^2-0.04", true),
    ("z (x^2+y^2) - 1", true),
    ("tan(x) - z", true),
    ("1/x - z", true),
    ("exp(-x^2) sin(5y) - z", true),
    ("sec(x) - z", true),
    ("csc(y) + x", true),
    ("cot(x) - y", true),
    ("asin(x/5) + acos(y/5) - z", true),
    ("atan(x y) - z", true),
    ("sinh(x) - cosh(y) + z", true),
    ("tanh(x z) - y", true),
    ("asinh(x) + acosh(abs(y) + 1) - z", true),
    ("atanh(x/6) - z", true),
    ("ln(x^2 + 1) - z", true),
    ("log(abs(x) + 0.5) + y", true),
    ("sqrt(abs(x y)) - z", true),
    ("sinc(x) - y", true),
    ("floor(x) + ceil(y) - z", true),
    ("round(x z) - y", true),
    ("sign(x) + sign(y - z)", true),
    ("min(x, y) - max(y, z)", true),
    ("x^3 - y^(-2) + z^0", true),
    ("x^(1/3) - z", true),
    ("abs(x)^1.5 - y", true),
    ("(x + 2)^(-0.5) - z", true),
    ("abs(x)^y - z", true),
    ("-x / (y - 0.3) + z", true),
    ("Re(x) + Im(y) + conj(z)", true),
    ("cos(3x) cos(3y) - z", true),
    ("sin(x)^2 + cos(y)^2 - 1 - z", true),
    ("exp(x) - y z", true),
    ("mod(x y, z) - 1", true),
    ("mod(x, 2) + y", true),
    ("x / (y z)", true),
    ("sqrt(x) + ln(y) - z", true),
    ("(x - 1)^(1.5) + asin(y/3)", true),
    // pixel_report の式（core/tests/pixel.rs）
    ("sqrt(x)+sqrt(y)+sqrt(z)=3", true),
    ("max(abs(x),max(abs(y),abs(z))) = 4", true),
    ("abs(x)+abs(y)+abs(z)=6", true),
    ("x^2+y^2 = z^2", true),
    ("x y z = 1", true),
    ("sin(x) sin(y) sin(z) = 0.001", true),
    ("x y = 0.0001", true),
    ("x^2 y^2 + y^2 z^2 + z^2 x^2 = 1", true),
    ("ln(x) + ln(y) + ln(z) = 1", true),
    ("tan(x) = z y", true),
    ("arcsin(x/5) + arcsin(y/5) = z", true),
    ("sin(40/(x^2+y^2+z^2+1)) = 0.5", true),
    ("x^(2/3) + y^(2/3) + z^(2/3) = 4", true),
    ("z^2 = sin(x) sin(y)", true),
    ("sqrt(36 - x^2 - y^2) - z = 0", true),
    ("1/(x y) - z = 0", true),
    ("(x^2+y^2+z^2-16)^2 = 0", true),
    ("x^2+y^2+z^2 = 0.0001", true),
    ("floor(x) = z", true),
    ("floor(x y) = z floor(z)", true),
    // パラメータ（uniform）
    ("a x^2 + y^2 - z^a", true),
    ("sin(a x) - y", true),
    // 32 ビットでは誤差が大きくなる式（計画 12.6 の 2。表に出すだけ）
    ("sin(100 x) - y", false),
    ("(x + 1000)^2 - (x + 999)^2 - 2 x", false),
    ("exp(x) exp(y) exp(z) - 1", false),
    ("x^8 + y^8 + z^8 - 1", false),
];

const PARAM_A: f64 = 1.3;

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// 陰関数の式は 左辺 − 右辺
fn field(src: &str) -> Expr {
    let chains = parse_system(src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
    let c = &chains[0];
    if c.rels == [Rel::Eq] {
        Expr::Bin(BinOp::Sub, Box::new(c.exprs[0].clone()), Box::new(c.exprs[1].clone()))
    } else {
        c.exprs[0].clone()
    }
}

fn num(v: f64) -> String {
    if v.is_finite() {
        format!("{v:e}")
    } else {
        "null".into()
    }
}

fn eval(p: &Program, pts: &[[f64; 4]]) -> Vec<f64> {
    let cols: Vec<Vec<f64>> = (0..p.vars().len()).map(|k| pts.iter().map(|q| q[k]).collect()).collect();
    let refs: Vec<&[f64]> = cols.iter().map(|c| &c[..]).collect();
    let mut out = vec![C::new(0.0, 0.0); pts.len()];
    p.eval_batch(&refs, &mut out);
    out.into_iter().map(graph_core::eval::as_real).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Ok(src) = std::env::var("GLSL_ONE") {
        let e = field(&src);
        let uses_a = e.uses_var("a");
        let vars: &[&str] = if uses_a { &["x", "y", "z", "a"] } else { &["x", "y", "z"] };
        let p = Program::compile(&e, vars).unwrap();
        let opts = GlslOptions { params: if uses_a { vec![3] } else { vec![] }, ..Default::default() };
        println!("{}", p.to_glsl(&opts).unwrap().source());
        return;
    }
    let path = args.get(1).expect("使い方: glsl_fixture <出力先.json>");
    let mut rng = Lcg(20261115);
    let mut js = String::from("[\n");
    for (ci, &(src, strict)) in CASES.iter().enumerate() {
        let e = field(src);
        let uses_a = e.uses_var("a");
        let vars: &[&str] = if uses_a { &["x", "y", "z", "a"] } else { &["x", "y", "z"] };
        let p = Program::compile(&e, vars).unwrap_or_else(|er| panic!("{src}: {er}"));
        let opts = GlslOptions { params: if uses_a { vec![3] } else { vec![] }, ..Default::default() };
        let g = p.to_glsl(&opts).unwrap_or_else(|er| panic!("{src}: {er}"));
        // 勾配の式（記号微分）
        let grads: Vec<Program> = ["x", "y", "z"].iter().map(|v| Program::compile(&diff(&e, v).unwrap(), vars).unwrap()).collect();

        // 点
        let n = 1000;
        let pts: Vec<[f64; 4]> = (0..n)
            .map(|_| {
                let r = |rng: &mut Lcg| ((10.0 * rng.next() - 5.0) as f32) as f64;
                [r(&mut rng), r(&mut rng), r(&mut rng), PARAM_A]
            })
            .collect();
        let vals = eval(&p, &pts);
        let gs: Vec<Vec<f64>> = grads.iter().map(|q| eval(q, &pts)).collect();

        // 箱
        let mut boxes = Vec::with_capacity(n);
        let mut stats = Vec::with_capacity(n);
        for _ in 0..n {
            let b: Vec<Interval> = (0..3)
                .map(|_| {
                    let (c, w) = (10.0 * rng.next() - 5.0, 2.0 * rng.next() * rng.next());
                    let lo = ((c - w / 2.0) as f32) as f64;
                    let hi = ((c + w / 2.0) as f32) as f64;
                    Interval::new(lo, hi.max(lo))
                })
                .collect();
            let mut inside: Vec<[f64; 4]> = Vec::with_capacity(24);
            for k in 0..24 {
                let mut q = [0.0, 0.0, 0.0, PARAM_A];
                for (ax, iv) in b.iter().enumerate() {
                    let t = if k < 8 { ((k >> ax) & 1) as f64 } else { rng.next() };
                    q[ax] = iv.lo + (iv.hi - iv.lo) * t;
                }
                inside.push(q);
            }
            let vs = eval(&p, &inside);
            let (mut mn, mut mx, mut undef) = (f64::INFINITY, f64::NEG_INFINITY, false);
            for v in vs {
                if v.is_finite() {
                    mn = mn.min(v);
                    mx = mx.max(v);
                } else {
                    undef = true;
                }
            }
            let mut bx: Vec<Interval> = b.clone();
            if uses_a {
                bx.push(Interval::point(PARAM_A));
            }
            let d = p.eval_interval_dec(&bx);
            let dec = match d.dec {
                Dec::Trv => 0,
                Dec::Def => 1,
                Dec::Dac => 2,
                Dec::Com => 3,
            };
            boxes.push(b);
            stats.push((mn, mx, undef, dec, d.lo, d.hi));
        }

        let mut s = String::new();
        let _ = write!(s, "{{\"src\":{:?},\"strict\":{strict},\"params\":[{}],\"glsl\":{:?},", src, if uses_a { num(PARAM_A) } else { String::new() }, g.source());
        s.push_str("\"points\":[");
        for (i, q) in pts.iter().enumerate() {
            let _ = write!(s, "{}[{},{},{}]", if i > 0 { "," } else { "" }, num(q[0]), num(q[1]), num(q[2]));
        }
        s.push_str("],\"values\":[");
        for (i, v) in vals.iter().enumerate() {
            let _ = write!(s, "{}{}", if i > 0 { "," } else { "" }, num(*v));
        }
        s.push_str("],\"grads\":[");
        for i in 0..n {
            let _ = write!(s, "{}[{},{},{}]", if i > 0 { "," } else { "" }, num(gs[0][i]), num(gs[1][i]), num(gs[2][i]));
        }
        s.push_str("],\"boxes\":[");
        for (i, b) in boxes.iter().enumerate() {
            let _ = write!(s, "{}[{},{},{},{},{},{}]", if i > 0 { "," } else { "" }, num(b[0].lo), num(b[1].lo), num(b[2].lo), num(b[0].hi), num(b[1].hi), num(b[2].hi));
        }
        s.push_str("],\"boxStats\":[");
        for (i, (mn, mx, undef, dec, lo, hi)) in stats.iter().enumerate() {
            let _ = write!(s, "{}[{},{},{},{},{},{}]", if i > 0 { "," } else { "" }, num(*mn), num(*mx), *undef as u8, dec, num(*lo), num(*hi));
        }
        s.push_str("]}");
        js.push_str(&s);
        js.push_str(if ci + 1 < CASES.len() { ",\n" } else { "\n" });
    }
    js.push_str("]\n");
    std::fs::write(path, js).unwrap();
    eprintln!("{} 式を {path} に書き出した", CASES.len());
}

//! 形を指定する命令（計画 `docs/plan-exact-areas.md` の SC2）: `@factor`・`@expand`・`@apart`。
//!
//! 行がまるごと命令（`@factor(x^4 - 1)`）のとき、厳密値の道は中身を指定の形にして出す。今の道（グラフ）は中身をそのまま描く（`expand.rs`）。
//! 答えの形は正規化で崩れる（数の積 641 · 6700417 は 1 つの数になる）ので、LaTeX と「行に追加」のエンジンの文字列はここで作る。
//! - factor: 整数・分数は素因数分解、1 変数の多項式・有理関数は有理数の上の因数分解
//! - expand: 積と整数乗を展開する（文字がいくつでも）
//! - apart: 1 変数の有理関数の部分分数（有理数の上）
//!
//! `simplify`（EX9 の簡約の探索）と `solve`（方程式の行で解が出る。SC1）は、まだ命令にしない（0 章）

use super::latex::{to_engine, to_latex, Style};
use super::num::{self, Int, Rat};
use super::poly::{self, Poly};
use super::term::{add, mul, pow, Term};
use super::{Budget, Stop};

/// 命令の名前
pub const COMMANDS: [&str; 3] = ["factor", "expand", "apart"];

/// 命令の答え: (LaTeX, 行として読める LaTeX, エンジンの文字列, 値の式)
pub struct Shaped {
    pub latex: String,
    pub input_latex: String,
    pub engine: String,
    pub value: Term,
}

fn nf(s: &str) -> Stop {
    Stop::NotFound(s.into())
}

/// 式の中の文字
fn syms(t: &Term, out: &mut Vec<String>) {
    match t {
        Term::Sym(s) => {
            if !out.contains(s) {
                out.push(s.clone());
            }
        }
        Term::Num(_) | Term::Atom(_) => {}
        Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) | Term::Matrix(_, _, v) => v.iter().for_each(|a| syms(a, out)),
        Term::Pow(b, e) => {
            syms(b, out);
            syms(e, out);
        }
    }
}

/// 和の項の並び
fn terms_of(t: Term) -> Vec<Term> {
    match t {
        Term::Add(items) => items,
        other => vec![other],
    }
}

/// 項の並びどうしを掛ける（どの項も和でないので、`mul` が和に戻すことはない）
fn distribute(a: &[Term], b: &[Term]) -> Vec<Term> {
    let mut out = Vec::with_capacity(a.len() * b.len());
    for x in a {
        for y in b {
            out.push(mul(vec![x.clone(), y.clone()]));
        }
    }
    out
}

/// 積と整数乗を展開する
pub fn expand(t: &Term) -> Term {
    match t {
        Term::Add(v) => add(v.iter().map(expand).collect()),
        Term::Mul(v) => {
            let mut acc = vec![Term::one()];
            for f in v {
                acc = distribute(&acc, &terms_of(expand(f)));
            }
            add(acc)
        }
        Term::Pow(b, e) => {
            if let Some(k) = e.as_rat().and_then(num::as_int).and_then(|k| u32::try_from(&k).ok()).filter(|k| (2..=64).contains(k)) {
                let bt = terms_of(expand(b));
                let mut acc = vec![Term::one()];
                for _ in 0..k {
                    // 途中で同類項をまとめる
                    acc = terms_of(add(distribute(&acc, &bt)));
                }
                add(acc)
            } else {
                pow(expand(b), (**e).clone())
            }
        }
        Term::Fn(f, v) => super::term::call(*f, v.iter().map(expand).collect()),
        _ => t.clone(),
    }
}

/// 1 変数の多項式を、次数の高い順の LaTeX とエンジンの文字列に（$x^{2}+x+1$）
fn poly_text(p: &Poly, x: &str, st: &Style) -> (String, String) {
    let mut l = String::new();
    let mut e = String::new();
    for (k, c) in p.0.iter().enumerate().rev() {
        if c.is_zero() {
            continue;
        }
        let neg = num::sign(c) < 0;
        let a = if neg { -c.clone() } else { c.clone() };
        let first = l.is_empty();
        if neg {
            l.push('-');
            e.push('-');
        } else if !first {
            l.push('+');
            e.push('+');
        }
        let xs = match k {
            0 => String::new(),
            1 => x.to_string(),
            _ => format!("{x}^{{{k}}}"),
        };
        let xe = match k {
            0 => String::new(),
            1 => x.to_string(),
            _ => format!("{x}^{k}"),
        };
        if a == Rat::ONE && k > 0 {
            l.push_str(&xs);
            e.push_str(&xe);
        } else if *a.denominator() == num::Nat::ONE {
            l.push_str(&format!("{}{xs}", a.numerator()));
            e.push_str(&format!("{}{xe}", a.numerator()));
        } else {
            let top = if k > 0 && *a.numerator() == Int::ONE { xs.clone() } else { format!("{}{xs}", a.numerator()) };
            l.push_str(&format!("\\frac{{{top}}}{{{}}}", a.denominator()));
            e.push_str(&format!("{}/{}{}", a.numerator(), a.denominator(), if k > 0 { format!("·{xe}") } else { String::new() }));
        }
    }
    let _ = st;
    if l.is_empty() {
        ("0".into(), "0".into())
    } else {
        (l, e)
    }
}

/// 整数の素因数分解の LaTeX（$2^{3} \cdot 3$）とエンジンの文字列
fn int_factors(n: &Int, budget: &Budget) -> Result<(String, String), Stop> {
    let (fs, rest) = num::factor_int(&n.clone().abs_int(), budget).map_err(|_| Stop::TimeUp)?;
    if rest.is_some() {
        return Err(nf("素因数分解しきれません"));
    }
    let mut l = Vec::new();
    let mut e = Vec::new();
    for (p, k) in fs {
        if k == 1 {
            l.push(p.to_string());
            e.push(p.to_string());
        } else {
            l.push(format!("{p}^{{{k}}}"));
            e.push(format!("{p}^{k}"));
        }
    }
    if l.is_empty() {
        l.push("1".into());
        e.push("1".into());
    }
    let sign = if *n < Int::ZERO { "-" } else { "" };
    Ok((format!("{sign}{}", l.join("\\cdot ")), format!("{sign}{}", e.join("·"))))
}

trait AbsInt {
    fn abs_int(self) -> Int;
}
impl AbsInt for Int {
    fn abs_int(self) -> Int {
        if self < Int::ZERO { -self } else { self }
    }
}

/// 1 変数の多項式の因数分解の積（LaTeX、エンジンの文字列）
fn poly_factors(p: &Poly, x: &str, st: &Style, budget: &Budget) -> Result<(String, String), Stop> {
    let fac = poly::factor(p, budget);
    if !fac.complete {
        return Err(Stop::TimeUp);
    }
    let mut l = String::new();
    let mut e = Vec::new();
    let unit = Term::Num(fac.unit.clone());
    let one_factor = fac.factors.len() == 1 && fac.factors[0].1 == 1;
    if fac.unit == -Rat::ONE {
        l.push('-');
        e.push("-1".to_string());
    } else if fac.unit != Rat::ONE {
        l.push_str(&to_latex(&unit, st));
        e.push(to_engine(&unit));
    }
    for (f, k) in &fac.factors {
        let (body, eng) = poly_text(f, x, st);
        let paren = !(one_factor && fac.unit == Rat::ONE) && f.0.iter().filter(|c| !c.is_zero()).count() > 1;
        let b = if paren { format!("\\left({body}\\right)") } else { body };
        l.push_str(&if *k > 1 { format!("{b}^{{{k}}}") } else { b });
        let eb = format!("({eng})");
        e.push(if *k > 1 { format!("{eb}^{k}") } else { eb });
    }
    Ok((l, e.join("·")))
}

/// 命令 `op` を中身 `arg` に使った答え
pub fn run(op: &str, arg: &Term, decimal: bool, budget: &Budget) -> Result<Shaped, Stop> {
    let st = Style { decimal, input: false };
    let sti = Style { decimal, input: true };
    let mut vars = Vec::new();
    syms(arg, &mut vars);
    match op {
        "expand" => {
            let t = expand(arg);
            // 1 変数の多項式なら次数の高い順に
            if let [x] = vars.as_slice() {
                if let Some(p) = poly::to_poly(&t, x) {
                    let (l, e) = poly_text(&p, x, &st);
                    return Ok(Shaped { latex: l.clone(), input_latex: l, engine: e, value: t });
                }
            }
            Ok(Shaped { latex: to_latex(&t, &st), input_latex: to_latex(&t, &sti), engine: to_engine(&t), value: t })
        }
        "factor" => {
            if vars.is_empty() {
                let q = arg.as_rat().ok_or_else(|| nf("factor の中身は、整数・分数か、1 変数の多項式・有理関数にしてください"))?;
                let (nl, ne) = int_factors(q.numerator(), budget)?;
                let d = Int::from(q.denominator().clone());
                let (l, e) = if d == Int::ONE {
                    (nl, ne)
                } else {
                    let (dl, de) = int_factors(&d, budget)?;
                    (format!("\\frac{{{nl}}}{{{dl}}}"), format!("({ne})/({de})"))
                };
                return Ok(Shaped { latex: l.clone(), input_latex: l, engine: e, value: arg.clone() });
            }
            let [x] = vars.as_slice() else { return Err(nf("factor は、文字が 1 つの式だけ（多変数の因数分解はまだ）")) };
            let (n, d) = poly::to_rational(arg, x).ok_or_else(|| nf("多項式・有理関数でありません"))?;
            let g = poly::gcd(&n, &d);
            let (n, d) = (n.divrem(&g).0, d.divrem(&g).0);
            if d.deg() < 1 {
                // 定数の分母は定数倍に入れる
                let n = n.scale(&(Rat::ONE / d.lc()));
                let (nl, ne) = poly_factors(&n, x, &st, budget)?;
                let (nli, _) = poly_factors(&n, x, &sti, budget)?;
                return Ok(Shaped { latex: nl, input_latex: nli, engine: ne, value: arg.clone() });
            }
            let (nl, ne) = poly_factors(&n, x, &st, budget)?;
            let (dl, de) = poly_factors(&d, x, &st, budget)?;
            let (nli, _) = poly_factors(&n, x, &sti, budget)?;
            let (dli, _) = poly_factors(&d, x, &sti, budget)?;
            Ok(Shaped { latex: format!("\\frac{{{nl}}}{{{dl}}}"), input_latex: format!("\\frac{{{nli}}}{{{dli}}}"), engine: format!("({ne})/({de})"), value: arg.clone() })
        }
        "apart" => {
            let [x] = vars.as_slice() else { return Err(nf("apart は、文字が 1 つの有理関数だけ")) };
            let (n, d) = poly::to_rational(arg, x).ok_or_else(|| nf("有理関数でありません"))?;
            let (q, parts) = poly::apart(&n, &d, budget).ok_or_else(|| nf("分母を有理数の上で分解しきれません"))?;
            // 多項式の部分 → 分数の項（分母の因数の順）。正規化の和にすると並びと分数線が崩れるので、ここで書く
            let (mut l, mut e) = if q.is_zero() { (String::new(), String::new()) } else { poly_text(&q, x, &st) };
            let mut value = vec![poly::from_poly(&q, x)];
            // (負か, LaTeX, エンジン)。正の項を先に（同じ符号の中は分母の因数の順）
            let mut items: Vec<(bool, String, String)> = Vec::new();
            for (a, f, j) in parts {
                value.push(mul(vec![poly::from_poly(&a, x), pow(poly::from_poly(&f, x), Term::int(-(j as i64)))]));
                // 分子の符号を外に出す（係数が全部負なら）
                let neg = a.0.iter().all(|c| num::sign(c) <= 0);
                let a = if neg { a.scale(&-Rat::ONE) } else { a };
                // 分子が定数 p/q なら q を分母に回す
                let (top, top_e, den_c) = if a.deg() == 0 {
                    let c = &a.0[0];
                    (c.numerator().to_string(), c.numerator().to_string(), Int::from(c.denominator().clone()))
                } else {
                    let (tl, te) = poly_text(&a, x, &st);
                    (tl, format!("({te})"), Int::ONE)
                };
                let (fl, fe) = poly_text(&f, x, &st);
                let terms_f = f.0.iter().filter(|c| !c.is_zero()).count();
                let base = if terms_f > 1 && (j > 1 || den_c != Int::ONE) { format!("\\left({fl}\\right)") } else { fl };
                let fpow = if j > 1 { format!("{base}^{{{j}}}") } else { base };
                let den = if den_c != Int::ONE { format!("{den_c}{fpow}") } else { fpow };
                let den_e = format!("{}({fe})^{j}", if den_c != Int::ONE { format!("{den_c}·") } else { String::new() });
                items.push((neg, format!("\\frac{{{top}}}{{{den}}}"), format!("{top_e}/({den_e})")));
            }
            items.sort_by_key(|i| i.0);
            for (neg, tl, te) in items {
                if neg {
                    l.push('-');
                    e.push('-');
                } else if !l.is_empty() {
                    l.push('+');
                    e.push('+');
                }
                l.push_str(&tl);
                e.push_str(&te);
            }
            Ok(Shaped { latex: l.clone(), input_latex: l, engine: e, value: add(value) })
        }
        _ => Err(nf("知らない命令")),
    }
}

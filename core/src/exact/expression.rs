//! 定義した関数を使った式を、代入した形で出す（`f(x) = x^2` の行があるとき `f(x+2)` → $(x+2)^2$、`f(\sin x)` → $\sin^2 x$）。
//!
//! 行が「定義した関数の呼び出しを含む式」か「`y = …`（左辺は座標 1 つ）」のとき、右辺を厳密値の道で変換し（定義の本体に引数を入れる）、
//! 答えに座標（・媒介変数）が残れば、その式を右下に出す（「行に追加」もできる）。
//! - 形は代入しただけ（展開しない。展開は `\operatorname{expand}`）。正規化で数どうしはまとまる
//! - スライダーの文字は今の値にせず、文字のまま（$f(x) = ax^2$ なら $f(x+2) = a(x+2)^2$）
//! - 照らし合わせ: 座標とスライダーにいくつかの有理数を入れ、今の道（定義を展開して f64 で計算）の値と比べる。
//!   定義域の違い（$\sqrt{x^2}$ を $x$ にするような変形の誤り）は、負の点で食い違って捨てる
//! - 答えに座標が残らない（`f(2)`）・スライダーだけが残る（`f(a)`）なら、今までの厳密値の道（数の値）に任せる

use super::latex::{to_engine, to_latex, Style};
use super::term::Term;
use super::{ball, Outcome, Report, Stop};
use crate::eval::Params;
use crate::expand::Def;
use crate::parser::{Expr, Rel};

/// 照らし合わせに入れる値（座標・スライダーごとにずらして使う。負の数も入れる）
const POINTS: [(i64, u64); 8] = [(3, 7), (-5, 3), (11, 4), (2, 9), (-7, 5), (13, 6), (5, 8), (-1, 3)];

/// 式の中に、定義した関数（`Def::Function`）の呼び出しがあるか
fn calls_defined_function(e: &Expr, defs: &crate::expand::Defs) -> bool {
    let found = std::cell::Cell::new(false);
    e.rewrite(&|n| {
        if let Expr::Apply { name, .. } = n {
            if matches!(defs.get(name), Some(Def::Function { .. })) {
                found.set(true);
            }
        }
        None
    });
    found.get()
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

/// 照らし合わせ: 答え `t` と元の式 `body`（今の道）の値を、いくつかの点で比べる。比べられた点が 3 つ以上要る
fn check(t: &Term, body: &Expr, params: &Params) -> Result<(), String> {
    let mut vars = Vec::new();
    syms(t, &mut vars);
    // 元の式が使う座標・媒介変数（答えで消えたものも入れる）
    let mut used = Vec::new();
    body.collect_vars(&mut used);
    for v in used {
        let coord = crate::equation::coord_index(&v).is_some() || matches!(v.as_str(), "t" | "u" | "v");
        if (coord || params.contains_key(&v)) && !vars.contains(&v) {
            vars.push(v);
        }
    }
    let (sliders, coords): (Vec<String>, Vec<String>) = vars.into_iter().partition(|v| params.contains_key(v));
    let mut ok = 0;
    'points: for k in 0..POINTS.len() {
        let value = |i: usize| POINTS[(k + 3 * i) % POINTS.len()];
        let mut p = params.clone();
        let mut exact = t.clone();
        let mut inputs: Vec<Vec<f64>> = Vec::new();
        for (i, name) in coords.iter().chain(&sliders).enumerate() {
            let (n, d) = value(i);
            // 0 で割る点は使わない
            let Ok(u) = super::diff::subst_checked(&exact, name, &Term::rat(n, d)) else { continue 'points };
            exact = u;
            let f = n as f64 / d as f64;
            if i < coords.len() {
                inputs.push(vec![f]);
            } else {
                p.insert(name.clone(), num_complex::Complex64::new(f, 0.0));
            }
        }
        if !exact.is_number() || exact.has_division_by_zero() {
            continue;
        }
        let Ok(a) = ball::value(&exact, 64) else { continue };
        let a = a.to_f64();
        // 今の道: 定義を展開して、座標を変数にして f64 で計算する
        let expanded = crate::expand::expand(body, &p).map_err(|e| format!("今の道で展開できません（{e}）"))?;
        let names: Vec<&str> = coords.iter().map(String::as_str).collect();
        let prog = crate::eval::Program::compile_with(&expanded, &names, &p).map_err(|e| format!("今の道で計算できません（{e}）"))?;
        let refs: Vec<&[f64]> = inputs.iter().map(Vec::as_slice).collect();
        let mut out = [num_complex::Complex64::new(0.0, 0.0)];
        prog.eval_batch(&refs, &mut out);
        let b = out[0];
        if !(b.re.is_finite() && b.im.is_finite()) {
            continue;
        }
        let d = ((a.0 - b.re).powi(2) + (a.1 - b.im).powi(2)).sqrt();
        if d > super::check::F64_TOLERANCE * b.norm().max(1.0) {
            return Err(format!("点 {k} で今の道の値 {b} と違います（{a:?}）"));
        }
        ok += 1;
    }
    if ok < 3 {
        return Err("確かめられる点が足りません".into());
    }
    Ok(())
}

/// 行が、定義した関数を使った式（`f(x+2)`・`y = f(\sin x)`）なら、代入した式の答え。そうでなければ None（ほかの道に任せる）
pub fn report(source: &str, params: &Params, work: u64) -> Option<Report> {
    let defs = crate::expand::current_defs();
    let names = defs.names();
    // `y = 右辺`（左辺は座標 1 つで、右辺に出てこない）か、関係の無い式
    let (head, body) = if source.contains('=') {
        let chains = crate::parser::parse_system_with(source, &names).ok()?;
        let [chain] = chains.as_slice() else { return None };
        let (Some(Expr::Var(h)), Some(r)) = (chain.exprs.first(), chain.exprs.get(1)) else { return None };
        if chain.rels != [Rel::Eq] || crate::equation::coord_index(h).is_none() || r.uses_var(h) {
            return None;
        }
        (Some(h.clone()), r.clone())
    } else {
        (None, crate::parser::parse_with(source, &names).ok()?)
    };
    if !calls_defined_function(&body, &defs) {
        return None;
    }
    let (r, used) = super::with_work_limit(work, || -> Result<(Term, bool, bool), Stop> {
        let mut c = super::convert::Converter::new(&defs, params);
        c.keep_params = true;
        let t = c.term(&body)?;
        Ok((t, c.decimal, c.antiderivative.is_some()))
    });
    let (t, decimal, antiderivative) = r.ok()?;
    // 不定積分の行は、原始関数の道（+ C と、微分して戻る確かめ）に任せる
    if antiderivative {
        return None;
    }
    // 座標・媒介変数が残らない式（`f(2)`・スライダーだけの `f(a)`）は、数の値の道に任せる。組・行列の値もまだ
    let mut vs = Vec::new();
    syms(&t, &mut vs);
    if !vs.iter().any(|v| !params.contains_key(v)) || matches!(t, Term::Tuple(_) | Term::List(_) | Term::Matrix(..)) {
        return None;
    }
    let mut rep = Report { decimal, work: used, ..Report::new(Outcome::NotFound(String::new())) };
    if let Err(why) = check(&t, &body, params) {
        rep.outcome = Outcome::Rejected(format!("代入した式が元の式と同じ値になりません（{why}）"));
        return Some(rep);
    }
    let st = Style { decimal, input: false };
    let sti = Style { decimal, input: true };
    let head_latex = head.as_ref().map(|h| format!("{h}=")).unwrap_or_default();
    rep.latex = to_latex(&t, &st);
    rep.input_latex = format!("{head_latex}{}", to_latex(&t, &sti));
    rep.engine = format!("{head_latex}{}", to_engine(&t));
    rep.rules.push("定義した関数に引数を代入し、いくつかの点で今の道と比べた");
    rep.outcome = Outcome::Found(t);
    Some(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_rejects_a_wrong_domain() {
        // √(x²) を x にするような変形の誤りは、負の点で食い違って捨てる
        let params = Params::new();
        let body = crate::parser::parse("sqrt(x^2)").unwrap();
        assert!(check(&Term::Sym("x".into()), &body, &params).is_err());
        assert!(check(&super::super::term::pow(super::super::term::pow(Term::Sym("x".into()), Term::int(2)), Term::rat(1, 2)), &body, &params).is_ok());
    }
}

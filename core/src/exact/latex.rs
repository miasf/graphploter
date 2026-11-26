//! 厳密値の道の式の書き出し: 画面の LaTeX と、「行に追加」のエンジンの文字列（計画 `docs/plan-exact-core.md` の EX4）。
//!
//! 答えの形（計画の 2.6 と、EX0 で足した決まり）:
//! - 数だけの和の項の順: 有理数 → 根号（中の数の小さい順）→ π → e → 対数 → γ → G → ζ。π と e のべきは指数の大きい順。
//!   先頭が負なら、最初の正の項を先頭に出す（$\sqrt5 - 1$、$1 + \sqrt2$、$\frac{\sqrt6 - \sqrt2}{4}$）
//! - 有理数と根号だけの和は 1 本の分数線にまとめる。定数（π など）を含む和は項ごとの分数（$\frac{\pi^3}{54} + \frac{\sqrt3\pi^2}{6}$）
//! - 負のべきは分母に置く（$-\frac{1}{\pi}$、$\frac{1}{2e}$）
//! - 複素数は「実部 + 虚部 i」（$\frac12 + \frac{\sqrt3}{2}i$、$\pi i$）
//! - 自然対数は `\ln`（このアプリの `\log` は常用対数。D5 の計画との違い）
//! - 小数の入力があれば、有限小数で書ける値は小数（`0.3`）。書けない値は分数（計画の 2.7）
//!
//! 同じ決まりを `tools/exact/cases.py` の `fmt` も持つ（例題の表の答えを作る）。片方を直したら、もう片方も直す。

use super::num::{self, Int, Nat, Rat};
use super::term::{re_im, split_coeff, Atom, Term};
use crate::functions::Func;
use dashu::base::{Abs, BitTest};

/// 書き出しの設定
#[derive(Debug, Clone, Copy, Default)]
pub struct Style {
    /// 入力に小数があった（有限小数で書ける値は小数で出す）
    pub decimal: bool,
    /// 行として読める形にする（γ・G を `\\operatorname{eulergamma}`・`\\operatorname{catalan}` にする。D10）
    pub input: bool,
}

const KINDS: [&str; 9] = ["rational", "sqrt", "pi", "e", "ln", "gamma", "catalan", "zeta", "other"];

fn kind_index(k: &str) -> usize {
    KINDS.iter().position(|x| *x == k).unwrap_or(8)
}

/// 並べるための種類と鍵
#[derive(Debug, Clone, Copy)]
struct Kind {
    name: &'static str,
    key: f64,
}

impl Kind {
    fn sort_key(&self) -> (usize, f64) {
        let k = if self.name == "pi" || self.name == "e" { -self.key } else { self.key };
        (kind_index(self.name), k)
    }
}

fn cmp_kind(a: &Kind, b: &Kind) -> std::cmp::Ordering {
    let (ai, ak) = a.sort_key();
    let (bi, bk) = b.sort_key();
    ai.cmp(&bi).then(ak.partial_cmp(&bk).unwrap_or(std::cmp::Ordering::Equal))
}

fn int_text(n: &Int) -> String {
    n.to_string()
}

/// 整数 c × 本体（本体は空でもよい）
fn coef_times(c: &Int, body: &str) -> String {
    if body.is_empty() {
        return int_text(c);
    }
    if *c == Int::ONE {
        return body.to_string();
    }
    if *c == -Int::ONE {
        return format!("-{body}");
    }
    format!("{}{body}", int_text(c))
}

/// 係数を除いた単項の LaTeX と種類（分子の部分だけ。分母は [`split_rest`] で分けてある）
fn atom(t: &Term, style: &Style) -> (String, Kind) {
    let k = |name: &'static str, key: f64| Kind { name, key };
    match t {
        Term::Num(q) if *q == Rat::ONE => (String::new(), k("rational", 0.0)),
        Term::Atom(Atom::Pi) => ("\\pi".into(), k("pi", 1.0)),
        Term::Atom(Atom::E) => ("e".into(), k("e", 1.0)),
        Term::Atom(Atom::I) => ("i".into(), k("other", 0.0)),
        Term::Atom(Atom::EulerGamma) => (if style.input { "\\operatorname{eulergamma}".into() } else { "\\gamma".into() }, k("gamma", 0.0)),
        Term::Atom(Atom::Catalan) => (if style.input { "\\operatorname{catalan}".into() } else { "G".into() }, k("catalan", 0.0)),
        Term::Pow(b, e) => {
            if let (Term::Num(r), Term::Num(q)) = (&**b, &**e) {
                if let (Some(r), true) = (num::as_int(r), *q.numerator() == Int::ONE) {
                    let n = u64::try_from(q.denominator()).unwrap_or(0);
                    let rf = r.to_f64().value();
                    return match n {
                        2 => (format!("\\sqrt{{{r}}}"), k("sqrt", rf)),
                        _ => (format!("\\sqrt[{n}]{{{r}}}"), k("sqrt", rf + 0.5)),
                    };
                }
            }
            if **b == Term::Atom(Atom::Pi) {
                if let Some(q) = e.as_rat() {
                    if *q == Rat::from_parts(Int::ONE, Nat::from(2u8)) {
                        return ("\\sqrt{\\pi}".into(), k("pi", 0.5));
                    }
                    if let Some(n) = num::as_int(q) {
                        return (format!("\\pi^{{{n}}}"), k("pi", n.to_f64().value()));
                    }
                }
            }
            if **b == Term::Atom(Atom::E) {
                if let Some(n) = e.as_rat().and_then(num::as_int) {
                    return (format!("e^{{{n}}}"), k("e", n.to_f64().value()));
                }
                return (format!("e^{{{}}}", to_latex(e, style)), k("e", 99.0));
            }
            if e.as_rat().is_some_and(|q| *q == Rat::from_parts(Int::ONE, Nat::from(2u8))) {
                return (format!("\\sqrt{{{}}}", to_latex(b, style)), k("sqrt", 1000.0));
            }
            (power(b, e, style), k("other", 0.0))
        }
        Term::Fn(Func::Ln, a) if a.len() == 1 && a[0].as_rat().and_then(num::as_int).is_some() => {
            let n = num::as_int(a[0].as_rat().unwrap()).unwrap();
            (format!("\\ln {n}"), k("ln", n.to_f64().value()))
        }
        Term::Fn(Func::Zeta, a) if a.len() == 1 && a[0].as_rat().and_then(num::as_int).is_some() => {
            let n = num::as_int(a[0].as_rat().unwrap()).unwrap();
            (format!("\\zeta({n})"), k("zeta", n.to_f64().value()))
        }
        Term::Mul(v) => {
            let mut parts: Vec<(String, Kind)> = v.iter().map(|f| atom(f, style)).collect();
            parts.sort_by(|a, b| cmp_kind(&a.1, &b.1));
            // 種類は、いちばん後ろの種類のもの（同じなら最初）
            let mut best = parts[0].1;
            for (_, kd) in &parts {
                if kind_index(kd.name) > kind_index(best.name) {
                    best = *kd;
                }
            }
            let mut s = String::new();
            for (i, (p, _)) in parts.iter().enumerate() {
                // 文字の命令の後に文字が続くときは空白を入れる（\pi i）
                if i > 0 && needs_space(&s, p) {
                    s.push(' ');
                }
                s.push_str(p);
            }
            (s, best)
        }
        other => (generic(other, style), k("other", 0.0)),
    }
}

fn needs_space(left: &str, right: &str) -> bool {
    let ends_command = left.rsplit('\\').next().is_some_and(|tail| left.contains('\\') && !tail.is_empty() && tail.chars().all(|c| c.is_ascii_alphabetic()));
    ends_command && right.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

/// 係数を除いた部分を、分子の因子と分母の因子に分ける
fn split_rest(rest: &Term) -> (Term, Term) {
    let fs: Vec<Term> = match rest {
        Term::Mul(v) => v.clone(),
        other => vec![other.clone()],
    };
    let mut num_f = Vec::new();
    let mut den_f = Vec::new();
    for f in fs {
        match &f {
            Term::Pow(b, e) if e.as_rat().is_some_and(|q| num::sign(q) < 0 && num::as_int(q).is_some()) => {
                let k = -e.as_rat().unwrap().clone();
                den_f.push(if k == Rat::ONE { (**b).clone() } else { Term::Pow(b.clone(), Box::new(Term::Num(k))) });
            }
            _ => num_f.push(f),
        }
    }
    let pack = |v: Vec<Term>| match v.len() {
        0 => Term::one(),
        1 => v.into_iter().next().unwrap(),
        _ => Term::Mul(v),
    };
    (pack(num_f), pack(den_f))
}

/// 実数の数の和の LaTeX（係数は有理数）
fn real_number(t: &Term, style: &Style) -> String {
    let parts: Vec<Term> = match t {
        Term::Add(v) => v.clone(),
        other => vec![other.clone()],
    };
    struct Item {
        c: Rat,
        num: String,
        den: String,
        kind: Kind,
    }
    let mut items: Vec<Item> = parts
        .iter()
        .map(|p| {
            let (c, rest) = split_coeff(p);
            let (nf, df) = split_rest(&rest);
            let (num, kind) = atom(&nf, style);
            let den = if df.is_one() { String::new() } else { atom(&df, style).0 };
            Item { c, num, den, kind }
        })
        .collect();
    items.sort_by(|a, b| cmp_kind(&a.kind, &b.kind));
    if num::sign(&items[0].c) < 0 {
        if let Some(pos) = items.iter().position(|it| num::sign(&it.c) > 0) {
            let it = items.remove(pos);
            items.insert(0, it);
        }
    }
    let algebraic = items.iter().all(|it| (it.kind.name == "rational" || it.kind.name == "sqrt") && it.den.is_empty());
    if algebraic && items.len() > 1 {
        let mut den = Int::ONE;
        for it in &items {
            den = num::lcm(&den, &Int::from(it.c.denominator().clone()));
        }
        // 全部の項が負なら、符号を分数線の前に出す（$-\frac{3+\sqrt{33}}{2}$）。分母が無いときは出さない（$-3-\sqrt{33}$）
        let neg = den != Int::ONE && items.iter().all(|it| num::sign(&it.c) < 0);
        let mut s = String::new();
        for (i, it) in items.iter().enumerate() {
            let m = num::as_int(&(it.c.clone() * Rat::from(den.clone()))).unwrap();
            let m = if neg { -m } else { m };
            let part = coef_times(&m, &it.num);
            if i > 0 && !part.starts_with('-') {
                s.push('+');
            }
            s.push_str(&part);
        }
        let body = if den == Int::ONE { s } else { format!("\\frac{{{s}}}{{{den}}}") };
        return if neg { format!("-{body}") } else { body };
    }
    let mut s = String::new();
    for (i, it) in items.iter().enumerate() {
        let q = it.c.denominator();
        let part = if *q == Nat::ONE && it.den.is_empty() {
            coef_times(it.c.numerator(), &it.num)
        } else {
            let p = it.c.numerator().clone().abs();
            let top = if it.num.is_empty() { int_text(&p) } else { coef_times(&p, &it.num) };
            let bottom = format!("{}{}", if *q == Nat::ONE { String::new() } else { q.to_string() }, it.den);
            format!("{}\\frac{{{top}}}{{{bottom}}}", if num::sign(&it.c) < 0 { "-" } else { "" })
        };
        if i > 0 && !part.starts_with('-') {
            s.push('+');
        }
        s.push_str(&part);
    }
    s
}

/// 有限小数で書けるなら、その 10 進の文字列
pub fn decimal_text(q: &Rat) -> Option<String> {
    let mut d = Int::from(q.denominator().clone());
    let (mut a, mut b) = (0usize, 0usize);
    while !d.bit(0) {
        d >>= 1;
        a += 1;
    }
    while (&d % Int::from(5)).is_zero() {
        d /= Int::from(5);
        b += 1;
    }
    if d != Int::ONE {
        return None;
    }
    let k = a.max(b);
    let scaled = num::as_int(&(q.clone() * Rat::from(Int::from(10).pow(k))))?;
    let neg = scaled < Int::ZERO;
    let digits = scaled.abs().to_string();
    let s = if k == 0 {
        digits
    } else {
        let padded = format!("{digits:0>width$}", width = k + 1);
        let (int, frac) = padded.split_at(padded.len() - k);
        let frac = frac.trim_end_matches('0');
        if frac.is_empty() { int.to_string() } else { format!("{int}.{frac}") }
    };
    Some(if neg { format!("-{s}") } else { s })
}

/// 画面に出す LaTeX
pub fn to_latex(t: &Term, style: &Style) -> String {
    if let Term::Num(q) = t {
        if style.decimal {
            if let Some(d) = decimal_text(q) {
                return d;
            }
        }
        return real_number(t, style);
    }
    if t.is_number() {
        if let Some((re, im)) = re_im(t) {
            if im.is_zero() {
                return real_number(&re, style);
            }
            let mut s = if re.is_zero() { String::new() } else { real_number(&re, style) };
            let it = if im.is_one() {
                "i".to_string()
            } else if im == Term::int(-1) {
                "-i".to_string()
            } else {
                let body = real_number(&im, style);
                let sp = if needs_space(&body, "i") { " " } else { "" };
                format!("{body}{sp}i")
            };
            if !s.is_empty() && !it.starts_with('-') {
                s.push('+');
            }
            s.push_str(&it);
            return s;
        }
    }
    generic(t, style)
}

fn func_latex(f: Func) -> String {
    match f {
        Func::Sin => "\\sin".into(),
        Func::Cos => "\\cos".into(),
        Func::Tan => "\\tan".into(),
        Func::Asin => "\\arcsin".into(),
        Func::Acos => "\\arccos".into(),
        Func::Atan => "\\arctan".into(),
        Func::Sinh => "\\sinh".into(),
        Func::Cosh => "\\cosh".into(),
        Func::Tanh => "\\tanh".into(),
        Func::Exp => "\\exp".into(),
        Func::Ln => "\\ln".into(),
        Func::Log10 => "\\log".into(),
        Func::Gamma => "\\Gamma".into(),
        Func::Zeta => "\\zeta".into(),
        Func::Digamma => "\\psi".into(),
        other => format!("\\operatorname{{{}}}", other.names()[0]),
    }
}

/// 一般の式（文字を含む式・根号の和の中など）。文書 8 の各ステップで形を整える
fn generic(t: &Term, style: &Style) -> String {
    match t {
        Term::Num(_) => to_latex(t, style),
        Term::Atom(_) => atom(t, style).0,
        Term::Sym(s) => s.clone(),
        Term::Add(v) => {
            // 全部の項に共通の指数関数 e^u があれば、くくり出す: (x² − 2x + 2)e^x
            if let Some(common) = common_exp(v) {
                let inv = super::term::pow(common.clone(), Term::int(-1));
                let rest = super::term::add(v.iter().map(|t| super::term::mul(vec![t.clone(), inv.clone()])).collect());
                if !matches!(rest, Term::Add(_)) || !common_exp(match &rest { Term::Add(r) => r, _ => unreachable!() }).is_some() {
                    return generic(&Term::Mul(vec![rest, common]), style);
                }
            }
            // 項の分母（係数の分母）が全部同じ 2 以上なら、1 本の分数線にまとめる: (x√(1 − x²) + arcsin x)/2
            if v.len() >= 2 && !t.is_number() {
                let dens: Vec<Int> = v.iter().map(|x| Int::from(split_coeff(x).0.denominator().clone())).collect();
                if dens[0] > Int::ONE && dens.iter().all(|d| *d == dens[0]) {
                    let d = Rat::from(dens[0].clone());
                    let scaled = super::term::add(v.iter().map(|x| super::term::mul(vec![Term::Num(d.clone()), x.clone()])).collect());
                    let neg_all = v.iter().all(|x| num::sign(&split_coeff(x).0) < 0);
                    let body = if neg_all { generic(&super::term::neg(scaled), style) } else { generic(&scaled, style) };
                    return format!("{}\\frac{{{body}}}{{{}}}", if neg_all { "-" } else { "" }, dens[0]);
                }
            }
            let mut parts: Vec<String> = v.iter().map(|x| to_latex(x, style)).collect();
            if parts.first().is_some_and(|p| p.starts_with('-')) {
                if let Some(pos) = parts.iter().position(|p| !p.starts_with('-')) {
                    let first = parts.remove(pos);
                    parts.insert(0, first);
                }
            }
            let mut s = String::new();
            for (i, part) in parts.into_iter().enumerate() {
                if i > 0 && !part.starts_with('-') {
                    s.push('+');
                }
                s.push_str(&part);
            }
            s
        }
        Term::Mul(_) => {
            let (c, rest) = split_coeff(t);
            let (nf, df) = split_rest(&rest);
            let body = factors(&nf, style);
            if df.is_one() && *c.denominator() == Nat::ONE {
                return coef_times(c.numerator(), &body);
            }
            let p = c.numerator().clone().abs();
            let top = if body.is_empty() { int_text(&p) } else { coef_times(&p, &body) };
            let den = if matches!(df, Term::Add(_)) { to_latex(&df, style) } else { factors(&df, style) };
            let q = c.denominator();
            let bottom = format!("{}{}", if *q == Nat::ONE { String::new() } else { q.to_string() }, den);
            format!("{}\\frac{{{top}}}{{{bottom}}}", if num::sign(&c) < 0 { "-" } else { "" })
        }
        Term::Pow(b, e) => power(b, e, style),
        Term::Fn(f, args) => call_latex(*f, args, None, style),
        Term::Tuple(v) => format!("\\left({}\\right)", v.iter().map(|x| to_latex(x, style)).collect::<Vec<_>>().join(",")),
        Term::List(v) => format!("\\left[{}\\right]", v.iter().map(|x| to_latex(x, style)).collect::<Vec<_>>().join(",")),
        Term::Matrix(r, c, v) => {
            let rows: Vec<String> = (0..*r).map(|i| (0..*c).map(|j| to_latex(&v[i * c + j], style)).collect::<Vec<_>>().join(" & ")).collect();
            format!("\\begin{{pmatrix}}{}\\end{{pmatrix}}", rows.join("\\\\"))
        }
    }
}

/// 和の全部の項に共通の e^u（u は文字を含む）
fn common_exp(v: &[Term]) -> Option<Term> {
    let exps = |t: &Term| -> Vec<Term> {
        let fs: Vec<Term> = match t {
            Term::Mul(f) => f.clone(),
            other => vec![other.clone()],
        };
        fs.into_iter().filter(|f| matches!(f, Term::Pow(b, e) if **b == Term::Atom(Atom::E) && !e.is_number())).collect()
    };
    let first = exps(v.first()?);
    first.into_iter().find(|c| v.iter().all(|t| exps(t).contains(c)))
}

fn factors(t: &Term, style: &Style) -> String {
    if t.is_one() {
        return String::new();
    }
    let fs: Vec<Term> = match t {
        Term::Mul(v) => v.clone(),
        other => vec![other.clone()],
    };
    // 掛ける順: 多項式の和（括弧）→ ほかの因子 → 多項式でない和（e^x(\sin x − \cos x)）
    let is_poly_sum = |f: &Term| matches!(f, Term::Add(_)) && !has_fn(f);
    let mut fs: Vec<Term> = fs.iter().filter(|f| is_poly_sum(f)).cloned().chain(fs.iter().filter(|f| !matches!(f, Term::Add(_))).cloned()).chain(fs.iter().filter(|f| matches!(f, Term::Add(_)) && !is_poly_sum(f)).cloned()).collect();
    let _ = &mut fs;
    let mut s = String::new();
    for f in fs {
        let p = match &f {
            Term::Add(_) => format!("\\left({}\\right)", to_latex(&f, style)),
            _ => atom(&f, style).0,
        };
        if needs_space(&s, &p) {
            s.push(' ');
        }
        s.push_str(&p);
    }
    s
}

/// 関数の呼び出し。引数が 1 文字・正の整数・「整数 × 1 文字」なら括弧を付けない（`\\sin x`・`\\ln 2`・`\\cos 3x`）。
/// `power` があれば関数の名前の肩に付ける（`\\sin^{2}x`）
fn call_latex(f: Func, args: &[Term], power: Option<&Term>, style: &Style) -> String {
    let name = func_latex(f);
    let name = match power {
        Some(k) => format!("{name}^{{{}}}", to_latex(k, style)),
        None => name,
    };
    if f == Func::Abs && args.len() == 1 && power.is_none() {
        return format!("\\left|{}\\right|", to_latex(&args[0], style));
    }
    // ln|u| は括弧を重ねない
    if let [Term::Fn(Func::Abs, inner)] = args {
        if inner.len() == 1 && matches!(f, Func::Ln | Func::Log10) {
            return format!("{name}\\left|{}\\right|", to_latex(&inner[0], style));
        }
    }
    if let [a] = args {
        let short = match a {
            Term::Sym(_) => true,
            Term::Num(q) => num::sign(q) > 0 && *q.denominator() == Nat::ONE,
            Term::Mul(v) => v.len() == 2 && matches!(&v[0], Term::Num(q) if num::sign(q) > 0 && *q.denominator() == Nat::ONE) && matches!(&v[1], Term::Sym(_)),
            _ => false,
        };
        if short && !name.starts_with("\\operatorname") {
            return format!("{name} {}", to_latex(a, style));
        }
    }
    let inner: Vec<String> = args.iter().map(|a| to_latex(a, style)).collect();
    format!("{name}\\left({}\\right)", inner.join(","))
}

fn power(b: &Term, e: &Term, style: &Style) -> String {
    if let Some(q) = e.as_rat() {
        // 負の整数乗は分数に
        if num::sign(q) < 0 && num::as_int(q).is_some() {
            let k = -q.clone();
            let den = if k != Rat::ONE { power(b, &Term::Num(k), style) } else if matches!(b, Term::Add(_)) { to_latex(b, style) } else { power_base(b, style) };
            return format!("\\frac{{1}}{{{den}}}");
        }
        // 1/n 乗は根号に
        if *q.numerator() == Int::ONE {
            let n = q.denominator();
            let inner = to_latex(b, style);
            return if *n == Nat::from(2u8) { format!("\\sqrt{{{inner}}}") } else { format!("\\sqrt[{n}]{{{inner}}}") };
        }
        // 関数の正の整数乗は名前の肩に
        if let (Term::Fn(f, args), Some(_)) = (b, num::as_int(q).filter(|k| *k > Int::ONE)) {
            if !matches!(f, Func::Abs) {
                return call_latex(*f, args, Some(e), style);
            }
        }
    }
    format!("{}^{{{}}}", power_base(b, style), to_latex(e, style))
}

fn power_base(b: &Term, style: &Style) -> String {
    match b {
        Term::Sym(_) | Term::Atom(_) => to_latex(b, style),
        Term::Num(q) if num::sign(q) > 0 && *q.denominator() == Nat::ONE => to_latex(b, style),
        _ => format!("\\left({}\\right)", to_latex(b, style)),
    }
}

/// 「行に追加」に使うエンジンの文字列（`parser.rs` が読める形。かっこは多めに付ける）
pub fn to_engine(t: &Term) -> String {
    match t {
        Term::Num(q) => {
            if *q.denominator() == Nat::ONE {
                let n = q.numerator();
                if *n < Int::ZERO { format!("({n})") } else { n.to_string() }
            } else {
                format!("({}/{})", q.numerator(), q.denominator())
            }
        }
        Term::Atom(Atom::Pi) => "π".into(),
        Term::Atom(Atom::E) => "e".into(),
        Term::Atom(Atom::I) => "i".into(),
        Term::Atom(Atom::EulerGamma) => "eulergamma".into(),
        Term::Atom(Atom::Catalan) => "catalan".into(),
        Term::Sym(s) => s.clone(),
        Term::Add(v) => format!("({})", v.iter().map(to_engine).collect::<Vec<_>>().join("+")),
        Term::Mul(v) => format!("({})", v.iter().map(to_engine).collect::<Vec<_>>().join("*")),
        Term::Pow(b, e) => format!("({})^({})", to_engine(b), to_engine(e)),
        Term::Fn(f, args) => format!("{}({})", f.names()[0], args.iter().map(to_engine).collect::<Vec<_>>().join(",")),
        Term::Tuple(v) => format!("({})", v.iter().map(to_engine).collect::<Vec<_>>().join(",")),
        Term::List(v) => format!("[{}]", v.iter().map(to_engine).collect::<Vec<_>>().join(",")),
        Term::Matrix(r, c, v) => {
            let rows: Vec<String> = (0..*r).map(|i| (0..*c).map(|j| to_engine(&v[i * c + j])).collect::<Vec<_>>().join(",")).collect();
            format!("⟦{}⟧", rows.join(";"))
        }
    }
}

/// 関数（sin・ln など）や e^u を含むか
fn has_fn(t: &Term) -> bool {
    match t {
        Term::Fn(..) => true,
        Term::Pow(b, e) => **b == Term::Atom(Atom::E) || has_fn(b) || has_fn(e),
        Term::Add(v) | Term::Mul(v) => v.iter().any(has_fn),
        _ => false,
    }
}

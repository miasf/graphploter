//! 極限（計画 `docs/plan-exact-areas.md` の LM1）。
//!
//! 1. 片側ずつ、級数展開（[`super::series`]）の先頭の項 $c h^p$ で決める: $p > 0$ なら 0、$p = 0$ なら $c$、$p < 0$ なら $c$ の符号の $\pm\infty$。
//!    係数に $\ln h$（ℓ）があれば、$h \to 0^+$ で ℓ → −∞ として決める（$x\ln x \to 0$ は $p = 1 > 0$）
//! 2. 両側（有限の点）は、右（$x = a + h$）と左（$x = a - h$）を別に求め、違えば「存在しません（左 …、右 …）」。
//!    片側の値が実数にならない（$\sqrt x$・$x^x$ の左）なら、定義域が片側だけとして、もう片側の値にする
//! 3. 振動: 三角関数の中身が発散する形（$\sin\frac1x$）だけ「存在しません」と言う（中身が ±∞ に行くので、どんな値にも近づかない）。
//!    それ以外で展開できなければ「見つからない」（$\frac{x^{100}}{e^x}$ のような ∞ での指数と対数の混ざりは LM2）

use super::num::{self, Rat};
use super::series::{leading, Map, LOG_H};
use super::term::{Atom, Term};
use super::{latex, Stop};
use crate::functions::Func;

/// 極限の結果
#[derive(Debug, Clone, PartialEq)]
pub enum Lim {
    Value(Term),
    PlusInf,
    MinusInf,
}

/// 近づく先
#[derive(Debug, Clone)]
pub enum Point {
    Finite(Term),
    PlusInf,
    MinusInf,
}

fn has_log(t: &Term) -> bool {
    match t {
        Term::Sym(s) => s == LOG_H,
        Term::Num(_) | Term::Atom(_) => false,
        Term::Add(v) | Term::Mul(v) | Term::Fn(_, v) | Term::Tuple(v) | Term::List(v) | Term::Matrix(_, _, v) => v.iter().any(has_log),
        Term::Pow(b, e) => has_log(b) || has_log(e),
    }
}

/// 係数 c（ℓ の 1 次式 c₁ℓ + c₀ のときだけ）を (c₁, c₀) に分ける
fn split_log(c: &Term) -> Option<(Term, Term)> {
    let l = super::diff::diff(c, LOG_H).ok()?;
    if has_log(&l) {
        return None;
    }
    let c0 = super::diff::subst(c, LOG_H, &Term::zero());
    Some((l, c0))
}

fn sign(t: &Term) -> Option<i32> {
    super::values::real_sign(t)
}

fn is_real(t: &Term) -> bool {
    super::term::re_im(t).is_some_and(|(_, im)| im.is_zero())
}

/// 片側の極限（`map` は $h \to 0^+$）
fn one_sided(t: &Term, x: &str, map: &Map) -> Result<Lim, Stop> {
    let (p, c) = match leading(t, x, map) {
        Ok(v) => v,
        Err(Stop::NotFound(r)) if r.starts_with("振動") => return Err(Stop::NotFound(r)),
        Err(e) => return Err(e),
    };
    if has_log(&c) {
        let (c1, c0) = split_log(&c).ok_or_else(|| Stop::NotFound("対数の入れ子の極限はまだ求められません".into()))?;
        if num::sign(&p) > 0 {
            return Ok(Lim::Value(Term::zero()));
        }
        // ℓ → −∞: c₁ℓ の符号は −c₁ の符号
        if c1.is_zero() {
            return classify(&p, &c0);
        }
        let s = sign(&c1).ok_or_else(|| Stop::NotFound("符号の決まらない係数".into()))?;
        return Ok(if s > 0 { Lim::MinusInf } else { Lim::PlusInf });
    }
    classify(&p, &c)
}

fn classify(p: &Rat, c: &Term) -> Result<Lim, Stop> {
    match num::sign(p) {
        1 => Ok(Lim::Value(Term::zero())),
        0 => Ok(Lim::Value(c.clone())),
        _ => {
            let s = sign(c).ok_or_else(|| Stop::NotFound("発散の向きが決まりません".into()))?;
            Ok(if s > 0 { Lim::PlusInf } else { Lim::MinusInf })
        }
    }
}

fn text(l: &Lim) -> String {
    match l {
        Lim::Value(t) => latex::to_latex(t, &latex::Style::default()),
        Lim::PlusInf => "+∞".into(),
        Lim::MinusInf => "−∞".into(),
    }
}

fn same(a: &Lim, b: &Lim) -> bool {
    match (a, b) {
        (Lim::Value(x), Lim::Value(y)) => {
            if x == y {
                return true;
            }
            let d = super::term::add(vec![x.clone(), super::term::neg(y.clone())]);
            let mut rules = Vec::new();
            super::ball::is_zero(&d, &mut rules) == Some(true)
        }
        _ => a == b,
    }
}

/// 三角関数の中身が発散する形（いちばん外が sin・cos）なら振動
fn oscillates(t: &Term, x: &str, map: &Map) -> bool {
    if let Term::Fn(Func::Sin | Func::Cos, a) = t {
        if let [arg] = a.as_slice() {
            if let Ok((p, _)) = leading(arg, x, map) {
                return num::sign(&p) < 0;
            }
        }
    }
    false
}

/// 極限。`side` は 0（両側）・1（右）・−1（左）
pub fn limit(t: &Term, x: &str, to: &Point, side: i32) -> Result<Lim, Stop> {
    let maps: Vec<(Map, &str)> = match to {
        Point::PlusInf => vec![(Map::Infinite(1), "")],
        Point::MinusInf => vec![(Map::Infinite(-1), "")],
        Point::Finite(a) => match side {
            1 => vec![(Map::Finite(a.clone(), 1), "右")],
            -1 => vec![(Map::Finite(a.clone(), -1), "左")],
            _ => vec![(Map::Finite(a.clone(), 1), "右"), (Map::Finite(a.clone(), -1), "左")],
        },
    };
    for (m, _) in &maps {
        if oscillates(t, x, m) {
            return Err(Stop::NotExist("振動します（三角関数の中身が発散します）".into()));
        }
    }
    let mut results = Vec::new();
    for (m, name) in &maps {
        results.push((one_sided(t, x, m), *name));
    }
    if results.len() == 1 {
        return results.pop().unwrap().0;
    }
    let (right, left) = (&results[0].0, &results[1].0);
    match (right, left) {
        (Ok(r), Ok(l)) => {
            // 片側の値が実数にならないなら、定義域が片側だけ
            let real = |v: &Lim| match v {
                Lim::Value(t) => is_real(t),
                _ => true,
            };
            match (real(r), real(l)) {
                (true, false) => return Ok(r.clone()),
                (false, true) => return Ok(l.clone()),
                _ => {}
            }
            if same(r, l) {
                Ok(r.clone())
            } else {
                Err(Stop::NotExist(format!("左からは {}、右からは {} に近づきます", text(l), text(r))))
            }
        }
        (Err(Stop::TimeUp), _) | (_, Err(Stop::TimeUp)) => Err(Stop::TimeUp),
        (Ok(r), Err(_)) => Err(Stop::NotFound(format!("左からの極限が求められません（右からは {}）", text(r)))),
        (Err(e), _) => Err(e.clone()),
    }
}

/// 定数の点の値として、π などを含む有限の値か
pub fn finite_point(t: &Term) -> bool {
    t.is_number() && !matches!(t, Term::Atom(Atom::I))
}


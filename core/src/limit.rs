//! 実数の 1 変数の極限の数値近似。有限個の標本から収束を判定できないときは誤りを返す。

use crate::eval::{Params, Program};
use crate::parser::{BinOp, Expr};
use num_complex::Complex64;

const STEPS: usize = 48;
const WINDOW: usize = 6;

#[derive(Clone, Copy)]
pub(crate) enum Behavior {
    Value(f64),
    Oscillates,
    Unknown,
}

impl Behavior {
    pub(crate) fn value(self) -> Option<f64> {
        match self {
            Self::Value(v) => Some(v),
            _ => None,
        }
    }

    pub(crate) fn pair(self) -> [f64; 2] {
        match self {
            Self::Value(v) => [if v.is_finite() { 1.0 } else { 2.0 }, v],
            Self::Oscillates => [3.0, f64::NAN],
            Self::Unknown => [0.0, f64::NAN],
        }
    }
}

#[derive(Clone)]
pub struct Report {
    pub(crate) variable: String,
    pub(crate) target: f64,
    pub(crate) side: i8,
    pub(crate) left: Behavior,
    pub(crate) right: Behavior,
    pub(crate) defined: Option<f64>,
}

impl Report {
    pub(crate) fn value(&self) -> Result<f64, String> {
        let value = match self.side {
            -1 => self.left,
            1 => self.right,
            _ if self.target.is_infinite() => if self.target.is_sign_negative() { self.left } else { self.right },
            _ => match (self.left.value(), self.right.value()) {
                (Some(left), Some(right)) if left == right => return Ok(left),
                (Some(left), Some(right)) if close(left, right) => return Ok(0.5 * left + 0.5 * right),
                (Some(_), Some(_)) => return Err("左極限と右極限が一致しません".into()),
                _ if matches!(self.left, Behavior::Oscillates) || matches!(self.right, Behavior::Oscillates) => Behavior::Oscillates,
                _ => Behavior::Unknown,
            },
        };
        match value {
            Behavior::Value(v) => Ok(v),
            Behavior::Oscillates => Err("振動のため、極限値を求められません".into()),
            Behavior::Unknown => Err("極限の収束を数値計算で確認できません".into()),
        }
    }

    pub(crate) fn marks(&self) -> (Vec<f32>, Vec<f32>) {
        let mut holes = Vec::new();
        let mut filled = Vec::new();
        if !self.target.is_finite() {
            return (holes, filled);
        }
        let left = self.left.value();
        let right = self.right.value();
        let point = |y: f64| [self.target as f32, y as f32, 0.0];
        for v in [left, right].into_iter().flatten().filter(|v| v.is_finite()) {
            if self.defined.is_none_or(|at| !close(at, v)) && !holes.chunks_exact(3).any(|p| p == point(v)) {
                holes.extend(point(v));
            }
        }
        if let Some(at) = self.defined {
            if ![left, right].into_iter().all(|v| v.is_some_and(|v| close(at, v))) {
                filled.extend(point(at));
            }
        }
        (holes, filled)
    }
}

/// 分母の先頭の Taylor 係数まで分子にも零点があれば、同じ次数の係数の商を取る。
/// (1−cos x)/x² のように、近い点を直接計算すると桁落ちする式にも使える。
pub(crate) fn removable_quotient(body: &Expr, var: &str, point: f64, params: &Params) -> Option<f64> {
    let Expr::Bin(BinOp::Div, numerator, denominator) = body else { return None };
    if !point.is_finite() {
        return None;
    }
    let value = |expr: &Expr| {
        let prog = Program::compile_with(expr, &[], params).ok()?;
        let mut out = [Complex64::default()];
        prog.eval_batch(&[], &mut out);
        Some(out[0])
    };
    let center = Complex64::new(point, 0.0);
    let d = crate::series::taylor(denominator, var, center, 4, &value)?;
    let order = d.iter().position(|c| *c != Complex64::default())?;
    if order == 0 || d.iter().any(|c| c.im != 0.0) {
        return None;
    }
    let n = crate::series::taylor(numerator, var, center, order, &value)?;
    if n.iter().any(|c| c.im != 0.0) || n[..order].iter().any(|c| c.re != 0.0) {
        return None;
    }
    let result = n[order].re / d[order].re;
    result.is_finite().then_some(result)
}

fn close(a: f64, b: f64) -> bool {
    a.is_finite() && b.is_finite() && (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// 差がほぼ等比で縮むときだけ、Aitken の Δ² で残りの変化を外挿する。
fn extrapolate([a, b, c]: [f64; 3]) -> Option<f64> {
    let first = b - a;
    let second = c - b;
    let ratio = second / first;
    if (0.0..0.95).contains(&ratio) {
        Some(c + second * ratio / (1.0 - ratio))
    } else {
        None
    }
}

fn finite_estimate(a: &[f64], b: &[f64]) -> Option<f64> {
    let mut stable = None;
    for (a, b) in a.windows(WINDOW).zip(b.windows(WINDOW)).rev() {
        if a.iter().chain(b).any(|x| !x.is_finite()) {
            continue;
        }
        let last = a[WINDOW - 1];
        if a.iter().chain(b).all(|&x| close(x, last)) {
            stable.get_or_insert(last);
            continue;
        }
        let Some(estimate) = extrapolate([a[0], a[1], a[2]]) else { continue };
        // 複数の距離・異なる間隔の列から、同じ値が得られる場合に限る。
        if a.windows(3)
            .chain(b.windows(3))
            .all(|w| extrapolate([w[0], w[1], w[2]]).is_some_and(|v| close(v, estimate)))
        {
            // 近い点が桁落ちで同じ値に丸められたときは、外挿と一致しない。
            return match stable {
                Some(last) => close(last, estimate).then_some(last),
                None => Some(estimate),
            };
        }
    }
    stable
}

fn growing(samples: &[f64]) -> Option<f64> {
    // オーバーフローの前までに、同じ符号で大きさが増え続けているか確かめる。
    let end = samples.iter().rposition(|x| x.is_finite())? + 1;
    let start = samples[..end].iter().rposition(|x| !x.is_finite()).map_or(0, |i| i + 1);
    let run = &samples[start..end];
    let tail = run.get(run.len().checked_sub(WINDOW)?..)?;
    let last = tail[WINDOW - 1];
    let geometric = last.abs() >= 1e6 * run[0].abs().max(1.0)
        && samples[end..].iter().all(|x| *x == last.signum() * f64::INFINITY)
        && tail.windows(2).all(|w| w[0].signum() == last.signum() && w[1].abs() > 1.25 * w[0].abs());
    // 対数の発散では、距離を倍にするごとの増分が一定になる。
    let delta = last - tail[WINDOW - 2];
    let logarithmic = end == samples.len()
        && run.len() >= 2 * WINDOW
        && last.abs() >= 8.0 * run[0].abs().max(1.0)
        && delta.signum() == last.signum()
        && delta.abs() > 0.0
        && run[run.len() - 2 * WINDOW..].windows(2).all(|w| ((w[1] - w[0]) / delta - 1.0).abs() < 1e-6);
    (geometric || logarithmic).then_some(last.signum() * f64::INFINITY)
}

fn oscillates(samples: &[f64]) -> bool {
    let span = |values: &[f64]| {
        let min = values.iter().copied().fold(f64::INFINITY, f64::min);
        let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (max - min, max.abs().max(min.abs()).max(1.0))
    };
    let tail = &samples[samples.len() - 4 * WINDOW..];
    if tail.iter().any(|v| !v.is_finite()) {
        return false;
    }
    let (before, after) = tail.split_at(tail.len() / 2);
    let (width, scale) = span(after);
    if !width.is_finite() || width < 1e-6 * scale || width < 0.5 * span(before).0 {
        return false;
    }
    let directions = after.windows(2).filter_map(|w| {
        let diff = w[1] - w[0];
        (diff.abs() >= 0.05 * width).then_some(diff.is_sign_positive())
    });
    directions.clone().zip(directions.skip(1)).filter(|(a, b)| a != b).count() >= 3
}

fn from_side(prog: &Program, point: f64, dir: f64) -> Behavior {
    let mut samples = [[0.0; STEPS]; 2];
    for (values, scale) in samples.iter_mut().zip([1.0, std::f64::consts::SQRT_2]) {
        let inputs = std::array::from_fn::<_, STEPS, _>(|k| {
            let step = 2.0_f64.powi(k as i32);
            if point.is_infinite() {
                dir * scale * step
            } else {
                point + dir * (point.abs().max(1.0) * (0.25 * scale / step))
            }
        });
        let mut out = [Complex64::default(); STEPS];
        prog.eval_batch(&[&inputs], &mut out);
        for ((v, x), y) in values.iter_mut().zip(inputs).zip(out) {
            // 実数の指数関数でも、桁あふれで複素数側の計算になると虚部が NaN になる。
            // その標本は有限の極限に使わず、桁あふれ前の増加を確認するときだけ使う。
            let real_or_overflow = y.im == 0.0 || (y.re.is_infinite() && y.im.is_nan());
            *v = if x != point && x.is_finite() && real_or_overflow { y.re } else { f64::NAN };
        }
    }
    let [a, b] = &samples;
    let value = finite_estimate(a, b).or_else(|| {
        let inf = growing(a)?;
        (growing(b)? == inf).then_some(inf)
    });
    if let Some(value) = value {
        Behavior::Value(value)
    } else if oscillates(a) && oscillates(b) {
        Behavior::Oscillates
    } else {
        Behavior::Unknown
    }
}

pub(crate) fn analyze(prog: &Program, variable: &str, target: f64, side: i8, removable: Option<f64>, graph: bool) -> Report {
    let finite = target.is_finite();
    let sample = |dir| removable.map_or_else(|| from_side(prog, target, dir), Behavior::Value);
    let left = if (finite && (graph || side <= 0)) || target == f64::NEG_INFINITY { sample(-1.0) } else { Behavior::Unknown };
    let right = if (finite && (graph || side >= 0)) || target == f64::INFINITY { sample(1.0) } else { Behavior::Unknown };
    let defined = if graph && finite {
        let mut out = [Complex64::default()];
        prog.eval_batch(&[&[target]], &mut out);
        (out[0].im == 0.0 && out[0].re.is_finite()).then_some(out[0].re)
    } else {
        None
    };
    Report { variable: variable.to_string(), target, side, left, right, defined }
}

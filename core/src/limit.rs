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
    pub(crate) sequence: bool,
    pub(crate) lesson: Vec<[f64; 2]>,
}

#[derive(Clone)]
pub enum Graph {
    Function(Program),
    Sampled(Vec<[f64; 2]>),
    Sequence(Vec<[f64; 2]>),
}

impl Report {
    pub(crate) fn finite_value(&self) -> Option<f64> {
        self.value().ok().filter(|v| v.is_finite() && (*v as f32).is_finite())
    }

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

/// Finite limit along a ray starting at an undefined point. A ray with no
/// defined samples close to its start contributes no marker.
pub(crate) fn finite_ray(radius: f64, sample: impl Fn(&[f64]) -> Vec<f64>) -> Option<f64> {
    let estimates = [1.0, std::f64::consts::SQRT_2].map(|scale| {
        let distances = std::array::from_fn::<_, STEPS, _>(|k| radius * scale * 0.5_f64.powi(k as i32));
        sample(&distances)
    });
    finite_estimate(&estimates[0], &estimates[1])
}

fn growing(samples: &[f64], growth: f64) -> Option<f64> {
    // オーバーフローの前までに、同じ符号で大きさが増え続けているか確かめる。
    let end = samples.iter().rposition(|x| x.is_finite())? + 1;
    let start = samples[..end].iter().rposition(|x| !x.is_finite()).map_or(0, |i| i + 1);
    let run = &samples[start..end];
    let tail = run.get(run.len().checked_sub(WINDOW)?..)?;
    let last = tail[WINDOW - 1];
    let geometric = last.abs() >= growth * run[0].abs().max(1.0)
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

/// 1/n の整数べきで近づく列を外挿する。各列の添字は倍々なので、補正の係数は 2^k になる。
fn sequence_extrapolate(a: &[f64], b: &[f64]) -> Option<f64> {
    let extrapolate_tail = |ys: &[f64]| {
        let mut values = ys[ys.len() - 6..].to_vec();
        for order in 1..=3 {
            let factor = f64::from((1 << order) - 1);
            values = values.windows(2).map(|w| w[1] + (w[1] - w[0]) / factor).collect();
        }
        values
    };
    let (a, b) = (extrapolate_tail(a), extrapolate_tail(b));
    let estimate = *a.last()?;
    a.iter().chain(&b).all(|&v| close(v, estimate)).then_some(estimate)
}

fn from_side(
    sample: &mut impl FnMut(&[f64]) -> Result<Vec<f64>, String>,
    point: f64,
    dir: f64,
) -> Result<Behavior, String> {
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
        for ((v, x), y) in values.iter_mut().zip(inputs).zip(sample(&inputs)?) {
            *v = if x != point && x.is_finite() { y } else { f64::NAN };
        }
    }
    let [a, b] = &samples;
    let value = finite_estimate(a, b).or_else(|| {
        let inf = growing(a, 1e6)?;
        (growing(b, 1e6)? == inf).then_some(inf)
    });
    Ok(if let Some(value) = value {
        Behavior::Value(value)
    } else if oscillates(a) && oscillates(b) {
        Behavior::Oscillates
    } else {
        Behavior::Unknown
    })
}

pub(crate) fn real_sample(y: Complex64) -> f64 {
    // 実数の指数関数でも、桁あふれで複素数側の計算になると虚部が NaN になる。
    if y.im == 0.0 || (y.re.is_infinite() && y.im.is_nan()) { y.re } else { f64::NAN }
}

pub(crate) fn program_samples(prog: &Program, xs: &[f64]) -> Vec<f64> {
    let mut values = vec![Complex64::default(); xs.len()];
    prog.eval_batch(&[xs], &mut values);
    values.into_iter().map(real_sample).collect()
}

pub(crate) fn analyze(
    sample: &mut impl FnMut(&[f64]) -> Result<Vec<f64>, String>,
    variable: &str,
    target: f64,
    side: i8,
    removable: Option<f64>,
    graph: bool,
) -> Result<Report, String> {
    let finite = target.is_finite();
    let mut approach = |dir| removable.map_or_else(|| from_side(sample, target, dir), |v| Ok(Behavior::Value(v)));
    let left = if (finite && (graph || side <= 0)) || target == f64::NEG_INFINITY { approach(-1.0)? } else { Behavior::Unknown };
    let right = if (finite && (graph || side >= 0)) || target == f64::INFINITY { approach(1.0)? } else { Behavior::Unknown };
    let defined = if graph && finite {
        let v = sample(&[target])?[0];
        v.is_finite().then_some(v)
    } else {
        None
    };
    let mut report = Report { variable: variable.to_string(), target, side, left, right, defined, sequence: false, lesson: Vec::new() };
    if graph && finite {
        let radius = (target.abs() / 8.0).max(1.0);
        let xs: Vec<_> = (-64i32..=64).filter(|&i| i != 0)
            .map(|i| target + radius * (f64::from(i) / 64.0).powi(3))
            .filter(|&x| x.is_finite() && x != target)
            .collect();
        report.lesson = sample(&xs)?.into_iter().zip(xs).map(|(y, x)| [x, y]).collect();
    }
    Ok(report)
}

/// 数列は整数の位置だけで調べる。標本を連続して取る区間も設け、等比に増やした添字が周期と重なる場合を検出する。
pub(crate) fn sequence(
    sample: &mut impl FnMut(&[f64]) -> Result<Vec<f64>, String>,
    variable: &str,
    graph: bool,
    start: f64,
) -> Result<(Option<Graph>, Report), String> {
    match sequence_through(sample, variable, graph, start, 4_096) {
        Err(e) if e.contains("項が多すぎます") || e.contains("項の数を減らしてください") => {
            sequence_through(sample, variable, graph, start, 512)
        }
        result => result,
    }
}

fn sequence_through(
    sample: &mut impl FnMut(&[f64]) -> Result<Vec<f64>, String>, variable: &str, graph: bool, start: f64, last: usize,
) -> Result<(Option<Graph>, Report), String> {
    const BLOCK: usize = 24;
    let offset = start - 1.0;
    let indices: Vec<f64> = (2..=last.ilog2()).map(|k| offset + f64::from(1 << k)).collect();
    let shifted: Vec<f64> = (2..=last.ilog2()).map(|k| offset + f64::from(3 << (k - 2))).collect();
    let tail = |end: usize| -> Vec<f64> { (end - BLOCK + 1..=end).map(|n| offset + n as f64).collect() };
    let a = sample(&indices)?;
    let b = sample(&shifted)?;
    let near = sample(&tail(last))?;
    let earlier = sample(&tail(last / 4))?;
    let max_error = |ys: &[f64], value: f64| {
        ys.iter().map(|y| if y.is_finite() { (y - value).abs() } else { f64::INFINITY }).fold(0.0, f64::max)
    };
    // 急に収束する列では、初期の外挿より十分離れた 2 つの連続区間の一致を優先する。
    let plateau = a.last().copied().filter(|&v| {
        a.iter().rev().take(3).chain(b.iter().rev().take(3)).chain(&earlier).chain(&near).all(|&y| close(y, v))
    });
    let finite = plateau.or_else(|| finite_estimate(&a, &b))
        .or_else(|| sequence_extrapolate(&a, &b))
        .filter(|&v| {
            let previous = max_error(&earlier, v);
            let last = max_error(&near, v);
            last.is_finite() && (last <= 1e-8 * v.abs().max(1.0) || (previous.is_finite() && last < 0.8 * previous))
        });
    let width = |v: &[f64]| max_error(v, v[0]);
    let divergent = || {
        let sign = growing(&a, 32.0)?;
        if growing(&b, 32.0)? != sign { return None; }
        let bound = a.iter().rev().find(|y| y.is_finite())?.abs() / 4.0;
        near.iter().all(|v| v.signum() == sign.signum() && v.abs() > bound).then_some(sign)
    };
    let right = if let Some(v) = finite.or_else(divergent) {
        Behavior::Value(v)
    } else if oscillates(&near) && width(&near) >= 0.5 * width(&earlier) {
        Behavior::Oscillates
    } else {
        Behavior::Unknown
    };
    let mut report = Report {
        variable: variable.to_string(), target: f64::INFINITY, side: 0, left: Behavior::Unknown, right, defined: None,
        sequence: true, lesson: Vec::new(),
    };
    if !graph {
        return Ok((None, report));
    }
    let xs: Vec<_> = (0..256).map(|i| start + f64::from(i)).collect();
    let mut shown: Vec<_> = sample(&xs)?.into_iter().zip(xs).map(|(y, x)| [x, y]).collect();
    for (xs, ys) in [(indices, a), (shifted, b), (tail(last / 4), earlier), (tail(last), near)] {
        shown.extend(xs.into_iter().zip(ys).map(|(x, y)| [x, y]));
    }
    shown.sort_by(|a, b| a[0].total_cmp(&b[0]));
    shown.dedup_by(|a, b| a[0] == b[0]);
    report.lesson = shown.clone();
    Ok((Some(Graph::Sequence(shown)), report))
}

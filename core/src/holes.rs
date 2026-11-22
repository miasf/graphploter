//! Finite endpoints omitted from explicit graphs.

use crate::critical::{self, Class};
use crate::eval::{as_real, Params, Program};
use crate::functions::Func;
use crate::limit::{finite_ray, removable_quotient};
use crate::mesh::Domain;
use crate::parser::{BinOp, Expr};
use num_complex::Complex64;

const CURVE_SAMPLES: usize = 400;
const MAX_MARKS: usize = 128;

pub(crate) struct Break {
    pub x: f64,
    pub sides: [Option<f64>; 2],
}

fn samples(program: &Program, inputs: &[&[f64]]) -> Vec<f64> {
    let mut values = vec![Complex64::default(); inputs[0].len()];
    program.eval_batch(inputs, &mut values);
    values.into_iter().map(as_real).collect()
}

pub(crate) fn curve_samples(program: &Program, xs: &[f64]) -> Vec<f64> {
    samples(program, &[xs, &vec![0.0; xs.len()]])
}

fn add_factors(expr: &Expr, out: &mut Vec<Expr>) {
    match expr {
        Expr::Bin(BinOp::Mul, a, b) => {
            add_factors(a, out);
            add_factors(b, out);
        }
        Expr::Neg(a) | Expr::Bin(BinOp::Pow, a, _) => add_factors(a, out),
        Expr::Call(Func::Abs | Func::Sqrt, args) => add_factors(&args[0], out),
        Expr::Num(_) | Expr::Const(_) => {}
        _ if !out.contains(expr) => out.push(expr.clone()),
        _ => {}
    }
}

fn boundaries(expr: &Expr) -> Vec<Expr> {
    let guards = std::cell::RefCell::new(Vec::new());
    // Expanded piecewise branches store their comparison margins in the call.
    let _ = expr.rewrite(&|node| {
        let mut out = guards.borrow_mut();
        match node {
            Expr::Bin(BinOp::Div, _, den) | Expr::Bin(BinOp::Pow, den, _) => add_factors(den, &mut out),
            Expr::Call(Func::Ln | Func::Sqrt, args) => {
                for arg in args {
                    add_factors(arg, &mut out);
                }
            }
            Expr::Call(Func::Piece, args) => {
                let mut rest = args.as_slice();
                while let [Expr::Num(count), _, tail @ ..] = rest {
                    let count = *count as usize;
                    for margin in &tail[..count] {
                        add_factors(margin, &mut out);
                    }
                    rest = &tail[count + 1..];
                }
            }
            _ => {}
        }
        None
    });
    guards.into_inner()
}

fn roots_on_grid(guard: &Program, grid: &[f64]) -> Vec<(f64, bool)> {
    let values = samples(guard, &[grid]);
    let mut roots = Vec::new();
    for (xs, ys) in grid.windows(2).zip(values.windows(2)) {
        let [mut a, mut b] = [xs[0], xs[1]];
        let [mut fa, mut fb] = [ys[0], ys[1]];
        if fa == 0.0 && fb.is_finite() && fb != 0.0 {
            roots.push((a, true));
        }
        if fb == 0.0 && fa.is_finite() && fa != 0.0 {
            roots.push((b, true));
        }
        if !fa.is_finite()
            || !fb.is_finite()
            || fa == 0.0
            || fb == 0.0
            || fa.is_sign_positive() == fb.is_sign_positive()
        {
            continue;
        }
        let initial = fa.abs().max(fb.abs());
        for _ in 0..64 {
            let m = a.midpoint(b);
            if m == a || m == b {
                break;
            }
            let fm = samples(guard, &[&[m]])[0];
            if fm == 0.0 {
                a = m;
                fa = 0.0;
                break;
            }
            if !fm.is_finite() {
                break;
            }
            if fm.is_sign_positive() == fa.is_sign_positive() {
                a = m;
                fa = fm;
            } else {
                b = m;
                fb = fm;
            }
        }
        if fa == 0.0 {
            roots.push((a, true));
        } else if fa.abs().max(fb.abs()) <= 1e-10 * initial {
            // A sign change across a pole or a jump does not converge to zero.
            roots.push((if fa.abs() < fb.abs() { a } else { b }, false));
        }
    }
    roots
}

pub(crate) fn curve(
    expr: &Expr,
    variable: &str,
    params: &Params,
    range: (f64, f64),
    sample: impl Fn(&[f64]) -> Vec<f64>,
) -> Vec<Break> {
    let (lo, hi) = range;
    let width = hi - lo;
    if !width.is_finite() || width <= 0.0 {
        return Vec::new();
    }
    let grid: Vec<_> = (0..CURVE_SAMPLES)
        .map(|i| lo + width * (i as f64 / CURVE_SAMPLES as f64))
        .chain(std::iter::once(hi))
        .collect();
    let values = sample(&grid);
    let mut candidates: Vec<_> = values
        .windows(3)
        .zip(&grid[1..])
        .filter_map(|(ys, &x)| (ys[0].is_finite() && !ys[1].is_finite() && ys[2].is_finite()).then_some(x))
        .collect();
    let mut approximate = Vec::new();
    for boundary in boundaries(expr) {
        if let Ok(guard) = Program::compile_with(&boundary, &[variable], params) {
            if guard.uses_var_index(0) {
                let mut at_zero = None;
                for (x, exact) in roots_on_grid(&guard, &grid) {
                    if exact {
                        candidates.push(x);
                    } else {
                        let program = at_zero.get_or_insert_with(|| {
                            let zeroed = expr.rewrite(&|e| (*e == boundary).then_some(Expr::Num(0.0)));
                            Program::compile_with(&zeroed, &[variable], params).ok()
                        });
                        if program.as_ref().is_some_and(|p| !samples(p, &[&[x]])[0].is_finite()) {
                            approximate.push(x);
                        }
                    }
                }
            }
        }
        for root in crate::solve::roots(&boundary, variable, &[variable], params).into_iter().flatten() {
            if let Ok(program) = Program::compile_with(&root, &[], params) {
                let mut value = [Complex64::default()];
                program.eval_batch(&[], &mut value);
                let x = as_real(value[0]);
                if (lo..=hi).contains(&x) {
                    candidates.push(x);
                }
            }
        }
    }
    candidates.sort_by(f64::total_cmp);
    candidates.dedup();
    let defined = sample(&candidates);
    let mut candidates: Vec<_> =
        candidates.into_iter().zip(defined).filter_map(|(x, y)| (!y.is_finite()).then_some(x)).collect();
    approximate.retain(|x| !candidates.iter().any(|y| (x - y).abs() <= 8.0 * f64::EPSILON * x.abs().max(1.0)));
    candidates.extend(approximate);
    candidates.sort_by(f64::total_cmp);
    candidates.dedup_by(|a, b| (*a as f32) == (*b as f32));
    candidates
        .iter()
        .enumerate()
        .filter_map(|(i, &x)| {
            let removable = removable_quotient(expr, variable, x, params);
            let sides = [-1.0, 1.0].map(|direction| {
                let neighbor = if direction < 0.0 {
                    i.checked_sub(1).map(|i| candidates[i])
                } else {
                    candidates.get(i + 1).copied()
                };
                let radius = neighbor.map_or(width, |other| (other - x).abs()).min(x.abs().max(1.0)) / 8.0;
                removable.or_else(|| {
                    finite_ray(radius, |ds| {
                        let xs: Vec<_> = ds.iter().map(|d| x + direction * d).collect();
                        sample(&xs).into_iter().zip(xs).map(|(y, t)| if t == x { f64::NAN } else { y }).collect()
                    })
                })
            });
            sides.iter().any(Option::is_some).then_some(Break { x, sides })
        })
        .take(MAX_MARKS)
        .collect()
}

fn isolated_zeros<const N: usize>(
    expr: &Expr,
    vars: &[String; N],
    bounds: &[(f64, f64); N],
    params: &Params,
) -> Vec<[f64; N]> {
    if !vars.iter().all(|var| expr.uses_var(var)) {
        return Vec::new();
    }
    let Ok(found) = critical::find(expr, vars, bounds, params) else { return Vec::new() };
    let names = vars.each_ref().map(String::as_str);
    let Ok(program) = Program::compile_with(expr, &names, params) else { return Vec::new() };
    let steps = bounds.map(|(lo, hi)| (hi - lo) * 1e-3);
    found
        .points
        .into_iter()
        .filter_map(|point| {
            if !matches!(point.class, Class::Min | Class::Max) {
                return None;
            }
            let center: [f64; N] = std::array::from_fn(|axis| point.point[axis]);
            let probes: [Vec<f64>; N] = std::array::from_fn(|axis| {
                let mut xs = vec![center[axis]; 2 * N];
                xs[2 * axis] -= steps[axis];
                xs[2 * axis + 1] += steps[axis];
                xs
            });
            let values = samples(&program, &probes.each_ref().map(Vec::as_slice));
            let sign = if point.class == Class::Min { 1.0 } else { -1.0 };
            values
                .iter()
                .all(|&value| {
                    value.is_finite() && sign * value > 0.0 && point.value.abs() <= 64.0 * f64::EPSILON * value.abs()
                })
                .then_some(center)
        })
        .collect()
}

pub(crate) fn curve_exclusions(
    expr: &Expr,
    program: &Program,
    names: &[String; 3],
    axis: usize,
    params: &Params,
    domain: Domain,
    exclusions: &[(Expr, Program)],
    slice: bool,
) -> Vec<[f64; 3]> {
    let free = usize::from(axis == 0);
    let vars = [names[free].clone()];
    let bounds = [(domain.lo[free], domain.hi[free])];
    let slice_value = |name: &str| (name == names[2]).then_some(Expr::Num(0.0));
    let expr = expr.substitute(&slice_value);
    let mut marks = Vec::new();
    for (exclusion, condition) in exclusions {
        if !slice && condition.uses_var_index(2) {
            continue;
        }
        let restricted = exclusion.substitute(&|name| {
            if name == names[axis] {
                Some(expr.clone())
            } else {
                slice_value(name)
            }
        });
        for [x] in isolated_zeros(&restricted, &vars, &bounds, params) {
            let y = curve_samples(program, &[x])[0];
            if y.is_finite() {
                let mut point = [0.0; 3];
                point[free] = x;
                point[axis] = y;
                marks.push(point);
            }
        }
    }
    marks
}

/// Compose the exclusion with the surface before testing for a tangent point.
/// Projecting the surface first loses the coordinate used by the exclusion.
pub(crate) fn surface(
    expr: &Expr,
    program: &Program,
    names: &[String; 3],
    axis: usize,
    params: &Params,
    domain: Domain,
    exclusions: &[(Expr, Program)],
) -> Vec<[f64; 3]> {
    let (a, b) = crate::equation::others(axis);
    let vars = [names[a].clone(), names[b].clone()];
    let bounds = [a, b].map(|k| (domain.lo[k], domain.hi[k]));
    let point = |[u, v]: [f64; 2], height| {
        let mut p = [0.0; 3];
        p[a] = u;
        p[b] = v;
        p[axis] = height;
        p
    };
    let mut marks = Vec::new();
    for (exclusion, _) in exclusions {
        let restricted = exclusion.substitute(&|name| (name == names[axis]).then(|| expr.clone()));
        for center in isolated_zeros(&restricted, &vars, &bounds, params) {
            let height = samples(program, &[&[center[0]], &[center[1]]])[0];
            if height.is_finite() {
                marks.push(point(center, height));
            }
        }
    }
    for guard in boundaries(expr) {
        for center in isolated_zeros(&guard, &vars, &bounds, params) {
            if samples(program, &[&[center[0]], &[center[1]]])[0].is_finite() {
                continue;
            }
            let radius = bounds.iter().map(|(lo, hi)| hi - lo).fold(f64::INFINITY, f64::min) / 8.0;
            let limits: Option<Vec<_>> = (0..12)
                .map(|i| {
                    let angle = std::f64::consts::TAU * i as f64 / 12.0;
                    finite_ray(radius, |ds| {
                        let xs: Vec<_> = ds.iter().map(|d| center[0] + d * angle.cos()).collect();
                        let ys: Vec<_> = ds.iter().map(|d| center[1] + d * angle.sin()).collect();
                        samples(program, &[&xs, &ys])
                            .into_iter()
                            .enumerate()
                            .map(|(j, value)| if xs[j] == center[0] && ys[j] == center[1] { f64::NAN } else { value })
                            .collect()
                    })
                })
                .collect();
            if let Some(values) = limits {
                let height = values[0];
                if values.iter().all(|v| (height - v).abs() <= 1e-7 * height.abs().max(1.0)) {
                    marks.push(point(center, height));
                }
            }
        }
    }
    marks
}

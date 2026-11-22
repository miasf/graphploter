//! 条件から入力側の図形を作り、その点・線分・三角形を集合の要素式で写す。

use crate::equation::{self, Plot, Space};
use crate::eval::{self, Params, Program, ProgramSet};
use crate::logic::Op;
use crate::mesh::{self, Domain, Mesh};
use crate::parser::{Chain, Expr, Formula, Rel};
use crate::{Built, Ctx};
use num_complex::Complex64;

pub struct Image {
    maps: Vec<Program>,
    parts: Vec<Part>,
    arity: usize,
    inputs: usize,
    uses: Vec<String>,
}

struct Part {
    source: Option<Plot>,
    bounds: [(Option<f64>, Option<f64>); 3],
}

impl Image {
    pub fn label(&self) -> &'static str {
        if self.arity == 1 {
            "像集合（数直線・近似）"
        } else {
            "像集合（近似）"
        }
    }

    pub fn used_params(&self) -> Vec<String> {
        self.uses.clone()
    }
}

fn constant(p: &Program) -> f64 {
    let mut out = [Complex64::default()];
    p.eval_batch(&[], &mut out);
    eval::as_real(out[0])
}

pub fn compile(point: &Expr, cond: &Formula, params: &Params, space: &Space) -> Result<Plot, String> {
    let point = crate::expand::expand(point, params)?;
    let items = match point {
        Expr::Tuple(items) => items,
        e => vec![e],
    };
    if items.len() > space.dims {
        return Err(format!("像集合の要素は {} 成分までです（次元を増やしてください）", space.dims));
    }
    let input_space = Space::standard();
    let coords = input_space.shown();
    let cond = crate::expand::expand_big(cond, params)?;
    let mut clauses = crate::logic::clauses(&cond, &coords, &crate::expand::set_def)?;
    for lit in clauses.iter_mut().flatten() {
        lit.l = crate::expand::expand(&lit.l, params)?;
        lit.r = crate::expand::expand(&lit.r, params)?;
    }
    let mut names = Vec::new();
    for expr in items.iter().chain(clauses.iter().flatten().flat_map(|l| [&l.l, &l.r])) {
        expr.collect_vars(&mut names);
    }
    names.retain(|n| !params.contains_key(n));
    if names.len() > coords.len() {
        return Err("像集合の入力変数は 3 つまでです".into());
    }
    let rename = |expr: &Expr| expr.substitute(&|n| names.iter().position(|p| p == n).map(|i| Expr::Var(coords[i].clone())));
    let refs = coords.each_ref().map(String::as_str);
    // 入力変数の付け替え先とスライダー名が衝突しないよう、式中のスライダーを先に別名にする。
    let mut consts = Params::new();
    let mut aliases = Vec::new();
    for (i, (name, value)) in params.iter().enumerate() {
        let alias = format!(" set image parameter {i}");
        aliases.push((alias.clone(), name.clone()));
        consts.insert(alias, *value);
    }
    let prepare = |expr: &Expr| {
        rename(&expr.substitute(&|n| {
            aliases
                .iter()
                .find(|(_, name)| name == n)
                .map(|(alias, _)| Expr::Var(alias.clone()))
        }))
    };
    let maps = items
        .iter()
        .map(|e| Program::compile_with(&prepare(e), &refs, &consts))
        .collect::<Result<Vec<_>, _>>()?;
    let mut uses = Vec::new();
    let mut record = |n: &str| {
        if let Some((_, name)) = aliases.iter().find(|(alias, _)| alias == n) {
            if !uses.contains(name) {
                uses.push(name.clone());
            }
        }
    };
    maps.iter().flat_map(Program::used_params).for_each(|n| record(n));
    let mut parts = Vec::new();
    'clause: for clause in clauses {
        let mut bounds = [(None, None); 3];
        let mut chains = Vec::new();
        let mut excluded = Vec::new();
        for lit in clause {
            let (l, r) = (prepare(&lit.l), prepare(&lit.r));
            let value = |e: &Expr| Program::compile_with(e, &[], &consts);
            let (lc, rc) = (value(&l).ok(), value(&r).ok());
            if let (Some(a), Some(b)) = (&lc, &rc) {
                a.used_params().iter().chain(b.used_params()).for_each(|n| record(n));
                let (a, b) = (constant(a), constant(b));
                let holds = !a.is_nan()
                    && !b.is_nan()
                    && match lit.op {
                        Op::Eq => a == b,
                        Op::Ne => a != b,
                        Op::Lt => a < b,
                        Op::Le => a <= b,
                    };
                if !holds {
                    continue 'clause;
                }
                continue;
            }
            if lit.op == Op::Ne {
                // 分割後の各枝は既存の不等式メッシュで描く。
                excluded.push((l, r));
                continue;
            }
            if let (Expr::Var(n), Some(c)) = (&l, &rc) {
                if let Some(i) = coords.iter().position(|x| x == n) {
                    bound(&mut bounds[i].1, constant(c), f64::min);
                    if lit.op == Op::Eq {
                        bound(&mut bounds[i].0, constant(c), f64::max);
                    }
                }
            }
            if let (Some(c), Expr::Var(n)) = (&lc, &r) {
                if let Some(i) = coords.iter().position(|x| x == n) {
                    bound(&mut bounds[i].0, constant(c), f64::max);
                    if lit.op == Op::Eq {
                        bound(&mut bounds[i].1, constant(c), f64::min);
                    }
                }
            }
            let rel = match lit.op {
                Op::Eq => Rel::Eq,
                Op::Lt => Rel::Lt,
                _ => Rel::Le,
            };
            chains.push(chain(l, rel, r));
        }
        if names.is_empty() {
            parts.push(Part { source: None, bounds });
            break;
        }
        if names.len() == 1 {
            chains.push(chain(Expr::Var(coords[1].clone()), Rel::Eq, Expr::Num(0.0)));
        } else if chains.is_empty() && excluded.is_empty() {
            // 全入力空間を描く。既存の領域コンパイラには座標を含む条件を渡す。
            let x = Expr::Var(coords[0].clone());
            chains.push(chain(x.clone(), Rel::Le, x));
        }
        let mut branches = vec![chains];
        for (l, r) in excluded {
            if branches.len() * 2 + parts.len() > crate::logic::MAX_CLAUSES {
                return Err("像集合の条件の分岐が多すぎます".into());
            }
            let mut reverse = branches.clone();
            branches.iter_mut().for_each(|cs| cs.push(chain(l.clone(), Rel::Lt, r.clone())));
            reverse.iter_mut().for_each(|cs| cs.push(chain(r.clone(), Rel::Lt, l.clone())));
            branches.extend(reverse);
        }
        for chains in branches {
            let source = equation::compile_chains(chains, &consts, &input_space)?;
            for n in source.used_params() {
                record(&n);
            }
            parts.push(Part {
                source: Some(source),
                bounds,
            });
        }
    }
    let arity = maps.len();
    Ok(Plot::SetImage(Box::new(Image {
        maps,
        parts,
        arity,
        inputs: names.len(),
        uses,
    })))
}

fn bound(end: &mut Option<f64>, value: f64, combine: fn(f64, f64) -> f64) {
    if value.is_finite() {
        *end = Some(end.map_or(value, |old| combine(old, value)));
    }
}

fn chain(l: Expr, rel: Rel, r: Expr) -> Chain {
    Chain {
        exprs: vec![l, r],
        rels: vec![rel],
        pos: vec![0; 2],
    }
}

fn source_domain(bounds: &[(Option<f64>, Option<f64>); 3], view: Domain) -> Option<Domain> {
    let mut domain = view;
    for (axis, &(lo, hi)) in bounds.iter().enumerate() {
        let radius = view.lo[axis].abs().max(view.hi[axis].abs()).max(4.0);
        let (lo, hi) = match (lo, hi) {
            (Some(a), Some(b)) if a > b => return None,
            (Some(a), Some(b)) => (a, b),
            (Some(a), None) => (a, radius.max(a + radius)),
            (None, Some(b)) => ((-radius).min(b - radius), b),
            (None, None) => (-radius, radius),
        };
        let pad = (hi - lo).max(radius * 1e-3) * 0.02;
        domain.lo[axis] = lo - pad;
        domain.hi[axis] = hi + pad;
    }
    Some(domain)
}

pub(super) fn build(image: &Image, ctx: &Ctx) -> Result<Built, String> {
    let source_space = Space::standard();
    let set = ProgramSet::new(&image.maps);
    let mut meshes = Vec::new();
    let mut intervals = Vec::new();
    let mut outline = Vec::new();
    for part in &image.parts {
        let Some(dom) = source_domain(&part.bounds, ctx.dom) else {
            continue;
        };
        let flat = image.inputs <= 2;
        let source_ctx = Ctx {
            space: &source_space,
            dom,
            view: source_space.view(0.0),
            flat,
            flat_slice: flat,
            size: (0..3).map(|a| (dom.hi[a] - dom.lo[a]).powi(2)).sum::<f64>().sqrt(),
            local: false,
            region_fog: false,
            complex_mode: mesh::ComplexMode::Off,
            ..ctx.clone()
        };
        let source = match &part.source {
            Some(plot) => crate::build(plot, &source_ctx)?,
            None => Built {
                inner: crate::system::points_mesh(&[[0.0; 3]]),
                curve: true,
                ..Default::default()
            },
        };
        let inner = match &source.shape {
            Some(sh) => mesh::project(sh, &source_ctx.view, dom),
            None => source.inner,
        };
        if inner.positions.is_empty() {
            continue;
        }
        let values = evaluate(&set, &inner.positions);
        if image.arity == 1 {
            scalar_intervals(&inner, &values, &mut intervals);
            // 立体の場合は境界の像だけでなく、内部の断面の像も採る。
            if let Some(Plot::System { eqs, cuts, .. }) = &part.source {
                if image.inputs == 3 && eqs.is_empty() {
                    let region = mesh::Field::region(cuts);
                    let n = ctx.implicit_resolution.clamp(12, 48);
                    for k in 1..n {
                        let z = dom.lo[2] + (dom.hi[2] - dom.lo[2]) * k as f64 / n as f64;
                        let slice = region.cap(dom, 2, z, true, n);
                        scalar_intervals(&slice, &evaluate(&set, &slice.positions), &mut intervals);
                    }
                }
            }
        } else {
            let mapped = project(&values, image.arity, ctx);
            if source.curve {
                outline.push(Mesh {
                    positions: mapped,
                    ..Default::default()
                });
            } else {
                let mut mapped = finite_triangles(mapped, inner.indices);
                mesh::compute_normals(&mut mapped);
                meshes.push(mapped);
            }
        }
    }
    if image.arity == 1 {
        intervals.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for (lo, hi) in intervals {
            if let Some(last) = merged.last_mut().filter(|last| lo <= last.1) {
                last.1 = last.1.max(hi);
            } else {
                merged.push((lo, hi));
            }
        }
        let values: Vec<_> = merged.into_iter().flat_map(|(lo, hi)| [lo, hi, f64::NAN]).collect();
        outline.push(Mesh {
            positions: project(&values, 1, ctx),
            ..Default::default()
        });
    }
    let surface = !meshes.is_empty();
    let mut inner = if surface {
        mesh::merge(meshes, false)
    } else {
        mesh::merge(std::mem::take(&mut outline), true)
    };
    let lines = mesh::merge(outline, true);
    let mut curve = !surface;
    if ctx.flat {
        let slice = ctx.flat_slice && inner.positions.chunks_exact(3).any(|p| p[2].abs() > 1e-7);
        inner = crate::flat_finish(inner, curve, slice);
        curve |= slice;
    }
    let lines = if ctx.flat { crate::flat_finish(lines, true, false) } else { lines };
    Ok(Built {
        inner,
        curve,
        outline: (lines.positions, Vec::new()),
        kind: image.label().into(),
        ..Default::default()
    })
}

fn finite_triangles(positions: Vec<f32>, indices: Vec<u32>) -> Mesh {
    if positions.iter().all(|v| v.is_finite()) {
        return Mesh {
            positions,
            indices,
            ..Default::default()
        };
    }
    let mut out = Mesh::default();
    let mut remap = Vec::with_capacity(positions.len() / 3);
    for p in positions.chunks_exact(3) {
        let index = p.iter().all(|v| v.is_finite()).then(|| {
            let i = (out.positions.len() / 3) as u32;
            out.positions.extend_from_slice(p);
            i
        });
        remap.push(index);
    }
    for tri in indices.chunks_exact(3) {
        if let (Some(a), Some(b), Some(c)) = (remap[tri[0] as usize], remap[tri[1] as usize], remap[tri[2] as usize]) {
            out.indices.extend([a, b, c]);
        }
    }
    out
}

fn evaluate(set: &ProgramSet, positions: &[f32]) -> Vec<f64> {
    let inputs: [Vec<f64>; 3] = [0, 1, 2].map(|a| positions.chunks_exact(3).map(|p| p[a] as f64).collect());
    let mut values = Vec::new();
    set.eval_points(&inputs.each_ref().map(Vec::as_slice), positions.len() / 3, &mut values);
    for (p, mapped) in positions.chunks_exact(3).zip(values.chunks_exact_mut(set.len())) {
        if p.iter().any(|v| !v.is_finite()) {
            mapped.fill(f64::NAN);
        }
    }
    values
}

fn scalar_intervals(mesh: &Mesh, values: &[f64], out: &mut Vec<(f64, f64)>) {
    // 連結でない入力の像を、ひとつの最小値・最大値でつながない。
    let mut add = |values: &[f64]| {
        let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if lo <= hi {
            out.push((lo, hi));
        }
    };
    if mesh.indices.is_empty() {
        let mut previous = f64::NAN;
        for &v in values {
            if v.is_finite() {
                add(&[previous, v]);
            }
            previous = v;
        }
    } else {
        for tri in mesh.indices.chunks_exact(3) {
            add(&[values[tri[0] as usize], values[tri[1] as usize], values[tri[2] as usize]]);
        }
    }
}

fn project(values: &[f64], arity: usize, ctx: &Ctx) -> Vec<f32> {
    let mut point = vec![0.0; ctx.space.dims];
    values
        .chunks_exact(arity)
        .flat_map(|p| {
            if p.iter().all(|v| v.is_finite()) {
                point[..arity].copy_from_slice(p);
                let q = ctx.view.apply(&point).map(|q| q as f32);
                if q.iter().all(|v| v.is_finite()) {
                    return q;
                }
            }
            [f32::NAN; 3]
        })
        .collect()
}

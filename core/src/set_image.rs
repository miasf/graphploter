//! 条件から入力側の図形を作り、その点・線分・三角形を集合の要素式で写す。

use crate::coords::{evaluate_map, refine_mapped};
use crate::equation::{self, Plot, Space};
use crate::eval::{self, Params, Program, ProgramSet};
use crate::interval::{Dec, Interval};
use crate::logic::Op;
use crate::mesh::{self, Domain, Mesh};
use crate::parser::{BinOp, Chain, Expr, Formula, Rel};
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

// 普通の座標名以外は、所属や範囲の連鎖で明示した名前を入力にできる。
pub(super) fn declared_inputs(formula: &Formula, params: &Params, inputs: &mut Vec<String>) {
    match formula {
        Formula::In(point @ Expr::Var(_), _) => point.collect_vars(inputs),
        Formula::In(Expr::Tuple(items), _) if items.iter().all(|e| matches!(e, Expr::Var(_))) => {
            items.iter().for_each(|e| e.collect_vars(inputs));
        }
        Formula::Chain(chain) => {
            for (terms, rels) in chain.exprs.windows(3).zip(chain.rels.windows(2)) {
                let ascending = |&r| matches!(r, Rel::Lt | Rel::Le);
                let descending = |&r| matches!(r, Rel::Gt | Rel::Ge);
                if (rels.iter().all(ascending) || rels.iter().all(descending))
                    && matches!(terms[1], Expr::Var(_))
                    && [&terms[0], &terms[2]].iter().any(|e| Program::compile_with(e, &[], params).is_ok())
                {
                    terms[1].collect_vars(inputs);
                }
            }
        }
        Formula::And(parts) | Formula::Or(parts) => parts.iter().for_each(|p| declared_inputs(p, params, inputs)),
        Formula::Xor(a, b) | Formula::Iff(a, b) | Formula::Implies(a, b) | Formula::Nand(a, b) | Formula::Nor(a, b) => {
            declared_inputs(a, params, inputs);
            declared_inputs(b, params, inputs);
        }
        Formula::Not(inner) => declared_inputs(inner, params, inputs),
        _ => {}
    }
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
    let mut declared = Vec::new();
    declared_inputs(&cond, params, &mut declared);
    if let Some(name) = names.iter().find(|n| {
        equation::coord_index(n).is_none()
            && !equation::is_param_var(n)
            && !space.coord_names.as_ref().is_some_and(|names| names.contains(n))
            && !declared.contains(n)
    }) {
        return Err(format!(
            "変数 '{name}' はここでは使えません（パラメータとして定義するか、条件で入力変数の範囲を指定してください）"
        ));
    }
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
        let mut chains = Vec::new();
        let mut excluded = Vec::new();
        // x ∈ ℤ（logic.rs が sin(π x) = 0 に書き直したもの）の x の番号と、元の式
        let mut integers: Vec<(usize, Chain)> = Vec::new();
        for lit in clause {
            let (l, r) = (prepare(&lit.l), prepare(&lit.r));
            if lit.op == Op::Eq && matches!(r, Expr::Num(v) if v == 0.0) {
                if let Some(i) = integer_coord(&l, &coords) {
                    integers.push((i, chain(l.clone(), Rel::Eq, r.clone())));
                    continue;
                }
            }
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
            let rel = match lit.op {
                Op::Eq => Rel::Eq,
                Op::Lt => Rel::Lt,
                _ => Rel::Le,
            };
            chains.push(chain(l, rel, r));
        }
        if names.is_empty() {
            parts.push(Part {
                source: None,
                bounds: [(None, None); 3],
            });
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
        if !integers.is_empty() {
            // 整数の座標は、範囲の中の整数 k ごとの x = k の枝にする（#146 #147）。sin(π x) = 0 の曲線の交わりでは、
            // 開区間の端の整数が切り取られず、格子に乗る y = 0 との交点が消えた。範囲が分からなければ、今までの sin(π x) = 0 のまま
            match integer_branches(&branches[0], &integers, &coords, &consts) {
                Some(b) => branches = b,
                None => branches[0].extend(integers.into_iter().map(|(_, c)| c)),
            }
            if branches.is_empty() {
                continue 'clause;
            }
        }
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
            let bounds = linear_bounds(&chains, &coords, &consts);
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

/// sin(π x)（x ∈ ℤ を書き直した式の左辺）の x が座標なら、その番号
fn integer_coord(e: &Expr, coords: &[String; 3]) -> Option<usize> {
    use crate::functions::Func;
    use crate::parser::Constant;
    let Expr::Call(Func::Sin, args) = e else { return None };
    let [Expr::Bin(BinOp::Mul, a, b)] = args.as_slice() else { return None };
    let v = match (&**a, &**b) {
        (Expr::Const(Constant::Pi), Expr::Var(v)) | (Expr::Var(v), Expr::Const(Constant::Pi)) => v,
        _ => return None,
    };
    coords.iter().position(|c| c == v)
}

/// 整数の座標ごとに、範囲（ほかの 1 次の条件）の中の整数 k の枝 {x = k, 残りの条件に x = k を入れたもの} を作る。
/// 数だけになった条件はその場で確かめる（−3 < x の x = −3 は捨てる）。範囲が分からない・枝が 2,000 を超えるなら None
fn integer_branches(chains: &[Chain], integers: &[(usize, Chain)], coords: &[String; 3], consts: &Params) -> Option<Vec<Vec<Chain>>> {
    const MAX_BRANCHES: usize = 2000;
    let mut branches = vec![chains.to_vec()];
    let mut done: Vec<usize> = Vec::new();
    for &(i, _) in integers {
        if done.contains(&i) {
            continue;
        }
        done.push(i);
        let mut next = Vec::new();
        for chains in &branches {
            let (Some(lo), Some(hi)) = linear_bounds(chains, coords, consts)[i] else { return None };
            let (from, to) = ((lo - 1e-9 * lo.abs().max(1.0)).ceil(), (hi + 1e-9 * hi.abs().max(1.0)).floor());
            if to - from + 1.0 > MAX_BRANCHES as f64 {
                return None;
            }
            let mut k = from;
            'k: while k <= to {
                let put = |e: &Expr| e.substitute(&|n| (n == coords[i]).then_some(Expr::Num(k)));
                let mut out = vec![chain(Expr::Var(coords[i].clone()), Rel::Eq, Expr::Num(k))];
                for c in chains {
                    let (l, r) = (put(&c.exprs[0]), put(&c.exprs[1]));
                    let value = |e: &Expr| Program::compile_with(e, &[], consts).ok().map(|p| constant(&p));
                    match (value(&l), value(&r)) {
                        (Some(a), Some(b)) => {
                            let holds = match c.rels[0] {
                                Rel::Eq => a == b,
                                Rel::Lt => a < b,
                                _ => a <= b,
                            };
                            if !holds {
                                k += 1.0;
                                continue 'k;
                            }
                        }
                        _ => out.push(chain(l, c.rels[0], r)),
                    }
                }
                next.push(out);
                if next.len() > MAX_BRANCHES {
                    return None;
                }
                k += 1.0;
            }
        }
        branches = next;
    }
    Some(branches)
}

// [x, y, z, 1] の係数。疎な単項の展開をせず、一次であることを式の構造から確かめる。
fn affine(e: &Expr, coords: &[String; 3], params: &Params) -> Option<[f64; 4]> {
    let value = |e| {
        Program::compile_with(e, &[], params)
            .ok()
            .map(|p| constant(&p))
            .filter(|v| v.is_finite())
    };
    let coefs = |e| affine(e, coords, params);
    Some(match e {
        Expr::Var(name) if coords.contains(name) => [
            f64::from(name == &coords[0]),
            f64::from(name == &coords[1]),
            f64::from(name == &coords[2]),
            0.0,
        ],
        Expr::Neg(inner) => coefs(inner)?.map(|v| -v),
        Expr::Bin(BinOp::Add | BinOp::Sub, l, r) => {
            let a = coefs(l)?;
            let b = coefs(r)?;
            let sign = if matches!(e, Expr::Bin(BinOp::Add, ..)) { 1.0 } else { -1.0 };
            [0, 1, 2, 3].map(|i| a[i] + sign * b[i])
        }
        Expr::Bin(BinOp::Mul, l, r) => {
            if let Some(k) = value(l) {
                coefs(r)?.map(|v| k * v)
            } else {
                let k = value(r)?;
                coefs(l)?.map(|v| k * v)
            }
        }
        Expr::Bin(BinOp::Div, l, r) => {
            let k = value(r).filter(|&k| k != 0.0)?;
            coefs(l)?.map(|v| v / k)
        }
        _ => [0.0, 0.0, 0.0, value(e)?],
    })
}

type Bounds = [(Option<f64>, Option<f64>); 3];

// u ≤ v, v ≤ 50 などでは、v の上限を u の探索範囲にも反映する。
fn linear_bounds(chains: &[Chain], coords: &[String; 3], params: &Params) -> Bounds {
    let mut rows = Vec::new();
    for chain in chains {
        let [l, r] = chain.exprs.as_slice() else { continue };
        if let (Some(l), Some(r)) = (affine(l, coords, params), affine(r, coords, params)) {
            let row = [0, 1, 2, 3].map(|i| l[i] - r[i]);
            if row.iter().all(|v| v.is_finite()) {
                rows.push(row);
                if chain.rels == [Rel::Eq] {
                    rows.push(row.map(|v| -v));
                }
            }
        }
    }
    let mut bounds: Bounds = [(None, None); 3];
    for _ in 0..6 {
        let previous = bounds;
        for row in &rows {
            for axis in (0..3).filter(|&i| row[i] != 0.0) {
                let rest = (0..3).filter(|&i| i != axis && row[i] != 0.0).try_fold(row[3], |sum, i| {
                    let (lo, hi) = bounds[i];
                    let end = if row[i] > 0.0 { lo } else { hi };
                    end.map(|v| sum + row[i] * v)
                });
                if let Some(end) = rest.map(|v| -v / row[axis]) {
                    if row[axis] > 0.0 {
                        bound(&mut bounds[axis].1, end, f64::min);
                    } else {
                        bound(&mut bounds[axis].0, end, f64::max);
                    }
                }
            }
        }
        if bounds == previous {
            break;
        }
    }
    bounds
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
            Some(Plot::System { base: None, eqs, cuts, .. }) if flat && eqs.is_empty() && !cuts.is_empty() => {
                let field = mesh::Field::region(cuts);
                let n = (ctx.implicit_resolution * 4).clamp(8, 1200) / 2;
                let make = |d| field.cap(d, 2, 0.0, true, n);
                let inner = if ctx.refine { crate::fit_to_shape(dom, n, make) } else { make(dom) };
                Built {
                    inner,
                    ..Default::default()
                }
            }
            Some(plot) => crate::build(plot, &source_ctx)?,
            None => Built {
                inner: crate::system::points_mesh(&[[0.0; 3]]),
                curve: true,
                ..Default::default()
            },
        };
        let mut inner = match &source.shape {
            Some(sh) => mesh::project(sh, &source_ctx.view, dom),
            None => source.inner,
        };
        if inner.positions.is_empty() {
            continue;
        }
        // 平面内の三角形の辺は、そのまま分割しても入力領域の近似から外れない。
        if flat && !source.curve && !inner.indices.is_empty() {
            weld_planar_vertices(&mut inner);
        }
        let mut values = evaluate_map(&set, &inner.positions);
        let can_split = if source.curve { image.inputs == 1 } else { flat };
        if ctx.refine && image.arity > 1 && can_split && refine_mapped(&mut inner, &values, image.arity, source.curve, image.arity) {
            values = evaluate_map(&set, &inner.positions);
        }
        let bounds = [0, 1, 2].map(|axis| Interval::new(dom.lo[axis], dom.hi[axis]));
        let guards: Vec<_> = image
            .maps
            .iter()
            .enumerate()
            .filter(|(_, p)| p.supports_interval() && p.eval_interval_dec(&bounds).dec < Dec::Dac)
            .collect();
        if image.arity == 1 {
            scalar_intervals(&inner, &values, &guards, &mut intervals);
            // 立体の場合は境界の像だけでなく、内部の断面の像も採る。
            if let Some(Plot::System { eqs, cuts, .. }) = &part.source {
                if image.inputs == 3 && eqs.is_empty() {
                    let region = mesh::Field::region(cuts);
                    let n = ctx.implicit_resolution.clamp(12, 48);
                    for k in 1..n {
                        let z = dom.lo[2] + (dom.hi[2] - dom.lo[2]) * k as f64 / n as f64;
                        let slice = region.cap(dom, 2, z, true, n);
                        scalar_intervals(&slice, &evaluate_map(&set, &slice.positions), &guards, &mut intervals);
                    }
                }
            }
        } else {
            let mapped = project(&values, image.arity, ctx);
            if source.curve {
                outline.push(Mesh {
                    positions: mapped_curve(mapped, &inner.positions, &values, image.arity, &guards),
                    ..Default::default()
                });
            } else {
                let mut indices = inner.indices;
                if !guards.is_empty() {
                    indices = indices
                        .chunks_exact(3)
                        .filter(|tri| {
                            join_vertices(
                                &[tri[0] as usize, tri[1] as usize, tri[2] as usize],
                                &inner.positions,
                                &values,
                                image.arity,
                                &guards,
                            )
                        })
                        .flatten()
                        .copied()
                        .collect();
                }
                let mut mapped = finite_triangles(mapped, indices);
                if let Some(curves) = mesh::collapsed_triangles(&mapped) {
                    outline.push(curves);
                } else {
                    mesh::compute_normals(&mut mapped);
                    meshes.push(mapped);
                }
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

pub(super) fn merge_union(parts: Vec<Built>, ctx: &Ctx) -> Built {
    let mut out = Built {
        kind: "集合の和".into(),
        ..Default::default()
    };
    let mut surfaces = Vec::new();
    let mut curves = Vec::new();
    for mut part in parts {
        let mut line = part.curve;
        let inner = if let Some(shape) = &part.shape {
            let projected = mesh::project(shape, &ctx.view, ctx.dom);
            let result = if ctx.flat {
                crate::flat_finish(projected, line, ctx.flat_slice)
            } else {
                projected
            };
            line |= ctx.flat_slice;
            result
        } else {
            part.inner
        };
        if !inner.positions.is_empty() {
            if line {
                curves.push(inner);
            } else {
                surfaces.push(inner);
            }
        }
        out.outline.0.append(&mut part.outline.0);
        out.outline.1.append(&mut part.outline.1);
        out.holes.append(&mut part.holes);
        out.source_holes.append(&mut part.source_holes);
        out.source_dashed.append(&mut part.source_dashed);
        out.fields.append(&mut part.fields);
    }
    out.curve = surfaces.is_empty();
    if out.curve {
        out.inner = mesh::merge(curves, true);
    } else {
        out.inner = mesh::merge(surfaces, false);
        out.outline.0 = mesh::merge(
            std::iter::once(Mesh {
                positions: out.outline.0,
                ..Default::default()
            })
            .chain(curves)
            .collect(),
            true,
        )
        .positions;
    }
    out
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

/// 平面領域の切り出しで複製された頂点をまとめる。像の法線はこの頂点を共有して計算する。
fn weld_planar_vertices(mesh: &mut Mesh) {
    let mut by_position = crate::adaptive::HashMap::default();
    let mut positions = Vec::new();
    let remap: Vec<u32> = mesh
        .positions
        .chunks_exact(3)
        .map(|p| {
            let key = [0, 1, 2].map(|i| if p[i] == 0.0 { 0 } else { p[i].to_bits() });
            *by_position.entry(key).or_insert_with(|| {
                let i = positions.len() as u32 / 3;
                positions.extend_from_slice(p);
                i
            })
        })
        .collect();
    for index in &mut mesh.indices {
        *index = remap[*index as usize];
    }
    mesh.positions = positions;
}

// 区間演算が飛びや極を示すとき、離れた像の点の間に辺を張らない。
fn join_vertices(indices: &[usize], positions: &[f32], values: &[f64], arity: usize, guards: &[(usize, &Program)]) -> bool {
    if guards.is_empty() {
        return true;
    }
    let point = |i: usize| [0, 1, 2].map(|a| positions[3 * i + a] as f64);
    let points = indices.iter().map(|&i| point(i));
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for p in points {
        if p.iter().any(|v| !v.is_finite()) {
            return false;
        }
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    let bounds = [0, 1, 2].map(|a| Interval::new(lo[a], hi[a]));
    guards.iter().all(|&(component, program)| {
        let first = values[arity * indices[0] + component];
        if indices.iter().all(|&i| values[arity * i + component] == first) {
            return true;
        }
        let range = program.eval_interval_dec(&bounds);
        match range.dec {
            Dec::Def => false,
            Dec::Trv if !range.bounded() => !indices.iter().any(|&i| {
                let sample = point(i).map(|x| Interval::new(x, x));
                program.eval_interval_dec(&sample).dec >= Dec::Dac
            }),
            _ => true,
        }
    })
}

fn mapped_curve(mapped: Vec<f32>, source: &[f32], values: &[f64], arity: usize, guards: &[(usize, &Program)]) -> Vec<f32> {
    if guards.is_empty() {
        return mapped;
    }
    let mut out = Vec::with_capacity(mapped.len());
    let mut connected = false;
    for (i, p) in mapped.chunks_exact(3).enumerate() {
        if connected && p[0].is_finite() && !join_vertices(&[i - 1, i], source, values, arity, guards) {
            out.extend([f32::NAN; 3]);
        }
        out.extend_from_slice(p);
        connected = p[0].is_finite();
    }
    out
}

fn scalar_intervals(mesh: &Mesh, values: &[f64], guards: &[(usize, &Program)], out: &mut Vec<(f64, f64)>) {
    // 連結でない入力の像を、ひとつの最小値・最大値でつながない。
    let mut add = |values: &[f64]| {
        let finite = values.iter().copied().filter(|v| v.is_finite());
        let lo = finite.clone().fold(f64::INFINITY, f64::min);
        let hi = finite.fold(f64::NEG_INFINITY, f64::max);
        if lo <= hi {
            out.push((lo, hi));
        }
    };
    if mesh.indices.is_empty() {
        let mut previous = f64::NAN;
        for (i, &v) in values.iter().enumerate() {
            if v.is_finite() {
                if previous.is_finite() && join_vertices(&[i - 1, i], &mesh.positions, values, 1, guards) {
                    add(&[previous, v]);
                } else {
                    add(&[v]);
                }
            }
            previous = v;
        }
    } else {
        for tri in mesh.indices.chunks_exact(3) {
            let indices = [tri[0] as usize, tri[1] as usize, tri[2] as usize];
            let vals = indices.map(|i| values[i]);
            if join_vertices(&indices, &mesh.positions, values, 1, guards) {
                add(&vals);
            } else {
                for value in vals {
                    add(&[value]);
                }
            }
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

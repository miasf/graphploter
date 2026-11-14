//! いくつかの式が同時に 0 になる所（交わる線・点）を描く。
//!
//! 2 つの式 $F_1 = F_2 = 0$ の線は、$F_1 = 0$ の面のメッシュの上で $F_2$ の等高線（値 0）を取り出し、
//! ニュートン法で 2 つの面の上へ寄せて作る（[`intersection_curve`]）。寄らない点（`tan` の極の偽の線など）は捨てる。
//! 3 つ以上の式の点は、はじめの 2 つの線の上で 3 つ目の式の符号が変わる所から、全部の式の上へ寄せて作る（[`points_on_curve`]）。
//!
//! 今は、$A^2 + B^2 = 0$ のように式の形から「面にならず、線・点だけになる」と分かる陰関数に使う
//! （[`crate::eval::Program::common_zero_sets`]）。複素数の陰関数 $\operatorname{Re} F = \operatorname{Im} F = 0$・連立方程式の土台にもなる。

use crate::eval::{as_real, Program};
use crate::mesh::{self, Domain, Mesh, Refine};
use num_complex::Complex64 as C;

/// ニュートン法のくり返しの回数
const NEWTON_STEPS: usize = 6;

/// メッシュの上の等高線（値 0）: 三角形の辺のうち、両端の `values` の符号が違うものに、直線補間した点を 1 つ置く
/// （辺ごとに 1 つ。隣の三角形と共有する）。三角形の中で点がちょうど 2 つなら、線分で結ぶ。
/// 値がちょうど 0 の頂点は正の側とみなす。どちらかの端の値が有限でない辺には、点を置かない。
/// 戻り値は点と、線分（点の番号の組）
pub fn contour_on_mesh(positions: &[[f64; 3]], indices: &[u32], values: &[f64]) -> (Vec<[f64; 3]>, Vec<[usize; 2]>) {
    let mut at: crate::adaptive::HashMap<(u32, u32), usize> = Default::default();
    let mut points = Vec::new();
    let mut segs = Vec::new();
    for t in indices.chunks_exact(3) {
        let mut found = [0usize; 3];
        let mut count = 0;
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (fa, fb) = (values[a as usize], values[b as usize]);
            if !(fa.is_finite() && fb.is_finite()) || (fa < 0.0) == (fb < 0.0) {
                continue;
            }
            let key = (a.min(b), a.max(b));
            let id = *at.entry(key).or_insert_with(|| {
                let s = fa / (fa - fb);
                let (pa, pb) = (positions[a as usize], positions[b as usize]);
                points.push([0, 1, 2].map(|c| pa[c] + s * (pb[c] - pa[c])));
                points.len() - 1
            });
            found[count] = id;
            count += 1;
        }
        if count == 2 && found[0] != found[1] {
            segs.push([found[0], found[1]]);
        }
    }
    (points, segs)
}

/// 式 `prog`（変数は 3 つの座標）の、点ごとの値（実数でなければ NaN）
fn values_at(prog: &Program, cols: &[Vec<f64>; 3]) -> Vec<f64> {
    let mut out = vec![C::new(0.0, 0.0); cols[0].len()];
    prog.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
    out.into_iter().map(as_real).collect()
}

/// 小さな連立 1 次方程式 $M x = b$（$m \times m$。部分ピボットのガウスの消去法）。解けなければ None
fn solve(mut m: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| m[i][c].abs().total_cmp(&m[j][c].abs()))?;
        if m[p][c] == 0.0 || m[p][c].is_nan() {
            return None;
        }
        m.swap(c, p);
        b.swap(c, p);
        for r in c + 1..n {
            let k = m[r][c] / m[c][c];
            let (top, rest) = m.split_at_mut(r);
            for (x, y) in rest[0][c..].iter_mut().zip(&top[c][c..]) {
                *x -= k * y;
            }
            b[r] -= k * b[c];
        }
    }
    let mut x = vec![0.0; n];
    for c in (0..n).rev() {
        let s: f64 = (c + 1..n).map(|j| m[c][j] * x[j]).sum();
        x[c] = (b[c] - s) / m[c][c];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// 点を、式 `progs` がどれも 0 になる所へ寄せる（ニュートン法。式の数 m は 1 以上で、3 より多くてもよい）。
///
/// $J$ は $m \times 3$ のヤコビ行列（中心差分。幅は $10^{-5} h$）で、
/// $\mathbf p \leftarrow \mathbf p - J^\top (J J^\top + \lambda I)^{-1} \mathbf F(\mathbf p)$、$\lambda = 10^{-12} \operatorname{tr}(J J^\top)$。
/// 1 歩の長さは $h$ まで。[`NEWTON_STEPS`] 回くり返し、最後の 1 歩が $10^{-3} h$ 未満で、最初の位置から動いた距離が $1.5 h$ 未満で、
/// どの式の値も $10^{-4} h |\nabla F_k|$ 以下になった点を「寄った」とする（寄らなければ None）。
/// 途中で値が有限でなくなった点も None
pub fn newton_onto(progs: &[&Program], points: &[[f64; 3]], h: f64) -> Vec<Option<[f64; 3]>> {
    newton_onto_in(progs, points, h, 3)
}

/// [`newton_onto`] で、はじめの `dims` 個の座標だけを動かす版（2D 表示では横・奥の 2 つ。縦の座標は 0 のまま）
pub fn newton_onto_in(progs: &[&Program], points: &[[f64; 3]], h: f64, dims: usize) -> Vec<Option<[f64; 3]>> {
    let m = progs.len();
    let d = 1e-5 * h;
    let mut cur: Vec<[f64; 3]> = points.to_vec();
    let mut alive: Vec<bool> = vec![true; points.len()];
    let mut last_step = vec![f64::INFINITY; points.len()];
    let mut residual_ok = vec![false; points.len()];
    for step in 0..=NEWTON_STEPS {
        let active: Vec<usize> = (0..cur.len()).filter(|&k| alive[k]).collect();
        if active.is_empty() {
            break;
        }
        // 点ごとに 7 点（中心と、各軸の ±）
        let mut cols: [Vec<f64>; 3] = Default::default();
        for &k in &active {
            for s in 0..7 {
                for (c, col) in cols.iter_mut().enumerate() {
                    let off = if s > 0 && (s - 1) / 2 == c { if s % 2 == 1 { d } else { -d } } else { 0.0 };
                    col.push(cur[k][c] + off);
                }
            }
        }
        let vals: Vec<Vec<f64>> = progs.iter().map(|p| values_at(p, &cols)).collect();
        for (j, &k) in active.iter().enumerate() {
            let f: Vec<f64> = (0..m).map(|i| vals[i][7 * j]).collect();
            let jac: Vec<[f64; 3]> = (0..m)
                .map(|i| {
                    [0, 1, 2].map(|c| {
                        if c < dims {
                            (vals[i][7 * j + 1 + 2 * c] - vals[i][7 * j + 2 + 2 * c]) / (2.0 * d)
                        } else {
                            0.0
                        }
                    })
                })
                .collect();
            if f.iter().chain(jac.iter().flatten()).any(|v| !v.is_finite()) {
                alive[k] = false;
                continue;
            }
            residual_ok[k] = (0..m).all(|i| {
                let g = jac[i].iter().map(|v| v * v).sum::<f64>().sqrt();
                f[i].abs() <= 1e-4 * h * g
            });
            if step == NEWTON_STEPS {
                continue;
            }
            let dot = |a: &[f64; 3], b: &[f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            let jjt: Vec<Vec<f64>> = (0..m).map(|r| (0..m).map(|c| dot(&jac[r], &jac[c])).collect()).collect();
            let lambda = 1e-12 * (0..m).map(|r| jjt[r][r]).sum::<f64>();
            let mut a = jjt;
            for (r, row) in a.iter_mut().enumerate() {
                row[r] += lambda;
            }
            let Some(y) = solve(a, f) else {
                alive[k] = false;
                continue;
            };
            let mut delta = [0.0; 3];
            for (r, row) in jac.iter().enumerate() {
                for c in 0..3 {
                    delta[c] += row[c] * y[r];
                }
            }
            let len = dot(&delta, &delta).sqrt();
            let scale = if len > h { h / len } else { 1.0 };
            for c in 0..3 {
                cur[k][c] -= delta[c] * scale;
            }
            last_step[k] = len * scale;
        }
    }
    (0..cur.len())
        .map(|k| {
            let moved = (0..3).map(|c| (cur[k][c] - points[k][c]).powi(2)).sum::<f64>().sqrt();
            (alive[k] && residual_ok[k] && last_step[k] < 1e-3 * h && moved < 1.5 * h).then_some(cur[k])
        })
        .collect()
}

/// 格子の 1 マスの幅（いちばん狭い軸）
fn cell(dom: Domain, n: usize) -> f64 {
    (0..3).map(|a| (dom.hi[a] - dom.lo[a]) / n.max(2) as f64).fold(f64::INFINITY, f64::min)
}

/// $F_1 = F_2 = 0$ の線（寄せた後の点と、線分）。$F_1 = 0$ の面を作り、その上で $F_2$ の等高線を取り出して寄せる。
/// どちらかの端が寄らなかった線分は捨てる。F の計算を Worker に分ける依頼（計画・一部の計算）のときは、空のまま返す
fn curve_points(f1: &Program, f2: &Program, dom: Domain, n: usize, refine: Refine) -> (Vec<[f64; 3]>, Vec<[usize; 2]>) {
    if mesh::field_mode_is_split() {
        mesh::mark_unsplittable();
        return (Vec::new(), Vec::new());
    }
    let surf = mesh::implicit_solid_opt(f1, dom, n, &[], 0, refine);
    let pos: Vec<[f64; 3]> = surf.positions.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect();
    if pos.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let cols: [Vec<f64>; 3] = [0, 1, 2].map(|c| pos.iter().map(|p| p[c]).collect());
    let values = values_at(f2, &cols);
    let (points, segs) = contour_on_mesh(&pos, &surf.indices, &values);
    let h = cell(dom, n);
    // 線が自分と交わる点（X の字）: 三角形の上の等高線では、鞍点の X が向かい合う 2 本の弧に分かれるので、先に交点を求めておく
    let crossings = crossing_points(f1, f2, &points, &segs, h);
    let moved = newton_onto(&[f1, f2], &points, h);
    let mut keep = Vec::new();
    let mut at = vec![usize::MAX; points.len()];
    let mut out_segs = Vec::new();
    for [a, b] in segs {
        let (Some(pa), Some(pb)) = (moved[a], moved[b]) else { continue };
        for (i, p) in [(a, pa), (b, pb)] {
            if at[i] == usize::MAX {
                at[i] = keep.len();
                keep.push(p);
            }
        }
        out_segs.push([at[a], at[b]]);
    }
    for c in crossings {
        join_at_crossing(f1, f2, &mut keep, &mut out_segs, c, h);
    }
    (keep, out_segs)
}

/// 交点の周りで線を切る半径（格子の何マス分か）。等高線の弧が交点から離れて丸まるのは半マスほどなので、その外から結び直す
const CROSS_RADIUS: f64 = 1.5;

/// $F_1 = F_2 = 0$ の線が自分と交わる点（X の字の真ん中）。
///
/// 三角形の上の等高線（区分的に 1 次）は、$F_1 = 0$ の面の上での $F_2$ の鞍点を通る X を、必ず向かい合う 2 本の弧に分ける
/// （ヴィヴィアーニの曲線の $(2, 0, 0)$ では、球と円柱が接していて、球の上で $F_2 \approx (y^2 - z^2)/2$）。
/// 1. 等高線の点のうち、空間では $1.5h$ より近いのに、線分を 6 本たどっても行けない 2 点を探す（弧どうしが近づいている所）
/// 2. その真ん中から、面の上での $F_2$ の停留点（$F_1 = 0$ かつ $\nabla F_2$ の面に沿った成分が 0）をガウス・ニュートン法で求める
/// 3. そこで $|F_2| \le 10^{-4} h |\nabla F_2|$ なら交点（[`newton_onto`] の「寄った」と同じ基準）。0 でなければ、交わらずに近づいているだけ
///    （双曲線の 2 本の枝・小さな輪の真ん中）なので使わない
fn crossing_points(f1: &Program, f2: &Program, points: &[[f64; 3]], segs: &[[usize; 2]], h: f64) -> Vec<[f64; 3]> {
    const NEAR: f64 = 1.5;
    const HOPS: usize = 6;
    if points.len() < 4 {
        return Vec::new();
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); points.len()];
    for &[a, b] in segs {
        adj[a].push(b);
        adj[b].push(a);
    }
    let r = NEAR * h;
    let key = |p: &[f64; 3]| p.map(|v| (v / r).floor() as i64);
    let mut grid: crate::adaptive::HashMap<[i64; 3], Vec<usize>> = Default::default();
    for (i, p) in points.iter().enumerate() {
        grid.entry(key(p)).or_default().push(i);
    }
    let dist = |a: &[f64; 3], b: &[f64; 3]| (0..3).map(|c| (a[c] - b[c]).powi(2)).sum::<f64>().sqrt();
    let mut seeds: Vec<[f64; 3]> = Vec::new();
    let mut seen = vec![usize::MAX; points.len()];
    for i in 0..points.len() {
        if adj[i].is_empty() {
            continue;
        }
        // i から線分を HOPS 本までたどって行ける点
        let mut frontier = vec![i];
        seen[i] = i;
        for _ in 0..HOPS {
            let mut next = Vec::new();
            for &u in &frontier {
                for &v in &adj[u] {
                    if seen[v] != i {
                        seen[v] = i;
                        next.push(v);
                    }
                }
            }
            frontier = next;
        }
        let k = key(&points[i]);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(list) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else { continue };
                    for &j in list {
                        if j <= i || seen[j] == i || adj[j].is_empty() || dist(&points[i], &points[j]) >= r {
                            continue;
                        }
                        let m = [0, 1, 2].map(|c| 0.5 * (points[i][c] + points[j][c]));
                        if seeds.iter().all(|s| dist(s, &m) > h) {
                            seeds.push(m);
                        }
                    }
                }
            }
        }
    }
    let mut out: Vec<[f64; 3]> = Vec::new();
    for s in seeds {
        if let Some(p) = saddle_on_surface(f1, f2, s, h) {
            if out.iter().all(|q| dist(q, &p) > h) {
                out.push(p);
            }
        }
    }
    out
}

/// 点 `p` での値と勾配（中心差分。幅は $10^{-5} h$）。値か勾配が有限でなければ None
fn value_grad(prog: &Program, p: [f64; 3], h: f64) -> Option<(f64, [f64; 3])> {
    let d = 1e-5 * h;
    let mut cols: [Vec<f64>; 3] = Default::default();
    for s in 0..7 {
        for (c, col) in cols.iter_mut().enumerate() {
            let off = if s > 0 && (s - 1) / 2 == c { if s % 2 == 1 { d } else { -d } } else { 0.0 };
            col.push(p[c] + off);
        }
    }
    let v = values_at(prog, &cols);
    let g = [0, 1, 2].map(|c| (v[1 + 2 * c] - v[2 + 2 * c]) / (2.0 * d));
    (v.iter().all(|x| x.is_finite())).then_some((v[0], g))
}

/// `seed` の近くで、$F_1 = 0$ の面の上での $F_2$ の停留点を求め、そこで $F_2 \approx 0$ なら返す（[`crossing_points`] の 2・3）。
/// 残差は $(F_1 / |\nabla F_1|,\ h\,(\nabla F_2 - (\nabla F_2 \cdot \mathbf n)\mathbf n) / |\nabla F_2|)$（$\mathbf n$ は $F_1$ の面の単位法線。長さの単位にそろえる）。
/// ヤコビ行列は残差の中心差分（幅 $10^{-3} h$）。1 歩は $0.5 h$ まで、`seed` から $1.5 h$ より離れたらやめる
fn saddle_on_surface(f1: &Program, f2: &Program, seed: [f64; 3], h: f64) -> Option<[f64; 3]> {
    let (_, g10) = value_grad(f1, seed, h)?;
    let (_, g20) = value_grad(f2, seed, h)?;
    let norm = |v: [f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let (s1, s2) = (norm(g10), norm(g20));
    if !(s1 > 0.0 && s2 > 0.0) {
        return None;
    }
    let residual = |p: [f64; 3]| -> Option<[f64; 4]> {
        let (v1, g1) = value_grad(f1, p, h)?;
        let (_, g2) = value_grad(f2, p, h)?;
        let l = norm(g1);
        if l == 0.0 {
            return None;
        }
        let n = g1.map(|v| v / l);
        let dn = g2[0] * n[0] + g2[1] * n[1] + g2[2] * n[2];
        let t = [0, 1, 2].map(|c| h * (g2[c] - dn * n[c]) / s2);
        Some([v1 / s1, t[0], t[1], t[2]])
    };
    let e = 1e-3 * h;
    let mut p = seed;
    for _ in 0..20 {
        let r = residual(p)?;
        let mut jac = [[0.0; 3]; 4];
        for c in 0..3 {
            let (mut a, mut b) = (p, p);
            a[c] += e;
            b[c] -= e;
            let (ra, rb) = (residual(a)?, residual(b)?);
            for k in 0..4 {
                jac[k][c] = (ra[k] - rb[k]) / (2.0 * e);
            }
        }
        // 正規方程式 JᵀJ δ = −Jᵀ r（少しだけ減衰を足す）
        let mut a = vec![vec![0.0; 3]; 3];
        let mut b = vec![0.0; 3];
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] = (0..4).map(|k| jac[k][i] * jac[k][j]).sum();
            }
            b[i] = -(0..4).map(|k| jac[k][i] * r[k]).sum::<f64>();
        }
        let tr = a[0][0] + a[1][1] + a[2][2];
        for (i, row) in a.iter_mut().enumerate() {
            row[i] += 1e-10 * tr;
        }
        let delta = solve(a, b)?;
        let len = norm([delta[0], delta[1], delta[2]]);
        let scale = if len > 0.5 * h { 0.5 * h / len } else { 1.0 };
        for c in 0..3 {
            p[c] += delta[c] * scale;
        }
        if norm([0, 1, 2].map(|c| p[c] - seed[c])) > 1.5 * h {
            return None;
        }
        if len < 1e-7 * h {
            break;
        }
    }
    let r = residual(p)?;
    let (v2, g2) = value_grad(f2, p, h)?;
    let ok = r[0].abs() <= 1e-6 * h && norm([r[1], r[2], r[3]]) <= 1e-4 * h && v2.abs() <= 1e-4 * h * norm(g2);
    ok.then_some(p)
}

/// 交点 `c` の周り（半径 [`CROSS_RADIUS`] マス）の線分を捨て、切れた線の端を `c` へつなぎ直す（X の字にする）。
/// つなぐ線は $0.25 h$ ごとに区切り、間の点は [`newton_onto`] で 2 つの面の上へ寄せる（寄らなければ直線のまま）
fn join_at_crossing(f1: &Program, f2: &Program, points: &mut Vec<[f64; 3]>, segs: &mut Vec<[usize; 2]>, c: [f64; 3], h: f64) {
    let rho = CROSS_RADIUS * h;
    let dist = |p: &[f64; 3]| (0..3).map(|k| (p[k] - c[k]).powi(2)).sum::<f64>().sqrt();
    let inside: Vec<bool> = points.iter().map(|p| dist(p) < rho).collect();
    let mut ends: Vec<usize> = Vec::new();
    let mut kept = Vec::with_capacity(segs.len());
    for &[a, b] in segs.iter() {
        match (inside[a], inside[b]) {
            (false, false) => kept.push([a, b]),
            (true, false) => ends.push(b),
            (false, true) => ends.push(a),
            (true, true) => {}
        }
    }
    ends.sort_unstable();
    ends.dedup();
    // 端が 2 つより少なければ、交点ではなかった（線の端・途切れ）。何もしない
    if ends.len() < 2 {
        return;
    }
    *segs = kept;
    let center = points.len();
    points.push(c);
    for e in ends {
        let p = points[e];
        let pieces = (dist(&p) / (0.25 * h)).ceil().max(1.0) as usize;
        let mids: Vec<[f64; 3]> = (1..pieces).map(|k| {
            let t = k as f64 / pieces as f64;
            [0, 1, 2].map(|i| p[i] + t * (c[i] - p[i]))
        }).collect();
        let moved = newton_onto(&[f1, f2], &mids, h);
        let mut prev = e;
        for (m, mv) in mids.iter().zip(moved) {
            points.push(mv.unwrap_or(*m));
            let id = points.len() - 1;
            segs.push([prev, id]);
            prev = id;
        }
        segs.push([prev, center]);
    }
}

/// $F_1 = F_2 = 0$ の交わる線（曲線のメッシュ。線分ごとに 2 点と NaN）
pub fn intersection_curve(f1: &Program, f2: &Program, dom: Domain, n: usize, refine: Refine) -> Mesh {
    let (points, segs) = curve_points(f1, f2, dom, n, refine);
    let scale = (0..3).map(|a| (dom.hi[a] - dom.lo[a]) as f32).sum::<f32>();
    let mut out = Vec::with_capacity(segs.len() * 9);
    for [a, b] in segs {
        let (pa, pb) = (points[a].map(|v| v as f32), points[b].map(|v| v as f32));
        // 長さの無い線分は描かない（描画側が点とみなして丸を付ける）
        if mesh::too_short(pa, pb, scale) {
            continue;
        }
        out.extend_from_slice(&pa);
        out.extend_from_slice(&pb);
        out.extend_from_slice(&[f32::NAN; 3]);
    }
    Mesh { positions: out, ..Default::default() }
}

/// 点を、互いに `tol` より近いものを 1 つにまとめ、表示範囲（はじめの `dims` 個の座標）の外を捨てる
fn distinct_inside(points: impl IntoIterator<Item = [f64; 3]>, dom: Domain, dims: usize, tol: f64) -> Vec<[f64; 3]> {
    let mut found: Vec<[f64; 3]> = Vec::new();
    for p in points {
        let inside = (0..dims).all(|c| p[c] >= dom.lo[c] - 1e-9 && p[c] <= dom.hi[c] + 1e-9);
        let near = |q: &[f64; 3]| (0..3).map(|c| (q[c] - p[c]).powi(2)).sum::<f64>() < tol * tol;
        if inside && !found.iter().any(near) {
            found.push(p);
        }
    }
    found
}

/// 線分の両端の値 `g` の符号が違う所に、直線補間で点を置く
fn sign_changes(points: &[[f64; 3]], segs: &[[usize; 2]], g: &[f64]) -> Vec<[f64; 3]> {
    let mut seeds = Vec::new();
    for &[a, b] in segs {
        let (ga, gb) = (g[a], g[b]);
        if ga.is_finite() && gb.is_finite() && (ga < 0.0) != (gb < 0.0) {
            let s = ga / (ga - gb);
            seeds.push([0, 1, 2].map(|c| points[a][c] + s * (points[b][c] - points[a][c])));
        }
    }
    seeds
}

/// 式 `progs`（3 つ以上）がどれも 0 になる点。はじめの 2 つの式の線の上で、3 つ目の式の符号が変わる所を探し、全部の式の上へ寄せる。
/// 互いに 0.25h より近い点は 1 つにまとめ、表示範囲の外の点は捨てる
pub fn points_on_curve(progs: &[Program], dom: Domain, n: usize, refine: Refine) -> Vec<[f64; 3]> {
    assert!(progs.len() >= 3);
    let (points, segs) = curve_points(&progs[0], &progs[1], dom, n, refine);
    let h = cell(dom, n);
    if points.is_empty() {
        return Vec::new();
    }
    let cols: [Vec<f64>; 3] = [0, 1, 2].map(|c| points.iter().map(|p| p[c]).collect());
    let seeds = sign_changes(&points, &segs, &values_at(&progs[2], &cols));
    let refs: Vec<&Program> = progs.iter().collect();
    distinct_inside(newton_onto(&refs, &seeds, h).into_iter().flatten(), dom, 3, 0.25 * h)
}

/// 点を、描画側が点とみなす形（同じ点を 2 つと NaN）の曲線のメッシュにする
pub fn points_mesh(points: &[[f64; 3]]) -> Mesh {
    let mut out = Vec::with_capacity(points.len() * 9);
    for p in points {
        let p = p.map(|v| v as f32);
        out.extend_from_slice(&p);
        out.extend_from_slice(&p);
        out.extend_from_slice(&[f32::NAN; 3]);
    }
    Mesh { positions: out, ..Default::default() }
}

/// 点のうち、不等式 `cuts`（G ≤ 0）を全部満たすもの
pub fn points_inside(points: Vec<[f64; 3]>, cuts: &[Program]) -> Vec<[f64; 3]> {
    let mut keep = vec![true; points.len()];
    for g in cuts {
        for (k, v) in values_of(g)(&points).into_iter().enumerate() {
            keep[k] &= v <= 0.0;
        }
    }
    points.into_iter().zip(keep).filter_map(|(p, k)| k.then_some(p)).collect()
}

/// 2D 表示の、2 本の式 F_1 = F_2 = 0 の交点（縦の座標は 0）。`implicit_curve_2d(F_1)` の線分の上で F_2 の符号が変わる所を探し、
/// 横・奥の座標だけを動かすニュートン法で寄せる
pub fn flat_points(f1: &Program, f2: &Program, dom: Domain, n: usize) -> Vec<[f64; 3]> {
    let curve = mesh::implicit_curve_2d(f1, dom, n);
    let mut points = Vec::new();
    let mut segs = Vec::new();
    for s in curve.positions.chunks_exact(9) {
        if s[..6].iter().all(|v| v.is_finite()) {
            points.push([s[0] as f64, s[1] as f64, 0.0]);
            points.push([s[3] as f64, s[4] as f64, 0.0]);
            segs.push([points.len() - 2, points.len() - 1]);
        }
    }
    let h = (0..2).map(|a| (dom.hi[a] - dom.lo[a]) / n.max(2) as f64).fold(f64::INFINITY, f64::min);
    let seeds = sign_changes(&points, &segs, &values_of(f2)(&points));
    distinct_inside(newton_onto_in(&[f1, f2], &seeds, h, 2).into_iter().flatten(), dom, 2, 0.25 * h)
}

/// 因数ごとの「同時に 0 になる式の組」（[`Program::common_zero_sets`]）を描く。式が 2 つの組は線、3 つ以上の組は点。
/// 戻り値はメッシュ（曲線）と、説明（線か点か）
pub fn common_zero_sets(sets: &[Vec<Program>], dom: Domain, n: usize, refine: Refine) -> (Mesh, String) {
    let meshes: Vec<Mesh> = sets
        .iter()
        .map(|s| {
            if s.len() == 2 {
                intersection_curve(&s[0], &s[1], dom, n, refine)
            } else {
                points_mesh(&points_on_curve(s, dom, n, refine))
            }
        })
        .collect();
    let (lines, points) = (sets.iter().any(|s| s.len() == 2), sets.iter().any(|s| s.len() > 2));
    let what = match (lines, points) {
        (true, true) => "線・点",
        (true, false) => "線",
        _ => "点",
    };
    (mesh::merge(meshes, true), format!("陰関数（{what}だけの解）"))
}

/// 複素数の陰関数として描く式か: 虚部が 0 でない定数（i・複素数のパラメータ）を含み、表示範囲の中の点（6³ 個）で、
/// 値が有限な点のうち 2 割以上で実数にならない（|Im F| > 10⁻⁹ (1 + |Re F|)）。
/// `|x + i y| = z^2 + 1` のように i を含んでも値が実数の式、`sqrt(x)` のように定義域の外でだけ複素数になる式は、ふつうの陰関数のまま
pub fn is_complex_valued(prog: &Program, dom: Domain) -> bool {
    if !prog.has_complex_const() || prog.vars().len() != 3 {
        return false;
    }
    const K: usize = 6;
    let mut cols: [Vec<f64>; 3] = Default::default();
    for k in 0..K * K * K {
        let idx = [k % K, k / K % K, k / (K * K)];
        for c in 0..3 {
            cols[c].push(dom.lo[c] + (dom.hi[c] - dom.lo[c]) * (idx[c] as f64 + 0.5) / K as f64);
        }
    }
    let mut out = vec![C::new(0.0, 0.0); K * K * K];
    prog.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
    let finite: Vec<&C> = out.iter().filter(|v| v.re.is_finite() && v.im.is_finite()).collect();
    let complex = finite.iter().filter(|v| v.im.abs() > 1e-9 * (1.0 + v.re.abs())).count();
    !finite.is_empty() && complex * 5 >= finite.len()
}

/// 式 `prog`（変数は 3 つの座標）を、点の列での値の関数にする（[`trim_mesh`]・[`clip_segments`] の条件）
pub fn values_of(prog: &Program) -> impl FnMut(&[[f64; 3]]) -> Vec<f64> + '_ {
    move |pts: &[[f64; 3]]| {
        let cols: [Vec<f64>; 3] = [0, 1, 2].map(|c| pts.iter().map(|p| p[c]).collect());
        values_at(prog, &cols)
    }
}

/// 辺 a → b（a が中、b が外）の上の境目の比 s（a から b へ）を、まとめて求める。
/// 両端の値がどちらも有限なら、はさみうち法（イリノイ法。残った側の値を半分にして、片側だけが動き続けるのを防ぐ）、
/// 外の端が NaN なら二分。全部の辺を 1 回ずつまとめて計算するのを `rounds` 回くり返し、中の側の端を返す
fn edge_cuts(
    ends: &[([f64; 3], [f64; 3], f64, f64)],
    value: &mut dyn FnMut(&[[f64; 3]]) -> Vec<f64>,
    rounds: usize,
) -> Vec<f64> {
    let mut lo = vec![0.0; ends.len()];
    let mut hi = vec![1.0; ends.len()];
    let mut glo: Vec<f64> = ends.iter().map(|e| e.2).collect();
    let mut ghi: Vec<f64> = ends.iter().map(|e| e.3).collect();
    // 前の回で動いた側（-1: 中の側, 1: 外の側, 0: まだ）
    let mut side = vec![0i8; ends.len()];
    for _ in 0..rounds {
        let ts: Vec<f64> = (0..ends.len())
            .map(|k| {
                let (a, b) = (glo[k], ghi[k]);
                if a.is_finite() && b.is_finite() && b > a {
                    let t = lo[k] + (hi[k] - lo[k]) * a / (a - b);
                    // 端にくっつきすぎないよう、区間の内側に収める
                    let w = 1e-3 * (hi[k] - lo[k]);
                    t.clamp(lo[k] + w, hi[k] - w)
                } else {
                    0.5 * (lo[k] + hi[k])
                }
            })
            .collect();
        let pts: Vec<[f64; 3]> =
            ends.iter().zip(&ts).map(|(&(a, b, _, _), &t)| [0, 1, 2].map(|c| a[c] + t * (b[c] - a[c]))).collect();
        let g = value(&pts);
        for k in 0..ends.len() {
            if g[k] <= 0.0 {
                lo[k] = ts[k];
                glo[k] = g[k];
                if side[k] == -1 {
                    ghi[k] *= 0.5;
                }
                side[k] = -1;
            } else {
                hi[k] = ts[k];
                ghi[k] = g[k];
                if side[k] == 1 {
                    glo[k] *= 0.5;
                }
                side[k] = 1;
            }
        }
    }
    lo
}

/// メッシュのうち、`value`（点の列での G。NaN は外）が 0 以下の所だけを残す。
/// 中と外が混ざる三角形は、中の側の多角形（三角形か四角形）にする。境目の点は辺ごとに 1 つ（隣の三角形と共有する）で、
/// 位置は辺の上の二分（`rounds` 回）で求める。新しい頂点の法線・`grid_coords`・`complex` は、辺の両端から同じ比で補間する。
/// 番号だけの粗い版（`lod`）は空にし（後で作り直す）、陰関数の粗い版（`lod_mesh`）は同じ条件で切る
pub fn trim_mesh(mesh: &mut Mesh, value: &mut dyn FnMut(&[[f64; 3]]) -> Vec<f64>, rounds: usize) {
    if let Some(l) = mesh.lod_mesh.as_deref_mut() {
        trim_mesh(l, value, rounds);
    }
    let count = mesh.positions.len() / 3;
    if count == 0 || mesh.indices.is_empty() {
        return;
    }
    let pos: Vec<[f64; 3]> = mesh.positions.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect();
    let g = value(&pos);
    let inside: Vec<bool> = g.iter().map(|v| *v <= 0.0).collect();
    if inside.iter().all(|&b| b) {
        return;
    }
    // 中と外を結ぶ辺（中の頂点, 外の頂点）
    let mut edge_id: crate::adaptive::HashMap<(u32, u32), usize> = Default::default();
    let mut edges: Vec<(u32, u32)> = Vec::new();
    for t in mesh.indices.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            if inside[a as usize] != inside[b as usize] {
                let key = if inside[a as usize] { (a, b) } else { (b, a) };
                edge_id.entry(key).or_insert_with(|| {
                    edges.push(key);
                    edges.len() - 1
                });
            }
        }
    }
    let ends: Vec<_> = edges.iter().map(|&(a, b)| (pos[a as usize], pos[b as usize], g[a as usize], g[b as usize])).collect();
    let ts = edge_cuts(&ends, value, rounds);
    // 残す頂点に新しい番号を付け、境目の点を足す
    let mut out = Mesh { keep_full: mesh.keep_full, note: mesh.note.clone(), stats: mesh.stats, ..Default::default() };
    let has_n = mesh.normals.len() == mesh.positions.len();
    let has_g = mesh.grid_coords.len() == mesh.positions.len();
    let has_c = mesh.complex.len() == mesh.positions.len();
    let mut map = vec![u32::MAX; count];
    let push_mix = |out: &mut Mesh, a: usize, b: usize, t: f64| -> u32 {
        let mix = |v: &[f32]| -> [f32; 3] { [0, 1, 2].map(|c| v[3 * a + c] + (t as f32) * (v[3 * b + c] - v[3 * a + c])) };
        out.positions.extend_from_slice(&mix(&mesh.positions));
        if has_n {
            let n = mix(&mesh.normals);
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            out.normals.extend(n.iter().map(|v| if len > 0.0 { v / len } else { *v }));
        }
        if has_g {
            out.grid_coords.extend_from_slice(&mix(&mesh.grid_coords));
        }
        if has_c {
            out.complex.extend_from_slice(&mix(&mesh.complex));
        }
        (out.positions.len() / 3 - 1) as u32
    };
    let keep = |out: &mut Mesh, map: &mut Vec<u32>, v: usize| -> u32 {
        if map[v] == u32::MAX {
            map[v] = push_mix(out, v, v, 0.0);
        }
        map[v]
    };
    let mut cut_at = vec![u32::MAX; edges.len()];
    for t in mesh.indices.chunks_exact(3) {
        let n_in = t.iter().filter(|&&v| inside[v as usize]).count();
        if n_in == 0 {
            continue;
        }
        let mut poly: Vec<u32> = Vec::with_capacity(4);
        for k in 0..3 {
            let (a, b) = (t[k] as usize, t[(k + 1) % 3] as usize);
            if inside[a] {
                poly.push(keep(&mut out, &mut map, a));
            }
            if inside[a] != inside[b] {
                let key = if inside[a] { (a as u32, b as u32) } else { (b as u32, a as u32) };
                let e = edge_id[&key];
                if cut_at[e] == u32::MAX {
                    cut_at[e] = push_mix(&mut out, key.0 as usize, key.1 as usize, ts[e]);
                }
                poly.push(cut_at[e]);
            }
        }
        for k in 1..poly.len() - 1 {
            out.indices.extend_from_slice(&[poly[0], poly[k], poly[k + 1]]);
        }
    }
    out.lod_mesh = mesh.lod_mesh.take();
    *mesh = out;
}

/// 曲線のメッシュ（点の列。NaN で切れ目。交わる線のような「線分ごとに 2 点と NaN」も、長さ 2 の列として同じに扱う）を、
/// `value` が 0 以下の所だけにする。中と外にまたがる線分は、境目を辺の上で求めて（`rounds` 回）、中の側だけ残す。
/// 長さの無い列（点）は、中ならそのまま残す。切った後に、ほとんど長さの無い列が残ったら捨てる（描画側が点とみなして丸を付けるため）
pub fn clip_segments(positions: &mut Vec<f32>, value: &mut dyn FnMut(&[[f64; 3]]) -> Vec<f64>, rounds: usize) {
    let pts: Vec<Option<[f64; 3]>> = positions
        .chunks_exact(3)
        .map(|q| q.iter().all(|v| v.is_finite()).then(|| [q[0] as f64, q[1] as f64, q[2] as f64]))
        .collect();
    let finite: Vec<[f64; 3]> = pts.iter().flatten().copied().collect();
    let gf = value(&finite);
    let mut g = vec![f64::NAN; pts.len()];
    let mut k = 0;
    for (i, p) in pts.iter().enumerate() {
        if p.is_some() {
            g[i] = gf[k];
            k += 1;
        }
    }
    let inside = |i: usize| g[i] <= 0.0;
    // 中と外にまたがる線分 (i, i + 1) の境目
    let mut mixed = Vec::new();
    let mut ends = Vec::new();
    for i in 0..pts.len().saturating_sub(1) {
        if let (Some(a), Some(b)) = (pts[i], pts[i + 1]) {
            if inside(i) != inside(i + 1) {
                mixed.push(i);
                ends.push(if inside(i) { (a, b, g[i], g[i + 1]) } else { (b, a, g[i + 1], g[i]) });
            }
        }
    }
    let ts = edge_cuts(&ends, value, rounds);
    let mut cut_at: std::collections::HashMap<usize, [f64; 3]> = Default::default();
    for (k, &i) in mixed.iter().enumerate() {
        let (a, b, _, _) = ends[k];
        cut_at.insert(i, [0, 1, 2].map(|c| a[c] + ts[k] * (b[c] - a[c])));
    }
    // 列ごとに組み立てる（列と、境目で切った列か）
    let mut runs: Vec<(Vec<[f64; 3]>, bool)> = Vec::new();
    let mut cur: (Vec<[f64; 3]>, bool) = (Vec::new(), false);
    for (i, p) in pts.iter().enumerate() {
        let Some(p) = *p else {
            runs.push(std::mem::take(&mut cur));
            continue;
        };
        if inside(i) {
            cur.0.push(p);
        }
        if let Some(q) = cut_at.get(&i) {
            if inside(i) {
                // 出ていく: 境目で列を終える
                cur.0.push(*q);
                cur.1 = true;
                runs.push(std::mem::take(&mut cur));
            } else {
                // 入ってくる: 境目から新しい列を始める
                runs.push(std::mem::take(&mut cur));
                cur = (vec![*q], true);
            }
        } else if !inside(i) {
            runs.push(std::mem::take(&mut cur));
        }
    }
    runs.push(cur);
    let mut out: Vec<f32> = Vec::with_capacity(positions.len());
    for (run, cut) in runs.into_iter().filter(|r| !r.0.is_empty()) {
        // 切って長さがほとんど無くなった列は捨てる（元から点の列は残す）
        let span = run.iter().map(|q| (0..3).map(|c| (q[c] - run[0][c]).abs()).sum::<f64>()).fold(0.0, f64::max);
        let scale = 1.0 + run[0].iter().map(|v| v.abs()).sum::<f64>();
        if cut && span <= 1e-6 * scale {
            continue;
        }
        for q in &run {
            out.extend(q.iter().map(|v| *v as f32));
        }
        out.extend_from_slice(&[f32::NAN; 3]);
    }
    *positions = out;
}

/// 不等式の領域 G_1 ≤ 0, …, G_k ≤ 0 の境目の面。面 G_i = 0 ごとに今の陰関数の道で作り、ほかの全部の G_j ≤ 0 で切り取ってまとめる
/// （角が立つ）。陰関数の粗い版も同じように切ってまとめる（どれかの面に粗い版が無ければ、全体の粗い版を作らない）
pub fn region_faces(cuts: &[Program], dom: Domain, n: usize, refine: Refine) -> Mesh {
    if mesh::field_mode_is_split() {
        mesh::mark_unsplittable();
        return Mesh::default();
    }
    let mut parts = Vec::new();
    let mut lods = Vec::new();
    for (i, g) in cuts.iter().enumerate() {
        let mut m = mesh::implicit_solid_opt(g, dom, n, &[], 0, refine);
        for (j, other) in cuts.iter().enumerate() {
            if j != i {
                trim_mesh(&mut m, &mut values_of(other), 8);
            }
        }
        lods.push(m.lod_mesh.take());
        parts.push(m);
    }
    if cuts.len() == 1 {
        let mut m = parts.pop().unwrap_or_default();
        m.lod_mesh = lods.pop().flatten();
        return m;
    }
    let mut out = mesh::merge(parts, false);
    if lods.iter().all(|l| l.is_some()) {
        out.lod_mesh = Some(Box::new(mesh::merge(lods.into_iter().flatten().map(|b| *b).collect(), false)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn prog(src: &str) -> Program {
        Program::compile(&parse(src).unwrap(), &["x", "y", "z"]).unwrap()
    }

    const DOM: Domain = Domain { lo: [-3.0, -3.0, -3.0], hi: [3.0, 3.0, 3.0] };

    /// 曲線のメッシュの点（NaN で区切った線分の端）
    fn ends(m: &Mesh) -> Vec<[f64; 3]> {
        m.positions.chunks_exact(3).filter(|p| p[0].is_finite()).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect()
    }

    /// 線分の長さの合計
    fn length(m: &Mesh) -> f64 {
        m.positions
            .chunks_exact(9)
            .map(|s| (0..3).map(|c| (s[c] as f64 - s[3 + c] as f64).powi(2)).sum::<f64>().sqrt())
            .sum()
    }

    #[test]
    fn sum_of_squares_is_split_into_parts() {
        let parts = |s: &str| prog(s).common_zero_sets().map(|v| v.iter().map(|p| p.len()).collect::<Vec<_>>());
        assert_eq!(parts("x^2 + y^2"), Some(vec![2]));
        assert_eq!(parts("x^2 - (-y^2)"), Some(vec![2]));
        assert_eq!(parts("-(x - 1)^2 - 3 abs(y z - 1)"), Some(vec![2]));
        assert_eq!(parts("x^2 + y^2 + z^4"), Some(vec![3]));
        assert_eq!(parts("(x^2 + y^2) ((x - 1)^2 + z^2 + y^2)"), Some(vec![2, 3]));
        assert_eq!(parts("sqrt((x^2 + y^2 - 4)^2 + z^2)"), Some(vec![2]));
        // 面になるもの・0 以上と言えないもの
        assert_eq!(parts("x^2 + y^2 - 1"), None);
        assert_eq!(parts("x^2 - y^2"), None);
        assert_eq!(parts("x^3 + y^2"), None);
        assert_eq!(parts("(x^2 + y^2) (z - 1)"), None);
        assert_eq!(parts("(x - 1)^2"), None);
    }

    #[test]
    fn line_of_two_surfaces() {
        let h = cell(DOM, 40);
        // x² + y² = 0: z 軸。回した式（格子に重ならない線）でも同じ
        for src in ["x^2 + y^2", "(x + 0.3 y - 0.2 z)^2 + (y - 0.7 z + 0.1)^2"] {
            let sets = prog(src).common_zero_sets().unwrap();
            let (m, kind) = common_zero_sets(&sets, DOM, 40, Refine::Normal);
            assert_eq!(kind, "陰関数（線だけの解）");
            let pts = ends(&m);
            assert!(!pts.is_empty(), "{src}");
            for p in &pts {
                for q in &sets[0] {
                    let cols = [vec![p[0]], vec![p[1]], vec![p[2]]];
                    assert!(values_at(q, &cols)[0].abs() < 1e-4, "{src}: {p:?}");
                }
            }
        }
        // 円 x² + y² = 4, z = 0 の長さは 4π（表示範囲に全部入る）
        let sets = prog("(x^2 + y^2 - 4)^2 + z^2").common_zero_sets().unwrap();
        let (m, _) = common_zero_sets(&sets, DOM, 40, Refine::Normal);
        let len = length(&m);
        assert!((len - 4.0 * std::f64::consts::PI).abs() < 0.02 * 4.0 * std::f64::consts::PI, "{len}");
        for p in ends(&m) {
            let r = (p[0] * p[0] + p[1] * p[1]).sqrt();
            assert!((r - 2.0).abs() < 1e-3 * h && p[2].abs() < 1e-3 * h, "{p:?}");
        }
    }

    #[test]
    fn tangent_poles_are_not_drawn() {
        // tan(x)² + y² = 0 の解は x = kπ, y = 0 の線。sin/cos に直さない tan の極（x = π/2 など）で符号が変わる所は、寄らないので描かない
        let sets = prog("tan(x)^2 + y^2").common_zero_sets().unwrap();
        let (m, _) = common_zero_sets(&sets, DOM, 40, Refine::Normal);
        let pts = ends(&m);
        assert!(!pts.is_empty());
        for p in pts {
            let k = (p[0] / std::f64::consts::PI).round();
            assert!((p[0] - k * std::f64::consts::PI).abs() < 1e-4 && p[1].abs() < 1e-4, "{p:?}");
        }
    }

    #[test]
    fn points_of_three_surfaces() {
        let sets = prog("x^2 + y^2 + z^2").common_zero_sets().unwrap();
        let (m, kind) = common_zero_sets(&sets, DOM, 40, Refine::Normal);
        assert_eq!(kind, "陰関数（点だけの解）");
        let pts = ends(&m);
        assert_eq!(pts.len(), 2, "{pts:?}");
        assert!(pts.iter().all(|p| p.iter().all(|v| v.abs() < 1e-6)));
        // 2 つの円の交わり: (x² + y² − 4)² + z² + (x − y)² = 0 → (±√2, ±√2, 0) の 2 点
        let sets = prog("(x^2 + y^2 - 4)^2 + z^2 + (x - y)^2").common_zero_sets().unwrap();
        let (m, _) = common_zero_sets(&sets, DOM, 40, Refine::Normal);
        let mut pts = ends(&m);
        pts.dedup();
        assert_eq!(pts.len(), 2, "{pts:?}");
        for p in pts {
            assert!((p[0].abs() - 2f64.sqrt()).abs() < 1e-5 && (p[0] - p[1]).abs() < 1e-5 && p[2].abs() < 1e-5, "{p:?}");
        }
    }
}

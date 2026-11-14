//! いくつかの式が同時に 0 になる所（交わる線・点）を描く。
//!
//! 2 つの式 $F_1 = F_2 = 0$ の線は、$F_1 = 0$ の面のメッシュの上で $F_2$ の等高線（値 0）を取り出し、
//! ニュートン法で 2 つの面の上へ寄せて作る（[`intersection_curve`]）。寄らない点（`tan` の極の偽の線など）は捨てる。
//! 3 つ以上の式の点は、はじめの 2 つの線の上で 3 つ目の式の符号が変わる所から、全部の式の上へ寄せて作る（[`common_points`]）。
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
            let jac: Vec<[f64; 3]> =
                (0..m).map(|i| [0, 1, 2].map(|c| (vals[i][7 * j + 1 + 2 * c] - vals[i][7 * j + 2 + 2 * c]) / (2.0 * d))).collect();
            if f.iter().chain(jac.iter().flatten()).any(|v| !v.is_finite()) {
                alive[k] = false;
                continue;
            }
            residual_ok[k] = (0..m).all(|i| {
                let g = jac[i].iter().map(|v| v * v).sum::<f64>().sqrt();
                f[i].abs() <= 1e-4 * h * g
            });
            if step == NEWTON_STEPS {
                break;
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
    let moved = newton_onto(&[f1, f2], &points, cell(dom, n));
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
    (keep, out_segs)
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

/// 式 `progs`（3 つ以上）がどれも 0 になる点。はじめの 2 つの式の線の上で、3 つ目の式の符号が変わる所を探し、全部の式の上へ寄せる。
/// 半マスより近い点は 1 つにまとめる。描画側が点とみなすよう、点ごとに同じ点を 2 つと NaN を並べる
pub fn common_points(progs: &[Program], dom: Domain, n: usize, refine: Refine) -> Mesh {
    assert!(progs.len() >= 3);
    let (points, segs) = curve_points(&progs[0], &progs[1], dom, n, refine);
    let h = cell(dom, n);
    let mut seeds = Vec::new();
    if !points.is_empty() {
        let cols: [Vec<f64>; 3] = [0, 1, 2].map(|c| points.iter().map(|p| p[c]).collect());
        let g = values_at(&progs[2], &cols);
        for [a, b] in segs {
            let (ga, gb) = (g[a], g[b]);
            if ga.is_finite() && gb.is_finite() && (ga < 0.0) != (gb < 0.0) {
                let s = ga / (ga - gb);
                seeds.push([0, 1, 2].map(|c| points[a][c] + s * (points[b][c] - points[a][c])));
            }
        }
    }
    let refs: Vec<&Program> = progs.iter().collect();
    let mut found: Vec<[f64; 3]> = Vec::new();
    for p in newton_onto(&refs, &seeds, h).into_iter().flatten() {
        let inside = (0..3).all(|c| p[c] >= dom.lo[c] - 1e-9 * h && p[c] <= dom.hi[c] + 1e-9 * h);
        let near = |q: &[f64; 3]| (0..3).map(|c| (q[c] - p[c]).powi(2)).sum::<f64>() < (0.5 * h).powi(2);
        if inside && !found.iter().any(near) {
            found.push(p);
        }
    }
    let mut out = Vec::with_capacity(found.len() * 9);
    for p in found {
        let p = p.map(|v| v as f32);
        out.extend_from_slice(&p);
        out.extend_from_slice(&p);
        out.extend_from_slice(&[f32::NAN; 3]);
    }
    Mesh { positions: out, ..Default::default() }
}

/// 因数ごとの「同時に 0 になる式の組」（[`Program::common_zero_sets`]）を描く。式が 2 つの組は線、3 つ以上の組は点。
/// 戻り値はメッシュ（曲線）と、説明（線か点か）
pub fn common_zero_sets(sets: &[Vec<Program>], dom: Domain, n: usize, refine: Refine) -> (Mesh, String) {
    let meshes: Vec<Mesh> = sets
        .iter()
        .map(|s| if s.len() == 2 { intersection_curve(&s[0], &s[1], dom, n, refine) } else { common_points(s, dom, n, refine) })
        .collect();
    let (lines, points) = (sets.iter().any(|s| s.len() == 2), sets.iter().any(|s| s.len() > 2));
    let what = match (lines, points) {
        (true, true) => "線・点",
        (true, false) => "線",
        _ => "点",
    };
    (mesh::merge(meshes, true), format!("陰関数（{what}だけの解）"))
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

/// 曲線のメッシュ（線分ごとに 2 点と NaN）を、`value` が 0 以下の所だけにする。両端とも中なら残し、両端とも外なら捨て、
/// 混ざる線分は二分（`rounds` 回）で境目を求めて、中の側だけ残す。長さの無い線分（点）は、中ならそのまま残す
pub fn clip_segments(positions: &mut Vec<f32>, value: &mut dyn FnMut(&[[f64; 3]]) -> Vec<f64>, rounds: usize) {
    let segs: Vec<([f64; 3], [f64; 3])> = positions
        .chunks_exact(9)
        .filter(|s| s[..6].iter().all(|v| v.is_finite()))
        .map(|s| ([s[0] as f64, s[1] as f64, s[2] as f64], [s[3] as f64, s[4] as f64, s[5] as f64]))
        .collect();
    let pts: Vec<[f64; 3]> = segs.iter().flat_map(|&(a, b)| [a, b]).collect();
    let g = value(&pts);
    let mut mixed = Vec::new();
    let mut out: Vec<f32> = Vec::with_capacity(positions.len());
    let emit = |out: &mut Vec<f32>, a: [f64; 3], b: [f64; 3]| {
        out.extend(a.iter().chain(&b).map(|v| *v as f32));
        out.extend_from_slice(&[f32::NAN; 3]);
    };
    for (k, &(a, b)) in segs.iter().enumerate() {
        let (ia, ib) = (g[2 * k] <= 0.0, g[2 * k + 1] <= 0.0);
        match (ia, ib) {
            (true, true) => emit(&mut out, a, b),
            (false, false) => {}
            (true, false) => mixed.push((a, b, g[2 * k], g[2 * k + 1])),
            (false, true) => mixed.push((b, a, g[2 * k + 1], g[2 * k])),
        }
    }
    let ts = edge_cuts(&mixed, value, rounds);
    for (&(a, b, _, _), t) in mixed.iter().zip(ts) {
        emit(&mut out, a, [0, 1, 2].map(|c| a[c] + t * (b[c] - a[c])));
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

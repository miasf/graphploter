//! 陰関数の面が自分と交わる線・尖った点（特異点）を、X の字・円錐の先のまま描く。
//!
//! 格子の 1 マスに頂点を 1 つ置く作り方（Surface Nets）では、2 枚の面が交わる線（ローマ曲面 $x^2y^2 + y^2z^2 + z^2x^2 = xyz$ の座標軸）の上で
//! X の字を作れず、交わる 1 マス手前で折れ曲がった「く」の字が向かい合う形になる（交わりが消える）。円錐の先（ケーリーの 3 次曲面の 4 点）も丸まる。
//! そうした所では $F = 0$ かつ $\nabla F = 0$ で、ヘッセ行列が正と負の両方の向きに曲がる（鞍の形）。
//!
//! 1. 隣り合う三角形の向きが 45° より大きく違う辺の頂点を候補にする（なめらかな所はここで外れるので、ほとんどの式は何もしない）
//! 2. 候補から $\nabla F = 0$ をガウス・ニュートン法で解く（ヘッセ行列は中心差分）。線の向きにはヘッセ行列が 0 なので動かず、
//!    線へ垂直に寄る。寄った点で $F \approx 0$ で、ヘッセ行列の固有値に正と負の両方があれば特異点。
//!    $(x^2+y^2+z^2-1)^2 = 0$ のような二重の面（固有値が 1 つだけ）・ひげのような線（同じ符号の固有値だけ）は外れる
//! 3. 特異点から [`CUT`] マス以内に頂点を持つ三角形を捨て、切れた縁の頂点を、同じ方法で特異点の上に寄せた点とつなぐ
//!    （縁の辺 a → b ごとに三角形 (b, a, a') と (b, a', b')）。交わる線なら 4 枚の半分の面がそれぞれ線に届いて X の字に、
//!    尖った点なら縁が 1 点に集まって円錐になる

use super::*;

/// 候補にする、隣り合う三角形の向きの違い（cos）
const TURN_COS: f64 = 0.7;
/// 特異点の周りで三角形を捨てる半径（格子の何マス分か）。丸まるのは 1 マスほどなので、その外から結び直す
const CUT: f64 = 1.5;
/// $\nabla F = 0$ へ寄せるくり返しの回数（減衰が強いので、1 回で寄る距離は 9 割ほど）
const STEPS: usize = 14;
/// 鞍の形とみなす、正と負の固有値の大きさ（いちばん大きい固有値に対する割合）
const SADDLE: f64 = 0.01;
/// 候補の頂点の数の上限（多すぎる式は、ほとんどが特異点ではない折れ目なので、何もしない）
const MAX_CANDIDATES: usize = 20_000;

/// 点ごとの値・勾配・ヘッセ行列（19 点の中心差分。幅 `e`）。どれかが有限でなければ None
fn derivs(prog: &Program, pts: &[[f64; 3]], e: f64) -> Vec<Option<(f64, [f64; 3], [[f64; 3]; 3])>> {
    // 並び: 中心, +x, −x, +y, −y, +z, −z, そして i < j の組ごとに (+i+j, +i−j, −i+j, −i−j)
    let mut offs: Vec<[f64; 3]> = vec![[0.0; 3]];
    for i in 0..3 {
        for s in [1.0, -1.0] {
            let mut o = [0.0; 3];
            o[i] = s * e;
            offs.push(o);
        }
    }
    for i in 0..3 {
        for j in i + 1..3 {
            for (si, sj) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
                let mut o = [0.0; 3];
                o[i] = si * e;
                o[j] = sj * e;
                offs.push(o);
            }
        }
    }
    let k = offs.len();
    let mut cols: [Vec<f64>; 3] = [Vec::with_capacity(k * pts.len()), Vec::with_capacity(k * pts.len()), Vec::with_capacity(k * pts.len())];
    for p in pts {
        for o in &offs {
            for c in 0..3 {
                cols[c].push(p[c] + o[c]);
            }
        }
    }
    let mut out = vec![C::new(0.0, 0.0); cols[0].len()];
    prog.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
    out.chunks_exact(k)
        .map(|v| {
            let f: Vec<f64> = v.iter().map(|z| as_real(*z)).collect();
            if f.iter().any(|x| !x.is_finite()) {
                return None;
            }
            let g = [0, 1, 2].map(|i| (f[1 + 2 * i] - f[2 + 2 * i]) / (2.0 * e));
            let mut hm = [[0.0; 3]; 3];
            for i in 0..3 {
                hm[i][i] = (f[1 + 2 * i] - 2.0 * f[0] + f[2 + 2 * i]) / (e * e);
            }
            let mut q = 7;
            for i in 0..3 {
                for j in i + 1..3 {
                    let v = (f[q] - f[q + 1] - f[q + 2] + f[q + 3]) / (4.0 * e * e);
                    hm[i][j] = v;
                    hm[j][i] = v;
                    q += 4;
                }
            }
            Some((f[0], g, hm))
        })
        .collect()
}

/// 対称な 3 × 3 行列の固有値（ヤコビ法）
fn eigenvalues(mut a: [[f64; 3]; 3]) -> [f64; 3] {
    for _ in 0..30 {
        let (mut p, mut q, mut big) = (0, 1, 0.0);
        for i in 0..3 {
            for j in i + 1..3 {
                if a[i][j].abs() > big {
                    (p, q, big) = (i, j, a[i][j].abs());
                }
            }
        }
        let scale = a[0][0].abs() + a[1][1].abs() + a[2][2].abs();
        if big <= 1e-14 * scale.max(f64::MIN_POSITIVE) {
            break;
        }
        let theta = 0.5 * (2.0 * a[p][q]).atan2(a[q][q] - a[p][p]);
        let (s, c) = theta.sin_cos();
        let mut b = a;
        for k in 0..3 {
            b[k][p] = c * a[k][p] - s * a[k][q];
            b[k][q] = s * a[k][p] + c * a[k][q];
        }
        let b2 = b;
        for k in 0..3 {
            b[p][k] = c * b2[p][k] - s * b2[q][k];
            b[q][k] = s * b2[p][k] + c * b2[q][k];
        }
        a = b;
    }
    [a[0][0], a[1][1], a[2][2]]
}

fn norm(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// 点の列のそれぞれから、$\nabla F = 0$ の点へ寄せる（減衰付きのガウス・ニュートン法。1 歩は `h` まで、[`STEPS`] 回）。
/// 寄った点で $|\nabla F| \le 10^{-3} \|H\| h$・$|F| \le 10^{-4} \|H\| h^2$・動いた距離が `reach` 以下で、$\|H\|$ が寄せる前の点での値の 1/4〜4 倍なら、
/// その点とヘッセ行列を返す
fn onto_singular(prog: &Program, starts: &[[f64; 3]], h: f64, reach: f64) -> Vec<Option<([f64; 3], [[f64; 3]; 3])>> {
    let e = 1e-2 * h;
    let mut cur: Vec<[f64; 3]> = starts.to_vec();
    let mut alive = vec![true; starts.len()];
    let mut last: Vec<Option<(f64, [f64; 3], [[f64; 3]; 3])>> = vec![None; starts.len()];
    // 寄せる前の点でのヘッセ行列の大きさ
    let mut first = vec![0.0f64; starts.len()];
    for step in 0..=STEPS {
        let idx: Vec<usize> = (0..cur.len()).filter(|&k| alive[k]).collect();
        if idx.is_empty() {
            break;
        }
        let pts: Vec<[f64; 3]> = idx.iter().map(|&k| cur[k]).collect();
        let d = derivs(prog, &pts, e);
        for (&k, dk) in idx.iter().zip(d) {
            let Some((f, g, hm)) = dk else {
                alive[k] = false;
                continue;
            };
            last[k] = Some((f, g, hm));
            if step == 0 {
                first[k] = hm.iter().flatten().map(|v| v * v).sum::<f64>().sqrt();
            }
            if step == STEPS {
                continue;
            }
            // (HᵀH + λI) δ = −Hᵀ g（H は対称なので Hᵀ = H）
            let mut a = vec![vec![0.0; 3]; 3];
            let mut b = vec![0.0; 3];
            for i in 0..3 {
                for j in 0..3 {
                    a[i][j] = (0..3).map(|m| hm[m][i] * hm[m][j]).sum();
                }
                b[i] = -(0..3).map(|m| hm[m][i] * g[m]).sum::<f64>();
            }
            let tr = a[0][0] + a[1][1] + a[2][2];
            if !(tr > 0.0) {
                alive[k] = false;
                continue;
            }
            // 減衰（レーベンバーグ・マーカート）: ヘッセ行列の固有値が小さい向き（交わる線に沿う向き）へは動かさない。
            // 弱くすると、線に沿う向きの小さな 2 階微分で割って大きく跳び、線の端（ピンチ点）まで滑っていく
            for (i, row) in a.iter_mut().enumerate() {
                row[i] += 1e-3 * tr / 3.0;
            }
            let Some(delta) = solve3(a, b) else {
                alive[k] = false;
                continue;
            };
            let len = norm(delta);
            let s = if len > h { h / len } else { 1.0 };
            for c in 0..3 {
                cur[k][c] += delta[c] * s;
            }
        }
    }
    (0..cur.len())
        .map(|k| {
            let (f, g, hm) = last[k]?;
            if !alive[k] {
                return None;
            }
            let hn = hm.iter().flatten().map(|v| v * v).sum::<f64>().sqrt();
            let moved = norm([0, 1, 2].map(|c| cur[k][c] - starts[k][c]));
            // ヘッセ行列の大きさが寄せる前と 4 倍以上違う点は使わない: 交わる線・尖った点ではヘッセ行列はほぼ一定。
            // ヘッセ行列が 0 になる退化した点（(x² + y² + z²)² = 30 x y z の原点）の近くでは、どの点でも値と勾配が小さく、ばらばらの点を拾ってしまう
            let steady = hn >= 0.25 * first[k] && hn <= 4.0 * first[k];
            (hn > 0.0 && steady && norm(g) <= 1e-3 * hn * h && f.abs() <= 1e-4 * hn * h * h && moved <= reach).then_some((cur[k], hm))
        })
        .collect()
}

/// 3 × 3 の連立 1 次方程式（部分ピボット）
fn solve3(mut m: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<[f64; 3]> {
    for c in 0..3 {
        let p = (c..3).max_by(|&i, &j| m[i][c].abs().total_cmp(&m[j][c].abs()))?;
        if m[p][c] == 0.0 || !m[p][c].is_finite() {
            return None;
        }
        m.swap(c, p);
        b.swap(c, p);
        for r in c + 1..3 {
            let k = m[r][c] / m[c][c];
            for j in c..3 {
                m[r][j] -= k * m[c][j];
            }
            b[r] -= k * b[c];
        }
    }
    let mut x = [0.0; 3];
    for c in (0..3).rev() {
        let s: f64 = (c + 1..3).map(|j| m[c][j] * x[j]).sum();
        x[c] = (b[c] - s) / m[c][c];
    }
    x.iter().all(|v| v.is_finite()).then_some(x)
}

/// ヘッセ行列が鞍の形（正と負の両方の固有値が、いちばん大きい固有値の [`SADDLE`] 倍以上）か
fn saddle(hm: &[[f64; 3]; 3]) -> bool {
    let ev = eigenvalues(*hm);
    let m = ev.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    m > 0.0 && ev.iter().any(|&v| v > SADDLE * m) && ev.iter().any(|&v| v < -SADDLE * m)
}

/// 面が自分と交わる線・尖った点の周りを、X の字・円錐の先に描き直す（モジュールの説明を参照）。`h` は格子の 1 マスの幅。
/// 頂点ごとの値（色分け・格子用の座標）を持つメッシュは、何もしない（足した頂点の値が決められないため）
pub(crate) fn sharpen_singularities(mesh: &mut Mesh, prog: &Program, h: f64) {
    if prog.vars().len() != 3 || mesh.indices.is_empty() || !mesh.complex.is_empty() || !mesh.grid_coords.is_empty() || !(h > 0.0) {
        return;
    }
    let pos: Vec<[f64; 3]> = mesh.positions.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect();
    let tri_n: Vec<[f64; 3]> = mesh
        .indices
        .chunks_exact(3)
        .map(|t| {
            let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
            let l = norm(n);
            if l > 0.0 { n.map(|x| x / l) } else { [0.0; 3] }
        })
        .collect();
    // 1. 辺ごとに、両側の三角形の向きの違いを見る
    let mut edge_tri: crate::adaptive::HashMap<(u32, u32), usize> = Default::default();
    let mut cand = vec![false; pos.len()];
    for (ti, t) in mesh.indices.chunks_exact(3).enumerate() {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let key = (a.min(b), a.max(b));
            if let Some(&o) = edge_tri.get(&key) {
                let (n1, n2) = (tri_n[ti], tri_n[o]);
                let cos = n1[0] * n2[0] + n1[1] * n2[1] + n1[2] * n2[2];
                if cos < TURN_COS {
                    for &v in t.iter().chain(&mesh.indices[3 * o..3 * o + 3]) {
                        cand[v as usize] = true;
                    }
                }
            } else {
                edge_tri.insert(key, ti);
            }
        }
    }
    let starts: Vec<[f64; 3]> = (0..pos.len()).filter(|&v| cand[v]).map(|v| pos[v]).collect();
    if starts.is_empty() || starts.len() > MAX_CANDIDATES {
        return;
    }
    // 2. 特異点（鞍の形のものだけ）。近すぎる点はまとめる
    let cell = CUT * h;
    let key = |p: &[f64; 3]| p.map(|v| (v / cell).floor() as i64);
    let mut grid: crate::adaptive::HashMap<[i64; 3], Vec<[f64; 3]>> = Default::default();
    let mut count = 0;
    for (p, hm) in onto_singular(prog, &starts, h, 2.0 * h).into_iter().flatten() {
        if !saddle(&hm) {
            continue;
        }
        let k = key(&p);
        let close = grid.get(&k).is_some_and(|l| l.iter().any(|q| norm([0, 1, 2].map(|c| p[c] - q[c])) < 0.25 * h));
        if !close {
            grid.entry(k).or_default().push(p);
            count += 1;
        }
    }
    if count == 0 {
        return;
    }
    let near = |p: &[f64; 3]| -> bool {
        let k = key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(l) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) {
                        if l.iter().any(|q| norm([0, 1, 2].map(|c| p[c] - q[c])) < cell) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    };
    // 3. 特異点の近くの三角形を捨て、切れた縁を特異点へつなぐ
    let close: Vec<bool> = pos.iter().map(near).collect();
    let mut kept: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    let mut removed_edges: crate::adaptive::HashMap<(u32, u32), u32> = Default::default();
    for t in mesh.indices.chunks_exact(3) {
        if t.iter().any(|&v| close[v as usize]) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *removed_edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        } else {
            kept.extend_from_slice(t);
        }
    }
    // 残した三角形の辺のうち、反対側の三角形を捨てたもの（向きは残した三角形の a → b）
    let mut border: Vec<(u32, u32)> = Vec::new();
    for t in kept.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            if removed_edges.contains_key(&(a.min(b), a.max(b))) {
                border.push((a, b));
            }
        }
    }
    let mut ends: Vec<u32> = border.iter().flat_map(|&(a, b)| [a, b]).collect();
    ends.sort_unstable();
    ends.dedup();
    let projected = onto_singular(prog, &ends.iter().map(|&v| pos[v as usize]).collect::<Vec<_>>(), h, (CUT + 2.0) * h);
    // 寄らなかった頂点（ピンチ点の近くなど、交わる角度が 0 に近い所）は、見つけた特異点のうち一番近い点へつなぐ（穴を空けない）
    let nearest = |p: &[f64; 3]| -> Option<[f64; 3]> {
        let k = key(p);
        let mut best: Option<(f64, [f64; 3])> = None;
        for dx in -2..=2 {
            for dy in -2..=2 {
                for dz in -2..=2 {
                    for q in grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]).into_iter().flatten() {
                        let d = norm([0, 1, 2].map(|c| p[c] - q[c]));
                        if best.is_none_or(|(bd, _)| d < bd) {
                            best = Some((d, *q));
                        }
                    }
                }
            }
        }
        best.filter(|(d, _)| *d <= (CUT + 1.0) * h).map(|(_, q)| q)
    };
    // 寄せた点のうち、0.01 h 以内のものは 1 つの頂点にする（円錐の先では、縁の頂点が全部 1 点に寄る。別々の頂点にすると、隣の扇と辺を共有せず、ひびになる）
    let tol = 0.01 * h;
    let mut made: crate::adaptive::HashMap<[i64; 3], Vec<(u32, [f64; 3])>> = Default::default();
    let mut proj_id: crate::adaptive::HashMap<u32, u32> = Default::default();
    for (&v, pr) in ends.iter().zip(projected) {
        let Some(p) = pr.map(|(p, _)| p).or_else(|| nearest(&pos[v as usize])) else { continue };
        let k = p.map(|x| (x / tol).floor() as i64);
        let mut found = None;
        'look: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &(id, q) in made.get(&[k[0] + dx, k[1] + dy, k[2] + dz]).into_iter().flatten() {
                        if norm([0, 1, 2].map(|c| p[c] - q[c])) <= tol {
                            found = Some(id);
                            break 'look;
                        }
                    }
                }
            }
        }
        let id = found.unwrap_or_else(|| {
            let id = (mesh.positions.len() / 3) as u32;
            mesh.positions.extend(p.map(|x| x as f32));
            mesh.normals.extend_from_slice(&[0.0; 3]);
            made.entry(k).or_default().push((id, p));
            id
        });
        proj_id.insert(v, id);
    }
    for (a, b) in border {
        let (Some(&pa), Some(&pb)) = (proj_id.get(&a), proj_id.get(&b)) else { continue };
        kept.extend_from_slice(&[b, a, pa]);
        if pa != pb {
            kept.extend_from_slice(&[b, pa, pb]);
        }
    }
    mesh.indices = kept;
    // 使われなくなった頂点はそのまま残す（番号を詰めると、粗い版などの番号がずれる）。法線は三角形から作り直す
    compute_normals(mesh);
}

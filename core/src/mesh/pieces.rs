//! 陰関数 F = 0 を、式を変形して部品に分けてから描く（[`crate::eval`] の pieces を参照）。
//!
//! 1. F を因数に分ける（`A B = 0` → `A = 0` と `B = 0`、`Aⁿ = 0` → `A = 0`）
//! 2. 段差の元（`floor`・`mod` など）のある因数は、枝の番号の組 τ ごとに、段差を外したなめらかな式 F_τ にする
//!    （F_τ をさらに因数に分ける）
//! 3. 部品ごとに、ふつうの陰関数として面を作る（符号がふつうに変わるので、推測が要らない）
//! 4. 面のうち、条件に合う部分だけを残す（その枝にいる所・元の式が定義される所）。境目は辺の上で二分して求める
//!
//! 面を部品ごとに別々に作るので、1 つのセルを何枚の面が通っても、互いに邪魔しない。

use super::*;
use crate::eval::Cond;

/// 枝の組の数がこれを超える式（`floor(1000 x)` など）は、部品に分けない
const MAX_TAUS: usize = 20_000;
/// 枝を調べる格子点の数の上限
const MAX_BRANCH_POINTS: usize = 40_000_000;

thread_local! {
    /// 次の [`implicit_solid_with`] で、面を作るブロックを絞る（1 回だけ使う）
    pub(super) static BLOCK_MASK: std::cell::RefCell<Option<Vec<bool>>> = const { std::cell::RefCell::new(None) };
}

/// 描く部品の数の上限（多い順に描く。再生・ドラッグ中は少なくする）
fn max_pieces(refine: Refine) -> usize {
    if refine == Refine::Light { 200 } else { 3000 }
}

/// 部品に分けて描く。分けられない（分けても変わらない）式は None
pub(super) fn implicit_by_pieces(prog: &Program, dom: Domain, n: usize, refine: Refine) -> Option<Mesh> {
    let (factors, changed) = prog.zero_factors();
    let branched = |f: &Program| (1..=MAX_BRANCH_SITES).contains(&f.branch_sites());
    if !changed && !factors.iter().any(branched) {
        return None;
    }
    // 枝の組を先に調べる（多すぎる式は、分けずに今までの道で描く）
    let mut groups: Vec<Option<Vec<BranchGroup>>> = Vec::new();
    let mut truncated = false;
    for f in &factors {
        groups.push(if branched(f) {
            let (g, thin) = branch_groups(f, dom, n)?;
            truncated |= thin;
            Some(g)
        } else {
            None
        });
    }
    // F の計算をいくつかの Worker に分ける仕組みは、1 つの格子でしか使えない。分けられない式として答える
    let split = FIELD_MODE.with(|mode| !matches!(*mode.borrow(), FieldMode::Normal));
    if split {
        FIELD_OUT.with(|out| out.borrow_mut().calls += 2);
        return Some(Mesh::default());
    }
    let size = (0..3).map(|a| (dom.hi[a] - dom.lo[a]).powi(2)).sum::<f64>().sqrt();
    let mut parts: Vec<Mesh> = Vec::new();
    let mut budget = max_pieces(refine);
    for (f, group) in factors.iter().zip(groups) {
        if f.is_constant() {
            continue;
        }
        let Some(group) = group else {
            let mut m = mesh_masked(f, dom, n, refine, None);
            if changed {
                clip_piece(&mut m, f, &[], &[prog], size);
            }
            parts.push(m);
            continue;
        };
        let nb3 = group.first().map_or(0, |g| g.nb3);
        for g in group {
            // 三角形が多すぎる（画面が固まる）ときも、残りの部品は描かない
            if budget == 0 || parts.iter().map(|m| m.indices.len() / 3).sum::<usize>() > MAX_IMPLICIT_TRIANGLES {
                truncated = true;
                break;
            }
            budget -= 1;
            let fz = f.frozen(&g.tau);
            let conds = f.branch_conds(&g.tau);
            let (subs, sub_changed) = fz.zero_factors();
            let mut mask = vec![false; nb3];
            for &b in &g.blocks {
                mask[b as usize] = true;
            }
            for sub in &subs {
                if sub.is_constant() {
                    continue;
                }
                // 枝の部品は、作り直し（小さい図形・細い部分を探して細かくする処理）をしない。
                // ブロックを絞ったせいで小さく見えるだけで、小さい図形ではない（1 つ 30 ms ほどかかる）
                let mut m = mesh_masked(sub, dom, n, Refine::Off, Some(mask.clone()));
                if m.indices.is_empty() {
                    continue;
                }
                let mut defined: Vec<&Program> = Vec::new();
                if sub_changed {
                    defined.push(&fz);
                }
                if changed {
                    defined.push(prog);
                }
                clip_piece(&mut m, sub, &conds, &defined, size);
                if !m.indices.is_empty() {
                    parts.push(m);
                }
            }
        }
    }
    let note = parts.iter().map(|m| m.note.as_str()).find(|s| !s.is_empty()).unwrap_or("").to_string();
    let mut out = merge(parts, false);
    out.note = if truncated { "（細かすぎて描けない部分あり）".into() } else { note };
    Some(out)
}

/// ブロックを絞って、ふつうの陰関数として面を作る
fn mesh_masked(prog: &Program, dom: Domain, n: usize, refine: Refine, mask: Option<Vec<bool>>) -> Mesh {
    BLOCK_MASK.with(|m| *m.borrow_mut() = mask);
    let mesh = implicit_solid_with(prog, dom, n, &[], 0, false, refine);
    BLOCK_MASK.with(|m| *m.borrow_mut() = None);
    mesh
}

/// 枝の組 1 つ分
struct BranchGroup {
    /// 段差の元ごとの枝の番号
    tau: Vec<f64>,
    /// この枝の格子点があるブロック（とその隣の 1 セル分）
    blocks: Vec<u32>,
    /// この枝の格子点の数（多い順に描く）
    count: usize,
    /// ブロックの総数
    nb3: usize,
}

/// 式の枝の組を、[`implicit_solid_with`] と同じ格子の点で調べる。格子点の多い順。多すぎる式は None。
///
/// 格子より薄い枝（`mod(x y, z)` の z = 0 の近くに無限に重なる層など）は、面を描き分けられないので入れない。
/// 薄いかどうかは、その枝の格子点のうち「3 つの軸のどれでも、隣（前か後ろ）が同じ枝の点」の割合で決める
/// （厚みが格子の 1 マスより薄い層は、厚みの向きに 2 点並ばない）。入れなかった枝があれば、2 つ目の戻り値が true
fn branch_groups(f: &Program, dom: Domain, n: usize) -> Option<(Vec<BranchGroup>, bool)> {
    // implicit_solid_with と同じ格子（表示範囲の外側に 1 マスずつ広げたもの）
    let n = n.max(2);
    let mut dom = dom;
    for ax in 0..3 {
        let step = (dom.hi[ax] - dom.lo[ax]) / n as f64;
        dom.lo[ax] -= step;
        dom.hi[ax] += step;
    }
    let n = n + 2;
    let m = n + 1;
    if m * m * m > MAX_BRANCH_POINTS {
        return None;
    }
    let nb = n.div_ceil(BLOCK);
    let k = f.branch_sites();
    let branch = f.branches();
    // 格子点ごとの枝の組の番号（層ごとに計算する）
    let mut ids: Vec<u32> = vec![NONE; m * m * m];
    let mut taus: Vec<Vec<f64>> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();
    let mut seen: crate::adaptive::HashMap<[i64; MAX_BRANCH_SITES], u32> = Default::default();
    let xs: Vec<f64> = (0..m * m).map(|l| dom.at(0, (l % m) as f64, n)).collect();
    let ys: Vec<f64> = (0..m * m).map(|l| dom.at(1, (l / m) as f64, n)).collect();
    for kz in 0..m {
        let zs = vec![dom.at(2, kz as f64, n); m * m];
        let br = branch.eval(&[&xs, &ys, &zs], m * m);
        for l in 0..m * m {
            let b = &br[l * k..(l + 1) * k];
            if b.iter().any(|v| !v.is_finite()) {
                continue;
            }
            let mut key = [0i64; MAX_BRANCH_SITES];
            for (o, &v) in key.iter_mut().zip(b) {
                *o = v as i64;
            }
            let id = *seen.entry(key).or_insert_with(|| {
                taus.push(b.to_vec());
                counts.push(0);
                (taus.len() - 1) as u32
            });
            counts[id as usize] += 1;
            ids[l + m * m * kz] = id;
        }
        if taus.len() > MAX_TAUS {
            return None;
        }
    }
    // 枝ごとの、その枝の格子点があるブロック。ブロックのセルの角と、その外側の 1 点までを見る
    let mut blocks: Vec<Vec<u32>> = vec![Vec::new(); taus.len()];
    let mut stamp: Vec<u32> = vec![0; taus.len()];
    for bk in 0..nb {
        for bj in 0..nb {
            for bi in 0..nb {
                let bid = (bi + nb * (bj + nb * bk)) as u32;
                let span = |b: usize| (BLOCK * b).saturating_sub(1)..=(BLOCK * (b + 1) + 1).min(m - 1);
                for kz in span(bk) {
                    for jy in span(bj) {
                        let row = m * (jy + m * kz);
                        for ix in span(bi) {
                            let id = ids[ix + row];
                            if id != NONE && stamp[id as usize] != bid + 1 {
                                stamp[id as usize] = bid + 1;
                                blocks[id as usize].push(bid);
                            }
                        }
                    }
                }
            }
        }
    }
    // 枝ごとの、厚みのある所の格子点の数
    let mut solid: Vec<usize> = vec![0; taus.len()];
    // 枝ごとの、表示範囲の中に格子点があるか（端で切れて薄くなっただけの枝は、「描けない部分」に数えない）
    let mut inner: Vec<bool> = vec![false; taus.len()];
    let strides = [1, m, m * m];
    for kz in 0..m {
        for jy in 0..m {
            for ix in 0..m {
                let l = ix + m * (jy + m * kz);
                let id = ids[l];
                if id == NONE {
                    continue;
                }
                let q = [ix, jy, kz];
                let paired = (0..3).all(|a| (q[a] > 0 && ids[l - strides[a]] == id) || (q[a] + 1 < m && ids[l + strides[a]] == id));
                solid[id as usize] += paired as usize;
                // 格子の端の 2 点より内側（表示範囲の中）
                inner[id as usize] |= q.iter().all(|&x| x >= 2 && x + 2 < m);
            }
        }
    }
    let mut thin = false;
    let mut out: Vec<BranchGroup> = Vec::new();
    for (id, (tau, blocks)) in taus.into_iter().zip(blocks).enumerate() {
        if 10 * solid[id] < 3 * counts[id] {
            thin |= inner[id];
            continue;
        }
        out.push(BranchGroup { tau, blocks, count: counts[id], nb3: nb * nb * nb });
    }
    // 格子点の多い順（同じ数なら見つけた順。毎回同じ並びにする）
    out.sort_by_key(|g| std::cmp::Reverse(g.count));
    Some((out, thin))
}

/// 点の列で式を計算する（実数。実数にならない所は NaN）
fn eval_at(prog: &Program, pts: &[[f64; 3]]) -> Vec<f64> {
    let cols: [Vec<f64>; 3] = [0, 1, 2].map(|a| pts.iter().map(|p| p[a]).collect());
    let mut out = vec![C::new(0.0, 0.0); pts.len()];
    prog.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
    out.into_iter().map(as_real).collect()
}

/// 点を面 `surf` = 0 の上に寄せる（勾配の向きにニュートン法で 3 回）。大きく動く点・計算できない点は元のまま
fn project_to_surface(surf: &Program, pts: &[[f64; 3]], size: f64) -> Vec<[f64; 3]> {
    let h = 1e-6 * size;
    let limit = 0.02 * size;
    let mut cur = pts.to_vec();
    for _ in 0..3 {
        let f0 = eval_at(surf, &cur);
        let mut grad = vec![[0.0f64; 3]; cur.len()];
        for a in 0..3 {
            let shift = |s: f64| -> Vec<[f64; 3]> {
                cur.iter()
                    .map(|p| {
                        let mut q = *p;
                        q[a] += s;
                        q
                    })
                    .collect()
            };
            let (fp, fm) = (eval_at(surf, &shift(h)), eval_at(surf, &shift(-h)));
            for (g, (p, m)) in grad.iter_mut().zip(fp.iter().zip(&fm)) {
                g[a] = (p - m) / (2.0 * h);
            }
        }
        for ((p, f), g) in cur.iter_mut().zip(&f0).zip(&grad) {
            let g2 = g[0] * g[0] + g[1] * g[1] + g[2] * g[2];
            if !f.is_finite() || !g2.is_finite() || g2 <= 0.0 {
                continue;
            }
            let step = [0, 1, 2].map(|a| f * g[a] / g2);
            if step.iter().all(|s| s.is_finite()) {
                for a in 0..3 {
                    p[a] -= step[a];
                }
            }
        }
    }
    for (c, p) in cur.iter_mut().zip(pts) {
        let d2: f64 = (0..3).map(|a| (c[a] - p[a]).powi(2)).sum();
        if d2.is_nan() || d2 > limit * limit {
            *c = *p;
        }
    }
    cur
}

/// 面 `surf` = 0 のメッシュから、条件に合う部分だけを残す。
/// - `conds`: 全部が成り立つ所だけ残す（その枝にいる所）
/// - `defined`: 全部の式の値が有限な所だけ残す（元の式が定義される所）
///
/// 条件の境目が面そのものに重なること（`mod(x y, z) = 0` の枝 τ の面 x y = τ z は、枝の境目 x y / z = τ の上）がある。
/// メッシュの頂点は面から少しずれているので、そのまま条件を調べると、境目のどちら側に出るかが点ごとにばらつく。
/// そこで頂点を面の上に寄せてから調べ、境目からのずれが丸め誤差ほどなら、境目の上にあるとみなす
/// （境目を含む条件なら成り立ち、含まない条件なら成り立たない）
fn clip_piece(mesh: &mut Mesh, surf: &Program, conds: &[Cond], defined: &[&Program], size: f64) {
    let count = mesh.positions.len() / 3;
    if count == 0 || (conds.is_empty() && defined.is_empty()) {
        return;
    }
    let pts: Vec<[f64; 3]> = mesh.positions.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect();
    let mut inside = vec![true; count];
    // 境目が面に重なっていない条件（辺の上で境目を探すときに調べる）
    let mut live: Vec<&Cond> = Vec::new();
    if !conds.is_empty() {
        let on = project_to_surface(surf, &pts, size);
        for c in conds {
            let s = eval_at(&c.prog, &on);
            let tol = |b: f64| 1e-6 * (1.0 + b.abs());
            let near = |v: f64, b: f64| b.is_finite() && (v - b).abs() <= tol(b);
            // ほとんどの頂点が同じ端の上にある: 面が境目に重なっている。条件は、どこでも成り立つか、どこでも成り立たない。
            // 「全部」にしないのは、面が自分と交わる線（x y = 0 の x = y = 0 など）の近くの頂点は、面の上に寄せきれないから
            let finite: Vec<f64> = s.iter().copied().filter(|v| v.is_finite()).collect();
            let mostly = |b: f64| !finite.is_empty() && 10 * finite.iter().filter(|&&v| near(v, b)).count() >= 9 * finite.len();
            if mostly(c.lo) {
                if !c.lo_closed {
                    inside.fill(false);
                }
                continue;
            }
            if mostly(c.hi) {
                if !c.hi_closed {
                    inside.fill(false);
                }
                continue;
            }
            live.push(c);
            for (k, &v) in s.iter().enumerate() {
                let ok = if near(v, c.lo) {
                    c.lo_closed
                } else if near(v, c.hi) {
                    c.hi_closed
                } else {
                    c.holds(v)
                };
                inside[k] &= ok;
            }
        }
    }
    for d in defined {
        for (k, v) in eval_at(d, &pts).into_iter().enumerate() {
            inside[k] &= !v.is_nan();
        }
    }
    if inside.iter().all(|&i| i) {
        return;
    }
    if !inside.iter().any(|&i| i) {
        *mesh = Mesh { note: std::mem::take(&mut mesh.note), ..Default::default() };
        return;
    }
    // 条件が変わる辺（成り立つ側の頂点, 成り立たない側の頂点）の上で、境目を二分で探す
    let holds_at = |q: &[[f64; 3]]| -> Vec<bool> {
        let mut ok = vec![true; q.len()];
        for c in &live {
            for (o, v) in ok.iter_mut().zip(eval_at(&c.prog, q)) {
                *o &= c.holds(v);
            }
        }
        for d in defined {
            for (o, v) in ok.iter_mut().zip(eval_at(d, q)) {
                *o &= !v.is_nan();
            }
        }
        ok
    };
    let mut cut_of: crate::adaptive::HashMap<(u32, u32), usize> = Default::default();
    let mut cuts: Vec<(u32, u32)> = Vec::new();
    for t in mesh.indices.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            let (i, o) = if inside[a as usize] && !inside[b as usize] {
                (a, b)
            } else if inside[b as usize] && !inside[a as usize] {
                (b, a)
            } else {
                continue;
            };
            cut_of.entry((i, o)).or_insert_with(|| {
                cuts.push((i, o));
                cuts.len() - 1
            });
        }
    }
    // 辺の上の位置 t（0: 成り立つ側の頂点）。lo は成り立つ、hi は成り立たない
    let mut lo = vec![0.0f64; cuts.len()];
    let mut hi = vec![1.0f64; cuts.len()];
    for _ in 0..24 {
        let mid: Vec<[f64; 3]> = cuts
            .iter()
            .enumerate()
            .map(|(e, &(i, o))| {
                let t = 0.5 * (lo[e] + hi[e]);
                let (p, q) = (pts[i as usize], pts[o as usize]);
                [0, 1, 2].map(|a| p[a] + (q[a] - p[a]) * t)
            })
            .collect();
        for (e, ok) in holds_at(&mid).into_iter().enumerate() {
            let t = 0.5 * (lo[e] + hi[e]);
            if ok {
                lo[e] = t;
            } else {
                hi[e] = t;
            }
        }
    }
    // 境目の頂点を足す（法線は両端の間を取る）
    let cut_vertex: Vec<u32> = cuts
        .iter()
        .enumerate()
        .map(|(e, &(i, o))| {
            let t = lo[e];
            let (p, q) = (pts[i as usize], pts[o as usize]);
            let id = mesh.push_vertex([0, 1, 2].map(|a| p[a] + (q[a] - p[a]) * t));
            let (ni, no) = (i as usize * 3, o as usize * 3);
            if mesh.normals.len() >= 3 * count {
                let nv: [f32; 3] = [0, 1, 2].map(|a| mesh.normals[ni + a] + (mesh.normals[no + a] - mesh.normals[ni + a]) * t as f32);
                let len = (nv[0] * nv[0] + nv[1] * nv[1] + nv[2] * nv[2]).sqrt().max(1e-30);
                mesh.normals.extend(nv.map(|c| c / len));
            }
            id
        })
        .collect();
    let cut = |i: u32, o: u32| cut_vertex[cut_of[&(i, o)]];
    let mut indices: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    for t in mesh.indices.chunks_exact(3) {
        let ins = [inside[t[0] as usize], inside[t[1] as usize], inside[t[2] as usize]];
        match ins.iter().filter(|&&i| i).count() {
            3 => indices.extend_from_slice(t),
            1 => {
                let k = ins.iter().position(|&i| i).unwrap_or(0);
                let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                indices.extend_from_slice(&[a, cut(a, b), cut(a, c)]);
            }
            2 => {
                let k = ins.iter().position(|&i| !i).unwrap_or(0);
                let (c, a, b) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                let (ca, cb) = (cut(a, c), cut(b, c));
                indices.extend_from_slice(&[a, b, cb, a, cb, ca]);
            }
            _ => {}
        }
    }
    // 使われなくなった頂点を捨てて詰める
    let total = mesh.positions.len() / 3;
    let mut remap = vec![NONE; total];
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let has_normals = mesh.normals.len() == mesh.positions.len();
    for k in indices.iter_mut() {
        let old = *k as usize;
        if remap[old] == NONE {
            remap[old] = (positions.len() / 3) as u32;
            positions.extend_from_slice(&mesh.positions[old * 3..old * 3 + 3]);
            if has_normals {
                normals.extend_from_slice(&mesh.normals[old * 3..old * 3 + 3]);
            }
        }
        *k = remap[old];
    }
    *mesh = Mesh { positions, normals, indices, note: std::mem::take(&mut mesh.note), ..Default::default() };
}


#[cfg(test)]
mod tests {
    use super::*;

    fn prog(src: &str) -> Program {
        Program::compile(&crate::parser::parse(src).unwrap(), &["x", "y", "z"]).unwrap()
    }

    /// 点 q の近く（1 辺 h の箱で、隣の箱まで）にメッシュの頂点があるか調べる関数
    fn near_fn(m: &Mesh, h: f64) -> impl Fn([f64; 3]) -> bool {
        let cells: std::collections::HashSet<[i64; 3]> =
            m.positions.chunks_exact(3).map(|p| [0, 1, 2].map(|a| (p[a] as f64 / h).floor() as i64)).collect();
        move |q: [f64; 3]| {
            let c = q.map(|v| (v / h).floor() as i64);
            (-1..=1).any(|a| (-1..=1).any(|b| (-1..=1).any(|e| cells.contains(&[c[0] + a, c[1] + b, c[2] + e]))))
        }
    }

    /// 頂点（位置でまとめる）でつながった成分ごとの三角形の数
    fn component_sizes(m: &Mesh) -> Vec<usize> {
        let mut ids: std::collections::HashMap<[i64; 3], usize> = Default::default();
        let vid: Vec<usize> = m
            .positions
            .chunks_exact(3)
            .map(|p| {
                let next = ids.len();
                *ids.entry([0, 1, 2].map(|a| (p[a] as f64 * 1e4).round() as i64)).or_insert(next)
            })
            .collect();
        let mut parent: Vec<usize> = (0..ids.len()).collect();
        fn root(p: &mut [usize], mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        for t in m.indices.chunks_exact(3) {
            let a = root(&mut parent, vid[t[0] as usize]);
            for &k in &t[1..] {
                let b = root(&mut parent, vid[k as usize]);
                parent[b] = a;
            }
        }
        let mut sizes: std::collections::HashMap<usize, usize> = Default::default();
        for t in m.indices.chunks_exact(3) {
            *sizes.entry(root(&mut parent, vid[t[0] as usize])).or_default() += 1;
        }
        sizes.into_values().collect()
    }

    #[test]
    fn squared_factors_are_drawn_as_plain_surfaces() {
        // (x² + y² − 4)² (z − 1)² = 0 は、円柱 x² + y² = 4 と平面 z = 1。F は 0 に触れるだけで符号が変わらないが、
        // 因数に分ければ、どちらもふつうの面。粗い格子・軽い計算（回転の再生中）でも、穴や破片ができない
        let d = Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
        for (n, refine) in [(80, Refine::Normal), (40, Refine::Light), (24, Refine::Light)] {
            let m = implicit_solid_opt(&prog("(x^2+y^2-4)^2 (z-1)^2"), d, n, &[], 0, refine);
            let h = 20.0 / n as f64;
            let near = near_fn(&m, h);
            let (mut hit, mut all) = (0, 0);
            for i in 0..=60 {
                for j in 0..=60 {
                    // 平面の上の点と、円柱の上の点
                    let (x, y) = (-9.0 + 0.3 * i as f64, -9.0 + 0.3 * j as f64);
                    let (a, z) = (i as f64 * 0.1047, -5.5 + 11.0 * j as f64 / 60.0);
                    for q in [[x, y, 1.0], [2.0 * a.cos(), 2.0 * a.sin(), z]] {
                        all += 1;
                        hit += near(q) as usize;
                    }
                }
            }
            assert_eq!(hit, all, "細かさ {n}: 面の欠け");
            // 面から離れた頂点が無い
            for p in m.positions.chunks_exact(3) {
                let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
                let dist = ((x * x + y * y).sqrt() - 2.0).abs().min((z - 1.0).abs());
                assert!(dist < 0.6 * h, "細かさ {n}: 面から {dist} 離れた頂点");
            }
            // 平面と円柱の 2 つだけ（破片が無い）
            assert_eq!(component_sizes(&m).len(), 2, "細かさ {n}");
        }
    }

    #[test]
    fn mod_surfaces_match_the_exact_pieces() {
        // mod(x y, z) = 0 は、x y = τ z（τ は整数）。τ = 0 は平面 x = 0 と y = 0、ほかは双曲放物面 z = x y / τ。
        // 上下（z の正負）どちらも全面に出て、破片が無い
        let d = Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
        let m = implicit_solid_opt(&prog("mod(x y, z)"), d, 80, &[], 0, Refine::Normal);
        let near = near_fn(&m, 0.25);
        let mut miss = 0;
        for i in 0..=76 {
            for j in 0..=44 {
                let (u, z) = (-9.5 + 0.25 * i as f64, -5.5 + 0.25 * j as f64);
                // 平面 x = 0 と y = 0（z = 0 の近くは、描き分けられない層が重なるので見ない）
                if z.abs() > 0.5 {
                    miss += !near([0.0, u, z]) as usize + !near([u, 0.0, z]) as usize;
                }
            }
        }
        assert_eq!(miss, 0, "平面 x = 0・y = 0 の欠け");
        for tau in [1.0, -1.0, 2.0, -3.0] {
            let mut miss = 0;
            for i in 0..=76 {
                for j in 0..=76 {
                    let (x, y) = (-9.5 + 0.25 * i as f64, -9.5 + 0.25 * j as f64);
                    let z = x * y / tau;
                    if z.abs() > 0.5 && z.abs() < 5.5 {
                        miss += !near([x, y, z]) as usize;
                    }
                }
            }
            assert_eq!(miss, 0, "面 x y = {tau} z の欠け");
        }
        // どの頂点も、どれかの面 x y = τ z の上にある
        for p in m.positions.chunks_exact(3) {
            let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
            if z.abs() < 0.5 {
                continue;
            }
            let tau = (x * y / z).round();
            let dist = (x * y - tau * z).abs() / (x * x + y * y + tau * tau).sqrt().max(1e-9);
            assert!(dist < 0.1, "面から {dist} 離れた頂点 ({x}, {y}, {z})");
        }
        // 小さな破片（手裏剣のような切れ端）が無い
        let small = component_sizes(&m).into_iter().filter(|&s| s <= 50).count();
        assert_eq!(small, 0, "破片の数");
    }

    #[test]
    fn unattained_branch_values_are_not_drawn() {
        // z = mod(x y, z) は、mod(x y, z) が z に近づくが z にはならないので、零点が無い。
        // 枝 τ の式 z − (x y − τ z) = 0 の面は、枝の境目（x y / z = τ + 1）の上で、その境目は枝に含まれない
        let d = Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
        let m = implicit_solid_opt(&prog("z - mod(x y, z)"), d, 80, &[], 0, Refine::Normal);
        let far = m.positions.chunks_exact(3).filter(|p| (p[2] as f64).abs() > 0.5).count();
        assert_eq!(far, 0, "z = 0 から離れた所の頂点");
    }

    #[test]
    fn branch_pieces_are_clipped_to_their_branch() {
        // floor(x) + z = 0 は、1 ≤ … の階段: i ≤ x < i + 1 で z = −i。段ごとに平らな面で、段の間に面は無い
        let d = Domain { lo: [-4.0, -4.0, -6.0], hi: [4.0, 4.0, 6.0] };
        let m = implicit_solid_opt(&prog("floor(x) + z"), d, 64, &[], 0, Refine::Normal);
        assert!(!m.indices.is_empty());
        for p in m.positions.chunks_exact(3) {
            let (x, z) = (p[0] as f64, p[2] as f64);
            // 段の端（x が整数の近く）は、どちらの段でもよい
            let ok = [x.floor(), (x + 1e-3).floor(), (x - 1e-3).floor()].iter().any(|i| (z + i).abs() < 1e-3);
            assert!(ok, "段の上に無い頂点 x = {x}, z = {z}");
        }
        let near = near_fn(&m, 0.125);
        for i in -3..3 {
            for s in 1..8 {
                let x = i as f64 + s as f64 / 8.0;
                assert!(near([x, 0.3, -(i as f64)]), "段 {i} の x = {x} が欠けている");
            }
        }
    }
}

//! 陰関数の面の折れ目（鋭い辺・角）を立てる（計画 `docs/plan-render-algorithms.md` の P2-1。Dual Contouring の頂点の置き方）。
//!
//! Surface Nets は 1 マスに頂点を 1 つ、辺の交わる点の重心に置く。`max(|x|, |y|, |z|) = 1` の立方体の辺・角のように、
//! ∇F が飛ぶ折れ目では、重心は折れ目から内側へずれ、辺・角が丸まる（テスト表 S-11）。Dual Contouring（Ju ほか 2002）は、
//! 交わる点とそこでの法線（ヘルミートデータ）から、接平面たちに最も近い点（二次誤差 QEF の最小）に頂点を置いて、折れ目を立てる。
//! ここでは、できあがったメッシュの頂点に後から同じことをする（Surface Nets の作り方は 4 か所にあるので、1 か所で済ませる）:
//!
//! 1. 頂点ごとに ∇F から法線を求め、隣の頂点と法線が 30° 以上違い、その間で勾配が跳ぶ（真ん中の法線がどちらか一方とほぼ同じ）
//!    頂点を候補にする（なめらかな面では、粗い格子で大きく曲がっていてもここで外れ、頂点は動かない）
//! 2. 候補とその隣の頂点を、ニュートン法で F = 0 の上へ寄せ（1.5 マスまで）、寄せた点の法線を求め直す
//! 3. 候補ごとに、自分と隣の寄せた点の接平面から QEF を作り、固有値の小さい向き（いちばん大きい固有値の 1/10 未満）を捨てた
//!    擬似逆行列で、重心に最も近い最小の点を求める。平面が 2 向き以上あれば（辺・角）そこへ、1 向きなら寄せた点へ動かす。
//!    1.75 マス（1 マスの対角線）より遠い・F ≈ 0 にならない点には動かさない

use super::*;

/// 折れ目の候補にする、隣どうしの法線のなす角（30°）の cos
const CREASE_COS: f64 = 0.866;
/// 候補の数の上限（多すぎる式は、折れ目ではない（細かく波打つ・段差が多い）ので何もしない）
const MAX_CANDIDATES: usize = 50_000;

/// 点ごとの F と ∇F（中心差分。刻み `e`）。どれかが有限でなければ None。
/// 前進差分だと、折れ目の上（max の引数が等しい所）の負の側で勾配が 0 になる（|x| が減っても max は変わらない）
fn value_grad(prog: &Program, pts: &[[f64; 3]], e: f64) -> Vec<Option<(f64, [f64; 3])>> {
    value_grad_kink(prog, pts, e).into_iter().map(|x| x.map(|(f, g, _)| (f, g))).collect()
}

/// [`value_grad`] に、その点が折れ目の上か（前進差分と後退差分が、勾配の大きさの 1/10 より食い違う）を足したもの。
/// 折れ目の上では中心差分の勾配が 2 つの面の法線の平均（偽の向き）になるので、その接平面は QEF に使わない
fn value_grad_kink(prog: &Program, pts: &[[f64; 3]], e: f64) -> Vec<Option<(f64, [f64; 3], bool)>> {
    let offs: [[f64; 3]; 7] = [[0.0; 3], [e, 0.0, 0.0], [-e, 0.0, 0.0], [0.0, e, 0.0], [0.0, -e, 0.0], [0.0, 0.0, e], [0.0, 0.0, -e]];
    let mut cols: [Vec<f64>; 3] = [Vec::with_capacity(7 * pts.len()), Vec::with_capacity(7 * pts.len()), Vec::with_capacity(7 * pts.len())];
    for p in pts {
        for o in &offs {
            for (b, col) in cols.iter_mut().enumerate() {
                col.push(p[b] + o[b]);
            }
        }
    }
    let mut out = vec![C::new(0.0, 0.0); cols[0].len()];
    prog.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
    out.chunks_exact(7)
        .map(|v| {
            let f: Vec<f64> = v.iter().map(|z| as_real(*z)).collect();
            if f.iter().any(|x| !x.is_finite()) {
                return None;
            }
            let g = [0, 1, 2].map(|a| (f[1 + 2 * a] - f[2 + 2 * a]) / (2.0 * e));
            let jump = (0..3).map(|a| ((f[1 + 2 * a] - f[0]) - (f[0] - f[2 + 2 * a])).abs() / e).fold(0.0f64, f64::max);
            Some((f[0], g, jump > 0.1 * norm(g)))
        })
        .collect()
}

fn norm(v: [f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// 3×3 の対称行列の固有値・固有ベクトル（ヤコビ法）。固有ベクトルは列ごと
fn eigen_sym(mut a: [[f64; 3]; 3]) -> ([f64; 3], [[f64; 3]; 3]) {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        let (mut p, mut q, mut big) = (0, 1, 0.0);
        for i in 0..3 {
            for j in i + 1..3 {
                if a[i][j].abs() > big {
                    (p, q, big) = (i, j, a[i][j].abs());
                }
            }
        }
        if big < 1e-15 * (a[0][0].abs() + a[1][1].abs() + a[2][2].abs()).max(1e-300) {
            break;
        }
        let theta = 0.5 * (2.0 * a[p][q]).atan2(a[q][q] - a[p][p]);
        let (c, s) = (theta.cos(), theta.sin());
        // a ← Jᵀ a J（J は (p, q) 平面の回転）
        let mut b = a;
        for k in 0..3 {
            b[k][p] = c * a[k][p] - s * a[k][q];
            b[k][q] = s * a[k][p] + c * a[k][q];
        }
        let mut d = b;
        for k in 0..3 {
            d[p][k] = c * b[p][k] - s * b[q][k];
            d[q][k] = s * b[p][k] + c * b[q][k];
        }
        a = d;
        for row in v.iter_mut() {
            let (vp, vq) = (row[p], row[q]);
            row[p] = c * vp - s * vq;
            row[q] = s * vp + c * vq;
        }
    }
    ([a[0][0], a[1][1], a[2][2]], v)
}

/// 接平面 (点 q, 単位法線 n) の組に最も近い点（QEF の最小）。重心 m に最も近い解を、固有値の小さい向きを捨てた擬似逆行列で。
/// 戻り値は (点, 残した向きの数)
fn qef(planes: &[([f64; 3], [f64; 3])]) -> ([f64; 3], usize) {
    let k = planes.len() as f64;
    let m = [0, 1, 2].map(|c| planes.iter().map(|p| p.0[c]).sum::<f64>() / k);
    let mut a = [[0.0; 3]; 3];
    let mut r = [0.0; 3]; // Aᵀ(b − A m) = Σ n (n · (q − m))
    for (q, n) in planes {
        let d = dot(*n, [q[0] - m[0], q[1] - m[1], q[2] - m[2]]);
        for i in 0..3 {
            r[i] += n[i] * d;
            for j in 0..3 {
                a[i][j] += n[i] * n[j];
            }
        }
    }
    let (lam, vecs) = eigen_sym(a);
    let top = lam.iter().fold(0.0f64, |x, &l| x.max(l));
    let mut x = m;
    let mut rank = 0;
    for i in 0..3 {
        if lam[i] > 0.1 * top {
            rank += 1;
            let u = [vecs[0][i], vecs[1][i], vecs[2][i]];
            let c = dot(u, r) / lam[i];
            for (xc, uc) in x.iter_mut().zip(u) {
                *xc += c * uc;
            }
        }
    }
    (x, rank)
}

/// 折れ目（鋭い辺・角）の頂点を、Dual Contouring と同じ置き方で折れ目の上へ動かす。`cell` は格子の 1 マスの大きさ
pub(crate) fn sharpen_creases(mesh: &mut Mesh, prog: &Program, cell: f64) {
    let count = mesh.positions.len() / 3;
    if count == 0 || prog.vars().len() != 3 || !(cell > 0.0) {
        return;
    }
    let pos = |k: usize| [0, 1, 2].map(|c| mesh.positions[3 * k + c] as f64);
    // 隣り合う頂点（三角形の辺）
    let mut nbr: Vec<Vec<u32>> = vec![Vec::new(); count];
    for t in mesh.indices.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            nbr[a as usize].push(b);
            nbr[b as usize].push(a);
        }
    }
    for list in nbr.iter_mut() {
        list.sort_unstable();
        list.dedup();
    }
    // 1. 法線と候補
    let e = 1e-4 * cell;
    let pts: Vec<[f64; 3]> = (0..count).map(pos).collect();
    let vg = value_grad(prog, &pts, e);
    let unit = |g: [f64; 3]| {
        let l = norm(g);
        (l.is_finite() && l > 1e-12).then(|| g.map(|c| c / l))
    };
    let normals: Vec<Option<[f64; 3]>> = vg.iter().map(|x| x.and_then(|(_, g)| unit(g))).collect();
    // 法線が 30° 以上違う隣どうしのうち、本当の折れ目（勾配が途中で跳ぶ）だけを選ぶ。なめらかに大きく曲がっている所
    // （粗い格子のジャイロイド）では、真ん中の法線が両方の間（それぞれから半分ほど）になる。折れ目では、真ん中の法線が
    // どちらか一方とほぼ同じか、角の近くでは 3 つ目の面の向きになる
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for v in 0..count {
        let Some(nv) = normals[v] else { continue };
        for &j in &nbr[v] {
            let j = j as usize;
            if j > v && normals[j].is_some_and(|nj| dot(nv, nj) < CREASE_COS) {
                pairs.push((v, j));
            }
        }
    }
    if pairs.len() > 4 * MAX_CANDIDATES {
        return;
    }
    let mids: Vec<[f64; 3]> = pairs.iter().map(|&(v, j)| [0, 1, 2].map(|c| 0.5 * (pts[v][c] + pts[j][c]))).collect();
    let vg_mid = value_grad_kink(prog, &mids, e);
    let mut is_cand = vec![false; count];
    for (&(v, j), m) in pairs.iter().zip(&vg_mid) {
        let (Some(nv), Some(nj), Some((_, gm, kink))) = (normals[v], normals[j], *m) else { continue };
        let Some(nm) = unit(gm) else { continue };
        let angle = |a: [f64; 3], b: [f64; 3]| dot(a, b).clamp(-1.0, 1.0).acos();
        let (whole, a, b) = (angle(nv, nj), angle(nm, nv), angle(nm, nj));
        // なめらか: 真ん中の法線が 2 つの間にあり（角の和がほぼ全体）、どちらからも 1/4 以上離れている。それ以外は折れ目
        // （角の近くでは、真ん中の法線が 3 つ目の面の向きになり、どちらからも離れる）
        // 真ん中がちょうど折れ目の上なら、中心差分の法線が 2 つの面の平均になるので、それも折れ目とする
        let smooth = !kink && a + b <= 1.1 * whole && a.min(b) >= 0.25 * whole;
        if !smooth {
            is_cand[v] = true;
            is_cand[j] = true;
        }
    }
    let cand: Vec<usize> = (0..count).filter(|&v| is_cand[v]).collect();
    if cand.is_empty() || cand.len() > MAX_CANDIDATES {
        return;
    }
    // 2. 候補と隣の頂点を F = 0 へ寄せる（ニュートン法 8 回。1.5 マスまで。折れ目の上では 1 回で 1/3 ほどしか寄らない）
    let mut involved: Vec<usize> = cand.iter().flat_map(|&v| std::iter::once(v).chain(nbr[v].iter().map(|&j| j as usize))).collect();
    involved.sort_unstable();
    involved.dedup();
    let mut q: Vec<[f64; 3]> = involved.iter().map(|&v| pts[v]).collect();
    for _ in 0..8 {
        let vgq = value_grad(prog, &q, e);
        for (i, x) in vgq.iter().enumerate() {
            let Some((f, g)) = x else { continue };
            let gg = dot(*g, *g);
            if !(gg > 1e-24) {
                continue;
            }
            let step = g.map(|c| c * f / gg);
            let next = [0, 1, 2].map(|c| q[i][c] - step[c]);
            let p0 = pts[involved[i]];
            if norm([next[0] - p0[0], next[1] - p0[1], next[2] - p0[2]]) <= 1.5 * cell {
                q[i] = next;
            }
        }
    }
    let vgq = value_grad_kink(prog, &q, e);
    let index_of = |v: usize| involved.binary_search(&v).ok();
    // 3. 候補ごとに QEF の最小へ
    let mut moves: Vec<(usize, [f64; 3])> = Vec::new();
    for &v in &cand {
        let mut planes = Vec::with_capacity(1 + nbr[v].len());
        for w in std::iter::once(v).chain(nbr[v].iter().map(|&j| j as usize)) {
            let Some(i) = index_of(w) else { continue };
            let Some((f, g, kink)) = vgq[i] else { continue };
            let Some(n) = unit(g) else { continue };
            if kink {
                continue; // 折れ目の上の点（中心差分の勾配が偽の向き）
            }
            // 面の上に寄った点（1 マスの 2% 以内）だけを使う（寄らなかった点の接平面は、ずれている）
            if f.abs() > 0.02 * cell * norm(g) {
                continue;
            }
            planes.push((q[i], n));
        }
        if planes.len() < 3 {
            continue;
        }
        let (x, rank) = qef(&planes);
        let target = if rank >= 2 { x } else { q[index_of(v).unwrap_or(0)] };
        let p0 = pts[v];
        if norm([target[0] - p0[0], target[1] - p0[1], target[2] - p0[2]]) <= 1.75 * cell {
            moves.push((v, target));
        }
    }
    if moves.is_empty() {
        return;
    }
    // 動かした先で F ≈ 0 になるものだけ動かす
    let targets: Vec<[f64; 3]> = moves.iter().map(|m| m.1).collect();
    let check = value_grad(prog, &targets, e);
    let mut moved = false;
    for ((v, t), c) in moves.iter().zip(check) {
        let Some((f, g)) = c else { continue };
        if f.abs() <= 0.05 * cell * norm(g).max(1e-300) {
            for k in 0..3 {
                mesh.positions[3 * v + k] = t[k] as f32;
            }
            moved = true;
        }
    }
    if moved {
        compute_normals(mesh);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qef_finds_edges_and_corners() {
        // 平面 x = 1 と y = 1 の点: 辺 (1, 1, z) の上の、重心に最も近い点
        let planes = [([1.0, 0.8, 0.1], [1.0, 0.0, 0.0]), ([1.0, 0.9, -0.1], [1.0, 0.0, 0.0]), ([0.8, 1.0, 0.0], [0.0, 1.0, 0.0]), ([0.9, 1.0, 0.2], [0.0, 1.0, 0.0])];
        let (x, rank) = qef(&planes);
        assert_eq!(rank, 2);
        assert!((x[0] - 1.0).abs() < 1e-12 && (x[1] - 1.0).abs() < 1e-12 && (x[2] - 0.05).abs() < 1e-12, "{x:?}");
        // 3 枚の平面: 角 (1, 1, 1)
        let planes = [([1.0, 0.8, 0.9], [1.0, 0.0, 0.0]), ([0.8, 1.0, 0.9], [0.0, 1.0, 0.0]), ([0.9, 0.8, 1.0], [0.0, 0.0, 1.0])];
        let (x, rank) = qef(&planes);
        assert_eq!(rank, 3);
        assert!(x.iter().all(|c| (c - 1.0).abs() < 1e-12), "{x:?}");
    }
}

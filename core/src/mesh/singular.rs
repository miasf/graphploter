//! 陰関数の面が自分と交わる線・尖った点（特異点）を、X の字・円錐の先のまま描く。
//!
//! 格子の 1 マスに頂点を 1 つ置く作り方（Surface Nets）では、2 枚の面が交わる線（ローマ曲面 $x^2y^2 + y^2z^2 + z^2x^2 = xyz$ の座標軸）の上で
//! X の字を作れず、交わる 1 マス手前で折れ曲がった「く」の字が向かい合う形になる（交わりが消える）。円錐の先（ケーリーの 3 次曲面の 4 点）も丸まる。
//! そうした所では $F = 0$ かつ $\nabla F = 0$ で、ヘッセ行列が正と負の両方の向きに曲がる（鞍の形）。
//!
//! 1. 周りの三角形の向きがそろっていない頂点を候補にし（なめらかな所はここで外れるので、ほとんどの式は何もしない）、
//!    1 回だけ微分して、勾配が小さくヘッセ行列が鞍の形の頂点に絞る
//! 2. 候補から $\nabla F = 0$ をガウス・ニュートン法で解く（ヘッセ行列は中心差分）。線の向きにはヘッセ行列が 0 なので動かず、
//!    線へ垂直に寄る。寄った点で $F \approx 0$ で、ヘッセ行列の固有値に正と負の両方があれば特異点。
//!    $(x^2+y^2+z^2-1)^2 = 0$ のような二重の面（固有値が 1 つだけ）・ひげのような線（同じ符号の固有値だけ）は外れる
//! 3. 特異点から [`CUT`] マス以内に頂点を持つ三角形を捨て、切れた縁の頂点を、同じ方法で特異点の上に寄せた点とつなぐ
//!    （縁の辺 a → b ごとに三角形 (b, a, a') と (b, a', b')）。交わる線なら 4 枚の半分の面がそれぞれ線に届いて X の字に、
//!    尖った点なら縁が 1 点に集まって円錐になる

use super::*;

/// 候補にする、頂点の周りの三角形の向きの平均の長さ（これより短ければ候補。周りの向きがそろっていれば 1）
const TURN_COS: f64 = 0.97;
/// 特異点の周りで三角形を捨てる半径（格子の何マス分か）。丸まるのは 1 マスほどなので、その外から結び直す
const CUT: f64 = 1.5;
/// 交わる角度 θ が小さい所（ピンチ点の近く）で捨てる半径の上限（マス）。2 枚の面は線から距離 r の所で r θ しか離れていないので、
/// 1.5 マス離れる r = 1.5 / θ までは、格子の 1 マスに 2 枚が入って混ざる（くしの歯のような切れ込み）。その外から結び直す
const MAX_CUT: f64 = 6.0;
/// $\nabla F = 0$ へ寄せるくり返しの回数（減衰が強いので、1 回で寄る距離は 9 割ほど）
const STEPS: usize = 14;
/// 鞍の形とみなす、正と負の固有値の大きさ（いちばん大きい固有値に対する割合）
const SADDLE: f64 = 0.01;
/// 候補の頂点の数の上限（多すぎる式は、ほとんどが特異点ではない折れ目なので、何もしない）
const MAX_CANDIDATES: usize = 5_000;

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
    // 寄り切った点（勾配が十分小さい）。それ以上は計算しない
    let mut done = vec![false; starts.len()];
    for step in 0..=STEPS {
        let idx: Vec<usize> = (0..cur.len()).filter(|&k| alive[k] && !done[k]).collect();
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
            let hn = hm.iter().flatten().map(|v| v * v).sum::<f64>().sqrt();
            if step == 0 {
                first[k] = hn;
            }
            if norm(g) <= 1e-4 * hn * h {
                done[k] = true;
                continue;
            }
            // 届く範囲を出たら、もう寄せない
            if norm([0, 1, 2].map(|c| cur[k][c] - starts[k][c])) > reach {
                alive[k] = false;
                continue;
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

/// 点を面 F = 0 へ寄せる（ニュートン法 4 回。1 回に 1 マスまで）。寄らない・動きすぎる（1 マス超）点はそのまま
fn onto_surface(prog: &Program, pts: &[[f64; 3]], h: f64) -> Vec<[f64; 3]> {
    let mut cur = pts.to_vec();
    for _ in 0..4 {
        if cur.is_empty() {
            break;
        }
        let d = derivs(prog, &cur, 1e-2 * h);
        for (q, dk) in cur.iter_mut().zip(d) {
            let Some((f, g, _)) = dk else { continue };
            let gg = g[0] * g[0] + g[1] * g[1] + g[2] * g[2];
            if !(gg > 0.0) {
                continue;
            }
            let step = g.map(|c| c * f / gg);
            if norm(step) <= h {
                for c in 0..3 {
                    q[c] -= step[c];
                }
            }
        }
    }
    cur.iter()
        .zip(pts)
        .map(|(q, p)| if norm([0, 1, 2].map(|c| q[c] - p[c])) <= h { *q } else { *p })
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

/// 鞍の形の特異点で、三角形を捨てる半径（マス）。正と負の固有値のうち大きいものどうしの比 ρ から、交わる角度
/// θ = 2 atan √ρ を出し、1.5 / θ（[`CUT`]〜[`MAX_CUT`]）
fn cut_radius(hm: &[[f64; 3]; 3]) -> f64 {
    let ev = eigenvalues(*hm);
    let pos = ev.iter().fold(0.0f64, |a, &v| a.max(v));
    let neg = ev.iter().fold(0.0f64, |a, &v| a.max(-v));
    let (lo, hi) = (pos.min(neg), pos.max(neg));
    if !(hi > 0.0) {
        return CUT;
    }
    let theta = 2.0 * (lo / hi).sqrt().atan();
    (1.5 / theta.max(1e-9)).clamp(CUT, MAX_CUT)
}

/// ヘッセ行列が鞍の形（正と負の両方の固有値が、いちばん大きい固有値の [`SADDLE`] 倍以上）か
fn saddle(hm: &[[f64; 3]; 3]) -> bool {
    let ev = eigenvalues(*hm);
    let m = ev.iter().fold(0.0f64, |a, v| a.max(v.abs()));
    m > 0.0 && ev.iter().any(|&v| v > SADDLE * m) && ev.iter().any(|&v| v < -SADDLE * m)
}

/// 面が自分と交わる線・尖った点の周りを、X の字・円錐の先に描き直す（モジュールの説明を参照）。`h` は格子の 1 マスの幅。
/// 頂点ごとの値（色分け・格子用の座標）を持つメッシュは、何もしない（足した頂点の値が決められないため）。見つけた特異点を返す
pub(crate) fn sharpen_singularities(mesh: &mut Mesh, prog: &Program, h: f64) -> Vec<[f64; 3]> {
    if prog.vars().len() != 3 || mesh.indices.is_empty() || !mesh.complex.is_empty() || !mesh.grid_coords.is_empty() || !(h > 0.0) {
        return Vec::new();
    }
    let pos: Vec<[f64; 3]> = mesh.positions.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect();
    // 1. 頂点ごとに、周りの三角形の向き（単位ベクトル）の平均の長さを見る。向きがそろっていれば 1 に近く、
    //    急に折れている所（丸まった交わり・尖った点）では短くなる。辺の表を作らないので、三角形が多い式でも軽い
    let mut sum = vec![[0.0f64; 3]; pos.len()];
    let mut cnt = vec![0u32; pos.len()];
    for t in mesh.indices.chunks_exact(3) {
        let (a, b, c) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
        let l = norm(n);
        if !(l > 0.0) {
            continue;
        }
        for &w in t {
            let s = &mut sum[w as usize];
            for k in 0..3 {
                s[k] += n[k] / l;
            }
            cnt[w as usize] += 1;
        }
    }
    let cand: Vec<bool> = (0..pos.len()).map(|w| cnt[w] >= 2 && norm(sum[w]) < TURN_COS * cnt[w] as f64).collect();
    let starts: Vec<[f64; 3]> = (0..pos.len()).filter(|&v| cand[v]).map(|v| pos[v]).collect();
    if starts.is_empty() || starts.len() > MAX_CANDIDATES {
        return Vec::new();
    }
    // 1'. 1 回だけ微分して絞る: 特異点から距離 d の所では |∇F| ≈ ‖H‖ d なので、|∇F| が ‖H‖ の 3 マス分より大きい頂点
    // （なめらかな面が粗い格子で折れて見えるだけの所。極小曲面など）と、ヘッセ行列が鞍の形でない頂点は、ニュートン法にかけない
    let first = derivs(prog, &starts, 1e-2 * h);
    let starts: Vec<[f64; 3]> = starts
        .into_iter()
        .zip(first)
        .filter_map(|(p, d)| {
            let (_, g, hm) = d?;
            let hn = hm.iter().flatten().map(|v| v * v).sum::<f64>().sqrt();
            (hn > 0.0 && norm(g) <= 3.0 * hn * h && saddle(&hm)).then_some(p)
        })
        .collect();
    if starts.is_empty() {
        return Vec::new();
    }
    // 2. 特異点（鞍の形のものだけ）と、その周りで三角形を捨てる半径。近すぎる点はまとめる
    // 格子の大きさ: 捨てる半径の上限 + 切れた縁の辺の長さ（2 マス）。近くの特異点は隣の箱までで見つかる
    let cell = (MAX_CUT + 2.0) * h;
    let key = |p: &[f64; 3]| p.map(|v| (v / cell).floor() as i64);
    let mut grid: crate::adaptive::HashMap<[i64; 3], Vec<([f64; 3], f64)>> = Default::default();
    let mut count = 0;
    for (p, hm) in onto_singular(prog, &starts, h, 2.0 * h).into_iter().flatten() {
        if !saddle(&hm) {
            continue;
        }
        let k = key(&p);
        let close = grid.get(&k).is_some_and(|l| l.iter().any(|(q, _)| norm([0, 1, 2].map(|c| p[c] - q[c])) < 0.25 * h));
        if !close {
            grid.entry(k).or_default().push((p, cut_radius(&hm) * h));
            count += 1;
        }
    }
    if count == 0 {
        return Vec::new();
    }
    let found: Vec<[f64; 3]> = grid.values().flatten().map(|(p, _)| *p).collect();
    let near = |p: &[f64; 3]| -> bool {
        let k = key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(l) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) {
                        if l.iter().any(|(q, r)| norm([0, 1, 2].map(|c| p[c] - q[c])) < *r) {
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
    let projected = onto_singular(prog, &ends.iter().map(|&v| pos[v as usize]).collect::<Vec<_>>(), h, (MAX_CUT + 2.0) * h);
    // 寄らなかった頂点（ピンチ点の近くなど、交わる角度が 0 に近い所）は、見つけた特異点のうち一番近い点へつなぐ（穴を空けない）
    let nearest = |p: &[f64; 3]| -> Option<[f64; 3]> {
        let k = key(p);
        let mut best: Option<(f64, [f64; 3], f64)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    for &(q, r) in grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]).into_iter().flatten() {
                        // 捨てた半径（＋2 マス。切れた縁の頂点は、捨てた三角形の辺 1 本分まで外にある）の中にある特異点のうち、いちばん近いもの
                        let d = norm([0, 1, 2].map(|c| p[c] - q[c]));
                        if d <= r + 2.0 * h && best.is_none_or(|(bd, _, _)| d < bd) {
                            best = Some((d, q, r));
                        }
                    }
                }
            }
        }
        best.map(|(_, q, _)| q)
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
    // 縁の頂点から特異点までを、1 マスほどの刻みの列にする（途中の点は面 F = 0 へ寄せる）。捨てる半径が大きい所
    // （ピンチ点の近くで 6 マスまで）を 1 枚の三角形で結ぶと、曲がった面から平らな三角形が離れて、穴・とげに見える
    let mut columns: crate::adaptive::HashMap<u32, Vec<u32>> = Default::default();
    {
        let mut mids: Vec<[f64; 3]> = Vec::new();
        let mut owner: Vec<(u32, usize)> = Vec::new(); // (縁の頂点, 列の中の番号)
        let mut cols: Vec<(u32, Vec<Option<u32>>)> = Vec::new();
        for (&v, &pv) in &proj_id {
            let a = pos[v as usize];
            let p = [0, 1, 2].map(|c| mesh.positions[3 * pv as usize + c] as f64);
            let k = (norm([0, 1, 2].map(|c| p[c] - a[c])) / h).ceil().max(1.0) as usize;
            let mut col = vec![Some(v)];
            for j in 1..k {
                let t = j as f64 / k as f64;
                mids.push([0, 1, 2].map(|c| a[c] + t * (p[c] - a[c])));
                owner.push((v, j));
                col.push(None);
            }
            col.push(Some(pv));
            cols.push((v, col));
        }
        let projected = onto_surface(prog, &mids, h);
        let mut slot: crate::adaptive::HashMap<(u32, usize), u32> = Default::default();
        for ((v, j), q) in owner.into_iter().zip(projected) {
            let id = (mesh.positions.len() / 3) as u32;
            mesh.positions.extend(q.map(|x| x as f32));
            mesh.normals.extend_from_slice(&[0.0; 3]);
            slot.insert((v, j), id);
        }
        for (v, col) in cols {
            let ids: Vec<u32> = col.iter().enumerate().map(|(j, c)| c.unwrap_or_else(|| slot[&(v, j)])).collect();
            columns.insert(v, ids);
        }
    }
    for (a, b) in border {
        let (Some(ca), Some(cb)) = (columns.get(&a), columns.get(&b)) else { continue };
        // 2 本の列の間を、t（縁 0 → 特異点 1）の小さいほうから進めて三角形で埋める。向きは今までの (b, a, a') と (b, a', b') と同じ
        let (na, nb) = (ca.len() - 1, cb.len() - 1);
        let (mut i, mut j) = (0, 0);
        while i < na || j < nb {
            let advance_a = j == nb || (i < na && (i + 1) as f64 / na as f64 <= (j + 1) as f64 / nb as f64);
            if advance_a {
                kept.extend_from_slice(&[cb[j], ca[i], ca[i + 1]]);
                i += 1;
            } else if cb[j + 1] != ca[i] {
                kept.extend_from_slice(&[cb[j], ca[i], cb[j + 1]]);
                j += 1;
            } else {
                j += 1;
            }
        }
    }
    mesh.indices = kept;
    // 使われなくなった頂点はそのまま残す（番号を詰めると、粗い版などの番号がずれる）。法線は三角形から作り直す
    compute_normals(mesh);
    found
}

/// 面にならない特異点の線（ローマ曲面の座標軸の |t| > 1/2 の「ひげ」、ホイットニーの傘 x² = y² z の z < 0 の「柄」）を、線として描く。
///
/// F ≥ 0（か ≤ 0）が線の上でだけ 0 に触れる所は、符号が変わらないので面にならず、格子では見つからない。そうした線は、交わる線（二重線）の
/// 端のピンチ点から続いている。見つけた特異点 `seeds` から、ヘッセ行列の 0 の固有値の向き（線の向き）へ約 1 マスずつ進み、
/// 進んだ先を ∇F = 0 へ寄せる（[`onto_singular`]）。ヘッセ行列の 0 でない 2 つの固有値の符号がそろう点（ひげ・柄）だけを線にし、
/// 二重線の上（鞍の形）は記録せずに通り過ぎる。表示範囲を出る・寄らない・F が 0 でない・通った所に戻ったら止める。
/// 線は `mesh.edge_lines`（2 点ずつ、間は NaN。`lib.rs` が実線の境目の線にする）に足す
pub(crate) fn trace_singular_lines(mesh: &mut Mesh, prog: &Program, h: f64, dom: Domain, seeds: &[[f64; 3]]) {
    if seeds.is_empty() || prog.vars().len() != 3 || !(h > 0.0) {
        return;
    }
    let inside = |p: &[f64; 3]| (0..3).all(|a| p[a] >= dom.lo[a] - 1e-9 && p[a] <= dom.hi[a] + 1e-9);
    let key = |p: &[f64; 3]| p.map(|v| (v / h).floor() as i64);
    let mut visited: crate::adaptive::HashMap<[i64; 3], ()> = Default::default();
    // 線の向き（ヘッセ行列の 0 の固有値の固有ベクトル）と、ひげか（ほかの 2 つの固有値の符号がそろう）。
    // ピンチ点の近くでは 0 に近い固有値が 2 つになるので、小さい 2 つのうち、今までの向き `prev` に近いほうを線の向きにする
    let classify = |hm: &[[f64; 3]; 3], prev: Option<[f64; 3]>| -> Option<([f64; 3], bool)> {
        let (ev, vecs) = super::crease::eigen_sym(*hm);
        let m = ev.iter().fold(0.0f64, |a, v| a.max(v.abs()));
        if !(m > 0.0) {
            return None;
        }
        let mut order = [0, 1, 2];
        order.sort_by(|&i, &j| ev[i].abs().total_cmp(&ev[j].abs()));
        let vec_of = |i: usize| [vecs[0][i], vecs[1][i], vecs[2][i]];
        let k = match prev {
            Some(d) if ev[order[1]].abs() <= SADDLE * m => {
                let align = |i: usize| {
                    let v = vec_of(i);
                    (v[0] * d[0] + v[1] * d[1] + v[2] * d[2]).abs()
                };
                if align(order[1]) > align(order[0]) { order[1] } else { order[0] }
            }
            _ => order[0],
        };
        if ev[k].abs() > SADDLE * m {
            return None; // 線ではない（尖った点など）
        }
        let others: Vec<f64> = (0..3).filter(|&i| i != k).map(|i| ev[i]).collect();
        let whisker = others.iter().all(|&v| v > SADDLE * m) || others.iter().all(|&v| v < -SADDLE * m);
        Some((vec_of(k), whisker))
    };
    let max_steps = (0..3).map(|a| (dom.hi[a] - dom.lo[a]) / h).sum::<f64>() as usize + 8;
    let mut lines: Vec<f32> = Vec::new();
    for seed in seeds {
        if visited.contains_key(&key(seed)) {
            continue;
        }
        let Some(Some((p0, hm0))) = onto_singular(prog, &[*seed], h, 2.0 * h).into_iter().next() else { continue };
        let Some((dir0, _)) = classify(&hm0, None) else { continue };
        visited.insert(key(&p0), ());
        for sign in [1.0, -1.0] {
            let (mut p, mut dir) = (p0, dir0.map(|c| c * sign));
            let mut prev_whisker: Option<[f64; 3]> = None;
            for _ in 0..max_steps {
                let guess = [0, 1, 2].map(|c| p[c] + h * dir[c]);
                let Some(Some((q, hm))) = onto_singular(prog, &[guess], h, 2.0 * h).into_iter().next() else { break };
                let Some((d, whisker)) = classify(&hm, Some(dir)) else { break };
                let step = [0, 1, 2].map(|c| q[c] - p[c]);
                // 戻った・ほとんど進まない（線の端）なら止める
                if norm(step) < 0.25 * h || (step[0] * dir[0] + step[1] * dir[1] + step[2] * dir[2]) <= 0.0 || !inside(&q) {
                    break;
                }
                let k = key(&q);
                if visited.contains_key(&k) {
                    break;
                }
                visited.insert(k, ());
                if whisker {
                    if let Some(a) = prev_whisker {
                        lines.extend([a[0] as f32, a[1] as f32, a[2] as f32, q[0] as f32, q[1] as f32, q[2] as f32, f32::NAN, f32::NAN, f32::NAN]);
                    }
                    prev_whisker = Some(q);
                } else {
                    prev_whisker = None;
                }
                // 向きは前と同じ側にそろえる
                let s = if d[0] * dir[0] + d[1] * dir[1] + d[2] * dir[2] < 0.0 { -1.0 } else { 1.0 };
                dir = d.map(|c| c * s);
                p = q;
            }
        }
    }
    mesh.edge_lines.extend(lines);
}

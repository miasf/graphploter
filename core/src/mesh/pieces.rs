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

/// 描く部品の数の上限（多い順に描く）
const MAX_PIECES: usize = 3000;
/// 枝を調べる格子の細かさの下限。どの枝を描くかは、面を作る格子の細かさ（再生・ドラッグ中は粗くなる）ではなく、
/// この細かさで決める。粗い格子で調べると、粗くしたときに枝ごと消える（止めたときと形が変わる）
const ENUM_N: usize = 80;
/// 枝の部品の三角形の数の目安。超えそうなら、枝の部品を作る格子を粗くする（面の数は減らさない）
const TARGET_PIECE_TRIANGLES: f64 = 600_000.0;
/// 枝の部品を作る格子の細かさの下限
const MIN_PIECE_N: usize = 24;

/// 部品に分けて描く。分けられない（分けても変わらない）式は None
pub(super) fn implicit_by_pieces(prog: &Program, dom: Domain, n: usize, refine: Refine) -> Option<Mesh> {
    // 極のある式は、通分した分子 N = 0 を描く（N に極は無い）。分母 D が 0 の所は、後で切り落とす
    let rational = prog.rationalized().filter(|(num, den)| poles_worth_removing(num, den, dom));
    let (base, den) = match &rational {
        Some((num, den)) => (num, Some(den)),
        None => (prog, None),
    };
    let nonzero: Vec<&Program> = den.into_iter().collect();
    let (factors, split_changed) = base.zero_factors();
    let changed = split_changed || rational.is_some();
    let branched = |f: &Program| (1..=MAX_BRANCH_SITES).contains(&f.branch_sites());
    if !changed && !factors.iter().any(branched) {
        return None;
    }
    // 枝の組を先に調べる（多すぎる式は、分けずに今までの道で描く）
    let mut groups: Vec<Option<Vec<BranchGroup>>> = Vec::new();
    let mut truncated = false;
    for f in &factors {
        groups.push(if branched(f) {
            let (g, thin) = branch_groups(f, dom, n.max(ENUM_N))?;
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
    let mut budget = MAX_PIECES;
    for (f, group) in factors.iter().zip(groups) {
        if f.is_constant() {
            continue;
        }
        let Some(group) = group else {
            let mut m = mesh_masked(f, dom, n, refine, None);
            if changed {
                clip_piece(&mut m, f, &[], &[prog], &nonzero, size);
            }
            parts.push(m);
            continue;
        };
        // 面の数が多い式は、三角形が増えすぎないよう、枝の部品を作る格子を粗くする（1 ブロックあたり約 80 個）
        let blocks: usize = group.iter().map(|g| g.blocks.len()).sum();
        let en = n.max(ENUM_N);
        let est = 80.0 * blocks as f64 * (n as f64 / en as f64).powi(2);
        let pn = if est > TARGET_PIECE_TRIANGLES {
            ((n as f64 * (TARGET_PIECE_TRIANGLES / est).sqrt()) as usize).max(MIN_PIECE_N).min(n)
        } else {
            n
        };
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
            let (sub_dom, sub_n, mask, tight) = piece_grid(&g.blocks, (g.lo, g.hi), dom, en, pn);
            for sub in &subs {
                if sub.is_constant() {
                    continue;
                }
                // 枝の部品は、作り直し（小さい図形・細い部分を探して細かくする処理）をしない。
                // ブロックを絞ったせいで小さく見えるだけで、小さい図形ではない（1 つ 30 ms ほどかかる）
                // 区間演算で面が無いと分かるブロックを外す（枝の範囲を面が通らない部品は、ここで終わる）
                let mut mask = mask.clone();
                if !prune_mask(sub, sub_dom, sub_n, tight, &mut mask) {
                    continue;
                }
                let mut m = mesh_masked(sub, sub_dom, sub_n, Refine::Off, Some(mask));
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
                clip_piece(&mut m, sub, &conds, &defined, &nonzero, size);
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

/// 通分した式 N/D で描くほうがよいか。
///
/// 通分すると、極（D = 0 で N ≠ 0。F の符号は変わるが零点でない所）は消えるが、分子と分母が同時に 0 になる所
/// （`sin(x)/x` の x = 0。F はそこでなめらかにつながる）は、逆に N = 0 の余計な面になる。
/// 粗い格子の辺のうち D の符号が変わる辺の上で、D = 0 の点を二分で求め、そこで N が 0 でない辺（本当の極）が
/// 2 割より少なければ、通分しない。D の符号が変わらない式（`1/x²` など）は、余計な面ができないので通分する
fn poles_worth_removing(num: &Program, den: &Program, dom: Domain) -> bool {
    const N: usize = 32;
    let m = N + 1;
    let pts: Vec<[f64; 3]> =
        (0..m * m * m).map(|l| [dom.at(0, (l % m) as f64, N), dom.at(1, (l / m % m) as f64, N), dom.at(2, (l / (m * m)) as f64, N)]).collect();
    let (nv, dv) = (eval_at(num, &pts), eval_at(den, &pts));
    // D の符号が変わる辺（両端の格子点の番号）
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for l in 0..m * m * m {
        let q = [l % m, l / m % m, l / (m * m)];
        for (a, stride) in [1, m, m * m].into_iter().enumerate() {
            if q[a] + 1 < m && dv[l] * dv[l + stride] < 0.0 && nv[l].is_finite() && nv[l + stride].is_finite() {
                edges.push((l, l + stride));
            }
        }
    }
    if edges.is_empty() {
        return true;
    }
    // 辺の上の位置 t（lo の側は D の符号が始点と同じ）
    let (mut lo, mut hi) = (vec![0.0f64; edges.len()], vec![1.0f64; edges.len()]);
    let at = |t: &[f64]| -> Vec<[f64; 3]> {
        edges.iter().zip(t).map(|(&(i, o), &t)| [0, 1, 2].map(|a| pts[i][a] + (pts[o][a] - pts[i][a]) * t)).collect()
    };
    for _ in 0..40 {
        let mid: Vec<f64> = lo.iter().zip(&hi).map(|(l, h)| 0.5 * (l + h)).collect();
        for (e, d) in eval_at(den, &at(&mid)).into_iter().enumerate() {
            if d * dv[edges[e].0] > 0.0 {
                lo[e] = mid[e];
            } else {
                hi[e] = mid[e];
            }
        }
    }
    let poles = eval_at(num, &at(&lo))
        .into_iter()
        .zip(&edges)
        .filter(|(v, (i, o))| v.abs() > 1e-6 * nv[*i].abs().max(nv[*o].abs()))
        .count();
    5 * poles >= edges.len()
}

/// ブロックを絞って、ふつうの陰関数として面を作る
fn mesh_masked(prog: &Program, dom: Domain, n: usize, refine: Refine, mask: Option<Vec<bool>>) -> Mesh {
    BLOCK_MASK.with(|m| *m.borrow_mut() = mask);
    let mesh = implicit_solid_with(prog, dom, n, &[], 0, false, refine);
    BLOCK_MASK.with(|m| *m.borrow_mut() = None);
    mesh
}

/// [`implicit_solid_with`] が使う格子（表示範囲の外側に 1 マスずつ広げたもの）の、(範囲, 1 辺のマスの数)
fn solid_grid(dom: Domain, n: usize) -> (Domain, usize) {
    let n = n.max(2);
    let mut dom = dom;
    for ax in 0..3 {
        let step = (dom.hi[ax] - dom.lo[ax]) / n as f64;
        dom.lo[ax] -= step;
        dom.hi[ax] += step;
    }
    (dom, n + 2)
}

/// 枝の部品を作る格子。細かさ `en` の格子のブロックの一覧（その枝を描く所）から、
/// (範囲, 1 辺のマスの数, ブロックごとの印) を作る。[`implicit_solid_with`] にそのまま渡せる。
///
/// 格子は、表示範囲を `pn` 等分した全体の格子の一部（格子の線が全体と同じなので、面の位置は全体で作ったときと同じ）。
/// 枝のブロックを囲む立方体（1 辺のマスの数は 3 つの軸で同じ）だけにして、小さい部品で格子全体の大きさの手間をかけない。
/// さらに、枝の格子点のある範囲 `bbox`（細かさ `en` の格子の点の番号。両端を含む）の 1 マス外までに絞る
/// （ブロックは 1 辺 [`BLOCK`] マスあるので、小さい枝では、ブロックの範囲は枝の何倍も広い。枝の外の面は、作っても切り落とすだけ）
fn piece_grid(blocks: &[u32], bbox: ([u32; 3], [u32; 3]), dom: Domain, en: usize, pn: usize) -> (Domain, usize, Vec<bool>, Domain) {
    let (edom, e) = solid_grid(dom, en);
    let enb = e.div_ceil(BLOCK);
    let pn = pn.max(2);
    let step = [0, 1, 2].map(|a| (dom.hi[a] - dom.lo[a]) / pn as f64);
    // 全体の格子（外側に 1 マス広げたもの。マスの番号は −1 〜 pn）での、点 x のマスの位置
    let cell = |a: usize, x: f64| (x - dom.lo[a]) / step[a];
    // ブロック b（en の格子）の、軸 a の範囲
    let eb = |b: u32| {
        let b = b as usize;
        [b % enb, (b / enb) % enb, b / (enb * enb)]
    };
    let world = |a: usize, k: usize| (edom.at(a, (BLOCK * k) as f64, e), edom.at(a, (BLOCK * (k + 1)).min(e) as f64, e));
    // 枝のブロックを囲むマスの範囲（1 マス外まで。全体の格子の中に収める）
    let (mut c0, mut c1) = ([i64::MAX; 3], [i64::MIN; 3]);
    for &b in blocks {
        let k = eb(b);
        for a in 0..3 {
            let (lo, hi) = world(a, k[a]);
            c0[a] = c0[a].min(cell(a, lo).floor() as i64 - 1);
            c1[a] = c1[a].max(cell(a, hi).ceil() as i64 + 1);
        }
    }
    for a in 0..3 {
        let (lo, hi) = (edom.at(a, bbox.0[a] as f64 - 1.0, e), edom.at(a, bbox.1[a] as f64 + 1.0, e));
        c0[a] = c0[a].max(cell(a, lo).floor() as i64 - 1);
        c1[a] = c1[a].min(cell(a, hi).ceil() as i64 + 1);
        c0[a] = c0[a].max(-1);
        c1[a] = c1[a].min(pn as i64 + 1).max(c0[a] + 4);
    }
    // 立方体にする（マスの数を、一番長い軸にそろえる）。implicit_solid_with は外側に 1 マスずつ広げるので、その分を引いて渡す
    let total = (0..3).map(|a| c1[a] - c0[a]).max().unwrap_or(4) as usize;
    let inner = total - 2;
    let sub = Domain {
        lo: [0, 1, 2].map(|a| dom.lo[a] + (c0[a] + 1) as f64 * step[a]),
        hi: [0, 1, 2].map(|a| dom.lo[a] + (c0[a] + 1 + inner as i64) as f64 * step[a]),
    };
    // ブロックごとの印（この格子のマスの番号は、全体の番号 − c0）
    let nb = total.div_ceil(BLOCK);
    let mut mask = vec![false; nb * nb * nb];
    for &b in blocks {
        let k = eb(b);
        let span = |a: usize| {
            let (lo, hi) = world(a, k[a]);
            // 立方体にそろえて広がった分（短い軸の、枝の範囲の外）には印を付けない
            let last = (c1[a] - c0[a] - 1).clamp(0, total as i64 - 1);
            let lo = (cell(a, lo).floor() as i64 - 1 - c0[a]).clamp(0, last) as usize;
            let hi = (cell(a, hi).ceil() as i64 + 1 - c0[a]).clamp(0, last) as usize;
            (lo / BLOCK)..=(hi / BLOCK).min(nb - 1)
        };
        for bk in span(2) {
            for bj in span(1) {
                for bi in span(0) {
                    mask[bi + nb * (bj + nb * bk)] = true;
                }
            }
        }
    }
    // 面を作る範囲（立方体にそろえる前の、枝の範囲）
    let tight = Domain {
        lo: [0, 1, 2].map(|a| dom.lo[a] + c0[a] as f64 * step[a]),
        hi: [0, 1, 2].map(|a| dom.lo[a] + c1[a] as f64 * step[a]),
    };
    (sub, inner, mask, tight)
}

/// 印の付いたブロックのうち、区間演算で「面が無い」と分かるものの印を外す。印が残れば true。
/// `dom`・`n`・`mask` は [`piece_grid`] の戻り値。ブロックの箱は、枝の範囲 `tight` に絞ってから調べる
/// （ブロックは枝よりずっと広いことが多く、絞らないと「あるかもしれない」ばかりになる）
fn prune_mask(prog: &Program, dom: Domain, n: usize, tight: Domain, mask: &mut [bool]) -> bool {
    use crate::interval::Interval;
    if !prog.supports_interval() {
        return true;
    }
    let step = [0, 1, 2].map(|a| (dom.hi[a] - dom.lo[a]) / n.max(1) as f64);
    let nb = (n + 2).div_ceil(BLOCK);
    let mut any = false;
    for (b, on) in mask.iter_mut().enumerate().filter(|(_, on)| **on) {
        let k = [b % nb, (b / nb) % nb, b / (nb * nb)];
        let boxes: Vec<Interval> = (0..3)
            .map(|a| {
                // implicit_solid_with は外側に 1 マスずつ広げた格子で作る
                let lo = dom.lo[a] + ((BLOCK * k[a]) as f64 - 1.0) * step[a];
                let hi = dom.lo[a] + ((BLOCK * (k[a] + 1)).min(n + 2) as f64 - 1.0) * step[a];
                let (lo, hi) = (lo.max(tight.lo[a]), hi.min(tight.hi[a]));
                Interval::new(lo.min(hi), lo.max(hi))
            })
            .collect();
        *on = prog.eval_interval(&boxes).may_contain_zero();
        any |= *on;
    }
    any
}

/// 枝の組 1 つ分
struct BranchGroup {
    /// 段差の元ごとの枝の番号
    tau: Vec<f64>,
    /// この枝の格子点があるブロック（とその隣の 1 セル分）
    blocks: Vec<u32>,
    /// この枝の格子点の数（多い順に描く）
    count: usize,
    /// この枝の格子点のある範囲（格子点の番号。両端を含む）
    lo: [u32; 3],
    hi: [u32; 3],
}

/// 式の枝の組を、[`implicit_solid_with`] と同じ格子の点で調べる。格子点の多い順。多すぎる式は None。
///
/// 格子より薄い所（`mod(x y, z)` の z = 0 の近くに無限に重なる層など）は、面を描き分けられないので、ブロックごとに外す
/// （枝まるごとではなく場所ごとに決める。一部だけ混み合う面は、描ける所だけ描く）。
/// 薄いかどうかは、ブロックの中のその枝の格子点のうち「3 つの軸のどれでも、隣（前か後ろ）が同じ枝の点」の割合で決める
/// （厚みが格子の 1 マスより薄い層は、厚みの向きに 2 点並ばない）。外した所があれば、2 つ目の戻り値が true
fn branch_groups(f: &Program, dom: Domain, n: usize) -> Option<(Vec<BranchGroup>, bool)> {
    let (dom, n) = solid_grid(dom, n);
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
    let mut last = NONE;
    for kz in 0..m {
        let zs = vec![dom.at(2, kz as f64, n); m * m];
        let br = branch.eval(&[&xs, &ys, &zs], m * m);
        for l in 0..m * m {
            let b = &br[l * k..(l + 1) * k];
            if b.iter().any(|v| !v.is_finite()) {
                continue;
            }
            // 隣の点と同じ組のことが多いので、直前と同じなら表を引かない
            let id = if last != NONE && taus[last as usize] == b {
                last
            } else {
                let mut key = [0i64; MAX_BRANCH_SITES];
                for (o, &v) in key.iter_mut().zip(b) {
                    *o = v as i64;
                }
                *seen.entry(key).or_insert_with(|| {
                    taus.push(b.to_vec());
                    counts.push(0);
                    (taus.len() - 1) as u32
                })
            };
            last = id;
            counts[id as usize] += 1;
            ids[l + m * m * kz] = id;
        }
        if taus.len() > MAX_TAUS {
            return None;
        }
    }
    // 格子点ごとの「厚みのある所か」: 3 つの軸のどれでも、隣（前か後ろ）が同じ枝の点
    let strides = [1, m, m * m];
    let mut paired: Vec<bool> = vec![false; m * m * m];
    let mut boxes: Vec<([u32; 3], [u32; 3])> = vec![([u32::MAX; 3], [0; 3]); taus.len()];
    for kz in 0..m {
        for jy in 0..m {
            for ix in 0..m {
                let l = ix + m * (jy + m * kz);
                let id = ids[l];
                if id == NONE {
                    continue;
                }
                let q = [ix, jy, kz];
                let bx = &mut boxes[id as usize];
                for a in 0..3 {
                    bx.0[a] = bx.0[a].min(q[a] as u32);
                    bx.1[a] = bx.1[a].max(q[a] as u32);
                }
                paired[l] = (0..3).all(|a| (q[a] > 0 && ids[l - strides[a]] == id) || (q[a] + 1 < m && ids[l + strides[a]] == id));
            }
        }
    }
    // 枝ごとの、その枝を描くブロック: その枝の格子点があり（ブロックのセルの角と、その外側の 1 点までを見る）、
    // そのうち厚みのある所の点が 3 割以上のブロック。薄い所（面が格子より細かく重なる所）は、ブロックごとに外す
    let mut blocks: Vec<Vec<u32>> = vec![Vec::new(); taus.len()];
    let mut seen_in: Vec<u32> = vec![0; taus.len()];
    let mut cnt: Vec<(u32, u32)> = vec![(0, 0); taus.len()];
    let mut here: Vec<u32> = Vec::new();
    let mut thin = false;
    for bk in 0..nb {
        for bj in 0..nb {
            for bi in 0..nb {
                let bid = (bi + nb * (bj + nb * bk)) as u32;
                let span = |b: usize| (BLOCK * b).saturating_sub(1)..=(BLOCK * (b + 1) + 1).min(m - 1);
                here.clear();
                for kz in span(bk) {
                    for jy in span(bj) {
                        let row = m * (jy + m * kz);
                        for ix in span(bi) {
                            let id = ids[ix + row];
                            if id == NONE {
                                continue;
                            }
                            if seen_in[id as usize] != bid + 1 {
                                seen_in[id as usize] = bid + 1;
                                cnt[id as usize] = (0, 0);
                                here.push(id);
                            }
                            cnt[id as usize].0 += 1;
                            cnt[id as usize].1 += paired[ix + row] as u32;
                        }
                    }
                }
                // 格子の端のブロック（表示範囲の端で切れて薄くなっただけの所）は、「描けない部分」に数えない
                let edge = [bi, bj, bk].iter().any(|&b| b == 0 || b + 1 == nb);
                for &id in &here {
                    let (all, solid) = cnt[id as usize];
                    if 10 * solid >= 3 * all {
                        blocks[id as usize].push(bid);
                    } else {
                        thin |= !edge;
                    }
                }
            }
        }
    }
    let mut out: Vec<BranchGroup> = Vec::new();
    for (id, (tau, blocks)) in taus.into_iter().zip(blocks).enumerate() {
        if !blocks.is_empty() {
            out.push(BranchGroup { tau, blocks, count: counts[id], lo: boxes[id].0, hi: boxes[id].1 });
        }
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

/// 点を面 `surf` = 0 の上に寄せる（ニュートン法で 2 回）。大きく動く点・計算できない点は元のまま。
/// `normals`（点ごとの面の法線）があれば、その向きにだけ動かす（1 回につき式の計算が 2 回で済む。
/// 面の上のどの点に寄せてもよいので、向きは面を横切っていれば足りる）。無ければ、勾配を差分で求めて、その向きに動かす
fn project_to_surface(surf: &Program, pts: &[[f64; 3]], normals: Option<&[f32]>, size: f64) -> Vec<[f64; 3]> {
    let h = 1e-6 * size;
    let limit = 0.02 * size;
    let mut cur = pts.to_vec();
    for _ in 0..2 {
        let f0 = eval_at(surf, &cur);
        let shift = |a: usize, s: f64| -> Vec<[f64; 3]> {
            cur.iter()
                .map(|p| {
                    let mut q = *p;
                    q[a] += s;
                    q
                })
                .collect()
        };
        let steps: Vec<[f64; 3]> = if let Some(nrm) = normals {
            let dir = |k: usize| [0, 1, 2].map(|a| nrm[3 * k + a] as f64);
            let ahead: Vec<[f64; 3]> = cur.iter().enumerate().map(|(k, p)| [0, 1, 2].map(|a| p[a] + h * dir(k)[a])).collect();
            let f1 = eval_at(surf, &ahead);
            f0.iter().zip(&f1).enumerate().map(|(k, (f, f1))| dir(k).map(|d| f * h / (f1 - f) * d)).collect()
        } else {
            let mut grad = vec![[0.0f64; 3]; cur.len()];
            for a in 0..3 {
                let (fp, fm) = (eval_at(surf, &shift(a, h)), eval_at(surf, &shift(a, -h)));
                for (g, (p, m)) in grad.iter_mut().zip(fp.iter().zip(&fm)) {
                    g[a] = (p - m) / (2.0 * h);
                }
            }
            f0.iter().zip(&grad).map(|(f, g)| g.map(|c| f * c / (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]))).collect()
        };
        for (p, step) in cur.iter_mut().zip(&steps) {
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
/// - `nonzero`: 全部の式の値が 0 でない所だけ残す（通分した式の分母。分子と分母が同時に 0 になる面は、元の式の零点ではない）
///
/// 条件の境目が面そのものに重なること（`mod(x y, z) = 0` の枝 τ の面 x y = τ z は、枝の境目 x y / z = τ の上）がある。
/// メッシュの頂点は面から少しずれているので、そのまま条件を調べると、境目のどちら側に出るかが点ごとにばらつく。
/// そこで頂点を面の上に寄せてから調べ、境目からのずれが丸め誤差ほどなら、境目の上にあるとみなす
/// （境目を含む条件なら成り立ち、含まない条件なら成り立たない）
fn clip_piece(mesh: &mut Mesh, surf: &Program, conds: &[Cond], defined: &[&Program], nonzero: &[&Program], size: f64) {
    let count = mesh.positions.len() / 3;
    if count == 0 || (conds.is_empty() && defined.is_empty() && nonzero.is_empty()) {
        return;
    }
    let pts: Vec<[f64; 3]> = mesh.positions.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect();
    let mut inside = vec![true; count];
    // 境目が面に重なっていない条件（辺の上で境目を探すときに調べる）
    let mut live: Vec<&Cond> = Vec::new();
    // 0 とみなす大きさ（式ごと。面の上の値のいちばん大きいものの 10⁻⁷ 倍）
    let mut zero_tol: Vec<f64> = Vec::new();
    if !conds.is_empty() || !nonzero.is_empty() {
        let normals = (mesh.normals.len() == mesh.positions.len()).then_some(mesh.normals.as_slice());
        let on = project_to_surface(surf, &pts, normals, size);
        for d in nonzero {
            let v = eval_at(d, &on);
            let tol = 1e-7 * v.iter().filter(|v| v.is_finite()).fold(0.0f64, |m, v| m.max(v.abs()));
            for (k, v) in v.iter().enumerate() {
                inside[k] &= !(v.abs() <= tol);
            }
            zero_tol.push(tol);
        }
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
        for (d, tol) in nonzero.iter().zip(&zero_tol) {
            for (o, v) in ok.iter_mut().zip(eval_at(d, q)) {
                *o &= !(v.abs() <= *tol);
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
    for _ in 0..16 {
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
        let cells: crate::adaptive::HashSet<[i64; 3]> =
            m.positions.chunks_exact(3).map(|p| [0, 1, 2].map(|a| (p[a] as f64 / h).floor() as i64)).collect();
        move |q: [f64; 3]| {
            let c = q.map(|v| (v / h).floor() as i64);
            (-1..=1).any(|a| (-1..=1).any(|b| (-1..=1).any(|e| cells.contains(&[c[0] + a, c[1] + b, c[2] + e]))))
        }
    }

    /// 頂点（位置でまとめる）でつながった成分ごとの三角形の数
    fn component_sizes(m: &Mesh) -> Vec<usize> {
        let mut ids: crate::adaptive::HashMap<[i64; 3], usize> = Default::default();
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
        let mut sizes: crate::adaptive::HashMap<usize, usize> = Default::default();
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
    fn poles_are_removed_by_clearing_denominators() {
        // tan(x) = z y は、通分した sin(x) − z y cos(x) = 0 を描く。極 cos(x) = 0 の平面に面を張らず、
        // 極のすぐ隣を走る面も欠けない。粗い格子・軽い計算（回転の再生中）でも同じ
        let d = Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
        for (n, refine) in [(80, Refine::Normal), (40, Refine::Light), (24, Refine::Light)] {
            let m = implicit_solid_opt(&prog("tan(x) - z y"), d, n, &[], 0, refine);
            let h = 20.0 / n as f64;
            // 面から離れた頂点が無い（面までの距離を、分子の値 ÷ 勾配の大きさで見積もる）
            for p in m.positions.chunks_exact(3) {
                let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
                let f = x.sin() - z * y * x.cos();
                let g = [x.cos() + z * y * x.sin(), -z * x.cos(), -y * x.cos()];
                let dist = f.abs() / (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
                assert!(dist < 0.6 * h, "細かさ {n}: 面から {dist} 離れた頂点 ({x}, {y}, {z})");
            }
            // 面の欠けが無い: z = tan(x) / y の上の点（表示範囲の端から 1 マスより内側）
            let near = near_fn(&m, h);
            let (mut hit, mut all) = (0, 0);
            for i in 0..=400 {
                for j in 0..=80 {
                    let (x, y) = (-9.0 + 0.045 * i as f64, -9.0 + 0.225 * j as f64);
                    let z = x.tan() / y;
                    if z.abs() < 6.0 - h && y.abs() > 1e-9 {
                        all += 1;
                        hit += near([x, y, z]) as usize;
                    }
                }
            }
            assert_eq!(hit, all, "細かさ {n}: 面の欠け");
        }
        // 分子と分母が同時に 0 になる所（sin(x)/x の x = 0）は極ではない。通分して x = 0 の余計な平面を作らない
        let m = implicit_solid_opt(&prog("sin(x)/x - z"), d, 80, &[], 0, Refine::Normal);
        assert!(!m.positions.is_empty());
        for p in m.positions.chunks_exact(3) {
            let (x, z) = (p[0] as f64, p[2] as f64);
            let want = if x == 0.0 { 1.0 } else { x.sin() / x };
            assert!((z - want).abs() < 0.2, "余計な面の頂点 ({x}, {z})");
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
        // 小さな破片（手裏剣のような切れ端）が無い。表示範囲の端にかかるもの（端で切れた面の一部）は数えない
        let inner = Mesh {
            positions: m.positions.clone(),
            indices: m
                .indices
                .chunks_exact(3)
                .filter(|t| {
                    t.iter().all(|&i| {
                        let q = &m.positions[i as usize * 3..i as usize * 3 + 3];
                        (0..3).all(|a| (q[a] as f64) > d.lo[a] + 0.5 && (q[a] as f64) < d.hi[a] - 0.5)
                    })
                })
                .flatten()
                .copied()
                .collect(),
            ..Default::default()
        };
        let small = component_sizes(&inner).into_iter().filter(|&s| s <= 50).count();
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


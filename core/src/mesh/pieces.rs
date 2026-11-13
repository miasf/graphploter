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
/// 切り取りの境目が、辺の上で成り立つ側の頂点からこの割合より近ければ、その頂点を境目にする
const CUT_SNAP: f64 = 1e-4;
/// 枝を調べる格子点の数の上限
const MAX_BRANCH_POINTS: usize = 40_000_000;

thread_local! {
    /// 次の [`implicit_solid_with`] で、面を作るブロックを絞る（1 回だけ使う）
    pub(super) static BLOCK_MASK: std::cell::RefCell<Option<Vec<bool>>> = const { std::cell::RefCell::new(None) };
    /// 立てている間、枝を調べる範囲を、範囲（立方体）に内接する球の中だけにする（[`in_inscribed_ball`]）
    static BALL_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// 回す前の形（[`LocalMesh`]）を作るとき: `f` の中で作る陰関数の段差の枝を、範囲（回転の中心を中心とする立方体）に内接する球の
/// 中だけで調べる。どの向きに回しても、表示範囲はこの球の中に入る（立方体の角は画面に出ない）。
/// 角まで調べると、枝の数（`mod(x, 2) + mod(y, 2) + mod(z, 2) = 2` で約 4,000）が部品の数の上限を超え、表示範囲の中の枝まで描かなくなる
pub fn in_inscribed_ball<T>(f: impl FnOnce() -> T) -> T {
    BALL_ONLY.with(|b| b.set(true));
    let out = f();
    BALL_ONLY.with(|b| b.set(false));
    out
}

/// 描く部品の数の上限（多い順に描く）
const MAX_PIECES: usize = 3000;
/// 枝を調べる格子の細かさの下限。どの枝を描くかは、面を作る格子の細かさ（再生・ドラッグ中は粗くなる）ではなく、
/// この細かさで決める。粗い格子で調べると、粗くしたときに枝ごと消える（止めたときと形が変わる）
const ENUM_N: usize = 80;
/// 枝の部品の三角形の数の目安。超えそうなら、枝の部品を作る格子を粗くする（面の数は減らさない）
const TARGET_PIECE_TRIANGLES: f64 = 600_000.0;
/// 薄いが面が通るので描くブロック（[`BranchGroup::extra`]）の部品の、三角形の数の目安。超えそうなら、その部品の格子を粗くする。
/// 面が格子とほぼ同じ間隔で重なる密な層（mod(x y z, 4) = 2 など）では、こうしたブロックが何千もあり、元の部品と同じ細かさで
/// 描くと三角形が 2〜4 倍になる
const TARGET_EXTRA_TRIANGLES: f64 = 200_000.0;
/// [`BranchGroup::extra`] の部品の三角形の数の上限（見積もりが外れても、これを超えたら残りは描かない。格子点の多い枝から描く）
const MAX_EXTRA_TRIANGLES: usize = 300_000;
/// 枝の部品を作る格子の細かさの下限
const MIN_PIECE_N: usize = 24;
/// 陽関数として描く薄い枝（[`thin_graph`]）の三角形の数の上限（厚い枝を描いた後に、残りの分だけ描く）
const MAX_THIN_TRIANGLES: usize = 400_000;

/// 部品に分けて描く。分けられない（分けても変わらない）式は None
pub(super) fn implicit_by_pieces(
    prog: &Program,
    dom: Domain,
    n: usize,
    refine: Refine,
) -> Option<Mesh> {
    // 極のある式は、通分した分子 N = 0 を描く（N に極は無い）。分母 D が 0 の所は、後で切り落とす
    let rational = prog
        .rationalized()
        .filter(|(num, den)| poles_worth_removing(num, den, dom));
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
            let (g, _) = branch_groups(f, dom, n.max(ENUM_N))?;
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
    let size = (0..3)
        .map(|a| (dom.hi[a] - dom.lo[a]).powi(2))
        .sum::<f64>()
        .sqrt();
    let mut parts: Vec<Mesh> = Vec::new();
    let mut budget = MAX_PIECES;
    let mut thin_jobs: Vec<(&Program, Vec<BranchGroup>, Vec<Option<LinearBranch>>)> = Vec::new();
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
        // 面の数が多い式は、三角形が増えすぎないよう、枝の部品を作る格子を粗くする（1 ブロックあたり約 80 個）。
        // 描くのは 2 回に分ける。1 回目は、厚みのある所のブロックがある枝（今までどおりの細かさと順番。薄いが面が通るので描く
        // ブロックも同じ部品に入れる。部品の格子の線は全体の格子と同じなので、今まで描いていた所の面は変わらない）。
        // 2 回目は、薄いが面が通るブロックしか無い枝（floor(x y) = z floor(z) の細い帯など）で、別の見積もりで細かさを決め、
        // 三角形の数にも上限を設ける（1 回目の格子や順番を変えないので、今まで描いていた面は、三角形の上限で落ちることもない）。
        // 同じ枝を 2 つの部品に分けないのは、部品どうしがブロックの境目のあたりで重なり、同じ面を 2 回描いてしまうから
        let en = n.max(ENUM_N);
        let piece_n = |blocks: usize, target: f64| {
            let est = 80.0 * blocks as f64 * (n as f64 / en as f64).powi(2);
            if est > target {
                ((n as f64 * (target / est).sqrt()) as usize)
                    .max(MIN_PIECE_N)
                    .min(n)
            } else {
                n
            }
        };
        // 格子より薄い枝（薄いので外したブロックがあるか、薄いが面が通るブロックしか無い枝）のうち、部品がどれも、ある座標 v に
        // ついて 1 次で、枝の範囲で v の係数が 0 にならない（陰関数定理の条件 ∂F/∂v ≠ 0 が範囲全体で成り立つ）枝は、
        // 面が v = g の 1 枚の陽関数なので、格子によらず描ける（thin_graph）。下の 2 回では描かず、厚い枝を描いた後に描く
        let (edom, _) = solid_grid(dom, en);
        let linear: Vec<Option<LinearBranch>> = group
            .iter()
            .map(|g| {
                // 今まで描いていなかった枝（どのブロックも薄くて外した枝）だけ。格子で描ける所のある枝は今までどおり
                // （その所まで陽関数の三角形の上限に回すと、細かさ 200 で上限に届いて、今まで描けていた所が欠ける）。
                // こうすると、今まで描いていた面は変わらず、足すだけになる
                if !g.blocks.is_empty() || !g.extra.is_empty() {
                    return None;
                }
                let bbox = Domain {
                    lo: [0, 1, 2].map(|a| edom.at(a, g.lo[a] as f64 - 1.0, en + 2).max(dom.lo[a])),
                    hi: [0, 1, 2].map(|a| edom.at(a, g.hi[a] as f64 + 1.0, en + 2).min(dom.hi[a])),
                };
                let fz = f.frozen(&g.tau);
                let (subs, sub_changed) = fz.zero_factors();
                if sub_changed {
                    return None;
                }
                let solved: Option<Vec<Solved>> = subs
                    .iter()
                    .filter(|sub| !sub.is_constant())
                    .map(|sub| solve_linear(sub, bbox))
                    .collect();
                solved.filter(|s| !s.is_empty()).map(|s| (bbox, s))
            })
            .collect();
        let done: Vec<bool> = linear.iter().map(|l| l.is_some()).collect();
        truncated |= group.iter().zip(&done).any(|(g, &d)| g.dropped && !d);
        let pns = [
            piece_n(
                group
                    .iter()
                    .zip(&done)
                    .filter(|(_, &d)| !d)
                    .map(|(g, _)| g.blocks.len())
                    .sum(),
                TARGET_PIECE_TRIANGLES,
            ),
            piece_n(
                group
                    .iter()
                    .zip(&done)
                    .filter(|(g, &d)| !d && g.blocks.is_empty())
                    .map(|(g, _)| g.extra.len())
                    .sum(),
                TARGET_EXTRA_TRIANGLES,
            ),
        ];
        'pass: for (pass, &pn) in pns.iter().enumerate() {
            let first = parts.iter().map(|m| m.indices.len() / 3).sum::<usize>();
            for (g, &d) in group.iter().zip(&done) {
                if d || (pass == 0) == g.blocks.is_empty()
                    || (g.blocks.is_empty() && g.extra.is_empty())
                {
                    continue;
                }
                let mut blocks = g.blocks.clone();
                blocks.extend_from_slice(&g.extra);
                blocks.sort_unstable();
                // 三角形が多すぎる（画面が固まる）ときも、残りの部品は描かない
                let total = parts.iter().map(|m| m.indices.len() / 3).sum::<usize>();
                if budget == 0
                    || total > MAX_IMPLICIT_TRIANGLES
                    || (pass == 1 && total - first > MAX_EXTRA_TRIANGLES)
                {
                    truncated = true;
                    break 'pass;
                }
                budget -= 1;
                let fz = f.frozen(&g.tau);
                let conds = f.branch_conds(&g.tau);
                let (subs, sub_changed) = fz.zero_factors();
                let relaxed = f.frozen_relaxed(&g.tau);
                let may_zero = |lo: [f64; 3], hi: [f64; 3]| {
                    relaxed.may_contain_zero(
                        &[0, 1, 2].map(|a| crate::interval::Interval::new(lo[a], hi[a])),
                    )
                };
                let Some((sub_dom, sub_n, mask, tight)) =
                    piece_grid(&blocks, (g.lo, g.hi), dom, en, pn, &may_zero)
                else {
                    continue;
                };
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
        if linear.iter().any(|l| l.is_some()) {
            thin_jobs.push((f, group, linear));
        }
    }
    // 陽関数として描く薄い枝（厚い枝を全部描いた後に、三角形の残りの分だけ）
    let start = parts.iter().map(|m| m.indices.len() / 3).sum::<usize>();
    let mut total = start;
    'thin: for (f, group, linear) in thin_jobs {
        for (g, l) in group.iter().zip(linear) {
            let Some((bbox, solved)) = l else { continue };
            if total > MAX_IMPLICIT_TRIANGLES || total - start > MAX_THIN_TRIANGLES {
                truncated = true;
                break 'thin;
            }
            let conds = f.branch_conds(&g.tau);
            let mut defined: Vec<&Program> = Vec::new();
            if changed {
                defined.push(prog);
            }
            // 枝のある列: 枝のブロック（厚い・薄い・面が通る）を、陽関数の変数 2 つの平面に写して 1 ブロック広げたもの
            let (edom, e) = solid_grid(dom, n.max(ENUM_N));
            let enb = e.div_ceil(BLOCK);
            let step = [0, 1, 2].map(|a| (edom.hi[a] - edom.lo[a]) / e as f64);
            for sv in &solved {
                let (ua, va) = others(sv.axis);
                let mut cols: crate::adaptive::HashSet<(i64, i64)> = Default::default();
                for &b in g.blocks.iter().chain(&g.extra).chain(&g.thin) {
                    let k = [
                        b as usize % enb,
                        b as usize / enb % enb,
                        b as usize / (enb * enb),
                    ];
                    for d in 0..9 {
                        cols.insert((k[ua] as i64 + d % 3 - 1, k[va] as i64 + d / 3 - 1));
                    }
                }
                let block_of =
                    |a: usize, c: f64| ((c - edom.lo[a]) / step[a]).round() as i64 / BLOCK as i64;
                let column = |u: f64, v: f64| cols.contains(&(block_of(ua, u), block_of(va, v)));
                let mut m = thin_graph(sv, &conds, bbox, dom, n, &column);
                if m.indices.is_empty() {
                    continue;
                }
                clip_piece_with(&mut m, None, &conds, &defined, &nonzero, size);
                drop_slivers(&mut m, (1e-5 * size) as f32);
                drop_small_components(&mut m, 12);
                total += m.indices.len() / 3;
                if !m.indices.is_empty() {
                    parts.push(m);
                }
            }
        }
    }
    let note = parts
        .iter()
        .map(|m| m.note.as_str())
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_string();
    let mut out = merge(parts, false);
    out.note = if truncated {
        "（細かすぎて描けない部分あり）".into()
    } else {
        note
    };
    Some(out)
}

/// 陽関数として描く薄い枝の、範囲と部品ごとの解いた式
type LinearBranch = (Domain, Vec<Solved>);

/// 部品 `sub` = 0 を座標 `axis` について解いた陽関数 v = g（[`solve_linear`]）
struct Solved {
    axis: usize,
    /// 残り 2 つの座標（軸の順）の式
    g: Program,
    /// v の係数 a（残り 2 つの座標の式）
    a: Program,
    /// a の符号（F の勾配の v の成分の符号。面の表の向きをそろえる）。範囲の中で符号が変わるかもしれないなら 0
    /// （そのときは格子のマスごとに、4 つの角の a の符号がそろうマスだけを描く）
    sign: f64,
}

/// 部品 `sub` = 0 を、どれかの座標 v について解いた陽関数 v = g にできるなら、その式。
/// `sub` が v について 1 次（a v + b）なら、a ≠ 0 の所では陰関数定理の条件 ∂F/∂v = a ≠ 0 が成り立ち、面は v = −b/a の 1 枚。
/// 範囲 `bbox` の中で a が 0 にならない（区間演算で確かめる）座標を先に選ぶ（縦 z から）。どの座標でも a が 0 になるかもしれなければ、
/// 最初に 1 次になった座標で、a ≠ 0 のマスだけを描く（mod(x y z, 4) = 2 の枝 x y z − 4τ − 2 は z について 1 次で、a = x y は
/// x = 0・y = 0 でだけ 0）
fn solve_linear(sub: &Program, bbox: Domain) -> Option<Solved> {
    use crate::interval::{Interval, Range};
    let mut varying: Option<Solved> = None;
    for v in [2, 0, 1] {
        let Some((g, a)) = sub.solved_for(v) else {
            continue;
        };
        let (ua, va) = others(v);
        let boxes = [ua, va].map(|k| Interval::new(bbox.lo[k], bbox.hi[k]));
        let sign = match a.eval_interval(&boxes) {
            Range::Known(i) if i.lo > 0.0 => 1.0,
            Range::Known(i) if i.hi < 0.0 => -1.0,
            _ => 0.0,
        };
        if sign != 0.0 {
            return Some(Solved {
                axis: v,
                g,
                a,
                sign,
            });
        }
        if varying.is_none() && !a.is_constant() {
            varying = Some(Solved {
                axis: v,
                g,
                a,
                sign,
            });
        }
    }
    varying
}

/// 薄い枝の陽関数の面 v = g を、範囲 `bbox` の中で、枝の条件 `conds` が成り立つかもしれないマスだけ三角形にする。
/// マスの大きさは、表示範囲を `n` 等分した格子と同じ。範囲の外は切り落とす（枝の境目での切り取りは、この後の clip_piece）。
/// `column(u, v)` は、その点の列に枝があるかもしれないか（無い列のマスは、式を計算しない）。
///
/// 枝が面に 1 つも無いことが多い（mod(x^2 + y^2, z) = 1 の細かさ 200 で、4,000 個あまりの枝が空）ので、まず 4 マスずつの
/// 粗い格子で調べ、枝があるかもしれない粗いマス（とその隣）だけを、細かい格子で調べる
fn thin_graph(
    sv: &Solved,
    conds: &[Cond],
    bbox: Domain,
    dom: Domain,
    n: usize,
    column: &dyn Fn(f64, f64) -> bool,
) -> Mesh {
    const K: usize = 4;
    let axis = sv.axis;
    let (ua, va) = others(axis);
    let cells = [ua, va].map(|a| {
        (((bbox.hi[a] - bbox.lo[a]) / (dom.hi[a] - dom.lo[a]) * n as f64).ceil() as usize).max(1)
    });
    let mu = cells[0] + 1;
    let at = |a: usize, i: usize, c: usize| {
        bbox.lo[a] + (bbox.hi[a] - bbox.lo[a]) * i.min(c) as f64 / c as f64
    };
    let uv = |l: usize| (at(ua, l % mu, cells[0]), at(va, l / mu, cells[1]));
    let near = |v: f64, b: f64| b.is_finite() && (v - b).abs() <= 1e-6 * (1.0 + b.abs());
    let on_open_edge = |c: &Cond, s: &[f64]| {
        let finite: Vec<f64> = s.iter().copied().filter(|v| v.is_finite()).collect();
        let mostly = |b: f64| {
            !finite.is_empty()
                && 10 * finite.iter().filter(|&&v| near(v, b)).count() >= 9 * finite.len()
        };
        (mostly(c.lo) && !c.lo_closed) || (mostly(c.hi) && !c.hi_closed)
    };
    // 格子点（番号の列）で v = g と、条件ごとの外の側（-1: 下、1: 上、0: 中か境目の上。v が範囲から外れる点も外）を計算する。
    // 面が、枝に含まれない境目に重なる枝（z = mod(x y, z) の枝は全部そう。面は 1 つも残らない）なら None
    let eval_nodes = |nodes: &[usize]| -> Option<(Vec<f64>, Vec<Vec<i8>>)> {
        let (us, vs): (Vec<f64>, Vec<f64>) = nodes.iter().map(|&l| uv(l)).unzip();
        let mut out = vec![C::new(0.0, 0.0); nodes.len()];
        sv.g.eval_batch(&[&us, &vs], &mut out);
        let h: Vec<f64> = out.iter().map(|&c| as_real(c)).collect();
        let pts: Vec<[f64; 3]> = (0..nodes.len())
            .map(|k| {
                let mut p = [0.0; 3];
                p[ua] = us[k];
                p[va] = vs[k];
                p[axis] = h[k];
                p
            })
            .collect();
        let mut sides = Vec::with_capacity(conds.len() + 1);
        for c in conds {
            let s = eval_at(&c.prog, &pts);
            if on_open_edge(c, &s) {
                return None;
            }
            let tol = |b: f64| 1e-5 * (1.0 + b.abs());
            sides.push(
                s.into_iter()
                    .map(|v| {
                        if v < c.lo - tol(c.lo) {
                            -1
                        } else if v > c.hi + tol(c.hi) {
                            1
                        } else {
                            0
                        }
                    })
                    .collect(),
            );
        }
        sides.push(
            h.iter()
                .map(|&v| {
                    if v < bbox.lo[axis] {
                        -1
                    } else if v > bbox.hi[axis] {
                        1
                    } else {
                        0
                    }
                })
                .collect(),
        );
        Some((h, sides))
    };
    // 角の番号（`at` の位置）で、マスに枝があるかもしれないか: 角がどれも有限で、どの条件でも角が全部同じ側の外ではない
    let may_have =
        |c: [usize; 4], h: &dyn Fn(usize) -> f64, side: &dyn Fn(usize, usize) -> i8, nc: usize| {
            c.iter().all(|&k| h(k).is_finite())
                && !(0..nc)
                    .any(|q| side(q, c[0]) != 0 && c.iter().all(|&k| side(q, k) == side(q, c[0])))
        };
    let nc = conds.len() + 1;

    // 1. 粗い格子（K マスずつ）
    let cc = [cells[0].div_ceil(K), cells[1].div_ceil(K)];
    let coarse_node = |i: usize, j: usize| (i * K).min(cells[0]) + mu * (j * K).min(cells[1]);
    let mut cnodes: Vec<usize> = Vec::new();
    let mut cid: crate::adaptive::HashMap<usize, usize> = Default::default();
    let mut ccells: Vec<(usize, usize)> = Vec::new();
    for j in 0..cc[1] {
        for i in 0..cc[0] {
            let (u0, v0) = uv(coarse_node(i, j));
            let (u1, v1) = uv(coarse_node(i + 1, j + 1));
            // 粗いマスの中のどこかの列に枝があるかもしれない（角と中心で調べる）
            let any = [
                (u0, v0),
                (u1, v0),
                (u0, v1),
                (u1, v1),
                (0.5 * (u0 + u1), 0.5 * (v0 + v1)),
            ]
            .iter()
            .any(|&(u, v)| column(u, v));
            if !any {
                continue;
            }
            ccells.push((i, j));
            for l in [
                coarse_node(i, j),
                coarse_node(i + 1, j),
                coarse_node(i, j + 1),
                coarse_node(i + 1, j + 1),
            ] {
                cid.entry(l).or_insert_with(|| {
                    cnodes.push(l);
                    cnodes.len() - 1
                });
            }
        }
    }
    if cnodes.is_empty() {
        return Mesh::default();
    }
    let Some((ch, cs)) = eval_nodes(&cnodes) else {
        return Mesh::default();
    };
    let mut live = vec![false; cc[0] * cc[1]];
    for &(i, j) in &ccells {
        let c = [
            coarse_node(i, j),
            coarse_node(i + 1, j),
            coarse_node(i, j + 1),
            coarse_node(i + 1, j + 1),
        ];
        let h = |l: usize| ch[cid[&l]];
        let side = |q: usize, l: usize| cs[q][cid[&l]];
        // 粗いマスの角が有限でない（極など）マスも、細かく調べる
        let finite = c.iter().all(|&l| h(l).is_finite());
        if !finite || may_have(c, &h, &side, nc) {
            live[i + cc[0] * j] = true;
        }
    }
    // 粗いマスの角の間で枝にかかるかもしれないので、隣の粗いマスまで細かく調べる
    let mut fine_cells: Vec<(usize, usize)> = Vec::new();
    let mut take = vec![false; cc[0] * cc[1]];
    for j in 0..cc[1] {
        for i in 0..cc[0] {
            if !live[i + cc[0] * j] {
                continue;
            }
            for d in 0..9 {
                let (x, y) = (i as i64 + d % 3 - 1, j as i64 + d / 3 - 1);
                if x >= 0 && y >= 0 && (x as usize) < cc[0] && (y as usize) < cc[1] {
                    take[x as usize + cc[0] * y as usize] = true;
                }
            }
        }
    }
    for j in 0..cc[1] {
        for i in 0..cc[0] {
            if !take[i + cc[0] * j] {
                continue;
            }
            for fj in j * K..((j + 1) * K).min(cells[1]) {
                for fi in i * K..((i + 1) * K).min(cells[0]) {
                    let l = fi + mu * fj;
                    let (u0, v0) = uv(l);
                    let (u1, v1) = uv(l + 1 + mu);
                    if column(0.5 * (u0 + u1), 0.5 * (v0 + v1)) {
                        fine_cells.push((fi, fj));
                    }
                }
            }
        }
    }
    if fine_cells.is_empty() {
        return Mesh::default();
    }
    // 2. 細かい格子（そのマスの角だけ）
    let corners = |i: usize, j: usize| {
        [
            i + mu * j,
            i + 1 + mu * j,
            i + mu * (j + 1),
            i + 1 + mu * (j + 1),
        ]
    };
    let mut nodes: Vec<usize> = Vec::new();
    let mut nid: crate::adaptive::HashMap<usize, usize> = Default::default();
    for &(i, j) in &fine_cells {
        for l in corners(i, j) {
            nid.entry(l).or_insert_with(|| {
                nodes.push(l);
                nodes.len() - 1
            });
        }
    }
    let Some((fh, fs)) = eval_nodes(&nodes) else {
        return Mesh::default();
    };
    let h = |l: usize| fh[nid[&l]];
    let side = |q: usize, l: usize| fs[q][nid[&l]];
    // a の符号が範囲の中で変わるかもしれないなら、格子点ごとの a の符号（0 に近すぎる点は 0）
    let asign: Vec<f64> = if sv.sign != 0.0 {
        Vec::new()
    } else {
        let (us, vs): (Vec<f64>, Vec<f64>) = nodes.iter().map(|&l| uv(l)).unzip();
        let mut out = vec![C::new(0.0, 0.0); nodes.len()];
        sv.a.eval_batch(&[&us, &vs], &mut out);
        let vals: Vec<f64> = out.iter().map(|&c| as_real(c)).collect();
        let big = vals
            .iter()
            .filter(|v| v.is_finite())
            .fold(0.0f64, |m, v| m.max(v.abs()));
        vals.iter()
            .map(|&v| {
                if v.is_finite() && v.abs() > 1e-9 * big {
                    v.signum()
                } else {
                    0.0
                }
            })
            .collect()
    };
    // マスの a の符号（4 つの角でそろわなければ 0 で、そのマスは描かない。面がそこで v の向きに立つ・極になる）
    let cell_sign = |c: [usize; 4]| -> f64 {
        if sv.sign != 0.0 {
            return sv.sign;
        }
        let s0 = asign[nid[&c[0]]];
        if s0 != 0.0 && c.iter().all(|&l| asign[nid[&l]] == s0) {
            s0
        } else {
            0.0
        }
    };
    let point = |l: usize| {
        let (u, v) = uv(l);
        let mut p = [0.0; 3];
        p[ua] = u;
        p[va] = v;
        p[axis] = h(l);
        p
    };
    let du = (bbox.hi[ua] - bbox.lo[ua]) / cells[0] as f64;
    let dv = (bbox.hi[va] - bbox.lo[va]) / cells[1] as f64;
    let mut mesh = Mesh::default();
    let mut index: crate::adaptive::HashMap<usize, u32> = Default::default();
    // 法線: 面 F = a (v − g) = 0 の勾配の向き sign(a) (e_v − ∇g)。∇g は、そのマスの角の差分
    let mut vert = |mesh: &mut Mesh, l: usize, gu: f64, gv: f64, sign: f64| -> u32 {
        *index.entry(l).or_insert_with(|| {
            let mut nr = [0.0; 3];
            nr[ua] = -gu * sign;
            nr[va] = -gv * sign;
            nr[axis] = sign;
            let len = (nr[0] * nr[0] + nr[1] * nr[1] + nr[2] * nr[2]).sqrt();
            let k = mesh.push_vertex(point(l));
            mesh.normals.extend(nr.map(|c| (c / len) as f32));
            k
        })
    };
    // 三角形の向き: (ua, va, axis) が右手系なら、u→v の向きに回すと +axis の向き
    let right = ((ua + 1) % 3 == va) as i8 * 2 - 1;
    for &(i, j) in &fine_cells {
        let c = corners(i, j);
        let sign = cell_sign(c);
        if sign == 0.0 || !may_have(c, &h, &side, nc) {
            continue;
        }
        let flip = right as f64 * sign < 0.0;
        let gu = 0.5 * ((h(c[1]) - h(c[0])) + (h(c[3]) - h(c[2]))) / du;
        let gv = 0.5 * ((h(c[2]) - h(c[0])) + (h(c[3]) - h(c[1]))) / dv;
        let k = c.map(|l| vert(&mut mesh, l, gu, gv, sign));
        for t in [[k[0], k[1], k[3]], [k[0], k[3], k[2]]] {
            mesh.indices
                .extend_from_slice(&if flip { [t[0], t[2], t[1]] } else { t });
        }
    }
    let inside = Domain {
        lo: [0, 1, 2].map(|a| bbox.lo[a].max(dom.lo[a])),
        hi: [0, 1, 2].map(|a| bbox.hi[a].min(dom.hi[a])),
    };
    clip_to_box(&mesh, inside)
}

/// メッシュを箱 `bx` の中だけにする（三角形を箱の 6 つの面で切る。法線は辺の上で直線で補間する）
fn clip_to_box(m: &Mesh, bx: Domain) -> Mesh {
    let has_n = m.normals.len() == m.positions.len();
    let vert = |k: u32| -> [f64; 6] {
        let k = k as usize * 3;
        let n = if has_n {
            [m.normals[k], m.normals[k + 1], m.normals[k + 2]]
        } else {
            [0.0; 3]
        };
        [
            m.positions[k],
            m.positions[k + 1],
            m.positions[k + 2],
            n[0],
            n[1],
            n[2],
        ]
        .map(|v| v as f64)
    };
    let tol = 1e-5
        * (0..3)
            .map(|a| (bx.hi[a] - bx.lo[a]).powi(2))
            .sum::<f64>()
            .sqrt();
    let mut out = Mesh::default();
    let mut map: crate::adaptive::HashMap<u32, u32> = Default::default();
    let push = |out: &mut Mesh, p: [f64; 6]| -> u32 {
        let k = out.push_vertex([p[0], p[1], p[2]]);
        out.normals.extend([p[3], p[4], p[5]].map(|v| v as f32));
        k
    };
    for t in m.indices.chunks_exact(3) {
        let pts = [vert(t[0]), vert(t[1]), vert(t[2])];
        if pts
            .iter()
            .all(|p| (0..3).all(|a| p[a] >= bx.lo[a] && p[a] <= bx.hi[a]))
        {
            let ids: Vec<u32> = t
                .iter()
                .zip(&pts)
                .map(|(&k, p)| *map.entry(k).or_insert_with(|| push(&mut out, *p)))
                .collect();
            out.indices.extend_from_slice(&ids);
            continue;
        }
        let mut poly: Vec<[f64; 6]> = pts.to_vec();
        for a in 0..3 {
            for (pos, below) in [(bx.lo[a], false), (bx.hi[a], true)] {
                let inside = |p: &[f64; 6]| if below { p[a] <= pos } else { p[a] >= pos };
                let mut next = Vec::with_capacity(poly.len() + 2);
                for i in 0..poly.len() {
                    let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
                    if inside(&p) {
                        next.push(p);
                    }
                    if inside(&p) != inside(&q) {
                        let s = (pos - p[a]) / (q[a] - p[a]);
                        let mut r = [0.0; 6];
                        for c in 0..6 {
                            r[c] = p[c] + (q[c] - p[c]) * s;
                        }
                        r[a] = pos;
                        next.push(r);
                    }
                }
                // 頂点がちょうど面の上にあると、その頂点と同じ交点が足される（つぶれた三角形になる）。続けて同じ点は 1 つにする
                // 面にごく近い頂点からの交点も同じ点とみなす（箱の大きさの 10⁻⁵ 倍以内）
                let same = |q: &[f64; 6], p: &[f64; 6]| (0..3).all(|c| (q[c] - p[c]).abs() <= tol);
                next.dedup_by(|q, p| same(q, p));
                while next.len() > 1 && same(&next[0], &next[next.len() - 1]) {
                    next.pop();
                }
                poly = next;
                if poly.len() < 3 {
                    break;
                }
            }
            if poly.len() < 3 {
                break;
            }
        }
        if poly.len() < 3 {
            continue;
        }
        let ids: Vec<u32> = poly.iter().map(|p| push(&mut out, *p)).collect();
        for k in 1..ids.len() - 1 {
            out.indices.extend_from_slice(&[ids[0], ids[k], ids[k + 1]]);
        }
    }
    out
}

/// 頂点でつながった成分のうち、三角形が `max` 個以下のものを捨てる（薄い枝の面を枝の境目で切り取ると、
/// 境目が面をかすめる所に小さな切れ端ができる。今まではその枝ごと描いていなかった所なので、捨てても今より欠けない）
fn drop_small_components(m: &mut Mesh, max: usize) {
    let nv = m.positions.len() / 3;
    let mut parent: Vec<u32> = (0..nv as u32).collect();
    fn find(p: &mut [u32], mut x: u32) -> u32 {
        while p[x as usize] != x {
            p[x as usize] = p[p[x as usize] as usize];
            x = p[x as usize];
        }
        x
    }
    for t in m.indices.chunks_exact(3) {
        let a = find(&mut parent, t[0]);
        for &k in &t[1..] {
            let b = find(&mut parent, k);
            if a != b {
                parent[b as usize] = a;
            }
        }
    }
    let mut size: crate::adaptive::HashMap<u32, usize> = Default::default();
    for t in m.indices.chunks_exact(3) {
        *size.entry(find(&mut parent, t[0])).or_default() += 1;
    }
    let mut out = Vec::with_capacity(m.indices.len());
    for t in m.indices.chunks_exact(3) {
        if size[&find(&mut parent, t[0])] > max {
            out.extend_from_slice(t);
        }
    }
    m.indices = out;
}

/// 距離 `tol` より近い頂点どうしを 1 つにまとめ、つぶれた三角形（2 つの頂点が同じになったもの）を捨てる
/// （箱での切り落としと枝の境目での切り取りが同じ所でぶつかると、ほぼ同じ位置の頂点ができる）
fn drop_slivers(m: &mut Mesh, tol: f32) {
    let mut rep: crate::adaptive::HashMap<[i64; 3], u32> = Default::default();
    let ids: Vec<u32> = m
        .positions
        .chunks_exact(3)
        .enumerate()
        .map(|(k, p)| {
            *rep.entry([0, 1, 2].map(|a| (p[a] / tol).round() as i64))
                .or_insert(k as u32)
        })
        .collect();
    let mut out = Vec::with_capacity(m.indices.len());
    for t in m.indices.chunks_exact(3) {
        let r = [ids[t[0] as usize], ids[t[1] as usize], ids[t[2] as usize]];
        if r[0] != r[1] && r[1] != r[2] && r[2] != r[0] {
            out.extend_from_slice(&r);
        }
    }
    m.indices = out;
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
    let pts: Vec<[f64; 3]> = (0..m * m * m)
        .map(|l| {
            [
                dom.at(0, (l % m) as f64, N),
                dom.at(1, (l / m % m) as f64, N),
                dom.at(2, (l / (m * m)) as f64, N),
            ]
        })
        .collect();
    let (nv, dv) = (eval_at(num, &pts), eval_at(den, &pts));
    // D の符号が変わる辺（両端の格子点の番号）
    let mut edges: Vec<(usize, usize)> = Vec::new();
    for l in 0..m * m * m {
        let q = [l % m, l / m % m, l / (m * m)];
        for (a, stride) in [1, m, m * m].into_iter().enumerate() {
            if q[a] + 1 < m
                && dv[l] * dv[l + stride] < 0.0
                && nv[l].is_finite()
                && nv[l + stride].is_finite()
            {
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
        edges
            .iter()
            .zip(t)
            .map(|(&(i, o), &t)| [0, 1, 2].map(|a| pts[i][a] + (pts[o][a] - pts[i][a]) * t))
            .collect()
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
fn mesh_masked(
    prog: &Program,
    dom: Domain,
    n: usize,
    refine: Refine,
    mask: Option<Vec<bool>>,
) -> Mesh {
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
///
/// `may_zero(lo, hi)` は、箱の中の、その枝にいる点に面があるかもしれないか。面が無いと分かるマスの層を、軸ごとに両端から外す
/// （縦に細長い枝を面が 1 か所で横切るだけの部品で、枝を囲む大きな立方体の格子を用意しない）。面がどこにも無ければ None
fn piece_grid(
    blocks: &[u32],
    bbox: ([u32; 3], [u32; 3]),
    dom: Domain,
    en: usize,
    pn: usize,
    may_zero: &dyn Fn([f64; 3], [f64; 3]) -> bool,
) -> Option<(Domain, usize, Vec<bool>, Domain)> {
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
    let world = |a: usize, k: usize| {
        (
            edom.at(a, (BLOCK * k) as f64, e),
            edom.at(a, (BLOCK * (k + 1)).min(e) as f64, e),
        )
    };
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
        let (lo, hi) = (
            edom.at(a, bbox.0[a] as f64 - 1.0, e),
            edom.at(a, bbox.1[a] as f64 + 1.0, e),
        );
        c0[a] = c0[a].max(cell(a, lo).floor() as i64 - 1);
        c1[a] = c1[a].min(cell(a, hi).ceil() as i64 + 1);
        c0[a] = c0[a].max(-1);
        c1[a] = c1[a].min(pn as i64 + 1).max(c0[a] + 4);
    }
    // 面が無いと分かる層を外す。面のあるマスの 2 マス外までは残す（面の三角形は隣のマスの頂点とつながり、
    // 枝の外に出た分は後で切り落とす。切り落とす境目の辺が欠けないようにする）
    // 外す前の範囲（ブロックの印は、この範囲の格子の区切りで付ける）
    let (o0, o1) = (c0, c1);
    // 層を外さないときに面を作る範囲: 枝の範囲を囲む立方体のうち、枝の範囲にかかるブロックまで（枝の範囲より少し広い。
    // 枝の格子点が無くても、格子点の間に枝の細い切れ端があって面が描けることがある）。ここから層を外す
    let old_total = (0..3).map(|a| o1[a] - o0[a]).max().unwrap_or(4);
    let bs = BLOCK as i64;
    for a in 0..3 {
        c1[a] = o0[a] + old_total.min(bs * ((o1[a] - o0[a] + bs - 1) / bs));
    }
    for a in 0..3 {
        let slab = |c0: &[i64; 3], c1: &[i64; 3], k: i64| {
            let at = |b: usize, c: i64| dom.lo[b] + c as f64 * step[b];
            let lo = [0, 1, 2].map(|b| at(b, if b == a { k } else { c0[b] }));
            let hi = [0, 1, 2].map(|b| at(b, if b == a { k + 1 } else { c1[b] }));
            may_zero(lo, hi)
        };
        let mut k0 = c0[a];
        while k0 < c1[a] && !slab(&c0, &c1, k0) {
            k0 += 1;
        }
        if k0 == c1[a] {
            return None;
        }
        let mut k1 = c1[a];
        while k1 > k0 + 1 && !slab(&c0, &c1, k1 - 1) {
            k1 -= 1;
        }
        let (old0, old1) = (c0[a], c1[a]);
        c0[a] = old0.max(k0 - 2);
        c1[a] = old1.min(k1 + 2);
        if c1[a] - c0[a] < 4 {
            c0[a] = old0.max(c1[a] - 4);
            c1[a] = old1.min(c0[a] + 4);
        }
    }
    // 立方体にする（マスの数を、一番長い軸にそろえる）。implicit_solid_with は外側に 1 マスずつ広げるので、その分を引いて渡す
    let total = (0..3).map(|a| c1[a] - c0[a]).max().unwrap_or(4) as usize;
    let inner = total - 2;
    // 立方体の始まりのマス。下の層を外した軸は、立方体の上の端が残した範囲の上の端に来るように下げる
    // （立方体が範囲の上にはみ出すと、表示範囲の外の面まで作ってしまう）。外す前の始まりよりは下げない。
    // 層を外していない軸は、今までどおり範囲の下の端から
    let org: [i64; 3] = [0, 1, 2].map(|a| {
        if c0[a] > o0[a] {
            o0[a].max(c0[a].min(c1[a] - total as i64))
        } else {
            c0[a]
        }
    });
    let sub = Domain {
        lo: [0, 1, 2].map(|a| dom.lo[a] + (org[a] + 1) as f64 * step[a]),
        hi: [0, 1, 2].map(|a| dom.lo[a] + (org[a] + 1 + inner as i64) as f64 * step[a]),
    };
    // ブロックごとの印。枝のブロック（とその 1 マス外）にかかるブロックに付ける。
    // ブロックの区切りは、層を外す前の範囲の格子（マスの番号は、全体の番号 − o0）で決めてから、この格子（全体の番号 − org）のブロックに直す。
    // 枝のブロックの外でも、区切りの都合で印の付いたブロックにある面は描く。層を外して区切りがずれても、描く範囲が減らないようにする
    let old_nb = (old_total as usize).div_ceil(BLOCK) as i64;
    let nb = total.div_ceil(BLOCK);
    let mut mask = vec![false; nb * nb * nb];
    for &b in blocks {
        let k = eb(b);
        let span = |a: usize| {
            let (lo, hi) = world(a, k[a]);
            // 立方体にそろえて広がった分（短い軸の、枝の範囲の外）には印を付けない
            let last = (o1[a] - o0[a] - 1).clamp(0, old_total - 1);
            let lo = (cell(a, lo).floor() as i64 - 1 - o0[a]).clamp(0, last) / bs;
            let hi = ((cell(a, hi).ceil() as i64 + 1 - o0[a]).clamp(0, last) / bs).min(old_nb - 1);
            // 前の区切りのブロック lo〜hi のマスの範囲（全体の番号）のうち、残した範囲に入る分
            let (c_lo, c_hi) = (
                (o0[a] + bs * lo).max(c0[a]),
                (o0[a] + bs * (hi + 1)).min(c1[a]),
            );
            if c_lo >= c_hi {
                return 1..=0;
            }
            (((c_lo - org[a]) / bs) as usize)..=(((c_hi - 1 - org[a]) / bs) as usize).min(nb - 1)
        };
        for bk in span(2) {
            for bj in span(1) {
                for bi in span(0) {
                    mask[bi + nb * (bj + nb * bk)] = true;
                }
            }
        }
    }
    // 面を探す範囲（立方体にそろえる前の、枝の範囲のうち、層を外して残った分）
    let tight = Domain {
        lo: [0, 1, 2].map(|a| dom.lo[a] + c0[a] as f64 * step[a]),
        hi: [0, 1, 2].map(|a| dom.lo[a] + c1[a].min(o1[a]) as f64 * step[a]),
    };
    Some((sub, inner, mask, tight))
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
    /// この枝の格子点があり、厚みのある所のブロック（とその隣の 1 セル分）
    blocks: Vec<u32>,
    /// 薄いが、枝の中を面が通るので描くブロック（`blocks` とは別の部品として、後から描く）
    extra: Vec<u32>,
    /// 格子より薄いので外したブロックがある（陽関数として描けなければ「細かすぎて描けない部分あり」）
    dropped: bool,
    /// 薄いので外したブロック（陽関数として描くとき、面を探す列を絞るのに使う）
    thin: Vec<u32>,
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
    // 格子点ごとの F の値（符号だけを使う）
    let mut fvals: Vec<f32> = vec![f32::NAN; m * m * m];
    let mut fout = vec![C::new(0.0, 0.0); m * m];
    // 内接する球の外の点（と 2 マスの余裕）は、どの枝にも入れない（[`in_inscribed_ball`]）
    let ball = BALL_ONLY.with(|b| b.get()).then(|| {
        let c = [0, 1, 2].map(|a| (dom.lo[a] + dom.hi[a]) / 2.0);
        let r = (0..3)
            .map(|a| (dom.hi[a] - dom.lo[a]) / 2.0)
            .fold(f64::INFINITY, f64::min);
        let step = (0..3)
            .map(|a| (dom.hi[a] - dom.lo[a]) / n as f64)
            .fold(0.0, f64::max);
        (c, r + step)
    });
    for kz in 0..m {
        let zs = vec![dom.at(2, kz as f64, n); m * m];
        let br = branch.eval(&[&xs, &ys, &zs], m * m);
        f.eval_batch(&[&xs, &ys, &zs], &mut fout);
        for (o, v) in fvals[m * m * kz..m * m * (kz + 1)].iter_mut().zip(&fout) {
            *o = as_real(*v) as f32;
        }
        for l in 0..m * m {
            let b = &br[l * k..(l + 1) * k];
            if b.iter().any(|v| !v.is_finite()) {
                continue;
            }
            if let Some((c, r)) = ball {
                let d2 = (xs[l] - c[0]).powi(2) + (ys[l] - c[1]).powi(2) + (zs[l] - c[2]).powi(2);
                if d2 > r * r {
                    continue;
                }
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
    // 格子点ごとの「厚みのある所か」: 3 つの軸のどれでも、隣（前か後ろ）が同じ枝の点。
    // 「面が通る所か」: どれかの軸の隣が同じ枝の点で、F の符号がそこで変わる（枝の中で面が格子の辺を横切る）
    let strides = [1, m, m * m];
    let mut paired: Vec<bool> = vec![false; m * m * m];
    let mut crossed: Vec<bool> = vec![false; m * m * m];
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
                paired[l] = (0..3).all(|a| {
                    (q[a] > 0 && ids[l - strides[a]] == id)
                        || (q[a] + 1 < m && ids[l + strides[a]] == id)
                });
                let fl = fvals[l];
                // 表示範囲の中の点だけ（格子は表示範囲の外側に 1 マス広げてある。外の切れ端のために枝を描かない）
                crossed[l] = fl.is_finite()
                    && q.iter().all(|&c| (1..m - 1).contains(&c))
                    && (0..3).any(|a| {
                        let other = |o: usize| {
                            ids[o] == id && fvals[o].is_finite() && (fvals[o] < 0.0) != (fl < 0.0)
                        };
                        (q[a] > 0 && other(l - strides[a]))
                            || (q[a] + 1 < m && other(l + strides[a]))
                    });
            }
        }
    }
    // 枝ごとの、その枝を描くブロック: その枝の格子点があり（ブロックのセルの角と、その外側の 1 点までを見る）、
    // そのうち厚みのある所の点が 3 割以上のブロック。薄い所（面が格子より細かく重なる所）は、ブロックごとに外す
    let mut blocks: Vec<Vec<u32>> = vec![Vec::new(); taus.len()];
    let mut seen_in: Vec<u32> = vec![0; taus.len()];
    let mut cnt: Vec<(u32, u32, u32)> = vec![(0, 0, 0); taus.len()];
    // 薄いが面が通るので描くかもしれないブロック（下で、同じ枝の隣のブロックを見て決める）
    let mut rescued: Vec<Vec<u32>> = vec![Vec::new(); taus.len()];
    let mut here: Vec<u32> = Vec::new();
    let mut thin = false;
    let mut dropped: Vec<bool> = vec![false; taus.len()];
    let mut thin_blocks: Vec<Vec<u32>> = vec![Vec::new(); taus.len()];
    for bk in 0..nb {
        for bj in 0..nb {
            for bi in 0..nb {
                let bid = (bi + nb * (bj + nb * bk)) as u32;
                let span =
                    |b: usize| (BLOCK * b).saturating_sub(1)..=(BLOCK * (b + 1) + 1).min(m - 1);
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
                                cnt[id as usize] = (0, 0, 0);
                                here.push(id);
                            }
                            cnt[id as usize].0 += 1;
                            cnt[id as usize].1 += paired[ix + row] as u32;
                            cnt[id as usize].2 += crossed[ix + row] as u32;
                        }
                    }
                }
                // 格子の端のブロック（表示範囲の端で切れて薄くなっただけの所）は、「描けない部分」に数えない
                let edge = [bi, bj, bk].iter().any(|&b| b == 0 || b + 1 == nb);
                for &id in &here {
                    let (all, solid, cross) = cnt[id as usize];
                    // 薄くても、枝の中で面が格子の辺を横切る所があれば描く（面に沿った向きにだけ薄い枝。
                    // floor(x y) = z floor(z) の水平な細い帯など。面に垂直な向きに薄い枝（面が格子より細かく重なる所）では、
                    // 同じ枝の隣の点がそもそも無いので、ここを通らない）
                    if 10 * solid >= 3 * all {
                        blocks[id as usize].push(bid);
                    } else if cross > 0 {
                        rescued[id as usize].push(bid);
                    } else {
                        thin |= !edge;
                        dropped[id as usize] |= !edge;
                        thin_blocks[id as usize].push(bid);
                    }
                }
            }
        }
    }
    // 薄いが面が通るブロックは、同じ枝の描くブロック（どちらの種類でも）が隣（26 方向）にあるときだけ描く。
    // 細い帯は長く続くので隣が見つかる。面が格子とほぼ同じ間隔で重なる密な層では、面が通るブロックが
    // ぽつんと 1 つだけになることがあり、それを描くと、層の一部だけが浮いた切れ端になる（mod(x y, z) = 0 の x y = −23 z など）
    let mut extra: Vec<Vec<u32>> = vec![Vec::new(); taus.len()];
    for (id, res) in rescued.iter().enumerate() {
        if res.is_empty() {
            continue;
        }
        let have: crate::adaptive::HashSet<u32> = blocks[id].iter().chain(res).copied().collect();
        let near = |b: u32, have: &crate::adaptive::HashSet<u32>| {
            let b = b as usize;
            let k = [b % nb, (b / nb) % nb, b / (nb * nb)];
            (0..27).any(|d| {
                let o = [d % 3, d / 3 % 3, d / 9];
                if o == [1, 1, 1] {
                    return false;
                }
                let q = [0, 1, 2].map(|a| (k[a] + o[a]).wrapping_sub(1));
                q.iter().all(|&c| c < nb)
                    && have.contains(&((q[0] + nb * (q[1] + nb * q[2])) as u32))
            })
        };
        let keep: Vec<u32> = res.iter().copied().filter(|&b| near(b, &have)).collect();
        if keep.len() < res.len() {
            thin = true;
            dropped[id] = true;
            thin_blocks[id].extend(res.iter().copied().filter(|b| !keep.contains(b)));
        }
        extra[id] = keep;
    }
    let mut out: Vec<BranchGroup> = Vec::new();
    // 薄くて外しただけの枝も残す（1 次の部品なら、陽関数として格子によらず描ける。implicit_by_pieces）
    for (id, (((tau, blocks), extra), thin)) in taus
        .into_iter()
        .zip(blocks)
        .zip(extra)
        .zip(thin_blocks)
        .enumerate()
    {
        if !blocks.is_empty() || !extra.is_empty() || dropped[id] {
            let (lo, hi) = boxes[id];
            out.push(BranchGroup {
                tau,
                blocks,
                extra,
                dropped: dropped[id],
                thin,
                count: counts[id],
                lo,
                hi,
            });
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
fn project_to_surface(
    surf: &Program,
    pts: &[[f64; 3]],
    normals: Option<&[f32]>,
    size: f64,
) -> Vec<[f64; 3]> {
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
            let ahead: Vec<[f64; 3]> = cur
                .iter()
                .enumerate()
                .map(|(k, p)| [0, 1, 2].map(|a| p[a] + h * dir(k)[a]))
                .collect();
            let f1 = eval_at(surf, &ahead);
            f0.iter()
                .zip(&f1)
                .enumerate()
                .map(|(k, (f, f1))| dir(k).map(|d| f * h / (f1 - f) * d))
                .collect()
        } else {
            let mut grad = vec![[0.0f64; 3]; cur.len()];
            for a in 0..3 {
                let (fp, fm) = (eval_at(surf, &shift(a, h)), eval_at(surf, &shift(a, -h)));
                for (g, (p, m)) in grad.iter_mut().zip(fp.iter().zip(&fm)) {
                    g[a] = (p - m) / (2.0 * h);
                }
            }
            f0.iter()
                .zip(&grad)
                .map(|(f, g)| g.map(|c| f * c / (g[0] * g[0] + g[1] * g[1] + g[2] * g[2])))
                .collect()
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

/// 細かく分ける回数の上限（1 回で辺の長さが半分）
const SPLIT_ROUNDS: usize = 3;
/// 陽関数として描いた部品では、条件の値の幅が、範囲の幅のこの倍より広い三角形は分けない。
/// そうした三角形は、範囲が細いのではなく、条件の式が極を持つ所（`mod(x y, z)` の枝の値 x y / z の z = 0）をまたいでいる。
/// 分けても範囲の中の頂点はできず、枝ごとに極の線に沿って 3 回分けるだけになる（`mod(x y, z) = 1` で 300 万点）
const POLE_SPAN: f64 = 100.0;

/// 条件の範囲をまたぐ三角形（ある頂点では下の端より下、別の頂点では上の端より上）を、辺の中点で分ける（[`SPLIT_ROUNDS`] 回まで）。
///
/// 切り取りは頂点ごとに条件を調べるので、条件の範囲が三角形より細い所（`floor(x y) = z floor(z)` の帯 i ≤ x y < i + 1 は、
/// x = 10 で幅 0.1。細かさ 80 のマスは 0.25）は、範囲の中に頂点が 1 つも無いと、三角形ごと捨てられていた。
/// 分けるのは辺ごとに決め、その辺を持つ三角形は全部同じ中点で分ける（隣の三角形と頂点を共有して、継ぎ目に隙間を作らない）。
/// 中点は辺の上（面の上に寄せない）。法線は両端の間を取る
fn split_straddling(mesh: &mut Mesh, conds: &[Cond], max_span: f64) {
    if conds.is_empty() {
        return;
    }
    let pos = |m: &Mesh, v: usize| [0, 1, 2].map(|a| m.positions[3 * v + a] as f64);
    let has_normals = mesh.normals.len() == mesh.positions.len();
    // 頂点ごとの条件の値（条件ごと）
    let mut vals: Vec<Vec<f64>> = vec![Vec::new(); conds.len()];
    for _ in 0..SPLIT_ROUNDS {
        let count = mesh.positions.len() / 3;
        let done = vals[0].len();
        if done < count {
            let new: Vec<[f64; 3]> = (done..count).map(|v| pos(mesh, v)).collect();
            for (c, v) in conds.iter().zip(vals.iter_mut()) {
                v.extend(eval_at(&c.prog, &new));
            }
        }
        // 分ける辺: 範囲をまたぐ三角形の 3 本の辺
        let straddles = |t: &[u32]| {
            conds.iter().zip(&vals).any(|(c, v)| {
                let s = t.iter().map(|&k| v[k as usize]);
                let (lo, hi) = s.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                    (a.min(x), b.max(x))
                });
                lo.is_finite()
                    && hi.is_finite()
                    && lo < c.lo
                    && hi > c.hi
                    && hi - lo <= max_span * (c.hi - c.lo)
            })
        };
        let mut mid: crate::adaptive::HashMap<(u32, u32), u32> = Default::default();
        for t in mesh.indices.chunks_exact(3) {
            if straddles(t) {
                for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                    mid.insert((a.min(b), a.max(b)), NONE);
                }
            }
        }
        if mid.is_empty() {
            return;
        }
        // 中点を足す（毎回同じ番号になるよう、辺の番号順に）
        let mut keys: Vec<(u32, u32)> = mid.keys().copied().collect();
        keys.sort_unstable();
        for (a, b) in keys {
            let (p, q) = (pos(mesh, a as usize), pos(mesh, b as usize));
            let id = mesh.push_vertex([0, 1, 2].map(|k| 0.5 * (p[k] + q[k])));
            if has_normals {
                let n: [f32; 3] = [0, 1, 2]
                    .map(|k| mesh.normals[3 * a as usize + k] + mesh.normals[3 * b as usize + k]);
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-30);
                mesh.normals.extend(n.map(|c| c / len));
            }
            mid.insert((a, b), id);
        }
        let m = |a: u32, b: u32| mid.get(&(a.min(b), a.max(b))).copied();
        let mut out: Vec<u32> = Vec::with_capacity(mesh.indices.len() * 2);
        for t in mesh.indices.chunks_exact(3) {
            // 分ける辺の並びが決まった形になるよう回す（向きは保つ）
            let rot = |r: usize| [t[r % 3], t[(r + 1) % 3], t[(r + 2) % 3]];
            let split = |v: [u32; 3]| [m(v[0], v[1]), m(v[1], v[2]), m(v[2], v[0])];
            match split(rot(0)).iter().filter(|x| x.is_some()).count() {
                0 => out.extend_from_slice(t),
                3 => {
                    let [a, b, c] = rot(0);
                    let [ab, bc, ca] = split(rot(0)).map(|x| x.unwrap_or(NONE));
                    out.extend_from_slice(&[a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca]);
                }
                1 => {
                    // 分ける辺を a-b にする
                    let r = (0..3).find(|&r| split(rot(r))[0].is_some()).unwrap_or(0);
                    let [a, b, c] = rot(r);
                    let ab = split(rot(r))[0].unwrap_or(NONE);
                    out.extend_from_slice(&[a, ab, c, ab, b, c]);
                }
                _ => {
                    // 分けない辺を c-a にする
                    let r = (0..3).find(|&r| split(rot(r))[2].is_none()).unwrap_or(0);
                    let [a, b, c] = rot(r);
                    let [ab, bc, _] = split(rot(r)).map(|x| x.unwrap_or(NONE));
                    out.extend_from_slice(&[ab, b, bc, a, ab, bc, a, bc, c]);
                }
            }
        }
        mesh.indices = out;
    }
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
fn clip_piece(
    mesh: &mut Mesh,
    surf: &Program,
    conds: &[Cond],
    defined: &[&Program],
    nonzero: &[&Program],
    size: f64,
) {
    clip_piece_with(mesh, Some(surf), conds, defined, nonzero, size)
}

/// [`clip_piece`] で、`surf` が None なら頂点を面に寄せない（頂点がもう面の上にある。陽関数として描いた部品）
fn clip_piece_with(
    mesh: &mut Mesh,
    surf: Option<&Program>,
    conds: &[Cond],
    defined: &[&Program],
    nonzero: &[&Program],
    size: f64,
) {
    if mesh.positions.is_empty() || (conds.is_empty() && defined.is_empty() && nonzero.is_empty()) {
        return;
    }
    split_straddling(
        mesh,
        conds,
        if surf.is_some() {
            f64::INFINITY
        } else {
            POLE_SPAN
        },
    );
    let count = mesh.positions.len() / 3;
    let pts: Vec<[f64; 3]> = mesh
        .positions
        .chunks_exact(3)
        .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
        .collect();
    let mut inside = vec![true; count];
    // 境目が面に重なっていない条件（辺の上で境目を探すときに調べる）
    let mut live: Vec<&Cond> = Vec::new();
    // 0 とみなす大きさ（式ごと。面の上の値のいちばん大きいものの 10⁻⁷ 倍）
    let mut zero_tol: Vec<f64> = Vec::new();
    if !conds.is_empty() || !nonzero.is_empty() {
        let normals =
            (mesh.normals.len() == mesh.positions.len()).then_some(mesh.normals.as_slice());
        let on = match surf {
            Some(surf) => project_to_surface(surf, &pts, normals, size),
            None => pts.clone(),
        };
        for d in nonzero {
            let v = eval_at(d, &on);
            let tol = 1e-7
                * v.iter()
                    .filter(|v| v.is_finite())
                    .fold(0.0f64, |m, v| m.max(v.abs()));
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
            let mostly = |b: f64| {
                !finite.is_empty()
                    && 10 * finite.iter().filter(|&&v| near(v, b)).count() >= 9 * finite.len()
            };
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
        *mesh = Mesh {
            note: std::mem::take(&mut mesh.note),
            ..Default::default()
        };
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
    // 境目の頂点を足す（法線は両端の間を取る）。境目が成り立つ側の頂点そのもの（頂点がちょうど境目の上にある。
    // 回していない式では、枝の境目が格子の面に重なるので多い）なら、頂点を足さずにその頂点を使う
    // 境目が成り立たない側の頂点のすぐ手前なら、その頂点の位置に 1 つだけ頂点を足して、その頂点を使う辺どうしで共有する
    // （別々に足すと、ほぼ同じ位置の 2 点でつぶれた三角形ができる。陽関数として描いた部品は、格子の線が境目のすぐ脇に来る）
    let mut at_outside: crate::adaptive::HashMap<u32, u32> = Default::default();
    let cut_vertex: Vec<u32> = cuts
        .iter()
        .enumerate()
        .map(|(e, &(i, o))| {
            let mut t = lo[e];
            if t < CUT_SNAP {
                return i;
            }
            if t > 1.0 - CUT_SNAP {
                if let Some(&id) = at_outside.get(&o) {
                    return id;
                }
                t = 1.0;
            }
            let (p, q) = (pts[i as usize], pts[o as usize]);
            let id = mesh.push_vertex([0, 1, 2].map(|a| p[a] + (q[a] - p[a]) * t));
            let (ni, no) = (i as usize * 3, o as usize * 3);
            if mesh.normals.len() >= 3 * count {
                let nv: [f32; 3] = [0, 1, 2].map(|a| {
                    mesh.normals[ni + a] + (mesh.normals[no + a] - mesh.normals[ni + a]) * t as f32
                });
                let len = (nv[0] * nv[0] + nv[1] * nv[1] + nv[2] * nv[2])
                    .sqrt()
                    .max(1e-30);
                mesh.normals.extend(nv.map(|c| c / len));
            }
            if t == 1.0 {
                at_outside.insert(o, id);
            }
            id
        })
        .collect();
    let cut = |i: u32, o: u32| cut_vertex[cut_of[&(i, o)]];
    // 切ってできた三角形のうち、頂点が重なってつぶれたもの（面積 0）は入れない
    let mut indices: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    let push = |indices: &mut Vec<u32>, t: [u32; 3]| {
        if t[0] != t[1] && t[1] != t[2] && t[2] != t[0] {
            indices.extend_from_slice(&t);
        }
    };
    for t in mesh.indices.chunks_exact(3) {
        let ins = [
            inside[t[0] as usize],
            inside[t[1] as usize],
            inside[t[2] as usize],
        ];
        match ins.iter().filter(|&&i| i).count() {
            3 => indices.extend_from_slice(t),
            1 => {
                let k = ins.iter().position(|&i| i).unwrap_or(0);
                let (a, b, c) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                push(&mut indices, [a, cut(a, b), cut(a, c)]);
            }
            2 => {
                let k = ins.iter().position(|&i| !i).unwrap_or(0);
                let (c, a, b) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                let (ca, cb) = (cut(a, c), cut(b, c));
                push(&mut indices, [a, b, cb]);
                push(&mut indices, [a, cb, ca]);
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
    *mesh = Mesh {
        positions,
        normals,
        indices,
        note: std::mem::take(&mut mesh.note),
        ..Default::default()
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prog(src: &str) -> Program {
        Program::compile(&crate::parser::parse(src).unwrap(), &["x", "y", "z"]).unwrap()
    }

    /// 点 q の近く（1 辺 h の箱で、隣の箱まで）にメッシュの頂点があるか調べる関数
    fn near_fn(m: &Mesh, h: f64) -> impl Fn([f64; 3]) -> bool {
        let cells: crate::adaptive::HashSet<[i64; 3]> = m
            .positions
            .chunks_exact(3)
            .map(|p| [0, 1, 2].map(|a| (p[a] as f64 / h).floor() as i64))
            .collect();
        move |q: [f64; 3]| {
            let c = q.map(|v| (v / h).floor() as i64);
            (-1..=1).any(|a| {
                (-1..=1).any(|b| (-1..=1).any(|e| cells.contains(&[c[0] + a, c[1] + b, c[2] + e])))
            })
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
                *ids.entry([0, 1, 2].map(|a| (p[a] as f64 * 1e4).round() as i64))
                    .or_insert(next)
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
            *sizes
                .entry(root(&mut parent, vid[t[0] as usize]))
                .or_default() += 1;
        }
        sizes.into_values().collect()
    }

    #[test]
    fn squared_factors_are_drawn_as_plain_surfaces() {
        // (x² + y² − 4)² (z − 1)² = 0 は、円柱 x² + y² = 4 と平面 z = 1。F は 0 に触れるだけで符号が変わらないが、
        // 因数に分ければ、どちらもふつうの面。粗い格子・軽い計算（回転の再生中）でも、穴や破片ができない
        let d = Domain {
            lo: [-10.0, -10.0, -6.0],
            hi: [10.0, 10.0, 6.0],
        };
        for (n, refine) in [
            (80, Refine::Normal),
            (40, Refine::Light),
            (24, Refine::Light),
        ] {
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
        let d = Domain {
            lo: [-10.0, -10.0, -6.0],
            hi: [10.0, 10.0, 6.0],
        };
        for (n, refine) in [
            (80, Refine::Normal),
            (40, Refine::Light),
            (24, Refine::Light),
        ] {
            let m = implicit_solid_opt(&prog("tan(x) - z y"), d, n, &[], 0, refine);
            let h = 20.0 / n as f64;
            // 面から離れた頂点が無い（面までの距離を、分子の値 ÷ 勾配の大きさで見積もる）
            for p in m.positions.chunks_exact(3) {
                let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
                let f = x.sin() - z * y * x.cos();
                let g = [x.cos() + z * y * x.sin(), -z * x.cos(), -y * x.cos()];
                let dist = f.abs() / (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
                assert!(
                    dist < 0.6 * h,
                    "細かさ {n}: 面から {dist} 離れた頂点 ({x}, {y}, {z})"
                );
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
        let d = Domain {
            lo: [-10.0, -10.0, -6.0],
            hi: [10.0, 10.0, 6.0],
        };
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
                        (0..3)
                            .all(|a| (q[a] as f64) > d.lo[a] + 0.5 && (q[a] as f64) < d.hi[a] - 0.5)
                    })
                })
                .flatten()
                .copied()
                .collect(),
            ..Default::default()
        };
        let small = component_sizes(&inner)
            .into_iter()
            .filter(|&s| s <= 50)
            .count();
        assert_eq!(small, 0, "破片の数");
    }

    #[test]
    fn unattained_branch_values_are_not_drawn() {
        // z = mod(x y, z) は、mod(x y, z) が z に近づくが z にはならないので、零点が無い。
        // 枝 τ の式 z − (x y − τ z) = 0 の面は、枝の境目（x y / z = τ + 1）の上で、その境目は枝に含まれない
        let d = Domain {
            lo: [-10.0, -10.0, -6.0],
            hi: [10.0, 10.0, 6.0],
        };
        let m = implicit_solid_opt(&prog("z - mod(x y, z)"), d, 80, &[], 0, Refine::Normal);
        let far = m
            .positions
            .chunks_exact(3)
            .filter(|p| (p[2] as f64).abs() > 0.5)
            .count();
        assert_eq!(far, 0, "z = 0 から離れた所の頂点");
    }

    #[test]
    fn branch_pieces_are_clipped_to_their_branch() {
        // floor(x) + z = 0 は、1 ≤ … の階段: i ≤ x < i + 1 で z = −i。段ごとに平らな面で、段の間に面は無い
        let d = Domain {
            lo: [-4.0, -4.0, -6.0],
            hi: [4.0, 4.0, 6.0],
        };
        let m = implicit_solid_opt(&prog("floor(x) + z"), d, 64, &[], 0, Refine::Normal);
        assert!(!m.indices.is_empty());
        for p in m.positions.chunks_exact(3) {
            let (x, z) = (p[0] as f64, p[2] as f64);
            // 段の端（x が整数の近く）は、どちらの段でもよい
            let ok = [x.floor(), (x + 1e-3).floor(), (x - 1e-3).floor()]
                .iter()
                .any(|i| (z + i).abs() < 1e-3);
            assert!(ok, "段の上に無い頂点 x = {x}, z = {z}");
        }
        let near = near_fn(&m, 0.125);
        for i in -3..3 {
            for s in 1..8 {
                let x = i as f64 + s as f64 / 8.0;
                assert!(
                    near([x, 0.3, -(i as f64)]),
                    "段 {i} の x = {x} が欠けている"
                );
            }
        }
    }

    #[test]
    fn tall_branches_keep_their_faces() {
        // floor(x) + floor(y) + z = mod(x y, 3) は、1 × 1 の柱ごとに z = mod(x y, 3) − floor(x) − floor(y) の面。
        // 枝は柱（縦に長い）だが、面は高さ 3 の範囲にしか無い。面の無い層を外して作っても、面が欠けない
        let d = Domain {
            lo: [-10.0, -10.0, -6.0],
            hi: [10.0, 10.0, 6.0],
        };
        let truth = |x: f64, y: f64| (x * y).rem_euclid(3.0) - x.floor() - y.floor();
        for n in [40usize, 80] {
            let m = implicit_solid_opt(
                &prog("floor(x) + floor(y) + z - mod(x y, 3)"),
                d,
                n,
                &[],
                0,
                Refine::Normal,
            );
            let h = 20.0 / n as f64;
            // 頂点は面の近く（ずれは格子の 1 マスの 3 割まで。柱の端・余りが飛ぶ所の近くは、隣の枝の値でもよい）
            let mut off = 0;
            for p in m.positions.chunks_exact(3) {
                let (x, y, z) = (p[0] as f64, p[1] as f64, p[2] as f64);
                let e = 1e-3;
                let ok = [
                    (0.0, 0.0),
                    (e, 0.0),
                    (-e, 0.0),
                    (0.0, e),
                    (0.0, -e),
                    (e, e),
                    (-e, -e),
                    (e, -e),
                    (-e, e),
                ]
                .iter()
                .any(|(dx, dy)| (z - truth(x + dx, y + dy)).abs() < 0.3 * h);
                off += !ok as usize;
            }
            assert!(
                off * 200 <= m.positions.len() / 3,
                "細かさ {n}: 面の上に無い頂点が {off} 個"
            );
            // 面の上の点（柱の中ほど）の近くに、メッシュがある
            let near = near_fn(&m, h);
            let (mut all, mut missing) = (0, 0);
            for i in -9..9 {
                for j in -9..9 {
                    for (sx, sy) in [(0.3, 0.3), (0.5, 0.7), (0.7, 0.4)] {
                        let (x, y) = (i as f64 + sx, j as f64 + sy);
                        let z = truth(x, y);
                        // 余りが飛ぶ線（x y が 3 の倍数）のすぐ近くと、表示範囲の外は見ない
                        let r = (x * y).rem_euclid(3.0);
                        if z.abs() > 5.5 || r < 0.3 || r > 2.7 {
                            continue;
                        }
                        all += 1;
                        missing += !near([x, y, z]) as usize;
                    }
                }
            }
            assert!(
                all > 300 && missing * 50 <= all,
                "細かさ {n}: {all} 点のうち {missing} 点が欠けている"
            );
        }
    }

    #[test]
    fn thin_bands_are_drawn() {
        // floor(x y) = z floor(z) の面は、z ∈ [k, k + 1) で高さ z = i/k（k² ≤ i < k² + k）の水平な帯 i ≤ x y < i + 1。
        // 帯は x = 10 で幅 0.1 と、格子（細かさ 80 で 0.25）より細い。枝は横に薄いが、面に垂直な向き（z）には厚いので描ける。
        // 前は「薄い枝」として描かず、描いても帯の中に頂点の無い三角形を捨てていたので、本当の面の 8 割が欠けていた
        let d = Domain {
            lo: [-10.0, -10.0, -6.0],
            hi: [10.0, 10.0, 6.0],
        };
        let p = prog("floor(x y) - z floor(z)");
        for (n, need) in [(80usize, 0.75), (24, 0.75)] {
            let m = implicit_solid_opt(&p, d, n, &[], 0, Refine::Light);
            let near = near_fn(&m, 0.1);
            let (mut all, mut hit) = (0, 0);
            for k in 1..=5i64 {
                for i in k * k..k * k + k {
                    let z = i as f64 / k as f64;
                    for s in 0..40 {
                        // 帯の真ん中の線 x y = i + 0.5 の上の点
                        let x = 1.0 + 8.5 * s as f64 / 40.0;
                        let y = (i as f64 + 0.5) / x;
                        if y.abs() > 9.5 || z > 5.5 {
                            continue;
                        }
                        for (px, py) in [(x, y), (-x, -y), (y, x)] {
                            all += 1;
                            hit += near([px, py, z]) as usize;
                        }
                    }
                }
            }
            assert!(all > 1000, "{all}");
            assert!(
                hit as f64 >= need * all as f64,
                "細かさ {n}: {all} 点のうち {hit} 点しか描けていない"
            );
        }
    }

    #[test]
    fn clipping_leaves_no_collapsed_triangles() {
        // 回していない式では、枝の境目が格子の面に重なり、切り取りの境目が頂点そのものになる。
        // そこで面積 0 の三角形（同じ頂点を 2 回使う三角形）を作らない
        let d = Domain {
            lo: [-10.0, -10.0, -6.0],
            hi: [10.0, 10.0, 6.0],
        };
        let m = implicit_solid_opt(
            &prog("mod(x, 2) + mod(y, 2) + mod(z, 2) - 2"),
            d,
            80,
            &[],
            0,
            Refine::Normal,
        );
        let key = |v: u32| {
            [0, 1, 2].map(|a| (m.positions[3 * v as usize + a] as f64 * 1e4).round() as i64)
        };
        let tris = m.indices.len() / 3;
        let collapsed = m
            .indices
            .chunks_exact(3)
            .filter(|t| key(t[0]) == key(t[1]) || key(t[1]) == key(t[2]) || key(t[2]) == key(t[0]))
            .count();
        assert!(
            tris > 100_000 && collapsed * 1000 <= tris,
            "{tris} 個のうち {collapsed} 個がつぶれている"
        );
    }

    #[test]
    fn coarse_gyroid_is_not_flipped() {
        // 回転の再生中の粗い格子（細かさ 24）でも、三角形の向きが全部そろっている（法線が F の勾配と同じ側を向く）
        let d = Domain {
            lo: [-10.0, -10.0, -6.0],
            hi: [10.0, 10.0, 6.0],
        };
        let m = implicit_solid_opt(
            &prog("sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)"),
            d,
            24,
            &[],
            0,
            Refine::Light,
        );
        let p = |v: u32| [0, 1, 2].map(|a| m.positions[3 * v as usize + a] as f64);
        let (mut pos, mut neg) = (0, 0);
        for t in m.indices.chunks_exact(3) {
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let (u, v) = (
                [0, 1, 2].map(|k| b[k] - a[k]),
                [0, 1, 2].map(|k| c[k] - a[k]),
            );
            let nrm = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let [x, y, z] = [0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0);
            let g = [
                x.cos() * y.cos() - z.sin() * x.sin(),
                -x.sin() * y.sin() + y.cos() * z.cos(),
                -y.sin() * z.sin() + z.cos() * x.cos(),
            ];
            let s = nrm[0] * g[0] + nrm[1] * g[1] + nrm[2] * g[2];
            if s > 0.0 {
                pos += 1;
            } else {
                neg += 1;
            }
        }
        assert!(
            pos + neg > 10_000 && pos.min(neg) == 0,
            "向きが混ざっている: {pos} / {neg}"
        );
    }
}

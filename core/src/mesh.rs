//! 曲面のメッシュ（三角形の集まり）を作る。

use crate::equation::others;
use crate::eval::{as_real, Program, ProgramSet};
use num_complex::Complex64 as C;

/// 表示範囲（直方体）。添字 0, 1, 2 が x, y, z。
#[derive(Debug, Clone, Copy)]
pub struct Domain {
    pub lo: [f64; 3],
    pub hi: [f64; 3],
}

impl Domain {
    /// axis 方向に n 等分したときの k 番目の座標
    fn at(&self, axis: usize, k: f64, n: usize) -> f64 {
        self.lo[axis] + (self.hi[axis] - self.lo[axis]) * k / n as f64
    }
}

/// 描画用のメッシュデータ。Three.js の BufferGeometry にそのまま渡せる形。
#[derive(Debug, Default)]
pub struct Mesh {
    /// 頂点座標 [x0, y0, z0, x1, y1, z1, ...]
    pub positions: Vec<f32>,
    /// 頂点の法線（陰影の計算に使う）
    pub normals: Vec<f32>,
    /// 三角形の頂点番号 [a0, b0, c0, a1, b1, c1, ...]
    pub indices: Vec<u32>,
    /// 格子線を引くための座標（頂点ごとに3つ）。空なら頂点座標 (x, y, z) を使う。
    /// 媒介変数表示の曲面では (u − u0, v − v0, z) を入れて、u, v に沿った格子線にする。
    pub grid_coords: Vec<f32>,
    /// 複素数の色分けをするなら、頂点ごとの関数の値 w の (Re w / |w|, Im w / |w|, log₂|w|)。空なら単色
    pub complex: Vec<f32>,
    /// 粗い版の三角形の頂点番号（頂点は同じものを使う）。視点を動かしている間は、こちらを描いて軽くする。
    /// 三角形が少ないメッシュでは空（そのまま描く）
    pub lod: Vec<u32>,
}

/// 三角形がこれより多いメッシュには、視点を動かしている間に描く粗い版（[`Mesh::lod`]）を作る
pub const LOD_MIN_TRIANGLES: usize = 60_000;
/// 粗い版の三角形の数の目安
const LOD_TARGET_TRIANGLES: usize = 30_000;

/// 三角形の向き（長さは面積の2倍）
fn tri_normal(p: &[f32], t: [u32; 3]) -> [f64; 3] {
    let v = |k: u32| {
        let k = k as usize * 3;
        [p[k] as f64, p[k + 1] as f64, p[k + 2] as f64]
    };
    let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
    let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
    [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]
}

fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// 格子でできていないメッシュ（陰関数など）の粗い版を作る（すでにあるか、三角形が少なければ何もしない）。
///
/// 空間を小さな立方体に区切り、同じ立方体に入る頂点を1つの頂点（最初に見つかったもの）で代表させる
/// （頂点クラスタリング）。3つの頂点が別々の立方体に入る三角形だけが残る。
/// 向きが大きく違う面（法線の一番大きい成分の軸と符号が違う面）は、同じ立方体の中でも別々に扱うので、
/// 薄い板の表と裏がくっつくことは少ない
pub fn add_cluster_lod(mesh: &mut Mesh) {
    let tris = mesh.indices.len() / 3;
    if !mesh.lod.is_empty() || tris <= LOD_MIN_TRIANGLES {
        return;
    }
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for p in mesh.positions.chunks_exact(3).filter(|p| p.iter().all(|v| v.is_finite())) {
        for a in 0..3 {
            lo[a] = lo[a].min(p[a]);
            hi[a] = hi[a].max(p[a]);
        }
    }
    let size = (0..3).map(|a| hi[a] - lo[a]).fold(0.0f32, f32::max);
    if size <= 0.0 || size.is_nan() {
        return;
    }
    let build = |cells: f32| -> Vec<u32> {
        let scale = cells / size;
        let mut first: std::collections::HashMap<u64, u32> = std::collections::HashMap::new();
        let rep: Vec<u32> = (0..mesh.positions.len() / 3)
            .map(|k| {
                let p = &mesh.positions[3 * k..3 * k + 3];
                if !p.iter().all(|v| v.is_finite()) {
                    return NONE;
                }
                let cell = |a: usize| ((p[a] - lo[a]) * scale) as u64;
                // 法線の一番大きい成分の軸と符号（6通り）
                let nr = mesh.normals.get(3 * k..3 * k + 3).unwrap_or(&[0.0, 0.0, 1.0]);
                let ax = (0..3).max_by(|&a, &b| nr[a].abs().total_cmp(&nr[b].abs())).unwrap_or(2);
                let side = (ax * 2 + (nr[ax] < 0.0) as usize) as u64;
                let key = cell(0) | cell(1) << 16 | cell(2) << 32 | side << 48;
                *first.entry(key).or_insert(k as u32)
            })
            .collect();
        let mut out = Vec::new();
        for t in mesh.indices.chunks_exact(3) {
            let (a, b, c) = (rep[t[0] as usize], rep[t[1] as usize], rep[t[2] as usize]);
            if a != b && b != c && a != c && a != NONE && b != NONE && c != NONE {
                // 代表の頂点に置き換えると三角形が裏返ることがある。元の三角形と同じ向きにそろえる
                // （そうしないと、裏面として暗く描かれる点々が出る）
                if dot3(tri_normal(&mesh.positions, [a, b, c]), tri_normal(&mesh.positions, [t[0], t[1], t[2]])) < 0.0 {
                    out.extend_from_slice(&[a, c, b]);
                } else {
                    out.extend_from_slice(&[a, b, c]);
                }
            }
        }
        out
    };
    // まず粗く作って三角形の数を見てから、目安の数になる細かさで作り直す（曲面の三角形の数は、細かさの2乗に比例する）
    let mut cells = 40.0f32;
    let mut lod = build(cells);
    let got = (lod.len() / 3).max(1);
    if got < LOD_TARGET_TRIANGLES / 2 {
        cells = (cells * (LOD_TARGET_TRIANGLES as f32 / got as f32).sqrt()).min(200.0);
        lod = build(cells);
    }
    // 粗くしても減らないメッシュ（細かい破片の集まりなど）は、そのまま描く
    if lod.len() < mesh.indices.len() / 2 && !lod.is_empty() {
        mesh.lod = lod;
    }
}

/// 複素数の値をとる陽関数の表し方。色は偏角 arg f で付け、高さは次のどれか
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexMode {
    /// 色分けしない（実数にならない点は描かない）
    Off,
    /// 高さ = |f|
    Abs,
    /// 高さ = Re f
    Re,
    /// 高さ = Im f
    Im,
}

impl ComplexMode {
    pub fn from_u32(k: u32) -> ComplexMode {
        match k {
            1 => ComplexMode::Abs,
            2 => ComplexMode::Re,
            3 => ComplexMode::Im,
            _ => ComplexMode::Off,
        }
    }
}

impl Mesh {
    fn push_vertex(&mut self, p: [f64; 3]) -> u32 {
        let k = (self.positions.len() / 3) as u32;
        self.positions.extend(p.iter().map(|&v| v as f32));
        k
    }
}

const NONE: u32 = u32::MAX;

// ---------------------------------------------------------------------------
// 陽関数
// ---------------------------------------------------------------------------

/// 陽関数の曲面（例: z = f(x, y)）を、`n × n` マスの格子で作る。
///
/// `axis` は関数の値になる座標（z = f(x, y) なら 2）。
/// `prog` の変数は残り2つの座標を x, y, z の順に並べたもの。
/// 値が定義されない点（NaN・無限大）や、表示範囲から大きく外れた点は省く。
pub fn explicit_surface(prog: &Program, dom: Domain, n: usize, axis: usize) -> Mesh {
    explicit_surface_with(prog, dom, n, axis, ComplexMode::Off)
}

/// [`explicit_surface`] で、複素数の値を `mode` の高さと偏角の色で表すもの
pub fn explicit_surface_with(prog: &Program, dom: Domain, n: usize, axis: usize, mode: ComplexMode) -> Mesh {
    explicit_surface_full(prog, dom, n, axis, mode, false)
}

/// 複素数の色分けで頂点に持たせる値 (Re z/|z|, Im z/|z|, log₂|z|)（色相は偏角、明るさの縞は |z|）
fn complex_attr(z: C) -> [f32; 3] {
    let r = z.norm();
    let (c, s) = if r > 0.0 { (z.re / r, z.im / r) } else { (0.0, 0.0) };
    [c as f32, s as f32, r.log2().max(-1e6) as f32]
}

/// 複素数の値 z を、`mode` の表し方での高さにする
fn height(z: C, mode: ComplexMode) -> f64 {
    let w = match mode {
        ComplexMode::Off => as_real(z),
        ComplexMode::Abs => z.norm(),
        ComplexMode::Re => z.re,
        ComplexMode::Im => z.im,
    };
    if z.re.is_finite() && z.im.is_finite() { w } else { f64::NAN }
}

/// 格子の辺の上で値が飛ぶ所（floor の段差、tan の極など）
#[derive(Debug, Clone, Copy)]
struct Jump {
    /// 辺の上の位置（0: 始点, 1: 終点）
    t: f64,
    /// 飛ぶ直前（始点の側）と直後（終点の側）の値
    before: C,
    after: C,
}

/// 陽関数の曲面。値が飛ぶ所（段差）を見つけて、そこで三角形を切り分ける（段差の縁がギザギザにならない）。
/// `walls` なら、段差の上下の縁を結ぶ縦の壁も描く。
///
/// 段差の見つけ方: 両端の高さの差が大きい辺を二分法で 12 回狭めていき、差がほとんど縮まらなければ
/// 「値が飛んでいる」とみなす（なめらかな関数なら、区間を半分にすると差もおよそ半分になる）。
/// 三角形の3辺のうち2辺で値が飛んでいれば、飛ぶ所を結ぶ線で三角形を2つの部分に分け、それぞれの側の値で描く。
/// 1辺・3辺のとき（段差の線が三角形の中で終わるなど）は、その三角形を描かない。
pub fn explicit_surface_full(
    prog: &Program,
    dom: Domain,
    n: usize,
    axis: usize,
    mode: ComplexMode,
    walls: bool,
) -> Mesh {
    let n = n.max(2);
    let m = n + 1; // 1辺の頂点数
    let (ua, va) = others(axis);

    // 格子点の座標を用意
    let mut us = Vec::with_capacity(m * m);
    let mut vs = Vec::with_capacity(m * m);
    for j in 0..m {
        for i in 0..m {
            us.push(dom.at(ua, i as f64, n));
            vs.push(dom.at(va, j as f64, n));
        }
    }

    // まとめて計算
    let mut vals = vec![C::new(0.0, 0.0); m * m];
    prog.eval_batch(&[&us, &vs], &mut vals);
    let hs: Vec<f64> = vals.iter().map(|&z| height(z, mode)).collect();

    // 表示範囲の上下に、範囲の高さ1つ分の余裕を持たせる。
    // はみ出た部分は描画側（Three.js のクリッピング）で切り落とす。
    // 範囲の外の値は、範囲の少し外（wlo, whi）にそろえて描く（極の近くで頂点を捨てると、その周りの三角形が
    // 抜けて穴が空くため）。はみ出た部分は描画側で切り落とされるので見えない
    let h = dom.hi[axis] - dom.lo[axis];
    let (wlo, whi) = (dom.lo[axis] - h, dom.hi[axis] + h);
    let usable = |w: f64| w.is_finite();
    let clamped = |z: C| height(z, mode).clamp(wlo, whi);
    // 段差を探すのは、範囲にそろえた高さで（両側とも +∞ に向かう極は段差ではない。+∞ から −∞ に飛ぶ極は段差）
    let hs_clamped: Vec<f64> = hs.iter().map(|&w| w.clamp(wlo, whi)).collect();

    // 格子の辺: 0 = 右 (i, j)–(i+1, j)、1 = 上 (i, j)–(i, j+1)、2 = 斜め (i, j)–(i+1, j+1)
    let edge_end = |k: usize, e: usize| match e {
        0 => k + 1,
        1 => k + m,
        _ => k + m + 1,
    };
    let edge_id = |k: usize, e: usize| 3 * k + e;
    let jumps = find_jumps(prog, &us, &vs, &vals, &hs_clamped, m, &clamped, h * 0.03, &usable);

    let mut mesh = Mesh::default();
    let complex = mode != ComplexMode::Off;
    let push = |mesh: &mut Mesh, u: f64, v: f64, z: C| -> u32 {
        let mut p = [0.0; 3];
        p[ua] = u;
        p[va] = v;
        p[axis] = height(z, mode).clamp(wlo, whi);
        if complex {
            mesh.complex.extend_from_slice(&complex_attr(z));
        }
        mesh.push_vertex(p)
    };

    // 使える頂点だけに番号を振る
    let mut index_of = vec![NONE; m * m];
    for k in 0..m * m {
        if usable(hs[k]) {
            index_of[k] = push(&mut mesh, us[k], vs[k], vals[k]);
        }
    }
    // 値が飛ぶ辺の上の2つの頂点（始点の側・終点の側）。隣り合う三角形で共有する
    let mut jump_vertex: std::collections::HashMap<usize, (u32, u32)> = std::collections::HashMap::new();
    for (&id, jp) in &jumps {
        let (k, e) = (id / 3, id % 3);
        let k2 = edge_end(k, e);
        let (u, v) = (us[k] + (us[k2] - us[k]) * jp.t, vs[k] + (vs[k2] - vs[k]) * jp.t);
        let a = push(&mut mesh, u, v, jp.before);
        let b = push(&mut mesh, u, v, jp.after);
        jump_vertex.insert(id, (a, b));
    }

    // 法線が axis の正の向きになるように三角形の向きを決める
    // （(ua, va, axis) が x→y→z の順の巡回なら、そのままで正の向き）
    let flip = (ua + 1) % 3 != va;
    let mut wall_quads: Vec<[(f64, f64, C, C); 2]> = Vec::new();

    // 各マスを2つの三角形に分ける。3頂点とも使えるものだけ残す。
    for j in 0..n {
        for i in 0..n {
            let k = j * m + i;
            let (a, b, c, d) = (k, k + 1, k + m, k + m + 1);
            // 三角形の頂点（格子点の番号）と、各辺 (p→q) の辺の番号・向き（格子の辺の向きと同じなら true）
            let tris = [
                ([a, b, d], [(edge_id(a, 0), true), (edge_id(b, 1), true), (edge_id(a, 2), false)]),
                ([a, d, c], [(edge_id(a, 2), true), (edge_id(c, 0), false), (edge_id(a, 1), false)]),
            ];
            for (vtx, edges) in tris {
                if vtx.iter().any(|&p| index_of[p] == NONE) {
                    continue;
                }
                let jumped: Vec<usize> = (0..3).filter(|&e| jumps.contains_key(&edges[e].0)).collect();
                let emit = |mesh: &mut Mesh, t: [u32; 3]| {
                    mesh.indices.extend_from_slice(&if flip { [t[0], t[2], t[1]] } else { t });
                };
                match jumped.len() {
                    0 => emit(&mut mesh, vtx.map(|p| index_of[p])),
                    2 => {
                        // 2つの辺に共通の頂点が、段差の片側に1つだけある頂点。
                        // 辺 e は頂点 e → e+1 なので、飛ばない辺 ne の反対の頂点は ne + 2
                        let ne = (0..3).find(|e| !jumped.contains(e)).unwrap_or(0);
                        let lone = (ne + 2) % 3;
                        // lone 番目の頂点の前後の辺: 辺 (lone−1) は lone に入る辺、辺 lone は lone から出る辺
                        let e_in = (lone + 2) % 3;
                        let e_out = lone;
                        // 辺の上の頂点のうち、指定した側（lone の側か、反対側か）のもの
                        let side = |e: usize, lone_side: bool| -> u32 {
                            let (id, forward) = edges[e];
                            let (start, end) = jump_vertex[&id];
                            // 三角形の辺の向きで、lone は辺 e_out の始点・辺 e_in の終点
                            let lone_is_start = (e == e_out) == forward;
                            if lone_is_start == lone_side { start } else { end }
                        };
                        let p = vtx[lone];
                        let q = vtx[(lone + 1) % 3];
                        let r = vtx[(lone + 2) % 3];
                        // lone の側: 三角形 (lone, 辺 e_out の点, 辺 e_in の点)
                        emit(&mut mesh, [index_of[p], side(e_out, true), side(e_in, true)]);
                        // 反対側: 四角形 (辺 e_out の点, q, r, 辺 e_in の点)
                        let (x, y) = (side(e_out, false), side(e_in, false));
                        emit(&mut mesh, [x, index_of[q], index_of[r]]);
                        emit(&mut mesh, [x, index_of[r], y]);
                        if walls {
                            let info = |e: usize| {
                                let (id, forward) = edges[e];
                                let jp = jumps[&id];
                                let (k0, k1) = (id / 3, edge_end(id / 3, id % 3));
                                let (u, v) = (us[k0] + (us[k1] - us[k0]) * jp.t, vs[k0] + (vs[k1] - vs[k0]) * jp.t);
                                let lone_is_start = (e == e_out) == forward;
                                let (near, far) =
                                    if lone_is_start { (jp.before, jp.after) } else { (jp.after, jp.before) };
                                (u, v, near, far)
                            };
                            wall_quads.push([info(e_out), info(e_in)]);
                        }
                    }
                    1 => {
                        // 段差の線が三角形の中で終わる（極のそばの急な所など）: 飛ぶ辺の上の点と向かいの頂点を結ぶ線で
                        // 2つに分け、それぞれの側の値で描く（段差は向かいの頂点に向かって細くなる）。
                        // 描かないと、そこだけ穴が空く
                        let e = jumped[0];
                        let (id, forward) = edges[e];
                        let (start, end) = jump_vertex[&id];
                        // 三角形の辺 e は 頂点 e → e+1。格子の辺と同じ向きなら、頂点 e の側が始点の側
                        let (near_p, near_q) = if forward { (start, end) } else { (end, start) };
                        let (p, q, w) = (vtx[e], vtx[(e + 1) % 3], vtx[(e + 2) % 3]);
                        emit(&mut mesh, [index_of[p], near_p, index_of[w]]);
                        emit(&mut mesh, [near_q, index_of[q], index_of[w]]);
                    }
                    _ => {} // 3辺とも飛ぶ: 描かない
                }
            }
        }
    }
    compute_normals(&mut mesh);

    // 段差の壁: 段差の線に沿って、lone の側の縁と反対側の縁を結ぶ縦の四角形（頂点は共有しない）
    for [(u0, v0, n0, f0), (u1, v1, n1, f1)] in wall_quads {
        let base = mesh.positions.len() as u32 / 3;
        for (u, v, z) in [(u0, v0, n0), (u1, v1, n1), (u1, v1, f1), (u0, v0, f0)] {
            push(&mut mesh, u, v, z);
        }
        mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        // 法線: 段差の線の向き × 縦の向き（どちら向きでも両面を描くので、明るさが合えばよい）
        let mut dir = [0.0; 3];
        dir[ua] = u1 - u0;
        dir[va] = v1 - v0;
        let mut up = [0.0; 3];
        up[axis] = 1.0;
        let nv = [
            dir[1] * up[2] - dir[2] * up[1],
            dir[2] * up[0] - dir[0] * up[2],
            dir[0] * up[1] - dir[1] * up[0],
        ];
        let len = (nv[0] * nv[0] + nv[1] * nv[1] + nv[2] * nv[2]).sqrt().max(1e-300);
        for _ in 0..4 {
            mesh.normals.extend(nv.iter().map(|c| (c / len) as f32));
        }
    }
    mesh
}

/// 格子の辺のうち、値が飛ぶものを探す（[`explicit_surface_full`] を参照）。
/// 戻り値は 辺の番号（3 × 始点 + 向き）→ 飛ぶ所
#[allow(clippy::too_many_arguments)]
fn find_jumps(
    prog: &Program,
    us: &[f64],
    vs: &[f64],
    vals: &[C],
    hs: &[f64],
    m: usize,
    height: &dyn Fn(C) -> f64,
    threshold: f64,
    usable: &dyn Fn(f64) -> bool,
) -> std::collections::HashMap<usize, Jump> {
    const ROUNDS: usize = 12;
    // 候補: 両端が使えて、高さの差が大きい辺
    struct Cand {
        id: usize,
        k0: usize,
        k1: usize,
        ta: f64,
        tb: f64,
        fa: C,
        fb: C,
        first: f64,
    }
    let mut cands = Vec::new();
    for j in 0..m {
        for i in 0..m {
            let k = j * m + i;
            for e in 0..3 {
                let (ok_i, ok_j) = (if e == 1 { true } else { i + 1 < m }, if e == 0 { true } else { j + 1 < m });
                if !ok_i || !ok_j {
                    continue;
                }
                let k1 = match e {
                    0 => k + 1,
                    1 => k + m,
                    _ => k + m + 1,
                };
                if !usable(hs[k]) || !usable(hs[k1]) {
                    continue;
                }
                let d = (hs[k1] - hs[k]).abs();
                if d > threshold {
                    cands.push(Cand { id: 3 * k + e, k0: k, k1, ta: 0.0, tb: 1.0, fa: vals[k], fb: vals[k1], first: d });
                }
            }
        }
    }
    let mut mu = vec![0.0; cands.len()];
    let mut mv = vec![0.0; cands.len()];
    let mut out = vec![C::new(0.0, 0.0); cands.len()];
    for _ in 0..ROUNDS {
        for (c, (a, b)) in cands.iter().zip(mu.iter_mut().zip(mv.iter_mut())) {
            let t = 0.5 * (c.ta + c.tb);
            *a = us[c.k0] + (us[c.k1] - us[c.k0]) * t;
            *b = vs[c.k0] + (vs[c.k1] - vs[c.k0]) * t;
        }
        prog.eval_batch(&[&mu, &mv], &mut out);
        for (c, &z) in cands.iter_mut().zip(&out) {
            let t = 0.5 * (c.ta + c.tb);
            let (ha, hb, hm) = (height(c.fa), height(c.fb), height(z));
            // 差の大きいほうの半分に狭める（真ん中が定義されなければ、始点の側に寄せる）
            if !hm.is_finite() || (hm - ha).abs() >= (hb - hm).abs() {
                c.tb = t;
                c.fb = z;
            } else {
                c.ta = t;
                c.fa = z;
            }
        }
    }
    let mut jumps = std::collections::HashMap::new();
    for c in cands {
        let d = (height(c.fb) - height(c.fa)).abs();
        // 区間を 1/4096 にしても差が半分以上残っていれば、値が飛んでいる
        // （どちらかの側が定義されない所は段差とはみなさない）
        if d.is_finite() && d > 0.5 * c.first {
            jumps.insert(c.id, Jump { t: 0.5 * (c.ta + c.tb), before: c.fa, after: c.fb });
        }
    }
    jumps
}

// ---------------------------------------------------------------------------
// 陰関数（Surface Nets 法）
//
// 1. 空間を n × n × n 個の小さな立方体（セル）に分け、格子点で F を計算する
// 2. 角の F の符号がそろっていないセル（= 曲面が通るセル）に頂点を1つ置く。
//    位置は「辺上で F = 0 になる点」の平均
// 3. 両端で F の符号が違う格子の辺ごとに、その辺を囲む4つのセルの頂点を
//    四角形（三角形2つ）でつなぐ
// ---------------------------------------------------------------------------

/// 立方体の8つの角（0/1 で各軸の位置を表す）
const CORNERS: [[usize; 3]; 8] = [
    [0, 0, 0],
    [1, 0, 0],
    [0, 1, 0],
    [1, 1, 0],
    [0, 0, 1],
    [1, 0, 1],
    [0, 1, 1],
    [1, 1, 1],
];

/// 立方体の12本の辺（CORNERS の番号の組）
const EDGES: [[usize; 2]; 12] = [
    [0, 1],
    [2, 3],
    [4, 5],
    [6, 7],
    [0, 2],
    [1, 3],
    [4, 6],
    [5, 7],
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
];

/// 陰関数の曲面 F(x, y, z) = 0 を作る。`prog` の変数は x, y, z。
pub fn implicit_surface(prog: &Program, dom: Domain, n: usize) -> Mesh {
    implicit_solid(prog, dom, n, &[], 0)
}

/// 範囲で重ねた立体: 変数 r₁, r₂, … をそれぞれ `ranges` の範囲で動かしたとき、
/// 曲面 F(x, y, z, r₁, r₂, …) = 0 が通る領域の境界面を作る。
/// `prog` の変数は x, y, z, r₁, r₂, …。`ranges` が空なら、ふつうの陰関数の曲面。
///
/// F が r について連続なら、点 p が領域に入る ⇔ min_r F(p, r) ≤ 0 ≤ max_r F(p, r)。
/// そこで G(p) = max(min_r F, −max_r F) を作ると、領域の内側で G ≤ 0 になるので、G = 0 の面を作ればよい。
/// min, max は、範囲をそれぞれ `samples` 等分した点での値から求める（近似）。
pub fn implicit_solid(prog: &Program, dom: Domain, n: usize, ranges: &[(f64, f64)], samples: usize) -> Mesh {
    // 表示範囲の外側に1マスずつ広げて計算する。Surface Nets 法は範囲の端で曲面が
    // ギザギザに途切れるので、少しはみ出して作っておき、端は描画側で真っ直ぐ切り落とす。
    let n = n.max(2);
    let mut dom = dom;
    for ax in 0..3 {
        let step = (dom.hi[ax] - dom.lo[ax]) / n as f64;
        dom.lo[ax] -= step;
        dom.hi[ax] += step;
    }
    let n = n + 2;
    let m = n + 1; // 1辺の格子点の数
    let idx = |i: usize, j: usize, k: usize| i + m * (j + m * k);

    // 1. 格子点で F を計算（メモリ節約のため z の層ごとに計算し、f32 で保存）
    let mut f = vec![0.0f32; m * m * m];
    {
        let mut xs = Vec::with_capacity(m * m);
        let mut ys = Vec::with_capacity(m * m);
        for j in 0..m {
            for i in 0..m {
                xs.push(dom.at(0, i as f64, n));
                ys.push(dom.at(1, j as f64, n));
            }
        }
        let mut zs = vec![0.0; m * m];
        let combos = sample_combos(ranges, samples);
        for k in 0..m {
            zs.fill(dom.at(2, k as f64, n));
            let layer = &mut f[k * m * m..(k + 1) * m * m];
            for (dst, v) in layer.iter_mut().zip(field_values(prog, &combos, [&xs, &ys, &zs])) {
                *dst = v as f32;
            }
        }
    }

    // 1'. 符号は変わらないが F = 0 に接している所（例: (x² + y² + z² − 1)² = 0）を探す
    let touch = if ranges.is_empty() { find_touches(prog, &f, dom, n) } else { Default::default() };

    // 格子の辺 p → p + e_a の上で曲面と交わる位置 t（0〜1）と、F の符号（接するだけの所は None）
    let crossing = |p: [usize; 3], a: usize| -> Option<(f64, Option<bool>)> {
        let mut q = p;
        q[a] += 1;
        let (f0, f1) = (f[idx(p[0], p[1], p[2])], f[idx(q[0], q[1], q[2])]);
        if !f0.is_finite() || !f1.is_finite() {
            return None;
        }
        if (f0 < 0.0) != (f1 < 0.0) {
            return Some(((f0 / (f0 - f1)) as f64, Some(f0 < 0.0)));
        }
        if touch.is_empty() {
            return None;
        }
        touch.get(&(3 * idx(p[0], p[1], p[2]) + a)).map(|&t| (t as f64, None))
    };

    // 2. 曲面が通るセルに頂点を置く。位置は「辺の上で曲面と交わる点」の平均。
    //    1つのセルを2枚以上の曲面が通るとき（近い曲面どうし）は、1つの頂点にまとめると曲面がくっつくので、
    //    曲面ごとに頂点を分ける: 角のうち少ないほうの符号の角を、セルの辺でつながっているものどうしで組に分け、
    //    交わる辺はその少ないほうの符号の端の角の組に属させる（組ごとに曲面が1枚）。
    //    接するだけの辺（符号が変わらない）は、まとめて1つの組にする。
    //    cell_slots[vert[セル]][辺の番号] が、その辺に属する頂点
    let cell = |i: usize, j: usize, k: usize| i + n * (j + n * k);
    let mut vert = vec![NONE; n * n * n];
    let mut cell_slots: Vec<[u32; 12]> = Vec::new();
    let mut mesh = Mesh::default();
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                let corner_neg = CORNERS.map(|d| f[idx(i + d[0], j + d[1], k + d[2])] < 0.0);
                // 角の符号がそろっていて、接するだけの所も無ければ、曲面は通らない（ほとんどのセルはここで終わる）
                let neg = corner_neg.iter().filter(|&&b| b).count();
                if touch.is_empty() && (neg == 0 || neg == 8) {
                    continue;
                }
                // 少ないほうの符号の角を、セルの辺でつながっているものどうしで組に分ける
                let minority = neg <= 4;
                let mut group = [usize::MAX; 8];
                let mut groups = 0;
                for c in 0..8 {
                    if corner_neg[c] != minority || group[c] != usize::MAX {
                        continue;
                    }
                    let mut stack = vec![c];
                    group[c] = groups;
                    while let Some(x) = stack.pop() {
                        for [ca, cb] in EDGES {
                            let y = if ca == x { cb } else if cb == x { ca } else { continue };
                            if corner_neg[y] == minority && group[y] == usize::MAX {
                                group[y] = groups;
                                stack.push(y);
                            }
                        }
                    }
                    groups += 1;
                }
                // 接するだけの辺（符号が変わらない）は、そのセルを通る曲面のどれか1枚の一部。
                // 符号の変わる辺もあるセルでは組に分けず、1つの頂点にまとめる（前と同じ）
                let has_touch = !touch.is_empty()
                    && EDGES.iter().any(|&[ca, cb]| {
                        let a = (0..3).find(|&x| CORNERS[ca][x] != CORNERS[cb][x]).unwrap_or(0);
                        let base = CORNERS[ca];
                        matches!(crossing([i + base[0], j + base[1], k + base[2]], a), Some((_, None)))
                    });
                if has_touch && groups > 0 {
                    for g in group.iter_mut().filter(|g| **g != usize::MAX) {
                        *g = 0;
                    }
                    groups = 1;
                }
                let touch_group = if groups > 0 { 0 } else { groups }; // 接するだけの辺の組
                // 交わる辺ごとの組と、交わる点（セルの中の位置）
                let mut hits: Vec<(usize, usize, [f64; 3])> = Vec::with_capacity(12);
                for (e, [ca, cb]) in EDGES.iter().copied().enumerate() {
                    let a = (0..3).find(|&x| CORNERS[ca][x] != CORNERS[cb][x]).unwrap_or(0);
                    let base = CORNERS[ca];
                    if let Some((t, sign)) = crossing([i + base[0], j + base[1], k + base[2]], a) {
                        let g = match sign {
                            Some(_) => group[if corner_neg[ca] == minority { ca } else { cb }],
                            None => touch_group,
                        };
                        let mut q = [base[0] as f64, base[1] as f64, base[2] as f64];
                        q[a] += t;
                        hits.push((e, g, q));
                    }
                }
                let mut sums = vec![([0.0f64; 3], 0.0f64); groups + 1];
                let mut edge_group = [usize::MAX; 12];
                for &(e, g, q) in &hits {
                    edge_group[e] = g;
                    let (sum, cnt) = &mut sums[g];
                    for ax in 0..3 {
                        sum[ax] += q[ax];
                    }
                    *cnt += 1.0;
                }
                let mut ids = vec![NONE; groups + 1];
                for (g, (sum, cnt)) in sums.iter().enumerate() {
                    if *cnt > 0.0 {
                        let p = [i as f64 + sum[0] / cnt, j as f64 + sum[1] / cnt, k as f64 + sum[2] / cnt];
                        ids[g] = mesh.push_vertex([dom.at(0, p[0], n), dom.at(1, p[1], n), dom.at(2, p[2], n)]);
                    }
                }
                if ids.iter().all(|&v| v == NONE) {
                    continue;
                }
                vert[cell(i, j, k)] = cell_slots.len() as u32;
                cell_slots.push(edge_group.map(|g| if g == usize::MAX { NONE } else { ids[g] }));
            }
        }
    }

    // セルの中の辺の番号: 向き a、セルの角 (0/1) から見た始点の位置 base
    let local_edge = |a: usize, base: [usize; 3]| -> usize {
        EDGES
            .iter()
            .position(|&[ca, cb]| CORNERS[ca] == base && (0..3).all(|x| (CORNERS[cb][x] != base[x]) == (x == a)))
            .unwrap_or(0)
    };

    // 3. 曲面と交わる格子の辺ごとに、周りの4セルの（その辺に属する）頂点を四角形でつなぐ
    let mut touch_tris: Vec<u32> = Vec::new();
    for a in 0..3 {
        let (b, c) = ((a + 1) % 3, (a + 2) % 3);
        // 周りの4セル（b-c 平面で反時計回り）から見た、この辺の番号
        let locals = [(1, 1), (0, 1), (0, 0), (1, 0)].map(|(db, dc)| {
            let mut base = [0; 3];
            base[b] = db;
            base[c] = dc;
            local_edge(a, base)
        });
        for k in 0..m {
            for j in 0..m {
                for i in 0..m {
                    let p = [i, j, k];
                    // 辺 p → p + e_a。周りの4セルが全部格子内にある辺だけを見る
                    if p[a] >= n || p[b] == 0 || p[b] >= n || p[c] == 0 || p[c] >= n {
                        continue;
                    }
                    let Some((_, sign)) = crossing(p, a) else { continue };
                    let mut around = [NONE; 4];
                    for (q, ((db, dc), le)) in [(1, 1), (0, 1), (0, 0), (1, 0)].iter().zip(locals).enumerate() {
                        let mut s = p;
                        s[b] -= db;
                        s[c] -= dc;
                        let slot = vert[cell(s[0], s[1], s[2])];
                        if slot != NONE {
                            around[q] = cell_slots[slot as usize][le];
                        }
                    }
                    if around.contains(&NONE) {
                        continue;
                    }
                    let [q0, q1, q2, q3] = around;
                    match sign {
                        // 法線が F の増える向きを向くように三角形の向きを決める
                        Some(true) => mesh.indices.extend_from_slice(&[q0, q1, q2, q0, q2, q3]),
                        Some(false) => mesh.indices.extend_from_slice(&[q0, q2, q1, q0, q3, q2]),
                        // 接するだけの所は、後で向きをそろえる
                        None => touch_tris.extend_from_slice(&[q0, q1, q2, q0, q2, q3]),
                    }
                }
            }
        }
    }
    if !touch_tris.is_empty() {
        orient_consistently(&mesh.positions, &mut touch_tris);
        mesh.indices.extend_from_slice(&touch_tris);
    }

    compute_normals(&mut mesh);
    mesh
}

/// 符号が変わらずに F = 0 に接している所を、格子の辺ごとに探す。
/// 戻り値は 辺の番号（3 × 始点の番号 + 向き）→ 辺の上の位置 t（0〜1）。
///
/// 辺 p → q（= p + e）について、|F| が p−e から p へ減り、q から q+e へ増えていて、
/// 内側の2点の |F| が外側の2点よりずっと小さければ、その間に |F| の極小がある。
/// 三分探索で極小の位置を求め、その値がほぼ 0 なら「接している」とみなす。
fn find_touches(prog: &Program, f: &[f32], dom: Domain, n: usize) -> std::collections::HashMap<usize, f32> {
    const ROUNDS: usize = 16;
    let m = n + 1;
    let idx = |p: [usize; 3]| p[0] + m * (p[1] + m * p[2]);
    // 候補: (辺の番号, 始点 p, 向き a, F の符号, 外側の |F| の平均)
    let mut cands: Vec<(usize, [usize; 3], usize, f64, f64)> = Vec::new();
    for a in 0..3 {
        let stride = [1, m, m * m][a];
        for k in 0..m {
            for j in 0..m {
                for i in 0..m {
                    let p = [i, j, k];
                    if p[a] < 1 || p[a] + 2 > n {
                        continue;
                    }
                    let c = idx(p);
                    let v = [f[c - stride], f[c], f[c + stride], f[c + 2 * stride]];
                    // まず安い判定: 内側の2点が外側より 0 に近い
                    if !(v[1].abs() < v[0].abs() && v[2].abs() < v[3].abs()) {
                        continue;
                    }
                    if v.iter().any(|x| !x.is_finite()) {
                        continue;
                    }
                    // 4点とも同じ側（0 は内側の2点だけ許す）
                    let s = v[0].signum();
                    if v[0] == 0.0 || v[3] * s <= 0.0 || v[1] * s < 0.0 || v[2] * s < 0.0 {
                        continue;
                    }
                    let g = v.map(|x| x.abs() as f64);
                    if g[1] + g[2] < 0.25 * (g[0] + g[3]) {
                        cands.push((3 * c + a, p, a, s as f64, 0.5 * (g[0] + g[3])));
                    }
                }
            }
        }
    }
    if cands.is_empty() {
        return Default::default();
    }
    // 三分探索（すべての候補をまとめて計算する）。区間は辺の前後1マスずつを含めた [−1, 2]
    let mut lo = vec![-1.0f64; cands.len()];
    let mut hi = vec![2.0f64; cands.len()];
    let len = 2 * cands.len();
    let mut cols: [Vec<f64>; 3] = [vec![0.0; len], vec![0.0; len], vec![0.0; len]];
    let mut out = vec![C::new(0.0, 0.0); len];
    let fill = |cols: &mut [Vec<f64>; 3], ts: &dyn Fn(usize) -> [f64; 2]| {
        for (c, &(_, p, a, _, _)) in cands.iter().enumerate() {
            let t2 = ts(c);
            for (h, t) in t2.iter().enumerate() {
                for ax in 0..3 {
                    let g = if ax == a { p[ax] as f64 + t } else { p[ax] as f64 };
                    cols[ax][2 * c + h] = dom.at(ax, g, n);
                }
            }
        }
    };
    for _ in 0..ROUNDS {
        fill(&mut cols, &|c| {
            let d = (hi[c] - lo[c]) / 3.0;
            [lo[c] + d, hi[c] - d]
        });
        prog.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
        for c in 0..cands.len() {
            // 符号をそろえた s F の極小を探す（定義されない点は大きい値とみなす）
            let s = cands[c].3;
            let val = |z: C| {
                let v = s * as_real(z);
                if v.is_nan() { f64::INFINITY } else { v }
            };
            let (g1, g2) = (val(out[2 * c]), val(out[2 * c + 1]));
            let d = (hi[c] - lo[c]) / 3.0;
            if g1 > g2 {
                lo[c] += d;
            } else {
                hi[c] -= d;
            }
        }
    }
    fill(&mut cols, &|c| {
        let t = 0.5 * (lo[c] + hi[c]);
        [t, t]
    });
    prog.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut out);
    let mut touch = std::collections::HashMap::new();
    for (c, &(id, _, _, s, outer)) in cands.iter().enumerate() {
        let t = 0.5 * (lo[c] + hi[c]);
        let g = s * as_real(out[2 * c]);
        // 極小がほぼ 0 なら接している。はっきり負なら、マスの中で2回交わっているだけ（接していない）。
        // 隣の辺と同じ極小を2回数えないよう、t が [0, 1) に入る辺だけに付ける
        if (0.0..1.0).contains(&t) && g.abs() <= 1e-3 * outer {
            touch.insert(id, t as f32);
        }
    }
    touch
}

/// 三角形の向き（表裏）をそろえる。辺を共有する三角形どうしが、その辺を逆向きにたどるように裏返していく。
/// つながった部分ごとに、重心から見て囲む体積（向き付き）が負なら全体を裏返す（閉じた曲面なら外向きになる）
fn orient_consistently(positions: &[f32], tris: &mut [u32]) {
    use std::collections::HashMap;
    let count = tris.len() / 3;
    // 辺（小さい番号, 大きい番号）→ その辺を持つ三角形
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for t in 0..count {
        for e in 0..3 {
            let (u, v) = (tris[3 * t + e], tris[3 * t + (e + 1) % 3]);
            edges.entry((u.min(v), u.max(v))).or_default().push(t);
        }
    }
    // 三角形 t が辺 u → v をこの向きにたどるか
    let has = |tris: &[u32], t: usize, u: u32, v: u32| (0..3).any(|e| tris[3 * t + e] == u && tris[3 * t + (e + 1) % 3] == v);
    let mut done = vec![false; count];
    for start in 0..count {
        if done[start] {
            continue;
        }
        done[start] = true;
        let mut comp = vec![start];
        let mut stack = vec![start];
        while let Some(t) = stack.pop() {
            for e in 0..3 {
                let (u, v) = (tris[3 * t + e], tris[3 * t + (e + 1) % 3]);
                let Some(list) = edges.get(&(u.min(v), u.max(v))) else { continue };
                if list.len() != 2 {
                    continue; // 3つ以上の三角形が集まる辺では、向きを決められない
                }
                for &o in list {
                    if o == t || done[o] {
                        continue;
                    }
                    // 隣は同じ辺を逆向き (v → u) にたどるべき
                    if has(tris, o, u, v) {
                        tris.swap(3 * o + 1, 3 * o + 2);
                    }
                    done[o] = true;
                    comp.push(o);
                    stack.push(o);
                }
            }
        }
        // 囲む体積（向き付き）が負なら裏返す
        let p = |k: u32| {
            let k = k as usize * 3;
            [positions[k] as f64, positions[k + 1] as f64, positions[k + 2] as f64]
        };
        // 基準点は部分の頂点の重心（開いた曲面でも、筒なら外向きになるように）
        let mut o = [0.0; 3];
        for &t in &comp {
            for e in 0..3 {
                let q = p(tris[3 * t + e]);
                for x in 0..3 {
                    o[x] += q[x] / (3 * comp.len()) as f64;
                }
            }
        }
        let mut vol = 0.0;
        for &t in &comp {
            let [a, b, c] = [p(tris[3 * t]), p(tris[3 * t + 1]), p(tris[3 * t + 2])].map(|q| [q[0] - o[0], q[1] - o[1], q[2] - o[2]]);
            vol += a[0] * (b[1] * c[2] - b[2] * c[1]) + a[1] * (b[2] * c[0] - b[0] * c[2]) + a[2] * (b[0] * c[1] - b[1] * c[0]);
        }
        if vol < 0.0 {
            for &t in &comp {
                tris.swap(3 * t + 1, 3 * t + 2);
            }
        }
    }
}

/// 点 (xs, ys, zs) での F の値。範囲で重ねる立体なら G = max(min F, −max F)（[`implicit_solid`] を参照）。
/// `combos` は範囲で動かす変数の値の組（[`sample_combos`]）。範囲が無ければ `[[]]`
fn field_values(prog: &Program, combos: &[Vec<f64>], [xs, ys, zs]: [&[f64]; 3]) -> Vec<f64> {
    let len = xs.len();
    let mut out = vec![C::new(0.0, 0.0); len];
    if combos.len() == 1 && combos[0].is_empty() {
        prog.eval_batch(&[xs, ys, zs], &mut out);
        return out.iter().map(|v| as_real(*v)).collect();
    }
    // 範囲の各点で F を計算して、最小値と最大値をとる
    let mut lo = vec![f64::INFINITY; len];
    let mut hi = vec![f64::NEG_INFINITY; len];
    let mut extra = vec![vec![0.0; len]; combos.first().map_or(0, |c| c.len())];
    for combo in combos {
        for (arr, &v) in extra.iter_mut().zip(combo) {
            arr.fill(v);
        }
        let mut inputs: Vec<&[f64]> = vec![xs, ys, zs];
        inputs.extend(extra.iter().map(|a| a.as_slice()));
        prog.eval_batch(&inputs, &mut out);
        for p in 0..len {
            let v = as_real(out[p]);
            if v.is_finite() {
                lo[p] = lo[p].min(v);
                hi[p] = hi[p].max(v);
            }
        }
    }
    (0..len).map(|p| if lo[p].is_finite() { lo[p].max(-hi[p]) } else { f64::NAN }).collect()
}

/// 陰関数・範囲で重ねた立体の「中身」。切り口のふたを作るのに使う
#[derive(Debug, Clone)]
pub struct Field {
    /// F（変数は表示中の3つの座標と、範囲で動かす変数）
    pub prog: Program,
    /// 範囲で動かす変数の値の組（範囲が無ければ `[[]]`）
    pub combos: Vec<Vec<f64>>,
    /// F < 0 の側が中身なら true（F > 0 の側なら false）
    pub negative: bool,
}

impl Field {
    /// 範囲で重ねた立体なら G ≤ 0 の側が中身。陰関数 F = 0 は、どちらの側が中身か決まっていないので、
    /// 表示範囲の中で狭いほう（例: 球なら内側）を中身とする
    pub fn new(prog: &Program, ranges: &[(f64, f64)], samples: usize, dom: Domain) -> Field {
        let combos = sample_combos(ranges, samples);
        let mut negative = true;
        if ranges.is_empty() {
            let n = 16;
            let (mut xs, mut ys, mut zs) = (Vec::new(), Vec::new(), Vec::new());
            for k in 0..n {
                for j in 0..n {
                    for i in 0..n {
                        // マスの中心（表示範囲の面の上は避ける）
                        xs.push(dom.at(0, i as f64 + 0.5, n));
                        ys.push(dom.at(1, j as f64 + 0.5, n));
                        zs.push(dom.at(2, k as f64 + 0.5, n));
                    }
                }
            }
            let v = field_values(prog, &combos, [&xs, &ys, &zs]);
            let neg = v.iter().filter(|&&f| f < 0.0).count();
            let pos = v.iter().filter(|&&f| f > 0.0).count();
            negative = neg <= pos;
        }
        Field { prog: prog.clone(), combos, negative }
    }

    /// 平面「`axis` 番目の座標 = `pos`」での切り口（中身の部分）を、`n × n` マスの格子で三角形にする。
    /// 法線は、切り落とした側（`keep_below` なら座標が大きい側）を向く
    pub fn cap(&self, dom: Domain, axis: usize, pos: f64, keep_below: bool, n: usize) -> Mesh {
        let n = n.max(2);
        let m = n + 1;
        let (ua, va) = others(axis);
        let mut cols: [Vec<f64>; 3] = [vec![0.0; m * m], vec![0.0; m * m], vec![0.0; m * m]];
        for j in 0..m {
            for i in 0..m {
                cols[ua][j * m + i] = dom.at(ua, i as f64, n);
                cols[va][j * m + i] = dom.at(va, j as f64, n);
                cols[axis][j * m + i] = pos;
            }
        }
        let f = field_values(&self.prog, &self.combos, [&cols[0], &cols[1], &cols[2]]);
        // 中身なら負になる値
        let g: Vec<f64> = if self.negative { f } else { f.iter().map(|v| -v).collect() };
        let point = |k: usize| [cols[0][k], cols[1][k], cols[2][k]];

        let mut mesh = Mesh::default();
        // 各マスを2つの三角形に分け、三角形ごとに中身の部分（g < 0）を切り出す（値は辺の上で直線で補間）
        for j in 0..n {
            for i in 0..n {
                let (a, b, c, d) = (j * m + i, j * m + i + 1, (j + 1) * m + i, (j + 1) * m + i + 1);
                for tri in [[a, b, d], [a, d, c]] {
                    if tri.iter().any(|&k| !g[k].is_finite()) || tri.iter().all(|&k| g[k] >= 0.0) {
                        continue;
                    }
                    let mut poly: Vec<[f64; 3]> = Vec::with_capacity(4);
                    for e in 0..3 {
                        let (p, q) = (tri[e], tri[(e + 1) % 3]);
                        if g[p] < 0.0 {
                            poly.push(point(p));
                        }
                        if (g[p] < 0.0) != (g[q] < 0.0) {
                            let t = g[p] / (g[p] - g[q]);
                            let (pp, pq) = (point(p), point(q));
                            poly.push([0, 1, 2].map(|x| pp[x] + (pq[x] - pp[x]) * t));
                        }
                    }
                    let base = mesh.positions.len() as u32 / 3;
                    for p in &poly {
                        mesh.push_vertex(*p);
                    }
                    for k in 1..poly.len() as u32 - 1 {
                        mesh.indices.extend_from_slice(&[base, base + k, base + k + 1]);
                    }
                }
            }
        }
        // 三角形の向き: (ua, va, axis) が x→y→z の順の巡回なら、そのままで +axis の向き
        let up = (ua + 1) % 3 == va;
        if up != keep_below {
            for t in mesh.indices.chunks_exact_mut(3) {
                t.swap(1, 2);
            }
        }
        let mut nrm = [0.0f32; 3];
        nrm[axis] = if keep_below { 1.0 } else { -1.0 };
        mesh.normals = (0..mesh.positions.len() / 3).flat_map(|_| nrm).collect();
        mesh
    }
}

/// 範囲 `ranges` の中から、計算に使う点の組を作る。各範囲を等分し、組の総数は 4096 個まで
fn sample_combos(ranges: &[(f64, f64)], samples: usize) -> Vec<Vec<f64>> {
    if ranges.is_empty() {
        return vec![vec![]];
    }
    let k = ranges.len() as f64;
    let per = ((samples.max(2) as f64).min(4096f64.powf(1.0 / k))).floor().max(2.0) as usize;
    let mut combos: Vec<Vec<f64>> = vec![vec![]];
    for &(a, b) in ranges {
        let vals: Vec<f64> = (0..per).map(|s| a + (b - a) * s as f64 / (per - 1) as f64).collect();
        combos = combos
            .into_iter()
            .flat_map(|c| {
                vals.iter().map(move |&v| {
                    let mut c2 = c.clone();
                    c2.push(v);
                    c2
                })
            })
            .collect();
    }
    combos
}

// ---------------------------------------------------------------------------
// 媒介変数表示
// ---------------------------------------------------------------------------

/// n 次元の点の集まり。媒介変数表示を、回転・射影する前の形で持っておく。
/// 回転や表示する軸が変わっても、式を計算し直さずに [`project`] だけで描き直せる。
#[derive(Debug, Clone, Default)]
pub struct NdShape {
    /// 点の次元
    pub dims: usize,
    /// 点の座標（dims 個ずつ並べる）。定義されない点・線の切れ目は NaN
    pub points: Vec<f64>,
    /// 曲面なら、格子の1辺の点の数 m と、各点の媒介変数 (u − u₀, v − v₀)。
    /// 曲面が何枚あってもよい（m × m 個ずつの点が続けて並ぶ）
    pub grid: Option<(usize, Vec<f32>)>,
    /// 媒介変数表示の立体なら true。曲面は6枚ずつで1つの立体（媒介変数の直方体の面）になっていて、
    /// `grid` には点ごとに3つの媒介変数の値 (u − u₀, v − v₀, t − t₀) が入る（その面で固定した媒介変数は [`NO_GRID`]）
    pub solid: bool,
    /// 複素数の色分けをするなら、点ごとの (Re w/|w|, Im w/|w|, log₂|w|)（[`Mesh::complex`] と同じ）。しないなら空
    pub complex: Vec<f32>,
}

/// 格子線を引かない成分の印（シェーダーはこれより小さい値の成分には線を引かない）
pub const NO_GRID: f32 = -1e30;

/// 各成分をまとめて計算し、n 次元の点を並べたもの（点ごとに d 個）にする。
/// どれかの成分が実数にならない点は NaN
fn eval_nd(set: &ProgramSet, inputs: &[&[f64]], n: usize) -> Vec<f64> {
    let mut pts = Vec::with_capacity(n * set.len());
    set.eval_points(inputs, n, &mut pts);
    pts
}

fn finite(p: &[f64]) -> bool {
    p.iter().all(|v| v.is_finite())
}

fn dist(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt()
}

/// 媒介変数表示の曲面を、`n × n` マスの格子で計算する。`sets` の1つ1つが1枚の曲面（n 次元の成分の式）
pub fn parametric_surface_nd(sets: &[Vec<Program>], u: (f64, f64), v: (f64, f64), n: usize) -> NdShape {
    let n = n.max(2);
    let m = n + 1;
    let mut us = Vec::with_capacity(m * m);
    let mut vs = Vec::with_capacity(m * m);
    let mut grid1 = Vec::with_capacity(m * m * 2);
    for j in 0..m {
        for i in 0..m {
            let (a, b) = (u.0 + (u.1 - u.0) * i as f64 / n as f64, v.0 + (v.1 - v.0) * j as f64 / n as f64);
            us.push(a);
            vs.push(b);
            grid1.extend_from_slice(&[(a - u.0) as f32, (b - v.0) as f32]);
        }
    }
    let dims = sets.first().map_or(0, |p| p.len());
    let mut points = Vec::with_capacity(sets.len() * m * m * dims);
    let mut grid = Vec::with_capacity(sets.len() * grid1.len());
    for progs in sets {
        ProgramSet::new(progs).eval_points(&[&us, &vs], m * m, &mut points);
        grid.extend_from_slice(&grid1);
    }
    NdShape { dims, points, grid: Some((m, grid)), solid: false, complex: Vec::new() }
}

/// 媒介変数表示の立体（媒介変数 3 つ）。媒介変数の範囲の直方体の6つの面を、それぞれ `n × n` マスの曲面として計算する。
///
/// 面の向きは、写像がそのまま（向きを変えない）なら外向きになるように並べる。
/// 媒介変数 k を固定した面では、残りの2つを (k+1, k+2)（3 で割った余り）の順に動かすと、
/// ∂p/∂a × ∂p/∂b が +k の向きになる。最小値の側の面は外向きが −k なので、2つの順を入れ替える。
/// 射影した後の向きは [`project`] で立体ごとに確かめて、裏返っていれば直す。
pub fn parametric_solid_nd(sets: &[Vec<Program>], r: [(f64, f64); 3], n: usize) -> NdShape {
    let n = n.max(2);
    let m = n + 1;
    let at = |k: usize, i: usize| r[k].0 + (r[k].1 - r[k].0) * i as f64 / n as f64;
    let dims = sets.first().map_or(0, |p| p.len());
    let count = 6 * m * m;
    let merged: Vec<ProgramSet> = sets.iter().map(|progs| ProgramSet::new(progs)).collect();
    let mut points = Vec::with_capacity(sets.len() * count * dims);
    // 6つの面の格子線用の値（立体ごとに同じ）
    let mut grid1 = Vec::with_capacity(count * 3);
    let mut faces = Vec::with_capacity(6);
    for k in 0..3 {
        for side in [0, 1] {
            let (a, b) = if side == 1 { ((k + 1) % 3, (k + 2) % 3) } else { ((k + 2) % 3, (k + 1) % 3) };
            let fixed = if side == 1 { r[k].1 } else { r[k].0 };
            let mut cols: [Vec<f64>; 3] = Default::default();
            for j in 0..m {
                for i in 0..m {
                    let mut p = [0.0; 3];
                    p[k] = fixed;
                    p[a] = at(a, i);
                    p[b] = at(b, j);
                    cols[a].push(p[a]);
                    cols[b].push(p[b]);
                    for c in 0..3 {
                        grid1.push(if c == k { NO_GRID } else { (p[c] - r[c].0) as f32 });
                    }
                }
            }
            faces.push((k, fixed, cols));
        }
    }
    let mut grid = Vec::with_capacity(sets.len() * grid1.len());
    for set in &merged {
        for (k, fixed, cols) in &faces {
            // その面で固定した媒介変数は定数にする（それだけで決まる部分は1回だけ計算すればよい）
            set.bind(&[(*k, *fixed)]).eval_points(&[&cols[0], &cols[1], &cols[2]], m * m, &mut points);
        }
        grid.extend_from_slice(&grid1);
    }
    NdShape { dims, points, grid: Some((m, grid)), solid: true, complex: Vec::new() }
}

/// 媒介変数が4つ以上の図形の、面の枚数と、1枚の格子のマス数。
/// `(i, j)` の組ごとに、残りの媒介変数を `div + 1` 個の値に固定した面を描くので、
/// 面は kC2 · (div + 1)^(k − 2) 枚。多すぎるときは `div` を減らす（1 なら範囲の両端だけ）。
/// 頂点の数の合計がおよそ 6 · `resolution`² になるよう、面が多いほど1枚を粗くする
pub fn sheet_layout(k: usize, div: usize, resolution: usize) -> (usize, usize, usize) {
    const MAX_SHEETS: usize = 2400;
    let pairs = k * (k - 1) / 2;
    let count = |d: usize| pairs.saturating_mul((d + 1).saturating_pow(k as u32 - 2));
    let mut div = div.max(1);
    while div > 1 && count(div) > MAX_SHEETS {
        div -= 1;
    }
    let sheets = count(div);
    let n = ((6.0 * (resolution * resolution) as f64 / sheets as f64).sqrt() as usize).clamp(3, resolution.max(3));
    (div, sheets, n)
}

/// 媒介変数表示で、媒介変数が4つ以上（k 個）の図形。3次元の画面には中身をそのまま描けないので、
/// 媒介変数を2つだけ動かした曲面（面）をたくさん描いて形を表す。
///
/// 媒介変数の組 (a, b) ごとに、残りの k − 2 個を、範囲を `div` 等分した値（`div` = 1 なら両端だけ）の
/// すべての組み合わせに固定する。`div` = 1 のとき、これは媒介変数の範囲の「k 次元の直方体」の
/// 2次元の面（4次元の立方体なら 24 枚）の像になる。`div` を増やすと、中を通る面も描く。
///
/// 面ごとに、固定した媒介変数を定数にした式を作って計算する（[`ProgramSet::bind`]）。
/// 格子線用の値は、面ごとに (a, b) を 0〜1 にそろえたもの
pub fn parametric_sheets_nd(sets: &[Vec<Program>], r: &[(f64, f64)], div: usize, resolution: usize) -> NdShape {
    let k = r.len();
    assert!(k >= 3);
    let (div, sheets, n) = sheet_layout(k, div, resolution);
    let m = n + 1;
    let dims = sets.first().map_or(0, |p| p.len());
    let merged: Vec<ProgramSet> = sets.iter().map(|progs| ProgramSet::new(progs)).collect();
    let mut points = Vec::with_capacity(sets.len() * sheets * m * m * dims);
    let mut grid1 = Vec::with_capacity(m * m * 2);
    for j in 0..m {
        for i in 0..m {
            grid1.extend_from_slice(&[i as f32 / n as f32, j as f32 / n as f32]);
        }
    }
    let mut grid = Vec::with_capacity(sets.len() * sheets * grid1.len());
    let empty: &[f64] = &[];
    for set in &merged {
        for a in 0..k {
            for b in a + 1..k {
                let at = |c: usize, i: usize| r[c].0 + (r[c].1 - r[c].0) * i as f64 / n as f64;
                let us: Vec<f64> = (0..m * m).map(|p| at(a, p % m)).collect();
                let vs: Vec<f64> = (0..m * m).map(|p| at(b, p / m)).collect();
                let mut inputs = vec![empty; k];
                inputs[a] = &us;
                inputs[b] = &vs;
                let others: Vec<usize> = (0..k).filter(|&c| c != a && c != b).collect();
                // 残りの媒介変数の値の組み合わせ（それぞれ div + 1 通り）を順にたどる
                let mut step = vec![0usize; others.len()];
                loop {
                    let fixed: Vec<(usize, f64)> = others
                        .iter()
                        .zip(&step)
                        .map(|(&c, &s)| (c, r[c].0 + (r[c].1 - r[c].0) * s as f64 / div as f64))
                        .collect();
                    set.bind(&fixed).eval_points(&inputs, m * m, &mut points);
                    grid.extend_from_slice(&grid1);
                    let mut c = 0;
                    while c < step.len() {
                        step[c] += 1;
                        if step[c] <= div {
                            break;
                        }
                        step[c] = 0;
                        c += 1;
                    }
                    if c == step.len() {
                        break;
                    }
                }
            }
        }
    }
    NdShape { dims, points, grid: Some((m, grid)), solid: false, complex: Vec::new() }
}

/// 陽関数の曲面（表示中の `axis` 番目の座標 = f(残り2つ)）を、n 次元の点として計算する。
/// 回転が表示中の座標どうしだけのときに、媒介変数表示 (u, v, f(u, v)) と同じように回すために使う。
///
/// - `axes`: 表示中の3つの座標の番号、`base`: 各座標の値（表示していない座標は断面の値）
/// - `u`, `v`: 残り2つの座標を動かす範囲
///
/// 格子線用の座標には、u, v の値そのもの（元の座標の値）を入れる。回転しても、格子線は元の座標の
/// きりの良い値の所に引かれる。
#[allow(clippy::too_many_arguments)]
pub fn explicit_nd(
    prog: &Program,
    axis: usize,
    axes: [usize; 3],
    base: &[f64],
    u: (f64, f64),
    v: (f64, f64),
    n: usize,
    mode: ComplexMode,
) -> NdShape {
    let n = n.max(2);
    let m = n + 1;
    let (ua, va) = others(axis);
    let d = base.len();
    let mut us = Vec::with_capacity(m * m);
    let mut vs = Vec::with_capacity(m * m);
    let mut grid = Vec::with_capacity(m * m * 2);
    for j in 0..m {
        for i in 0..m {
            let (a, b) = (u.0 + (u.1 - u.0) * i as f64 / n as f64, v.0 + (v.1 - v.0) * j as f64 / n as f64);
            us.push(a);
            vs.push(b);
            grid.extend_from_slice(&[a as f32, b as f32]);
        }
    }
    let mut vals = vec![C::new(0.0, 0.0); m * m];
    prog.eval_batch(&[&us, &vs], &mut vals);
    let mut points = Vec::with_capacity(m * m * d);
    let mut complex = Vec::new();
    for k in 0..m * m {
        let mut p = base.to_vec();
        p[axes[ua]] = us[k];
        p[axes[va]] = vs[k];
        // 複素数の色分けをするなら、高さは |f|・Re f・Im f（色は偏角）
        p[axes[axis]] = height(vals[k], mode);
        points.extend_from_slice(&p);
        if mode != ComplexMode::Off {
            complex.extend_from_slice(&complex_attr(vals[k]));
        }
    }
    NdShape { dims: d, points, grid: Some((m, grid)), solid: false, complex }
}

/// 媒介変数表示の曲線（成分は `progs`、n 次元）。
///
/// 最初に `n` 等分して計算し、曲がっている所・途切れる所だけを細かく分け直す（適応サンプリング）。
/// 曲線が途切れる所には NaN の点が入る。`tol` は「直線で結んだときのずれ」の許容量（n 次元の距離）。
/// 回転して3次元に射影しても距離は縮むだけなので、射影した後もこの許容量に収まる。
pub fn parametric_curve_nd(sets: &[Vec<Program>], t: (f64, f64), n: usize, tol: f64) -> NdShape {
    let dims = sets.first().map_or(0, |p| p.len());
    let mut points = Vec::new();
    for (k, progs) in sets.iter().enumerate() {
        if k > 0 {
            points.extend(std::iter::repeat_n(f64::NAN, dims)); // 曲線どうしの間で線を切る
        }
        let set = ProgramSet::new(progs);
        points.extend(curve_nd(&|ts: &[f64]| eval_nd(&set, &[ts], ts.len()), progs.len(), t, n, tol));
    }
    NdShape { dims, points, grid: None, solid: false, complex: Vec::new() }
}

/// 1本の曲線の点（適応サンプリング）。`eval` は媒介変数の値の列から、d 次元の点を並べたものを計算する
fn curve_nd(eval: &dyn Fn(&[f64]) -> Vec<f64>, d: usize, t: (f64, f64), n: usize, tol: f64) -> Vec<f64> {
    const MAX_POINTS: usize = 200_000;
    const PASSES: usize = 14;
    let split = |flat: Vec<f64>| -> Vec<Vec<f64>> { flat.chunks_exact(d).map(|c| c.to_vec()).collect() };

    let n = n.max(2);
    let mut ts: Vec<f64> = (0..=n).map(|k| t.0 + (t.1 - t.0) * k as f64 / n as f64).collect();
    let mut ps = split(eval(&ts));
    // 区間 [ts[k], ts[k+1]] をまだ分け直す必要があるか
    let mut todo = vec![true; ts.len() - 1];

    for _ in 0..PASSES {
        // 分け直す候補の区間の中点をまとめて計算する
        let cand: Vec<usize> =
            (0..todo.len()).filter(|&k| todo[k] && (finite(&ps[k]) || finite(&ps[k + 1]))).collect();
        if cand.is_empty() || ts.len() + cand.len() > MAX_POINTS {
            break;
        }
        let mids: Vec<f64> = cand.iter().map(|&k| 0.5 * (ts[k] + ts[k + 1])).collect();
        let mps = split(eval(&mids));

        // 中点を入れるべきか判定しながら、新しい列を作る
        let mut nts = Vec::with_capacity(ts.len() + cand.len());
        let mut nps = Vec::with_capacity(ts.len() + cand.len());
        let mut ntodo = Vec::with_capacity(ts.len() + cand.len());
        let mut c = 0;
        for k in 0..ts.len() {
            nts.push(ts[k]);
            nps.push(ps[k].clone());
            if k + 1 == ts.len() {
                break;
            }
            if c < cand.len() && cand[c] == k {
                let (a, b, m) = (&ps[k], &ps[k + 1], &mps[c]);
                let insert = if finite(a) && finite(b) && finite(m) {
                    // 中点が、両端を結ぶ線分の中点からどれだけずれているか
                    let mid: Vec<f64> = a.iter().zip(b).map(|(x, y)| (x + y) / 2.0).collect();
                    dist(m, &mid) > tol
                } else {
                    true // 定義されない点との境目を探す
                };
                if insert {
                    nts.push(mids[c]);
                    nps.push(m.clone());
                    ntodo.extend_from_slice(&[true, true]);
                } else {
                    ntodo.push(false);
                }
                c += 1;
            } else {
                ntodo.push(todo[k]);
            }
        }
        ts = nts;
        ps = nps;
        todo = ntodo;
    }

    // 分け直しきれなかった長い区間は、値が飛んでいる（例: tan t の極）とみなして線を切る
    let jump = tol * 200.0;
    let mut points = Vec::with_capacity(ps.len() * d);
    for k in 0..ps.len() {
        if k > 0 && todo[k - 1] && finite(&ps[k - 1]) && finite(&ps[k]) && dist(&ps[k - 1], &ps[k]) > jump {
            points.extend(std::iter::repeat_n(f64::NAN, d));
        }
        points.extend_from_slice(&ps[k]);
    }
    points
}

/// いくつかのメッシュを1つにまとめる（リストで複数の図形になったとき）。曲線なら間に NaN を入れて線を切る
pub fn merge(meshes: Vec<Mesh>, curve: bool) -> Mesh {
    let mut out = Mesh::default();
    let grid = meshes.iter().any(|m| !m.grid_coords.is_empty());
    let complex = meshes.iter().any(|m| !m.complex.is_empty());
    let lod = meshes.iter().any(|m| !m.lod.is_empty());
    for m in meshes {
        if curve && !out.positions.is_empty() {
            out.positions.extend_from_slice(&[f32::NAN; 3]);
        }
        let base = (out.positions.len() / 3) as u32;
        let count = m.positions.len() / 3;
        // 格子線用の座標・複素数の値が無いメッシュは、頂点座標・0 で埋めてそろえる
        if grid {
            if m.grid_coords.is_empty() {
                out.grid_coords.extend_from_slice(&m.positions);
            } else {
                out.grid_coords.extend_from_slice(&m.grid_coords);
            }
        }
        if complex {
            if m.complex.is_empty() {
                out.complex.extend(std::iter::repeat_n(0.0, count * 3));
            } else {
                out.complex.extend_from_slice(&m.complex);
            }
        }
        out.positions.extend_from_slice(&m.positions);
        out.normals.extend_from_slice(&m.normals);
        out.indices.extend(m.indices.iter().map(|k| k + base));
        // 粗い版が無いメッシュは、そのままの三角形を使う
        if lod {
            let src = if m.lod.is_empty() { &m.indices } else { &m.lod };
            out.lod.extend(src.iter().map(|k| k + base));
        }
    }
    out
}

/// n 次元の点の集まりを1つにまとめる。点の並べ方（次元・格子・立体か）が同じでなければ None
pub fn merge_shapes(shapes: Vec<NdShape>) -> Option<NdShape> {
    let first = shapes.first()?;
    let (dims, solid, m) = (first.dims, first.solid, first.grid.as_ref().map(|g| g.0));
    if shapes.iter().any(|s| s.dims != dims || s.solid != solid || s.grid.as_ref().map(|g| g.0) != m) {
        return None;
    }
    let mut out = NdShape { dims, points: Vec::new(), grid: m.map(|m| (m, Vec::new())), solid, complex: Vec::new() };
    for s in shapes {
        if m.is_none() && !out.points.is_empty() {
            out.points.extend(std::iter::repeat_n(f64::NAN, dims)); // 曲線どうしの間で線を切る
        }
        out.points.extend_from_slice(&s.points);
        out.complex.extend_from_slice(&s.complex);
        if let (Some((_, g)), Some((_, sg))) = (out.grid.as_mut(), s.grid) {
            g.extend_from_slice(&sg);
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// 2D 表示（画面の縦の座標を 0 にして、横・奥の平面に描く）
// ---------------------------------------------------------------------------

/// 縦の座標を 0 にして平らにする（2D 表示）。曲面の法線はすべて +縦 の向きにする
pub fn flatten(mesh: &mut Mesh, curve: bool) {
    for p in mesh.positions.chunks_exact_mut(3) {
        if p[2].is_finite() {
            p[2] = 0.0;
        }
    }
    if !curve {
        mesh.normals = (0..mesh.positions.len() / 3).flat_map(|_| [0.0f32, 0.0, 1.0]).collect();
    }
}

/// 2D 表示の `z = f(x, y)`（縦の座標が横・奥の座標の関数）: 平面を f の値で塗る（色の濃さと等高線）。
/// 頂点は (x, y, 0)、格子線用の座標は (x, y, f) なので、z 方向の格子線が等高線になる。
/// 複素数の色分けをするなら、[`explicit_surface_with`] と同じく偏角の色を付ける
pub fn flat_map(prog: &Program, dom: Domain, n: usize, mode: ComplexMode) -> Mesh {
    let n = n.max(2);
    let m = n + 1;
    let mut xs = Vec::with_capacity(m * m);
    let mut ys = Vec::with_capacity(m * m);
    for j in 0..m {
        for i in 0..m {
            xs.push(dom.at(0, i as f64, n));
            ys.push(dom.at(1, j as f64, n));
        }
    }
    let mut vals = vec![C::new(0.0, 0.0); m * m];
    prog.eval_batch(&[&xs, &ys], &mut vals);
    let mut index_of = vec![NONE; m * m];
    let mut mesh = Mesh::default();
    for k in 0..m * m {
        let z = vals[k];
        let w = match mode {
            ComplexMode::Off => as_real(z),
            ComplexMode::Abs => z.norm(),
            ComplexMode::Re => z.re,
            ComplexMode::Im => z.im,
        };
        if !w.is_finite() || !z.re.is_finite() || !z.im.is_finite() {
            continue;
        }
        index_of[k] = mesh.push_vertex([xs[k], ys[k], 0.0]);
        mesh.grid_coords.extend_from_slice(&[xs[k] as f32, ys[k] as f32, w as f32]);
        if mode != ComplexMode::Off {
            let r = z.norm();
            let (c, s) = if r > 0.0 { (z.re / r, z.im / r) } else { (0.0, 0.0) };
            mesh.complex.extend_from_slice(&[c as f32, s as f32, r.log2().max(-1e6) as f32]);
        }
    }
    for j in 0..n {
        for i in 0..n {
            let a = index_of[j * m + i];
            let b = index_of[j * m + i + 1];
            let c = index_of[(j + 1) * m + i];
            let d = index_of[(j + 1) * m + i + 1];
            for tri in [[a, b, d], [a, d, c]] {
                if !tri.contains(&NONE) {
                    mesh.indices.extend_from_slice(&tri);
                }
            }
        }
    }
    flatten(&mut mesh, false);
    mesh
}

/// 2D 表示の陽関数の曲線 `y = f(x)`（`axis` = 1）や `x = f(y)`（`axis` = 0）。
/// `prog` の変数は残り2つの表示中の座標（2つ目は縦の座標で、0 にする）。曲がっている所を細かく計算する
pub fn explicit_curve_2d(prog: &Program, axis: usize, dom: Domain, tol: f64) -> Mesh {
    let free = if axis == 0 { 1 } else { 0 };
    let eval = |ts: &[f64]| -> Vec<f64> {
        let zeros = vec![0.0; ts.len()];
        let mut out = vec![C::new(0.0, 0.0); ts.len()];
        prog.eval_batch(&[ts, &zeros], &mut out);
        let mut pts = Vec::with_capacity(ts.len() * 3);
        for (t, v) in ts.iter().zip(&out) {
            let mut p = [0.0; 3];
            p[free] = *t;
            p[axis] = as_real(*v);
            if !p[axis].is_finite() {
                p = [f64::NAN; 3];
            }
            pts.extend_from_slice(&p);
        }
        pts
    };
    let points = curve_nd(&eval, 3, (dom.lo[free], dom.hi[free]), 400, tol);
    Mesh { positions: points.iter().map(|&v| v as f32).collect(), ..Default::default() }
}

/// 2D 表示の陰関数の曲線 F(x, y, 0) = 0。`prog` の変数は表示中の3つの座標（縦の座標は 0）。
/// 格子の各マスで、辺の上の F = 0 の点を結ぶ（マーチング・スクエア法）。
/// 線分ごとに (p, q, NaN) と並べる（NaN で線を切る）
pub fn implicit_curve_2d(prog: &Program, dom: Domain, n: usize) -> Mesh {
    let n = n.max(2);
    let m = n + 1;
    let mut xs = Vec::with_capacity(m * m);
    let mut ys = Vec::with_capacity(m * m);
    for j in 0..m {
        for i in 0..m {
            xs.push(dom.at(0, i as f64, n));
            ys.push(dom.at(1, j as f64, n));
        }
    }
    let zs = vec![0.0; m * m];
    let mut out = vec![C::new(0.0, 0.0); m * m];
    prog.eval_batch(&[&xs, &ys, &zs], &mut out);
    let f: Vec<f64> = out.iter().map(|v| as_real(*v)).collect();
    let mut pts: Vec<f32> = Vec::new();
    for j in 0..n {
        for i in 0..n {
            // マスの4つの角（反時計回り）
            let ks = [j * m + i, j * m + i + 1, (j + 1) * m + i + 1, (j + 1) * m + i];
            let v = ks.map(|k| f[k]);
            if v.iter().any(|x| !x.is_finite()) {
                continue;
            }
            // 符号が変わる辺の上の F = 0 の点
            let mut cross: Vec<[f64; 2]> = Vec::with_capacity(4);
            for e in 0..4 {
                let (a, b) = (e, (e + 1) % 4);
                if (v[a] < 0.0) != (v[b] < 0.0) {
                    let t = v[a] / (v[a] - v[b]);
                    let (ka, kb) = (ks[a], ks[b]);
                    cross.push([xs[ka] + (xs[kb] - xs[ka]) * t, ys[ka] + (ys[kb] - ys[ka]) * t]);
                }
            }
            let pairs: Vec<(usize, usize)> = match cross.len() {
                2 => vec![(0, 1)],
                4 => {
                    // 鞍点: マスの中心の値で、どちらの組み合わせで結ぶか決める
                    let center = v.iter().sum::<f64>() / 4.0;
                    // 中心が角 0 と同じ側なら、角 0 と 2 がつながっていて、角 1 と 3 が切り離される
                    if (center < 0.0) == (v[0] < 0.0) { vec![(0, 1), (2, 3)] } else { vec![(0, 3), (1, 2)] }
                }
                _ => vec![],
            };
            for (a, b) in pairs {
                pts.extend_from_slice(&[cross[a][0] as f32, cross[a][1] as f32, 0.0]);
                pts.extend_from_slice(&[cross[b][0] as f32, cross[b][1] as f32, 0.0]);
                pts.extend_from_slice(&[f32::NAN; 3]);
            }
        }
    }
    Mesh { positions: pts, ..Default::default() }
}

/// 画面の1つの軸の座標を、n 次元の点の1次式 c₀ + Σ cₖ·pₖ で表したもの（回転と射影）
pub type ViewRow = (f64, Vec<f64>);

/// n 次元の点を、画面の3次元に移す方法（回転・射影・4次元の遠近法）
#[derive(Debug, Clone)]
pub struct View {
    /// 画面の横・奥・縦の座標
    pub rows: [ViewRow; 3],
    /// 回転の中心の、画面の横・奥・縦の座標（遠近法で縮めるときの中心）
    pub center: [f64; 3],
    /// 遠近法: 表示していない座標（回転後）と、その中心の値
    pub hidden: Vec<(ViewRow, f64)>,
    /// 遠近法の視点までの距離（0 なら遠近法なし）
    pub dist: f64,
}

impl View {
    /// 回転なし・遠近法なしで、最初の3つの成分をそのまま画面の軸にする
    #[cfg(test)]
    pub fn identity(d: usize) -> View {
        View { rows: identity_rows(d), center: [0.0; 3], hidden: Vec::new(), dist: 0.0 }
    }

    /// n 次元の点 p を画面の3次元の点にする。視点より後ろ（または近すぎる）点は NaN
    fn apply(&self, p: &[f64]) -> [f64; 3] {
        let row = |(c0, cs): &ViewRow| c0 + cs.iter().zip(p).map(|(c, x)| c * x).sum::<f64>();
        let q = self.rows.each_ref().map(row);
        if self.hidden.is_empty() {
            return q;
        }
        let mut s = 1.0;
        for (r, c) in &self.hidden {
            let depth = self.dist - (row(r) - c);
            if depth < self.dist * 0.05 {
                return [f64::NAN; 3];
            }
            s *= self.dist / depth;
        }
        [0, 1, 2].map(|a| self.center[a] + (q[a] - self.center[a]) * s)
    }
}

/// n 次元の点の集まりを、`view`（回転・射影・4次元の遠近法）で3次元に移してメッシュにする。
/// 曲面なら三角形と法線も作る。曲線なら点の列（切れ目は NaN）だけ。
pub fn project(shape: &NdShape, view: &View, dom: Domain) -> Mesh {
    let d = shape.dims;
    let count = shape.points.len().checked_div(d).unwrap_or(0);
    let to3 = |k: usize| view.apply(&shape.points[k * d..(k + 1) * d]);

    let Some((m, grid)) = &shape.grid else {
        // 曲線: 点をそのまま並べる（NaN は線の切れ目として残る）
        let positions = (0..count).flat_map(|k| to3(k).map(|v| v as f32)).collect();
        return Mesh { positions, ..Default::default() };
    };

    // 表示範囲から大きく外れた点（無限大の近くなど）は省く。はみ出た部分は描画側で切り落とす。
    let ok = |p: &[f64; 3]| {
        finite(p)
            && (0..3).all(|a| {
                let h = dom.hi[a] - dom.lo[a];
                p[a] >= dom.lo[a] - h && p[a] <= dom.hi[a] + h
            })
    };
    let m = *m;
    let n = m - 1;
    let gd = if shape.solid { 3 } else { 2 };
    let mut index_of = vec![NONE; count];
    let mut mesh = Mesh::default();
    for (k, slot) in index_of.iter_mut().enumerate() {
        let p = to3(k);
        if ok(&p) {
            *slot = mesh.push_vertex(p);
            if !shape.complex.is_empty() {
                mesh.complex.extend_from_slice(&shape.complex[3 * k..3 * k + 3]);
            }
            let g = &grid[gd * k..gd * k + gd];
            mesh.grid_coords.extend_from_slice(&[g[0], g[1], if shape.solid { g[2] } else { p[2] as f32 }]);
        }
    }
    // 曲面ごとに m × m 個の点が並んでいる。立体は6枚ずつ
    let per = if shape.solid { 6 * m * m } else { m * m };
    // 粗い版: 格子の点を stride 個おきに使う（最後の行・列は必ず入れるので、面の端は変わらない）
    let quads = count / (m * m) * n * n;
    let stride = if 2 * quads > LOD_MIN_TRIANGLES {
        ((2 * quads) as f64 / LOD_TARGET_TRIANGLES as f64).sqrt().ceil() as usize
    } else {
        0
    };
    let picks: Vec<usize> = if stride > 1 {
        let mut v: Vec<usize> = (0..n).step_by(stride).collect();
        v.push(n);
        v
    } else {
        Vec::new()
    };
    for start in (0..count).step_by(per) {
        let first = mesh.indices.len();
        let lod_first = mesh.lod.len();
        for base in (start..start + per).step_by(m * m) {
            for js in picks.windows(2) {
                for is in picks.windows(2) {
                    let a = index_of[base + js[0] * m + is[0]];
                    let b = index_of[base + js[0] * m + is[1]];
                    let c = index_of[base + js[1] * m + is[0]];
                    let e = index_of[base + js[1] * m + is[1]];
                    for tri in [[a, b, e], [a, e, c]] {
                        if !tri.contains(&NONE) {
                            mesh.lod.extend_from_slice(&tri);
                        }
                    }
                }
            }
            for j in 0..n {
                for i in 0..n {
                    let a = index_of[base + j * m + i];
                    let b = index_of[base + j * m + i + 1];
                    let c = index_of[base + (j + 1) * m + i];
                    let e = index_of[base + (j + 1) * m + i + 1];
                    for tri in [[a, b, e], [a, e, c]] {
                        if !tri.contains(&NONE) {
                            mesh.indices.extend_from_slice(&tri);
                        }
                    }
                }
            }
        }
        // 立体: 面で囲まれた体積（向き付き）が負なら裏返っている（写像や射影が向きを変えた）ので、三角形の向きを直す
        if shape.solid && signed_volume(&mesh, first) < 0.0 {
            for t in mesh.indices[first..].chunks_exact_mut(3) {
                t.swap(1, 2);
            }
            for t in mesh.lod[lod_first..].chunks_exact_mut(3) {
                t.swap(1, 2);
            }
        }
    }
    compute_normals(&mut mesh);
    mesh
}

/// `first` 番目以降の三角形で囲まれた体積（向き付き。面が外向きなら正）。Σ a · (b × c) / 6
fn signed_volume(mesh: &Mesh, first: usize) -> f64 {
    let p = &mesh.positions;
    let v = |k: u32| {
        let k = k as usize * 3;
        [p[k] as f64, p[k + 1] as f64, p[k + 2] as f64]
    };
    // 原点の代わりに最初の頂点を基準にする（桁落ちを防ぐ）
    let Some(&k0) = mesh.indices.get(first) else { return 0.0 };
    let o = v(k0);
    let mut sum = 0.0;
    for t in mesh.indices[first..].chunks_exact(3) {
        let [a, b, c] = [v(t[0]), v(t[1]), v(t[2])].map(|q| [q[0] - o[0], q[1] - o[1], q[2] - o[2]]);
        sum += a[0] * (b[1] * c[2] - b[2] * c[1]) + a[1] * (b[2] * c[0] - b[0] * c[2]) + a[2] * (b[0] * c[1] - b[1] * c[0]);
    }
    sum / 6.0
}

/// 回転なしで、最初の3つの成分をそのまま画面の軸にする射影
#[cfg(test)]
fn identity_rows(d: usize) -> [ViewRow; 3] {
    [0, 1, 2].map(|a| (0.0, (0..d).map(|k| (k == a) as u8 as f64).collect()))
}

// ---------------------------------------------------------------------------

/// 周りの三角形の向き（面積で重み付け）を平均して、頂点の法線を求める。
fn compute_normals(mesh: &mut Mesh) {
    let p = &mesh.positions;
    let mut nrm = vec![0.0f64; p.len()];
    for t in mesh.indices.chunks_exact(3) {
        let v = |k: u32| {
            let k = k as usize * 3;
            [p[k] as f64, p[k + 1] as f64, p[k + 2] as f64]
        };
        let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        // 外積 e1 × e2（長さが三角形の面積の2倍なので、そのまま足すと面積の重み付けになる）
        let cr = [
            e1[1] * e2[2] - e1[2] * e2[1],
            e1[2] * e2[0] - e1[0] * e2[2],
            e1[0] * e2[1] - e1[1] * e2[0],
        ];
        for &k in t {
            let k = k as usize * 3;
            for d in 0..3 {
                nrm[k + d] += cr[d];
            }
        }
    }
    mesh.normals = nrm
        .chunks_exact(3)
        .flat_map(|n| {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let s = if len > 0.0 { 1.0 / len } else { 0.0 };
            [(n[0] * s) as f32, (n[1] * s) as f32, (n[2] * s) as f32]
        })
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;
    const AXES: [&str; 3] = ["x", "y", "z"];
    use crate::parser::parse;

    fn dom() -> Domain {
        Domain { lo: [-1.0; 3], hi: [1.0; 3] }
    }
    fn prog(src: &str, vars: &[&str]) -> Program {
        Program::compile(&parse(src).unwrap(), vars).unwrap()
    }

    #[test]
    fn plane() {
        let m = explicit_surface(&prog("0", &["x", "y"]), dom(), 4, 2);
        assert_eq!(m.positions.len(), 25 * 3);
        assert_eq!(m.indices.len(), 4 * 4 * 2 * 3);
        // 平面 z = 0 の法線は (0, 0, 1)
        assert!((m.normals[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn explicit_other_axes_point_positive() {
        // y = 0 の法線は +y、x = 0 の法線は +x
        let m = explicit_surface(&prog("0", &["x", "z"]), dom(), 4, 1);
        assert!((m.normals[1] - 1.0).abs() < 1e-6);
        let m = explicit_surface(&prog("0", &["y", "z"]), dom(), 4, 0);
        assert!((m.normals[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn complex_coloring() {
        // f = x + iy: 高さ |f| は円錐、色は偏角。y < 0 でも描かれる（Off なら実数の y = 0 の所だけ）
        let p = prog("x + i y", &["x", "y"]);
        let m = explicit_surface_with(&p, dom(), 10, 2, ComplexMode::Abs);
        assert_eq!(m.positions.len(), 121 * 3);
        assert_eq!(m.complex.len(), m.positions.len());
        for (q, c) in m.positions.chunks_exact(3).zip(m.complex.chunks_exact(3)) {
            let r = (q[0] * q[0] + q[1] * q[1]).sqrt();
            assert!((q[2] - r).abs() < 1e-5);
            if r > 0.0 {
                assert!((c[0] - q[0] / r).abs() < 1e-5 && (c[1] - q[1] / r).abs() < 1e-5);
            }
        }
        assert!(explicit_surface(&p, dom(), 10, 2).indices.is_empty());
        // 高さ = Im f = y
        let m = explicit_surface_with(&p, dom(), 4, 2, ComplexMode::Im);
        assert!(m.positions.chunks_exact(3).all(|q| (q[2] - q[1]).abs() < 1e-6));
    }

    #[test]
    fn steps() {
        // floor(2x) の段差: 段差の所で三角形を切り分けるので、どの三角形も段差をまたがない
        // （3頂点の値がすべて同じ整数）。段差の縁は x = 0.5 k の線の上にそろう
        let d = Domain { lo: [-1.0; 3], hi: [1.0; 3] };
        let p = prog("floor(2x)", &["x", "y"]);
        let m = explicit_surface(&p, d, 13, 2);
        assert!(!m.indices.is_empty());
        let mut area = 0.0;
        for t in m.indices.chunks_exact(3) {
            let v = |k: u32| {
                let k = k as usize * 3;
                [m.positions[k] as f64, m.positions[k + 1] as f64, m.positions[k + 2] as f64]
            };
            let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
            assert!(a[2] == b[2] && b[2] == c[2], "段差をまたぐ三角形 {a:?} {b:?} {c:?}");
            area += ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) / 2.0;
            for q in [a, b, c] {
                // 頂点の x は、段差のすぐ手前か格子点
                let k = (2.0 * q[0]).floor();
                assert!(q[2] == k || (q[2] - k).abs() == 1.0 && (2.0 * q[0] - (2.0 * q[0]).round()).abs() < 1e-3);
            }
        }
        // 段差で抜けた所が無い: 面積は 2 × 2 = 4（向きもすべて上向き）
        assert!((area - 4.0).abs() < 1e-3, "面積 {area}");
        // 壁を付けると、縦の四角形が増える
        let w = explicit_surface_full(&p, d, 13, 2, ComplexMode::Off, true);
        assert!(w.indices.len() > m.indices.len());
        // なめらかな急な関数は段差とみなさない
        let p = prog("20 x", &["x", "y"]);
        let full = explicit_surface(&p, Domain { lo: [-1.0, -1.0, -30.0], hi: [1.0, 1.0, 30.0] }, 10, 2);
        assert_eq!(full.positions.len(), 11 * 11 * 3);
    }

    /// 三角形でつながった部分（曲面）の数
    fn pieces(m: &Mesh) -> usize {
        let n = m.positions.len() / 3;
        let mut parent: Vec<usize> = (0..n).collect();
        fn root(p: &mut [usize], x: usize) -> usize {
            let mut r = x;
            while p[r] != r {
                r = p[r];
            }
            p[x] = r;
            r
        }
        for t in m.indices.chunks_exact(3) {
            for e in [(t[0], t[1]), (t[1], t[2])] {
                let (a, b) = (root(&mut parent, e.0 as usize), root(&mut parent, e.1 as usize));
                parent[a] = b;
            }
        }
        let used: std::collections::HashSet<usize> = m.indices.iter().map(|&k| k as usize).collect();
        used.iter().map(|&k| root(&mut parent, k)).collect::<std::collections::HashSet<_>>().len()
    }

    #[test]
    fn close_surfaces_stay_apart() {
        // 半径 1 と 1.15 の2つの球（間は 0.15 = 格子の間隔の 1.5 倍）。1つのセルを両方の球が通る所があるが、くっつかずに2枚になる
        let p = prog("(x^2 + y^2 + z^2 - 1) (x^2 + y^2 + z^2 - 1.3225)", &AXES);
        let m = implicit_surface(&p, Domain { lo: [-2.0; 3], hi: [2.0; 3] }, 40);
        assert_eq!(pieces(&m), 2);
        // 頂点はどれも、どちらかの球の上にある（2つの球の間の、くっついた所に頂点が無い）
        for q in m.positions.chunks_exact(3) {
            let r = ((q[0] * q[0] + q[1] * q[1] + q[2] * q[2]) as f64).sqrt();
            assert!((r - 1.0).abs() < 0.02 || (r - 1.15).abs() < 0.02, "r = {r}");
        }
    }

    #[test]
    fn complex_pole_has_no_holes() {
        // Re(1/(x + iy)) は原点の極で +∞ から −∞ に飛ぶ。範囲の外の値は範囲の少し外にそろえて描くので、
        // 極の周りに穴が空かない（平面に落とした面積が、表示範囲の面積 4 にほぼ等しい）
        let d = Domain { lo: [-1.0, -1.0, -3.0], hi: [1.0, 1.0, 3.0] };
        let p = prog("1 / (x + i y)", &["x", "y"]);
        let m = explicit_surface_full(&p, d, 40, 2, ComplexMode::Re, false);
        let mut area = 0.0;
        for t in m.indices.chunks_exact(3) {
            let v = |k: u32| [m.positions[k as usize * 3] as f64, m.positions[k as usize * 3 + 1] as f64];
            let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
            area += ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0;
        }
        assert!((area - 4.0).abs() < 0.02, "面積 {area}");
        // 色分け用の値は頂点ごとに3つ
        assert_eq!(m.complex.len(), m.positions.len());
    }

    #[test]
    fn rotated_explicit_keeps_complex_colors() {
        // 回した陽関数（表示中の座標どうしの回転）でも、高さを |f| にして色分けの値を持つ
        let p = prog("(x + i y)^2", &["x", "y"]);
        let shape = explicit_nd(&p, 2, [0, 1, 2], &[0.0; 3], (-1.0, 1.0), (-1.0, 1.0), 4, ComplexMode::Abs);
        assert_eq!(shape.complex.len(), shape.points.len()); // 3次元の点1つにつき3つ
        for q in shape.points.chunks_exact(3) {
            assert!((q[2] - (q[0] * q[0] + q[1] * q[1])).abs() < 1e-9); // |(x+iy)²| = x² + y²
        }
    }

    #[test]
    fn undefined_region_is_removed() {
        // x < 0 では sqrt(x) が実数にならないので、そこは三角形が無くなる
        let full = explicit_surface(&prog("0", &["x", "y"]), dom(), 10, 2);
        let half = explicit_surface(&prog("sqrt(x)", &["x", "y"]), dom(), 10, 2);
        assert!(half.indices.len() < full.indices.len());
        assert!(!half.indices.is_empty());
    }

    #[test]
    fn sphere() {
        let r = 0.7;
        let m = implicit_surface(&prog("x^2 + y^2 + z^2 - 0.49", &AXES), dom(), 40);
        assert!(!m.indices.is_empty());
        for (p, nv) in m.positions.chunks_exact(3).zip(m.normals.chunks_exact(3)) {
            // 頂点は半径 r の球の近くにある
            let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((len - r).abs() < 0.03, "len = {len}");
            // 法線は外向き（F が増える向き）
            let dot = p[0] * nv[0] + p[1] * nv[1] + p[2] * nv[2];
            assert!(dot > 0.0);
        }
        // 閉じた曲面: どの辺もちょうど2つの三角形に共有される
        let mut edges = std::collections::HashMap::new();
        for t in m.indices.chunks_exact(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *edges.entry((a.min(b), a.max(b))).or_insert(0) += 1;
            }
        }
        assert!(edges.values().all(|&c| c == 2));
    }

    fn progs3(src: [&str; 3], vars: &[&str]) -> Vec<Program> {
        src.iter().map(|s| prog(s, vars)).collect()
    }

    #[test]
    fn param_surface_cylinder() {
        let d = Domain { lo: [-2.0; 3], hi: [2.0; 3] };
        let p = progs3(["cos(u)", "sin(u)", "v"], &["u", "v"]);
        let shape = parametric_surface_nd(&[p], (0.0, std::f64::consts::TAU), (-1.0, 1.0), 20);
        let m = project(&shape, &View::identity(3), d);
        assert_eq!(m.positions.len(), 21 * 21 * 3);
        assert_eq!(m.grid_coords.len(), m.positions.len());
        // 円柱の法線は横向き（z 成分がほぼ 0）
        assert!(m.normals.chunks_exact(3).all(|n| n[2].abs() < 1e-3));
    }

    #[test]
    fn param_solid_ball() {
        // 球の極座標 (r sin v cos u, r sin v sin u, r cos v)、0 ≤ r ≤ 1 は半径 1 の球（立体）
        let vars = ["u", "v", "t"];
        let p = progs3(["t sin(v) cos(u)", "t sin(v) sin(u)", "t cos(v)"], &vars);
        let pi = std::f64::consts::PI;
        let shape = parametric_solid_nd(&[p], [(0.0, 2.0 * pi), (0.0, pi), (0.0, 1.0)], 24);
        let m = project(&shape, &View::identity(3), Domain { lo: [-2.0; 3], hi: [2.0; 3] });
        assert_eq!(m.grid_coords.len(), m.positions.len());
        // 体積はおよそ 4π/3（多面体で近似するので少し小さい）、外側の面（t = 1）の法線は外向き
        assert!((signed_volume(&m, 0) - 4.0 * pi / 3.0).abs() < 0.1, "{}", signed_volume(&m, 0));
        let mut outer = 0;
        for ((q, nv), g) in m.positions.chunks_exact(3).zip(m.normals.chunks_exact(3)).zip(m.grid_coords.chunks_exact(3)) {
            let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
            // t を固定した面（格子線用の t 成分が NO_GRID）のうち、t = 1 の面
            if g[2] == NO_GRID && len > 0.5 && q[2].abs() < 0.9 {
                outer += 1;
                assert!(q[0] * nv[0] + q[1] * nv[1] + q[2] * nv[2] > 0.0);
            }
        }
        assert!(outer > 0);
        // 向きを変える写像 (−u, v, t) でも、面は外向きに直る
        let p = progs3(["-u", "v", "t"], &vars);
        let shape = parametric_solid_nd(&[p], [(0.0, 1.0); 3], 4);
        let m = project(&shape, &View::identity(3), dom());
        assert!((signed_volume(&m, 0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn curve_adaptive_and_breaks() {
        // 円: 点はすべて半径 1 の上
        let p = progs3(["cos(t)", "sin(t)", "0"], &["t"]);
        let pts = project(&parametric_curve_nd(&[p], (0.0, std::f64::consts::TAU), 8, 1e-3), &View::identity(3), dom()).positions;
        assert!(pts.len() / 3 > 9, "曲がっている所は細かく分け直される");
        for q in pts.chunks_exact(3) {
            assert!(((q[0] * q[0] + q[1] * q[1]).sqrt() - 1.0).abs() < 1e-5);
        }
        // tan t は極で線が切れる（NaN が入る）
        let p = progs3(["t", "tan(t)", "0"], &["t"]);
        let pts = parametric_curve_nd(&[p], (-3.0, 3.0), 50, 1e-3).points;
        assert!(pts.iter().any(|v| v.is_nan()));
    }

    #[test]
    fn projection_of_4d() {
        // 4次元のクリフォードトーラス (cos u, sin u, cos v, sin v)。x, y, w を表示すると半径1の円柱
        let p: Vec<Program> = ["cos(u)", "sin(u)", "cos(v)", "sin(v)"].iter().map(|s| prog(s, &["u", "v"])).collect();
        let shape = parametric_surface_nd(&[p], (0.0, 6.0), (0.0, 6.0), 10);
        assert_eq!(shape.dims, 4);
        let rows: [ViewRow; 3] = [0, 1, 3].map(|a| (0.0, (0..4).map(|k| (k == a) as u8 as f64).collect()));
        let view = View { rows, center: [0.0; 3], hidden: Vec::new(), dist: 0.0 };
        let m = project(&shape, &view, Domain { lo: [-2.0; 3], hi: [2.0; 3] });
        for q in m.positions.chunks_exact(3) {
            assert!(((q[0] * q[0] + q[1] * q[1]).sqrt() - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn perspective_4d() {
        // 4次元の点 (1, 0, 0, w) を、w 方向の距離 4 の視点から見ると、x = 4 / (4 − w) 倍になる
        let rows = identity_rows(4);
        let w_row: ViewRow = (0.0, vec![0.0, 0.0, 0.0, 1.0]);
        let view = View { rows, center: [0.0; 3], hidden: vec![(w_row, 0.0)], dist: 4.0 };
        assert!((view.apply(&[1.0, 0.0, 0.0, 2.0])[0] - 2.0).abs() < 1e-12); // 手前（w = 2）は大きく
        assert!((view.apply(&[1.0, 0.0, 0.0, -4.0])[0] - 0.5).abs() < 1e-12); // 奥（w = −4）は小さく
        assert!(view.apply(&[1.0, 0.0, 0.0, 5.0])[0].is_nan()); // 視点より後ろは描かない
    }

    #[test]
    fn touching_surface() {
        // (x² + y² + z² − 0.49)² = 0 は符号が変わらないが、半径 0.7 の球に接している
        let p = prog("(x^2 + y^2 + z^2 - 0.49)^2", &AXES);
        let m = implicit_surface(&p, dom(), 30);
        assert!(m.indices.len() > 1000, "三角形 {}", m.indices.len() / 3);
        for (q, nv) in m.positions.chunks_exact(3).zip(m.normals.chunks_exact(3)) {
            let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
            assert!((len - 0.7).abs() < 0.03, "len = {len}");
            // 向きがそろって外向き
            assert!(q[0] * nv[0] + q[1] * nv[1] + q[2] * nv[2] > 0.0);
        }
        // 0 にならない (…)² + 0.1 は何も描かない
        let p = prog("(x^2 + y^2 + z^2 - 0.49)^2 + 0.1", &AXES);
        assert!(implicit_surface(&p, dom(), 30).indices.is_empty());
        // 符号が変わる面と、接するだけの面が混ざっていても描ける: x y² = 0 は平面 x = 0 と y = 0
        let p = prog("x y^2", &AXES);
        let m = implicit_surface(&p, dom(), 21);
        let on_y0 = m.positions.chunks_exact(3).filter(|q| q[1].abs() < 0.02 && q[0].abs() > 0.2).count();
        assert!(on_y0 > 50, "y = 0 の面の頂点 {on_y0}");
    }

    #[test]
    fn solid_by_range() {
        // 球 x² + y² + z² = r² を 0.3 ≤ r ≤ 0.7 で重ねると、内側に穴のある厚い球殻になる。
        // 境界は半径 0.3 と 0.7 の2つの球面
        let p = prog("x^2 + y^2 + z^2 - r^2", &["x", "y", "z", "r"]);
        let m = implicit_solid(&p, dom(), 40, &[(0.3, 0.7)], 16);
        assert!(!m.indices.is_empty());
        let (mut inner, mut outer) = (0, 0);
        for (q, nv) in m.positions.chunks_exact(3).zip(m.normals.chunks_exact(3)) {
            let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
            let dot = (q[0] * nv[0] + q[1] * nv[1] + q[2] * nv[2]) / len;
            if (len - 0.3).abs() < 0.04 {
                inner += 1;
                assert!(dot < 0.0, "内側の面の法線は中心向き（立体の外向き）");
            } else if (len - 0.7).abs() < 0.04 {
                outer += 1;
                assert!(dot > 0.0, "外側の面の法線は外向き");
            } else {
                panic!("境界面以外に頂点がある: 半径 {len}");
            }
        }
        assert!(inner > 0 && outer > 0);
    }

    #[test]
    fn caps() {
        // 球 x² + y² + z² = 0.49 を z = 0 で切った切り口は半径 0.7 の円（面積 ≈ 0.49π）
        let p = prog("x^2 + y^2 + z^2 - 0.49", &AXES);
        let f = Field::new(&p, &[], 0, dom());
        assert!(f.negative);
        let area = |m: &Mesh| -> f64 {
            m.indices
                .chunks_exact(3)
                .map(|t| {
                    let v = |k: u32| {
                        let k = k as usize * 3;
                        [m.positions[k] as f64, m.positions[k + 1] as f64, m.positions[k + 2] as f64]
                    };
                    let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                    let (e1, e2) = ([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
                    (e1[0] * e2[1] - e1[1] * e2[0]) / 2.0 // xy 平面での向き付きの面積
                })
                .sum()
        };
        let m = f.cap(dom(), 2, 0.0, true, 60);
        assert!((area(&m) - 0.49 * std::f64::consts::PI).abs() < 0.01, "{}", area(&m));
        assert!(m.positions.chunks_exact(3).all(|q| q[2] == 0.0));
        // 下側を残すなら、ふたは上（+z）向き。上側を残すなら下向き
        assert!(m.normals[2] == 1.0);
        assert!(area(&f.cap(dom(), 2, 0.0, false, 60)) < 0.0);
        // 左右を入れ替えた式 0.49 = x² + y² + z² でも、狭いほう（球の内側）が中身
        let p = prog("0.49 - (x^2 + y^2 + z^2)", &AXES);
        let f = Field::new(&p, &[], 0, dom());
        assert!(!f.negative);
        assert!((area(&f.cap(dom(), 2, 0.0, true, 60)) - 0.49 * std::f64::consts::PI).abs() < 0.01);
        // 球の外で切ると、ふたは無い
        assert!(f.cap(dom(), 0, 0.9, true, 30).indices.is_empty());
    }

    #[test]
    fn two_d() {
        // 円 x² + y² = 0.49 の曲線: 点はすべて半径 0.7 の上
        let p = prog("x^2 + y^2 + z - 0.49", &AXES); // 縦の座標 z は 0 になる
        let m = implicit_curve_2d(&p, dom(), 40);
        assert!(m.positions.len() > 30 * 9);
        for q in m.positions.chunks_exact(3).filter(|q| q[0].is_finite()) {
            assert!(((q[0] * q[0] + q[1] * q[1]).sqrt() - 0.7).abs() < 0.01);
            assert_eq!(q[2], 0.0);
        }
        // y = x²（式の変数は x と縦の z）
        let p = prog("x^2 + z", &["x", "z"]);
        let m = explicit_curve_2d(&p, 1, dom(), 1e-4);
        for q in m.positions.chunks_exact(3).filter(|q| q[0].is_finite()) {
            assert!((q[1] - q[0] * q[0]).abs() < 1e-5);
        }
        // z = x y の塗り分け: 頂点は z = 0、格子線用の座標の3つ目が値
        let p = prog("x y", &["x", "y"]);
        let m = flat_map(&p, dom(), 4, ComplexMode::Off);
        for (q, g) in m.positions.chunks_exact(3).zip(m.grid_coords.chunks_exact(3)) {
            assert_eq!(q[2], 0.0);
            assert!((g[2] - q[0] * q[1]).abs() < 1e-6);
        }
        assert_eq!(m.indices.len(), 4 * 4 * 2 * 3);
    }

    #[test]
    fn combos() {
        assert_eq!(sample_combos(&[], 10), vec![Vec::<f64>::new()]);
        assert_eq!(sample_combos(&[(0.0, 1.0)], 3), vec![vec![0.0], vec![0.5], vec![1.0]]);
        assert_eq!(sample_combos(&[(0.0, 1.0), (0.0, 1.0)], 100).len(), 64 * 64);
    }

    #[test]
    fn sheets_of_many_params() {
        // 4次元の立方体 (t₁, t₂, t₃, t₄): 両端だけなら、面は 4C2 · 2² = 24 枚
        let vars = ["t1", "t2", "t3", "t4"];
        let progs: Vec<Program> = ["t_1", "t_2", "t_3", "t_4"].iter().map(|v| prog(v, &vars)).collect();
        let r = [(-1.0, 1.0); 4];
        assert_eq!(sheet_layout(4, 1, 100).1, 24);
        assert_eq!(sheet_layout(4, 2, 100).1, 54); // 中を通る面も入れると 6 · 3²
        assert_eq!(sheet_layout(5, 1, 100).1, 80);
        // 面が多すぎるときは分割を減らす
        assert_eq!(sheet_layout(8, 4, 100), (1, 28 * 64, 5));
        let shape = parametric_sheets_nd(std::slice::from_ref(&progs), &r, 1, 20);
        let (m, grid) = shape.grid.as_ref().unwrap();
        let per = m * m;
        assert_eq!(shape.points.len(), 24 * per * 4);
        assert_eq!(grid.len(), 24 * per * 2);
        // どの点も立方体の表面にあり、面ごとに2つの成分が ±1 に固定されている
        for face in shape.points.chunks_exact(per * 4) {
            let fixed = (0..4)
                .filter(|&c| face.chunks_exact(4).all(|p| p[c] == face[c]) && face[c].abs() == 1.0)
                .count();
            assert_eq!(fixed, 2);
        }
        // 3次元に射影すると、立方体 [−1, 1]³ に収まる面の集まりになる
        let mesh = project(&shape, &View::identity(4), dom());
        assert!(!mesh.indices.is_empty());
        assert!(mesh.positions.iter().all(|v| v.abs() <= 1.0));
        // 中を通る面（div = 2）: 固定する値に 0 も入る
        let shape = parametric_sheets_nd(&[progs], &r, 2, 20);
        let per = shape.grid.as_ref().unwrap().0.pow(2);
        assert_eq!(shape.points.len(), 54 * per * 4);
        assert!(shape.points.chunks_exact(per * 4).any(|f| f.chunks_exact(4).all(|p| p[2] == 0.0 && p[3] == 0.0)));
    }

    #[test]
    fn coarse_versions() {
        // 細かい格子の曲面: 粗い版は点を間引いた格子で、端は同じ
        let torus: Vec<Program> =
            ["(1.5 + 0.5cos(v)) cos(u)", "(1.5 + 0.5cos(v)) sin(u)", "0.5sin(v)"].iter().map(|s| prog(s, &["u", "v"])).collect();
        let shape = parametric_surface_nd(&[torus], (0.0, 6.0), (0.0, 6.0), 300);
        let m = project(&shape, &View::identity(3), dom());
        assert_eq!(m.indices.len() / 3, 180_000);
        let lod = m.lod.len() / 3;
        assert!(lod > 10_000 && lod <= 2 * LOD_TARGET_TRIANGLES, "{lod}");
        assert!(m.lod.iter().all(|&k| (k as usize) < m.positions.len() / 3));
        // 四隅の点は粗い版にも入っている
        for corner in [0, 300, 300 * 301, 301 * 301 - 1] {
            assert!(m.lod.contains(&corner));
        }
        // 小さいメッシュには作らない
        let small = project(&parametric_surface_nd(&[progs3(["u", "v", "0"], &["u", "v"])], (0.0, 1.0), (0.0, 1.0), 50), &View::identity(3), dom());
        assert!(small.lod.is_empty());

        // 陰関数（格子でないメッシュ）: 頂点をまとめた粗い版。球の表面から離れない
        let mut sphere = implicit_surface(&prog("x^2 + y^2 + z^2 - 0.64", &AXES), dom(), 160);
        let tris = sphere.indices.len() / 3;
        assert!(tris > LOD_MIN_TRIANGLES);
        add_cluster_lod(&mut sphere);
        let lod = sphere.lod.len() / 3;
        assert!(lod > 5_000 && lod < tris / 2, "{lod} / {tris}");
        // 粗い版の三角形は裏返っていない（球の中心から見た向きが、元の三角形と同じ）
        let v = |k: u32| {
            let k = k as usize * 3;
            [sphere.positions[k] as f64, sphere.positions[k + 1] as f64, sphere.positions[k + 2] as f64]
        };
        let side = |t: &[u32]| dot3(tri_normal(&sphere.positions, [t[0], t[1], t[2]]), v(t[0])).signum();
        let full = side(&sphere.indices[..3]);
        let flipped = sphere.lod.chunks_exact(3).filter(|t| side(t) != full).count();
        assert!(flipped * 200 < lod, "{flipped} / {lod}");
        // まとめても、粗い版の番号はずれない
        let n0 = sphere.positions.len() as u32 / 3;
        let merged = merge(vec![small, sphere], false);
        assert!(!merged.lod.is_empty() && merged.lod.iter().all(|&k| k < merged.positions.len() as u32 / 3));
        assert!(merged.lod.len() < merged.indices.len() && n0 > 0);
    }

    /// 速さの目安を測る    /// 速さの目安を測る: `cargo test --release -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn timing() {
        let d = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        for n in [80, 120, 200] {
            let t = std::time::Instant::now();
            let m = implicit_surface(&prog("sin(x) + sin(y) + sin(z)", &AXES), d, n);
            println!("陰関数 n = {n}: {:?}, 三角形 {}", t.elapsed(), m.indices.len() / 3);
        }
        let t = std::time::Instant::now();
        let m = explicit_surface(&prog("sin(x) cos(y) + exp(-x^2 - y^2)", &["x", "y"]), d, 400, 2);
        println!("陽関数 n = 400: {:?}, 三角形 {}", t.elapsed(), m.indices.len() / 3);
        // 3次元球面の中身（4変数）
        let vars = ["t1", "t2", "t3", "t4"];
        let ball = [
            "t_1 sin(t_2) sin(t_3) cos(t_4)",
            "t_1 sin(t_2) sin(t_3) sin(t_4)",
            "t_1 sin(t_2) cos(t_3)",
            "t_1 cos(t_2)",
        ];
        let progs: Vec<Program> = ball.iter().map(|s| prog(s, &vars)).collect();
        let (pi, tau) = (std::f64::consts::PI, std::f64::consts::TAU);
        let r = [(0.0, 2.0), (0.0, pi), (0.0, pi), (0.0, tau)];
        for div in [1, 3] {
            let t = std::time::Instant::now();
            let shape = parametric_sheets_nd(std::slice::from_ref(&progs), &r, div, 150);
            let t1 = t.elapsed();
            let mesh = project(&shape, &View::identity(4), d);
            println!("4変数 div = {div}: 計算 {t1:?} + 射影 {:?}, 三角形 {}", t.elapsed() - t1, mesh.indices.len() / 3);
        }
        let torus: Vec<Program> = ["(3 + cos(v)) cos(u)", "(3 + cos(v)) sin(u)", "sin(v)"].iter().map(|s| prog(s, &["u", "v"])).collect();
        let t = std::time::Instant::now();
        let shape = parametric_surface_nd(&[torus], (0.0, tau), (0.0, tau), 400);
        println!("媒介変数表示の曲面 n = 400: {:?}, 点 {}", t.elapsed(), shape.points.len() / 3);
    }
}



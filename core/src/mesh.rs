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
    /// 粗い版（頂点をまとめる [`add_cluster_lod`]）を作らない（段差のある陽関数など、まとめると形が崩れるもの）
    pub keep_full: bool,
    /// 自分の頂点を持つ粗い版（陰関数。F の格子を間引いて作り直したもの）。[`Mesh::lod`] とはどちらか一方
    pub lod_mesh: Option<Box<Mesh>>,
    /// 作るのにかかった時間の内訳（`lib.rs` の入口で記録する。メッシュを作る関数自体は 0 のまま）
    pub stats: Stats,
}

/// メッシュを作るのにかかった時間の内訳（ミリ秒）
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    /// 式の計算（[`crate::timing`] で集めた時間）
    pub eval_ms: f64,
    /// 式の計算と粗い版を除いた、メッシュを作る時間（式の解析・射影を含む）
    pub mesh_ms: f64,
    /// 粗い版（`add_cluster_lod`）を作る時間
    pub lod_ms: f64,
    /// 式を計算した点の数
    pub points: f64,
    /// 陰関数（[`implicit_solid`]）のときだけ: 部分ごとの時間
    /// [格子点での F の計算, 偽の交点と接する所の候補探し, セルの頂点, 辺の四角形と法線]（ほかは 0）
    pub phases: [f64; 4],
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
    if !mesh.lod.is_empty() || mesh.keep_full || mesh.lod_mesh.is_some() || tris <= LOD_MIN_TRIANGLES {
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
    /// 高さ = arg f
    Arg,
    /// 複素平面の図形（[`crate::equation::Plot::ComplexPlane`]）で、縦の軸を媒介変数にする。陽関数では色分けしない
    Param,
}

impl ComplexMode {
    pub fn from_u32(k: u32) -> ComplexMode {
        match k {
            1 => ComplexMode::Abs,
            2 => ComplexMode::Re,
            3 => ComplexMode::Im,
            4 => ComplexMode::Arg,
            5 => ComplexMode::Param,
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
        ComplexMode::Off | ComplexMode::Param => as_real(z),
        ComplexMode::Abs => z.norm(),
        ComplexMode::Re => z.re,
        ComplexMode::Im => z.im,
        ComplexMode::Arg => z.arg(),
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
    /// 辺の両端の高さの差（段差の大きさの目安）
    first: f64,
}

/// 陽関数の曲面。値が飛ぶ所（段差）を見つけて、そこで三角形を切り分ける（段差の縁がギザギザにならない）。
/// `walls` なら、段差の上下の縁を結ぶ縦の壁も描く。
///
/// 形が急に変わる所（極の近く、定義域の縁、表示範囲の上下の端を横切る急な斜面）は、三角形を二分して細かくする
/// （[`crate::adaptive`]。6 回まで。三角形は最初の数と同じだけ増やせる）。分けるのは、見えている高さにかかる三角形で、
/// 斜辺の中点の値と、両端の値の平均とのずれが、表示範囲の高さの 0.4% を超えるもの、
/// または頂点・中点で定義される所とされない所が混ざるもの。
/// 段差のある辺を含む三角形は分けない（段差は下の切り分けで描く）。2 回続けて分けてもずれが半分にならない所も、
/// 段差の疑いがあるので、それ以上分けない。
///
/// 段差の見つけ方: 両端の高さの差が大きい辺を二分法で 12 回狭めていき、差がほとんど縮まらなければ
/// 「値が飛んでいる」とみなす（なめらかな関数なら、区間を半分にすると差もおよそ半分になる）。
/// 三角形の3辺のうち2辺で値が飛んでいれば、飛ぶ所を結ぶ線で三角形を2つの部分に分け、それぞれの側の値で描く。
/// 1辺のときは、飛ぶ所と向かいの頂点を結ぶ線で分ける。3辺のときは、その三角形を描かない。
pub fn explicit_surface_full(
    prog: &Program,
    dom: Domain,
    n: usize,
    axis: usize,
    mode: ComplexMode,
    walls: bool,
) -> Mesh {
    explicit_surface_opt(prog, dom, n, axis, mode, walls, true)
}

/// [`explicit_surface_full`] で、`refine` が偽なら細かくしない（一様な格子のまま。再生・ドラッグ中）
pub fn explicit_surface_opt(
    prog: &Program,
    dom: Domain,
    n: usize,
    axis: usize,
    mode: ComplexMode,
    walls: bool,
    refine: bool,
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

    // 表示範囲の上下に、範囲の高さ1つ分の余裕を持たせる。
    // はみ出た部分は描画側（Three.js のクリッピング）で切り落とす。
    // 範囲の外の値は、範囲の少し外（wlo, whi）にそろえて描く（極の近くで頂点を捨てると、その周りの三角形が
    // 抜けて穴が空くため）。はみ出た部分は描画側で切り落とされるので見えない
    let h = dom.hi[axis] - dom.lo[axis];
    let (wlo, whi) = (dom.lo[axis] - h, dom.hi[axis] + h);
    let usable = |w: f64| w.is_finite();
    let clamped = |z: C| height(z, mode).clamp(wlo, whi);
    let threshold = h * 0.03;

    // 最初の格子の辺: 右 (i, j)–(i+1, j)、上 (i, j)–(i, j+1)、斜め (i, j)–(i+1, j+1)。番号の小さいほうが始点
    let mut grid_edges = Vec::with_capacity(3 * m * m);
    for j in 0..m {
        for i in 0..m {
            let k = j * m + i;
            if i + 1 < m {
                grid_edges.push((k, k + 1));
            }
            if j + 1 < m {
                grid_edges.push((k, k + m));
            }
            if i + 1 < m && j + 1 < m {
                grid_edges.push((k, k + m + 1));
            }
        }
    }
    // 段差は、切り分けるもの（差が threshold より大きい）より小さいものまで探す。小さい段差（階段の 1 段が
    // 表示範囲の高さの 3% より低いなど）は切り分けずに描くが、そこを細かくしても急な斜面が増えるだけなので、細かくしない
    let mut steps: crate::adaptive::HashSet<(u32, u32)> = Default::default();
    let base_jumps: crate::adaptive::HashMap<(u32, u32), Jump> = {
        let hs: Vec<f64> = vals.iter().map(|&z| clamped(z)).collect();
        let mut out: crate::adaptive::HashMap<(u32, u32), Jump> = Default::default();
        let small = if refine { 0.01 * h } else { threshold };
        for (e, jp) in find_jumps(
            prog,
            &us,
            &vs,
            &vals,
            &hs,
            &grid_edges,
            &clamped,
            small,
            &usable,
        ) {
            let key = (grid_edges[e].0 as u32, grid_edges[e].1 as u32);
            steps.insert(key);
            if jp.first > threshold {
                out.insert(key, jp);
            }
        }
        out
    };

    // 形が急に変わる所を細かくする
    let r = {
        let mut eval = |a: &[f64], b: &[f64]| {
            let mut out = vec![C::new(0.0, 0.0); a.len()];
            prog.eval_batch(&[a, b], &mut out);
            out
        };
        let (lo, hi) = (dom.lo[axis], dom.hi[axis]);
        let tol = 0.004 * h;
        let ek = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
        let mut decide = |c: &crate::adaptive::Candidate<C>| -> Option<f64> {
            let [a, b, w] = c.ids;
            if [ek(a, b), ek(b, w), ek(w, a)]
                .iter()
                .any(|e| steps.contains(e))
            {
                return None; // 段差のある三角形は、段差の切り分けで描く
            }
            let hv = [
                clamped(*c.vals[0]),
                clamped(*c.vals[1]),
                clamped(*c.vals[2]),
                clamped(*c.mid),
            ];
            let defined = hv.iter().filter(|v| v.is_finite()).count();
            // 見えない所（全部が表示範囲の上・下にある）は分けない
            let above = hv.iter().all(|&v| !v.is_finite() || v >= hi);
            let below = hv.iter().all(|&v| !v.is_finite() || v <= lo);
            if defined == 0 || above || below {
                return None;
            }
            if defined < 4 {
                return Some(h); // 定義域の縁
            }
            let e = (hv[3] - 0.5 * (hv[0] + hv[1])).abs();
            if e <= tol {
                return None;
            }
            // 2 回分けてもずれが半分にならない: 段差の疑い（なめらかなら 2 回で 1/4 になる）
            let grand = c.history[1];
            if grand.is_finite() && e > 0.5 * grand {
                return None;
            }
            Some(e)
        };
        let base_tris = 2 * n * n;
        let limits = crate::adaptive::Limits {
            passes: if refine { 6 } else { 0 },
            max_new_triangles: base_tris.max(30_000),
        };
        let (u0, u1) = (dom.lo[ua], dom.hi[ua]);
        let (v0, v1) = (dom.lo[va], dom.hi[va]);
        crate::adaptive::refine(n, (u0, u1), (v0, v1), vals, &mut eval, &mut decide, limits)
    };
    let count = r.uv.len();
    let us: Vec<f64> = r.uv.iter().map(|p| p[0]).collect();
    let vs: Vec<f64> = r.uv.iter().map(|p| p[1]).collect();
    let vals = r.vals;
    let hs: Vec<f64> = vals.iter().map(|&z| height(z, mode)).collect();

    // 細かくしてできた辺の段差を探す（最初の格子のままの辺は、もう調べてある）
    let mut jumps = base_jumps;
    {
        let mut new_edges: Vec<(usize, usize)> = Vec::new();
        let mut seen: crate::adaptive::HashSet<(u32, u32)> = Default::default();
        for t in &r.tris {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let e = if a < b { (a, b) } else { (b, a) };
                if (e.1 as usize) >= m * m && seen.insert(e) {
                    new_edges.push((e.0 as usize, e.1 as usize));
                }
            }
        }
        if !new_edges.is_empty() {
            let hs_clamped: Vec<f64> = hs.iter().map(|&w| w.clamp(wlo, whi)).collect();
            for (e, jp) in find_jumps(
                prog,
                &us,
                &vs,
                &vals,
                &hs_clamped,
                &new_edges,
                &clamped,
                threshold,
                &usable,
            ) {
                jumps.insert((new_edges[e].0 as u32, new_edges[e].1 as u32), jp);
            }
        }
    }

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

    // 三角形に使われる、使える頂点だけに番号を振る
    let mut index_of = vec![NONE; count];
    let mut used = vec![false; count];
    for t in &r.tris {
        for &k in t {
            used[k as usize] = true;
        }
    }
    for k in 0..count {
        if used[k] && usable(hs[k]) {
            index_of[k] = push(&mut mesh, us[k], vs[k], vals[k]);
        }
    }
    // 値が飛ぶ辺の上の2つの頂点（始点の側・終点の側）。隣り合う三角形で共有する
    let mut jump_vertex: crate::adaptive::HashMap<(u32, u32), (u32, u32)> = Default::default();
    let mut keys: Vec<(u32, u32)> = jumps.keys().copied().collect();
    keys.sort_unstable(); // 頂点の並びを毎回同じにする
    for e in keys {
        let jp = jumps[&e];
        let (k0, k1) = (e.0 as usize, e.1 as usize);
        let (u, v) = (
            us[k0] + (us[k1] - us[k0]) * jp.t,
            vs[k0] + (vs[k1] - vs[k0]) * jp.t,
        );
        let a = push(&mut mesh, u, v, jp.before);
        let b = push(&mut mesh, u, v, jp.after);
        jump_vertex.insert(e, (a, b));
    }

    // 法線が axis の正の向きになるように三角形の向きを決める
    // （(ua, va, axis) が x→y→z の順の巡回なら、そのままで正の向き）
    let flip = (ua + 1) % 3 != va;
    let mut wall_quads: Vec<[(f64, f64, C, C); 2]> = Vec::new();

    // 段差の無い三角形（表示範囲の上端・下端で切ってから描く）
    let mut plain: Vec<[usize; 3]> = Vec::new();
    // 3頂点とも使える三角形だけ残す
    for t in &r.tris {
        let vtx = [t[0] as usize, t[1] as usize, t[2] as usize];
        if vtx.iter().any(|&p| index_of[p] == NONE) {
            // 頂点の一部だけが定義される三角形: 定義域の縁を求めて、定義される部分を描く（後でまとめて）
            if vtx.iter().any(|&p| index_of[p] != NONE) {
                plain.push(vtx);
            }
            continue;
        }
        // 各辺 (p→q) の辺の名前（番号の小さい頂点が先）と、向き（三角形の辺の向きと同じなら true）
        let edge = |p: u32, q: u32| {
            if p < q {
                ((p, q), true)
            } else {
                ((q, p), false)
            }
        };
        let edges = [edge(t[0], t[1]), edge(t[1], t[2]), edge(t[2], t[0])];
        let jumped: Vec<usize> = (0..3)
            .filter(|&e| jumps.contains_key(&edges[e].0))
            .collect();
        let emit = |mesh: &mut Mesh, t: [u32; 3]| {
            mesh.indices
                .extend_from_slice(&if flip { [t[0], t[2], t[1]] } else { t });
        };
        match jumped.len() {
            0 => plain.push(vtx), // 表示範囲の上端・下端で切るものがあるので、後でまとめて描く
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
                    if lone_is_start == lone_side {
                        start
                    } else {
                        end
                    }
                };
                let p = vtx[lone];
                let q = vtx[(lone + 1) % 3];
                let rr = vtx[(lone + 2) % 3];
                // lone の側: 三角形 (lone, 辺 e_out の点, 辺 e_in の点)
                emit(
                    &mut mesh,
                    [index_of[p], side(e_out, true), side(e_in, true)],
                );
                // 反対側: 四角形 (辺 e_out の点, q, r, 辺 e_in の点)
                let (x, y) = (side(e_out, false), side(e_in, false));
                emit(&mut mesh, [x, index_of[q], index_of[rr]]);
                emit(&mut mesh, [x, index_of[rr], y]);
                if walls {
                    let info = |e: usize| {
                        let (id, forward) = edges[e];
                        let jp = jumps[&id];
                        let (k0, k1) = (id.0 as usize, id.1 as usize);
                        let (u, v) = (
                            us[k0] + (us[k1] - us[k0]) * jp.t,
                            vs[k0] + (vs[k1] - vs[k0]) * jp.t,
                        );
                        let lone_is_start = (e == e_out) == forward;
                        let (near, far) = if lone_is_start {
                            (jp.before, jp.after)
                        } else {
                            (jp.after, jp.before)
                        };
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
                // 三角形の辺 e は 頂点 e → e+1。辺の名前と同じ向きなら、頂点 e の側が始点の側
                let (near_p, near_q) = if forward { (start, end) } else { (end, start) };
                let (p, q, w) = (vtx[e], vtx[(e + 1) % 3], vtx[(e + 2) % 3]);
                emit(&mut mesh, [index_of[p], near_p, index_of[w]]);
                emit(&mut mesh, [near_q, index_of[q], index_of[w]]);
            }
            _ => {} // 3辺とも飛ぶ: 描かない
        }
    }
    clip_to_height_range(prog, &mut mesh, &plain, &us, &vs, &vals, &index_of, axis, mode, (dom.lo[axis], dom.hi[axis]), flip, &push);
    compute_normals(&mut mesh);
    // 段差のある曲面は、頂点をまとめた粗い版にすると段差と壁が崩れるので、粗い版を作らない
    mesh.keep_full = !jumps.is_empty();

    // 段差の壁: 段差の線に沿って、lone の側の縁と反対側の縁を結ぶ縦の四角形（頂点は共有しない）
    for [(u0, v0, n0, f0), (u1, v1, n1, f1)] in wall_quads {
        let base = mesh.positions.len() as u32 / 3;
        for (u, v, z) in [(u0, v0, n0), (u1, v1, n1), (u1, v1, f1), (u0, v0, f0)] {
            push(&mut mesh, u, v, z);
        }
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
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
        let len = (nv[0] * nv[0] + nv[1] * nv[1] + nv[2] * nv[2])
            .sqrt()
            .max(1e-300);
        for _ in 0..4 {
            mesh.normals.extend(nv.iter().map(|c| (c / len) as f32));
        }
    }
    mesh
}

/// 陽関数の段差の無い三角形 `tris`（細かくした格子の頂点の番号）を、定義域の縁と、表示範囲の高さ `[lo, hi]` の
/// 上端・下端で切って描く。
/// 1. 頂点の一部だけが定義される三角形は、定義される頂点と定義されない頂点を結ぶ辺の上で、定義域の縁を二分法で求め、
///    定義される部分の多角形を描く（捨てると、定義域の縁の近くに穴が空く。`1/sqrt(1 - x^2 - y^2)` の上端の近くなど）
/// 2. 上端・下端を横切る辺の交わる点を、実際の関数で二分法で求めて、範囲の中の部分だけを描く。急な斜面が表示範囲の端を
///    横切る所（`1/sqrt(1 - x^2 - y^2)` の上端の円など）も、格子の細かさに関係なくなめらかな縁になる。
///    範囲の外に出た部分は描かない（描画側でも切り落とされる部分）。上端と下端の両方を横切る多角形はそのまま描く。
///
/// 辺の上に求めた点は、辺を共有する隣の三角形と同じものを使うので、継ぎ目に隙間はできない
#[allow(clippy::too_many_arguments)]
fn clip_to_height_range(
    prog: &Program,
    mesh: &mut Mesh,
    tris: &[[usize; 3]],
    us: &[f64],
    vs: &[f64],
    vals: &[C],
    index_of: &[u32],
    axis: usize,
    mode: ComplexMode,
    (lo, hi): (f64, f64),
    flip: bool,
    push: &dyn Fn(&mut Mesh, f64, f64, C) -> u32,
) {
    // 点の一覧（元の頂点の後ろに、辺の上に求めた点を足していく）
    let mut pu = us.to_vec();
    let mut pv = vs.to_vec();
    let mut pval = vals.to_vec();
    let mut pid: Vec<u32> = index_of.to_vec();
    let mut level: Vec<Option<f64>> = vec![None; us.len()]; // 端の高さにちょうど置く点
    let defined = |val: &C| height(*val, mode).is_finite();
    // 辺 (a, b) の上で、条件 inside が成り立つ側（a の側）と成り立たない側の境目を、二分法でまとめて求めて点を足す
    let bisect = |pairs: &[(usize, usize)],
                      inside: &dyn Fn(&C) -> bool,
                      at: &dyn Fn(usize) -> Option<f64>,
                      pu: &mut Vec<f64>,
                      pv: &mut Vec<f64>,
                      pval: &mut Vec<C>,
                      pid: &mut Vec<u32>,
                      level: &mut Vec<Option<f64>>|
     -> Vec<usize> {
        let n = pairs.len();
        let (mut ta, mut tb) = (vec![0.0f64; n], vec![1.0f64; n]);
        let mut za: Vec<C> = pairs.iter().map(|&(a, _)| pval[a]).collect();
        let (mut mu, mut mv) = (vec![0.0; n], vec![0.0; n]);
        let mut out = vec![C::new(0.0, 0.0); n];
        for _ in 0..30 {
            for (k, &(a, b)) in pairs.iter().enumerate() {
                let t = 0.5 * (ta[k] + tb[k]);
                mu[k] = pu[a] + (pu[b] - pu[a]) * t;
                mv[k] = pv[a] + (pv[b] - pv[a]) * t;
            }
            prog.eval_batch(&[&mu, &mv], &mut out);
            for k in 0..n {
                let t = 0.5 * (ta[k] + tb[k]);
                if inside(&out[k]) {
                    ta[k] = t;
                    za[k] = out[k];
                } else {
                    tb[k] = t;
                }
            }
        }
        let mut ids = Vec::with_capacity(n);
        for (k, &(a, b)) in pairs.iter().enumerate() {
            pu.push(pu[a] + (pu[b] - pu[a]) * ta[k]);
            pv.push(pv[a] + (pv[b] - pv[a]) * ta[k]);
            pval.push(za[k]);
            pid.push(NONE);
            level.push(at(k));
            ids.push(pu.len() - 1);
        }
        ids
    };

    // 1. 定義域の縁
    let mut edge_pt: crate::adaptive::HashMap<(usize, usize), usize> = Default::default();
    let mut todo: Vec<(usize, usize)> = Vec::new();
    for t in tris {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            for (i, o) in [(a, b), (b, a)] {
                if defined(&vals[i]) && !defined(&vals[o]) && !edge_pt.contains_key(&(i, o)) {
                    edge_pt.insert((i, o), usize::MAX);
                    todo.push((i, o));
                }
            }
        }
    }
    if !todo.is_empty() {
        let ids = bisect(&todo, &|v| defined(v), &|_| None, &mut pu, &mut pv, &mut pval, &mut pid, &mut level);
        for (pair, id) in todo.iter().zip(ids) {
            edge_pt.insert(*pair, id);
        }
    }
    // 定義される部分の多角形（点の番号。向きは元の三角形と同じ）
    // 面積がほとんど 0 の多角形（x + iy のように線の上でしか定義されない式の、縁の細い切れ端）は描かない
    let area = |poly: &[usize], pu: &[f64], pv: &[f64]| {
        let mut a = 0.0;
        for e in 0..poly.len() {
            let (p, q) = (poly[e], poly[(e + 1) % poly.len()]);
            a += pu[p] * pv[q] - pu[q] * pv[p];
        }
        0.5 * a.abs()
    };
    let polys: Vec<Vec<usize>> = tris
        .iter()
        .map(|t| {
            let mut poly = Vec::with_capacity(4);
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                let (da, db) = (defined(&vals[a]), defined(&vals[b]));
                if da {
                    poly.push(a);
                }
                if da != db {
                    poly.push(edge_pt[&if da { (a, b) } else { (b, a) }]);
                }
            }
            (poly, area(t, &pu, &pv))
        })
        .filter(|(p, whole)| p.len() >= 3 && area(p, &pu, &pv) > 1e-6 * whole)
        .map(|(p, _)| p)
        .collect();

    // 2. 上端・下端で切る
    let h = |val: &C| height(*val, mode);
    // 点の位置: 0 = 範囲の中、1 = 上端より上（定義されない点も）、-1 = 下端より下
    let side = |val: &C| {
        let v = h(val);
        if !v.is_finite() || v > hi { 1i8 } else if v < lo { -1 } else { 0 }
    };
    let outside_of = |poly: &[usize], pval: &[C]| -> i8 {
        let s: Vec<i8> = poly.iter().map(|&k| side(&pval[k])).collect();
        if !s.contains(&0) || (s.contains(&1) && s.contains(&-1)) {
            0 // 全部外（描画側で切り落とされる）か、上下の両方に出る（そのまま描く）
        } else {
            s.iter().copied().find(|&x| x != 0).unwrap_or(0)
        }
    };
    let mut cut_pt: crate::adaptive::HashMap<(usize, usize), usize> = Default::default();
    let mut todo: Vec<(usize, usize)> = Vec::new();
    let mut todo_level: Vec<f64> = Vec::new();
    for poly in &polys {
        let out = outside_of(poly, &pval);
        if out == 0 {
            continue;
        }
        for e in 0..poly.len() {
            let (a, b) = (poly[e], poly[(e + 1) % poly.len()]);
            for (i, o) in [(a, b), (b, a)] {
                if side(&pval[i]) == 0 && side(&pval[o]) == out && !cut_pt.contains_key(&(i, o)) {
                    cut_pt.insert((i, o), usize::MAX);
                    todo.push((i, o));
                    todo_level.push(if out > 0 { hi } else { lo });
                }
            }
        }
    }
    if !todo.is_empty() {
        // 上端・下端の両方の辺をまとめて二分する（中の側の条件は、辺ごとの高さで決める）
        let (up, down): (Vec<usize>, Vec<usize>) = (0..todo.len()).partition(|&k| todo_level[k] == hi);
        for (group, inside) in [(up, &(|v: &C| h(v).is_finite() && h(v) <= hi) as &dyn Fn(&C) -> bool), (down, &|v: &C| h(v).is_finite() && h(v) >= lo)] {
            if group.is_empty() {
                continue;
            }
            let pairs: Vec<(usize, usize)> = group.iter().map(|&k| todo[k]).collect();
            let lv = todo_level[group[0]];
            let ids = bisect(&pairs, inside, &|_| Some(lv), &mut pu, &mut pv, &mut pval, &mut pid, &mut level);
            for (pair, id) in pairs.iter().zip(ids) {
                cut_pt.insert(*pair, id);
            }
        }
    }

    // 3. 描く
    let mut vid = |k: usize, mesh: &mut Mesh| -> u32 {
        if pid[k] == NONE {
            let v = push(mesh, pu[k], pv[k], pval[k]);
            if let Some(l) = level[k] {
                mesh.positions[3 * v as usize + axis] = l as f32; // ちょうど端の高さに置く
            }
            pid[k] = v;
        }
        pid[k]
    };
    for poly in &polys {
        let out = outside_of(poly, &pval);
        let kept: Vec<usize> = if out == 0 {
            let s: Vec<i8> = poly.iter().map(|&k| side(&pval[k])).collect();
            if !s.contains(&0) && !(s.contains(&1) && s.contains(&-1)) {
                continue; // 全部が上端より上（または下端より下）: 見えない
            }
            poly.clone()
        } else {
            let mut kept = Vec::with_capacity(poly.len() + 1);
            for e in 0..poly.len() {
                let (a, b) = (poly[e], poly[(e + 1) % poly.len()]);
                let (sa, sb) = (side(&pval[a]) == 0, side(&pval[b]) == 0);
                if sa {
                    kept.push(a);
                }
                if sa != sb {
                    kept.push(cut_pt[&if sa { (a, b) } else { (b, a) }]);
                }
            }
            kept
        };
        if kept.len() < 3 {
            continue;
        }
        let ids: Vec<u32> = kept.iter().map(|&k| vid(k, mesh)).collect();
        for k in 1..ids.len() - 1 {
            let t = [ids[0], ids[k], ids[k + 1]];
            mesh.indices.extend_from_slice(&if flip { [t[0], t[2], t[1]] } else { t });
        }
    }
}

/// 辺 `edges`（頂点の番号の組。始点 → 終点）のうち、値が飛ぶものを探す（[`explicit_surface_full`] を参照）。
/// 戻り値は 辺の番号（`edges` の中の位置）→ 飛ぶ所（位置 t は始点から終点へ）
#[allow(clippy::too_many_arguments)]
fn find_jumps(
    prog: &Program,
    us: &[f64],
    vs: &[f64],
    vals: &[C],
    hs: &[f64],
    edges: &[(usize, usize)],
    height: &dyn Fn(C) -> f64,
    threshold: f64,
    usable: &dyn Fn(f64) -> bool,
) -> std::collections::HashMap<usize, Jump> {
    find_jumps_with(
        prog, us, vs, vals, hs, edges, height, threshold, usable, true,
    )
}

/// [`find_jumps`] の本体。`early_exit` が偽なら、途中で候補を外さない（テストで結果を比べるため）
#[allow(clippy::too_many_arguments)]
fn find_jumps_with(
    prog: &Program,
    us: &[f64],
    vs: &[f64],
    vals: &[C],
    hs: &[f64],
    edges: &[(usize, usize)],
    height: &dyn Fn(C) -> f64,
    threshold: f64,
    usable: &dyn Fn(f64) -> bool,
    early_exit: bool,
) -> std::collections::HashMap<usize, Jump> {
    const ROUNDS: usize = 12;
    // この回数だけ二分した後、差が最初の差の EARLY_KEEP 倍より小さい候補は外す
    const EARLY_ROUND: usize = 3;
    const EARLY_KEEP: f64 = 0.3;
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
    for (id, &(k, k1)) in edges.iter().enumerate() {
        if !usable(hs[k]) || !usable(hs[k1]) {
            continue;
        }
        let d = (hs[k1] - hs[k]).abs();
        if d > threshold {
            cands.push(Cand {
                id,
                k0: k,
                k1,
                ta: 0.0,
                tb: 1.0,
                fa: vals[k],
                fb: vals[k1],
                first: d,
            });
        }
    }
    let mut mu = vec![0.0; cands.len()];
    let mut mv = vec![0.0; cands.len()];
    let mut out = vec![C::new(0.0, 0.0); cands.len()];
    for round in 0..ROUNDS {
        // 急なだけでなめらかな辺（z = x² − y² ではほとんどの辺が候補になる）は、二分すると差が縮んでいくので、
        // 3 回二分した時点で差が最初の 0.3 倍より小さければ候補から外す（最後の判定は「0.5 倍より大きい」）。
        // 段差のある辺は、二分しても差が縮まないので残る。差が有限でない（片側が定義されない）辺も、
        // 最後の判定で外れるので、ここで外してよい
        if early_exit && round == EARLY_ROUND {
            cands.retain(|c| (height(c.fb) - height(c.fa)).abs() >= EARLY_KEEP * c.first);
            mu.truncate(cands.len());
            mv.truncate(cands.len());
            out.truncate(cands.len());
        }
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
            jumps.insert(c.id, Jump { t: 0.5 * (c.ta + c.tb), before: c.fa, after: c.fb, first: c.first });
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
    implicit_solid_with(prog, dom, n, ranges, samples, false)
}

/// [`implicit_solid`] の本体。`all_blocks` なら区間演算を使わず、全部のブロックを「ある」にする（テストで比べるため）
fn implicit_solid_with(prog: &Program, dom: Domain, n: usize, ranges: &[(f64, f64)], samples: usize, all_blocks: bool) -> Mesh {
    // 表示範囲の外側に1マスずつ広げて計算する。Surface Nets 法は範囲の端で曲面が
    // ギザギザに途切れるので、少しはみ出して作っておき、端は描画側で真っ直ぐ切り落とす。
    let t_start = crate::timing::now_ms();
    let n = n.max(2);
    let mut dom = dom;
    for ax in 0..3 {
        let step = (dom.hi[ax] - dom.lo[ax]) / n as f64;
        dom.lo[ax] -= step;
        dom.hi[ax] += step;
    }
    let n = n + 2;
    let m = n + 1; // 1辺の格子点の数

    let combos = sample_combos(ranges, samples);
    // 1. 格子を 1 辺 BLOCK マスのブロックに分け、区間演算で「曲面が無い」と分かったブロック（空のブロック）では
    //    F を計算しない。あるブロックの格子点と、その周りの1点（隣の辺の傾き・接する所の判定に使う）で F を計算し、
    //    f32 で保存する（[`SparseField`]）
    // F の計算の仕方（ふつうは全部ここで計算する。重い式は、Worker が計算を分けるために計画だけ・一部だけを頼む）
    let field = {
        let mut plan = SparseField::plan(prog, ranges, dom, n, all_blocks);
        let vals = FIELD_MODE.with(|mode| {
            let mut mode = mode.borrow_mut();
            FIELD_OUT.with(|out| {
                let mut out = out.borrow_mut();
                out.calls += 1;
                match &mut *mode {
                    FieldMode::Normal => Some(plan.values(prog, &combos, dom, 0..plan.chunks())),
                    FieldMode::Plan => {
                        out.chunks = plan.chunks();
                        None
                    }
                    FieldMode::Chunks(a, b) => {
                        let (a, b) = ((*a).min(plan.chunks()), (*b).min(plan.chunks()));
                        out.values = plan.values(prog, &combos, dom, a..b.max(a));
                        None
                    }
                    // 渡された値を使う（数が合わなければ、ここで計算する）。1 回だけ使う
                    FieldMode::Given(v) => {
                        let v = std::mem::take(v);
                        *mode = FieldMode::Normal;
                        Some(if v.len() == plan.len() { v } else { plan.values(prog, &combos, dom, 0..plan.chunks()) })
                    }
                }
            })
        });
        let Some(vals) = vals else { return Mesh::default() };
        plan.fill(&vals);
        plan.field
    };
    let idx = |i: usize, j: usize, k: usize| i + m * (j + m * k);

    let t_field = crate::timing::now_ms();

    // 1''. 符号は変わるが F = 0 になる点が無い辺（tan の極など、F が +∞ から −∞ へ飛ぶ所）。ここには面を張らない。
    // 1'.  符号は変わらないが F = 0 に接している所（例: (x² + y² + z² − 1)² = 0）。
    //      どちらも、あるブロックの中だけを調べる（空のブロックの辺は、区間演算が「F は 0 にならない」と保証する）。
    //      ブロックごとに、値を周りの点ごと小さな配列に写してから調べる（1点ずつページを引かないように）
    let mut sign_edges: Vec<SignEdge> = Vec::new();
    let mut sign_ids: Vec<usize> = Vec::new();
    let mut touch_cands: Vec<TouchCand> = Vec::new();
    {
        let mut buf = vec![f32::NAN; BLOCK_BUF * BLOCK_BUF * BLOCK_BUF];
        let pt = |q: [usize; 3]| [0, 1, 2].map(|x| dom.at(x, q[x] as f64, n));
        field.for_blocks(&mut buf, |buf, view, owned| {
            for k in owned[2].clone() {
                for j in owned[1].clone() {
                    for i in owned[0].clone() {
                        let q = [i, j, k];
                        let l = view.at(q);
                        for a in 0..3 {
                            let st = view.strides[a];
                            if q[a] + 1 >= m {
                                continue;
                            }
                            let (f0, f1) = (buf[l], buf[l + st]);
                            if (f0 < 0.0) != (f1 < 0.0) {
                                if !f0.is_finite() || !f1.is_finite() {
                                    continue;
                                }
                                // 安い判定: 同じ直線の上の隣の辺（p − e → p、q → q + e）の傾きと、この辺の傾きが同じ向きで、
                                // 大きさも 4 倍以内なら、なめらかに 0 を横切っているだけなので調べない（ほとんどの辺はここで終わる）。
                                // 奇数次の極（+∞ から −∞ へ飛ぶ）では、この辺の傾きが隣と逆向きになり、
                                // 値が飛ぶ所では隣の傾きがほぼ 0 になるので、どちらもここを通らずに調べられる
                                if q[a] >= 1 && q[a] + 2 < m {
                                    let (fl, fr) = (buf[l - st], buf[l + 2 * st]);
                                    let (sl, s, sr) = (f0 - fl, f1 - f0, fr - f1);
                                    if sl * s > 0.0 && sr * s > 0.0 && s.abs() <= 4.0 * sl.abs().max(sr.abs()) {
                                        continue;
                                    }
                                }
                                let mut q1 = q;
                                q1[a] += 1;
                                sign_edges.push(SignEdge { p0: pt(q), p1: pt(q1), f0: f0 as f64, f1: f1 as f64 });
                                sign_ids.push(3 * idx(i, j, k) + a);
                            } else if ranges.is_empty() && q[a] >= 1 && q[a] + 2 <= n {
                                let v = [buf[l - st], f0, f1, buf[l + 2 * st]];
                                let around = || same_sign_around(|r| buf[view.at(r)], m, q, a);
                                if let Some((s, outer)) = touch_candidate(v, around) {
                                    touch_cands.push((3 * idx(i, j, k) + a, q, a, s, outer));
                                }
                            }
                        }
                    }
                }
            }
        });
    }
    let not_zero: std::collections::HashSet<usize> = {
        let mut sc = FieldScratch::default();
        let flags = false_crossings(&mut |xyz| field_values_with(prog, &combos, xyz, &mut sc).to_vec(), &sign_edges);
        sign_ids.into_iter().zip(flags).filter(|&(_, bad)| bad).map(|(id, _)| id).collect()
    };
    let touch = if ranges.is_empty() { touch_search(prog, touch_cands, dom, n) } else { Default::default() };
    let cell = |i: usize, j: usize, k: usize| i + n * (j + n * k);
    // 接するだけの辺を囲むセルに印を付ける。印の無いセル・辺は、接する所を調べずに済ませる
    // （1つでも接する所があると全部のセルが遅い道を通る、ということにならないように）
    let mut touch_cell = vec![false; if touch.is_empty() { 0 } else { n * n * n }];
    for &id in touch.keys() {
        let (a, mut q) = (id % 3, id / 3);
        let mut p = [0; 3];
        for x in p.iter_mut() {
            *x = q % m;
            q /= m;
        }
        let (b, c) = ((a + 1) % 3, (a + 2) % 3);
        for db in 0..2 {
            for dc in 0..2 {
                let mut s = p;
                if s[b] < db || s[c] < dc {
                    continue;
                }
                s[b] -= db;
                s[c] -= dc;
                if s.iter().all(|&x| x < n) {
                    touch_cell[cell(s[0], s[1], s[2])] = true;
                }
            }
        }
    }
    let marked = |i: usize, j: usize, k: usize| !touch_cell.is_empty() && touch_cell[cell(i, j, k)];
    let t_touch = crate::timing::now_ms();

    // 格子の辺 p → p + e_a の上で曲面と交わる位置 t（0〜1）と、F の符号（接するだけの所は None）。
    // 接するだけの所は、印のあるセルに接する辺（near_touch）でだけ調べる
    //    両端の値 f0, f1 を渡す版（cross_vals）と、格子から値を読む版（crossing）
    let cross_vals = |f0: f32, f1: f32, p: [usize; 3], a: usize, near_touch: bool| -> Option<(f64, Option<bool>)> {
        if !f0.is_finite() || !f1.is_finite() {
            return None;
        }
        if (f0 < 0.0) != (f1 < 0.0) {
            if !not_zero.is_empty() && not_zero.contains(&(3 * idx(p[0], p[1], p[2]) + a)) {
                return None; // F が飛ぶだけで、0 になる点は無い
            }
            return Some(((f0 / (f0 - f1)) as f64, Some(f0 < 0.0)));
        }
        if !near_touch {
            return None;
        }
        touch.get(&(3 * idx(p[0], p[1], p[2]) + a)).map(|&t| (t as f64, None))
    };
    let crossing = |p: [usize; 3], a: usize, near_touch: bool| {
        let mut q = p;
        q[a] += 1;
        cross_vals(field.f(p), field.f(q), p, a, near_touch)
    };

    // 2. 曲面が通るセルに頂点を置く。位置は「辺の上で曲面と交わる点」の平均。
    //    1つのセルを2枚以上の曲面が通るとき（近い曲面どうし）は、1つの頂点にまとめると曲面がくっつくので、
    //    曲面ごとに頂点を分ける: 角のうち少ないほうの符号の角を、セルの辺でつながっているものどうしで組に分け、
    //    交わる辺はその少ないほうの符号の端の角の組に属させる（組ごとに曲面が1枚）。
    //    接するだけの辺（符号が変わらない）は、まとめて1つの組にする。
    //    cell_slots[vert[セル]][辺の番号] が、その辺に属する頂点
    //
    //    速くするため、格子点ごとの符号（1 で作ったもの）から、セルの8つの角の符号を1バイトにまとめる。
    //    空のブロックのセルは見ない（区間演算が「曲面は無い」と保証する）。あるブロックごとに、値を小さな配列に
    //    写してから見る（[`SparseField::for_blocks`]）
    // セルの12本の辺の向き（0: x, 1: y, 2: z）
    let edge_axis: [usize; 12] = EDGES.map(|[ca, cb]| (0..3).find(|&x| CORNERS[ca][x] != CORNERS[cb][x]).unwrap_or(0));
    let mut vert = vec![NONE; n * n * n];
    let mut cell_slots: Vec<[u32; 12]> = Vec::new();
    // 頂点を置いたセル（cell_slots と同じ順番）
    let mut vert_cells: Vec<[usize; 3]> = Vec::new();
    let mut mesh = Mesh::default();
    let mut cbuf = vec![f32::NAN; BLOCK_BUF * BLOCK_BUF * BLOCK_BUF];
    field.for_blocks(&mut cbuf, |buf, view, _| {
    // セルの角 c の、data の中での位置のずれ
    let [sx, sy, sz] = view.strides;
    let corner_off: [usize; 8] = CORNERS.map(|d| d[0] * sx + d[1] * sy + d[2] * sz);
    let cells = view.origin.map(|o| o..(o + BLOCK).min(n));
    for k in cells[2].clone() {
        for j in cells[1].clone() {
            for i in cells[0].clone() {
                let l = view.at([i, j, k]);
                // セルの辺（角 base から向き a）の交わり。値はブロックの見え方から読む
                let crossing = |base: [usize; 3], a: usize, near: bool| {
                    let l0 = l + base[0] * sx + base[1] * sy + base[2] * sz;
                    cross_vals(buf[l0], buf[l0 + view.strides[a]], [i + base[0], j + base[1], k + base[2]], a, near)
                };
                let mut mask = 0u8;
                for (c, off) in corner_off.iter().enumerate() {
                    mask |= ((buf[l + off] < 0.0) as u8) << c;
                }
                // 角の符号がそろっていて、接するだけの所も無ければ、曲面は通らない（ほとんどのセルはここで終わる）
                let near_touch = marked(i, j, k);
                if !near_touch && (mask == 0 || mask == 0xFF) {
                    continue;
                }
                let corner_neg: [bool; 8] = std::array::from_fn(|c| mask >> c & 1 == 1);
                let neg = mask.count_ones() as usize;
                // 少ないほうの符号の角を、セルの辺でつながっているものどうしで組に分ける
                let minority = neg <= 4;
                let mut group = [usize::MAX; 8];
                let mut groups = 0;
                for c in 0..8 {
                    if corner_neg[c] != minority || group[c] != usize::MAX {
                        continue;
                    }
                    // 角は8つなので、積む場所は固定の大きさで足りる（セルごとに Vec を作らない）
                    let mut stack = [0usize; 8];
                    let mut top = 1;
                    stack[0] = c;
                    group[c] = groups;
                    while top > 0 {
                        top -= 1;
                        let x = stack[top];
                        for [ca, cb] in EDGES {
                            let y = if ca == x { cb } else if cb == x { ca } else { continue };
                            if corner_neg[y] == minority && group[y] == usize::MAX {
                                group[y] = groups;
                                stack[top] = y;
                                top += 1;
                            }
                        }
                    }
                    groups += 1;
                }
                // 接するだけの辺（符号が変わらない）は、そのセルを通る曲面のどれか1枚の一部。
                // 符号の変わる辺もあるセルでは組に分けず、1つの頂点にまとめる（前と同じ）
                let has_touch = near_touch
                    && EDGES.iter().enumerate().any(|(e, &[ca, _])| {
                        let a = edge_axis[e];
                        let base = CORNERS[ca];
                        matches!(crossing(base, a, true), Some((_, None)))
                    });
                if has_touch && groups > 0 {
                    for g in group.iter_mut().filter(|g| **g != usize::MAX) {
                        *g = 0;
                    }
                    groups = 1;
                }
                let touch_group = if groups > 0 { 0 } else { groups }; // 接するだけの辺の組
                // 交わる辺ごとの組と、交わる点（セルの中の位置）。組ごとに足し合わせる
                // （組は高々 8 個 + 接するだけの組。セルごとに Vec を作らないよう、固定の大きさにする）
                let mut sums = [([0.0f64; 3], 0.0f64); 9];
                let mut edge_group = [usize::MAX; 12];
                for (e, [ca, cb]) in EDGES.iter().copied().enumerate() {
                    let a = edge_axis[e];
                    let base = CORNERS[ca];
                    if let Some((t, sign)) = crossing(base, a, near_touch) {
                        let g = match sign {
                            Some(_) => group[if corner_neg[ca] == minority { ca } else { cb }],
                            None => touch_group,
                        };
                        let mut q = [base[0] as f64, base[1] as f64, base[2] as f64];
                        q[a] += t;
                        edge_group[e] = g;
                        let (sum, cnt) = &mut sums[g];
                        for ax in 0..3 {
                            sum[ax] += q[ax];
                        }
                        *cnt += 1.0;
                    }
                }
                let mut ids = [NONE; 9];
                for (g, (sum, cnt)) in sums.iter().enumerate().take(groups + 1) {
                    if *cnt > 0.0 {
                        let p = [i as f64 + sum[0] / cnt, j as f64 + sum[1] / cnt, k as f64 + sum[2] / cnt];
                        ids[g] = mesh.push_vertex([dom.at(0, p[0], n), dom.at(1, p[1], n), dom.at(2, p[2], n)]);
                    }
                }
                if ids[..=groups].iter().all(|&v| v == NONE) {
                    continue;
                }
                vert[cell(i, j, k)] = cell_slots.len() as u32;
                cell_slots.push(edge_group.map(|g| if g == usize::MAX { NONE } else { ids[g] }));
                vert_cells.push([i, j, k]);
            }
        }
    }
    });

    let t_cells = crate::timing::now_ms();
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
        // 四角形を作るのは、周りの4セルすべてに頂点がある辺だけ。そこで、頂点を置いたセルごとに、
        // そのセルが周りの4セルのうち (0, 0) の位置にある辺（セルの最小の角から向き a に出る辺）だけを見る
        for &p in &vert_cells {
            {
                {
                    // 辺 p → p + e_a。周りの4セルが全部格子内にある辺だけを見る（p[a] < n, p[b] < n, p[c] < n は満たしている）
                    if p[b] == 0 || p[c] == 0 {
                        continue;
                    }
                    // 符号の変わらない辺は、周りの4セルのどれかに印があるときだけ接する所を調べる
                    let near_touch = !touch_cell.is_empty()
                        && [(1, 1), (0, 1), (0, 0), (1, 0)].iter().any(|&(db, dc)| {
                            let mut s = p;
                            s[b] -= db;
                            s[c] -= dc;
                            marked(s[0], s[1], s[2])
                        });
                    let Some((_, sign)) = crossing(p, a, near_touch) else { continue };
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
    // 粗い版（視点を動かしている間に描く）: F の格子を間引いて作り直す
    if let Some(k) = lod_stride(mesh.indices.len() / 3) {
        let mut sc = FieldScratch::default();
        let mut eval = |xyz: [&[f64]; 3]| field_values_with(prog, &combos, xyz, &mut sc).to_vec();
        let lod = coarse_nets(&field, dom, n, k, &not_zero, &mut eval);
        if !lod.indices.is_empty() {
            mesh.lod_mesh = Some(Box::new(lod));
        }
    }
    let t_end = crate::timing::now_ms();
    mesh.stats.phases = [t_field - t_start, t_touch - t_field, t_cells - t_touch, t_end - t_cells];
    mesh
}

/// 陰関数の粗い版の、格子の間引き方: 三角形が `tris` 個のメッシュなら、三角形の数（間引くと 1/k² になる）が
/// 4 万個以下になる最小の k（2 か 4）。粗い版が要らない（三角形が少ない）なら None
fn lod_stride(tris: usize) -> Option<usize> {
    if tris <= LOD_MIN_TRIANGLES {
        return None;
    }
    Some(if tris / 4 <= 40_000 { 2 } else { 4 })
}

/// 陰関数の粗い版: 計算済みの F の格子（`field`。範囲を外に 1 マス広げた `n` マスの格子）を `k` 個おきに間引いた格子で、
/// Surface Nets をもう一度走らせる（F はほとんど計算し直さない）。k はブロックの 1 辺（8 マス）を割り切るので、
/// 粗いセルはどれも 1 つのブロックの中にあり、空のブロックのセルは見なくてよい。
/// 格子の端（マス数が k の倍数でない所）の点など、持っていない値だけを `eval` で計算する。
/// 細かい版で「零点ではない」とされた辺（`not_zero`）を含む粗い辺には面を張らない。接するだけの所は探さない
fn coarse_nets(
    field: &SparseField,
    dom: Domain,
    n: usize,
    k: usize,
    not_zero: &std::collections::HashSet<usize>,
    eval: &mut dyn FnMut([&[f64]; 3]) -> Vec<f64>,
) -> Mesh {
    let m = n + 1;
    let nc = n.div_ceil(k);
    let mc = nc + 1;
    let at = |q: [usize; 3]| q[0] + mc * (q[1] + mc * q[2]);
    let nb = field.nb;
    let block_of = |c: [usize; 3]| c.map(|x| (x * k / BLOCK).min(nb - 1));
    let cell_ok = |c: [usize; 3]| {
        let b = block_of(c);
        field.occupied[b[0] + nb * (b[1] + nb * b[2])]
    };
    // 粗い格子の値（持っていない点は NaN）
    let mut vals = vec![f32::NAN; mc * mc * mc];
    for z in 0..mc {
        for y in 0..mc {
            for x in 0..mc {
                let q = [x * k, y * k, z * k];
                if q.iter().all(|&v| v < m) {
                    vals[at([x, y, z])] = field.f(q);
                }
            }
        }
    }
    // 見るセルの角で、値を持っていない点を計算する
    let mut missing: Vec<usize> = Vec::new();
    let mut seen = vec![false; mc * mc * mc];
    for z in 0..nc {
        for y in 0..nc {
            for x in 0..nc {
                if !cell_ok([x, y, z]) {
                    continue;
                }
                for c in CORNERS {
                    let l = at([x + c[0], y + c[1], z + c[2]]);
                    if vals[l].is_nan() && !seen[l] {
                        seen[l] = true;
                        missing.push(l);
                    }
                }
            }
        }
    }
    if !missing.is_empty() {
        let coord = |l: usize, a: usize| {
            let q = [l % mc, (l / mc) % mc, l / (mc * mc)];
            dom.at(a, (q[a] * k) as f64, n)
        };
        let xs: Vec<f64> = missing.iter().map(|&l| coord(l, 0)).collect();
        let ys: Vec<f64> = missing.iter().map(|&l| coord(l, 1)).collect();
        let zs: Vec<f64> = missing.iter().map(|&l| coord(l, 2)).collect();
        for (&l, v) in missing.iter().zip(eval([&xs, &ys, &zs])) {
            vals[l] = v as f32;
        }
    }
    let fine = |i: usize, j: usize, kk: usize| i + m * (j + m * kk);
    // 粗い辺 p → p + e_a に、細かい版で「零点ではない」とされた辺が含まれるか
    let bad_edge = |p: [usize; 3], a: usize| {
        if not_zero.is_empty() {
            return false;
        }
        (0..k).any(|t| {
            let mut q = p.map(|v| v * k);
            q[a] += t;
            q.iter().all(|&v| v < m) && not_zero.contains(&(3 * fine(q[0], q[1], q[2]) + a))
        })
    };
    let cross = |p: [usize; 3], a: usize| -> Option<(f64, bool)> {
        let mut q = p;
        q[a] += 1;
        let (f0, f1) = (vals[at(p)], vals[at(q)]);
        if !f0.is_finite() || !f1.is_finite() || (f0 < 0.0) == (f1 < 0.0) || bad_edge(p, a) {
            return None;
        }
        Some(((f0 / (f0 - f1)) as f64, f0 < 0.0))
    };
    let pos = |p: [f64; 3]| [0, 1, 2].map(|a| dom.at(a, p[a] * k as f64, n));
    nets_on_grid(nc, &cell_ok, &|c| vals[at(c)] < 0.0, &cross, &pos)
}

/// Surface Nets（1 辺 `nc` マスの格子）。`cell_ok` が偽のセルは見ない。`neg(角)` は角の F が負か、
/// `cross(p, a)` は辺 p → p + e_a の交わる位置と始点の符号（負なら true）。`pos` は格子の座標から空間の座標へ。
/// 1 つのセルを 2 枚以上の曲面が通るときは、細かい版と同じく、少ないほうの符号の角の組ごとに頂点を分ける
/// （分けないと、鞍の形の面で 4 つの三角形が 1 本の辺に集まる）
fn nets_on_grid(
    nc: usize,
    cell_ok: &dyn Fn([usize; 3]) -> bool,
    neg: &dyn Fn([usize; 3]) -> bool,
    cross: &dyn Fn([usize; 3], usize) -> Option<(f64, bool)>,
    pos: &dyn Fn([f64; 3]) -> [f64; 3],
) -> Mesh {
    let cell = |c: [usize; 3]| c[0] + nc * (c[1] + nc * c[2]);
    let edge_axis: [usize; 12] = EDGES.map(|[ca, cb]| (0..3).find(|&x| CORNERS[ca][x] != CORNERS[cb][x]).unwrap_or(0));
    let mut vert = vec![NONE; nc * nc * nc];
    let mut slots: Vec<[u32; 12]> = Vec::new();
    let mut cells: Vec<[usize; 3]> = Vec::new();
    let mut mesh = Mesh::default();
    for z in 0..nc {
        for y in 0..nc {
            for x in 0..nc {
                let c = [x, y, z];
                if !cell_ok(c) {
                    continue;
                }
                let corner = |d: [usize; 3]| [x + d[0], y + d[1], z + d[2]];
                let corner_neg: [bool; 8] = std::array::from_fn(|i| neg(corner(CORNERS[i])));
                let count = corner_neg.iter().filter(|&&b| b).count();
                if count == 0 || count == 8 {
                    continue;
                }
                let minority = count <= 4;
                let mut group = [usize::MAX; 8];
                let mut groups = 0;
                for i in 0..8 {
                    if corner_neg[i] != minority || group[i] != usize::MAX {
                        continue;
                    }
                    let mut stack = vec![i];
                    group[i] = groups;
                    while let Some(a) = stack.pop() {
                        for [ca, cb] in EDGES {
                            let b = if ca == a { cb } else if cb == a { ca } else { continue };
                            if corner_neg[b] == minority && group[b] == usize::MAX {
                                group[b] = groups;
                                stack.push(b);
                            }
                        }
                    }
                    groups += 1;
                }
                let mut sums = [([0.0f64; 3], 0.0f64); 8];
                let mut edge_group = [usize::MAX; 12];
                for (e, [ca, cb]) in EDGES.iter().copied().enumerate() {
                    let a = edge_axis[e];
                    if let Some((t, _)) = cross(corner(CORNERS[ca]), a) {
                        let g = group[if corner_neg[ca] == minority { ca } else { cb }];
                        let mut q = CORNERS[ca].map(|v| v as f64);
                        q[a] += t;
                        edge_group[e] = g;
                        for (sum, v) in sums[g].0.iter_mut().zip(q) {
                            *sum += v;
                        }
                        sums[g].1 += 1.0;
                    }
                }
                let mut ids = [NONE; 8];
                for g in 0..groups {
                    let (sum, cnt) = sums[g];
                    if cnt > 0.0 {
                        ids[g] = mesh.push_vertex(pos([x as f64 + sum[0] / cnt, y as f64 + sum[1] / cnt, z as f64 + sum[2] / cnt]));
                    }
                }
                if ids.iter().all(|&v| v == NONE) {
                    continue;
                }
                vert[cell(c)] = slots.len() as u32;
                slots.push(edge_group.map(|g| if g == usize::MAX { NONE } else { ids[g] }));
                cells.push(c);
            }
        }
    }
    let local_edge = |a: usize, base: [usize; 3]| -> usize {
        EDGES
            .iter()
            .position(|&[ca, cb]| CORNERS[ca] == base && (0..3).all(|x| (CORNERS[cb][x] != base[x]) == (x == a)))
            .unwrap_or(0)
    };
    for a in 0..3 {
        let (b, c) = ((a + 1) % 3, (a + 2) % 3);
        let around = [(1, 1), (0, 1), (0, 0), (1, 0)];
        let locals = around.map(|(db, dc)| {
            let mut base = [0; 3];
            base[b] = db;
            base[c] = dc;
            local_edge(a, base)
        });
        for &p in &cells {
            if p[b] == 0 || p[c] == 0 {
                continue;
            }
            let Some((_, sign)) = cross(p, a) else { continue };
            let mut q = [NONE; 4];
            for (i, ((db, dc), le)) in around.iter().zip(locals).enumerate() {
                let mut s = p;
                s[b] -= db;
                s[c] -= dc;
                let slot = vert[cell(s)];
                if slot != NONE {
                    q[i] = slots[slot as usize][le];
                }
            }
            if q.contains(&NONE) {
                continue;
            }
            if sign {
                mesh.indices.extend_from_slice(&[q[0], q[1], q[2], q[0], q[2], q[3]]);
            } else {
                mesh.indices.extend_from_slice(&[q[0], q[2], q[1], q[0], q[3], q[2]]);
            }
        }
    }
    compute_normals(&mut mesh);
    mesh
}

/// 速くする前の [`implicit_solid`]（テストで、出力が完全に同じことを確かめる）
#[cfg(test)]
fn implicit_solid_reference(prog: &Program, dom: Domain, n: usize, ranges: &[(f64, f64)], samples: usize) -> Mesh {
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
    let cell = |i: usize, j: usize, k: usize| i + n * (j + n * k);
    // 接するだけの辺を囲むセルに印を付ける。印の無いセル・辺は、接する所を調べずに済ませる
    // （1つでも接する所があると全部のセルが遅い道を通る、ということにならないように）
    let mut touch_cell = vec![false; if touch.is_empty() { 0 } else { n * n * n }];
    for &id in touch.keys() {
        let (a, mut q) = (id % 3, id / 3);
        let mut p = [0; 3];
        for x in p.iter_mut() {
            *x = q % m;
            q /= m;
        }
        let (b, c) = ((a + 1) % 3, (a + 2) % 3);
        for db in 0..2 {
            for dc in 0..2 {
                let mut s = p;
                if s[b] < db || s[c] < dc {
                    continue;
                }
                s[b] -= db;
                s[c] -= dc;
                if s.iter().all(|&x| x < n) {
                    touch_cell[cell(s[0], s[1], s[2])] = true;
                }
            }
        }
    }
    let marked = |i: usize, j: usize, k: usize| !touch_cell.is_empty() && touch_cell[cell(i, j, k)];

    // 格子の辺 p → p + e_a の上で曲面と交わる位置 t（0〜1）と、F の符号（接するだけの所は None）。
    // 接するだけの所は、印のあるセルに接する辺（near_touch）でだけ調べる
    let crossing = |p: [usize; 3], a: usize, near_touch: bool| -> Option<(f64, Option<bool>)> {
        let mut q = p;
        q[a] += 1;
        let (f0, f1) = (f[idx(p[0], p[1], p[2])], f[idx(q[0], q[1], q[2])]);
        if !f0.is_finite() || !f1.is_finite() {
            return None;
        }
        if (f0 < 0.0) != (f1 < 0.0) {
            return Some(((f0 / (f0 - f1)) as f64, Some(f0 < 0.0)));
        }
        if !near_touch {
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
    let mut vert = vec![NONE; n * n * n];
    let mut cell_slots: Vec<[u32; 12]> = Vec::new();
    let mut mesh = Mesh::default();
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                let corner_neg = CORNERS.map(|d| f[idx(i + d[0], j + d[1], k + d[2])] < 0.0);
                // 角の符号がそろっていて、接するだけの所も無ければ、曲面は通らない（ほとんどのセルはここで終わる）
                let neg = corner_neg.iter().filter(|&&b| b).count();
                let near_touch = marked(i, j, k);
                if !near_touch && (neg == 0 || neg == 8) {
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
                let has_touch = near_touch
                    && EDGES.iter().any(|&[ca, cb]| {
                        let a = (0..3).find(|&x| CORNERS[ca][x] != CORNERS[cb][x]).unwrap_or(0);
                        let base = CORNERS[ca];
                        matches!(crossing([i + base[0], j + base[1], k + base[2]], a, true), Some((_, None)))
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
                    if let Some((t, sign)) = crossing([i + base[0], j + base[1], k + base[2]], a, near_touch) {
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
                    // 符号の変わらない辺は、周りの4セルのどれかに印があるときだけ接する所を調べる
                    let near_touch = !touch_cell.is_empty()
                        && [(1, 1), (0, 1), (0, 0), (1, 0)].iter().any(|&(db, dc)| {
                            let mut s = p;
                            s[b] -= db;
                            s[c] -= dc;
                            marked(s[0], s[1], s[2])
                        });
                    let Some((_, sign)) = crossing(p, a, near_touch) else { continue };
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

/// 陰関数の格子のブロックの1辺のマス数
const BLOCK: usize = 8;
/// あるブロックの割合がこれを超えたら、ブロックごとの区間演算をやめて全部を「ある」にする（区間演算の時間が無駄になるため）
const DENSE_FRACTION: f64 = 0.6;

/// 陰関数の格子点での F の値を、要る所だけ持つもの（[`implicit_solid`] の 1）。
///
/// 格子（n マス、格子点は m = n + 1 個ずつ）を 1 辺 BLOCK マスのブロックに分け、区間演算で
/// 「箱の上で F は 0 にならない」と分かったブロックを空にする。値は、あるブロックの格子点と、その周りの1点
/// （隣の辺の傾き・接する所の判定に使う）だけで計算し、格子点 8 × 8 × 8 個ずつの「ページ」に分けて持つ。
/// 区間演算が使えない式は、全部のブロックが「ある」になるだけで、同じ道を通る
struct SparseField {
    /// 1辺の格子点の数
    m: usize,
    /// 1辺のブロックの数と、ブロックごとの「ある」
    nb: usize,
    occupied: Vec<bool>,
    /// 1辺のページの数と、ページの番号 → 値の置き場所（NONE なら持っていない）
    np: usize,
    pages: Vec<u32>,
    /// 値。ページごとに 512 個ずつ。
    /// 全部のブロックがあるとき（dense）は、ページに分けずに格子点の番号 i + m (j + m k) の順に持つ
    /// （値を読むたびにページを引かずに済む）
    vals: Vec<f32>,
    dense: bool,
}

/// 1ページの格子点の数（8 × 8 × 8）
const PAGE: usize = 512;
/// ブロックの値の見え方（[`SparseField::for_blocks`]）。格子点 q の値は `data[v.at(q)]`
#[derive(Clone, Copy)]
struct BlockView {
    /// ブロックの最小の角
    origin: [usize; 3],
    /// data の中で、最小の角の 1 点手前（origin − (1, 1, 1)）の位置（足し算は桁あふれを許す）
    base: usize,
    /// y, z の向きに 1 進んだときの、data の中での位置のずれ
    strides: [usize; 3],
}

impl BlockView {
    #[inline]
    fn at(&self, q: [usize; 3]) -> usize {
        let [sx, sy, sz] = self.strides;
        self.base
            .wrapping_add((q[0] + 1 - self.origin[0]) * sx)
            .wrapping_add((q[1] + 1 - self.origin[1]) * sy)
            .wrapping_add((q[2] + 1 - self.origin[2]) * sz)
    }
}

/// ブロックごとに値を写す小さな配列の1辺（ブロックの 1 点手前から 1 + BLOCK + 3 点。辺の先の 2 点まで届く）
const BLOCK_BUF: usize = BLOCK + 4;

/// 陰関数の F の計算の仕方（[`implicit_solid`] が見る）。重い式の F の計算を、いくつかの Worker に分けるために使う
#[derive(Debug, Default)]
pub enum FieldMode {
    /// 全部ここで計算する（ふつう）
    #[default]
    Normal,
    /// 計画（かたまりの数）だけを [`FieldOut::chunks`] に入れて、メッシュは作らない
    Plan,
    /// かたまり a..b の値だけを計算して [`FieldOut::values`] に入れ、メッシュは作らない
    Chunks(usize, usize),
    /// 全部のかたまりの値（ほかの Worker が計算したもの）を使ってメッシュを作る
    Given(Vec<f32>),
}

/// [`FieldMode`] の結果
#[derive(Debug, Default)]
pub struct FieldOut {
    /// [`implicit_solid`] が呼ばれた回数（1 回でなければ、分けられない式）
    pub calls: usize,
    pub chunks: usize,
    pub values: Vec<f32>,
}

thread_local! {
    pub static FIELD_MODE: std::cell::RefCell<FieldMode> = const { std::cell::RefCell::new(FieldMode::Normal) };
    pub static FIELD_OUT: std::cell::RefCell<FieldOut> = std::cell::RefCell::new(FieldOut::default());
}

/// F の計算の仕方を決めて、結果を空にする
pub fn set_field_mode(mode: FieldMode) {
    FIELD_MODE.with(|m| *m.borrow_mut() = mode);
    FIELD_OUT.with(|o| *o.borrow_mut() = FieldOut::default());
}

/// F の計算の仕方をふつうに戻して、結果を取り出す
pub fn take_field_out() -> FieldOut {
    FIELD_MODE.with(|m| *m.borrow_mut() = FieldMode::Normal);
    FIELD_OUT.with(|o| std::mem::take(&mut *o.borrow_mut()))
}

/// F を計算する前の [`SparseField`] と、F を計算する点の一覧。点は「かたまり」（密な格子なら 1 層、
/// まばらなら [`FIELD_CHUNK`] 点ずつ）に分けて計算する。かたまりの切れ目はいつも同じなので、かたまりを
/// いくつかに分けて（別々の Worker で）計算してつなげても、1 回で計算したときと同じ値になる
struct FieldPlan {
    field: SparseField,
    /// 範囲を外に広げた格子のマス数
    n: usize,
    /// まばらなら、要る格子点（値の置き場所, 格子点の番号 i + m (j + m k)）。密なら空（全部の点を順に使う）
    spots: Vec<(u32, u32)>,
}

/// まばらな格子で、1 回に F を計算する点の数
const FIELD_CHUNK: usize = 1 << 15;

impl FieldPlan {
    /// かたまりの数
    fn chunks(&self) -> usize {
        if self.field.dense { self.field.m } else { self.spots.len().div_ceil(FIELD_CHUNK) }
    }

    /// かたまり `range` の点で F を計算して、点の順に並べたもの
    fn values(&self, prog: &Program, combos: &[Vec<f64>], dom: Domain, range: std::ops::Range<usize>) -> Vec<f32> {
        let (m, n) = (self.field.m, self.n);
        let mut scratch = FieldScratch::default();
        let mut out = Vec::new();
        if self.field.dense {
            let mut xs = Vec::with_capacity(m * m);
            let mut ys = Vec::with_capacity(m * m);
            for j in 0..m {
                for i in 0..m {
                    xs.push(dom.at(0, i as f64, n));
                    ys.push(dom.at(1, j as f64, n));
                }
            }
            let mut zs = vec![0.0; m * m];
            for k in range {
                zs.fill(dom.at(2, k as f64, n));
                out.extend(field_values_with(prog, combos, [&xs, &ys, &zs], &mut scratch).iter().map(|&v| v as f32));
            }
            return out;
        }
        let coord: [Vec<f64>; 3] = [0, 1, 2].map(|ax| (0..m).map(|q| dom.at(ax, q as f64, n)).collect());
        let mut cols: [Vec<f64>; 3] = Default::default();
        for c in range {
            let part = &self.spots[c * FIELD_CHUNK..((c + 1) * FIELD_CHUNK).min(self.spots.len())];
            for col in cols.iter_mut() {
                col.clear();
            }
            for &(_, p) in part {
                let p = p as usize;
                let q = [p % m, (p / m) % m, p / (m * m)];
                for ax in 0..3 {
                    cols[ax].push(coord[ax][q[ax]]);
                }
            }
            out.extend(field_values_with(prog, combos, [&cols[0], &cols[1], &cols[2]], &mut scratch).iter().map(|&v| v as f32));
        }
        out
    }

    /// 全部のかたまりの値（[`FieldPlan::values`] を全範囲でつなげたもの）を、値の置き場所に入れる
    fn fill(&mut self, vals: &[f32]) {
        if self.field.dense {
            self.field.vals.copy_from_slice(vals);
        } else {
            for (&(at, _), &v) in self.spots.iter().zip(vals) {
                self.field.vals[at as usize] = v;
            }
        }
    }

    /// 全部の点の数
    fn len(&self) -> usize {
        if self.field.dense { self.field.m.pow(3) } else { self.spots.len() }
    }
}

impl SparseField {
    #[cfg(test)]
    fn build(prog: &Program, combos: &[Vec<f64>], ranges: &[(f64, f64)], dom: Domain, n: usize, all_blocks: bool) -> SparseField {
        let mut plan = SparseField::plan(prog, ranges, dom, n, all_blocks);
        let vals = plan.values(prog, combos, dom, 0..plan.chunks());
        plan.fill(&vals);
        plan.field
    }

    /// F の値を入れる前の形（あるブロック・値の置き場所）と、F を計算する点の一覧（[`FieldPlan`]）
    fn plan(prog: &Program, ranges: &[(f64, f64)], dom: Domain, n: usize, all_blocks: bool) -> FieldPlan {
        let m = n + 1;
        let nb = n.div_ceil(BLOCK);
        let occupied = if all_blocks { vec![true; nb * nb * nb] } else { block_occupancy(prog, ranges, dom, n, nb) };
        let np = m.div_ceil(8);
        let mut field =
            SparseField { m, nb, occupied, np, pages: Vec::new(), vals: Vec::new(), dense: false };
        // 全部のブロックがあるなら、密な格子にする（層ごとにまとめて計算する。速くする前と同じ）
        if field.occupied.iter().all(|&o| o) {
            field.dense = true;
            field.vals = vec![0.0; m * m * m];
            return FieldPlan { field, n, spots: Vec::new() };
        }
        field.pages = vec![NONE; np * np * np];
        // 要る格子点: あるブロックのセルの角と、その周りの1点
        let all = false;
        let mut need: Vec<bool> = Vec::new();
        for bk in 0..nb {
            for bj in 0..nb {
                for bi in 0..nb {
                    if !field.occupied[bi + nb * (bj + nb * bk)] {
                        continue;
                    }
                    let span = |b: usize| (BLOCK * b).saturating_sub(1)..=(BLOCK * (b + 1)).min(n).saturating_add(1).min(m - 1);
                    let (si, sj, sk) = (span(bi), span(bj), span(bk));
                    // ページを用意する
                    for pk in (*sk.start() / 8)..=(*sk.end() / 8) {
                        for pj in (*sj.start() / 8)..=(*sj.end() / 8) {
                            for pi in (*si.start() / 8)..=(*si.end() / 8) {
                                let slot = &mut field.pages[pi + np * (pj + np * pk)];
                                if *slot == NONE {
                                    *slot = (field.vals.len() / PAGE) as u32;
                                    field.vals.resize(field.vals.len() + PAGE, f32::NAN);
                                    need.resize(field.vals.len(), all);
                                }
                            }
                        }
                    }
                    if all {
                        continue; // 全部のブロックがあるなら、格子の点は全部要る
                    }
                    // x の向きにはページの中で続いているので、まとめて印を付ける
                    for k in sk.clone() {
                        for j in sj.clone() {
                            let mut i = *si.start();
                            while i <= *si.end() {
                                let run = (8 - (i & 7)).min(si.end() + 1 - i);
                                let at = field.at([i, j, k]);
                                need[at..at + run].fill(true);
                                i += run;
                            }
                        }
                    }
                }
            }
        }
        // 要る格子点の一覧（値の置き場所, 格子点の番号）。ページの順に並べる
        let mut spots: Vec<(u32, u32)> = Vec::new();
        for pk in 0..np {
            for pj in 0..np {
                for pi in 0..np {
                    let slot = field.pages[pi + np * (pj + np * pk)];
                    if slot == NONE {
                        continue;
                    }
                    for l in 0..PAGE {
                        let at = slot as usize * PAGE + l;
                        let q = [8 * pi + (l & 7), 8 * pj + ((l >> 3) & 7), 8 * pk + (l >> 6)];
                        if !need[at] || q.iter().any(|&x| x >= m) {
                            continue;
                        }
                        spots.push((at as u32, (q[0] + m * (q[1] + m * q[2])) as u32));
                    }
                }
            }
        }
        FieldPlan { field, n, spots }
    }

    /// 格子点 q の値の置き場所（持っていなければ usize::MAX）
    #[inline]
    fn at(&self, q: [usize; 3]) -> usize {
        if self.dense {
            return q[0] + self.m * (q[1] + self.m * q[2]);
        }
        let slot = self.pages[(q[0] >> 3) + self.np * ((q[1] >> 3) + self.np * (q[2] >> 3))];
        if slot == NONE {
            return usize::MAX;
        }
        slot as usize * PAGE + (q[0] & 7) + 8 * ((q[1] & 7) + 8 * (q[2] & 7))
    }

    /// 格子点 q での F の値（持っていない点は NaN。あるブロックの角とその周りの1点は必ず持っている）
    #[inline]
    fn f(&self, q: [usize; 3]) -> f32 {
        match self.at(q) {
            usize::MAX => f32::NAN,
            at => self.vals[at],
        }
    }

    /// あるブロックごとに、`visit(data, view, 受け持つ格子点の範囲)` を呼ぶ。格子点 q の値は `data[view.at(q)]` で、
    /// ブロックの格子点とその周り（最小の角の 1 点手前から 2 点先まで）が読める（格子の外・持っていない点は NaN）。
    /// 密な格子なら全体の配列をそのまま渡し、ページに分けて持っているなら `buf`（BLOCK_BUF³ 個）に写して渡す。
    /// 受け持つ格子点は、ブロックの最小の角の側の点（最後のブロックは格子の端まで）。格子の辺 p → p + e は、
    /// 始点 p を受け持つブロックが調べる
    fn for_blocks(&self, buf: &mut [f32], mut visit: impl FnMut(&[f32], BlockView, [std::ops::Range<usize>; 3])) {
        let nb = self.nb;
        let m = self.m;
        for bk in 0..nb {
            for bj in 0..nb {
                for bi in 0..nb {
                    if !self.occupied[bi + nb * (bj + nb * bk)] {
                        continue;
                    }
                    let b = [bi, bj, bk];
                    let origin = b.map(|x| BLOCK * x);
                    let owned = b.map(|x| BLOCK * x..if x + 1 == nb { m } else { BLOCK * (x + 1) });
                    if self.dense {
                        // 全体の配列で、origin − (1, 1, 1) の位置（負になることがあるので、桁あふれを許して計算する）
                        let strides = [1, m, m * m];
                        let base = (origin[0] + m * (origin[1] + m * origin[2])).wrapping_sub(1 + m + m * m);
                        visit(&self.vals, BlockView { origin, base, strides }, owned);
                        continue;
                    }
                    // 値を写す（x の向きに続く所はまとめて写す）
                    for lk in 0..BLOCK_BUF {
                        for lj in 0..BLOCK_BUF {
                            let row = &mut buf[BLOCK_BUF * (lj + BLOCK_BUF * lk)..][..BLOCK_BUF];
                            let (j, k) = ((origin[1] + lj).wrapping_sub(1), (origin[2] + lk).wrapping_sub(1));
                            if j >= m || k >= m {
                                row.fill(f32::NAN);
                                continue;
                            }
                            let mut li = 0;
                            while li < BLOCK_BUF {
                                let i = (origin[0] + li).wrapping_sub(1);
                                if i >= m {
                                    row[li] = f32::NAN;
                                    li += 1;
                                    continue;
                                }
                                // 同じページの中で続く点の数
                                let run = (8 - (i & 7)).min(m - i).min(BLOCK_BUF - li);
                                match self.at([i, j, k]) {
                                    usize::MAX => row[li..li + run].fill(f32::NAN),
                                    at => row[li..li + run].copy_from_slice(&self.vals[at..at + run]),
                                }
                                li += run;
                            }
                        }
                    }
                    visit(buf, BlockView { origin, base: 0, strides: [1, BLOCK_BUF, BLOCK_BUF * BLOCK_BUF] }, owned);
                }
            }
        }
    }

    /// 値を持っている格子点の数（テスト・ベンチマーク用）
    #[cfg(test)]
    fn stored(&self) -> usize {
        self.vals.len()
    }
}

/// ブロックごとに、F = 0 の曲面があるかもしれないか（区間演算で 0 を含むか。分からなければ「ある」）。
/// 範囲で重ねた立体は、箱 × 重ねる変数の範囲で調べる（0 ∉ F(B × R) なら G = max(min F, −max F) > 0 で境界は無い）。
/// まず 2 ブロックおき（各軸で1つおき）のブロックで割合を見積もり、DENSE_FRACTION を超えたら全部を「ある」にする
fn block_occupancy(prog: &Program, ranges: &[(f64, f64)], dom: Domain, n: usize, nb: usize) -> Vec<bool> {
    use crate::interval::Interval;
    if !prog.supports_interval() {
        return vec![true; nb * nb * nb];
    }
    let extra: Vec<Interval> = ranges.iter().map(|&(a, b)| Interval::new(a.min(b), a.max(b))).collect();
    // セル [c0, c1) の箱で 0 を含むかもしれないか
    let may_cross = |c0: [usize; 3], c1: [usize; 3]| {
        let mut boxes: Vec<Interval> = (0..3)
            .map(|ax| {
                let (a, b) = (dom.at(ax, c0[ax] as f64, n), dom.at(ax, c1[ax].min(n) as f64, n));
                Interval::new(a.min(b), a.max(b))
            })
            .collect();
        boxes.extend_from_slice(&extra);
        prog.eval_interval(&boxes).may_contain_zero()
    };
    // まず 2 ブロックおき（各軸で1つおき。全体の 1/8）のブロックで、あるブロックの割合を見積もる
    let id = |b: [usize; 3]| b[0] + nb * (b[1] + nb * b[2]);
    let test = |b: [usize; 3]| {
        let c0 = b.map(|x| BLOCK * x);
        may_cross(c0, c0.map(|c| c + BLOCK))
    };
    let mut occupied = vec![false; nb * nb * nb];
    let mut known = vec![false; nb * nb * nb];
    let (mut sampled, mut hit) = (0usize, 0usize);
    for bk in (0..nb).step_by(2) {
        for bj in (0..nb).step_by(2) {
            for bi in (0..nb).step_by(2) {
                let b = [bi, bj, bk];
                let o = test(b);
                occupied[id(b)] = o;
                known[id(b)] = true;
                sampled += 1;
                hit += o as usize;
            }
        }
    }
    if hit as f64 > DENSE_FRACTION * sampled as f64 {
        return vec![true; nb * nb * nb];
    }
    for bk in 0..nb {
        for bj in 0..nb {
            for bi in 0..nb {
                let b = [bi, bj, bk];
                if !known[id(b)] {
                    occupied[id(b)] = test(b);
                }
            }
        }
    }
    occupied
}

/// 両端で F の符号が違う格子の辺（[`false_crossings`] に渡す）
struct SignEdge {
    /// 両端の座標
    p0: [f64; 3],
    p1: [f64; 3],
    /// 両端の F の値（符号が違う）
    f0: f64,
    f1: f64,
}

/// 符号の変わる辺のうち、F = 0 になる点が無いもの（F が +∞ から −∞ へ飛ぶ極や、値が飛ぶ所）を探す。
/// 戻り値は辺ごとに「零点ではない」なら true。`eval` は点の列 (x, y, z) での F の値。
///
/// 1. 直線で補間した交点 t* = f₀/(f₀ − f₁) で F を計算し、|F(t*)| ≤ 0.25 min(|f₀|, |f₁|) なら本当の零点
///    （ほとんどの辺はここで終わる）
/// 2. 残った辺は、符号の変わる側を残す二分を 6 回行う。本当の零点なら、区間が 1/64 になると両端の |F| も
///    縮む（なめらかなら約 1/64）。最後の両端の値 f_a, f_b が |f_a| + |f_b| ≥ 0.5 (|f₀| + |f₁|)（縮まなかった）なら
///    「零点ではない」。途中で値が有限でなくなった辺も「零点ではない」
///    （両端の小さいほうの値だけで比べると、零点が辺の端のすぐ近くにあるとき、端の値が変わらないので見誤る）
///
/// 計算は、全部の辺をまとめて行う
fn false_crossings(eval: &mut dyn FnMut([&[f64]; 3]) -> Vec<f64>, edges: &[SignEdge]) -> Vec<bool> {
    const ROUNDS: usize = 6;
    let mut result = vec![false; edges.len()];
    if edges.is_empty() {
        return result;
    }
    let at = |e: &SignEdge, t: f64| [0, 1, 2].map(|x| e.p0[x] + (e.p1[x] - e.p0[x]) * t);
    let mut cols: [Vec<f64>; 3] = Default::default();
    let fill = |cols: &mut [Vec<f64>; 3], pts: &mut dyn Iterator<Item = [f64; 3]>| {
        for c in cols.iter_mut() {
            c.clear();
        }
        for q in pts {
            for x in 0..3 {
                cols[x].push(q[x]);
            }
        }
    };
    // 1. 直線で補間した交点
    fill(&mut cols, &mut edges.iter().map(|e| at(e, e.f0 / (e.f0 - e.f1))));
    let v = eval([&cols[0], &cols[1], &cols[2]]);
    // 残った辺: (辺の番号, ta, tb, fa, fb)
    let mut rest: Vec<(usize, f64, f64, f64, f64)> = edges
        .iter()
        .enumerate()
        .filter(|&(k, e)| !(v[k].abs() <= 0.25 * e.f0.abs().min(e.f1.abs())))
        .map(|(k, e)| (k, 0.0, 1.0, e.f0, e.f1))
        .collect();
    // 2. 二分
    for _ in 0..ROUNDS {
        if rest.is_empty() {
            break;
        }
        fill(&mut cols, &mut rest.iter().map(|&(k, ta, tb, _, _)| at(&edges[k], 0.5 * (ta + tb))));
        let v = eval([&cols[0], &cols[1], &cols[2]]);
        rest = rest
            .into_iter()
            .zip(v)
            .filter_map(|((k, ta, tb, fa, fb), fm)| {
                if !fm.is_finite() {
                    result[k] = true;
                    return None;
                }
                let t = 0.5 * (ta + tb);
                Some(if (fm < 0.0) != (fa < 0.0) { (k, ta, t, fa, fm) } else { (k, t, tb, fm, fb) })
            })
            .collect();
    }
    for (k, _, _, fa, fb) in rest {
        let e = &edges[k];
        if fa.abs() + fb.abs() >= 0.5 * (e.f0.abs() + e.f1.abs()) {
            result[k] = true;
        }
    }
    result
}

/// 符号が変わらずに F = 0 に接している所を、格子の辺ごとに探す。
/// 戻り値は 辺の番号（3 × 始点の番号 + 向き）→ 辺の上の位置 t（0〜1）。
///
/// 辺 p → q（= p + e）について、|F| が p−e から p へ減り、q から q+e へ増えていて、
/// 内側の2点の |F| が外側の2点よりずっと小さければ、その間に |F| の極小がある。
/// 三分探索で極小の位置を求め、その値がほぼ 0 なら「接している」とみなす。
/// ただし、辺を囲む4つのセルの角に符号の変わる所があれば、符号の変わる曲面が通っているので候補にしない
/// （`same_sign_around`。三分探索の前に外しても、結果は最後に外すのと同じ）。
#[cfg(test)]
fn find_touches(prog: &Program, f: &[f32], dom: Domain, n: usize) -> std::collections::HashMap<usize, f32> {
    touch_search(prog, touch_candidates_dense(f, n), dom, n)
}

/// 接する所の候補: (辺の番号, 始点 p, 向き a, F の符号, 外側の |F| の平均)
type TouchCand = (usize, [usize; 3], usize, f64, f64);

/// 4 点 p − e, p, p + e, p + 2e の値 v から、辺 p → p + e が接する所の候補になるか（なるなら (F の符号, 外側の |F| の平均)）。
/// `around` は辺を囲む4セルの角の符号がそろっているか（[`same_sign_around`]。必要なときだけ呼ぶ）
fn touch_candidate(v: [f32; 4], around: impl FnOnce() -> bool) -> Option<(f64, f64)> {
    // まず安い判定: 内側の2点が外側より 0 に近い（ほとんどの辺はここで終わる）
    if !(v[1].abs() < v[0].abs() && v[2].abs() < v[3].abs()) {
        return None;
    }
    if v.iter().any(|x| !x.is_finite()) {
        return None;
    }
    // 4点とも同じ側（0 は内側の2点だけ許す）
    let s = v[0].signum();
    if v[0] == 0.0 || v[3] * s <= 0.0 || v[1] * s < 0.0 || v[2] * s < 0.0 {
        return None;
    }
    let g = v.map(|x| x.abs() as f64);
    (g[1] + g[2] < 0.25 * (g[0] + g[3]) && around()).then_some((s as f64, 0.5 * (g[0] + g[3])))
}

/// 接する所の候補を、密な格子（全部の格子点の値 f）から集める（速くする前の版との比較用）
#[cfg(test)]
fn touch_candidates_dense(f: &[f32], n: usize) -> Vec<TouchCand> {
    let m = n + 1;
    let idx = |p: [usize; 3]| p[0] + m * (p[1] + m * p[2]);
    let mut cands: Vec<TouchCand> = Vec::new();
    for a in 0..3 {
        let stride = [1, m, m * m][a];
        for k in 0..m {
            for j in 0..m {
                // 辺の始点 p は 1 ≤ p[a] ≤ n − 2。y, z の向きなら行ごと飛ばし、x の向きなら i の範囲を狭める
                let (lo, hi) = match a {
                    0 => (1, n - 1),
                    1 if (1..n - 1).contains(&j) => (0, m),
                    2 if (1..n - 1).contains(&k) => (0, m),
                    _ => continue,
                };
                let row = m * (j + m * k);
                // 4 点 p − e, p, p + e, p + 2e の値の列（連続した場所を順に読む）
                // （lo を先に足す: row + d·stride だけだと、row = 0, d = −1 で途中が負になる）
                let cols = [-1isize, 0, 1, 2].map(|d| &f[((row + lo) as isize + d * stride as isize) as usize..][..hi - lo]);
                for (off, (((&w0, &w1), &w2), &w3)) in cols[0].iter().zip(cols[1]).zip(cols[2]).zip(cols[3]).enumerate() {
                    let p = [lo + off, j, k];
                    let get = |q: [usize; 3]| f[idx(q)];
                    if let Some((s, outer)) = touch_candidate([w0, w1, w2, w3], || same_sign_around(get, m, p, a)) {
                        cands.push((3 * idx(p) + a, p, a, s, outer));
                    }
                }
            }
        }
    }
    cands
}

/// 接する所の候補を三分探索で確かめる（[`find_touches`] を参照）
fn touch_search(prog: &Program, cands: Vec<TouchCand>, dom: Domain, n: usize) -> std::collections::HashMap<usize, f32> {
    const ROUNDS: usize = 16;
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

/// 辺 p → p + e_a を囲む4つのセルの角（合わせて 18 個の格子点）の符号が、すべて同じか。
/// 符号の変わる角が近くにあれば、そこは符号の変わる曲面が通っている（例: 格子の平面にちょうど接する球の極）ので、
/// 接するだけの所として扱わない。格子の外の点と、有限でない点は見ない。
fn same_sign_around(f: impl Fn([usize; 3]) -> f32, m: usize, p: [usize; 3], a: usize) -> bool {
    let (b, c) = ((a + 1) % 3, (a + 2) % 3);
    let mut sign = None;
    for da in 0..2 {
        for db in -1i64..=1 {
            for dc in -1i64..=1 {
                let mut q = [p[0] as i64, p[1] as i64, p[2] as i64];
                q[a] += da;
                q[b] += db;
                q[c] += dc;
                if q.iter().any(|&x| x < 0 || x >= m as i64) {
                    continue;
                }
                let v = f([q[0] as usize, q[1] as usize, q[2] as usize]);
                if !v.is_finite() {
                    continue;
                }
                // 符号の判定はメッシュを作る所と同じ（f < 0 かどうか）
                let neg = v < 0.0;
                if *sign.get_or_insert(neg) != neg {
                    return false;
                }
            }
        }
    }
    true
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
fn field_values(prog: &Program, combos: &[Vec<f64>], xyz: [&[f64]; 3]) -> Vec<f64> {
    field_values_with(prog, combos, xyz, &mut FieldScratch::default()).to_vec()
}

/// [`field_values`] の作業用の配列（層ごとに作り直さず、使い回す）
#[derive(Default)]
struct FieldScratch {
    out: Vec<C>,
    lo: Vec<f64>,
    hi: Vec<f64>,
    extra: Vec<Vec<f64>>,
    result: Vec<f64>,
}

/// [`field_values`] と同じ値を、作業用の配列 `sc` を使い回して計算する（結果は `sc` の中）
fn field_values_with<'a>(prog: &Program, combos: &[Vec<f64>], [xs, ys, zs]: [&[f64]; 3], sc: &'a mut FieldScratch) -> &'a [f64] {
    let len = xs.len();
    sc.out.resize(len, C::new(0.0, 0.0));
    sc.result.clear();
    if combos.len() == 1 && combos[0].is_empty() {
        prog.eval_batch(&[xs, ys, zs], &mut sc.out);
        sc.result.extend(sc.out.iter().map(|v| as_real(*v)));
        return &sc.result;
    }
    // 範囲の各点で F を計算して、最小値と最大値をとる
    sc.lo.clear();
    sc.lo.resize(len, f64::INFINITY);
    sc.hi.clear();
    sc.hi.resize(len, f64::NEG_INFINITY);
    sc.extra.resize(combos.first().map_or(0, |c| c.len()), Vec::new());
    for arr in sc.extra.iter_mut() {
        arr.resize(len, 0.0);
    }
    for combo in combos {
        for (arr, &v) in sc.extra.iter_mut().zip(combo) {
            arr.fill(v);
        }
        let mut inputs: Vec<&[f64]> = vec![xs, ys, zs];
        inputs.extend(sc.extra.iter().map(|a| a.as_slice()));
        prog.eval_batch(&inputs, &mut sc.out);
        for p in 0..len {
            let v = as_real(sc.out[p]);
            if v.is_finite() {
                sc.lo[p] = sc.lo[p].min(v);
                sc.hi[p] = sc.hi[p].max(v);
            }
        }
    }
    let (lo, hi) = (&sc.lo, &sc.hi);
    sc.result.extend((0..len).map(|p| if lo[p].is_finite() { lo[p].max(-hi[p]) } else { f64::NAN }));
    &sc.result
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

    /// 範囲で重ねた立体か（ふつうの陰関数は、どちらの側が中身か決まっていない）
    pub fn is_stacked(&self) -> bool {
        self.combos.iter().any(|c| !c.is_empty())
    }

    /// 表示範囲の 6 つの面での切り口（中身がはみ出す所をふさぐふた）。面からほんの少し（対角線の 1e-4 倍）内側に作る
    /// （面のちょうど上だと、描画側の切り落としの誤差で点々に欠けるため）。
    /// `clip` = (軸, 位置, 位置以下を残すか) があれば、その切り口で切り落とされる側は描かない
    pub fn box_caps(&self, dom: Domain, n: usize, clip: Option<(usize, f64, bool)>) -> Mesh {
        let size = (0..3).map(|a| (dom.hi[a] - dom.lo[a]).powi(2)).sum::<f64>().sqrt();
        let eps = size * 1e-4;
        let mut parts = Vec::new();
        for a in 0..3 {
            parts.push(self.cap_clipped(dom, a, dom.lo[a] + eps, false, n, clip));
            parts.push(self.cap_clipped(dom, a, dom.hi[a] - eps, true, n, clip));
        }
        merge(parts, false)
    }

    /// 平面「`axis` 番目の座標 = `pos`」での切り口（中身の部分）を、`n × n` マスの格子で三角形にする。
    /// 法線は、切り落とした側（`keep_below` なら座標が大きい側）を向く
    pub fn cap(&self, dom: Domain, axis: usize, pos: f64, keep_below: bool, n: usize) -> Mesh {
        self.cap_clipped(dom, axis, pos, keep_below, n, None)
    }

    /// [`Field::cap`] で、`clip` = (軸, 位置, 位置以下を残すか) の平面で切り落とされる側を除いたもの
    fn cap_clipped(
        &self,
        dom: Domain,
        axis: usize,
        pos: f64,
        keep_below: bool,
        n: usize,
        clip: Option<(usize, f64, bool)>,
    ) -> Mesh {
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
                    if let Some((ca, cpos, below)) = clip {
                        poly = clip_poly(&poly, ca, cpos, below);
                        if poly.len() < 3 {
                            continue;
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

/// 凸多角形 `poly` のうち、平面「`axis` 番目の座標 = `pos`」の残す側（`below` なら pos 以下）の部分
fn clip_poly(poly: &[[f64; 3]], axis: usize, pos: f64, below: bool) -> Vec<[f64; 3]> {
    let side = |p: &[f64; 3]| if below { pos - p[axis] } else { p[axis] - pos }; // 残す側で正
    let mut out = Vec::with_capacity(poly.len() + 1);
    for k in 0..poly.len() {
        let (p, q) = (poly[k], poly[(k + 1) % poly.len()]);
        let (sp, sq) = (side(&p), side(&q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            let t = sp / (sp - sq);
            out.push([0, 1, 2].map(|x| p[x] + (q[x] - p[x]) * t));
        }
    }
    out
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
    /// 曲面を細かくした（[`crate::adaptive`]）なら、(一様な格子の点の数, 三角形)。
    /// 点は、一様な格子の点（曲面ごとに m × m 個）を先に全部並べ、細かくして足した点をその後ろに並べる。
    /// 三角形はこの一覧を使う（粗い版は、今までどおり一様な格子の点を間引いて作る）。None なら一様な格子の三角形
    pub tris: Option<(usize, Vec<u32>)>,
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
    parametric_surface_nd_opt(sets, u, v, n, None)
}

/// [`parametric_surface_nd`] で、`tol` があれば、急に曲がる所・定義域の縁を細かくする（[`crate::adaptive`]）。
/// 分けるのは、斜辺の中点の点と、斜辺の両端の点の中点との n 次元の距離が `tol` を超える三角形と、
/// 頂点・中点で定義される所とされない所が混ざる三角形。回転は距離を変えず、射影は距離を縮めるだけなので、
/// 回した・射影した後もこの許容量に収まる。2 回分けてもずれが半分にならない所（飛びの疑い）は、それ以上分けない
pub fn parametric_surface_nd_opt(
    sets: &[Vec<Program>],
    u: (f64, f64),
    v: (f64, f64),
    n: usize,
    tol: Option<f64>,
) -> NdShape {
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
    let Some(tol) = tol.filter(|t| *t > 0.0 && dims > 0) else {
        return NdShape { dims, points, grid: Some((m, grid)), solid: false, complex: Vec::new(), tris: None };
    };
    // 曲面ごとに細かくする。足した点は、全部の曲面の一様な格子の点の後ろに並べる
    let base = sets.len() * m * m;
    let (mut extra_points, mut extra_grid, mut tris) = (Vec::new(), Vec::new(), Vec::new());
    let mut refined_any = false;
    for (s, progs) in sets.iter().enumerate() {
        let set = ProgramSet::new(progs);
        let first: Vec<Vec<f64>> = points[s * m * m * dims..(s + 1) * m * m * dims].chunks_exact(dims).map(|p| p.to_vec()).collect();
        let mut eval = |a: &[f64], b: &[f64]| {
            let mut out = Vec::with_capacity(a.len() * dims);
            set.eval_points(&[a, b], a.len(), &mut out);
            out.chunks_exact(dims).map(|p| p.to_vec()).collect::<Vec<_>>()
        };
        let mut decide = |c: &crate::adaptive::Candidate<Vec<f64>>| -> Option<f64> {
            let pts = [c.vals[0], c.vals[1], c.vals[2], c.mid];
            let defined = pts.iter().filter(|p| finite(p)).count();
            if defined == 0 {
                return None;
            }
            if defined < 4 {
                return Some(f64::INFINITY); // 定義域の縁
            }
            let e = c.mid.iter().zip(c.vals[0].iter().zip(c.vals[1])).map(|(&q, (&a, &b))| (q - 0.5 * (a + b)).powi(2)).sum::<f64>().sqrt();
            if e <= tol {
                return None;
            }
            let grand = c.history[1];
            if grand.is_finite() && e > 0.5 * grand {
                return None;
            }
            Some(e)
        };
        let limits = crate::adaptive::Limits { passes: 6, max_new_triangles: (2 * n * n).max(30_000) };
        let r = crate::adaptive::refine(n, u, v, first, &mut eval, &mut decide, limits);
        if r.uv.len() > m * m {
            refined_any = true;
        }
        let offset = base + extra_points.len() / dims;
        for (p, w) in r.uv[m * m..].iter().zip(&r.vals[m * m..]) {
            extra_points.extend_from_slice(w);
            extra_grid.extend_from_slice(&[(p[0] - u.0) as f32, (p[1] - v.0) as f32]);
        }
        let map = |k: u32| if (k as usize) < m * m { (s * m * m) as u32 + k } else { (offset + k as usize - m * m) as u32 };
        tris.extend(r.tris.iter().flat_map(|t| t.map(map)));
    }
    if !refined_any {
        return NdShape { dims, points, grid: Some((m, grid)), solid: false, complex: Vec::new(), tris: None };
    }
    points.extend_from_slice(&extra_points);
    grid.extend_from_slice(&extra_grid);
    NdShape { dims, points, grid: Some((m, grid)), solid: false, complex: Vec::new(), tris: Some((base, tris)) }
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
    NdShape { dims, points, grid: Some((m, grid)), solid: true, complex: Vec::new(), tris: None }
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
    NdShape { dims, points, grid: Some((m, grid)), solid: false, complex: Vec::new(), tris: None }
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
    NdShape { dims: d, points, grid: Some((m, grid)), solid: false, complex, tris: None }
}

/// 媒介変数表示の曲線（成分は `progs`、n 次元）。
///
/// 最初に `n` 等分して計算し、曲がっている所・途切れる所だけを細かく分け直す（適応サンプリング）。
/// 曲線が途切れる所には NaN の点が入る。`tol` は「直線で結んだときのずれ」の許容量（n 次元の距離）。
/// 回転して3次元に射影しても距離は縮むだけなので、射影した後もこの許容量に収まる。
/// 多変数の図形（媒介変数 k 個、範囲 `r`）の辺だけ: 媒介変数を 1 つだけ動かし、残りを範囲の両端のどちらかに
/// 固定した曲線を、全部の組み合わせで作る（k · 2^(k−1) 本。4 個なら正八胞体の辺 32 本）。
/// 曲線は曲がっている所だけ細かく計算する（[`parametric_curve_nd`] と同じ）。曲線どうしの間は NaN で切る
pub fn parametric_edges_nd(sets: &[Vec<Program>], r: &[(f64, f64)], n: usize, tol: f64) -> NdShape {
    let k = r.len();
    let dims = sets.first().map_or(0, |p| p.len());
    let mut points = Vec::new();
    let empty: &[f64] = &[];
    for progs in sets {
        let set = ProgramSet::new(progs);
        for a in 0..k {
            let others: Vec<usize> = (0..k).filter(|&c| c != a).collect();
            for bits in 0..1usize << others.len() {
                let fixed: Vec<(usize, f64)> = others
                    .iter()
                    .enumerate()
                    .map(|(j, &c)| (c, if bits >> j & 1 == 0 { r[c].0 } else { r[c].1 }))
                    .collect();
                let bound = set.bind(&fixed);
                if !points.is_empty() {
                    points.extend(std::iter::repeat_n(f64::NAN, dims)); // 辺どうしの間で線を切る
                }
                let eval = |ts: &[f64]| {
                    let mut inputs = vec![empty; k];
                    inputs[a] = ts;
                    eval_nd(&bound, &inputs, ts.len())
                };
                points.extend(curve_nd(&eval, dims, r[a], n, tol));
            }
        }
    }
    NdShape { dims, points, grid: None, solid: false, complex: Vec::new(), tris: None }
}

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
    NdShape { dims, points, grid: None, solid: false, complex: Vec::new(), tris: None }
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
pub fn merge(mut meshes: Vec<Mesh>, curve: bool) -> Mesh {
    // 自分の頂点を持つ粗い版（陰関数）があれば、粗い版どうしもつなげる。粗い版の無いメッシュは、
    // そのまま（三角形の番号の粗い版があればそれ）を粗い版として使う
    let lod_mesh = if !curve && meshes.iter().any(|m| m.lod_mesh.is_some()) {
        let parts = meshes
            .iter_mut()
            .map(|m| match m.lod_mesh.take() {
                Some(l) => *l,
                None => Mesh {
                    positions: m.positions.clone(),
                    normals: m.normals.clone(),
                    indices: if m.lod.is_empty() { m.indices.clone() } else { m.lod.clone() },
                    ..Default::default()
                },
            })
            .collect();
        Some(Box::new(merge(parts, false)))
    } else {
        None
    };
    let mut out = merge_inner(meshes, curve);
    if lod_mesh.is_some() {
        out.lod = Vec::new();
        out.lod_mesh = lod_mesh;
    }
    out
}

fn merge_inner(meshes: Vec<Mesh>, curve: bool) -> Mesh {
    let mut out = Mesh::default();
    let grid = meshes.iter().any(|m| !m.grid_coords.is_empty());
    let complex = meshes.iter().any(|m| !m.complex.is_empty());
    let lod = meshes.iter().any(|m| !m.lod.is_empty());
    out.keep_full = meshes.iter().any(|m| m.keep_full);
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
    let complex = shapes.iter().any(|s| !s.complex.is_empty());
    // 細かくした曲面があれば、一様な格子の点を全部先に並べ、足した点をその後ろに並べ直す（[`NdShape::tris`]）
    if let (Some(m), true) = (m, shapes.iter().any(|s| s.tris.is_some())) {
        let gd = if solid { 3 } else { 2 };
        let count = |s: &NdShape| s.points.len() / dims.max(1);
        let base_of = |s: &NdShape| s.tris.as_ref().map_or(count(s), |t| t.0);
        let base: usize = shapes.iter().map(base_of).sum();
        let mut out = NdShape {
            dims,
            points: Vec::new(),
            grid: Some((m, Vec::new())),
            solid,
            complex: Vec::new(),
            tris: Some((base, Vec::new())),
        };
        let (mut base_at, mut extra_at) = (0usize, base);
        let mut extra = (Vec::new(), Vec::new(), Vec::new());
        for s in &shapes {
            let (c, b) = (count(s), base_of(s));
            let grid = &s.grid.as_ref().map_or(&[][..], |g| &g.1[..]);
            let cx = |k: usize| -> [f32; 3] {
                if s.complex.is_empty() { [0.0; 3] } else { [s.complex[3 * k], s.complex[3 * k + 1], s.complex[3 * k + 2]] }
            };
            let (o, g) = out.grid.as_mut().map(|g| (&mut out.points, &mut g.1)).unwrap();
            o.extend_from_slice(&s.points[..b * dims]);
            g.extend_from_slice(&grid[..b * gd]);
            extra.0.extend_from_slice(&s.points[b * dims..]);
            extra.1.extend_from_slice(&grid[b * gd..]);
            if complex {
                for k in 0..b {
                    out.complex.extend_from_slice(&cx(k));
                }
                for k in b..c {
                    extra.2.extend_from_slice(&cx(k));
                }
            }
            let map = |k: u32| if (k as usize) < b { (base_at + k as usize) as u32 } else { (extra_at + k as usize - b) as u32 };
            let tris = &mut out.tris.as_mut().unwrap().1;
            match &s.tris {
                Some((_, t)) => tris.extend(t.iter().map(|&k| map(k))),
                None => tris.extend(grid_tris(m, b).into_iter().map(map)),
            }
            base_at += b;
            extra_at += c - b;
        }
        out.points.extend_from_slice(&extra.0);
        out.grid.as_mut().unwrap().1.extend_from_slice(&extra.1);
        out.complex.extend_from_slice(&extra.2);
        return Some(out);
    }
    let mut out =
        NdShape { dims, points: Vec::new(), grid: m.map(|m| (m, Vec::new())), solid, complex: Vec::new(), tris: None };
    for s in shapes {
        if m.is_none() && !out.points.is_empty() {
            out.points.extend(std::iter::repeat_n(f64::NAN, dims)); // 曲線どうしの間で線を切る
        }
        out.points.extend_from_slice(&s.points);
        if complex {
            if s.complex.is_empty() {
                out.complex.extend(std::iter::repeat_n(0.0, s.points.len() / dims.max(1) * 3));
            } else {
                out.complex.extend_from_slice(&s.complex);
            }
        }
        if let (Some((_, g)), Some((_, sg))) = (out.grid.as_mut(), s.grid) {
            g.extend_from_slice(&sg);
        }
    }
    Some(out)
}

/// 一様な格子（m × m 個の点が `count / m²` 枚）の三角形。[`project`] と同じ並び・向き
fn grid_tris(m: usize, count: usize) -> Vec<u32> {
    let n = m - 1;
    let mut out = Vec::with_capacity(count / (m * m) * n * n * 6);
    for base in (0..count).step_by(m * m) {
        for j in 0..n {
            for i in 0..n {
                let a = (base + j * m + i) as u32;
                let (b, c, e) = (a + 1, a + m as u32, a + m as u32 + 1);
                out.extend_from_slice(&[a, b, e, a, e, c]);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 2D 表示（画面の縦の座標を 0 にして、横・奥の平面に描く）
// ---------------------------------------------------------------------------

/// 縦の座標を 0 にして平らにする（2D 表示）。曲面の法線はすべて +縦 の向きにする
pub fn flatten(mesh: &mut Mesh, curve: bool) {
    mesh.lod_mesh = None; // 平らにした後の粗い版は作らない（2D 表示は軽い）
    for p in mesh.positions.chunks_exact_mut(3) {
        if p[2].is_finite() {
            p[2] = 0.0;
        }
    }
    if !curve {
        mesh.normals = (0..mesh.positions.len() / 3).flat_map(|_| [0.0f32, 0.0, 1.0]).collect();
    }
}

/// 2D 表示の「切り口」: 曲面のメッシュと、平面「縦の座標 = `level`」の交わりの線（曲線のメッシュ）。
/// 三角形ごとに、平面をまたぐ2つの辺の上の点を結ぶ。線分どうしの間は NaN で切る
pub fn slice_mesh(mesh: &Mesh, level: f32) -> Mesh {
    let p = &mesh.positions;
    let at = |k: u32| {
        let k = k as usize * 3;
        [p[k], p[k + 1], p[k + 2]]
    };
    let mut out: Vec<f32> = Vec::new();
    for t in mesh.indices.chunks_exact(3) {
        let v = [at(t[0]), at(t[1]), at(t[2])];
        // 平面の上にちょうど乗った頂点は、わずかに上にあるとみなす（同じ線を2回引かないように）
        let d = v.map(|q| if q[2] == level { f32::MIN_POSITIVE } else { q[2] - level });
        let mut pts: Vec<[f32; 3]> = Vec::with_capacity(2);
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            if (d[a] > 0.0) != (d[b] > 0.0) {
                let s = d[a] / (d[a] - d[b]);
                pts.push([0, 1, 2].map(|c| if c == 2 { level } else { v[a][c] + (v[b][c] - v[a][c]) * s }));
            }
        }
        // 長さの無い線は描かない（描画側が点とみなして丸を付ける）
        if pts.len() == 2 && pts[0] != pts[1] {
            out.extend_from_slice(&pts[0]);
            out.extend_from_slice(&pts[1]);
            out.extend_from_slice(&[f32::NAN; 3]);
        }
    }
    Mesh { positions: out, ..Default::default() }
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
            ComplexMode::Off | ComplexMode::Param => as_real(z),
            ComplexMode::Abs => z.norm(),
            ComplexMode::Re => z.re,
            ComplexMode::Im => z.im,
            ComplexMode::Arg => z.arg(),
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
/// 2D 表示の陰関数の曲線 F(横, 奥, 0) = 0（マーチング・スクエア法）。
/// `prog` の変数は (横, 奥, 縦) の3つか、縦の無い (横, 奥) の2つ（`z = f(x, y)` の切り口 f = 0 など）
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
    if prog.vars().len() == 2 {
        prog.eval_batch(&[&xs, &ys], &mut out);
    } else {
        prog.eval_batch(&[&xs, &ys, &zs], &mut out);
    }
    let f: Vec<f64> = out.iter().map(|v| as_real(*v)).collect();
    // 符号は変わるが F = 0 になる点が無い辺（tan(x) = y の極の縦線など。[`false_crossings`]）。
    // 辺の番号は 2 × 小さいほうの格子点 + (縦なら 1)
    let not_zero: std::collections::HashSet<usize> = {
        let mut ids = Vec::new();
        let mut edges = Vec::new();
        for k in 0..m * m {
            let (i, j) = (k % m, k / m);
            for (dir, k1) in [(0, (i + 1 < m).then_some(k + 1)), (1, (j + 1 < m).then_some(k + m))] {
                let Some(k1) = k1 else { continue };
                let (f0, f1) = (f[k], f[k1]);
                if (f0 < 0.0) != (f1 < 0.0) && f0.is_finite() && f1.is_finite() {
                    edges.push(SignEdge { p0: [xs[k], ys[k], 0.0], p1: [xs[k1], ys[k1], 0.0], f0, f1 });
                    ids.push(2 * k + dir);
                }
            }
        }
        let two = prog.vars().len() == 2;
        let mut buf = Vec::new();
        let mut eval = |[x, y, z]: [&[f64]; 3]| {
            buf.resize(x.len(), C::new(0.0, 0.0));
            if two {
                prog.eval_batch(&[x, y], &mut buf);
            } else {
                prog.eval_batch(&[x, y, z], &mut buf);
            }
            buf.iter().map(|v| as_real(*v)).collect()
        };
        let flags = false_crossings(&mut eval, &edges);
        ids.into_iter().zip(flags).filter(|&(_, bad)| bad).map(|(id, _)| id).collect()
    };
    let edge_id = |ka: usize, kb: usize| 2 * ka.min(kb) + (ka.abs_diff(kb) == m) as usize;
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
                    let (ka, kb) = (ks[a], ks[b]);
                    if !not_zero.is_empty() && not_zero.contains(&edge_id(ka, kb)) {
                        continue; // F が飛ぶだけで、0 になる点は無い
                    }
                    let t = v[a] / (v[a] - v[b]);
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
                // 長さの無い線（ちょうど 0 になる格子点で、2 つの辺の交わる点が同じになった所。floor の段など）は描かない。
                // 描くと、描画側が「長さの無い曲線 = 点」とみなして丸を付ける
                // （比べるのは画面に渡す f32 に丸めた後: f64 では別でも、丸めると同じ点になることがある）
                let (pa, pb) = (cross[a].map(|v| v as f32), cross[b].map(|v| v as f32));
                if pa == pb {
                    continue;
                }
                pts.extend_from_slice(&[pa[0], pa[1], 0.0]);
                pts.extend_from_slice(&[pb[0], pb[1], 0.0]);
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
    // 曲面ごとに m × m 個の点が並んでいる。立体は6枚ずつ。
    // 細かくした曲面は、一様な格子の点（base 個）の後ろに足した点が並び、三角形は tris を使う
    let per = if shape.solid { 6 * m * m } else { m * m };
    let (grid_count, tris) = match &shape.tris {
        Some((base, t)) => (*base, Some(t)),
        None => (count, None),
    };
    // 粗い版: 格子の点を stride 個おきに使う（最後の行・列は必ず入れるので、面の端は変わらない）
    let quads = tris.map_or(grid_count / (m * m) * n * n, |t| t.len() / 6);
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
    if let Some(t) = tris {
        for tri in t.chunks_exact(3) {
            let v = [index_of[tri[0] as usize], index_of[tri[1] as usize], index_of[tri[2] as usize]];
            if !v.contains(&NONE) {
                mesh.indices.extend_from_slice(&v);
            }
        }
    }
    for start in (0..grid_count).step_by(per) {
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
            if tris.is_some() {
                continue; // 三角形は tris から作った
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
        // 円錐の先（原点）の周りは、形が急に変わるので細かくなる
        assert!(m.positions.len() >= 121 * 3);
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

    /// `explicit_surface_full` と同じ入力で `find_jumps_with` を呼び、(辺の番号, 位置 t) を番号順に並べたもの
    fn jumps_of(src: &str, early_exit: bool) -> Vec<(usize, f64)> {
        let p = prog(src, &["x", "y"]);
        // 範囲を少しずらして、極 x = 0 が格子の点にも二分の点にもならないようにする
        // （値が有限でない点を踏むと、その辺は段差とみなされない）
        let dom = Domain { lo: [-4.93, -4.93, -5.0], hi: [5.07, 5.07, 5.0] };
        let n = 60;
        let m = n + 1;
        let (mut us, mut vs) = (Vec::new(), Vec::new());
        for j in 0..m {
            for i in 0..m {
                us.push(dom.at(0, i as f64, n));
                vs.push(dom.at(1, j as f64, n));
            }
        }
        let mut vals = vec![C::new(0.0, 0.0); m * m];
        p.eval_batch(&[&us, &vs], &mut vals);
        let h = dom.hi[2] - dom.lo[2];
        let (wlo, whi) = (dom.lo[2] - h, dom.hi[2] + h);
        let clamped = |z: C| height(z, ComplexMode::Off).clamp(wlo, whi);
        let hs: Vec<f64> = vals.iter().map(|&z| clamped(z)).collect();
        let usable = |w: f64| w.is_finite();
        let mut edges = Vec::new();
        for j in 0..m {
            for i in 0..m {
                let k = j * m + i;
                for (ok, k1) in [(i + 1 < m, k + 1), (j + 1 < m, k + m), (i + 1 < m && j + 1 < m, k + m + 1)] {
                    if ok {
                        edges.push((k, k1));
                    }
                }
            }
        }
        let jumps = find_jumps_with(&p, &us, &vs, &vals, &hs, &edges, &clamped, h * 0.03, &usable, early_exit);
        let mut out: Vec<(usize, f64)> = jumps.iter().map(|(&id, j)| (id, j.t)).collect();
        out.sort_by_key(|&(id, _)| id);
        out
    }

    #[test]
    fn jumps_unchanged_by_early_exit() {
        // 途中で候補を外しても、見つかる段差（辺と位置）は外さないときと同じ
        for src in ["floor(2x)", "tan(x) y", "1/x + 1/y", "floor(x) + floor(y)", "sign(x) + y"] {
            let (fast, full) = (jumps_of(src, true), jumps_of(src, false));
            assert!(!full.is_empty(), "{src}: 段差が見つからない");
            assert_eq!(fast, full, "{src}");
        }
    }

    #[test]
    fn steps() {
        // floor(2x) の段差: 段差の所で三角形を切り分けるので、どの三角形も段差をまたがない
        // （3頂点の値がすべて同じ整数）。段差の縁は x = 0.5 k の線の上にそろう。
        // 高さの範囲は値（−2〜1）が全部入る広さにする（範囲の外の段は描かないので）
        let d = Domain { lo: [-1.0, -1.0, -3.0], hi: [1.0, 1.0, 2.0] };
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
        // 極の周りに穴が空かない。表示範囲の高さ ±3 の外に出る所（|x/(x² + y²)| > 3: 半径 1/6 の円 2 つ）は
        // 上端・下端で切って描かないので、平面に落とした面積は 4 − 2π/36 以上（穴があればこれより小さくなる）
        let d = Domain { lo: [-1.0, -1.0, -3.0], hi: [1.0, 1.0, 3.0] };
        let p = prog("1 / (x + i y)", &["x", "y"]);
        let m = explicit_surface_full(&p, d, 40, 2, ComplexMode::Re, false);
        let mut area = 0.0;
        for t in m.indices.chunks_exact(3) {
            let v = |k: u32| [m.positions[k as usize * 3] as f64, m.positions[k as usize * 3 + 1] as f64];
            let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
            area += ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0;
        }
        let visible = 4.0 - 2.0 * std::f64::consts::PI / 36.0;
        assert!(area > visible - 0.02 && area < 4.0 + 1e-9, "面積 {area}");
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

    /// 陽関数の三角形を (x, y) の平面に落としたもの
    fn flat_tris(m: &Mesh) -> (Vec<[f64; 2]>, Vec<[u32; 3]>) {
        let uv = m
            .positions
            .chunks_exact(3)
            .map(|q| [q[0] as f64, q[1] as f64])
            .collect();
        (
            uv,
            m.indices
                .chunks_exact(3)
                .map(|t| [t[0], t[1], t[2]])
                .collect(),
        )
    }

    #[test]
    fn adaptive_mesh_is_conforming() {
        // 細かくした所でも、辺の途中に別の頂点が乗らない（面に隙間が無い）
        let d = Domain {
            lo: [-5.0, -5.0, -5.0],
            hi: [5.0, 5.0, 5.0],
        };
        for (src, mode) in [
            ("gamma(x + i y)", ComplexMode::Abs),
            ("1/(x^2+y^2)", ComplexMode::Off),
            ("sqrt(4-x^2-y^2)", ComplexMode::Off),
            ("tan(x) y", ComplexMode::Off),
        ] {
            let p = prog(src, &["x", "y"]);
            let m = explicit_surface_with(&p, d, 60, 2, mode);
            let (uv, tris) = flat_tris(&m);
            crate::adaptive::tests::assert_conforming(&uv, &tris);
            let uniform = explicit_surface_opt(&p, d, 60, 2, mode, false, false);
            assert!(
                m.indices.len() > uniform.indices.len(),
                "{src}: 細かくなっていない"
            );
        }
    }

    #[test]
    fn narrow_pole_reaches_top() {
        // |Γ(x + iy)| の極は格子より細い。細かくすると、x = 0, −1, −2, −3 の極で表示範囲の上（z = 5）まで届き、
        // x = −4 の極も見える（一様な格子では x = −3 は途中で切れ、x = −4 は高さ 1 にも届かなかった）
        let d = Domain {
            lo: [-5.0, -5.0, -5.0],
            hi: [5.0, 5.0, 5.0],
        };
        let m = explicit_surface_with(
            &prog("gamma(x + i y)", &["x", "y"]),
            d,
            150,
            2,
            ComplexMode::Abs,
        );
        let near = |x0: f64, z: f64| {
            m.positions.chunks_exact(3).any(|q| {
                ((q[0] as f64 - x0).powi(2) + (q[1] as f64).powi(2)).sqrt() < 0.4
                    && q[2] as f64 >= z
            })
        };
        for x0 in [0.0, -1.0, -2.0, -3.0] {
            assert!(near(x0, 5.0), "x = {x0} の極が上まで届かない");
        }
        assert!(near(-4.0, 1.0), "x = −4 の極が見えない");
        // 三角形の数は、一様な格子の 2 倍 + 3 万個以下
        assert!(m.indices.len() / 3 <= 2 * 2 * 150 * 150 + 30_000);
    }

    #[test]
    fn domain_edge_is_smooth() {
        // sqrt(4 − x² − y²) の縁（三角形が 1 つだけの辺）は、円 r = 2 のすぐ近くまで来る（格子の間隔は 0.067）
        let d = Domain {
            lo: [-5.0, -5.0, -5.0],
            hi: [5.0, 5.0, 5.0],
        };
        let m = explicit_surface(&prog("sqrt(4-x^2-y^2)", &["x", "y"]), d, 150, 2);
        let mut count: std::collections::HashMap<(u32, u32), usize> =
            std::collections::HashMap::new();
        for t in m.indices.chunks_exact(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let mut worst = 0.0f64;
        for (&(a, b), &c) in &count {
            if c == 1 {
                for k in [a, b] {
                    let q = &m.positions[3 * k as usize..3 * k as usize + 3];
                    let r = ((q[0] * q[0] + q[1] * q[1]) as f64).sqrt();
                    worst = worst.max(2.0 - r);
                }
            }
        }
        assert!(worst < 0.02, "縁が円から {worst} 離れている");
    }

    #[test]
    fn rim_is_cut_exactly_at_the_top() {
        // 1/sqrt(1 − x² − y²) は表示範囲の上端 z = 6 を、半径 √(1 − 1/36) の円で横切る。
        // 上端で切った縁の点は、格子の細かさに関係なく、この円のすぐ近くにある（切れ込み・トゲが無い）
        let d = Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
        let m = explicit_surface(&prog("1/sqrt(1 - x^2 - y^2)", &["x", "y"]), d, 150, 2);
        let r0 = (1.0f64 - 1.0 / 36.0).sqrt();
        let rim: Vec<f64> = m
            .positions
            .chunks_exact(3)
            .filter(|q| q[2] == 6.0)
            .map(|q| ((q[0] * q[0] + q[1] * q[1]) as f64).sqrt())
            .collect();
        assert!(rim.len() > 50, "{}", rim.len());
        assert!(rim.iter().all(|r| (r - r0).abs() < 1e-4), "{:?}", rim.iter().map(|r| r - r0).fold(0.0f64, |a, b| a.max(b.abs())));
        // 上端より上の頂点は描かない
        assert!(m.indices.iter().all(|&k| m.positions[3 * k as usize + 2] <= 6.0));
        // 穴が無い: 三角形が 1 つだけの辺（縁）は、上端の円の上だけにある（定義域の縁 r = 1 の近くの三角形も描くので、
        // 上端のすぐ下に切れ込みができない）
        let mut count: std::collections::HashMap<(u32, u32), usize> = std::collections::HashMap::new();
        for t in m.indices.chunks_exact(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let z = |k: u32| m.positions[3 * k as usize + 2];
        let holes = count.iter().filter(|&(&(a, b), &c)| c == 1 && !(z(a) == 6.0 && z(b) == 6.0)).count();
        assert_eq!(holes, 0);
    }

    #[test]
    fn smooth_surface_is_not_refined() {
        // なめらかな曲面は、一様な格子のまま（150 × 150 × 2 = 45,000 個の三角形）。
        // 表示範囲の上端・下端で切られないよう、高さの範囲は値より広くする（x² − y² は ±25）
        let d = Domain {
            lo: [-5.0, -5.0, -30.0],
            hi: [5.0, 5.0, 30.0],
        };
        for src in ["sin(x) cos(y)", "x^2 - y^2"] {
            let m = explicit_surface(&prog(src, &["x", "y"]), d, 150, 2);
            assert_eq!(m.indices.len() / 3, 45_000, "{src}");
        }
    }

    #[test]
    fn parametric_bump_is_refined() {
        // 格子より細い山 exp(−200 (u² + v²))（山の頂上が格子の点からずれる範囲）。細かくすると、山の上の点が増えて頂上に届く
        let p = progs3(["u", "v", "exp(-200 (u^2 + v^2))"], &["u", "v"]);
        let r = (-5.03, 4.97);
        let tol = 3f64.sqrt() * 10.0 * 1e-3; // 表示範囲 ±5 の対角線の 10⁻³ 倍（lib.rs と同じ）
        let uniform = parametric_surface_nd(std::slice::from_ref(&p), r, r, 150);
        let fine = parametric_surface_nd_opt(&[p], r, r, 150, Some(tol));
        let high = |s: &NdShape| s.points.chunks_exact(3).filter(|q| q[2] > 0.1).count();
        let top = |s: &NdShape| s.points.chunks_exact(3).map(|q| q[2]).fold(0.0, f64::max);
        assert!(high(&uniform) < 20, "{}", high(&uniform));
        assert!(high(&fine) >= 40, "{}", high(&fine));
        assert!(top(&fine) >= 0.9, "{}", top(&fine));
        // 射影した曲面にも、細かくした三角形が使われる
        let m = project(&fine, &View::identity(3), Domain { lo: [-5.0; 3], hi: [5.0; 3] });
        assert!(m.indices.len() / 3 > 2 * 150 * 150);
    }

    #[test]
    fn parametric_adaptive_is_conforming() {
        // 媒介変数の平面 (u, v) の上で、細かくした三角形の継ぎ目に隙間が無い（リストで 2 枚にしても）
        let bump = progs3(["u", "v", "exp(-200 (u^2 + v^2))"], &["u", "v"]);
        let edge = progs3(["u", "v", "sqrt(1 - u^2 - v^2)"], &["u", "v"]);
        let s = parametric_surface_nd_opt(&[bump, edge], (-2.0, 2.0), (-2.0, 2.0), 40, Some(0.01));
        let (base, tris) = s.tris.clone().expect("細かくなっていない");
        assert_eq!(base, 2 * 41 * 41);
        let grid = &s.grid.as_ref().unwrap().1;
        // 2 枚の曲面の点は (u, v) が重なるので、1 枚ずつ調べる（三角形は曲面の順に並ぶ）
        let count = s.points.len() / 3;
        let mut sheet = vec![usize::MAX; count];
        for t in tris.chunks_exact(3) {
            let k = t.iter().map(|&k| k as usize).find(|&k| k < base).map_or(usize::MAX, |k| k / (41 * 41));
            for &v in t {
                sheet[v as usize] = sheet[v as usize].min(k);
            }
        }
        for which in 0..2 {
            let uv: Vec<[f64; 2]> = (0..count).map(|k| [grid[2 * k] as f64, grid[2 * k + 1] as f64]).collect();
            let mine: Vec<[u32; 3]> = tris
                .chunks_exact(3)
                .filter(|t| t.iter().all(|&k| sheet[k as usize] == which))
                .map(|t| [t[0], t[1], t[2]])
                .collect();
            assert!(mine.len() >= 2 * 40 * 40 / 4);
            crate::adaptive::tests::assert_conforming(&uv, &mine);
        }
    }

    #[test]
    fn merge_refined_and_uniform_shapes() {
        // 細かくした曲面と一様な曲面をつなげても、三角形の数と点の対応がそのまま
        let bump = progs3(["u", "v", "exp(-200 (u^2 + v^2))"], &["u", "v"]);
        let plane = progs3(["u", "v", "1"], &["u", "v"]);
        let d = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        let a = parametric_surface_nd_opt(&[bump], (-2.03, 1.97), (-2.03, 1.97), 30, Some(0.01));
        let b = parametric_surface_nd(&[plane], (-2.0, 2.0), (-2.0, 2.0), 30);
        assert!(a.tris.is_some() && b.tris.is_none());
        let (ma, mb) = (project(&a, &View::identity(3), d), project(&b, &View::identity(3), d));
        let merged = merge_shapes(vec![a, b]).unwrap();
        let mm = project(&merged, &View::identity(3), d);
        assert_eq!(mm.indices.len(), ma.indices.len() + mb.indices.len());
        assert_eq!(mm.positions.len(), ma.positions.len() + mb.positions.len());
        // 平面 z = 1 の三角形は、z = 1 の点だけでできている
        let z = |k: u32| mm.positions[3 * k as usize + 2];
        let flat = mm.indices.chunks_exact(3).filter(|t| t.iter().all(|&k| z(k) == 1.0)).count();
        assert_eq!(flat, mb.indices.len() / 3);
    }

    #[test]
    fn tesseract_has_32_edges() {
        // (t₁, t₂, t₃, t₄) の辺は 32 本、媒介変数 3 つなら 12 本（NaN で区切られた曲線の数）
        let curves = |s: &NdShape| {
            let d = s.dims;
            let mut count = 0;
            let mut inside = false;
            for p in s.points.chunks_exact(d) {
                let ok = p.iter().all(|v| v.is_finite());
                if ok && !inside {
                    count += 1;
                }
                inside = ok;
            }
            count
        };
        let names = ["t1", "t2", "t3", "t4"];
        let p4: Vec<Program> = ["t_1", "t_2", "t_3", "t_4"].iter().map(|v| prog(v, &names)).collect();
        let s = parametric_edges_nd(&[p4], &[(-1.0, 1.0); 4], 400, 1e-3);
        assert_eq!(curves(&s), 32);
        // 頂点はどれも、座標が ±1 か、1 つの座標だけが範囲の中
        for p in s.points.chunks_exact(4).filter(|p| p[0].is_finite()) {
            assert!(p.iter().filter(|v| (v.abs() - 1.0).abs() > 1e-12).count() <= 1, "{p:?}");
        }
        let names = ["u", "v", "t"];
        let p3: Vec<Program> = ["u cos(v)", "u sin(v)", "t"].iter().map(|e| prog(e, &names)).collect();
        assert_eq!(curves(&parametric_edges_nd(&[p3], &[(0.0, 1.0), (0.0, 6.0), (0.0, 1.0)], 400, 1e-3)), 12);
    }

    #[test]
    fn box_caps_cover_the_faces() {
        // x² + y² + z² = a² を a の範囲で重ねた立体（中身の詰まった球）の、表示範囲 ±5 の面のふた
        let p = prog("x^2 + y^2 + z^2 - a^2", &["x", "y", "z", "a"]);
        let d = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        let area_of = |m: &Mesh, axis: usize, at: f64| {
            let mut area = 0.0;
            for t in m.indices.chunks_exact(3) {
                let v = |k: u32| [0, 1, 2].map(|x| m.positions[3 * k as usize + x] as f64);
                let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                if [a, b, c].iter().all(|q| (q[axis] - at).abs() < 0.01) {
                    let (e1, e2) = ([0, 1, 2].map(|x| b[x] - a[x]), [0, 1, 2].map(|x| c[x] - a[x]));
                    area += 0.5 * (e1[(axis + 1) % 3] * e2[(axis + 2) % 3] - e1[(axis + 2) % 3] * e2[(axis + 1) % 3]).abs();
                }
            }
            area
        };
        // 半径 10 の球は箱を丸ごと含む: どの面も全体（100）がふさがる
        let f = Field::new(&p, &[(0.0, 10.0)], 24, d);
        assert!(f.is_stacked());
        let m = f.box_caps(d, 40, None);
        for axis in 0..3 {
            for at in [-5.0, 5.0] {
                let a = area_of(&m, axis, at);
                assert!((a - 100.0).abs() < 3.0, "面 {axis} {at}: {a}");
            }
        }
        // 外向きの法線
        for (q, nrm) in m.positions.chunks_exact(3).zip(m.normals.chunks_exact(3)) {
            let axis = (0..3).find(|&a| nrm[a] != 0.0).unwrap();
            assert!((q[axis].abs() - 5.0).abs() < 0.01);
            assert_eq!(nrm[axis].signum(), q[axis].signum());
        }
        // 半径 6: 各面のふたは半径 √11 の円
        let m = Field::new(&p, &[(0.0, 6.0)], 24, d).box_caps(d, 80, None);
        for axis in 0..3 {
            let a = area_of(&m, axis, 5.0);
            assert!((a - 11.0 * std::f64::consts::PI).abs() < 0.05 * 11.0 * std::f64::consts::PI, "{a}");
        }
        // 面に届かない（半径 4）なら空
        assert!(Field::new(&p, &[(0.0, 4.0)], 24, d).box_caps(d, 40, None).indices.is_empty());
        // 切り口 x ≤ 0 を残すなら、x > 0 の部分は描かない（x = 5 の面は無くなり、ほかの面は半分）
        let m = f.box_caps(d, 40, Some((0, 0.0, true)));
        assert!(m.positions.chunks_exact(3).all(|q| q[0] <= 1e-9));
        assert!((area_of(&m, 1, 5.0) - 50.0).abs() < 2.0);
        // ふつうの陰関数は、範囲で重ねた立体ではない
        assert!(!Field::new(&prog("x^2 + y^2 + z^2 - 4", &AXES), &[], 0, d).is_stacked());
    }

    #[test]
    fn smooth_torus_is_not_refined() {
        // ふつうのトーラスは、一様な格子のまま（点の数も同じ）
        let torus = progs3(["(3 + cos v) cos u", "(3 + cos v) sin u", "sin v"], &["u", "v"]);
        let tau = std::f64::consts::TAU;
        let tol = 3f64.sqrt() * 10.0 * 1e-3;
        let s = parametric_surface_nd_opt(&[torus], (0.0, tau), (0.0, tau), 150, Some(tol));
        assert!(s.tris.is_none());
        assert_eq!(s.points.len(), 151 * 151 * 3);
    }

    #[test]
    fn undefined_region_is_removed() {
        // x < 0 では sqrt(x) が実数にならないので、そこは三角形が無くなる
        let full = explicit_surface(&prog("0", &["x", "y"]), dom(), 10, 2);
        let half = explicit_surface(&prog("sqrt(x)", &["x", "y"]), dom(), 10, 2);
        assert!(!half.indices.is_empty());
        // 平面に落とした面積: 全体は 2 × 2 = 4、x ≥ 0 の所だけなら 2（縁は細かくするので、ほぼ 2）
        let (a_full, a_half) = (flat_area(&full), flat_area(&half));
        assert!((a_full - 4.0).abs() < 1e-9);
        assert!((a_half - 2.0).abs() < 0.03, "{a_half}");
    }

    /// 三角形を xy 平面に落とした面積の合計
    fn flat_area(m: &Mesh) -> f64 {
        let mut area = 0.0;
        for t in m.indices.chunks_exact(3) {
            let v = |k: u32| {
                [
                    m.positions[k as usize * 3] as f64,
                    m.positions[k as usize * 3 + 1] as f64,
                ]
            };
            let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
            area += ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() / 2.0;
        }
        area
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

    /// 三角形が1つだけの辺（縁）の数と、3つ以上の三角形が集まる辺（重なり）の数
    fn bad_edges(m: &Mesh) -> (usize, usize) {
        let mut count = std::collections::HashMap::new();
        for t in m.indices.chunks_exact(3) {
            for e in 0..3 {
                let (u, v) = (t[e], t[(e + 1) % 3]);
                *count.entry((u.min(v), u.max(v))).or_insert(0usize) += 1;
            }
        }
        (count.values().filter(|&&c| c == 1).count(), count.values().filter(|&&c| c >= 3).count())
    }

    #[test]
    fn implicit_mesh_unchanged() {
        // 速くした implicit_solid と、速くする前の版（implicit_solid_reference）の出力（頂点・三角形の集合）が同じ。
        // F が飛ぶ所（tan の極など）は、その後で面を張らないようにしたので（implicit_pole_is_not_a_surface）、ここでは比べない
        let big = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        let cases: Vec<(&str, Vec<&str>, Domain, usize, Vec<(f64, f64)>)> = vec![
            ("x^2+y^2+z^2-4.01", AXES.to_vec(), big, 40, vec![]),
            ("x^2+y^2+z^2-4", AXES.to_vec(), big, 40, vec![]),
            ("sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)", AXES.to_vec(), big, 40, vec![]),
            ("(x^2 + y^2 + z^2 - 1) (x^2 + y^2 + z^2 - 1.3225)", AXES.to_vec(), Domain { lo: [-2.0; 3], hi: [2.0; 3] }, 40, vec![]),
            ("(x^2 + y^2 + z^2 - 0.49)^2", AXES.to_vec(), dom(), 30, vec![]),
            ("x y^2", AXES.to_vec(), dom(), 21, vec![]),
            ("x^2 + y^2 + z^2 - r^2", vec!["x", "y", "z", "r"], dom(), 30, vec![(0.3, 0.7)]),
        ];
        for (src, vars, d, n, ranges) in cases {
            let p = prog(src, &vars);
            let a = implicit_solid(&p, d, n, &ranges, 8);
            let b = implicit_solid_reference(&p, d, n, &ranges, 8);
            assert!(!a.indices.is_empty(), "{src}");
            // 空のブロックを飛ばすようにして（S9）、頂点の順番はブロックの順になったので、集合で比べる
            assert!(mesh_sets(&a) == mesh_sets(&b), "{src}: 頂点・三角形の集合が違う");
        }
    }

    /// 頂点のうち、極の平面 x = c の近く（|x − c| < h）で |g(x) − z| > 2 のもの（極の上の偽の面）の数
    fn fake_pole_vertices(m: &Mesh, poles: &[f64], h: f64, g: impl Fn(f64) -> f64) -> usize {
        m.positions
            .chunks_exact(3)
            .filter(|q| {
                let (x, z) = (q[0] as f64, q[2] as f64);
                poles.iter().any(|c| (x - c).abs() < h) && (g(x) - z).abs() > 2.0
            })
            .count()
    }

    #[test]
    fn implicit_pole_is_not_a_surface() {
        // tan(x) = z の極 x = ±π/2, ±3π/2 では F が +∞ から −∞ へ飛ぶ。そこに偽の面を張らない（変更前は頂点の約半分）
        let d = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        let h = 10.0 / 80.0;
        let poles: Vec<f64> = [-1.5, -0.5, 0.5, 1.5].iter().map(|k| k * std::f64::consts::PI).collect();
        let m = implicit_surface(&prog("tan(x) - z", &AXES), d, 80);
        assert!(m.positions.len() / 3 > 10_000, "本当の曲面は残る");
        assert_eq!(fake_pole_vertices(&m, &poles, h, f64::tan), 0);
        // 1/x = z（x の範囲をずらして、極 x = 0 が格子の点にならないようにする）
        let d = Domain { lo: [-4.95, -5.0, -5.0], hi: [5.05, 5.0, 5.0] };
        let m = implicit_surface(&prog("1/x - z", &AXES), d, 80);
        assert!(m.positions.len() / 3 > 5_000, "本当の曲面は残る");
        assert_eq!(fake_pole_vertices(&m, &[0.0], h, |x| 1.0 / x), 0);
        // 2D 表示の陰関数の曲線 tan(x) = y にも、極の縦線を描かない
        let c = implicit_curve_2d(&prog("tan(x) - y", &["x", "y"]), Domain { lo: [-5.0; 3], hi: [5.0; 3] }, 200);
        let near_pole = c
            .positions
            .chunks_exact(3)
            .filter(|q| q[0].is_finite() && poles.iter().any(|p| (q[0] as f64 - p).abs() < 0.05) && ((q[0] as f64).tan() - q[1] as f64).abs() > 2.0)
            .count();
        assert_eq!(near_pole, 0, "極の縦線");
        assert!(c.positions.len() > 300, "本当の曲線は残る");
    }

    #[test]
    fn steep_root_is_still_a_surface() {
        // 急なだけで本当に 0 になる所は、今までどおり面を張る（頂点の数が、極の判定をしない版と同じ）
        let d = Domain { lo: [-2.0; 3], hi: [2.0; 3] };
        for src in ["x^3 - z", "x^(1/3) - z", "1000 (x^2+y^2+z^2-1)", "exp(5x) - z - 1"] {
            let p = prog(src, &AXES);
            let a = implicit_surface(&p, d, 40);
            let b = implicit_solid_reference(&p, d, 40, &[], 0);
            assert!(!a.indices.is_empty(), "{src}");
            assert_eq!(a.positions.len(), b.positions.len(), "{src}");
        }
        // z (x² + y²) = 1 は、z 軸に向かって伸びる本当の曲面。頂点の数が 95% 以上残る
        let p = prog("z (x^2+y^2) - 1", &AXES);
        let d = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        let (a, b) = (implicit_surface(&p, d, 80), implicit_solid_reference(&p, d, 80, &[], 0));
        assert!(a.positions.len() as f64 >= 0.95 * b.positions.len() as f64, "{} / {}", a.positions.len(), b.positions.len());
    }

    /// 頂点の集合と三角形の集合（位置を丸めて並べ替えたもの。順番によらずに比べる）
    fn mesh_sets(m: &Mesh) -> (Vec<[i64; 3]>, Vec<[[i64; 3]; 3]>) {
        let key = |v: u32| {
            let q = &m.positions[3 * v as usize..3 * v as usize + 3];
            [0, 1, 2].map(|x| (q[x] as f64 * 1e5).round() as i64)
        };
        let mut verts: Vec<[i64; 3]> = (0..m.positions.len() as u32 / 3).map(key).collect();
        verts.sort();
        let mut tris: Vec<[[i64; 3]; 3]> = m
            .indices
            .chunks_exact(3)
            .map(|t| {
                // 向きは保ったまま、回して一番小さい頂点を先頭にする
                let k = [key(t[0]), key(t[1]), key(t[2])];
                let r = (0..3).min_by_key(|&r| k[r]).unwrap();
                [k[r], k[(r + 1) % 3], k[(r + 2) % 3]]
            })
            .collect();
        tris.sort();
        (verts, tris)
    }

    #[test]
    fn sparse_equals_dense() {
        // 区間演算で空のブロックを飛ばしても、全部のブロックを計算したときと頂点・三角形の集合が同じ
        let big = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        let cases: Vec<(&str, Vec<&str>, Domain, usize, Vec<(f64, f64)>)> = vec![
            ("x^2+y^2+z^2-4", AXES.to_vec(), big, 80, vec![]),
            ("x^2+y^2+z^2-4.01", AXES.to_vec(), big, 80, vec![]),
            ("sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)", AXES.to_vec(), big, 60, vec![]),
            ("(sqrt(x^2+y^2)-3)^2+z^2-0.04", AXES.to_vec(), big, 120, vec![]),
            ("(x^2 + y^2 + z^2 - 1) (x^2 + y^2 + z^2 - 1.3225)", AXES.to_vec(), Domain { lo: [-2.0; 3], hi: [2.0; 3] }, 40, vec![]),
            ("(x^2 + y^2 + z^2 - 0.49)^2", AXES.to_vec(), dom(), 30, vec![]),
            ("z (x^2+y^2) - 1", AXES.to_vec(), big, 80, vec![]),
            ("x^2 + y^2 + z^2 - r^2", vec!["x", "y", "z", "r"], dom(), 30, vec![(0.3, 0.7)]),
        ];
        for (src, vars, d, n, ranges) in cases {
            let p = prog(src, &vars);
            let sparse = implicit_solid_with(&p, d, n, &ranges, 8, false);
            let dense = implicit_solid_with(&p, d, n, &ranges, 8, true);
            assert!(!sparse.indices.is_empty(), "{src}");
            assert!(mesh_sets(&sparse) == mesh_sets(&dense), "{src}: 頂点・三角形の集合が違う");
        }
    }

    #[test]
    fn sparse_field_saves_memory() {
        // 球（細かさ 200）で、F の値を持つ点が、密な格子（203³ 点）の 1/5 以下
        let p = prog("x^2+y^2+z^2-4.01", &AXES);
        let d = Domain { lo: [-5.0 - 0.05; 3], hi: [5.0 + 0.05; 3] }; // implicit_solid と同じく 1 マス広げた範囲
        let n = 202;
        let field = SparseField::build(&p, &[vec![]], &[], d, n, false);
        let dense = (n + 1).pow(3);
        assert!(field.stored() * 5 <= dense, "{} / {dense}", field.stored());
        // 区間演算が使えない式は、全部を持つ
        let g = prog("abs(gamma(x + i y)) - z", &AXES);
        let field = SparseField::build(&g, &[vec![]], &[], Domain { lo: [-5.0; 3], hi: [5.0; 3] }, 40, false);
        assert!(field.stored() >= 41usize.pow(3));
    }

    #[test]
    fn sphere_tangent_to_grid_is_closed() {
        // 範囲 ±5・細かさ 80 では y = 2 などが格子の座標になり、球の6つの極が格子の平面にちょうど接する。
        // 符号の変わる曲面なので、接するだけの所として扱わず、閉じた曲面になる
        let p = prog("x^2+y^2+z^2-4", &AXES);
        let m = implicit_surface(&p, Domain { lo: [-5.0; 3], hi: [5.0; 3] }, 80);
        assert!(!m.indices.is_empty());
        assert_eq!(bad_edges(&m), (0, 0));
    }

    #[test]
    fn torus_tangent_to_grid_is_closed() {
        // z = ±0.2 の円が格子の平面に接するトーラス（表示範囲の端にはかからない）
        let p = prog("(sqrt(x^2+y^2)-3)^2+z^2-0.04", &AXES);
        let m = implicit_surface(&p, Domain { lo: [-5.0; 3], hi: [5.0; 3] }, 200);
        assert!(!m.indices.is_empty());
        assert_eq!(bad_edges(&m), (0, 0));
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

        // 格子でないメッシュ: 頂点をまとめた粗い版。球の表面から離れない
        // （陰関数はふつう格子を間引いて作り直した粗い版 lod_mesh を持つので、ここではそれを外して試す）
        let mut sphere = implicit_surface(&prog("x^2 + y^2 + z^2 - 0.64", &AXES), dom(), 160);
        assert!(sphere.lod_mesh.is_some());
        sphere.lod_mesh = None;
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

    /// 3 つ以上の三角形に使われる辺の数
    fn crowded_edges(m: &Mesh) -> usize {
        let mut count: std::collections::HashMap<(u32, u32), usize> = std::collections::HashMap::new();
        for t in m.indices.chunks_exact(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        count.values().filter(|&&c| c > 2).count()
    }

    #[test]
    fn implicit_lod_is_manifold() {
        // 陰関数の粗い版は、F の格子を間引いて作り直す。頂点をまとめる方法と違い、3 つ以上の三角形が集まる辺ができない
        let big = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        for (src, n) in [("x^2 + y^2 + z^2 - 16.3", 150), ("sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)", 80)] {
            let m = implicit_surface(&prog(src, &AXES), big, n);
            let lod = m.lod_mesh.as_ref().expect("粗い版が無い");
            let tris = lod.indices.len() / 3;
            assert!((20_000..=45_000).contains(&tris), "{src}: {tris}");
            assert_eq!(crowded_edges(lod), 0, "{src}");
            assert!(lod.indices.iter().all(|&k| (k as usize) < lod.positions.len() / 3));
            assert_eq!(lod.normals.len(), lod.positions.len());
        }
        // 球の粗い版の頂点は、球の表面の近く
        let m = implicit_surface(&prog("x^2 + y^2 + z^2 - 16.3", &AXES), big, 150);
        let lod = m.lod_mesh.unwrap();
        for q in lod.positions.chunks_exact(3) {
            let r = ((q[0] * q[0] + q[1] * q[1] + q[2] * q[2]) as f64).sqrt();
            assert!((r - 16.3f64.sqrt()).abs() < 0.1, "{r}");
        }
        // 小さいメッシュには作らない。リストでまとめると粗い版もまとまる
        assert!(implicit_surface(&prog("x^2 + y^2 + z^2 - 4", &AXES), big, 40).lod_mesh.is_none());
        let a = implicit_surface(&prog("x^2 + y^2 + z^2 - 16.3", &AXES), big, 150);
        let b = implicit_surface(&prog("x^2 + y^2 + z^2 - 4", &AXES), big, 40);
        let (la, nb) = (a.lod_mesh.as_ref().unwrap().indices.len(), b.indices.len());
        let merged = merge(vec![a, b], false);
        assert_eq!(merged.lod_mesh.as_ref().unwrap().indices.len(), la + nb);
    }

    #[test]
    fn field_split_matches_whole() {
        // F の計算をかたまりで 3 つに分けてつなげた値は、1 回で計算した値と同じ。その値を渡して作ったメッシュも同じ
        let big = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        let cases: [(&str, &[&str], Vec<(f64, f64)>); 3] = [
            ("x^2 + y^2 + z^2 - 4.01", &AXES, vec![]),                       // まばら
            ("sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)", &AXES, vec![]), // 密
            ("(x - a)^2 + y^2 + z^2 - 1", &["x", "y", "z", "a"], vec![(-2.0, 2.0)]), // 範囲で重ねた立体
        ];
        for (src, vars, ranges) in cases {
            let p = prog(src, vars);
            let run = |mode: FieldMode| {
                set_field_mode(mode);
                let m = implicit_solid(&p, big, 60, &ranges, 8);
                (m, take_field_out())
            };
            let (whole, _) = run(FieldMode::Normal);
            let (_, plan) = run(FieldMode::Plan);
            assert_eq!(plan.calls, 1);
            let c = plan.chunks;
            assert!(c >= 1, "{src}");
            let (a, b) = (c / 3, 2 * c / 3);
            let mut vals = Vec::new();
            for (x, y) in [(0, a), (a, b), (b, c)] {
                let (m, out) = run(FieldMode::Chunks(x, y));
                assert!(m.indices.is_empty());
                vals.extend(out.values);
            }
            let (_, all) = run(FieldMode::Chunks(0, c));
            assert_eq!(vals.len(), all.values.len(), "{src}");
            assert!(vals.iter().zip(&all.values).all(|(x, y)| x.to_bits() == y.to_bits()), "{src}");
            let (given, _) = run(FieldMode::Given(vals));
            assert_eq!(given.positions, whole.positions, "{src}");
            assert_eq!(given.indices, whole.indices, "{src}");
        }
    }

    #[test]
    fn flat_slice_has_no_zero_length_segments() {
        // floor の段: ちょうど 0 になる格子点で、長さの無い線（描画側では丸に見える）を作らない
        // 鞍形 (x² − y²)/4 の切り口 x = ±y も、格子点をちょうど通る（細かさによっては、f32 に丸めると同じ点になる）
        let d = Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
        for (src, n) in [("floor(x)/2 + floor(y)/4", 320), ("(x^2 - y^2)/4", 320), ("(x^2 - y^2)/4", 1200)] {
            let m = implicit_curve_2d(&prog(src, &["x", "y"]), d, n);
            assert!(!m.positions.is_empty());
            for seg in m.positions.chunks_exact(9) {
                assert!(seg[0..3] != seg[3..6], "{src}（{n}）: 長さの無い線 {:?}", &seg[0..3]);
            }
        }
    }

    #[test]
    fn slice_of_surface() {
        // 球面 (2 sin u cos v, 2 sin u sin v, 2 cos u) と平面 z = 0 の交わりは、半径 2 の円
        let sphere = progs3(["2sin(u)cos(v)", "2sin(u)sin(v)", "2cos(u)"], &["u", "v"]);
        let shape = parametric_surface_nd(&[sphere], (0.0, std::f64::consts::PI), (0.0, std::f64::consts::TAU), 60);
        let m = project(&shape, &View::identity(3), Domain { lo: [-3.0; 3], hi: [3.0; 3] });
        let line = slice_mesh(&m, 0.0);
        let pts: Vec<&[f32]> = line.positions.chunks_exact(3).filter(|p| p[0].is_finite()).collect();
        assert!(pts.len() >= 100);
        for p in pts {
            assert!(p[2] == 0.0 && ((p[0] * p[0] + p[1] * p[1]).sqrt() - 2.0).abs() < 0.01, "{p:?}");
        }
        // 2D の z = f(x, y) の切り口: 変数が2つの式でも、曲線 f = 0 を描ける（x² + y² − 1 = 0 は単位円）
        let c = implicit_curve_2d(&prog("x^2 + y^2 - 1", &["x", "y"]), Domain { lo: [-2.0; 3], hi: [2.0; 3] }, 80);
        assert!(c.positions.chunks_exact(3).filter(|p| p[0].is_finite()).all(|p| ((p[0] * p[0] + p[1] * p[1]).sqrt() - 1.0).abs() < 0.02));
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

//! 曲面のメッシュ（三角形の集まり）を作る。

use crate::equation::others;
use crate::eval::{as_real, Program};
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
    let h = dom.hi[axis] - dom.lo[axis];
    let (wlo, whi) = (dom.lo[axis] - h, dom.hi[axis] + h);

    // 使える頂点だけに番号を振る
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
        if w.is_finite() && w >= wlo && w <= whi && z.re.is_finite() && z.im.is_finite() {
            let mut p = [0.0; 3];
            p[ua] = us[k];
            p[va] = vs[k];
            p[axis] = w;
            index_of[k] = mesh.push_vertex(p);
            if mode != ComplexMode::Off {
                let r = z.norm();
                let (c, s) = if r > 0.0 { (z.re / r, z.im / r) } else { (0.0, 0.0) };
                mesh.complex.extend_from_slice(&[c as f32, s as f32, r.log2().max(-1e6) as f32]);
            }
        }
    }

    // 法線が axis の正の向きになるように三角形の向きを決める
    // （(ua, va, axis) が x→y→z の順の巡回なら、そのままで正の向き）
    let flip = (ua + 1) % 3 != va;

    // 各マスを2つの三角形に分ける。3頂点とも使えるものだけ残す。
    for j in 0..n {
        for i in 0..n {
            let a = index_of[j * m + i];
            let b = index_of[j * m + i + 1];
            let c = index_of[(j + 1) * m + i];
            let d = index_of[(j + 1) * m + i + 1];
            for [p, q, r] in [[a, b, d], [a, d, c]] {
                if p != NONE && q != NONE && r != NONE {
                    let tri = if flip { [p, r, q] } else { [p, q, r] };
                    mesh.indices.extend_from_slice(&tri);
                }
            }
        }
    }

    compute_normals(&mut mesh);
    mesh
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

    // 2. 曲面が通るセルに頂点を置く
    let cell = |i: usize, j: usize, k: usize| i + n * (j + n * k);
    let mut vert = vec![NONE; n * n * n];
    let mut mesh = Mesh::default();
    for k in 0..n {
        for j in 0..n {
            for i in 0..n {
                let mut v = [0.0f32; 8];
                let mut neg = 0;
                let mut ok = true;
                for (c, d) in CORNERS.iter().enumerate() {
                    v[c] = f[idx(i + d[0], j + d[1], k + d[2])];
                    ok &= v[c].is_finite();
                    neg += (v[c] < 0.0) as usize;
                }
                if !ok || neg == 0 || neg == 8 {
                    continue;
                }
                // 符号が変わる辺の上で F = 0 になる点（直線で補間）の平均
                let mut sum = [0.0f64; 3];
                let mut cnt = 0.0;
                for [a, b] in EDGES {
                    if (v[a] < 0.0) != (v[b] < 0.0) {
                        let t = (v[a] / (v[a] - v[b])) as f64;
                        for ax in 0..3 {
                            let (pa, pb) = (CORNERS[a][ax] as f64, CORNERS[b][ax] as f64);
                            sum[ax] += pa + (pb - pa) * t;
                        }
                        cnt += 1.0;
                    }
                }
                let g = [i as f64 + sum[0] / cnt, j as f64 + sum[1] / cnt, k as f64 + sum[2] / cnt];
                vert[cell(i, j, k)] = mesh.push_vertex([dom.at(0, g[0], n), dom.at(1, g[1], n), dom.at(2, g[2], n)]);
            }
        }
    }

    // 3. 符号が変わる格子の辺ごとに、周りの4セルの頂点を四角形でつなぐ
    for a in 0..3 {
        let (b, c) = ((a + 1) % 3, (a + 2) % 3);
        for k in 0..m {
            for j in 0..m {
                for i in 0..m {
                    let p = [i, j, k];
                    // 辺 p → p + e_a。周りの4セルが全部格子内にある辺だけを見る
                    if p[a] >= n || p[b] == 0 || p[b] >= n || p[c] == 0 || p[c] >= n {
                        continue;
                    }
                    let mut q = p;
                    q[a] += 1;
                    let (f0, f1) = (f[idx(p[0], p[1], p[2])], f[idx(q[0], q[1], q[2])]);
                    if !f0.is_finite() || !f1.is_finite() || (f0 < 0.0) == (f1 < 0.0) {
                        continue;
                    }
                    // 周りの4セル（b-c 平面で反時計回り）
                    let around = [(1, 1), (0, 1), (0, 0), (1, 0)].map(|(db, dc)| {
                        let mut s = p;
                        s[b] -= db;
                        s[c] -= dc;
                        vert[cell(s[0], s[1], s[2])]
                    });
                    if around.contains(&NONE) {
                        continue;
                    }
                    let [q0, q1, q2, q3] = around;
                    // 法線が F の増える向きを向くように三角形の向きを決める
                    if f0 < 0.0 {
                        mesh.indices.extend_from_slice(&[q0, q1, q2, q0, q2, q3]);
                    } else {
                        mesh.indices.extend_from_slice(&[q0, q2, q1, q0, q3, q2]);
                    }
                }
            }
        }
    }

    compute_normals(&mut mesh);
    mesh
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
}

/// 格子線を引かない成分の印（シェーダーはこれより小さい値の成分には線を引かない）
pub const NO_GRID: f32 = -1e30;

/// 各成分をまとめて計算し、n 次元の点を並べたもの（点ごとに d 個）にする。
/// どれかの成分が実数にならない点は NaN
fn eval_nd(progs: &[Program], inputs: &[&[f64]], n: usize) -> Vec<f64> {
    let d = progs.len();
    let mut pts = vec![0.0; n * d];
    let mut out = vec![C::new(0.0, 0.0); n];
    for (ax, prog) in progs.iter().enumerate() {
        prog.eval_batch(inputs, &mut out);
        for (k, v) in out.iter().enumerate() {
            pts[k * d + ax] = as_real(*v);
        }
    }
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
        points.extend(eval_nd(progs, &[&us, &vs], m * m));
        grid.extend_from_slice(&grid1);
    }
    NdShape { dims, points, grid: Some((m, grid)), solid: false }
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
    // 6つの面の媒介変数の値と、格子線用の値
    let mut cols: [Vec<f64>; 3] = Default::default();
    let mut grid1 = Vec::with_capacity(6 * m * m * 3);
    for k in 0..3 {
        for side in [0, 1] {
            let (a, b) = if side == 1 { ((k + 1) % 3, (k + 2) % 3) } else { ((k + 2) % 3, (k + 1) % 3) };
            let fixed = if side == 1 { r[k].1 } else { r[k].0 };
            for j in 0..m {
                for i in 0..m {
                    let mut p = [0.0; 3];
                    p[k] = fixed;
                    p[a] = at(a, i);
                    p[b] = at(b, j);
                    for c in 0..3 {
                        cols[c].push(p[c]);
                        grid1.push(if c == k { NO_GRID } else { (p[c] - r[c].0) as f32 });
                    }
                }
            }
        }
    }
    let dims = sets.first().map_or(0, |p| p.len());
    let count = 6 * m * m;
    let mut points = Vec::with_capacity(sets.len() * count * dims);
    let mut grid = Vec::with_capacity(sets.len() * grid1.len());
    for progs in sets {
        points.extend(eval_nd(progs, &[&cols[0], &cols[1], &cols[2]], count));
        grid.extend_from_slice(&grid1);
    }
    NdShape { dims, points, grid: Some((m, grid)), solid: true }
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
    for k in 0..m * m {
        let mut p = base.to_vec();
        p[axes[ua]] = us[k];
        p[axes[va]] = vs[k];
        p[axes[axis]] = as_real(vals[k]);
        points.extend_from_slice(&p);
    }
    NdShape { dims: d, points, grid: Some((m, grid)), solid: false }
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
        points.extend(curve_nd(progs, t, n, tol));
    }
    NdShape { dims, points, grid: None, solid: false }
}

/// 1本の曲線の点（適応サンプリング）
fn curve_nd(progs: &[Program], t: (f64, f64), n: usize, tol: f64) -> Vec<f64> {
    const MAX_POINTS: usize = 200_000;
    const PASSES: usize = 14;
    let d = progs.len();
    let split = |flat: Vec<f64>| -> Vec<Vec<f64>> { flat.chunks_exact(d).map(|c| c.to_vec()).collect() };

    let n = n.max(2);
    let mut ts: Vec<f64> = (0..=n).map(|k| t.0 + (t.1 - t.0) * k as f64 / n as f64).collect();
    let mut ps = split(eval_nd(progs, &[&ts], ts.len()));
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
        let mps = split(eval_nd(progs, &[&mids], mids.len()));

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
            let g = &grid[gd * k..gd * k + gd];
            mesh.grid_coords.extend_from_slice(&[g[0], g[1], if shape.solid { g[2] } else { p[2] as f32 }]);
        }
    }
    // 曲面ごとに m × m 個の点が並んでいる。立体は6枚ずつ
    let per = if shape.solid { 6 * m * m } else { m * m };
    for start in (0..count).step_by(per) {
        let first = mesh.indices.len();
        for base in (start..start + per).step_by(m * m) {
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
    fn combos() {
        assert_eq!(sample_combos(&[], 10), vec![Vec::<f64>::new()]);
        assert_eq!(sample_combos(&[(0.0, 1.0)], 3), vec![vec![0.0], vec![0.5], vec![1.0]]);
        assert_eq!(sample_combos(&[(0.0, 1.0), (0.0, 1.0)], 100).len(), 64 * 64);
    }

    /// 速さの目安を測る: `cargo test --release -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn timing() {
        let d = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        for n in [80, 120, 200] {
            let t = std::time::Instant::now();
            let m = implicit_surface(&prog("sin(x) + sin(y) + sin(z)", &AXES), d, n);
            println!("陰関数 n = {n}: {:?}, 三角形 {}", t.elapsed(), m.indices.len() / 3);
        }
    }
}

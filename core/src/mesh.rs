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
        let w = as_real(vals[k]);
        if w.is_finite() && w >= wlo && w <= whi {
            let mut p = [0.0; 3];
            p[ua] = us[k];
            p[va] = vs[k];
            p[axis] = w;
            index_of[k] = mesh.push_vertex(p);
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
        let mut out = vec![C::new(0.0, 0.0); m * m];
        for k in 0..m {
            zs.fill(dom.at(2, k as f64, n));
            prog.eval_batch(&[&xs, &ys, &zs], &mut out);
            for (dst, v) in f[k * m * m..(k + 1) * m * m].iter_mut().zip(&out) {
                *dst = as_real(*v) as f32;
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

// ---------------------------------------------------------------------------
// 媒介変数表示
// ---------------------------------------------------------------------------

/// 3つの成分をまとめて計算し、点の列にする。どれかの成分が実数にならない点は NaN。
fn eval_points(progs: &[Program; 3], inputs: &[&[f64]], n: usize) -> Vec<[f64; 3]> {
    let mut pts = vec![[0.0; 3]; n];
    let mut out = vec![C::new(0.0, 0.0); n];
    for (ax, prog) in progs.iter().enumerate() {
        prog.eval_batch(inputs, &mut out);
        for (p, v) in pts.iter_mut().zip(&out) {
            p[ax] = as_real(*v);
        }
    }
    pts
}

fn finite(p: &[f64; 3]) -> bool {
    p.iter().all(|v| v.is_finite())
}

fn dist(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// 媒介変数表示の曲面 (x(u,v), y(u,v), z(u,v)) を `n × n` マスの格子で作る。
pub fn parametric_surface(progs: &[Program; 3], dom: Domain, u: (f64, f64), v: (f64, f64), n: usize) -> Mesh {
    let n = n.max(2);
    let m = n + 1;
    let mut us = Vec::with_capacity(m * m);
    let mut vs = Vec::with_capacity(m * m);
    for j in 0..m {
        for i in 0..m {
            us.push(u.0 + (u.1 - u.0) * i as f64 / n as f64);
            vs.push(v.0 + (v.1 - v.0) * j as f64 / n as f64);
        }
    }
    let pts = eval_points(progs, &[&us, &vs], m * m);

    // 表示範囲から大きく外れた点（無限大の近くなど）は省く。はみ出た部分は描画側で切り落とす。
    let ok = |p: &[f64; 3]| {
        finite(p)
            && (0..3).all(|a| {
                let h = dom.hi[a] - dom.lo[a];
                p[a] >= dom.lo[a] - h && p[a] <= dom.hi[a] + h
            })
    };

    let mut index_of = vec![NONE; m * m];
    let mut mesh = Mesh::default();
    for k in 0..m * m {
        if ok(&pts[k]) {
            index_of[k] = mesh.push_vertex(pts[k]);
            mesh.grid_coords
                .extend_from_slice(&[(us[k] - u.0) as f32, (vs[k] - v.0) as f32, pts[k][2] as f32]);
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
    compute_normals(&mut mesh);
    mesh
}

/// 媒介変数表示の曲線 (x(t), y(t), z(t))。
///
/// 最初に `n` 等分して計算し、曲がっている所・途切れる所だけを細かく分け直す（適応サンプリング）。
/// 戻り値は点の座標を並べたもの。曲線が途切れる所には NaN の点が入る。
/// `tol` は「直線で結んだときのずれ」の許容量。
pub fn parametric_curve(progs: &[Program; 3], t: (f64, f64), n: usize, tol: f64) -> Vec<f32> {
    const MAX_POINTS: usize = 200_000;
    const PASSES: usize = 14;

    let n = n.max(2);
    let mut ts: Vec<f64> = (0..=n).map(|k| t.0 + (t.1 - t.0) * k as f64 / n as f64).collect();
    let mut ps = eval_points(progs, &[&ts], ts.len());
    // 区間 [ts[k], ts[k+1]] をまだ分け直す必要があるか
    let mut todo = vec![true; ts.len() - 1];

    for _ in 0..PASSES {
        // 分け直す候補の区間の中点をまとめて計算する
        let cand: Vec<usize> = (0..todo.len())
            .filter(|&k| todo[k] && (finite(&ps[k]) || finite(&ps[k + 1])))
            .collect();
        if cand.is_empty() || ts.len() + cand.len() > MAX_POINTS {
            break;
        }
        let mids: Vec<f64> = cand.iter().map(|&k| 0.5 * (ts[k] + ts[k + 1])).collect();
        let mps = eval_points(progs, &[&mids], mids.len());

        // 中点を入れるべきか判定しながら、新しい列を作る
        let mut nts = Vec::with_capacity(ts.len() + cand.len());
        let mut nps = Vec::with_capacity(ts.len() + cand.len());
        let mut ntodo = Vec::with_capacity(ts.len() + cand.len());
        let mut c = 0;
        for k in 0..ts.len() {
            nts.push(ts[k]);
            nps.push(ps[k]);
            if k + 1 == ts.len() {
                break;
            }
            if c < cand.len() && cand[c] == k {
                let (a, b, m) = (&ps[k], &ps[k + 1], &mps[c]);
                let insert = if finite(a) && finite(b) && finite(m) {
                    // 中点が、両端を結ぶ線分の中点からどれだけずれているか
                    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0, (a[2] + b[2]) / 2.0];
                    dist(m, &mid) > tol
                } else {
                    true // 定義されない点との境目を探す
                };
                if insert {
                    nts.push(mids[c]);
                    nps.push(*m);
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
    let mut out = Vec::with_capacity(ps.len() * 3);
    for k in 0..ps.len() {
        if k > 0 && todo[k - 1] && finite(&ps[k - 1]) && finite(&ps[k]) && dist(&ps[k - 1], &ps[k]) > jump {
            out.extend_from_slice(&[f32::NAN; 3]);
        }
        out.extend(ps[k].iter().map(|&v| v as f32));
    }
    out
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
    use crate::equation::AXES;
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

    fn progs3(src: [&str; 3], vars: &[&str]) -> [Program; 3] {
        src.map(|s| prog(s, vars))
    }

    #[test]
    fn param_surface_cylinder() {
        let d = Domain { lo: [-2.0; 3], hi: [2.0; 3] };
        let p = progs3(["cos(u)", "sin(u)", "v"], &["u", "v"]);
        let m = parametric_surface(&p, d, (0.0, std::f64::consts::TAU), (-1.0, 1.0), 20);
        assert_eq!(m.positions.len(), 21 * 21 * 3);
        assert_eq!(m.grid_coords.len(), m.positions.len());
        // 円柱の法線は横向き（z 成分がほぼ 0）
        assert!(m.normals.chunks_exact(3).all(|n| n[2].abs() < 1e-3));
    }

    #[test]
    fn curve_adaptive_and_breaks() {
        // 円: 点はすべて半径 1 の上
        let p = progs3(["cos(t)", "sin(t)", "0"], &["t"]);
        let pts = parametric_curve(&p, (0.0, std::f64::consts::TAU), 8, 1e-3);
        assert!(pts.len() / 3 > 9, "曲がっている所は細かく分け直される");
        for q in pts.chunks_exact(3) {
            assert!(((q[0] * q[0] + q[1] * q[1]).sqrt() - 1.0).abs() < 1e-5);
        }
        // tan t は極で線が切れる（NaN が入る）
        let p = progs3(["t", "tan(t)", "0"], &["t"]);
        let pts = parametric_curve(&p, (-3.0, 3.0), 50, 1e-3);
        assert!(pts.iter().any(|v| v.is_nan()));
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

//! 形の正しさを測る物差し（計画 S17）
//!
//! ```text
//! cargo test --release --test shape -- --ignored --nocapture   # 全部の式の報告（数分かかる）
//! SHAPE_ONLY=floor cargo test --release --test shape -- --ignored --nocapture   # 「floor」を含む式だけ
//! ```
//!
//! 表示範囲を 128³ 個の小さい箱（ボクセル）に分け、「正解の面が通る箱」と「メッシュが通る箱」を比べる。
//!
//! - **欠け**: 正解の箱のうち、1 箱以内（周りの 26 箱を含む）にメッシュの箱が無いものの割合（%）
//! - **余計**: メッシュの箱のうち、1 箱以内に正解の箱が無いものの割合（%）
//! - 「3 箱以内」で数えたものも出す（大きく外れているものだけが残る）。1 箱は表示範囲の約 0.8%
//! - **破片**: 頂点でつながった成分のうち、三角形が 8 個以下のものの数（紙吹雪のような破片）
//!
//! 正解は 2 通り。
//!
//! - 陽関数の正解（[`reference_explicit`]）: 元の座標で細かく点を取り、隣どうしの間を二分して埋める（速い）
//! - 陰関数の正解（[`reference_implicit`]）: 3 つの軸に平行な線の上で F の符号が変わる所を二分する（遅いが、どの式にも使える）

use graph_core::{equation, eval, plot, MeshResult};
use num_complex::Complex64 as C;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// 表示範囲（x, y, z の下端・上端）。z の端を 6 でなく 6.4 にするのは、`floor` などの整数の値の面が、
/// 表示範囲の端の面にちょうど乗らないようにするため
const DOMAIN: [f64; 6] = [-10.0, 10.0, -10.0, 10.0, -6.4, 6.4];
/// 1 軸あたりの箱の数
const N: i32 = 128;
/// 陽関数・媒介変数表示の細かさ（既定）
const RES: u32 = 150;
/// 陰関数の細かさ（既定）
const IRES: u32 = 80;

type Voxels = HashSet<(i32, i32, i32)>;

/// a 番目の軸の箱の大きさ
fn vs(a: usize) -> f64 {
    (DOMAIN[2 * a + 1] - DOMAIN[2 * a]) / N as f64
}

/// 点を「箱の単位」の座標にする（箱 i は [i, i + 1)）
fn scaled(q: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|a| (q[a] - DOMAIN[2 * a]) / vs(a))
}

/// 点が入る箱（表示範囲の外なら None）
fn voxel_of(q: [f64; 3]) -> Option<(i32, i32, i32)> {
    let s = scaled(q);
    if s.iter().any(|v| !v.is_finite() || *v < 0.0 || *v >= N as f64) {
        return None;
    }
    Some((s[0] as i32, s[1] as i32, s[2] as i32))
}

// ---- 回転 ----

/// 測る回転の種類
#[derive(Clone, Copy, PartialEq, Debug)]
enum Rot {
    None,
    /// 表示中の座標どうし
    Xy,
    Xz,
    /// 表示していない w が絡む
    Xw,
    Zw,
    /// 5 つの組を混ぜたもの
    Mixed,
}

const ROTS: [Rot; 6] = [Rot::None, Rot::Xy, Rot::Xz, Rot::Xw, Rot::Zw, Rot::Mixed];

impl Rot {
    fn label(self) -> &'static str {
        match self {
            Rot::None => "回転なし",
            Rot::Xy => "xy 0.5",
            Rot::Xz => "xz 0.4",
            Rot::Xw => "xw 0.6",
            Rot::Zw => "zw 0.6",
            Rot::Mixed => "混ぜた回転",
        }
    }

    /// `plot` に渡す回転角（5 次元の 10 個の組。(0,1), (0,2), (0,3), (0,4), (1,2), (1,3), (1,4), (2,3), (2,4), (3,4) の順）
    fn angles(self) -> Vec<f64> {
        let mut a = vec![0.0; 10];
        match self {
            Rot::None => {}
            Rot::Xy => a[0] = 0.5,
            Rot::Xz => a[1] = 0.4,
            Rot::Xw => a[2] = 0.6,
            Rot::Zw => a[7] = 0.6,
            Rot::Mixed => {
                a[0] = 0.3; // xy
                a[1] = 0.2; // xz
                a[2] = 0.4; // xw
                a[5] = 0.35; // yw
                a[7] = 0.5; // zw
            }
        }
        a
    }

    /// 回転を設定した 3 次元の空間（式をコンパイルするため）
    fn space(self) -> equation::Space {
        let mut s = equation::Space::basic(3, [0, 1, 2], vec![0.0; 3]);
        s.rotation = self.angles();
        s
    }
}

// ---- メッシュ ----

/// 画面と同じ入口でメッシュを作る（壁は描かない）。かかった時間（ms）も返す
fn mesh(src: &str, rot: Rot, ires: u32) -> (MeshResult, f64) {
    let rotation = rot.angles();
    let t = Instant::now();
    let r = plot(
        src,
        &DOMAIN,
        RES,
        ires,
        "",
        &[],
        "",
        &[],
        "",
        3,
        &[0, 1, 2],
        &[0.0; 3],
        &[f64::NAN; 6],
        "",
        &[],
        16,
        &rotation,
        &[0.0; 5],
        0.0,
        0,
        false,
        false,
        1,
        false,
        0,
    )
    .unwrap_or_else(|_| panic!("{src} が計算できない"));
    (r, t.elapsed().as_secs_f64() * 1e3)
}

/// メッシュが通る箱。三角形の上に、箱の半分以下の間隔で点を打ち、表示範囲の中の点が入る箱を集める。
/// NaN の頂点を持つ三角形は飛ばす。1 つの三角形の点は 1 辺 600 分割まで
/// （大きい三角形は 4 つに分けて、表示範囲の外にはみ出した部分を飛ばしながら点を打つ）
fn voxels_of_mesh(positions: &[f32], indices: &[u32]) -> Voxels {
    let mut out = Voxels::new();
    let p = |i: u32| {
        let k = i as usize * 3;
        [positions[k] as f64, positions[k + 1] as f64, positions[k + 2] as f64]
    };
    for t in indices.chunks_exact(3) {
        let v = [p(t[0]), p(t[1]), p(t[2])];
        if v.iter().flatten().any(|c| !c.is_finite()) {
            continue;
        }
        let s = v.map(scaled);
        let len = |i: usize, j: usize| (0..3).map(|a| (s[i][a] - s[j][a]).powi(2)).sum::<f64>().sqrt();
        let longest = len(0, 1).max(len(1, 2)).max(len(2, 0));
        let n = ((longest / 0.5).ceil() as usize).clamp(1, 600);
        triangle_voxels(s, n, &mut out);
    }
    out
}

/// 三角形 `s`（箱の単位の座標）を 1 辺 `n` 分割した点が入る箱を `out` に足す
fn triangle_voxels(s: [[f64; 3]; 3], n: usize, out: &mut Voxels) {
    // 3 つの頂点が全部、表示範囲の同じ側の外にあれば飛ばす
    if (0..3).any(|a| s.iter().all(|q| q[a] < 0.0) || s.iter().all(|q| q[a] >= N as f64)) {
        return;
    }
    if n > 16 {
        let m = |i: usize, j: usize| [0, 1, 2].map(|a| (s[i][a] + s[j][a]) / 2.0);
        let (m01, m12, m20) = (m(0, 1), m(1, 2), m(2, 0));
        let h = n.div_ceil(2);
        for t in [[s[0], m01, m20], [m01, s[1], m12], [m20, m12, s[2]], [m01, m12, m20]] {
            triangle_voxels(t, h, out);
        }
        return;
    }
    // 一番長い辺 s[a]–s[b] に平行な列に分けて、点を打つ（細長い三角形でも、点の数が面積に比例するように）
    let len = |p: [f64; 3], q: [f64; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f64>().sqrt();
    let (a, b, c) = [(0, 1, 2), (1, 2, 0), (2, 0, 1)]
        .into_iter()
        .max_by(|x, y| len(s[x.0], s[x.1]).total_cmp(&len(s[y.0], s[y.1])))
        .unwrap();
    let base = len(s[a], s[b]);
    let cross = {
        let (e, f) = ([0, 1, 2].map(|k| s[b][k] - s[a][k]), [0, 1, 2].map(|k| s[c][k] - s[a][k]));
        let x = [e[1] * f[2] - e[2] * f[1], e[2] * f[0] - e[0] * f[2], e[0] * f[1] - e[1] * f[0]];
        (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt()
    };
    let height = if base > 0.0 { cross / base } else { 0.0 };
    let rows = ((height / 0.5).ceil() as usize).max(1);
    for i in 0..=rows {
        let t = i as f64 / rows as f64;
        // 列の両端: s[a] → s[c] と s[b] → s[c] を t で分けた点
        let p = [0, 1, 2].map(|k| s[a][k] + t * (s[c][k] - s[a][k]));
        let q = [0, 1, 2].map(|k| s[b][k] + t * (s[c][k] - s[b][k]));
        let m = (((1.0 - t) * base / 0.5).ceil() as usize).max(1);
        for j in 0..=m {
            let r = j as f64 / m as f64;
            let x = [0, 1, 2].map(|k| p[k] + r * (q[k] - p[k]));
            if x.iter().all(|v| (0.0..N as f64).contains(v)) {
                out.insert((x[0] as i32, x[1] as i32, x[2] as i32));
            }
        }
    }
}

/// `a` の箱のうち、`tol` 箱以内（周りの (2 tol + 1)³ 個の箱）に `b` の箱が無いものの割合（%）。`a` が空なら 0
fn miss(a: &Voxels, b: &Voxels, tol: i32) -> f64 {
    if a.is_empty() {
        return 0.0;
    }
    let idx = |x: i32, y: i32, z: i32| ((x * N + y) * N + z) as usize;
    let mut dense = vec![false; (N * N * N) as usize];
    for &(x, y, z) in b {
        dense[idx(x, y, z)] = true;
    }
    let near = |&(x, y, z): &(i32, i32, i32)| {
        for dx in -tol..=tol {
            for dy in -tol..=tol {
                for dz in -tol..=tol {
                    let (i, j, k) = (x + dx, y + dy, z + dz);
                    if (0..N).contains(&i) && (0..N).contains(&j) && (0..N).contains(&k) && dense[idx(i, j, k)] {
                        return true;
                    }
                }
            }
        }
        false
    };
    let missing = a.iter().filter(|v| !near(v)).count();
    100.0 * missing as f64 / a.len() as f64
}

/// 頂点でつながった成分のうち、三角形が 8 個以下のものの数（紙吹雪の破片の数）。
/// 同じ位置の頂点は同じ頂点として数える（切り分けで頂点を複製するメッシュがあるため）
fn fragments(positions: &[f32], indices: &[u32]) -> usize {
    let mut weld: HashMap<[u32; 3], usize> = HashMap::new();
    let id: Vec<usize> = positions
        .chunks_exact(3)
        .map(|q| {
            let key = [q[0].to_bits(), q[1].to_bits(), q[2].to_bits()];
            let n = weld.len();
            *weld.entry(key).or_insert(n)
        })
        .collect();
    let mut parent: Vec<usize> = (0..weld.len()).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for t in indices.chunks_exact(3) {
        let a = find(&mut parent, id[t[0] as usize]);
        for &k in &t[1..] {
            let b = find(&mut parent, id[k as usize]);
            if a != b {
                parent[b] = a;
            }
        }
    }
    let mut count: HashMap<usize, usize> = HashMap::new();
    for t in indices.chunks_exact(3) {
        *count.entry(find(&mut parent, id[t[0] as usize])).or_insert(0) += 1;
    }
    count.values().filter(|&&c| c <= 8).count()
}

/// 極の壁のつながり: |x − 極| < 0.07 の頂点を、y の 0.25 ごとの 80 区間に分け、
/// z ≥ 6 の頂点と z ≤ −6 の頂点の両方がある区間の数（回転なしのメッシュで使う）
fn wall_coverage(positions: &[f32], pole: f64) -> usize {
    let mut top = [false; 80];
    let mut bottom = [false; 80];
    for q in positions.chunks_exact(3) {
        let (x, y, z) = (q[0] as f64, q[1] as f64, q[2] as f64);
        if (x - pole).abs() >= 0.07 || !(-10.0..10.0).contains(&y) {
            continue;
        }
        let k = ((y + 10.0) / 0.25) as usize;
        if k < 80 {
            top[k] |= z >= 6.0;
            bottom[k] |= z <= -6.0;
        }
    }
    (0..80).filter(|&k| top[k] && bottom[k]).count()
}

// ---- 正解 ----

/// 陽関数 `z = f(x, y)` の f（変数は x, y）
fn explicit_prog(f: &str) -> eval::Program {
    match equation::compile(&format!("z = {f}")) {
        Ok(equation::Plot::Explicit { axis: 2, prog, .. }) => prog,
        _ => panic!("z = {f} が陽関数にならない"),
    }
}

/// 陽関数の正解。元の座標で (u, v) を「1 箱あたり 4 × 4 点」の細かさで取り、f を計算して点 (u, v, f) を作る。
///
/// - 隣どうしの点が半箱より離れていたら、間を二分して埋める。8 回二分しても半箱より離れたままなら、
///   飛び（面は無い）として埋めない（表示範囲の 1/100,000 より薄い面は正解に入れない）
/// - 片方が定義されない（NaN）隣どうしは、定義される側へ 8 回二分して、縁までの点を足す
/// - 回転があるときは、点を表示中の座標へ移す（f が表示していない座標を使わず、回転の中心・断面の値が 0 のとき）。
///   M_kj = R_jk（k は元の x, y, z、j は表示中の 3 つの座標）として q = M⁻¹ (u, v, f)。
///   (u, v) を取る範囲は、表示範囲の 8 つの角を M で移したものを囲む長方形。|det M| < 0.05 なら None
fn reference_explicit(f: &str, rot: Rot) -> Option<Voxels> {
    let prog = explicit_prog(f);
    let m: [[f64; 3]; 3] = match rot.space().rotation_matrix() {
        Some(r) => [0, 1, 2].map(|k| [0, 1, 2].map(|j| r[j][k])),
        None => [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    };
    let minv = inverse(m)?;
    // (u, v) の範囲
    let (mut ulo, mut uhi, mut vlo, mut vhi) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for c in 0..8 {
        let q = [0, 1, 2].map(|a| DOMAIN[2 * a + ((c >> a) & 1)]);
        let p = mul(m, q);
        ulo = ulo.min(p[0]);
        uhi = uhi.max(p[0]);
        vlo = vlo.min(p[1]);
        vhi = vhi.max(p[1]);
    }
    let (du, dv) = (vs(0) / 4.0, vs(1) / 4.0);
    let nu = ((uhi - ulo) / du).ceil() as usize + 1;
    let nv = ((vhi - vlo) / dv).ceil() as usize + 1;
    // 格子点にちょうど乗らないよう、少しずらす
    let us: Vec<f64> = (0..nu).map(|i| ulo + (i as f64 + 0.013) * du).collect();
    let vs_: Vec<f64> = (0..nv).map(|j| vlo + (j as f64 + 0.007) * dv).collect();
    let mut h = vec![0.0; nu * nv];
    let mut out = vec![C::new(0.0, 0.0); nu];
    for (j, &v) in vs_.iter().enumerate() {
        let vv = vec![v; nu];
        prog.eval_batch(&[&us, &vv], &mut out);
        for i in 0..nu {
            h[j * nu + i] = eval::as_real(out[i]);
        }
    }
    let mut r = Explicit { prog: &prog, minv, out: Voxels::new() };
    for j in 0..nv {
        for i in 0..nu {
            let a = [us[i], vs_[j], h[j * nu + i]];
            r.add(a);
            if i + 1 < nu {
                r.fill(a, [us[i + 1], vs_[j], h[j * nu + i + 1]], 0);
            }
            if j + 1 < nv {
                r.fill(a, [us[i], vs_[j + 1], h[(j + 1) * nu + i]], 0);
            }
        }
    }
    Some(r.out)
}

/// 陽関数の正解を作る途中の状態
struct Explicit<'a> {
    prog: &'a eval::Program,
    minv: [[f64; 3]; 3],
    out: Voxels,
}

impl Explicit<'_> {
    /// (u, v) での f
    fn f(&self, u: f64, v: f64) -> f64 {
        let mut o = [C::new(0.0, 0.0)];
        self.prog.eval_batch(&[&[u], &[v]], &mut o);
        eval::as_real(o[0])
    }

    /// 点 (u, v, f) を表示中の座標に移したもの
    fn q(&self, p: [f64; 3]) -> [f64; 3] {
        mul(self.minv, p)
    }

    fn add(&mut self, p: [f64; 3]) {
        if p[2].is_finite() {
            if let Some(k) = voxel_of(self.q(p)) {
                self.out.insert(k);
            }
        }
    }

    fn mid(&self, a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
        let (u, v) = ((a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0);
        [u, v, self.f(u, v)]
    }

    /// 隣どうしの点 a, b の間を埋める（`depth` はこれまでに二分した回数）
    fn fill(&mut self, a: [f64; 3], b: [f64; 3], depth: u32) {
        match (a[2].is_nan(), b[2].is_nan()) {
            (true, true) => {}
            (false, true) | (true, false) => {
                // 定義される側へ 8 回二分して、縁の近くの定義される点を探す
                let (good, mut bad) = if a[2].is_nan() { (b, a) } else { (a, b) };
                let mut g = good;
                for _ in 0..8 {
                    let m = self.mid(g, bad);
                    if m[2].is_nan() {
                        bad = m;
                    } else {
                        g = m;
                    }
                }
                if g != good {
                    self.add(g);
                    self.fill(good, g, depth);
                }
            }
            (false, false) => {
                let (sa, sb) = (scaled(self.q(a)), scaled(self.q(b)));
                let dist = (0..3).map(|k| (sa[k] - sb[k]).powi(2)).sum::<f64>().sqrt();
                if dist <= 0.5 {
                    return;
                }
                // 両方が表示範囲の同じ側の外なら、見えない
                if (0..3).any(|k| (sa[k] < 0.0 && sb[k] < 0.0) || (sa[k] >= N as f64 && sb[k] >= N as f64)) {
                    return;
                }
                if depth >= 8 {
                    return;
                }
                let m = self.mid(a, b);
                self.add(m);
                self.fill(a, m, depth + 1);
                self.fill(m, b, depth + 1);
            }
        }
    }
}

fn mul(m: [[f64; 3]; 3], p: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|k| (0..3).map(|j| m[k][j] * p[j]).sum())
}

/// 3 × 3 の逆行列（|det| < 0.05 なら None）
fn inverse(m: [[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 0.05 {
        return None;
    }
    let c = |i: usize, j: usize| {
        let (r0, r1) = ((i + 1) % 3, (i + 2) % 3);
        let (c0, c1) = ((j + 1) % 3, (j + 2) % 3);
        m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0]
    };
    // 逆行列は余因子行列の転置 / det
    Some([0, 1, 2].map(|i| [0, 1, 2].map(|j| c(j, i) / det)))
}

/// 陰関数の正解。式を `(左辺) - (右辺) = 0` の形にして F を取り出し（陽関数 `z = f` なら `(f) - z = 0`。回転も代入される）、
/// 3 つの軸それぞれについて、箱の中心を通る 128² 本の線の上で、1 箱あたり 12 点の間隔で F を計算し、
/// 符号が変わる所を 24 回二分する。|F| が最初（両端の大きいほう）の 10⁻³ 倍より小さくなったものだけを零点とする（極・飛びを除く）
fn reference_implicit(src: &str, rot: Rot) -> Voxels {
    let (l, r) = src.split_once('=').unwrap_or((src, "0"));
    let f = format!("({l}) - ({r}) = 0");
    let prog = match equation::compile_with(&f, &eval::Params::new(), &rot.space()) {
        Ok(equation::Plot::Implicit { prog, .. }) => prog,
        _ => panic!("{f} が陰関数にならない"),
    };
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let parts: Vec<Voxels> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let prog = prog.clone();
                s.spawn(move || {
                    let mut out = Voxels::new();
                    for i in (t as i32..N).step_by(threads) {
                        for a in 0..3 {
                            for j in 0..N {
                                roots_on_line(&prog, a, i, j, &mut out);
                            }
                        }
                    }
                    out
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    parts.into_iter().flatten().collect()
}

/// 軸 a に平行で、ほかの 2 軸の箱 (i, j) の中心（を少しずらした所）を通る線の上の零点を、`out` に足す
fn roots_on_line(prog: &eval::Program, a: usize, i: i32, j: i32, out: &mut Voxels) {
    const PER: usize = 12;
    let (b, c) = ((a + 1) % 3, (a + 2) % 3);
    let qb = DOMAIN[2 * b] + (i as f64 + 0.5 + 0.0131) * vs(b);
    let qc = DOMAIN[2 * c] + (j as f64 + 0.5 + 0.0077) * vs(c);
    let step = vs(a) / PER as f64;
    let n = N as usize * PER;
    let ts: Vec<f64> = (0..n).map(|k| DOMAIN[2 * a] + (k as f64 + 0.37) * step).collect();
    let eval_at = |ts: &[f64]| -> Vec<f64> {
        let m = ts.len();
        let mut cols = [vec![0.0; m], vec![0.0; m], vec![0.0; m]];
        cols[a].copy_from_slice(ts);
        cols[b].fill(qb);
        cols[c].fill(qc);
        let mut o = vec![C::new(0.0, 0.0); m];
        prog.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut o);
        o.into_iter().map(eval::as_real).collect()
    };
    let fs = eval_at(&ts);
    let mut add = |t: f64| {
        let mut q = [0.0; 3];
        q[a] = t;
        q[b] = qb;
        q[c] = qc;
        if let Some(k) = voxel_of(q) {
            out.insert(k);
        }
    };
    // 符号が変わる区間 (左端, 右端, 左端の F, 最初の |F|)
    let mut ivs: Vec<(f64, f64, f64, f64)> = Vec::new();
    for k in 0..n {
        if fs[k] == 0.0 {
            add(ts[k]);
        }
        if k + 1 < n && fs[k].is_finite() && fs[k + 1].is_finite() && fs[k] * fs[k + 1] < 0.0 {
            ivs.push((ts[k], ts[k + 1], fs[k], fs[k].abs().max(fs[k + 1].abs())));
        }
    }
    if ivs.is_empty() {
        return;
    }
    for _ in 0..24 {
        let mids: Vec<f64> = ivs.iter().map(|v| (v.0 + v.1) / 2.0).collect();
        let fm = eval_at(&mids);
        for (v, (&m, &f)) in ivs.iter_mut().zip(mids.iter().zip(&fm)) {
            if !f.is_finite() {
                // 途中で定義されなくなった区間は捨てる（最後の確かめで外れるようにする）
                v.3 = 0.0;
            } else if f * v.2 > 0.0 {
                v.0 = m;
                v.2 = f;
            } else {
                v.1 = m;
            }
        }
    }
    let mids: Vec<f64> = ivs.iter().map(|v| (v.0 + v.1) / 2.0).collect();
    let fm = eval_at(&mids);
    for (v, (&m, &f)) in ivs.iter().zip(mids.iter().zip(&fm)) {
        if f.abs() < 1e-3 * v.3 {
            add(m);
        }
    }
}

// ---- 報告 ----

/// 1 つの式・1 つの回転の結果
struct Row {
    miss1: f64,
    miss3: f64,
    extra1: f64,
    extra3: f64,
    frags: usize,
    tris: usize,
    ms: f64,
}

fn measure(reference: &Voxels, r: &MeshResult, ms: f64) -> Row {
    let (pos, idx) = (r.positions(), r.indices());
    let got = voxels_of_mesh(&pos, &idx);
    Row {
        miss1: miss(reference, &got, 1),
        miss3: miss(reference, &got, 3),
        extra1: miss(&got, reference, 1),
        extra3: miss(&got, reference, 3),
        frags: fragments(&pos, &idx),
        tris: idx.len() / 3,
        ms,
    }
}

/// 陽関数を 1 つの回転で測る。回転なし・xw・zw では陽関数の正解、xy・xz・混ぜた回転では陰関数の正解を使う
fn measure_explicit(f: &str, rot: Rot) -> Row {
    let src = format!("z = {f}");
    let (r, ms) = mesh(&src, rot, IRES);
    let reference = match rot {
        Rot::None | Rot::Xw | Rot::Zw => reference_explicit(f, rot).unwrap_or_else(|| reference_implicit(&src, rot)),
        _ => reference_implicit(&src, rot),
    };
    measure(&reference, &r, ms)
}

fn measure_implicit(src: &str, rot: Rot, ires: u32) -> Row {
    let (r, ms) = mesh(src, rot, ires);
    measure(&reference_implicit(src, rot), &r, ms)
}

fn table_header() {
    println!(
        "| {:<48} | {:>7} | {:>7} | {:>7} | {:>7} | {:>5} | {:>8} | {:>8} |",
        "式", "欠け1", "欠け3", "余計1", "余計3", "破片", "三角形", "ms"
    );
    println!("|{}|", ["---"; 8].join("|"));
}

fn table_row(label: &str, r: &Row) {
    println!(
        "| {:<48} | {:>7.2} | {:>7.2} | {:>7.2} | {:>7.2} | {:>5} | {:>8} | {:>8.1} |",
        label, r.miss1, r.miss3, r.extra1, r.extra3, r.frags, r.tris, r.ms
    );
}

/// まとめ: (欠けの平均, 1% 以上欠ける式の数, 余計の平均, 破片の合計, 時間の合計)
fn summary(rows: &[Row]) -> (f64, usize, f64, usize, f64) {
    let n = rows.len().max(1) as f64;
    (
        rows.iter().map(|r| r.miss1).sum::<f64>() / n,
        rows.iter().filter(|r| r.miss1 >= 1.0).count(),
        rows.iter().map(|r| r.extra1).sum::<f64>() / n,
        rows.iter().map(|r| r.frags).sum(),
        rows.iter().map(|r| r.ms).sum(),
    )
}

fn summary_header() {
    println!("| {:<14} | {:>10} | {:>14} | {:>10} | {:>8} | {:>10} |", "回転", "欠けの平均", "1% 以上欠ける式", "余計の平均", "破片", "時間 ms");
    println!("|{}|", ["---"; 6].join("|"));
}

fn summary_row(label: &str, rows: &[Row]) {
    let (m, c, e, fr, ms) = summary(rows);
    println!("| {:<14} | {:>10.2} | {:>8} / {:<3} | {:>10.2} | {:>8} | {:>10.0} |", label, m, c, rows.len(), e, fr, ms);
}

/// 陽関数（`z =` を付けて使う）
const EXPLICIT: [&str; 36] = [
    "sin(x) cos(y)",
    "floor(x) + floor(y)",
    "floor(x y / 4)",
    "floor(sqrt(x^2+y^2))",
    "3 (x/2 - floor(x/2))",
    "2 sign(sin(x) sin(y))",
    "tan(x + y)",
    "1/(x^2+y^2)",
    "1/(x y)",
    "1/(sin(x) + cos(y))",
    "3 sin(1/x)",
    "3 sin(10/(x^2+y^2+0.1))",
    "sqrt(36 - x^2 - y^2)",
    "sqrt(4 sin(x) sin(y))",
    "ln(x y)",
    "abs(x) + abs(y) - 5",
    "3 max(sin(x), cos(y))",
    "5 exp(-50((x-1.23)^2 + (y-0.77)^2))",
    "5 exp(-2000((x-1.23)^2 + (y-0.77)^2))",
    "3 sin((x^2 + y^2)/3)",
    "3 sin(5x) sin(5y)",
    "4 x y / (x^2 + y^2)",
    "arg(x + i y)",
    "arctan(y/x)",
    "floor(x) sin(y)",
    "sign(x) y / 2",
    "floor(3 sin(x) cos(y))",
    "floor(3x)/3",
    "tan(x y / 5)",
    "sqrt(abs(x)) - sqrt(abs(y))",
    "sin(x)+sin(3x)/2+sin(9x)/4+sin(27x)/8 + y/5",
    "gamma(x) / 5 + y/5",
    "mod(x, 2) mod(y, 2)",
    "(x^2+y^2)^(1/3) sin(4 arg(x + i y))",
    "x^y / 10",
    "round(x) round(y) / 8",
];

/// 陽関数の追加の組（mod・max・min・nCr・nPr）
const EXPLICIT_EXTRA: [&str; 24] = [
    "mod(x, y)",
    "mod(x y, 3)",
    "mod(x^2 + y^2, 4)",
    "mod(x, 1.5) + mod(y, 1.5)",
    "mod(x, sin(y) + 1.5)",
    "mod(floor(x), 3) + y/5",
    "5 mod(x, 0.4)",
    "mod(x + y, 2) mod(x - y, 2)",
    "max(x, y)/2",
    "min(abs(x), abs(y))",
    "min(tan(x), y)",
    "max(x^2 - 9, min(y^2 - 4, 3))/2",
    "2 max(mod(x, 2), mod(y, 2))",
    "max(floor(x), y)/2",
    "min(1/x, max(1/y, x y))",
    "max(sin(3x), min(cos(3y), x/5)) 3",
    "nCr(x, y)",
    "nCr(x, 3)/20",
    "nCr(5, x) + y/5",
    "nCr(x, y)/nCr(y, x)",
    "nCr(floor(x), floor(y))/10",
    "nPr(x, y)/10",
    "nPr(x, 2)/10 + y/5",
    "nPr(floor(x), floor(y/2))/20",
];

/// 陰関数
const IMPLICIT: [&str; 16] = [
    "sqrt(x)+sqrt(y)+sqrt(z)=3",
    "max(abs(x),max(abs(y),abs(z))) = 4",
    "abs(x)+abs(y)+abs(z)=6",
    "x^2+y^2 = z^2",
    "x y z = 1",
    "sin(x) sin(y) sin(z) = 0.001",
    "x y = 0.0001",
    "x^2 y^2 + y^2 z^2 + z^2 x^2 = 1",
    "ln(x) + ln(y) + ln(z) = 1",
    "tan(x) = z y",
    "arcsin(x/5) + arcsin(y/5) = z",
    "sin(40/(x^2+y^2+z^2+1)) = 0.5",
    "x^(2/3) + y^(2/3) + z^(2/3) = 4",
    "z^2 = sin(x) sin(y)",
    "sqrt(36 - x^2 - y^2) - z = 0",
    "1/(x y) - z = 0",
];

/// 陰関数の追加の組（3 つの座標が全部 mod・floor の中に入るものなど）
const IMPLICIT_EXTRA: [&str; 12] = [
    "mod(x y, z) = 1",
    "z = mod(x y, z)",
    "mod(x y, z) = z/2",
    "mod(x + y, z) = 0.5",
    "mod(x^2 + y^2, z) = 1",
    "mod(x, 2) + mod(y, 2) + mod(z, 2) = 2",
    "mod(x y z, 4) = 2",
    "max(x y, z) = 2",
    "min(x y, z) = mod(x, 2)",
    "floor(x) + floor(y) + z = mod(x y, 3)",
    "mod(z, x y) = 1",
    "floor(x y) = z floor(z)",
];

/// 環境変数 SHAPE_ONLY が付いていれば、それを含む式だけ
fn selected(f: &str) -> bool {
    std::env::var("SHAPE_ONLY").map_or(true, |s| f.contains(s.as_str()))
}

fn report_explicit(title: &str, list: &[&str]) {
    let list: Vec<&str> = list.iter().copied().filter(|f| selected(f)).collect();
    if list.is_empty() {
        return;
    }
    let mut all: Vec<(Rot, Vec<Row>)> = Vec::new();
    for rot in ROTS {
        println!("\n### {title}: {}\n", rot.label());
        table_header();
        let rows: Vec<Row> = list
            .iter()
            .map(|f| {
                let r = measure_explicit(f, rot);
                table_row(&format!("z = {f}"), &r);
                r
            })
            .collect();
        all.push((rot, rows));
    }
    println!("\n### {title}: まとめ（欠け・余計は 1 箱）\n");
    summary_header();
    for (rot, rows) in &all {
        summary_row(rot.label(), rows);
    }
}

#[test]
#[ignore]
fn shape_report() {
    println!("\n## 形の正しさ（表示範囲 x, y ∈ [−10, 10]、z ∈ [−6.4, 6.4]、128³ 個の箱。数字は %）");
    println!("\n欠け1・欠け3 = 正解の箱のうち 1 箱・3 箱以内にメッシュが無い割合。余計 = その逆。破片 = 三角形 8 個以下の成分の数");

    // 正解どうしの確かめ
    if selected("sin(x) cos(y)") {
        let a = reference_explicit("sin(x) cos(y)", Rot::None).unwrap();
        let b = reference_implicit("z = sin(x) cos(y)", Rot::None);
        println!(
            "\n正解どうしの食い違い（z = sin(x) cos(y)、回転なし、1 箱）: 陽関数の正解 → 陰関数の正解 {:.3}%、逆 {:.3}%",
            miss(&a, &b, 1),
            miss(&b, &a, 1)
        );
    }

    println!("\n## 陽関数（細かさ {RES}）");
    report_explicit("陽関数", &EXPLICIT);
    println!("\n## 陽関数の追加の組（mod・max・min・nCr・nPr）");
    report_explicit("追加の組", &EXPLICIT_EXTRA);

    let implicit: Vec<&str> = IMPLICIT.iter().copied().filter(|f| selected(f)).collect();
    if !implicit.is_empty() {
        println!("\n## 陰関数（細かさ {IRES}）");
        let mut all: Vec<(Rot, Vec<Row>)> = Vec::new();
        for rot in ROTS {
            println!("\n### 陰関数: {}\n", rot.label());
            table_header();
            let rows: Vec<Row> = implicit
                .iter()
                .map(|f| {
                    let r = measure_implicit(f, rot, IRES);
                    table_row(f, &r);
                    r
                })
                .collect();
            all.push((rot, rows));
        }
        println!("\n### 陰関数: まとめ（欠け・余計は 1 箱）\n");
        summary_header();
        for (rot, rows) in &all {
            summary_row(rot.label(), rows);
        }
    }

    let extra: Vec<&str> = IMPLICIT_EXTRA.iter().copied().filter(|f| selected(f)).collect();
    if !extra.is_empty() {
        println!("\n## 陰関数の追加の組");
        let mut all: Vec<(String, Vec<Row>)> = Vec::new();
        for (rot, ires) in [(Rot::None, IRES), (Rot::Mixed, IRES), (Rot::None, 200)] {
            let label = format!("{}（細かさ {ires}）", rot.label());
            println!("\n### 陰関数の追加の組: {label}\n");
            table_header();
            let rows: Vec<Row> = extra
                .iter()
                .map(|f| {
                    let r = measure_implicit(f, rot, ires);
                    table_row(f, &r);
                    r
                })
                .collect();
            all.push((label, rows));
        }
        println!("\n### 陰関数の追加の組: まとめ（欠け・余計は 1 箱）\n");
        summary_header();
        for (label, rows) in &all {
            summary_row(label, rows);
        }
    }

    // 極の壁のつながり（回転なし）
    let walls: Vec<&str> =
        ["gamma(x) / 5 + y/5", "nCr(x, y)", "nPr(x, y)/10", "nCr(x, y)/nCr(y, x)"].into_iter().filter(|f| selected(f)).collect();
    if !walls.is_empty() {
        println!("\n## 極の壁のつながり（回転なし。極 x = −1, …, −5 の上の 80 区間のうち、上端と下端の両方に届く区間の数）\n");
        println!("| {:<32} | {:>6} | {:>6} | {:>6} | {:>6} | {:>6} | {:>8} | {:>8} |", "式", "−1", "−2", "−3", "−4", "−5", "三角形", "ms");
        println!("|{}|", ["---"; 8].join("|"));
        for f in walls {
            let (r, ms) = mesh(&format!("z = {f}"), Rot::None, IRES);
            let pos = r.positions();
            let c: Vec<String> = (1..=5).map(|k| wall_coverage(&pos, -(k as f64)).to_string()).collect();
            println!(
                "| {:<32} | {:>6} | {:>6} | {:>6} | {:>6} | {:>6} | {:>8} | {:>8.1} |",
                format!("z = {f}"),
                c[0],
                c[1],
                c[2],
                c[3],
                c[4],
                r.indices().len() / 3,
                ms
            );
        }
    }
}

/// ふつうの `cargo test` で動く小さい確かめ（速い陽関数の正解だけ）。
/// 欠け（1 箱）が 0.5% 以下。以後のステップで、直した式をここに足していく
#[test]
fn shape_regression() {
    let cases = [
        ("sin(x) cos(y)", Rot::None),
        ("floor(x y / 4)", Rot::None),
        ("1/(x y)", Rot::None),
        ("sqrt(36 - x^2 - y^2)", Rot::None),
        ("tan(x + y)", Rot::None),
        // S18: 表示中の座標どうしの回転で、極の近くの面が欠けない（xy 回転は M が回転行列そのものなので、陽関数の正解が使える）
        ("1/(x y)", Rot::Xy),
    ];
    for (f, rot) in cases {
        let (r, _) = mesh(&format!("z = {f}"), rot, IRES);
        let reference = reference_explicit(f, rot).unwrap();
        let got = voxels_of_mesh(&r.positions(), &r.indices());
        let m = miss(&reference, &got, 1);
        assert!(m <= 0.5, "z = {f}（{}）の欠けが {m:.3}%（0.5% 以下のはず）", rot.label());
    }
}

#[test]
fn measure_tools() {
    // 1 つの三角形の箱と、ずれの数え方
    let pos = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let v = voxels_of_mesh(&pos, &[0, 1, 2]);
    assert!(!v.is_empty());
    assert_eq!(miss(&v, &v, 1), 0.0);
    let far: Voxels = v.iter().map(|&(x, y, z)| (x, y, z + 2)).collect();
    assert_eq!(miss(&v, &far, 1), 100.0);
    assert_eq!(miss(&v, &far, 3), 0.0);
    // 破片: 離れた 2 つの三角形
    let pos2 = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 5.0, 5.0, 5.0, 6.0, 5.0, 5.0, 5.0, 6.0, 5.0];
    assert_eq!(fragments(&pos2, &[0, 1, 2, 3, 4, 5]), 2);
    // 逆行列
    let m = [[0.8, 0.0, 0.6], [0.0, 1.0, 0.0], [-0.6, 0.0, 0.8]];
    let inv = inverse(m).unwrap();
    let p = mul(inv, mul(m, [1.0, 2.0, 3.0]));
    assert!((p[0] - 1.0).abs() < 1e-12 && (p[1] - 2.0).abs() < 1e-12 && (p[2] - 3.0).abs() < 1e-12);
}

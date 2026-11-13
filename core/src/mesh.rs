//! 曲面のメッシュ（三角形の集まり）を作る。

use crate::equation::others;
use crate::eval::{as_real, Program, ProgramSet};
use num_complex::Complex64 as C;

mod pieces;

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
    /// 式の説明（`kind`）に足す注意書き（陰関数で、格子を間引いて表示したときなど）。ふつうは空
    pub note: String,
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
        let mut first: crate::adaptive::HashMap<u64, u32> = crate::adaptive::HashMap::default();
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
    /// 辺の中に、ほかにも段差がありそう（段差の線の交わる所を、辺が斜めに通るなど）
    multi: bool,
    /// `multi` のとき、もう 1 つの段差が始点の側（始点〜この段差）にありそう
    rest_at_start: bool,
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
/// 切り分けるのは、表示範囲の高さの 0.5% より大きい段差（再生・ドラッグ中は 3%）。
/// 三角形の3辺のうち2辺で値が飛んでいれば、飛ぶ所を結ぶ線で三角形を2つの部分に分け、それぞれの側の値で描く。
/// 段差の線が三角形の中で終わる・枝分かれする・交わる所（1 辺だけ、または 3 辺とも飛ぶ三角形）は、そこだけを
/// 5 回まで二分して細かくする（足す三角形は 20,000 個まで。再生・ドラッグ中はしない）。
/// その後も残る 1 辺のときは、飛ぶ所と向かいの頂点を結ぶ線で分ける。3辺のときは、頂点ごとに、頂点と両側の辺の
/// 飛ぶ所を結ぶ三角形をその頂点の側の値で描く（真ん中の小さい三角形は描かない）。
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
    explicit_surface_scaled(prog, dom, n, axis, mode, walls, refine, dom.hi[axis] - dom.lo[axis])
}

/// [`explicit_surface_opt`] で、段差・細かくする量の目安にする高さ `h` を別に指定するもの。
/// 回した陽関数は表示範囲より広い範囲で計算するので、目安は表示範囲の高さのままにする
/// （計算する範囲の高さにすると、階段の低い段差を段差とみなさなくなる）
#[allow(clippy::too_many_arguments)]
fn explicit_surface_scaled(
    prog: &Program,
    dom: Domain,
    n: usize,
    axis: usize,
    mode: ComplexMode,
    walls: bool,
    refine: bool,
    h: f64,
) -> Mesh {
    let n = n.max(2);
    let m = n + 1; // 1辺の頂点数
    let (ua, va) = others(axis);
    // 計算する範囲を、幅の 1e-7 ほど内側に縮める（下端と上端で縮める量を変える）。格子の点が、0 や整数などの
    // ちょうどの値に乗らないようにするため。乗ると、その線の上だけの値（`sign(x)` の x = 0 での 0、範囲の上端
    // x = 10 での `floor(x)` の 10 など。両側と違う値）で、幅の無い細い帯を描いてしまう
    let dom = {
        let mut d = dom;
        for a in [ua, va] {
            let w = d.hi[a] - d.lo[a];
            d.lo[a] += 1.3e-7 * w;
            d.hi[a] -= 0.7e-7 * w;
        }
        d
    };

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
    let t_base = crate::timing::now_ms();
    prog.eval_batch(&[&us, &vs], &mut vals);
    let base_ms = crate::timing::now_ms() - t_base;

    // 表示範囲の上下に、範囲の高さ1つ分の余裕を持たせる。
    // はみ出た部分は描画側（Three.js のクリッピング）で切り落とす。
    // 範囲の外の値は、範囲の少し外（wlo, whi）にそろえて描く（極の近くで頂点を捨てると、その周りの三角形が
    // 抜けて穴が空くため）。はみ出た部分は描画側で切り落とされるので見えない
    let (wlo, whi) = (dom.lo[axis] - h, dom.hi[axis] + h);
    let usable = |w: f64| w.is_finite();
    let clamped = |z: C| height(z, mode).clamp(wlo, whi);
    // 切り分ける段差の高さ。再生・ドラッグ中は、候補が増えて遅くならないよう高めのまま
    let threshold = if refine { h * 0.005 } else { h * 0.03 };

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
    // 段差のある辺（細かくしない）
    let mut steps: crate::adaptive::HashSet<(u32, u32)> = Default::default();
    let mut base_jumps: crate::adaptive::HashMap<(u32, u32), Jump> = {
        let hs: Vec<f64> = vals.iter().map(|&z| clamped(z)).collect();
        let mut out: crate::adaptive::HashMap<(u32, u32), Jump> = Default::default();
        for (e, jp) in find_jumps(
            prog,
            &us,
            &vs,
            &vals,
            &hs,
            &grid_edges,
            &clamped,
            threshold,
            &usable,
        ) {
            let key = (grid_edges[e].0 as u32, grid_edges[e].1 as u32);
            steps.insert(key);
            out.insert(key, jp);
        }
        out
    };

    // 格子より細い山・谷・極の「種」（[`hidden_features`]）。再生・ドラッグ中は探さない
    let mut seeds = if refine {
        let hs: Vec<f64> = vals.iter().map(|&z| clamped(z)).collect();
        let interval = mode == ComplexMode::Off && prog.supports_interval();
        hidden_features(prog, dom, (ua, va), n, &hs, h, (dom.lo[axis], dom.hi[axis]), &clamped, interval, base_ms)
    } else {
        SeedMap::default()
    };
    // 細い極の線: 最初の格子の辺の上で極の位置を探して、段差と同じように切り分ける（細分で追いかけない）。線に沿ってたどる
    if refine {
        let hs: Vec<f64> = vals.iter().map(|&z| clamped(z)).collect();
        // 差が範囲の幅の 10% を超える段差（floor などのふつうの段差は、ふつうここまで大きくない）
        let known: Vec<(usize, usize)> = base_jumps
            .iter()
            .filter(|(_, j)| (clamped(j.after) - clamped(j.before)).abs() > 0.1 * (whi - wlo))
            .map(|(&(a, b), _)| (a as usize, b as usize))
            .collect();
        let poles =
            trace_poles(prog, &us, &vs, &hs, n, &seeds, (dom.lo[axis], dom.hi[axis]), (wlo, whi), &clamped, base_ms, &known);
        for (e, jp) in poles {
            steps.insert(e);
            base_jumps.insert(e, jp);
        }
    }
    // 段差・極の辺を持つマスの種は使わない（段差・極の切り分けで描く。極の隣の急な斜面の種で細かくすると、三角形の上限を使い切る）
    if !seeds.is_empty() && !steps.is_empty() {
        let cell_of = |k: u32| ((k as usize) % m, (k as usize) / m);
        let mut cells: crate::adaptive::HashSet<usize> = Default::default();
        for &(a, b) in steps.iter() {
            let ((i0, j0), (i1, j1)) = (cell_of(a), cell_of(b));
            // 辺を持つマス（辺の向きによって 1〜2 個。周りのマスもまとめて外す）
            for j in j0.saturating_sub(1)..=j1.min(n - 1) {
                for i in i0.saturating_sub(1)..=i1.min(n - 1) {
                    cells.insert(j * n + i);
                }
            }
        }
        seeds.cells.retain(|c, _| !cells.contains(c));
    }

    let mut eval = |a: &[f64], b: &[f64]| {
        let mut out = vec![C::new(0.0, 0.0); a.len()];
        prog.eval_batch(&[a, b], &mut out);
        out
    };
    // 形が急に変わる所を細かくする
    let mut r = {
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
            // 種を含む三角形は、斜辺が種の大きさの 2 倍になるまで必ず分ける（いちばん先に）
            if let Some(x) = seeds.force(&c.pos) {
                return Some(SEED_PRIORITY * h + x);
            }
            // 種の無い三角形は、今までどおり 6 回のパスまで（種のある三角形は、ずれで分けるのも 16 回まで続ける）
            if c.pass >= NORMAL_PASSES && !seeds.touches(&c.pos) {
                return None;
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
            passes: if !refine { 0 } else if seeds.is_empty() { NORMAL_PASSES } else { SEED_PASSES },
            max_new_triangles: base_tris.max(30_000),
        };
        let (u0, u1) = (dom.lo[ua], dom.hi[ua]);
        let (v0, v1) = (dom.lo[va], dom.hi[va]);
        crate::adaptive::refine_keep(n, (u0, u1), (v0, v1), vals, &mut eval, &mut decide, limits)
    };

    // 細かくしてできた辺の段差を探す（最初の格子のままの辺は、もう調べてある）。
    // 二分してできた辺は、どれも新しい頂点を端に持つので、番号が `from` 以上の頂点を持つ辺だけを調べればよい
    let mut jumps = base_jumps;
    let ek = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
    let search_new_edges = |r: &crate::adaptive::Triangulation<C>,
                            tris: &[[u32; 3]],
                            from: usize,
                            jumps: &mut crate::adaptive::HashMap<(u32, u32), Jump>| {
        let mut new_edges: Vec<(usize, usize)> = Vec::new();
        let mut seen: crate::adaptive::HashSet<(u32, u32)> = Default::default();
        for t in tris {
            if t.iter().all(|&k| (k as usize) < from) {
                continue;
            }
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let e = ek(a, b);
                if (e.1 as usize) >= from && seen.insert(e) {
                    new_edges.push((e.0 as usize, e.1 as usize));
                }
            }
        }
        if new_edges.is_empty() {
            return;
        }
        let (us, vs): (Vec<f64>, Vec<f64>) = r.uv().iter().map(|p| (p[0], p[1])).unzip();
        let vals = r.vals();
        let hs_clamped: Vec<f64> = vals.iter().map(|&z| clamped(z)).collect();
        for (e, jp) in find_jumps(prog, &us, &vs, vals, &hs_clamped, &new_edges, &clamped, threshold, &usable) {
            jumps.insert((new_edges[e].0 as u32, new_edges[e].1 as u32), jp);
        }
    };
    let mut tris = r.tris();
    search_new_edges(&r, &tris, m * m, &mut jumps);

    // 段差の線が三角形の中で終わる・枝分かれする・交わる所（1 辺だけ、または 3 辺とも飛ぶ三角形と、
    // 1 本の辺に段差が 2 つ以上ありそうな三角形）だけを細かくする。
    // 段差の線の途中（2 辺で飛ぶ三角形）は、切り分けだけで正しく描ける
    if refine && !jumps.is_empty() {
        const ROUNDS: usize = 5;
        const MAX_NEW: usize = 20_000;
        let mut added = 0;
        for _ in 0..ROUNDS {
            let hs: Vec<f64> = r.vals().iter().map(|&z| height(z, mode)).collect();
            let mut marked: Vec<[u32; 3]> = tris
                .iter()
                .filter(|t| {
                    t.iter().all(|&k| usable(hs[k as usize])) && {
                        let js: Vec<&Jump> = [ek(t[0], t[1]), ek(t[1], t[2]), ek(t[2], t[0])]
                            .iter()
                            .filter_map(|e| jumps.get(e))
                            .collect();
                        js.len() == 1 || js.len() == 3 || js.iter().any(|j| j.multi)
                    }
                })
                .copied()
                .collect();
            if marked.is_empty() || added >= MAX_NEW {
                break;
            }
            // 1 つ分けると、継ぎ目を合わせる分も含めて 2 つほど増える
            let room = (MAX_NEW - added) / 2;
            let last = marked.len() > room;
            marked.truncate(room.max(1));
            let from = r.uv().len();
            added += r.bisect(&marked, &mut eval);
            tris = r.tris();
            search_new_edges(&r, &tris, from, &mut jumps);
            if last {
                break;
            }
        }
        // 分けられて無くなった辺の段差は消す
        let mut alive: crate::adaptive::HashSet<(u32, u32)> = Default::default();
        for t in &tris {
            for e in [ek(t[0], t[1]), ek(t[1], t[2]), ek(t[2], t[0])] {
                if jumps.contains_key(&e) {
                    alive.insert(e);
                }
            }
        }
        jumps.retain(|e, _| alive.contains(e));
    }

    let r = r.finish();
    let count = r.uv.len();
    let us: Vec<f64> = r.uv.iter().map(|p| p[0]).collect();
    let vs: Vec<f64> = r.uv.iter().map(|p| p[1]).collect();
    let vals = r.vals;
    let hs: Vec<f64> = vals.iter().map(|&z| height(z, mode)).collect();

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
    // 壁の四角形: 段差の線の両端の (u, v, lone の側の値, 反対側の値) と、lone の頂点の (u, v)
    type WallEnd = (f64, f64, C, C);
    let mut wall_quads: Vec<([WallEnd; 2], (f64, f64))> = Vec::new();

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
        if jumped.is_empty() {
            plain.push(vtx); // 表示範囲の上端・下端で切るものがあるので、後でまとめて描く
            continue;
        }
        // 飛ぶ辺ごとの (三角形の辺の始点の側の頂点, 終点の側の頂点)
        let mut jv: [Option<(u32, u32)>; 3] = [None; 3];
        // 辺 e の飛ぶ所の (u, v)
        let jump_at = |e: usize| {
            let (id, _) = edges[e];
            let jp = jumps[&id];
            let (k0, k1) = (id.0 as usize, id.1 as usize);
            (us[k0] + (us[k1] - us[k0]) * jp.t, vs[k0] + (vs[k1] - vs[k0]) * jp.t)
        };
        // 3 辺とも飛ぶとき、頂点 k の段の、辺 k（k の側）・辺 k + 2（k の側）の飛ぶ所での値
        let mut side_vals: [[C; 2]; 3] = [[C::new(0.0, 0.0); 2]; 3];
        if jumped.len() == 3 {
            // 3辺とも飛ぶ: 辺にもう 1 つ段差がありそうで、それが頂点 p の側にあるときは、見つけた段差の手前の値は
            // p の段の値ではないので、p の値で頂点を置く（頂点を足す順番は、角の三角形を描く順）
            let mut sides = [[NONE; 2]; 3];
            // 頂点 p の側の頂点と、その値
            let near = |mesh: &mut Mesh, e: usize, p: u32| -> (u32, C) {
                let (id, _) = edges[e];
                let (a, b) = jump_vertex[&id];
                let jp = jumps[&id];
                let at_start = id.0 == p;
                if jp.multi && jp.rest_at_start == at_start {
                    let (u, v) = jump_at(e);
                    (push(mesh, u, v, vals[p as usize]), vals[p as usize])
                } else if at_start {
                    (a, jp.before)
                } else {
                    (b, jp.after)
                }
            };
            for k in 0..3 {
                let (i0, z0) = near(&mut mesh, k, t[k]);
                let (i1, z1) = near(&mut mesh, (k + 2) % 3, t[k]);
                sides[k][0] = i0;
                sides[(k + 2) % 3][1] = i1;
                side_vals[k] = [z0, z1];
            }
            for k in 0..3 {
                jv[k] = Some((sides[k][0], sides[k][1]));
            }
        } else {
            for &e in &jumped {
                let (id, forward) = edges[e];
                let (start, end) = jump_vertex[&id];
                jv[e] = Some(if forward { (start, end) } else { (end, start) });
            }
        }
        let ids = [index_of[vtx[0]], index_of[vtx[1]], index_of[vtx[2]]];
        let (pieces, lone) = cut_by_jumps(ids, jv);
        for piece in pieces {
            emit(&mut mesh, piece);
        }
        if jumped.len() == 3 {
            // 段差の線の交わる所: 3 つの飛ぶ所への距離の和がいちばん小さい点 c（フェルマー点）を、段差の線が交わる点とみなす。
            // 重心にすると、細長い三角形では、交わる点から段差の線に沿ってずれ、向きの違う壁ができる
            // 頂点 k の段を c まで延ばし（真ん中の三角形を 3 つに分ける）、隣の段との間（飛ぶ所 → c）に壁を立てる。
            // 真ん中を空けたままにすると、段の角に、向こうが見える隙間が開く
            let at: [(f64, f64); 3] = [jump_at(0), jump_at(1), jump_at(2)];
            let (cu, cv) = fermat_point(at);
            for k in 0..3 {
                let (jk, jprev) = jv[k].zip(jv[(k + 2) % 3]).map(|(a, b)| (a.0, b.1)).unwrap();
                // c での頂点 k の段の値（2 つの飛ぶ所の値の平均。floor などの段ではどちらも同じ）
                let zc = (side_vals[k][0] + side_vals[k][1]) * 0.5;
                let c = push(&mut mesh, cu, cv, zc);
                emit(&mut mesh, [jk, c, jprev]);
            }
            if walls {
                // 辺 k の飛ぶ所 → c の壁。頂点 k の段（辺 k の始点の側）と、頂点 k + 1 の段（辺 k の終点の側）の間
                for k in 0..3 {
                    let k1 = (k + 1) % 3;
                    let (near_j, far_j) = (side_vals[k][0], side_vals[k1][1]);
                    let near_c = (side_vals[k][0] + side_vals[k][1]) * 0.5;
                    let far_c = (side_vals[k1][0] + side_vals[k1][1]) * 0.5;
                    let p = vtx[k];
                    wall_quads.push(([(at[k].0, at[k].1, near_j, far_j), (cu, cv, near_c, far_c)], (us[p], vs[p])));
                }
            }
        } else if jumped.len() == 1 && walls {
            // 段差の線が三角形の中で終わる: 飛ぶ所から向かいの頂点 w まで、段差が細くなって消える三角形の壁
            // （2 つの側の面は w でつながるが、飛ぶ所では高さが違うので、間に隙間が開く）
            let e = jumped[0];
            let (id, forward) = edges[e];
            let jp = jumps[&id];
            let (start, end) = if forward { (jp.before, jp.after) } else { (jp.after, jp.before) };
            let (ju, jv_) = jump_at(e);
            let (p, w) = (vtx[e], vtx[(e + 2) % 3]);
            wall_quads.push(([(ju, jv_, start, end), (us[w], vs[w], vals[w], vals[w])], (us[p], vs[p])));
        }
        // 段差の壁（段差の線が三角形を横切る所）
        if let (Some(lone), true) = (lone, walls) {
            let e_in = (lone + 2) % 3;
            let e_out = lone;
            let p = vtx[lone];
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
            wall_quads.push(([info(e_out), info(e_in)], (us[p], vs[p])));
        }
    }
    clip_to_height_range(prog, &mut mesh, &plain, &us, &vs, &vals, &index_of, axis, mode, (dom.lo[axis], dom.hi[axis]), flip, &push);
    compute_normals(&mut mesh);
    // 段差のある曲面は、頂点をまとめた粗い版にすると段差と壁が崩れるので、粗い版を作らない
    mesh.keep_full = !jumps.is_empty();

    // 段差の壁: 段差の線に沿って、lone の側の縁と反対側の縁を結ぶ縦の四角形（頂点は共有しない）
    for ([(u0, v0, n0, f0), (u1, v1, n1, f1)], (lu, lv)) in wall_quads {
        let base = mesh.positions.len() as u32 / 3;
        for (u, v, z) in [(u0, v0, n0), (u1, v1, n1), (u1, v1, f1), (u0, v0, f0)] {
            push(&mut mesh, u, v, z);
        }
        // 表示中の座標の点（u, v は高さでない 2 つの軸、h は高さの軸）
        let at = |u: f64, v: f64, h: f64| {
            let mut q = [0.0; 3];
            q[ua] = u;
            q[va] = v;
            q[axis] = h;
            q
        };
        let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        // 法線: 段差の線に垂直な水平の向きのうち、段の低い側（外から壁が見える側）を向くもの。
        // 段差の線の向きは三角形ごとに入れ替わるので、線の向きだけで決めると、隣の区間と法線が逆になって
        // 明るさが交互に変わる（壁が縞模様になる）
        let horizontal = cross(at(u1 - u0, v1 - v0, 0.0), at(0.0, 0.0, 1.0));
        let to_lone = at(lu - 0.5 * (u0 + u1), lv - 0.5 * (v0 + v1), 0.0);
        // どちらの段が低いかは、高さの差が大きいほうの端で決める（段差の線の交わる所では、
        // 交わる点の側の端で、段の組み合わせが変わって差が小さくなったり 0 になったりする）
        let (near, far) = if (clamped(n0) - clamped(f0)).abs() >= (clamped(n1) - clamped(f1)).abs() {
            (clamped(n0), clamped(f0))
        } else {
            (clamped(n1), clamped(f1))
        };
        let toward_near = if dot(horizontal, to_lone) >= 0.0 { horizontal } else { horizontal.map(|c| -c) };
        let nv = if near <= far { toward_near } else { toward_near.map(|c| -c) };
        // 三角形の向き（表側）も法線にそろえる
        let (p0, p1, p2) = (at(u0, v0, clamped(n0)), at(u1, v1, clamped(n1)), at(u1, v1, clamped(f1)));
        if dot(cross(sub(p1, p0), sub(p2, p0)), nv) >= 0.0 {
            mesh.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            mesh.indices.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
        }
        let len = dot(nv, nv).sqrt().max(1e-300);
        for _ in 0..4 {
            mesh.normals.extend(nv.iter().map(|c| (c / len) as f32));
        }
    }
    mesh
}

/// 3 点への距離の和がいちばん小さい点（フェルマー点）。120° 以上の角があればその頂点、なければ Weiszfeld 法で求める
fn fermat_point(p: [(f64, f64); 3]) -> (f64, f64) {
    for k in 0..3 {
        let (a, b, c) = (p[k], p[(k + 1) % 3], p[(k + 2) % 3]);
        let (u, v) = ((b.0 - a.0, b.1 - a.1), (c.0 - a.0, c.1 - a.1));
        let (lu, lv) = ((u.0 * u.0 + u.1 * u.1).sqrt(), (v.0 * v.0 + v.1 * v.1).sqrt());
        // 2 点が重なっている（長さ 0 の辺がある）か、角が 120° 以上
        if lu * lv == 0.0 || (u.0 * v.0 + u.1 * v.1) <= -0.5 * lu * lv {
            return a;
        }
    }
    let mut x = ((p[0].0 + p[1].0 + p[2].0) / 3.0, (p[0].1 + p[1].1 + p[2].1) / 3.0);
    for _ in 0..40 {
        let (mut su, mut sv, mut sw) = (0.0, 0.0, 0.0);
        for q in p {
            let d = ((q.0 - x.0).powi(2) + (q.1 - x.1).powi(2)).sqrt().max(1e-300);
            su += q.0 / d;
            sv += q.1 / d;
            sw += 1.0 / d;
        }
        x = (su / sw, sv / sw);
    }
    x
}

/// 値が飛ぶ辺のある三角形を、飛ぶ所で切り分ける（陽関数と媒介変数の曲面で共通。頂点の中身によらない）。
/// `t` は三角形の頂点の番号、`jv[e]` は辺 e（頂点 e → e+1）が飛ぶなら、飛ぶ所に置いた (頂点 e の側の頂点, 頂点 e+1 の側の頂点)。
/// 戻り値は、三角形の一覧（向きは `t` と同じ）と、2 辺で飛ぶときの「段差の片側に 1 つだけある頂点」（壁を作るのに使う）。
/// - 2 辺で飛ぶ（段差の線が横切る）: 1 つだけの頂点の側の三角形と、反対側の四角形（三角形 2 つ）
/// - 1 辺だけ飛ぶ（段差の線が三角形の中で終わる）: 飛ぶ所と向かいの頂点を結ぶ線で 2 つに分け、それぞれの側の値で描く
///   （段差は向かいの頂点に向かって細くなる。描かないと、そこだけ穴が空く）
/// - 3 辺とも飛ぶ（段差の線の交わる所・値が細かく暴れる所）: 頂点ごとに、頂点とその両側の辺の飛ぶ所（頂点の側）を結ぶ
///   三角形を描く。真ん中（3 つの飛ぶ所を結ぶ三角形）は値が決まらないので描かない
fn cut_by_jumps(t: [u32; 3], jv: [Option<(u32, u32)>; 3]) -> (Vec<[u32; 3]>, Option<usize>) {
    let jumped: Vec<usize> = (0..3).filter(|&e| jv[e].is_some()).collect();
    match jumped.len() {
        0 => (vec![t], None),
        1 => {
            let e = jumped[0];
            let (near_p, near_q) = jv[e].unwrap();
            let (p, q, w) = (t[e], t[(e + 1) % 3], t[(e + 2) % 3]);
            (vec![[p, near_p, w], [near_q, q, w]], None)
        }
        2 => {
            // 辺 e は頂点 e → e+1 なので、飛ばない辺 ne の反対の頂点は ne + 2
            let ne = (0..3).find(|e| !jumped.contains(e)).unwrap_or(0);
            let lone = (ne + 2) % 3;
            // lone から出る辺 e_out の始点の側と、lone に入る辺 e_in の終点の側が、lone の側
            let (e_out, e_in) = (lone, (lone + 2) % 3);
            let (out_lone, out_far) = jv[e_out].unwrap();
            let (in_far, in_lone) = jv[e_in].unwrap();
            let (p, q, r) = (t[lone], t[(lone + 1) % 3], t[(lone + 2) % 3]);
            (vec![[p, out_lone, in_lone], [out_far, q, r], [out_far, r, in_far]], Some(lone))
        }
        _ => {
            let mut out = Vec::with_capacity(3);
            for k in 0..3 {
                // 辺 k（k → k+1）の k の側と、辺 k+2（k+2 → k）の k の側
                out.push([t[k], jv[k].unwrap().0, jv[(k + 2) % 3].unwrap().1]);
            }
            (out, None)
        }
    }
}

/// 細い極の線（`nCr(x, y)` の x = −1, −2, … など）を、最初の格子の辺の上で探す。極は「辺の上のどこか 1 点で、値が
/// 上の端から下の端へ飛ぶ所」なので、見つかった辺を段差と同じように切り分ければ、極を細分で追いかけなくてよい
/// （細分で追いかけると、極の線が何本もある式では三角形の上限を使い切り、壁が途切れる）。
///
/// 1. 調べ始める辺: S21 の種を含むマスの辺と、辺の中点の値が見える高さの外で、両端は中にある辺
///    （中点は、区間演算の使えない式なら [`hidden_features`] が計算してある。使える式では計算しない。
///    使えない式でも最初の格子の計算が 3 ms 以上なら、計算していないので、4 辺に 1 辺だけ中点を計算する）
/// 2. 辺を 16 等分した点で f を計算し、隣どうしの点の高さ（範囲 `ends` にそろえたもの）の差がいちばん大きい区間を、
///    差の大きいほうの半分に狭めて 30 回二分する。狭めた両側が、一方は上の端の近く、もう一方は下の端の近くに届いているか、
///    差が最初の 4 倍以上に広がって範囲の幅の 10% を超えていれば、極とみなし、両側の値を [`Jump`] にする（ふつうの零点・なめらかな所は差が縮むので外れる。両側とも同じ向きに大きくなる極、
///    `1/(x^2+y^2)` などは、細分で描けるので扱わない）。段差探しで見つかった差の大きい段差の辺の隣からも、たどり始める
/// 3. 極の見つかった辺を持つ三角形の、残りの辺を同じように調べ、見つかれば隣の三角形へ進む（合わせて 30,000 辺まで）
#[allow(clippy::too_many_arguments)]
fn trace_poles(
    prog: &Program,
    us: &[f64],
    vs: &[f64],
    hs: &[f64],
    n: usize,
    seeds: &SeedMap,
    visible: (f64, f64),
    ends: (f64, f64),
    height: &dyn Fn(C) -> f64,
    base_ms: f64,
    known: &[(usize, usize)],
) -> crate::adaptive::HashMap<(u32, u32), Jump> {
    const SAMPLES: usize = 16;
    // 細い極（nCr(x, y) の x = −8 は、極から 10⁻⁶ ほどの所でしか端に届かない）まで狭める
    const ROUNDS: usize = 30;
    const MAX_EDGES: usize = 30_000;
    let m = n + 1;
    let (lo, hi) = visible;
    let (wlo, whi) = ends;
    // 端の近く（範囲の幅の 5% 以内）
    let near = 0.05 * (whi - wlo);
    let at_top = |h: f64| h.is_finite() && h >= whi - near;
    let at_bottom = |h: f64| h.is_finite() && h <= wlo + near;
    let key = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };
    // 格子の三角形（マス (i, j) の 0: (a, b, d)、1: (a, d, c)）の辺
    let tri_edges = |i: usize, j: usize, t: usize| -> [(usize, usize); 3] {
        let a = j * m + i;
        let (b, c, d) = (a + 1, a + m, a + m + 1);
        if t == 0 { [key(a, b), key(b, d), key(a, d)] } else { [key(a, d), key(d, c), key(a, c)] }
    };
    // 辺を持つ三角形（1 つか 2 つ）
    let edge_tris = |e: (usize, usize)| -> Vec<(usize, usize, usize)> {
        let (k0, k1) = e;
        let (i, j) = (k0 % m, k0 / m);
        let mut out = Vec::with_capacity(2);
        if k1 == k0 + 1 {
            // 横の辺: 下のマスの 0、上のマス (j − 1) の 1
            if j < n {
                out.push((i, j, 0));
            }
            if j > 0 {
                out.push((i, j - 1, 1));
            }
        } else if k1 == k0 + m {
            // 縦の辺: このマスの 1、左のマスの 0
            if i < n {
                out.push((i, j, 1));
            }
            if i > 0 {
                out.push((i - 1, j, 0));
            }
        } else {
            out.push((i, j, 0));
            out.push((i, j, 1));
        }
        out
    };

    // 1. 調べ始める辺
    let mut start: Vec<(usize, usize)> = Vec::new();
    for &c in seeds.cells.keys() {
        let (i, j) = (c % n, c / n);
        for t in 0..2 {
            start.extend_from_slice(&tri_edges(i, j, t));
        }
    }
    let outside = |h: f64| !h.is_finite() || h > hi || h < lo;
    let inside = |h: f64| h.is_finite() && (lo..=hi).contains(&h);
    if !seeds.mids.is_empty() {
        for &(e, h) in &seeds.mids {
            if outside(h) && inside(hs[e.0]) && inside(hs[e.1]) {
                start.push(key(e.0, e.1));
            }
        }
    } else if base_ms >= 3.0 && !prog.supports_interval() {
        // 重い式: 4 辺に 1 辺だけ中点を計算する
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut c = 0usize;
        for j in 0..m {
            for i in 0..m {
                let k = j * m + i;
                for (ok, e) in [(i + 1 < m, (k, k + 1)), (j + 1 < m, (k, k + m)), (i + 1 < m && j + 1 < m, (k, k + m + 1))] {
                    if ok {
                        if c.is_multiple_of(4) && inside(hs[e.0]) && inside(hs[e.1]) {
                            edges.push(e);
                        }
                        c += 1;
                    }
                }
            }
        }
        let xs: Vec<f64> = edges.iter().map(|e| 0.5 * (us[e.0] + us[e.1])).collect();
        let ys: Vec<f64> = edges.iter().map(|e| 0.5 * (vs[e.0] + vs[e.1])).collect();
        let mut out = vec![C::new(0.0, 0.0); edges.len()];
        prog.eval_batch(&[&xs, &ys], &mut out);
        for (e, z) in edges.into_iter().zip(out) {
            if outside(height(z)) {
                start.push(e);
            }
        }
    }
    // 段差探し（[`find_jumps`]）で見つかった、差の大きい段差（極かもしれない）の辺の隣からもたどる
    // （その辺そのものは、段差探しが極のすぐそばまで狭めてあるので、調べ直さない）
    let mut checked: crate::adaptive::HashSet<(usize, usize)> = Default::default();
    for &e in known {
        checked.insert(e);
        for (i, j, t) in edge_tris(e) {
            start.extend(tri_edges(i, j, t).into_iter().filter(|x| *x != e));
        }
    }
    let mut found: crate::adaptive::HashMap<(u32, u32), Jump> = Default::default();
    if start.is_empty() {
        return found;
    }
    start.sort_unstable();
    start.dedup();
    let mut todo = start;
    while !todo.is_empty() && checked.len() < MAX_EDGES {
        let room = MAX_EDGES - checked.len();
        let batch: Vec<(usize, usize)> = todo.drain(..).filter(|e| checked.insert(*e)).take(room).collect();
        if batch.is_empty() {
            break;
        }
        // 2. 辺を 16 等分した点
        let mut xs = Vec::with_capacity(batch.len() * (SAMPLES - 1));
        let mut ys = Vec::with_capacity(batch.len() * (SAMPLES - 1));
        for &(k0, k1) in &batch {
            for s in 1..SAMPLES {
                let t = s as f64 / SAMPLES as f64;
                xs.push(us[k0] + (us[k1] - us[k0]) * t);
                ys.push(vs[k0] + (vs[k1] - vs[k0]) * t);
            }
        }
        let mut zs = vec![C::new(0.0, 0.0); xs.len()];
        prog.eval_batch(&[&xs, &ys], &mut zs);
        // 区間 (辺の番号, ta, tb, 値 a, 値 b)
        let mut iv: Vec<(usize, f64, f64, C, C)> = Vec::new();
        // 区間ごとの、最初の差
        let mut first: Vec<f64> = Vec::new();
        for (b, &(k0, k1)) in batch.iter().enumerate() {
            let z_at = |s: usize| -> Option<C> {
                if s == 0 || s == SAMPLES {
                    return None;
                }
                Some(zs[b * (SAMPLES - 1) + s - 1])
            };
            let h_at = |s: usize| -> f64 {
                match s {
                    0 => hs[k0],
                    _ if s == SAMPLES => hs[k1],
                    _ => height(z_at(s).unwrap_or(C::new(f64::NAN, 0.0))),
                }
            };
            // 差がいちばん大きい隣どうしの区間（極なら、そこで値が上の端と下の端へ分かれていく）
            let mut best: Option<(usize, f64)> = None;
            for s in 0..SAMPLES {
                let (ha, hb) = (h_at(s), h_at(s + 1));
                let d = (hb - ha).abs();
                if d.is_finite() && best.is_none_or(|(_, bd)| d > bd) {
                    best = Some((s, d));
                }
            }
            if let Some((s, d0)) = best {
                first.push(d0);
                // 区間の両端の値（格子点は高さしか持っていないので、下でまとめて計算し直す）
                iv.push((b, s as f64 / SAMPLES as f64, (s + 1) as f64 / SAMPLES as f64, C::new(f64::NAN, 0.0), C::new(f64::NAN, 0.0)));
            }
        }
        // 区間の両端の値を計算し直してから、二分する
        {
            let mut xs = Vec::with_capacity(2 * iv.len());
            let mut ys = Vec::with_capacity(2 * iv.len());
            for &(b, ta, tb, ..) in &iv {
                let (k0, k1) = batch[b];
                for t in [ta, tb] {
                    xs.push(us[k0] + (us[k1] - us[k0]) * t);
                    ys.push(vs[k0] + (vs[k1] - vs[k0]) * t);
                }
            }
            let mut zs = vec![C::new(0.0, 0.0); xs.len()];
            prog.eval_batch(&[&xs, &ys], &mut zs);
            for (q, c) in iv.iter_mut().enumerate() {
                c.3 = zs[2 * q];
                c.4 = zs[2 * q + 1];
            }
        }
        // 3 回目の二分の後の差（そこから縮まなければ、値が飛んでいる）
        let mut mid_diff = vec![f64::NAN; iv.len()];
        for round in 0..ROUNDS {
            if round == 3 {
                for (d, c) in mid_diff.iter_mut().zip(&iv) {
                    *d = (height(c.4) - height(c.3)).abs();
                }
            }
            let mut xs = Vec::with_capacity(iv.len());
            let mut ys = Vec::with_capacity(iv.len());
            for &(b, ta, tb, ..) in &iv {
                let (k0, k1) = batch[b];
                let t = 0.5 * (ta + tb);
                xs.push(us[k0] + (us[k1] - us[k0]) * t);
                ys.push(vs[k0] + (vs[k1] - vs[k0]) * t);
            }
            let mut zs = vec![C::new(0.0, 0.0); xs.len()];
            prog.eval_batch(&[&xs, &ys], &mut zs);
            for (c, &z) in iv.iter_mut().zip(&zs) {
                let t = 0.5 * (c.1 + c.2);
                let (ha, hb, hm) = (height(c.3), height(c.4), height(z));
                if !hm.is_finite() || (hm - ha).abs() >= (hb - hm).abs() {
                    c.2 = t;
                    c.4 = z;
                } else {
                    c.1 = t;
                    c.3 = z;
                }
            }
        }
        for ((&(b, ta, tb, fa, fb), &d0), &d3) in iv.iter().zip(&first).zip(&mid_diff) {
            let (ha, hb) = (height(fa), height(fb));
            // 極とみなすのは、狭めた後に次のどれか:
            // - 上の端と下の端の両方に届いている
            // - 差が最初の 4 倍以上に広がり、範囲の幅の 10% を超えている（極のすぐそばでも端まで届かない細い極）
            // - 差が 3 回目の二分の後から縮んでおらず、片側が端に届いている（格子の点が極のすぐ隣にあり、最初から片側が端にある）
            // 零点・なめらかな所は差が縮むので外れる。floor などの段差は、ふつう端（見える範囲のさらに外）に届かないので外れる
            let d = (hb - ha).abs();
            let spans = (at_top(ha) && at_bottom(hb)) || (at_bottom(ha) && at_top(hb));
            let grows = d.is_finite() && d >= 4.0 * d0 && d > 0.1 * (whi - wlo);
            let one_end = [ha, hb].iter().any(|&x| at_top(x) || at_bottom(x));
            let kept = d.is_finite() && d >= 0.9 * d3 && d > 0.1 * (whi - wlo) && one_end;
            if !(spans || grows || kept) {
                continue;
            }
            let (k0, k1) = batch[b];
            found.insert(
                (k0 as u32, k1 as u32),
                Jump { t: 0.5 * (ta + tb), before: fa, after: fb, multi: false, rest_at_start: false },
            );
            // 3. 線をたどる: この辺を持つ三角形の、残りの辺
            for (i, j, t) in edge_tris((k0, k1)) {
                for e in tri_edges(i, j, t) {
                    if !checked.contains(&e) {
                        todo.push(e);
                    }
                }
            }
        }
        todo.sort_unstable();
        todo.dedup();
    }
    found
}

/// 陽関数の細分のパスの数（種の無い三角形）と、種のある三角形のパスの数
const NORMAL_PASSES: usize = 6;
const SEED_PASSES: usize = 16;
/// 種を含む三角形のずれ（ほかのどの三角形より先に分ける）
const SEED_PRIORITY: f64 = 1e9;
/// 区間のはみ出しの許し τ = 0.02 h + TAU_SPREAD (M − m) の係数
const TAU_SPREAD: f64 = 0.1;

/// 格子より細い山・谷・極の見つかった点（[`hidden_features`]）
#[derive(Clone, Copy, Debug)]
struct Seed {
    u: f64,
    v: f64,
    /// 見つけたときの箱の大きさ（u の幅）
    size: f64,
    /// 最初の格子の双 1 次補間からのずれ
    excess: f64,
}

/// 種を、最初の格子のマスごとに分けて持つ（三角形に入る種を速く探すため）
#[derive(Default)]
struct SeedMap {
    n: usize,
    u: (f64, f64),
    v: (f64, f64),
    cells: crate::adaptive::HashMap<usize, Vec<Seed>>,
    /// 区間演算が使えない式で計算した、最初の格子の辺の中点の高さ（辺の両端の格子点の番号, 高さ）。[`trace_poles`] が使う
    mids: Vec<((usize, usize), f64)>,
}

impl SeedMap {
    fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    fn cell_of(&self, p: [f64; 2]) -> (usize, usize) {
        let n = self.n as f64;
        let i = ((p[0] - self.u.0) / (self.u.1 - self.u.0) * n).floor().clamp(0.0, n - 1.0) as usize;
        let j = ((p[1] - self.v.0) / (self.v.1 - self.v.0) * n).floor().clamp(0.0, n - 1.0) as usize;
        (i, j)
    }

    fn insert(&mut self, s: Seed) {
        let (i, j) = self.cell_of([s.u, s.v]);
        self.cells.entry(j * self.n + i).or_default().push(s);
    }

    /// 三角形 `pos` が、まだ分けなければならない種（斜辺が種の大きさの 2 倍より長い）を含むなら、その種のずれ（いちばん大きいもの）
    fn force(&self, pos: &[[f64; 2]; 3]) -> Option<f64> {
        self.find(pos, true)
    }

    /// 三角形 `pos` が種を含むか
    fn touches(&self, pos: &[[f64; 2]; 3]) -> bool {
        self.find(pos, false).is_some()
    }

    fn find(&self, pos: &[[f64; 2]; 3], must_split: bool) -> Option<f64> {
        if self.cells.is_empty() {
            return None;
        }
        let hyp = ((pos[1][0] - pos[0][0]).powi(2) + (pos[1][1] - pos[0][1]).powi(2)).sqrt();
        let lo = [0, 1].map(|a| pos.iter().map(|p| p[a]).fold(f64::INFINITY, f64::min));
        let hi = [0, 1].map(|a| pos.iter().map(|p| p[a]).fold(f64::NEG_INFINITY, f64::max));
        let (i0, j0) = self.cell_of(lo);
        let (i1, j1) = self.cell_of(hi);
        // 三角形の中か（辺の上も含める）
        let cross = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
        let area = cross(pos[0], pos[1], pos[2]);
        let eps = 1e-12 * (hyp * hyp + 1e-300);
        let inside = |p: [f64; 2]| {
            (0..3).all(|k| cross(pos[k], pos[(k + 1) % 3], p) * area.signum() >= -eps)
        };
        let mut best: Option<f64> = None;
        for j in j0..=j1 {
            for i in i0..=i1 {
                let Some(list) = self.cells.get(&(j * self.n + i)) else { continue };
                for s in list {
                    if (!must_split || hyp > 2.0 * s.size) && inside([s.u, s.v]) {
                        best = Some(best.map_or(s.excess, |b: f64| b.max(s.excess)));
                    }
                }
            }
        }
        best
    }
}

/// 格子の点がたまたま乗らないと見えない、格子より細い山・谷・極を探して、その点（種）を返す。
///
/// 区間演算が使える式（`interval`）: 最初の格子（`n` マス、値は `hs`。表示範囲の高さ `h`、見える高さ `visible`）を
/// 小さいブロック（細かさ 150 なら 2 × 2 マス）に分け、ブロックの上で f の値が入る区間を求める。区間を見える高さに切ったもの [a, b] が、
/// ブロックの中の格子点の値の範囲 [m, M] から τ = 0.02 h + 0.1 (M − m) より外にはみ出していれば（`Unknown` も）疑わしい。
/// 疑わしい箱は、はみ出しの大きい順に四つに分け、新しい点（中心と辺の中点）で f を計算して、小さい箱ごとに調べ直す
/// （深さ 8 段、最初の格子の 1 マスの 1/32 まで。区間演算は合わせて 30,000 回まで）。
/// 計算した点の値が、最初の格子の双 1 次補間から 0.004 h より離れていて、見える高さにかかっていれば種にする。
///
/// 区間演算が使えない式（特殊関数・色分け）: 最初の格子の計算（`base_ms`）が 3 ms 未満のときだけ、全部のマスの中心と
/// 辺の中点で f を計算して、同じ基準で種にする（それより重い式では何もしない）
#[allow(clippy::too_many_arguments)]
fn hidden_features(
    prog: &Program,
    dom: Domain,
    (ua, va): (usize, usize),
    n: usize,
    hs: &[f64],
    h: f64,
    visible: (f64, f64),
    clamped: &dyn Fn(C) -> f64,
    interval: bool,
    base_ms: f64,
) -> SeedMap {
    use crate::interval::{Interval, Range};
    const UNIT: i64 = 64; // 最初の格子の 1 マス = 64 単位（1/32 マスの箱でも、中点が整数になる）
    // 深さ（最初の格子の 1 マスの 1/32 まで）
    const MAX_DEPTH: u32 = 8;
    const MAX_INTERVALS: usize = 30_000;
    let m = n + 1;
    let (u0, u1, v0, v1) = (dom.lo[ua], dom.hi[ua], dom.lo[va], dom.hi[va]);
    let mut out = SeedMap { n, u: (u0, u1), v: (v0, v1), cells: Default::default(), mids: Vec::new() };
    let total = n as i64 * UNIT;
    let pu = |iu: i64| u0 + (u1 - u0) * iu as f64 / total as f64;
    let pv = |iv: i64| v0 + (v1 - v0) * iv as f64 / total as f64;
    let du = (u1 - u0) / n as f64;
    let tol = 0.004 * h;
    let (lo, hi) = visible;
    // 最初の格子の双 1 次補間
    let bilinear = |iu: i64, iv: i64| -> f64 {
        let (i, j) = ((iu / UNIT).min(n as i64 - 1) as usize, (iv / UNIT).min(n as i64 - 1) as usize);
        let (fu, fv) = ((iu - i as i64 * UNIT) as f64 / UNIT as f64, (iv - j as i64 * UNIT) as f64 / UNIT as f64);
        let g = |a: usize, b: usize| hs[b * m + a];
        let (a, b, c, d) = (g(i, j), g(i + 1, j), g(i, j + 1), g(i + 1, j + 1));
        a * (1.0 - fu) * (1.0 - fv) + b * fu * (1.0 - fv) + c * (1.0 - fu) * fv + d * fu * fv
    };
    // 計算した点の値を調べて、種にする
    let consider = |iu: i64, iv: i64, val: f64, size: f64, out: &mut SeedMap| {
        if !val.is_finite() {
            return;
        }
        let base = bilinear(iu, iv);
        if !base.is_finite() {
            return;
        }
        let dev = (val - base).abs();
        // 見える高さにかかる（補間の値から計算した値までの区間が、見える範囲と重なる）
        if dev > tol && val.max(base) >= lo && val.min(base) <= hi {
            out.insert(Seed { u: pu(iu), v: pv(iv), size, excess: dev });
        }
    };
    let eval_points = |pts: &[(i64, i64)]| -> Vec<f64> {
        let xs: Vec<f64> = pts.iter().map(|p| pu(p.0)).collect();
        let ys: Vec<f64> = pts.iter().map(|p| pv(p.1)).collect();
        let mut v = vec![C::new(0.0, 0.0); pts.len()];
        prog.eval_batch(&[&xs, &ys], &mut v);
        v.into_iter().map(clamped).collect()
    };

    if !interval {
        // 区間演算が使えない式: 軽い式だけ、マスの中心と辺の中点を調べる
        if base_ms >= 3.0 {
            return out;
        }
        let half = UNIT / 2;
        let mut pts: Vec<(i64, i64)> = Vec::with_capacity(3 * m * m);
        for j in 0..=n as i64 {
            for i in 0..=n as i64 {
                let (x, y) = (i * UNIT, j * UNIT);
                if i < n as i64 && j < n as i64 {
                    pts.push((x + half, y + half));
                }
                if i < n as i64 {
                    pts.push((x + half, y));
                }
                if j < n as i64 {
                    pts.push((x, y + half));
                }
            }
        }
        let vals = eval_points(&pts);
        for (&(iu, iv), &val) in pts.iter().zip(&vals) {
            consider(iu, iv, val, 0.5 * du, &mut out);
            // 中点を持つ辺（マスの中心は対角線 (i, j)–(i+1, j+1) の中点）
            let (i, j) = ((iu / UNIT) as usize, (iv / UNIT) as usize);
            let k = j * m + i;
            let e = match (iu % UNIT != 0, iv % UNIT != 0) {
                (true, true) => (k, k + m + 1),
                (true, false) => (k, k + 1),
                _ => (k, k + m),
            };
            out.mids.push((e, val));
        }
        return out;
    }

    // 区間演算が使える式
    // 点の値（単位の座標 → 値）。最初は最初の格子の点
    let mut known: crate::adaptive::HashMap<(i64, i64), f64> = Default::default();
    let grid = |iu: i64, iv: i64| -> Option<f64> {
        (iu % UNIT == 0 && iv % UNIT == 0).then(|| hs[(iv / UNIT) as usize * m + (iu / UNIT) as usize])
    };
    let value = |known: &crate::adaptive::HashMap<(i64, i64), f64>, p: (i64, i64)| grid(p.0, p.1).or_else(|| known.get(&p).copied());
    let mut intervals = 0usize;
    // 箱 [x0, x1] × [y0, y1]（単位）のはみ出し（疑わしくなければ None）。`vals` は箱の角と中の点の値
    let excess = |x0: i64, x1: i64, y0: i64, y1: i64, vals: &[f64], intervals: &mut usize| -> Option<f64> {
        let fin: Vec<f64> = vals.iter().copied().filter(|v| v.is_finite()).collect();
        if fin.is_empty() {
            return None;
        }
        let (mn, mx) = fin.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
        *intervals += 1;
        let r = prog.eval_interval(&[Interval::new(pu(x0), pu(x1)), Interval::new(pv(y0), pv(y1))]);
        let Range::Known(iv) = r else { return Some(h) };
        let (a, b) = (iv.lo.clamp(lo, hi), iv.hi.clamp(lo, hi));
        let tau = 0.02 * h + TAU_SPREAD * (mx - mn);
        let e = (b - (mx + tau)).max((mn - tau) - a);
        (e > 0.0).then_some(e)
    };
    // 最初のブロック
    let mut frontier: Vec<(f64, [i64; 4], u32)> = Vec::new();
    // 最初の箱の 1 辺のマス数（ブロックの数が 8,000 個ほどまでになるように。細かさ 150 なら 2 マス）。
    // 大きいブロックだと、ブロックの別の所で格子の点が山の頂上の近くに乗ると、ブロック全体の値の範囲が広くなり、
    // 細い尾根の見落としを疑わなくなる
    let block_cells = ((n as f64 / 89.0).ceil() as i64).clamp(2, 8);
    let nb = (n as i64 + block_cells - 1) / block_cells;
    for bj in 0..nb {
        for bi in 0..nb {
            let (i0, i1) = (bi * block_cells, ((bi + 1) * block_cells).min(n as i64));
            let (j0, j1) = (bj * block_cells, ((bj + 1) * block_cells).min(n as i64));
            let mut vals = Vec::with_capacity(81);
            for j in j0..=j1 {
                for i in i0..=i1 {
                    vals.push(hs[j as usize * m + i as usize]);
                }
            }
            // 全部の点が見える範囲の外（同じ側）なら調べない
            if vals.iter().all(|&v| v.is_finite() && v > hi) || vals.iter().all(|&v| v.is_finite() && v < lo) {
                continue;
            }
            let b = [i0 * UNIT, i1 * UNIT, j0 * UNIT, j1 * UNIT];
            if let Some(e) = excess(b[0], b[1], b[2], b[3], &vals, &mut intervals) {
                frontier.push((e, b, 0));
            }
        }
    }
    // 疑わしい箱を、はみ出しの大きい順に四つに分けていく
    while !frontier.is_empty() && intervals < MAX_INTERVALS {
        frontier.sort_by(|a, b| b.0.total_cmp(&a.0));
        // 子の区間演算の回数（4 回ずつ）が上限を超えない分だけ
        let room = (MAX_INTERVALS - intervals) / 4;
        frontier.truncate(room.max(1));
        let boxes: Vec<([i64; 4], u32)> = frontier.drain(..).map(|(_, b, d)| (b, d)).collect();
        // 新しい点（中心と辺の中点）をまとめて計算する
        let mut pts: Vec<(i64, i64)> = Vec::new();
        for &([x0, x1, y0, y1], _) in &boxes {
            let (xm, ym) = ((x0 + x1) / 2, (y0 + y1) / 2);
            for p in [(xm, ym), (xm, y0), (xm, y1), (x0, ym), (x1, ym)] {
                if value(&known, p).is_none() {
                    pts.push(p);
                }
            }
        }
        pts.sort_unstable();
        pts.dedup();
        let vals = eval_points(&pts);
        for (&p, &val) in pts.iter().zip(&vals) {
            known.insert(p, val);
        }
        for &([x0, x1, y0, y1], d) in &boxes {
            let (xm, ym) = ((x0 + x1) / 2, (y0 + y1) / 2);
            let size = (u1 - u0) * ((xm - x0) as f64 / total as f64);
            for p in [(xm, ym), (xm, y0), (xm, y1), (x0, ym), (x1, ym)] {
                if let Some(val) = value(&known, p) {
                    if grid(p.0, p.1).is_none() {
                        consider(p.0, p.1, val, size, &mut out);
                    }
                }
            }
            if d + 1 >= MAX_DEPTH || x1 - x0 < 4 || y1 - y0 < 4 {
                continue;
            }
            for (a0, a1) in [(x0, xm), (xm, x1)] {
                for (b0, b1) in [(y0, ym), (ym, y1)] {
                    let corners: Vec<f64> = [(a0, b0), (a1, b0), (a0, b1), (a1, b1)]
                        .iter()
                        .filter_map(|&p| value(&known, p))
                        .collect();
                    if intervals >= MAX_INTERVALS {
                        break;
                    }
                    if let Some(e) = excess(a0, a1, b0, b1, &corners, &mut intervals) {
                        frontier.push((e, [a0, a1, b0, b1], d + 1));
                    }
                }
            }
        }
    }
    out
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
) -> crate::adaptive::HashMap<usize, Jump> {
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
) -> crate::adaptive::HashMap<usize, Jump> {
    const ROUNDS: usize = 12;
    // 差が threshold の POLE_JUMP 倍（細かくするときは高さの範囲の 0.3 倍）より大きい段差は、さらに POLE_ROUNDS 回二分する
    const POLE_JUMP: f64 = 60.0;
    const POLE_ROUNDS: usize = 18;
    // この回数だけ二分した後、差が最初の差の EARLY_KEEP 倍より小さい候補は外す
    const EARLY_ROUND: usize = 3;
    const EARLY_KEEP: f64 = 0.3;
    // 1 回目と 2 回目の二分の後、どちらも中点と両端の平均とのずれが、両端の差のこの倍より小さい候補は外す
    const MID_KEEP: f64 = 0.2;
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
        /// 1 回目の二分の中点が、両端の平均に近かった
        smooth: bool,
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
                smooth: false,
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
        let mut keep = vec![true; cands.len()];
        for ((c, &z), keep) in cands.iter_mut().zip(&out).zip(keep.iter_mut()) {
            let t = 0.5 * (c.ta + c.tb);
            let (ha, hb, hm) = (height(c.fa), height(c.fb), height(z));
            // 1 回目と 2 回目の二分の直後: どちらでも中点が両端の平均に近い辺は、なめらかな斜面なので外す。
            // 辺の中で値が飛んでいれば、飛ぶ所を含む区間の中点はどちらかの端の値に近く、平均とのずれは差の約半分になる
            // （なめらかな斜面なら、ずっと小さい）。1 回目だけで決めないのは、段差が 2 つ入った辺（段差の線の交わる所を
            // 斜めに通る辺）では、1 回目の中点がちょうど真ん中の段に乗って、平均と同じになることがあるから
            if early_exit && round < 2 {
                let smooth = hm.is_finite() && (hm - 0.5 * (ha + hb)).abs() < MID_KEEP * (hb - ha).abs();
                if round == 0 {
                    c.smooth = smooth;
                } else if smooth && c.smooth {
                    *keep = false;
                    continue;
                }
            }
            // 差の大きいほうの半分に狭める（真ん中が定義されなければ、始点の側に寄せる）
            if !hm.is_finite() || (hm - ha).abs() >= (hb - hm).abs() {
                c.tb = t;
                c.fb = z;
            } else {
                c.ta = t;
                c.fa = z;
            }
        }
        if keep.contains(&false) {
            let mut it = keep.iter();
            cands.retain(|_| *it.next().unwrap());
            mu.truncate(cands.len());
            mv.truncate(cands.len());
            out.truncate(cands.len());
        }
    }
    // 差の大きい段差（極かもしれない）は、さらに二分して、極のすぐそばまで狭める。細い極（nPr(x, y) の x = −8 など）は、
    // 12 回の二分（辺の 1/4096）では極の手前で止まり、そこで切ると壁が上下の端まで届かない
    {
        let big: Vec<usize> = (0..cands.len())
            .filter(|&k| {
                let d = (height(cands[k].fb) - height(cands[k].fa)).abs();
                d.is_finite() && d > POLE_JUMP * threshold
            })
            .collect();
        if !big.is_empty() {
            let mut mu = vec![0.0; big.len()];
            let mut mv = vec![0.0; big.len()];
            let mut out = vec![C::new(0.0, 0.0); big.len()];
            // 範囲の端に張り付いた値（1,000 倍しても、範囲にそろえた高さが変わらない）
            let stuck = |z: C| {
                let h0 = height(z);
                h0 != 0.0 && h0 == height(z * 1000.0)
            };
            let mut big = big;
            for _ in 0..POLE_ROUNDS {
                // 両側とも端に張り付いた段差は、それ以上狭めても変わらないので外す
                big.retain(|&k| !(stuck(cands[k].fa) && stuck(cands[k].fb)));
                if big.is_empty() {
                    break;
                }
                mu.truncate(big.len());
                mv.truncate(big.len());
                out.truncate(big.len());
                for (q, &k) in big.iter().enumerate() {
                    let c = &cands[k];
                    let t = 0.5 * (c.ta + c.tb);
                    mu[q] = us[c.k0] + (us[c.k1] - us[c.k0]) * t;
                    mv[q] = vs[c.k0] + (vs[c.k1] - vs[c.k0]) * t;
                }
                prog.eval_batch(&[&mu, &mv], &mut out);
                for (q, &k) in big.iter().enumerate() {
                    let c = &mut cands[k];
                    let t = 0.5 * (c.ta + c.tb);
                    let (ha, hb, hm) = (height(c.fa), height(c.fb), height(out[q]));
                    if !hm.is_finite() || (hm - ha).abs() >= (hb - hm).abs() {
                        c.tb = t;
                        c.fb = out[q];
                    } else {
                        c.ta = t;
                        c.fa = out[q];
                    }
                }
            }
        }
    }
    let mut jumps = crate::adaptive::HashMap::default();
    // 見つけた段差の両側の残り（始点〜段差、段差〜終点）のうち、差の大きいほうに、もう 1 つ段差がありそうか。
    // 残りの差が threshold より大きく、その中点が両端の平均から離れていれば、ありそうとみなす
    // （段差の線の交わる所を辺が斜めに通ると、1 本の辺に段差が 2 つ入る。見つかるのはそのうちの 1 つだけ）
    let mut rest: Vec<(usize, f64, f64, f64, f64)> = Vec::new(); // (jumps の鍵, t0, t1, h0, h1)。t0 = 0 なら始点の側
    for c in cands {
        let d = (height(c.fb) - height(c.fa)).abs();
        // 区間を 1/4096 にしても差が 0.3 倍より多く残っていれば、値が飛んでいる（なめらかなら 1/4096 ほどになる）。
        // 半分より小さくてもよいのは、1 本の辺に段差が 2 つ入ると、残るのはそのうちの 1 つ（差の約半分）だから。
        // どちらかの側が定義されない所は段差とはみなさない
        if d.is_finite() && d > EARLY_KEEP * c.first {
            let (h0, h1) = (hs[c.k0], hs[c.k1]);
            let (ha, hb) = (height(c.fa), height(c.fb));
            let side = if (ha - h0).abs() >= (h1 - hb).abs() { (0.0, c.ta, h0, ha) } else { (c.tb, 1.0, hb, h1) };
            if (side.3 - side.2).abs() > threshold {
                rest.push((c.id, side.0, side.1, side.2, side.3));
            }
            jumps.insert(c.id, Jump { t: 0.5 * (c.ta + c.tb), before: c.fa, after: c.fb, multi: false, rest_at_start: false });
        }
    }
    if !rest.is_empty() {
        let (mut mu, mut mv) = (Vec::with_capacity(rest.len()), Vec::with_capacity(rest.len()));
        for &(id, t0, t1, ..) in &rest {
            let (k0, k1) = edges[id];
            let t = 0.5 * (t0 + t1);
            mu.push(us[k0] + (us[k1] - us[k0]) * t);
            mv.push(vs[k0] + (vs[k1] - vs[k0]) * t);
        }
        let mut out = vec![C::new(0.0, 0.0); rest.len()];
        prog.eval_batch(&[&mu, &mv], &mut out);
        for (&(id, t0, _, h0, h1), &z) in rest.iter().zip(&out) {
            let hm = height(z);
            if !hm.is_finite() || (hm - 0.5 * (h0 + h1)).abs() >= MID_KEEP * (h1 - h0).abs() {
                if let Some(j) = jumps.get_mut(&id) {
                    j.multi = true;
                    j.rest_at_start = t0 == 0.0;
                }
            }
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

/// セルの角のうち、少ないほうの符号（負なら true）の角を、つながっているものどうしの組に分ける（組ごとに曲面が 1 枚）。
/// `v` は角の F の値（負かどうかで符号を決める）。戻り値は (角ごとの組の番号（少ないほうの符号でない角は usize::MAX）,
/// 組の数, 少ないほうの符号)。
///
/// 角は、セルの辺で隣り合うときと、あいまいな面（対角の 2 つが同じ符号で、残りの 2 つが逆の符号の面）の対角で
/// 「その面の中でつながっている」ときにつながる。
///
/// 組に分けるのは少ないほうの符号の角だけなので、あいまいな面では、そのセルが「少ないほうの符号の対角がつながる」か
/// 「多いほうの符号の対角がつながる（少ないほうは切れる）」かを決めることになる。面を共有する隣のセルと判断が食い違うと、
/// その面の上に四角形が張られず穴が開く（前は辺のつながりだけで決めていたので、少ないほうの符号が面の両側で逆のとき、
/// 両側とも「相手の符号がつながる」とみなしていた。面が格子の平面にほぼ重なるジャイロイドなどで出る）。そこで:
///
/// - 隣のセルの少ないほうの符号がこのセルと逆なら、面の上で F を双一次に補間したときの鞍点の値
///   $s = \frac{ab - cd}{(a + b) - (c + d)}$（a, b は対角の 2 つ、c, d は残りの 2 つ）の符号の対角がつながるとする（漸近判定）。
///   隣のセルも同じ 4 つの値で同じ $s$ を計算するので、判断がそろう
/// - 隣のセルの少ないほうの符号がこのセルと同じなら、前と同じく切れるとする（両側とも切るので、そろう。
///   両側ともつなぐと、セルより細い管が 1 本の辺につぶれて、3 つ以上の三角形が集まる辺になる）
///
/// `far(f, side)` は、軸 f の向きの面（side = 0: 手前、1: 奥）の向こうのセルの、その面と反対側の面の 4 つの角の値
/// （格子の外なら None。そのときは鞍点の値だけで決める）
fn corner_groups(v: &[f32; 8], far: &dyn Fn(usize, usize) -> Option<[f32; 4]>) -> ([usize; 8], usize, bool) {
    let neg: [bool; 8] = v.map(|x| x < 0.0);
    let count = neg.iter().filter(|&&b| b).count();
    let minority = count <= 4;
    let linked = |x: usize, y: usize| -> bool {
        let diff = (0..3).filter(|&a| CORNERS[x][a] != CORNERS[y][a]).count();
        match diff {
            1 => true,
            2 => {
                // 面の対角。面の残りの 2 つの角が同じ符号なら（あいまいな面）、鞍点の値で決める
                let f = (0..3).find(|&a| CORNERS[x][a] == CORNERS[y][a]).unwrap_or(0);
                let mut w = [0usize; 2];
                let mut k = 0;
                for c in 0..8 {
                    if c != x && c != y && CORNERS[c][f] == CORNERS[x][f] {
                        w[k] = c;
                        k += 1;
                    }
                }
                if neg[w[0]] == neg[x] || neg[w[1]] == neg[x] {
                    return false; // あいまいでない面（辺でつながるなら、辺をたどってつながる）
                }
                // 隣のセルの少ないほうの符号（面の 4 つの角と、向こうの 4 つの角から）
                if let Some(other) = far(f, CORNERS[x][f]) {
                    let count = 2 + other.iter().filter(|&&q| q < 0.0).count();
                    if (count <= 4) == neg[x] {
                        return false;
                    }
                }
                let (a, b, c, d) = (v[x] as f64, v[y] as f64, v[w[0]] as f64, v[w[1]] as f64);
                let s = (a * b - c * d) / ((a + b) - (c + d));
                (s < 0.0) == neg[x]
            }
            _ => false,
        }
    };
    let mut group = [usize::MAX; 8];
    let mut groups = 0;
    for c in 0..8 {
        if neg[c] != minority || group[c] != usize::MAX {
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
            for y in 0..8 {
                if y != x && neg[y] == minority && group[y] == usize::MAX && linked(x, y) {
                    group[y] = groups;
                    stack[top] = y;
                    top += 1;
                }
            }
        }
        groups += 1;
    }
    (group, groups, minority)
}

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
    implicit_solid_opt(prog, dom, n, ranges, samples, Refine::Normal)
}

/// [`implicit_solid`] で、格子より小さい図形を探して細かく作り直すか（`refine`）を選ぶ版
pub fn implicit_solid_opt(prog: &Program, dom: Domain, n: usize, ranges: &[(f64, f64)], samples: usize, refine: Refine) -> Mesh {
    // 式を変形して部品に分けられるなら、部品ごとに描く（因数・段差の枝。pieces.rs）
    if ranges.is_empty() && refine != Refine::Off {
        if let Some(mesh) = pieces::implicit_by_pieces(prog, dom, n, refine) {
            return mesh;
        }
    }
    implicit_solid_with(prog, dom, n, ranges, samples, false, refine)
}

/// 陰関数で、格子より小さい図形（小さい球・細い輪など）を探して、その所だけ細かく作り直すか（[`refine_components`]）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refine {
    /// 作り直さない（テスト用。細分を足す前と同じ結果）
    Off,
    /// 軽く作り直す（再生・ドラッグ中）: 小さい図形だけを、図形の大きさに合わせた細かさ（[`ZOOM_TARGET`]）で作り直す。
    /// 大きい曲面の角ばった所（(c)）は見ない
    Light,
    /// ふつう（探す手間と細かさに上限がある）
    Normal,
    /// 念入りに探す（区間演算の回数の上限なし、8 倍の細かさまで。遅くなる）
    Thorough,
}

/// [`implicit_solid`] の本体。`all_blocks` なら区間演算を使わず、全部のブロックを「ある」にする（テストで比べるため）
fn implicit_solid_with(
    prog: &Program,
    dom: Domain,
    n: usize,
    ranges: &[(f64, f64)],
    samples: usize,
    all_blocks: bool,
    refine: Refine,
) -> Mesh {
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
        let mask = pieces::BLOCK_MASK.with(|m| m.borrow_mut().take());
        let mut plan = SparseField::plan(prog, ranges, dom, n, all_blocks, mask.as_deref());
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
        // 値が有限でない（NaN・±∞ の）格子点（格子点の番号）。1'''（定義域の縁）で使う
        let nan_pts: Vec<usize> = if plan.field.dense {
            plan.field.vals.iter().enumerate().filter(|(_, v)| !v.is_finite()).map(|(l, _)| l).collect()
        } else {
            plan.spots.iter().filter(|&&(at, _)| !plan.field.vals[at as usize].is_finite()).map(|&(_, p)| p as usize).collect()
        };
        (plan.field, nan_pts)
    };
    let (mut field, nan_pts) = field;
    let idx = |i: usize, j: usize, k: usize| i + m * (j + m * k);
    // 1'''. 定義されない格子点のうち、軸の向きの隣に定義される点があるものを、縁での値で埋める（[`extend_undefined`]）。
    //       こうすると、定義域の縁をまたぐセルにも頂点が置かれ、面が縁の少し外まで延びる（外にはみ出た所は 4' で切り落とす）
    let ext = if nan_pts.is_empty() { Extended::default() } else { extend_undefined(prog, &combos, &mut field, dom, n, &nan_pts) };
    let field = field;
    let filled = |p: usize| !ext.filled.is_empty() && ext.filled.contains(&p);
    let cell = |i: usize, j: usize, k: usize| i + n * (j + n * k);
    // 1''''. 段差（floor・mod など）をまたぐセルは、枝ごとのなめらかな続きで別に作る（[`branch_meshes`]）。
    //        ふつうの Surface Nets では、そのセルに頂点を置かない
    let branches = if refine != Refine::Off && ranges.is_empty() && (1..=MAX_BRANCH_SITES).contains(&prog.branch_sites()) {
        branch_meshes(prog, &field, dom, n)
    } else {
        BranchMeshes::default()
    };
    let in_branch_cell = |i: usize, j: usize, k: usize| !branches.mixed.is_empty() && branches.mixed.contains(&cell(i, j, k));

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
                                // 片方の端が埋めた点の辺は、間に定義されない所があるので調べない（交わる位置は 1''' で決めた）
                                if filled(idx(i, j, k)) || filled(idx(q1[0], q1[1], q1[2])) {
                                    continue;
                                }
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
    let not_zero: crate::adaptive::HashSet<usize> = {
        let mut sc = FieldScratch::default();
        let flags = false_crossings(&mut |xyz| field_values_with(prog, &combos, xyz, &mut sc).to_vec(), &sign_edges);
        sign_ids.into_iter().zip(flags).filter(|&(_, bad)| bad).map(|(id, _)| id).collect()
    };
    let touch = if ranges.is_empty() { touch_search(prog, touch_cands, dom, n) } else { Default::default() };
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
    // 1'''''. 極に張り付く面: 極の近くのセルだけ、辺を細かく調べて零点を数え直し、別に作る（[`pole_meshes`]）。
    //         ふつうの Surface Nets では、そのセルに頂点を置かない
    // 段差の元（floor・mod など）を含む式は行わない（段差の飛びは極ではなく、細かさ 200 では零点でない辺が何百万本にもなる）
    let poles = if refine != Refine::Off
        && ranges.is_empty()
        && prog.branch_sites() == 0
        && (!not_zero.is_empty() || !nan_pts.is_empty())
        && not_zero.len() + nan_pts.len() <= MAX_POLE_SEEDS
    {
        pole_meshes(prog, &combos, &field, dom, n, &not_zero, &nan_pts, &ext.filled, &branches.mixed)
    } else {
        BranchMeshes::default()
    };
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
            if let Some(&t) = (!ext.edge_t.is_empty()).then(|| ext.edge_t.get(&(3 * idx(p[0], p[1], p[2]) + a))).flatten() {
                return Some((t as f64, Some(f0 < 0.0))); // 定義される部分にある本当の零点
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
    // 頂点ごとの、頂点を置いたセルの番号（4 の細分で使う）
    let mut vertex_cell: Vec<u32> = Vec::new();
    // 1 つのセルに頂点を 2 つ以上置いた（近い曲面を分けた）か
    let mut multi_surface = false;
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
                let corner_val = |c: usize| buf[l + corner_off[c]];
                // 段差をまたぐセルは枝ごとに、極の近くのセルは零点を数え直して、別に作る
                if in_branch_cell(i, j, k) || (!poles.mixed.is_empty() && poles.mixed.contains(&cell(i, j, k))) {
                    continue;
                }
                // 角の符号がそろっていて、接するだけの所も無ければ、曲面は通らない（ほとんどのセルはここで終わる）
                let near_touch = marked(i, j, k);
                if !near_touch && (mask == 0 || mask == 0xFF) {
                    continue;
                }
                let corner_neg: [bool; 8] = std::array::from_fn(|c| mask >> c & 1 == 1);
                // 少ないほうの符号の角を、つながっているものどうしで組に分ける（[`corner_groups`]）
                let q = [i, j, k];
                let far = |f: usize, side: usize| -> Option<[f32; 4]> {
                    // 向こうのセルの反対側の面: 軸 f の座標が q[f] − 1（手前）か q[f] + 2（奥）
                    if (side == 0 && q[f] == 0) || (side == 1 && q[f] + 2 > n) {
                        return None;
                    }
                    let (g, h) = ((f + 1) % 3, (f + 2) % 3);
                    let base = if side == 0 { l - view.strides[f] } else { l + 2 * view.strides[f] };
                    Some([(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dg, dh)| buf[base + dg * view.strides[g] + dh * view.strides[h]]))
                };
                let (mut group, mut groups, minority) = corner_groups(&std::array::from_fn(corner_val), &far);
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
                if groups > 1 {
                    multi_surface = true;
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
                        vertex_cell.push(cell(i, j, k) as u32);
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

    // 3'. 段差をまたぐセルの面（枝ごとに作って、切り取ったもの）と、極の近くの面を足す。ふつうの Surface Nets の頂点と
    //     同じ位置の頂点は、その頂点を使う（継ぎ目で頂点を共有する）
    weld_append(&mut mesh, &mut vertex_cell, &branches);
    weld_append(&mut mesh, &mut vertex_cell, &poles);
    // 4'. 定義されない所に延びた面を、定義域の縁で切り落とす（[`trim_undefined`]）
    if !ext.filled.is_empty() {
        trim_undefined(prog, &combos, &mut mesh, &mut vertex_cell, n, &ext.filled);
    }
    // 4''. 極・値が飛ぶ所・定義域の縁にできる小さな破片と、値が飛ぶ所で「接する」と見誤った細かい切れ端を消す（[`drop_fragments`]）
    let mut dropped = 0;
    if !not_zero.is_empty() || !nan_pts.is_empty() || !touch.is_empty() {
        dropped = drop_fragments(&mut mesh, dom, n, &field, &not_zero, &ext.filled, true);
    }
    if dropped > 1000 {
        mesh.note.push_str("（細かすぎて描けない部分あり）");
    }
    // 4'''. 三角形が多すぎる（面が詰まりすぎて描ききれない）ときは、F の格子を間引いた版を本体にする（画面が固まらないように）
    let mut coarse = None;
    if mesh.indices.len() / 3 > MAX_IMPLICIT_TRIANGLES {
        let mut sc = FieldScratch::default();
        let mut eval = |xyz: [&[f64]; 3]| field_values_with(prog, &combos, xyz, &mut sc).to_vec();
        for k in [2, 4] {
            let c = coarse_nets(&field, dom, n, k, &not_zero, &mut eval);
            let ok = c.indices.len() / 3 <= MAX_IMPLICIT_TRIANGLES;
            coarse = Some((k, c));
            if ok {
                break;
            }
        }
    }
    if let Some((k, c)) = coarse {
        let note = std::mem::take(&mut mesh.note);
        mesh = c;
        mesh.note = note + "（粗くして表示）";
        compute_normals(&mut mesh);
        if mesh.indices.len() / 3 > LOD_MIN_TRIANGLES {
            let mut sc = FieldScratch::default();
            let mut eval = |xyz: [&[f64]; 3]| field_values_with(prog, &combos, xyz, &mut sc).to_vec();
            let lod = coarse_nets(&field, dom, n, 2 * k, &not_zero, &mut eval);
            if !lod.indices.is_empty() {
                mesh.lod_mesh = Some(Box::new(lod));
            }
        }
        let t_end = crate::timing::now_ms();
        mesh.stats.phases = [t_field - t_start, t_touch - t_field, t_cells - t_touch, t_end - t_cells];
        return mesh;
    }

    compute_normals(&mut mesh);
    // 3''（法線を計算した後に行う）. 大きい曲面につながった細い部分（円錐の先、細い首）を、そこだけ 2 倍の細かさで作り直す（[`stitched_nets`]）。
    //      極・値が飛ぶ所・定義域の縁・接する所・近い曲面・段差をまたぐセルが無い式だけ（作り直すと、それらの処理が無くなるため）
    if matches!(refine, Refine::Normal | Refine::Thorough)
        && ranges.is_empty()
        && not_zero.is_empty()
        && touch.is_empty()
        && nan_pts.is_empty()
        && branches.mixed.is_empty()
        && !multi_surface
        && !mesh.indices.is_empty()
    {
        let sel = sharp_blocks(&mesh, &vertex_cell, n, field.nb);
        if !sel.is_empty() && sel.len() <= MAX_STITCH_BLOCKS {
            let mut sc = FieldScratch::default();
            let mut eval = |xyz: [&[f64]; 3]| field_values_with(prog, &combos, xyz, &mut sc).to_vec();
            let (fine, cells) = stitched_nets(&field, dom, n, &sel, &mut eval);
            if fine.indices.len() / 3 <= mesh.indices.len() / 3 + MAX_STITCH_TRIANGLES {
                mesh.positions = fine.positions;
                mesh.indices = fine.indices;
                vertex_cell = cells;
                compute_normals(&mut mesh);
            }
        }
    }
    // 4. 格子より小さい図形（見落とした・角ばった所）を探して、その所だけ細かく作り直す
    if refine != Refine::Off && ranges.is_empty() {
        let before = (mesh.positions.len(), mesh.indices.len());
        refine_components(prog, &combos, dom, n, &field, &mut mesh, &vertex_cell, refine);
        // 作り直しが、値の飛ぶ所・極の近くに作った細かい切れ端も消す（作り直しでメッシュが変わったときだけ）
        if (mesh.positions.len(), mesh.indices.len()) != before {
            let more = drop_fragments(&mut mesh, dom, n, &field, &not_zero, &ext.filled, true);
            if more > 0 {
                compute_normals(&mut mesh);
            }
        }
    }
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

/// 2 段の細かさの葉のセル（[`stitched_nets`]）
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Leaf {
    /// 元の格子のセル
    Coarse([usize; 3]),
    /// 細かくしたブロックの中の、2 倍の細かさのセル（細かい格子の座標）
    Fine([usize; 3]),
}

/// 陰関数を、選んだブロック（`sel`。1 辺 [`BLOCK`] マス）だけ 2 倍の細かさの格子で作る。継ぎ目にひびを作らないよう、
/// 面は「符号の変わる、一番細かい辺」ごとに張る: その辺を囲む 4 か所を、葉のセル（細かいセルか、元のセル）に写し、
/// 同じセルが続けて出たら 1 つにまとめて、3 つなら三角形、4 つなら四角形でつなぐ。葉のセルの頂点は、そのセルが
/// 囲む一番細かい辺の、F = 0 になる点の平均（元のセルは、自分の面の上にある細かい辺の交点も使う）。
/// `field` は元の格子（`n` マス、範囲 `dom`）の F。細かい格子の、元の格子に無い点の F は `eval` で計算する
fn stitched_nets(
    field: &SparseField,
    dom: Domain,
    n: usize,
    sel: &crate::adaptive::HashSet<usize>,
    eval: &mut dyn FnMut([&[f64]; 3]) -> Vec<f64>,
) -> (Mesh, Vec<u32>) {
    let nb = field.nb;
    let nf = 2 * n; // 細かい格子のマス数
    let bid = |b: [usize; 3]| b[0] + nb * (b[1] + nb * b[2]);
    let block_of = |c: [usize; 3]| c.map(|x| (x / BLOCK).min(nb - 1));
    let mut sel_vec = vec![false; nb * nb * nb];
    for &x in sel {
        sel_vec[x] = true;
    }
    let selected = |c: [usize; 3]| sel_vec[bid(block_of(c))];
    let occupied = |c: [usize; 3]| field.occupied[bid(block_of(c))];
    // 細かくしたブロックごとの、細かい格子の点の F（1 辺 2 BLOCK + 1 点。元の格子の点は field、ほかは計算する）
    const S: usize = 2 * BLOCK + 1;
    let mut slot = vec![usize::MAX; nb * nb * nb];
    let mut sel_list: Vec<usize> = sel.iter().copied().collect();
    sel_list.sort_unstable();
    let mut fvals: Vec<f32> = vec![f32::NAN; sel_list.len() * S * S * S];
    {
        let mut need: Vec<(usize, [usize; 3])> = Vec::new();
        for (k, &b) in sel_list.iter().enumerate() {
            slot[b] = k;
            let b3 = [b % nb, (b / nb) % nb, b / (nb * nb)];
            let lo = b3.map(|x| 2 * BLOCK * x);
            for z in 0..S {
                for y in 0..S {
                    for x in 0..S {
                        let q = [lo[0] + x, lo[1] + y, lo[2] + z];
                        if q.iter().any(|&v| v > nf) {
                            continue;
                        }
                        let at = k * S * S * S + x + S * (y + S * z);
                        if q.iter().all(|v| v % 2 == 0) {
                            fvals[at] = field.f(q.map(|v| v / 2));
                        } else {
                            need.push((at, q));
                        }
                    }
                }
            }
        }
        let cols: [Vec<f64>; 3] = [0, 1, 2].map(|a| need.iter().map(|(_, q)| dom.at(a, q[a] as f64 / 2.0, n)).collect());
        let vals = eval([&cols[0], &cols[1], &cols[2]]);
        for ((at, _), v) in need.into_iter().zip(vals) {
            fvals[at] = v as f32;
        }
    }
    // 細かい格子の点 q の F（元の格子の点なら field。ほかは、q を含む細かくしたブロックの配列）
    let value = |q: [usize; 3]| -> f32 {
        if q.iter().all(|v| v % 2 == 0) {
            return field.f(q.map(|v| v / 2));
        }
        // q を含むブロック（ブロックの境目の点は、どちらのブロックにもある）
        let cand = |v: usize| {
            let b = v / (2 * BLOCK);
            if v % (2 * BLOCK) == 0 && b > 0 { [b.min(nb - 1), b - 1] } else { [b.min(nb - 1), b.min(nb - 1)] }
        };
        for bz in cand(q[2]) {
            for by in cand(q[1]) {
                for bx in cand(q[0]) {
                    let k = slot[bid([bx, by, bz])];
                    if k == usize::MAX {
                        continue;
                    }
                    let l = [q[0] - 2 * BLOCK * bx, q[1] - 2 * BLOCK * by, q[2] - 2 * BLOCK * bz];
                    if l.iter().all(|&v| v < S) {
                        return fvals[k * S * S * S + l[0] + S * (l[1] + S * l[2])];
                    }
                }
            }
        }
        f32::NAN
    };
    // 細かい格子の座標でのセル（細かいセル 1 つ分の場所）を、葉のセルに写す
    let leaf = |fc: [i64; 3]| -> Option<Leaf> {
        if fc.iter().any(|&v| v < 0 || v >= nf as i64) {
            return None;
        }
        let fc = fc.map(|v| v as usize);
        let c = fc.map(|v| v / 2);
        if !occupied(c) {
            return None;
        }
        Some(if selected(c) { Leaf::Fine(fc) } else { Leaf::Coarse(c) })
    };
    // 一番細かい辺（細かい格子の座標で、始点 p、向き a、長さ len = 1 か 2）の一覧
    let mut edges: Vec<([usize; 3], usize, usize, f32, f32)> = Vec::new();
    // (i) 細かくしたブロックの、細かい格子の辺（周りに細かいセルが 1 つでもあるもの）
    {
        let mut seen: crate::adaptive::HashSet<([usize; 3], usize)> = Default::default();
        for &b in sel {
            let b3 = [b % nb, (b / nb) % nb, b / (nb * nb)];
            let lo = b3.map(|x| 2 * BLOCK * x);
            let hi = b3.map(|x| (2 * BLOCK * (x + 1)).min(nf));
            for z in lo[2]..=hi[2] {
                for y in lo[1]..=hi[1] {
                    for x in lo[0]..=hi[0] {
                        for a in 0..3 {
                            let p = [x, y, z];
                            if p[a] + 1 > hi[a] {
                                continue;
                            }
                            // 符号の変わらない辺は、面を張らないので調べない
                            let mut q = p;
                            q[a] += 1;
                            let (f0, f1) = (value(p), value(q));
                            if !f0.is_finite() || !f1.is_finite() || (f0 < 0.0) == (f1 < 0.0) {
                                continue;
                            }
                            let (bx, cx) = ((a + 1) % 3, (a + 2) % 3);
                            let any_fine = [(0i64, 0i64), (1, 0), (0, 1), (1, 1)].iter().any(|&(db, dc)| {
                                let mut fc = p.map(|v| v as i64);
                                fc[bx] -= db;
                                fc[cx] -= dc;
                                matches!(leaf(fc), Some(Leaf::Fine(_)))
                            });
                            if any_fine && seen.insert((p, a)) {
                                edges.push((p, a, 1, f0, f1));
                            }
                        }
                    }
                }
            }
        }
    }
    // (ii) 元の格子の辺で、周りのセルが全部元のセルのもの
    for bz in 0..nb {
        for by in 0..nb {
            for bx_ in 0..nb {
                let b = [bx_, by, bz];
                if !field.occupied[bid(b)] {
                    continue;
                }
                let lo = b.map(|x| BLOCK * x);
                let hi = b.map(|x| (BLOCK * (x + 1)).min(n));
                for z in lo[2]..hi[2] {
                    for y in lo[1]..hi[1] {
                        for x in lo[0]..hi[0] {
                            let c = [x, y, z];
                            let f0 = field.f(c);
                            for a in 0..3 {
                                // 符号の変わらない辺は、面を張らないので調べない（ほとんどの辺はここで終わる）
                                let mut q = c;
                                q[a] += 1;
                                let f1 = field.f(q);
                                if !f0.is_finite() || !f1.is_finite() || (f0 < 0.0) == (f1 < 0.0) {
                                    continue;
                                }
                                let (bx, cx) = ((a + 1) % 3, (a + 2) % 3);
                                let all_coarse = [(0i64, 0i64), (1, 0), (0, 1), (1, 1)].iter().all(|&(db, dc)| {
                                    let mut fc = c.map(|v| 2 * v as i64);
                                    fc[bx] -= 2 * db;
                                    fc[cx] -= 2 * dc;
                                    !matches!(leaf(fc), Some(Leaf::Fine(_)))
                                });
                                if all_coarse && c[a] < n {
                                    edges.push((c.map(|v| 2 * v), a, 2, f0, f1));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // 符号の変わる辺ごとに、交わる点と、周りの葉のセル
    let mut sums: crate::adaptive::HashMap<Leaf, ([f64; 3], f64)> = Default::default();
    let mut polys: Vec<(Vec<Leaf>, bool)> = Vec::new();
    for &(p, a, len, f0, f1) in &edges {
        let t = (f0 / (f0 - f1)) as f64;
        let mut x = p.map(|v| v as f64);
        x[a] += t * len as f64;
        let (bx, cx) = ((a + 1) % 3, (a + 2) % 3);
        let mut ring: Vec<Leaf> = Vec::with_capacity(4);
        let mut ok = true;
        for (db, dc) in [(1i64, 1i64), (0, 1), (0, 0), (1, 0)] {
            let mut fc = p.map(|v| v as i64);
            fc[bx] -= db * len as i64;
            fc[cx] -= dc * len as i64;
            match leaf(fc) {
                Some(l) => {
                    if ring.last() != Some(&l) {
                        ring.push(l);
                    }
                }
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        if ring.len() > 1 && ring.first() == ring.last() {
            ring.pop();
        }
        for l in &ring {
            let e = sums.entry(*l).or_insert(([0.0; 3], 0.0));
            for (s, v) in e.0.iter_mut().zip(x) {
                *s += v;
            }
            e.1 += 1.0;
        }
        if ring.len() >= 3 {
            polys.push((ring, f0 < 0.0));
        }
    }
    let mut mesh = Mesh::default();
    let mut ids: crate::adaptive::HashMap<Leaf, u32> = Default::default();
    let mut order: Vec<Leaf> = sums.keys().copied().collect();
    order.sort_by_key(|l| match l {
        Leaf::Coarse(c) => (0, c[2], c[1], c[0]),
        Leaf::Fine(c) => (1, c[2], c[1], c[0]),
    });
    let mut cells: Vec<u32> = Vec::with_capacity(order.len());
    for l in order {
        let (sum, cnt) = sums[&l];
        let p = sum.map(|v| v / cnt / 2.0); // 細かい格子の座標 → 元の格子の座標
        ids.insert(l, mesh.push_vertex([dom.at(0, p[0], n), dom.at(1, p[1], n), dom.at(2, p[2], n)]));
        let c = match l {
            Leaf::Coarse(c) => c,
            Leaf::Fine(f) => f.map(|v| v / 2),
        };
        cells.push((c[0] + n * (c[1] + n * c[2])) as u32);
    }
    for (ring, neg) in polys {
        let v: Vec<u32> = ring.iter().map(|l| ids[l]).collect();
        let tri = |a: u32, b: u32, c: u32| if neg { [a, b, c] } else { [a, c, b] };
        if v.len() == 3 {
            mesh.indices.extend_from_slice(&tri(v[0], v[1], v[2]));
        } else {
            mesh.indices.extend_from_slice(&tri(v[0], v[1], v[2]));
            mesh.indices.extend_from_slice(&tri(v[0], v[2], v[3]));
        }
    }
    (mesh, cells)
}

/// 細かくするブロックの数と、細かくして足してよい三角形の数の上限（[`stitched_nets`]）。ブロックが多い式（立方体の辺のように、
/// 角が長く続く形）は作り直さない（時間がかかりすぎ、角の丸まりは元の格子でも目立たない。細い部分は、選ぶブロックが少ない）
const MAX_STITCH_BLOCKS: usize = 128;
const MAX_STITCH_TRIANGLES: usize = 300_000;

/// 細かくするブロック: 辺でつながる頂点どうしの法線のなす角が 35° を超える所（細い部分・尖った所）を含むブロックと、
/// その隣（面・辺・角で接するもの）
fn sharp_blocks(mesh: &Mesh, vertex_cell: &[u32], n: usize, nb: usize) -> crate::adaptive::HashSet<usize> {
    let cos = 35f32.to_radians().cos();
    let nr = &mesh.normals;
    let block = |v: u32| {
        let c = vertex_cell[v as usize] as usize;
        [c % n, (c / n) % n, c / (n * n)].map(|x| (x / BLOCK).min(nb - 1))
    };
    let mut hit: crate::adaptive::HashSet<[usize; 3]> = Default::default();
    for t in mesh.indices.chunks_exact(3) {
        let (a, b, c) = (3 * t[0] as usize, 3 * t[1] as usize, 3 * t[2] as usize);
        let dot = |p: usize, q: usize| nr[p] * nr[q] + nr[p + 1] * nr[q + 1] + nr[p + 2] * nr[q + 2];
        // 3 本の辺のどれかで、法線が大きくずれる（ふつうは無いので、まとめて確かめる）
        if dot(a, b) >= cos && dot(b, c) >= cos && dot(c, a) >= cos {
            continue;
        }
        for e in 0..3 {
            let (u, v) = (t[e], t[(e + 1) % 3]);
            if dot(3 * u as usize, 3 * v as usize) < cos {
                hit.insert(block(u));
                hit.insert(block(v));
            }
        }
    }
    let mut out = crate::adaptive::HashSet::default();
    for b in hit {
        for dz in -1i64..=1 {
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let c = [b[0] as i64 + dx, b[1] as i64 + dy, b[2] as i64 + dz];
                    if c.iter().all(|&v| v >= 0 && v < nb as i64) {
                        out.insert(c[0] as usize + nb * (c[1] as usize + nb * c[2] as usize));
                    }
                }
            }
        }
    }
    out
}

/// 極に張り付く面（`1/(x y) = z` の壁など）を描く。極のすぐ隣を走る面は、格子の辺の上に「極と零点が 1 つずつ」入るので、
/// 両端の符号がそろってしまい、ふつうの Surface Nets では見落とす。面を張るのは本当は「辺の上の零点の数が奇数の辺」なので、
/// 極の近くのセルだけ、辺を 32 等分して零点を数え直し、その範囲の面を作り直す:
/// 1. 調べ始めるセル: S4 で零点でないとされた辺（極・飛び）を持つセルと、有限でない角（`bad_pts`）を持つセル
/// 2. その辺を 32 等分した点で F を計算し、符号の変わる小さい区間ごとに、二分して |F| が縮むか（零点）、縮まないか（極・飛び）で分ける。
///    零点がちょうど 1 つの辺を交点にする（その前後の F の符号で、面の向きを決める）。両端の符号が違い、零点ではないとされていない辺は、
///    ふつうと同じ直線の補間の交点にする。辺を 32 等分して交点が見つかったら、その辺を囲むセルも調べる（合わせて 200,000 辺まで）
/// 3. 調べたセルと、それと辺を共有するセル（ふつうの交点だけで、ふつうの Surface Nets と同じ位置の頂点）に頂点を置き、
///    調べたセルの辺に面を張る。調べたセルには、ふつうの Surface Nets は頂点を置かない
/// 1 つのセルの中で、極をはさんで両側に零点がある所も、頂点は 1 つにまとめる（分けない）
#[allow(clippy::too_many_arguments)]
fn pole_meshes(
    prog: &Program,
    combos: &[Vec<f64>],
    field: &SparseField,
    dom: Domain,
    n: usize,
    not_zero: &crate::adaptive::HashSet<usize>,
    bad_pts: &[usize],
    filled: &crate::adaptive::HashSet<usize>,
    skip: &crate::adaptive::HashSet<usize>,
) -> BranchMeshes {
    const SAMPLES: usize = 32;
    const ROUNDS: usize = 12;
    const MAX_EDGES: usize = 200_000;
    let m = n + 1;
    let idx = |q: [usize; 3]| q[0] + m * (q[1] + m * q[2]);
    let cell_id = |c: [usize; 3]| c[0] + n * (c[1] + n * c[2]);
    let pt = |q: [usize; 3]| [0, 1, 2].map(|a| dom.at(a, q[a] as f64, n));
    let nb = field.nb;
    let occupied = |c: [usize; 3]| field.occupied[(c[0] / BLOCK).min(nb - 1) + nb * ((c[1] / BLOCK).min(nb - 1) + nb * (c[2] / BLOCK).min(nb - 1))];
    // 使えるセル: 格子の中、値を持っている、段差をまたぐセル・埋めた角のセルでない
    let usable = |c: [usize; 3]| {
        c.iter().all(|&v| v < n)
            && occupied(c)
            && !skip.contains(&cell_id(c))
            && CORNERS.iter().all(|d| {
                let q = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                field.at(q) != usize::MAX && (filled.is_empty() || !filled.contains(&idx(q)))
            })
    };
    // 辺（始点 p、向き a）を囲む 4 つのセル
    let around = |p: [usize; 3], a: usize| -> Vec<[usize; 3]> {
        let (b, c) = ((a + 1) % 3, (a + 2) % 3);
        let mut out = Vec::with_capacity(4);
        for (db, dc) in [(1usize, 1usize), (0, 1), (0, 0), (1, 0)] {
            if p[b] < db || p[c] < dc {
                continue;
            }
            let mut s = p;
            s[b] -= db;
            s[c] -= dc;
            out.push(s);
        }
        out
    };
    // 1. 調べ始めるセル
    let mut region: crate::adaptive::HashSet<usize> = Default::default();
    let mut frontier: Vec<[usize; 3]> = Vec::new();
    let add = |c: [usize; 3], region: &mut crate::adaptive::HashSet<usize>, frontier: &mut Vec<[usize; 3]>| {
        if usable(c) && region.insert(cell_id(c)) {
            frontier.push(c);
        }
    };
    for &id in not_zero {
        let (a, q) = (id % 3, id / 3);
        let p = [q % m, (q / m) % m, q / (m * m)];
        for c in around(p, a) {
            add(c, &mut region, &mut frontier);
        }
    }
    // 有限でない格子点（`bad_pts`）を角に持つセルのうち、有限な角もあるもの
    for &p in bad_pts {
        let q = [p % m, (p / m) % m, p / (m * m)];
        for d in CORNERS {
            if q.iter().zip(d).any(|(&v, o)| v < o) {
                continue;
            }
            let c = [q[0] - d[0], q[1] - d[1], q[2] - d[2]];
            if c.iter().any(|&v| v >= n) || !occupied(c) {
                continue;
            }
            if CORNERS.iter().any(|o| field.f([c[0] + o[0], c[1] + o[1], c[2] + o[2]]).is_finite()) {
                add(c, &mut region, &mut frontier);
            }
        }
    }
    let mut out = BranchMeshes::default();
    if region.is_empty() {
        return out;
    }
    // 2. 辺ごとの交点（辺の番号 → (位置 t, 始点の側の F が負か)。交点が無ければ None）
    let mut cross: crate::adaptive::HashMap<usize, Option<(f64, bool)>> = Default::default();
    let mut sc = FieldScratch::default();
    let mut checked = 0usize;
    while !frontier.is_empty() && checked < MAX_EDGES {
        // 新しいセルの辺のうち、まだ調べていないもの
        let mut todo: Vec<([usize; 3], usize)> = Vec::new();
        for c in std::mem::take(&mut frontier) {
            for [ca, cb] in EDGES {
                let (a, b) = (CORNERS[ca], CORNERS[cb]);
                let ax = (0..3).find(|&x| a[x] != b[x]).unwrap_or(0);
                let p = [c[0] + a[0], c[1] + a[1], c[2] + a[2]];
                let id = 3 * idx(p) + ax;
                if let std::collections::hash_map::Entry::Vacant(e) = cross.entry(id) {
                    e.insert(None);
                    todo.push((p, ax));
                }
            }
        }
        checked += todo.len();
        // ふつうの辺（両端が有限で符号が違い、零点でないとされていない）は、直線の補間。ほかは 32 等分して調べる
        let mut sample: Vec<([usize; 3], usize)> = Vec::new();
        for &(p, ax) in &todo {
            let mut q = p;
            q[ax] += 1;
            let (f0, f1) = (field.f(p), field.f(q));
            let id = 3 * idx(p) + ax;
            if f0.is_finite() && f1.is_finite() && (f0 < 0.0) != (f1 < 0.0) {
                // 両端の符号が違う辺: ふつうは直線の補間。零点でないとされた辺（極が 1 つ）は、零点が偶数個（0 か 2）なので、
                // 零点がちょうど 1 つにはならない（調べない）
                if !not_zero.contains(&id) {
                    cross.insert(id, Some(((f0 / (f0 - f1)) as f64, f0 < 0.0)));
                }
            } else {
                sample.push((p, ax));
            }
        }
        if sample.is_empty() {
            continue;
        }
        let at = |p: [usize; 3], ax: usize, t: f64| {
            let mut x = pt(p);
            x[ax] += (dom.at(ax, 1.0, n) - dom.at(ax, 0.0, n)) * t;
            x
        };
        let mut cols: [Vec<f64>; 3] = Default::default();
        for &(p, ax) in &sample {
            for s in 0..=SAMPLES {
                let x = at(p, ax, s as f64 / SAMPLES as f64);
                for k in 0..3 {
                    cols[k].push(x[k]);
                }
            }
        }
        let vals = field_values_with(prog, combos, [&cols[0], &cols[1], &cols[2]], &mut sc).to_vec();
        // 符号の変わる小さい区間 (辺, ta, tb, fa, fb, 最初の |fa| + |fb|)
        let mut iv: Vec<(usize, f64, f64, f64, f64, f64)> = Vec::new();
        for (e, _) in sample.iter().enumerate() {
            let v = &vals[e * (SAMPLES + 1)..(e + 1) * (SAMPLES + 1)];
            for s in 0..SAMPLES {
                let (a, b) = (v[s], v[s + 1]);
                if a.is_finite() && b.is_finite() && (a < 0.0) != (b < 0.0) {
                    iv.push((e, s as f64 / SAMPLES as f64, (s + 1) as f64 / SAMPLES as f64, a, b, a.abs() + b.abs()));
                }
            }
        }
        for _ in 0..ROUNDS {
            let mut cols: [Vec<f64>; 3] = Default::default();
            for &(e, ta, tb, ..) in &iv {
                let (p, ax) = sample[e];
                let x = at(p, ax, 0.5 * (ta + tb));
                for k in 0..3 {
                    cols[k].push(x[k]);
                }
            }
            let mid = field_values_with(prog, combos, [&cols[0], &cols[1], &cols[2]], &mut sc).to_vec();
            for (c, &f) in iv.iter_mut().zip(&mid) {
                let t = 0.5 * (c.1 + c.2);
                if !f.is_finite() || (f < 0.0) == (c.3 < 0.0) {
                    if f.is_finite() {
                        c.1 = t;
                        c.3 = f;
                    } else {
                        c.2 = t; // 定義されない所は、終点の側に寄せる（その区間は零点にならない）
                    }
                } else {
                    c.2 = t;
                    c.4 = f;
                }
            }
        }
        // 辺ごとに零点を数える
        let mut zeros: Vec<Vec<(f64, bool)>> = vec![Vec::new(); sample.len()];
        for &(e, ta, tb, fa, fb, first) in &iv {
            if fa.is_finite() && fb.is_finite() && (fa < 0.0) != (fb < 0.0) && fa.abs() + fb.abs() < 0.5 * first {
                zeros[e].push((0.5 * (ta + tb), fa < 0.0));
            }
        }
        for (e, z) in zeros.iter().enumerate() {
            let (p, ax) = sample[e];
            if z.len() != 1 {
                continue;
            }
            cross.insert(3 * idx(p) + ax, Some(z[0]));
            // 隠れた交点の辺を囲むセルも調べる
            for c in around(p, ax) {
                add(c, &mut region, &mut frontier);
            }
        }
    }
    // 3. 頂点: 調べたセルと、それと辺を共有するセル
    let mut ring: Vec<usize> = Vec::new();
    for &c in &region {
        let q = [c % n, (c / n) % n, c / (n * n)];
        for dz in -1i64..=1 {
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    if [dx, dy, dz].iter().filter(|&&d| d != 0).count() > 2 {
                        continue;
                    }
                    let d = [q[0] as i64 + dx, q[1] as i64 + dy, q[2] as i64 + dz];
                    if d.iter().any(|&v| v < 0 || v >= n as i64) {
                        continue;
                    }
                    let id = cell_id(d.map(|v| v as usize));
                    if !region.contains(&id) {
                        ring.push(id);
                    }
                }
            }
        }
    }
    ring.sort_unstable();
    ring.dedup();
    // 交点の位置（調べた辺はその結果、ほかは直線の補間）
    let crossing = |p: [usize; 3], ax: usize| -> Option<(f64, bool)> {
        let id = 3 * idx(p) + ax;
        if let Some(c) = cross.get(&id) {
            return *c;
        }
        let mut q = p;
        q[ax] += 1;
        let (f0, f1) = (field.f(p), field.f(q));
        if f0.is_finite() && f1.is_finite() && (f0 < 0.0) != (f1 < 0.0) && !not_zero.contains(&id) {
            Some(((f0 / (f0 - f1)) as f64, f0 < 0.0))
        } else {
            None
        }
    };
    let mut cells: Vec<usize> = region.iter().copied().chain(ring.iter().copied()).collect();
    cells.sort_unstable();
    let mut vert: crate::adaptive::HashMap<usize, u32> = Default::default();
    for &c in &cells {
        let q = [c % n, (c / n) % n, c / (n * n)];
        if !occupied(q) {
            continue;
        }
        let (mut sum, mut cnt) = ([0.0f64; 3], 0.0f64);
        for [ca, cb] in EDGES {
            let (a, b) = (CORNERS[ca], CORNERS[cb]);
            let ax = (0..3).find(|&x| a[x] != b[x]).unwrap_or(0);
            if let Some((t, _)) = crossing([q[0] + a[0], q[1] + a[1], q[2] + a[2]], ax) {
                let mut r = [a[0] as f64, a[1] as f64, a[2] as f64];
                r[ax] += t;
                for x in 0..3 {
                    sum[x] += r[x];
                }
                cnt += 1.0;
            }
        }
        if cnt > 0.0 {
            let p = [q[0] as f64 + sum[0] / cnt, q[1] as f64 + sum[1] / cnt, q[2] as f64 + sum[2] / cnt];
            vert.insert(c, out.mesh.push_vertex([dom.at(0, p[0], n), dom.at(1, p[1], n), dom.at(2, p[2], n)]));
            out.vertex_cell.push(c as u32);
        }
    }
    // 面: 調べたセルの辺のうち、交点のあるもの
    let mut seen: crate::adaptive::HashSet<usize> = Default::default();
    let mut sorted: Vec<usize> = region.iter().copied().collect();
    sorted.sort_unstable();
    for &c in &sorted {
        let q = [c % n, (c / n) % n, c / (n * n)];
        for [ca, cb] in EDGES {
            let (a, b) = (CORNERS[ca], CORNERS[cb]);
            let ax = (0..3).find(|&x| a[x] != b[x]).unwrap_or(0);
            let p = [q[0] + a[0], q[1] + a[1], q[2] + a[2]];
            if !seen.insert(3 * idx(p) + ax) {
                continue;
            }
            let Some((_, neg)) = crossing(p, ax) else { continue };
            let cs = around(p, ax);
            if cs.len() != 4 || cs.iter().any(|&s| s.iter().any(|&v| v >= n)) {
                continue;
            }
            let ids: Vec<u32> = cs.iter().filter_map(|&s| vert.get(&cell_id(s)).copied()).collect();
            if ids.len() != 4 {
                continue;
            }
            let [q0, q1, q2, q3] = [ids[0], ids[1], ids[2], ids[3]];
            if neg {
                out.mesh.indices.extend_from_slice(&[q0, q1, q2, q0, q2, q3]);
            } else {
                out.mesh.indices.extend_from_slice(&[q0, q2, q1, q0, q3, q2]);
            }
        }
    }
    out.mixed = region;
    out
}

/// `extra` の面を `mesh` に足す。`mesh` の頂点とビットまで同じ位置の頂点は、その頂点を使う（継ぎ目で頂点を共有する）
fn weld_append(mesh: &mut Mesh, vertex_cell: &mut Vec<u32>, extra: &BranchMeshes) {
    if extra.mesh.indices.is_empty() {
        return;
    }
    let key = |p: &[f32]| [p[0].to_bits(), p[1].to_bits(), p[2].to_bits()];
    let mut at: crate::adaptive::HashMap<[u32; 3], u32> = Default::default();
    for (v, p) in mesh.positions.chunks_exact(3).enumerate() {
        at.entry(key(p)).or_insert(v as u32);
    }
    let mut map: Vec<u32> = Vec::with_capacity(extra.mesh.positions.len() / 3);
    for (v, p) in extra.mesh.positions.chunks_exact(3).enumerate() {
        map.push(match at.get(&key(p)) {
            Some(&id) => id,
            None => {
                let id = (mesh.positions.len() / 3) as u32;
                mesh.positions.extend_from_slice(p);
                vertex_cell.push(extra.vertex_cell[v]);
                at.insert(key(p), id);
                id
            }
        });
    }
    mesh.indices.extend(extra.mesh.indices.iter().map(|&v| map[v as usize]));
}

/// 枝ごとに作る（[`branch_meshes`]）セルの数の上限。これより多い式は、ふつうの Surface Nets のまま
const MAX_BRANCH_CELLS: usize = 400_000;
/// 極に張り付く面（[`pole_meshes`]）を探す、調べ始めの辺・点の数の上限
const MAX_POLE_SEEDS: usize = 100_000;
/// 段差の元がこれより多い式は、枝ごとに作る道を使わない
const MAX_BRANCH_SITES: usize = 8;
/// 1 つのセルに、これより多い種類の枝が出たら、そのセルは枝ごとに作らず、ふつうの Surface Nets に任せる（面が詰まりすぎた所）
const MAX_CELL_BRANCHES: usize = 2;

/// [`branch_meshes`] の結果
#[derive(Default)]
struct BranchMeshes {
    /// 段差をまたぐセル（ふつうの Surface Nets では頂点を置かない）
    mixed: crate::adaptive::HashSet<usize>,
    /// 段差をまたぐセルの面（枝ごとに作って、本当の枝の領域で切り取ったもの）
    mesh: Mesh,
    /// 頂点ごとの、頂点を置いたセル
    vertex_cell: Vec<u32>,
}

/// `floor`・`mod` などを含む陰関数で、段差をまたぐセル（8 つの角の枝の番号の組 τ が混ざるセル）の面を作る。
/// 枝の番号を固定した式 F_τ（[`Program::frozen`]。段差の無い、なめらかな続き）で、τ ごとに小さな Surface Nets を作る:
/// τ の頂点を置くのは、τ を角に持つ段差をまたぐセルと、それと辺を共有するセル（角が全部 τ のふつうのセルの頂点は、
/// ふつうの Surface Nets と同じ位置になるので、継ぎ目で頂点を共有する。ほかの枝のセルの頂点は、τ の面を段差の向こうまで
/// 延ばすためのもので、切り取りで消える）。面を張るのは、段差をまたぐセルを囲む辺だけ
/// （ほかの辺は、ふつうの Surface Nets が張る）。できた面は、本当の枝の番号が τ である所だけを残す（[`trim_by`]）
fn branch_meshes(prog: &Program, field: &SparseField, dom: Domain, n: usize) -> BranchMeshes {
    let m = n + 1;
    let k = prog.branch_sites();
    let branch = prog.branches();
    let idx = |q: [usize; 3]| q[0] + m * (q[1] + m * q[2]);
    let cell_id = |c: [usize; 3]| c[0] + n * (c[1] + n * c[2]);
    let pt = |q: [usize; 3]| [0, 1, 2].map(|a| dom.at(a, q[a] as f64, n));
    let nb = field.nb;
    let occupied = |c: [usize; 3]| {
        let b = c.map(|x| (x / BLOCK).min(nb - 1));
        field.occupied[b[0] + nb * (b[1] + nb * b[2])]
    };
    // 使われているブロックのセルの角になる格子点すべてで、枝の番号の組 τ を計算して番号をつける（NONE: 計算できない点）
    let key = |b: &[f64]| -> [i64; MAX_BRANCH_SITES] {
        let mut out = [0i64; MAX_BRANCH_SITES];
        for (o, &v) in out.iter_mut().zip(b) {
            *o = if v.is_nan() { i64::MIN } else { v as i64 };
        }
        out
    };
    let mut tau_all: Vec<u32> = vec![NONE; m * m * m];
    let mut taus: Vec<Vec<f64>> = Vec::new();
    {
        let mut need = vec![false; m * m * m];
        for kz in 0..n {
            for jy in 0..n {
                for ix in 0..n {
                    if occupied([ix, jy, kz]) {
                        for d in CORNERS {
                            need[idx([ix + d[0], jy + d[1], kz + d[2]])] = true;
                        }
                    }
                }
            }
        }
        let pts: Vec<usize> = (0..m * m * m).filter(|&p| need[p]).collect();
        let cols: [Vec<f64>; 3] = [0, 1, 2].map(|a| pts.iter().map(|&p| pt([p % m, (p / m) % m, p / (m * m)])[a]).collect());
        let br = branch.eval(&[&cols[0], &cols[1], &cols[2]], pts.len());
        let mut ids: crate::adaptive::HashMap<[i64; MAX_BRANCH_SITES], u32> = Default::default();
        for (q, &p) in pts.iter().enumerate() {
            let b = &br[q * k..(q + 1) * k];
            if b.iter().any(|v| !v.is_finite()) {
                continue;
            }
            tau_all[p] = *ids.entry(key(b)).or_insert_with(|| {
                taus.push(b.to_vec());
                (taus.len() - 1) as u32
            });
        }
    }
    let tau_at = |p: usize| (tau_all[p] != NONE).then_some(tau_all[p]);
    // 枝 t と枝 u の境目が、t に含まれるか（違う段差の元すべてで含まれるときだけ。決めない元は含まれるとみなす）。
    // 含まれないなら、F_t が境目で 0 になっても、そこは F の零点ではない（z = mod(x y, z) は z に近づくが z にはならない）
    let closed_high = prog.branch_closed_high();
    let owns = |t: u32, u: u32| {
        let (a, b) = (&taus[t as usize], &taus[u as usize]);
        (0..k).all(|j| a[j] == b[j] || closed_high[j].is_none_or(|high| (a[j] > b[j]) == high))
    };
    // セルの角の τ（計算できない角があれば None）と、その中の違う τ
    let corner_taus = |c: [usize; 3]| -> Option<([u32; 8], Vec<u32>)> {
        let mut ts = [NONE; 8];
        for (t, d) in ts.iter_mut().zip(CORNERS) {
            *t = tau_at(idx([c[0] + d[0], c[1] + d[1], c[2] + d[2]]))?;
        }
        let mut uniq = ts.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        Some((ts, uniq))
    };

    // 調べるセル:
    // - 角の F の符号が混ざるセル（面が通りうるセル）
    // - 角の τ が混ざり、どれかの枝 τ の続き F_τ の符号がセルの中で変わるセル。mod(x y, z) = 0 のように、
    //   F が 0 に触れるだけで符号が変わらない段差（z > 0 では F ∈ [0, z)）の面は、F の符号では見つからない。
    //   F の符号だけで選ぶと、F = 0 ちょうどを正に数えるので、z < 0 の側（F ∈ (z, 0]）だけ見つかり、上下が非対称になっていた
    let mut cand: Vec<[usize; 3]> = Vec::new();
    let mut stepped: Vec<([usize; 3], [u32; 8], Vec<u32>)> = Vec::new();
    for kz in 0..n {
        for jy in 0..n {
            for ix in 0..n {
                let c = [ix, jy, kz];
                if !occupied(c) {
                    continue;
                }
                let (mut neg, mut pos, mut bad) = (false, false, false);
                for d in CORNERS {
                    let v = field.f([ix + d[0], jy + d[1], kz + d[2]]);
                    if !v.is_finite() {
                        bad = true;
                        break;
                    }
                    if v < 0.0 { neg = true } else { pos = true }
                }
                if bad {
                    continue;
                }
                if neg && pos {
                    cand.push(c);
                } else if let Some((ts, uniq)) = corner_taus(c) {
                    if (2..=MAX_CELL_BRANCHES).contains(&uniq.len()) {
                        stepped.push((c, ts, uniq));
                    }
                }
            }
        }
    }
    // 符号の混ざるセルだけで多すぎる式は、ここでやめる（τ の混ざるセルは、下で絞り込んでから数える）
    if cand.len() > MAX_BRANCH_CELLS {
        return BranchMeshes::default();
    }
    let mut sign_mixed = cand.clone();
    // τ の混ざるセルのうち、F_τ が 0 になるもの（F は 0 に触れるだけで、符号は変わらない段差）
    let mut touching: Vec<[usize; 3]> = Vec::new();
    // 符号のそろった、τ の混ざるセル: 各 τ について、ほかの枝の角での F_τ を計算し、符号が変わるか調べる
    if !stepped.is_empty() {
        // (格子点, τ) → F_τ。τ ごとにまとめて計算する
        let mut want: crate::adaptive::HashMap<u32, Vec<usize>> = Default::default();
        for (c, ts, uniq) in &stepped {
            for &t in uniq {
                for (&tc, d) in ts.iter().zip(CORNERS) {
                    if tc != t && owns(t, tc) {
                        want.entry(t).or_default().push(idx([c[0] + d[0], c[1] + d[1], c[2] + d[2]]));
                    }
                }
            }
        }
        let mut cont: crate::adaptive::HashMap<(usize, u32), f32> = Default::default();
        let mut order: Vec<u32> = want.keys().copied().collect();
        order.sort_unstable();
        for t in order {
            let mut ps = std::mem::take(want.get_mut(&t).unwrap());
            ps.sort_unstable();
            ps.dedup();
            let cols: [Vec<f64>; 3] = [0, 1, 2].map(|a| ps.iter().map(|&p| pt([p % m, (p / m) % m, p / (m * m)])[a]).collect());
            let mut v = vec![C::new(0.0, 0.0); ps.len()];
            prog.frozen(&taus[t as usize]).eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut v);
            for (&p, z) in ps.iter().zip(v) {
                cont.insert((p, t), as_real(z) as f32);
            }
        }
        for (c, ts, uniq) in &stepped {
            let crosses = uniq.iter().any(|&t| {
                let (mut neg, mut pos) = (false, false);
                for (&tc, d) in ts.iter().zip(CORNERS) {
                    // t が境目を含まない枝の角は見ない（そこで F_t が 0 になっても、F の零点ではない）
                    if tc != t && !owns(t, tc) {
                        continue;
                    }
                    let p = idx([c[0] + d[0], c[1] + d[1], c[2] + d[2]]);
                    let v = if tc == t { field.f([c[0] + d[0], c[1] + d[1], c[2] + d[2]]) } else { cont[&(p, t)] };
                    if !v.is_finite() {
                        continue;
                    }
                    if v < 0.0 { neg = true } else { pos = true }
                }
                neg && pos
            });
            if crosses {
                touching.push(*c);
            }
        }
    }
    // 面で隣り合うセルも足す（段差の所では、F の符号がそろっていても、隣の枝の F_τ の面が通ることがある）
    let expand = |mut cand: Vec<[usize; 3]>| -> Vec<[usize; 3]> {
        let mut set: crate::adaptive::HashSet<usize> = cand.iter().map(|&c| cell_id(c)).collect();
        let mut more: Vec<[usize; 3]> = Vec::new();
        for &c in &cand {
            for a in 0..3 {
                for up in [false, true] {
                    let mut d = c;
                    if up {
                        if d[a] + 1 >= n {
                            continue;
                        }
                        d[a] += 1;
                    } else {
                        if d[a] == 0 {
                            continue;
                        }
                        d[a] -= 1;
                    }
                    if CORNERS.iter().all(|o| field.f([d[0] + o[0], d[1] + o[1], d[2] + o[2]]).is_finite()) && set.insert(cell_id(d)) {
                        more.push(d);
                    }
                }
            }
        }
        cand.extend(more);
        cand
    };
    // 調べるセルが多すぎるときは、0 に触れるだけの段差のセルを外して、符号の混ざるセルだけにする。
    // それでも多すぎる式（細かさ 200 の、面が詰まった式など）は、枝ごとに作らない（メモリと時間がかかりすぎる）
    let mut all = cand.clone();
    all.extend_from_slice(&touching);
    let mut cand = expand(all);
    if cand.len() > MAX_BRANCH_CELLS && !touching.is_empty() {
        cand = expand(std::mem::take(&mut sign_mixed));
    }
    if cand.len() > MAX_BRANCH_CELLS {
        return BranchMeshes::default();
    }
    let mut out = BranchMeshes::default();
    // 段差をまたぐセル（調べるセルのうち、角の τ が混ざるもの）と、そこに出てくる τ
    let mut cells_of: crate::adaptive::HashMap<u32, Vec<[usize; 3]>> = Default::default();
    for &c in &cand {
        let mut ts: [u32; 8] = [u32::MAX; 8];
        let mut ok = true;
        for (t, d) in ts.iter_mut().zip(CORNERS) {
            match tau_at(idx([c[0] + d[0], c[1] + d[1], c[2] + d[2]])) {
                Some(id) => *t = id,
                None => {
                    ok = false;
                    break;
                }
            }
        }
        if !ok {
            continue;
        }
        let mut uniq: Vec<u32> = ts.to_vec();
        uniq.sort_unstable();
        uniq.dedup();
        if uniq.len() == 1 {
            continue;
        }
        // 枝が多すぎるセル（面が詰まりすぎて、枝ごとには描けない所）は、ふつうの Surface Nets に任せる
        if uniq.len() > MAX_CELL_BRANCHES {
            continue;
        }
        for t in uniq {
            cells_of.entry(t).or_default().push(c);
        }
    }
    if cells_of.is_empty() {
        return out;
    }
    // τ ごとに作る
    let mut order: Vec<u32> = cells_of.keys().copied().collect();
    order.sort_unstable();
    for t in order {
        let mixed_cells = &cells_of[&t];
        let fz = prog.frozen(&taus[t as usize]);
        // 頂点を置くセル: 段差をまたぐセルと、それと辺を共有するセル
        let mut cells: crate::adaptive::HashSet<usize> = mixed_cells.iter().map(|&c| cell_id(c)).collect();
        for &c in mixed_cells {
            for dz in -1i64..=1 {
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        if [dx, dy, dz].iter().filter(|&&d| d != 0).count() > 2 {
                            continue;
                        }
                        let nb = [c[0] as i64 + dx, c[1] as i64 + dy, c[2] as i64 + dz];
                        if nb.iter().any(|&v| v < 0 || v >= n as i64) {
                            continue;
                        }
                        let d = nb.map(|v| v as usize);
                        // 角が全部 τ のふつうのセル（継ぎ目）と、ほかの枝のセル（τ の面を段差の向こうまで延ばし、段差の所で切り取る）
                        if CORNERS.iter().all(|o| field.f([d[0] + o[0], d[1] + o[1], d[2] + o[2]]).is_finite()) {
                            cells.insert(cell_id(d));
                        }
                    }
                }
            }
        }
        // 角での F_τ（角の枝が τ なら、ふつうの F。そうでなければ、固定した式で計算する）
        let mut fval: crate::adaptive::HashMap<usize, f64> = Default::default();
        let mut need: Vec<usize> = Vec::new();
        for &c in &cells {
            let q = [c % n, (c / n) % n, c / (n * n)];
            for d in CORNERS {
                let p = idx([q[0] + d[0], q[1] + d[1], q[2] + d[2]]);
                if fval.contains_key(&p) {
                    continue;
                }
                if tau_at(p) == Some(t) {
                    fval.insert(p, field.f([p % m, (p / m) % m, p / (m * m)]) as f64);
                } else {
                    fval.insert(p, f64::NAN);
                    need.push(p);
                }
            }
        }
        if !need.is_empty() {
            let cols: [Vec<f64>; 3] = [0, 1, 2].map(|a| need.iter().map(|&p| pt([p % m, (p / m) % m, p / (m * m)])[a]).collect());
            let mut v = vec![C::new(0.0, 0.0); need.len()];
            fz.eval_batch(&[&cols[0], &cols[1], &cols[2]], &mut v);
            for (&p, z) in need.iter().zip(v) {
                // f32 に丸めて、ふつうの格子の値と同じ精度にする
                fval.insert(p, (as_real(z) as f32) as f64);
            }
        }
        let f = |q: [usize; 3]| fval.get(&idx(q)).copied().unwrap_or(f64::NAN);
        // 頂点（セルの中の、辺の上で F_τ = 0 になる点の平均）。ふつうのセルの頂点が、ふつうの Surface Nets と
        // ビットまで同じ位置になるよう、同じ手順で計算する（f32 の値で補間し、セルの中の位置を足してから格子の座標にする）
        let mut local = Mesh::default();
        let mut local_cell: Vec<u32> = Vec::new();
        let mut vert: crate::adaptive::HashMap<usize, u32> = Default::default();
        let mut sorted: Vec<usize> = cells.iter().copied().collect();
        sorted.sort_unstable();
        for &c in &sorted {
            let q = [c % n, (c / n) % n, c / (n * n)];
            let (mut sum, mut cnt) = ([0.0f64; 3], 0.0f64);
            for [ca, cb] in EDGES {
                let (a, b) = (CORNERS[ca], CORNERS[cb]);
                let ax = (0..3).find(|&x| a[x] != b[x]).unwrap_or(0);
                let pa = [q[0] + a[0], q[1] + a[1], q[2] + a[2]];
                let pb = [q[0] + b[0], q[1] + b[1], q[2] + b[2]];
                let (f0, f1) = (f(pa) as f32, f(pb) as f32);
                if !f0.is_finite() || !f1.is_finite() || (f0 < 0.0) == (f1 < 0.0) {
                    continue;
                }
                let t = (f0 / (f0 - f1)) as f64;
                let mut r = [a[0] as f64, a[1] as f64, a[2] as f64];
                r[ax] += t;
                for x in 0..3 {
                    sum[x] += r[x];
                }
                cnt += 1.0;
            }
            if cnt > 0.0 {
                let p = [q[0] as f64 + sum[0] / cnt, q[1] as f64 + sum[1] / cnt, q[2] as f64 + sum[2] / cnt];
                let id = local.push_vertex([dom.at(0, p[0], n), dom.at(1, p[1], n), dom.at(2, p[2], n)]);
                vert.insert(c, id);
                local_cell.push(c as u32);
            }
        }
        // 枝ごとに作るのは、τ の頂点を置いた段差をまたぐセルだけ（どの枝の F_τ にも零点が無いセルの面は、F が飛ぶ所に
        // しか無いので、ふつうの Surface Nets に任せる）
        let active: Vec<[usize; 3]> = mixed_cells.iter().copied().filter(|&c| vert.contains_key(&cell_id(c))).collect();
        for &c in &active {
            out.mixed.insert(cell_id(c));
        }
        // 面: その段差をまたぐセルの辺のうち、F_τ の符号が変わり、周りの 4 セルが全部 τ の頂点を持つもの
        let mut seen: crate::adaptive::HashSet<(usize, usize)> = Default::default();
        for &c in &active {
            for [ca, cb] in EDGES {
                let (a, b) = (CORNERS[ca], CORNERS[cb]);
                let ax = (0..3).find(|&x| a[x] != b[x]).unwrap_or(0);
                let p0 = if a[ax] == 0 { [c[0] + a[0], c[1] + a[1], c[2] + a[2]] } else { [c[0] + b[0], c[1] + b[1], c[2] + b[2]] };
                if !seen.insert((idx(p0), ax)) {
                    continue;
                }
                let mut p1 = p0;
                p1[ax] += 1;
                let (f0, f1) = (f(p0), f(p1));
                if !f0.is_finite() || !f1.is_finite() || (f0 < 0.0) == (f1 < 0.0) {
                    continue;
                }
                let (bx, cx) = ((ax + 1) % 3, (ax + 2) % 3);
                let mut around = [NONE; 4];
                let mut ok = true;
                for (r, (db, dc)) in [(1usize, 1usize), (0, 1), (0, 0), (1, 0)].into_iter().enumerate() {
                    let mut s = p0;
                    if s[bx] < db || s[cx] < dc {
                        ok = false;
                        break;
                    }
                    s[bx] -= db;
                    s[cx] -= dc;
                    if s.iter().any(|&v| v >= n) {
                        ok = false;
                        break;
                    }
                    match vert.get(&cell_id(s)) {
                        Some(&v) => around[r] = v,
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !ok {
                    continue;
                }
                let [q0, q1, q2, q3] = around;
                if f0 < 0.0 {
                    local.indices.extend_from_slice(&[q0, q1, q2, q0, q2, q3]);
                } else {
                    local.indices.extend_from_slice(&[q0, q2, q1, q0, q3, q2]);
                }
            }
        }
        if local.indices.is_empty() {
            continue;
        }
        // 本当の枝の番号が τ である所だけ残す
        let want = key(&taus[t as usize]);
        let inside = |xyz: [&[f64]; 3]| -> Vec<bool> {
            let len = xyz[0].len();
            let b = branch.eval(&xyz, len);
            (0..len).map(|q| key(&b[q * k..(q + 1) * k]) == want).collect()
        };
        trim_by(&mut local, &mut local_cell, &inside, 10);
        let base = (out.mesh.positions.len() / 3) as u32;
        out.mesh.positions.extend_from_slice(&local.positions);
        out.mesh.indices.extend(local.indices.iter().map(|&v| v + base));
        out.vertex_cell.extend_from_slice(&local_cell);
    }
    out
}

/// メッシュのうち、`inside` が真の所だけを残す。3 頂点とも外の三角形は捨て、混ざる三角形は、辺の上で内側の端を
/// `rounds` 回の二分で探して、そこまでの多角形に切る（辺の上に求めた点は、隣の三角形と共有する）。
/// 足した頂点の `vertex_cell` は、内側の頂点のセルにする
fn trim_by(mesh: &mut Mesh, vertex_cell: &mut Vec<u32>, inside: &dyn Fn([&[f64]; 3]) -> Vec<bool>, rounds: usize) {
    let nv = mesh.positions.len() / 3;
    let pos = |mesh: &Mesh, v: usize| [0, 1, 2].map(|a| mesh.positions[3 * v + a] as f64);
    let cols: [Vec<f64>; 3] = [0, 1, 2].map(|a| (0..nv).map(|v| pos(mesh, v)[a]).collect());
    let ins = inside([&cols[0], &cols[1], &cols[2]]);
    if ins.iter().all(|&b| b) {
        return;
    }
    let mut edge_pt: crate::adaptive::HashMap<(u32, u32), u32> = Default::default();
    let mut todo: Vec<(u32, u32)> = Vec::new();
    for t in mesh.indices.chunks_exact(3) {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            for (d, u) in [(a, b), (b, a)] {
                if ins[d as usize] && !ins[u as usize] && !edge_pt.contains_key(&(d, u)) {
                    edge_pt.insert((d, u), NONE);
                    todo.push((d, u));
                }
            }
        }
    }
    let (mut ta, mut tb) = (vec![0.0f64; todo.len()], vec![1.0f64; todo.len()]);
    let lerp = |mesh: &Mesh, (d, u): (u32, u32), t: f64, a: usize| {
        let (p, q) = (pos(mesh, d as usize)[a], pos(mesh, u as usize)[a]);
        p + (q - p) * t
    };
    let mut cols: [Vec<f64>; 3] = Default::default();
    for _ in 0..rounds {
        for (a, col) in cols.iter_mut().enumerate() {
            col.clear();
            col.extend(todo.iter().enumerate().map(|(k, &e)| lerp(mesh, e, 0.5 * (ta[k] + tb[k]), a)));
        }
        let v = inside([&cols[0], &cols[1], &cols[2]]);
        for k in 0..todo.len() {
            let t = 0.5 * (ta[k] + tb[k]);
            if v[k] { ta[k] = t } else { tb[k] = t }
        }
    }
    for (k, &e) in todo.iter().enumerate() {
        let p = [0, 1, 2].map(|a| lerp(mesh, e, ta[k], a));
        let id = mesh.push_vertex(p);
        vertex_cell.push(vertex_cell[e.0 as usize]);
        edge_pt.insert(e, id);
    }
    let mut out: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    for t in mesh.indices.chunks_exact(3) {
        let bad = t.iter().filter(|&&v| !ins[v as usize]).count();
        if bad == 0 {
            out.extend_from_slice(t);
            continue;
        }
        if bad == 3 {
            continue;
        }
        let mut poly: Vec<u32> = Vec::with_capacity(4);
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            let (ia, ib) = (ins[a as usize], ins[b as usize]);
            if ia {
                poly.push(a);
            }
            if ia != ib {
                poly.push(if ia { edge_pt[&(a, b)] } else { edge_pt[&(b, a)] });
            }
        }
        for k in 1..poly.len().saturating_sub(1) {
            out.extend_from_slice(&[poly[0], poly[k], poly[k + 1]]);
        }
    }
    mesh.indices = out;
}

/// 陰関数のメッシュの三角形の数の上限。超えたら、F の格子を間引いた版を本体にする
const MAX_IMPLICIT_TRIANGLES: usize = 1_500_000;
/// 縁での値が ±∞ のとき、代わりに使う大きな値（符号は保つ）
const FILL_BIG: f64 = 1e30;

/// [`extend_undefined`] の結果
#[derive(Default)]
struct Extended {
    /// 縁での値で埋めた格子点（格子点の番号 i + m (j + m k)）
    filled: crate::adaptive::HashSet<usize>,
    /// 片方の端が埋めた点の辺のうち、定義される部分（有限の端から縁まで）に本当の零点があるもの:
    /// 辺の番号（3 × 始点 + 向き）→ 交わる位置（始点からの割合）
    edge_t: crate::adaptive::HashMap<usize, f32>,
}

/// F が有限でない（NaN・±∞ の）格子点 `nan_pts` のうち、軸の向きの隣（6 つ）に定義される点があり、
/// 反対側の隣も有限でないもの（定義されない所の端）を、
/// 縁での値で埋める。辺の上で「定義される側の端」を二分で 10 回探し、そこでの F の値（縁に近づいたときの値）を取り、
/// 隣が複数あれば平均する（±∞ は符号を保って [`FILL_BIG`] にする）。埋めるのは 1 層だけ。
/// 埋めた点と定義される隣を結ぶ辺で符号が変わり、しかも定義される部分（隣から縁まで）にも本当の零点があるなら、
/// それを二分で求めて交わる位置にする。無ければ、ふつうの直線の補間の位置（定義されない側に出る。後で切り落とす）
fn extend_undefined(
    prog: &Program,
    combos: &[Vec<f64>],
    field: &mut SparseField,
    dom: Domain,
    n: usize,
    nan_pts: &[usize],
) -> Extended {
    let m = n + 1;
    let coords = |p: usize| [p % m, (p / m) % m, p / (m * m)];
    let pt = |q: [usize; 3]| [0, 1, 2].map(|x| dom.at(x, q[x] as f64, n));
    // (埋める点, 定義される隣, 向き)
    let mut pairs: Vec<(usize, [usize; 3], [usize; 3], usize)> = Vec::new();
    // 有限でない点の集まり（ページの中の、計算していない点の NaN と区別する）
    let bad: crate::adaptive::HashSet<usize> = nan_pts.iter().copied().collect();
    let idx = |q: [usize; 3]| q[0] + m * (q[1] + m * q[2]);
    for &p in nan_pts {
        let q = coords(p);
        // 隣 r が有限で、反対側の隣も有限でない（または格子の外）ときだけ、r から埋める（q が、その直線の上で定義されない所の端）。
        // 1 層だけ定義されない所（1/x の x = 0 などの極。両側で定義される）は埋めない（埋めると、極に偽の面ができる）
        for a in 0..3 {
            for up in [false, true] {
                if (!up && q[a] == 0) || (up && q[a] + 1 >= m) {
                    continue;
                }
                let mut r = q;
                if up { r[a] += 1 } else { r[a] -= 1 }
                if !field.f(r).is_finite() {
                    continue;
                }
                let opposite_bad = if up { q[a] == 0 } else { q[a] + 1 >= m } || {
                    let mut o = q;
                    if up { o[a] -= 1 } else { o[a] += 1 }
                    bad.contains(&idx(o))
                };
                if opposite_bad {
                    pairs.push((p, q, r, a));
                }
            }
        }
    }
    let mut ext = Extended::default();
    if pairs.is_empty() {
        return ext;
    }
    let mut sc = FieldScratch::default();
    let np = pairs.len();
    // 辺の上の位置 t（定義される隣 r が 0、埋める点 q が 1）
    let at = |k: usize, t: f64, ax: usize| {
        let (_, q, r, _) = pairs[k];
        let (a, b) = (pt(r)[ax], pt(q)[ax]);
        a + (b - a) * t
    };
    let (mut ta, mut tb) = (vec![0.0f64; np], vec![1.0f64; np]);
    let fr: Vec<f64> = pairs.iter().map(|&(_, _, r, _)| field.f(r) as f64).collect();
    let mut fa = fr.clone();
    let mut cols: [Vec<f64>; 3] = Default::default();
    for _ in 0..10 {
        for (ax, col) in cols.iter_mut().enumerate() {
            col.clear();
            col.extend((0..np).map(|k| at(k, 0.5 * (ta[k] + tb[k]), ax)));
        }
        let v = field_values_with(prog, combos, [&cols[0], &cols[1], &cols[2]], &mut sc);
        for k in 0..np {
            let t = 0.5 * (ta[k] + tb[k]);
            if v[k].is_nan() {
                tb[k] = t;
            } else {
                ta[k] = t;
                fa[k] = v[k];
            }
        }
    }
    let fa: Vec<f64> = fa.iter().map(|&v| if v.is_infinite() { FILL_BIG.copysign(v) } else { v }).collect();
    // 埋める点ごとに平均する
    let mut sum: crate::adaptive::HashMap<usize, (f64, f64)> = Default::default();
    for (k, &(p, ..)) in pairs.iter().enumerate() {
        let e = sum.entry(p).or_default();
        e.0 += fa[k];
        e.1 += 1.0;
    }
    for (&p, &(s, c)) in &sum {
        let slot = field.at(coords(p));
        if slot != usize::MAX {
            field.vals[slot] = (s / c) as f32;
            ext.filled.insert(p);
        }
    }
    // 2 層目: 埋めた点と軸の向きで隣り合う、まだ有限でない点に、隣の埋めた値の平均を写す。
    // 縁が格子に対して斜めだと、縁をまたぐセルの角に 2 層目の点が入り、その角への辺で面が途切れて穴が空くので、それを防ぐ
    // （2 層目は定義域の外なので、面は 4' でいつも切り落とされる）
    let mut second: Vec<(usize, f32)> = Vec::new();
    for &p in nan_pts {
        if ext.filled.contains(&p) {
            continue;
        }
        let q = coords(p);
        let (mut s, mut c) = (0.0f64, 0.0f64);
        for a in 0..3 {
            for up in [false, true] {
                if (!up && q[a] == 0) || (up && q[a] + 1 >= m) {
                    continue;
                }
                let mut r = q;
                if up { r[a] += 1 } else { r[a] -= 1 }
                if ext.filled.contains(&idx(r)) {
                    s += field.f(r) as f64;
                    c += 1.0;
                }
            }
        }
        if c > 0.0 {
            second.push((p, (s / c) as f32));
        }
    }
    for (p, v) in second {
        let slot = field.at(coords(p));
        if slot != usize::MAX {
            field.vals[slot] = v;
            ext.filled.insert(p);
        }
    }
    // 符号が変わる辺のうち、定義される部分に本当の零点があるもの（隣の値と縁での値の符号が違う）を二分で求める
    let zero: Vec<usize> = (0..np)
        .filter(|&k| {
            let fq = field.f(pairs[k].1) as f64;
            (fr[k] < 0.0) != (fq < 0.0) && (fr[k] < 0.0) != (fa[k] < 0.0)
        })
        .collect();
    if zero.is_empty() {
        return ext;
    }
    let (mut lo, mut hi): (Vec<f64>, Vec<f64>) = (vec![0.0; zero.len()], zero.iter().map(|&k| ta[k]).collect());
    for _ in 0..12 {
        for (ax, col) in cols.iter_mut().enumerate() {
            col.clear();
            col.extend(zero.iter().enumerate().map(|(z, &k)| at(k, 0.5 * (lo[z] + hi[z]), ax)));
        }
        let v = field_values_with(prog, combos, [&cols[0], &cols[1], &cols[2]], &mut sc);
        for (z, &k) in zero.iter().enumerate() {
            let t = 0.5 * (lo[z] + hi[z]);
            if !v[z].is_nan() && (v[z] < 0.0) == (fr[k] < 0.0) {
                lo[z] = t;
            } else {
                hi[z] = t;
            }
        }
    }
    for (z, &k) in zero.iter().enumerate() {
        let (_, q, r, a) = pairs[k];
        let t = 0.5 * (lo[z] + hi[z]);
        // 辺の始点（座標の小さいほうの端）からの割合にする
        let (start, t) = if r[a] < q[a] { (r, t) } else { (q, 1.0 - t) };
        ext.edge_t.insert(3 * (start[0] + m * (start[1] + m * start[2])) + a, t as f32);
    }
    ext
}

/// 縁での値で埋めた点（`filled`）を角に持つセルの頂点で F を計算し、定義されない（NaN の）頂点を見つけて、
/// 定義される所だけを残す。3 頂点とも定義されない三角形は捨て、混ざる三角形は、定義される頂点と定義されない頂点を
/// 結ぶ辺の上で定義される側の端を二分で 8 回探し、そこまでの多角形に切る（辺の上に求めた点は、隣の三角形と共有する）。
/// 足した頂点の `vertex_cell` は、定義される側の頂点のセルにする
fn trim_undefined(
    prog: &Program,
    combos: &[Vec<f64>],
    mesh: &mut Mesh,
    vertex_cell: &mut Vec<u32>,
    n: usize,
    filled: &crate::adaptive::HashSet<usize>,
) {
    let m = n + 1;
    let nv = mesh.positions.len() / 3;
    let near: Vec<usize> = (0..nv)
        .filter(|&v| {
            let c = vertex_cell[v] as usize;
            let q = [c % n, (c / n) % n, c / (n * n)];
            CORNERS.iter().any(|d| filled.contains(&((q[0] + d[0]) + m * ((q[1] + d[1]) + m * (q[2] + d[2])))))
        })
        .collect();
    if near.is_empty() {
        return;
    }
    let pos = |mesh: &Mesh, v: usize| [0, 1, 2].map(|a| mesh.positions[3 * v + a] as f64);
    let mut sc = FieldScratch::default();
    let mut undefined = vec![false; nv];
    {
        let cols: [Vec<f64>; 3] = [0, 1, 2].map(|a| near.iter().map(|&v| pos(mesh, v)[a]).collect());
        let vals = field_values_with(prog, combos, [&cols[0], &cols[1], &cols[2]], &mut sc);
        for (&v, &f) in near.iter().zip(vals) {
            undefined[v] = f.is_nan();
        }
    }
    if !undefined.iter().any(|&u| u) {
        return;
    }
    // 定義される頂点 → 定義されない頂点の辺の一覧
    let mut edge_pt: crate::adaptive::HashMap<(u32, u32), u32> = Default::default();
    let mut todo: Vec<(u32, u32)> = Vec::new();
    for t in mesh.indices.chunks_exact(3) {
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            for (d, u) in [(a, b), (b, a)] {
                if !undefined[d as usize] && undefined[u as usize] && !edge_pt.contains_key(&(d, u)) {
                    edge_pt.insert((d, u), NONE);
                    todo.push((d, u));
                }
            }
        }
    }
    let (mut ta, mut tb) = (vec![0.0f64; todo.len()], vec![1.0f64; todo.len()]);
    let lerp = |mesh: &Mesh, (d, u): (u32, u32), t: f64, a: usize| {
        let (p, q) = (pos(mesh, d as usize)[a], pos(mesh, u as usize)[a]);
        p + (q - p) * t
    };
    let mut cols: [Vec<f64>; 3] = Default::default();
    for _ in 0..8 {
        for (a, col) in cols.iter_mut().enumerate() {
            col.clear();
            col.extend(todo.iter().enumerate().map(|(k, &e)| lerp(mesh, e, 0.5 * (ta[k] + tb[k]), a)));
        }
        let v = field_values_with(prog, combos, [&cols[0], &cols[1], &cols[2]], &mut sc);
        for k in 0..todo.len() {
            let t = 0.5 * (ta[k] + tb[k]);
            if v[k].is_nan() { tb[k] = t } else { ta[k] = t }
        }
    }
    for (k, &e) in todo.iter().enumerate() {
        let p = [0, 1, 2].map(|a| lerp(mesh, e, ta[k], a));
        let id = mesh.push_vertex(p);
        vertex_cell.push(vertex_cell[e.0 as usize]);
        edge_pt.insert(e, id);
    }
    let mut out: Vec<u32> = Vec::with_capacity(mesh.indices.len());
    for t in mesh.indices.chunks_exact(3) {
        let bad = t.iter().filter(|&&v| undefined[v as usize]).count();
        if bad == 0 {
            out.extend_from_slice(t);
            continue;
        }
        if bad == 3 {
            continue;
        }
        let mut poly: Vec<u32> = Vec::with_capacity(4);
        for e in 0..3 {
            let (a, b) = (t[e], t[(e + 1) % 3]);
            let (ua, ub) = (undefined[a as usize], undefined[b as usize]);
            if !ua {
                poly.push(a);
            }
            if ua != ub {
                poly.push(if ua { edge_pt[&(b, a)] } else { edge_pt[&(a, b)] });
            }
        }
        for k in 1..poly.len().saturating_sub(1) {
            out.extend_from_slice(&[poly[0], poly[k], poly[k + 1]]);
        }
    }
    mesh.indices = out;
}

/// 頂点でつながった成分のうち、三角形が 12 個以下で、大きさがどの向きにも 2 マス以下で、
/// 「零点ではない」とされた辺（`not_zero`）か、定義されない・埋めた角（`filled`）を持つセルに頂点があるものを捨てる
/// （極・値が飛ぶ所・定義域の縁にできる紙吹雪のような破片）。`open_too` なら、縁の辺を持つ（閉じていない）成分も捨てる
/// （小さい図形の作り直しが、値が飛ぶ線の上に作る細かい切れ端）。それ以外の小さい成分（本当に小さい球など）は残す。
/// 捨てた成分の数を返す
fn drop_fragments(
    mesh: &mut Mesh,
    dom: Domain,
    n: usize,
    field: &SparseField,
    not_zero: &crate::adaptive::HashSet<usize>,
    filled: &crate::adaptive::HashSet<usize>,
    open_too: bool,
) -> usize {
    const MAX_TRIS: usize = 12;
    let m = n + 1;
    let nv = mesh.positions.len() / 3;
    let mut parent: Vec<u32> = (0..nv as u32).collect();
    fn root(p: &mut [u32], mut x: u32) -> u32 {
        while p[x as usize] != x {
            p[x as usize] = p[p[x as usize] as usize];
            x = p[x as usize];
        }
        x
    }
    for t in mesh.indices.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2])] {
            let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
            if ra != rb {
                parent[ra as usize] = rb;
            }
        }
    }
    let mut tris = vec![0u32; nv];
    for t in mesh.indices.chunks_exact(3) {
        let r = root(&mut parent, t[0]) as usize;
        tris[r] = tris[r].saturating_add(1);
    }
    let small: Vec<bool> = (0..nv as u32)
        .map(|v| {
            let r = root(&mut parent, v) as usize;
            tris[r] > 0 && tris[r] as usize <= MAX_TRIS
        })
        .collect();
    if !small.iter().any(|&s| s) {
        return 0;
    }
    // 小さい成分ごとの、頂点を囲む箱と「怪しいセルに頂点がある」
    let cell_size: [f64; 3] = [0, 1, 2].map(|a| (dom.hi[a] - dom.lo[a]) / n as f64);
    let idx = |q: [usize; 3]| q[0] + m * (q[1] + m * q[2]);
    let suspect_cell = |c: usize| {
        let q = [c % n, (c / n) % n, c / (n * n)];
        let corner_bad = CORNERS.iter().any(|d| {
            let p = [q[0] + d[0], q[1] + d[1], q[2] + d[2]];
            !field.f(p).is_finite() || (!filled.is_empty() && filled.contains(&idx(p)))
        });
        corner_bad
            || (!not_zero.is_empty()
                && EDGES.iter().any(|&[ca, cb]| {
                    let a = (0..3).find(|&x| CORNERS[ca][x] != CORNERS[cb][x]).unwrap_or(0);
                    let s =if CORNERS[ca][a] == 0 { CORNERS[ca] } else { CORNERS[cb] };
                    not_zero.contains(&(3 * idx([q[0] + s[0], q[1] + s[1], q[2] + s[2]]) + a))
                }))
    };
    // 頂点の位置から、頂点のあるセル（細かく作り直した頂点にも使えるように、位置から求める）
    let cell_of = |v: usize| {
        let q = [0, 1, 2].map(|a| {
            let t = (mesh.positions[3 * v + a] as f64 - dom.lo[a]) / cell_size[a];
            (t.floor().max(0.0) as usize).min(n - 1)
        });
        q[0] + n * (q[1] + n * q[2])
    };
    let mut info: crate::adaptive::HashMap<u32, ([f64; 3], [f64; 3], bool)> = Default::default();
    for v in (0..nv).filter(|&v| small[v]) {
        let r = root(&mut parent, v as u32);
        let e = info.entry(r).or_insert(([f64::INFINITY; 3], [f64::NEG_INFINITY; 3], false));
        for a in 0..3 {
            let x = mesh.positions[3 * v + a] as f64;
            e.0[a] = e.0[a].min(x);
            e.1[a] = e.1[a].max(x);
        }
        if !e.2 && suspect_cell(cell_of(v)) {
            e.2 = true;
        }
    }
    // `open_too` なら、縁の辺（三角形が 1 つだけの辺）を持つ成分も怪しいとする（閉じた小さい球・輪は残る）
    if open_too {
        let mut count: crate::adaptive::HashMap<(u32, u32), u32> = Default::default();
        for t in mesh.indices.chunks_exact(3) {
            if !small[t[0] as usize] {
                continue;
            }
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        for (&(a, _), &c) in &count {
            if c == 1 {
                let r = root(&mut parent, a);
                if let Some(e) = info.get_mut(&r) {
                    e.2 = true;
                }
            }
        }
    }
    let drop: crate::adaptive::HashSet<u32> = info
        .iter()
        .filter(|(_, (lo, hi, sus))| *sus && (0..3).all(|a| hi[a] - lo[a] <= 2.0 * cell_size[a] + 1e-9))
        .map(|(&r, _)| r)
        .collect();
    if drop.is_empty() {
        return 0;
    }
    let mut out = Vec::with_capacity(mesh.indices.len());
    for t in mesh.indices.chunks_exact(3) {
        if !drop.contains(&root(&mut parent, t[0])) {
            out.extend_from_slice(t);
        }
    }
    mesh.indices = out;
    drop.len()
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
    not_zero: &crate::adaptive::HashSet<usize>,
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
    nets_on_grid([nc; 3], &cell_ok, &|c| vals[at(c)], &cross, &pos)
}

/// Surface Nets（`dims` マスの格子）。`cell_ok` が偽のセルは見ない。`val(角)` は角の F の値（負かどうかで符号を決める）、
/// `cross(p, a)` は辺 p → p + e_a の交わる位置と始点の符号（負なら true）。`pos` は格子の座標から空間の座標へ。
/// 1 つのセルを 2 枚以上の曲面が通るときは、細かい版と同じく、少ないほうの符号の角の組ごとに頂点を分ける
/// （分けないと、鞍の形の面で 4 つの三角形が 1 本の辺に集まる）
fn nets_on_grid(
    dims: [usize; 3],
    cell_ok: &dyn Fn([usize; 3]) -> bool,
    val: &dyn Fn([usize; 3]) -> f32,
    cross: &dyn Fn([usize; 3], usize) -> Option<(f64, bool)>,
    pos: &dyn Fn([f64; 3]) -> [f64; 3],
) -> Mesh {
    let cell = |c: [usize; 3]| c[0] + dims[0] * (c[1] + dims[1] * c[2]);
    let edge_axis: [usize; 12] = EDGES.map(|[ca, cb]| (0..3).find(|&x| CORNERS[ca][x] != CORNERS[cb][x]).unwrap_or(0));
    let mut vert = vec![NONE; dims[0] * dims[1] * dims[2]];
    let mut slots: Vec<[u32; 12]> = Vec::new();
    let mut cells: Vec<[usize; 3]> = Vec::new();
    let mut mesh = Mesh::default();
    for z in 0..dims[2] {
        for y in 0..dims[1] {
            for x in 0..dims[0] {
                let c = [x, y, z];
                if !cell_ok(c) {
                    continue;
                }
                let corner = |d: [usize; 3]| [x + d[0], y + d[1], z + d[2]];
                let corner_val: [f32; 8] = std::array::from_fn(|i| val(corner(CORNERS[i])));
                let corner_neg: [bool; 8] = corner_val.map(|v| v < 0.0);
                let count = corner_neg.iter().filter(|&&b| b).count();
                if count == 0 || count == 8 {
                    continue;
                }
                let far = |f: usize, side: usize| -> Option<[f32; 4]> {
                    if (side == 0 && c[f] == 0) || (side == 1 && c[f] + 2 > dims[f]) {
                        return None;
                    }
                    let (g, h) = ((f + 1) % 3, (f + 2) % 3);
                    Some([(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dg, dh)| {
                        let mut p = c;
                        p[f] = if side == 0 { c[f] - 1 } else { c[f] + 2 };
                        p[g] += dg;
                        p[h] += dh;
                        val(p)
                    }))
                };
                let (group, groups, minority) = corner_groups(&corner_val, &far);
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

/// 1 つの成分のブロックの数の上限（超えたら、その成分は細かくしない）
const REFINE_MAX_COMPONENT_BLOCKS: usize = 64;
/// 細かく作り直すブロックの数の合計の上限（細かさを上げてやり直した分も数える）
const REFINE_MAX_BLOCKS: usize = 512;
/// 細かく作り直して足す三角形の数の合計の上限
const REFINE_MAX_TRIANGLES: usize = 150_000;
/// 「曲面の無い所」を確かめる区間演算の回数の上限（ふつう。念入りなら上限なし）
const REFINE_INTERVAL_BUDGET: usize = 200_000;
/// 細かい格子で F を計算する点の数の合計の上限（ふつう / 念入り）
const REFINE_POINT_BUDGET: [usize; 2] = [4_000_000, 32_000_000];
/// 1 つの成分の細かい格子（成分を囲む直方体）の格子点の数の上限
const REFINE_MAX_GRID: usize = 8_000_000;
/// 辺でつながる頂点どうしの法線のなす角がこれ（35°）を超えたら「角ばっている」。値は cos 35°
const REFINE_COS: f32 = 0.819;

/// 陰関数の 4: 格子より小さい図形（小さい球、細い輪、細い管）が消える・角ばるのを直す。
///
/// 1. 疑わしいブロックを集める
///    - (a) 区間演算が使える式: 「F = 0 があるかもしれない」のに頂点が 1 つも無いブロック。
///      箱を 8 等分して区間演算をやり直すことを 3 段まで繰り返し、どの小箱も 0 を含まなくなったら外す
///    - (b) 区間演算が使えない式: 周り（±2 点）の符号が全部同じなのに、|F| < 0.87 |∇F| h の格子点があるブロック
///      （∇F は格子の値の中心差分）。見落としはあり得る
///    - (c) 曲面はあるが角ばっているブロック: 辺でつながる頂点どうしの法線のなす角が 35° を超える
/// 2. 疑わしいブロックを、面・辺・角で接するものどうし、および同じメッシュの成分でつながるものどうしで、成分に分ける
///    （大きい曲面につながったものは、ブロックが多すぎて 4 の上限で諦める）
/// 3. 小さい成分から順に、ブロックを 2 倍の細かさで作り直す（[`refine_blocks`]）。成分の外側の面の上に
///    符号の変わる辺があれば、その向こうのブロックを成分に足してやり直す。作り直した形が、まだ空か角ばっていれば
///    4 倍（念入りなら 8 倍まで）でもう一度
/// 4. 成分の中の元の三角形を捨て、細かい三角形に差し替える。成分の外側の面を曲面が横切らないので、
///    継ぎ目は無い（元の三角形が成分の内と外にまたがっていたら、念のため差し替えない）
#[allow(clippy::too_many_arguments)]
fn refine_components(
    prog: &Program,
    combos: &[Vec<f64>],
    dom: Domain,
    n: usize,
    field: &SparseField,
    mesh: &mut Mesh,
    vertex_cell: &[u32],
    refine: Refine,
) {
    let thorough = refine == Refine::Thorough;
    let light = refine == Refine::Light;
    // 元の格子の 1 マスの大きさ（いちばん大きい向き）
    let cell_size = (0..3).map(|a| (dom.hi[a] - dom.lo[a]) / n as f64).fold(0.0, f64::max);
    use crate::adaptive::{HashMap, HashSet};
    let nb = field.nb;
    let bid = |b: [usize; 3]| b[0] + nb * (b[1] + nb * b[2]);
    let unbid = |id: usize| [id % nb, (id / nb) % nb, id / (nb * nb)];
    let block_of_cell = |c: usize| bid([c % n, (c / n) % n, c / (n * n)].map(|x| (x / BLOCK).min(nb - 1)));
    let vblock: Vec<usize> = vertex_cell.iter().map(|&c| block_of_cell(c as usize)).collect();
    let mut has_vert = vec![false; nb * nb * nb];
    for &b in &vblock {
        has_vert[b] = true;
    }
    let mut suspect = vec![false; nb * nb * nb];

    // (b) の判定: 格子点 q で |F| < 0.87 |∇F| h（∇F は中心差分）なのに、周り（±2 点）の符号が全部同じ
    let m = n + 1;
    let near_zero = |q: [usize; 3]| -> bool {
        let f = |q: [usize; 3]| field.f(q);
        if q.iter().any(|&x| x == 0 || x + 1 >= m) {
            return false;
        }
        let v = f(q);
        if !v.is_finite() {
            return false;
        }
        let mut g2 = 0.0f32;
        for a in 0..3 {
            let (mut lo, mut hi) = (q, q);
            lo[a] -= 1;
            hi[a] += 1;
            let d = 0.5 * (f(hi) - f(lo));
            g2 += d * d;
        }
        if g2.is_nan() || v * v >= 0.87 * 0.87 * g2 {
            return false;
        }
        let neg = v < 0.0;
        let r = |x: usize| x.saturating_sub(2)..=(x + 2).min(m - 1);
        r(q[2]).all(|k| r(q[1]).all(|j| r(q[0]).all(|i| f([i, j, k]).is_nan() || (f([i, j, k]) < 0.0) == neg)))
    };
    // 1 (a), (b)
    if prog.supports_interval() {
        let mut budget = if thorough {
            usize::MAX
        } else if light {
            REFINE_INTERVAL_BUDGET / 10
        } else {
            REFINE_INTERVAL_BUDGET
        };
        for id in 0..nb * nb * nb {
            if !field.occupied[id] || has_vert[id] {
                continue;
            }
            let b = unbid(id);
            let lo = [0, 1, 2].map(|a| dom.at(a, (BLOCK * b[a]) as f64, n));
            let hi = [0, 1, 2].map(|a| dom.at(a, (BLOCK * (b[a] + 1)).min(n) as f64, n));
            if !zero_in_box(prog, lo, hi, 3, &mut budget) {
                continue;
            }
            // 曲面のあるブロックの隣では、区間演算の広がりすぎ（曲面がすぐそばを通るだけ）がよく起きるので、
            // 格子点の値にも (b) の兆しがあるときだけ疑う（大きい曲面のすぐそばの小さい図形は見落とし得る）
            let by_surface = (0..27).any(|d| {
                let q = [0, 1, 2].map(|a| (b[a] + d / [1, 3, 9][a] % 3).wrapping_sub(1));
                q.iter().all(|&x| x < nb) && has_vert[bid(q)]
            });
            if by_surface {
                let span = |a: usize| BLOCK * b[a]..(BLOCK * (b[a] + 1)).min(m);
                let hint = span(2).any(|k| span(1).any(|j| span(0).any(|i| near_zero([i, j, k]))));
                if !hint {
                    continue;
                }
            }
            suspect[id] = true;
        }
    } else if field.dense {
        // 密な格子: 先に配列を直接読んで |F| < 0.87 |∇F| だけを安く確かめる（ほとんどの点はここで終わる）
        let v = &field.vals;
        let (sy, sz) = (m, m * m);
        for k in 1..m - 1 {
            for j in 1..m - 1 {
                for i in 1..m - 1 {
                    let l = i + sy * j + sz * k;
                    let f = v[l];
                    let (gx, gy, gz) = (v[l + 1] - v[l - 1], v[l + sy] - v[l - sy], v[l + sz] - v[l - sz]);
                    // 0.87² / 4 ≈ 0.19（中心差分の 1/2 を含める）
                    if (f * f).partial_cmp(&(0.19 * (gx * gx + gy * gy + gz * gz))) != Some(std::cmp::Ordering::Less) {
                        continue;
                    }
                    if near_zero([i, j, k]) {
                        suspect[bid([i, j, k].map(|x| (x / BLOCK).min(nb - 1)))] = true;
                    }
                }
            }
        }
    }
    // 1 (c)（軽く作り直すときは見ない）
    let nrm = &mesh.normals;
    let dotn = |a: u32, b: u32| {
        let (a, b) = (a as usize * 3, b as usize * 3);
        nrm[a] * nrm[b] + nrm[a + 1] * nrm[b + 1] + nrm[a + 2] * nrm[b + 2]
    };
    for t in mesh.indices.chunks_exact(3) {
        if light {
            break;
        }
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            if dotn(a, b) < REFINE_COS {
                suspect[vblock[a as usize]] = true;
                suspect[vblock[b as usize]] = true;
            }
        }
    }
    // 1 (d) 小さい成分: メッシュの成分を囲む箱が ZOOM_TARGET マスより小さい（頂点はあるが、粗い多面体になっている）
    let target = ZOOM_TARGET[if light { 0 } else if thorough { 2 } else { 1 }];
    for comp in mesh_components(mesh) {
        let (lo, hi) = vertex_box(mesh, comp.iter().copied());
        if (0..3).all(|a| hi[a] - lo[a] < target as f64 * cell_size) {
            for &v in &comp {
                suspect[vblock[v as usize]] = true;
            }
        }
    }
    if !suspect.iter().any(|&s| s) {
        return;
    }

    // 2. 成分に分ける（ブロックの union-find）
    let mut parent: Vec<usize> = (0..nb * nb * nb).collect();
    fn root(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let union = |p: &mut Vec<usize>, a: usize, b: usize| {
        let (ra, rb) = (root(p, a), root(p, b));
        if ra != rb {
            p[ra] = rb;
        }
    };
    for t in mesh.indices.chunks_exact(3) {
        let [a, b, c] = [0, 1, 2].map(|k| vblock[t[k] as usize]);
        union(&mut parent, a, b);
        union(&mut parent, a, c);
    }
    for id in 0..nb * nb * nb {
        if !suspect[id] {
            continue;
        }
        let b = unbid(id);
        for dz in 0..3 {
            for dy in 0..3 {
                for dx in 0..3 {
                    let d = [dx, dy, dz];
                    let q = [0, 1, 2].map(|a| (b[a] + d[a]).wrapping_sub(1));
                    if q.iter().all(|&x| x < nb) && suspect[bid(q)] {
                        union(&mut parent, id, bid(q));
                    }
                }
            }
        }
    }
    let mut groups: HashMap<usize, (Vec<usize>, bool)> = HashMap::default();
    for id in 0..nb * nb * nb {
        if suspect[id] || has_vert[id] {
            let g = groups.entry(root(&mut parent, id)).or_default();
            g.0.push(id);
            g.1 |= suspect[id];
        }
    }
    let mut groups: Vec<Vec<usize>> = groups.into_values().filter(|(_, s)| *s).map(|(b, _)| b).collect();
    // 大きい成分は、図形の大きさに合わせて作り直すときだけ使う（[`refine_blocks`] は 64 ブロックを超えると諦める）
    groups.retain(|g| g.len() <= REFINE_MAX_BLOCKS);
    if groups.is_empty() {
        return;
    }
    groups.sort_by_key(|g| (g.len(), g[0]));

    // 3. 小さい成分から順に作り直す
    let mut budget = RefineBudget {
        blocks: REFINE_MAX_BLOCKS,
        points: REFINE_POINT_BUDGET[thorough as usize],
    };
    let max_r = if thorough { 8 } else { 4 };
    let mut zoom_budget = ZOOM_POINT_BUDGET[if light { 0 } else if thorough { 2 } else { 1 }];
    // ブロックごとの、頂点を持つ三角形（差し替えるときに、内と外にまたがらないかを見る）
    let mut block_tris: HashMap<usize, Vec<u32>> = HashMap::default();
    for (ti, t) in mesh.indices.chunks_exact(3).enumerate() {
        let bs = [0, 1, 2].map(|k| vblock[t[k] as usize]);
        for (k, &b) in bs.iter().enumerate() {
            if !bs[..k].contains(&b) {
                block_tris.entry(b).or_default().push(ti as u32);
            }
        }
    }
    let mut replaced: HashSet<usize> = HashSet::default();
    let mut drop_tri = vec![false; mesh.indices.len() / 3];
    let mut parts: Vec<Mesh> = Vec::new();
    let mut added = 0usize;
    for group in groups {
        let mut blocks: HashSet<usize> = group.into_iter().collect();
        // 小さい成分は、図形を囲む箱に合わせた細かい格子で作り直す（図形の大きさによらず同じ細かさになる）
        let mut best: Option<(Mesh, HashSet<usize>)> = None;
        if let Some((fine, more)) = zoom_component(
            prog, combos, dom, n, nb, &blocks, &has_vert, mesh, &vblock, cell_size, target, &mut zoom_budget,
        ) {
            blocks.extend(more);
            best = Some((fine, blocks.clone()));
        }
        let mut r = if best.is_some() || light { max_r + 1 } else { 2 };
        while r <= max_r {
            let Some(fine) = refine_blocks(prog, combos, dom, n, field, &mut blocks, r, &mut budget) else { break };
            let done = !fine.indices.is_empty() && !is_angular(&fine);
            if !fine.indices.is_empty() {
                best = Some((fine, blocks.clone()));
            }
            if done {
                break;
            }
            r *= 2;
        }
        let Some((fine, blocks)) = best else { continue };
        if blocks.iter().any(|b| replaced.contains(b)) || added + fine.indices.len() / 3 > REFINE_MAX_TRIANGLES {
            continue;
        }
        // 元の三角形のうち、成分の中のもの。内と外にまたがるものがあれば差し替えない
        let mut inside: Vec<u32> = Vec::new();
        let mut mixed = false;
        for b in &blocks {
            for &ti in block_tris.get(b).map(|v| v.as_slice()).unwrap_or(&[]) {
                let t = &mesh.indices[3 * ti as usize..3 * ti as usize + 3];
                if t.iter().all(|&v| blocks.contains(&vblock[v as usize])) {
                    inside.push(ti);
                } else {
                    mixed = true;
                }
            }
        }
        if mixed {
            continue;
        }
        for ti in inside {
            drop_tri[ti as usize] = true;
        }
        added += fine.indices.len() / 3;
        replaced.extend(blocks);
        parts.push(fine);
    }
    if parts.is_empty() {
        return;
    }

    // 4. 差し替える（使われなくなった頂点は除く）
    let old = std::mem::take(mesh);
    let mut remap = vec![NONE; old.positions.len() / 3];
    let mut out = Mesh { stats: old.stats, ..Mesh::default() };
    for (ti, t) in old.indices.chunks_exact(3).enumerate() {
        if drop_tri[ti] {
            continue;
        }
        for &v in t {
            let v = v as usize;
            if remap[v] == NONE {
                remap[v] = (out.positions.len() / 3) as u32;
                out.positions.extend_from_slice(&old.positions[3 * v..3 * v + 3]);
            }
            out.indices.push(remap[v]);
        }
    }
    for part in parts {
        let base = (out.positions.len() / 3) as u32;
        out.positions.extend_from_slice(&part.positions);
        out.indices.extend(part.indices.iter().map(|&v| v + base));
    }
    compute_normals(&mut out);
    *mesh = out;
}

/// 箱 [lo, hi] の上で F = 0 になるかもしれないか（区間演算）。`depth` 段まで 8 等分して確かめ、どこかの小箱が
/// 0 を含むかもしれなければ true。区間演算の回数が `budget` を使い切ったら false（疑わしいままにしない）
fn zero_in_box(prog: &Program, lo: [f64; 3], hi: [f64; 3], depth: usize, budget: &mut usize) -> bool {
    use crate::interval::Interval;
    if *budget == 0 {
        return false;
    }
    *budget -= 1;
    let boxes: Vec<Interval> = (0..3).map(|a| Interval::new(lo[a].min(hi[a]), lo[a].max(hi[a]))).collect();
    if !prog.eval_interval(&boxes).may_contain_zero() {
        return false;
    }
    if depth == 0 {
        return true;
    }
    let mid = [0, 1, 2].map(|a| 0.5 * (lo[a] + hi[a]));
    CORNERS.iter().any(|c| {
        let l = [0, 1, 2].map(|a| if c[a] == 0 { lo[a] } else { mid[a] });
        let h = [0, 1, 2].map(|a| if c[a] == 0 { mid[a] } else { hi[a] });
        zero_in_box(prog, l, h, depth - 1, budget)
    })
}

/// 図形を囲む箱のいちばん長い辺を何マスに分けて作り直すか（軽く / ふつう / 念入り）。箱が元の格子でこのマス数より
/// 小さい成分を作り直す（[`zoom_component`]）。境目で細かさがそろうので、半径を動かしても途中で急に粗くならない
const ZOOM_TARGET: [usize; 3] = [24, 40, 64];
/// 図形の大きさに合わせた格子で F を計算する点の数の合計の上限（軽く / ふつう / 念入り）
const ZOOM_POINT_BUDGET: [usize; 3] = [300_000, 2_000_000, 16_000_000];
/// 図形を囲む箱を区間演算で狭めるときの、区間演算の回数の上限（1 つの成分あたり）
const ZOOM_INTERVAL_BUDGET: usize = 20_000;
/// 図形の大きさに合わせた格子で、区間演算で「曲面が無い」かを確かめる塊の 1 辺のマス数（[`local_nets`]）
const ZOOM_BRICK: usize = 4;

/// メッシュの成分（三角形でつながる頂点の組）
fn mesh_components(mesh: &Mesh) -> Vec<Vec<u32>> {
    let nv = mesh.positions.len() / 3;
    let mut parent: Vec<u32> = (0..nv as u32).collect();
    fn root(p: &mut [u32], mut x: u32) -> u32 {
        while p[x as usize] != x {
            p[x as usize] = p[p[x as usize] as usize];
            x = p[x as usize];
        }
        x
    }
    for t in mesh.indices.chunks_exact(3) {
        for (a, b) in [(t[0], t[1]), (t[1], t[2])] {
            let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
            if ra != rb {
                parent[ra as usize] = rb;
            }
        }
    }
    let mut groups: crate::adaptive::HashMap<u32, Vec<u32>> = crate::adaptive::HashMap::default();
    let mut used = vec![false; nv];
    for &v in &mesh.indices {
        used[v as usize] = true;
    }
    for v in 0..nv as u32 {
        if used[v as usize] {
            let r = root(&mut parent, v);
            groups.entry(r).or_default().push(v);
        }
    }
    groups.into_values().collect()
}

/// 頂点の組を囲む箱
fn vertex_box(mesh: &Mesh, verts: impl Iterator<Item = u32>) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for v in verts {
        for a in 0..3 {
            let x = mesh.positions[3 * v as usize + a] as f64;
            lo[a] = lo[a].min(x);
            hi[a] = hi[a].max(x);
        }
    }
    (lo, hi)
}

/// 箱 [lo, hi] の中で F = 0 になるかもしれない所を囲む箱（区間演算で 8 等分を繰り返して狭める）。
/// 0 にならないと分かれば None。箱の大きさが、残った小箱を囲む箱の 1/8 になるか、小箱が多くなりすぎたら止める
fn tight_zero_box(
    prog: &Program,
    lo: [f64; 3],
    hi: [f64; 3],
    budget: &mut usize,
) -> Option<([f64; 3], [f64; 3])> {
    use crate::interval::Interval;
    let may = |l: [f64; 3], h: [f64; 3], budget: &mut usize| {
        if *budget == 0 {
            return true;
        }
        *budget -= 1;
        let boxes: Vec<Interval> = (0..3).map(|a| Interval::new(l[a], h[a])).collect();
        prog.eval_interval(&boxes).may_contain_zero()
    };
    if !may(lo, hi, budget) {
        return None;
    }
    let mut level = vec![(lo, hi)];
    let bound = |level: &[([f64; 3], [f64; 3])]| {
        let mut b = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for (l, h) in level {
            for a in 0..3 {
                b.0[a] = b.0[a].min(l[a]);
                b.1[a] = b.1[a].max(h[a]);
            }
        }
        b
    };
    for _ in 0..64 {
        let (bl, bh) = bound(&level);
        let ext = (0..3).map(|a| bh[a] - bl[a]).fold(0.0, f64::max);
        let (l0, h0) = level[0];
        let size = (0..3).map(|a| h0[a] - l0[a]).fold(0.0, f64::max);
        if level.len() > 512 || *budget == 0 || size * 8.0 < ext || size <= ext * 1e-12 {
            break;
        }
        let mut next = Vec::new();
        for (l, h) in &level {
            let mid = [0, 1, 2].map(|a| 0.5 * (l[a] + h[a]));
            for c in CORNERS.iter() {
                let cl = [0, 1, 2].map(|a| if c[a] == 0 { l[a] } else { mid[a] });
                let ch = [0, 1, 2].map(|a| if c[a] == 0 { mid[a] } else { h[a] });
                if may(cl, ch, budget) {
                    next.push((cl, ch));
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        level = next;
    }
    Some(bound(&level))
}

/// 箱 [lo, lo + dims·s] の、1 辺 s の格子で Surface Nets を走らせる。箱の外側の面の上で符号が変わる辺があれば
/// （曲面が箱からはみ出すなら）、メッシュの代わりに Err を返す
fn local_nets(
    prog: &Program,
    combos: &[Vec<f64>],
    lo: [f64; 3],
    s: f64,
    dims: [usize; 3],
) -> Result<Mesh, ()> {
    let pts = dims.map(|d| d + 1);
    let len = pts[0] * pts[1] * pts[2];
    let at = |q: [usize; 3]| q[0] + pts[0] * (q[1] + pts[1] * q[2]);
    let unat = |l: usize| [l % pts[0], (l / pts[0]) % pts[1], l / (pts[0] * pts[1])];
    let point = |l: usize| {
        let q = unat(l);
        [0, 1, 2].map(|a| lo[a] + q[a] as f64 * s)
    };
    let mut vals = vec![f32::NAN; len];
    let mut sc = FieldScratch::default();
    // 区間演算が使える式では、1 辺 ZOOM_BRICK マスの塊ごとに「F = 0 が無い」かを確かめ、無い塊は角の 1 点だけ計算して
    // その値で埋める（符号が変わらないので、面は張られない。塊の外側の面も 0 を含まないので、隣の塊の辺も横切らない）
    let mut need = vec![!prog.supports_interval(); len];
    if prog.supports_interval() {
        use crate::interval::Interval;
        let nbk = dims.map(|d| d.div_ceil(ZOOM_BRICK));
        let mut fill: Vec<([usize; 3], [usize; 3])> = Vec::new();
        for bz in 0..nbk[2] {
            for by in 0..nbk[1] {
                for bx in 0..nbk[0] {
                    let q0 = [bx, by, bz].map(|b| b * ZOOM_BRICK);
                    let q1 = [0, 1, 2].map(|a| (q0[a] + ZOOM_BRICK).min(dims[a]));
                    let boxes: Vec<Interval> = (0..3)
                        .map(|a| Interval::new(lo[a] + q0[a] as f64 * s, lo[a] + q1[a] as f64 * s))
                        .collect();
                    if prog.eval_interval(&boxes).may_contain_zero() {
                        for z in q0[2]..=q1[2] {
                            for y in q0[1]..=q1[1] {
                                for x in q0[0]..=q1[0] {
                                    need[at([x, y, z])] = true;
                                }
                            }
                        }
                    } else {
                        fill.push((q0, q1));
                    }
                }
            }
        }
        if !fill.is_empty() {
            let cols: [Vec<f64>; 3] = [0, 1, 2].map(|a| {
                fill.iter()
                    .map(|(q0, _)| lo[a] + q0[a] as f64 * s)
                    .collect()
            });
            let v =
                field_values_with(prog, combos, [&cols[0], &cols[1], &cols[2]], &mut sc).to_vec();
            for ((q0, q1), x) in fill.iter().zip(v) {
                for z in q0[2]..=q1[2] {
                    for y in q0[1]..=q1[1] {
                        for xx in q0[0]..=q1[0] {
                            let l = at([xx, y, z]);
                            if !need[l] {
                                vals[l] = x as f32;
                            }
                        }
                    }
                }
            }
        }
    }
    let ids: Vec<usize> = (0..len).filter(|&l| need[l]).collect();
    for part in ids.chunks(FIELD_CHUNK) {
        let cols: [Vec<f64>; 3] = [0, 1, 2].map(|a| part.iter().map(|&l| point(l)[a]).collect());
        let v = field_values_with(prog, combos, [&cols[0], &cols[1], &cols[2]], &mut sc);
        for (&l, &x) in part.iter().zip(v) {
            vals[l] = x as f32;
        }
    }
    let changes = |l: usize, st: usize| {
        let (f0, f1) = (vals[l], vals[l + st]);
        f0.is_finite() && f1.is_finite() && (f0 < 0.0) != (f1 < 0.0)
    };
    // 符号の変わる辺のうち、F = 0 になる点が無いもの（極など）を除く（[`refine_blocks`] と同じ）
    let mut sign_edges: Vec<SignEdge> = Vec::new();
    let mut sign_ids: Vec<usize> = Vec::new();
    for l in 0..len {
        let p = unat(l);
        for a in 0..3 {
            if p[a] + 1 >= pts[a] {
                continue;
            }
            let st = [1, pts[0], pts[0] * pts[1]][a];
            if !changes(l, st) {
                continue;
            }
            let (f0, f1) = (vals[l], vals[l + st]);
            if p[a] >= 1 && p[a] + 2 < pts[a] {
                let (fl, fr) = (vals[l - st], vals[l + 2 * st]);
                let (sl, d, sr) = (f0 - fl, f1 - f0, fr - f1);
                if sl * d > 0.0 && sr * d > 0.0 && d.abs() <= 4.0 * sl.abs().max(sr.abs()) {
                    continue;
                }
            }
            sign_edges.push(SignEdge {
                p0: point(l),
                p1: point(l + st),
                f0: f0 as f64,
                f1: f1 as f64,
            });
            sign_ids.push(3 * l + a);
        }
    }
    let not_zero: crate::adaptive::HashSet<usize> = {
        let flags = false_crossings(
            &mut |xyz| field_values_with(prog, combos, xyz, &mut sc).to_vec(),
            &sign_edges,
        );
        sign_ids
            .into_iter()
            .zip(flags)
            .filter(|&(_, bad)| bad)
            .map(|(id, _)| id)
            .collect()
    };
    // 箱の外側の面の上で、曲面が横切る辺があるか
    for l in 0..len {
        let p = unat(l);
        for a in 0..3 {
            if p[a] + 1 >= pts[a] {
                continue;
            }
            let on_face = (0..3).any(|b| b != a && (p[b] == 0 || p[b] + 1 == pts[b]));
            if on_face
                && changes(l, [1, pts[0], pts[0] * pts[1]][a])
                && !not_zero.contains(&(3 * l + a))
            {
                return Err(());
            }
        }
    }
    let cross = |p: [usize; 3], a: usize| -> Option<(f64, bool)> {
        let mut q = p;
        q[a] += 1;
        let (f0, f1) = (vals[at(p)], vals[at(q)]);
        if !f0.is_finite()
            || !f1.is_finite()
            || (f0 < 0.0) == (f1 < 0.0)
            || not_zero.contains(&(3 * at(p) + a))
        {
            return None;
        }
        Some(((f0 / (f0 - f1)) as f64, f0 < 0.0))
    };
    let pos = |p: [f64; 3]| [0, 1, 2].map(|a| lo[a] + p[a] * s);
    Ok(nets_on_grid(
        dims,
        &|_| true,
        &|c| vals[at(c)],
        &cross,
        &pos,
    ))
}

/// 小さい成分（ブロックの集まり `group`）を、図形を囲む箱に合わせた格子（いちばん長い辺を `target` マス）で作り直す。
/// 囲む箱は、区間演算が使える式なら区間演算で狭め、使えない式なら成分の頂点から決める。作った形が箱よりずっと
/// 小さければ、形を囲む箱で作り直す（3 回まで）。曲面が箱からはみ出すなら箱を広げる。
/// 箱が、成分の外の「頂点のあるブロック」にかかるなら諦める（その曲面を二重に描かないように）。
/// 返すのは、作った形と、箱がかかる成分の外のブロック（頂点の無いもの。差し替えの印に足す）
#[allow(clippy::too_many_arguments)]
fn zoom_component(
    prog: &Program,
    combos: &[Vec<f64>],
    dom: Domain,
    n: usize,
    nb: usize,
    group: &crate::adaptive::HashSet<usize>,
    has_vert: &[bool],
    mesh: &Mesh,
    vblock: &[usize],
    cell_size: f64,
    target: usize,
    budget: &mut usize,
) -> Option<(Mesh, Vec<usize>)> {
    let unbid = |id: usize| [id % nb, (id / nb) % nb, id / (nb * nb)];
    let bid = |b: [usize; 3]| b[0] + nb * (b[1] + nb * b[2]);
    let block_lo = |b: [usize; 3]| [0, 1, 2].map(|a| dom.at(a, (BLOCK * b[a]) as f64, n));
    let block_hi =
        |b: [usize; 3]| [0, 1, 2].map(|a| dom.at(a, (BLOCK * (b[a] + 1)).min(n) as f64, n));
    let ext = |b: &([f64; 3], [f64; 3])| (0..3).map(|a| b.1[a] - b.0[a]).fold(0.0, f64::max);
    let join = |x: Option<([f64; 3], [f64; 3])>, y: ([f64; 3], [f64; 3])| {
        Some(match x {
            None => y,
            Some(x) => (
                [0, 1, 2].map(|a| x.0[a].min(y.0[a])),
                [0, 1, 2].map(|a| x.1[a].max(y.1[a])),
            ),
        })
    };
    let small = target as f64 * cell_size;
    // 成分の頂点を囲む箱（大きすぎれば、ここで諦める）
    let verts = (0..vblock.len() as u32).filter(|&v| group.contains(&vblock[v as usize]));
    let vbox = vertex_box(mesh, verts);
    let has_verts = vbox.0[0].is_finite();
    if has_verts && ext(&vbox) >= small {
        return None;
    }
    // 1. 図形を囲む箱
    let mut bx: Option<([f64; 3], [f64; 3])> = None;
    if prog.supports_interval() {
        let mut ib = ZOOM_INTERVAL_BUDGET;
        for &id in group {
            let b = unbid(id);
            if let Some(t) = tight_zero_box(prog, block_lo(b), block_hi(b), &mut ib) {
                bx = join(bx, t);
            }
        }
    } else {
        if has_verts {
            bx = Some(vbox);
        }
        if bx.is_none() {
            for &id in group {
                let b = unbid(id);
                bx = join(bx, (block_lo(b), block_hi(b)));
            }
        }
    }
    let mut bx = bx?;
    if ext(&bx) >= small {
        return None;
    }
    // 作った形が、元の頂点の広がりより明らかに小さければ使わない（0 に接するだけの所など、
    // 符号の変わらない部分は Surface Nets では作れないので、それを落とさないように）
    let covers = |m: &Mesh| {
        if !has_verts || ext(&vbox) < 4.0 * cell_size {
            return true;
        }
        let b = vertex_box(m, 0..(m.positions.len() / 3) as u32);
        (0..3).all(|a| b.1[a] - b.0[a] >= (vbox.1[a] - vbox.0[a]) - 2.5 * cell_size)
    };
    // 2. 箱に合わせた格子で作る
    let mut best: Option<(Mesh, Vec<usize>)> = None;
    for _ in 0..6 {
        let e = ext(&bx).max(cell_size * 1e-9);
        let s = e / target as f64;
        let lo = [0, 1, 2].map(|a| (bx.0[a] - 2.0 * s).max(dom.lo[a]));
        let hi = [0, 1, 2].map(|a| (bx.1[a] + 2.0 * s).min(dom.hi[a]));
        // 箱がかかるブロック。成分の外で頂点のあるものにかかるなら諦める
        let cell_of = |x: f64, a: usize| {
            ((x - dom.lo[a]) / (dom.hi[a] - dom.lo[a]) * n as f64)
                .floor()
                .clamp(0.0, (n - 1) as f64) as usize
        };
        let b0 = [0, 1, 2].map(|a| (cell_of(lo[a], a) / BLOCK).min(nb - 1));
        let b1 = [0, 1, 2].map(|a| (cell_of(hi[a], a) / BLOCK).min(nb - 1));
        let mut more = Vec::new();
        for z in b0[2]..=b1[2] {
            for y in b0[1]..=b1[1] {
                for x in b0[0]..=b1[0] {
                    let id = bid([x, y, z]);
                    if group.contains(&id) {
                        continue;
                    }
                    if has_vert[id] {
                        return best;
                    }
                    more.push(id);
                }
            }
        }
        let dims = [0, 1, 2].map(|a| (((hi[a] - lo[a]) / s).ceil() as usize).max(1));
        let count = (dims[0] + 1) * (dims[1] + 1) * (dims[2] + 1);
        if count > *budget {
            return best;
        }
        *budget -= count;
        match local_nets(prog, combos, lo, s, dims) {
            // 曲面が箱からはみ出す: 箱を広げてやり直す
            Err(()) => {
                let grow = 0.5 * e;
                bx = (
                    [0, 1, 2].map(|a| bx.0[a] - grow),
                    [0, 1, 2].map(|a| bx.1[a] + grow),
                );
                if ext(&bx) >= 2.0 * small {
                    return best;
                }
            }
            Ok(m) => {
                if m.indices.is_empty() || !covers(&m) {
                    return best;
                }
                let vb = vertex_box(&m, 0..(m.positions.len() / 3) as u32);
                let small = ext(&vb) < 0.6 * e;
                best = Some((m, more));
                if !small {
                    return best;
                }
                bx = vb;
            }
        }
    }
    best
}

/// 辺でつながる頂点どうしの法線のなす角が、どこかで 35° を超えるか
fn is_angular(mesh: &Mesh) -> bool {
    let nrm = &mesh.normals;
    let dotn = |a: u32, b: u32| {
        let (a, b) = (a as usize * 3, b as usize * 3);
        nrm[a] * nrm[b] + nrm[a + 1] * nrm[b + 1] + nrm[a + 2] * nrm[b + 2]
    };
    mesh.indices.chunks_exact(3).any(|t| dotn(t[0], t[1]) < REFINE_COS || dotn(t[1], t[2]) < REFINE_COS || dotn(t[2], t[0]) < REFINE_COS)
}

/// 細かく作り直す手間の残り（[`refine_components`]）
struct RefineBudget {
    blocks: usize,
    points: usize,
}

/// ブロックの集まり `blocks`（元の格子の 1 辺 BLOCK マスのブロックの番号）を、`r` 倍の細かさの格子で作り直す。
/// 成分の外側の面の上に符号の変わる辺があれば、その向こうのブロックを `blocks` に足してやり直す。
/// 上限（ブロックの数・点の数・格子の大きさ）を超えたら None
#[allow(clippy::too_many_arguments)]
fn refine_blocks(
    prog: &Program,
    combos: &[Vec<f64>],
    dom: Domain,
    n: usize,
    field: &SparseField,
    blocks: &mut crate::adaptive::HashSet<usize>,
    r: usize,
    budget: &mut RefineBudget,
) -> Option<Mesh> {
    let nb = field.nb;
    let unbid = |id: usize| [id % nb, (id / nb) % nb, id / (nb * nb)];
    let bid = |b: [usize; 3]| b[0] + nb * (b[1] + nb * b[2]);
    // ブロックの、元の格子のセルの範囲
    let span = |b: usize| (BLOCK * b, (BLOCK * (b + 1)).min(n));
    // 成分の外側の面（ブロック b の向き a・側 side の面。向こうのブロックが成分の外）の上の格子の辺で、
    // `changes(点, 向き)` が真のものがあれば、向こうのブロックを返す。`scale` は格子の細かさ（元の格子なら 1）、
    // `origin` は格子の原点（元の格子のセル）
    let outward = |blocks: &crate::adaptive::HashSet<usize>, scale: usize, origin: [usize; 3], changes: &dyn Fn([usize; 3], usize) -> bool| {
        let mut grow: Vec<usize> = Vec::new();
        for &id in blocks.iter() {
            let b = unbid(id);
            let fs = |a: usize| {
                let (s, e) = span(b[a]);
                ((s - origin[a]) * scale, (e - origin[a]) * scale)
            };
            for a in 0..3 {
                for side in 0..2 {
                    let Some(nbr) = (if side == 0 { b[a].checked_sub(1) } else { Some(b[a] + 1).filter(|&x| x < nb) }) else {
                        continue;
                    };
                    let mut q = b;
                    q[a] = nbr;
                    if blocks.contains(&bid(q)) || grow.contains(&bid(q)) {
                        continue;
                    }
                    let plane = if side == 0 { fs(a).0 } else { fs(a).1 };
                    let (u, v) = ((a + 1) % 3, (a + 2) % 3);
                    let ((us, ue), (vs, ve)) = (fs(u), fs(v));
                    let crossed = (us..=ue).any(|x| {
                        (vs..=ve).any(|y| {
                            let mut p = [0; 3];
                            p[a] = plane;
                            p[u] = x;
                            p[v] = y;
                            (x < ue && changes(p, u)) || (y < ve && changes(p, v))
                        })
                    });
                    if crossed {
                        grow.push(bid(q));
                    }
                }
            }
        }
        grow
    };
    // まず元の格子の値で、外側の面を曲面が横切らないかを見る（細かい格子で計算する前に、大きい曲面につながった成分を除く）
    loop {
        if blocks.len() > REFINE_MAX_COMPONENT_BLOCKS {
            return None;
        }
        let coarse = |p: [usize; 3], a: usize| {
            let mut q = p;
            q[a] += 1;
            let (f0, f1) = (field.f(p), field.f(q));
            f0.is_finite() && f1.is_finite() && (f0 < 0.0) != (f1 < 0.0)
        };
        let grow = outward(blocks, 1, [0; 3], &coarse);
        if grow.is_empty() {
            break;
        }
        blocks.extend(grow);
    }
    loop {
        if blocks.len() > REFINE_MAX_COMPONENT_BLOCKS || blocks.len() > budget.blocks {
            return None;
        }
        // 成分を囲む直方体（ブロックの番号の範囲 b0..b1）と、その細かい格子
        let mut b0 = [usize::MAX; 3];
        let mut b1 = [0; 3];
        for &id in blocks.iter() {
            let b = unbid(id);
            for a in 0..3 {
                b0[a] = b0[a].min(b[a]);
                b1[a] = b1[a].max(b[a] + 1);
            }
        }
        let c0 = [0, 1, 2].map(|a| span(b0[a]).0);
        let c1 = [0, 1, 2].map(|a| span(b1[a] - 1).1);
        let dims = [0, 1, 2].map(|a| (c1[a] - c0[a]) * r);
        let pts = dims.map(|d| d + 1);
        if pts[0] * pts[1] * pts[2] > REFINE_MAX_GRID {
            return None;
        }
        let at = |q: [usize; 3]| q[0] + pts[0] * (q[1] + pts[1] * q[2]);
        // ブロック b の、細かい格子点の範囲（両端を含む）
        let fine_span = |b: [usize; 3], a: usize| {
            let (s, e) = span(b[a]);
            ((s - c0[a]) * r, (e - c0[a]) * r)
        };
        // 直方体の中のブロックが成分に入っているか
        let bd = [0, 1, 2].map(|a| b1[a] - b0[a]);
        let mut inside = vec![false; bd[0] * bd[1] * bd[2]];
        for &id in blocks.iter() {
            let b = unbid(id);
            inside[(b[0] - b0[0]) + bd[0] * ((b[1] - b0[1]) + bd[1] * (b[2] - b0[2]))] = true;
        }
        // 計算する格子点
        let mut need = vec![false; pts[0] * pts[1] * pts[2]];
        for &id in blocks.iter() {
            let b = unbid(id);
            let [(x0, x1), (y0, y1), (z0, z1)] = [0, 1, 2].map(|a| fine_span(b, a));
            for z in z0..=z1 {
                for y in y0..=y1 {
                    let row = at([x0, y, z]);
                    need[row..=row + (x1 - x0)].fill(true);
                }
            }
        }
        let list: Vec<usize> = (0..need.len()).filter(|&l| need[l]).collect();
        if list.len() > budget.points {
            return None;
        }
        budget.points -= list.len();
        budget.blocks -= blocks.len();
        let coord = |l: usize, a: usize| {
            let q = [l % pts[0], (l / pts[0]) % pts[1], l / (pts[0] * pts[1])];
            dom.at(a, c0[a] as f64 + q[a] as f64 / r as f64, n)
        };
        let mut vals = vec![f32::NAN; need.len()];
        let mut sc = FieldScratch::default();
        for part in list.chunks(FIELD_CHUNK) {
            let cols: [Vec<f64>; 3] = [0, 1, 2].map(|a| part.iter().map(|&l| coord(l, a)).collect());
            let v = field_values_with(prog, combos, [&cols[0], &cols[1], &cols[2]], &mut sc);
            for (&l, &x) in part.iter().zip(v) {
                vals[l] = x as f32;
            }
        }
        // 成分の外側の面の上に、符号の変わる辺があるか。あれば向こうのブロックを足してやり直す
        let changes = |p: [usize; 3], a: usize| {
            let mut q = p;
            q[a] += 1;
            let (f0, f1) = (vals[at(p)], vals[at(q)]);
            f0.is_finite() && f1.is_finite() && (f0 < 0.0) != (f1 < 0.0)
        };
        let grow = outward(blocks, r, c0, &changes);
        if !grow.is_empty() {
            blocks.extend(grow);
            continue;
        }

        // 符号は変わるが F = 0 になる点が無い辺（極など）には面を張らない（[`false_crossings`]）
        let mut sign_edges: Vec<SignEdge> = Vec::new();
        let mut sign_ids: Vec<usize> = Vec::new();
        let point = |l: usize| [0, 1, 2].map(|a| coord(l, a));
        for &l in &list {
            let p = [l % pts[0], (l / pts[0]) % pts[1], l / (pts[0] * pts[1])];
            for a in 0..3 {
                if p[a] + 1 >= pts[a] {
                    continue;
                }
                let st = [1, pts[0], pts[0] * pts[1]][a];
                let (f0, f1) = (vals[l], vals[l + st]);
                if !f0.is_finite() || !f1.is_finite() || (f0 < 0.0) == (f1 < 0.0) {
                    continue;
                }
                // なめらかに 0 を横切っているだけの辺は調べない（[`implicit_solid`] の 1'' と同じ判定）
                if p[a] >= 1 && p[a] + 2 < pts[a] {
                    let (fl, fr) = (vals[l - st], vals[l + 2 * st]);
                    let (sl, s, sr) = (f0 - fl, f1 - f0, fr - f1);
                    if sl * s > 0.0 && sr * s > 0.0 && s.abs() <= 4.0 * sl.abs().max(sr.abs()) {
                        continue;
                    }
                }
                sign_edges.push(SignEdge { p0: point(l), p1: point(l + st), f0: f0 as f64, f1: f1 as f64 });
                sign_ids.push(3 * l + a);
            }
        }
        let not_zero: crate::adaptive::HashSet<usize> = {
            let flags = false_crossings(&mut |xyz| field_values_with(prog, combos, xyz, &mut sc).to_vec(), &sign_edges);
            sign_ids.into_iter().zip(flags).filter(|&(_, bad)| bad).map(|(id, _)| id).collect()
        };
        let cell_ok = |c: [usize; 3]| {
            let b = [0, 1, 2].map(|a| ((c0[a] + c[a] / r) / BLOCK).min(nb - 1) - b0[a]);
            inside[b[0] + bd[0] * (b[1] + bd[1] * b[2])]
        };
        let cross = |p: [usize; 3], a: usize| -> Option<(f64, bool)> {
            let mut q = p;
            q[a] += 1;
            let (f0, f1) = (vals[at(p)], vals[at(q)]);
            if !f0.is_finite() || !f1.is_finite() || (f0 < 0.0) == (f1 < 0.0) || not_zero.contains(&(3 * at(p) + a)) {
                return None;
            }
            Some(((f0 / (f0 - f1)) as f64, f0 < 0.0))
        };
        let pos = |p: [f64; 3]| [0, 1, 2].map(|a| dom.at(a, c0[a] as f64 + p[a] / r as f64, n));
        return Some(nets_on_grid(dims, &cell_ok, &|c| vals[at(c)], &cross, &pos));
    }
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
                // 少ないほうの符号の角を、つながっているものどうしで組に分ける
                let q = [i, j, k];
                let far = |fa: usize, side: usize| -> Option<[f32; 4]> {
                    if (side == 0 && q[fa] == 0) || (side == 1 && q[fa] + 2 > n) {
                        return None;
                    }
                    let (g, h) = ((fa + 1) % 3, (fa + 2) % 3);
                    Some([(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dg, dh)| {
                        let mut p = q;
                        p[fa] = if side == 0 { q[fa] - 1 } else { q[fa] + 2 };
                        p[g] += dg;
                        p[h] += dh;
                        f[idx(p[0], p[1], p[2])]
                    }))
                };
                let (mut group, mut groups, minority) =
                    corner_groups(&CORNERS.map(|d| f[idx(i + d[0], j + d[1], k + d[2])]), &far);
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
        let mut plan = SparseField::plan(prog, ranges, dom, n, all_blocks, None);
        let vals = plan.values(prog, combos, dom, 0..plan.chunks());
        plan.fill(&vals);
        plan.field
    }

    /// F の値を入れる前の形（あるブロック・値の置き場所）と、F を計算する点の一覧（[`FieldPlan`]）
    fn plan(prog: &Program, ranges: &[(f64, f64)], dom: Domain, n: usize, all_blocks: bool, mask: Option<&[bool]>) -> FieldPlan {
        let m = n + 1;
        let nb = n.div_ceil(BLOCK);
        // 面を作るブロックを絞る（部品に分けて描くとき。pieces.rs）。絞ったブロックでは、区間演算で空かどうかを調べない
        // （小さい部品をたくさん作るとき、全部のブロックを調べる手間のほうが大きい）
        let occupied = match mask.filter(|k| k.len() == nb * nb * nb) {
            // 大きい部品（何枚も重なる大きな面）は、絞ったうえで、区間演算でも空のブロックを外す
            Some(mask) if mask.iter().filter(|&&k| k).count() > 64 => {
                let mut occ = block_occupancy(prog, ranges, dom, n, nb);
                for (o, &k) in occ.iter_mut().zip(mask) {
                    *o &= k;
                }
                occ
            }
            Some(mask) => mask.to_vec(),
            None if all_blocks => vec![true; nb * nb * nb],
            None => block_occupancy(prog, ranges, dom, n, nb),
        };
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
/// 3. 2 で「縮まなかった」辺は、二分をさらに [`EXTRA_ROUNDS`] 回続けてから、同じ基準で決め直す。面が辺にほぼ沿っていて
///    両端の値がとても小さい辺（ジャイロイドの鞍の近くなど）は、零点があっても 6 回では半分まで縮まないことがある。
///    本当の零点なら、続ければ値は区間の幅に比例して縮む（極なら増え、値が飛ぶ所なら変わらない）
///
/// 計算は、全部の辺をまとめて行う
fn false_crossings(eval: &mut dyn FnMut([&[f64]; 3]) -> Vec<f64>, edges: &[SignEdge]) -> Vec<bool> {
    const ROUNDS: usize = 6;
    const EXTRA_ROUNDS: usize = 18;
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
    let shrunk = |k: usize, fa: f64, fb: f64| fa.abs() + fb.abs() < 0.5 * (edges[k].f0.abs() + edges[k].f1.abs());
    for round in 0..ROUNDS + EXTRA_ROUNDS {
        if round == ROUNDS {
            // 3. 縮んだ辺は本当の零点。縮まなかった辺だけ、二分を続ける
            rest.retain(|&(k, _, _, fa, fb)| !shrunk(k, fa, fb));
        }
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
        if !shrunk(k, fa, fb) {
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
fn find_touches(prog: &Program, f: &[f32], dom: Domain, n: usize) -> crate::adaptive::HashMap<usize, f32> {
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
fn touch_search(prog: &Program, cands: Vec<TouchCand>, dom: Domain, n: usize) -> crate::adaptive::HashMap<usize, f32> {
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
    let mut touch = crate::adaptive::HashMap::default();
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
    use crate::adaptive::HashMap;
    let count = tris.len() / 3;
    // 辺（小さい番号, 大きい番号）→ その辺を持つ三角形
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::default();
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
    /// 表示中の座標 q を、F の変数 p に移す 1 次式 p = A q + b（陰関数を回す前の形で持つとき。[`LocalMesh`]）。
    /// None なら p = q
    pub map: Option<([[f64; 3]; 3], [f64; 3])>,
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
        Field { prog: prog.clone(), combos, negative, map: None }
    }

    /// 回す前の形の F（変数は回す前の表示中の座標）と、表示中の座標から F の変数への 1 次式 p = A q + b。
    /// どちらの側が中身かは、表示範囲 `dom`（表示中の座標）で決める
    pub fn mapped(prog: &Program, a: [[f64; 3]; 3], b: [f64; 3], dom: Domain) -> Field {
        let mut f = Field { prog: prog.clone(), combos: vec![Vec::new()], negative: true, map: Some((a, b)) };
        let n = 16;
        let mut cols: [Vec<f64>; 3] = Default::default();
        for k in 0..n {
            for j in 0..n {
                for i in 0..n {
                    cols[0].push(dom.at(0, i as f64 + 0.5, n));
                    cols[1].push(dom.at(1, j as f64 + 0.5, n));
                    cols[2].push(dom.at(2, k as f64 + 0.5, n));
                }
            }
        }
        let v = f.values(cols);
        let neg = v.iter().filter(|&&x| x < 0.0).count();
        let pos = v.iter().filter(|&&x| x > 0.0).count();
        f.negative = neg <= pos;
        f
    }

    /// 表示中の座標の点（列ごと）での F の値
    fn values(&self, cols: [Vec<f64>; 3]) -> Vec<f64> {
        let Some((a, b)) = &self.map else {
            return field_values(&self.prog, &self.combos, [&cols[0], &cols[1], &cols[2]]);
        };
        let p: [Vec<f64>; 3] = [0, 1, 2].map(|r| {
            (0..cols[0].len())
                .map(|k| b[r] + a[r][0] * cols[0][k] + a[r][1] * cols[1][k] + a[r][2] * cols[2][k])
                .collect()
        });
        field_values(&self.prog, &self.combos, [&p[0], &p[1], &p[2]])
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
        let point = |k: usize| [cols[0][k], cols[1][k], cols[2][k]];
        let f = self.values(cols.clone());
        // 中身なら負になる値
        let g: Vec<f64> = if self.negative { f } else { f.iter().map(|v| -v).collect() };

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
    /// 段差のある陽関数を回したもの。射影したメッシュの [`Mesh::keep_full`] にする（まとめると段差が崩れる）
    pub keep_full: bool,
    /// 表示範囲から遠い点も捨てない（陽関数から作った形。[`project`] を参照）
    pub keep_far: bool,
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

/// 媒介変数の曲面の、値が飛ぶ所（[`param_jumps`]）
struct ParamJump {
    /// 辺の上の位置（0: 番号の小さい端, 1: 大きい端）
    t: f64,
    /// 飛ぶ直前（番号の小さい端の側）と直後の点
    before: Vec<f64>,
    after: Vec<f64>,
}

/// [`param_jumps`] の結果
struct ParamCut {
    /// 値が飛ぶ辺（番号の小さい頂点, 大きい頂点）
    jumps: crate::adaptive::HashMap<(u32, u32), ParamJump>,
    /// 極の先の点を、遠くにそろえた（射影で捨てないよう、[`NdShape::keep_far`] を立てる）
    far: bool,
}

/// 媒介変数の曲面で、値が飛ぶ所（`floor` の段差、`tan` の極）を探す。細分の後の辺のうち、両端の n 次元の距離が
/// 表示範囲の対角線 `diag` の 2% を超えるものを、(u, v) の上で距離の大きいほうの半分に狭めて 12 回二分する。
/// 距離が最初の半分以上残れば「飛び」とする（なめらかなら 1/4096 ほどに縮む）。陽関数の段差探しと同じく、
/// 1・2 回目の二分の中点がどちらも両端の平均に近い辺と、3 回二分して距離が 0.3 倍より小さくなった辺は、早めに外す。
/// 片側が無限大に飛ぶ極（`tan(u)`）: 二分した先の点が、その側の頂点から対角線の 100 倍より遠ければ、その向きに対角線の
/// 2 倍だけ離れた点にそろえる。
/// その後、陽関数（S20）と同じく、段差の線が三角形の中で終わる・交わる所（1 辺だけ・3 辺とも飛ぶ三角形）だけを、
/// 5 回まで二分する（足す三角形は 20,000 個まで）
fn param_jumps(
    r: &mut crate::adaptive::Triangulation<Vec<f64>>,
    eval: &mut crate::adaptive::Eval<'_, Vec<f64>>,
    diag: f64,
) -> ParamCut {
    const ROUNDS: usize = 12;
    let threshold = 0.02 * diag;
    let dist = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| (x - y).powi(2)).sum::<f64>().sqrt();
    let ek = |a: u32, b: u32| if a < b { (a, b) } else { (b, a) };
    let mut far = false;
    // 辺の一覧（`from` 以上の番号の頂点を持つ辺だけ）で段差を探す
    let mut search = |r: &crate::adaptive::Triangulation<Vec<f64>>,
                      tris: &[[u32; 3]],
                      from: usize,
                      eval: &mut crate::adaptive::Eval<'_, Vec<f64>>,
                      out: &mut crate::adaptive::HashMap<(u32, u32), ParamJump>| {
        let (uv, vals) = (r.uv(), r.vals());
        let mut seen: crate::adaptive::HashSet<(u32, u32)> = Default::default();
        struct Cand {
            e: (u32, u32),
            ta: f64,
            tb: f64,
            pa: Vec<f64>,
            pb: Vec<f64>,
            first: f64,
            smooth: bool,
        }
        let mut cands: Vec<Cand> = Vec::new();
        for t in tris {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let e = ek(a, b);
                if (e.1 as usize) < from || !seen.insert(e) {
                    continue;
                }
                let (pa, pb) = (&vals[e.0 as usize], &vals[e.1 as usize]);
                if !finite(pa) || !finite(pb) {
                    continue;
                }
                let d = dist(pa, pb);
                if d > threshold {
                    cands.push(Cand { e, ta: 0.0, tb: 1.0, pa: pa.clone(), pb: pb.clone(), first: d, smooth: false });
                }
            }
        }
        for round in 0..ROUNDS {
            if round == 3 {
                cands.retain(|c| dist(&c.pa, &c.pb) >= 0.3 * c.first);
            }
            if cands.is_empty() {
                break;
            }
            let (mut us, mut vs) = (Vec::with_capacity(cands.len()), Vec::with_capacity(cands.len()));
            for c in &cands {
                let (a, b) = (uv[c.e.0 as usize], uv[c.e.1 as usize]);
                let t = 0.5 * (c.ta + c.tb);
                us.push(a[0] + (b[0] - a[0]) * t);
                vs.push(a[1] + (b[1] - a[1]) * t);
            }
            let mids = eval(&us, &vs);
            let mut keep = vec![true; cands.len()];
            for ((c, pm), keep) in cands.iter_mut().zip(mids).zip(keep.iter_mut()) {
                let t = 0.5 * (c.ta + c.tb);
                let ok = finite(&pm);
                if round < 2 {
                    let avg: Vec<f64> = c.pa.iter().zip(&c.pb).map(|(x, y)| 0.5 * (x + y)).collect();
                    let smooth = ok && dist(&pm, &avg) < 0.2 * dist(&c.pa, &c.pb);
                    if round == 0 {
                        c.smooth = smooth;
                    } else if smooth && c.smooth {
                        *keep = false;
                        continue;
                    }
                }
                if !ok || dist(&pm, &c.pa) >= dist(&c.pb, &pm) {
                    c.tb = t;
                    c.pb = pm;
                } else {
                    c.ta = t;
                    c.pa = pm;
                }
            }
            let mut it = keep.iter();
            cands.retain(|_| *it.next().unwrap());
        }
        for mut c in cands {
            let d = dist(&c.pa, &c.pb);
            if !(d.is_finite() && d >= 0.5 * c.first && finite(&c.pa) && finite(&c.pb)) {
                continue;
            }
            // 極の先の点を、その側の頂点から対角線の 2 倍の所にそろえる
            for (p, base) in [(&mut c.pa, &vals[c.e.0 as usize]), (&mut c.pb, &vals[c.e.1 as usize])] {
                let l = dist(p, base);
                if l > 100.0 * diag {
                    for (x, b) in p.iter_mut().zip(base) {
                        *x = b + (*x - b) / l * 2.0 * diag;
                    }
                    far = true;
                }
            }
            out.insert(c.e, ParamJump { t: 0.5 * (c.ta + c.tb), before: c.pa, after: c.pb });
        }
    };
    let mut jumps: crate::adaptive::HashMap<(u32, u32), ParamJump> = Default::default();
    let mut tris = r.tris();
    search(r, &tris, 0, eval, &mut jumps);
    if !jumps.is_empty() {
        const PASSES: usize = 5;
        const MAX_NEW: usize = 20_000;
        let mut added = 0;
        for _ in 0..PASSES {
            let vals = r.vals();
            let mut marked: Vec<[u32; 3]> = tris
                .iter()
                .filter(|t| {
                    t.iter().all(|&k| finite(&vals[k as usize])) && {
                        let c = [ek(t[0], t[1]), ek(t[1], t[2]), ek(t[2], t[0])].iter().filter(|e| jumps.contains_key(e)).count();
                        c == 1 || c == 3
                    }
                })
                .copied()
                .collect();
            if marked.is_empty() || added >= MAX_NEW {
                break;
            }
            let room = (MAX_NEW - added) / 2;
            let last = marked.len() > room;
            marked.truncate(room.max(1));
            let from = r.uv().len();
            added += r.bisect(&marked, eval);
            tris = r.tris();
            search(r, &tris, from, eval, &mut jumps);
            if last {
                break;
            }
        }
        // 分けられて無くなった辺の段差は消す
        let mut alive: crate::adaptive::HashSet<(u32, u32)> = Default::default();
        for t in &tris {
            for e in [ek(t[0], t[1]), ek(t[1], t[2]), ek(t[2], t[0])] {
                alive.insert(e);
            }
        }
        jumps.retain(|e, _| alive.contains(e));
    }
    ParamCut { jumps, far }
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
        return NdShape { dims, points, grid: Some((m, grid)), solid: false, complex: Vec::new(), tris: None, keep_full: false, keep_far: false };
    };
    // 曲面ごとに細かくする。足した点は、全部の曲面の一様な格子の点の後ろに並べる
    let base = sets.len() * m * m;
    let (mut extra_points, mut extra_grid, mut tris) = (Vec::new(), Vec::new(), Vec::new());
    let mut refined_any = false;
    let (mut keep_full, mut keep_far) = (false, false);
    // 表示範囲の対角線（`tol` は対角線の 10⁻³ 倍。lib.rs を参照）
    let diag = tol * 1e3;
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
        let mut r = crate::adaptive::refine_keep(n, u, v, first, &mut eval, &mut decide, limits);
        // 値が飛ぶ所（floor の段差、tan の極）を探して、三角形を切り分ける（S20 と同じく、段差の線が終わる・交わる所だけ細かくする）
        let mut cut = param_jumps(&mut r, &mut eval, diag);
        let r = r.finish();
        if r.uv.len() > m * m || !cut.jumps.is_empty() {
            refined_any = true;
        }
        keep_full |= !cut.jumps.is_empty();
        keep_far |= cut.far;
        let offset = base + extra_points.len() / dims;
        for (p, w) in r.uv[m * m..].iter().zip(&r.vals[m * m..]) {
            extra_points.extend_from_slice(w);
            extra_grid.extend_from_slice(&[(p[0] - u.0) as f32, (p[1] - v.0) as f32]);
        }
        let map = |k: u32| if (k as usize) < m * m { (s * m * m) as u32 + k } else { (offset + k as usize - m * m) as u32 };
        // 飛ぶ所の 2 つの点（直前・直後）を、点の列の後ろに足す
        let mut jump_vertex: crate::adaptive::HashMap<(u32, u32), (u32, u32)> = Default::default();
        let mut keys: Vec<(u32, u32)> = cut.jumps.keys().copied().collect();
        keys.sort_unstable();
        for e in keys {
            let jp = cut.jumps.remove(&e).unwrap();
            let (a, b) = (r.uv[e.0 as usize], r.uv[e.1 as usize]);
            let uv = [a[0] + (b[0] - a[0]) * jp.t, a[1] + (b[1] - a[1]) * jp.t];
            let mut ids = [0u32; 2];
            for (q, pt) in [jp.before, jp.after].into_iter().enumerate() {
                ids[q] = (base + (extra_points.len() / dims)) as u32;
                extra_points.extend_from_slice(&pt);
                extra_grid.extend_from_slice(&[(uv[0] - u.0) as f32, (uv[1] - v.0) as f32]);
            }
            jump_vertex.insert(e, (ids[0], ids[1]));
        }
        for t in &r.tris {
            let mut jv: [Option<(u32, u32)>; 3] = [None; 3];
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                if let Some(&(start, end)) = jump_vertex.get(&(a.min(b), a.max(b))) {
                    jv[e] = Some(if a < b { (start, end) } else { (end, start) });
                }
            }
            let (pieces, _) = cut_by_jumps(t.map(map), jv);
            for piece in pieces {
                tris.extend_from_slice(&piece);
            }
        }
    }
    if !refined_any {
        return NdShape { dims, points, grid: Some((m, grid)), solid: false, complex: Vec::new(), tris: None, keep_full: false, keep_far: false };
    }
    points.extend_from_slice(&extra_points);
    grid.extend_from_slice(&extra_grid);
    NdShape { dims, points, grid: Some((m, grid)), solid: false, complex: Vec::new(), tris: Some((base, tris)), keep_full, keep_far }
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
    NdShape { dims, points, grid: Some((m, grid)), solid: true, complex: Vec::new(), tris: None, keep_full: false, keep_far: false }
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
    NdShape { dims, points, grid: Some((m, grid)), solid: false, complex: Vec::new(), tris: None, keep_full: false, keep_far: false }
}

/// 陽関数の曲面（表示中の `axis` 番目の座標 = f(残り2つ)）を、n 次元の点として計算する。
/// 回転が表示中の座標どうしだけのときに、媒介変数表示 (u, v, f(u, v)) と同じように回すために使う。
///
/// - `axes`: 表示中の3つの座標の番号、`base`: 各座標の値（表示していない座標は断面の値）
/// - `dom`: 計算する範囲（表示中の座標で。回しても表示範囲が欠けないよう、表示範囲より広くする）
/// - `h`: 段差・細かくする量の目安にする高さ（表示範囲の高さ）
///
/// 回さないときと同じ [`explicit_surface_scaled`] で作るので、値が飛ぶ所（階段の段差など）は切り分けて
/// 描き（`walls` なら壁も）、急に変わる所は細かくし（`refine`）、高さは `dom` の上下の端で切る。
/// 格子線用の座標には、残り2つの座標の値そのもの（元の座標の値）を入れる。回転しても、格子線は元の座標の
/// きりの良い値の所に引かれる。
#[allow(clippy::too_many_arguments)]
pub fn explicit_nd(
    prog: &Program,
    axis: usize,
    axes: [usize; 3],
    base: &[f64],
    dom: Domain,
    n: usize,
    mode: ComplexMode,
    walls: bool,
    refine: bool,
    h: f64,
) -> NdShape {
    let mesh = explicit_surface_scaled(prog, dom, n, axis, mode, walls, refine, h);
    mesh_to_nd(&mesh, axes, base, Some(axis))
}

/// 傾いた座標系の陽関数 t = g(u, v)（[`crate::equation::Plot::Tilted`]）の曲面を、表示中の座標で作る。
///
/// 表示範囲の 8 つの角を (u, v, t) = (q·e₁, q·e₂, q·e₃) に移し、それを囲む直方体を「計算する範囲」にして、
/// 回さない陽関数と同じ [`explicit_surface_scaled`] で作る（段差の切り分け・壁・急な所の細分・定義域の縁がそのまま効く）。
/// 計算する範囲の u, v の幅の大きいほうが、表示範囲の一番長い辺の k 倍なら、分割数を k 倍にする（1 ≤ k ≤ 2）。
/// 段差・細分の目安の高さは、表示範囲の一番短い辺の長さ。
/// 頂点と法線は q = u e₁ + v e₂ + t e₃ で表示中の座標に移す。
/// `color` があれば、頂点の (u, v) で計算して複素数の色分けにする（段差の縁の頂点は、同じ (u, v) の 2 つが同じ色になる）。
/// 格子線用の座標は、`grid` があればその 2 つの 1 次式（元の座標の値）、無ければ (u, v)。3 つ目は表示中の縦の座標
#[allow(clippy::too_many_arguments)]
pub fn explicit_tilted(
    prog: &Program,
    color: Option<&Program>,
    frame: [[f64; 3]; 3],
    grid: Option<[[f64; 3]; 2]>,
    dom: Domain,
    n: usize,
    walls: bool,
    refine: bool,
) -> Mesh {
    let dot = |x: [f64; 3], y: [f64; 3]| x[0] * y[0] + x[1] * y[1] + x[2] * y[2];
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    for corner in 0..8 {
        let q = [0, 1, 2].map(|a| if (corner >> a) & 1 == 0 { dom.lo[a] } else { dom.hi[a] });
        for k in 0..3 {
            let x = dot(q, frame[k]);
            lo[k] = lo[k].min(x);
            hi[k] = hi[k].max(x);
        }
    }
    let ext = Domain { lo, hi };
    let edges = [0, 1, 2].map(|a| dom.hi[a] - dom.lo[a]);
    let longest = edges.iter().cloned().fold(0.0, f64::max);
    let shortest = edges.iter().cloned().fold(f64::INFINITY, f64::min);
    let k = ((hi[0] - lo[0]).max(hi[1] - lo[1]) / longest).clamp(1.0, 2.0);
    let mut mesh = explicit_surface_scaled(
        prog,
        ext,
        (n as f64 * k).round() as usize,
        2,
        ComplexMode::Off,
        walls,
        refine,
        shortest,
    );
    let uvt: Vec<[f64; 3]> =
        mesh.positions.chunks_exact(3).map(|p| [p[0] as f64, p[1] as f64, p[2] as f64]).collect();
    let to_q = |p: [f64; 3]| [0, 1, 2].map(|a| p[0] * frame[0][a] + p[1] * frame[1][a] + p[2] * frame[2][a]);
    mesh.positions = uvt.iter().flat_map(|&p| to_q(p).map(|v| v as f32)).collect();
    let normals: Vec<f32> = mesh
        .normals
        .chunks_exact(3)
        .flat_map(|nv| to_q([nv[0] as f64, nv[1] as f64, nv[2] as f64]).map(|v| v as f32))
        .collect();
    mesh.normals = normals;
    let lin = |c: [f64; 3], p: [f64; 3]| (c[0] + c[1] * p[0] + c[2] * p[1]) as f32;
    mesh.grid_coords = uvt
        .iter()
        .zip(mesh.positions.chunks_exact(3))
        .flat_map(|(&p, q)| match grid {
            Some([g1, g2]) => [lin(g1, p), lin(g2, p), q[2]],
            None => [p[0] as f32, p[1] as f32, q[2]],
        })
        .collect();
    if let Some(cp) = color {
        let us: Vec<f64> = uvt.iter().map(|p| p[0]).collect();
        let vs: Vec<f64> = uvt.iter().map(|p| p[1]).collect();
        let mut vals = vec![C::new(0.0, 0.0); uvt.len()];
        cp.eval_batch(&[&us, &vs], &mut vals);
        mesh.complex = vals.into_iter().flat_map(complex_attr).collect();
    }
    mesh
}

/// 表示中の座標で作ったメッシュを、n 次元の点（表示していない座標は `base` の値）の形にする。
/// 曲面（`surface` が Some(高さの軸)）なら、三角形は [`NdShape::tris`] にそのまま入れ（一様な格子の点は無い）、
/// 格子線用の座標には高さでない2つの座標の値を入れる。None なら曲線（NaN は線の切れ目）
fn mesh_to_nd(mesh: &Mesh, axes: [usize; 3], base: &[f64], surface: Option<usize>) -> NdShape {
    let d = base.len();
    let count = mesh.positions.len() / 3;
    let mut points = Vec::with_capacity(count * d);
    for q in mesh.positions.chunks_exact(3) {
        if q.iter().all(|v| v.is_finite()) {
            let mut p = base.to_vec();
            for k in 0..3 {
                p[axes[k]] = q[k] as f64;
            }
            points.extend_from_slice(&p);
        } else {
            points.extend(std::iter::repeat_n(f64::NAN, d));
        }
    }
    let Some(axis) = surface else {
        return NdShape { dims: d, points, keep_full: mesh.keep_full, keep_far: true, ..Default::default() };
    };
    let (ua, va) = others(axis);
    let grid: Vec<f32> = mesh.positions.chunks_exact(3).flat_map(|q| [q[ua], q[va]]).collect();
    NdShape {
        dims: d,
        points,
        // 一様な格子の点は無い（m は 2 にしておく。0 個の点を m × m 個ずつ見ても何もしない）
        grid: Some((2, grid)),
        solid: false,
        complex: mesh.complex.clone(),
        tris: Some((0, mesh.indices.clone())),
        keep_full: mesh.keep_full,
        // 陽関数の点は、高さを「計算する範囲 ± 表示範囲の高さ」にそろえてあるので、遠くても f32 で無限大にならない
        keep_far: true,
    }
}

/// 2D 表示の、回した `y = f(x)`（`x = f(y)`）の曲線を n 次元の点として計算する（`dom` は計算する範囲）。
/// 回さないときと同じ [`explicit_curve_2d`] で作るので、値が飛ぶ所では線を切る
pub fn explicit_curve_nd(prog: &Program, axis: usize, axes: [usize; 3], base: &[f64], dom: Domain, tol: f64) -> NdShape {
    let mut mesh = explicit_curve_2d(prog, axis, dom, tol);
    // 極の近くのとても大きな値は、回すと横にも大きくはみ出す。計算する範囲の外は形が見えないので、
    // 範囲の何倍かの所にそろえる（f32 にしたとき無限大にならないように）
    let (lo, hi) = (dom.lo[axis], dom.hi[axis]);
    let r = 100.0 * (hi - lo);
    for q in mesh.positions.chunks_exact_mut(3) {
        if q[axis].is_finite() {
            q[axis] = q[axis].clamp((lo - r) as f32, (hi + r) as f32);
        }
    }
    let mut base = base.to_vec();
    base[axes[2]] = 0.0; // 2D 表示では縦の座標は 0
    mesh_to_nd(&mesh, axes, &base, None)
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
    NdShape { dims, points, grid: None, solid: false, complex: Vec::new(), tris: None, keep_full: false, keep_far: false }
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
    NdShape { dims, points, grid: None, solid: false, complex: Vec::new(), tris: None, keep_full: false, keep_far: false }
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
            keep_full: shapes.iter().any(|s| s.keep_full),
            keep_far: shapes.iter().any(|s| s.keep_far),
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
        NdShape {
            dims,
            points: Vec::new(),
            grid: m.map(|m| (m, Vec::new())),
            solid,
            complex: Vec::new(),
            tris: None,
            keep_full: shapes.iter().any(|s| s.keep_full),
            keep_far: shapes.iter().any(|s| s.keep_far),
        };
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

/// 2D の線分 a–b が、長さが無い・とても短い（大きさ `scale` の 10⁻⁶ 倍以下）か。
/// 描画側は、ほぼ同じ点だけの線を「点」とみなして丸を付けるので、そうした線は描かない。
/// 回した曲面の切り口では、格子点をちょうど通る所で、ちょうど同じではない（f32 の誤差ほど違う）点の組ができる
fn too_short(a: [f32; 3], b: [f32; 3], scale: f32) -> bool {
    let d: f32 = (0..3).map(|c| (a[c] - b[c]).abs()).sum();
    d <= 1e-6 * scale || a == b
}

/// 2D 表示の「切り口」: 曲面のメッシュと、平面「縦の座標 = `level`」の交わりの線（曲線のメッシュ）。
/// 三角形ごとに、平面をまたぐ2つの辺の上の点を結ぶ。線分どうしの間は NaN で切る
pub fn slice_mesh(mesh: &Mesh, level: f32) -> Mesh {
    let p = &mesh.positions;
    // メッシュの大きさ（とても短い線を捨てる目安）
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for q in p.chunks_exact(3).filter(|q| q.iter().all(|v| v.is_finite())) {
        for c in 0..3 {
            lo[c] = lo[c].min(q[c]);
            hi[c] = hi[c].max(q[c]);
        }
    }
    let scale = (0..3).map(|c| (hi[c] - lo[c]).max(0.0)).sum::<f32>();
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
        // 長さの無い線・とても短い線は描かない（描画側が点とみなして丸を付ける）
        if pts.len() == 2 && !too_short(pts[0], pts[1], scale) {
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
    let scale = ((dom.hi[0] - dom.lo[0]) + (dom.hi[1] - dom.lo[1])) as f32;
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
    let not_zero: crate::adaptive::HashSet<usize> = {
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
    let exact = exact_crossings_2d(prog, &f, &xs, &ys, m, &not_zero);
    // F が符号を変えずに 0 に接するだけの所（(x² + y² − 4)² = 0 の円など）。格子の線ごとに探す
    let touch = touches_2d(prog, &f, &xs, &ys, m);
    // 接するだけの所がある辺の印（マスごとに表を引くと遅いので、先に印で絞る）
    let mut has_touch = vec![false; if touch.is_empty() { 0 } else { 2 * m * m }];
    for &id in touch.keys() {
        has_touch[id] = true;
    }
    // 接するだけの線分を作ったマスと、線分の端に使った点（ほかの所で「点」として描かないように）
    let mut touch_cell = vec![false; n * n];
    let mut used: crate::adaptive::HashSet<[u32; 2]> = Default::default();
    let key = |p: [f64; 2]| [(p[0] as f32).to_bits(), (p[1] as f32).to_bits()];
    let mut pts: Vec<f32> = Vec::new();
    for j in 0..n {
        for i in 0..n {
            // マスの4つの角（反時計回り）
            let ks = [j * m + i, j * m + i + 1, (j + 1) * m + i + 1, (j + 1) * m + i];
            let v = ks.map(|k| f[k]);
            if v.iter().any(|x| !x.is_finite()) {
                continue;
            }
            // 角の符号がそろっていれば、符号の変わる線は通らない。接するだけの所があれば、それを結ぶ
            if !touch.is_empty() && (v.iter().all(|&x| x >= 0.0) || v.iter().all(|&x| x < 0.0)) {
                // マスの4辺の上の点と、角にちょうどある点（隣のマスの辺の始点 t = 0 として見つかったもの）
                let k0 = ks[0];
                let mut found: Vec<[f64; 2]> = Vec::new();
                let ids = [2 * k0, 2 * (k0 + m), 2 * k0 + 1, 2 * (k0 + 1) + 1];
                let corners = ks.iter().flat_map(|&c| [2 * c, 2 * c + 1]);
                for id in ids.into_iter().chain(corners) {
                    if !has_touch[id] {
                        continue;
                    }
                    let Some(&(p, t)) = touch.get(&id) else { continue };
                    let corner_only = !ids.contains(&id);
                    if corner_only && t != 0.0 {
                        continue;
                    }
                    // 同じ所を、縦と横の線（や、隣の辺の端）で2回見つけることがある。マスの 1/10 より近い点はまとめる
                    let (cw, ch) = (xs[ks[1]] - xs[ks[0]], ys[ks[3]] - ys[ks[0]]);
                    if !found.iter().any(|q| (q[0] - p[0]).abs() < 0.1 * cw && (q[1] - p[1]).abs() < 0.1 * ch) {
                        found.push(p);
                    }
                }
                if found.len() == 2 {
                    let (pa, pb) = (found[0].map(|v| v as f32), found[1].map(|v| v as f32));
                    if !too_short([pa[0], pa[1], 0.0], [pb[0], pb[1], 0.0], scale) {
                        pts.extend_from_slice(&[pa[0], pa[1], 0.0]);
                        pts.extend_from_slice(&[pb[0], pb[1], 0.0]);
                        pts.extend_from_slice(&[f32::NAN; 3]);
                        touch_cell[j * n + i] = true;
                    }
                }
                // 3 点以上（線が込み入った所）は結ばないが、点としても描かない
                if found.len() >= 2 {
                    for p in &found {
                        used.insert(key(*p));
                    }
                }
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
                    if let Some(&p) = exact.get(&edge_id(ka, kb)) {
                        cross.push(p);
                        continue;
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
                if too_short([pa[0], pa[1], 0.0], [pb[0], pb[1], 0.0], scale) {
                    continue;
                }
                pts.extend_from_slice(&[pa[0], pa[1], 0.0]);
                pts.extend_from_slice(&[pb[0], pb[1], 0.0]);
                pts.extend_from_slice(&[f32::NAN; 3]);
            }
        }
    }
    // 0 に接するだけの1点（x² + y² = 0 の原点など）: 描画側が丸を付ける「長さの無い曲線」にする。
    // 格子の線の上で見つかったのに線分の端にならなかった点と、格子点の周りで |F| が一番小さい所を探して見つけた点
    // 接するだけの線分の近く（周りのマス）の点は、その線の一部なので点にしない
    let near_segment = |p: [f64; 2]| {
        let cell = |a: usize| (((p[a] - dom.lo[a]) / (dom.hi[a] - dom.lo[a]) * n as f64).max(0.0) as usize).min(n - 1);
        let (i, j) = (cell(0), cell(1));
        (i.saturating_sub(1)..(i + 2).min(n)).any(|a| (j.saturating_sub(1)..(j + 2).min(n)).any(|b| touch_cell[b * n + a]))
    };
    let dots: Vec<[f64; 2]> = touch
        .values()
        .map(|&(p, _)| p)
        .filter(|&p| !used.contains(&key(p)))
        .chain(touch_points_2d(prog, &f, &xs, &ys, m, |i, j| {
            // 格子点 (i, j) の周りのマス（2 マス先まで）に、接するだけの線分があれば調べない
            (i.saturating_sub(2)..(i + 2).min(n)).any(|a| (j.saturating_sub(2)..(j + 2).min(n)).any(|b| touch_cell[b * n + a]))
        }))
        .filter(|&p| !near_segment(p))
        .collect();
    // 同じ点を何度も見つけることがある（格子の縦と横の線・格子点の周りの探索）ので、マスの大きさの半分より近い点はまとめる
    let h = 0.5 * ((dom.hi[0] - dom.lo[0]) / n as f64).min((dom.hi[1] - dom.lo[1]) / n as f64);
    let mut kept: Vec<[f64; 2]> = Vec::new();
    for p in dots {
        if kept.iter().all(|q| (p[0] - q[0]).abs() > h || (p[1] - q[1]).abs() > h) {
            kept.push(p);
        }
    }
    for p in kept {
        let q = [p[0] as f32, p[1] as f32, 0.0];
        pts.extend_from_slice(&q);
        pts.extend_from_slice(&q);
        pts.extend_from_slice(&[f32::NAN; 3]);
    }
    Mesh { positions: pts, ..Default::default() }
}

/// 符号が変わる辺のうち、両端の値から直線で求めた交わる点が、実際の F = 0 の点からずれているもの
/// （その点での |F| が両端の差の 10⁻³ 倍より大きい、または F がちょうど 0 の平らな所）は、二分法で交わる点を求め直す。
/// 値が段になって飛ぶ式（回した階段 floor(x)/2 + floor(y)/4 など）で、格子に対して斜めの段の縁がぎざぎざにならないように。
/// 戻り値は 辺の番号（2 × 小さいほうの格子点 + 縦なら 1）→ 交わる点（求め直した辺だけ）
fn exact_crossings_2d(
    prog: &Program,
    f: &[f64],
    xs: &[f64],
    ys: &[f64],
    m: usize,
    not_zero: &crate::adaptive::HashSet<usize>,
) -> crate::adaptive::HashMap<usize, [f64; 2]> {
    // (辺の番号, 始点, 終点)
    let mut edges: Vec<(usize, usize, usize)> = Vec::new();
    for k in 0..m * m {
        let (i, j) = (k % m, k / m);
        for (dir, k1) in [(0, (i + 1 < m).then_some(k + 1)), (1, (j + 1 < m).then_some(k + m))] {
            let Some(k1) = k1 else { continue };
            let (f0, f1) = (f[k], f[k1]);
            if (f0 < 0.0) != (f1 < 0.0) && f0.is_finite() && f1.is_finite() && !not_zero.contains(&(2 * k + dir)) {
                edges.push((2 * k + dir, k, k1));
            }
        }
    }
    let at = |k: usize, k1: usize, t: f64| [xs[k] + (xs[k1] - xs[k]) * t, ys[k] + (ys[k1] - ys[k]) * t];
    let lin: Vec<[f64; 2]> = edges.iter().map(|&(_, k, k1)| at(k, k1, f[k] / (f[k] - f[k1]))).collect();
    let v = eval_2d(prog, &lin.iter().map(|p| p[0]).collect::<Vec<_>>(), &lin.iter().map(|p| p[1]).collect::<Vec<_>>());
    let bad: Vec<usize> = (0..edges.len())
        .filter(|&c| {
            let (_, k, k1) = edges[c];
            v[c] == 0.0 || !(v[c].abs() <= 1e-3 * (f[k] - f[k1]).abs())
        })
        .collect();
    // 二分法（まとめて計算する）。始点と同じ側か（F < 0 かどうか）で狭める
    let mut lo = vec![0.0f64; bad.len()];
    let mut hi = vec![1.0f64; bad.len()];
    for _ in 0..14 {
        let ps: Vec<[f64; 2]> = bad
            .iter()
            .enumerate()
            .map(|(b, &c)| at(edges[c].1, edges[c].2, 0.5 * (lo[b] + hi[b])))
            .collect();
        let w = eval_2d(prog, &ps.iter().map(|p| p[0]).collect::<Vec<_>>(), &ps.iter().map(|p| p[1]).collect::<Vec<_>>());
        for (b, &c) in bad.iter().enumerate() {
            let mid = 0.5 * (lo[b] + hi[b]);
            if !w[b].is_nan() && (w[b] < 0.0) == (f[edges[c].1] < 0.0) {
                lo[b] = mid;
            } else {
                hi[b] = mid;
            }
        }
    }
    bad.iter()
        .enumerate()
        .map(|(b, &c)| (edges[c].0, at(edges[c].1, edges[c].2, 0.5 * (lo[b] + hi[b]))))
        .collect()
}

/// [`implicit_curve_2d`] の F を、点の列 (x, y) でまとめて計算する（縦の座標があれば 0）
fn eval_2d(prog: &Program, x: &[f64], y: &[f64]) -> Vec<f64> {
    let mut out = vec![C::new(0.0, 0.0); x.len()];
    if prog.vars().len() == 2 {
        prog.eval_batch(&[x, y], &mut out);
    } else {
        let z = vec![0.0; x.len()];
        prog.eval_batch(&[x, y, &z], &mut out);
    }
    out.iter().map(|v| as_real(*v)).collect()
}

/// 調べる候補の数の上限（ざらざらした式で、候補ばかりになって遅くならないように）。
/// 格子が細かいときは、格子点の数の 1/16 まで増やす（接するだけの線が多い式で、線が途中で切れないように）
const TOUCH_2D_MAX: usize = 20_000;

/// 2D の格子（m × m 個の点、値 f）の線の上で、F が符号を変えずに 0 に接する所を探す（3次元の [`find_touches`] と同じ考え方）。
/// 辺 p → p + e について、|F| が p − e から p へ減り、p + e から p + 2e へ増えていて、内側の2点が外側よりずっと小さければ、
/// 三分探索で極小を求め、その値がほぼ 0（外側の 10⁻³ 倍以下）なら接している。
/// ただし、辺の両側のマスの角に符号の変わる所があれば候補にしない（符号の変わる線が通っている）。
/// 戻り値は 辺の番号（2 × 始点 + 縦なら 1）→ (点, 辺の上の位置 t)。t は [0, 1)（隣の辺と同じ所を2回数えない）
fn touches_2d(
    prog: &Program,
    f: &[f64],
    xs: &[f64],
    ys: &[f64],
    m: usize,
) -> crate::adaptive::HashMap<usize, ([f64; 2], f64)> {
    // 候補: (辺の番号, 始点, 向き（0: 横, 1: 縦）, 符号, 外側の |F| の平均)
    let mut cands: Vec<(usize, usize, usize, f64, f64)> = Vec::new();
    let same_sign = |ks: &[usize], s: f64| ks.iter().all(|&k| f[k].is_finite() && f[k] * s >= 0.0);
    'outer: for dir in 0..2 {
        let step = if dir == 0 { 1 } else { m };
        for k in 0..m * m {
            let (i, j) = (k % m, k / m);
            let along = if dir == 0 { i } else { j };
            if along < 1 || along + 2 >= m {
                continue;
            }
            let v = [f[k - step], f[k], f[k + step], f[k + 2 * step]].map(|x| x as f32);
            let across = if dir == 0 { m } else { 1 };
            let side = if dir == 0 { j } else { i };
            let around = || {
                // 辺を挟む2つのマスの角（6 点）の符号がそろっているか
                let s = if f[k] != 0.0 { f[k].signum() } else { f[k - step].signum() };
                (side == 0 || same_sign(&[k - across, k + step - across], s)) && (side + 1 >= m || same_sign(&[k + across, k + step + across], s))
            };
            if let Some((s, outer)) = touch_candidate(v, around) {
                cands.push((2 * k + dir, k, dir, s, outer));
                if cands.len() >= TOUCH_2D_MAX.max(m * m / 16) {
                    break 'outer;
                }
            }
        }
    }
    let mut out = crate::adaptive::HashMap::default();
    if cands.is_empty() {
        return out;
    }
    let at = |c: &(usize, usize, usize, f64, f64), t: f64| -> [f64; 2] {
        let (k, dir) = (c.1, c.2);
        let step = if dir == 0 { 1 } else { m };
        let (x0, y0) = (xs[k], ys[k]);
        let (dx, dy) = (xs[k + step] - x0, ys[k + step] - y0);
        [x0 + dx * t, y0 + dy * t]
    };
    // 三分探索（すべての候補をまとめて計算する）。区間は辺の前後1マスずつを含めた [−1, 2]
    let mut lo = vec![-1.0f64; cands.len()];
    let mut hi = vec![2.0f64; cands.len()];
    for _ in 0..24 {
        let (mut px, mut py) = (Vec::with_capacity(2 * cands.len()), Vec::with_capacity(2 * cands.len()));
        for (c, cand) in cands.iter().enumerate() {
            let d = (hi[c] - lo[c]) / 3.0;
            for t in [lo[c] + d, hi[c] - d] {
                let p = at(cand, t);
                px.push(p[0]);
                py.push(p[1]);
            }
        }
        let vals = eval_2d(prog, &px, &py);
        for (c, cand) in cands.iter().enumerate() {
            let g = |v: f64| if v.is_nan() { f64::INFINITY } else { cand.3 * v };
            let d = (hi[c] - lo[c]) / 3.0;
            if g(vals[2 * c]) > g(vals[2 * c + 1]) {
                lo[c] += d;
            } else {
                hi[c] -= d;
            }
        }
    }
    let ts: Vec<f64> = (0..cands.len()).map(|c| 0.5 * (lo[c] + hi[c])).collect();
    let ps: Vec<[f64; 2]> = cands.iter().zip(&ts).map(|(c, &t)| at(c, t)).collect();
    let vals = eval_2d(prog, &ps.iter().map(|p| p[0]).collect::<Vec<_>>(), &ps.iter().map(|p| p[1]).collect::<Vec<_>>());
    for (c, cand) in cands.iter().enumerate() {
        // 格子点にちょうどある極小は、t がほんのわずかずれても同じ辺の始点（t = 0）とみなす
        let t = if ts[c].abs() < 1e-6 { 0.0 } else { ts[c] };
        if (0.0..1.0).contains(&t) && vals[c].abs() <= 1e-3 * cand.4 {
            let p = if t == 0.0 { [xs[cand.1], ys[cand.1]] } else { ps[c] };
            out.insert(cand.0, (p, t));
        }
    }
    out
}

/// 格子の線の上には無い、0 に接するだけの1点（(x − 0.3)² + (y − 0.21)² = 0 など）を探す。
/// 格子点のうち、|F| が周りの 8 点のどれよりも小さく、9 点とも同じ符号のもの（F の極小・極大）の周りで、
/// x, y の向きに交互に三分探索して |F| の極小を求め、ほぼ 0（周りの平均の 10⁻³ 倍以下）なら点とする。
/// `skip(i, j)` が true の格子点（接するだけの線の近く）は調べない。候補はまとめて計算する
fn touch_points_2d(prog: &Program, f: &[f64], xs: &[f64], ys: &[f64], m: usize, skip: impl Fn(usize, usize) -> bool) -> Vec<[f64; 2]> {
    // 候補: (格子点, 符号, 周りの |F| の平均)
    let mut cands: Vec<(usize, f64, f64)> = Vec::new();
    'outer: for j in 1..m.saturating_sub(1) {
        for i in 1..m - 1 {
            let k = j * m + i;
            let c = f[k];
            if !c.is_finite() {
                continue;
            }
            let around = [k - m - 1, k - m, k - m + 1, k - 1, k + 1, k + m - 1, k + m, k + m + 1];
            let s = if c != 0.0 { c.signum() } else { f[k + 1].signum() };
            if !around.iter().all(|&q| f[q].is_finite() && f[q] * s > 0.0 && f[q].abs() > c.abs()) || skip(i, j) {
                continue;
            }
            cands.push((k, s, around.iter().map(|&q| f[q].abs()).sum::<f64>() / 8.0));
            if cands.len() >= TOUCH_2D_MAX / 10 {
                break 'outer;
            }
        }
    }
    if cands.is_empty() {
        return Vec::new();
    }
    // 格子点の周り（最初は前後1マス）で、x, y の向きに交互に三分探索する。探す幅は1周ごとに半分にする
    let (hx, hy) = (xs[1] - xs[0], ys[m] - ys[0]);
    let mut pos: Vec<[f64; 2]> = cands.iter().map(|&(k, _, _)| [xs[k], ys[k]]).collect();
    let val = |v: f64, s: f64| if v.is_nan() { f64::INFINITY } else { s * v };
    let mut w = [hx, hy];
    for _ in 0..6 {
        for axis in 0..2 {
            let mut lo: Vec<f64> = pos.iter().map(|p| p[axis] - w[axis]).collect();
            let mut hi: Vec<f64> = pos.iter().map(|p| p[axis] + w[axis]).collect();
            for _ in 0..12 {
                let (mut px, mut py) = (Vec::with_capacity(2 * pos.len()), Vec::with_capacity(2 * pos.len()));
                for (c, p) in pos.iter().enumerate() {
                    let d = (hi[c] - lo[c]) / 3.0;
                    for t in [lo[c] + d, hi[c] - d] {
                        let mut q = *p;
                        q[axis] = t;
                        px.push(q[0]);
                        py.push(q[1]);
                    }
                }
                let v = eval_2d(prog, &px, &py);
                for c in 0..pos.len() {
                    let s = cands[c].1;
                    let d = (hi[c] - lo[c]) / 3.0;
                    if val(v[2 * c], s) > val(v[2 * c + 1], s) {
                        lo[c] += d;
                    } else {
                        hi[c] -= d;
                    }
                }
            }
            for (c, p) in pos.iter_mut().enumerate() {
                p[axis] = 0.5 * (lo[c] + hi[c]);
            }
        }
        w = [0.5 * w[0], 0.5 * w[1]];
    }
    let v = eval_2d(prog, &pos.iter().map(|p| p[0]).collect::<Vec<_>>(), &pos.iter().map(|p| p[1]).collect::<Vec<_>>());
    cands
        .iter()
        .zip(pos)
        .zip(v)
        .filter(|&((&(k, _, mean), p), g)| (p[0] - xs[k]).abs() <= hx && (p[1] - ys[k]).abs() <= hy && g.abs() <= 1e-3 * mean)
        .map(|((_, p), _)| p)
        .collect()
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

/// 回す前の形で作った陰関数のメッシュ（変数は表示中の 3 つの座標）。回転が「表示中の座標どうし」だけなら、
/// 回転は形を変えずに動かすだけなので、角度が変わっても F を計算し直さず [`LocalMesh::project`] で描き直せる。
/// 回転の中心から表示範囲の角までの距離 ρ の立方体で作っておくので、どの角度に回しても表示範囲が欠けない
/// （はみ出た部分は描画側で切り落とす）
#[derive(Debug)]
pub struct LocalMesh {
    pub mesh: Mesh,
    /// 回す前の F（切り口のふた用）
    pub prog: Program,
    /// 次元・表示中の座標の番号・断面の値（表示していない座標は、この値のまま回る）
    pub dims: usize,
    pub axes: [usize; 3],
    pub slice: Vec<f64>,
}

impl LocalMesh {
    /// `view` の回転で、回す前の座標 p から表示中の座標 q への 1 次式 q = M p + t。
    /// 遠近法は使わない（陰関数は今までも遠近法なしで描いていた）
    pub fn affine(&self, view: &View) -> ([[f64; 3]; 3], [f64; 3]) {
        let mut m = [[0.0; 3]; 3];
        let mut t = [0.0; 3];
        for (a, (c0, cs)) in view.rows.iter().enumerate() {
            t[a] = *c0;
            for (k, &c) in cs.iter().enumerate().take(self.dims) {
                match self.axes.iter().position(|&x| x == k) {
                    Some(b) => m[a][b] = c,
                    None => t[a] += c * self.slice.get(k).copied().unwrap_or(0.0),
                }
            }
        }
        (m, t)
    }

    /// 回して表示中の座標のメッシュにする
    pub fn project(&self, view: &View) -> Mesh {
        let (m, t) = self.affine(view);
        let mv = |p: &[f32], shift: bool| -> [f32; 3] {
            [0, 1, 2].map(|a| {
                let v = (0..3).map(|b| m[a][b] * p[b] as f64).sum::<f64>() + if shift { t[a] } else { 0.0 };
                v as f32
            })
        };
        let move_mesh = |src: &Mesh| -> Mesh {
            Mesh {
                positions: src.positions.chunks_exact(3).flat_map(|p| mv(p, true)).collect(),
                normals: src.normals.chunks_exact(3).flat_map(|n| mv(n, false)).collect(),
                indices: src.indices.clone(),
                grid_coords: src.grid_coords.clone(),
                complex: src.complex.clone(),
                lod: src.lod.clone(),
                keep_full: src.keep_full,
                lod_mesh: None,
                note: src.note.clone(),
                stats: src.stats,
            }
        };
        let mut out = move_mesh(&self.mesh);
        out.lod_mesh = self.mesh.lod_mesh.as_deref().map(|l| Box::new(move_mesh(l)));
        out
    }

    /// 回したときの切り口のふたの元（表示中の座標 q から回す前の座標 p = Mᵀ (q − t)）
    pub fn field(&self, view: &View, dom: Domain) -> Field {
        let (m, t) = self.affine(view);
        let a = [0, 1, 2].map(|r| [0, 1, 2].map(|c| m[c][r]));
        let b = [0, 1, 2].map(|r| -(0..3).map(|c| m[c][r] * t[c]).sum::<f64>());
        Field::mapped(&self.prog, a, b, dom)
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
    // 陽関数から作った形（keep_far）は、有限かどうかだけを見る。遠い頂点を捨てると、その頂点を使う
    // 三角形（見えている部分を含む）ごと消えて、極の近くの面が欠けるため。
    // 4 次元の遠近法で視点の後ろに回った点（View::apply が NaN を返す）は、どちらでも捨てる
    let ok = |p: &[f64; 3]| {
        finite(p)
            && (shape.keep_far
                || (0..3).all(|a| {
                    let h = dom.hi[a] - dom.lo[a];
                    p[a] >= dom.lo[a] - h && p[a] <= dom.hi[a] + h
                }))
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
    mesh.keep_full = shape.keep_full;
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
        let jumps = find_jumps_with(&p, &us, &vs, &vals, &hs, &edges, &clamped, h * 0.005, &usable, early_exit);
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

    /// 三角形ごとの (xy 平面に落とした面積, 3 頂点の高さの差)
    fn tri_spreads(m: &Mesh) -> Vec<(f64, f64)> {
        m.indices
            .chunks_exact(3)
            .map(|t| {
                let v = |k: u32| {
                    let k = k as usize * 3;
                    [m.positions[k] as f64, m.positions[k + 1] as f64, m.positions[k + 2] as f64]
                };
                let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                let area = (((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) / 2.0).abs();
                let hi = a[2].max(b[2]).max(c[2]);
                let lo = a[2].min(b[2]).min(c[2]);
                (area, hi - lo)
            })
            .collect()
    }

    #[test]
    fn step_corners_have_no_spikes() {
        // floor(x) + floor(y) の段差の線が交わる所（格子の点）に、針のような三角形が立たない
        let d = Domain { lo: [-3.1, -3.1, -10.0], hi: [3.1, 3.1, 10.0] };
        let m = explicit_surface_full(&prog("floor(x) + floor(y)", &["x", "y"]), d, 60, 2, ComplexMode::Off, false);
        let tris = tri_spreads(&m);
        let total: f64 = tris.iter().map(|t| t.0).sum();
        let spikes: f64 = tris.iter().filter(|t| t.1 > 0.5).map(|t| t.0).sum();
        assert!((total - 6.2 * 6.2).abs() < 0.05, "面積 {total}");
        assert!(spikes <= 0.0005 * total, "針の面積 {spikes}（全体 {total}）");
    }

    #[test]
    fn low_steps_are_cut() {
        // 表示範囲の高さの 3% より低い段差（1 段 1/3、範囲の高さ 12.8）も、斜面ではなく段として描く
        let d = Domain { lo: [-5.0, -5.0, -6.4], hi: [5.0, 5.0, 6.4] };
        let m = explicit_surface(&prog("floor(3x)/3", &["x", "y"]), d, 60, 2);
        let slopes = tri_spreads(&m).iter().filter(|t| t.1 > 0.1).count();
        assert_eq!(slopes, 0);
    }

    /// 表示範囲の内側（縁から少し離れた所）で、1 つの三角形にしか使われていない辺の数（向こうが見える隙間の縁）。
    /// 壁は頂点を共有しないので、頂点の座標で辺をまとめる
    fn open_edges_inside(m: &Mesh, d: Domain) -> usize {
        let key = |i: u32| {
            let k = i as usize * 3;
            [0, 1, 2].map(|a| (m.positions[k + a] as f64 * 1e4).round() as i64)
        };
        let inside = |i: u32| {
            let k = i as usize * 3;
            (0..3).all(|a| {
                let x = m.positions[k + a] as f64;
                x > d.lo[a] + 1e-3 && x < d.hi[a] - 1e-3
            })
        };
        let mut count: crate::adaptive::HashMap<([i64; 3], [i64; 3]), usize> = Default::default();
        for t in m.indices.chunks_exact(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (ka, kb) = (key(a), key(b));
                // 壁の縦の辺は数えない（段の高さが場所で変わると、壁の端が隣の壁の途中に当たる T 字のつなぎ目になり、
                // 閉じていても 1 回しか使われない）
                if ka == kb || (ka[0] == kb[0] && ka[1] == kb[1]) || !inside(a) || !inside(b) {
                    continue;
                }
                *count.entry(if ka < kb { (ka, kb) } else { (kb, ka) }).or_default() += 1;
            }
        }
        count.values().filter(|&&c| c == 1).count()
    }

    #[test]
    fn step_corners_have_no_gaps() {
        // 段差の線が交わる所・三角形の中で終わる所に、向こうが見える隙間が開かない（壁つき）。
        // 段の角で、真ん中の三角形を空けたまま壁を立てていなかったとき、階段のサンプルで白い隙間が見えた
        for src in ["floor(x)/2 + floor(y)/4", "floor(x) + floor(y)", "round(x) round(y) / 8"] {
            let d = Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
            let m = explicit_surface_full(&prog(src, &["x", "y"]), d, 150, 2, ComplexMode::Off, true);
            // 修正前は 678・32・508 本
            assert_eq!(open_edges_inside(&m, d), 0, "{src}");
        }
    }

    #[test]
    fn touching_steps_are_drawn_on_both_sides() {
        // mod(x y, z) = 0 は x y = n z（n は整数）の面。z > 0 では F ∈ [0, z)、z < 0 では F ∈ (z, 0] で、
        // F は 0 に触れるだけで符号が変わらない。前は F = 0 ちょうどを正に数えて段差のセルを選んでいたので、
        // z < 0 の側しか描けなかった（z = x y の面が z > 0 で 16%、z < 0 で 97%）
        let d = Domain { lo: [-4.0, -4.0, -3.0], hi: [4.0, 4.0, 3.0] };
        let m = implicit_solid_opt(&prog("mod(x y, z)", &["x", "y", "z"]), d, 48, &[], 0, Refine::Normal);
        let h = 0.2;
        let cells: crate::adaptive::HashSet<[i64; 3]> =
            m.positions.chunks_exact(3).map(|p| [0, 1, 2].map(|a| (p[a] as f64 / h).floor() as i64)).collect();
        let near = |q: [f64; 3]| {
            let c = q.map(|v| (v / h).floor() as i64);
            (-1..=1).any(|a| (-1..=1).any(|b| (-1..=1).any(|e| cells.contains(&[c[0] + a, c[1] + b, c[2] + e]))))
        };
        for nn in [1.0, -1.0, 2.0] {
            let (mut hit, mut all) = ([0; 2], [0; 2]);
            for i in 0..=40 {
                for j in 0..=40 {
                    let (x, y) = (-4.0 + 0.2 * i as f64, -4.0 + 0.2 * j as f64);
                    let z = x * y / nn;
                    if z.abs() < 0.5 || z.abs() > 2.8 {
                        continue;
                    }
                    let side = (z < 0.0) as usize;
                    all[side] += 1;
                    hit[side] += near([x, y, z]) as usize;
                }
            }
            for side in 0..2 {
                let r = hit[side] as f64 / all[side] as f64;
                assert!(r > 0.9, "x y = {nn} z の面の、z {} 0 の側が {:.0}% しか描けていない", ["> ", "<"][side], 100.0 * r);
            }
        }
    }

    #[test]
    fn wall_normals_face_lower_side() {
        // 段差の壁の法線は、三角形の向き（表側）と同じで、段の低い側を向く。
        // 段差の線の向きは三角形ごとに入れ替わるので、そろっていないと壁が縞模様になる
        for (src, low) in [("floor(2 x)", -1.0), ("floor(-2 x)", 1.0), ("floor(2 x) + floor(2 y)", -1.0)] {
            let m = explicit_surface_full(&prog(src, &["x", "y"]), Domain { lo: [-1.0, -1.0, -3.0], hi: [1.0, 1.0, 3.0] }, 13, 2, ComplexMode::Off, true);
            let v = |i: u32| {
                let k = i as usize * 3;
                [m.positions[k] as f64, m.positions[k + 1] as f64, m.positions[k + 2] as f64]
            };
            let mut walls = 0;
            for t in m.indices.chunks_exact(3) {
                let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                let (e1, e2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
                let g = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                let len = (g[0] * g[0] + g[1] * g[1] + g[2] * g[2]).sqrt();
                // x = 0.5 の段差の壁（法線がほぼ ±x）
                if len < 1e-9 || (g[0] / len).abs() < 0.99 || ((a[0] + b[0] + c[0]) / 3.0 - 0.5).abs() > 1e-3 {
                    continue;
                }
                walls += 1;
                assert!(g[0] * low > 0.0, "{src}: 壁が段の高い側を向いている");
                for &i in t {
                    assert!(m.normals[i as usize * 3] as f64 * g[0] > 0.0, "{src}: 頂点の法線が面と逆");
                }
            }
            assert!(walls > 0, "{src}: 壁が無い");
        }
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
        let used: crate::adaptive::HashSet<usize> = m.indices.iter().map(|&k| k as usize).collect();
        used.iter().map(|&k| root(&mut parent, k)).collect::<crate::adaptive::HashSet<_>>().len()
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
        let dom = Domain { lo: [-1.0, -1.0, -10.0], hi: [1.0, 1.0, 10.0] };
        let shape = explicit_nd(&p, 2, [0, 1, 2], &[0.0; 3], dom, 4, ComplexMode::Abs, false, false, 20.0);
        assert_eq!(shape.complex.len(), shape.points.len()); // 3次元の点1つにつき3つ
        for q in shape.points.chunks_exact(3) {
            // |(x+iy)²| = x² + y²（頂点は f32 なので、その精度で）
            assert!((q[2] - (q[0] * q[0] + q[1] * q[1])).abs() < 1e-6);
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
        let mut count: crate::adaptive::HashMap<(u32, u32), usize> =
            crate::adaptive::HashMap::default();
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
        let mut count: crate::adaptive::HashMap<(u32, u32), usize> = crate::adaptive::HashMap::default();
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

    /// 形の正しさの報告（7.1）と同じ表示範囲
    const SHAPE_DOM: Domain = Domain { lo: [-10.0, -10.0, -6.4], hi: [10.0, 10.0, 6.4] };

    #[test]
    fn thin_peak_is_found() {
        // 幅 0.02 の山（格子の間隔 0.13）。格子の点がたまたま乗らないので、細分の前は、ほぼ平らなまま
        let m = explicit_surface(&prog("5 exp(-2000((x-1.23)^2 + (y-0.77)^2))", &["x", "y"]), SHAPE_DOM, 150, 2);
        let top = m.positions.chunks_exact(3).filter(|q| q[2] >= 4.5).count();
        assert!(top >= 1, "高さ 4.5 以上の頂点が無い");
    }

    #[test]
    fn thin_ridge_is_continuous() {
        // 幅 0.05 の曲がった尾根。どの x の所でも、尾根の上の頂点がある
        let m = explicit_surface(&prog("3 exp(-400 (y - sin(x))^2)", &["x", "y"]), SHAPE_DOM, 150, 2);
        for k in 0..39 {
            let x = -9.5 + 0.5 * k as f64;
            let top = m
                .positions
                .chunks_exact(3)
                .filter(|q| (q[0] as f64 - x).abs() < 0.1)
                .map(|q| q[2] as f64)
                .fold(f64::NEG_INFINITY, f64::max);
            assert!(top >= 2.5, "x = {x} の所の高さの最大が {top}");
        }
    }

    #[test]
    fn thin_pole_is_found() {
        // 極の半径は、高さ 6 で 0.02。表示範囲の上端（z ≥ 6.3）にかかる頂点がある
        let m = explicit_surface(&prog("1/(400 ((x-0.31)^2 + (y+0.57)^2))", &["x", "y"]), SHAPE_DOM, 150, 2);
        let top = m.positions.chunks_exact(3).filter(|q| q[2] >= 6.3).count();
        assert!(top >= 8, "上端にかかる頂点 {top} 個");
    }

    #[test]
    fn repeated_variables_are_not_refined_much() {
        // 同じ変数が何度も出る式（区間演算が広めになる）でも、三角形はほとんど増えない
        let d = Domain { lo: [-5.0, -5.0, -30.0], hi: [5.0, 5.0, 30.0] };
        for src in ["x sin(x) cos(y) / 3", "(x - y)(x + y)/10"] {
            let a = explicit_surface_opt(&prog(src, &["x", "y"]), d, 150, 2, ComplexMode::Off, false, false).indices.len();
            let b = explicit_surface(&prog(src, &["x", "y"]), d, 150, 2).indices.len();
            assert!(b as f64 <= 1.05 * a as f64, "{src}: {} → {}", a / 3, b / 3);
        }
    }

    /// 媒介変数の曲面の三角形（3 頂点の点）
    fn shape_tris(s: &NdShape) -> Vec<[[f64; 3]; 3]> {
        let p = |k: u32| {
            let k = k as usize * s.dims;
            [s.points[k], s.points[k + 1], s.points[k + 2]]
        };
        let (_, tris) = s.tris.as_ref().expect("細かくした三角形");
        tris.chunks_exact(3).map(|t| [p(t[0]), p(t[1]), p(t[2])]).collect()
    }

    #[test]
    fn parametric_steps_are_cut() {
        // 段と段を結ぶ三角形（3 頂点の z の差が 0.4 を超えるもの）が無い（変更前は 4,500 個・13,500 個）
        let r = (-5.0, 5.0);
        let tol = 3f64.sqrt() * 10.0 * 1e-3;
        for src in [["u", "v", "floor(u)"], ["u cos(v)", "u sin(v)", "floor(3u)/2"]] {
            let s = parametric_surface_nd_opt(&[progs3(src, &["u", "v"])], r, r, 150, Some(tol));
            assert!(s.keep_full, "{src:?}");
            let bad = shape_tris(&s)
                .iter()
                .filter(|t| t.iter().all(|q| q.iter().all(|x| x.is_finite())))
                .filter(|t| {
                    let z = t.map(|q| q[2]);
                    z.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)) - z.iter().fold(f64::INFINITY, |a, &b| a.min(b)) > 0.4
                })
                .count();
            assert_eq!(bad, 0, "{src:?}");
        }
    }

    #[test]
    fn parametric_pole_has_no_bridge() {
        // tan(u) の極をまたぐ偽の面（z > 3 と z < −3 の頂点を両方持つ三角形）が無い
        let r = (-5.0, 5.0);
        let tol = 3f64.sqrt() * 10.0 * 1e-3;
        let s = parametric_surface_nd_opt(&[progs3(["u", "v", "tan(u)"], &["u", "v"])], r, r, 150, Some(tol));
        let bridges = shape_tris(&s)
            .iter()
            .filter(|t| t.iter().any(|q| q[2] > 3.0) && t.iter().any(|q| q[2] < -3.0))
            .count();
        assert_eq!(bridges, 0);
        assert!(s.keep_far);
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
        // （計算する範囲は幅の 1e-7 だけ内側に縮めてあるので、全体も 4 よりわずかに小さい）
        assert!((a_full - 4.0).abs() < 1e-5);
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
        let mut edges = crate::adaptive::HashMap::default();
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
        let mut count = crate::adaptive::HashMap::default();
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
            let a = implicit_solid_opt(&p, d, n, &ranges, 8, Refine::Off);
            let b = implicit_solid_reference(&p, d, n, &ranges, 8);
            assert!(!a.indices.is_empty(), "{src}");
            // 空のブロックを飛ばすようにして（S9）、頂点の順番はブロックの順になったので、集合で比べる
            assert!(mesh_sets(&a) == mesh_sets(&b), "{src}: 頂点・三角形の集合が違う");
        }
    }

    /// メッシュのつながった成分の数（頂点を三角形でつないだもの）
    fn components(m: &Mesh) -> usize {
        let nv = m.positions.len() / 3;
        let mut p: Vec<usize> = (0..nv).collect();
        fn root(p: &mut [usize], mut x: usize) -> usize {
            while p[x] != x {
                p[x] = p[p[x]];
                x = p[x];
            }
            x
        }
        for t in m.indices.chunks_exact(3) {
            for k in 1..3 {
                let (a, b) = (root(&mut p, t[0] as usize), root(&mut p, t[k] as usize));
                p[a] = b;
            }
        }
        let used: crate::adaptive::HashSet<usize> = m.indices.iter().map(|&v| root(&mut p, v as usize)).collect();
        used.len()
    }

    /// 中心 c・半径 r の球の頂点の数と、|中心からの距離 − r| の最大
    fn sphere_fit(m: &Mesh, c: f64, r: f64) -> (usize, f64) {
        let err = m
            .positions
            .chunks_exact(3)
            .map(|q| (q.iter().map(|&v| (v as f64 - c).powi(2)).sum::<f64>().sqrt() - r).abs())
            .fold(0.0, f64::max);
        (m.positions.len() / 3, err)
    }

    #[test]
    fn small_sphere_is_found() {
        // 範囲 ±10・細かさ 80（h = 0.25）で、格子より小さい球。細分しないと、半径 0.1 の球は頂点が 0 個
        let d = Domain { lo: [-10.0; 3], hi: [10.0; 3] };
        let small = prog("(x-0.12)^2+(y-0.12)^2+(z-0.12)^2-0.01", &AXES);
        assert_eq!(implicit_solid_opt(&small, d, 80, &[], 0, Refine::Off).positions.len(), 0);
        let m = implicit_surface(&small, d, 80);
        let (count, err) = sphere_fit(&m, 0.12, 0.1);
        assert!(count >= 20, "頂点 {count} 個");
        assert!(err < 0.02, "半径のずれ {err}");
        assert_eq!(bad_edges(&m), (0, 0));
        assert_eq!(components(&m), 1);
        // 半径 0.3 の球（細分しないと頂点 26 個）
        let mid = prog("(x-0.12)^2+(y-0.12)^2+(z-0.12)^2-0.09", &AXES);
        let before = implicit_solid_opt(&mid, d, 80, &[], 0, Refine::Off).positions.len() / 3;
        let m = implicit_surface(&mid, d, 80);
        let (count, err) = sphere_fit(&m, 0.12, 0.3);
        assert!(count >= 100, "頂点 {count} 個（細分しないと {before} 個）");
        assert!(err < 0.03, "半径のずれ {err}");
        assert_eq!(bad_edges(&m), (0, 0));
    }

    #[test]
    fn small_sphere_keeps_its_detail_at_any_radius() {
        // 半径をいくら小さくしても、形の細かさ（頂点の数）は変わらず、なめらかな球になる（ふつう・軽く）
        let d = Domain {
            lo: [-5.0; 3],
            hi: [5.0; 3],
        };
        for (refine, n, min_count) in [(Refine::Normal, 80, 1500), (Refine::Light, 20, 500)] {
            for r in [1e-4, 1e-3, 0.01, 0.1, 0.3, 0.7, 1.0, 2.0, 2.4] {
                let (cx, cy, cz) = (0.12, -0.07, 0.03);
                let src = format!("(x-{cx})^2+(y-({cy}))^2+(z-{cz})^2-{}", r * r);
                let m = implicit_solid_opt(&prog(&src, &AXES), d, n, &[], 0, refine);
                let count = m.positions.len() / 3;
                let err = m
                    .positions
                    .chunks_exact(3)
                    .map(|q| {
                        (((q[0] as f64 - cx).powi(2)
                            + (q[1] as f64 - cy).powi(2)
                            + (q[2] as f64 - cz).powi(2))
                        .sqrt()
                            - r)
                            .abs()
                    })
                    .fold(0.0, f64::max);
                assert!(count >= min_count, "{refine:?} r = {r}: 頂点 {count} 個");
                assert!(err < 0.02 * r, "{refine:?} r = {r}: 半径のずれ {err}");
                assert_eq!(bad_edges(&m), (0, 0), "{refine:?} r = {r}");
                assert_eq!(components(&m), 1, "{refine:?} r = {r}");
            }
        }
    }

    #[test]
    fn small_sphere_without_intervals_is_found() {
        // 区間演算が使えない式（複素数を通る式）でも、勾配からの見積もりで見つける。|exp(i x)| = 1
        let d = Domain { lo: [-10.0; 3], hi: [10.0; 3] };
        let p = prog("abs(exp(i x)) ((x-0.12)^2+(y-0.12)^2+(z-0.12)^2) - 0.04", &AXES);
        assert!(!p.supports_interval());
        let before = implicit_solid_opt(&p, d, 80, &[], 0, Refine::Off).positions.len() / 3;
        let m = implicit_surface(&p, d, 80);
        let (count, err) = sphere_fit(&m, 0.12, 0.2);
        assert!(count >= 50 && count > 2 * before, "頂点 {count} 個（細分しないと {before} 個）");
        assert!(err < 0.03, "半径のずれ {err}");
        assert_eq!(bad_edges(&m), (0, 0));
    }

    #[test]
    fn thin_torus_is_closed() {
        // 管の半径 0.05、h = 0.125。つながった 1 つの閉じた曲面になる
        let p = prog("(sqrt(x^2+y^2)-3)^2+z^2-0.0025", &AXES);
        let m = implicit_surface(&p, Domain { lo: [-5.0; 3], hi: [5.0; 3] }, 80);
        assert_eq!(bad_edges(&m), (0, 0));
        assert_eq!(components(&m), 1);
    }

    #[test]
    fn refinement_never_cracks() {
        // 大きい球と小さい球を掛け合わせた式・近い 2 つの球で、縁の辺が無い
        let big = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        for (src, d, n) in [
            ("(x^2+y^2+z^2-4) ((x-0.12)^2+(y-0.12)^2+(z-0.12)^2-0.01)", big, 80),
            ("(x^2+y^2+z^2-4) ((x-2.6)^2+(y-0.1)^2+(z-0.1)^2-0.0225)", big, 80),
            ("(x^2 + y^2 + z^2 - 1) (x^2 + y^2 + z^2 - 1.3225)", Domain { lo: [-2.0; 3], hi: [2.0; 3] }, 40),
        ] {
            let m = implicit_surface(&prog(src, &AXES), d, n);
            assert_eq!(bad_edges(&m).0, 0, "{src}");
        }
    }

    #[test]
    fn flags_bit0_skips_refinement() {
        // 細分しないなら、細分を足す前（S9）と同じ結果
        let p = prog("(x-0.12)^2+(y-0.12)^2+(z-0.12)^2-0.09", &AXES);
        let d = Domain { lo: [-10.0; 3], hi: [10.0; 3] };
        let off = implicit_solid_opt(&p, d, 80, &[], 0, Refine::Off);
        let s9 = implicit_solid_with(&p, d, 80, &[], 0, false, Refine::Off);
        assert_eq!(off.positions, s9.positions);
        assert_eq!(off.indices, s9.indices);
        assert_eq!(off.positions.len() / 3, 26);
        assert!(implicit_surface(&p, d, 80).positions.len() / 3 > 26);
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
    fn implicit_domain_edge_is_smooth() {
        // sqrt(36 − x² − y²) − z = 0（半径 6 の半球）は x² + y² ≤ 36 でしか定義されない。面は縁まで延び、
        // 縁の辺（三角形が 1 つだけの辺）は、定義域の縁（円柱 x² + y² = 36）と半球の上に乗る（格子の間隔 0.25。
        // 変更前は 1 マスほど内側で、ギザギザに途切れていた）。
        // 縁では面が垂直なので、Surface Nets の角の丸まりで、縁の辺は z = 0 より 1 マスほど上で切れる（z ≤ 2 マスまで許す）
        let d = Domain { lo: [-10.0; 3], hi: [10.0; 3] };
        let m = implicit_surface(&prog("sqrt(36 - x^2 - y^2) - z", &AXES), d, 80);
        let mut count: crate::adaptive::HashMap<(u32, u32), usize> = Default::default();
        for t in m.indices.chunks_exact(3) {
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let p = |v: u32| [0, 1, 2].map(|a| m.positions[3 * v as usize + a] as f64);
        let mut border = 0;
        let (mut off_edge, mut off_sphere, mut top) = (0.0f64, 0.0f64, f64::NEG_INFINITY);
        for (&(a, b), &c) in &count {
            if c != 1 {
                continue;
            }
            for v in [a, b] {
                let q = p(v);
                if q.iter().any(|x| x.abs() >= 10.0) {
                    continue;
                }
                border += 1;
                let r = (q[0] * q[0] + q[1] * q[1]).sqrt();
                off_edge = off_edge.max((r - 6.0).abs());
                off_sphere = off_sphere.max(((r * r + q[2] * q[2]).sqrt() - 6.0).abs());
                top = top.max(q[2]);
            }
        }
        assert!(border > 100, "縁の頂点 {border} 個");
        assert!(off_edge <= 0.05, "縁の頂点が定義域の縁から {off_edge} 離れている");
        assert!(off_sphere <= 0.05, "縁の頂点が半球から {off_sphere} 離れている");
        assert!(top <= 0.5, "縁の頂点の高さが {top}");
    }

    #[test]
    fn implicit_step_edge_is_straight() {
        // floor(x) − z + y/5 = 0（陽関数に直さず、陰関数のまま）。面は段差 x = n の所で終わり、その縁の辺（三角形が 1 つだけの辺）は
        // 平面 x = n にまっすぐ乗る（格子の間隔 0.25。枝ごとに作って切り取る前は、格子に沿ってギザギザだった）
        let d = Domain { lo: [-10.0; 3], hi: [10.0; 3] };
        let m = implicit_surface(&prog("floor(x) - z + y/5", &AXES), d, 80);
        let mut count: crate::adaptive::HashMap<(u32, u32), usize> = Default::default();
        for t in m.indices.chunks_exact(3) {
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let (mut border, mut worst) = (0, 0.0f64);
        for (&(a, b), &c) in &count {
            if c != 1 {
                continue;
            }
            for v in [a, b] {
                let q = [0, 1, 2].map(|k| m.positions[3 * v as usize + k] as f64);
                if q.iter().any(|x| x.abs() >= 9.9) {
                    continue;
                }
                border += 1;
                worst = worst.max((q[0] - q[0].round()).abs());
            }
        }
        assert!(border > 100, "縁の頂点 {border} 個");
        assert!(worst <= 0.02, "縁が整数の平面から {worst} 離れている");
    }

    /// 縁の辺（三角形が 1 つだけの辺。範囲 `inner` の外の頂点を持つものは数えない）と、3 つ以上の三角形が集まる辺の数
    fn crack_counts(m: &Mesh, inner: f64) -> (usize, usize) {
        let mut count: crate::adaptive::HashMap<(u32, u32), usize> = Default::default();
        for t in m.indices.chunks_exact(3) {
            for e in 0..3 {
                let (a, b) = (t[e], t[(e + 1) % 3]);
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let inside = |v: u32| (0..3).all(|k| (m.positions[3 * v as usize + k] as f64).abs() < inner);
        let border = count.iter().filter(|(&(a, b), &c)| c == 1 && inside(a) && inside(b)).count();
        let many = count.values().filter(|&&c| c >= 3).count();
        (border, many)
    }

    #[test]
    fn stitched_refinement_has_no_cracks() {
        // 細い部分（円錐の先・細い首）を 2 倍の細かさで作り直しても、継ぎ目にひびが無い
        let d = Domain { lo: [-10.0; 3], hi: [10.0; 3] };
        for src in ["x^2 + y^2 - z^2", "(x^2+y^2+z^2)^2 - 30 x y z", "x^2 + y^2 - (z^2 - 4)^2 / 20 - 0.01"] {
            let m = implicit_surface(&prog(src, &AXES), d, 80);
            assert_eq!(crack_counts(&m, 9.9), (0, 0), "{src}");
        }
        // 円錐の先: 原点の近くに頂点がある（作り直す前は、格子の間隔 0.25 で丸まる）
        let m = implicit_surface(&prog("x^2 + y^2 - z^2", &AXES), d, 80);
        let before = implicit_solid_opt(&prog("x^2 + y^2 - z^2", &AXES), d, 80, &[], 0, Refine::Off);
        let tip = |m: &Mesh| m.positions.chunks_exact(3).map(|q| q.iter().map(|&v| (v as f64).powi(2)).sum::<f64>().sqrt()).fold(f64::INFINITY, f64::min);
        eprintln!("TIP {} (before {})", tip(&m), tip(&before));
        assert!(tip(&m) < tip(&before), "{} / {}", tip(&m), tip(&before));
    }

    #[test]
    fn stitched_seams_have_no_cracks() {
        // ブロックを市松模様に選んで 2 倍の細かさにし（継ぎ目がたくさんある状態）、継ぎ目にひびが無いか。
        // 円錐の先・細い首・(x² + y² + z²)² = 30 x y z
        let d = Domain { lo: [-5.0; 3], hi: [5.0; 3] };
        let n = 40;
        for src in ["x^2 + y^2 - z^2", "(x^2+y^2+z^2)^2 - 30 x y z", "x^2 + y^2 - (z^2 - 4)^2 / 20 - 0.01"] {
            let p = prog(src, &AXES);
            let combos = sample_combos(&[], 0);
            let field = SparseField::build(&p, &combos, &[], d, n, true);
            let nb = field.nb;
            let mut sel = crate::adaptive::HashSet::default();
            for bz in 0..nb {
                for by in 0..nb {
                    for bx in 0..nb {
                        if (bx + by + bz) % 2 == 0 {
                            sel.insert(bx + nb * (by + nb * bz));
                        }
                    }
                }
            }
            let mut sc = FieldScratch::default();
            let mut eval = |xyz: [&[f64]; 3]| field_values_with(&p, &combos, xyz, &mut sc).to_vec();
            let (mut m, _) = stitched_nets(&field, d, n, &sel, &mut eval);
            compute_normals(&mut m);
            assert!(m.indices.len() > 1000, "{src}");
            assert_eq!(crack_counts(&m, 4.5), (0, 0), "{src}");
        }
    }

    #[test]
    fn steep_root_is_still_a_surface() {
        // 急なだけで本当に 0 になる所は、今までどおり面を張る（頂点の数が、極の判定をしない版と同じ）。
        // 小さい図形の作り直し（半径 1 の球は 20 マスなので作り直す）は比べない
        let d = Domain { lo: [-2.0; 3], hi: [2.0; 3] };
        for src in ["x^3 - z", "x^(1/3) - z", "1000 (x^2+y^2+z^2-1)", "exp(5x) - z - 1"] {
            let p = prog(src, &AXES);
            let a = implicit_solid_opt(&p, d, 40, &[], 0, Refine::Off);
            let b = implicit_solid_reference(&p, d, 40, &[], 0);
            assert!(!a.indices.is_empty(), "{src}");
            if src == "x^(1/3) - z" {
                // x < 0 では定義されない。S22 から、面が定義域の縁 x = 0 まで延びるので、頂点は比べる版より多い
                assert!(a.positions.len() >= b.positions.len(), "{src}");
                continue;
            }
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
            let sparse = implicit_solid_with(&p, d, n, &ranges, 8, false, Refine::Off);
            let dense = implicit_solid_with(&p, d, n, &ranges, 8, true, Refine::Off);
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
        let mut count: crate::adaptive::HashMap<(u32, u32), usize> = crate::adaptive::HashMap::default();
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

    /// 表示範囲の端から `margin` より内側にある、1 つの三角形にしか使われない辺（穴の縁）の数。頂点は位置でまとめる
    fn inner_border_edges(m: &Mesh, d: Domain, margin: f64) -> usize {
        let mut ids: crate::adaptive::HashMap<[i64; 3], u32> = Default::default();
        let vid: Vec<u32> = m
            .positions
            .chunks_exact(3)
            .map(|p| {
                let next = ids.len() as u32;
                *ids.entry([0, 1, 2].map(|a| (p[a] as f64 * 1e4).round() as i64)).or_insert(next)
            })
            .collect();
        let mut count: crate::adaptive::HashMap<(u32, u32), (usize, [f64; 3])> = Default::default();
        for t in m.indices.chunks_exact(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (va, vb) = (vid[a as usize], vid[b as usize]);
                if va == vb {
                    continue;
                }
                let mid = [0, 1, 2].map(|k| 0.5 * (m.positions[3 * a as usize + k] + m.positions[3 * b as usize + k]) as f64);
                count.entry((va.min(vb), va.max(vb))).or_insert((0, mid)).0 += 1;
            }
        }
        count.values().filter(|(c, q)| *c == 1 && (0..3).all(|k| q[k] > d.lo[k] + margin && q[k] < d.hi[k] - margin)).count()
    }

    #[test]
    fn gyroid_has_no_holes() {
        // ジャイロイドは表示範囲の中で縁の無い面。面が格子の平面にほぼ重なる所（x = ±7π/4 など）では、格子の面の上で F の符号が
        // 市松に入れ替わる。面の両側のセルで「どちらの対角がつながるか」の判断が食い違うと、四角形が張られず穴が開いていた。
        // 細かさ 80（粗い版も）・40・24（回転の再生中）のどれでも、穴の縁が無い
        let d = Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
        let p = prog("sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x)", &AXES);
        for n in [80, 40, 24] {
            let m = implicit_solid_opt(&p, d, n, &[], 0, Refine::Light);
            assert!(m.indices.len() > 10_000);
            assert_eq!(inner_border_edges(&m, d, 0.4), 0, "細かさ {n}");
            if let Some(lod) = &m.lod_mesh {
                assert_eq!(inner_border_edges(lod, d, 0.4), 0, "細かさ {n} の粗い版");
            }
        }
        assert!(implicit_surface(&p, d, 80).lod_mesh.is_some());
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

    /// 2D の曲線のメッシュを、線分（両端が違う）と点（両端が同じ）に分ける
    fn segments_and_dots(m: &Mesh) -> (Vec<[f32; 4]>, Vec<[f32; 2]>) {
        let (mut segs, mut dots) = (Vec::new(), Vec::new());
        for q in m.positions.chunks_exact(9) {
            if q[0..3] == q[3..6] {
                dots.push([q[0], q[1]]);
            } else {
                segs.push([q[0], q[1], q[3], q[4]]);
            }
        }
        (segs, dots)
    }

    #[test]
    fn flat_curve_touching_zero() {
        let d = Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
        // 0 に接するだけの円: 線になる（点は描かない）
        let m = implicit_curve_2d(&prog("(x^2 + y^2 - 4)^2", &["x", "y"]), d, 320);
        let (segs, dots) = segments_and_dots(&m);
        assert!(dots.is_empty(), "円の上に点: {dots:?}");
        assert!(segs.len() > 100, "線分 {} 本", segs.len());
        for s in &segs {
            for p in [[s[0], s[1]], [s[2], s[3]]] {
                assert!((p[0].hypot(p[1]) - 2.0).abs() < 0.02, "円から外れた点 {p:?}");
            }
        }
        // 0 に接するだけの1点: 格子点の上（原点）でも、格子の線から外れた所でも、点を1つだけ描く
        for (src, at) in [("x^2 + y^2", [0.0, 0.0]), ("(x - 0.3)^2 + (y - 0.21)^2", [0.3, 0.21]), ("abs(x - 1.1) + 2 (y + 0.37)^2", [1.1, -0.37])] {
            let m = implicit_curve_2d(&prog(src, &["x", "y"]), d, 320);
            let (segs, dots) = segments_and_dots(&m);
            assert!(segs.is_empty(), "{src}: 線分 {}", segs.len());
            assert_eq!(dots.len(), 1, "{src}: 点 {dots:?}");
            assert!((dots[0][0] - at[0]).abs() < 1e-3 && (dots[0][1] - at[1]).abs() < 1e-3, "{src}: {:?}", dots[0]);
        }
        // 0 にならない（極小が 0 より大きい）: 何も描かない
        for src in ["x^2 + y^2 + 1", "(x - 0.3)^2 + (y - 0.21)^2 + 0.01"] {
            let m = implicit_curve_2d(&prog(src, &["x", "y"]), d, 320);
            assert!(m.positions.is_empty(), "{src}: {:?}", segments_and_dots(&m));
        }
        // ふつうの曲線（符号が変わる）には、余計な点を足さない
        let (_, dots) = segments_and_dots(&implicit_curve_2d(&prog("x^2 + y^2 - 4", &["x", "y"]), d, 320));
        assert!(dots.is_empty());
    }

    #[test]
    fn rotated_slice_has_no_tiny_segments() {
        // 回した階段の切り口: 格子点をちょうど通る所で、ほぼ同じ点の組（描画側では丸に見える）を作らない
        let p = prog("floor(x)/2 + floor(y)/4", &["x", "y"]);
        let base = Domain { lo: [-10.0, -10.0, -6.0], hi: [10.0, 10.0, 6.0] };
        let ext = Domain { lo: [-15.5; 3], hi: [15.5; 3] };
        for deg in [0.5f64, 30.0, 82.6, 133.9] {
            let a = deg.to_radians();
            let shape = explicit_nd(&p, 2, [0, 1, 2], &[0.0; 3], ext, 200, ComplexMode::Off, true, false, 12.0);
            let view = View {
                rows: [(0.0, vec![a.cos(), -a.sin(), 0.0]), (0.0, vec![a.sin(), a.cos(), 0.0]), (0.0, vec![0.0, 0.0, 1.0])],
                center: [0.0; 3],
                hidden: Vec::new(),
                dist: 0.0,
            };
            let m = slice_mesh(&project(&shape, &view, base), 0.0);
            assert!(!m.positions.is_empty());
            for seg in m.positions.chunks_exact(9) {
                let d: f32 = (0..3).map(|c| (seg[c] - seg[3 + c]).abs()).sum();
                assert!(d > 1e-4, "{deg}°: とても短い線 {:?}", &seg[0..6]);
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


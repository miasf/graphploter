//! 2 次元の範囲の三角形分割を、形が急に変わる所だけ細かくする（適応格子）。
//!
//! 「最も新しい頂点の向かいの辺（斜辺）を二分する」方式（newest vertex bisection）。
//! - 最初の分割は一様な格子（n × n マスを対角線で 2 つに分けたもの）。斜辺は対角線
//! - 三角形 [p, q, r]（斜辺 p–q、向かいの頂点 r）を、斜辺の中点 m で [r, p, m] と [q, r, m] に分ける。
//!   子の斜辺は、新しい頂点 m の向かいの辺（r–p と q–r）になる。三角形の向き（表裏）は変わらない
//! - 三角形を分けるときは、斜辺の向こうの三角形も同じ辺を斜辺にしていなければならない。していなければ、
//!   先に向こうの三角形をその斜辺で分ける（その子の 1 つが、ちょうどこの辺を斜辺に持つ）。
//!   こうすると、辺の途中に別の三角形の頂点が乗る「T 字の継ぎ目」ができない（面に隙間ができない）
//! - 1 回のパスで、調べる三角形の斜辺の中点を集めて（辺ごとに 1 回）まとめて計算する

use std::collections::{HashMap as StdMap, HashSet as StdSet};
use std::hash::{BuildHasherDefault, Hasher};

/// 整数の組を鍵にする表用の、速いハッシュ（標準の SipHash は、ここでは遅すぎる）
#[derive(Default, Clone, Copy)]
pub struct FastHasher(u64);

impl Hasher for FastHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_u32(&mut self, x: u32) {
        self.write_u64(x as u64);
    }
    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_usize(&mut self, x: usize) {
        self.write_u64(x as u64);
    }
}

pub type HashMap<K, V> = StdMap<K, V, BuildHasherDefault<FastHasher>>;
pub type HashSet<K> = StdSet<K, BuildHasherDefault<FastHasher>>;

/// 細かくする回数と量の上限
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// パスの回数（1 回のパスで、分けたい三角形を 1 回ずつ分ける）
    pub passes: usize,
    /// 足してよい三角形の数。近づいたら、ずれの大きい三角形から分ける
    pub max_new_triangles: usize,
}

/// `decide` に渡す、調べる三角形
pub struct Candidate<'a, V> {
    /// 頂点の番号 [斜辺の始点, 斜辺の終点, 向かいの頂点]
    pub ids: [u32; 3],
    /// 頂点の値（ids と同じ順）
    pub vals: [&'a V; 3],
    /// 斜辺の中点の値
    pub mid: &'a V,
    /// この三角形を作った分割のずれ（親）と、その前の分割のずれ（祖父母）。最初の格子の三角形は NaN
    pub history: [f64; 2],
}

/// 細かくした結果
pub struct Refined<V> {
    /// 頂点の (u, v)。最初の (n + 1)² 個は一様な格子の点（j * (n + 1) + i の順）
    pub uv: Vec<[f64; 2]>,
    /// 頂点の値
    pub vals: Vec<V>,
    /// 三角形（向きは最初の格子の三角形 (a, b, d), (a, d, c) と同じ）
    pub tris: Vec<[u32; 3]>,
}

type Edge = (u32, u32);

/// 点 (u, v) の列での値をまとめて計算する関数
pub type Eval<'a, V> = dyn FnMut(&[f64], &[f64]) -> Vec<V> + 'a;

/// 隣接表の空き
const NO: u32 = u32::MAX;

/// 辺 e を持つ三角形に t を足す
fn link(adj: &mut HashMap<Edge, [u32; 2]>, e: Edge, t: u32) {
    let slot = adj.entry(e).or_insert([NO, NO]);
    if slot[0] == NO {
        slot[0] = t;
    } else {
        debug_assert!(slot[1] == NO, "3 つ目の三角形");
        slot[1] = t;
    }
}

fn key(a: u32, b: u32) -> Edge {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

struct Tri {
    /// [斜辺の始点, 斜辺の終点, 向かいの頂点]
    v: [u32; 3],
    history: [f64; 2],
    alive: bool,
}

impl Tri {
    fn edge(&self) -> Edge {
        key(self.v[0], self.v[1])
    }
}

struct State<V> {
    uv: Vec<[f64; 2]>,
    vals: Vec<V>,
    tris: Vec<Tri>,
    /// 最初の格子の 1 辺のマス数と、最初の三角形の数（2n²）。最初の三角形がどの辺を持つかは計算で分かるので、
    /// 表には、分けてできた三角形だけを入れる（なめらかな所の何万もの三角形を表に入れると遅い）
    n: usize,
    base_tris: u32,
    /// 辺 → その辺を持つ、分けてできた（生きている）三角形（2 つまで。空きは NO）
    adj: HashMap<Edge, [u32; 2]>,
    /// 計算した辺の中点（使うかどうかはまだ分からない）
    mids: HashMap<Edge, ([f64; 2], V)>,
    /// 中点に頂点を置いた辺 → 頂点の番号
    placed: HashMap<Edge, u32>,
    /// このパスで作った三角形
    born: Vec<u32>,
}

impl<V: Clone> State<V> {
    fn add_tri(&mut self, v: [u32; 3], history: [f64; 2]) -> u32 {
        let id = self.tris.len() as u32;
        self.tris.push(Tri {
            v,
            history,
            alive: true,
        });
        for e in [key(v[0], v[1]), key(v[1], v[2]), key(v[2], v[0])] {
            link(&mut self.adj, e, id);
        }
        id
    }

    /// 辺 e を持つ、生きている三角形（2 つまで）
    fn adjacent(&self, e: Edge) -> [u32; 2] {
        let mut out = [NO, NO];
        let mut k = 0;
        let mut add = |t: u32| {
            if k < 2 && t != NO && self.tris[t as usize].alive {
                out[k] = t;
                k += 1;
            }
        };
        let m = self.n + 1;
        let total = (m * m) as u32;
        if e.1 < total {
            // 最初の格子の辺（両端とも格子の点）: 持っている最初の三角形を計算で求める
            // （マス s = j n + i の三角形 2s は [d, a, b]、2s + 1 は [a, d, c]）
            let (k0, d) = (e.0 as usize, (e.1 - e.0) as usize);
            let (i, j) = (k0 % m, k0 / m);
            let sq = |i: usize, j: usize| (j * self.n + i) as u32;
            if d == 1 && i < self.n {
                // 横の辺: マス (i, j) の下の辺と、マス (i, j − 1) の上の辺
                if j < self.n {
                    add(2 * sq(i, j));
                }
                if j > 0 {
                    add(2 * sq(i, j - 1) + 1);
                }
            } else if d == m {
                // 縦の辺: マス (i, j) の左の辺と、マス (i − 1, j) の右の辺
                if i < self.n && j < self.n {
                    add(2 * sq(i, j) + 1);
                }
                if i > 0 && j < self.n {
                    add(2 * sq(i - 1, j));
                }
            } else if d == m + 1 && i < self.n && j < self.n {
                add(2 * sq(i, j));
                add(2 * sq(i, j) + 1);
            }
        }
        if let Some(slot) = self.adj.get(&e) {
            for &t in slot {
                add(t);
            }
        }
        out
    }

    fn remove_tri(&mut self, id: u32) {
        let v = self.tris[id as usize].v;
        self.tris[id as usize].alive = false;
        if id < self.base_tris {
            return; // 最初の三角形は表に入っていない
        }
        for e in [key(v[0], v[1]), key(v[1], v[2]), key(v[2], v[0])] {
            if let Some(slot) = self.adj.get_mut(&e) {
                for s in slot.iter_mut() {
                    if *s == id {
                        *s = NO;
                    }
                }
                if slot == &[NO, NO] {
                    self.adj.remove(&e);
                }
            }
        }
    }

    /// 辺 e の中点の頂点（無ければ置く。中点を計算していなければ、ここで 1 点だけ計算する）
    fn mid_vertex(&mut self, e: Edge, eval: &mut Eval<'_, V>) -> u32 {
        if let Some(&k) = self.placed.get(&e) {
            return k;
        }
        let (uv, val) = match self.mids.remove(&e) {
            Some(x) => x,
            None => {
                let (a, b) = (self.uv[e.0 as usize], self.uv[e.1 as usize]);
                let m = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
                let v = eval(&[m[0]], &[m[1]]).swap_remove(0);
                (m, v)
            }
        };
        let k = self.uv.len() as u32;
        self.uv.push(uv);
        self.vals.push(val);
        self.placed.insert(e, k);
        k
    }

    /// 辺 e を二分する（e を持つ三角形をすべて分ける。斜辺が e でない三角形は、先にその斜辺で分ける）
    fn split_edge(&mut self, e: Edge, err: f64, eval: &mut Eval<'_, V>) {
        while let Some(t) = self.adjacent(e).into_iter().find(|&t| t != NO) {
            let tri = &self.tris[t as usize];
            if tri.edge() == e {
                let [p, q, r] = tri.v;
                // 分けた三角形のずれは、分けると決めた三角形のものを子に伝える（強制的に分けた三角形は NaN のまま）
                let parent = tri.history[0];
                let mine = if err.is_nan() { parent } else { err };
                let history = [mine, parent];
                let m = self.mid_vertex(e, eval);
                self.remove_tri(t);
                let a = self.add_tri([r, p, m], history);
                let b = self.add_tri([q, r, m], history);
                self.born.extend([a, b]);
            } else {
                // 向こうの三角形を先に、その斜辺で分ける（深さは有限: 斜辺は分けるたびに短くなる）
                let other = tri.edge();
                self.split_edge(other, f64::NAN, eval);
            }
        }
    }
}

/// 一様な格子（`n × n` マス、範囲 `u` × `v`）から始めて、`decide` が「分ける」と言った三角形を細かくする。
///
/// - `base`: 格子の点 (n + 1)² 個の値（`j * (n + 1) + i` の順）
/// - `eval`: 点 (u, v) の列での値をまとめて計算する
/// - `decide`: 三角形を分けるなら、そのずれの大きさ（上限に近いとき、大きいものから分ける）。分けないなら `None`
pub fn refine<V: Clone>(
    n: usize,
    u: (f64, f64),
    v: (f64, f64),
    base: Vec<V>,
    eval: &mut Eval<'_, V>,
    decide: &mut dyn FnMut(&Candidate<V>) -> Option<f64>,
    limits: Limits,
) -> Refined<V> {
    let m = n + 1;
    assert_eq!(base.len(), m * m);
    let at = |a: (f64, f64), k: usize| a.0 + (a.1 - a.0) * k as f64 / n as f64;
    let mut uv = Vec::with_capacity(m * m);
    for j in 0..m {
        for i in 0..m {
            uv.push([at(u, i), at(v, j)]);
        }
    }
    let mut st = State {
        uv,
        vals: base,
        tris: Vec::with_capacity(2 * n * n),
        n,
        base_tris: (2 * n * n) as u32,
        adj: HashMap::default(),
        mids: HashMap::default(),
        placed: HashMap::default(),
        born: Vec::new(),
    };
    // 最初の三角形（辺の表には入れない。State::adjacent を参照）
    let nan = [f64::NAN; 2];
    for j in 0..n {
        for i in 0..n {
            let k = (j * m + i) as u32;
            let (a, b, c, d) = (k, k + 1, k + m as u32, k + m as u32 + 1);
            st.tris.push(Tri {
                v: [d, a, b],
                history: nan,
                alive: true,
            }); // (a, b, d) の向き
            st.tris.push(Tri {
                v: [a, d, c],
                history: nan,
                alive: true,
            });
        }
    }
    let mut cands: Vec<u32> = (0..st.tris.len() as u32).collect();
    let mut added = 0usize;
    // 最初の格子の対角線の中点（マスの番号の順）。最初のパスだけは、表を使わずに配列で持つ（なめらかな曲面では
    // 分ける三角形が無いので、ここが時間のほとんどになる）
    let mut diag: Vec<Option<([f64; 2], V)>> = Vec::new();
    if limits.passes > 0 && n > 0 {
        let (mut us, mut vs) = (Vec::with_capacity(n * n), Vec::with_capacity(n * n));
        for j in 0..n {
            for i in 0..n {
                let (a, d) = (st.uv[j * m + i], st.uv[(j + 1) * m + i + 1]);
                us.push(0.5 * (a[0] + d[0]));
                vs.push(0.5 * (a[1] + d[1]));
            }
        }
        let vals = eval(&us, &vs);
        diag = vals
            .into_iter()
            .enumerate()
            .map(|(k, v)| Some(([us[k], vs[k]], v)))
            .collect();
    }
    for pass in 0..limits.passes {
        if cands.is_empty() || added >= limits.max_new_triangles {
            break;
        }
        let mut marked: Vec<(f64, u32)> = Vec::new();
        if pass == 0 {
            for (t, tri) in st.tris.iter().enumerate() {
                let Some((_, mid)) = diag[t / 2].as_ref() else {
                    continue;
                };
                let c = Candidate {
                    ids: tri.v,
                    vals: [
                        &st.vals[tri.v[0] as usize],
                        &st.vals[tri.v[1] as usize],
                        &st.vals[tri.v[2] as usize],
                    ],
                    mid,
                    history: tri.history,
                };
                if let Some(err) = decide(&c) {
                    marked.push((err, t as u32));
                }
            }
            // 分ける三角形の対角線の中点だけを、表に移す
            for &(_, t) in &marked {
                let e = st.tris[t as usize].edge();
                if let Some(x) = diag[t as usize / 2].take() {
                    st.mids.insert(e, x);
                }
            }
            diag = Vec::new();
        } else {
            // 調べる三角形の斜辺の中点をまとめて計算する
            let edges: Vec<Edge> = cands.iter().map(|&t| st.tris[t as usize].edge()).collect();
            evaluate_mids(&mut st, edges, eval);
            for &t in &cands {
                let tri = &st.tris[t as usize];
                if !tri.alive {
                    continue;
                }
                let e = tri.edge();
                let Some((_, mid)) = st.mids.get(&e) else {
                    continue;
                };
                let c = Candidate {
                    ids: tri.v,
                    vals: [
                        &st.vals[tri.v[0] as usize],
                        &st.vals[tri.v[1] as usize],
                        &st.vals[tri.v[2] as usize],
                    ],
                    mid,
                    history: tri.history,
                };
                if let Some(err) = decide(&c) {
                    marked.push((err, t));
                }
            }
        }
        if marked.is_empty() {
            break;
        }
        // 上限に近いときは、ずれの大きい三角形から（1 つ分けると、継ぎ目を合わせる分も含めて 2 つほど増える）
        let room = (limits.max_new_triangles - added) / 2;
        if marked.len() > room {
            marked.sort_by(|a, b| b.0.total_cmp(&a.0));
            marked.truncate(room.max(1));
        }
        // 分ける辺: 分けたい三角形の斜辺と、継ぎ目を合わせるために先に分ける向こうの三角形の斜辺（たどれるだけ）
        let mut split: HashSet<Edge> = HashSet::default();
        let mut order: Vec<(Edge, f64)> = Vec::new();
        let mut stack: Vec<Edge> = Vec::new();
        let mut err_of: HashMap<Edge, f64> = HashMap::default();
        for &(err, t) in &marked {
            let e = st.tris[t as usize].edge();
            let x = err_of.entry(e).or_insert(err);
            *x = x.max(err);
            stack.push(e);
        }
        while let Some(e) = stack.pop() {
            if !split.insert(e) {
                continue;
            }
            order.push((e, err_of.get(&e).copied().unwrap_or(f64::NAN)));
            for t in st.adjacent(e).into_iter().filter(|&t| t != NO) {
                let r = st.tris[t as usize].edge();
                if r != e {
                    stack.push(r);
                }
            }
        }
        evaluate_mids(&mut st, order.iter().map(|&(e, _)| e).collect(), eval);
        st.born.clear();
        let before = st.tris.iter().filter(|t| t.alive).count();
        for (e, err) in order {
            st.split_edge(e, err, eval);
        }
        added += st.tris.iter().filter(|t| t.alive).count() - before;
        cands = std::mem::take(&mut st.born)
            .into_iter()
            .filter(|&t| st.tris[t as usize].alive)
            .collect();
    }
    Refined {
        uv: st.uv,
        vals: st.vals,
        tris: st
            .tris
            .iter()
            .filter(|t| t.alive)
            .map(|t| [t.v[2], t.v[0], t.v[1]])
            .collect(),
    }
}

/// 辺の中点のうち、まだ計算していないものをまとめて計算する
fn evaluate_mids<V: Clone>(st: &mut State<V>, edges: Vec<Edge>, eval: &mut Eval<'_, V>) {
    let mut seen = HashSet::default();
    let todo: Vec<Edge> = edges
        .into_iter()
        .filter(|e| !st.mids.contains_key(e) && !st.placed.contains_key(e) && seen.insert(*e))
        .collect();
    if todo.is_empty() {
        return;
    }
    let (mut us, mut vs) = (
        Vec::with_capacity(todo.len()),
        Vec::with_capacity(todo.len()),
    );
    for &(a, b) in &todo {
        let (p, q) = (st.uv[a as usize], st.uv[b as usize]);
        us.push(0.5 * (p[0] + q[0]));
        vs.push(0.5 * (p[1] + q[1]));
    }
    let vals = eval(&us, &vs);
    for (k, (e, val)) in todo.into_iter().zip(vals).enumerate() {
        st.mids.insert(e, ([us[k], vs[k]], val));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// どの辺も 2 つ以下の三角形に使われ、頂点が別の三角形の辺の途中に乗っていない（T 字の継ぎ目が無い）
    pub fn assert_conforming(uv: &[[f64; 2]], tris: &[[u32; 3]]) {
        let mut count: HashMap<Edge, usize> = HashMap::default();
        for t in tris {
            for e in [key(t[0], t[1]), key(t[1], t[2]), key(t[2], t[0])] {
                *count.entry(e).or_default() += 1;
            }
        }
        assert!(
            count.values().all(|&c| c <= 2),
            "3 つ以上の三角形に使われる辺がある"
        );
        // 辺の途中の頂点: 辺の中点に頂点があれば、継ぎ目が合っていない（二分でできる頂点は必ず辺の中点）
        let mut at: HashMap<(i64, i64), u32> = HashMap::default();
        let q = |p: [f64; 2]| ((p[0] * 1e9).round() as i64, (p[1] * 1e9).round() as i64);
        for t in tris {
            for &k in t {
                at.insert(q(uv[k as usize]), k);
            }
        }
        for &(a, b) in count.keys() {
            let (p, r) = (uv[a as usize], uv[b as usize]);
            let m = [0.5 * (p[0] + r[0]), 0.5 * (p[1] + r[1])];
            assert!(!at.contains_key(&q(m)), "辺 {p:?}–{r:?} の途中に頂点がある");
        }
    }

    #[test]
    fn refine_near_point_is_conforming_and_keeps_orientation() {
        // 点 (0.3, 0.2) の近くの三角形を分け続ける
        let n = 8;
        let f = |u: f64, v: f64| ((u - 0.3).powi(2) + (v - 0.2).powi(2)).sqrt();
        let base: Vec<f64> = (0..(n + 1) * (n + 1))
            .map(|k| {
                f(
                    (k % (n + 1)) as f64 / n as f64,
                    (k / (n + 1)) as f64 / n as f64,
                )
            })
            .collect();
        let mut eval = |us: &[f64], vs: &[f64]| {
            us.iter()
                .zip(vs)
                .map(|(&u, &v)| f(u, v))
                .collect::<Vec<_>>()
        };
        let mut decide = |c: &Candidate<f64>| (c.vals.iter().any(|&&d| d < 0.2)).then_some(1.0);
        let r = refine(
            n,
            (0.0, 1.0),
            (0.0, 1.0),
            base,
            &mut eval,
            &mut decide,
            Limits {
                passes: 8,
                max_new_triangles: 100_000,
            },
        );
        assert!(r.tris.len() > 2 * n * n);
        assert_conforming(&r.uv, &r.tris);
        // 向きはすべて同じ（面積が正）で、合計は 1
        let mut area = 0.0;
        for t in &r.tris {
            let (a, b, c) = (
                r.uv[t[0] as usize],
                r.uv[t[1] as usize],
                r.uv[t[2] as usize],
            );
            let s = ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])) / 2.0;
            assert!(s > 0.0);
            area += s;
        }
        assert!((area - 1.0).abs() < 1e-12);
    }
}

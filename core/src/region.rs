//! 累次積分の形に書けない領域（x² + y² ≤ 1・x² ≤ y ≤ x + 2 など）の上の積分の点と重み。
//!
//! 条件 G_k(p) ≥ 0 を点ごとに計算できる（積分の変数とパラメータだけの式）とき、
//! 1. 領域を囲む箱を、格子で探す（端の書かれていない変数は、原点を中心に幅を 2 倍ずつ広げる）
//! 2. 外側の変数から順に、領域の切り口（その変数の区間の集まり）を標本で見つけ、端を二分法で求める
//! 3. 区間ごとにガウス・ルジャンドルの点を置く。内側に変数が残っている段では、t = 中点 + 半幅 · sin(πs/2) と置き換える
//!    （切り口の長さは端で √ の形に 0 になることが多く（円など）、そのままでは精度が落ちる。置き換えるとなめらかになる）
//!
//! 結果は「点と重み」の列で、積分は Σ 重み · f(点)。

use crate::eval::{as_real, Program};
use crate::expand::gauss_legendre;
use crate::quad;

type C = num_complex::Complex64;

/// 変数の数ごとの、段ごと（外側から）の 1 区間の点の数
const COUNTS: [&[usize]; 3] = [&[32], &[32, 16], &[20, 12, 8]];
/// 変数の数ごとの、切り口を探す標本の数（1 本の線の上）
const SAMPLES: [usize; 3] = [512, 128, 48];
/// 変数の数ごとの、箱を探す格子の分割数
const GRID: [usize; 3] = [2048, 128, 40];
/// 1 本の線の上の区間の数の上限
const MAX_RUNS: usize = 6;
/// 標本の間隔より細い所を探すときの、拡大の回数（1 回ごとに幅を 1/8）
const ZOOMS: u32 = 4;
/// 1 つの区間を、内側の切り口の区間の数が変わる所で分ける数の上限
const MAX_PANELS: usize = 6;
/// 切り口の端の二分法の回数
const BISECTIONS: usize = 32;

/// 切り口の 1 つの区間 [a, b] と、両端の近くで見つけた領域の点
struct Section {
    a: f64,
    b: f64,
    wa: [f64; 3],
    wb: [f64; 3],
}

struct Region {
    progs: Vec<Program>,
    n: usize,
    lo: [f64; 3],
    hi: [f64; 3],
    /// 座標ごとの変数変換（箱の座標 s → 積分の変数 x）。点と重みを返す道（[`quadrature`]）ではすべて恒等
    maps: [Map; 3],
    /// 一番内側の座標の、切り口の端の二分法の回数（ほかの座標は [`BISECTIONS`]）
    inner_bisections: usize,
    /// 標本の間隔より細い所を探すときの拡大の回数（[`ZOOMS`]。適応型の積分では、尖った先の近くの細い切り口も拾えるよう増やす）
    zooms: u32,
}

/// 箱の座標 s から積分の変数 x への変換。端の書かれていない向きを、有限の箱に写す（[`integrate`] だけで使う）
#[derive(Clone, Copy, PartialEq)]
enum Map {
    Id,
    /// x = a + s/(1 − s)（s ∈ [0, 1)）: [a, ∞)
    Lower(f64),
    /// x = b − s/(1 − s): (−∞, b]
    Upper(f64),
    /// x = s/(1 − s²)（s ∈ (−1, 1)）: (−∞, ∞)
    Both,
}

impl Map {
    fn x(self, s: f64) -> f64 {
        match self {
            Map::Id => s,
            Map::Lower(a) => a + s / (1.0 - s),
            Map::Upper(b) => b - s / (1.0 - s),
            Map::Both => s / (1.0 - s * s),
        }
    }

    /// dx/ds
    fn jac(self, s: f64) -> f64 {
        match self {
            Map::Id => 1.0,
            Map::Lower(_) | Map::Upper(_) => 1.0 / ((1.0 - s) * (1.0 - s)),
            Map::Both => (1.0 + s * s) / ((1.0 - s * s) * (1.0 - s * s)),
        }
    }
}

impl Region {
    /// d 番目の座標を ts に変えた点（ほかは p のまま）が、領域に入るか
    fn inside_line(&self, p: &[f64; 3], d: usize, ts: &[f64]) -> Vec<bool> {
        let cols: Vec<Vec<f64>> = (0..self.n).map(|k| if k == d { ts.to_vec() } else { vec![p[k]; ts.len()] }).collect();
        self.inside(&cols, ts.len())
    }

    fn inside(&self, cols: &[Vec<f64>], len: usize) -> Vec<bool> {
        let mapped: Vec<Vec<f64>>;
        let cols = if self.maps[..self.n].iter().all(|m| *m == Map::Id) {
            cols
        } else {
            mapped = cols.iter().zip(&self.maps).map(|(c, m)| c.iter().map(|&s| m.x(s)).collect()).collect();
            &mapped
        };
        let inputs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
        let mut ok = vec![true; len];
        let mut out = vec![C::new(0.0, 0.0); len];
        for prog in &self.progs {
            prog.eval_batch(&inputs, &mut out);
            for (o, v) in ok.iter_mut().zip(&out) {
                // NaN（計算できない所）は領域の外
                *o = *o && as_real(*v) >= 0.0;
            }
        }
        ok
    }

    /// 点 p（座標 d を t にしたもの）で、条件の値のいちばん小さいもの（領域に入れば 0 以上。計算できなければ NaN）
    fn margin(&self, p: &[f64; 3], d: usize, t: f64) -> f64 {
        let cols: Vec<Vec<f64>> = (0..self.n).map(|k| vec![self.maps[k].x(if k == d { t } else { p[k] })]).collect();
        let inputs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
        let mut out = [C::new(0.0, 0.0)];
        let mut m = f64::INFINITY;
        for prog in &self.progs {
            prog.eval_batch(&inputs, &mut out);
            let v = as_real(out[0]);
            if v.is_nan() {
                return f64::NAN;
            }
            m = m.min(v);
        }
        m
    }

    /// 一番内側の座標 d の、入る点 a と入らない点 b の間の境目（Illinois 法。入らない側の値が NaN なら二分法の 1 歩）。
    /// 戻り値は境目のすぐ内側
    fn boundary(&self, p: &[f64; 3], d: usize, mut a: f64, mut b: f64) -> f64 {
        let (mut ga, mut gb) = (self.margin(p, d, a), self.margin(p, d, b));
        let mut last = 0i8;
        for _ in 0..self.inner_bisections {
            let mid = 0.5 * (a + b);
            let mut c = if ga.is_finite() && gb.is_finite() && ga > gb { a + (b - a) * ga / (ga - gb) } else { mid };
            if !(c > a.min(b) && c < a.max(b)) {
                c = mid;
            }
            if c == a || c == b {
                break;
            }
            let gc = self.margin(p, d, c);
            if gc >= 0.0 {
                (a, ga) = (c, gc);
                if last == 1 {
                    gb *= 0.5; // 同じ側が続いたら、反対の端の値を半分にする（Illinois 法）
                }
                last = 1;
            } else {
                (b, gb) = (c, gc);
                if last == -1 {
                    ga *= 0.5;
                }
                last = -1;
            }
            if (b - a).abs() <= 4.0 * f64::EPSILON * a.abs().max(b.abs()).max(1e-300) {
                break;
            }
        }
        a
    }

    /// 座標 d の標本。zoom = 0 なら箱の端から端まで。zoom ≥ 1 なら、center を中心に幅を 1/8^zoom にした範囲（箱からはみ出さない）
    fn samples(&self, d: usize, center: f64, zoom: u32) -> Vec<f64> {
        let m = SAMPLES[self.n - 1];
        let (mut a, mut b) = (self.lo[d], self.hi[d]);
        if zoom > 0 {
            let half = 0.5 * (b - a) / 8f64.powi(zoom as i32);
            (a, b) = ((center - half).max(a), (center + half).min(b));
        }
        (0..=m).map(|j| a + (b - a) * j as f64 / m as f64).collect()
    }

    /// 座標 0..d を p で決めたとき、残りの座標のどこかで領域に入る点（無ければ None）。
    /// 標本の間隔より細い所（領域の先の尖った所の近く）は、hint（近くで見つけた領域の点）の周りを細かく探す
    fn find(&self, p: &mut [f64; 3], d: usize, hint: Option<&[f64; 3]>) -> Option<[f64; 3]> {
        if d == self.n {
            return self.inside_line(p, d - 1, &[p[d - 1]])[0].then_some(*p);
        }
        for zoom in 0..=self.zooms {
            if zoom > 0 && hint.is_none() {
                break;
            }
            let ts = self.samples(d, hint.map_or(0.0, |h| h[d]), zoom);
            if d == self.n - 1 {
                let flags = self.inside_line(p, d, &ts);
                if let Some(first) = flags.iter().position(|&b| b) {
                    // 続いて入っている標本の真ん中
                    let len = flags[first..].iter().take_while(|&&b| b).count();
                    p[d] = ts[first + (len - 1) / 2];
                    return Some(*p);
                }
            } else {
                for &t in &ts {
                    p[d] = t;
                    if let Some(w) = self.find(p, d + 1, hint) {
                        return Some(w);
                    }
                }
            }
        }
        None
    }

    /// 座標 0..d を p で決めたときの、座標 d の切り口（区間の集まり）。区間ごとに、両端の近くで見つけた領域の点も返す
    fn sections(&self, p: &mut [f64; 3], d: usize, hint: Option<&[f64; 3]>) -> Vec<Section> {
        let innermost = d == self.n - 1;
        for zoom in 0..=self.zooms {
            if zoom > 0 && hint.is_none() {
                break;
            }
            let ts = self.samples(d, hint.map_or(0.0, |h| h[d]), zoom);
            // 標本ごとの、領域に入る点（入らなければ None）
            let mut hits: Vec<Option<[f64; 3]>> = if innermost {
                let flags = self.inside_line(p, d, &ts);
                ts.iter().zip(flags).map(|(&t, ok)| ok.then(|| { let mut q = *p; q[d] = t; q })).collect()
            } else {
                ts.iter().map(|&t| { p[d] = t; self.find(p, d + 1, hint) }).collect()
            };
            if hits.iter().all(|h| h.is_none()) {
                continue;
            }
            // 内側に座標が残っている段では、入らなかった標本が本当に外とは限らない（内側の切り口が内側の標本の間隔より細い所。
            // 円と直線の間のくさびの先など）。入っている標本の隣で入らなかった標本は、隣で見つけた点の周りを細かく探し直す。
            // 探し直さないと、端の二分法がその標本の手前で止まり、くさびの先が抜け落ちる
            if !innermost {
                let mut tried = vec![false; hits.len()];
                loop {
                    let mut changed = false;
                    for j in 0..hits.len() {
                        if hits[j].is_some() || tried[j] {
                            continue;
                        }
                        let near = if j > 0 && hits[j - 1].is_some() { hits[j - 1] } else if j + 1 < hits.len() { hits[j + 1] } else { None };
                        let Some(w) = near else { continue };
                        tried[j] = true;
                        p[d] = ts[j];
                        if let Some(h) = self.find(p, d + 1, Some(&w)) {
                            hits[j] = Some(h);
                            changed = true;
                        }
                    }
                    if !changed {
                        break;
                    }
                }
            }
            // 入っている点 a（領域の点 w）と入っていない点 b の間の境目
            let mut edge = |mut a: f64, mut b: f64, mut w: [f64; 3]| {
                if innermost && self.inner_bisections > BISECTIONS {
                    // 一番内側: 条件の値（いちばん小さい G_k）で、はさみうち法（Illinois 法）。二分法の 52 回より少ない回数で詰まる
                    a = self.boundary(p, d, a, b);
                    p[d] = a;
                    w = *p;
                    return (a, w);
                }
                for _ in 0..BISECTIONS {
                    let c = 0.5 * (a + b);
                    if c == a || c == b {
                        break;
                    }
                    p[d] = c;
                    match self.find(p, d + 1, Some(&w)) {
                        Some(nw) => (a, w) = (c, nw),
                        None => b = c,
                    }
                }
                (a, w)
            };
            let mut out = Vec::new();
            let mut j = 0;
            while j < hits.len() && out.len() < MAX_RUNS {
                let Some(first) = hits[j] else {
                    j += 1;
                    continue;
                };
                let start = j;
                while j + 1 < hits.len() && hits[j + 1].is_some() {
                    j += 1;
                }
                let last = hits[j].unwrap();
                // 標本の端まで入っているとき: 箱の端から端までの標本なら、そこが端。拡大した標本なら、区間は標本の外へ続いているので、
                // 箱の端との間で境目を探す（拡大するのは、端から端までの標本で見つからない細い区間だけなので、箱の端は外にある）
                let (a, wa) = if start > 0 {
                    edge(ts[start], ts[start - 1], first)
                } else if zoom > 0 && ts[0] > self.lo[d] {
                    edge(ts[0], self.lo[d], first)
                } else {
                    (ts[0], first)
                };
                let (b, wb) = if j + 1 < hits.len() {
                    edge(ts[j], ts[j + 1], last)
                } else if zoom > 0 && ts[j] < self.hi[d] {
                    edge(ts[j], self.hi[d], last)
                } else {
                    (ts[j], last)
                };
                out.push(Section { a, b, wa, wb });
                j += 1;
            }
            return out;
        }
        Vec::new()
    }

    fn nodes(&self, p: &mut [f64; 3], d: usize, w: f64, hint: Option<&[f64; 3]>, out: &mut Vec<([f64; 3], f64)>) {
        let innermost = d == self.n - 1;
        let (xs, ws) = gauss_legendre(COUNTS[self.n - 1][d]);
        let secs = self.sections(p, d, hint);
        for sec in secs {
            if sec.b <= sec.a {
                continue;
            }
            if innermost {
                let (c, h) = (0.5 * (sec.a + sec.b), 0.5 * (sec.b - sec.a));
                for (x, wx) in xs.iter().zip(&ws) {
                    p[d] = c + h * x;
                    out.push((*p, w * h * wx));
                }
                continue;
            }
            // 内側の切り口は、近いほうの端で見つけた点の周りも探す（端の近くでは、切り口が標本の間隔より細い）
            let mid = 0.5 * (sec.a + sec.b);
            let near = |t: f64| if t < mid { sec.wa } else { sec.wb };
            // 内側の切り口の形が変わる所で分ける。そこでは切り口の長さ（面積）が折れるので、またいで点を置くと精度が落ちる:
            // - 区間の数が変わる所（輪の穴の端など。長さは √ の形に折れる）。もう 1 段内側の区間の数（球殻の穴）も見る
            // - 区間の端が折れる所（ひし形の角など）
            let m = SAMPLES[self.n - 1];
            let mut shape = |t: f64| {
                p[d] = t;
                let secs = self.sections(p, d + 1, Some(&near(t)));
                let mut deep = 0;
                if d + 2 < self.n {
                    for &u in &self.samples(d + 1, 0.0, 0) {
                        p[d + 1] = u;
                        let flags = self.inside_line(p, d + 2, &self.samples(d + 2, 0.0, 0));
                        deep = deep.max((0..flags.len()).filter(|&j| flags[j] && (j == 0 || !flags[j - 1])).count());
                    }
                }
                (secs, deep)
            };
            let ts: Vec<f64> = (0..m).map(|j| sec.a + (sec.b - sec.a) * (j as f64 + 0.5) / m as f64).collect();
            let shapes: Vec<(Vec<Section>, usize)> = ts.iter().map(|&t| shape(t)).collect();
            let same = |x: &(Vec<Section>, usize), y: &(Vec<Section>, usize)| x.0.len() == y.0.len() && x.1 == y.1;
            let mut cuts = Vec::new();
            let mut start = 0;
            for j in 1..=m {
                if j < m && same(&shapes[j], &shapes[j - 1]) {
                    continue;
                }
                // 形が同じ標本の列 start..j の中で、区間の端が折れる所を探す
                for e in 0..2 * shapes[start].0.len() {
                    let val = |i: usize| if e % 2 == 0 { shapes[i].0[e / 2].a } else { shapes[i].0[e / 2].b };
                    let bend = |i: usize| (val(i + 1) - 2.0 * val(i) + val(i - 1)).abs();
                    let floor = 1e-5 * (self.hi[d + 1] - self.lo[d + 1]);
                    let mut i = start + 2;
                    while i + 2 < j {
                        // 折れ目は、その両隣（2 つ先）に比べて 2 階差分が飛び抜けて大きい。なめらかな曲がり（円の端の近くなど）は、隣も大きい
                        let here = bend(i).max(bend(i + 1));
                        if here > floor && here > 6.0 * bend(i - 1).max(if i + 3 < j { bend(i + 2) } else { 0.0 }) {
                            // 折れ目の位置: 両側の直線の交点
                            let (s0, s1) = ((val(i - 1) - val(i - 2)) / (ts[i - 1] - ts[i - 2]), (val(i + 2) - val(i + 1)) / (ts[i + 2] - ts[i + 1]));
                            let t = if s0 != s1 { (val(i + 1) - val(i - 1) + s0 * ts[i - 1] - s1 * ts[i + 1]) / (s0 - s1) } else { ts[i] };
                            cuts.push(if t > ts[i - 1] && t < ts[i + 2] { t } else { 0.5 * (ts[i] + ts[i + 1]) });
                            i += 3;
                        } else {
                            i += 1;
                        }
                    }
                }
                if j < m {
                    // 形が変わる所
                    let (mut lo, mut hi) = (ts[j - 1], ts[j]);
                    for _ in 0..BISECTIONS {
                        let c = 0.5 * (lo + hi);
                        if same(&shape(c), &shapes[j - 1]) {
                            lo = c
                        } else {
                            hi = c
                        }
                    }
                    cuts.push(0.5 * (lo + hi));
                }
                start = j;
            }
            cuts.sort_by(|x, y| x.partial_cmp(y).unwrap());
            cuts.dedup_by(|x, y| (*x - *y).abs() < 1e-9 * (sec.b - sec.a));
            cuts.truncate(MAX_PANELS);
            cuts.insert(0, sec.a);
            cuts.push(sec.b);
            for k in 1..cuts.len() {
                let (c, h) = (0.5 * (cuts[k - 1] + cuts[k]), 0.5 * (cuts[k] - cuts[k - 1]));
                for (x, wx) in xs.iter().zip(&ws) {
                    let s = std::f64::consts::FRAC_PI_2 * x;
                    p[d] = c + h * s.sin();
                    let hint = near(p[d]);
                    self.nodes(p, d + 1, w * h * std::f64::consts::FRAC_PI_2 * s.cos() * wx, Some(&hint), out);
                }
            }
        }
    }

    /// 箱 [lo, hi] を各軸 g 等分した格子で、領域に入る点の番号の範囲（軸ごとの最小・最大）。入る点が無ければ None
    fn grid_extent(&self, g: usize) -> Option<[(usize, usize); 3]> {
        let total = (g + 1).pow(self.n as u32);
        let mut cols = vec![Vec::with_capacity(total); self.n];
        for idx in 0..total {
            let mut r = idx;
            for (k, col) in cols.iter_mut().enumerate() {
                col.push(self.lo[k] + (self.hi[k] - self.lo[k]) * (r % (g + 1)) as f64 / g as f64);
                r /= g + 1;
            }
        }
        let ok = self.inside(&cols, total);
        let mut ext: Option<[(usize, usize); 3]> = None;
        for (idx, _) in ok.iter().enumerate().filter(|(_, b)| **b) {
            let e = ext.get_or_insert([(usize::MAX, 0); 3]);
            let mut r = idx;
            for item in e.iter_mut().take(self.n) {
                let j = r % (g + 1);
                *item = (item.0.min(j), item.1.max(j));
                r /= g + 1;
            }
        }
        ext
    }
}

/// 領域 {p | すべての progs(p) ≥ 0} の上の積分の点と重み。progs の変数は積分の変数（n 個。3 個まで）。
/// known は、条件に数で書かれている変数ごとの端（下・上。無ければ None）。
/// 領域が見つからない・有界でないときは Err
pub fn quadrature(progs: Vec<Program>, n: usize, known: &[(Option<f64>, Option<f64>)]) -> Result<Vec<([f64; 3], f64)>, String> {
    assert!((1..=3).contains(&n) && known.len() == n);
    let g = GRID[n - 1];
    let mut region = Region { progs, n, lo: [0.0; 3], hi: [0.0; 3], maps: [Map::Id; 3], inner_bisections: BISECTIONS, zooms: ZOOMS };
    // 1. 箱を探す: 端の書かれていない向きは、幅を 2 倍ずつ広げる。領域が箱の端（広げた向き）に触れなくなったら終わり
    let mut ext = None;
    let mut seen = false;
    for k in -3..=20 {
        let s = 2f64.powi(k);
        for (i, (lo, hi)) in known.iter().enumerate() {
            (region.lo[i], region.hi[i]) = match (lo, hi) {
                (Some(a), Some(b)) => (*a, *b),
                (Some(a), None) => (*a, a.max(0.0) + s),
                (None, Some(b)) => (b.min(0.0) - s, *b),
                (None, None) => (-s, s),
            };
            if region.lo[i] >= region.hi[i] {
                return Ok(Vec::new()); // 下の端 ≥ 上の端: 空の領域
            }
        }
        let open = known.iter().any(|(lo, hi)| lo.is_none() || hi.is_none());
        let found = region.grid_extent(g);
        if let Some(e) = found {
            seen = true;
            let touches = (0..n).any(|i| (known[i].0.is_none() && e[i].0 == 0) || (known[i].1.is_none() && e[i].1 == g));
            if !touches {
                ext = Some(e);
                break;
            }
        }
        if !open {
            // 全部の端が書かれている: 箱はこれで決まり。入る点が無ければ空の領域（積分は 0）
            match found {
                Some(e) => ext = Some(e),
                None => return Ok(Vec::new()),
            }
            break;
        }
    }
    let Some(ext) = ext else {
        return Err(if seen {
            "積分する領域が、どこまでも続いているようです（変数の範囲を 0 ≤ x ≤ 1 のように書いてください）".into()
        } else {
            "積分する領域が見つかりません（空か、とても小さいか、原点からとても遠くにあります。変数の範囲を 0 ≤ x ≤ 1 のように書いてください）".into()
        });
    };
    region.shrink(ext, known);
    // 2・3. 切り口ごとに点を置く
    let mut out = Vec::new();
    region.nodes(&mut [0.0; 3], 0, 1.0, None, &mut out);
    Ok(out)
}

impl Region {
    /// 箱を領域に合わせて縮める（入る点の範囲の 3 マス外まで。書かれている端は越えない）。箱があまり縮まなくなったら終わり。
    /// 格子の点が領域の先の細い所（格子の間隔より細い）を外すので、余裕を持たせる。細い所は、後で線の上の標本と二分法で探す
    fn shrink(&mut self, mut ext: [(usize, usize); 3], known: &[(Option<f64>, Option<f64>)]) {
        let (n, g) = (self.n, GRID[self.n - 1]);
        let region = self;
    for _ in 0..60 {
        let mut shrunk = false;
        let (lo, hi) = (region.lo, region.hi);
        for i in 0..n {
            let step = (hi[i] - lo[i]) / g as f64;
            let a = (lo[i] + step * (ext[i].0 as f64 - 3.0)).max(known[i].0.unwrap_or(f64::NEG_INFINITY));
            let b = (lo[i] + step * (ext[i].1 as f64 + 3.0)).min(known[i].1.unwrap_or(f64::INFINITY));
            shrunk |= b - a < 0.5 * (hi[i] - lo[i]);
            (region.lo[i], region.hi[i]) = (a, b);
        }
        if !shrunk {
            break;
        }
        match region.grid_extent(g) {
            Some(e) => ext = e,
            None => break, // 格子より細い領域: この箱のまま、線の上の標本に任せる
        }
    }
    }

    /// 座標 0..d を p で決めたときの、残りの座標の上の f の積分（適応型。`quad.rs`）。
    /// 切り口の区間ごとに、その区間の上の「内側の積分」を積分する。切り口の形が変わる所（穴の端・角）では内側の積分が
    /// なめらかでなくなるが、その近くの区間が細かくなるので、そのまま収束する
    fn integrate_level(&self, p: &mut [f64; 3], d: usize, hint: Option<&[f64; 3]>, f: &Integrand, budget: &quad::Budget) -> f64 {
        let n = self.n;
        let innermost = d == n - 1;
        // 外側ほど目標を厳しくする（内側の積分の誤差は、外側の値の誤差として足される）。区間の数の上限は 3 重で控えめに
        let tol = quad::Tol { rel: [[1e-10, 0.0, 0.0], [1e-9, 1e-11, 0.0], [1e-8, 1e-9, 1e-10]][n - 1][d], panels: [[400, 0, 0], [300, 150, 0], [40, 30, 30]][n - 1][d], ends: true };
        let mut total = 0.0;
        let secs = self.sections(p, d, hint);
        for sec in secs {
            if sec.b <= sec.a {
                continue;
            }
            let mid = 0.5 * (sec.a + sec.b);
            let near = |t: f64| if t < mid { sec.wa } else { sec.wb };
            let base = *p;
            let v = if innermost {
                // 外側の座標の変換の重み
                let outer: f64 = (0..d).map(|k| self.maps[k].jac(base[k])).product();
                let mut g = |ts: &[f64], out: &mut [f64]| {
                    let len = ts.len();
                    let cols: Vec<Vec<f64>> = (0..n)
                        .map(|k| if k == d { ts.iter().map(|&s| self.maps[k].x(s)).collect() } else { vec![self.maps[k].x(base[k]); len] })
                        .collect();
                    let inputs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
                    let mut vals = vec![0.0; len];
                    f(&inputs, &mut vals);
                    for ((o, v), &s) in out.iter_mut().zip(&vals).zip(ts) {
                        let fv = *v;
                        // 無限遠で 0 になる中身は、変換の重みが大きくても 0
                        *o = if fv == 0.0 { 0.0 } else { fv * outer * self.maps[d].jac(s) };
                    }
                };
                quad::integrate(&mut g, sec.a, sec.b, tol, budget)
            } else {
                let mut g = |ts: &[f64], out: &mut [f64]| {
                    for (o, &t) in out.iter_mut().zip(ts) {
                        let mut q = base;
                        q[d] = t;
                        let h = near(t);
                        *o = self.integrate_level(&mut q, d + 1, Some(&h), f, budget);
                    }
                };
                quad::integrate(&mut g, sec.a, sec.b, tol, budget)
            };
            total += v;
        }
        total
    }
}

/// 積分の中身: 積分の変数の値の列（変数ごと）から、点ごとの値を出す
pub type Integrand<'a> = dyn Fn(&[&[f64]], &mut [f64]) + 'a;

/// 式（積分の変数の順に変数を持つ）を [`Integrand`] にする。値が実数でなければ NaN
pub fn program_integrand(f: &Program) -> impl Fn(&[&[f64]], &mut [f64]) + '_ {
    move |inputs: &[&[f64]], out: &mut [f64]| {
        let mut vals = vec![C::new(0.0, 0.0); out.len()];
        f.eval_batch(inputs, &mut vals);
        for (o, v) in out.iter_mut().zip(&vals) {
            *o = as_real(*v);
        }
    }
}

/// 値が定数になる、領域 {p | すべての progs(p) ≥ 0} の上の f の積分（適応型。点と重みを返す [`quadrature`] より、ずっと正確）。
/// 端の書かれていない向きは、有限の箱に写して探す（どこまでも続く領域 x ≥ 0, y ≥ 0 の上の e^(−x−y) も積分できる）。
/// 計算の回数が上限を超えた・格子の点が 1 つも入らないときは None（呼び出し側が [`quadrature`] に戻す）。
/// 誤差が目標に届かなかったかは `budget.rough`
pub fn integrate(progs: Vec<Program>, n: usize, known: &[(Option<f64>, Option<f64>)], f: &Integrand, budget: &quad::Budget) -> Option<f64> {
    assert!((1..=3).contains(&n) && known.len() == n);
    let mut region = Region { progs, n, lo: [0.0; 3], hi: [0.0; 3], maps: [Map::Id; 3], inner_bisections: 52, zooms: 7 };
    let mut boxed = Vec::with_capacity(n);
    for (i, &(lo, hi)) in known.iter().enumerate() {
        let (m, a, b) = match (lo, hi) {
            (Some(a), Some(b)) => (Map::Id, a, b),
            (Some(a), None) => (Map::Lower(a), 0.0, 1.0),
            (None, Some(b)) => (Map::Upper(b), 0.0, 1.0),
            (None, None) => (Map::Both, -1.0, 1.0),
        };
        if a >= b {
            return Some(0.0); // 下の端 ≥ 上の端: 空の領域
        }
        (region.maps[i], region.lo[i], region.hi[i]) = (m, a, b);
        boxed.push((Some(a), Some(b)));
    }
    let ext = region.grid_extent(GRID[n - 1])?;
    region.shrink(ext, &boxed);
    // 領域が無限遠まで届いているか（写した箱の、無限遠の側の端まで縮まなかった）
    let unbounded = (0..n).any(|i| match region.maps[i] {
        Map::Id => false,
        Map::Lower(_) | Map::Upper(_) => region.hi[i] >= 1.0,
        Map::Both => region.lo[i] <= -1.0 || region.hi[i] >= 1.0,
    });
    // 縮めた箱を両側に幅の 1/4 ずつ広げ直す（元の箱は越えない）。格子の点より細い尖った先（√x + √y ≤ 1 の (1, 0) の近く）は
    // 格子では見つからず、縮めた箱の外に切り落とされる。箱の中なら、切り口を探すときの拡大で拾える
    for (i, &(a, b)) in boxed.iter().enumerate() {
        let w = region.hi[i] - region.lo[i];
        region.lo[i] = (region.lo[i] - 0.25 * w).max(a.unwrap_or(f64::NEG_INFINITY));
        region.hi[i] = (region.hi[i] + 0.25 * w).min(b.unwrap_or(f64::INFINITY));
    }
    let v = region.integrate_level(&mut [0.0; 3], 0, None, f, budget);
    // 無限遠まで届く領域で誤差が目標に届かないのは、ほとんど積分が発散しているとき（帯 0 ≤ x + y ≤ 1 の上の 1 など）。
    // 数を返さず、今までの道（「どこまでも続いている」の誤り）に任せる
    if unbounded && budget.rough.get() {
        return None;
    }
    (!budget.exhausted() && v.is_finite()).then_some(v)
}

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
}

impl Region {
    /// d 番目の座標を ts に変えた点（ほかは p のまま）が、領域に入るか
    fn inside_line(&self, p: &[f64; 3], d: usize, ts: &[f64]) -> Vec<bool> {
        let cols: Vec<Vec<f64>> = (0..self.n).map(|k| if k == d { ts.to_vec() } else { vec![p[k]; ts.len()] }).collect();
        self.inside(&cols, ts.len())
    }

    fn inside(&self, cols: &[Vec<f64>], len: usize) -> Vec<bool> {
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
        for zoom in 0..=ZOOMS {
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
        for zoom in 0..=ZOOMS {
            if zoom > 0 && hint.is_none() {
                break;
            }
            let ts = self.samples(d, hint.map_or(0.0, |h| h[d]), zoom);
            // 標本ごとの、領域に入る点（入らなければ None）
            let hits: Vec<Option<[f64; 3]>> = if innermost {
                let flags = self.inside_line(p, d, &ts);
                ts.iter().zip(flags).map(|(&t, ok)| ok.then(|| { let mut q = *p; q[d] = t; q })).collect()
            } else {
                ts.iter().map(|&t| { p[d] = t; self.find(p, d + 1, hint) }).collect()
            };
            if hits.iter().all(|h| h.is_none()) {
                continue;
            }
            // 入っている点 a（領域の点 w）と入っていない点 b の間の境目
            let mut edge = |mut a: f64, mut b: f64, mut w: [f64; 3]| {
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
        for sec in self.sections(p, d, hint) {
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
    let mut region = Region { progs, n, lo: [0.0; 3], hi: [0.0; 3] };
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
    let Some(mut ext) = ext else {
        return Err(if seen {
            "積分する領域が、どこまでも続いているようです（変数の範囲を 0 ≤ x ≤ 1 のように書いてください）".into()
        } else {
            "積分する領域が見つかりません（空か、とても小さいか、原点からとても遠くにあります。変数の範囲を 0 ≤ x ≤ 1 のように書いてください）".into()
        });
    };
    // 箱を領域に合わせて縮める（入る点の範囲の 3 マス外まで。書かれている端は越えない）。箱があまり縮まなくなったら終わり。
    // 格子の点が領域の先の細い所（格子の間隔より細い）を外すので、余裕を持たせる。細い所は、後で線の上の標本と二分法で探す
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
    // 2・3. 切り口ごとに点を置く
    let mut out = Vec::new();
    region.nodes(&mut [0.0; 3], 0, 1.0, None, &mut out);
    Ok(out)
}

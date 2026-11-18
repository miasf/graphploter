//! 適応型の数値積分（値が定数になる積分を、誤差を見積もりながら求める）。
//!
//! グラフに描く積分（端や中身が座標で変わるもの）は、点ごとに同じ式を何万回も計算するので、点の数を決めた
//! ガウス・ルジャンドルの和の式にしている（`expand.rs`）。値が定数になる積分（$\int_0^1 \frac{dt}{\sqrt t}$・
//! 教科書の重積分）は 1 回だけ計算すればよいので、ここで精度よく求める:
//! - 区間ごとにガウス・クロンロッドの 7–15 点の公式で値と誤差を見積もり、誤差のいちばん大きい区間を 2 つに分けていく
//!   （大域的な適応型。QUADPACK の QAG と同じ考え方。誤差の見積もりも QUADPACK の qk15 と同じ式）。
//!   端で √ の形になる関数・端で発散する関数（積分できるもの）・途中で折れる関数も、その近くの区間が細かくなって収束する
//! - 無限の区間は変数変換で有限にする: $[a, \infty)$ は $t = a + s/(1-s)$、$(-\infty, \infty)$ は $[0, \infty)$ と $(-\infty, 0]$ に分ける
//! - 関数の値の計算の回数に上限を付ける（[`Budget`]）。超えたら諦めて、呼び出し側が今までの方法に戻す

use std::cell::Cell;
use std::collections::BinaryHeap;

/// ガウス・クロンロッド 15 点の点（正の側。最後が 0）
const XGK: [f64; 8] = [
    0.991_455_371_120_812_6,
    0.949_107_912_342_758_5,
    0.864_864_423_359_769_1,
    0.741_531_185_599_394_4,
    0.586_087_235_467_691_1,
    0.405_845_151_377_397_2,
    0.207_784_955_007_898_5,
    0.0,
];
/// クロンロッド 15 点の重み
const WGK: [f64; 8] = [
    0.022_935_322_010_529_22,
    0.063_092_092_629_978_55,
    0.104_790_010_322_250_18,
    0.140_653_259_715_525_92,
    0.169_004_726_639_267_9,
    0.190_350_578_064_785_4,
    0.204_432_940_075_298_9,
    0.209_482_141_084_727_83,
];
/// ガウス 7 点の重み（点は XGK[1], XGK[3], XGK[5], 0）
const WG: [f64; 4] = [0.129_484_966_168_869_7, 0.279_705_391_489_276_7, 0.381_830_050_505_118_9, 0.417_959_183_673_469_4];

/// 関数の値を計算する回数の上限（入れ子の積分全体で共有する）
pub struct Budget {
    left: Cell<u64>,
    /// 誤差の見積もりが目標に届かなかった区間があったか
    pub rough: Cell<bool>,
}

impl Budget {
    pub fn new(evals: u64) -> Budget {
        Budget { left: Cell::new(evals), rough: Cell::new(false) }
    }

    /// n 回分を使う。足りなければ false
    pub fn take(&self, n: u64) -> bool {
        let left = self.left.get();
        if left < n {
            self.left.set(0);
            return false;
        }
        self.left.set(left - n);
        true
    }

    pub fn left(&self) -> u64 {
        self.left.get()
    }

    pub fn exhausted(&self) -> bool {
        self.left.get() == 0
    }
}

/// 積分の設定
#[derive(Clone, Copy)]
pub struct Tol {
    /// 相対誤差の目標
    pub rel: f64,
    /// 区間の数の上限
    pub panels: usize,
    /// 有限の区間で、t = 中点 + 半幅 · sin(πs/2) と置き換えるか。端で √ の形に 0 になる関数（円の切り口の長さ）・
    /// 端で 1/√ の形に発散する関数は、置き換えるとなめらかになり、区間をほとんど分けずに収束する
    pub ends: bool,
}

/// 区間 [a, b] の 15 点（`out` に入れる。並びは中心、そのあと ±XGK[j]）
fn nodes15(a: f64, b: f64, out: &mut [f64; 15]) {
    let (c, h) = (0.5 * (a + b), 0.5 * (b - a));
    out[0] = c;
    for j in 0..7 {
        out[1 + 2 * j] = c - h * XGK[j];
        out[2 + 2 * j] = c + h * XGK[j];
    }
}

/// 15 点の値から、積分の値と誤差の見積もり（QUADPACK の qk15 と同じ）
fn rule15(a: f64, b: f64, f: &[f64; 15]) -> (f64, f64, f64) {
    let h = 0.5 * (b - a);
    let fc = f[0];
    let mut resk = WGK[7] * fc;
    let mut resg = WG[3] * fc;
    let mut resabs = resk.abs();
    for j in 0..7 {
        let (f1, f2) = (f[1 + 2 * j], f[2 + 2 * j]);
        resk += WGK[j] * (f1 + f2);
        resabs += WGK[j] * (f1.abs() + f2.abs());
        if j % 2 == 1 {
            resg += WG[j / 2] * (f1 + f2);
        }
    }
    let reskh = 0.5 * resk;
    let mut resasc = WGK[7] * (fc - reskh).abs();
    for j in 0..7 {
        resasc += WGK[j] * ((f[1 + 2 * j] - reskh).abs() + (f[2 + 2 * j] - reskh).abs());
    }
    let (result, ah) = (resk * h, h.abs());
    let (resabs, resasc) = (resabs * ah, resasc * ah);
    let raw = ((resk - resg) * h).abs();
    let mut err = raw;
    if resasc != 0.0 && err != 0.0 {
        err = resasc * (200.0 * err / resasc).powf(1.5).min(1.0);
    }
    if resabs > f64::MIN_POSITIVE / (50.0 * f64::EPSILON) {
        err = err.max(50.0 * f64::EPSILON * resabs);
    }
    (result, err, raw)
}

/// [a, b]（有限）の上の f の積分。`f(ts, out)` は 15 点ずつまとめて計算する。値が有限でなければ NaN を返す
fn finite(f: &mut dyn FnMut(&[f64], &mut [f64]), a: f64, b: f64, tol: Tol, budget: &Budget) -> f64 {
    if a == b {
        return 0.0;
    }
    let mut xs = [0.0; 15];
    let mut fs = [0.0; 15];
    // 区間の値・誤差・分ける所
    let mut eval = |a: f64, b: f64, xs: &mut [f64; 15], fs: &mut [f64; 15]| -> Option<(f64, f64, f64)> {
        if !budget.take(15) {
            return None;
        }
        nodes15(a, b, xs);
        f(xs, fs);
        let r = rule15(a, b, fs);
        if !(r.0.is_finite() && r.1.is_finite()) {
            return None;
        }
        // 折れ目があれば、その位置を 1 点ずつの計算で詰める: 折れ目の左では値が左の直線に、右では右の直線に近い
        let mut err = r.1;
        let cut = match kink(xs, fs) {
            None => 0.5 * (a + b),
            Some(k) => {
                // 折れ目のある区間では、QUADPACK の見積もり（なめらかな関数に合わせて小さめに出る）ではなく、
                // 7 点と 15 点の公式の差そのものを誤差とする
                err = err.max(r.2);
                let (mut lo, mut hi, mut t) = (k.lo, k.hi, k.t);
                for _ in 0..48 {
                    if !budget.take(1) {
                        break;
                    }
                    let mut o = [0.0];
                    f(&[t], &mut o);
                    let (dl, dr) = ((o[0] - quad3(&k.left, t)).abs(), (o[0] - quad3(&k.right, t)).abs());
                    if !(dl.is_finite() && dr.is_finite()) {
                        break;
                    }
                    if dl <= dr { lo = t } else { hi = t }
                    t = 0.5 * (lo + hi);
                    if hi - lo <= 1e-15 * lo.abs().max(hi.abs()).max(1e-300) {
                        break;
                    }
                }
                t
            }
        };
        Some((r.0, err, cut))
    };
    // 区間の並び: (a, b, 値, 誤差, 分ける所)。関数が有限でない点に当たった区間は、値 0・誤差 ∞ にして分け直す
    // （区間の中の 1 点で発散するだけなら、分けると点が区間の端に来て、計算しなくなる）
    let mut panel = |a: f64, b: f64, xs: &mut [f64; 15], fs: &mut [f64; 15]| -> Option<(f64, f64, f64, f64, f64)> {
        if budget.exhausted() {
            return None;
        }
        Some(match eval(a, b, xs, fs) {
            Some((v, e, cut)) => (a, b, v, e, cut),
            None => (a, b, 0.0, f64::INFINITY, 0.5 * (a + b)),
        })
    };
    // 最初は、真ん中から少しずらした所で 2 つに分ける（対称な区間の真ん中 x = 0 は、発散する点・定義されない点のことが多い）
    let split = a + (b - a) * 0.534_726_195_3;
    let (Some(l), Some(r)) = (panel(a, split, &mut xs, &mut fs), panel(split, b, &mut xs, &mut fs)) else { return f64::NAN };
    let mut panels: Vec<(f64, f64, f64, f64, f64)> = vec![l, r];
    // 誤差の大きい順のヒープ（区間の番号つき。分けた区間の古い項目は、誤差が今の値と違うので読み飛ばす）。
    // 値と誤差の合計は増減で持ち、64 回ごとに足し直す（区間が 2 万個でも、分けるたびに全部を見ない）
    let mut heap: BinaryHeap<ByErr> = BinaryHeap::from([ByErr(l.3, 0), ByErr(r.3, 1)]);
    let sums = |ps: &[(f64, f64, f64, f64, f64)]| {
        let total: f64 = ps.iter().map(|p| p.2).sum();
        let err: f64 = ps.iter().map(|p| p.3).sum();
        (total, err)
    };
    let (mut total, mut err) = sums(&panels);
    let mut steps = 0usize;
    loop {
        if steps % 64 == 0 || !err.is_finite() {
            (total, err) = sums(&panels);
        }
        if err <= tol.rel * total.abs() || err <= f64::MIN_POSITIVE {
            return sums(&panels).0;
        }
        // 誤差のいちばん大きい区間を 2 つに分ける
        let k = loop {
            let Some(ByErr(e, k)) = heap.pop() else { return sums(&panels).0 };
            if e.to_bits() == panels[k].3.to_bits() {
                break k;
            }
        };
        let (pa, pb, pv, pe, cut) = panels[k];
        let mid = if cut > pa && cut < pb { cut } else { 0.5 * (pa + pb) };
        if panels.len() >= tol.panels || mid <= pa || mid >= pb {
            // 区間の数の上限・浮動小数点の幅の下限に届いた。有限でない区間が残っていれば、積分できない
            let (total, err) = sums(&panels);
            if !err.is_finite() {
                return f64::NAN;
            }
            if err > 1e3 * tol.rel * total.abs() {
                budget.rough.set(true);
            }
            return total;
        }
        let (Some(l), Some(r)) = (panel(pa, mid, &mut xs, &mut fs), panel(mid, pb, &mut xs, &mut fs)) else { return f64::NAN };
        total += l.2 + r.2 - pv;
        err += l.3 + r.3 - pe;
        panels[k] = l;
        panels.push(r);
        heap.push(ByErr(l.3, k));
        heap.push(ByErr(r.3, panels.len() - 1));
        steps += 1;
    }
}

/// 誤差で並べるヒープの項目（誤差, 区間の番号）
struct ByErr(f64, usize);

impl PartialEq for ByErr {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}
impl Eq for ByErr {}
impl PartialOrd for ByErr {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ByErr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0).then(other.1.cmp(&self.1))
    }
}

/// 区間の中の折れ目（傾きが急に変わる所。|x − c|・min・max・領域の切り口の形が変わる所）の見積もり。
/// 両側の直線（左: 点 (xl, yl) を通る傾き sl、右: (xr, yr)・sr）と、その交点 t、折れ目のある範囲 [lo, hi]
struct Kink {
    t: f64,
    lo: f64,
    hi: f64,
    /// 左側・右側の 3 点（2 次式で近似する。直線だと、曲がった側の曲率の分だけ折れ目の位置がずれる）
    left: [(f64, f64); 3],
    right: [(f64, f64); 3],
}

/// 3 点を通る 2 次式の t での値
fn quad3(p: &[(f64, f64); 3], t: f64) -> f64 {
    let [(x0, y0), (x1, y1), (x2, y2)] = *p;
    y0 * (t - x1) * (t - x2) / ((x0 - x1) * (x0 - x2)) + y1 * (t - x0) * (t - x2) / ((x1 - x0) * (x1 - x2)) + y2 * (t - x0) * (t - x1) / ((x2 - x0) * (x2 - x1))
}

/// 15 点の値から折れ目を探す（はっきりした折れ目が無ければ None）。折れ目で区間を分けると、折れ目が区間の端に来るので、
/// 次の段でほぼ誤差が無くなる（真ん中で分け続けるより、ずっと少ない回数で済む）
fn kink(xs: &[f64; 15], fs: &[f64; 15]) -> Option<Kink> {
    let mut idx: [usize; 15] = std::array::from_fn(|i| i);
    idx.sort_by(|&i, &j| xs[i].total_cmp(&xs[j]));
    let x: [f64; 15] = std::array::from_fn(|k| xs[idx[k]]);
    let y: [f64; 15] = std::array::from_fn(|k| fs[idx[k]]);
    if !y.iter().all(|v| v.is_finite()) {
        return None;
    }
    let s: [f64; 14] = std::array::from_fn(|k| (y[k + 1] - y[k]) / (x[k + 1] - x[k]));
    // 2 階の差分（点 k+1 の近くの曲がり方）
    let d2: [f64; 13] = std::array::from_fn(|k| (s[k + 1] - s[k]).abs() / (x[k + 2] - x[k]));
    let (km, m) = d2.iter().enumerate().fold((0, 0.0f64), |acc, (k, &v)| if v > acc.1 { (k, v) } else { acc });
    let others = d2.iter().enumerate().filter(|(k, _)| k + 1 < km || *k > km + 1).map(|(_, v)| *v).fold(0.0f64, f64::max);
    // 折れ目は [x_km, x_{km+2}] の中。両側のきれいな直線（左は点 km−1, km、右は点 km+2, km+3）の交点
    if !(m > 0.0 && m > 100.0 * others) || km < 2 || km + 4 > 14 {
        return None;
    }
    let (xl, yl, sl) = (x[km], y[km], s[km - 1]);
    let (xr, yr, sr) = (x[km + 2], y[km + 2], s[km + 2]);
    if sl == sr {
        return None;
    }
    let t = (yr - yl + sl * xl - sr * xr) / (sl - sr);
    let t = if t > x[km] && t < x[km + 2] { t } else { 0.5 * (x[km] + x[km + 2]) };
    Some(Kink { t, lo: x[km], hi: x[km + 2], left: [(x[km - 2], y[km - 2]), (x[km - 1], y[km - 1]), (x[km], y[km])], right: [(x[km + 2], y[km + 2]), (x[km + 3], y[km + 3]), (x[km + 4], y[km + 4])] })
}

/// [a, b] の上の f の積分（端は ±∞ でもよい。a > b なら符号を変える）。値が有限でなければ NaN
pub fn integrate(f: &mut dyn FnMut(&[f64], &mut [f64]), a: f64, b: f64, tol: Tol, budget: &Budget) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a > b {
        return -integrate(f, b, a, tol, budget);
    }
    if a == b {
        return 0.0;
    }
    match (a.is_finite(), b.is_finite()) {
        (true, true) if tol.ends => {
            // t = c + h sin(πs/2)、dt = h (π/2) cos(πs/2) ds
            let (c, h) = (0.5 * (a + b), 0.5 * (b - a));
            let q = std::f64::consts::FRAC_PI_2;
            let mut ts = [0.0; 15];
            let mut g = |ss: &[f64], out: &mut [f64]| {
                let n = ss.len();
                for (t, &s) in ts[..n].iter_mut().zip(ss) {
                    *t = c + h * (q * s).sin();
                }
                f(&ts[..n], out);
                for (o, &s) in out.iter_mut().zip(ss) {
                    *o *= h * q * (q * s).cos();
                }
            };
            finite(&mut g, -1.0, 1.0, tol, budget)
        }
        (true, true) => finite(f, a, b, tol, budget),
        (true, false) => infinite(f, a, 1.0, tol, budget),
        (false, true) => infinite(f, b, -1.0, tol, budget),
        (false, false) => infinite(f, 0.0, 1.0, tol, budget) + infinite(f, 0.0, -1.0, tol, budget),
    }
}

/// base から dir の向きの無限の区間: t = base + dir · L s / (1 − s)、dt = L ds / (1 − s)²（s ∈ [0, 1)。L = max(1, |base|)）
fn half_line(f: &mut dyn FnMut(&[f64], &mut [f64]), base: f64, dir: f64, tol: Tol, budget: &Budget) -> f64 {
    // 縮尺 L: 端が原点から遠い（∫_4097^∞）と、L = 1 では被積分関数が s = 1 のすぐ近くに鋭い山を作る
    let l = base.abs().max(1.0);
    let mut ts = [0.0; 15];
    let mut g = |ss: &[f64], out: &mut [f64]| {
        let n = ss.len();
        for (t, &s) in ts[..n].iter_mut().zip(ss) {
            *t = base + dir * l * s / (1.0 - s);
        }
        f(&ts[..n], out);
        for (o, &s) in out.iter_mut().zip(ss) {
            let j = l / ((1.0 - s) * (1.0 - s));
            // 無限遠で 0 になる関数は、変換の重みが大きくても 0（0 · ∞ を NaN にしない）
            *o = if *o == 0.0 { 0.0 } else { *o * j };
        }
    };
    // 端の置き換えも使う（∫_N^∞ x^(−3/2) dx のように遅く 0 になる関数は、写した後に s = 1 で 1/√ の形に発散する）
    integrate(&mut g, 0.0, 1.0, Tol { ends: true, ..tol }, budget)
}

/// base から dir の向きの無限の区間。写して積分し、収束しなければ（振動して遅く 0 になる関数。∫₀^∞ sin t / t dt）、
/// 零点の間ごとに積分して部分和を Wynn の ε で加速する（[`oscillatory`]）
fn infinite(f: &mut dyn FnMut(&[f64], &mut [f64]), base: f64, dir: f64, tol: Tol, budget: &Budget) -> f64 {
    let was = budget.rough.replace(false);
    let v = half_line(f, base, dir, tol, budget);
    let rough = budget.rough.get();
    if v.is_finite() && !rough {
        budget.rough.set(was);
        return v;
    }
    if let Some(w) = oscillatory(f, base, dir, tol, budget) {
        budget.rough.set(was);
        return w;
    }
    budget.rough.set(was || rough);
    v
}

/// 振動する関数の、base から dir の向きの無限の区間の積分。零点を順に探し（間隔は直前の零点の間隔の 1/8 の歩幅で。
/// 振動が速くなっていく sin(t²) にもついていく）、零点の間ごとの積分の部分和を Wynn の ε 算法で加速する。
/// 部分和の数を 2 通り（32・48）にして、加速した値が合えば返す。零点が見つからない（振動しない）・合わなければ None
fn oscillatory(f: &mut dyn FnMut(&[f64], &mut [f64]), base: f64, dir: f64, tol: Tol, budget: &Budget) -> Option<f64> {
    let mut at = |x: f64| -> Option<f64> {
        if !budget.take(1) {
            return None;
        }
        let mut o = [0.0];
        f(&[x], &mut o);
        o[0].is_finite().then_some(o[0])
    };
    const ZEROS: usize = 49;
    let mut zeros: Vec<f64> = Vec::with_capacity(ZEROS);
    let mut step = 0.1f64;
    let (mut x, mut fx) = (base, at(base)?);
    let limit = 1e4 * (1.0 + base.abs());
    while zeros.len() < ZEROS {
        let nx = x + dir * step;
        if (nx - base).abs() > limit && zeros.len() < 4 {
            return None;
        }
        let fn_ = at(nx)?;
        if fx != 0.0 && fn_ != 0.0 && fx.signum() != fn_.signum() {
            // 符号の変わる所を二分法で詰める
            let (mut lo, mut hi, flo) = (x, nx, fx);
            for _ in 0..60 {
                let m = 0.5 * (lo + hi);
                if m == lo || m == hi {
                    break;
                }
                let fm = at(m)?;
                if fm.signum() == flo.signum() { lo = m } else { hi = m }
            }
            let z = 0.5 * (lo + hi);
            if let Some(&last) = zeros.last() {
                step = ((z - last).abs() / 8.0).max(1e-9 * (1.0 + z.abs()));
            }
            zeros.push(z);
        }
        (x, fx) = (nx, fn_);
    }
    // 部分和 S_k = ∫_base^{z_k} f
    let inner = Tol { panels: 200, ..tol };
    let mut sums = Vec::with_capacity(ZEROS);
    let mut acc = 0.0;
    let mut from = base;
    for &z in &zeros {
        let (a, b) = if dir > 0.0 { (from, z) } else { (z, from) };
        let piece = dir * finite(f, a, b, inner, budget);
        if !piece.is_finite() {
            return None;
        }
        acc += if dir > 0.0 { piece } else { -piece };
        sums.push(acc);
        from = z;
    }
    let (e1, e2) = (wynn(&sums[..33]), wynn(&sums));
    ((e1 - e2).abs() <= 1e-10 * e2.abs().max(1e-300) || (e1 - e2).abs() <= 1e-14).then_some(e2)
}

/// Wynn の ε 算法（数列の極限を加速する）。ε_{k+1}^{(j)} = ε_{k−1}^{(j+1)} + 1 / (ε_k^{(j+1)} − ε_k^{(j)}) の偶数の列の最後の値
fn wynn(s: &[f64]) -> f64 {
    let mut prev = vec![0.0; s.len() + 1];
    let mut cur = s.to_vec();
    let mut best = *s.last().unwrap_or(&f64::NAN);
    let mut k = 0;
    while cur.len() >= 2 {
        let mut next = Vec::with_capacity(cur.len() - 1);
        for j in 0..cur.len() - 1 {
            let d = cur[j + 1] - cur[j];
            if d == 0.0 {
                return if k % 2 == 0 { cur[j + 1] } else { best };
            }
            next.push(prev[j + 1] + 1.0 / d);
        }
        k += 1;
        if k % 2 == 0 {
            best = *next.last().unwrap_or(&best);
        }
        prev = cur;
        cur = next;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int1(f: impl Fn(f64) -> f64, a: f64, b: f64) -> f64 {
        let budget = Budget::new(10_000_000);
        let mut g = |xs: &[f64], out: &mut [f64]| {
            for (o, &x) in out.iter_mut().zip(xs) {
                *o = f(x);
            }
        };
        integrate(&mut g, a, b, Tol { rel: 1e-12, panels: 2000, ends: false }, &budget)
    }

    #[test]
    fn smooth_singular_and_infinite() {
        use std::f64::consts::PI;
        let close = |got: f64, want: f64, tol: f64| assert!((got - want).abs() <= tol * want.abs().max(1.0), "{got} vs {want}");
        close(int1(|x| x * x, 0.0, 1.0), 1.0 / 3.0, 1e-15);
        close(int1(|x| x.sqrt(), 0.0, 1.0), 2.0 / 3.0, 1e-12);
        close(int1(|x| 1.0 / x.sqrt(), 0.0, 1.0), 2.0, 1e-10);
        close(int1(|x| x.ln(), 0.0, 1.0), -1.0, 1e-11);
        close(int1(|x| x.sin(), 0.0, 100.0), 1.0 - 100f64.cos(), 1e-12);
        close(int1(|x| (x - 0.3).abs(), 0.0, 1.0), 0.29, 1e-13);
        close(int1(|x| x.min(1.0 - x * x), 0.0, 1.0), 0.5 * 0.618_033_988_749_895f64.powi(2) + (1.0 - 1.0 / 3.0) - (0.618_033_988_749_895 - 0.618_033_988_749_895f64.powi(3) / 3.0), 1e-12);
        close(int1(|x| (-x * x).exp(), f64::NEG_INFINITY, f64::INFINITY), PI.sqrt(), 1e-12);
        close(int1(|x| 1.0 / (1.0 + x * x), 0.0, f64::INFINITY), PI / 2.0, 1e-12);
        close(int1(|x| x.powf(-1.5), 1.0, f64::INFINITY), 2.0, 1e-11);
        close(int1(|x| if x == 0.0 { 1.0 } else { x.sin() / x }, 0.0, f64::INFINITY), PI / 2.0, 1e-9);
        close(int1(|x| (x * x).sin(), 0.0, f64::INFINITY), (PI / 8.0).sqrt(), 1e-9);
        close(int1(|x| x, 1.0, 0.0), -0.5, 1e-15);
    }
}

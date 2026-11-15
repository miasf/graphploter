//! 画素ごとに「この画素に曲線（曲面）が写るか」を、飾り付きの区間演算（[`crate::interval::DecInterval`]）で判定する基準の実装
//! （計画 `docs/plan-hard-tasks.md` 12 章の G2）。GPU の版（G3〜G5）はこれを GLSL に移したもので、結果を画素ごとに突き合わせる。
//!
//! 判定の 3 つの結果（Tupper と同じ）。箱 B で:
//! - 区間 F(B) が 0 を含まない（または空）→ **無い**
//! - B で連続（飾りが Dac 以上）で、B の中の 2 点で F の符号が逆 → 中間値の定理で **必ずある**
//! - それ以外 → **分からない**。B を分ける。分けられる限度（画素の大きさ）に来たら、連続なら描く（接するだけの零点。
//!   符号の変わらない $(x^2+y^2+z^2-1)^2 = 0$ など）、連続でなければ描かない（極・段差）

use crate::eval::Program;
use crate::interval::{Dec, DecInterval, Interval};
use num_complex::Complex64 as C;

/// 2D の矩形の判定の結果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// 曲線は通らない
    None,
    /// 必ず通る（連続で、符号が変わる）
    Sure,
    /// 分からないが連続（接するだけの零点かもしれない。描く）
    MaybeContinuous,
    /// 分からず、連続とも言えない（極・段差。描かない）
    MaybeBroken,
}

impl Cell {
    /// 描くか
    pub fn drawn(self) -> bool {
        matches!(self, Cell::Sure | Cell::MaybeContinuous)
    }
}

/// 点での F の値（実数。実数にならなければ NaN）
fn value_at(prog: &Program, p: &[f64]) -> f64 {
    let cols: Vec<[f64; 1]> = p.iter().map(|&v| [v]).collect();
    let refs: Vec<&[f64]> = cols.iter().map(|c| &c[..]).collect();
    let mut out = [C::new(0.0, 0.0)];
    prog.eval_batch(&refs, &mut out);
    crate::eval::as_real(out[0])
}

/// 符号の逆な 2 つの値があるか
fn sign_change(vals: &[f64]) -> bool {
    let pos = vals.iter().any(|&v| v > 0.0);
    let neg = vals.iter().any(|&v| v < 0.0);
    pos && neg
}

/// 2D: 矩形 [x0, x1] × [y0, y1] を、`depth` 段まで 4 分割しながら判定する。`prog` の変数は (横, 奥)
pub fn classify_rect(prog: &Program, rect: [f64; 4], depth: u32) -> Cell {
    let [x0, x1, y0, y1] = rect;
    let d = prog.eval_interval_dec(&[Interval::new(x0, x1), Interval::new(y0, y1)]);
    if d.is_empty() || !d.may_contain_zero() {
        return Cell::None;
    }
    if d.dec >= Dec::Dac {
        // 4 隅と中心の 5 点
        let (xm, ym) = (0.5 * (x0 + x1), 0.5 * (y0 + y1));
        let vals: Vec<f64> = [[x0, y0], [x1, y0], [x0, y1], [x1, y1], [xm, ym]].iter().map(|p| value_at(prog, p)).collect();
        if sign_change(&vals) {
            return Cell::Sure;
        }
    }
    if depth == 0 {
        return if d.dec >= Dec::Dac { Cell::MaybeContinuous } else { Cell::MaybeBroken };
    }
    let (xm, ym) = (0.5 * (x0 + x1), 0.5 * (y0 + y1));
    let mut best = Cell::None;
    for r in [[x0, xm, y0, ym], [xm, x1, y0, ym], [x0, xm, ym, y1], [xm, x1, ym, y1]] {
        let c = classify_rect(prog, r, depth - 1);
        best = match (best, c) {
            (_, Cell::Sure) => return Cell::Sure,
            (Cell::MaybeContinuous, _) | (_, Cell::MaybeContinuous) => Cell::MaybeContinuous,
            (Cell::MaybeBroken, _) | (_, Cell::MaybeBroken) => Cell::MaybeBroken,
            _ => Cell::None,
        };
    }
    best
}

/// 光線の当たり方
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RayHit {
    /// 当たらない
    Miss,
    /// t で当たった。`sure` は符号が変わる零点（false は、連続で符号の変わらない接するだけの零点）
    Hit { t: f64, sure: bool },
    /// 繰り返しの上限で打ち切った
    Aborted,
}

/// 光線の設定
#[derive(Debug, Clone, Copy)]
pub struct RaySettings {
    /// 距離 t での 1 画素の大きさは `pixel * t`（$2\tan(\mathrm{fov}/2) / H$。H は画面の高さの画素数）
    pub pixel: f64,
    /// 区間を分ける回数の上限
    pub max_iter: usize,
}

/// 3D: 光線 p(t) = o + t d（t ∈ [t0, t1]）が F = 0 に最初に当たる所（Mitchell・Knoll の方法）。`prog` の変数は (x, y, z)。
/// 光線の区間 T の上の箱 B = o + T d で F(B) を区間演算し、0 を含まなければ捨てる。含めば、T の幅がその深さでの
/// 1 画素の大きさより大きい間は二分し、手前の半分を先に調べる。1 画素の大きさになったら、上の 3 つの結果で判定する
pub fn ray_hit(prog: &Program, o: [f64; 3], d: [f64; 3], t0: f64, t1: f64, s: RaySettings) -> RayHit {
    let at = |t: f64| [o[0] + t * d[0], o[1] + t * d[1], o[2] + t * d[2]];
    let boxes = |a: f64, b: f64| -> [Interval; 3] {
        let (p, q) = (at(a), at(b));
        [0, 1, 2].map(|k| Interval::new(p[k].min(q[k]), p[k].max(q[k])))
    };
    let mut stack = vec![(t0, t1)];
    let mut iter = 0;
    while let Some((a, b)) = stack.pop() {
        iter += 1;
        if iter > s.max_iter {
            return RayHit::Aborted;
        }
        let f: DecInterval = prog.eval_interval_dec(&boxes(a, b));
        if f.is_empty() || !f.may_contain_zero() {
            continue;
        }
        let eps = (s.pixel * a.max(1e-9)).max(1e-12);
        if b - a > eps {
            let m = 0.5 * (a + b);
            stack.push((m, b)); // 奥の半分（後で調べる）
            stack.push((a, m)); // 手前の半分（先に調べる）
            continue;
        }
        if f.dec < Dec::Dac {
            continue; // 極・段差（連続でない）。描かない
        }
        let (fa, fb) = (value_at(prog, &at(a)), value_at(prog, &at(b)));
        if fa * fb < 0.0 {
            // 二分法で詰める
            let (mut lo, mut hi, mut flo) = (a, b, fa);
            for _ in 0..50 {
                let m = 0.5 * (lo + hi);
                let fm = value_at(prog, &at(m));
                if fm == 0.0 {
                    return RayHit::Hit { t: m, sure: true };
                }
                if flo * fm < 0.0 {
                    hi = m;
                } else {
                    lo = m;
                    flo = fm;
                }
            }
            return RayHit::Hit { t: 0.5 * (lo + hi), sure: true };
        }
        if fa == 0.0 || fb == 0.0 {
            return RayHit::Hit { t: if fa == 0.0 { a } else { b }, sure: true };
        }
        // 連続で、符号が同じ: 接するだけの零点か、かすめるだけ（区間演算の広がり）か。1/16 画素まで詰めて決める
        match refine_touch(prog, &at, &boxes, a, b, eps / 16.0, 64) {
            Some(hit) => return hit,
            None => continue,
        }
    }
    RayHit::Miss
}

/// 連続で両端の符号が同じ区間 [a, b] を、幅 `eps` まで詰める（`budget` 回まで）。
/// 符号の変わる所が見つかれば当たり、全部の小区間で 0 を含まなければ None（当たらない）、
/// 詰めきっても 0 を含む小区間が残れば、接するだけの零点としてその真ん中
fn refine_touch(
    prog: &Program,
    at: &dyn Fn(f64) -> [f64; 3],
    boxes: &dyn Fn(f64, f64) -> [Interval; 3],
    a: f64,
    b: f64,
    eps: f64,
    mut budget: usize,
) -> Option<RayHit> {
    let mut stack = vec![(a, b)];
    let mut touch = None;
    while let Some((lo, hi)) = stack.pop() {
        if budget == 0 {
            return Some(RayHit::Hit { t: touch.unwrap_or(0.5 * (a + b)), sure: false });
        }
        budget -= 1;
        let f = prog.eval_interval_dec(&boxes(lo, hi));
        if f.is_empty() || !f.may_contain_zero() {
            continue;
        }
        let m = 0.5 * (lo + hi);
        let (fl, fm, fh) = (value_at(prog, &at(lo)), value_at(prog, &at(m)), value_at(prog, &at(hi)));
        if fl * fm <= 0.0 || fm * fh <= 0.0 {
            // 符号が変わる: 二分法で詰める
            let (mut x0, mut x1, mut f0) = if fl * fm <= 0.0 { (lo, m, fl) } else { (m, hi, fm) };
            for _ in 0..50 {
                let c = 0.5 * (x0 + x1);
                let fc = value_at(prog, &at(c));
                if f0 * fc <= 0.0 {
                    x1 = c;
                } else {
                    x0 = c;
                    f0 = fc;
                }
            }
            return Some(RayHit::Hit { t: 0.5 * (x0 + x1), sure: true });
        }
        if hi - lo <= eps {
            touch.get_or_insert(m);
            // 手前の接する所を返す（奥は見なくてよい）
            return Some(RayHit::Hit { t: m, sure: false });
        }
        stack.push((m, hi));
        stack.push((lo, m));
    }
    touch.map(|t| RayHit::Hit { t, sure: false })
}

/// 光線と箱（表示範囲）の交わる t の範囲。交わらなければ None
pub fn ray_box(o: [f64; 3], d: [f64; 3], lo: [f64; 3], hi: [f64; 3]) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for k in 0..3 {
        if d[k] == 0.0 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
    }
    (t0 < t1).then_some((t0, t1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn prog(src: &str, vars: &[&str]) -> Program {
        Program::compile(&parse(src).unwrap(), vars).unwrap()
    }

    #[test]
    fn rect_cases() {
        let circle = prog("x^2 + y^2 - 1", &["x", "y"]);
        assert_eq!(classify_rect(&circle, [0.9, 1.1, -0.05, 0.05], 4), Cell::Sure);
        assert_eq!(classify_rect(&circle, [2.0, 3.0, 2.0, 3.0], 4), Cell::None);
        // 接するだけの円（符号が変わらない）は、連続なので描く
        let touch = prog("(x^2 + y^2 - 1)^2", &["x", "y"]);
        assert!(classify_rect(&touch, [0.99, 1.01, -0.01, 0.01], 6).drawn());
        // tan の極は描かない
        let pole = prog("tan(x) - y", &["x", "y"]);
        let c = classify_rect(&pole, [1.5707, 1.5709, 100.0, 100.1], 6);
        assert!(!c.drawn(), "{c:?}");
        // floor の段差の縦の線は描かない
        let step = prog("floor(x) - y", &["x", "y"]);
        assert!(!classify_rect(&step, [0.999, 1.001, 0.4, 0.6], 6).drawn());
        assert!(classify_rect(&step, [0.4, 0.6, 0.999, 1.001], 6).drawn() || classify_rect(&step, [0.4, 0.6, -0.001, 0.001], 6).drawn());
    }

    #[test]
    fn ray_cases() {
        let s = RaySettings { pixel: 1e-3, max_iter: 2048 };
        let sphere = prog("x^2+y^2+z^2-4", &["x", "y", "z"]);
        match ray_hit(&sphere, [-10.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.0, 20.0, s) {
            RayHit::Hit { t, sure: true } => assert!((t - 8.0).abs() < 1e-9, "{t}"),
            r => panic!("{r:?}"),
        }
        assert_eq!(ray_hit(&sphere, [-10.0, 3.0, 0.0], [1.0, 0.0, 0.0], 0.0, 20.0, s), RayHit::Miss);
        // 接する球
        let touch = prog("(x^2+y^2+z^2-1)^2", &["x", "y", "z"]);
        assert!(matches!(ray_hit(&touch, [-10.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.0, 20.0, s), RayHit::Hit { .. }));
        // 格子より細い球（半径 0.01）
        let tiny = prog("x^2+y^2+z^2-0.0001", &["x", "y", "z"]);
        match ray_hit(&tiny, [-10.0, 0.003, 0.0], [1.0, 0.0, 0.0], 0.0, 20.0, s) {
            RayHit::Hit { t, .. } => assert!((t - 10.0).abs() < 0.01),
            r => panic!("{r:?}"),
        }
        // tan の極の上に壁を作らない（光線は x 方向。x = π/2 で F は +∞ から −∞ へ飛ぶ）
        let pole = prog("tan(x) - z y - 100", &["x", "y", "z"]);
        if let RayHit::Hit { t, .. } = ray_hit(&pole, [1.0, 0.0, 0.0], [1.0, 0.0, 0.0], 0.0, 1.0, s) {
            let x = 1.0 + t;
            assert!((x.tan() - 100.0).abs() < 1.0, "x = {x}");
        }
    }

    #[test]
    fn box_entry() {
        let r = ray_box([-10.0, 0.0, 0.0], [1.0, 0.0, 0.0], [-1.0; 3], [1.0; 3]).unwrap();
        assert_eq!(r, (9.0, 11.0));
        assert!(ray_box([-10.0, 5.0, 0.0], [1.0, 0.0, 0.0], [-1.0; 3], [1.0; 3]).is_none());
    }
}

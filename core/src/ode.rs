//! 常微分方程式の解法（計画 `docs/plan-math-notation.md` の M9）。ドルマン・プリンス法（5 次・4 次の組、刻み幅は自動）。
//!
//! $$\frac{d\mathbf y}{dt} = \mathbf f(t, \mathbf y), \qquad \text{1 歩の誤差の見積もり} \le 10^{-9} + 10^{-6}\,\lVert\mathbf y\rVert_\infty$$
//!
//! 同じ入力なら同じ出力になるように、時間ではなく歩数だけで打ち切る

/// 1 本の解の歩数の上限
pub const MAX_STEPS: usize = 200_000;

/// 解き方
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// 状態がこの箱（成分ごとの [最小, 最大]）を出たら止める。None なら止めない
    pub limit: f64,
    /// 歩の弦がこれより長ければ、エルミートの 3 次補間で間を埋める（出力の点の間隔の目安）
    pub chord: f64,
    /// 弧長（状態の先頭 `arc_dims` 成分の）の上限。無限大なら制限なし
    pub max_arc: f64,
    pub arc_dims: usize,
    /// 歩数の上限
    pub max_steps: usize,
}

/// 解の点と、歩数の上限で打ち切ったか
#[derive(Debug, Clone)]
pub struct Solution {
    /// 点ごとの (t, 状態)
    pub points: Vec<(f64, Vec<f64>)>,
    pub truncated: bool,
}

// ドルマン・プリンス法の係数
const C: [f64; 7] = [0.0, 1.0 / 5.0, 3.0 / 10.0, 4.0 / 5.0, 8.0 / 9.0, 1.0, 1.0];
const A: [[f64; 6]; 7] = [
    [0.0; 6],
    [1.0 / 5.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    [3.0 / 40.0, 9.0 / 40.0, 0.0, 0.0, 0.0, 0.0],
    [44.0 / 45.0, -56.0 / 15.0, 32.0 / 9.0, 0.0, 0.0, 0.0],
    [19372.0 / 6561.0, -25360.0 / 2187.0, 64448.0 / 6561.0, -212.0 / 729.0, 0.0, 0.0],
    [9017.0 / 3168.0, -355.0 / 33.0, 46732.0 / 5247.0, 49.0 / 176.0, -5103.0 / 18656.0, 0.0],
    [35.0 / 384.0, 0.0, 500.0 / 1113.0, 125.0 / 192.0, -2187.0 / 6784.0, 11.0 / 84.0],
];
/// 5 次の重み（A の最後の行と同じ）と、4 次との差
const B5: [f64; 7] = [35.0 / 384.0, 0.0, 500.0 / 1113.0, 125.0 / 192.0, -2187.0 / 6784.0, 11.0 / 84.0, 0.0];
const E: [f64; 7] = [
    71.0 / 57600.0,
    0.0,
    -71.0 / 16695.0,
    71.0 / 1920.0,
    -17253.0 / 339200.0,
    22.0 / 525.0,
    -1.0 / 40.0,
];

/// `y0` から、時刻 `t0` から `t1` へ（t1 < t0 なら過去へ）解く。`f(t, y, dy)` は dy に右辺を書く
pub fn solve(f: &mut dyn FnMut(f64, &[f64], &mut [f64]), t0: f64, y0: &[f64], t1: f64, opt: Options) -> Solution {
    let n = y0.len();
    let mut points = vec![(t0, y0.to_vec())];
    if t1 == t0 || n == 0 {
        return Solution { points, truncated: false };
    }
    let dir = (t1 - t0).signum();
    let span = (t1 - t0).abs();
    let mut t = t0;
    let mut y = y0.to_vec();
    let mut k: Vec<Vec<f64>> = vec![vec![0.0; n]; 7];
    f(t, &y, &mut k[0]);
    let mut h = (span / 100.0).min(0.01 * span.max(1.0)) * dir;
    let mut tmp = vec![0.0; n];
    let mut y5 = vec![0.0; n];
    let mut arc = 0.0;
    let mut steps = 0;
    let finite = |v: &[f64]| v.iter().all(|x| x.is_finite());
    if !finite(&k[0]) {
        return Solution { points, truncated: false };
    }
    while (t1 - t) * dir > 0.0 {
        if steps >= opt.max_steps {
            return Solution { points, truncated: true };
        }
        steps += 1;
        if (t + h - t1) * dir > 0.0 {
            h = t1 - t;
        }
        for s in 1..7 {
            for i in 0..n {
                tmp[i] = y[i] + h * (0..s).map(|j| A[s][j] * k[j][i]).sum::<f64>();
            }
            let (head, tail) = k.split_at_mut(s);
            let _ = head;
            f(t + C[s] * h, &tmp, &mut tail[0]);
        }
        // 5 次の解（k[6] は、その点での右辺。次の歩の k[0] に使う）
        for i in 0..n {
            y5[i] = y[i] + h * (0..7).map(|j| B5[j] * k[j][i]).sum::<f64>();
        }
        let scale = 1e-9 + 1e-6 * y.iter().chain(y5.iter()).fold(0.0f64, |m, v| m.max(v.abs()));
        let err = (0..n).map(|i| (h * (0..7).map(|j| E[j] * k[j][i]).sum::<f64>()).abs()).fold(0.0, f64::max) / scale;
        if !err.is_finite() || !finite(&y5) {
            // 値が有限でない: 刻みを小さくしてやり直す。小さくしすぎたら、そこで止める
            h *= 0.25;
            if h.abs() < 1e-14 * span.max(1.0) {
                break;
            }
            continue;
        }
        if err > 1.0 {
            h *= (0.9 * err.powf(-0.2)).max(0.2);
            if h.abs() < 1e-14 * span.max(1.0) {
                break;
            }
            continue;
        }
        // 受け入れる。弦が長ければ、エルミートの 3 次補間で間を埋める
        let (dy0, dy1) = (k[0].clone(), k[6].clone());
        let chord = (0..opt.arc_dims.min(n)).map(|i| (y5[i] - y[i]).powi(2)).sum::<f64>().sqrt();
        if chord > opt.chord && opt.chord > 0.0 {
            let m = ((chord / opt.chord).ceil() as usize).min(64);
            for j in 1..m {
                let s = j as f64 / m as f64;
                let (h00, h10, h01, h11) = (2.0 * s.powi(3) - 3.0 * s * s + 1.0, s.powi(3) - 2.0 * s * s + s, -2.0 * s.powi(3) + 3.0 * s * s, s.powi(3) - s * s);
                let p = (0..n).map(|i| h00 * y[i] + h10 * h * dy0[i] + h01 * y5[i] + h11 * h * dy1[i]).collect();
                points.push((t + s * h, p));
            }
        }
        arc += chord;
        t += h;
        y.copy_from_slice(&y5);
        points.push((t, y.clone()));
        k[0] = dy1;
        if y.iter().any(|v| v.abs() > opt.limit) || arc > opt.max_arc {
            break;
        }
        h *= (0.9 * err.max(1e-10).powf(-0.2)).min(5.0);
    }
    Solution { points, truncated: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opt() -> Options {
        Options { limit: 1e9, chord: 0.0, max_arc: f64::INFINITY, arc_dims: 2, max_steps: MAX_STEPS }
    }

    #[test]
    fn exponential() {
        let s = solve(&mut |_, y, dy| dy[0] = y[0], 0.0, &[1.0], 5.0, opt());
        let (t, y) = s.points.last().unwrap();
        assert_eq!(*t, 5.0);
        assert!((y[0] / 5f64.exp() - 1.0).abs() < 1e-5, "{}", y[0]);
        // 過去へ
        let s = solve(&mut |_, y, dy| dy[0] = y[0], 0.0, &[1.0], -3.0, opt());
        assert!((s.points.last().unwrap().1[0] / (-3f64).exp() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn circle_stays_on_circle_and_is_deterministic() {
        let run = || solve(&mut |_, y, dy| {
            dy[0] = -y[1];
            dy[1] = y[0];
        }, 0.0, &[1.0, 0.0], 20.0 * std::f64::consts::PI, Options { chord: 0.05, ..opt() });
        let a = run();
        assert!(a.points.iter().all(|(_, p)| (p[0] * p[0] + p[1] * p[1] - 1.0).abs() < 1e-4));
        let b = run();
        assert_eq!(a.points.len(), b.points.len());
        assert!(a.points.iter().zip(&b.points).all(|(x, y)| x.0.to_bits() == y.0.to_bits() && x.1.iter().zip(&y.1).all(|(u, v)| u.to_bits() == v.to_bits())));
    }

    #[test]
    fn blow_up_stops_at_the_box() {
        // y' = y²、y(0) = 1 は t = 1 で発散する
        let s = solve(&mut |_, y, dy| dy[0] = y[0] * y[0], 0.0, &[1.0], 5.0, Options { limit: 100.0, ..opt() });
        let (t, y) = s.points.last().unwrap();
        assert!(*t < 1.0 && y[0] > 100.0 && !s.truncated);
    }
}

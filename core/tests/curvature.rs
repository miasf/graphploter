//! 曲率の色分け（計画 N2）: plot の flags のビット 6（ガウス曲率 K）・ビット 7（平均曲率 H）

use graph_core::plot;

const GAUSS: u32 = 64;
const MEAN: u32 = 128;

struct Out {
    kind: String,
    /// 頂点の座標と、その頂点の曲率
    verts: Vec<([f32; 3], f32)>,
    range: Vec<f64>,
    diverging: bool,
    available: bool,
    gpu_reason: String,
}

fn draw_with(src: &str, ranges: (&str, &[f64]), d: f64, flags: u32, rotation: &[f64]) -> Out {
    graph_core::definitions("");
    let mut m = plot(src, &[-d, d, -d, d, -d, d], 64, 48, ranges.0, ranges.1, "", &[], "", 3, &[0, 1, 2], &[0.0; 3], &[f64::NAN; 6], "", &[], 16, rotation, &[0.0; 5], 0.0, 0, false, false, 1, false, flags)
        .unwrap_or_else(|_| panic!("{src} が描けない"));
    let pos = m.positions();
    let vals = m.take_complex_values();
    let verts = pos
        .chunks_exact(3)
        .enumerate()
        .map(|(k, p)| ([p[0], p[1], p[2]], vals.get(3 * k).copied().unwrap_or(f32::NAN)))
        .collect();
    Out { kind: m.kind(), verts, range: m.value_range(), diverging: m.diverging(), available: m.curvature_available(), gpu_reason: m.gpu_reason() }
}

fn draw(src: &str, flags: u32) -> Out {
    draw_with(src, ("", &[]), 5.0, flags, &[0.0; 10])
}

/// 表示範囲の箱の縁（切れた所）から離れた頂点だけ
fn inner(o: &Out, d: f32) -> impl Iterator<Item = &([f32; 3], f32)> {
    o.verts.iter().filter(move |(p, _)| p.iter().all(|c| c.abs() < d))
}

/// 球 x² + y² + z² = 4: K = 1/4、H = 1/2（中が F < 0 の向き）。頂点はどこでも同じ値
#[test]
fn sphere_has_constant_curvature() {
    for (flags, want) in [(GAUSS, 0.25), (MEAN, 0.5)] {
        let o = draw("x^2+y^2+z^2=4", flags);
        assert!(o.available && o.diverging, "{}", o.kind);
        assert!(o.verts.len() > 500);
        let worst = o.verts.iter().map(|(_, v)| (v - want).abs()).fold(0.0, f32::max);
        assert!(worst < 1e-5, "flags {flags}: 誤差 {worst}");
        assert!((o.range[1] - want as f64).abs() < 1e-6 && (o.range[0] + want as f64).abs() < 1e-6, "{:?}", o.range);
        assert!(o.kind.contains(if flags == GAUSS { "ガウス曲率" } else { "平均曲率" }), "{}", o.kind);
        // 光線は曲率を求めないので、メッシュで描く
        assert!(o.gpu_reason.contains("曲率"), "{}", o.gpu_reason);
    }
}

/// トーラス（中心の円の半径 2、管の半径 1）: K の符号が外側（正）と内側（負）で変わる。
/// 外側の赤道 K = 1/3、内側の赤道 K = −1
#[test]
fn torus_gauss_changes_sign() {
    let o = draw("(sqrt(x^2+y^2)-2)^2+z^2=1", GAUSS);
    let at = |r: f32| {
        o.verts
            .iter()
            .filter(|(p, _)| p[2].abs() < 0.15 && ((p[0].hypot(p[1])) - r).abs() < 0.1)
            .map(|(_, v)| *v)
            .collect::<Vec<_>>()
    };
    let (outer, inside) = (at(3.0), at(1.0));
    assert!(!outer.is_empty() && !inside.is_empty());
    assert!(outer.iter().all(|v| (v - 1.0 / 3.0).abs() < 1e-2), "{outer:?}");
    assert!(inside.iter().all(|v| (v + 1.0).abs() < 3e-2), "{inside:?}");
}

/// 陽関数 z = x² + y²（下に凸）: 原点で K = 4・H = 2。z = x² − y²（鞍）: 原点で K = −4・H = 0
#[test]
fn explicit_surfaces() {
    let near0 = |o: &Out| -> f32 {
        let (_, v) = o.verts.iter().min_by(|a, b| (a.0[0].hypot(a.0[1])).total_cmp(&b.0[0].hypot(b.0[1]))).unwrap();
        *v
    };
    for (src, flags, want) in [("z=x^2+y^2", GAUSS, 4.0), ("z=x^2+y^2", MEAN, 2.0), ("z=x^2-y^2", GAUSS, -4.0), ("z=x^2-y^2", MEAN, 0.0)] {
        let o = draw(src, flags);
        assert!(o.available, "{src}");
        let v = near0(&o);
        assert!((v - want).abs() < 1e-3, "{src} flags {flags}: {v}");
        // 鞍はどこでも K < 0
        if src == "z=x^2-y^2" && flags == GAUSS {
            assert!(inner(&o, 4.9).all(|(_, v)| *v < 0.0));
        }
    }
    // 平面は 0
    let o = draw("z=2x+3y", GAUSS);
    assert!(o.verts.iter().all(|(_, v)| v.abs() < 1e-6));
}

/// 媒介変数の曲面: 球 (2 sin u cos v, 2 sin u sin v, 2 cos u) は K = 1/4。陽関数と同じ形 (u, v, u² + v²) は陽関数と同じ値
#[test]
fn parametric_surfaces() {
    let o = draw_with("(2sin(u)cos(v),2sin(u)sin(v),2cos(u))", ("u,v", &[0.0, std::f64::consts::PI, 0.0, std::f64::consts::TAU]), 5.0, GAUSS, &[0.0; 10]);
    assert!(o.available && o.verts.len() > 100);
    // 極（sin u = 0）では r_u × r_v = 0 で求められない（0 = 中央の色）。それ以外は 1/4
    let off = o.verts.iter().filter(|(p, _)| p[0].hypot(p[1]) > 0.1).map(|(_, v)| (v - 0.25).abs()).fold(0.0, f32::max);
    assert!(off < 1e-4, "{off}");
    let o = draw_with("(u,v,u^2+v^2)", ("u,v", &[-1.0, 1.0, -1.0, 1.0]), 5.0, MEAN, &[0.0; 10]);
    let worst = o
        .verts
        .iter()
        .map(|(p, v)| {
            let (fu, fv) = (2.0 * p[0], 2.0 * p[1]);
            let w = 1.0 + fu * fu + fv * fv;
            let want = ((1.0 + fv * fv) * 2.0 + (1.0 + fu * fu) * 2.0) / (2.0 * w * w.sqrt());
            (v - want).abs()
        })
        .fold(0.0, f32::max);
    assert!(worst < 1e-4, "{worst}");
}

/// 回した陽関数（射影する前の点で求める。回転で曲率は変わらない）
#[test]
fn rotated_explicit_keeps_curvature() {
    let o = draw_with("z=x^2+y^2", ("", &[]), 5.0, GAUSS, &[0.0, 0.4, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    assert!(o.available, "{}", o.kind);
    let vmax = o.verts.iter().map(|(_, v)| *v).fold(f32::MIN, f32::max);
    // K = 4 / (1 + 4r²)² ≤ 4（原点の真上の頂点は無いので、最大は 4 より少し小さい）
    assert!(vmax <= 4.0 + 1e-3 && vmax > 3.5, "{vmax}");
    assert!(o.verts.iter().all(|(_, v)| *v > 0.0));
}

/// 色分けしないとき・使えない式
#[test]
fn off_and_unsupported() {
    // 頼まなければ値は付けない（使えることだけ返す）
    let o = draw("x^2+y^2+z^2=4", 0);
    assert!(o.available && !o.diverging && o.range.is_empty());
    // 曲線・連立方程式は使えない（説明に出す）
    let o = draw("(cos(t),sin(t),t/5)", GAUSS);
    assert!(!o.available && !o.diverging && o.kind.contains("曲率の色分けは、この式では使えません"), "{}", o.kind);
    let o = draw("x^2+y^2+z^2=4, z>0", GAUSS);
    assert!(!o.available && !o.diverging);
}

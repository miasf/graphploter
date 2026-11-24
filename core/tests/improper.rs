//! 広義積分・特異点のある積分（計画 `docs/plan-stage2b-integrals.md` の 4 章。#109 #110 #111 #112 #113 #159）。
//! 式は利用者が打つ LaTeX を `web/src/lib/latex.ts` で変換したもの。値は手で計算した厳密な値

use graph_core::equation::{compile_with, eval_constant, Plot, Space};
use std::f64::consts::{E, PI};

fn close(src: &str, want: f64, rel: f64) {
    match eval_constant(src) {
        Ok(v) => assert!((v - want).abs() <= rel * want.abs().max(1.0), "{src}: {v} （正しくは {want}）"),
        Err(e) => panic!("{src}: {e}（正しくは {want}）"),
    }
}

/// 値を出さずに、理由を出す（発散する・収束を確かめられない）。理由の文に `why` が入っていること
fn refused(src: &str, why: &str) {
    match eval_constant(src) {
        Ok(v) => panic!("{src}: {v} を返した（{why}と出すはず）"),
        Err(e) => assert!(e.contains(why), "{src}: 「{e}」（{why}と出すはず）"),
    }
}

const DIVERGES: &str = "発散";

/// 4.1 発散するものは、値を出さない（#109 #111）
#[test]
fn divergent_integrals_are_refused() {
    refused("@int(t,0,1)(((1)/(t)))", DIVERGES);
    refused("@int(t,-1,1)(((1)/(t)))", DIVERGES);
    refused("@int(t,-1,1)(((1)/(t^(2))))", DIVERGES);
    refused("@int(t,0,2)(tan t)", DIVERGES);
    refused("@int(t,0,1)(((1)/(t-0.3)))", DIVERGES);
    refused("@int(t,0,1)(((1)/(t^(1.5))))", DIVERGES);
    // ∞ までの積分
    refused("@int(t,0,∞)(sin (t))", "収束しません");
    refused("@int(t,0,∞)(cos (t))", "収束しません");
    refused("@int(t,0,∞)(t sin t)", "収束しません");
    // 今まで弾いていたものも弾いたまま
    refused("@int(t,1,∞)(((1)/(t)))", "収束しない");
    refused("@int(t,0,∞)(((1)/(1+t)))", "収束しない");
    refused("@int(t,0,∞)(1)", "収束しない");
}

/// 4.1 収束が遅すぎて確かめられないものは、そう出して値を出さない（決めたこと 2）
#[test]
fn too_slow_integrals_say_so() {
    refused("@int(t,0,1)(((1)/(t^(0.97))))", "確かめられません");
    refused("@int(t,0,0.5)(((1)/(t (ln t)^(2))))", "確かめられません");
    // 振動の小さくなり方が遅い（収束はするが、零点 48 個では確かめられない）
    refused("@int(t,1,∞)(((sin t)/(t^(0.1))))", "確かめられません");
}

/// 4.2 端で発散するが積分できるもの（#109 の対照）
#[test]
fn integrable_end_singularities() {
    close("@int(t,0,1)(((1)/(sqrt(t))))", 2.0, 1e-8);
    close("@int(t,-1,1)(((1)/(sqrt(abs(t)))))", 4.0, 1e-8);
    close("@int(t,0,1)(((1)/(t^(0.9))))", 10.0, 1e-8);
    close("@int(t,0,1)(ln t)", -1.0, 1e-8);
    close("@int(t,0,1)(((ln t)^(2)))", 2.0, 1e-8);
    // sin 1 − Ci(1)
    close("@int(t,0,1)(sin (((1)/(t))))", 0.504_067_061_906_928_4, 1e-8);
    // 1/√ の形で発散する端が両方
    close("@int(t,-1,1)(((1)/(sqrt(1-t^(2)))))", PI, 1e-8);
}

/// 4.2 区間の中の特異点（#110）。どこに置いても、そこで分けた計算と同じ
#[test]
fn interior_singularities() {
    for c in [0.25f64, 0.5, 0.3183, 0.123_456_789] {
        let src = format!("@int(t,0,1)(((1)/(sqrt(abs(t-{c})))))");
        close(&src, 2.0 * (c.sqrt() + (1.0 - c).sqrt()), 1e-8);
    }
    // 区間の外の特異点
    close("@int(t,0,1)(((1)/(sqrt(abs(t-2)))))", 2.0 * (2f64.sqrt() - 1.0), 1e-8);
}

/// 4.2 場合分けの細いパルス（#159）
#[test]
fn narrow_pulses() {
    for left in [0.123456, 0.234567, 0.435678, 0.765432, 0.891234] {
        for w in [0.01, 1e-4, 1e-6] {
            let right = format!("{:.9}", left + w);
            let src = format!("@int(t,0,1)(@cases(1, {left}<t<{right}; 0))");
            close(&src, w, 1e-6);
        }
    }
    close("@int(t,0,1)(@cases(1, t>=0.99999; 0))", 1e-5, 1e-6);
}

/// 4.2 振動する ∞ までの積分（#111 の対照・#112）
#[test]
fn oscillatory_infinite_integrals() {
    close("@int(t,0,∞)(((sin (t))/(t)))", PI / 2.0, 1e-8);
    close("@int(t,0,∞)(sinc(t))", PI / 2.0, 1e-8);
    close("@int(t,0,∞)(@cases(1, t=0; ((sin (t))/(t))))", PI / 2.0, 1e-8);
    close("@int(t,0,∞)(((sin t)/(sqrt(t))))", (PI / 2.0).sqrt(), 1e-8);
    close("@int(t,0,∞)(e^(-t) sin (t))", 0.5, 1e-8);
    close("@int(t,0,∞)(sin (t^(2)))", (PI / 8.0).sqrt(), 1e-8);
    close("@int(t,1,∞)(((sin (t))/(t)))", 0.624_713_256_427_713_6, 1e-8);
    close("@int(t,0,∞)(((cos t)/(1+t^(2))))", PI / (2.0 * E), 1e-8);
}

/// 4.2 ふつうの積分は今までどおり
#[test]
fn ordinary_integrals() {
    close("@int(t,0,1)(t^(2))", 1.0 / 3.0, 1e-12);
    close("@int(t,0,100)(sin (t))", 1.0 - 100f64.cos(), 1e-10);
    close("@int(t,0,1)(abs(t-0.3))", 0.29, 1e-12);
    close("@int(t,0,∞)(e^(-t^(2)))", PI.sqrt() / 2.0, 1e-10);
    close("@int(t,1,0)(t)", -0.5, 1e-12);
    close("@int(t,0,100)(sin (t^(2)))", 0.631_417_921_867_157_2, 1e-8);
    close("@int(t,0,3)(floor(t))", 3.0, 1e-10);
}

/// 4.2 変数上限の積分の曲線（#113）: 描いた点は正しく、誤差の大きい点は描かない（NaN）
#[test]
fn variable_upper_limit() {
    let sp = Space::standard();
    let ps = Default::default();
    let at = |src: &str, xs: &[f64]| -> Vec<f64> {
        let Ok(Plot::Explicit { prog, .. }) = compile_with(src, &ps, &sp) else { panic!("{src} が陽関数にならない") };
        let ys = vec![0.0; xs.len()];
        let mut out = vec![num_complex::Complex64::new(0.0, 0.0); xs.len()];
        prog.eval_batch(&[xs, &ys], &mut out);
        out.iter().map(|c| if c.im == 0.0 { c.re } else { f64::NAN }).collect()
    };
    let xs = [0.5, 1.0, 3.0, 10.0, 50.0, 100.0];
    for (x, v) in xs.iter().zip(at("z= @int(t,0,x)(t^(2))", &xs)) {
        assert!((v - x * x * x / 3.0).abs() <= 1e-9 * (x * x * x), "∫_0^{x} t² dt = {v}");
    }
    for (x, v) in xs.iter().zip(at("z= @int(t,0,x)(((1)/(sqrt(t))))", &xs)) {
        assert!(v.is_nan() || (v - 2.0 * x.sqrt()).abs() <= 1e-6 * x.sqrt(), "∫_0^{x} 1/√t dt = {v}");
    }
    let near = |x: f64| eval_constant(&format!("@int(t,0,{x})(sin (t^(2)))")).unwrap();
    let xs: Vec<f64> = (1..=200).map(|k| k as f64 * 0.5).collect();
    let vs = at("z= @int(t,0,x)(sin (t^(2)))", &xs);
    let mut drawn = 0;
    for (x, v) in xs.iter().zip(&vs) {
        if v.is_nan() {
            continue;
        }
        drawn += 1;
        assert!((v - near(*x)).abs() <= 1e-3, "∫_0^{x} sin t² dt = {v}（正しくは {}）", near(*x));
    }
    assert!(drawn >= 8, "x の小さい所は描く（{drawn} 点）");
    // 下の端で発散する積分は、曲線にしない
    assert!(compile_with("z= @int(t,0,x)(((1)/(t)))", &ps, &sp).is_err());
}

//! 大学の微積分の問題集にある領域での重積分（物差し）。式は利用者が打つ LaTeX を `web/src/lib/latex.ts` で変換したもの。
//! 値は手で計算した厳密な値。`cargo test --release --test textbook_regions -- --nocapture` で表を出す。
//! 目標は相対誤差 1e-6。届かない例の数が [`KNOWN_MISSES`] を超えたら落ちる

use graph_core::equation::eval_constant;
use std::f64::consts::PI;
use std::time::Instant;

/// (名前, 計算エンジン用の式, 厳密な値)
fn cases() -> Vec<(&'static str, &'static str, f64)> {
    let s2 = 2f64.sqrt();
    let s3 = 3f64.sqrt();
    vec![
        ("四分円", "@intset((x,y),x^(2)+y^(2)<=1,x>=0,y>=0)(xy)", 1.0 / 8.0),
        ("ずれた円", "@intset((x,y),x^(2)+y^(2)<=2x)(x)", PI),
        ("半円環", "@intset((x,y),1<=x^(2)+y^(2)<=4,y>=0)(y)", 14.0 / 3.0),
        ("ひし形", "@intset((x,y),abs(x)+ abs(y)<=1)(x^(2))", 1.0 / 3.0),
        ("放物線と√", "@intset((x,y),0<=x<=1,x^(2)<=y<= sqrt(x))(1)", 1.0 / 3.0),
        ("横向き放物線", "@intset((x,y),y^(2)<=x<=y+2)(1)", 4.5),
        ("楕円", "@intset((x,y),((x^(2))/(4))+((y^(2))/(9))<=1)(1)", 6.0 * PI),
        ("レムニスケート", "@intset((x,y),(x^(2)+y^(2))^(2)<=2(x^(2)-y^(2)))(1)", 2.0),
        ("原点で不連続な被積分関数", "@intset((x,y),x>=0,y>=0,x+y<=1)(e^(((y-x)/(y+x))))", 1f64.sinh() / 2.0),
        ("半球の体積（端で√）", "@intset((x,y),x^(2)+y^(2)<=1)(sqrt(1-x^(2)-y^(2)))", 2.0 * PI / 3.0),
        ("原点で発散（広義積分）", "@intset((x,y),x^(2)+y^(2)<=1)(((1)/(sqrt(x^(2)+y^(2)))))", 2.0 * PI),
        ("順序の交換", "@intset((x,y),0<=y<=x<= pi)(((sin x)/(x)))", 2.0),
        ("x²+y²≤x", "@intset((x,y),x^(2)+y^(2)<=x)(sqrt(x^(2)+y^(2)))", 4.0 / 9.0),
        ("円の半分（斜め）", "@intset((x,y),x^(2)+y^(2)<=1,y>=x)(1)", PI / 2.0),
        ("扇形の環", "@intset((x,y),1<=x^(2)+y^(2)<=4,0<=y<=x)(1)", 3.0 * PI / 8.0),
        ("和集合（2 つの円）", "@intset((x,y),x^(2)+y^(2)<=1∨(x-1)^(2)+y^(2)<=1)(1)", 4.0 * PI / 3.0 + s3 / 2.0),
        ("差集合（否定）", "@intset((x,y),x^(2)+y^(2)<=4,¬(x^(2)+y^(2)<1))(1)", 3.0 * PI),
        ("無限の領域 e^{-x-y}", "@intset((x,y),x>=0,y>=0)(e^(-x-y))", 1.0),
        ("無限の領域（ガウス）", "@intset((x,y),x>=0,y>=0)(e^(-x^(2)-y^(2)))", PI / 4.0),
        ("角で発散 1/(1-xy)", "@intset((x,y),0<=x<=1,0<=y<=1)(((1)/(1-xy)))", PI * PI / 6.0),
        ("sin の下（0〜π）", "@intset((x,y),0<=y<= sin x,0<=x<= pi)(1)", 2.0),
        ("sin の下（0〜2π。空の所あり）", "@intset((x,y),0<=y<= sin x,0<=x<=2 pi)(1)", 2.0),
        ("x²≤y≤1（x の範囲なし）", "@intset((x,y),x^(2)<=y<=1)(x^(2))", 4.0 / 15.0),
        ("2 本の放物線", "@intset((x,y),x^(2)<=y<=2-x^(2))(1)", 8.0 / 3.0),
        ("双曲線の下", "@intset((x,y),xy<=1,0<=x<=3,0<=y<=3)(1)", 1.0 + 9f64.ln()),
        ("大きい円から小さい円を抜く", "@intset((x,y),x^(2)+y^(2)<=4,x^(2)+y^(2)>=2x)(1)", 3.0 * PI),
        ("円と放物線", "@intset((x,y),x^(2)+y^(2)<=2,y>=x^(2))(1)", PI / 2.0 + 1.0 / 3.0),
        ("心臓形（極座標の点で書く）", "@intset((r cos theta ,r sin theta ),0<=r<=1+ cos theta ,0<= theta <=2 pi)(1)", 3.0 * PI / 2.0),
        ("球", "@intset((x,y,z),x^(2)+y^(2)+z^(2)<=1)(z^(2))", 4.0 * PI / 15.0),
        ("四面体", "@intset((x,y,z),x>=0,y>=0,z>=0,x+y+z<=1)(x)", 1.0 / 24.0),
        ("放物面の中", "@intset((x,y,z),x^(2)+y^(2)<=z<=1)(z)", PI / 3.0),
        ("円柱と円錐", "@intset((x,y,z),x^(2)+y^(2)<=2x,0<=z<= sqrt(x^(2)+y^(2)))(1)", 32.0 / 9.0),
        ("ヴィヴィアーニの体", "@intset((x,y,z),x^(2)+y^(2)+z^(2)<=4,x^(2)+y^(2)<=2x)(1)", 16.0 / 9.0 * (3.0 * PI - 4.0)),
        ("円柱と x³+y³（画面の例）", "@intset((x,y,z),x^(2)+y^(2)<=1,0<=z<=x^(3)+y^(3))(1)", s2 / 3.0),
        ("2 本の円柱（シュタインメッツ）", "@intset((x,y,z),x^(2)+z^(2)<=1,y^(2)+z^(2)<=1)(1)", 16.0 / 3.0),
        ("アイスクリーム", "@intset((x,y,z),x^(2)+y^(2)+z^(2)<=1,z>= sqrt(x^(2)+y^(2)))(1)", 2.0 * PI / 3.0 * (1.0 - 1.0 / s2)),
        ("ナプキンリング", "@intset((x,y,z),x^(2)+y^(2)+z^(2)<=4,x^(2)+y^(2)>=1)(1)", 4.0 * s3 * PI),
        ("√x+√y+√z≤1", "@intset((x,y,z),sqrt(x)+ sqrt(y)+ sqrt(z)<=1)(1)", 1.0 / 90.0),
        ("八面体", "@intset((x,y,z),abs(x)+ abs(y)+ abs(z)<=1)(x^(2))", 2.0 / 15.0),
        ("無限の領域（3 重）", "@intset((x,y,z),x>=0,y>=0,z>=0)(e^(-x-y-z))", 1.0),
    ]
}

/// 目標（相対誤差 1e-6）に届かない例の数の上限。直して減ったら、この数も減らす
const KNOWN_MISSES: usize = 0;

#[test]
fn textbook_regions() {
    let mut misses = 0;
    println!("| 領域 | 値 | 厳密な値 | 相対誤差 | 時間 (ms) | |");
    println!("|---|---|---|---|---|---|");
    for (name, src, want) in cases() {
        let t = Instant::now();
        let got = eval_constant(src);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let (shown, rel) = match &got {
            Ok(v) => (format!("{v:.12}"), (v - want).abs() / want.abs().max(1e-300)),
            Err(e) => (format!("誤り: {e}"), f64::INFINITY),
        };
        let miss = !(rel <= 1e-6);
        misses += miss as usize;
        println!("| {name} | {shown} | {want:.12} | {rel:.1e} | {ms:.0} | {} |", if miss { "✗" } else { "" });
    }
    println!("目標に届かない: {misses}");
    assert!(misses <= KNOWN_MISSES, "目標に届かない例が増えた（{misses} 個）");
}

/// 1 重の定積分（値が定数になるもの）: 端で発散する・∞ まで・長い区間で振動する・発散する
#[test]
fn single_integrals() {
    let close = |src: &str, want: f64| {
        let v = eval_constant(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        assert!((v - want).abs() <= 1e-9 * want.abs().max(1.0), "{src}: {v} (want {want})");
    };
    close("@int(t,0,1)(1/sqrt(t))", 2.0);
    close("@int(t,0,1)(ln(t))", -1.0);
    close("@int(t,0,1)(sqrt(t))", 2.0 / 3.0);
    close("@int(t,0,100)(sin(t))", 1.0 - 100f64.cos());
    close("@int(t,0,∞)(exp(-t))", 1.0);
    close("@int(t,-∞,∞)(exp(-t^2))", PI.sqrt());
    close("@int(t,0,1)(abs(t-0.3))", 0.29);
    close("@int(x,0,1)(@int(y,0,x)(xy))", 0.125);
    // 発散する積分は数を返さない
    assert!(eval_constant("@int(t,0,∞)(1)").is_err());
}

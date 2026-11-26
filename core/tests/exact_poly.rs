//! 多項式（計画 `docs/plan-exact-core.md` の EX6 の完了の条件）

use graph_core::exact::num::{Int, Rat};
use graph_core::exact::poly::{apart, factor, gcd, squarefree, Poly};
use graph_core::exact::Budget;

fn p(c: &[i64]) -> Poly {
    Poly::new(c.iter().map(|&k| Rat::from(Int::from(k))).collect())
}

fn b() -> Budget {
    Budget::new(50_000_000)
}

fn degs(f: &graph_core::exact::poly::Factored) -> Vec<(i64, u32)> {
    f.factors.iter().map(|(g, k)| (g.deg(), *k)).collect()
}

#[test]
fn factoring() {
    let f = factor(&p(&[-1, 0, 0, 0, 1]), &b());
    assert!(f.complete);
    assert_eq!(f.factors, vec![(p(&[-1, 1]), 1), (p(&[1, 1]), 1), (p(&[1, 0, 1]), 1)]);
    // x^12 − 1 は 6 個の円分多項式
    let mut c = vec![0i64; 13];
    c[0] = -1;
    c[12] = 1;
    let f = factor(&p(&c), &b());
    assert!(f.complete);
    assert_eq!(degs(&f), vec![(1, 1), (1, 1), (2, 1), (2, 1), (2, 1), (4, 1)]);
    // x^6 + x^3 + 1 は既約
    let f = factor(&p(&[1, 0, 0, 1, 0, 0, 1]), &b());
    assert!(f.complete && f.factors.len() == 1, "{f:?}");
    // 重なり: (x − 1)^3 (x + 2)^2
    let g = p(&[-1, 1]).pow(3).mul(&p(&[2, 1]).pow(2));
    let f = factor(&g, &b());
    assert_eq!(f.factors, vec![(p(&[-1, 1]), 3), (p(&[2, 1]), 2)]);
    assert_eq!(squarefree(&g).len(), 2);
    assert_eq!(gcd(&p(&[-1, 0, 1]), &p(&[1, 2, 1])), p(&[1, 1]));
    // 有理数の係数: 2x² − 1/2 = 2 (x − 1/2)(x + 1/2) → 原始の形 (2x − 1)(2x + 1) と定数 1/2
    let f = factor(&Poly::new(vec![Rat::from_parts(Int::from(-1), 2u8.into()), Rat::ZERO, Rat::from(Int::from(2))]), &b());
    assert_eq!(f.factors, vec![(p(&[-1, 2]), 1), (p(&[1, 2]), 1)]);
    assert_eq!(f.unit, Rat::from_parts(Int::ONE, 2u8.into()));
}

/// Swinnerton-Dyer の多項式 S₃（根 ±√2 ± √3 ± √5。8 次で既約）
#[test]
#[cfg_attr(debug_assertions, ignore)]
fn swinnerton_dyer() {
    let f = factor(&p(&[576, 0, -960, 0, 352, 0, -40, 0, 1]), &b());
    assert!(f.complete && f.factors.len() == 1, "{f:?}");
}

/// 乱数の多項式の積を分解して、掛け直すと元に戻る
#[test]
fn random_products() {
    let mut s: u64 = 7;
    let mut next = |m: i64| {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((s >> 33) as i64 % (2 * m + 1)) - m
    };
    for _ in 0..200 {
        let a = p(&[next(5), next(5), 1 + next(2).abs()]);
        let c = p(&[next(5), 1 + next(3).abs()]);
        let g = a.mul(&c);
        let f = factor(&g, &b());
        let mut back = Poly::new(vec![f.unit.clone()]);
        for (h, k) in &f.factors {
            back = back.mul(&h.pow(*k));
        }
        assert_eq!(back, g, "{g:?} → {f:?}");
    }
}

#[test]
fn partial_fractions() {
    // x / (x² − 3x + 2) = 2/(x − 2) − 1/(x − 1)
    let (q, parts) = apart(&p(&[0, 1]), &p(&[2, -3, 1]), &b()).unwrap();
    assert!(q.is_zero());
    let mut got: Vec<(Poly, Poly, u32)> = parts;
    got.sort_by(|a, b| a.1.cmp(&b.1));
    assert_eq!(got, vec![(p(&[2]), p(&[-2, 1]), 1), (p(&[-1]), p(&[-1, 1]), 1)]);
    // 1/(x³ + 1) = 1/(3(x + 1)) + (−x + 2)/(3(x² − x + 1))
    let (_, parts) = apart(&p(&[1]), &p(&[1, 0, 0, 1]), &b()).unwrap();
    let third = |n: i64| Rat::from_parts(Int::from(n), 3u8.into());
    assert!(parts.contains(&(Poly::new(vec![third(1)]), p(&[1, 1]), 1)), "{parts:?}");
    assert!(parts.contains(&(Poly::new(vec![third(2), third(-1)]), p(&[1, -1, 1]), 1)), "{parts:?}");
    // 重なり: 1/(x² (x + 1)) = 1/x² − 1/x + 1/(x + 1)
    let (_, parts) = apart(&p(&[1]), &p(&[0, 0, 1, 1]), &b()).unwrap();
    assert_eq!(parts.len(), 3, "{parts:?}");
}

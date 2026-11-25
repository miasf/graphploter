//! 厳密値の道の数（計画 `docs/plan-exact-core.md` の EX1 の完了の条件）

use graph_core::exact::num::*;
use graph_core::exact::Budget;

fn int(s: &str) -> Int {
    s.parse().unwrap()
}

fn b() -> Budget {
    Budget::unlimited()
}

#[test]
fn factorial_and_binomial_are_exact() {
    assert_eq!(factorial(25, &b()).unwrap(), int("15511210043330985984000000"));
    assert_eq!(factorial(0, &b()).unwrap(), Int::ONE);
    assert_eq!(factorial(100, &b()).unwrap().to_string().len(), 158);
    assert_eq!(binomial(&int("60"), &int("30"), &b()).unwrap(), int("118264581564861424"));
    assert_eq!(binomial(&int("100"), &int("50"), &b()).unwrap(), int("100891344545564193334812497256"));
    // 今の f64 の道と同じ約束: C(−3, 2) = 6、C(5, 7) = 0
    assert_eq!(binomial(&int("-3"), &int("2"), &b()).unwrap(), int("6"));
    assert_eq!(binomial(&int("5"), &int("7"), &b()).unwrap(), Int::ZERO);
    assert_eq!(binomial(&int("5"), &int("-1"), &b()).unwrap(), Int::ZERO);
    assert_eq!(multinomial(&[2, 3, 1], &b()).unwrap(), int("60"));
    assert_eq!(stirling2(5, 2, &b()).unwrap(), int("15"));
    assert_eq!(stirling2(10, 3, &b()).unwrap(), int("9330"));
}

#[test]
fn gcd_inverse_crt() {
    assert_eq!(gcd(&(int("2").pow(60) + Int::ONE), &int("3").pow(40)), Int::ONE);
    assert_eq!(gcd(&int("462"), &int("1071")), int("21"));
    assert_eq!(lcm(&int("12"), &int("18")), int("36"));
    assert_eq!(mod_inverse(&int("3"), &int("7")), Some(int("5")));
    assert_eq!(mod_inverse(&int("6"), &int("9")), None);
    assert_eq!(crt(&[(int("2"), int("3")), (int("3"), int("5"))]), Some((int("8"), int("15"))));
    assert_eq!(crt(&[(int("1"), int("4")), (int("2"), int("6"))]), None);
    assert_eq!(crt(&[(int("1"), int("4")), (int("3"), int("6"))]), Some((int("9"), int("12"))));
    assert_eq!(mod_pow(&int("2"), &"100".parse().unwrap(), &int("13")), int("3"));
    assert_eq!(mod_pow(&int("-3"), &"3".parse().unwrap(), &int("7")), int("1"));
}

/// 乱数（線形合同法。テストを毎回同じにする）の 1 万組で、gcd と逆元が定義どおり
#[test]
fn random_gcd_and_inverse() {
    let mut s: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        s >> 1
    };
    for _ in 0..10_000 {
        let a = Int::from(next() % 1_000_000_007) * Int::from(next() % 1000 + 1);
        let m = Int::from(next() % 1_000_000 + 2);
        let g = gcd(&a, &m);
        assert!((&a % &g).is_zero() && (&m % &g).is_zero());
        match mod_inverse(&a, &m) {
            Some(x) => {
                assert_eq!(g, Int::ONE);
                assert_eq!(modulo(&(&a * &x), &m), Int::ONE);
            }
            None => assert!(g != Int::ONE),
        }
    }
}

#[test]
fn primes_and_factoring() {
    assert!(is_prime(&(int("2").pow(521) - Int::ONE)));
    assert!(!is_prime(&(int("2").pow(523) - Int::ONE)));
    assert!(is_prime(&int("1000000007")));
    assert!(!is_prime(&int("561"))); // カーマイケル数
    assert!(!is_prime(&int("3317044064679887385961981"))); // 13 個の底の判定の境目（合成数）
    assert!(is_prime(&int("170141183460469231731687303715884105727"))); // 2^127 − 1（BPSW の道）
    let (fs, rest) = factor_int(&(int("2").pow(32) + Int::ONE), &b()).unwrap();
    assert_eq!(fs, vec![(int("641"), 1), (int("6700417"), 1)]);
    assert!(rest.is_none());
    let (fs, _) = factor_int(&int("360"), &b()).unwrap();
    assert_eq!(fs, vec![(int("2"), 3), (int("3"), 2), (int("5"), 1)]);
    let (fs, _) = factor_int(&(int("1000003") * int("1000033") * int("1000037")), &b()).unwrap();
    assert_eq!(fs, vec![(int("1000003"), 1), (int("1000033"), 1), (int("1000037"), 1)]);
    let (fs, _) = factor_int(&int("1000003").pow(3), &b()).unwrap();
    assert_eq!(fs, vec![(int("1000003"), 3)]);
    assert_eq!(totient(&int("36"), &b()).unwrap(), int("12"));
    assert_eq!(totient(&int("1"), &b()).unwrap(), Int::ONE);
    // 予算が足りなければ時間切れ
    assert!(factor_int(&(int("1000000007") * int("998244353")), &Budget::new(50)).is_err());
}

#[test]
fn bernoulli_numbers() {
    let r = |n: i64, d: u64| Rat::from_parts(Int::from(n), Nat::from(d));
    assert_eq!(bernoulli(0), r(1, 1));
    assert_eq!(bernoulli(1), r(-1, 2));
    assert_eq!(bernoulli(2), r(1, 6));
    assert_eq!(bernoulli(3), Rat::ZERO);
    assert_eq!(bernoulli(12), r(-691, 2730));
    assert_eq!(bernoulli(20), r(-174611, 330));
}

#[test]
fn literals_become_short_decimals() {
    let r = |n: i64, d: u64| Rat::from_parts(Int::from(n), Nat::from(d));
    assert_eq!(rat_of_literal(0.1), Some(r(1, 10)));
    assert_eq!(rat_of_literal(0.3), Some(r(3, 10)));
    assert_eq!(rat_of_literal(9.81), Some(r(981, 100)));
    assert_eq!(rat_of_literal(-2.5), Some(r(-5, 2)));
    assert_eq!(rat_of_literal(3.0), Some(r(3, 1)));
    assert_eq!(rat_of_literal(1e300).map(|q| as_int(&q).unwrap().to_string().len()), Some(301));
    assert_eq!(rat_of_literal(f64::NAN), None);
    assert_eq!(exact_root(&int("-27"), 3), Some(int("-3")));
    assert_eq!(exact_root(&int("26"), 3), None);
}

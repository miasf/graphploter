//! 高い精度の値と 0 の判定（計画 `docs/plan-exact-core.md` の EX3 の完了の条件）。
//! 参照値は mpmath 1.3.0 で 310 桁で計算し、300 桁に丸めたもの

use graph_core::exact::ball::{self, Ball, F};
use graph_core::exact::num::{Int, Rat};
use graph_core::exact::term::{add, call, mul, pow, Atom, Term};
use graph_core::functions::Func;
use std::str::FromStr;

const PI: &str = "3.14159265358979323846264338327950288419716939937510582097494459230781640628620899862803482534211706798214808651328230664709384460955058223172535940812848111745028410270193852110555964462294895493038196442881097566593344612847564823378678316527120190914564856692346034861045432664821339360726024914127";
const E: &str = "2.71828182845904523536028747135266249775724709369995957496696762772407663035354759457138217852516642742746639193200305992181741359662904357290033429526059563073813232862794349076323382988075319525101901157383418793070215408914993488416750924476146066808226480016847741185374234544243710753907774499207";
const EULER: &str = "0.577215664901532860606512090082402431042159335939923598805767234884867726777664670936947063291746749514631447249807082480960504014486542836224173997644923536253500333742937337737673942792595258247094916008735203948165670853233151776611528621199501507984793745085705740029921354786146694029604325421519";
const CATALAN: &str = "0.915965594177219015054603514932384110774149374281672134266498119621763019776254769479356512926115106248574422619196199579035898803325859059431594737481158406995332028773319460519038727478164087865909024706484152163000228727640942388259957741508816397470252482011560707644883807873370489900864775113226";
const ZETA3: &str = "1.20205690315959428539973816151144999076498629234049888179227155534183820578631309018645587360933525814619915779526071941849199599867328321377639683720790016145394178294936006671919157552224249424396156390966410329115909578096551465127991840510571525598801543710978110203982753256678760352233698494166";
const GAMMA_THIRD: &str = "2.67893853470774763365569294097467764412868937795730110095042832759041761016774381954098288904118878941915904920007226333571908456950447225997771336770846976816728982305000321834255032224715694181755544995272878439477944130576582840161231914159646652603372758402058063551394324103201583941538270085524";
const DIGAMMA_QUARTER: &str = "-4.22745353337626540808953014609668357736724443870824227165527955951895679582985331706855445695206134613170993356816029300556398702653622562318486595661439198077082509757902090386137488097574475612565594856923418564684975534298902061830111567040930488113526687990645720031424844607256610830667930612338";
const ZETA5: &str = "1.03692775514336992633136548645703416805708091950191281197419267790380358978628148456004310655713333637962034146655660904280096177915597084183511072180087644866286337180353598363962365128888981335276775239827503202243684576644466595811599391797774503924464391966661596640162053252050215192267135125679";
const GAMMA_7_2: &str = "3.32335097044784255118406403126464721774540523022947586540088960597420865860818534007803248138477124765542529429552981999919278662323329237973455070550522575336632068497457210534360969942928279917924767883612743025032744815378402447596881285659731378484771307717796120058860332911107415576016124843920";
const LN2: &str = "0.693147180559945309417232121458176568075500134360255254120680009493393621969694715605863326996418687542001481020570685733685520235758130557032670751635075961930727570828371435190307038623891673471123350115364497955239120475172681574932065155524734139525882950453007095326366642654104239157814952043740";

/// 参照値の文字列を 2 進の数にする
fn reference(s: &str, p: usize) -> F {
    let d = dashu::float::DBig::from_str(s).unwrap();
    d.with_base_and_precision::<2>(p).value().with_rounding()
}

/// 300 桁（約 1000 ビット）で計算し、参照値との差が 10^{-290}（相対）未満で、半径が差を含む
fn check(t: &Term, refs: &str) {
    let b: Ball = ball::value(t, 1000).unwrap_or_else(|e| panic!("{t:?}: {e:?}"));
    let r = reference(refs, 1100);
    let diff = (&b.re - &r).clone();
    let dm = ball::mag_of(&diff);
    let vm = ball::mag_of(&r);
    assert!(dm.log2() - vm.log2() < -963.0, "{t:?}: 相対の差 2^{}", dm.log2() - vm.log2());
    // 参照値そのものの丸め（300 桁 ≈ 2^{−996}）も足して比べる
    let allowed = b.rad.add(vm.mul(ball::Mag::pow2(-990)));
    assert!(!allowed.lt(&dm), "{t:?}: 半径 2^{} が差 2^{} を含まない", b.rad.log2(), dm.log2());
    // 10 進で 290 桁まで一致
    let got = ball::to_decimal(&b.re, 295);
    assert_eq!(&got[..285], &refs[..285], "{t:?}");
}

fn q(n: i64, d: u64) -> Term {
    Term::rat(n, d)
}

#[test]
fn constants_to_300_digits() {
    check(&Term::Atom(Atom::Pi), PI);
    check(&Term::Atom(Atom::E), E);
    check(&Term::Atom(Atom::EulerGamma), EULER);
    check(&Term::Atom(Atom::Catalan), CATALAN);
    check(&call(Func::Zeta, vec![Term::int(3)]), ZETA3);
    check(&call(Func::Zeta, vec![Term::int(5)]), ZETA5);
    check(&call(Func::Gamma, vec![q(1, 3)]), GAMMA_THIRD);
    check(&call(Func::Gamma, vec![q(7, 2)]), GAMMA_7_2);
    check(&call(Func::Digamma, vec![q(1, 4)]), DIGAMMA_QUARTER);
    check(&call(Func::Ln, vec![Term::int(2)]), LN2);
}

#[test]
fn zero_test() {
    let mut rules = Vec::new();
    // sin π は式の形では 0 にならない（特別な値の表は SC0）が、数値で 0 とみなす
    let s = call(Func::Sin, vec![Term::Atom(Atom::Pi)]);
    assert_eq!(ball::is_zero(&s, &mut rules), Some(true));
    assert_eq!(rules, vec!["数値で 0 とみなした"]);
    // e^{π√163} − 262537412640768744 ≈ −7.5 × 10^{−13} は 0 でない
    let big: Int = "262537412640768744".parse().unwrap();
    let ramanujan = add(vec![pow(Term::Atom(Atom::E), mul(vec![Term::Atom(Atom::Pi), pow(Term::int(163), q(1, 2))])), Term::Num(-Rat::from(big))]);
    let mut rules = Vec::new();
    assert_eq!(ball::is_zero(&ramanujan, &mut rules), Some(false));
    let v = ball::value(&ramanujan, 64).unwrap().to_f64().0;
    assert!((v + 7.499274e-13).abs() < 1e-18, "{v}");
    // sin(10^22)（f64 の引数の簡約が難しい例）
    let s = call(Func::Sin, vec![pow(Term::int(10), Term::int(22))]);
    let v = ball::value(&s, 64).unwrap().to_f64().0;
    assert!((v - (-0.8522008497671888)).abs() < 1e-15, "{v}");
    // 文字を含む式は値が決まらない
    assert!(ball::value(&Term::Sym("x".into()), 64).is_err());
}

#[test]
fn complex_values() {
    // ln(−1) は Term の形で πi。e^{iπ/5}（表に無い角）は数値で
    let z = pow(Term::Atom(Atom::E), mul(vec![q(1, 5), Term::Atom(Atom::Pi), Term::Atom(Atom::I)]));
    let (re, im) = ball::value(&z, 64).unwrap().to_f64();
    assert!((re - (std::f64::consts::PI / 5.0).cos()).abs() < 1e-15);
    assert!((im - (std::f64::consts::PI / 5.0).sin()).abs() < 1e-15);
    // (−1)^{1/5}（主値）
    let w = pow(Term::int(-1), q(1, 5));
    let (re2, im2) = ball::value(&w, 64).unwrap().to_f64();
    assert!((re2 - re).abs() < 1e-15 && (im2 - im).abs() < 1e-15, "{re2} {im2}");
}

#[test]
fn cross_check() {
    use graph_core::exact::check::{against_f64, Check};
    use graph_core::exact::{evaluate, Outcome};
    let params = graph_core::eval::Params::new();
    let _d = graph_core::expand::install(graph_core::expand::Defs::parse("", &params));
    let pi2 = pow(Term::Atom(Atom::Pi), Term::int(2));
    let right = mul(vec![q(1, 6), pi2.clone()]);
    let wrong = mul(vec![q(1, 7), pi2]);
    assert_eq!(against_f64(&right, "@sum(k,1,∞) 1/k^2", &params), Check::Agree);
    assert!(matches!(against_f64(&wrong, "@sum(k,1,∞) 1/k^2", &params), Check::Disagree { .. }));
    // 評価の入口
    let r = evaluate("1/3+1/6", &params);
    assert!(matches!(&r.outcome, Outcome::Found(t) if *t == q(1, 2)), "{r:?}");
    assert_eq!(r.approx, Some((0.5, 0.0)));
    assert!(r.digits.as_deref().unwrap().starts_with("0.5"), "{:?}", r.digits);
    let r = evaluate("ln(-1)", &params);
    assert!(matches!(&r.outcome, Outcome::Found(_)), "{r:?}");
    assert!((r.approx.unwrap().1 - std::f64::consts::PI).abs() < 1e-15);
    // 今の道で計算できない整数（相手なし）は、四則と整数の関数だけなので採る
    let r = evaluate("gcd(2^60+1, 3^40)", &params);
    assert!(matches!(&r.outcome, Outcome::Found(t) if *t == Term::int(1)), "{r:?}");
    assert!(r.rules.contains(&"四則と整数の関数だけなので、相手なしで採った"));
    // 今の道が間違える桁落ち（mod の大きなべき）は、厳密値の道の方が正しいが、相手と食い違うので捨てる
    let r = evaluate("mod(3^40, 7)", &params);
    assert!(matches!(&r.outcome, Outcome::Rejected(_)), "{r:?}");
}

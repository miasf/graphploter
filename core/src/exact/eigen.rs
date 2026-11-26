//! 固有値・固有ベクトル・対角化・ジョルダン標準形・グラム・シュミット（計画 `docs/plan-exact-areas.md` の LA1）。
//!
//! 成分が有理数の正方行列 $A$ について:
//! 1. 固有多項式 $\det(\lambda I - A)$ をファデーエフ・ルヴェリエの方法で有理数のまま求め、有理数の上で因数分解する（EX6）
//! 2. 既約因数 $f$ ごとに、体 $\mathbb Q(\alpha) = \mathbb Q[t]/(f)$ の中で $A - \alpha I$（とそのべき）を掃き出す。
//!    固有ベクトル・一般固有ベクトルの成分は $\alpha$ の多項式で、根号を使わずに計算するので 0 の判定はちょうど。根の形は最後に入れる
//! 3. 根は 4 次まで根号（[`solve::irreducible_roots`]。複素数も）。表せない根は「$f(\lambda) = 0$ の根」のまま、固有ベクトルは $\lambda$ の式
//! 4. 照らし合わせ: 固有ベクトルは $A\mathbf v = \lambda\mathbf v$、対角化・ジョルダン標準形は $AP = PJ$ と $\det P \ne 0$ を高い精度の値で確かめ、
//!    固有値を今の道（f64。`linalg::Matrix::eigenvalues`）と比べる。グラム・シュミットは正規直交と張る空間を確かめ、今の道の
//!    `linalg::orthonormalize` と比べる
//!
//! 行がまるごと `@eig(A)`・`@diagonalize(A)`・`@jordan(A)`・`@gramschmidt(v₁, v₂, …)` のときに [`report`] が答える。
//! 答えは文（`λ = 1: …`・`P = …, D = …`）なので、方程式の解と同じく `Report::equation` を立てる（画面で「=」を前に付けない）

use super::latex::{to_latex, Style};
use super::num::{self, Int, Rat};
use super::poly::{self, Poly};
use super::term::{add, mul, neg, pow, Term};
use super::{ball, solve, Budget, Outcome, Report, Stop};

/// 行の命令の名前
pub const OPS: [&str; 4] = ["eig", "diagonalize", "jordan", "gramschmidt"];

/// 固有値の計算の行列の大きさの上限（今の道の行列の上限と同じ）
const MAX_EIG: usize = 6;
/// ジョルダン標準形・グラム・シュミットの大きさの上限（D4）
const MAX_JORDAN: usize = 4;

fn nf(s: impl Into<String>) -> Stop {
    Stop::NotFound(s.into())
}

fn charge(budget: &Budget, n: usize) -> Result<(), Stop> {
    budget.charge(n as u64).map_err(|_| Stop::TimeUp)
}

// ---------------------------------------------------------------- 体 Q(α)

/// $\mathbb Q(\alpha) = \mathbb Q[t]/(f)$（$f$ はモニックの既約多項式）。元は次数が $\deg f$ 未満の多項式
struct Field {
    f: Poly,
}

impl Field {
    fn red(&self, a: Poly) -> Poly {
        if a.deg() < self.f.deg() {
            a
        } else {
            a.divrem(&self.f).1
        }
    }
    fn mul(&self, a: &Poly, b: &Poly, budget: &Budget) -> Result<Poly, Stop> {
        if a.is_zero() || b.is_zero() {
            return Ok(Poly::zero());
        }
        charge(budget, (a.0.len() + 1) * (b.0.len() + 1))?;
        Ok(self.red(a.mul(b)))
    }
    /// 逆元（a ≠ 0。f が既約なので gcd(a, f) = 1）
    fn inv(&self, a: &Poly) -> Poly {
        let (_, s, _) = poly::ext_gcd(a, &self.f);
        self.red(s)
    }
    /// α そのもの
    fn alpha(&self) -> Poly {
        self.red(Poly::x())
    }
}

type FMat = Vec<Vec<Poly>>;
type FVec = Vec<Poly>;

fn fmat_mul(fd: &Field, a: &FMat, b: &FMat, budget: &Budget) -> Result<FMat, Stop> {
    let (n, m, p) = (a.len(), b.len(), b.first().map_or(0, Vec::len));
    let mut out = vec![vec![Poly::zero(); p]; n];
    for i in 0..n {
        for j in 0..p {
            let mut s = Poly::zero();
            for k in 0..m {
                s = s.add(&fd.mul(&a[i][k], &b[k][j], budget)?);
            }
            out[i][j] = s;
        }
    }
    Ok(out)
}

fn fmat_vec(fd: &Field, a: &FMat, v: &FVec, budget: &Budget) -> Result<FVec, Stop> {
    a.iter()
        .map(|row| {
            let mut s = Poly::zero();
            for (x, y) in row.iter().zip(v) {
                s = s.add(&fd.mul(x, y, budget)?);
            }
            Ok(s)
        })
        .collect()
}

/// 簡約された行列と主成分の列
fn rref(fd: &Field, mut a: FMat, budget: &Budget) -> Result<(FMat, Vec<usize>), Stop> {
    let rows = a.len();
    let cols = a.first().map_or(0, Vec::len);
    let mut pivots = Vec::new();
    let mut r = 0;
    for c in 0..cols {
        if r >= rows {
            break;
        }
        let Some(p) = (r..rows).find(|&i| !a[i][c].is_zero()) else { continue };
        a.swap(p, r);
        let inv = fd.inv(&a[r][c]);
        for j in 0..cols {
            let v = fd.mul(&a[r][j], &inv, budget)?;
            a[r][j] = v;
        }
        for i in 0..rows {
            if i == r || a[i][c].is_zero() {
                continue;
            }
            let f = a[i][c].clone();
            for j in 0..cols {
                let v = a[i][j].sub(&fd.mul(&f, &a[r][j], budget)?);
                a[i][j] = v;
            }
        }
        pivots.push(c);
        r += 1;
    }
    Ok((a, pivots))
}

/// 核の基底（自由変数の 1 つを 1、ほかの自由変数を 0 にした解）
fn null_basis(fd: &Field, a: &FMat, budget: &Budget) -> Result<Vec<FVec>, Stop> {
    let cols = a.first().map_or(0, Vec::len);
    let (r, piv) = rref(fd, a.clone(), budget)?;
    let mut out = Vec::new();
    for free in (0..cols).filter(|c| !piv.contains(c)) {
        let mut v = vec![Poly::zero(); cols];
        v[free] = Poly::constant(Rat::ONE);
        for (i, &pc) in piv.iter().enumerate() {
            v[pc] = r[i][free].scale(&-Rat::ONE);
        }
        out.push(v);
    }
    Ok(out)
}

/// ベクトルの並びの階数
fn rank_of(fd: &Field, vs: &[FVec], budget: &Budget) -> Result<usize, Stop> {
    if vs.is_empty() {
        return Ok(0);
    }
    Ok(rref(fd, vs.to_vec(), budget)?.1.len())
}

/// ベクトルの並び（1 本の鎖でもよい）を、同じ正の有理数倍で、係数が互いに素な整数になるようにする
fn clear_denominators(vs: &mut [FVec]) {
    let mut den = Int::ONE;
    let mut g = Int::ZERO;
    for v in vs.iter() {
        for c in v {
            for q in &c.0 {
                den = num::lcm(&den, &Int::from(q.denominator().clone()));
            }
        }
    }
    let scale = Rat::from(den);
    for v in vs.iter_mut() {
        for c in v.iter_mut() {
            *c = c.scale(&scale);
            for q in &c.0 {
                g = num::gcd(&g, q.numerator());
            }
        }
    }
    if g > Int::ONE {
        let inv = Rat::from_parts_signed(Int::ONE, g);
        for v in vs.iter_mut() {
            for c in v.iter_mut() {
                *c = c.scale(&inv);
            }
        }
    }
}

// ---------------------------------------------------------------- 固有多項式と固有空間

/// 成分が有理数の正方行列（そうでなければ見つからない）
fn rational_square(m: &super::matrix::EMat, max: usize) -> Result<Vec<Vec<Rat>>, Stop> {
    if m.rows != m.cols {
        return Err(nf(format!("固有値は正方行列で求めます（{}×{}）", m.rows, m.cols)));
    }
    if m.rows > max {
        return Err(nf(format!("{max}×{max} より大きい行列はまだ厳密に計算しません")));
    }
    let mut a = Vec::with_capacity(m.rows);
    for i in 0..m.rows {
        let mut row = Vec::with_capacity(m.cols);
        for j in 0..m.cols {
            match m.at(i, j) {
                Term::Num(q) => row.push(q.clone()),
                t if !t.is_number() => return Err(nf("文字を含む行列の固有値はまだ厳密に計算しません")),
                _ => return Err(nf("成分が有理数（整数・分数・有限小数）の行列だけ、固有値を厳密に計算します")),
            }
        }
        a.push(row);
    }
    Ok(a)
}

/// 固有多項式 $\det(\lambda I - A)$（モニック）。ファデーエフ・ルヴェリエの方法（今の道の `char_poly` と同じ手順を有理数で）
pub fn char_poly(a: &[Vec<Rat>]) -> Poly {
    let n = a.len();
    let mut c = vec![Rat::ZERO; n + 1];
    c[n] = Rat::ONE;
    let mut m = vec![vec![Rat::ZERO; n]; n];
    for k in 1..=n {
        let mut next = vec![vec![Rat::ZERO; n]; n];
        for i in 0..n {
            for j in 0..n {
                let mut s = Rat::ZERO;
                for l in 0..n {
                    if !a[i][l].is_zero() && !m[l][j].is_zero() {
                        s += a[i][l].clone() * &m[l][j];
                    }
                }
                if i == j {
                    s += &c[n - k + 1];
                }
                next[i][j] = s;
            }
        }
        m = next;
        let mut tr = Rat::ZERO;
        for i in 0..n {
            for l in 0..n {
                tr += a[i][l].clone() * &m[l][i];
            }
        }
        c[n - k] = -tr / Rat::from(Int::from(k as i64));
    }
    Poly::new(c)
}

/// 固有多項式の既約因数 1 つぶんの固有値
pub struct Group {
    /// モニックの既約因数
    pub factor: Poly,
    /// 代数的重複度（因数の重なり）
    pub mult: usize,
    /// 根（根号で表せれば。並びは [`solve::irreducible_roots`] の順）
    pub roots: Option<Vec<Term>>,
    /// 固有空間の基底（成分は α の多項式。係数は互いに素な整数）
    pub vectors: Vec<FVec>,
    field: Field,
    /// $A - \alpha I$
    shifted: FMat,
}

/// 固有値の分解
pub struct Eigen {
    pub n: usize,
    pub a: Vec<Vec<Rat>>,
    /// 固有多項式（モニック）
    pub charpoly: Poly,
    pub groups: Vec<Group>,
}

/// 固有多項式・固有値・固有空間
pub fn eigen(m: &super::matrix::EMat, max: usize, budget: &Budget) -> Result<Eigen, Stop> {
    let a = rational_square(m, max)?;
    let n = a.len();
    let cp = char_poly(&a);
    let fac = poly::factor(&cp, budget);
    if !fac.complete {
        return Err(Stop::TimeUp);
    }
    let mut groups = Vec::new();
    for (f, k) in fac.factors {
        let field = Field { f: f.monic() };
        let alpha = field.alpha();
        // A − αI（成分は Q(α) の元）
        let shifted: FMat = (0..n)
            .map(|i| (0..n).map(|j| if i == j { Poly::constant(a[i][j].clone()).sub(&alpha) } else { Poly::constant(a[i][j].clone()) }).collect())
            .collect();
        let mut vectors = null_basis(&field, &shifted, budget)?;
        if vectors.is_empty() || vectors.len() > k as usize {
            return Err(nf("内部エラー: 固有空間の次元が重複度と合いません"));
        }
        for v in vectors.iter_mut() {
            clear_denominators(std::slice::from_mut(v));
        }
        let roots = solve::irreducible_roots(&field.f, budget)?;
        groups.push(Group { factor: field.f.clone(), mult: k as usize, roots, vectors, field, shifted });
    }
    let total: i64 = groups.iter().map(|g| g.factor.deg() * g.mult as i64).sum();
    if total != n as i64 {
        return Err(nf("内部エラー: 固有多項式の因数の次数の和が合いません"));
    }
    Ok(Eigen { n, a, charpoly: cp, groups })
}

/// 成分が α の多項式のベクトルに、根を入れる
fn at_root(v: &FVec, root: &Term) -> Vec<Term> {
    v.iter()
        .map(|c| add(c.0.iter().enumerate().filter(|(_, q)| !q.is_zero()).map(|(k, q)| mul(vec![Term::Num(q.clone()), pow(root.clone(), Term::int(k as i64))])).collect()))
        .collect()
}

/// 数のベクトルの並び（1 本の鎖でもよい）を、同じ正の有理数倍で、和の項の係数が互いに素な整数になるようにする
/// （根を入れた後の $\frac{1 - \sqrt5}{2}$ を $1 - \sqrt5$ に。向きは変えない）
fn clear_terms(vs: &mut [Vec<Term>]) {
    let coeffs = |t: &Term| -> Vec<Rat> {
        match t {
            Term::Add(v) => v.iter().map(|a| super::term::split_coeff(a).0).collect(),
            other => vec![super::term::split_coeff(other).0],
        }
    };
    let mut den = Int::ONE;
    for t in vs.iter().flatten() {
        for q in coeffs(t) {
            den = num::lcm(&den, &Int::from(q.denominator().clone()));
        }
    }
    let scale = Term::Num(Rat::from(den));
    for t in vs.iter_mut().flatten() {
        *t = mul(vec![scale.clone(), t.clone()]);
    }
    let mut g = Int::ZERO;
    for t in vs.iter().flatten() {
        for q in coeffs(t) {
            g = num::gcd(&g, q.numerator());
        }
    }
    if g > Int::ONE {
        let inv = Term::Num(Rat::from_parts_signed(Int::ONE, g));
        for t in vs.iter_mut().flatten() {
            *t = mul(vec![inv.clone(), t.clone()]);
        }
    }
}

/// 根 `root` の固有ベクトル（根を入れて、係数をそろえたもの）
fn root_vectors(g: &Group, root: &Term) -> Vec<Vec<Term>> {
    g.vectors
        .iter()
        .map(|v| {
            let mut one = vec![at_root(v, root)];
            clear_terms(&mut one);
            one.pop().unwrap()
        })
        .collect()
}

/// 有理数の行列 × 数のベクトル
fn apply(a: &[Vec<Rat>], v: &[Term]) -> Vec<Term> {
    a.iter().map(|row| add(row.iter().zip(v).filter(|(q, _)| !q.is_zero()).map(|(q, x)| mul(vec![Term::Num(q.clone()), x.clone()])).collect())).collect()
}

fn is_zero_number(t: &Term) -> Result<bool, Stop> {
    ball::is_zero(t, &mut Vec::new()).ok_or_else(|| nf("値を確かめられません"))
}

/// 固有値の根（並び。表せる根だけ）と、その群・根の番号。並びは実数（小さい順）→ 実数でないもの（実部、虚部の順）
fn ordered_roots(e: &Eigen) -> Vec<(Term, usize)> {
    let mut out: Vec<(Term, usize, (f64, f64))> = Vec::new();
    for (gi, g) in e.groups.iter().enumerate() {
        for r in g.roots.iter().flatten() {
            let v = ball::value(r, 64).map(|b| b.to_f64()).unwrap_or((f64::NAN, f64::NAN));
            out.push((r.clone(), gi, v));
        }
    }
    let real = |k: (f64, f64)| k.1.abs() <= 1e-12 * (1.0 + k.0.abs());
    out.sort_by(|a, b| real(b.2).cmp(&real(a.2)).then(a.2 .0.total_cmp(&b.2 .0)).then(a.2 .1.total_cmp(&b.2 .1)));
    out.into_iter().map(|(t, gi, _)| (t, gi)).collect()
}

/// 照らし合わせ（固有ベクトル）: 体の中で $(A - \alpha I)\mathbf v = 0$、根を入れて $A\mathbf v = \lambda\mathbf v$（$\mathbf v \ne 0$）
fn check_vectors(e: &Eigen, budget: &Budget) -> Result<(), String> {
    for g in &e.groups {
        for v in &g.vectors {
            let w = fmat_vec(&g.field, &g.shifted, v, budget).map_err(|_| "時間切れ".to_string())?;
            if w.iter().any(|c| !c.is_zero()) {
                return Err("固有ベクトルが核に入っていません".into());
            }
        }
        // 3 次以上の因数の根（固有ベクトルは λ の式で出すので、根そのものを確かめる）: f(r) = 0
        for r in g.roots.iter().flatten().filter(|_| g.factor.deg() > 2) {
            let v = at_root(&vec![g.factor.clone()], r).pop().unwrap();
            if !is_zero_number(&v).map_err(|_| "値を確かめられません".to_string())? {
                return Err("根を入れても固有多項式の因数が 0 になりません".into());
            }
        }
        for r in g.roots.iter().flatten().filter(|_| g.factor.deg() <= 2) {
            for x in root_vectors(g, r) {
                let ax = apply(&e.a, &x);
                for (p, q) in ax.iter().zip(&x) {
                    let d = add(vec![p.clone(), neg(mul(vec![r.clone(), q.clone()]))]);
                    if !is_zero_number(&d).map_err(|_| "値を確かめられません".to_string())? {
                        return Err("根を入れた固有ベクトルが Av = λv を満たしません".into());
                    }
                }
                let mut nonzero = false;
                for q in &x {
                    if !is_zero_number(q).map_err(|_| "値を確かめられません".to_string())? {
                        nonzero = true;
                        break;
                    }
                }
                if !nonzero {
                    return Err("根を入れた固有ベクトルが 0 です".into());
                }
            }
        }
    }
    Ok(())
}

/// 照らし合わせ（固有値）:
/// 1. 因数の積が固有多項式に戻る（ちょうど）。固有多項式の係数が今の道（f64 のファデーエフ・ルヴェリエ）と同じ
/// 2. 根が今の道の固有値（デュラン・ケルナー法）と、重なりも込めて同じ。重なった根は今の道で ε^{1/m} くらいしか合わないので、
///    重なり m の根は $100\,\varepsilon^{1/m}$ まで許す（4 重の根で今の道は 2 × 10⁻³ ずれる）。表せない根の群は、数だけ数える
fn check_against_f64(e: &Eigen) -> Result<(), String> {
    let mut prod = Poly::constant(Rat::ONE);
    for g in &e.groups {
        prod = prod.mul(&g.factor.pow(g.mult as u32));
    }
    if prod != e.charpoly {
        return Err("因数の積が固有多項式に戻りません".into());
    }
    let flat: Vec<f64> = e.a.iter().flatten().map(|q| q.to_f64().value()).collect();
    let fm = crate::linalg::Matrix::new(e.n, e.n, flat);
    let cf = fm.char_poly();
    let big = cf.iter().fold(1.0f64, |m, c| m.max(c.abs()));
    for (k, c) in cf.iter().enumerate() {
        let x = e.charpoly.0.get(k).map_or(0.0, |q| q.to_f64().value());
        if (x - c).abs() > 1e-9 * big {
            return Err(format!("固有多項式の {k} 次の係数が今の道（{c}）と違います（{x}）"));
        }
    }
    let mut theirs: Vec<(f64, f64)> = Vec::new();
    for ev in fm.eigenvalues() {
        for _ in 0..ev.mult {
            theirs.push((ev.re, ev.im));
        }
    }
    if theirs.len() != e.n {
        return Err(format!("今の道の固有値の数が {} です", theirs.len()));
    }
    let scale = e.a.iter().flatten().map(|q| q.to_f64().value().abs()).fold(0.0f64, f64::max);
    let mut used = vec![false; theirs.len()];
    let mut unexpressed = 0usize;
    for g in &e.groups {
        let Some(rs) = &g.roots else {
            unexpressed += g.factor.deg() as usize * g.mult;
            continue;
        };
        for r in rs {
            let (re, im) = ball::value(r, 64).map_err(|_| "値を確かめられません".to_string())?.to_f64();
            // 重なった根は、今の道（デュラン・ケルナー法）で m 乗根の精度（ε^{1/m}）しか合わない
            let tol = (1e-6f64).max(100.0 * f64::EPSILON.powf(1.0 / g.mult as f64)) * scale.max((re * re + im * im).sqrt()).max(1.0);
            for _ in 0..g.mult {
                let best = theirs.iter().enumerate().filter(|(i, _)| !used[*i]).map(|(i, t)| (i, ((t.0 - re).powi(2) + (t.1 - im).powi(2)).sqrt())).min_by(|a, b| a.1.total_cmp(&b.1));
                match best {
                    Some((i, d)) if d <= tol => used[i] = true,
                    _ => return Err(format!("固有値 {re} + {im}i が今の道の固有値にありません")),
                }
            }
        }
    }
    if used.iter().filter(|u| !**u).count() != unexpressed {
        return Err("今の道の固有値の数が合いません".into());
    }
    Ok(())
}

// ---------------------------------------------------------------- ジョルダン鎖

/// 1 つの群のジョルダン鎖（長い鎖から）。鎖は [固有ベクトル $N^{j-1}\mathbf c$, …, $N\mathbf c$, 頭 $\mathbf c$]（$N = A - \alpha I$）
fn jordan_chains(g: &Group, n: usize, budget: &Budget) -> Result<Vec<Vec<FVec>>, Stop> {
    let fd = &g.field;
    let k = g.mult;
    // 核 K_j = ker N^j（K_0 = {0}）。次元が重複度に届くまで
    let mut kers: Vec<Vec<FVec>> = vec![Vec::new(), g.vectors.clone()];
    let mut p = g.shifted.clone();
    while kers.last().unwrap().len() < k {
        if kers.len() > n + 1 {
            return Err(nf("内部エラー: 一般固有空間の次元が重複度に届きません"));
        }
        p = fmat_mul(fd, &p, &g.shifted, budget)?;
        kers.push(null_basis(fd, &p, budget)?);
    }
    let m = kers.len() - 1;
    // levels[j]: もう選んだ鎖の、高さ j のベクトル
    let mut levels: Vec<Vec<FVec>> = vec![Vec::new(); m + 1];
    let mut chains: Vec<Vec<FVec>> = Vec::new();
    for j in (1..=m).rev() {
        let mut base: Vec<FVec> = kers[j - 1].clone();
        base.extend(levels[j].iter().cloned());
        let mut r = rank_of(fd, &base, budget)?;
        for c in &kers[j] {
            let mut trial = base.clone();
            trial.push(c.clone());
            let r2 = rank_of(fd, &trial, budget)?;
            if r2 <= r {
                continue;
            }
            base = trial;
            r = r2;
            let mut chain = vec![c.clone()];
            for _ in 1..j {
                let next = fmat_vec(fd, &g.shifted, chain.last().unwrap(), budget)?;
                chain.push(next);
            }
            for (t, v) in chain.iter().enumerate().skip(1) {
                levels[j - t].push(v.clone());
            }
            chain.reverse();
            clear_denominators(&mut chain);
            chains.push(chain);
        }
    }
    if chains.iter().map(Vec::len).sum::<usize>() != k {
        return Err(nf("内部エラー: ジョルダン鎖の長さの和が重複度と合いません"));
    }
    Ok(chains)
}

/// 標準形の行列 J（ジョルダン細胞を対角に並べる）と P。`diagonal` なら細胞は全部 1 × 1
fn assemble(e: &Eigen, chains: &[Vec<Vec<FVec>>]) -> Result<(Vec<Vec<Term>>, Vec<Vec<Term>>, Vec<(Term, usize)>), Stop> {
    let n = e.n;
    let mut cols: Vec<Vec<Term>> = Vec::new();
    let mut blocks: Vec<(Term, usize)> = Vec::new();
    if e.groups.iter().any(|g| g.factor.deg() >= 3) {
        return Err(nf("固有値が 3 次以上の既約因数の根なので、P と標準形はまだ出しません（@eig で固有ベクトルを λ の式で出します）"));
    }
    for (root, gi) in ordered_roots(e) {
        for chain in &chains[gi] {
            let mut vs: Vec<Vec<Term>> = chain.iter().map(|v| at_root(v, &root)).collect();
            clear_terms(&mut vs);
            cols.extend(vs);
            blocks.push((root.clone(), chain.len()));
        }
    }
    if cols.len() != n {
        return Err(nf("固有値を根号で表せないので、P と標準形は出しません"));
    }
    let p: Vec<Vec<Term>> = (0..n).map(|i| (0..n).map(|j| cols[j][i].clone()).collect()).collect();
    let mut j = vec![vec![Term::zero(); n]; n];
    let mut at = 0;
    for (r, len) in &blocks {
        for t in 0..*len {
            j[at + t][at + t] = r.clone();
            if t + 1 < *len {
                j[at + t][at + t + 1] = Term::one();
            }
        }
        at += len;
    }
    Ok((p, j, blocks))
}

/// 照らし合わせ: $AP = PJ$ と $\det P \ne 0$
fn check_similar(a: &[Vec<Rat>], p: &[Vec<Term>], j: &[Vec<Term>]) -> Result<(), String> {
    let n = a.len();
    for r in 0..n {
        for c in 0..n {
            let ap = add((0..n).filter(|&k| !a[r][k].is_zero()).map(|k| mul(vec![Term::Num(a[r][k].clone()), p[k][c].clone()])).collect());
            let pj = add((0..n).map(|k| mul(vec![p[r][k].clone(), j[k][c].clone()])).collect());
            if !is_zero_number(&add(vec![ap, neg(pj)])).map_err(|_| "値を確かめられません".to_string())? {
                return Err("AP = PJ になりません".into());
            }
        }
    }
    let pm = super::matrix::EMat { rows: n, cols: n, a: p.iter().flatten().cloned().collect() };
    let d = super::matrix::det(&pm).map_err(|_| "det P を計算できません".to_string())?;
    if is_zero_number(&d).map_err(|_| "値を確かめられません".to_string())? {
        return Err("P が正則ではありません".into());
    }
    Ok(())
}

// ---------------------------------------------------------------- 書き出し

fn mat_latex(rows: &[Vec<Term>], st: &Style) -> String {
    let body: Vec<String> = rows.iter().map(|r| r.iter().map(|t| to_latex(t, st)).collect::<Vec<_>>().join(" & ")).collect();
    format!("\\begin{{pmatrix}}{}\\end{{pmatrix}}", body.join("\\\\"))
}

fn col_latex(v: &[Term], st: &Style) -> String {
    format!("\\begin{{pmatrix}}{}\\end{{pmatrix}}", v.iter().map(|t| to_latex(t, st)).collect::<Vec<_>>().join("\\\\"))
}

/// α の多項式を λ の式で（表せない根の固有ベクトル）
fn lambda_col(v: &FVec, st: &Style) -> String {
    let items: Vec<String> = v.iter().map(|c| super::commands::poly_text(c, "\\lambda", st).0).collect();
    format!("\\begin{{pmatrix}}{}\\end{{pmatrix}}", items.join("\\\\"))
}

fn mult_text(k: usize) -> String {
    if k > 1 {
        format!("\\ \\left(\\text{{重複度 }}{k}\\right)")
    } else {
        String::new()
    }
}

/// 固有多項式の因数分解の LaTeX（λ の式。モニックの因数を、次数の低い順・1 次は根の小さい順に）
fn charpoly_latex(e: &Eigen, st: &Style) -> String {
    let mut fs: Vec<&Group> = e.groups.iter().collect();
    let key = |g: &Group| g.roots.as_ref().and_then(|r| r.first()).and_then(|r| ball::value(r, 64).ok()).map(|b| b.to_f64().0).unwrap_or(f64::INFINITY);
    fs.sort_by(|a, b| a.factor.deg().cmp(&b.factor.deg()).then(key(a).total_cmp(&key(b))));
    let one = fs.len() == 1 && fs[0].mult == 1;
    let mut out = String::new();
    for g in fs {
        // 1 次の因数は根の書き方で（小数の入力なら λ − 0.3）
        let body = if g.factor.deg() == 1 {
            let r = -g.factor.0[0].clone();
            match num::sign(&r) {
                0 => "\\lambda".to_string(),
                s if s > 0 => format!("\\lambda-{}", to_latex(&Term::Num(r), st)),
                _ => format!("\\lambda+{}", to_latex(&Term::Num(-r), st)),
            }
        } else {
            super::commands::poly_text(&g.factor, "\\lambda", st).0
        };
        let paren = !one && g.factor.0.iter().filter(|c| !c.is_zero()).count() > 1;
        let b = if paren { format!("\\left({body}\\right)") } else { body };
        out.push_str(&if g.mult > 1 { format!("{b}^{{{}}}", g.mult) } else { b });
    }
    out
}

fn eig_latex(e: &Eigen, st: &Style) -> String {
    let mut items: Vec<String> = vec![format!("\\text{{固有多項式 }}{}", charpoly_latex(e, st))];
    // 2 次までの因数の根: 根ごとに固有ベクトル（根を入れる）
    for (root, gi) in ordered_roots(e) {
        let g = &e.groups[gi];
        if g.factor.deg() > 2 {
            continue;
        }
        let vs: Vec<String> = root_vectors(g, &root).iter().map(|v| col_latex(v, st)).collect();
        items.push(format!("\\lambda={}{}:\\ {}", to_latex(&root, st), mult_text(g.mult), vs.join(",\\ ")));
    }
    // 3 次以上の因数: 根を並べ（表せれば）、固有ベクトルは λ の式（根を入れると式が大きくなるため）
    for g in e.groups.iter().filter(|g| g.factor.deg() > 2) {
        let (f, _) = super::commands::poly_text(&g.factor, "\\lambda", st);
        let vs: Vec<String> = g.vectors.iter().map(|v| lambda_col(v, st)).collect();
        let head = match &g.roots {
            Some(rs) => format!("\\lambda={}\\ \\left({f}=0\\text{{ の根}}\\right)", rs.iter().map(|r| to_latex(r, st)).collect::<Vec<_>>().join(",\\ ")),
            None => format!("{f}=0\\text{{ の根 }}\\lambda"),
        };
        items.push(format!("{head}{}:\\ {}", mult_text(g.mult), vs.join(",\\ ")));
    }
    items.join(",\\quad ")
}

// ---------------------------------------------------------------- グラム・シュミット

/// 正規直交にしたベクトル 1 本: 単位ベクトルの成分と、見せる形（$\frac{1}{\sqrt{n}}$ × 整数のベクトル）
struct Unit {
    unit: Vec<Term>,
    latex: String,
}

/// $n = a^2 b$（$b$ は平方因子を持たない）の (a, b)
fn square_part(n: &Int, budget: &Budget) -> Result<(Int, Int), Stop> {
    let (fs, rest) = num::factor_int(n, budget).map_err(|_| Stop::TimeUp)?;
    if rest.is_some() {
        return Err(nf("ノルムを素因数分解しきれません"));
    }
    let (mut a, mut b) = (Int::ONE, Int::ONE);
    for (p, k) in fs {
        let p = Int::from(p);
        for _ in 0..k / 2 {
            a *= &p;
        }
        if k % 2 == 1 {
            b *= &p;
        }
    }
    Ok((a, b))
}

fn dot(x: &[Term], y: &[Term]) -> Term {
    add(x.iter().zip(y).map(|(p, q)| mul(vec![p.clone(), q.clone()])).collect())
}

/// グラム・シュミット。1 次結合で長さが 0 になったベクトルは除く（今の道の `orthonormalize` と同じ）。(単位ベクトル, 除いた番号)
fn gram_schmidt(vs: &[Vec<Term>], st: &Style, budget: &Budget) -> Result<(Vec<Unit>, Vec<usize>), Stop> {
    let rational = vs.iter().flatten().all(|t| t.as_rat().is_some());
    let mut ws: Vec<Vec<Term>> = Vec::new();
    let mut out = Vec::new();
    let mut dropped = Vec::new();
    for (k, v) in vs.iter().enumerate() {
        charge(budget, v.len() * (ws.len() + 1))?;
        let mut u = v.clone();
        for w in &ws {
            let c = super::term::div(dot(v, w), dot(w, w));
            u = u.iter().zip(w).map(|(a, b)| add(vec![a.clone(), neg(mul(vec![c.clone(), b.clone()]))])).collect();
        }
        let mut zero = true;
        for c in &u {
            if !is_zero_number(c)? {
                zero = false;
                break;
            }
        }
        if zero {
            dropped.push(k);
            continue;
        }
        if rational {
            // 整数の互いに素な成分にそろえる（正の倍なので向きは同じ）
            let mut fv: Vec<FVec> = vec![u.iter().map(|t| Poly::constant(t.as_rat().cloned().unwrap_or(Rat::ZERO))).collect()];
            clear_denominators(&mut fv);
            let w: Vec<Term> = fv[0].iter().map(|c| Term::Num(c.0.first().cloned().unwrap_or(Rat::ZERO))).collect();
            let n2 = dot(&w, &w);
            let n2i = n2.as_rat().and_then(num::as_int).ok_or_else(|| nf("内部エラー: ノルムが整数になりません"))?;
            let (a, b) = square_part(&n2i, budget)?;
            let coef = match (a == Int::ONE, b == Int::ONE) {
                (true, true) => String::new(),
                (false, true) => format!("\\frac{{1}}{{{a}}}"),
                (true, false) => format!("\\frac{{1}}{{\\sqrt{{{b}}}}}"),
                (false, false) => format!("\\frac{{1}}{{{a}\\sqrt{{{b}}}}}"),
            };
            let s = pow(n2, Term::rat(-1, 2));
            let unit: Vec<Term> = w.iter().map(|c| mul(vec![c.clone(), s.clone()])).collect();
            out.push(Unit { unit, latex: format!("{coef}{}", col_latex(&w, st)) });
            ws.push(w);
        } else {
            let s = pow(dot(&u, &u), Term::rat(-1, 2));
            let unit: Vec<Term> = u.iter().map(|c| mul(vec![c.clone(), s.clone()])).collect();
            out.push(Unit { latex: col_latex(&unit, st), unit });
            ws.push(u);
        }
    }
    Ok((out, dropped))
}

/// 照らし合わせ（グラム・シュミット）: $\mathbf u_i \cdot \mathbf u_j = \delta_{ij}$、$\mathbf v_k$ が $\mathbf u_1, \ldots$ の張る空間に入る、今の道の値と同じ
fn check_gram_schmidt(vs: &[Vec<Term>], units: &[Unit], dropped: &[usize]) -> Result<(), String> {
    let z = |t: &Term| is_zero_number(t).map_err(|_| "値を確かめられません".to_string());
    for (i, a) in units.iter().enumerate() {
        for (j, b) in units.iter().enumerate().skip(i) {
            let d = dot(&a.unit, &b.unit);
            let d = if i == j { add(vec![d, Term::int(-1)]) } else { d };
            if !z(&d)? {
                return Err("正規直交になりません".into());
            }
        }
    }
    let mut used = 0;
    for (k, v) in vs.iter().enumerate() {
        if !dropped.contains(&k) {
            used += 1;
        }
        let mut r = v.clone();
        for u in &units[..used] {
            let c = dot(v, &u.unit);
            r = r.iter().zip(&u.unit).map(|(a, b)| add(vec![a.clone(), neg(mul(vec![c.clone(), b.clone()]))])).collect();
        }
        for c in &r {
            if !z(c)? {
                return Err("元のベクトルが、正規直交にしたベクトルの張る空間に入りません".into());
            }
        }
    }
    // 今の道（f64 の修正グラム・シュミット）
    let fv: Vec<Vec<f64>> = vs.iter().map(|v| v.iter().map(|t| ball::value(t, 64).map(|b| b.to_f64().0).unwrap_or(f64::NAN)).collect()).collect();
    let theirs = crate::linalg::orthonormalize(fv);
    if theirs.len() != units.len() {
        return Err(format!("今の道の本数（{}）と違います", theirs.len()));
    }
    for (u, t) in units.iter().zip(&theirs) {
        for (a, b) in u.unit.iter().zip(t) {
            let x = ball::value(a, 64).map_err(|_| "値を確かめられません".to_string())?.to_f64().0;
            if (x - b).abs() > 1e-8 {
                return Err(format!("今の道の値 {b} と違います（{x}）"));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- 行の答え

/// 行がまるごと `@eig(A)`・`@diagonalize(A)`・`@jordan(A)`・`@gramschmidt(v₁, …)` なら、その答え。そうでなければ None
pub fn report(source: &str, params: &crate::eval::Params, work: u64) -> Option<Report> {
    let defs = crate::expand::current_defs();
    let names = defs.names();
    let e = crate::parser::parse_with(source, &names).ok()?;
    let crate::parser::Expr::Op { op, args } = &e else { return None };
    if !OPS.contains(&op.as_str()) {
        return None;
    }
    let (r, used) = super::with_work_limit(work, || -> Result<Report, Stop> {
        let mut c = super::convert::Converter::new(&defs, params);
        let vals = args.iter().map(|a| c.term(a)).collect::<Result<Vec<_>, _>>()?;
        let st = Style { decimal: c.decimal, input: false };
        let budget = Budget::new(work);
        let mut rep = Report { decimal: c.decimal, equation: true, ..Report::new(Outcome::NotFound(String::new())) };
        if op == "gramschmidt" {
            gram_schmidt_report(&vals, &st, &budget, &mut rep)?;
            return Ok(rep);
        }
        let [m] = vals.as_slice() else { return Err(nf(format!("@{op} の引数は行列 1 つです"))) };
        let m = super::matrix::EMat::of_term(m).filter(|m| m.cols > 1 || m.rows == 1).ok_or_else(|| nf(format!("@{op} は正方行列に使ってください")))?;
        let max = if op == "jordan" { MAX_JORDAN } else { MAX_EIG };
        let e = eigen(&m, max, &budget)?;
        if let Err(why) = check_vectors(&e, &budget) {
            rep.outcome = Outcome::Rejected(why);
            return Ok(rep);
        }
        if let Err(why) = check_against_f64(&e) {
            rep.outcome = Outcome::Rejected(format!("照らし合わせで食い違いました（{why}）"));
            return Ok(rep);
        }
        match op.as_str() {
            "eig" => {
                rep.latex = eig_latex(&e, &st);
                rep.rules.push("固有多項式を因数分解し、体 Q(α) で A − αI を掃き出した");
                rep.rules.push("固有ベクトルを Av = λv で確かめ、固有値を今の道と比べた");
                rep.outcome = Outcome::Found(Term::List(ordered_roots(&e).into_iter().map(|(t, _)| t).collect()));
            }
            "diagonalize" => {
                // 固有ベクトルが足りない固有値（いちばん小さいもの。表せない根の因数は後ろ）
                let order: Vec<usize> = ordered_roots(&e).into_iter().map(|(_, gi)| gi).chain(0..e.groups.len()).collect();
                if let Some(g) = order.iter().map(|&gi| &e.groups[gi]).find(|g| g.vectors.len() < g.mult) {
                    let which = match &g.roots {
                        Some(rs) if rs.len() == 1 => format!("固有値 }}\\lambda={}\\text{{ ", to_latex(&rs[0], &st)),
                        _ => format!("固有多項式の因数 }}{}\\text{{ の根", super::commands::poly_text(&g.factor, "\\lambda", &st).0),
                    };
                    rep.latex = format!("\\text{{対角化できません（{which}の重複度は {}、固有ベクトルは {} 本）}}", g.mult, g.vectors.len());
                    rep.rules.push("固有空間の次元が重複度より小さい");
                    rep.outcome = Outcome::Found(Term::List(Vec::new()));
                    return Ok(rep);
                }
                let chains: Vec<Vec<Vec<FVec>>> = e.groups.iter().map(|g| g.vectors.iter().map(|v| vec![v.clone()]).collect()).collect();
                let (p, d, _) = assemble(&e, &chains)?;
                if let Err(why) = check_similar(&e.a, &p, &d) {
                    rep.outcome = Outcome::Rejected(why);
                    return Ok(rep);
                }
                rep.latex = format!("P={},\\ D={}\\quad\\left(P^{{-1}}AP=D\\right)", mat_latex(&p, &st), mat_latex(&d, &st));
                rep.rules.push("対角化: AP = PD と det P ≠ 0 を確かめた");
                rep.outcome = Outcome::Found(Term::List(vec![to_matrix(&p), to_matrix(&d)]));
            }
            _ => {
                let mut chains = Vec::new();
                for g in &e.groups {
                    chains.push(jordan_chains(g, e.n, &budget)?);
                }
                let (p, j, _) = assemble(&e, &chains)?;
                if let Err(why) = check_similar(&e.a, &p, &j) {
                    rep.outcome = Outcome::Rejected(why);
                    return Ok(rep);
                }
                rep.latex = format!("P={},\\ J={}\\quad\\left(P^{{-1}}AP=J\\right)", mat_latex(&p, &st), mat_latex(&j, &st));
                rep.rules.push("ジョルダン標準形: 一般固有ベクトルの鎖。AP = PJ と det P ≠ 0 を確かめた");
                rep.outcome = Outcome::Found(Term::List(vec![to_matrix(&p), to_matrix(&j)]));
            }
        }
        Ok(rep)
    });
    Some(match r {
        Ok(mut rep) => {
            rep.work = used;
            rep
        }
        Err(Stop::NotFound(s)) => Report { work: used, equation: true, ..Report::new(Outcome::NotFound(s)) },
        Err(Stop::NotExist(s)) => Report { work: used, equation: true, ..Report::new(Outcome::NotExist(s)) },
        Err(Stop::TimeUp) => Report { work: used, equation: true, ..Report::new(Outcome::NotFound("時間切れ".into())) },
    })
}

fn to_matrix(rows: &[Vec<Term>]) -> Term {
    let (r, c) = (rows.len(), rows.first().map_or(0, Vec::len));
    Term::Matrix(r, c, rows.iter().flatten().cloned().collect())
}

fn gram_schmidt_report(vals: &[Term], st: &Style, budget: &Budget, rep: &mut Report) -> Result<(), Stop> {
    // 引数はベクトルの並び、または行列 1 つ（列ベクトル）
    let vs: Vec<Vec<Term>> = match vals {
        [Term::Matrix(r, c, a)] => (0..*c).map(|j| (0..*r).map(|i| a[i * c + j].clone()).collect()).collect(),
        _ => vals
            .iter()
            .map(|t| match t {
                Term::Tuple(v) => Ok(v.clone()),
                Term::Matrix(_, 1, v) => Ok(v.clone()),
                _ => Err(nf("@gramschmidt の引数はベクトルの並びか行列にしてください")),
            })
            .collect::<Result<_, _>>()?,
    };
    if vs.is_empty() || vs.len() > MAX_JORDAN {
        return Err(nf(format!("グラム・シュミットは {MAX_JORDAN} 本までのベクトルで計算します")));
    }
    let dim = vs[0].len();
    if dim < 2 || dim > MAX_EIG || vs.iter().any(|v| v.len() != dim) {
        return Err(nf("ベクトルの成分の数がそろっていません"));
    }
    for t in vs.iter().flatten() {
        if !t.is_number() {
            return Err(nf("文字を含むベクトルはまだ厳密に計算しません"));
        }
        let b = ball::value(t, 64).map_err(|_| nf("成分の値を計算できません"))?;
        if !b.is_real() {
            return Err(nf("成分が実数のベクトルだけ、グラム・シュミットを厳密に計算します"));
        }
    }
    let (units, dropped) = gram_schmidt(&vs, st, budget)?;
    if let Err(why) = check_gram_schmidt(&vs, &units, &dropped) {
        rep.outcome = Outcome::Rejected(why);
        return Ok(());
    }
    let mut items: Vec<String> = units.iter().enumerate().map(|(k, u)| format!("\\mathbf{{u}}_{{{}}}={}", k + 1, u.latex)).collect();
    if !dropped.is_empty() {
        let which: Vec<String> = dropped.iter().map(|k| format!("{}", k + 1)).collect();
        items.push(format!("\\text{{（{} 本目は前のベクトルの 1 次結合なので除いた）}}", which.join("・")));
    }
    rep.latex = items.join(",\\ ");
    rep.rules.push("グラム・シュミット: 正規直交と張る空間を確かめ、今の道と比べた");
    rep.outcome = Outcome::Found(Term::List(units.into_iter().map(|u| Term::Tuple(u.unit)).collect()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(n: i64) -> Rat {
        Rat::from(Int::from(n))
    }

    #[test]
    fn faddeev_leverrier() {
        // ⟦0,1,0;0,0,1;2,−5,4⟧ → λ³ − 4λ² + 5λ − 2 = (λ − 1)²(λ − 2)
        let a = vec![vec![q(0), q(1), q(0)], vec![q(0), q(0), q(1)], vec![q(2), q(-5), q(4)]];
        assert_eq!(char_poly(&a), Poly::new(vec![q(-2), q(5), q(-4), q(1)]));
    }

    #[test]
    fn square_parts() {
        let b = Budget::unlimited();
        assert_eq!(square_part(&Int::from(8), &b).unwrap(), (Int::from(2), Int::from(2)));
        assert_eq!(square_part(&Int::from(6), &b).unwrap(), (Int::ONE, Int::from(6)));
        assert_eq!(square_part(&Int::from(9), &b).unwrap(), (Int::from(3), Int::ONE));
    }
}

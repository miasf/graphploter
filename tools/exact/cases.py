#!/usr/bin/env python3
"""厳密値の例題の表（`core/tests/exact_cases/*.tsv`）を作り、答えを確かめる（計画 `docs/plan-exact-core.md` の EX0）。

    python3 -m venv /tmp/venv && /tmp/venv/bin/pip install sympy==1.13.3
    /tmp/venv/bin/python tools/exact/cases.py          # 確かめて tsv を書き直す
    /tmp/venv/bin/python tools/exact/cases.py --check  # 確かめるだけ（tsv と食い違えば終了コード 1）

- 1 問ごとに「問題の値（SymPy の式）」と「期待する答え（SymPy の式）」を 30 桁で比べる。
  不定積分は、答えを微分して中身と一致することを 3 点で確かめる。極限・級数で「存在しない・発散」の行は確かめない（理由を書く）
- 期待する答えの LaTeX は、計画の 2.6 の答えの形で書く。数（有理数・根号・定数の和）は下の `fmt` が作る
- CI では動かさない（SymPy が要る）。tsv はリポジトリに入れる

答えの形の細かい決まり（2.6 に書いていない所。`fmt` の動き）:
- 数だけの和の項の順: 有理数 → 根号（中の数の小さい順）→ π → e → 対数 → γ → G → ζ。先頭が負なら、最初の正の項を先頭に出す
  （$\\sqrt5 - 1$、$1 + \\sqrt2$、$2 - \\sqrt3$、$\\sqrt6 - \\sqrt2$）
- 有理数と根号だけの和は 1 本の分数線にまとめる（$\\frac{\\sqrt6-\\sqrt2}{4}$）。定数（π など）を含む和は項ごとの分数
- 自然対数は `\\ln`（このアプリの `\\log` は常用対数なので。文書 7 の D5 の計画との違い）
- 複素数は「実部 + 虚部 i」で、虚部は項ごとに $\\frac{\\sqrt3}{2}i$
"""

import math
import re as re_mod
import sys
from pathlib import Path

from sympy import (
    Abs, Add, Catalan, E, EulerGamma, I, Integer, Matrix, Mul, Pow, Rational, S, Symbol, atan, acos, asin, binomial, cos,
    diff, exp, factorial, gcd, ilcm, igcd, integrate, limit, log, nsimplify, oo, pi, sin, sqrt, symbols, tan, zeta, N,
    Sum, cbrt, harmonic, zeros,
)

x, k, n = symbols("x k n")

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "core" / "tests" / "exact_cases"

# ---------------------------------------------------------------- 答えの形の LaTeX

KIND_ORDER = ["rational", "sqrt", "pi", "e", "ln", "gamma", "catalan", "zeta", "other"]


def atom_latex(a):
    """係数を除いた因子（単項）の LaTeX と、並べるための種類"""
    if a == 1:
        return "", ("rational", 0)
    if a == pi:
        return r"\pi", ("pi", 1)
    if a == E:
        return "e", ("e", 1)
    if a == EulerGamma:
        return r"\gamma", ("gamma", 0)
    if a == Catalan:
        return "G", ("catalan", 0)
    if isinstance(a, Pow):
        b, ex = a.args
        if b == pi and ex.is_Integer:
            return r"\pi^{%d}" % ex, ("pi", int(ex))
        if b == pi and ex == Rational(1, 2):
            return r"\sqrt{\pi}", ("pi", 0.5)
        if b.is_Integer and ex == Rational(1, 2):
            return r"\sqrt{%d}" % b, ("sqrt", int(b))
        if b.is_Integer and ex == Rational(1, 3):
            return r"\sqrt[3]{%d}" % b, ("sqrt", int(b) + 0.5)
        if b == E or (hasattr(b, "func") and b.func == exp):
            pass
    if a.func == exp:
        ex = a.args[0]
        if ex.is_Integer:
            return "e^{%d}" % ex, ("e", int(ex))
        return "e^{%s}" % fmt(ex), ("e", 99)
    if a.func == log and a.args[0].is_Integer:
        return r"\ln %d" % a.args[0], ("ln", int(a.args[0]))
    if a.func == zeta:
        return r"\zeta(%s)" % a.args[0], ("zeta", int(a.args[0]))
    if isinstance(a, Pow) and a.args[1] == Rational(1, 2):
        return r"\sqrt{%s}" % fmt(a.args[0]), ("sqrt", 1000)
    if isinstance(a, Mul):
        parts = sorted((atom_latex(f) for f in a.args), key=lambda p: kind_key(p[1]))
        return "".join(p[0] for p in parts), max((p[1] for p in parts), key=lambda kd: KIND_ORDER.index(kd[0]))
    raise ValueError(f"形が分からない因子: {a!r}")


def kind_key(kd):
    """並べる順の鍵: 種類の順、根号は中の数の小さい順、π と e は指数の大きい順（多項式の降べきと同じ）"""
    return (KIND_ORDER.index(kd[0]), -kd[1] if kd[0] in ("pi", "e") else kd[1])


def split_rest(rest):
    """係数を除いた部分を、分子の因子と分母の因子に分ける（π^{-1} → 分母の π、e^{-1} → 分母の e）"""
    num, den = S(1), S(1)
    for f in Mul.make_args(rest):
        if isinstance(f, Pow) and f.args[1].is_Integer and f.args[1] < 0:
            den *= f.args[0] ** (-f.args[1])
        elif f.func == exp and f.args[0] == -1:
            den *= E
        else:
            num *= f
    return num, den


def split_term(t):
    c, rest = t.as_coeff_Mul()
    if not c.is_Rational:
        raise ValueError(f"係数が有理数でない: {t!r}")
    return Rational(c), rest


def coef_times(c, body):
    """整数 c × body（body は空でもよい）"""
    if body == "":
        return str(c)
    if c == 1:
        return body
    if c == -1:
        return "-" + body
    return f"{c}{body}"


def fmt_real(v):
    v = nsimplify(v) if v.is_Float else v
    terms = [split_term(t) for t in Add.make_args(v.expand())]
    infos = []
    for c, rest in terms:
        num, den = split_rest(rest)
        lx, kind = atom_latex(num)
        dx = atom_latex(den)[0] if den != 1 else ""
        infos.append((c, dx, lx, kind))
    infos.sort(key=lambda t: kind_key(t[3]))
    if infos and infos[0][0] < 0:
        pos = next((i for i, t in enumerate(infos) if t[0] > 0), None)
        if pos is not None:
            infos.insert(0, infos.pop(pos))
    algebraic = all(t[3][0] in ("rational", "sqrt") and not t[1] for t in infos)
    if algebraic and len(infos) > 1:
        den = 1
        for c, *_ in infos:
            den = ilcm(den, c.q)
        # 全部の項が負なら符号を分数線の前に（分母が無いときは項ごとの符号のまま: -3-\sqrt{33}）
        neg = den != 1 and all(c < 0 for c, *_ in infos)
        parts = []
        for i, (c, _, lx, _) in enumerate(infos):
            m = int(c * den) * (-1 if neg else 1)
            s = coef_times(m, lx)
            if i > 0 and not s.startswith("-"):
                s = "+" + s
            parts.append(s)
        num = "".join(parts)
        body = num if den == 1 else r"\frac{%s}{%d}" % (num, den)
        return ("-" + body) if neg else body
    out = []
    for i, (c, dx, lx, _) in enumerate(infos):
        if c.q == 1 and not dx:
            s = coef_times(int(c), lx)
        else:
            p = abs(int(c.p))
            top = coef_times(p, lx) if lx else str(p)
            bottom = ("" if c.q == 1 else str(c.q)) + dx
            s = ("-" if c < 0 else "") + r"\frac{%s}{%s}" % (top, bottom)
        if i > 0 and not s.startswith("-"):
            s = "+" + s
        out.append(s)
    return "".join(out)


def fmt(v):
    v = S(v)
    if v.has(I):
        re, im = v.as_real_imag()
        re, im = re.simplify(), im.simplify()
        s = "" if re == 0 else fmt_real(re)
        if im != 0:
            if im == 1:
                t = "i"
            elif im == -1:
                t = "-i"
            else:
                body = fmt_real(im)
                t = body + (" i" if re_mod.search(r"\\[a-zA-Z]+$", body) else "i")
            if s and not t.startswith("-"):
                t = "+" + t
            s += t
        return s or "0"
    if v == 0:
        return "0"
    return fmt_real(v)


def fmt_matrix(m):
    rows = [" & ".join(fmt(m[i, j]) for j in range(m.cols)) for i in range(m.rows)]
    return r"\begin{pmatrix}" + r"\\".join(rows) + r"\end{pmatrix}"


# ---------------------------------------------------------------- 例題

CASES = []  # (area, input, latex, approx(str), source, mark, note)


def num_text(v):
    v = complex(N(v, 30))
    r = repr(float(v.real))
    if abs(v.imag) > 0:
        return f"{r};{float(v.imag)!r}"
    return r


def add(area, inp, problem, answer=None, latex=None, source="自作", mark="目標"):
    """problem: 問題の値（SymPy）。answer: 期待する答え（省略なら problem をそのまま答えの形にする）"""
    ans = problem if answer is None else answer
    a, b = complex(N(problem, 40)), complex(N(ans, 40))
    if abs(a - b) > 1e-25 * max(1.0, abs(b)):
        raise AssertionError(f"{inp}: 問題 {a} と答え {b} が違う")
    CASES.append((area, inp, latex if latex is not None else fmt(ans), num_text(ans), source, mark))


def add_symbolic(area, inp, integrand, answer_latex, answer, var=x, source="自作", mark="目標"):
    """不定積分: answer を微分して integrand と 3 点で一致"""
    d = diff(answer, var) - integrand
    for p in (Rational(3, 7), Rational(11, 5), Rational(5, 2)):
        val = complex(N(d.subs(var, p), 30))
        if abs(val) > 1e-20:
            raise AssertionError(f"{inp}: 微分して戻らない（{p} で {val}）")
    CASES.append((area, inp, answer_latex, "-", source, mark))


def add_not_exist(area, inp, reason, source="自作", mark="存在しない"):
    CASES.append((area, inp, "", "-", source, mark))


# 学校: 三角関数（π/12・π/8・π/5 の倍数）
for q in (12,):
    for p in range(0, 2 * q):
        ang = Rational(p, q) * pi
        angle_in = "0" if p == 0 else (f"{p}π/{q}" if p != 1 else f"π/{q}")
        for name, f in (("sin", sin), ("cos", cos), ("tan", tan)):
            if name == "tan" and cos(ang) == 0:
                continue
            add("学校", f"{name}({angle_in})", f(ang))
for p in (1, 3, 5, 7):
    ang = Rational(p, 8) * pi
    for name, f in (("sin", sin), ("cos", cos)):
        v = f(ang)
        s = 1 if v > 0 else -1
        inner = (v * v * 4).expand()  # 2 ± √2
        latex = (r"-" if s < 0 else "") + r"\frac{\sqrt{%s}}{2}" % fmt(inner)
        add("学校", f"{name}({'' if p == 1 else p}π/8)", v, latex=latex)
    add("学校", f"tan({'' if p == 1 else p}π/8)", tan(Rational(p, 8) * pi))
for p in (1, 2, 3, 4):
    ang = Rational(p, 5) * pi
    v = cos(ang).rewrite(sqrt)
    add("学校", f"cos({'' if p == 1 else p}π/5)", cos(ang), v)
for p in (1, 2):
    ang = Rational(p, 5) * pi
    v = sin(ang)
    inner = (v * v * 16).expand()
    add("学校", f"sin({'' if p == 1 else p}π/5)", v, latex=r"\frac{\sqrt{%s}}{4}" % fmt(inner))

# 学校: 逆三角関数
for inp, f, a in [
    ("arcsin(1/2)", asin, Rational(1, 2)), ("arcsin(sqrt(2)/2)", asin, sqrt(2) / 2), ("arcsin(sqrt(3)/2)", asin, sqrt(3) / 2),
    ("arcsin(1)", asin, 1), ("arcsin(-1/2)", asin, -Rational(1, 2)), ("arcsin(0)", asin, 0),
    ("arccos(1/2)", acos, Rational(1, 2)), ("arccos(-1/2)", acos, -Rational(1, 2)), ("arccos(0)", acos, 0),
    ("arccos(-1)", acos, -1), ("arccos(sqrt(2)/2)", acos, sqrt(2) / 2), ("arccos(-sqrt(3)/2)", acos, -sqrt(3) / 2),
    ("arctan(1)", atan, 1), ("arctan(sqrt(3))", atan, sqrt(3)), ("arctan(1/sqrt(3))", atan, 1 / sqrt(3)),
    ("arctan(-1)", atan, -1), ("arctan(0)", atan, 0), ("arctan(2-sqrt(3))", atan, 2 - sqrt(3)),
]:
    add("学校", inp, f(a))

# 学校: 根号・有理化・入れ子の根号
for m in (2, 8, 12, 18, 20, 27, 32, 45, 48, 50, 72, 75, 98, 200):
    add("学校", f"sqrt({m})", sqrt(m))
for inp, v, ans in [
    ("1/(sqrt(3)-1)", 1 / (sqrt(3) - 1), (1 + sqrt(3)) / 2),
    ("1/sqrt(2)", 1 / sqrt(2), sqrt(2) / 2),
    ("3/sqrt(3)", 3 / sqrt(3), sqrt(3)),
    ("2/(sqrt(5)+sqrt(3))", 2 / (sqrt(5) + sqrt(3)), sqrt(5) - sqrt(3)),
    ("1/(2-sqrt(3))", 1 / (2 - sqrt(3)), 2 + sqrt(3)),
    ("(sqrt(6)+sqrt(2))/sqrt(2)", (sqrt(6) + sqrt(2)) / sqrt(2), 1 + sqrt(3)),
    ("sqrt(3/4)", sqrt(Rational(3, 4)), sqrt(3) / 2),
    ("sqrt(1/2)", sqrt(Rational(1, 2)), sqrt(2) / 2),
    ("1/(1+sqrt(2)+sqrt(3))", 1 / (1 + sqrt(2) + sqrt(3)), (2 + sqrt(2) - sqrt(6)) / 4),
    ("sqrt(3+2sqrt(2))", sqrt(3 + 2 * sqrt(2)), 1 + sqrt(2)),
    ("sqrt(7-4sqrt(3))", sqrt(7 - 4 * sqrt(3)), 2 - sqrt(3)),
    ("sqrt(5+2sqrt(6))", sqrt(5 + 2 * sqrt(6)), sqrt(2) + sqrt(3)),
    ("sqrt(4+2sqrt(3))", sqrt(4 + 2 * sqrt(3)), 1 + sqrt(3)),
    ("sqrt(6-2sqrt(5))", sqrt(6 - 2 * sqrt(5)), sqrt(5) - 1),
    ("(1+sqrt(2))^2", (1 + sqrt(2)) ** 2, 3 + 2 * sqrt(2)),
    ("(sqrt(3)+sqrt(2))(sqrt(3)-sqrt(2))", (sqrt(3) + sqrt(2)) * (sqrt(3) - sqrt(2)), 1),
    ("(2+sqrt(3))^3", (2 + sqrt(3)) ** 3, 26 + 15 * sqrt(3)),
    ("sqrt(2)sqrt(8)", sqrt(2) * sqrt(8), 4),
    ("sqrt(3)/sqrt(12)", sqrt(3) / sqrt(12), Rational(1, 2)),
    ("sqrt(2)^2-2", sqrt(2) ** 2 - 2, 0),
    ("sqrt(12)sqrt(3)", sqrt(12) * sqrt(3), 6),
    ("(sqrt(5)-1)/2+1", (sqrt(5) - 1) / 2 + 1, (1 + sqrt(5)) / 2),
]:
    add("学校", inp, v, ans)

# 学校: 数・分数・小数・指数と対数
for inp, v, latex in [
    ("1/3+1/6", Rational(1, 2), None), ("2/3-3/4", Rational(-1, 12), None), ("(1/2)^10", Rational(1, 1024), None),
    ("2^10", Integer(1024), None), ("2^(-3)", Rational(1, 8), None), ("8^(2/3)", Integer(4), None),
    ("27^(-1/3)", Rational(1, 3), None), ("(4/9)^(3/2)", Rational(8, 27), None), ("16^(3/4)", Integer(8), None),
    ("0.1+0.2", Rational(3, 10), "0.3"), ("0.5·0.2", Rational(1, 10), "0.1"), ("1.5^2", Rational(9, 4), "2.25"),
    ("0.1/0.3", Rational(1, 3), r"\frac{1}{3}"), ("9.81·2", Rational(981, 50), "19.62"),
    ("@logbase(2) 8", Integer(3), None), ("@logbase(4) 8", Rational(3, 2), None), ("@logbase(3) (1/9)", Integer(-2), None),
    ("log(2)+log(5)", Integer(1), None), ("log(1000)", Integer(3), None), ("ln(e^3)", Integer(3), None),
    ("e^(2ln(3))", Integer(9), None), ("ln(1)", Integer(0), None), ("e^(ln(5))", Integer(5), None),
    ("@logbase(2) 3+@logbase(2) (4/3)", Integer(2), None), ("ln(e)/2", Rational(1, 2), None),
]:
    add("学校", inp, v, latex=latex)

# 複素数
for inp, v, ans in [
    ("(1+i)^2", (1 + I) ** 2, 2 * I), ("(1+i)(1-i)", (1 + I) * (1 - I), 2), ("1/(1+i)", 1 / (1 + I), Rational(1, 2) - I / 2),
    ("(1+i)^10", (1 + I) ** 10, 32 * I), ("e^(i π/3)", exp(I * pi / 3), Rational(1, 2) + sqrt(3) / 2 * I),
    ("ln(-1)", pi * I, pi * I), ("abs(3+4i)", Integer(5), 5), ("arg(1+i)", pi / 4, pi / 4),
    ("(1+sqrt(3)i)^6", (1 + sqrt(3) * I) ** 6, 64), ("e^(i π)+1", exp(I * pi) + 1, 0), ("i^2", Integer(-1), -1),
    ("(2+3i)/(1-i)", (2 + 3 * I) / (1 - I), Rational(-1, 2) + Rational(5, 2) * I), ("i^i", exp(-pi / 2), exp(-pi / 2)),
]:
    add("複素", inp, v, ans)

# 離散: 整数
for m in (10, 15, 20, 25, 30):
    add("離散", f"{m}!", factorial(m))
for a, b in ((10, 3), (20, 10), (30, 15), (50, 25), (60, 30), (100, 50), (40, 7), (64, 32)):
    add("離散", f"nCr({a},{b})", binomial(a, b))
for a, b, text in ((12, 18, None), (2**60 + 1, 3**40, "gcd(2^60+1, 3^40)"), (123456789, 987654321, None), (2**40, 6**20, "gcd(2^40, 6^20)"), (1001, 7007, None)):
    add("離散", text or f"gcd({a},{b})", Integer(igcd(a, b)))
for a, b in ((12, 18), (2**20, 3**10), (21, 6)):
    add("離散", f"lcm({a},{b})", Integer(ilcm(a, b)))
for inp, v in [("2^64", 2**64), ("3^40", 3**40), ("2^100", 2**100), ("7^30", 7**30), ("2^64-1", 2**64 - 1),
               ("mod(2^61-1, 1000)", (2**61 - 1) % 1000), ("mod(3^40, 7)", 3**40 % 7), ("mod(10^20+3, 9)", (10**20 + 3) % 9)]:
    add("離散", inp, Integer(v))
for inp, v in [("@sum(k,1,100) k^2", 338350), ("@sum(k,1,10) k^3", 3025), ("@sum(k,0,20) 2^k", 2**21 - 1),
               ("@sum(k,1,10) 1/(k(k+1))", Rational(10, 11)), ("@prod(k,1,10) k", 3628800), ("@sum(k,0,10) nCr(10,k)", 1024)]:
    add("離散", inp, S(v))

# 線形代数
for inp, m in [
    ("@det(⟦1,2;3,4⟧)", Matrix([[1, 2], [3, 4]])), ("@det(⟦1,2,3;4,5,6;7,8,10⟧)", Matrix([[1, 2, 3], [4, 5, 6], [7, 8, 10]])),
    ("@det(⟦2,0,1;1,3,2;1,1,1⟧)", Matrix([[2, 0, 1], [1, 3, 2], [1, 1, 1]])),
    ("@det(⟦1/2,1/3;1/4,1/5⟧)", Matrix([[Rational(1, 2), Rational(1, 3)], [Rational(1, 4), Rational(1, 5)]])),
    ("@det(⟦sqrt(2),1;1,sqrt(2)⟧)", Matrix([[sqrt(2), 1], [1, sqrt(2)]])),
    ("@det(⟦1,1,1,1;1,2,4,8;1,3,9,27;1,4,16,64⟧)", Matrix([[1, 1, 1, 1], [1, 2, 4, 8], [1, 3, 9, 27], [1, 4, 16, 64]])),
    ("@det(⟦0.5,1;2,0.1⟧)", Matrix([[Rational(1, 2), 1], [2, Rational(1, 10)]])),
]:
    latex = "-1.95" if "0.5" in inp else None
    add("線形", inp, m.det(), latex=latex)
for inp, v in [("@rank(⟦1,2;2,4⟧)", 1), ("@rank(⟦1,2,3;4,5,6;7,8,9⟧)", 2), ("@rank(⟦1,0;0,1⟧)", 2), ("@tr(⟦1,2;3,4⟧)", 5)]:
    add("線形", inp, Integer(v))
for inp, m in [
    ("@inv(⟦1,2;3,4⟧)", Matrix([[1, 2], [3, 4]])), ("@inv(⟦1,2,3;4,5,6;7,8,10⟧)", Matrix([[1, 2, 3], [4, 5, 6], [7, 8, 10]])),
    ("@inv(⟦2,1;1,1⟧)", Matrix([[2, 1], [1, 1]])),
]:
    inv = m.inv()
    CASES.append(("線形", inp, fmt_matrix(inv), "-", "自作", "目標"))
CASES.append(("線形", "⟦1,2;3,4⟧⟦0,1;1,0⟧", fmt_matrix(Matrix([[1, 2], [3, 4]]) * Matrix([[0, 1], [1, 0]])), "-", "自作", "目標"))


# 固有値・対角化・ジョルダン標準形・グラム・シュミット（文書 8 の LA1）。
# 答えの形は厳密値の道（core/src/exact/eigen.rs）と同じ決まりでここで作り、値は SymPy で確かめる:
# - 固有ベクトルは A − λI の簡約された行列の核の基底（自由変数の 1 つを 1。基底は一意）を、和の項の係数が互いに素な整数になる正の数で割ったもの
# - 固有値の並びは、実数（小さい順）→ 実数でないもの（実部、虚部の順）。固有多項式はモニックの因数を、次数の低い順・根の小さい順に
# - ジョルダン鎖は、ker N^j の基底から ker N^{j-1} と既に選んだ鎖に 1 次独立なものを順に頭に選ぶ（長い鎖から）。鎖ごとに係数をそろえる
lam = Symbol("lambda")


def poly_text_lam(coeffs):
    """core/src/exact/commands.rs の poly_text（係数は高い次数から。変数は λ）"""
    deg = len(coeffs) - 1
    out = ""
    for i, c in enumerate(coeffs):
        k = deg - i
        c = Rational(c)
        if c == 0:
            continue
        neg = c < 0
        a = -c if neg else c
        if neg:
            out += "-"
        elif out:
            out += "+"
        xs = "" if k == 0 else (r"\lambda" if k == 1 else r"\lambda^{%d}" % k)
        if a == 1 and k > 0:
            out += xs
        elif a.q == 1:
            out += f"{a.p}{xs}"
        else:
            top = xs if (k > 0 and a.p == 1) else f"{a.p}{xs}"
            out += r"\frac{%s}{%d}" % (top, a.q)
    return out or "0"


def canon(vs):
    """ベクトルの並びを、同じ正の有理数倍で、和の項の係数が互いに素な整数にする"""
    from sympy import expand as sp_expand, expand_complex, radsimp
    vs = [[sp_expand(radsimp(expand_complex(c))) for c in v] for v in vs]
    coeffs = [Rational(t.as_coeff_Mul()[0]) for v in vs for c in v for t in Add.make_args(c) if c != 0]
    den = 1
    for q in coeffs:
        den = ilcm(den, q.q)
    g = 0
    for q in coeffs:
        g = igcd(g, int(q * den))
    f = Rational(den, g) if g else Rational(1)
    return [[sp_expand(c * f) for c in v] for v in vs]


def root_key(r):
    v = complex(N(r, 30))
    return (abs(v.imag) > 1e-20, v.real, v.imag)


def eigen_groups(M):
    """[(モニックの既約因数, 重なり, [根（並び）])]、全部の根の並び [(根, 因数の番号)]"""
    from sympy import factor_list, Poly as SPoly, roots as sp_roots
    cp = M.charpoly(lam).as_expr()
    _, fl = factor_list(cp, lam)
    groups = []
    for f, k in fl:
        fp = SPoly(f, lam).monic()
        rs = sorted(sp_roots(fp.as_expr(), lam).keys(), key=root_key)
        assert len(rs) == fp.degree() <= 2, (M, f)
        groups.append((fp, k, rs))
    order = sorted([(r, gi) for gi, g in enumerate(groups) for r in g[2]], key=lambda t: root_key(t[0]))
    return groups, order


def charpoly_latex(groups):
    def key(g):
        return (g[0].degree(), complex(N(g[2][0], 30)).real)
    gs = sorted(groups, key=key)
    one = len(gs) == 1 and gs[0][1] == 1
    out = ""
    for fp, k, rs in gs:
        if fp.degree() == 1:
            r = -fp.all_coeffs()[1]
            body = r"\lambda" if r == 0 else (r"\lambda-" + fmt(r) if r > 0 else r"\lambda+" + fmt(-r))
        else:
            body = poly_text_lam(fp.all_coeffs())
        paren = not one and sum(1 for c in fp.all_coeffs() if c != 0) > 1
        b = r"\left(%s\right)" % body if paren else body
        out += (b + "^{%d}" % k) if k > 1 else b
    return out


def col(v):
    return r"\begin{pmatrix}" + r"\\".join(fmt(c) for c in v) + r"\end{pmatrix}"


def eigvecs(M, r):
    from sympy import simplify, eye as sp_eye
    ns = (M - r * sp_eye(M.rows)).applyfunc(simplify).nullspace(simplify=True)
    out = []
    for v in ns:
        c = canon([[simplify(x) for x in v]])[0]
        assert (M * Matrix(c) - r * Matrix(c)).applyfunc(simplify) == zeros(M.rows, 1)
        out.append(c)
    return out


def mult_text(k):
    return r"\ \left(\text{重複度 }%d\right)" % k if k > 1 else ""


def eig_case(inp, M, latex=None):
    from sympy import zeros as _z
    groups, order = eigen_groups(M)
    # 固有値の重なりが SymPy と同じ
    ev = M.eigenvals()
    for fp, k, rs in groups:
        for r in rs:
            assert any(abs(complex(N(r - e, 30))) < 1e-25 and m == k for e, m in ev.items()), (inp, r)
    items = [r"\text{固有多項式 }" + charpoly_latex(groups)]
    for r, gi in order:
        vs = eigvecs(M, r)
        items.append(r"\lambda=%s%s:\ %s" % (fmt(r), mult_text(groups[gi][1]), r",\ ".join(col(v) for v in vs)))
    CASES.append(("固有値", inp, latex or r",\quad ".join(items), "-", "自作", "目標"))


def mat_tex(rows):
    return r"\begin{pmatrix}" + r"\\".join(" & ".join(fmt(c) for c in row) for row in rows) + r"\end{pmatrix}"


def similar_check(inp, M, P, J):
    from sympy import simplify
    P, J = Matrix(P), Matrix(J)
    assert simplify(P.det()) != 0, inp
    assert (M * P - P * J).applyfunc(simplify) == zeros(M.rows, M.cols), inp


def diag_case(inp, M):
    groups, order = eigen_groups(M)
    for r, gi in order:
        k = groups[gi][1]
        if len(eigvecs(M, r)) < k:
            assert not M.is_diagonalizable(), inp
            latex = r"\text{対角化できません（固有値 }\lambda=%s\text{ の重複度は %d、固有ベクトルは %d 本）}" % (fmt(r), k, len(eigvecs(M, r)))
            CASES.append(("固有値", inp, latex, "-", "自作", "目標"))
            return
    cols, ds = [], []
    for r, gi in order:
        for v in eigvecs(M, r):
            cols.append(v)
            ds.append(r)
    n = M.rows
    P = [[cols[j][i] for j in range(n)] for i in range(n)]
    D = [[ds[i] if i == j else 0 for j in range(n)] for i in range(n)]
    similar_check(inp, M, P, D)
    CASES.append(("固有値", inp, r"P=%s,\ D=%s\quad\left(P^{-1}AP=D\right)" % (mat_tex(P), mat_tex(D)), "-", "自作", "目標"))


def jordan_case(inp, M):
    from sympy import eye as sp_eye
    n = M.rows
    groups, order = eigen_groups(M)
    chains_of = {}
    for r, gi in order:
        k = groups[gi][1]
        Nm = M - r * sp_eye(n)
        kers = [[], Nm.nullspace()]
        Pw = Nm
        while len(kers[-1]) < k:
            Pw = Pw * Nm
            kers.append(Pw.nullspace())
        m = len(kers) - 1
        levels = [[] for _ in range(m + 1)]
        chains = []
        for j in range(m, 0, -1):
            base = list(kers[j - 1]) + levels[j]
            rk = Matrix.hstack(*base).rank() if base else 0
            for c in kers[j]:
                trial = base + [c]
                r2 = Matrix.hstack(*trial).rank()
                if r2 <= rk:
                    continue
                base, rk = trial, r2
                chain = [c]
                for _ in range(1, j):
                    chain.append(Nm * chain[-1])
                for t in range(1, j):
                    levels[j - t].append(chain[t])
                chain.reverse()
                chains.append(canon([list(v) for v in chain]))
        chains_of[r] = chains
    cols, blocks = [], []
    for r, gi in order:
        for ch in chains_of[r]:
            cols.extend(ch)
            blocks.append((r, len(ch)))
    P = [[cols[j][i] for j in range(n)] for i in range(n)]
    J = [[0] * n for _ in range(n)]
    at = 0
    for r, ln in blocks:
        for t in range(ln):
            J[at + t][at + t] = r
            if t + 1 < ln:
                J[at + t][at + t + 1] = 1
        at += ln
    similar_check(inp, M, P, J)
    # 細胞の大きさが SymPy のジョルダン標準形と同じ
    _, Js = M.jordan_form()
    def cells(Jm):
        out, i = [], 0
        while i < n:
            ln = 1
            while i + ln < n and Jm[i + ln - 1, i + ln] == 1:
                ln += 1
            out.append((str(Jm[i, i]), ln))
            i += ln
        return sorted(out)
    assert cells(Matrix(J)) == cells(Js), (inp, cells(Matrix(J)), cells(Js))
    CASES.append(("固有値", inp, r"P=%s,\ J=%s\quad\left(P^{-1}AP=J\right)" % (mat_tex(P), mat_tex(J)), "-", "自作", "目標"))


def gs_case(inp, vecs):
    from sympy import GramSchmidt, factorint, simplify
    ws, units, parts = [], [], []
    for v in vecs:
        v = [Rational(c) for c in v]
        u = list(v)
        for w in ws:
            c = Rational(sum(a * b for a, b in zip(v, w)), sum(b * b for b in w))
            u = [a - c * b for a, b in zip(u, w)]
        w = canon([u])[0]
        n2 = int(sum(c * c for c in w))
        a, b = 1, 1
        for p_, e in factorint(n2).items():
            a *= p_ ** (e // 2)
            b *= p_ ** (e % 2)
        coef = "" if (a, b) == (1, 1) else (r"\frac{1}{%d}" % a if b == 1 else (r"\frac{1}{\sqrt{%d}}" % b if a == 1 else r"\frac{1}{%d\sqrt{%d}}" % (a, b)))
        parts.append(r"\mathbf{u}_{%d}=%s%s" % (len(ws) + 1, coef, col(w)))
        ws.append(w)
        units.append([c / sqrt(n2) for c in w])
    theirs = GramSchmidt([Matrix(v) for v in vecs], True)
    for u, t in zip(units, theirs):
        assert all(simplify(a - b) == 0 for a, b in zip(u, t)), inp
    CASES.append(("固有値", inp, r",\ ".join(parts), "-", "自作", "目標"))


for inp, m in [
    ("@eig(⟦2,1;1,2⟧)", [[2, 1], [1, 2]]), ("@eig(⟦4,1;2,3⟧)", [[4, 1], [2, 3]]), ("@eig(⟦1,2;2,1⟧)", [[1, 2], [2, 1]]),
    ("@eig(⟦4,-2;1,1⟧)", [[4, -2], [1, 1]]), ("@eig(⟦5,-2;-2,2⟧)", [[5, -2], [-2, 2]]), ("@eig(⟦1,2;3,4⟧)", [[1, 2], [3, 4]]),
    ("@eig(⟦1,1;1,0⟧)", [[1, 1], [1, 0]]), ("@eig(⟦0,-1;1,0⟧)", [[0, -1], [1, 0]]), ("@eig(⟦1,-1;1,1⟧)", [[1, -1], [1, 1]]),
    ("@eig(⟦3,1;-1,1⟧)", [[3, 1], [-1, 1]]), ("@eig(⟦0,1,0;0,0,1;2,-5,4⟧)", [[0, 1, 0], [0, 0, 1], [2, -5, 4]]),
    ("@eig(⟦2,0,0;0,2,0;0,0,3⟧)", [[2, 0, 0], [0, 2, 0], [0, 0, 3]]), ("@eig(⟦2,-1,0;-1,2,-1;0,-1,2⟧)", [[2, -1, 0], [-1, 2, -1], [0, -1, 2]]),
    ("@eig(⟦0,0,1;1,0,0;0,1,0⟧)", [[0, 0, 1], [1, 0, 0], [0, 1, 0]]), ("@eig(⟦3,-2,4;-2,6,2;4,2,3⟧)", [[3, -2, 4], [-2, 6, 2], [4, 2, 3]]),
    ("@eig(⟦1,1,1;1,1,1;1,1,1⟧)", [[1, 1, 1], [1, 1, 1], [1, 1, 1]]), ("@eig(⟦1,1,0;0,1,0;0,0,2⟧)", [[1, 1, 0], [0, 1, 0], [0, 0, 2]]),
    ("@eig(⟦2,0,0,0;0,3,4,0;0,4,9,0;0,0,0,1⟧)", [[2, 0, 0, 0], [0, 3, 4, 0], [0, 4, 9, 0], [0, 0, 0, 1]]),
]:
    eig_case(inp, Matrix(m))
# 小数の入力は小数で出す（固有ベクトルの成分は整数）
eig_case("@eig(⟦0.5,0.2;0.5,0.8⟧)", Matrix([[Rational(1, 2), Rational(1, 5)], [Rational(1, 2), Rational(4, 5)]]),
         latex=r"\text{固有多項式 }\left(\lambda-0.3\right)\left(\lambda-1\right),\quad \lambda=0.3:\ \begin{pmatrix}-1\\1\end{pmatrix},\quad \lambda=1:\ \begin{pmatrix}2\\5\end{pmatrix}")
for inp, m in [
    ("@diagonalize(⟦4,1;2,3⟧)", [[4, 1], [2, 3]]), ("@diagonalize(⟦2,1;1,2⟧)", [[2, 1], [1, 2]]), ("@diagonalize(⟦1,2;2,1⟧)", [[1, 2], [2, 1]]),
    ("@diagonalize(⟦0,-1;1,0⟧)", [[0, -1], [1, 0]]), ("@diagonalize(⟦1,1;1,0⟧)", [[1, 1], [1, 0]]), ("@diagonalize(⟦3,1;-1,1⟧)", [[3, 1], [-1, 1]]),
    ("@diagonalize(⟦3,-2,4;-2,6,2;4,2,3⟧)", [[3, -2, 4], [-2, 6, 2], [4, 2, 3]]), ("@diagonalize(⟦1,1,1;1,1,1;1,1,1⟧)", [[1, 1, 1], [1, 1, 1], [1, 1, 1]]),
    ("@diagonalize(⟦0,1,0;0,0,1;2,-5,4⟧)", [[0, 1, 0], [0, 0, 1], [2, -5, 4]]), ("@diagonalize(⟦2,0,0;0,2,0;0,0,3⟧)", [[2, 0, 0], [0, 2, 0], [0, 0, 3]]),
]:
    diag_case(inp, Matrix(m))
for inp, m in [
    ("@jordan(⟦0,1,0;0,0,1;2,-5,4⟧)", [[0, 1, 0], [0, 0, 1], [2, -5, 4]]), ("@jordan(⟦3,1;-1,1⟧)", [[3, 1], [-1, 1]]),
    ("@jordan(⟦1,1;0,1⟧)", [[1, 1], [0, 1]]), ("@jordan(⟦2,-1;1,0⟧)", [[2, -1], [1, 0]]), ("@jordan(⟦1,1,0;0,1,1;0,0,1⟧)", [[1, 1, 0], [0, 1, 1], [0, 0, 1]]),
    ("@jordan(⟦2,1,0;0,2,0;0,0,3⟧)", [[2, 1, 0], [0, 2, 0], [0, 0, 3]]), ("@jordan(⟦4,1;2,3⟧)", [[4, 1], [2, 3]]),
    ("@jordan(⟦5,4,2,1;0,1,-1,-1;-1,-1,3,0;1,1,-1,2⟧)", [[5, 4, 2, 1], [0, 1, -1, -1], [-1, -1, 3, 0], [1, 1, -1, 2]]),
    ("@jordan(⟦2,1,0,0;0,2,0,0;0,0,2,1;0,0,0,2⟧)", [[2, 1, 0, 0], [0, 2, 0, 0], [0, 0, 2, 1], [0, 0, 0, 2]]),
    ("@jordan(⟦1,-1,0;-1,2,-1;0,-1,1⟧)", [[1, -1, 0], [-1, 2, -1], [0, -1, 1]]),
]:
    jordan_case(inp, Matrix(m))
for inp, vs in [
    ("@gramschmidt((1,1,0),(1,0,1),(0,1,1))", [(1, 1, 0), (1, 0, 1), (0, 1, 1)]), ("@gramschmidt((1,0),(1,1))", [(1, 0), (1, 1)]),
    ("@gramschmidt((3,4),(1,0))", [(3, 4), (1, 0)]), ("@gramschmidt((1,2,2),(2,1,-2),(1,1,1))", [(1, 2, 2), (2, 1, -2), (1, 1, 1)]),
    ("@gramschmidt((1,1,1),(0,1,1),(0,0,1))", [(1, 1, 1), (0, 1, 1), (0, 0, 1)]), ("@gramschmidt((1,0,1),(1,1,0))", [(1, 0, 1), (1, 1, 0)]),
    ("@gramschmidt((1,1,0,0),(1,0,1,0),(1,0,0,1))", [(1, 1, 0, 0), (1, 0, 1, 0), (1, 0, 0, 1)]),
]:
    gs_case(inp, vs)

# 微分係数
for inp, f, a in [
    ("@at(@diff(x) x^3 sin(x), x, π/3)", x**3 * sin(x), pi / 3), ("@at(@diff(x) sqrt(x), x, 4)", sqrt(x), 4),
    ("@at(@diff(x) ln(x), x, 2)", log(x), 2), ("@at(@diff(x) arctan(x), x, 1)", atan(x), 1),
    ("@at(@diff(x) x^x, x, 1)", x**x, 1), ("@at(@diff(x) sin(2x), x, π/6)", sin(2 * x), pi / 6),
    ("@at(@diff(x) tan(x), x, π/4)", tan(x), pi / 4), ("@at(@diff(x) 1/(1+x^2), x, 1)", 1 / (1 + x**2), 1),
    ("@at(@diff(x) e^(x^2), x, 1)", exp(x**2), 1), ("@at(@diff(x) x^2 e^x, x, 1)", x**2 * exp(x), 1),
    ("@at(@diff(x) cos(x)^2, x, π/4)", cos(x) ** 2, pi / 4), ("@at(@diff(x) (x^2+1)/(x-1), x, 2)", (x**2 + 1) / (x - 1), 2),
    ("@at(@diff(x) arcsin(x), x, 1/2)", asin(x), Rational(1, 2)), ("@at(@diff(x) sqrt(1+x^2), x, sqrt(3))", sqrt(1 + x**2), sqrt(3)),
    ("@at(@diff(x) x ln(x), x, e)", x * log(x), E), ("@at(@diff(x) sin(x)/x, x, π)", sin(x) / x, pi),
    ("@at(@diff(x) 2^x, x, 0)", 2**x, 0), ("@at(@diff(x) (2x+1)^5, x, 0)", (2 * x + 1) ** 5, 0),
]:
    add("微分", inp, diff(f, x).subs(x, a).simplify())
for inp, f, a in [("@at(@diff(x, 2) e^x cos(x), x, 0)", exp(x) * cos(x), 0), ("@at(@diff(x, 2) x^4, x, 1)", x**4, 1),
                  ("@at(@diff(x, 3) sin(x), x, π)", sin(x), pi)]:
    order = int(inp.split("@diff(x, ")[1][0])
    add("微分", inp, diff(f, x, order).subs(x, a).simplify())
add_not_exist("微分", "@at(@diff(x) abs(x), x, 0)", "左右の微分係数が −1 と 1")

# 極限
for inp, f, at, side in [
    ("@lim(x,0) sin(x)/x", sin(x) / x, 0, "+-"), ("@lim(x,0) (1-cos(x))/x^2", (1 - cos(x)) / x**2, 0, "+-"),
    ("@lim(x,0) (tan(x)-sin(x))/x^3", (tan(x) - sin(x)) / x**3, 0, "+-"), ("@lim(x,0) (e^x-1)/x", (exp(x) - 1) / x, 0, "+-"),
    ("@lim(x,0) ln(1+x)/x", log(1 + x) / x, 0, "+-"), ("@lim(x,∞) (1+1/x)^x", (1 + 1 / x) ** x, oo, "-"),
    ("@lim(x,∞) (1+2/x)^x", (1 + 2 / x) ** x, oo, "-"), ("@lim(x,∞) x^(1/x)", x ** (1 / x), oo, "-"),
    ("@lim(x,∞) (sqrt(x^2+x)-x)", sqrt(x**2 + x) - x, oo, "-"), ("@lim(x,1) (x^2-1)/(x-1)", (x**2 - 1) / (x - 1), 1, "+-"),
    ("@lim(x,∞) (2x^2+1)/(x^2-1)", (2 * x**2 + 1) / (x**2 - 1), oo, "-"), ("@lim(x,0,1) x^x", x**x, 0, "+"),
    ("@lim(x,0,1) x ln(x)", x * log(x), 0, "+"), ("@lim(x,0) (e^x-1-x)/x^2", (exp(x) - 1 - x) / x**2, 0, "+-"),
    ("@lim(x,∞) x^100/e^x", x**100 / exp(x), oo, "-"), ("@lim(x,∞) ln(ln(x))/ln(x)", log(log(x)) / log(x), oo, "-"),
    ("@lim(x,0,1) sign(x)", Integer(1), 0, "+"), ("@lim(x,0) (1-cos(2x))/x^2", (1 - cos(2 * x)) / x**2, 0, "+-"),
    ("@lim(x,0) arcsin(x)/x", asin(x) / x, 0, "+-"), ("@lim(x,0) (sqrt(1+x)-1)/x", (sqrt(1 + x) - 1) / x, 0, "+-"),
    ("@lim(x,0) (x-sin(x))/x^3", (x - sin(x)) / x**3, 0, "+-"), ("@lim(x,∞) x sin(1/x)", x * sin(1 / x), oo, "-"),
    ("@lim(x,∞) (3x+1)/(2x-5)", (3 * x + 1) / (2 * x - 5), oo, "-"), ("@lim(x,2) (x^3-8)/(x-2)", (x**3 - 8) / (x - 2), 2, "+-"),
    ("@lim(x,0) sin(3x)/sin(5x)", sin(3 * x) / sin(5 * x), 0, "+-"), ("@lim(x,∞) (1-1/x)^x", (1 - 1 / x) ** x, oo, "-"),
    ("@lim(x,0) (e^(2x)-1)/(e^x-1)", (exp(2 * x) - 1) / (exp(x) - 1), 0, "+-"), ("@lim(x,0) tan(x)/x", tan(x) / x, 0, "+-"),
    ("@lim(x,∞) x(e^(1/x)-1)", x * (exp(1 / x) - 1), oo, "-"), ("@lim(x,0) (cos(x)-1)/(x sin(x))", (cos(x) - 1) / (x * sin(x)), 0, "+-"),
]:
    v = limit(f, x, at, side if side in "+-" and len(side) == 1 else "+-")
    add("極限", inp, v)
add("極限", "@lim(n,∞) ((@sum(k,1,n) 1/k) - ln(n))", EulerGamma)
add_not_exist("極限", "@lim(x,0) sin(1/x)", "振動")
add_not_exist("極限", "@lim(x,0) abs(x)/x", "左 −1、右 1")
add_not_exist("極限", "@lim(x,0) 1/x", "左右で ±∞")

# 無限級数（値）
for inp, term, lo, ans in [
    ("@sum(k,1,∞) 1/(k(k+1))", 1 / (k * (k + 1)), 1, 1), ("@sum(k,0,∞) k/2^k", k / 2**k, 0, 2),
    ("@sum(k,0,∞) 1/k!", 1 / factorial(k), 0, E), ("@sum(k,1,∞) 1/k^2", 1 / k**2, 1, pi**2 / 6),
    ("@sum(k,1,∞) (-1)^(k+1)/k", (-1) ** (k + 1) / k, 1, log(2)), ("@sum(k,1,∞) 1/k^4", 1 / k**4, 1, pi**4 / 90),
    ("@sum(k,1,∞) 1/(2k-1)^2", 1 / (2 * k - 1) ** 2, 1, pi**2 / 8), ("@sum(k,0,∞) (-1)^k/(2k+1)", (-1) ** k / (2 * k + 1), 0, pi / 4),
    ("@sum(k,1,∞) 1/(4k^2-1)", 1 / (4 * k**2 - 1), 1, Rational(1, 2)), ("@sum(k,0,∞) (-1)^k/(2k+1)^2", (-1) ** k / (2 * k + 1) ** 2, 0, Catalan),
    ("@sum(k,1,∞) 1/k^3", 1 / k**3, 1, zeta(3)), ("@sum(k,0,∞) k^2/3^k", k**2 / 3**k, 0, Rational(3, 2)),
    ("@sum(k,1,∞) 1/(k(k+2))", 1 / (k * (k + 2)), 1, Rational(3, 4)), ("@sum(k,1,∞) 1/2^k", 1 / 2**k, 1, 1),
    ("@sum(k,0,∞) (2/3)^k", Rational(2, 3) ** k, 0, 3), ("@sum(k,2,∞) 1/(k^2-1)", 1 / (k**2 - 1), 2, Rational(3, 4)),
    ("@sum(k,0,∞) (-1)^k/k!", (-1) ** k / factorial(k), 0, exp(-1)), ("@sum(k,1,∞) k/(k+1)!", k / factorial(k + 1), 1, 1),
    ("@sum(k,1,∞) 1/(k(k+1)(k+2))", 1 / (k * (k + 1) * (k + 2)), 1, Rational(1, 4)), ("@sum(k,1,∞) 1/k^6", 1 / k**6, 1, pi**6 / 945),
    ("@sum(k,1,∞) (-1)^(k+1)/k^2", (-1) ** (k + 1) / k**2, 1, pi**2 / 12), ("@sum(k,1,∞) 1/(2^k k)", 1 / (2**k * k), 1, log(2)),
    ("@sum(k,0,∞) 1/(2k+1)!", 1 / factorial(2 * k + 1), 0, (E - exp(-1)) / 2),
]:
    s = Sum(term, (k, lo, oo)).doit()
    add("級数", inp, s if not s.has(Sum) else ans, S(ans))
add_not_exist("級数", "@sum(k,1,∞) 1/k", "調和級数は発散")
add_not_exist("級数", "@sum(k,2,∞) 1/(k ln(k))", "積分判定法で発散")
add_not_exist("級数", "@sum(k,0,∞) (-1)^k", "項が 0 に近づかない")

# 定積分
for inp, f, a, b, latex in [
    ("@int(x,0,1) x^2", x**2, 0, 1, None), ("@int(x,0,π) sin(x)", sin(x), 0, pi, None), ("@int(x,1,e) 1/x", 1 / x, 1, E, None),
    ("@int(x,0,1) e^x", exp(x), 0, 1, None), ("@int(x,0,π/2) cos(x)", cos(x), 0, pi / 2, None),
    ("@int(x,0,1) 1/(1+x^2)", 1 / (1 + x**2), 0, 1, None), ("@int(x,0,1) x e^x", x * exp(x), 0, 1, None),
    ("@int(x,0,1) sqrt(1-x^2)", sqrt(1 - x**2), 0, 1, None), ("@int(x,0,1) 1/sqrt(1-x^2)", 1 / sqrt(1 - x**2), 0, 1, None),
    ("@int(x,0,π) x sin(x)", x * sin(x), 0, pi, None), ("@int(x,0,1) ln(x)", log(x), 0, 1, None),
    ("@int(x,0,∞) e^(-x)", exp(-x), 0, oo, None), ("@int(x,1,∞) 1/x^2", 1 / x**2, 1, oo, None),
    ("@int(x,0,∞) 1/(1+x^2)", 1 / (1 + x**2), 0, oo, None), ("@int(x,0,π) sin(x)^2", sin(x) ** 2, 0, pi, None),
    ("@int(x,1,2) ln(x)", log(x), 1, 2, r"2\ln 2-1"), ("@int(x,0,∞) e^(-x^2)", exp(-x**2), 0, oo, None),
    ("@int(x,0,∞) x^3 e^(-x)", x**3 * exp(-x), 0, oo, None), ("@int(x,-∞,∞) 1/(x^2+1)", 1 / (x**2 + 1), -oo, oo, None),
    ("@int(x,0,1) x^2(1-x)^3", x**2 * (1 - x) ** 3, 0, 1, None), ("@int(x,0,∞) 1/(1+x^4)", 1 / (1 + x**4), 0, oo, None),
    ("@int(x,0,∞) e^(-x) ln(x)", exp(-x) * log(x), 0, oo, None), ("@int(x,0,π/2) sin(x)^4 cos(x)^2", sin(x) ** 4 * cos(x) ** 2, 0, pi / 2, None),
    ("@int(x,0,1) 1/sqrt(x)", 1 / sqrt(x), 0, 1, None), ("@int(x,-∞,∞) 1/(x^2+2x+5)", 1 / (x**2 + 2 * x + 5), -oo, oo, None),
    ("@int(x,0,2π) 1/(2+cos(x))", 1 / (2 + cos(x)), 0, 2 * pi, None), ("@int(x,0,1) x/(1+x^2)", x / (1 + x**2), 0, 1, r"\frac{\ln 2}{2}"),
    ("@int(x,0,π/4) tan(x)", tan(x), 0, pi / 4, r"\frac{\ln 2}{2}"), ("@int(x,0,1) x^3", x**3, 0, 1, None),
    ("@int(x,-1,1) x^5 cos(x)", x**5 * cos(x), -1, 1, None), ("@int(x,0,2) abs(x-1)", Abs(x - 1), 0, 2, None),
]:
    v = integrate(f, (x, a, b))
    add("積分", inp, v, latex=latex)
add("積分", "@int(x,0,∞) sin(x)/x", pi / 2)
CASES.append(("積分", "@int(x,0,1) arctan(x)/x", "G", num_text(Catalan), "自作", "目標"))
add_not_exist("積分", "@int(x,-1,1) 1/x", "0 で発散")
add_not_exist("積分", "@int(x,1,∞) 1/x", "対数の速さで発散")

# 不定積分（答えの式。微分して戻ることを確かめる）
for inp, f, latex, ans in [
    ("@int(x) x^2", x**2, r"\frac{x^{3}}{3}+C", x**3 / 3),
    ("@int(x) 1/x", 1 / x, r"\ln\left|x\right|+C", log(x)),
    ("@int(x) e^(2x)", exp(2 * x), r"\frac{e^{2x}}{2}+C", exp(2 * x) / 2),
    ("@int(x) sin(3x)", sin(3 * x), r"-\frac{\cos 3x}{3}+C", -cos(3 * x) / 3),
    ("@int(x) x^2 e^x", x**2 * exp(x), r"\left(x^{2}-2x+2\right)e^{x}+C", (x**2 - 2 * x + 2) * exp(x)),
    ("@int(x) x ln(x)", x * log(x), r"\frac{x^{2}\ln x}{2}-\frac{x^{2}}{4}+C", x**2 * log(x) / 2 - x**2 / 4),
    ("@int(x) sin(x)^3", sin(x) ** 3, r"\frac{\cos^{3}x}{3}-\cos x+C", cos(x) ** 3 / 3 - cos(x)),
    ("@int(x) e^x sin(x)", exp(x) * sin(x), r"\frac{e^{x}\left(\sin x-\cos x\right)}{2}+C", exp(x) * (sin(x) - cos(x)) / 2),
    ("@int(x) sqrt(1-x^2)", sqrt(1 - x**2), r"\frac{x\sqrt{1-x^{2}}+\arcsin x}{2}+C", (x * sqrt(1 - x**2) + asin(x)) / 2),
    ("@int(x) 1/sqrt(x^2+1)", 1 / sqrt(x**2 + 1), r"\ln\left(x+\sqrt{x^{2}+1}\right)+C", log(x + sqrt(x**2 + 1))),
    ("@int(x) x sqrt(x^2+1)", x * sqrt(x**2 + 1), r"\frac{\left(x^{2}+1\right)^{\frac{3}{2}}}{3}+C", (x**2 + 1) ** Rational(3, 2) / 3),
    ("@int(x) 1/(x ln(x))", 1 / (x * log(x)), r"\ln\left|\ln x\right|+C", log(log(x))),
    ("@int(x) 1/(1+x^2)", 1 / (1 + x**2), r"\arctan x+C", atan(x)),
    ("@int(x) x/(x^2-3x+2)", x / (x**2 - 3 * x + 2), r"2\ln\left|x-2\right|-\ln\left|x-1\right|+C", 2 * log(x - 2) - log(x - 1)),
    ("@int(x) 1/(x^2-1)", 1 / (x**2 - 1), r"\frac{1}{2}\ln\left|\frac{x-1}{x+1}\right|+C", log((x - 1) / (x + 1)) / 2),
    ("@int(x) cos(x)^2", cos(x) ** 2, r"\frac{x}{2}+\frac{\sin 2x}{4}+C", x / 2 + sin(2 * x) / 4),
    ("@int(x) tan(x)", tan(x), r"-\ln\left|\cos x\right|+C", -log(cos(x))),
    ("@int(x) x cos(x)", x * cos(x), r"x\sin x+\cos x+C", x * sin(x) + cos(x)),
    ("@int(x) ln(x)", log(x), r"x\ln x-x+C", x * log(x) - x),
    ("@int(x) arctan(x)", atan(x), r"x\arctan x-\frac{\ln\left(x^{2}+1\right)}{2}+C", x * atan(x) - log(x**2 + 1) / 2),
]:
    add_symbolic("不定積分", inp, f, latex, ans)

# 追加の例題（教科書の型の数を変えたもの）
for area, inp, v, ans, latex in [
    ("学校", "sin(π/12)cos(π/12)", sin(pi / 12) * cos(pi / 12), Rational(1, 4), None),
    ("学校", "sin(π/8)^2", sin(pi / 8) ** 2, (2 - sqrt(2)) / 4, None),
    ("学校", "cos(π/12)^2-sin(π/12)^2", cos(pi / 12) ** 2 - sin(pi / 12) ** 2, sqrt(3) / 2, None),
    ("学校", "tan(π/12)+tan(5π/12)", tan(pi / 12) + tan(5 * pi / 12), 4, None),
    ("学校", "sin(π/3)+cos(π/6)", sin(pi / 3) + cos(pi / 6), sqrt(3), None),
    ("学校", "2sin(π/4)cos(π/4)", 2 * sin(pi / 4) * cos(pi / 4), 1, None),
    ("学校", "((sqrt(5)+1)/2)^2-(sqrt(5)+1)/2", ((sqrt(5) + 1) / 2) ** 2 - (sqrt(5) + 1) / 2, 1, None),
    ("学校", "sqrt(2)+sqrt(8)+sqrt(18)", sqrt(2) + sqrt(8) + sqrt(18), 6 * sqrt(2), None),
    ("学校", "sqrt(12)-sqrt(27)+sqrt(48)", sqrt(12) - sqrt(27) + sqrt(48), 3 * sqrt(3), None),
    ("学校", "(sqrt(3)-1)^2", (sqrt(3) - 1) ** 2, 4 - 2 * sqrt(3), None),
    ("学校", "1/(sqrt(2)-1)", 1 / (sqrt(2) - 1), 1 + sqrt(2), None),
    ("学校", "(sqrt(3)+1)/(sqrt(3)-1)", (sqrt(3) + 1) / (sqrt(3) - 1), 2 + sqrt(3), None),
    ("学校", "sqrt(2)/(sqrt(2)+1)", sqrt(2) / (sqrt(2) + 1), 2 - sqrt(2), None),
    ("学校", "3^(1/2)3^(3/2)", sqrt(3) * 3 ** Rational(3, 2), 9, None),
    ("学校", "2^(1/3)4^(1/3)", 2 ** Rational(1, 3) * 4 ** Rational(1, 3), 2, None),
    ("学校", "@logbase(2) 6-@logbase(2) 3", log(6, 2) - log(3, 2), 1, None),
    ("学校", "@logbase(9) 27", log(27, 9), Rational(3, 2), None),
    ("学校", "log(0.01)", Integer(-2), -2, None),
    ("学校", "ln(sqrt(e))", Rational(1, 2), Rational(1, 2), None),
    ("学校", "e^(ln(2)+ln(3))", Integer(6), 6, None),
    ("学校", "0.25+0.125", Rational(3, 8), Rational(3, 8), "0.375"),
    ("学校", "1.2·1.5", Rational(9, 5), Rational(9, 5), "1.8"),
    ("学校", "2.5/0.5", Integer(5), 5, "5"),
    ("学校", "0.3^2", Rational(9, 100), Rational(9, 100), "0.09"),
    ("学校", "1/8+0.125", Rational(1, 4), Rational(1, 4), "0.25"),
    ("離散", "12!/(5! 7!)", Integer(792), 792, None),
    ("離散", "nCr(10,5)+nCr(10,4)", Integer(462), 462, None),
    ("離散", "gcd(462,1071)", Integer(21), 21, None),
    ("離散", "2^31-1", Integer(2**31 - 1), 2**31 - 1, None),
    ("離散", "100!/98!", Integer(9900), 9900, None),
    ("離散", "@sum(k,1,50) k", Integer(1275), 1275, None),
    ("離散", "@prod(k,1,5) (2k-1)", Integer(945), 945, None),
    ("微分", "@at(@diff(x) x^3-3x, x, 1)", Integer(0), 0, None),
    ("微分", "@at(@diff(x) cos(x), x, π/3)", -sin(pi / 3), -sqrt(3) / 2, None),
    ("微分", "@at(@diff(x) e^(-x), x, 0)", Integer(-1), -1, None),
    ("極限", "@lim(x,0) (2^x-1)/x", log(2), log(2), None),
    ("極限", "@lim(x,∞) (x^2+3x)/(2x^2-1)", Rational(1, 2), Rational(1, 2), None),
    ("極限", "@lim(x,0) sin(x)^2/x^2", Integer(1), 1, None),
    ("積分", "@int(x,0,π) cos(x)^2", integrate(cos(x) ** 2, (x, 0, pi)), pi / 2, None),
    ("積分", "@int(x,0,1) 1/(1+x)", integrate(1 / (1 + x), (x, 0, 1)), log(2), None),
    ("積分", "@int(x,0,1) x^2 e^x", integrate(x**2 * exp(x), (x, 0, 1)), E - 2, None),
    ("級数", "@sum(k,1,∞) 1/(k(k+3))", Sum(1 / (k * (k + 3)), (k, 1, oo)).doit(), Rational(11, 18), None),
    ("級数", "@sum(k,0,∞) 1/4^k", Sum(Rational(1, 4) ** k, (k, 0, oo)).doit(), Rational(4, 3), None),
]:
    add(area, inp, v, S(ans), latex=latex)


# 方程式（文書 8 の SC1）。期待する答えの LaTeX は手で書き、解の値を SymPy で確かめる。
# 近似値の列は、有限個の解の値を空白で並べたもの（連立方程式は点の座標を順に）。一般解・解なし・恒等式は「-」
y_, z_ = symbols("y z")


from sympy import im as sp_im


def numeric_roots(e, u):
    """[−30, 30] で e の符号が変わる所を 30 桁で（極は除く）"""
    import mpmath
    mpmath.mp.dps = 40
    f = lambda t: complex(N(e.subs(u, t), 40))
    pts = [Rational(-30) + Rational(60, 3001) * k + Rational(1, 7919) for k in range(3002)]
    vals = []
    for t in pts:
        try:
            v = f(t)
            vals.append(v.real if abs(v.imag) < 1e-30 else None)
        except Exception:
            vals.append(None)
    out = []
    for a, b, fa, fb in zip(pts, pts[1:], vals, vals[1:]):
        if fa is None or fb is None or (fa > 0) == (fb > 0):
            continue
        lo, hi = a, b
        for _ in range(110):
            m = (lo + hi) / 2
            fm = f(m).real
            if (fm > 0) == (fa > 0):
                lo = m
            else:
                hi = m
        r = N((lo + hi) / 2, 30)
        if abs(f(r)) < 1e-20:
            out.append(r)
    return out


def eq_check(inp, eqs, unknowns, want, latex, families=None, mark="目標"):
    """want: 実数の解の並び（1 変数は値、連立は組）。families: 一般解 [(底, 周期)]。want が "all" なら恒等式"""
    from sympy import solveset, Interval, Reals, FiniteSet, Eq, nsolve
    if len(unknowns) == 1:
        u = unknowns[0]
        e = eqs[0]
        if want == "all":
            assert solveset(e, u, Reals) == Reals, inp
            CASES.append(("方程式", inp, latex, "-", "自作", mark))
            return
        if families:
            # 族の n = −2..2 が解で、[−10, 10] の解が全部どれかの族に入る
            for b, per in families:
                for nn in range(-2, 3):
                    assert abs(N(e.subs(u, b + nn * per), 30)) < 1e-25, (inp, b, nn)
            sols = solveset(e, u, Interval(-10, 10))
            assert sols == S.EmptySet or isinstance(sols, FiniteSet), (inp, sols)
            sols = [] if sols == S.EmptySet else list(sols)
            for r in sols:
                rv = N(r, 30)
                ok = any(abs(N((rv - b) / per, 30) - round(N((rv - b) / per, 30))) < 1e-20 for b, per in families)
                assert ok, (inp, r)
            CASES.append(("方程式", inp, latex, "-", "自作", mark))
            return
        sols = solveset(e, u, Reals)
        if not (sols == S.EmptySet or isinstance(sols, FiniteSet)):
            # 形が複雑な集合（3 次の根など）: 多項式なら実根を、それ以外は [−30, 30] で符号の変わる所を数値で
            from sympy import together, Poly
            numer, den = together(e).as_numer_denom()
            try:
                sols = FiniteSet(*[r for r in Poly(numer, u).real_roots() if N(den.subs(u, r)) != 0])
            except Exception:
                sols = FiniteSet(*numeric_roots(e, u))
        assert sols == S.EmptySet or isinstance(sols, FiniteSet), (inp, sols)
        sols = [] if sols == S.EmptySet else list(sols)
        # SymPy は対数をまとめて定義域の外の解（ln x + ln(x − 1) の −2）を出すことがあるので、元の式で確かめる
        def holds(r):
            v = N(e.subs(u, r), 30)
            return v.is_real is not False and abs(v) < 1e-25 and abs(N(sp_im(v), 30)) < 1e-25
        sols = [r for r in sols if holds(r)]
        for w in want:
            assert abs(N(e.subs(u, w), 30)) < 1e-25, (inp, w)
        got = sorted(N(r, 30) for r in sols)
        exp_ = sorted(N(w, 30) for w in want)
        assert len(got) == len(exp_) and all(abs(a - b) < 1e-25 for a, b in zip(got, exp_)), (inp, got, exp_)
        approx = " ".join(repr(float(v)) for v in exp_) if exp_ else "-"
        CASES.append(("方程式", inp, latex, approx, "自作", mark))
        return
    from sympy import solve as ssolve
    sols = ssolve(eqs, unknowns, dict=True)
    real = sorted(tuple(N(sol[v], 30) for v in unknowns) for sol in sols if all(N(sol[v]).is_real for v in unknowns))
    exp_ = sorted(tuple(N(c, 30) for c in w) for w in want)
    assert len(real) == len(exp_) and all(all(abs(a - b) < 1e-25 for a, b in zip(p, q)) for p, q in zip(real, exp_)), (inp, real, exp_)
    approx = " ".join(repr(float(c)) for w in exp_ for c in w) if exp_ else "-"
    CASES.append(("方程式", inp, latex, approx, "自作", mark))


X = [x]
XY = [x, y_]
for inp, e, want, latex in [
    ("2x + 3 = 7", 2 * x + 3 - 7, [2], "x=2"),
    ("x^2 = 2", x**2 - 2, [-sqrt(2), sqrt(2)], r"x=\pm\sqrt{2}"),
    ("x^2 - 5x + 6 = 0", x**2 - 5 * x + 6, [2, 3], r"x=2,\ 3"),
    ("x^2 + x - 1 = 0", x**2 + x - 1, [(-1 - sqrt(5)) / 2, (-1 + sqrt(5)) / 2], r"x=\frac{-1\pm\sqrt{5}}{2}"),
    ("x^2 - 2x - 1 = 0", x**2 - 2 * x - 1, [1 - sqrt(2), 1 + sqrt(2)], r"x=1\pm\sqrt{2}"),
    ("x^2 + 1 = 0", x**2 + 1, [], r"\text{実数の解はありません}"),
    ("x^2 - 4x + 4 = 0", x**2 - 4 * x + 4, [2], "x=2"),
    ("3x^2 - 2x - 1 = 0", 3 * x**2 - 2 * x - 1, [Rational(-1, 3), 1], r"x=-\frac{1}{3},\ 1"),
    ("x^3 - 6x^2 + 11x - 6 = 0", x**3 - 6 * x**2 + 11 * x - 6, [1, 2, 3], r"x=1,\ 2,\ 3"),
    ("x^3 = 8", x**3 - 8, [2], "x=2"),
    ("x^3 - 2 = 0", x**3 - 2, [cbrt(2)], r"x=\sqrt[3]{2}"),
    ("x^3 - 3x + 1 = 0", x**3 - 3 * x + 1, [-2 * cos(pi / 9), 2 * cos(4 * pi / 9), 2 * cos(2 * pi / 9)],
     r"x=-2\cos\left(\frac{\pi}{9}\right),\ 2\cos\left(\frac{4\pi}{9}\right),\ 2\cos\left(\frac{2\pi}{9}\right)"),
    ("x^3 + x + 1 = 0", x**3 + x + 1, [cbrt((sqrt(93) - 9) / 18) - cbrt((9 + sqrt(93)) / 18)],
     r"x=\sqrt[3]{\frac{\sqrt{93}-9}{18}}-\sqrt[3]{\frac{9+\sqrt{93}}{18}}"),
    ("x^3 - x = 0", x**3 - x, [-1, 0, 1], r"x=0,\ \pm1"),
    ("x^4 - 10x^2 + 1 = 0", x**4 - 10 * x**2 + 1, [-sqrt(2) - sqrt(3), sqrt(2) - sqrt(3), sqrt(3) - sqrt(2), sqrt(2) + sqrt(3)],
     r"x=\pm\left(\sqrt{3}-\sqrt{2}\right),\ \pm\left(\sqrt{2}+\sqrt{3}\right)"),
    ("x^4 - 5x^2 + 4 = 0", x**4 - 5 * x**2 + 4, [-2, -1, 1, 2], r"x=\pm1,\ \pm2"),
    ("x^4 = 16", x**4 - 16, [-2, 2], r"x=\pm2"),
    ("x^4 + x + 1 = 0", x**4 + x + 1, [], r"\text{実数の解はありません}"),
    ("x^4 - 4x^2 + 2 = 0", x**4 - 4 * x**2 + 2, [-sqrt(2 + sqrt(2)), -sqrt(2 - sqrt(2)), sqrt(2 - sqrt(2)), sqrt(2 + sqrt(2))],
     r"x=\pm\sqrt{2-\sqrt{2}},\ \pm\sqrt{2+\sqrt{2}}"),
    ("x^4 - 2x^2 - 3 = 0", x**4 - 2 * x**2 - 3, [-sqrt(3), sqrt(3)], r"x=\pm\sqrt{3}"),
    ("x^4 + 4x - 1 = 0", x**4 + 4 * x - 1, [-(sqrt(2) + sqrt(4 * sqrt(2) - 2)) / 2, (sqrt(4 * sqrt(2) - 2) - sqrt(2)) / 2],
     r"x=-\frac{\sqrt{2}+\sqrt{4\sqrt{2}-2}}{2},\ \frac{\sqrt{4\sqrt{2}-2}-\sqrt{2}}{2}"),
    ("(x - 1)(x + 2)(2x - 3) = 0", (x - 1) * (x + 2) * (2 * x - 3), [-2, 1, Rational(3, 2)], r"x=-2,\ 1,\ \frac{3}{2}"),
    ("1/x + 1/(x - 1) = 2", 1 / x + 1 / (x - 1) - 2, [(2 - sqrt(2)) / 2, (2 + sqrt(2)) / 2], r"x=\frac{2\pm\sqrt{2}}{2}"),
    ("(x^2 - 1)/(x - 1) = 0", (x**2 - 1) / (x - 1), [-1], "x=-1"),
    ("x/(x - 2) = 2/(x - 2)", x / (x - 2) - 2 / (x - 2), [], r"\text{実数の解はありません}"),
    ("(x^2 - 4)/(x + 2) = 1", (x**2 - 4) / (x + 2) - 1, [3], "x=3"),
    ("sqrt(x + 2) = x", sqrt(x + 2) - x, [2], "x=2"),
    ("sqrt(2x + 3) = x", sqrt(2 * x + 3) - x, [3], "x=3"),
    ("sqrt(x) + sqrt(x - 5) = 5", sqrt(x) + sqrt(x - 5) - 5, [9], "x=9"),
    ("sqrt(x + 3) - sqrt(x) = 1", sqrt(x + 3) - sqrt(x) - 1, [1], "x=1"),
    ("sqrt(x - 1) = -2", sqrt(x - 1) + 2, [], r"\text{実数の解はありません}"),
    ("x^(1/3) = 2", cbrt(x) - 2, [8], "x=8"),
    ("abs(x - 1) = 2", Abs(x - 1) - 2, [-1, 3], r"x=-1,\ 3"),
    ("abs(2x + 1) = x + 4", Abs(2 * x + 1) - x - 4, [Rational(-5, 3), 3], r"x=-\frac{5}{3},\ 3"),
    ("2^x = 8", 2**x - 8, [3], "x=3"),
    ("2^x = 5", 2**x - 5, [log(5) / log(2)], r"x=\frac{\ln 5}{\ln 2}"),
    ("e^x = 3", exp(x) - 3, [log(3)], r"x=\ln 3"),
    ("e^(2x) - 3e^x + 2 = 0", exp(2 * x) - 3 * exp(x) + 2, [0, log(2)], r"x=0,\ \ln 2"),
    ("4^x - 3·2^x + 2 = 0", 4**x - 3 * 2**x + 2, [0, 1], r"x=0,\ 1"),
    ("9^x - 4·3^x + 3 = 0", 9**x - 4 * 3**x + 3, [0, 1], r"x=0,\ 1"),
    ("3^(2x - 1) = 27", 3 ** (2 * x - 1) - 27, [2], "x=2"),
    ("2^(x^2) = 16", 2 ** (x**2) - 16, [-2, 2], r"x=\pm2"),
    ("2^x = 3^(x - 1)", 2**x - 3 ** (x - 1), [log(3) / (log(3) - log(2))], r"x=\frac{\ln 3}{\ln 3-\ln 2}"),
    ("ln(x) = 2", log(x) - 2, [exp(2)], "x=e^{2}"),
    ("log(x) = 3", log(x, 10) - 3, [1000], "x=1000"),
    ("ln(x) + ln(x - 1) = ln(6)", log(x) + log(x - 1) - log(6), [3], "x=3"),
    ("log(x) + log(x - 3) = 1", log(x, 10) + log(x - 3, 10) - 1, [5], "x=5"),
    ("2ln(x) = ln(4)", 2 * log(x) - log(4), [2], "x=2"),
    ("ln(x^2) = 0", log(x**2), [-1, 1], r"x=\pm1"),
    ("@logbase(2) x = 5", log(x, 2) - 5, [32], "x=32"),
    ("x + 1 = x", Integer(1), [], r"\text{実数の解はありません}"),
]:
    eq_check(inp, [e], X, want, latex)
eq_check("x^2 = x^2", [x**2 - x**2], X, "all", r"x\in\mathbb{R}")

P2 = 2 * pi
for inp, e, fams, latex in [
    ("sin(x) = 1/2", sin(x) - Rational(1, 2), [(pi / 6, P2), (5 * pi / 6, P2)], r"x=\frac{\pi}{6}+2\pi n,\ \frac{5\pi}{6}+2\pi n\quad\left(n\in\mathbb{Z}\right)"),
    ("cos(x) = -1/2", cos(x) + Rational(1, 2), [(2 * pi / 3, P2), (4 * pi / 3, P2)], r"x=\frac{2\pi}{3}+2\pi n,\ \frac{4\pi}{3}+2\pi n\quad\left(n\in\mathbb{Z}\right)"),
    ("tan(x) = 1", tan(x) - 1, [(pi / 4, pi)], r"x=\frac{\pi}{4}+\pi n\quad\left(n\in\mathbb{Z}\right)"),
    ("tan(x) = sqrt(3)", tan(x) - sqrt(3), [(pi / 3, pi)], r"x=\frac{\pi}{3}+\pi n\quad\left(n\in\mathbb{Z}\right)"),
    ("sin(x) = 0", sin(x), [(0, pi)], r"x=\pi n\quad\left(n\in\mathbb{Z}\right)"),
    ("cos(2x) = 1/2", cos(2 * x) - Rational(1, 2), [(pi / 6, pi), (5 * pi / 6, pi)], r"x=\frac{\pi}{6}+\pi n,\ \frac{5\pi}{6}+\pi n\quad\left(n\in\mathbb{Z}\right)"),
    ("2cos(x)^2 - cos(x) - 1 = 0", 2 * cos(x) ** 2 - cos(x) - 1, [(0, 2 * pi / 3)], r"x=\frac{2\pi n}{3}\quad\left(n\in\mathbb{Z}\right)"),
    ("2sin(x)^2 + 3sin(x) + 1 = 0", 2 * sin(x) ** 2 + 3 * sin(x) + 1, [(7 * pi / 6, P2), (3 * pi / 2, P2), (11 * pi / 6, P2)],
     r"x=\frac{7\pi}{6}+2\pi n,\ \frac{3\pi}{2}+2\pi n,\ \frac{11\pi}{6}+2\pi n\quad\left(n\in\mathbb{Z}\right)"),
    ("cos(x)^2 + sin(x) = 1", cos(x) ** 2 + sin(x) - 1, [(0, pi), (pi / 2, P2)], r"x=\pi n,\ \frac{\pi}{2}+2\pi n\quad\left(n\in\mathbb{Z}\right)"),
    ("sin(x - π/3) = 1", sin(x - pi / 3) - 1, [(5 * pi / 6, P2)], r"x=\frac{5\pi}{6}+2\pi n\quad\left(n\in\mathbb{Z}\right)"),
    ("x sin(x) = 0", x * sin(x), [(0, pi)], r"x=\pi n\quad\left(n\in\mathbb{Z}\right)"),
    ("sin(x) = cos(x)", sin(x) - cos(x), [(pi / 4, pi)], r"x=\frac{\pi}{4}+\pi n\quad\left(n\in\mathbb{Z}\right)"),
]:
    eq_check(inp, [e], X, None, latex, families=fams)
eq_check("sin(x) = 2", [sin(x) - 2], X, [], r"\text{実数の解はありません}")

for inp, eqs, want, latex in [
    ("x^2 + y^2 = 1, y = x", [x**2 + y_**2 - 1, y_ - x], [(-sqrt(2) / 2, -sqrt(2) / 2), (sqrt(2) / 2, sqrt(2) / 2)],
     r"\left(x,y\right)=\left(-\frac{\sqrt{2}}{2},-\frac{\sqrt{2}}{2}\right),\ \left(\frac{\sqrt{2}}{2},\frac{\sqrt{2}}{2}\right)"),
    ("x + y = 3, x y = 2", [x + y_ - 3, x * y_ - 2], [(1, 2), (2, 1)], r"\left(x,y\right)=\left(1,2\right),\ \left(2,1\right)"),
    ("x^2 + y^2 = 5, x - y = 1", [x**2 + y_**2 - 5, x - y_ - 1], [(-1, -2), (2, 1)], r"\left(x,y\right)=\left(-1,-2\right),\ \left(2,1\right)"),
    ("x^2 + y^2 = 4, x^2 - y = 2", [x**2 + y_**2 - 4, x**2 - y_ - 2], [(-sqrt(3), 1), (0, -2), (sqrt(3), 1)],
     r"\left(x,y\right)=\left(-\sqrt{3},1\right),\ \left(0,-2\right),\ \left(\sqrt{3},1\right)"),
    ("y = x^2, y = 2x + 3", [y_ - x**2, y_ - 2 * x - 3], [(-1, 1), (3, 9)], r"\left(x,y\right)=\left(-1,1\right),\ \left(3,9\right)"),
]:
    eq_check(inp, eqs, XY, want, latex)
eq_check("x + y + z = 6, x - y = 1, z = 2", [x + y_ + z_ - 6, x - y_ - 1, z_ - 2], [x, y_, z_], [(Rational(5, 2), Rational(3, 2), 2)],
         r"\left(x,y,z\right)=\left(\frac{5}{2},\frac{3}{2},2\right)")


# 命令（文書 8 の SC2）。答えの LaTeX は手で書き、値が中身と同じことを SymPy で確かめる（文字の式は 3 点で）
def cmd_check(inp, inner, shaped, latex, mark="目標"):
    from sympy import expand as sp_expand
    for pt in [Rational(3, 7), Rational(-5, 3), Rational(11, 4)]:
        a, b = N(inner.subs(x, pt), 30), N(shaped.subs(x, pt), 30)
        assert abs(a - b) < 1e-25 * max(1, abs(a)), (inp, pt, a, b)
    CASES.append(("命令", inp, latex, "-" if inner.free_symbols else num_text(inner), "自作", mark))


for inp, inner, shaped, latex in [
    ("@factor(x^4 - 1)", x**4 - 1, (x - 1) * (x + 1) * (x**2 + 1), r"\left(x-1\right)\left(x+1\right)\left(x^{2}+1\right)"),
    ("@factor(x^2 - 5x + 6)", x**2 - 5 * x + 6, (x - 3) * (x - 2), r"\left(x-3\right)\left(x-2\right)"),
    ("@factor(2x^2 - 8)", 2 * x**2 - 8, 2 * (x - 2) * (x + 2), r"2\left(x-2\right)\left(x+2\right)"),
    ("@factor(x^3 - 1)", x**3 - 1, (x - 1) * (x**2 + x + 1), r"\left(x-1\right)\left(x^{2}+x+1\right)"),
    ("@factor(x^4 + 4)", x**4 + 4, (x**2 - 2 * x + 2) * (x**2 + 2 * x + 2), r"\left(x^{2}-2x+2\right)\left(x^{2}+2x+2\right)"),
    ("@factor(x^2 - 2x + 1)", x**2 - 2 * x + 1, (x - 1) ** 2, r"\left(x-1\right)^{2}"),
    ("@factor(x^3 + x)", x**3 + x, x * (x**2 + 1), r"x\left(x^{2}+1\right)"),
    ("@factor(x^2 + 1)", x**2 + 1, x**2 + 1, r"x^{2}+1"),
    ("@factor((x^2 - 1)/(x^2 + 2x + 1))", (x**2 - 1) / (x**2 + 2 * x + 1), (x - 1) / (x + 1), r"\frac{x-1}{x+1}"),
    ("@factor(2^32 + 1)", Integer(2**32 + 1), Integer(641 * 6700417), r"641\cdot 6700417"),
    ("@factor(360)", Integer(360), Integer(360), r"2^{3}\cdot 3^{2}\cdot 5"),
    ("@factor(1001/12)", Rational(1001, 12), Rational(1001, 12), r"\frac{7\cdot 11\cdot 13}{2^{2}\cdot 3}"),
    ("@expand((x + 1)^3)", (x + 1) ** 3, x**3 + 3 * x**2 + 3 * x + 1, r"x^{3}+3x^{2}+3x+1"),
    ("@expand((x - 2)(x + 3))", (x - 2) * (x + 3), x**2 + x - 6, r"x^{2}+x-6"),
    ("@expand((2x - 1)^2)", (2 * x - 1) ** 2, 4 * x**2 - 4 * x + 1, r"4x^{2}-4x+1"),
    ("@expand((x^2 + x + 1)(x - 1))", (x**2 + x + 1) * (x - 1), x**3 - 1, r"x^{3}-1"),
    ("@apart(x/(x^2 - 3x + 2))", x / (x**2 - 3 * x + 2), 2 / (x - 2) - 1 / (x - 1), r"\frac{2}{x-2}-\frac{1}{x-1}"),
    ("@apart(1/(x^2 - 1))", 1 / (x**2 - 1), 1 / (2 * (x - 1)) - 1 / (2 * (x + 1)), r"\frac{1}{2\left(x-1\right)}-\frac{1}{2\left(x+1\right)}"),
    ("@apart((x^2 + 1)/(x(x - 1)))", (x**2 + 1) / (x * (x - 1)), 1 + 2 / (x - 1) - 1 / x, r"1+\frac{2}{x-1}-\frac{1}{x}"),
    ("@apart(1/(x(x^2 + 1)))", 1 / (x * (x**2 + 1)), 1 / x - x / (x**2 + 1), r"\frac{1}{x}-\frac{x}{x^{2}+1}"),
    ("@apart(1/((x - 1)^2 (x + 1)))", 1 / ((x - 1) ** 2 * (x + 1)), Rational(1, 2) / (x - 1) ** 2 - Rational(1, 4) / (x - 1) + Rational(1, 4) / (x + 1),
     r"\frac{1}{4\left(x+1\right)}+\frac{1}{2\left(x-1\right)^{2}}-\frac{1}{4\left(x-1\right)}"),
]:
    cmd_check(inp, inner, shaped, latex)


REQUIRED_FILE = Path(__file__).with_name("required.txt")


def write():
    # 厳密値の道で正しい形が出るようになった行は「必須」（外れたらテストが落ちる）。一覧は core/tests/exact.rs が書き出す（EXACT_DUMP）
    required = set(REQUIRED_FILE.read_text().splitlines()) if REQUIRED_FILE.exists() else set()
    rows = []
    for area, inp, latex, approx, source, mark in CASES:
        if inp in required and mark == "目標":
            mark = "必須"
        rows.append((area, inp, latex, approx, source, mark))
    by_area = {}
    for row in rows:
        by_area.setdefault(row[0], []).append(row)
    names = {"学校": "school", "複素": "complex", "離散": "discrete", "線形": "linear", "微分": "derivative",
             "極限": "limits", "級数": "series", "積分": "integrals", "不定積分": "antiderivatives", "方程式": "equations", "命令": "commands",
             "固有値": "eigen"}
    files = {}
    for area, rows in by_area.items():
        lines = ["# 入力\t期待する答え（LaTeX）\t近似値\t分野\t出典\t印  （tools/exact/cases.py が作る。手で直さない）"]
        for area_, inp, latex, approx, source, mark in rows:
            lines.append("\t".join([inp, latex, approx, area_, source, mark]))
        files[OUT / f"{names[area]}.tsv"] = "\n".join(lines) + "\n"
    return files


def main():
    files = write()
    check = "--check" in sys.argv
    bad = False
    for path, text in files.items():
        if check:
            if not path.exists() or path.read_text() != text:
                print(f"食い違い: {path}")
                bad = True
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
    print(f"{len(CASES)} 問（{len(files)} ファイル）")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()

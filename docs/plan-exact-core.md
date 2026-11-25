# 計画: 厳密値と高い精度の計算の土台

文書 7（`docs/plan-roadmap.md`）。式の値を、近似値だけでなく**厳密値**（$\frac{\sqrt 3}{2}$・$\frac{\pi^2}{6}$・$x^3/3 + C$）でも出すための土台を、
1 ステップずつ（1 ステップ = 1 PR）の指示書にする。ステップの名前は **EX0〜EX12**。
分野ごとの計算（学校の数学・線形代数・離散数学・微積分・極限と級数・積分・微分方程式・ベクトル解析・複素関数）は**文書 8**（`docs/plan-exact-areas.md`）で、この文書の上に乗る。
各ステップの「担当」は `docs/agent-tiers.md` の段階。各ステップの「実装の土台」は、関数の形・データの形・処理の流れの下書き。
**土台は出発点で、命令ではない**。コードを読んで合わない所があれば直してよく、そのときは 0 章の表に「計画との違い」を書く。

**この文書の芯**:

- 目標は Wolfram ではなく、**学校の数学と、工学系の大学の数学**（教養の線形代数・微積分と、ベクトル解析・複素関数・微分方程式・離散数学）の式を、ちゃんと求められること
- 今までの近似値の道（f64）は**そのまま残す**。厳密値は別の Worker で計算し、終わったら画面の値を置き換える。終わるまでは「計算中」と出して待たせる
- 出した厳密値は、**必ず別の道で計算した値と照らし合わせ**、ずれたら捨てる（規則の誤りを画面に出さない）
- 答えは「見つかった」「見つからない」「存在しない（証明できたときだけ）」の 3 通り。見つからないときは近似値だけを出す

## 0. 進み具合（実装の記録）

| ステップ | 状態 | メモ（計画との違い） |
|---|---|---|
| EX0 | 済み（2026-11-26） | 例題は 410 問（9 ファイル）。答えの確かめと表づくりを 1 つのスクリプト `tools/exact/cases.py` にした（計画の `check_cases.py` の代わり。問題と答えを SymPy の式で持ち、30 桁で比べてから tsv を書く。`--check` で食い違いだけを見る）。印は全部 `目標` か `存在しない`（`必須` は各ステップが済んだときに付け替える）。近似値の列は複素数なら `実部;虚部`、式の答えは `-`。テストは今の f64 の道の値とも比べ、違う 7 行（`gcd` の大きな数・`mod` のべき・数値で判定できない極限 3 つ）を出す。答えの形で 2.6 に足した決まりは 2.6 の最後 |

## 1. 調べた結果（2026-11-25）

### 1.1 今ある部品

| 部品 | 場所 | この計画での使い道 |
|---|---|---|
| 構文木 | `parser.rs` の `Expr`（`Num(f64)`・`Const`（π・e・i だけ）・`Var`・`Neg`・`Bin`・`Call`・`Tuple`・`List`、展開で消える `Apply`・`Binder`・`Op`・`Matrix`・`Index`・`Prime`・`Cond`）。数の字句は `Tok::Num(f64)` で、**解析の段では数をまとめない** | 厳密値の道の入口（2.2）。`0.3` は f64 になっているが、最短の 10 進表記（`0.3`）に戻せば $\frac{3}{10}$ になる（2.7） |
| 展開の段 | `expand.rs` の `expand`（1089 行）・`Expander::val`（記法ごとの振り分けは 1360 行あたり）。積分・極限・∞ の和は**ここで数（f64）になる**。`diff.rs` の `add`・`mul` も数どうしをその場で f64 にまとめる | 厳密値の道は**展開の前の木から作る**（2.2）。展開の段は通らない |
| 定義の表 | `expand.rs` の `Defs`・`Def::Function { args, body }`・`Def::Value`・`Def::Set`。`lib.rs` の `definitions(src)` と `take_definitions` で行の一覧から作る | 厳密値の道でも同じ表を使い、`Apply` を定義の本体に置き換える |
| 定数の式の値 | `equation.rs` の `eval_constant`（3287 行）・`eval_constant_with`（3321 行）。f64 の 1 回の評価 | 照らし合わせの相手の 1 つ（2.4） |
| 値の表示の計画 | `plan-analysis.md`（文書 2）の V0・V1。`value.rs` の `Shown { approx, exact: Option<String> }`、画面は「厳密値があれば先に出し、近似値を ≈ で添える」（2.2）。**まだ実装されていない** | EX5 はこの表示の場所と書式に乗る（V1 が先） |
| 記号微分 | `diff.rs`（`diff_n_marked`。表に無い関数は中心差分の式。折れ目の印） | 規則の表を厳密値の道の微分（文書 8 の DF0）の手本にする |
| 記号の不定積分 | `integrate.rs` の `antiderivative`（中身が 1 次式の基本の関数だけ） | 文書 8 の IG0 で置き換える |
| 多項式の連立方程式 | `groebner.rs`（係数は **i128 の分数**。あふれたら諦める）。`polynomial_integral` も同じ分数 | EX12 で大きな整数に替える |
| 2 次以下の解 | `solve.rs` の `quadratic`・`roots` | 文書 8 の SC1 で、厳密な解の土台にする |
| テイラー係数 | `series.rs` の `taylor`（f64 の係数。特殊関数・`abs` は None） | 文書 8 の LM0（式の級数展開）の手本 |
| 1 変数の極限 | `limit.rs`（数値。`analyze`・`sequence`） | 極限の厳密値の照らし合わせの相手 |
| 数値積分・∞ の和 | `quad.rs`（ガウス・クロンロッドの適応型）、`expand.rs` の `sum_to_infinity`（4424 行） | 照らし合わせの相手。EX10 で高い精度の版を別に作る |
| 区間演算 | `interval.rs`（f64 の区間） | 考え方は EX3 のボール演算と同じ。共有はしない（精度が違う） |
| 桁落ちを避けた和 | `accurate.rs`（Shewchuk の和など） | 今のまま |
| 論理式の標準形 | `logic.rs`（選言標準形） | 文書 8 の DM3（論理式の簡約）の入口 |
| 特殊関数 | `functions.rs` の `Func`（`Gamma`・`Digamma`・`Polygamma`・`Zeta`・`Gcd`・`Lcm`・`NCr`・`Mod` など。全部 f64） | 定数の原子（2.8）の値の照らし合わせの相手。`special_values.rs` の参照値も使える |
| 計算の Worker | `web/src/lib/plotClient.ts`（Worker を最大 4 つ。式ごとに担当を決める）、`plot.worker.ts`（`init()` で WASM を読む） | EX5 で厳密値の Worker を 1 つ足す |
| WASM の大きさの予算 | `plan-render-algorithms.md` 5 章の 3（1 ステップ +50 KB が目安）。今の gzip は約 1,163 KB（`plan-performance.md` 59 行） | dashu は D1 で決めたので例外（2.10） |

### 1.2 計算エンジンで確かめた今の動き

`core/tests/` に一時的なテストを置き、`eval_constant` で確かめた（終わったので消した）。

| 式（エンジンの文字列） | 今の値（f64） | 正しい値 | 問題 |
|---|---|---|---|
| `1/3+1/6` | 0.5 | $\frac12$ | 形が分数にならない |
| `0.1+0.2` | 0.30000000000000004 | $0.3$ | 2 進の丸め |
| `sqrt(2)^2-2` | $4.4 \times 10^{-16}$ | $0$ | 0 にならない |
| `25!` | $1.5511210043330977 \times 10^{25}$ | 15511210043330985984000000 | 下の桁が違う |
| `nCr(100,50)` | $1.0089134454556422 \times 10^{29}$ | 100891344545564193334812497256 | 下の桁が無い |
| `gcd(2^60+1, 3^40)` | 誤り「実数の値になりません」 | 1 | f64 で整数として扱えない |
| `@at((1-cos(x))/x^2, x, 10^(-8))` | **0** | $\approx 0.5$ | 桁落ち |
| `@lim(x,∞) (1+1/x)^x` | 2.71828182874912 | $e = 2.718281828459045\ldots$ | 数値の極限の誤差 $3 \times 10^{-10}$ |
| `@lim(x,∞) x^100/e^x` | 誤り（収束を確認できない） | 0 | 数値では判定できない |
| `@lim(n,∞) (@sum(k,1,n) 1/k) - ln(n)` | 誤り（同上） | $\gamma$ | 同上 |
| `@lim(x,0) x^x` | 誤り（同上） | 1（右から） | 同上 |
| `@sum(k,1,∞) 1/k^2` | 1.6449340668482362 | $\frac{\pi^2}{6}$ | 値は正しいが形が無い |
| `@int(x,0,∞) e^(-x^2)` | 0.8862269254527582 | $\frac{\sqrt\pi}{2}$ | 同上 |
| `@int(x,0,1) arctan(x)/x` | 0.9159655941772191 | カタランの定数 $G$ | 同上 |
| `@at(@diff(x) x^3 sin(x), x, π/3)` | 3.4232996877764683 | $\frac{\pi^2(\pi + 9\sqrt3)}{54}$ | 同上 |
| `ln(-1)`・`(1+i)^10` | 誤り「実数の値になりません」 | $\pi i$・$32i$ | 複素数の値（文書 2 の V0 で直る） |
| `@det(⟦1,2,3;4,5,6;7,8,10⟧)` | $-3$ | $-3$ | 正しい（`linalg.rs` の `exact_det`） |

### 1.3 使えるパッケージ（2026-11-25 に crates.io で確かめた）

`wasm32-unknown-unknown` で試しにビルドし、`opt-level = 3`・`lto`・`panic = "abort"`（このリポジトリと同じ設定）で大きさを測った。

| パッケージ | 版 | ライセンス | 中身 | WASM（gzip） | 判断 |
|---|---|---|---|---|---|
| **dashu**（`dashu-int`・`dashu-ratio`・`dashu-float`） | 0.6.2 | MIT / Apache-2.0 | 大きな整数・分数・高い精度の浮動小数点数。`sin`・`cos`・`tan`・`atan`・`asin`・`exp`・`ln`・`sqrt`・`sinh` などを**正しく丸めて**出す（Ziv の方法）。誤差の幅を持つボール（`ball.rs`）。`no_std` 対応 | 整数・分数・小数と sin・exp・ln・atan・sqrt を使う試しで **約 101 KB** | **使う**（D1） |
| num-bigint ＋ num-rational | 0.4.8 ＋ 0.4.2 | MIT / Apache-2.0 | 整数と分数だけ | 約 34 KB | 使わない（高い精度の小数を自作することになる） |
| astro-float | 0.9.6 | MIT | 高い精度の浮動小数点数 | WASM では `getrandom` の設定が要り、そのままではビルドできない | 使わない |
| malachite | 0.15.0 | **LGPL-3.0** | 速い整数・分数 | — | 使わない（WASM に静的に入れると LGPL の条件が重い） |
| rug | 1.30.0 | **LGPL-3.0+**（GMP・MPFR の C） | — | — | 使わない（C のライブラリで WASM に向かない） |
| feanor-math | 3.6.2 | MIT | 多項式の因数分解・LLL・グレブナー基底など | — | 使わない（**nightly** の Rust が要り、依存が多い）。**実装を読む参考**にはよい |
| symbolica | 3.0.2 | 独自（標準のオープンソースではない） | 数式処理 | — | 使わない |

### 1.4 参考になる資料

| 資料 | ライセンス・入手 | 使い道 |
|---|---|---|
| SymPy（`sympy/polys`・`sympy/series/gruntz.py`・`sympy/concrete/gosper.py`・`sympy/integrals/manualintegrate.py`・`sympy/integrals/meijerint.py`・`sympy/solvers/ode`） | BSD-3 | アルゴリズムの読みやすい実装。**テストの式と答え**を例題の表に使える（出典とライセンスの文を残す） |
| mpmath（`quadts`（二重指数型の数値積分）・`nsum`（級数の加速）・`identify`・`pslq`） | BSD-3 | EX10・EX11 の手本 |
| Rubi（Rule-based Integrator）の `MathematicaSyntaxTestSuite` | MIT（GitHub のリポジトリの表示） | 積分の例題（不定積分と、もっとも簡単な答え）。教養の範囲の分を抜き出して例題の表にする（文書 8 の IG0） |
| K. O. Geddes ほか『Algorithms for Computer Algebra』、J. von zur Gathen・J. Gerhard『Modern Computer Algebra』 | 書籍 | 多項式の GCD・因数分解（ヘンゼル持ち上げ）・部分分数・終結式 |
| M. Bronstein『Symbolic Integration I』 | 書籍 | 有理関数の積分（Hermite の簡約・Rothstein–Trager）と Risch の考え方 |
| D. Gruntz の博士論文（1996） | 公開 | $\infty$ での極限（文書 8 の LM2） |
| M. Petkovšek・H. Wilf・D. Zeilberger『A = B』 | 著者が無料で公開 | Gosper のアルゴリズム（文書 8 の DM1） |
| H. Ferguson・D. Bailey の PSLQ の論文 | 公開 | EX11 |
| DLMF（NIST の数学関数の表） | 公開 | 特別な値と公式（定数の原子の関係） |

### 1.5 Wolfram の作り（会話で調べたことのまとめ）

Wolfram の強さは、1 つのアルゴリズムではなく、次の 3 つの組み合わせにある:
(1) 書き換えの規則を変わらなくなるまで当てる評価器と、自動の正規化（和・積の項を決まった順に並べ、同類項をまとめる）、
(2) 分野ごとのアルゴリズム（因数分解・グレブナー基底・積分・和・微分方程式）、
(3) 関数ごとの特別な値の規則の巨大な表と、**高い精度の数値で確かめること**（0 の判定 `PossibleZeroQ` も数値頼み）。
この計画は (1) と (3) の作りをまねし、(2) は教養・工学の範囲に絞る。Meijer の G 関数・CAD・Kovacic のアルゴリズムは作らない（文書 8 の 2 章）。

## 2. 全体の決めごと

### 2.1 2 つの道

```text
入力の行 ──┬─ 今までの道（plot.worker、f64）……………… すぐに近似値と絵を出す（今のまま）
           └─ 厳密値の道（exact.worker、新規）……… 計算中 → 厳密値（照らし合わせに通ったものだけ）
```

- 厳密値の道は**別の Worker**（`exact.worker.ts`）で動く。同じ WASM を別に読み込む（メモリは別）。描画を止めない
- 厳密値の道は絵を描かない。絵を置き換えるのは、答えの式を行として足したとき（D2）と、微分方程式の厳密解（文書 8 の OD2）だけ
- 近似値の道の**桁落ちの所を高い精度で描き直すことはしない**（f64 は画素より細かく、絵が間違う原因の多くは精度ではないため。会話で決めた）

### 2.2 展開の前の木から作る

展開の段（`expand.rs`）は積分・極限・∞ の和を f64 にし、`diff.rs` も数どうしを f64 にまとめる。そこを通ると $\frac13$ が 0.333… になって戻せない。
厳密値の道は、**解析した直後の木**（`parse_with(src, &expand::names())` の結果。`Apply`・`Binder`・`Op` が残っている）から、自分の式の形 `Term`（2.10）を作る。
定義の表 `Defs` は同じものを使う。記法（`Binder`・`Op`）ごとの厳密な計算は、文書 8 の各ステップが 1 つずつ足す。**まだ扱えない記法が 1 つでもあれば「見つからない」**。

### 2.3 結果の 4 通り

| 結果 | 画面 | 例 |
|---|---|---|
| 見つかった | `= 厳密値`、その下に小さく `≈ 近似値` | $= \frac{\pi^2}{6}$ ≈ 1.644934067 |
| 推定（EX11） | `= 推定の式 ？（推定）`。厳密値とはっきり分ける | $= G$ ？（推定） |
| 見つからない | 近似値だけ（今と同じ）。値の上にポインターを置くと「厳密値は見つかりませんでした（理由）」 | $\int_0^1 e^{-x^4}\,dx$ |
| 存在しない（証明つき） | 「存在しません」と理由 | $\lim_{x\to0}\frac{|x|}{x}$（左右で違う） |

「見つからない」と「存在しない」を混ぜない。数値で「存在しない」と言えるものは文書 2 の言葉（2.3）のまま。

### 2.4 照らし合わせ（必ずする）

厳密値の道が出した答え $A$ は、画面に出す前に、**元の問題を別の道で計算した値** $B$ と比べる。

1. $A$ を高い精度（256 ビット）で計算する（EX3。誤差の幅つき）
2. $B$ は、EX10 の高い精度の数値（元の積分・和・極限を、厳密値の規則を使わずに数値で計算したもの）があればそれ、無ければ今の f64 の道（`eval_constant_with`）
3. 一致の基準: $B$ が高い精度なら $|A - B| \le 10^{-30}\max(1, |B|)$、f64 なら $|A - B| \le 10^{-8}\max(1, |B|)$
4. 一致しなければ $A$ を**捨てて**「見つからない」にし、理由「照らし合わせで食い違いました」を記録する（D11）。
   ただし $B$ が f64 で、高い精度の $B'$（EX10）とも比べられ $A$ と $B'$ が一致したなら、$A$ を採って、近似値の表示を $B'$ に替える（f64 の道の桁落ちを直す）
5. 答えが式（文字を含む）なら、文字に 3 組の値（有理数。定義域の中）を入れて同じことをする。不定積分は微分して元に戻ることも確かめる（文書 8 の IG0）

### 2.5 予算と打ち切り

- エンジンの中で**仕事の量を数える**（大きな整数の掛け算・規則の適用の回数を、重みを付けて足す）。予算を超えたら「見つからない（時間切れ）」で戻る。
  ステップ数で決めるので、同じ入力なら同じ結果（`ode.rs` が時間ではなく歩数で打ち切るのと同じ考え）
- 既定の予算は、ふつうの PC で約 2 秒の量（EX5 で測って決める。D7）
- 画面側は 8 秒で Worker を止めて作り直す（最後の手段。止まらない不具合への備え）
- 入力を変えたら古い依頼は捨てる。スライダーを動かしている間は計算しない（止まってから 300 ms 後に始める。D6）

### 2.6 答えの形の決まり（日本の教科書に合わせる。D13）

- 分母は**有理化**する: $\frac{1}{\sqrt3 - 1} = \frac{\sqrt3 + 1}{2}$
- 根号の中は平方因子を含まない整数: $\sqrt 8 = 2\sqrt 2$、$\sqrt{\frac{3}{4}} = \frac{\sqrt 3}{2}$
- 分数は 1 本の分数線にまとめる: $\frac{\sqrt3}{2}$（$\frac12\sqrt3$ にしない）、$\frac{3\pi}{4}$、$-\frac{\pi}{6}$（符号は分数の前）
- 数・根号・定数・文字の順に掛ける: $2\sqrt3\,\pi$、$3\pi x^2$
- 多項式は降べきの順。因数分解した形と展開した形は、問題の種類で決める（`factor` は因数分解、それ以外は展開した形と因数分解した形の短い方）
- 入れ子の根号は外せるなら外す: $\sqrt{3 + 2\sqrt2} = 1 + \sqrt2$
- 複素数は $a + bi$（$i$ は最後）。極形式を求めたときだけ $re^{i\theta}$
- 自然対数の書き方は D5
- 小数の入力があれば答えも小数（2.7）
- 条件つきの答えは、答えの後に「（$a > 0$ のとき）」
- （EX0 で足した）数だけの和の項の順: 有理数 → 根号（中の数の小さい順）→ $\pi$ → $e$ → 対数 → $\gamma$ → $G$ → $\zeta$。$\pi$ と $e$ のべきは指数の大きい順。
  先頭が負なら、最初の正の項を先頭に出す（$\sqrt5 - 1$、$1 + \sqrt2$、$2 - \sqrt3$、$\frac{\sqrt6 - \sqrt2}{4}$、$\frac{\sqrt2 + \sqrt6}{4}$）。
  有理数と根号だけの和は 1 本の分数線にまとめ、定数（$\pi$ など）を含む和は項ごとの分数にする（$\frac{\pi^3}{54} + \frac{\sqrt3\,\pi^2}{6}$）。負のべきは分母に置く（$-\frac{1}{\pi}$、$\frac{1}{2e}$）
- （EX0 で足した）LaTeX を比べるときは、`\left`・`\right`・空白を無視する（命令の後に文字が続く所の空白だけ 1 つ残す: `\pi i`）

### 2.7 小数の入力（D3）

- 字句の f64 を、**それを正しく戻す最短の 10 進表記**（Rust の `format!("{}", v)`）に直し、その 10 進数を分数にする: `0.3` → $\frac{3}{10}$
- 入力に 1 つでも小数があれば、答えも小数で出す。有限小数で書ける値は `= 0.3`、書けない値（$\frac13$・$\sqrt2$）は分数・根号の厳密値のあとに `≈ 0.3333333333`
- `9.81` のような数を含む式でも、厳密な計算は分数で行う（$\frac{981}{100}$）

### 2.8 定数の原子

厳密値の道は、次の定数を「計算しない記号」として持つ。それぞれに LaTeX・名前・任意の桁数の値・知られている関係を持たせる。

| 種類 | 定数 | 扱い |
|---|---|---|
| 基本 | $\pi$・$e$・$i$ | 今の `Constant` と同じ |
| 関数の値で書けるもの | $\gamma$（オイラーの定数）・$G$（カタランの定数）・$\zeta(3)$ などの奇数のゼータ・$\ln 2$ などの素数の対数・$\Gamma(\frac13)$ などのガンマの値 | 原子として残す。答えに出してよい。名前を添える（「$G$（カタランの定数）」） |
| 名前だけの代数的数 | 黄金比 $\phi$ | 原子にしない（$\frac{1 + \sqrt5}{2}$ として計算する） |
| 範囲外 | ファイゲンバウム定数など | 持たない |

- **入力の名前は予約しない**: ギリシャ文字 1 字（$\gamma$・$\phi$）は今までどおり変数（工学で比熱比・角度に使うため）。入力には `eulergamma`・`catalan` を使う（D10）
- 原子は**互いに代数的に無関係**とみなす（$\gamma$ が有理数かどうかも分かっていないが、数式処理システムの普通の扱い。照らし合わせで守る）
- 関係の表は、答えを決まった基本（$\pi$・$e$・$\gamma$・$G$・奇数の $\zeta$・素数の $\ln p$・$\Gamma$ の値）に寄せるためにだけ使う: $\Gamma(\frac12) = \sqrt\pi$、$\psi(\frac12) = -\gamma - 2\ln 2$、$\zeta(2) = \frac{\pi^2}{6}$

### 2.9 文字の性質

答えが文字を含む分野（フーリエ係数・微分方程式・和）では、文字の性質が分からないと簡約できない（$\cos n\pi = (-1)^n$ は $n$ が整数のときだけ）。
性質は**式の形から決め**（Σ の添字は整数、$\lim_{n\to\infty}$ の $n$ は自然数、座標は実数）、利用者が書く仕組みはまだ作らない（EX8）。
性質で場合が分かれる答え（$\int x^n\,dx$ の $n = -1$）は、場合を分けるか、条件を添える。

### 2.10 実装の土台（全体の作り）

**新しいファイル**（`core/src/exact/` にまとめる）

| ファイル | 中身 | ステップ |
|---|---|---|
| `exact/mod.rs` | 入口 `evaluate`・結果の型・予算 | EX2・EX5 |
| `exact/num.rs` | 整数・分数（dashu）・最短の 10 進表記からの変換・素因数分解・ベルヌーイ数など | EX1 |
| `exact/term.rs` | 式の形 `Term` と自動の正規化 | EX2 |
| `exact/convert.rs` | 展開の前の `Expr` → `Term`（定義・スライダー・記法の振り分け） | EX2 |
| `exact/consts.rs` | 定数の原子の表 | EX2・EX3 |
| `exact/ball.rs` | `Term` の高い精度の値（誤差の幅つき）・0 の判定 | EX3 |
| `exact/check.rs` | 照らし合わせ | EX3 |
| `exact/latex.rs`・`exact/form.rs` | LaTeX とエンジンの文字列への書き出し、答えの形 | EX4 |
| `exact/poly.rs`・`exact/mpoly.rs`・`exact/factor.rs` | 1 変数・多変数の多項式、GCD・因数分解・終結式・部分分数 | EX6 |
| `exact/algebraic.rs` | 実根の分離・根号での表示・入れ子の根号・有理化 | EX7 |
| `exact/assume.rs` | 文字の性質・符号の判定・条件 | EX8 |
| `exact/simplify.rs` | 簡約の探索 | EX9 |
| `exact/hiprec.rs` | 元の問題の高い精度の数値（積分・和・極限） | EX10 |
| `exact/pslq.rs` | 形の推定 | EX11 |
| `web/src/lib/exact.worker.ts`・`exactClient.ts` | 厳密値の Worker と窓口 | EX5 |
| `web/src/lib/ExactValue.svelte` | 式の行の右下の表示 | EX5 |
| `core/tests/exact.rs`・`core/tests/exact_cases/*.tsv` | 物差しと例題の表 | EX0 |

**データの形（下書き）**

```rust
// exact/num.rs
pub type Int = dashu::integer::IBig;
pub type Rat = dashu::rational::RBig;
/// 字句の f64 を、最短の 10 進表記の分数にする（0.3 → 3/10）。非有限は None
pub fn rat_of_literal(v: f64) -> Option<Rat>;

// exact/term.rs
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)] // Ord は 2.6 とは別の「正規の順」（同類項を見つけるため）
pub enum Term {
    Num(Rat),
    Atom(ConstId),                     // π, e, i, γ, G, ζ(3), …（consts.rs の表の番号）
    Sym(String),                       // 文字（座標・束縛された変数）。性質は Assumptions（EX8）が持つ
    Add(Vec<Term>),                    // 2 個以上。並びは正規の順。同類項はまとめ済み。数の項は最後に 1 つだけ
    Mul(Vec<Term>),                    // 2 個以上。数の係数は先頭に 1 つだけ。同じ底はまとめ済み
    Pow(Box<Term>, Box<Term>),         // 指数が有理数の底は正規化済み（√8 → 2·2^(1/2)。2.6）
    Fn(Func, Vec<Term>),               // sin, ln, gamma, zeta, abs, …（functions.rs の Func を使う）
    Root { poly: Vec<Rat>, index: u32 }, // 根号で書けない代数的数（EX7。poly の index 番目の根）
    Tuple(Vec<Term>), List(Vec<Term>), Matrix { rows: usize, cols: usize, items: Vec<Term> },
}

// exact/mod.rs
pub enum Outcome {
    Found { term: Term, conditions: Vec<Condition> },
    Guess { term: Term },                  // EX11 の推定
    NotFound { reason: String },           // 扱えない記法・時間切れ・照らし合わせの食い違い
    NotExist { reason: String },           // 証明できた「存在しない」
}
pub struct Budget { pub limit: u64, used: Cell<u64> } // 2.5。charge(n) が超えたら Err(TimeUp)
pub struct Report {
    pub outcome: Outcome,
    pub latex: String, pub engine: String, // 答えの LaTeX と、行に足すときのエンジンの文字列（Found・Guess のとき）
    pub approx: Option<(f64, f64)>,        // 照らし合わせに使った高い精度の値を f64 に丸めたもの（re, im）
    pub digits: Option<String>,            // 高い精度の値の 10 進（50 桁。D9）
    pub decimal: bool,                     // 小数で出す（2.7）
    pub rules: Vec<&'static str>,          // 使った規則の名前（将来の途中式のため。D4）
    pub work: u64,
}
pub fn evaluate(source: &str, params: &Params, budget: &Budget) -> Report;
```

**画面までの道**

1. `core/src/lib.rs`: `#[wasm_bindgen] pub fn exact_eval(source: &str, defs: &str, names: &str, values: &[f64], budget: u32) -> ExactResult`
   （`ExactResult` は `Report` を `status: u8`・`latex`・`engine`・`approx: Vec<f64>`・`digits`・`conditions: Vec<String>`・`reason` の getter にしたもの）。
   `defs` は `definitions(src)` と同じ行の一覧
2. `web/src/lib/exact.worker.ts`: `init()` の後、依頼ごとに `exact_eval` を呼んで返す。`exactClient.ts` が依頼の順番・捨てる・8 秒の打ち切り・結果の覚え（同じ式・同じ定義・同じスライダーの値なら再計算しない）を受け持つ
3. `App.svelte`: 行が変わったら（文書 2 の値の依頼と同じ所で）`exactClient.request(item)`。結果は `item.exact`
4. `ExpressionItem.svelte` の値の場所（文書 2 の V1。右下）に `ExactValue.svelte` を置く
5. **⚙ のビットは使わない**（描画とは別の道なので）。アプリの設定に「厳密値を計算する」（既定はオン）を 1 つ足す

**WASM の大きさ**: dashu を入れると gzip で約 +100 KB（1.3）。1 ステップ +50 KB の目安を超えるが、D1 で決めたので例外とし、EX1 で測った数を `plan-render-algorithms.md` の表に書く。
EX6 以降は今までどおり 1 ステップ +50 KB を目安にする。

## 3. 決めたこと（2026-11-25。D1〜D4 は書く前に聞いて決まり、D5〜D13 は全部おすすめの案で決まった）

D1〜D4 は書く前に利用者に聞いて決まった。D5〜D13 は、おすすめを付けて出したところ、利用者が「全部おすすめで」と答えて決まった（どちらも 2026-11-25）。

| # | 内容 | 決まったこと | ほかの案 |
|---|---|---|---|
| D1 | 大きな整数・分数・高い精度の小数 | **決定: dashu**（MIT / Apache）。文書 3 の K1 の高い精度の数も dashu-float にする（自作しない） | num-bigint ＋ num-rational、全部自作 |
| D2 | 答えが式のもの（不定積分・一般解・因数分解）の出し方 | **決定: 行の右下に「= 答えの式」を出し、ボタン「行に追加」で新しい行として足す**（足した行はふつうに描ける） | 右下に出すだけ、別のパネル |
| D3 | 小数の入力 | **決定: 分数で厳密に計算し、入力に小数があれば答えも小数で出す**（2.7） | 小数なら厳密値を出さない、分数で出す |
| D4 | 途中式（Step-by-step） | **決定: この計画には入れない（将来の別計画）**。使った規則の名前だけ `Report::rules` に記録する | 一部だけ入れる、入れる |
| D5 | 自然対数の書き方 | 決定: **入力で使った方に合わせる**（`\ln` を使った行は $\ln$、`\log` は $\log$）。どちらも無ければ $\log$（日本の教科書）。**EX0 で直した: このアプリの `\log` は常用対数（`latex.ts` と `functions.rs` の `Log10`）なので、自然対数はいつも $\ln$ で書く**（$\log$ で書くと「行に追加」で常用対数として読まれる）。常用対数は $\log$ のまま | いつも $\ln$、いつも $\log$ |
| D6 | スライダーの文字 | 決定: **今の値（最短の 10 進表記の分数）を入れて計算**。動かしている間は計算せず、止まってから 300 ms 後に計算する | 文字のまま計算して後から値を入れる（一般の場合の答えが特別な値で間違うことがある） |
| D7 | 1 問の予算 | 決定: **ふつうの PC で約 2 秒の仕事量**（EX5 で測って数を決める）。画面側の打ち切りは 8 秒 | 1 秒、5 秒、設定で選ぶ |
| D8 | 推定（PSLQ）の出し方 | 決定: **「= 式 ？（推定）」と、厳密値と違う見た目で出す**（文字の色を薄くし、ポインターで「数値から推定した形です。証明ではありません」） | 出さない |
| D9 | 値を押したときの桁 | 決定: 文書 2 の D1（10 桁 ⇄ 17 桁）に **50 桁を足す**（10 → 17 → 50 → 10。50 桁は高い精度の値があるときだけ） | 17 桁まで |
| D10 | 定数の入力の名前 | 決定: **`eulergamma`・`catalan`**（LaTeX は `\operatorname{eulergamma}`）。ギリシャ文字 1 字は予約しない。出力は $\gamma$・$G$ に名前を添える | $\gamma$・$G$ を予約する |
| D11 | 照らし合わせで食い違ったとき | 決定: **捨てて「見つからない」**（理由を記録） | 「確かめられない厳密値」として薄く出す |
| D12 | 0 かどうかを数値で決めたとき（構造で決められず、256 ビットと 1024 ビットの両方で 0 を含む） | 決定: **0 とみなし、`rules` に「数値で 0 とみなした」と記録する**（画面には出さない） | 画面にも印を出す、0 とみなさない（答えが出にくくなる） |
| D13 | 答えの形の決まり | 決定: **2.6 の決まり** | — |

## 4. ステップの一覧

| # | 内容 | 前提 | 大きさ | 担当 |
|---|---|---|---|---|
| EX0 | 物差し: 例題の表と、分野ごとの割合を出すテスト | — | 小〜中 | C |
| EX1 | 数: dashu・字句の分数・整数の計算（素因数分解・ベルヌーイ数など） | — | 中 | B |
| EX2 | 式の形 `Term` と自動の正規化、展開の前の木からの変換 | EX1 | 大 | A |
| EX3 | 高い精度の値（ボール演算）・照らし合わせ・0 の判定・定数の原子の値 | EX2 | 大 | A |
| EX4 | LaTeX とエンジンの文字列への書き出し、答えの形 | EX2 | 中 | B |
| EX5 | 厳密値の Worker と画面（計算中・右下の表示・行に追加・予算） | EX3, EX4, 文書 2 の V1 | 中 | B |
| EX6 | 多項式（1 変数・多変数、GCD・無平方分解・因数分解・終結式・部分分数） | EX2 | 大 | A |
| EX7 | 代数的数（実根の分離・根号での表示・入れ子の根号・有理化） | EX6 | 大 | A |
| EX8 | 文字の性質・符号の判定・条件つきの答え | EX2 | 中 | A |
| EX9 | 簡約の探索（いくつもの変形を試し、答えの形の物差しで選ぶ） | EX6, EX7, EX8 | 中 | B |
| EX10 | 元の問題の高い精度の数値（積分・和・極限） | EX3 | 大 | A |
| EX11 | 形の推定（PSLQ・最小多項式の推定） | EX10 | 中 | B |
| EX12 | グレブナー基底を大きな整数にする | EX6 | 小〜中 | B |

**おすすめの順番**: EX0 → EX1 → EX2 → EX3 → EX4 → EX5（ここで初めて画面に出る: $\frac13 + \frac16 = \frac12$、$\sqrt8 = 2\sqrt2$、$25!$ の全桁）→ EX6 → EX7 → EX8 → EX9 → EX10 → EX11 → EX12。
文書 8 の SC0・DM0・LA0 は EX5 の後から始められる。EX6 の後は文書 8 と並べて進めてよい（文書 8 の 4 章）。

## 5. ステップごとの指示書

共通の決まり: コメントと README は日本語。`core/` で `cargo test --release`、`web/` で `npm test` と `npm run check` が通ること。
テストのエラーの中身は WASM 以外では読めない（`JsError` を作るとパニックする）ので、計算エンジンの内側の関数（`Result<_, String>` を返すもの）を
テストから呼ぶか、`std::panic::catch_unwind` で成功・失敗を見る。厳密値の道の関数は最初から `Result<_, String>`・`Report` を返す形にする。
**全部のステップの完了の条件に「例題の表で、間違い（照らし合わせに通らない答えを出した数）が 0」を含める**。

### EX0: 物差し

- **担当**: C（理由: 形式を決めて表を作る定型の仕事。答えの正しさは別の数式処理（SymPy）と数値で確かめる）
- **目的**: 「学校・工学の式をどれだけちゃんと求められるか」を、分野ごとの割合で測れるようにする。この表がそのまま「範囲」の定義になる
- **触るファイル**: `core/tests/exact.rs`（新規）、`core/tests/exact_cases/*.tsv`（新規。分野ごとに 1 ファイル）、`core/tests/exact_cases/SOURCES.md`（出典とライセンス）、
  `tools/exact/check_cases.py`（新規。表の答えを SymPy と mpmath で確かめる。CI では動かさない）
- **やること**
  1. 表の形（タブ区切り、1 行 1 問）: `入力（エンジンの文字列）` `期待する答え（LaTeX。2.6 の形）` `期待する近似値（17 桁）` `分野` `出典` `印`。
     印は `必須`（このステップ以降、外れたらテストが落ちる）か `目標`（割合に数えるだけ）か `存在しない`（答えが「存在しない」）
  2. 例題を集める。分野ごとに 30〜80 問、合わせて 400 問ほど。出典は「自作」「教科書の型（数を変えて自作）」「SymPy のテスト（BSD-3）」「Rubi のテスト（MIT）」。
     外から写した行は出典を書き、`SOURCES.md` にライセンスの文を残す
  3. 答えの確かめ: `check_cases.py` で、期待する答えの値と問題の値を 30 桁で比べる（SymPy で問題が解けないもの（$\int_0^1 \frac{\arctan x}{x}\,dx$）は mpmath の数値で比べる）
  4. `exact.rs`: 全部の行を `exact::evaluate` にかけ、行ごとに「正しい形」「値は正しいが形が違う」「見つからない」「推定」「照らし合わせで捨てた」「**間違い**（期待と違う値を出した）」に分けて数え、
     分野ごとの表を出力する。落ちる条件は「間違いが 1 つでもある」「`必須` の行が正しい形でない」。EX0 の時点では `evaluate` が無いので、仮の関数（いつも見つからない）で動かす
  5. 時間も測る: 1 問の時間の中央値と最大（`--release`）
- **実装の土台**

  ```rust
  // core/tests/exact.rs
  struct Case { input: String, latex: String, approx: f64, area: String, source: String, mark: Mark }
  enum Mark { Required, Target, NotExist }
  enum Verdict { Exact, WrongForm, NotFound, Guess, Rejected, Wrong }
  fn judge(c: &Case, r: &graph_core::exact::Report) -> Verdict {
      // latex は空白を除いて比べる。値は |approx − 期待| ≤ 1e-12·max(1, |期待|)
  }
  #[test] fn table() { /* 全部の tsv を読み、分野 × Verdict の表を println!。Wrong > 0 か Required の外れで panic */ }
  ```

  ```text
  # core/tests/exact_cases/school.tsv（例。列はタブ）
  sin(π/12)	\frac{\sqrt{6}-\sqrt{2}}{4}	0.25881904510252076	学校	自作	必須
  1/(sqrt(3)-1)	\frac{\sqrt{3}+1}{2}	1.3660254037844386	学校	自作	目標
  ```

- **やらないこと**: 厳密値の計算そのもの
- **完了の条件**: 表が 400 行以上、全部の行が `check_cases.py` で 30 桁一致。`cargo test --release --test exact` が通り、分野ごとの表が出る（この時点では全部「見つからない」）
- **壊してはいけないもの**: ほかのテスト
- **README**: 書かない（EX5 でまとめて書く）

### EX1: 数

- **担当**: B（理由: アルゴリズムは教科書どおりで、乱数のテストで確かめられる）
- **目的**: 大きな整数と分数を使えるようにし、f64 では壊れる整数の計算（$25!$・$\binom{100}{50}$・$\gcd$）を正しくする土台を作る
- **触るファイル**: `core/Cargo.toml`（`dashu = "0.6"`）、`core/src/exact/mod.rs`・`exact/num.rs`（新規）、`core/tests/exact_num.rs`（新規）、
  `docs/plan-render-algorithms.md`（WASM の大きさの表）
- **やること**
  1. `dashu` を入れ、WASM（`npm run wasm` の後の `.wasm`）の大きさと gzip を測って記録する。目安は +110 KB 以内
  2. `rat_of_literal`（2.7）: `format!("{}", v)` の文字列（`0.3`・`1e300` は全部の桁の形になる）を、10 進の分数に読む
  3. 整数の計算: $\gcd$・$\operatorname{lcm}$・合同式の逆元・中国剰余定理・オイラーの $\varphi$・階乗（二分割で掛ける）・二項係数・多項係数・整数の $n$ 乗根（あれば）
  4. 素因数分解: 小さい素数で割る（$10^4$ まで）→ Miller–Rabin（$3.3 \times 10^{24}$ 未満は決まった 13 個の底で確定）→ それより大きい数は BPSW（確率的素数判定。「たぶん素数」）→
     Pollard–Brent の $\rho$。予算を使い切ったら、分解できなかった部分を残す
  5. ベルヌーイ数 $B_n$（分数。Akiyama–Tanigawa か漸化式）。文書 8 のゼータの偶数の値と和の公式に使う
- **実装の土台**

  ```rust
  pub fn rat_of_literal(v: f64) -> Option<Rat> {
      if !v.is_finite() { return None; }
      let s = format!("{v}");                  // "0.3", "123.45", "1000000…"
      let (int, frac) = s.split_once('.').unwrap_or((&s, ""));
      let digits: Int = format!("{int}{frac}").parse().ok()?;  // 符号は int に入っている
      Some(Rat::from_parts(digits, UBig::from(10u8).pow(frac.len())))
  }
  pub fn factor_int(n: &Int, budget: &Budget) -> Result<(Vec<(Int, u32)>, Option<Int> /* 分解できなかった部分 */), TimeUp>;
  pub fn bernoulli(n: usize) -> Rat; // B_1 = −1/2 の流儀
  ```

- **やらないこと**: 式（`Term`）、画面
- **完了の条件**（`core/tests/exact_num.rs`）: $25! = 15511210043330985984000000$、$\binom{60}{30} = 118264581564861424$、
  $\binom{100}{50} = 100891344545564193334812497256$、$\gcd(2^{60}+1, 3^{40}) = 1$、$2^{32}+1 = 641 \times 6700417$、$2^{521}-1$ は素数、
  $100!$ は 158 桁、$B_{20} = -\frac{174611}{330}$、`rat_of_literal(0.1) = 1/10`・`rat_of_literal(9.81) = 981/100`、
  乱数の 1 万組で $\gcd$ と逆元が定義どおり。WASM の gzip の増え方を記録
- **壊してはいけないもの**: 今の f64 の道（何も変えない）
- **README**: 書かない

### EX2: 式の形と自動の正規化

- **担当**: A（理由: 正規の順・同類項・べきのまとめ方を間違えると、後の全部のステップの答えが静かにずれる。$\sqrt{x^2}$ を $x$ にするような「いつも正しいとは限らない書き換え」を入れてしまいやすい）
- **目的**: 厳密値の道の中心の形 `Term` を作り、展開の前の木から変換する。四則・べき・根号・定数・定義した関数までを厳密に計算できるようにする
- **触るファイル**: `core/src/exact/term.rs`・`exact/convert.rs`・`exact/consts.rs`（新規）、`exact/mod.rs`、`core/tests/exact_term.rs`（新規）
- **やること**
  1. `Term`（2.10）と、作るための関数 `add`・`mul`・`pow`・`neg`・`div`（作るたびに正規化する。Wolfram の自動の評価と同じ）:
     - 数どうしは分数のまま計算する
     - 和: 入れ子をほどき、同類項（数の係数を除いた部分が同じ項）をまとめ、0 の項を除く。並べる順は `Ord`
     - 積: 入れ子をほどき、同じ底のべきをまとめ（$x \cdot x^2 = x^3$）、係数を先頭に 1 つ
     - べき: 整数の指数の積・和は**展開しない**（$(x+1)^2$ のまま。展開は EX9 の変形）。$(ab)^n$ は $n$ が整数のときだけ分ける。
       $(x^a)^b = x^{ab}$ は $b$ が整数のときだけ（$\sqrt{x^2} \ne x$）
     - 有理数の有理数乗: 素因数分解して、整数の部分を外に出す（$8^{1/2} = 2 \cdot 2^{1/2}$、$(\frac{3}{4})^{1/2} = \frac12 \cdot 3^{1/2}$、$(-8)^{1/3}$ は**主値**で $1 + \sqrt3\,i$）。
       素因数分解が予算を超える数は、そのまま残す
     - $i^2 = -1$、$e^{\ln x}$ は $x$ が正と分かるときだけ $x$（EX8 の前は、$x$ が正の数のときだけ）
     - 関数: ここでは自明な値だけ（$\sin 0 = 0$、$\cos 0 = 1$、$e^0 = 1$、$\ln 1 = 0$、$\ln e = 1$）。特別な値の表は文書 8 の SC0
  2. `convert.rs`: 展開の前の `Expr` → `Term`。
     - `Num` は `rat_of_literal`、小数があったら `decimal` の印
     - `Apply` は定義の本体に引数を入れる（再帰は深さ 32 まで）
     - `Var` は、スライダーなら値（D6）、座標・束縛された変数なら `Sym`、それ以外は誤り（今の道と同じ文）
     - `Binder`・`Op` は**表に登録された記法だけ**を、その記法の関数に渡す（`register(op, handler)` の表。文書 8 の各ステップが 1 つずつ登録する）。
       登録の無い記法は `NotFound { reason: "…はまだ厳密に計算できません" }`
     - `Tuple`・`List`・`Matrix` は中身ごとに変換する
  3. 大きさの上限: 節の数が 10 万を超えたら「見つからない（式が大きすぎる）」
  4. `consts.rs`: 原子の表（番号・エンジンの名前・LaTeX・日本語の名前）。値の計算は EX3
  5. `exact::evaluate` の最初の形: 変換して `Found`（照らし合わせは EX3、書き出しは EX4。それまでは `Term` の Debug の文字列をテストで見る）
- **実装の土台**

  ```rust
  pub fn add(a: Term, b: Term) -> Term;  // 中で flatten → 同類項 → 並べる → 1 個なら中身、0 個なら Num(0)
  pub fn mul(a: Term, b: Term) -> Term;  // 中で flatten → 係数をまとめる → 同じ底の指数を足す → 0 なら Num(0)
  pub fn pow(b: Term, e: Term) -> Term;  // 数の底・数の指数は rational_power、それ以外は上の規則
  fn rational_power(b: &Rat, e: &Rat, budget: &Budget) -> Term {
      // b = ±∏ p_i^{k_i}（分子・分母とも）。e = m/n。各 p_i^{k_i m} を p_i^{q} · p_i^{r/n}（0 ≤ r < n）に分ける。
      // 負の底は n が奇数なら主値（複素数）、n が偶数なら i を出す: (−4)^{1/2} = 2i
  }
  pub struct Converter<'a> { defs: &'a Defs, params: &'a Params, bound: Vec<String>, budget: &'a Budget, decimal: bool }
  type Handler = fn(&mut Converter, var: Option<&str>, args: &[Expr], body: Option<&Expr>) -> Result<Term, Stop>;
  pub enum Stop { NotFound(String), NotExist(String), TimeUp }
  ```

- **やらないこと**: 積の展開・因数分解（EX6・EX9）、特別な値の表（文書 8）、書き出し（EX4）
- **完了の条件**（`core/tests/exact_term.rs`。`Term` を直接作って比べる）: $\frac13 + \frac16 = \frac12$、$0.1 + 0.2 = \frac{3}{10}$、$\sqrt2^2 - 2 = 0$、$\sqrt 8 = 2\sqrt2$、
  $\sqrt{12} \cdot \sqrt{3} = 6$、$(-8)^{1/3} = 1 + \sqrt3\,i$、$x + x = 2x$、$x \cdot x^2 = x^3$、$(x+1)^2$ はそのまま、$\sqrt{x^2}$ はそのまま、
  定義 $f(x) = x^2 + 1$ で $f(\frac12) = \frac54$、和の項の並べ方が入力の順に関係しない（乱数で並べ替えた 1000 個の式で同じ `Term`）
- **壊してはいけないもの**: 今の道（`expand.rs` は変えない）
- **README**: 書かない

### EX3: 高い精度の値・照らし合わせ・0 の判定

- **担当**: A（理由: 誤差の幅を小さく見積もると、間違った答えが照らし合わせを通る。定数の原子の値（$\gamma$・$G$・$\zeta$・$\Gamma$）の誤差の見積もりは間違えやすい）
- **目的**: `Term` の値を、誤差の幅つきで好きな桁数まで出す。それで 2.4 の照らし合わせと、0 の判定をする
- **触るファイル**: `core/src/exact/ball.rs`・`exact/check.rs`（新規）、`exact/consts.rs`、`core/tests/exact_ball.rs`（新規）
- **やること**
  1. 複素数のボール（中心は dashu の `FBig`、半径は上からの見積もり）。四則・べき・初等関数（dashu の正しく丸めた関数に、引数の半径の伝わり方を足す）。
     分岐のある関数（$\sqrt{}$・$\ln$・$\arg$）は、ボールが分岐の切れ目にかかったら「決まらない」を返す
  2. 定数の原子の値: $\pi$・$e$ は dashu。$\gamma$（Brent–McMillan）、$G$（$G = \frac{\pi}{8}\ln(2 + \sqrt3) + \frac38\sum_{k\ge0}\frac{(k!)^2}{(2k)!\,(2k+1)^2}$）、
     $\zeta(n)$（Borwein の交代級数）、$\Gamma$・$\ln\Gamma$（漸化式でずらしてスターリングの級数。誤差の項はベルヌーイ数で上から押さえる）、
     $\psi$・$\psi^{(m)}$（同じく漸化式と漸近展開）。どれも 1000 ビットまで
  3. `value(term, bits) -> Result<Ball, Undecided>`。精度が足りなければ（半径が大きすぎれば）ビット数を 2 倍にしてやり直す（2048 ビットまで）
  4. 0 の判定（`is_zero`）: まず構造（`Num(0)`）。決まらなければ 256 ビットと 1024 ビットで値を出し、両方で 0 を含めば「0 とみなす」（D12。`rules` に記録）、
     どちらかで 0 を含まなければ「0 でない」
  5. `check.rs`: 2.4 の照らし合わせ。相手の f64 は `equation::eval_constant_with`（定義は同じ `Defs` を入れる）。答えが文字を含む場合は 3 組の値を入れる（入れる値は EX8 の性質に合う有理数）
  6. `evaluate` の最後に照らし合わせを入れる。食い違えば D11
- **実装の土台**

  ```rust
  pub struct Ball { pub re: FBig, pub im: FBig, pub rad: FBig } // 真の値は中心から rad 以内（実部・虚部を合わせた距離）
  pub enum Undecided { Branch, Precision, Domain(String) }
  pub fn value(t: &Term, bits: usize, cache: &mut ConstCache) -> Result<Ball, Undecided>;
  pub fn is_zero(t: &Term, rules: &mut Vec<&'static str>) -> Option<bool>;
  pub fn agree(a: &Ball, b: Approx, tol: Tol) -> bool;
  ```

- **やらないこと**: 元の問題の高い精度の数値（EX10）
- **完了の条件**（`core/tests/exact_ball.rs`）: $\pi$・$e$・$\gamma$・$G$・$\zeta(3)$・$\Gamma(\frac13)$・$\psi(\frac14)$ を 300 桁出し、参照値（テストに文字列で持つ。出典を書く）と全桁一致し、
  出した半径の中に参照値が入る。$\sqrt2^2 - 2$ は 0、$e^{\pi\sqrt{163}} - 262537412640768744$ は 0 でない（$-7.5 \times 10^{-13}$）、
  $\sin(10^{22})$ が $-0.8522008497671888\ldots$。わざと間違えた答え（$\sum 1/k^2 = \frac{\pi^2}{7}$）が照らし合わせで捨てられる
- **壊してはいけないもの**: 今の道
- **README**: 書かない

### EX4: 書き出しと答えの形

- **担当**: B（理由: 規則は 2.6 に決めてあり、見た目と往復で確かめられる）
- **目的**: `Term` を、画面に出す LaTeX と、「行に追加」で使うエンジンの文字列に書き出す。答えの形（2.6）をここで整える
- **触るファイル**: `core/src/exact/latex.rs`・`exact/form.rs`（新規）、`core/tests/exact_latex.rs`（新規）、`web/src/lib/latex.test.ts`（往復のテスト）
- **やること**
  1. `form.rs`: 正規の `Term` を、見せる形の木（`Shown`。分数・根号・符号・掛ける順を持つ）に直す。2.6 の決まりを全部ここに書く（有理化は EX7 の後に本当の有理化に替え、ここでは整数の根号 1 つの分母だけ）
  2. `latex.rs`: `Shown` → LaTeX（`\frac`・`\sqrt`・`\sqrt[3]`・`\left(\right)`・`\cdot` は数どうしの間だけ・関数は `\sin`・`\log`・`\operatorname{…}`）、
     `Shown` → エンジンの文字列（`sqrt(…)`・`(…)/(…)`・`^(…)`）
  3. 小数の答え（2.7）: 有限小数で書けるかを分母の素因数（2 と 5 だけ）で判定し、書けるなら小数、書けなければ分数と ≈
  4. D5 の自然対数の書き方（入力の文字列に `\ln`・`ln(` があるか）
  5. 往復: LaTeX → `web/src/lib/latex.ts` → エンジン → 値、が元の値と一致すること（テストで確かめる）
- **実装の土台**

  ```rust
  pub enum Shown { Num(Rat), Neg(Box<Shown>), Frac(Box<Shown>, Box<Shown>), Root(Box<Shown>, u32), Prod(Vec<Shown>),
                   Sum(Vec<(bool /* − */, Shown)>), Pow(Box<Shown>, Box<Shown>), Call(&'static str, Vec<Shown>), Atom(ConstId), Sym(String), Decimal(String) }
  pub fn shape(t: &Term, style: &Style) -> Shown; // Style { ln: bool, decimal: bool }
  pub fn to_latex(s: &Shown) -> String;
  pub fn to_engine(s: &Shown) -> String;
  ```

- **やらないこと**: 簡約（EX9）。ここは形を整えるだけで、値の違う書き換えはしない
- **完了の条件**: 例: $\frac{\sqrt3}{2}$（`\frac{\sqrt{3}}{2}`）、$-\frac{\pi}{6}$、$2\sqrt3\,\pi$、$\frac{3\pi}{4}$、$x^{3} - 2x + 1$、$\frac{1}{2}$ と小数の入力の $0.3$、$\frac13$ の小数の入力は `\frac{1}{3}` と ≈ 0.3333333333。
  例題の表の全部の答えの LaTeX が `latex.ts` で読めて、値が往復で一致（`npm test`）
- **壊してはいけないもの**: `latex.ts` の今のテスト
- **README**: 書かない

### EX5: 厳密値の Worker と画面

- **担当**: B（理由: 画面と Worker の両方を触る。計算の中身は前のステップで済んでいる）
- **目的**: 厳密値を画面に出す。ここで初めて利用者に見える
- **触るファイル**: `core/src/lib.rs`（`exact_eval`・`ExactResult`）、`web/src/lib/exact.worker.ts`・`exactClient.ts`・`ExactValue.svelte`（新規）、
  `web/src/lib/types.ts`（`ExactState`）、`App.svelte`、`ExpressionItem.svelte`、`persist.ts`（設定だけ）、`README.md`
- **やること**
  1. `exact_eval`（2.10 の画面までの道の 1）。予算 `budget` は D7（ふつうの PC で約 2 秒になる数を、例題の表で測って決め、`exactClient.ts` の定数にする）
  2. `exactClient.ts`: 依頼の列（同時に 1 つ）。行が変わったら古い依頼を捨てる。スライダーを動かしている間は依頼しない（D6）。
     8 秒で `worker.terminate()` して作り直し、その行は「見つからない（時間切れ）」。結果の覚え（鍵は行の文字列・使った定義の文字列・使ったスライダーの値。200 個まで）
  3. 表示（`ExactValue.svelte`。文書 2 の V1 の値の場所）:
     - 計算中: 近似値はそのまま出し、0.5 秒たっても終わらなければ「厳密値を計算中…」と小さく出す
     - 見つかった: `= 厳密値`（MathLive の静的な表示で LaTeX を描く）、その下に `≈ 近似値`（10 桁。D9 で 17・50 桁）。条件があれば「（$a > 0$ のとき）」
     - 推定（EX11 の後）: D8 の見た目
     - 見つからない: 近似値だけ。値の上にポインターを置くと理由
     - 答えが文字を含むとき: ボタン「行に追加」（D2）。押すと `engine` を LaTeX にした行を、その行のすぐ下に足す
  4. アプリの設定「厳密値を計算する」（既定はオン）。オフなら依頼しない
  5. README の「値の表示」の段落に、厳密値の説明（何が厳密に出るか・照らし合わせ・見つからないときは近似値だけ・小数の入力・定数の名前）
- **実装の土台**

  ```ts
  // types.ts
  export type ExactState =
    | { kind: 'computing' }
    | { kind: 'found'; latex: string; engine: string; approx: [number, number]; digits?: string; conditions: string[]; addable: boolean }
    | { kind: 'guess'; latex: string; engine: string; approx: [number, number] }
    | { kind: 'notFound'; reason: string }
    | { kind: 'notExist'; reason: string }
  // exactClient.ts
  export function request(key: ExactKey, job: ExactJob): Promise<ExactState> // 古い依頼は 'cancelled' で解決して捨てる
  ```

- **やらないこと**: 新しい記法（文書 8）
- **完了の条件**: 画面で `\frac{1}{3}+\frac{1}{6}` → $= \frac12$、`\sqrt{8}` → $= 2\sqrt2$、`25!` → 全桁、`0.1+0.2` → $= 0.3$、
  `\int_0^1 x^2 dx`（この時点ではまだ記法が無い）→ 近似値だけ（「まだ厳密に計算できません」）。スライダーを動かしている間は依頼が 0 回（テストで数える）、
  わざと止まらない式（テスト用の隠しの関数）で 8 秒後に Worker が作り直され、次の行の計算が続く。例題の表の 1 問の時間の中央値と最大を 0 章に書く
- **壊してはいけないもの**: 近似値の表示（文書 2）・描画の速さ（厳密値の Worker は描画の Worker とは別）
- **README**: 上の 5

### EX6: 多項式

- **担当**: A（理由: GCD・因数分解・部分分数は、間違えると積分・極限・固有値・微分方程式のすべてが間違う。係数のあふれ・内容（係数の GCD）の扱いを落としやすい）
- **目的**: 方程式・部分分数・固有値・ラプラス変換・留数のすべての土台になる多項式の計算
- **触るファイル**: `core/src/exact/poly.rs`・`exact/mpoly.rs`・`exact/factor.rs`（新規）、`core/tests/exact_poly.rs`（新規）
- **やること**
  1. 1 変数（係数は有理数。中では整数の原始多項式にする）: 四則・割り算の余り・GCD（部分終結式の列）・微分・無平方分解（Yun）・有理数の根・終結式・判別式
  2. 因数分解（整数の上）: 無平方分解 → $\bmod p$ での分解（Cantor–Zassenhaus。$p$ は分解の数が少ない素数を数個試して選ぶ）→ ヘンゼル持ち上げ → 組み合わせ（Zassenhaus。$\bmod p$ の因数が 12 個以下まで。
     超えたら諦めて無平方分解の形を返す。van Hoeij の格子の方法は作らない）
  3. 多変数（疎な表現）: 四則・GCD（1 変数ずつの部分終結式で再帰）。因数分解は (a) 係数の GCD・単項式の括り出し、(b) ある変数について 1 次なら 1 次の因数、(c) 2 変数の同次式は 1 変数にして分解、(d) それ以外は諦める
  4. `Term` ⇄ 多項式: 文字と「多項式の変数とみなす式」（$\sin x$・$e^x$・$\sqrt2$ を 1 つの変数として扱う）を集めて変換する
  5. 部分分数（有理数の上）: 分母を因数分解し、拡張ユークリッドで分ける。既約な 2 次式はそのまま残す（$\frac{ax+b}{x^2+px+q}$）。重なった因数は $\frac{A_k}{(x-a)^k}$
- **実装の土台**

  ```rust
  #[derive(Clone, PartialEq, Eq)] pub struct Poly { pub c: Vec<Rat> } // c[k] が x^k の係数。末尾の 0 は持たない
  pub struct MPoly { pub vars: Vec<Term>, pub terms: Vec<(Vec<u32>, Rat)> } // 辞書式の順に並べる
  pub fn gcd(a: &Poly, b: &Poly) -> Poly;
  pub fn squarefree(p: &Poly) -> Vec<(Poly, u32)>;
  pub fn factor(p: &Poly, budget: &Budget) -> Result<(Rat, Vec<(Poly, u32)>), TimeUp>;
  pub fn apart(num: &Poly, den: &Poly, budget: &Budget) -> Result<Vec<(Poly /* 分子 */, Poly /* 既約因数 */, u32 /* 重なり */)>, TimeUp>;
  pub fn to_poly(t: &Term, var: &Term) -> Option<(Poly, Vec<Term> /* 係数の中の文字 */)>;
  ```

- **やらないこと**: 代数的数の上の因数分解（EX7 で必要な分だけ）
- **完了の条件**（`core/tests/exact_poly.rs`）: $x^4 - 1 = (x-1)(x+1)(x^2+1)$、$x^{12} - 1$ の 6 個の円分多項式、Swinnerton-Dyer の多項式 $S_3$（8 次・既約）が既約と分かる、
  $x^6 + x^3 + 1$（既約）、$\frac{x}{x^2 - 3x + 2} = \frac{2}{x-2} - \frac{1}{x-1}$、$\frac{1}{x^3+1} = \frac{1}{3(x+1)} + \frac{-x+2}{3(x^2-x+1)}$、
  $x^2 - y^2 = (x-y)(x+y)$、$x^3 + x^2y - xy^2 - y^3 = (x-y)(x+y)^2$。乱数の 1000 個の多項式の積を分解して元に戻る（掛け直して一致）。$\gcd$ の結果で両方が割り切れる
- **壊してはいけないもの**: 今の道
- **README**: 書かない

### EX7: 代数的数と根号

- **担当**: A（理由: 根の番号付け・入れ子の根号の外し方・3 次方程式の場合分けは、見た目がもっともらしいまま間違えやすい）
- **目的**: 多項式の根を厳密に持ち、根号で書けるものは根号で、書けないものは「何番目の根」として出す。有理化と入れ子の根号の外しもここ
- **触るファイル**: `core/src/exact/algebraic.rs`（新規）、`exact/form.rs`（有理化）、`core/tests/exact_algebraic.rs`（新規）
- **やること**
  1. 実根の分離: デカルトの符号の規則と二分（Vincent–Collins–Akritas）で、根ごとに有理数の区間を作る。複素根は数値（EX3 のボール）で囲み、重ならないことを確かめる
  2. `Term::Root { poly, index }`: 既約な多項式と根の番号（実根は小さい順、複素根は実部・虚部の順）
  3. 根号で書く: 1 次・2 次は根の公式。3 次は判別式で分け、実根 1 つならカルダノ、実根 3 つ（還元不能）なら三角関数の形 $2\sqrt{-\frac p3}\cos\left(\frac13\arccos(\ldots) - \frac{2\pi k}{3}\right)$、
     角が $\pi$ の有理数倍になれば簡単にする（$x^3 - 3x + 1 = 0$ → $2\cos\frac{2\pi}{9}$・$2\cos\frac{4\pi}{9}$・$2\cos\frac{8\pi}{9}$）。4 次はフェラーリ。5 次以上は `Root` のまま（近似値つき）
  4. 入れ子の根号: $\sqrt{a + b\sqrt c}$ で $a^2 - b^2c$ が有理数の平方なら $\sqrt{\frac{a + d}{2}} \pm \sqrt{\frac{a - d}{2}}$（$d = \sqrt{a^2 - b^2c}$）
  5. 有理化: 分母が 1 つの根号の和なら共役を掛ける。それ以外は分母の最小多項式から逆数を作る（$\mathbb Q(\alpha)$ の中の逆元）
  6. 根どうしの和・積の最小多項式（終結式）。0 の判定は最小多項式で正確にできる（EX3 の数値に頼らない）
- **実装の土台**

  ```rust
  pub struct RealRoot { pub poly: Poly /* 既約・原始 */, pub lo: Rat, pub hi: Rat /* この区間に根は 1 つ */ }
  pub fn isolate(p: &Poly) -> Vec<(Rat, Rat)>;
  pub fn radicals(p: &Poly, index: usize) -> Option<Term>; // 4 次まで。還元不能な 3 次は三角関数の形
  pub fn denest_sqrt(a: &Rat, b: &Rat, c: &Rat) -> Option<Term>;
  pub fn rationalize(t: &Term) -> Term;
  pub fn min_poly(t: &Term, budget: &Budget) -> Result<Option<Poly>, TimeUp>; // 根号と有理数だけの式の最小多項式
  ```

- **やらないこと**: 根号で書けない 5 次以上の根を根号で書く試み、多変数の代数関数
- **完了の条件**（`core/tests/exact_algebraic.rs`）: $\sqrt{3 + 2\sqrt2} = 1 + \sqrt2$、$\sqrt{7 - 4\sqrt3} = 2 - \sqrt3$、$\sqrt{5 + 2\sqrt6} = \sqrt3 + \sqrt2$、
  $\frac{1}{\sqrt3 - 1} = \frac{\sqrt3 + 1}{2}$、$\frac{1}{1 + \sqrt2 + \sqrt3} = \frac{2 + \sqrt2 - \sqrt6}{4}$、$x^3 - 3x + 1$ の 3 根が $2\cos\frac{2\pi}{9}$ など、
  $x^3 - x - 1$ の実根がカルダノの形、$x^4 - 10x^2 + 1$ の根が $\pm\sqrt3 \pm \sqrt2$、$x^5 - x - 1$ は `Root`、
  $\sqrt2 + \sqrt3$ の最小多項式が $x^4 - 10x^2 + 1$、$(\sqrt2 + \sqrt3)^2 - 5 - 2\sqrt6$ が（数値に頼らず）0
- **壊してはいけないもの**: EX2〜EX6 のテスト
- **README**: 書かない

### EX8: 文字の性質と条件

- **担当**: A（理由: 性質から引き出す書き換えは、条件を 1 つ落とすと特別な値で答えが間違う。間違いは数値の照らし合わせでも、選んだ 3 組の値によっては見つからない）
- **目的**: $\cos n\pi = (-1)^n$、$\sqrt{x^2} = |x|$ のような書き換えを、性質が分かるときだけ正しく行う。場合で答えが変わるときに条件を添える
- **触るファイル**: `core/src/exact/assume.rs`（新規）、`exact/term.rs`（書き換えの入口）、`core/tests/exact_assume.rs`（新規）
- **やること**
  1. 性質: 整数・自然数・実数・正・0 以上・0 でない（偶数・奇数は整数の式から判定する）。文字ごとに集合で持つ
  2. 性質の決め方（2.9）: Σ・Π の添字は整数（範囲の下端以上）、$\lim_{n\to\infty}$ の $n$ は自然数、座標と媒介変数は実数、積分の変数は積分の区間の中の実数
  3. 符号の判定 `sign(t) -> Pos | Neg | Zero | NonNeg | NonPos | Unknown`: 文字が無ければ EX3 の値、あれば性質と式の形（正の数の和・積・偶数乗）から
  4. 書き換え（性質が足りるときだけ）: $\sin n\pi = 0$、$\cos n\pi = (-1)^n$、$e^{2\pi i n} = 1$、$(-1)^{2n} = 1$、$\sqrt{x^2} = |x|$（実数なら）・$x$（0 以上なら）、$|x| = x$（0 以上なら）、
     $\ln(ab) = \ln a + \ln b$（両方が正なら。EX9 で使う）
  5. 条件: 答えを作る規則が「$a \ne 0$ なら」のように場合を分けるとき、`Condition` を答えに付ける。場合の数が 2 つなら両方を出してよい
     （$\int x^n\,dx = \frac{x^{n+1}}{n+1} + C$（$n \ne -1$）、$\log|x| + C$（$n = -1$））
- **実装の土台**

  ```rust
  pub struct Props(u8); // ビットの組: INTEGER | NATURAL | REAL | POSITIVE | NONNEG | NONZERO
  pub struct Assumptions { map: HashMap<String, Props> }
  pub enum Sign { Pos, Neg, Zero, NonNeg, NonPos, Unknown }
  pub fn sign(t: &Term, a: &Assumptions) -> Sign;
  pub fn refine(t: &Term, a: &Assumptions) -> Term;
  pub enum Condition { Ne(Term, Term), Gt(Term, Term), Ge(Term, Term), IsInteger(Term) }
  ```

- **やらないこと**: 利用者が性質を書く記法（必要になったら別に計画する）
- **完了の条件**: $n$ が整数で $\cos n\pi = (-1)^n$、$\sin(2n+1)\frac{\pi}{2} = (-1)^n$、$n$ が分からなければそのまま、$x$ が実数で $\sqrt{x^2} = |x|$、
  $\sqrt{x^2}$ は性質が無ければそのまま、$\int x^n\,dx$ が条件つきの 2 つの答え（文書 8 の IG0 の後で確かめる。ここでは条件を作る関数をテスト）。
  照らし合わせに使う 3 組の値が性質を満たす（整数の文字には整数を入れる）
- **壊してはいけないもの**: EX2〜EX7 のテスト
- **README**: 書かない

### EX9: 簡約の探索

- **担当**: B（理由: 変形の部品は前のステップにあり、ここは組み合わせと物差し）
- **目的**: 答えを 2.6 の形に一番近いものにする。Wolfram の `Simplify` と同じく、いくつもの変形を試して物差しで選ぶ
- **触るファイル**: `core/src/exact/simplify.rs`（新規）、`exact/mod.rs`（答えの最後に呼ぶ）、`core/tests/exact_simplify.rs`（新規）
- **やること**
  1. 変形: 展開・因数分解（EX6）・通分・部分分数・約分・根号（EX7 の有理化・入れ子の根号）・三角関数（$\sin^2 + \cos^2 = 1$、2 倍角は減らす向きだけ）・対数（まとめる・分ける。EX8 の条件つき）・性質の書き換え（EX8）
  2. 探索: 幅の狭い最良優先（候補を 8 つまで持ち、各候補に全部の変形を 1 回ずつ当て、物差しで上位 8 つを残す。4 回まで）。予算を使う
  3. 物差し（小さいほどよい）: 節の数 ＋ 分母の根号 ×5 ＋ 入れ子の根号 ×3 ＋ 負の指数 ×2 ＋ 2.6 に合わない形の数 ×2。展開した形と因数分解した形は、問題の種類の希望（`Prefer::Factored` など）で重みを替える
  4. 値の変わらない変形だけを使う（各変形は単体テストで、乱数の値で値が同じことを確かめる）
- **実装の土台**

  ```rust
  pub enum Prefer { Any, Expanded, Factored, PartialFractions }
  pub fn simplify(t: &Term, a: &Assumptions, prefer: Prefer, budget: &Budget) -> Term;
  fn score(t: &Term, prefer: Prefer) -> u32;
  ```

- **やらないこと**: 新しい数学（積分・極限）。物差しを人の好みに完全に合わせること（例題の表で外れた形があれば重みを直す）
- **完了の条件**: $\frac{x^2 - 1}{x - 1} = x + 1$（$x \ne 1$ の条件つき）、$\sin^2 x + \cos^2 x = 1$、$\frac{1}{x} + \frac{1}{x+1} = \frac{2x+1}{x(x+1)}$、
  $\ln 2 + \ln 3 = \ln 6$、$\frac{\sqrt6 + \sqrt2}{\sqrt2} = \sqrt3 + 1$。例題の表の「値は正しいが形が違う」が EX9 の前より減る（数を 0 章に書く）。1 問の簡約の時間が予算の 30% 以内
- **壊してはいけないもの**: 前のステップのテスト（簡約で正しい形が崩れない）
- **README**: 書かない

### EX10: 元の問題の高い精度の数値

- **担当**: A（理由: 数値積分・級数の加速の誤差の見積もりが甘いと、照らし合わせの相手が間違って、正しい答えを捨てたり、間違った推定（EX11）を出したりする）
- **目的**: 厳密値の規則を使わずに、元の積分・和・極限を 60 桁ほどで計算する。照らし合わせ（2.4）の強い相手、推定（EX11）の入力、50 桁の表示（D9）に使う
- **触るファイル**: `core/src/exact/hiprec.rs`（新規）、`exact/convert.rs`（記法ごとに数値の道を登録）、`core/tests/exact_hiprec.rs`（新規）
- **やること**
  1. 中身の関数は `Term` の値（EX3 のボール）で計算する（変数に数を入れて評価する）
  2. 積分: 二重指数型（tanh-sinh。端の特異点に強い）。無限区間は exp-sinh・sinh-sinh。刻みを半分にして 2 回の値の差で誤差を見積もり、目標の桁に届くまで細かくする（mpmath の `quadts` と同じ考え）
  3. 和（$\infty$ まで）: 交代級数は Cohen–Rodriguez Villegas–Zagier の加速、正の項の級数は Richardson の外挿（項が $k$ の有理関数・べきに近いとき）と、Euler–Maclaurin（項の導関数を EX3 の値で数値微分しないで済む形にする。
     実装で難しければ Levin の $u$ 変換）。どれも 2 つの方法か 2 つの深さで一致した桁だけを採る
  4. 極限: 数列・関数の値を $2^{-j}$ で近づけ、Richardson の外挿。一致した桁だけを採る
  5. 桁が足りなければ「高い精度の値は無い」を返す（照らし合わせは f64 に戻る）
- **実装の土台**

  ```rust
  pub struct HiValue { pub re: FBig, pub im: FBig, pub digits: u32 /* 信用できる桁 */ }
  pub fn integral(f: &dyn Fn(&Ball) -> Result<Ball, Undecided>, a: Bound, b: Bound, digits: u32, budget: &Budget) -> Option<HiValue>;
  pub fn series(term: &dyn Fn(&Int) -> Result<Ball, Undecided>, from: Int, digits: u32, budget: &Budget) -> Option<HiValue>;
  pub fn limit(seq: &dyn Fn(u32) -> Result<Ball, Undecided>, digits: u32, budget: &Budget) -> Option<HiValue>;
  ```

- **やらないこと**: 振動する無限区間の積分（$\int_0^\infty \frac{\sin x}{x}\,dx$）の高い精度（諦めて f64 に戻る。必要なら後で Ooura の方法）
- **完了の条件**（60 桁）: $\int_0^\infty e^{-x^2}dx = \frac{\sqrt\pi}{2}$、$\int_0^1 \frac{\arctan x}{x}dx = G$、$\int_0^1 \frac{dx}{\sqrt x} = 2$、$\sum \frac{1}{k^2} = \frac{\pi^2}{6}$、
  $\sum \frac{(-1)^{k+1}}{k} = \ln 2$、$\sum \frac{1}{k^3} = \zeta(3)$、$\lim (H_n - \ln n) = \gamma$、$\lim (1 + \frac1n)^n = e$ が 60 桁一致。
  発散する $\sum \frac1k$ と $\sum \frac{1}{k \ln k}$ で値を出さない（「無い」を返す）。1 問 1 秒以内
- **壊してはいけないもの**: 今の f64 の数値の道
- **README**: 書かない（EX11 でまとめて）

### EX11: 形の推定

- **担当**: B（理由: PSLQ は論文どおりに作れる。受け入れの基準は 3 章で決めてある）
- **目的**: 厳密値が見つからなかったとき、高い精度の値から形を推定して「推定」として出す（D8）
- **触るファイル**: `core/src/exact/pslq.rs`（新規）、`exact/mod.rs`、`ExactValue.svelte`（推定の見た目）、`core/tests/exact_pslq.rs`（新規）、`README.md`
- **やること**
  1. PSLQ（Ferguson–Bailey）を dashu の小数で作る
  2. 候補の基本（小さく保つ）: $\{1, \pi, \pi^2, \ln 2, \sqrt2, \sqrt3, \gamma, G, \zeta(3)\}$ から、値の大きさと分野で 6 個まで選ぶ。積の形は $\ln$ をとって $\{\ln\pi, \ln 2, \ln 3\}$ の関係として探す
  3. 代数的数の推定: 値の 1〜4 乗に対して整数の関係（最小多項式の推定。Wolfram の `RootApproximant`）
  4. 受け入れ: 50 桁で見つけた関係が、100 桁でも成り立つ（残りが $10^{-95}$ 未満）。係数の絶対値は $10^4$ まで。これを満たさなければ出さない
  5. EX10 の値が無ければ推定しない
- **実装の土台**

  ```rust
  pub fn pslq(x: &[FBig], max_coeff: u64, budget: &Budget) -> Option<Vec<Int>>;
  pub fn guess(v: &HiValue, area: Area, budget: &Budget) -> Option<Term>;
  ```

- **やらないこと**: 推定を「厳密値」として扱うこと（照らし合わせを通っても「推定」のまま）
- **完了の条件**: $\int_0^1 \frac{\arctan x}{x}dx$ → $G$ ？、$\sum_{k\ge1} \frac{1}{k^2 \binom{2k}{k}}$ → $\frac{\pi^2}{18}$ ？、$\int_0^\infty \frac{x}{e^x - 1}dx$ → $\frac{\pi^2}{6}$ ？、
  $\sqrt2 + \sqrt3$ の数値 → $x^4 - 10x^2 + 1$ の根 ？。乱数の 100 個の数（関係の無い値）で推定を 1 つも出さない
- **壊してはいけないもの**: 見つかった厳密値の表示
- **README**: 「値の表示」の段落に、推定の意味（数値から推定した形で、証明ではない）

### EX12: グレブナー基底を大きな整数にする

- **担当**: B（理由: 今のアルゴリズムはそのままで、数の型を替える仕事）
- **目的**: `groebner.rs` の i128 の分数を、EX1 の分数に替え、あふれで諦めていた連立方程式を解けるようにする。交点の座標を厳密に出す土台（文書 8 の SC1）と、文書 6 の L4（媒介変数を消す）にも効く
- **触るファイル**: `core/src/groebner.rs`、`core/src/linalg.rs`（`groebner::Rat` を使う所）、`core/tests/systems.rs`
- **やること**
  1. `Rat { n: i128, d: i128 }` を `exact::num::Rat` に替える。あふれの諦めを消し、代わりに予算（係数の大きさと手間）で諦める
  2. 解（実数の点）を、f64 のほかに `Term`（EX7 の根）でも返す入口を足す
  3. 速さを測る（`core/tests/systems.rs` の例で、前と比べて 1.5 倍より遅くならない）
- **実装の土台**

  ```rust
  pub fn solve_exact(eqs: &[Expr], names: &[&str], params: &Params, budget: &Budget) -> Option<Vec<Vec<Term>>>;
  ```

- **やらないこと**: 新しい消去の方法
- **完了の条件**: 今のテストが全部通る。前はあふれで諦めていた例（係数の大きい 3 変数の系。実装時に 1 つ作ってテストに足す）が解ける。速さの比を 0 章に書く
- **壊してはいけないもの**: 連立方程式の点・接する交点
- **README**: 書かない

## 6. 危ない所

- **静かな間違い**: 厳密値の規則の誤りは、もっともらしい答えになる。照らし合わせ（2.4）を必ず通し、例題の表の「間違い」を 0 に保つ。
  照らし合わせの相手が f64 のときは基準が $10^{-8}$ と緩いので、近い値の別の答え（$\frac{\pi^2}{6}$ と $1.6449$ に近い別の数）を見逃しうる。EX10 の後は、なるべく高い精度の相手を使う
- **0 の判定**: 数値で 0 とみなす（D12）のは賭け。$e^{\pi\sqrt{163}}$ のように「ほぼ整数」の値があるので、256 ビットと 1024 ビットの両方で確かめる
- **`Term` の正規化の書き換え**: いつも正しいとは限らない書き換え（$\sqrt{x^2} = x$、$(x^a)^b = x^{ab}$、$\ln(ab) = \ln a + \ln b$）を正規化に入れない。性質が要る書き換えは EX8 だけ
- **主値**: $(-8)^{1/3}$ は主値で複素数（$1 + \sqrt3 i$）になる。学校の「実数の 3 乗根」と違う。`\sqrt[3]{-8}` の入力の意味は今の道（f64）と同じにする（今の道が実数の根を返すなら、厳密値の道も合わせる。EX2 で今の道の動きを確かめる）
- **予算と再現性**: 時間で打ち切ると、PC の速さで答えが変わる。予算は仕事の量で数える（2.5）
- **WASM の大きさ**: dashu で +100 KB。EX6 以降は 1 ステップ +50 KB を目安に測る
- **展開の段との違い**: 厳密値の道は展開の段を通らないので、記法の意味（積分の端の書き方・範囲の式）が二重に実装される。記法ごとに「今の道と同じ値になること」を例題の表で確かめる

## 7. ほかの計画との関係

| 計画 | 関係 |
|---|---|
| 文書 8（`plan-exact-areas.md`） | この文書の上に乗る分野ごとの計算。記法の登録（EX2）を 1 つずつ足す |
| 文書 2（`plan-analysis.md`） | **EX5 は V0・V1 の後**（値の表示の場所と書式）。V0 の `Shown::exact` は、厳密値の Worker の結果を画面で合わせる形にする（エンジンの `value.rs` で埋めるのではなく、画面で `item.exact` を重ねる）。X3 の「判定の目安」は文書 8 の SR0 で「判定（証明つき）」になる |
| 文書 3（`plan-fractal-hyperbolic.md`） | **K1 の `BigFloat` は自作せず dashu-float の `FBig` を使う**（D1）。K1 の完了の条件（四則が最後のビットまで正しい）は dashu の正しい丸めで満たせる |
| 文書 5（`plan-performance.md`） | WASM の大きさの測り方（O0）を共有する |
| 文書 6（`plan-locus-complex-integral.md`） | L4（媒介変数を消す）は EX12 の大きな整数のグレブナー基底を使える。Z3（留数）は文書 8 の CF0 で厳密な留数になる |
| `plan-math-notation.md` | 7.1 の表に 1 行足した |

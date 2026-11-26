# 計画: 分野ごとの厳密値（学校の数学・教養・工学の数学）

文書 8（`docs/plan-roadmap.md`）。文書 7（`docs/plan-exact-core.md`。厳密値の土台）の上に、分野ごとの厳密な計算を、1 ステップずつ（1 ステップ = 1 PR）の指示書にする。
ステップの名前は分野の 2 文字: **SC**（学校の数の値と方程式）・**LA**（線形代数）・**DM**（離散数学）・**DF**（微分と点での値）・**LM**（級数展開と極限）・
**SR**（無限級数）・**IG**（積分とベクトル解析）・**OD**（微分方程式とラプラス変換）・**CF**（複素関数）。
各ステップの「担当」は `docs/agent-tiers.md` の段階。各ステップの「実装の土台」は、関数の形・データの形・処理の流れの下書き。
**土台は出発点で、命令ではない**。コードを読んで合わない所があれば直してよく、そのときは 0 章の表に「計画との違い」を書く。

**この文書の芯**:

- 範囲は**学校の数学と、工学系の大学の数学**（教養の線形代数・微積分、ベクトル解析・複素関数・微分方程式・離散数学）。Wolfram の全部はやらない
- 進み具合は**例題の表**（文書 7 の EX0）の「正しい形で出せた割合」で測る。ステップごとに、その分野の目標の割合を完了の条件にする
- どのステップも、文書 7 の照らし合わせ（2.4）を通った答えだけを出す。例題の表の「間違い」は常に 0
- 答えが式のもの（不定積分・一般解・因数分解）は、行の右下に出して「行に追加」で描ける（文書 7 の D2）

## 0. 進み具合（実装の記録）

| ステップ | 状態 | メモ（計画との違い） |
|---|---|---|
| SC0 | 済み（2026-11-26） | `exact/values.rs`。例題の表の学校の行は **192 問中 185 問が正しい形（96%）**、複素数は 13/13。残りの 7 問は入れ子の根号と、根号 2 つ以上の分母（文書 7 の EX7）。三角関数の表は 120 等分の角の番号で引く（分母 1, 2, 3, 4, 5, 6, 8, 10, 12）。$e^{i\pi s}$・$(-1)^s$ もこの表を使う（文書 7 の EX2 の分母 1・2・3・4・6 から広げた）。`\arg`・`\operatorname{Re}`・`\operatorname{Im}`・`\operatorname{conj}` もここ。計画との違い: (1) ディガンマ関数は整数と半整数だけ（一般の有理数のガウスの定理は、$\ln\sin$ の値の表が要るので後回し）。(2) 常用対数の和は、和の正規化（`term::add`）で 10 の整数乗の対数になるときだけ整数にする（$\log 2 + \log 5 = 1$。$\log 5$ だけなら $\log 5$ のまま）。(3) 和の項の順は 2.6 の決まり（対数 → γ）なので $\psi(\frac12) = -2\ln 2 - \gamma$ と出る。(4) 入れ子の根号は、中身の値が正と分かれば実数として扱う（$e^{i\pi/5}$ の虚部を $\frac{\sqrt{10 - 2\sqrt5}}{4}i$ と書くため） |
| DF0 | 一部済み（2026-11-26） | `exact/diff.rs`（`diff`・`subst`）、`convert.rs` に `@diff`・`@at`・`f′(a)`・`@grad`・`@div`・`@rot`・`@lap`。例題の表の微分の行は **25 問中 25 問**（$\lvert x\rvert$ の 0 は、LM1 の後に左右の極限で「存在しない」と言うようにした）。代入は 0 で割るかを、積の 0 で消える前に調べる（`diff::subst_checked`。前は $x \cdot \lvert x\rvert^{-1}$ に $x = 0$ を入れると 0 になり、照らし合わせが捨てていた）。組・リストの答えは成分ごとに照らし合わせる（`equation::eval_constant_items` を足した）。**まだのもの**: 接線・接平面の式（N4 の点の説明）と、停留点（`@crit`）の厳密な座標（EX12 の後） |
| LA0 | 一部済み（2026-11-26） | `exact/matrix.rs`（`EMat`・`rref`・`det`・`inverse`・`rank`・積・べき）と `Term::Matrix`。`@det`・`@tr`・`@inv`・`@transpose`・`@rank`・`@norm`・`@dot`、行列とベクトルの四則とべき（$A^{-1}$ も）。例題の表の線形代数は **15 問中 15 問**。ヒルベルト行列 6 × 6 の逆行列が整数で出る。文字を含む行列式（$\det\begin{pmatrix}x&1\\1&x\end{pmatrix} = x^2 - 1$）は式の形まで（文字を含む答えを画面に出すのは、文字の答えの照らし合わせを作ってから）。計画との違い: 四則と整数の関数だけ（極限・積分・微分を使わない）の答えは、照らし合わせの基準を $10^{-6}$ に緩めた（今の道の f64 がヒルベルト行列の逆行列で $4 \times 10^{-8}$ ずれ、正しい答えを捨てていたため。`check::F64_TOLERANCE_ARITHMETIC`）。**まだのもの**: 核と像の基底（`@ker`・`@im` は今の道でも 1 行だけの記法）、行列の描き方（M7）の表の中の厳密値 |
| LM0 | 済み（2026-11-26） | `exact/series.rs`。指数は有理数（ピュイズー級数）、$\ln h$ は係数の文字 ℓ。計画との違い: (1) 係数の表し方を「指数 × 分母」の整数でなく、有理数の指数にした。(2) 打ち切りは「指数 `cap` 未満」で、べき・割り算では底を $cap + v(1 - q)$ まで先に展開する（$\frac{1}{\sin x}$ を $O(x^5)$ まで正しく）。定数はちょうどの値なので打ち切りの誤差を持たない。(3) 関数は定数の部分と残りに分けて作る（$\sin(s_0+u) = \sin s_0\cos u + \cos s_0\sin u$、$\arctan(s_0+u) = \arctan s_0 + \arctan\frac{u}{1 + s_0 s}$、$\arcsin s = \arctan\frac{s}{\sqrt{1-s^2}}$）。**まだのもの**: `@taylor` を厳密な係数で出すこと（答えが文字を含むので、文字の答えの照らし合わせを作ってから） |
| LM1 | 済み（2026-11-26） | `exact/limit.rs`、`convert.rs` に `@lim`・`@seqlim`。例題の表の極限は **37 問中 34 問**（「存在しない」の 3 問を含む。残りは $\frac{x^{100}}{e^x}$・$\frac{\ln\ln x}{\ln x}$・$H_n - \ln n$ で LM2）。計画との違い: (1) 「まず代入」はしないで、いつも展開の先頭の項で決める（代入は不連続な関数・$0^0$ で間違えるため）。(2) $\pm\infty$ への発散は「存在しません（+∞ に発散します）」として出す。(3) `@seqlim` は、関数の極限があればその値、関数の極限が無いときは数列では分からないので「見つからない」。(4) 両側の極限で、片側が今の道の数値で確かめられないとき（$\sqrt x$ の 0）は照らし合わせの相手が無いので出さない（片側の書き方 `@lim(x,0,1)` なら出る） |

## 1. 調べた結果（2026-11-25）

### 1.1 今の道で、分野ごとに出るもの・出ないもの

`core/tests/` に一時的なテストを置き、`eval_constant`（f64）で確かめた（終わったので消した）。正しい値は SymPy 1.13.3 と mpmath で確かめた。

| 分野 | 式 | 今の値 | 厳密値 |
|---|---|---|---|
| 学校 | `sin(π/12)` | 0.25881904510252074 | $\frac{\sqrt6 - \sqrt2}{4}$ |
| 学校 | `cos(2π/5)` | 0.30901699437494745 | $\frac{\sqrt5 - 1}{4}$ |
| 線形代数 | `@det(⟦1,2,3;4,5,6;7,8,10⟧)` | $-3$ | $-3$（今も正しい） |
| 線形代数 | `@rank(⟦1,2;2,4⟧)` | 1 | 1（今も正しい） |
| 離散 | `nCr(100,50)` | $1.0089134454556422 \times 10^{29}$ | 100891344545564193334812497256 |
| 離散 | `@sum(k,1,100) k^2` | 338350 | 338350（今も正しい。$n$ を文字にした閉じた形は無い） |
| 微分 | `@at(@diff(x) x^3 sin(x), x, π/3)` | 3.4232996877764683 | $\frac{\pi^2(\pi + 9\sqrt3)}{54}$ |
| 微分 | `@at(@diff(x) abs(x), x, 0)` | 誤り「導関数は存在しません」（左 −1、右 1） | 存在しない（今も正しい） |
| 極限 | `@lim(x,0) (1-cos(x))/x^2` | 0.5 | $\frac12$ |
| 極限 | `@lim(x,∞) x^100/e^x` | 誤り（確認できない） | 0 |
| 極限 | `@lim(x,0) x^x` | 誤り（確認できない） | 1（右から） |
| 極限 | `@lim(x,0) sin(1/x)` | 誤り「振動のため」 | 存在しない |
| 級数 | `@sum(k,0,∞) (-1)^k/(2k+1)^2` | 0.9159655941772188 | $G$（カタランの定数） |
| 級数 | `@sum(k,1,∞) 1/(k(k+1))` | 1.0000000000000007 | 1 |
| 積分 | `@int(x,0,π) sin(x)^2` | 1.5707963267948963 | $\frac{\pi}{2}$ |
| 積分 | `@int(x,0,∞) sin(x)/x` | 1.5707963267948963 | $\frac{\pi}{2}$ |
| 積分 | `@int(x,1,2) ln(x)` | 0.38629436111989074 | $2\log 2 - 1$ |
| 複素 | `(1+i)^10`・`ln(-1)` | 誤り「実数の値になりません」 | $32i$・$\pi i$ |

**記法について**: `@lim(x,∞) sqrt(x^2+x)-x` は「(極限) − x」と読まれて誤りになる（極限の中身は最初の項だけ）。例題の表では `@lim(x,∞) (sqrt(x^2+x)-x)` と括弧で書く。

### 1.2 今ある記法（厳密値の道で登録し直すもの）

`expand.rs` の `Expander::val`（1360 行あたり）と `apply_op` の振り分けにある記法: `sum`・`prod`（Σ・Π）、`int`（積分。端なしは不定積分の数値の道）・`pvint`（主値）・`intset`（領域の積分）、
`lim`・`seqlim`、`diff`（微分）・`at`（点での値）・`fdiff`・`bdiff`・`cdiff`（差分）、`taylor`、`det`・`tr`・`inv`・`transpose`・`norm`・`dot`・`angle`・`rank`・`dim`・`eig`（行列）、
`grad`・`div`・`rot`・`curl`・`lap`（ベクトル解析）、`cross`、`cases`、`sumset`・`prodset`、`max`・`min`・`sup`・`inf`。
1 変数の常微分方程式は `y'=…` の行（`equation.rs`。README の「常微分方程式」）。
**無い記法**: 因数分解・展開・部分分数・方程式を解く命令、ラプラス変換、留数（文書 6 の Z3 で計画済み）、性質の指定

### 1.3 参考にする作り（文書 7 の 1.4 の資料のうち、この文書で使うもの）

| 分野 | 方法 | 出典 |
|---|---|---|
| 有限和 | Gosper のアルゴリズム（不定和）、Faulhaber の公式 | 『A = B』、SymPy の `concrete/gosper.py` |
| 極限 | 級数展開、Gruntz のアルゴリズム（比べられる大きさの段に分ける） | Gruntz の論文、SymPy の `series/gruntz.py` |
| 級数の値 | 部分分数とディガンマ関数の有理数での値（ガウスの定理）、偶数のゼータとベルヌーイ数、超幾何級数として表で当てる | DLMF 5.4・25.6、SymPy の `concrete/summations.py` |
| 不定積分 | 規則を当てる方法（Rubi・SymPy の `manualintegrate`）、有理関数は Hermite の簡約と Rothstein–Trager（Lazard–Rioboo–Trager） | Bronstein、Rubi の例題 |
| 定積分 | 原始関数と、区間の中の不連続の確かめ、ガンマ・ベータの型（Meijer の G 関数の小さい版） | 会話のまとめ（文書 7 の 1.5） |
| 微分方程式 | 型を見分けて解く、定数係数は特性方程式、ラプラス変換 | SymPy の `solvers/ode` |

## 2. 全体の決めごと

### 2.1 範囲（会話で決まったこと）

| 分野 | やる | やらない |
|---|---|---|
| 学校の数学 | 根号・有名な角の三角関数と逆三角関数・指数と対数・方程式（4 次まで根号、三角方程式の一般解）・数列の和・微分・極限・積分・行列・複素数 | — |
| 線形代数 | 有理数（と文字）の行列の掃き出し・階数・核と像・逆行列・行列式・固有値（4 次まで根号）・対角化・ジョルダン標準形・グラム・シュミット（大きさの上限 D4） | 5 次以上の固有値の根号（`Root` と近似値） |
| 離散数学 | 数論・組合せ・有限和の閉じた形・線形漸化式の一般項・論理式の簡約 | グラフ理論・証明・文章の数え上げ |
| 微積分 | 微分・偏微分・点での値（微分係数・接線・勾配・ヘッセ行列）・テイラー展開・極値（多項式ならグレブナー基底）・不定積分・定積分・重積分 | Risch のアルゴリズムの全部 |
| 極限 | 級数展開・Gruntz・片側・「存在しない」の証明 | 多変数の「存在する」の一般の証明（有理関数の次数の比較だけ） |
| 無限級数 | 収束・発散の判定（証明つき）・値（等比・望遠鏡・部分分数とディガンマ・ゼータ・べき級数の表・超幾何の表）・収束半径 | $\zeta(3)$ などの閉じた形（原子のまま出す） |
| 微分方程式 | 変数分離・1 階線形・ベルヌーイ・完全微分・同次形・定数係数の線形（未定係数・定数変化）・オイラー型・連立の定数係数・ラプラス変換 | 非線形の一般の式・Kovacic のアルゴリズム・偏微分方程式の一般（変数分離の決まった型も今回はやらない。D11） |
| ベクトル解析 | grad・div・rot の厳密値・線積分・面積分（媒介変数）・ガウスとストークスの確かめ | — |
| 複素関数 | 複素数の厳密な計算・極形式・ド・モアブル・1 の累乗根・留数（有理関数と簡単な有理型関数）・留数による実積分 | Meijer の G 関数・CAD |

### 2.2 答えの出し方（行の種類ごと）

| 行の種類 | 右下に出すもの | 例 |
|---|---|---|
| 定数の式（座標を使わない） | `= 厳密値` と `≈ 近似値` | $\sin\frac{\pi}{12} = \frac{\sqrt6 - \sqrt2}{4}$ |
| 1 変数の方程式の行（2D の `x^2=2` は縦の直線を描く） | **解**（D1）: $x = \pm\sqrt2$ | $x^3 - 6x^2 + 11x - 6 = 0$ → $x = 1, 2, 3$ |
| 不定積分の行（今は $\int_0^x$ を数値で描く） | `= 原始関数 + C` と「行に追加」 | $\int x^2 e^x\,dx = (x^2 - 2x + 2)e^x + C$ |
| 命令の行（新しい。D2）`\operatorname{factor}(…)` など | 結果の式と「行に追加」 | $\operatorname{factor}(x^4 - 1) = (x-1)(x+1)(x^2+1)$ |
| 常微分方程式の行 | 一般解・特殊解（初期値の欄があれば）と、厳密解に描き替え（OD2） | $y' + y = x$、$y(0) = 1$ → $y = x - 1 + 2e^{-x}$ |
| 連立方程式の点・停留点（`@crit`）・交点 | 点の厳密な座標（点の説明の所） | $\left(\frac{\sqrt2}{2}, \frac{\sqrt2}{2}\right)$ |
| 極限・級数の行 | 値か「存在しません」「発散します」と理由 | $\lim_{x\to0}\frac{|x|}{x}$ は存在しません（左 $-1$、右 $1$） |
| 行列の式 | 行列の厳密値（行列の描き方 M7 の表の中） | $A^{-1}$ の成分が分数 |

### 2.3 記法の登録

文書 7 の EX2 の表（`register(op, handler)`）に、記法ごとの厳密な計算を登録する。**登録の無い記法を含む行は「まだ厳密に計算できません」**。
登録するときは、今の道（展開の段）と同じ意味になること（積分の端・範囲の式・添字の範囲）を、例題の表の近似値で確かめる。

### 2.4 分野ごとの目標の割合

例題の表（文書 7 の EX0）の分野ごとの「正しい形」の割合を、各ステップの完了の条件にする。表の問題は教科書の型に合わせて集めるので、目標は高めにする。

| 分野 | 目標 |
|---|---|
| 学校・線形代数・離散数学・微分 | 95% |
| 極限・級数の判定 | 90% |
| 級数の値・不定積分・定積分・微分方程式 | 85% |
| 重積分・ベクトル解析・複素関数 | 80% |

### 2.5 実装の土台（全体の作り）

**新しいファイル**（`core/src/exact/` の中。文書 7 の 2.10 の続き）

| ファイル | 中身 | ステップ |
|---|---|---|
| `exact/values.rs` | 関数の特別な値の表（三角関数・逆三角関数・対数・ガンマ・ゼータ・ディガンマ） | SC0 |
| `exact/solve.rs` | 方程式の厳密な解 | SC1 |
| `exact/commands.rs` | 命令（`factor`・`expand`・`apart`・`simplify`・`solve`） | SC2 |
| `exact/matrix.rs` | 厳密な行列 | LA0・LA1 |
| `exact/numtheory.rs`・`exact/finite_sum.rs`・`exact/recurrence.rs`・`exact/boolean.rs` | 離散数学 | DM0〜DM3 |
| `exact/diff.rs` | 厳密な微分（`Term` の上） | DF0 |
| `exact/series.rs`・`exact/limit.rs`・`exact/gruntz.rs`・`exact/multilimit.rs` | 級数展開と極限 | LM0〜LM3 |
| `exact/convergence.rs`・`exact/infinite_sum.rs` | 無限級数 | SR0・SR1 |
| `exact/integrate.rs`・`exact/rational_int.rs`・`exact/definite.rs`・`exact/multiple.rs` | 積分 | IG0〜IG3 |
| `exact/ode.rs`・`exact/laplace.rs` | 微分方程式 | OD0・OD1 |
| `exact/complex.rs`・`exact/residue.rs` | 複素関数 | CF0 |

**データの形（下書き）**

```rust
// exact/solve.rs
pub enum Solutions {
    Finite(Vec<Term>),                                // 有限個（重なりは 1 つにする）
    Periodic { bases: Vec<Term>, period: Term, n: String }, // 三角方程式の一般解: x = base + period·n（n は整数）
    All, None,                                        // 恒等式・解なし
}
// exact/ode.rs
pub struct OdeAnswer { pub general: Term /* 定数は Sym("C_1") など */, pub particular: Option<Term>, pub method: &'static str }
```

**画面までの道**: 全部、文書 7 の EX5 の道（`exact_eval` → `ExactState` → `ExactValue.svelte`）に乗る。新しく足すのは:

1. `ExactState` に `kind: 'solutions'`（解の並び）・`kind: 'points'`（点の座標）・`kind: 'ode'`（一般解と特殊解）
2. 方程式の行・常微分方程式の行・点の説明の所に `ExactValue.svelte` を置く（それぞれのステップで）
3. 厳密解に描き替える道（OD2）: `PlotRequest` に `exactSource?: string`。あれば `plot.worker.ts` は常微分方程式の代わりにその式（陽関数・媒介変数の曲線）を描く
4. **⚙ のビットは使わない**

## 3. 決めたこと（2026-11-25。範囲は会話で決まり、D1〜D12 は全部おすすめの案で決まった）

会話で決まったこと（2026-11-25）: 2.1 の範囲（6 分野と学校の数学）。多変数の極限は「存在しない」の証明と、有理関数の「存在する」だけ。級数の判定は文書 2 の「目安」を「証明つき」に格上げする。
微分方程式の厳密解が出たら、数値解の曲線から描き替える。Meijer の G 関数・CAD・Kovacic は作らない。Gosper は作る。Rubi の例題を使う。固有値は 4 次まで根号。グラフ理論はやらない。

D1〜D12 は、おすすめを付けて出したところ、利用者が「全部おすすめで」と答えて決まった（2026-11-25）。

| # | 内容 | 決まったこと | ほかの案 |
|---|---|---|---|
| D1 | 1 変数の方程式の行（`x^2=2`）に解を出すか | **出す**（右下に $x = \pm\sqrt2$。2D で縦の直線を描くのは今のまま） | 命令 `solve` を書いたときだけ |
| D2 | 命令の書き方 | **`\operatorname{factor}`・`\operatorname{expand}`・`\operatorname{apart}`（部分分数）・`\operatorname{simplify}`・`\operatorname{solve}`**。エンジンは `@factor(…)` など。数式キーボードの「∑∫∈」に置く | 命令は作らず、いつも全部の形を並べて出す |
| D3 | 三角方程式の解 | **一般解**（$x = \frac{\pi}{6} + 2n\pi,\ \frac{5\pi}{6} + 2n\pi$（$n$ は整数））。表示範囲の中の解の近似値も添える | 表示範囲の中の解だけ |
| D4 | ジョルダン標準形・グラム・シュミットの大きさ | **4 × 4・4 本まで**（それより大きいと式が巨大になる） | 3 × 3、6 × 6（今の行列の上限） |
| D5 | 不定積分の答え | **「+ C」を付け、実数の変数なら対数に絶対値**（$\log\lvert x\rvert + C$）。範囲で分かれる定数（$x < 0$ と $x > 0$ で別の $C$）までは書かない | 絶対値を付けない（Wolfram と同じ） |
| D6 | 3 次方程式の実根が 3 つのとき | **三角関数の形**（$2\cos\frac{2\pi}{9}$）。文書 7 の EX7 のとおり | 複素数の根号（カルダノのまま） |
| D7 | 微分方程式の答えの定数の名前 | **$C_1, C_2, \ldots$**（`C_1`。行に追加したらスライダーにできる） | $A, B$ |
| D8 | 厳密解への描き替えの見せ方 | **300 ms で重ねて切り替える**（数値解を薄くしながら消し、厳密解を濃くする）。説明に「厳密解 $y = \ldots$ で描いています」 | すぐ切り替える、数値解も残して並べる |
| D9 | 級数の判定の言葉 | **「収束します（比判定法: $\lim\frac{a_{k+1}}{a_k} = \frac1e < 1$）」のように判定法と数を添える**。絶対収束と条件収束を分ける | 「収束します」だけ |
| D10 | ラプラス変換の書き方 | **`\mathcal{L}\left[f(t)\right]`・`\mathcal{L}^{-1}\left[F(s)\right]`**（変数は $t$ と $s$ に決める）。エンジンは `@laplace(t, s) 式`・`@ilaplace(s, t) 式` | 命令の形 `\operatorname{laplace}(f, t, s)` |
| D11 | 偏微分方程式（熱・波動の変数分離） | **今回はやらない**（2.1）。必要になったら別に計画する | 長方形の領域・よくある境界条件だけ作る |
| D12 | フーリエ級数の係数 | **やる**（IG2 の定積分と文書 7 の EX8 の文字の性質で、$a_n$・$b_n$ を $n$ の式で出す。書き方は `\operatorname{fourier}(f, x, L)`） | 文書 4 の E（周転円）に任せて、ここではやらない |

## 4. ステップの一覧

| # | 内容 | 前提 | 大きさ | 担当 |
|---|---|---|---|---|
| SC0 | 関数の特別な値（有名な角・逆三角関数・対数・累乗根・ガンマとゼータの値） | 文書 7 の EX5 | 中 | B |
| SC1 | 方程式の厳密な解（多項式・有理式・指数と対数・三角方程式・連立方程式の点） | SC0, EX7, EX12 | 大 | A |
| SC2 | 命令（因数分解・展開・部分分数・簡約・解く） | EX9 | 小〜中 | B |
| LA0 | 厳密な行列（掃き出し・階数・核と像・逆行列・行列式。文字を含む行列） | EX5 | 中 | B |
| LA1 | 固有値・固有ベクトル・対角化・ジョルダン標準形・グラム・シュミット | LA0, EX7 | 大 | A |
| DM0 | 数論と組合せ（関数を大きな整数に） | EX5 | 小〜中 | C |
| DM1 | 有限和の閉じた形（Faulhaber・等比・Gosper） | EX6 | 大 | A |
| DM2 | 線形漸化式の一般項（特性方程式・母関数） | EX7 | 中 | B |
| DM3 | 論理式の簡約と真理値表 | — | 小 | C |
| DF0 | 厳密な微分と点での値（微分係数・接線・勾配・ヘッセ行列・極値の点） | SC0 | 中 | B |
| LM0 | 式の級数展開（テイラー・ローラン・ピュイズー・∞ での展開） | DF0 | 大 | A |
| LM1 | 極限（展開・片側・存在しないの証明） | LM0 | 中 | A |
| LM2 | Gruntz のアルゴリズム（∞ で指数と対数が混ざる式） | LM1 | 大 | A |
| LM3 | 多変数の極限の厳密な判定 | LM1, 文書 2 の X0 | 中 | A |
| SR0 | 収束・発散の判定（証明つき） | LM1 | 中 | A |
| SR1 | 級数の値と収束半径 | SR0, EX6 | 大 | A |
| IG0 | 不定積分（規則を当てる） | DF0, EX9 | 大 | A |
| IG1 | 有理関数の積分（Hermite の簡約・対数の部分） | EX6, EX7 | 中 | A |
| IG2 | 定積分・広義積分（原始関数と不連続の確かめ・ガンマとベータの型・表・フーリエ係数） | IG0, IG1, LM1 | 大 | A |
| IG3 | 重積分・線積分・面積分（ベクトル解析） | IG2 | 中 | B |
| OD0 | 1 階の微分方程式（型を見分けて解く） | IG0 | 大 | A |
| OD1 | 2 階以上の線形・連立・ラプラス変換 | OD0, IG1 | 大 | A |
| OD2 | 厳密解への描き替え | OD0 | 中 | B |
| CF0 | 複素数の厳密値と留数・留数による実積分 | SC0, IG1, 文書 6 の Z3 | 中 | B |

**おすすめの順番**: SC0 → DM0 → LA0 → DF0 → SC1 → SC2 → DM3 → LA1 → DM2 → LM0 → LM1 → SR0 → SR1 → IG1 → IG0 → IG2 → OD0 → OD2 → OD1 → DM1 → LM2 → IG3 → CF0 → LM3。
早く効果が出るもの（学校の値・整数・行列・微分係数）を先にし、土台（級数展開・有理関数の積分）を使う分野をその後に置く。DM1（Gosper）と LM2（Gruntz）は難しいわりに例題の割合への効き目が小さいので後ろ。

## 5. ステップごとの指示書

共通の決まり: 文書 7 の 5 章の始めと同じ（コメントと README は日本語、`cargo test --release`・`npm test`・`npm run check`、エラーの中身の確かめ方）。
**どのステップも、例題の表（`core/tests/exact_cases/`）にその分野の行を足してから実装する**（文書 7 の EX0 の形式。出典を書く）。完了の条件の割合はその表で測る。

### SC0: 関数の特別な値

- **担当**: B（理由: 表と規則の仕事。値は照らし合わせで確かめられる）
- **目的**: 学校の「式の値」を正しい形で出す
- **触るファイル**: `core/src/exact/values.rs`（新規）、`exact/term.rs`（`Fn` を作るときに表を引く）、`core/tests/exact_cases/school.tsv`
- **やること**
  1. 三角関数: 角を $\frac{p}{q}\pi$ に直し、$[0, 2\pi)$ に寄せ、対称性で $[0, \frac{\pi}{4}]$ にする。$q \in \{1, 2, 3, 4, 5, 6, 8, 10, 12\}$ の値の表（$\sin\frac{\pi}{5} = \frac{\sqrt{10 - 2\sqrt5}}{4}$、$\tan\frac{\pi}{8} = \sqrt2 - 1$ など）。それ以外は $\sin\frac{\pi}{7}$ のまま
  2. 逆三角関数: 上の表の逆（$\arcsin\frac12 = \frac{\pi}{6}$、$\arccos(-\frac12) = \frac{2\pi}{3}$、$\arctan\sqrt3 = \frac{\pi}{3}$）。値域は主値
  3. 指数と対数: $\log_b a$ を素因数で $\frac{\ln a}{\ln b}$ にして、同じ素数の対数を約す（$\log_4 8 = \frac32$、$\log_{10}2 + \log_{10}5 = 1$）。$e^{k\ln a} = a^k$（$a > 0$）。$\ln(e^3) = 3$
  4. 累乗根（文書 7 の EX2 の続き）: $8^{2/3} = 4$
  5. 特殊関数の値: $\Gamma$ の整数・半整数（$\Gamma(\frac52) = \frac{3\sqrt\pi}{4}$）、$\zeta$ の偶数（ベルヌーイ数）・負の整数（$\zeta(-1) = -\frac{1}{12}$）、
     $\psi$ の有理数（ガウスのディガンマの定理。$\psi(\frac12) = -\gamma - 2\ln 2$）、$B(a, b) = \frac{\Gamma(a)\Gamma(b)}{\Gamma(a+b)}$
  6. 双曲線関数は $e$ の式に直さず、$\sinh 0 = 0$ などの自明な値だけ
- **実装の土台**

  ```rust
  /// 関数 f を引数 args で作るとき、特別な値なら Some。
  pub fn special(f: Func, args: &[Term], a: &Assumptions) -> Option<Term>;
  fn as_pi_multiple(t: &Term) -> Option<Rat>;     // t = (p/q)·π
  fn trig_table(q: u32, p: u32) -> Option<(Term, Term)>; // (sin, cos)。p/q ∈ [0, 1/4]
  ```

- **やらないこと**: 三角関数の式の簡約（EX9）、方程式（SC1）
- **完了の条件**: 学校の表の値の行で 95%。$\sin\frac{\pi}{12} = \frac{\sqrt6 - \sqrt2}{4}$、$\cos\frac{2\pi}{5} = \frac{\sqrt5 - 1}{4}$、$\sin\frac{7\pi}{6} = -\frac12$、$\tan\frac{\pi}{8} = \sqrt2 - 1$、
  $\log_4 8 = \frac32$、$\Gamma(\frac12) = \sqrt\pi$、$\zeta(-1) = -\frac{1}{12}$、$\psi(\frac12) = -\gamma - 2\log 2$、$\sin\frac{\pi}{7}$ はそのまま（近似値つき）
- **壊してはいけないもの**: 文書 7 のテスト
- **README**: 「値の表示」の段落に、厳密に出る関数の値の一覧

### SC1: 方程式の厳密な解

- **担当**: A（理由: 解の取りこぼし・余分な解（両辺を 2 乗したとき・分母が 0 になる所）・定義域（対数の真数）の確かめを落としやすい）
- **目的**: 1 変数の方程式の解と、連立方程式の点を厳密に出す（D1）
- **触るファイル**: `core/src/exact/solve.rs`（新規）、`exact/convert.rs`（方程式の行の入口）、`core/src/equation.rs`（方程式の行・連立方程式の点で厳密値の依頼に必要な形を残す）、
  `web/src/lib/ExpressionItem.svelte`（解の表示）、`core/tests/exact_cases/equations.tsv`
- **やること**
  1. 多項式: 因数分解（EX6）→ 1 次・2 次は公式、3 次・4 次は EX7（D6）、5 次以上の既約因数は `Root`
  2. 有理式: 通分して分子 = 0、分母が 0 になる解を除く
  3. 根号を含む式: 根号を 1 つずつ孤立させて 2 乗し、最後に元の式に入れて余分な解を除く
  4. 指数と対数: $a^{f(x)} = b$ → $f(x) = \log_a b$、$\log f(x) = c$ → $f(x) = e^c$（真数 > 0 を確かめる）、$a^x$ の 2 次式（$4^x - 3\cdot 2^x + 2 = 0$）は置き換え
  5. 三角方程式（D3）: $\sin f(x) = c$ などの基本形を一般解にする。$\sin$ と $\cos$ の 2 次式は置き換え。それ以外は「見つからない」
  6. 連立方程式の点: EX12 の `solve_exact`。点の説明に厳密な座標
  7. 恒等式・解なしも答える（`Solutions::All`・`None`）
  8. 照らし合わせ: 各解を元の方程式に入れて、EX3 の値で 0 になることを確かめる。今の道の f64 の解（縦の直線の位置）とも比べる
- **実装の土台**

  ```rust
  pub fn solve(lhs: &Term, rhs: &Term, var: &str, a: &Assumptions, budget: &Budget) -> Result<Solutions, Stop>;
  ```

- **やらないこと**: 不等式（領域の描き方がある）、超越方程式の一般（$x = \cos x$。見つからない）
- **完了の条件**: 方程式の表で 90%。$x^2 = 2$ → $\pm\sqrt2$、$x^3 - 6x^2 + 11x - 6 = 0$ → $1, 2, 3$、$x^4 - 10x^2 + 1 = 0$ → $\pm\sqrt3 \pm \sqrt2$、$x^3 - 3x + 1 = 0$ → $2\cos\frac{2\pi}{9}$ ほか 2 つ、
  $\sqrt{x + 2} = x$ → $2$（$-1$ は除く）、$4^x - 3\cdot2^x + 2 = 0$ → $0, 1$、$\sin x = \frac12$ → $\frac{\pi}{6} + 2n\pi,\ \frac{5\pi}{6} + 2n\pi$、
  $x^2 + y^2 = 1,\ y = x$ → $\left(\pm\frac{\sqrt2}{2}, \pm\frac{\sqrt2}{2}\right)$（複号同順）、$x + y = 3,\ xy = 2$ → $(1, 2), (2, 1)$
- **壊してはいけないもの**: 方程式の行の今の描き方
- **README**: 「方程式の行」に解の表示

### SC2: 命令

- **担当**: B（理由: 中身は EX6・EX9 にあり、記法と画面の仕事）
- **目的**: 利用者が形を指定できる命令（D2）
- **触るファイル**: `core/src/parser.rs`・`web/src/lib/latex.ts`（`\operatorname{factor}` などの読み）、`core/src/exact/commands.rs`（新規）、
  `core/src/expand.rs`（今の道では中身をそのまま返す。描けるように）、`web/src/lib/keyboardSymbols.ts`、`core/tests/exact_cases/commands.tsv`
- **やること**
  1. `factor`（有理数の上で因数分解。整数なら素因数分解）・`expand`（展開）・`apart`（部分分数。変数は 1 つだけのときに決まる）・`simplify`（EX9）・`solve`（SC1）
  2. 今の道（展開の段）では、命令は中身の値をそのまま返す（$\operatorname{factor}(x^2 - 1)$ のグラフは $x^2 - 1$）
  3. 結果は「行に追加」できる
- **実装の土台**

  ```rust
  pub fn run(cmd: &str, args: &[Term], a: &Assumptions, budget: &Budget) -> Result<Term, Stop>;
  ```

- **やらないこと**: 新しい変形
- **完了の条件**: $\operatorname{factor}(x^4 - 1) = (x - 1)(x + 1)(x^2 + 1)$、$\operatorname{factor}(2^{32} + 1) = 641 \cdot 6700417$、$\operatorname{expand}((x + 1)^3) = x^3 + 3x^2 + 3x + 1$、
  $\operatorname{apart}\left(\frac{x}{x^2 - 3x + 2}\right) = \frac{2}{x - 2} - \frac{1}{x - 1}$、命令の行の今のグラフが中身と同じ
- **壊してはいけないもの**: `factor` などを名前に使っている今の式（定義の名前と衝突しないか確かめる）
- **README**: 命令の一覧

### LA0: 厳密な行列

- **担当**: B（理由: 掃き出しは定型。今の f64 の結果と比べられる）
- **目的**: 有理数と文字の行列を厳密に計算する
- **触るファイル**: `core/src/exact/matrix.rs`（新規）、`exact/convert.rs`（`Matrix` と `det`・`inv`・`rank`・`transpose`・`tr`・`norm`・`dot`・積）、`core/tests/exact_cases/linear.tsv`
- **やること**
  1. 成分が `Term` の行列。四則と積・転置・トレース
  2. 掃き出し（有理数は分数のまま。文字を含むときは分数を作らない Bareiss の方法で行列式）
  3. 階数・簡約された行列・核の基底・像の基底・逆行列（無いときは「逆行列はありません（行列式が 0）」）
  4. 文字を含む行列の行列式・逆行列（$\det\begin{pmatrix} a & 1 \\ 1 & a \end{pmatrix} = a^2 - 1$）。階数は文字の値で変わるので、場合を分けずに「一般の場合」と条件（$a \ne \pm1$）
  5. 行列の描き方（M7）の表の中の成分を厳密値で出す
- **実装の土台**

  ```rust
  pub struct EMat { pub rows: usize, pub cols: usize, pub a: Vec<Term> }
  pub fn rref(m: &EMat, budget: &Budget) -> Result<(EMat, Vec<usize> /* 主成分の列 */), Stop>;
  pub fn det(m: &EMat, budget: &Budget) -> Result<Term, Stop>;
  pub fn kernel(m: &EMat, budget: &Budget) -> Result<Vec<Vec<Term>>, Stop>;
  ```

- **やらないこと**: 固有値（LA1）
- **完了の条件**: 線形代数の表の掃き出しの行で 95%。$\begin{pmatrix} 1&2&3\\4&5&6\\7&8&10 \end{pmatrix}^{-1} = \begin{pmatrix} -\frac23&-\frac43&1\\-\frac23&\frac{11}{3}&-2\\1&-2&1 \end{pmatrix}$、
  $\operatorname{rank}\begin{pmatrix}1&2\\2&4\end{pmatrix} = 1$ と核 $(-2, 1)$、$\det\begin{pmatrix}a&1\\1&a\end{pmatrix} = a^2 - 1$、ヒルベルト行列 $6 \times 6$ の逆行列が整数（f64 では誤差が出る）
- **壊してはいけないもの**: 行列の今の計算（f64 の道）
- **README**: 行列の段落に厳密値

### LA1: 固有値と標準形

- **担当**: A（理由: 重なった固有値・ジョルダン鎖・根号の固有ベクトルは、場合分けを落としやすい）
- **目的**: 固有値・固有ベクトル・対角化・ジョルダン標準形・正規直交化を厳密に出す
- **触るファイル**: `core/src/exact/matrix.rs`、`exact/convert.rs`（`eig`・新しい `diag`・`jordan`・`gramschmidt`）、`core/tests/exact_cases/linear.tsv`
- **やること**
  1. 特性多項式（文字を含んでよい）→ 因数分解（EX6）→ 根（EX7。4 次まで根号、それ以外は `Root`）
  2. 固有ベクトル: $(A - \lambda I)$ の核（$\lambda$ が根号なら $\mathbb Q(\lambda)$ の中で掃き出す）
  3. 対角化: $P$・$D$。できないときは「対角化できません（固有値 1 の重なり 2、固有ベクトル 1 本）」
  4. ジョルダン標準形（D4 の大きさまで）: 一般固有ベクトルの鎖
  5. グラム・シュミット（D4）: 正規化した基底。根号は EX7 で整える
- **実装の土台**

  ```rust
  pub fn eigen(m: &EMat, budget: &Budget) -> Result<Vec<(Term /* λ */, u32 /* 重なり */, Vec<Vec<Term>> /* 固有ベクトル */)>, Stop>;
  pub fn jordan(m: &EMat, budget: &Budget) -> Result<(EMat /* P */, EMat /* J */), Stop>;
  ```

- **やらないこと**: 特異値分解の厳密値
- **完了の条件**: 線形代数の表の固有値の行で 90%。$\begin{pmatrix}2&1\\1&2\end{pmatrix}$ → $1, 3$、$\begin{pmatrix}4&1\\2&3\end{pmatrix}$ → $P = \begin{pmatrix}-1&1\\2&1\end{pmatrix}$、$D = \operatorname{diag}(2, 5)$、
  $\begin{pmatrix}0&1&0\\0&0&1\\2&-5&4\end{pmatrix}$ の特性多項式 $(\lambda - 1)^2(\lambda - 2)$ とジョルダン標準形 $J_2(1) \oplus (2)$、
  $(1,1,0), (1,0,1), (0,1,1)$ のグラム・シュミット → $\frac{1}{\sqrt2}(1,1,0)$、$\frac{1}{\sqrt6}(1,-1,2)$、$\frac{1}{\sqrt3}(-1,1,1)$
- **壊してはいけないもの**: `eig` の今の数値の道
- **README**: 固有値の段落

### DM0: 数論と組合せ

- **担当**: C（理由: 文書 7 の EX1 の関数を記法につなぐ定型の仕事）
- **目的**: f64 では壊れる整数の関数を全部正しくする
- **触るファイル**: `core/src/exact/numtheory.rs`（新規）、`exact/convert.rs`（`gcd`・`lcm`・`mod`・`nCr`・`!`・新しい `totient`・`modinv`・`stirling2`・`isprime`）、`core/src/functions.rs`（新しい関数の f64 の道）、`core/tests/exact_cases/discrete.tsv`
- **やること**
  1. 既存の関数を厳密値の道で計算: $\gcd$・$\operatorname{lcm}$・$\bmod$・$\binom{n}{k}$・$n!$・べき
  2. 新しい関数（f64 の道は整数のときだけ値、ほかは誤り）: $\varphi(n)$・合同式の逆元・第 2 種スターリング数・素数判定・素因数分解（SC2 の `factor`）
  3. 合同式のべき（$2^{100} \bmod 13$）は、べきを作らずに計算する（式の形 `mod(a^b, m)` を見つける）
- **実装の土台**

  ```rust
  pub fn mod_pow(a: &Int, e: &Int, m: &Int) -> Int;
  ```

- **やらないこと**: 中国剰余定理の記法（必要なら後で）
- **完了の条件**: 離散の表の数論の行で 95%。$\varphi(36) = 12$、$3^{-1} \equiv 5 \pmod 7$、$2^{100} \bmod 13 = 3$、$S(5, 2) = 15$、$\binom{100}{50}$ の全桁
- **壊してはいけないもの**: `special_values.rs` の f64 のテスト
- **README**: 関数の一覧

### DM1: 有限和の閉じた形

- **担当**: A（理由: Gosper のアルゴリズムは、有理関数の分解（Gosper の形）を間違えると、もっともらしい間違った和を出す）
- **目的**: $\sum_{k=1}^{n}$ の閉じた形を、$n$ の式で出す
- **触るファイル**: `core/src/exact/finite_sum.rs`（新規）、`exact/convert.rs`（`sum` の上端が文字のとき）、`core/tests/exact_cases/discrete.tsv`
- **やること**
  1. 多項式の和: Faulhaber（ベルヌーイ数）
  2. 等比と多項式 × 等比: 差分の逆を未定係数で
  3. Gosper のアルゴリズム（項の比が有理関数の項。超幾何項）。見つからなければ「閉じた形は見つかりません」
  4. 部分分数で望遠鏡のように消える和（$\sum\frac{1}{k(k+1)} = \frac{n}{n+1}$）
  5. 確かめ: $S(n) - S(n-1)$ が項に等しいこと（記号で）と、$n = 1..20$ の値
- **実装の土台**

  ```rust
  pub fn closed_form(term: &Term, k: &str, lo: &Term, hi: &Term, budget: &Budget) -> Result<Option<Term>, Stop>;
  fn gosper(term: &Term, k: &str, budget: &Budget) -> Result<Option<Term> /* 不定和 */, Stop>;
  ```

- **やらないこと**: Zeilberger のアルゴリズム（$\sum_k \binom{n}{k} = 2^n$ のような定和は表で持つ）
- **完了の条件**: 離散の表の和の行で 90%。$\sum k^2 = \frac{n(n+1)(2n+1)}{6}$、$\sum_{k=1}^{n} k\,2^k = (n-1)2^{n+1} + 2$、$\sum_{k=0}^{n} k\cdot k! = (n+1)! - 1$、
  $\sum_{k=1}^{n}\frac{1}{k(k+1)} = \frac{n}{n+1}$、$\sum_{k=0}^{n}\binom{n}{k} = 2^n$（表）、$\sum_{k=1}^{n}\frac1k$ は閉じた形なし（$H_n$ のまま）
- **壊してはいけないもの**: 上端が数の和（今の道）
- **README**: 和の段落

### DM2: 線形漸化式の一般項

- **担当**: B（理由: 特性方程式は EX7 の根を使う定型の手順）
- **目的**: 数列の漸化式（M10 の書き方）から一般項を出す
- **触るファイル**: `core/src/exact/recurrence.rs`（新規）、`exact/convert.rs`（数列の定義 `Seq` を読む）、`core/tests/exact_cases/discrete.tsv`
- **やること**
  1. 定数係数の線形の漸化式（同次・非同次で右辺が多項式 × 等比）: 特性方程式の根から一般項、初項から定数を決める
  2. 重根は $(A + Bn)r^n$。複素根は実数の形（$r^n\cos n\theta$）
  3. 1 階の $a_{n+1} = p\,a_n + q$ は公式
  4. 一般項を行の右下に出し、「行に追加」で $a_n$ の式として描ける
- **実装の土台**

  ```rust
  pub fn general_term(coeffs: &[Term], rhs: &Term, initial: &[Term], n: &str, budget: &Budget) -> Result<Option<Term>, Stop>;
  ```

- **やらないこと**: 係数が $n$ の式の漸化式（Petkovšek の Hyper）
- **完了の条件**: $a_{n+2} = a_{n+1} + a_n$、$a_0 = 0, a_1 = 1$ → $\frac{1}{\sqrt5}\left(\left(\frac{1+\sqrt5}{2}\right)^n - \left(\frac{1-\sqrt5}{2}\right)^n\right)$、
  $a_{n+1} = 2a_n + 1$、$a_0 = 0$ → $2^n - 1$、$a_{n+2} = 4a_{n+1} - 4a_n$ → $(A + Bn)2^n$ の形。$n = 0..30$ で今の数列の値と一致
- **壊してはいけないもの**: 数列の描き方
- **README**: 数列の段落

### DM3: 論理式の簡約

- **担当**: C（理由: Quine–McCluskey は定型。真理値表で全部確かめられる）
- **目的**: 命題の論理式を、いちばん簡単な選言標準形と真理値表にする
- **触るファイル**: `core/src/exact/boolean.rs`（新規）、`core/src/logic.rs`（式の読み）、`core/tests/exact_cases/discrete.tsv`
- **やること**: 変数 8 個まで。Quine–McCluskey と、主項の選び方（小さいときは全部試す、大きいときは Petrick の方法）。真理値表は行の右下の「表を開く」で
- **実装の土台**

  ```rust
  pub fn minimize(f: &Formula, vars: &[String]) -> Result<Formula, String>;
  ```

- **やらないこと**: 述語論理
- **完了の条件**: $(A \land B) \lor (A \land \lnot B) = A$、$\lnot(A \lor B) \lor (\lnot A \land B) = \lnot A$、変数 4 個の乱数の式 1000 個で、真理値表が元と同じで項の数が最小
- **壊してはいけないもの**: 集合・論理の描き方（M11）
- **README**: 論理式の段落

### DF0: 厳密な微分と点での値

- **担当**: B（理由: 規則は `diff.rs` の表と同じ。点での値は SC0 の値の表を使う）
- **目的**: 微分係数・接線・勾配・ヘッセ行列・極値の点を厳密に出す
- **触るファイル**: `core/src/exact/diff.rs`（新規）、`exact/convert.rs`（`diff`・`at`・`grad`・`div`・`rot`・`lap`・`D_{\mathbf u}`（文書 2 の W0））、`core/src/critical.rs`（停留点の厳密な座標の依頼）、`core/tests/exact_cases/calculus.tsv`
- **やること**
  1. `Term` の微分（`diff.rs` の表をそのまま移す。中心差分の代わりは無いので、表に無い関数は「見つからない」）
  2. 点での値（`at`）: 微分してから入れる。折れ目（$|x|$ の 0）では左右の微分係数を LM1 の片側の極限で求め、違えば「存在しません」（今の道の文と同じ）。LM1 の前は「見つからない」
  3. 接線・接平面の式（陽関数の点を押したとき。N4 の点の説明に「接線 $y = \frac14 x + 1$」）
  4. 停留点（`@crit`）: 多項式なら EX12 で厳密な座標と分類（ヘッセ行列の厳密な主小行列式）
- **実装の土台**

  ```rust
  pub fn diff(t: &Term, var: &str, n: u32, budget: &Budget) -> Result<Term, Stop>;
  ```

- **やらないこと**: 陰関数の微分の一般（$\frac{dy}{dx} = -\frac{F_x}{F_y}$ に点を入れる形だけやる）
- **完了の条件**: 微分の表で 95%。$\left.\frac{d}{dx}x^3\sin x\right|_{x=\pi/3} = \frac{\pi^2(\pi + 9\sqrt3)}{54}$、$\left.\frac{d^2}{dx^2}e^x\cos x\right|_{x=0} = 0$、
  $y = \sqrt x$ の $x = 4$ の接線 $y = \frac{x}{4} + 1$、$\nabla(x^2y)$ の $(1, 2)$ での値 $(4, 1)$、$x^3 - 3x$ の停留点 $(\pm1, \mp2)$ と分類
- **壊してはいけないもの**: 今の微分の描き方
- **README**: 微分の段落

### LM0: 式の級数展開

- **担当**: A（理由: 打ち切りの次数の管理（相殺で先頭の項が消えるとき、もっと高い次数まで要る）と、対数・分数べきの項を間違えやすい）
- **目的**: 極限・級数の判定・テイラー展開の土台になる、式の級数展開
- **触るファイル**: `core/src/exact/series.rs`（新規）、`exact/convert.rs`（`taylor`）、`core/tests/exact_series.rs`（新規）
- **やること**
  1. 展開の形: $\sum c_k (x - a)^{k/d}$（$d$ は分母。ピュイズー級数）と、$\log(x - a)$ を係数に含む形
  2. 四則・合成・べき・$\exp$・$\log$・三角関数・逆三角関数の展開（葉から順に。`series.rs` の漸化式を厳密な係数で）
  3. 先頭の項が相殺で消えたら、打ち切りの次数を上げてやり直す（上限 40 次）
  4. $x \to \infty$ は $x = \frac1t$ にする
  5. `taylor` の記法（今の道）を厳密な係数でも出す
- **実装の土台**

  ```rust
  pub struct Series { pub var: String, pub at: Point, pub den: u32, pub terms: Vec<(i64 /* 指数 × den */, Term /* 係数（log を含んでよい） */)>, pub order: i64 /* O(x^(order/den)) */ }
  pub fn expand(t: &Term, var: &str, at: Point, order: i64, budget: &Budget) -> Result<Series, Stop>;
  ```

- **やらないこと**: 本質的特異点のまわり（$e^{1/x}$ の 0）の展開（LM2 に任せる）
- **完了の条件**: $\tan x = x + \frac{x^3}{3} + \frac{2x^5}{15} + \frac{17x^7}{315} + O(x^9)$、$\frac{1}{1-x} - e^x = \frac{x^2}{2} + \frac{5x^3}{6} + O(x^4)$、
  $\frac{1}{\sin x} = \frac1x + \frac{x}{6} + \frac{7x^3}{360} + O(x^5)$、$\sqrt{x + x^2} = x^{1/2} + \frac{x^{3/2}}{2} - \frac{x^{5/2}}{8} + O(x^{7/2})$、
  $\sin x - x + \frac{x^3}{6}$ の先頭が $\frac{x^5}{120}$（相殺）
- **壊してはいけないもの**: `taylor` の今の描き方
- **README**: テイラー展開の段落

### LM1: 極限

- **担当**: A（理由: 片側・振動・定義域（$x^x$ は $x > 0$ だけ）の判断を間違えると、「存在しない」と「見つからない」を取り違える）
- **目的**: 1 変数の極限を厳密に出す。「存在しない」は証明できるときだけ言う
- **触るファイル**: `core/src/exact/limit.rs`（新規）、`exact/convert.rs`（`lim`・`seqlim`）、`core/tests/exact_cases/limits.tsv`
- **やること**
  1. まず代入（連続なら値）。だめなら LM0 で展開して先頭の項
  2. 片側: 指定があればその側。両側なら左右を別に求め、違えば「存在しません（左 $-1$、右 $1$）」。定義域が片側だけ（$x^x$、$\sqrt x$）なら、その側の値（説明に「右からの極限」）
  3. $\pm\infty$ に発散するときは「$+\infty$ に発散します」
  4. 振動の証明: 2 つの数列（$\sin\frac1x$ なら $x = \frac{1}{2n\pi}$ と $x = \frac{1}{2n\pi + \pi/2}$）で違う値になることを、式で示せる型（三角関数の中身が $\infty$ へ行く）だけ。それ以外は「見つからない」
  5. LM0 で展開できない式（$\infty$ で指数と対数が混ざる）は LM2 に回す（LM2 の前は「見つからない」）
- **実装の土台**

  ```rust
  pub enum LimitResult { Value(Term), PlusInf, MinusInf, NotExist(String), Unknown }
  pub fn limit(t: &Term, var: &str, to: Point, side: Side, a: &Assumptions, budget: &Budget) -> Result<LimitResult, Stop>;
  ```

- **やらないこと**: 多変数（LM3）
- **完了の条件**: 極限の表で 85%（LM2 の後で 90%）。$\frac{1 - \cos x}{x^2} \to \frac12$、$\frac{\tan x - \sin x}{x^3} \to \frac12$、$\sqrt{x^2 + x} - x \to \frac12$（$x \to \infty$）、
  $x^x \to 1$（右から）、$\left(1 + \frac1x\right)^x \to e$、$\frac{e^x - 1 - x}{x^2} \to \frac12$、$x\log x \to 0$（右から）、$\frac{|x|}{x}$ は存在しない、$\sin\frac1x$ は存在しない
- **壊してはいけないもの**: 極限の結果パネル（文書 2・`LimitLesson.svelte`）
- **README**: 極限の段落

### LM2: Gruntz のアルゴリズム

- **担当**: A（理由: 「いちばん速く大きくなる部分」の比べ方と書き換えを間違えると、もっともらしい間違った極限を出す）
- **目的**: $\infty$ で指数と対数が混ざる式の極限（$\frac{x^{100}}{e^x}$、$\frac{\ln\ln x}{\ln x}$、$\frac{e^x}{e^{x + 1/x}}$）
- **触るファイル**: `core/src/exact/gruntz.rs`（新規）、`exact/limit.rs`、`core/tests/exact_cases/limits.tsv`
- **やること**: Gruntz の論文の手順（いちばん速い段の部分式の集合 → 1 つの $\omega$ に書き換える → $\omega$ で展開 → 先頭の項の符号と指数）。SymPy の `gruntz.py` を読みながら作る。
  調和数 $H_n$ のような離散の関数は、漸近展開の表（$H_n = \ln n + \gamma + \frac{1}{2n} - \cdots$）に置き換えてから渡す
- **実装の土台**

  ```rust
  fn mrv(t: &Term, x: &str) -> Vec<Term>;          // いちばん速く変わる部分式の集合
  fn rewrite(t: &Term, mrv: &[Term], omega: &Term) -> Term;
  pub fn gruntz(t: &Term, x: &str, budget: &Budget) -> Result<LimitResult, Stop>; // x → +∞
  ```

- **やらないこと**: 三角関数が $\infty$ へ行く式（振動。LM1 の 4 だけ）
- **完了の条件**: $\frac{x^{100}}{e^x} \to 0$、$\frac{\ln\ln x}{\ln x} \to 0$、$x^{1/x} \to 1$、$\frac{e^x}{e^{x + 1/x}} \to 1$、$H_n - \ln n \to \gamma$。極限の表で 90%
- **壊してはいけないもの**: LM1 のテスト
- **README**: 書かない

### LM3: 多変数の極限の厳密な判定

- **担当**: A（理由: 経路を選ぶ所で、存在しない極限を見逃しやすい（$y = mx$ では同じでも $y = x^2$ で違う））
- **目的**: 文書 2 の X0 の数値の判定に、厳密な「存在しない」の証明と、有理関数の「存在する」の証明を足す
- **触るファイル**: `core/src/exact/multilimit.rs`（新規）、`core/src/multilimit.rs`（文書 2。結果の言葉を「証明つき」にする）
- **やること**
  1. 存在しない: 直線 $y = m(x - a) + b$ を代入して LM1 で厳密な極限を求め、$m$ によって変われば証明。変わらなければ $y = b + c(x - a)^k$（$k = 2, 3, \frac12$）でも試す
  2. 存在する（有理関数だけ）: 点を原点に移し、分母が正定値の同次式で、分子の次数 > 分母の次数なら 0（不等式 $|f| \le C r^{\deg N - \deg D}$）
  3. それ以外は、文書 2 の数値の言葉（「どの経路でも $L$ に近づいて見えます」）のまま
- **実装の土台**

  ```rust
  pub fn multi_limit(t: &Term, vars: &[&str], at: &[Term], budget: &Budget) -> Result<MultiVerdict, Stop>;
  pub enum MultiVerdict { NotExist { path_a: Term, path_b: Term, va: Term, vb: Term }, Exists(Term), Unknown }
  ```

- **やらないこと**: 一般の「存在する」の証明
- **完了の条件**: $\frac{xy}{x^2 + y^2}$ は存在しない（$y = mx$ で $\frac{m}{1 + m^2}$）、$\frac{x^2y}{x^4 + y^2}$ は存在しない（$y = mx$ では 0、$y = x^2$ では $\frac12$）、$\frac{x^2y}{x^2 + y^2} \to 0$（存在する）
- **壊してはいけないもの**: 文書 2 の X0・X1
- **README**: 多変数の極限の段落の言葉

### SR0: 収束・発散の判定

- **担当**: A（理由: 判定法の条件（項が正・単調・0 へ）を 1 つ落とすと、嘘の証明になる。$\sum\frac{1}{k\ln k}$ が落とし穴）
- **目的**: 文書 2 の X3 の「判定の目安」を、厳密に示せるときは「判定（証明つき）」にする（D9）
- **触るファイル**: `core/src/exact/convergence.rs`（新規）、`core/src/convergence.rs`（文書 2。結果の言葉）、`core/tests/exact_cases/series.tsv`
- **やること**（上から順に試す）
  1. 項が 0 に近づかない（LM1）→ 発散
  2. 項を $k \to \infty$ で展開（LM0）: $a_k \sim Ck^{-p}$ で、項の符号が最後には一定なら、$p > 1$ で収束・$p \le 1$ で発散（比較判定法）
  3. 比 $\frac{a_{k+1}}{a_k}$ の極限（LM1・LM2）: $< 1$ なら収束、$> 1$ なら発散
  4. 交代級数: 符号が交互で、$|a_k|$ が最後には単調に減って 0 へ（単調は $|a_k|$ を連続の関数として微分の符号で確かめる）→ 収束（ライプニッツ）。絶対収束かも 2 で調べる
  5. 積分判定法の型: $a_k = f(k)$ で $f$ が正・減少、$\int^\infty f$ を IG2 で（$\frac{1}{k\ln k}$ → $\ln\ln k$ → 発散）
  6. どれでも決まらなければ、文書 2 の目安のまま
- **実装の土台**

  ```rust
  pub enum Verdict { Converges { absolutely: bool, test: &'static str, detail: Term }, Diverges { test: &'static str, detail: Term }, Unknown }
  pub fn judge(term: &Term, k: &str, from: &Int, a: &Assumptions, budget: &Budget) -> Result<Verdict, Stop>;
  ```

- **やらないこと**: 二重級数
- **完了の条件**: 級数の判定の表で 90%。$\sum\frac{1}{k^2}$ 収束（$p = 2$）、$\sum\frac1k$ 発散、$\sum\frac{1}{k\ln k}$ 発散（積分判定法）、$\sum\frac{(-1)^k}{k}$ 条件収束、
  $\sum\frac{k!}{k^k}$ 収束（比 $\to \frac1e$）、$\sum\sin\frac1k$ 発散（$\sim\frac1k$）、$\sum\frac{k^2}{2^k}$ 収束
- **壊してはいけないもの**: 文書 2 の X3 のパネル
- **README**: 収束の段落の言葉（「証明つき」と「目安」の違い）

### SR1: 級数の値と収束半径

- **担当**: A（理由: 部分分数からディガンマ関数の値に直す所と、和の始まりの番号のずれを間違えやすい）
- **目的**: 収束する無限級数の値を閉じた形で出す
- **触るファイル**: `core/src/exact/infinite_sum.rs`（新規）、`exact/convert.rs`（`sum` の上端が $\infty$）、`core/tests/exact_cases/series.tsv`
- **やること**（上から順に試す）
  1. 等比級数とその微分の形（$\sum k^m r^k$）
  2. 項が有理関数: 部分分数（EX6。分母の根は EX7）→ 根の差が整数なら望遠鏡で有限の和、そうでなければ $\sum\frac{1}{(k + a)^s}$ の形の和に分け、
     $s = 1$ の部分はディガンマ関数 $-\sum c_j\psi(a_j)$（係数の和が 0 のときだけ収束）、$s \ge 2$ はフルヴィッツのゼータ。有理数での値を SC0 の表で（ガウスの定理・$\zeta(2m)$）
  3. 交代の有理関数（$(-1)^k$ を掛けた形）: 偶数と奇数の項に分けて 2 に戻す
  4. べき級数の表（20 個ほど: $e^x$・$\sin$・$\cos$・$\sinh$・$\cosh$・$\log(1 + x)$・$\arctan$・$(1 + x)^a$・$\frac{1}{1 - x}$ とその微分）に、置き換え（$x \to cx^m$）で当てる。
     項の比が有理関数なら超幾何級数として表を引く（$\sum\frac{x^k}{k!}$・$\sum\frac{x^k}{k}$・$\sum\binom{2k}{k}x^k$）
  5. 見つからなければ EX10 の高い精度の値と EX11 の推定へ
  6. 収束半径: 係数の比か $k$ 乗根の極限（LM1）
- **実装の土台**

  ```rust
  pub fn infinite_sum(term: &Term, k: &str, from: &Int, a: &Assumptions, budget: &Budget) -> Result<Option<Term>, Stop>;
  pub fn radius(coeff: &Term, k: &str, budget: &Budget) -> Result<Option<Term>, Stop>;
  ```

- **やらないこと**: 二重級数・多重ゼータ値
- **完了の条件**: 級数の値の表で 85%。$\sum_{k\ge1}\frac{1}{k(k+1)} = 1$、$\sum_{k\ge0}\frac{k}{2^k} = 2$、$\sum\frac{1}{k!} = e$、$\sum\frac{1}{k^2} = \frac{\pi^2}{6}$、$\sum\frac{(-1)^{k+1}}{k} = \log 2$、
  $\sum\frac{1}{k^4} = \frac{\pi^4}{90}$、$\sum_{k\ge1}\frac{1}{(2k-1)^2} = \frac{\pi^2}{8}$、$\sum_{k\ge0}\frac{(-1)^k}{2k+1} = \frac{\pi}{4}$、$\sum_{k\ge1}\frac{1}{4k^2 - 1} = \frac12$、
  $\sum_{k\ge0}\frac{(-1)^k}{(2k+1)^2} = G$、$\sum\frac{1}{k^3} = \zeta(3)$、$\sum_{k\ge0}\frac{k^2}{3^k} = \frac32$、$\sum_{k\ge1}\frac{1}{k(k+2)} = \frac34$、$\sum_{k\ge1}\frac{x^k}{k} = -\log(1 - x)$（$|x| < 1$）。
  収束半径: $\sum\frac{x^k}{k!}$ は $\infty$、$\sum kx^k$ は 1
- **壊してはいけないもの**: ∞ の和の今の数値の道
- **README**: 級数の値の段落

### IG0: 不定積分

- **担当**: A（理由: 置換・部分積分の規則が多く、規則の当て方の順番で答えの形と成否が変わる。間違いは微分して戻ることで確かめられるが、形の良し悪しは確かめにくい）
- **目的**: 不定積分を、教科書の答えの形で出す（D5）
- **触るファイル**: `core/src/exact/integrate.rs`（新規）、`exact/convert.rs`（`int` の端なし）、`core/tests/exact_cases/integrals.tsv`（Rubi の例題から教養の範囲を抜き出す。出典とライセンスを書く）
- **やること**（SymPy の `manualintegrate` と Rubi の考え方。上から順に試す）
  1. 線形性と表（基本の関数・中身が 1 次式）
  2. 中身の微分が外にある置換（$\int f(g(x))g'(x)\,dx$。$g$ の候補は部分式から集め、$\frac{\text{被積分関数}}{g'}$ が $g$ だけで書けるか調べる）
  3. 部分積分（LIATE の順: 対数・逆三角・代数・三角・指数）。$e^x\sin x$ のような戻ってくる形は方程式として解く
  4. 三角関数のべき（$\sin^m\cos^n$）、$\tan$・$\sec$ のべき
  5. 根号の置換: $\sqrt{a^2 - x^2}$・$\sqrt{x^2 + a^2}$・$\sqrt{x^2 - a^2}$ の三角・双曲置換、$\sqrt{ax + b}$ の置換
  6. 有理関数は IG1、三角関数の有理式は $t = \tan\frac{x}{2}$（最後の手段）
  7. 答えを EX9 で整え、**微分して元に戻ること**を確かめる（記号で 0 にならなければ EX3 の数値で 3 点）
  8. 見つからなければ「見つからない」（今の道は $\int_0^x$ を数値で描くまま）
- **実装の土台**

  ```rust
  pub fn antiderivative(t: &Term, x: &str, a: &Assumptions, budget: &Budget) -> Result<Option<Term>, Stop>;
  type Rule = fn(&Term, &str, &mut Ctx) -> Result<Option<Term>, Stop>; // 規則の表。rules に名前を記録（文書 7 の D4）
  ```

- **やらないこと**: Risch のアルゴリズム（「初等関数で書けない」の証明）
- **完了の条件**: 不定積分の表で 85%。$\int x^2e^x\,dx = (x^2 - 2x + 2)e^x + C$、$\int x\log x\,dx = \frac{x^2}{2}\log x - \frac{x^2}{4} + C$、$\int\sin^3 x\,dx = -\cos x + \frac{\cos^3 x}{3} + C$、
  $\int e^x\sin x\,dx = \frac{e^x(\sin x - \cos x)}{2} + C$、$\int\sqrt{1 - x^2}\,dx = \frac{x\sqrt{1 - x^2} + \arcsin x}{2} + C$、$\int\frac{dx}{\sqrt{x^2 + 1}} = \log\left(x + \sqrt{x^2 + 1}\right) + C$、
  $\int x\sqrt{x^2 + 1}\,dx = \frac{(x^2 + 1)^{3/2}}{3} + C$、$\int\frac{dx}{x\log x} = \log|\log x| + C$、$\int\frac{dx}{\cos x} = \log\left|\frac{1}{\cos x} + \tan x\right| + C$。全部の答えが微分して戻る
- **壊してはいけないもの**: 不定積分の行の今の描き方
- **README**: 不定積分の段落（答えの形・+ C・絶対値）

### IG1: 有理関数の積分

- **担当**: A（理由: 対数の部分（Rothstein–Trager）は代数的数の上の計算で、係数の取り違えが起きやすい）
- **目的**: 有理関数の不定積分を必ず求める（有理関数の積分はいつも初等関数で書ける）
- **触るファイル**: `core/src/exact/rational_int.rs`（新規）、`exact/integrate.rs`（有理関数の規則として呼ぶ）
- **やること**: Hermite の簡約で有理の部分と対数の部分に分け、対数の部分は Lazard–Rioboo–Trager（終結式）。答えに複素数の対数が出たら、実数の $\arctan$ の形に直す（Rioboo の方法）。
  分母が 2 次までの因数に分かれるときは、学校の形（部分分数と $\arctan$）にする
- **実装の土台**

  ```rust
  pub fn integrate_rational(num: &Poly, den: &Poly, x: &str, budget: &Budget) -> Result<Term, Stop>;
  ```

- **やらないこと**: 代数関数の積分
- **完了の条件**: $\int\frac{x}{x^2 - 3x + 2}dx = 2\log|x - 2| - \log|x - 1| + C$、$\int\frac{dx}{x^3 + 1} = \frac13\log|x + 1| - \frac16\log(x^2 - x + 1) + \frac{1}{\sqrt3}\arctan\frac{2x - 1}{\sqrt3} + C$、
  $\int\frac{dx}{x^4 + 1}$ が実数の形、乱数の有理関数 200 個で微分して戻る
- **壊してはいけないもの**: IG0 のテスト
- **README**: 書かない

### IG2: 定積分・広義積分

- **担当**: A（理由: 原始関数が区間の中で不連続（$\int_0^{2\pi}\frac{dx}{2 + \cos x}$ で $\tan\frac x2$ の置換）なのに、端の値の差をとると間違える）
- **目的**: 定積分を厳密に出す。広義積分は極限で
- **触るファイル**: `core/src/exact/definite.rs`（新規）、`exact/convert.rs`（`int` の端あり・`pvint`）、`core/tests/exact_cases/integrals.tsv`
- **やること**
  1. 対称性: 奇関数の対称な区間は 0（収束するときだけ）
  2. 原始関数（IG0・IG1）があれば、区間の中の不連続の点（分母の 0・$\tan$ の極・対数の枝）を探して区間を分け、各区間の端を LM1 の片側の極限で計算する
  3. 広義の端（$\pm\infty$・端の特異点）は極限。発散すれば「発散します」
  4. 表と型（原始関数が無くても値が出るもの）: ガンマの型 $\int_0^\infty x^a e^{-bx^c}dx = \frac{\Gamma\left(\frac{a+1}{c}\right)}{c\,b^{(a+1)/c}}$、ベータの型 $\int_0^1 x^a(1 - x)^b dx$・$\int_0^\infty\frac{x^a}{(1 + x^b)^c}dx$・$\int_0^{\pi/2}\sin^p\theta\cos^q\theta\,d\theta$、
     ディリクレ $\int_0^\infty\frac{\sin x}{x}dx = \frac{\pi}{2}$、$\int_0^\infty e^{-x}\log x\,dx = -\gamma$、ガウス積分
  5. 中身を級数にして項ごとに積分（$\int_0^1\frac{\arctan x}{x}dx = \sum\frac{(-1)^k}{(2k+1)^2} = G$。SR1 につなぐ）
  6. フーリエ係数（D12）: $\operatorname{fourier}(f, x, L)$ で $a_n$・$b_n$ を $n$ の式で（$n$ は自然数。EX8）
- **実装の土台**

  ```rust
  pub fn definite(t: &Term, x: &str, lo: &Bound, hi: &Bound, a: &Assumptions, budget: &Budget) -> Result<IntResult, Stop>;
  pub enum IntResult { Value(Term), Diverges(String), Unknown }
  fn discontinuities(anti: &Term, x: &str, lo: &Bound, hi: &Bound) -> Vec<Term>;
  ```

- **やらないこと**: Meijer の G 関数（2.1）
- **完了の条件**: 定積分の表で 85%。$\int_0^\pi\sin^2x\,dx = \frac{\pi}{2}$、$\int_1^2\log x\,dx = 2\log2 - 1$、$\int_0^\infty e^{-x^2}dx = \frac{\sqrt\pi}{2}$、$\int_0^\infty x^3e^{-x}dx = 6$、
  $\int_{-\infty}^{\infty}\frac{dx}{x^2 + 1} = \pi$、$\int_0^1 x^2(1 - x)^3dx = \frac{1}{60}$、$\int_0^\infty\frac{dx}{1 + x^4} = \frac{\sqrt2\,\pi}{4}$、$\int_0^\infty e^{-x}\log x\,dx = -\gamma$、
  $\int_0^{\pi/2}\sin^4x\cos^2x\,dx = \frac{\pi}{32}$、$\int_0^1\frac{dx}{\sqrt x} = 2$、$\int_0^{2\pi}\frac{dx}{2 + \cos x} = \frac{2\pi}{\sqrt3}$（不連続の確かめ）、$\int_0^\infty\frac{\sin x}{x}dx = \frac{\pi}{2}$、
  $\int_0^1\frac{\arctan x}{x}dx = G$、$\int_{-1}^{1}\frac{dx}{x}$ は発散。矩形波 $f(x) = \operatorname{sgn} x$（$-\pi < x < \pi$）のフーリエ係数 $a_n = 0$・$b_n = \frac{2(1 - (-1)^n)}{n\pi}$
- **壊してはいけないもの**: 定積分の今の数値の道
- **README**: 定積分の段落

### IG3: 重積分・線積分・面積分

- **担当**: B（理由: IG2 を繰り返し使う。領域の端の式は今の道（`intset`・`region.rs`）が持っている）
- **目的**: 重積分と、ベクトル解析の積分を厳密に出す
- **触るファイル**: `core/src/exact/multiple.rs`（新規）、`exact/convert.rs`（`intset`・積分の入れ子）、`core/tests/exact_cases/vector.tsv`
- **やること**
  1. 累次積分: 内側から IG2。端が外の変数の式でもよい
  2. 領域 $\{(x, y) \mid \ldots\}$: 今の道の端の求め方（`isolate`）を厳密な端にする。円・扇形は極座標に直す（ヤコビアン $r$）
  3. 線積分・面積分: 媒介変数で 1 変数・2 変数の積分に直す
  4. ガウス・ストークス・グリーンの定理で、両辺を計算して一致を確かめる例をテストに置く
- **実装の土台**

  ```rust
  pub fn iterated(t: &Term, bounds: &[(String, Bound, Bound)], a: &Assumptions, budget: &Budget) -> Result<IntResult, Stop>;
  ```

- **やらないこと**: 一般の領域（端が解けない不等式）
- **完了の条件**: 重積分・ベクトル解析の表で 80%。$\int_0^1\int_0^{1-x}xy\,dy\,dx = \frac{1}{24}$、$\iint_{\mathbb R^2}e^{-(x^2 + y^2)}dA = \pi$、単位円の $\oint(-y\,dx + x\,dy) = 2\pi$、
  単位球面の $\mathbf F = (x, y, z)$ の流束 $4\pi$（発散定理の両辺）
- **壊してはいけないもの**: 領域の積分の今の数値の道
- **README**: 重積分の段落

### OD0: 1 階の微分方程式

- **担当**: A（理由: 型の見分けと、割り算で失う解（$y = 0$）・積分定数の置き方を間違えやすい）
- **目的**: 1 階の常微分方程式の一般解と、初期値の特殊解
- **触るファイル**: `core/src/exact/ode.rs`（新規）、`exact/convert.rs`（常微分方程式の行の入口。`equation.rs` が解いた形 $y' = f(x, y)$ を受け取る）、`core/tests/exact_cases/ode.tsv`
- **やること**（上から順に型を見分ける）
  1. 変数分離形 $y' = g(x)h(y)$（$h(y) = 0$ の解も添える）
  2. 1 階線形 $y' + p(x)y = q(x)$（積分因子）
  3. ベルヌーイ形 $y' + p y = q y^n$
  4. 同次形 $y' = F\left(\frac{y}{x}\right)$
  5. 完全微分形 $M + Ny' = 0$（$M_y = N_x$。積分因子が $x$ だけ・$y$ だけの式なら探す）
  6. 初期値の欄があれば定数を決める（解けなければ一般解だけ）
  7. 照らし合わせ: 答えを方程式に入れて 0、初期値で数値解（今の道）と 5 点で一致
- **実装の土台**

  ```rust
  pub fn solve_first_order(rhs: &Term, x: &str, y: &str, init: Option<(&Term, &Term)>, budget: &Budget) -> Result<Option<OdeAnswer>, Stop>;
  ```

- **やらないこと**: リッカチ形の一般・クレロー形の特異解
- **完了の条件**: 微分方程式の表の 1 階の行で 85%。$y' = 2xy$ → $y = C_1e^{x^2}$、$y' + y = x,\ y(0) = 1$ → $y = x - 1 + 2e^{-x}$、$y' = y^2$ → $y = -\frac{1}{x + C_1}$、
  $y' + y = y^2$ → $y = \frac{1}{1 + C_1e^x}$、$(2xy + 1) + (x^2 + 2y)y' = 0$ → $x^2y + x + y^2 = C_1$（陰の形）
- **壊してはいけないもの**: 常微分方程式の今の数値の道
- **README**: 常微分方程式の段落に厳密解

### OD1: 2 階以上の線形・連立・ラプラス変換

- **担当**: A（理由: 共鳴（右辺が同次解と重なる）の場合分けと、ラプラス変換の逆の部分分数を間違えやすい）
- **目的**: 工学でいちばん使う、定数係数の線形の微分方程式とラプラス変換
- **触るファイル**: `core/src/exact/ode.rs`、`core/src/exact/laplace.rs`（新規）、`core/src/parser.rs`・`web/src/lib/latex.ts`（`\mathcal{L}`。D10）、`core/tests/exact_cases/ode.tsv`
- **やること**
  1. 定数係数の線形: 特性方程式（EX7）→ 同次解。右辺が多項式・指数・三角関数の積なら未定係数法（共鳴なら $x^m$ を掛ける）、それ以外は定数変化法（IG0 の積分）
  2. オイラー型 $x^2y'' + axy' + by = 0$（$x = e^t$）
  3. 連立の定数係数 $\mathbf x' = A\mathbf x$（LA1 の固有値。ジョルダン標準形なら $te^{\lambda t}$）
  4. ラプラス変換: 表（多項式・指数・三角・双曲・ヘヴィサイド・ずらし・微分・積分）と、逆は部分分数（EX6）。微分方程式を $s$ の代数の式にして解く道も
- **実装の土台**

  ```rust
  pub fn solve_linear_const(coeffs: &[Term], rhs: &Term, x: &str, init: &[Term], budget: &Budget) -> Result<Option<OdeAnswer>, Stop>;
  pub fn laplace(f: &Term, t: &str, s: &str, budget: &Budget) -> Result<Option<Term>, Stop>;
  pub fn inverse_laplace(F: &Term, s: &str, t: &str, budget: &Budget) -> Result<Option<Term>, Stop>;
  ```

- **やらないこと**: 変数係数の 2 階（オイラー型以外）、偏微分方程式（D11）
- **完了の条件**: 微分方程式の表の 2 階以上の行で 85%。$y'' + 2y' + y = 0$ → $y = (C_1 + C_2x)e^{-x}$、$y'' + 4y = \sin x$ → $y = C_1\cos2x + C_2\sin2x + \frac{\sin x}{3}$、
  $y'' + y = \sin x$ → $y = C_1\cos x + C_2\sin x - \frac{x\cos x}{2}$（共鳴）、$x^2y'' - 2y = 0$ → $y = C_1x^2 + \frac{C_2}{x}$、
  $\mathcal L[t^2e^{-3t}] = \frac{2}{(s + 3)^3}$、$\mathcal L^{-1}\left[\frac{s + 1}{s^2 + 4s + 13}\right] = e^{-2t}\left(\cos3t - \frac13\sin3t\right)$
- **壊してはいけないもの**: OD0 のテスト
- **README**: ラプラス変換の段落

### OD2: 厳密解への描き替え

- **担当**: B（理由: 描画の道に式を差し込むだけで、計算は OD0・OD1 にある）
- **目的**: 最初の希望「計算中は今までどおり、終わったら正確な方へ自然に移り変わる」を、微分方程式で実現する（D8）
- **触るファイル**: `web/src/lib/types.ts`（`PlotRequest.exactSource`）、`plot.worker.ts`、`App.svelte`、`web/src/lib/Viewer.ts`（重ねて切り替える）、`README.md`
- **やること**
  1. 常微分方程式の行で特殊解（初期値つき）が見つかったら、`exactSource`（エンジンの文字列。陽関数 $y = f(x)$、連立なら媒介変数の曲線）を付けて描き直す
  2. 古い絵（数値解）を 300 ms で薄くしながら消し、新しい絵を濃くする
  3. 説明に「厳密解 $y = \ldots$ で描いています」。初期値の欄にリスト（解曲線が何本も）のときは、本数ぶん式を作る
  4. 厳密解が表示範囲で定義されない所（$y = -\frac{1}{x + C}$ の極）は、数値解が止まる所と同じく切る
- **実装の土台**

  ```ts
  // PlotRequest
  exactSource?: string // あれば常微分方程式の代わりにこれを描く（説明に厳密解を出す）
  ```

- **やらないこと**: 一般解（初期値が無い解）を描くこと
- **完了の条件**: $y' + y = x,\ y(0) = 1$ で、最初に数値解が出て、その後に厳密解へ 300 ms で切り替わる。2 本の曲線の差が表示範囲で $10^{-6}$ 以下（数値解の精度の範囲）。
  厳密解が見つからない式（$y' = \sin(xy)$）は数値解のまま
- **壊してはいけないもの**: 常微分方程式の描き方（傾きの場・連立・リストの初期値）
- **README**: 常微分方程式の段落

### CF0: 複素関数

- **担当**: B（理由: 複素数の厳密な計算は EX2 で済み、留数は LM0 の展開の係数を読むだけ）
- **目的**: 複素数の値・極形式・留数・留数による実積分を厳密に出す
- **触るファイル**: `core/src/exact/complex.rs`・`exact/residue.rs`（新規）、`exact/convert.rs`（`Re`・`Im`・`arg`・`abs`・`conj`・新しい `\operatorname{Res}`（文書 6 の Z3 の書き方に合わせる））、`core/tests/exact_cases/complex.tsv`
- **やること**
  1. 複素数の四則・べき・主値の $\log$・$\arg$（$\arg(1 + i) = \frac{\pi}{4}$）、極形式（$re^{i\theta}$）、1 の累乗根
  2. 留数: 極の位数を分母の因数分解から決め、LM0 のローラン展開の $-1$ 次の係数
  3. 留数による実積分: $\int_{-\infty}^\infty R(x)\,dx$、$\int_{-\infty}^\infty R(x)e^{iax}dx$（ジョルダンの補題）、$\int_0^{2\pi}R(\cos\theta, \sin\theta)\,d\theta$。IG2 の 1 つの方法として登録
  4. 文書 6 の Z3（留数の数値の表示）に厳密値を出す
- **実装の土台**

  ```rust
  pub fn residue(f: &Term, z: &str, at: &Term, budget: &Budget) -> Result<Option<Term>, Stop>;
  ```

- **やらないこと**: 多価関数の枝の切れ目をまたぐ積分（文書 1 の R5・文書 6 の Z5）
- **完了の条件**: 複素関数の表で 80%。$(1 + i)^{10} = 32i$、$e^{i\pi/3} = \frac12 + \frac{\sqrt3}{2}i$、$\log(-1) = \pi i$、$z^3 = 1$ の根、$\operatorname{Res}_{z=i}\frac{1}{z^2 + 1} = -\frac{i}{2}$、
  $\operatorname{Res}_{z=0}\frac{e^z}{z^3} = \frac12$、$\int_{-\infty}^\infty\frac{\cos x}{x^2 + 1}dx = \frac{\pi}{e}$、$\int_{-\infty}^\infty\frac{dx}{x^2 + 2x + 5} = \frac{\pi}{2}$
- **壊してはいけないもの**: 複素数の色分けの描き方
- **README**: 複素数の段落

## 6. 危ない所

- **規則の順番**: 不定積分・微分方程式は、規則を当てる順番で答えの形が変わる。例題の表の「値は正しいが形が違う」を見て順番を直す（答えの正しさは照らし合わせで守る）
- **余分な解・失う解**: 方程式の 2 乗、微分方程式の割り算（$y = 0$）。元の式に入れて確かめることを省かない
- **原始関数の不連続**: 定積分を「端の値の差」にする前に、区間の中の不連続を必ず探す（IG2 の 2）
- **「存在しない」の言い過ぎ**: 極限・微分係数で「存在しない」と言うのは、左右の値・2 つの経路が厳密に違うと示せたときだけ。数値で違って見えるだけなら文書 2 の言葉
- **収束の判定の条件落ち**: 比較判定法は項の符号が最後には一定のときだけ、交代級数は単調に減るときだけ
- **例題の表への偏り**: 表の問題に合わせた規則を足すと、少し変えた問題で外れる。表の問題は数を変えた版も入れる（教科書の型ごとに 2〜3 問）
- **今の道との意味の違い**: 記法を厳密値の道に登録するとき、今の道と同じ意味か（端・範囲・添字）を近似値で確かめる（2.3）

## 7. ほかの計画との関係

| 計画 | 関係 |
|---|---|
| 文書 7（`plan-exact-core.md`） | この文書の土台。EX5 の後から SC0・DM0・LA0 を始められる |
| 文書 2（`plan-analysis.md`） | SR0 が X3 の「判定の目安」を「判定（証明つき）」にする。LM3 が X0 に厳密な「存在しない」を足す。DF0 は W0（方向微分）の厳密値も出す |
| 文書 4（`plan-visual-extras.md`） | フーリエ級数の周転円（E）は、IG2 のフーリエ係数（D12）を使える |
| 文書 6（`plan-locus-complex-integral.md`） | CF0 が Z3 の留数に厳密値を出す。N4（点の説明）に DF0 の接線と SC1 の点の座標を出す |
| `plan-math-notation.md` | M3（微分）・M4（和）・M6〜M7（行列）・M9（常微分方程式）・M10（数列）・M11（論理）の記法に、厳密値の道を登録する |

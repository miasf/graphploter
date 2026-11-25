# 計画: 解析（近似値の表示・方向微分・多変数の極限）

文書 2（`docs/plan-roadmap.md`）。式の横に値を出す（Desmos のように）、方向微分 $D_{\mathbf u} f$、多変数の極限と、
広義積分・級数の収束・発散の判定を、1 ステップずつ（1 ステップ = 1 PR）の指示書にする。
ステップの名前は、近似値の表示が **V0〜V1**、方向微分が **W0〜W2**、極限が **X0〜X4**。
各ステップの「担当」は `docs/agent-tiers.md` の段階。各ステップの「実装の土台」は、関数の形・データの形・処理の流れの下書き。
**土台は出発点で、命令ではない**。コードを読んで合わない所があれば直してよく、そのときは 0 章の表に「計画との違い」を書く。

**この文書の芯**: 数値で言えることと言えないことを分ける。

- 値は**近似値**として出す（f64）。厳密値はやらない（利用者が将来、自分でアルゴリズムを作る。7 章）。ただし、後で厳密値を足せる形にしておく（2.2）
- 極限は、数値では「**存在しない**」（違う値に近づく 2 つの経路）は示せるが、「**存在する**」とは言えない。言葉は 2.3 の表に決める

## 0. 進み具合（実装の記録）

| ステップ | 状態 | メモ（計画との違い） |
|---|---|---|
| （まだ無い） | | |

## 1. 調べた結果（2026-11-25）

### 1.1 今ある部品

| 部品 | 場所 | この計画での使い道 |
|---|---|---|
| 定数の式の値の表示 | `web/src/App.svelte` の「陽関数の高さ」の所（1359 行あたり）。頂点の縦の座標（**f32**）の最小 = 最大なら `値 ${formatValue(…)}`（有効 6 桁。1393 行の `formatValue`） | V1 で置き換える |
| 複素平面の点の値の表示 | 同 1396 行の `complexPointText`（頂点の f32 から有効 4 桁） | V1 で置き換える |
| 定数の値をエンジンで計算する入口 | `core/src/lib.rs` の `eval_constant`・`eval_range`（実数だけ。定義は `eval_range` だけが使う）、`equation.rs` の `eval_constant_with`（3321 行） | V0 で、複素数・組・リストも返す形にする |
| `=` の無い式の振り分け | `equation.rs` 1565 行あたり（`chain.exprs.len()` が 1 の所）: 組 → `parametric`・`field`、`complex_plane`（座標も媒介変数も使わず、`i` か複素数のパラメータを使えば点）、それ以外は「縦の座標 = 式」の陽関数 | V0 で「座標も媒介変数も使わない式」をここで拾う |
| 1 変数の極限 | `core/src/limit.rs`（`analyze`・`sequence`・`finite_estimate`・`growing`・`oscillates`・`removable_quotient`）、`expand.rs` の `limit_expression`（2038 行あたり）・`limit_samples`（標本の上限は 12,000。1159 行） | W1・X0〜X4 の 1 本ずつの経路の極限 |
| 極限の結果パネル | `MeshResult::limit_values`（`lib.rs` 270 行）→ `plot.worker.ts` の `limitResult` → `types.ts` の `LimitResult` → `ExpressionItem.svelte` 603 行の `limit-result`・`LimitLesson.svelte`（ε–δ・ε–N。canvas に描く） | X0〜X3 の結果パネルの手本。X3 はそのまま使う |
| ∞ までの和 | `expand.rs` の `sum_to_infinity`（4424 行。そのまま足す → 交代級数の加速 → オイラー・マクローリン） | X3 で、値が出ないときの判定に回す |
| 広義積分 | `integrate.rs`・`quad.rs`。テストは `core/tests/improper.rs`（発散の理由の文に「発散」「収束しません」が入る） | X3。`eval_constant` の挙動は変えない |
| 記号微分・勾配 | `diff.rs` の `diff`・`add`・`mul` など、`expand.rs` の `vector_operator`（2717 行。$\nabla$ は座標 x, y, z だけ） | W0 |
| 点での値 | `@at(式, x, a, y, b)`（`expand.rs` 1416 行。LaTeX は `\left.式\right|_{x=a}`） | W0 の $D_{\mathbf u} f(a, b)$ |
| ベクトルの名前 | `\mathbf{u}`・`\vec{u}` → `{vec_u}`（`latex.ts` 865 行あたり） | W0 の向き |
| ⚙ の項目を足す道（手本は曲率の色分け N2） | `types.ts` のビットの定数 → `plot.worker.ts` 137 行の `req.flags & (…)`（キャッシュの鍵）→ `lib.rs` の `plot` の中、`build` の直後 → `MeshResult` のフィールドと getter → `ExpressionItem.svelte` | W2 |
| 使っているビット | `flags` のビット 0〜7 が使用中、8（256）・9（512）は文書 1 が予約 | この文書は **10（1024）** を W2 に使う |

### 1.2 計算エンジンで確かめた今の動き

`core/tests/` に一時的なテストを置き、`plot`（3 次元、表示範囲 $[-3, 3]^3$）と `eval_constant` で確かめた（終わったので消した）。

**近似値の表示**

| 式（エンジンの文字列） | `eval_constant`（f64） | `plot` の頂点の縦（f32） | 画面に今出るもの |
|---|---|---|---|
| `1+2` | 3 | 3 | 値 3 |
| `1/3` | 0.333333333333333315 | 0.33333334 | 値 0.333333（6 桁） |
| `π 10^(8)` | 314159265.358979 | **9**（表示範囲の 3 倍で止めている。`mesh.rs` 416 行の `clamp`） | **値 9**・表示範囲の外（**間違い**） |
| `10^(50)` | $10^{50}$ | **9** | **値 9**（間違い） |
| `10^(-50)` | $10^{-50}$ | **0**（f32 で 0 になる） | **値 0**（間違い） |
| `sin(π)` | $1.2246 \times 10^{-16}$ | $1.2246 \times 10^{-16}$ | 値 1.22465e−16 |
| `1+2i`・`e^(i π/3)` | 誤り「実数の値になりません」 | 複素平面の点 | 点の値（4 桁） |
| `ln(-1)`（値は $\pi i$） | 誤り | 縦が NaN（点にならない） | 何も出ない |
| `[1,2,3]`・`[1,2,3]^2` | 誤り | 平面 3 枚（`z = f(x, y) × 3`） | 何も出ない（最小 ≠ 最大） |
| `(1,2,3)`・`(1,2) + (3,4)` | 誤り | 媒介変数表示の曲線（長さの無い点） | 「点」 |
| `f(2)`（定義 `f(x) = x^2`） | 誤り（`eval_constant` は定義を使わない） | 4 | 値 4 |
| `g(1,2)`（`g(x,y) = x y`） | 誤り | 2 | 値 2 |
| `@int(t,0,1) t^2`・`@sum(k,1,∞) 1/k^2`・`@lim(x,0) sin(x)/x` | 正しい値 | 正しい値（f32） | 値（6 桁） |
| `1/0`・`0/0` | 誤り | NaN | 何も出ない |
| 2D 表示の定数 | — | 線（`mesh.curve`） | **何も出ない**（`App.svelte` が `!res.mesh.curve` の時だけ読む） |

**方向微分**: 記法は無い。`D_{\mathbf{u}}f` は `latex.ts` で `D_({vec_u})f`（数列 D の項 × f）になる。`\nabla_{\mathbf u}` は差分の記号として誤り。
内積の形 `(@grad(x^2 y))(cos(1), sin(1), 0)` を `@at` で点に入れると $3.0026802\ldots$ で正しい（$\nabla$ は x, y, z の 3 成分）。
`\frac{\partial f}{\partial x}\left(1,2\right)` は `@diff(x) f (1,2)` になり、点での値にならない（関数 × 組）。**W0 で同じ落とし穴を避ける**

**多変数の極限**: `\lim_{(x,y)\to(0,0)}` は `latex.ts` で誤り（「lim の下に x → a の形で」）。`\lim_{x\to0,y\to0}` も誤り。
繰り返しの極限（`@lim(x,0) (@lim(y,0) …)`）と、経路を代入した 1 変数の極限は今でも正しく求まる:

| 式 | 結果 |
|---|---|
| $\lim_{x\to0}\lim_{y\to0} \dfrac{x^2-y^2}{x^2+y^2}$ / 順番を逆 | $-1$ / $1$ |
| $\dfrac{xy}{x^2+y^2}$ を $y = mx$ で（$m = 0, 1, 2$） | $0,\ 0.5,\ 0.4$ |
| $\dfrac{x^2 y}{x^4 + y^2}$ を $y = c x^2$ で（$c = \tfrac12, 1, 2$）/ $y = 0.5x$ で | $0.4,\ 0.5,\ 0.4$ / $0$ |
| $\dfrac{h^2 \cos^2 1 \cdot h \sin 1}{h^2} / h$（$\tfrac{x^2 y}{x^2+y^2}$ の原点での方向微分の定義） | $\cos^2 1 \sin 1 = 0.2456477482\ldots$ |
| $(\lvert h\cos 1\rvert + \lvert h \sin 1\rvert)/h$、$h \to 0$ / $h \to 0^+$ | 「左極限と右極限が一致しません」/ $1.3817732907$ |

1 つの極限の計算は 0.1 ms ほど（初等関数）。積分を含む繰り返しの極限 `@lim(x,0) (@lim(y,0) (@int(t,0,x y) cos t)/(x y))` は
「極限の標本の計算量が上限に達しました」（47 ms）。

**広義積分・級数**

| 式 | 今の結果 | 正しくは |
|---|---|---|
| $\sum_{k=1}^\infty 1/k^2$、$\sum k/2^k$、$\sum k!/k^k$、$\sum (-1)^{k+1}/k$ | 値（0.2〜4 ms） | 同じ |
| $\sum 1/k$、$\sum (-1)^k$ | 誤り「Σ が収束しないようです」 | 発散・振動 |
| $\sum 1/k^{1.01}$（$\approx 100.58$）、$\sum 1/\left(k \ln(k+1)^2\right)$（収束） | 同じ誤り（88 ms） | 収束するが、数値では値を出せない |
| $\sum_{k \ge 2} 1/(k \ln k)$ | 同じ誤り | 発散（とても遅い） |
| $\int_1^\infty 1/x$、$\int_0^1 1/x$ | 誤り（発散の理由つき） | 発散 |
| $\int_1^\infty x^{-1.01}$（$= 100$） | 誤り「収束しないようです」（41 ms） | 収束 |
| $\int_0^\infty \sin x$ / $\int_0^\infty \frac{\sin x}{x}$ | 誤り「振動が小さくならない」/ $\pi/2$ | 同じ |
| $\lim_{x\to\infty} \int_1^x \frac{dt}{t}$ | 右の極限 $+\infty$（極限のパネル） | 同じ |
| $\lim_{n\to\infty}\sum_{k=1}^{n} \frac{1}{2^k}$ | 1 | 同じ |
| $\lim_{n\to\infty}\sum_{k=1}^{n} \frac{1}{k^2}$ | **「極限の収束を数値計算で確認できません」** | $\pi^2/6$ |
| $\lim_{n\to\infty}\sum_{k=1}^{n} \frac1k$ | 「確認できません」 | $+\infty$ |
| $\lim_{n\to\infty}\sum_{k=1}^{n} (-1)^k$ | 振動 | 同じ |

部分和の極限が弱いのは、数列の標本の添字が 4,096 まで（$\Sigma$ の項が 2,000 を超えると 512 まで）で、`@sum(k,1,n)` を添字ごとに展開し直しているため（`limit.rs` の `sequence`）。
対数の速さの発散の検出（`growing` の `logarithmic`）は、倍にするごとの増分が $10^{-6}$ の相対差で一定であることを求めるので、
$S_{2N} - S_N = \ln 2 + O(1/N)$ の調和級数は $N = 4096$ では通らない。

### 1.3 無いもの

- f64 の値を画面に返す道（今は f32 の頂点から読んでいて、表示範囲の外・とても小さい値・2D 表示・複素数・リスト・組で間違うか、出ない）
- 方向微分の記法と、定義の極限で確かめる仕組み
- 多変数の極限の記法・経路・極座標の図
- 部分和を累積で計算する道、収束の判定の目安

## 2. 全体の決めごと

### 2.1 値はエンジンが f64 で出し、画面は書式だけを受け持つ

頂点の座標（f32、しかも表示範囲で止めている）から値を読むのをやめる。エンジンが**展開した後の式**を f64 で計算して数で返し、
画面（`web/src/lib/valueText.ts`、新規）が文字列にする。表示の向き・回転・2D/3D・座標系に関係なく、同じ式なら同じ値を出す。

### 2.2 厳密値を後で足せる形

値の 1 つ 1 つを「近似値」と「厳密値（この計画では常に無し）」の組で持つ。画面は「厳密値があれば先に出し、近似値を ≈ で添える」と書いておく
（今は厳密値が無いので近似値だけ）。将来の厳密値の計画は、エンジンで `exact` を埋めるだけで画面に出る。

```text
= √2          ← 将来（厳密値あり）: 厳密値 ＋ 小さく「≈ 1.414213562」
≈ 1.414213562 ← この計画
```

### 2.3 数値で言えることの言葉

| 状況 | 出す言葉 | 出さない言葉 |
|---|---|---|
| 2 つの経路で違う有限の値に近づく、ある経路で発散・振動する | **極限は存在しません**（経路と値を添える） | — |
| 調べた経路が全部同じ値 $L$、極座標の円の上の $\lvert f - L\rvert$ の最大も縮む | **どの経路でも $L$ に近づいて見えます**（数値では、存在するとは言い切れません） | 「存在します」「$= L$」 |
| 経路の極限が求まらない（計算できない・振動の判定ができない） | **判定できません** | — |
| 級数・広義積分の値が数値で出た | 収束（値 ≈ …）※今と同じ | — |
| 値が出ず、項の大きさが $k^{-p}$ の形に見える | **収束の目安**／**発散の目安**（$p \approx 1.01$） | 「収束します」 |
| 方向微分で、記号の値（$\nabla f \cdot \hat{\mathbf u}$）と定義の極限が食い違う | **この点で $f$ は微分可能ではありません**（理由を添える） | — |
| 記号の値と定義の極限が一致 | 値だけ | 「微分可能です」（全方向で 1 次でも、微分可能とは限らない。6 章） |

「存在しない」「微分可能ではない」は、数値の誤差を見込んでも言える場合だけ（違いが誤差の見積もりの 100 倍以上）。
判定に使った数（経路の式と値、$p$ など）は必ず添える。

### 2.4 方向微分の 2 つの求め方

$$D_{\mathbf u} f(\mathbf a) = \lim_{h \to 0} \frac{f(\mathbf a + h\hat{\mathbf u}) - f(\mathbf a)}{h}, \qquad \hat{\mathbf u} = \frac{\mathbf u}{\lVert\mathbf u\rVert}$$

が定義。$f$ が $\mathbf a$ で微分可能なら $D_{\mathbf u} f(\mathbf a) = \nabla f(\mathbf a) \cdot \hat{\mathbf u}$。

- **式（点を決めない）**: $\nabla f \cdot \hat{\mathbf u}$ を記号微分で作る（W0）。曲面・曲線として描ける
- **点での値**: 記号の値に加えて、定義の極限（1 変数の極限 `limit::analyze`）でも求め、食い違えば知らせる（W1）。
  さらに 8 方向で極限を求め、$\theta \mapsto D_{(\cos\theta, \sin\theta)} f(\mathbf a)$ が $A\cos\theta + B\sin\theta$ の形でなければ「$\mathbf u$ について 1 次でない」と知らせる

### 2.5 実装の土台（全体の作り）

**新しいファイル**

| ファイル | 中身 |
|---|---|
| `core/src/value.rs` | 定数の式の値（V0）。`Values`・`Shown` |
| `core/src/directional.rs` | 方向微分の点での確かめ（W1）、学習表示のための標本（W2） |
| `core/src/multilimit.rs` | 多変数の極限: 経路（X0）、極座標の円の上の最大と経路の追跡（X1） |
| `core/src/convergence.rs` | 部分和の累積（X2）、収束の判定の目安（X3） |
| `web/src/lib/valueText.ts`・`valueText.test.ts` | 値の書式（V1） |
| `web/src/lib/DirectionalLesson.svelte` | 方向微分の学習表示（W2） |
| `web/src/lib/MultiLimitPanel.svelte` | 多変数の極限の結果パネルと極座標の図（X0・X1） |
| `core/tests/values.rs`・`directional.rs`・`multilimit.rs`・`convergence.rs` | 物差し |

**データの形（下書き）**

```rust
// value.rs
pub enum Approx { Real(f64), Complex(f64, f64) }
/// 画面に出す数 1 つ。exact は将来の厳密値（LaTeX）。この計画では常に None
pub struct Shown { pub approx: Approx, pub exact: Option<String> }
pub enum Shape { Scalar, Tuple, List, ListOfTuples }
pub struct Values {
    pub shape: Shape,
    pub items: Vec<Vec<Shown>>,  // Scalar: [[v]]、Tuple: [[v1, v2, v3]]、List: [[v1], [v2], …]
    pub total: usize,            // リストの全部の個数（items は先頭 64 個まで）
    pub numeric: bool,           // 積分・極限・∞ の和・差分の微分・常微分方程式を通ったか（V1 で「≈」にする）
}

// multilimit.rs
pub enum PathKind { Line { theta: f64 }, Power { c: f64, k: f64, swap: bool }, Traced }
pub struct PathResult { pub kind: PathKind, pub behavior: crate::limit::Behavior, pub samples: Vec<[f64; 3]> }
pub enum Verdict { NotExist { a: usize, b: usize }, Diverges { path: usize }, LooksLike(f64), Unknown }
pub struct MultiReport {
    pub vars: Vec<String>, pub point: Vec<f64>,
    pub paths: Vec<PathResult>, pub verdict: Verdict,
    pub radial: Option<Radial>,   // X1（2 変数のときだけ）
}
pub struct Radial { pub r: Vec<f64>, pub sup: Vec<f64>, pub power: Option<f64>, pub grid: Vec<f32> /* θ × log r の g の値 */ }
```

**画面までの道**

1. 値（V0）: `MeshResult` に `values: Vec<f64>`（下の並び）と getter。ビットは使わない（定数の式でだけ計算し、計算は 1 回の `eval_batch`）
   `[形 (0 なし, 1 数, 2 組, 3 リスト, 4 組のリスト), 個数, 1 個の長さ, numeric (0/1), 全部の個数, (re, im) × 個数 × 長さ]`
2. `plot.worker.ts`: `values` を読んで `PlotResponse` の `value?: ShownValue` に入れる（`limitResult` と同じ所）
3. `App.svelte`: `item.value = res.value`。「陽関数の高さ」の値の行を消す（V1）
4. `ExpressionItem.svelte`: 式の行の右下に値（Desmos と同じ位置）
5. 方向微分の学習表示（W2）だけ新しいビット `PLOT_DIRECTIONAL = 1024`（パネルを開いている間だけ標本を作る）
6. 多変数の極限（X0・X1）・収束の判定（X3）は、その行の目的そのものなのでビットを使わない。`MeshResult` に `multi_limit: Vec<f64>`・`convergence: Vec<f64>` と getter

## 3. 決めてもらうこと（おすすめ付き）

| # | 内容 | おすすめ | ほかの案 |
|---|---|---|---|
| D1 | 値の桁数 | **有効 10 桁**（末尾の 0 は消す）。値を押すと 17 桁（f64 を正確に戻せる桁）に切り替わり、もう一度押すと戻る | 12 桁（Desmos に近い）・6 桁（今の表示） |
| D2 | 「=」と「≈」 | 数値の手続き（積分・極限・∞ の和・差分の微分・常微分方程式）を通った値と、10 桁に丸めた値は「**≈**」、それ以外（10 桁で f64 がそのまま戻る）は「**=**」 | いつも「≈」・いつも「=」 |
| D3 | 値を出す場所 | 式の行の**右下**（Desmos と同じ）。今の説明の行（種類・三角形の数・時間）とは分ける | 説明の行の中（今と同じ） |
| D4 | 定数の式の図形 | **今のまま描く**（3D は平面 $z = c$、2D は直線。README にある決まり） | 値だけを出して描かない（Desmos と同じ） |
| D5 | 方向微分の書き方 | `D_{\mathbf u} f`・`D_{\vec u} f`・`D_{(1, 2)} f`（**太字・矢印・組だけ**）。`D_u` は今までどおり数列 $D$ の $u$ 番目の項 | `D_u` も方向微分にする（数列の記法と衝突） |
| D6 | 向き $\mathbf u$ の長さ | **単位ベクトルに直して計算**し、説明に「$\hat{\mathbf u} = \mathbf u / \lVert\mathbf u\rVert$ で計算」と出す | そのまま（$\nabla f \cdot \mathbf u$。長さで値が変わる） |
| D7 | 定義の極限 | **両側**（$h \to 0$）。両側が無く片側（$h \to 0^+$）だけあるときは、その値も添える | 片側（$h \to 0^+$）を定義にする |
| D8 | 多変数の極限の書き方 | `\lim_{(x,y)\to(a,b)}`（**組だけ**。3 変数まで）。`\lim_{x\to a,\ y\to b}` は「組で書いてください」の誤り | 両方の書き方を同じ意味にする |
| D9 | 多変数の極限の図 | 3D: 元の関数の曲面（点の近く、薄く）と、経路を曲面の上の線で描く（値で色分け）。パネル: 極座標 $g(r, \theta)$ の色の図と $\max_\theta \lvert g - L\rvert$ の線 | 図はパネルだけ |
| D10 | 広義積分・級数の判定を出す場所 | 1 行に $\sum^{\infty}$・広義積分だけを書いた行で、**値が出ないときに**極限と同じ結果パネル（部分和の点・部分積分の線・判定・目安）を出す。値が出るときは今と同じ（値だけ） | 値が出るときもパネルを出す |
| D11 | 判定の目安（比・べき $p$） | 「目安」と書いて出す。確かな理由（項が 0 に近づかない）とは分けて書く | 出さない |

## 4. ステップの一覧

| # | 内容 | 前提 | 大きさ | 担当 |
|---|---|---|---|---|
| V0 | エンジンが定数の式の値を f64 で返す（複素数・組・リスト・定義した関数・極限の行） | — | 中 | B |
| V1 | 値の表示（書式・場所・「=」と「≈」・17 桁）。f32 から読む道を消す | V0 | 小〜中 | C |
| W0 | 方向微分の記法 $D_{\mathbf u} f$ と、記号微分での展開 | — | 中 | B |
| W1 | 点での方向微分を定義の極限でも求め、微分可能でない点を知らせる | W0, V0 | 中 | A |
| W2 | 学習表示（$\mathbf u$ を回すと切り口と接線が動く・$\theta$ の関数のグラフ） | W1 | 中 | B |
| X2 | 部分和の列を累積で計算し、対数の速さの発散も見分ける | — | 中 | B |
| X3 | 広義積分・級数を極限の仲間として出す（判定と目安） | X2 | 中〜大 | A |
| X0 | 多変数の極限の記法と、経路の極限 | — | 大 | A |
| X1 | 極座標の図 $g(r, \theta)$ と、円の上の最大・経路の追跡 | X0 | 大 | A |
| X4 | 極限の中の積分・級数（多変数の極限でも時間内に） | X0, X2 | 中 | B |

**おすすめの順番**: V0 → V1 → W0 → W1 → W2 → X2 → X3 → X0 → X1 → X4。
V は全部の土台（値の書式は W・X のパネルでも使う）。X2・X3 は今の極限のパネルに乗るので、多変数（X0・X1）より先にすると早く効果が出る。

## 5. ステップごとの指示書

共通の決まり: コメントと README は日本語。`core/` で `cargo test --release`、`web/` で `npm test` と `npm run check` が通ること。
テストのエラーの中身は WASM 以外では読めない（`JsError` を作るとパニックする）ので、計算エンジンの内側の関数（`Result<_, String>` を返すもの）を
テストから呼ぶか、`std::panic::catch_unwind` で成功・失敗を見る。`plot` を通すテストは `core/tests/notation.rs` の `run_flags` と同じ形。

### V0: エンジンが定数の式の値を f64 で返す

- **担当**: B（理由: 計算は 1 回の評価だが、どの図形の種類（陽関数・複素平面の点・リスト・組・極限）でも同じ値を拾う所を間違えやすい）
- **目的**: 1.2 の表の「間違い」「何も出ない」を、全部 f64 の値にする
- **触るファイル**: `core/src/value.rs`（新規）、`core/src/equation.rs`（振り分けの所）、`core/src/expand.rs`（数値の手続きの印）、
  `core/src/lib.rs`（`MeshResult::values`）、`core/tests/values.rs`（新規）
- **やること**
  1. `=` の無い式の振り分け（`equation.rs` 1565 行あたり、`chain.exprs.len()` が 1 の所）で、展開した後の式 `e` が**座標も媒介変数も使わない**なら、
     `e` を thread_local（`LAST_EXPLICIT` と同じ形の `LAST_VALUE`）に入れる。リスト `[…]`・組 `(…)`・複素数の点も、ここを通る前に入れる
     （組は `parametric` に、点は `complex_plane` に行く前）。**展開した後の式かどうかを実装時に確かめる**（積分・極限が数になっていること）
  2. 極限だけの行（`Plot::Limit`）は、`report.value()` を値にする（有限・$\pm\infty$。求まらなければ値なし）
  3. `lib.rs` の `plot` の中で `value::of_expr(&e, &consts)` を呼び、`MeshResult` の `values` に 2.5 の並びで入れる。座標を使う式（`x + 1`）は空
  4. 数値の手続きの印: `expand.rs` に thread_local の `NUMERIC_USED: Cell<bool>` を置き、積分・極限・∞ の和・差分で微分した関数（表の無い関数の導関数）・
     常微分方程式の所で立てる。`plot` の始めに倒す（`expand::used_params` を空にする所と同じ）
  5. 値が NaN・$\pm\infty$ もそのまま返す（書式は V1）
- **実装の土台**

  ```rust
  pub fn of_expr(e: &Expr, params: &Params) -> Result<Option<Values>, String> {
      // 1. 形を決める: Expr::List(items) → List（中身が組なら ListOfTuples）、Expr::Tuple(items) → Tuple、それ以外 → Scalar
      //    リストは先頭 64 個だけ計算し、total に全部の数（リストの展開を先にするなら、展開の上限 256 個のまま）
      // 2. 成分ごとに Program::compile_with(&item, &[], params)?、eval_batch(&[], &mut out) で 1 回だけ計算
      // 3. 値 z: im == 0 なら Approx::Real(z.re)、そうでなければ Complex(z.re, z.im)（as_real は使わない。ln(−1) を NaN にしないため）
      // 4. numeric = expand::numeric_used()
  }
  pub fn flatten(v: &Values) -> Vec<f64> { /* 2.5 の並び。Real は (re, 0) */ }
  ```

- **やらないこと**: 行列の値（M7 の描き方がある）、厳密値、パラメータの定義の行 `a = 2`（スライダーが値を出している）
- **完了の条件**（`core/tests/values.rs`。$\epsilon$ は f64 の 1 ulp）

  | 式 | 値 |
  |---|---|
  | `1+2` | 3（ちょうど）、numeric = false |
  | `sqrt(2)` | `std::f64::consts::SQRT_2` と 1 ulp 以内 |
  | `π 10^(8)` | 314159265.35897931（相対 $10^{-15}$）※表示範囲の外でも |
  | `10^(50)`・`10^(-50)` | $10^{50}$・$10^{-50}$（相対 $10^{-15}$） |
  | `1+2i`・`e^(i π/3)`・`ln(-1)` | $1 + 2i$、$0.5 + 0.8660254037844386i$、$\pi i$（$10^{-15}$） |
  | `[1,2,3]^2` | リスト $[1, 4, 9]$ |
  | `(1,2) + (3,4)`・`@at(@grad(x^2 y), x, 1, y, 2)` | 組 $(4, 6)$・$(4, 1, 0)$ |
  | `f(2)`（`f(x) = x^2`）・`g(1,2)`（`g(x,y) = x y`） | 4・2 |
  | `@int(t,0,1) t^2`・`@sum(k,1,∞) 1/k^2`・`@lim(x,0) sin(x)/x` | $1/3$・$\pi^2/6$（$10^{-11}$）・1、numeric = true |
  | `1/0` | 値は非有限（NaN か $\infty$） |
  | `x + 1`・`e^(i t)` | 値なし |
  | 2D 表示（`flat = true`）・回転したときの `1+2` | 3 |

  `bench.rs` の時間が増えない（定数でない式では何もしない）
- **壊してはいけないもの**: 描き方（定数の平面・点・リストの平面は今のまま。D4）、`eval_constant` の挙動
- **README**: V1 でまとめて書く

### V1: 値の表示

- **担当**: C（理由: 書式の表が決まっていて、テストの表を写せばよい）
- **触るファイル**: `web/src/lib/valueText.ts`・`valueText.test.ts`（新規）、`types.ts`（`ShownValue`）、`plot.worker.ts`、`App.svelte`、`ExpressionItem.svelte`、README
- **やること**
  1. 書式（D1・D2）。数 1 つ:
     - 0 は `0`。NaN は「定義されません」、$\pm\infty$ は `∞`・`−∞`
     - $10^{-4} \le \lvert v\rvert < 10^{10}$ は `toPrecision(10)` から末尾の 0 と小数点を消す。それ以外は仮数 10 桁と `× 10⁻¹⁶` のような上付きの指数
     - 負の符号は `−`（今の `formatValue` と同じ）
     - 複素数は `a + bi`・`a − bi`（虚部が 0 なら実数だけ、実部が 0 なら `bi`。$\pm1$ は `i`・`−i`）
     - 組は `(a, b, c)`、リストは `[a, b, c]`（10 個まで。それより多ければ先頭 10 個と `…（全 200 個）`）
     - 「=」か「≈」: `numeric` なら「≈」。そうでなければ、出した文字列を `Number()` で読み戻して元の f64 と全部一致すれば「=」、1 つでも違えば「≈」
     - 押すと 17 桁（`String(v)` の最短の表し方）と切り替え
  2. `ExpressionItem.svelte`: 式の行の右下に `<output class="value">`（D3）。押すと桁を切り替え、長押しでコピー（端末の決まりに合わせる）
  3. `App.svelte`: 「陽関数の高さ」の所の `値 ${formatValue(…)}` と `complexPointText` の値の部分を消す。
     「値が表示範囲の外なので、見えません」は、V0 の f64 の値（実数 1 つ）と縦の表示範囲から判定し直す（`10^{50}` で正しく出る）
  4. 極限のパネルの数（`limitNumber`）も同じ書式にする
  5. README: 「値の表示」の段落（書式の表の抜粋と、「近似値です。厳密値は出しません」）
- **実装の土台**

  ```ts
  // types.ts
  export interface ShownValue {
    shape: 'scalar' | 'tuple' | 'list' | 'listOfTuples'
    items: { re: number; im: number }[][]
    total: number
    numeric: boolean
    exact?: string[][]   // 将来の厳密値（LaTeX）。この計画では使わない
  }
  // valueText.ts
  export function formatNumber(v: number, digits = 10): string
  export function formatComplex(z: { re: number; im: number }, digits = 10): string
  export function valueText(v: ShownValue, full = false): { relation: '=' | '≈' | ''; text: string }
  ```

- **完了の条件**（`valueText.test.ts`）

  | 値（f64） | numeric | 表示 |
  |---|---|---|
  | 3 | いいえ | `= 3` |
  | $\sqrt2$ | いいえ | `≈ 1.414213562` |
  | 0.30000000000000004（`0.1+0.2`） | いいえ | `≈ 0.3` |
  | 314159265.35897931 | いいえ | `≈ 314159265.4` |
  | $10^{50}$（f64 の最近値） | いいえ | `= 1 × 10⁵⁰` |
  | $1.2246467991473532 \times 10^{-16}$ | いいえ | `≈ 1.224646799 × 10⁻¹⁶` |
  | $-2.5$ | いいえ | `= −2.5` |
  | $1 + 2i$ / $0.5 + 0.8660254037844386i$ / $\pi i$ | いいえ | `= 1 + 2i` / `≈ 0.5 + 0.8660254038i` / `≈ 3.141592654i` |
  | リスト $[1, 4, 9]$ / 組 $(4, 1, 0)$ | いいえ | `= [1, 4, 9]` / `= (4, 1, 0)` |
  | 1〜200 のリスト | いいえ | `= [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, …]（全 200 個）` |
  | $1/3$ | はい | `≈ 0.3333333333` |
  | 2（積分の値がちょうど 2） | はい | `≈ 2` |
  | NaN | — | `定義されません` |
  | $\sqrt2$ を押した後 | いいえ | `≈ 1.4142135623730951` |

  画面: `10^{50}` の行に「値 9」が出ない。2D 表示でも値が出る
- **壊してはいけないもの**: 説明の行（種類・三角形の数・時間）、複素平面の点の「表示範囲の外か、値が計算できません」の知らせ、極限のパネル

### W0: 方向微分の記法と、記号微分での展開

- **担当**: B（理由: 記法の読み分け（数列の項 $D_n$ と、関数の点での値 $f(1, 2)$）に注意が要る。計算は既存の記号微分）
- **平たい説明**（README にも書く）: $D_{\mathbf u} f$ は、点から向き $\mathbf u$ に進んだときの $f$ の増え方（坂の傾き）。
  $\mathbf u = (1, 0)$ なら $\partial f / \partial x$、$\mathbf u = (0, 1)$ なら $\partial f / \partial y$
- **触るファイル**: `web/src/lib/latex.ts`（`D_{…}` の読み分け）、`core/src/parser.rs`（演算子 `ddir`）、`core/src/expand.rs`（展開）、
  `core/tests/directional.rs`（新規）、`web/src/lib/latex.test.ts`、README
- **やること**
  1. `latex.ts`: `D_{\mathbf{u}}`・`D_{\vec{u}}`・`D_{\boldsymbol{u}}`・`D_{\left(a,b\right)}` のあと（D5）:
     - 直後が「1 文字の名前 + 括弧の引数」（`f\left(1,2\right)`）なら `@at(@ddir(U) f, x, 1, y, 2)`（引数 3 つなら z まで）。**関数の点での値**（$f'(2)$ と同じ読み方）
     - それ以外は ` @ddir(U) ` を前に置く（直後の項に掛かる。$\frac{d}{dx}$ と同じ）
     - 下付きが太字・矢印・組でない `D_{u}`・`D_{n}` は今のまま（数列の項）
  2. `parser.rs`: `ddir` を「括弧の中は式 1 つ、中身は直後の項」の演算子として足す（今の `Binder` は 1 つ目が変数の名前でないといけないので、
     新しい種類 `OpKind::Directed` を足すか、`Binder` の読み方を広げる。合う方でよい）
  3. `expand.rs` の展開（下の土台）。向きが定数で長さ 0 なら「向きのベクトルが 0 です」。次元と関数の引数の数が合わなければ
     「f は 2 変数ですが、向きは 3 次元です」
  4. 説明に「$\hat{\mathbf u} = \mathbf u / \lVert\mathbf u\rVert$ で計算」（D6。$\mathbf u$ が単位ベクトルなら出さない）
- **実装の土台**

  ```rust
  /// D_u f = Σ_i (∂f/∂x_i) u_i / |u|（座標は x, y, z の先頭 n 個。n は u の成分の数 = 2 か 3）
  fn directional(&mut self, u: &Expr, body: &Expr) -> Result<Expr, String> {
      const XYZ: [&str; 3] = ["x", "y", "z"];
      let comps = match self.val(u)? { Val::S(Expr::Tuple(c)) if (2..=3).contains(&c.len()) => c, _ => return Err("向きは 2 つか 3 つの成分のベクトルで書いてください".into()) };
      let n = comps.len();
      let f = match self.val(body)? {
          Val::S(s) => s,
          Val::Fn { arity, body } if arity == n => apply_body(&body, &XYZ[..n].iter().map(|c| Expr::Var((*c).into())).collect::<Vec<_>>()),
          Val::Fn { arity, .. } => return Err(format!("関数は {arity} 変数ですが、向きは {n} 次元です")),
      };
      let norm = sqrt(Σ comps[i]^2);           // 定数なら値を確かめ、0 なら Err
      Σ_i mul(diff(&f, XYZ[i])?, div(comps[i], norm))
  }
  ```

- **やらないこと**: 4 変数以上、座標系（極座標など）での方向微分、$\nabla_{\mathbf u}$（今は差分の記号）
- **完了の条件**（`core/tests/directional.rs` と `latex.test.ts`）
  - `D_{\left(1,1\right)}\left(x^2y\right)` の曲面が $z = (2xy + x^2)/\sqrt2$ と表示範囲の格子で $10^{-12}$ 以内
  - `D_{\mathbf{u}}f\left(1,2\right)`（`f(x,y) = x^2 y`、`\mathbf{u} = (3, 4)`）の値が $16/5 = 3.2$（$10^{-12}$。V0 の値）
  - `D_{\left(1,1,1\right)}f\left(1,2,3\right)`（`f(x,y,z) = xyz`）が $11/\sqrt3$
  - `D_{\left(0,0\right)}f` は「向きのベクトルが 0 です」、`D_{n}` を使う数列の式（`D_{n} = 1/n`）は今と同じ
  - `latex.test.ts`: 上の 3 つの変換結果の文字列
- **README**: 「方向微分」の段落（書き方・平たい説明・単位ベクトルに直すこと）

### W1: 点での方向微分を定義の極限でも求める

- **担当**: A（理由: 記号の値と極限の値の食い違いの判定を間違えると、「微分可能ではない」を黙って言い間違える）
- **目的**: $\nabla f \cdot \hat{\mathbf u}$ が使えない点（微分可能でない点）で、正しい値か、無いことを出す（2.3・2.4）
- **触るファイル**: `core/src/directional.rs`（新規）、`core/src/expand.rs`（`@at(@ddir…)` を見つけたとき）、`core/src/lib.rs`（`ddir_report`）、
  `web/src/lib/ExpressionItem.svelte`（説明）、`core/tests/directional.rs`
- **やること**
  1. 点での値（`@at(@ddir(u) f, …)` で点がどれも定数）のとき、記号の値 $S$ に加えて、$q(h) = \dfrac{f(\mathbf a + h\hat{\mathbf u}) - f(\mathbf a)}{h}$ の
     $h \to 0$ の極限を `limit::analyze`（両側）で求める
  2. 判定（2.3 の言葉）:
     - $f(\mathbf a)$ が定義されない → 値なし「$f(\mathbf a)$ が定義されていないので、方向微分はありません」
     - 両側の極限 $L$ があり、$\lvert S - L\rvert \le 10^{-6} \max(1, \lvert L\rvert)$ → 値 $S$
     - 両側の極限 $L$ があり、$S$ と違う（か $S$ が NaN）→ 値 $L$、説明「この点で $f$ は微分可能ではないので、$\nabla f \cdot \hat{\mathbf u}$ は使えません（定義の極限で計算）」
     - 両側が無い（左右の極限が違う）→ 値なし「両側の方向微分はありません」、右の極限があれば「$h \to 0^+$ なら …」（D7）
  3. 2 変数なら 8 方向 $\theta_k = k\pi/4$ でも $L(\theta_k)$ を求め、$A\cos\theta + B\sin\theta$ に最小二乗で合わせた残りが
     $10^{-6} \max_k \lvert L(\theta_k)\rvert$ を超えたら「方向微分は $\mathbf u$ について 1 次ではありません（だから微分可能ではありません）」
  4. 結果は `MeshResult::ddir_report()`: `[S, 両側の状態, L, 右の状態, 右の値, 1 次か (0 いいえ / 1 はい / 2 不明)]`。画面は値（V1）の下に説明
- **実装の土台**

  ```rust
  pub struct DirReport { pub symbolic: f64, pub two_sided: Behavior, pub right: Behavior, pub linear: Option<bool> }
  pub fn check(f: &Expr, vars: &[&str], a: &[f64], u_hat: &[f64], params: &Params) -> Result<DirReport, String> {
      // q(h) を Expr で作る: (f(a + h û) − f(a)) / h（f に a_i + h û_i を代入。h は利用者が打てない名前 "#h"）
      // Program::compile_with(&q, &["#h"], params) → limit::program_samples を標本にして limit::analyze(…, 0.0, 0, None, false)
      // removable_quotient（Taylor 係数）は q の分母 h で効くので、そのまま渡してよい
  }
  ```

- **やらないこと**: 微分可能であることの判定（言わない。6 章）、3 変数での 1 次かの判定（方向が球面になるので、まず 2 変数だけ）
- **完了の条件**（`core/tests/directional.rs`）

  | $f$、点、$\mathbf u$ | 期待 |
  |---|---|
  | $x^2 y$、$(1, 2)$、$(3, 4)$ | 値 3.2、記号と極限が $10^{-8}$ で一致、1 次 |
  | $\sin x \, e^y$、$(0, 0)$、$(1, 1)$ | 値 $1/\sqrt2$ |
  | $\begin{cases} \frac{x^2 y}{x^2 + y^2} & (x, y) \ne (0, 0) \\ 0 & \text{それ以外} \end{cases}$、原点、$(\cos 1, \sin 1)$ | 値 $\cos^2 1 \sin 1 = 0.24564774821694$（$10^{-8}$）、「微分可能ではない」、1 次でない |
  | $\lvert x\rvert + \lvert y\rvert$、原点、$(\cos 1, \sin 1)$ | 値なし（両側なし）、$h \to 0^+$ なら $\cos 1 + \sin 1 = 1.3817732907$ |
  | $\sqrt{x^2 + y^2}$、原点、$(1, 0)$ | 値なし、$h \to 0^+$ なら 1 |
  | $1/x$、$(0, 1)$ | 「$f(\mathbf a)$ が定義されていない」 |

  1 つの点で 20 ms 以内（8 方向を含む）
- **README**: 「方向微分」の段落に、微分可能でない点の例（上の表の 3 つ目）

### W2: 方向微分の学習表示

- **担当**: B（理由: ⚙ のビットと新しいパネル。計算は W1 の部品）
- **平たい説明**: 曲面 $z = f(x, y)$ を、点 $\mathbf a$ を通り向き $\mathbf u$ の縦の平面で切ると、切り口は 1 本の曲線になる。
  その曲線の $\mathbf a$ での傾きが $D_{\mathbf u} f(\mathbf a)$。$\mathbf u$ を回すと傾きが変わり、勾配 $\nabla f$ の向きでいちばん急（値 $\lVert\nabla f\rVert$）、
  それと直角な向きで 0（等高線の向き）になる
- **触るファイル**: `web/src/lib/DirectionalLesson.svelte`（新規）、`ExpressionItem.svelte`、`types.ts`（`PLOT_DIRECTIONAL = 1024`）、
  `plot.worker.ts`（137 行のキャッシュの鍵にビットを足す）、`Viewer.ts`（切り口の平面・曲線・接線）、`core/src/directional.rs`・`lib.rs`
- **やること**
  1. 2 変数の点での方向微分の行に、極限のパネルと同じ形の「向きを回して見る」を出す。開いている間だけビット 1024 を立てる
  2. エンジンは 1 回だけ、点の周りの標本を返す: $f(\mathbf a + r(\cos\theta_j, \sin\theta_j))$（$\theta_j = 5° \times j$、72 本。
     $r$ は $[-R, R]$ を 65 点。$R$ は表示範囲の横の幅の $1/4$）と、W1 の $L(\theta_j)$（微分可能なら $\nabla f \cdot (\cos\theta_j, \sin\theta_j)$ の式の値）
  3. パネル: $\theta$ のスライダー（▶ で回し続ける）、$\hat{\mathbf u} = (\cos\theta, \sin\theta)$ と値、canvas に $\theta \mapsto D_{\hat{\mathbf u}} f(\mathbf a)$ のグラフ
     （$[0, 2\pi]$、今の $\theta$ の印、最大・最小・0 の所の印と「勾配の向き」「等高線の向き」の言葉）
  4. 3D: 縦の平面（半透明の長方形）、切り口の曲線、点 $(\mathbf a, f(\mathbf a))$、傾き $D_{\hat{\mathbf u}} f$ の接線。
     $\theta$ を動かしても**エンジンに頼まない**（2 の標本を、隣の $\theta_j$ の間で線形に補間）
  5. 微分可能でない点（W1）では、グラフが $A\cos\theta + B\sin\theta$ の形にならないことが見える（説明も出す）
  6. 3 変数では値だけ（「図は 2 変数だけ」）
- **実装の土台**

  ```rust
  /// 学習表示の標本: [R, f(a), 72 × 65 の f の値 …, 72 個の L(θ_j) …]
  pub fn lesson_samples(f: &Program, a: [f64; 2], radius: f64, slopes: &[f64; 72]) -> Vec<f64>;
  ```

  ```ts
  // DirectionalLesson.svelte: θ から、両隣の j と重み w を求めて
  // 切り口の点 k: (a + r_k û, (1 − w) F[j][k] + w F[j+1][k])、接線: z = f(a) + g(θ) r（g も同じ補間。微分可能なら ∇f·û の式で正確に）
  ```

- **完了の条件**
  - $f = x^2 y$、$\mathbf a = (1, 2)$: グラフの最大が $\sqrt{17} = 4.1231056256$（$\theta = \operatorname{atan2}(1, 4) = 0.2449786631$）、
    0 になる所が $\theta = 1.8157749899$ と $4.9573676435$（どれも $10^{-6}$）
  - 切り口の点が曲面の上（標本の $\theta_j$ で $\lvert z - f\rvert < 10^{-12}$）。接線の傾きがパネルの値と $10^{-9}$ で一致
  - $\theta$ のスライダーを動かしている間、計算担当への依頼が増えない（`plotClient` の依頼の数で確かめる）
  - ビット 1024 を立てないとき、結果が今と同じ
- **README**: 「方向微分」の段落に学習表示

### X2: 部分和の列を累積で計算する

- **担当**: B（理由: 今の `limit::sequence` に標本の作り方を足すだけ。判定の言葉は今のまま）
- **目的**: $\lim_{n\to\infty}\sum_{k=1}^{n} a_k$ を、項 $2^{20}$ 個まで速く調べる（1.2 で、$1/k^2$ が「確認できません」になっている）
- **触るファイル**: `core/src/convergence.rs`（新規）、`core/src/expand.rs`（`limit_expression`）、`core/src/limit.rs`（`growing`）、`core/tests/convergence.rs`（新規）
- **やること**
  1. `limit_expression` で、数列の極限の中身が「上の端が極限の変数の $\Sigma$」（`@sum(k, a, n) 項`。項は $n$ を使わない）なら、
     項の `Program` を 1 つ作り、$k = a, a+1, \dots$ の値を 4,096 個ずつ `eval_batch` して**累積**（Neumaier の補償つき和。`sum_to_infinity` と同じ）し、
     $S_n$ の表から `sequence` の標本を返す。添字の上限を $2^{20}$ にする（今は 4,096）
  2. 対数の速さの発散: $d_j = S_{2^{j+1}} - S_{2^j}$ の列が正の定数 $c$ に近づく（$d_j$ を `finite_estimate` で外挿して $c > 10^{-3} \max(1, \lvert S\rvert)$）なら
     $+\infty$（負なら $-\infty$）。`growing` の `logarithmic` の条件（相対 $10^{-6}$ で一定）を、この外挿に替えるか、並べて使う
  3. 項が $n$ を使う式（$\sum_{k=1}^{n} \frac{1}{n + k}$）は今の道のまま
- **実装の土台**

  ```rust
  /// 項 term(k)（k = start, start+1, …）の部分和 S_n を、n = 添字の一覧（昇順）の所だけ返す
  pub fn partial_sums(term: &Program, start: i64, at: &[u64]) -> Result<Vec<f64>, String> {
      // k を 4096 個ずつ増やしながら eval_batch → 補償つきで足し、at の次の添字に来たら記録。項が NaN なら、それ以降は NaN
      // 上限 2^20 項。時間は初等関数の項で 30 ms ほどのはず（実装時に測る）
  }
  ```

- **完了の条件**（`core/tests/convergence.rs`。`plot` で極限だけの行として）

  | 式 | 結果 | 時間 |
  |---|---|---|
  | $\lim_{n\to\infty}\sum_{k=1}^{n} \frac{1}{k^2}$ | $\pi^2/6$（$10^{-8}$） | 100 ms 以内 |
  | $\lim_{n\to\infty}\sum_{k=1}^{n} \frac{1}{k^{1.5}}$ | $\zeta(1.5) = 2.612375348685488$（$10^{-6}$） | 同 |
  | $\lim_{n\to\infty}\sum_{k=1}^{n} \frac1k$ | $+\infty$ に発散 | 同 |
  | $\lim_{n\to\infty}\sum_{k=1}^{n} (-1)^k$ | 振動 | 同 |
  | $\lim_{n\to\infty}\sum_{k=1}^{n} \frac{1}{2^k}$、$\lim_{n\to\infty} (1 + 1/n)^n$ | 1、$e$（今と同じ） | — |

  `notation.rs` の数列の極限のテストが全部通る
- **README**: 「数列の極限」の段落に「部分和は $2^{20}$ 項まで」

### X3: 広義積分・級数を極限の仲間として出す

- **担当**: A（理由: 判定の目安（$p$ の推定）がずれると、発散する級数に「収束の目安」と出してしまう。$\sum 1/(k \ln k)$ が落とし穴）
- **平たい説明**（README にも書く）: $\sum_{k=1}^\infty a_k$ は部分和 $S_N = \sum_{k=1}^N a_k$ の $N \to \infty$ の極限、
  $\int_a^\infty f\,dx$ は $F(R) = \int_a^R f\,dx$ の $R \to \infty$ の極限。だから、極限と同じパネルで、$S_N$ や $F(R)$ がどう動くかを見せる
- **触るファイル**: `core/src/convergence.rs`、`core/src/equation.rs`（1 行に $\sum^\infty$・広義積分だけの行）、`core/src/lib.rs`（`convergence`）、
  `web/src/lib/ExpressionItem.svelte`（極限のパネルに「判定の目安」の欄）、`core/tests/convergence.rs`、README
- **やること**
  1. 1 行に `@sum(k, a, ∞) 項` か広義積分（∞ の端、または端で中身が発散する積分）だけがあり、**値が出ないとき**（今は誤りになる所。D10）、
     誤りの代わりに `Plot::Limit` を作る: 級数は X2 の部分和を `Graph::Sequence`、積分は $F(R)$（$R$ は ∞ の端なら $a + 2^j$、有限の端 $c$ なら $c \pm 2^{-j}$）を `Graph::Sampled`
  2. 判定は今の `Report`（収束・$\pm\infty$・振動・判定できない）
  3. 「判定の目安」（D11。値が出なかったときだけ）: 級数
     - **項の極限**: $\lim a_k$（`limit::sequence`）が 0 でない有限の値か振動 → 「項が 0 に近づかないので発散（第 $n$ 項判定法）」（確かな理由として出す）
     - **比**: $\rho = \lim \lvert a_{k+1}/a_k\rvert$。$\rho < 1$「収束の目安」、$\rho > 1$「発散の目安」、$\rho = 1$「比では決まりません」
     - **べき**: $\lvert a_k\rvert \approx C k^{-p}$ を、$k \in [2^{10}, 2^{15}]$ と $[2^{15}, 2^{20}]$ の 2 つの範囲で両対数の傾きから推定。
       2 つの $p$ の差が $0.02$ 以下なら、$p > 1$「収束の目安（$p \approx 1.01$）」、$p \le 1$「発散の目安」。差が大きければ「項は $k^{-p}$ の形ではありません」
     - **交代**: 符号が交互で $\lvert a_k\rvert$ が単調に減って 0 に近づく → 「交代級数なので収束（ライプニッツ）」
     積分は、∞ の端で $\lvert f\rvert \approx C x^{-p}$（同じ 2 つの範囲）、有限の端 $c$ で $\lvert f\rvert \approx C\lvert x - c\rvert^{-p}$（$p < 1$ で収束の目安）
  4. `eval_constant` は今のまま誤りを返す（`core/tests/improper.rs` の `refused` を壊さない）。変わるのは `plot` の 1 行だけの道
- **実装の土台**

  ```rust
  pub enum Hint { TermNotZero, Ratio(f64), Power { p: f64, stable: bool }, Alternating, EndPower { at: f64, p: f64, stable: bool } }
  pub struct ConvergenceReport { pub report: crate::limit::Report, pub hints: Vec<Hint> }
  pub fn series(term: &Program, start: i64) -> Result<ConvergenceReport, String>;
  pub fn improper(f: &Program, a: f64, b: f64) -> Result<ConvergenceReport, String>;
  /// 両対数の傾き: log|a_k| を log k に最小二乗で合わせる（k は範囲の中の 2 のべき 6 点と、その 3/2 倍の点）
  fn power(values: &[(f64, f64)]) -> Option<f64>;
  ```

  `MeshResult::convergence()`: `[目安の数, (種類, 値, 安定 0/1) × 目安の数]`。画面は種類ごとに決まった文を組み立てる
- **完了の条件**（`core/tests/convergence.rs`）

  | 式 | 判定 | 目安 |
  |---|---|---|
  | $\sum_{k=1}^\infty \frac1k$ | $+\infty$ に発散 | $p \approx 1.00$（発散の目安） |
  | $\sum_{k=1}^\infty (-1)^k$ | 振動 | 項が 0 に近づかない |
  | $\sum_{k=1}^\infty \frac{1}{k^{1.01}}$ | 判定できない（値は出ない） | $p = 1.01 \pm 0.005$（収束の目安） |
  | $\sum_{k=2}^\infty \frac{1}{k \ln k}$ | 「収束」と出さない | 「$k^{-p}$ の形ではありません」か発散の目安。**収束の目安を出さない** |
  | $\sum_{k=1}^\infty \frac{1}{k (\ln(k+1))^2}$ | 「収束」と出さない（値が出ない） | 収束の目安を出してよいが、$p$ の安定は求めない |
  | $\sum_{k=1}^\infty \frac{(-1)^{k+1}}{k}$ | 値 $\ln 2$（今と同じ。パネルは出さない） | — |
  | $\int_1^\infty \frac{dx}{x}$・$\int_0^1 \frac{dx}{x}$ | $+\infty$ に発散 | $p \approx 1$ |
  | $\int_1^\infty x^{-1.01}\,dx$ | 判定できない | $p \approx 1.01$（収束の目安） |
  | $\int_0^\infty \sin x\,dx$ | 振動 | — |
  | $\sum \frac{1}{k^2}$・$\int_0^\infty \frac{\sin x}{x}\,dx$ | 値（今と同じ。パネルは出さない） | — |

  1 行 200 ms 以内。`core/tests/improper.rs` が全部通る
- **README**: 「級数・広義積分の収束」の段落（平たい説明、判定と目安の違い、目安は証明ではないこと）

### X0: 多変数の極限の記法と、経路の極限

- **担当**: A（理由: 経路の選び方と判定の言葉（2.3）を誤ると、存在しない極限に値を出してしまう）
- **平たい説明**（README にも書く）: 1 変数では左右の 2 方向から近づくだけだが、平面では近づき方が無数にある（直線・放物線・らせん…）。
  2 つの近づき方で違う値になれば、極限は存在しない。全部の近づき方を数値で試すことはできないので、「存在する」とは言わない
- **触るファイル**: `web/src/lib/latex.ts`（`\lim_{(x,y)\to(a,b)}`）、`core/src/parser.rs`（`mlim`）、`core/src/multilimit.rs`（新規）、`core/src/expand.rs`・`equation.rs`・`lib.rs`、
  `web/src/lib/MultiLimitPanel.svelte`（新規）・`ExpressionItem.svelte`・`Viewer.ts`、`core/tests/multilimit.rs`（新規）、README
- **やること**
  1. `latex.ts`: `\lim_{\left(x,y\right)\to\left(a,b\right)}` → ` @mlim(x,y,a,b) `（3 変数も同じ。変数の数と点の成分の数が違えば誤り）。
     `\lim_{x\to a, y\to b}` は「多変数の極限は (x, y) → (a, b) の形で書いてください」（D8）
  2. `parser.rs`: `mlim` は、括弧の前半が束縛する変数の名前、後半が近づく先、中身は直後の項
  3. 経路（2 変数。点 $(a, b)$ からのずれを $s \to 0^+$ で）:
     - 直線 $(a + s\cos\theta_k,\ b + s\sin\theta_k)$、$\theta_k = k\pi/12$（24 本）
     - べき $y - b = c\,(x - a)^m$ と $x - a = c\,(y - b)^m$（$m = 2, 3$、$c = \pm\tfrac12, \pm1, \pm2$、$x - a = \pm s$ の両側）
     3 変数は、直線 26 本（立方体の面・辺・頂点の向き）と、$z - c_0 = (x - a)^2$ など座標の組ごとのべき 12 本
  4. 経路ごとに 1 変数の極限（`limit::analyze`、右から）。標本が全部 NaN の経路（定義されない所を通る）は数えない
  5. 判定（2.3）: 2 本が違う有限の値（差が誤差の見積もりの 100 倍以上）→ 存在しない。1 本でも $\pm\infty$・振動で、ほかに有限の値の経路がある → 存在しない。
     全部 $+\infty$ → 「どの経路でも $+\infty$ に発散して見えます」。全部同じ有限の値 $L$ → 「どの経路でも $L$ に近づいて見えます」（X1 の円の上の判定を足す）
  6. 結果の値（V0）は「近づいて見える」ときだけ $L$（≈）。存在しないときは値なし
  7. 画面（D9）: パネルに結論・違う値の 2 本の経路の式と値（例「$y = 0$ に沿うと 0、$y = x$ に沿うと 0.5」）・経路の一覧（折りたたみ）。
     3D に元の関数の曲面（点の周り、薄く）と経路の線（曲面の上。値で色分け）、経路の行き先の値に白丸
- **実装の土台**

  ```rust
  pub fn analyze(body: &Expr, vars: &[String], point: &[f64], params: &Params, sampler: &mut Sampler) -> Result<MultiReport, String> {
      // 経路ごとに、s の式で中身を書き直す（vars[i] に point[i] + path_i(s) を代入）→ Program（変数 "#s"）
      // → limit::program_samples を標本に limit::analyze(&mut sample, "s", 0.0, 1, removable, false)
      // 中身に積分などがあって Program にできないときは、expand の limit_samples の道（X4）
      // 経路の線（画面用）: s = R·2^{-j}（j = 0..40）の点 (x, y, f) を samples に
  }
  ```

- **やらないこと**: 4 変数以上、記号的な証明（はさみうちの式を作る）、極座標以外の変数変換
- **完了の条件**（`core/tests/multilimit.rs`）

  | 式 | 点 | 結論 | 根拠に出る経路 |
  |---|---|---|---|
  | $\frac{xy}{x^2 + y^2}$ | $(0, 0)$ | 存在しない | 値 0 の経路と 0.5 の経路（$y = x$） |
  | $\frac{x^2 y}{x^4 + y^2}$ | $(0, 0)$ | 存在しない | 直線は全部 0、$y = x^2$ で 0.5 |
  | $\frac{(x-1)(y-2)}{(x-1)^2 + (y-2)^2}$ | $(1, 2)$ | 存在しない | — |
  | $\frac{x^2 y^2}{x^2 + y^2}$ | $(0, 0)$ | 0 に近づいて見える | — |
  | $\frac{\sin(x^2 + y^2)}{x^2 + y^2}$ | $(0, 0)$ | 1 に近づいて見える | — |
  | $\frac{1}{x^2 + y^2}$ | $(0, 0)$ | どの経路でも $+\infty$ | — |
  | $\frac{xy + yz + zx}{x^2 + y^2 + z^2}$ | $(0, 0, 0)$ | 存在しない | 軸で 0、$x = y = z$ で 1 |

  初等関数の式で、2 変数 150 ms・3 変数 300 ms 以内
- **README**: 「多変数の極限」の段落

### X1: 極座標の図と、円の上の最大・経路の追跡

- **担当**: A（理由: 細い山（$y = x^3$ の上だけ値が大きい）を、$r$ を小さくしながら見失わずに追う所がいちばん難しい）
- **平たい説明**: 点を中心に半径 $r$ の円を描き、円の上での $f$ と $L$ の差の最大 $M(r)$ を見る。極限が $L$ であることは、
  $r \to 0$ で $M(r) \to 0$ になることと同じ。$M(r)$ が縮まなければ、円の上に必ず $L$ から離れた点があり、それをつなぐと「違う値に近づく経路」ができる
- **触るファイル**: `core/src/multilimit.rs`、`web/src/lib/MultiLimitPanel.svelte`、`core/tests/multilimit.rs`
- **やること**（2 変数だけ）
  1. $g(r, \theta) = f(a + r\cos\theta,\ b + r\sin\theta)$、$r_j = R \cdot 2^{-j}$（$j = 0, \dots, 40$）、$L$ は X0 の経路の値（無ければ $g(r_{40}, \cdot)$ の中央値）
  2. 各 $r_j$ で $M(r_j) = \max_\theta \lvert g(r_j, \theta) - L\rvert$ を求める: $\theta$ を 720 等分した格子の極大と、**前の $r$ で見つけた山の位置**の両方から、
     黄金分割で山の頂上を $10^{-14}$ まで詰める（山の幅は $r$ とともに細くなるので、前の $r$ の山から続けて追う）
  3. 判定: 最後の 10 段で $M$ が縮んでいく（両対数の傾き $p > 0.05$）→ 「円の上の差の最大も $M(r) \approx C r^{p}$ で縮んで見えます」。
     縮まない（$M(r_{j+1}) / M(r_j) > 0.9$ が最後の 10 段で続く）→ 山の頂上の点の列を経路として 1 変数の極限を求め、値 $L'$ を出して「存在しない」。
     経路の形を $y - b \approx c\,(x - a)^m$ に合わせて（両対数）、「$y \approx 1.0\,x^{3.0}$ に沿うと 0.5」のように添える
  4. 図（D9）: パネルに $g(r, \theta)$ の色の図（横 $\theta \in [0, 2\pi]$、縦 $\log_2 r$、色は $g - L$ を 0 を中心にした発散型の色の表）と、山の頂上の線、
     $M(r)$ の両対数の線。3D に追った経路の線
- **実装の土台**

  ```rust
  pub fn radial(g: &impl Fn(f64, f64) -> f64, radius: f64, limit: f64) -> Radial {
      let mut peaks: Vec<f64> = Vec::new();   // 前の r で見つけた山の θ
      for j in 0..=40 {
          let r = radius * 0.5f64.powi(j);
          // 1. 格子 720 点の |g − L| の極大の θ と、peaks の θ を候補に
          // 2. 候補ごとに、幅 (前の山の幅) の区間で黄金分割（区間の幅が 1e-14 になるまで）
          // 3. 近い候補をまとめ、値の大きい順に 8 個まで peaks に残す。M(r) = 最大値
          // 4. grid に g − L を 720 点（色の図用。f32）
      }
      // 最後の 10 段の傾きと比で判定（やること 3）
  }
  ```

- **完了の条件**（`core/tests/multilimit.rs`）

  | 式 | 期待 |
  |---|---|
  | $\frac{x^3 y}{x^6 + y^2}$（X0 の経路では全部 0） | 存在しない。$M(r) \to 0.5 \pm 0.01$、追った経路の値 $0.5 \pm 10^{-3}$、形 $y \approx x^{m}$ で $m = 3 \pm 0.05$ |
  | $\frac{x^5 y}{x^{10} + y^2}$ | 存在しない（山の幅が $r^4$ でも見失わない）、$m = 5 \pm 0.1$ |
  | $\frac{x^2 y^2}{x^2 + y^2}$ | $p = 2 \pm 0.05$ |
  | $\frac{xy}{\sqrt{x^2 + y^2}}$ | $p = 1 \pm 0.05$ |
  | $\frac{\sin(x^2 + y^2)}{x^2 + y^2}$ | $p = 4 \pm 0.1$（$L = 1$。差は $r^4/6$） |

  2 変数で 400 ms 以内（X0 と合わせて）
- **README**: 「多変数の極限」の段落に、極座標の図の読み方

### X4: 極限の中の積分・級数

- **担当**: B（理由: 今の標本の道（`limit_samples`）の計算量の配り方の仕事）
- **目的**: 利用者の答え (a)「極限の式の中に積分・級数が入る場合」を、多変数でも時間内に
- **触るファイル**: `core/src/expand.rs`（`limit_samples` の上限）、`core/src/multilimit.rs`、`core/tests/multilimit.rs`
- **やること**
  1. 中身を `Program` にできない（積分の端・級数の項が極限の変数を使う）式は、経路の数を減らす（直線 8 本、べき 4 本）
  2. 標本の上限（今は 1 行で 12,000）を、経路ごとに等分して配る。多変数の極限では全体を 24,000 にし、時間は 2 秒で打ち切る（打ち切ったら「判定できません（計算量の上限）」と、調べた経路だけの結果）
  3. 経路の上の標本は $s = R \cdot 2^{-j}$ の 48 点を基本にし、`finite_estimate` が収束を決めたら残りを計算しない
- **完了の条件**

  | 式 | 期待 | 時間 |
  |---|---|---|
  | $\lim_{(x,y)\to(0,0)} \frac{1}{xy}\int_0^{xy} \cos t\,dt$ | 1 に近づいて見える | 2 秒以内 |
  | $\lim_{x\to0} \frac1x \sum_{k=1}^{\infty} \frac{x^k}{k}$ | 1（1 変数。今の道） | 1 秒以内 |
  | $\lim_{(x,y)\to(0,0)} \frac{\int_0^{x} e^{-t^2}dt \cdot y}{x^2 + y^2}$ | 存在しない（$y = x$ で 0.5、$y = 0$ で 0） | 2 秒以内 |
  | $\lim_{x\to0}\lim_{y\to0} \frac{1}{xy}\int_0^{xy} \cos t\,dt$（今「計算量の上限」） | 1 | 2 秒以内 |

- **README**: 「多変数の極限」の段落に、積分・級数を含む式は遅いこと

## 6. 危ない所

- **f32 から値を読む**: 頂点の座標は f32 で、しかも表示範囲の 3 倍で止めてある（`mesh.rs` 416 行）。V1 の後、値をメッシュから読む所を残さない
- **「=」の主張**: 「=」は「10 桁の文字列から f64 がそのまま戻る」意味でしかない（`10^{50}` は数学の $10^{50}$ ではなく、その f64 の最近値）。
  README に「値は近似値」と書く。$\sin\pi = 1.22 \times 10^{-16}$ を 0 に丸めない（正直に出す）
- **「存在する」と言わない**: 経路の数・円の上の格子は有限なので、細い山を見落としうる（X1 は追跡で減らすが、無くならない）。言葉は 2.3 の表だけを使う
- **方向微分が全部 1 次でも、微分可能とは限らない**: 例えば放物線 $y = x^2$（原点を除く）の上だけ 1、ほかは 0 の関数は、原点でどの向きの方向微分も 0（1 次）だが、連続でもない。
  だから W1 は「微分可能です」とは言わない
- **$p$ の推定のずれ**: $\sum 1/(k \ln k)$ の項は $k^{-1 - 1/\ln k}$ のように見え、1 つの範囲では $p > 1$ と推定される。2 つの範囲で $p$ が合うことを必ず求める（X3）
- **記法の衝突**: `D_n` は数列の項（今の記法）。方向微分は太字・矢印・組の下付きだけ（D5）。`\frac{\partial f}{\partial x}(1,2)` が点での値にならない今の落とし穴を、`D_{\mathbf u} f(1,2)` で繰り返さない（W0 の `@at` に直す）
- **計算量**: 多変数の極限は経路の数だけ 1 変数の極限を計算する。積分を含む式は標本ごとに積分するので、X4 の上限と時間の打ち切りを必ず入れる
- **スライダーを動かしている間**: 値（V）は描くたびに出る。多変数の極限・収束の判定は重いので、再生・ドラッグ中（`PLOT_NO_REFINE`）は前の結果を残して、止まってから計算し直す

## 7. ほかの計画との関係

| 計画 | 関係 |
|---|---|
| 厳密値 | **将来の別計画**（利用者が自分のアルゴリズムで作る）。V0 の `Shown::exact` と V1 の `ShownValue.exact` を埋めれば画面に出る |
| `plan-math-notation.md` 7.3 の N4（点を押すと値・極値に印） | 値の書式（V1 の `valueText.ts`）を使う。文書 6 で軌跡の L2 と一緒のステップになる |
| `plan-locus-complex-integral.md` の Z（複素積分・留数） | 留数・積分の値の表示に V（複素数の書式）を使う |
| `plan-stage2b-integrals.md`（広義積分）・`core/tests/improper.rs` | X3 は `eval_constant` の誤りを変えず、1 行だけの式の `plot` の道で判定を足す |
| `core/src/limit.rs`（1 変数の極限・ε–δ） | W1・X0〜X4 の 1 本ずつの経路の計算。X2 で `growing` の対数の発散の条件を見直す |
| 文書 1（`plan-riemann-topology.md`） | ビット 256・512 は文書 1、1024 はこの文書（W2） |
| 文書 5 の N10（特殊関数の精度の検証） | V で 10 桁を出すので、特殊関数の誤差が見えやすくなる。N10 の表で、10 桁を信じてよい関数を確かめる |
| 文書 4（等角写像など） | 関係なし |

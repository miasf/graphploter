# 計画: 軌跡の問題と、複素積分

軌跡の問題（動く点がえがく図形・条件をみたす点の集合・包絡線・通過領域）と、複素積分（$\oint_C f(z)\,dz$・留数）を、
ほかの計画と同じ形の指示書にまとめる。実装担当には 1 ステップずつ（1 ステップ = 1 PR）渡す。
ステップの名前は、軌跡が **L0〜L7**、複素積分が **Z0〜Z5**（`plan-hard-tasks.md` の S・G・H、`plan-math-notation.md` の M・N と重ならないように）。
点を押す・ドラッグする **N4**（`plan-math-notation.md` 7.3 の N4 の残り）も、軌跡の L2 と一緒にこの文書で扱う。

各ステップの「担当」は `docs/agent-tiers.md` の段階。各ステップの「実装の土台」は、関数の形・データの形・処理の流れの下書き。
**土台は出発点で、命令ではない**。コードを読んで合わない所があれば直してよく、そのときは 0 章の表に「計画との違い」を書く。
（担当・N4・実装の土台は、2026-11-25 に文書 6（`docs/plan-roadmap.md`）として足した）

## 0. 進み具合（実装の記録）

| ステップ | 状態 | メモ（計画との違い） |
|---|---|---|
| （まだ無い） | | |

## 1. 調べた結果（2026-11-25。計算エンジンに直接式を入れて確かめた）

### 1.1 軌跡

| 確かめたこと | 結果 |
|---|---|
| 名前付きの点の定義 `A=(2,0)`、`P=(cos(t),sin(t))`、`M=(A+P)/2` を定義の行にして `M` を描く | **描ける**（文書 6 を書くときに確かめ直した: 媒介変数表示の曲線、頂点 1,203 個）。`expand.rs` の `Def::Value`（右辺にベクトルがある行）が、ほかの定義の名前を使える。$t$ を媒介変数とする曲線になる |
| `(A+P)/2` を直接描く | 描ける（同じ曲線） |
| ベクトルの長さ | **`abs(A-(3,4))` が組の長さ**（$\sqrt{17} = 4.1231…$）になる（確かめ直した）。`norm(…)` は式の中では「組は使えません」、`|…|` は計算エンジンの文字列には無い（LaTeX の `\left|…\right|` が `abs` になる）。L1 の `dist` は `abs` に落とせばよい |
| 動かしている点の残像 | **回転の残像だけある**（`Viewer.ts` の `rotationTrails`・`RotationSurfaceTrail.ts`）。スライダーを動かしたときの残像は無い |
| 媒介変数を消して方程式を出す | **無い**。`groebner.rs` は交点を解く用途だけ。係数は `i128` の分数で、あふれたら諦める。変数は 2 つか 3 つ |
| $\exists$（通過領域） | ある（`equation.rs` の `quantified`）。中身が等式なら範囲で重ねる。不等式は範囲を 32 等分した点で見る**近似** |
| `orbit` | 写像を繰り返す離散的な軌道（数列・行列・分岐図）。連続な媒介変数で動く点の軌跡とは別物 |
| `@locus`・`@mid`・`@envelope`・`@res` | どれも「その演算子はありません」（まだ無い。確かめ直した） |

### 1.2 複素積分

| 確かめたこと（エンジンの文字列） | 結果 |
|---|---|
| `@int(s,0,2*pi)((1/e^(i*s))*i*e^(i*s))`（$\oint_{\lvert z\rvert=1}\frac{dz}{z}$ を手で媒介変数にしたもの） | $2\pi i$（f64 で `6.283185307179586i`、実部 $2 \times 10^{-17}$）。**値が複素数の 1 変数の積分は、もう正しく計算できる** |
| `@int(s,0,2*pi)((e^(2e^(i*s))/(2e^(i*s)-1))*2i*e^(i*s))`（$\oint_{\lvert z\rvert=2}\frac{e^z}{z-1}dz$） | $17.0794684453…i$ $= 2\pi i e$（正しい。文書 6 で足した確かめ） |
| `@int(s,0,2*pi)((1/(e^(i*s)-0.5)^2)*i*e^(i*s))` | $0$（正しい。2 位の極の留数は 0） |
| 極が経路から 0.001 の所 `1/(e^(i*s)-0.999)` | **NaN**（正しくは $2\pi i$）。**0.01 の所（`-0.99`）でも NaN**（文書 6 で確かめた） |
| 極が経路の上 `1/(e^(i*s)-1)` | **$\pi i$ を黙って返す**（本当は発散。主値にあたる値になっている） |
| NaN になる理由（文書 6 でコードを読んだ） | 値が複素数の中身は、`eval::as_real` が NaN にするので、適応型の求積（`expand.rs` の `closed_integral_value` → `quad::integrate_split`）を通らない。代わりに `integral_checked`（端に点を集める置き換えの後、決まった 8 区間 × 15 点のガウス・クロンロッドの和の式。$\lvert K - G\rvert > 10^{-5}(1 + \lvert K\rvert)$ の所は NaN）で計算される。極が近いと点が足りず NaN |
| `@intset(( cos t, sin t),0<=t<=2 pi; {d_x})(1/(x+i*y)) + …{d_y}(i/(x+i*y))`（$dz = dx + i\,dy$ に分けて書いたもの） | $2\pi i$。**媒介変数で書いた経路なら、今でも書き方を工夫すれば計算できる** |
| 同じものを、経路を等式 `x^2+y^2=1` で書く | エラー（`implicit.rs` の、点と重みを数値で求める道が、値が複素数の被積分関数に対応していない） |
| `\oint_{|z|=1}\frac{1}{z}\,dz`（LaTeX の変換は `@intset((z),abs(z)=1; {d_z})(…)`） | エラー「等式の数が変数の数と同じなので、範囲が点になります」（$z$ を 1 つの実数の変数と読んでいる） |
| `\oint_{\{e^{it}\mid 0\le t\le 2\pi\}}\frac{1}{z}\,dz`（変換は `@intset(e^(it),…; {d_z})(…)`） | エラー「積分の dz の文字が、範囲の変数（t, x）にも、ds・dS・dA・dV にもなっていません」 |
| 留数 `\operatorname{Res}` | 無い |
| 積分の値の画面への道 | `App.svelte` の `integralValues`（`value?: number`）と `evalValue`（`evalExpr.ts`）は**実数だけ**。複素数の値は画面に出せない（文書 2 の V0 が複素数を返す形にする） |

**まとめ**: 複素積分は、積分そのものの計算は今の求積で足りる。足りないのは (1) $dz$ と複素平面の経路の書き方、(2) 特異点が経路に近い・経路の上にあるときの扱い（今の複素数の道は点の数が決まっている）、(3) 留数と、描き方、(4) 複素数の値を画面に出す道。

### 1.3 点を押す・ドラッグ（N4）の今（文書 6 で調べた）

`plan-math-notation.md` 7.3 の N4 は、**前半がもう入っている**（コミット `4b3a2b3`・`9a6edba`・`6b758d5`）:

| 機能 | 状態 | 場所 |
|---|---|---|
| グラフを押すと、一番近い曲線の点・曲面の点に印と座標の札 | 済み（⚙ 設定 →「操作」の「グラフを押したら値を出す」。既定はオフ） | `Viewer.ts` の `setupPick`・`pickAtScreen`・`showPick`、`App.svelte` の `handlePick` |
| 押した点を式の上へ戻す | **陽関数の行だけ**（押した横・奥の座標で式を計算し直す）。陰関数・媒介変数は、画面の折れ線・三角形の上の点のまま | `App.svelte` の `handlePick` の後半 |
| 陽関数の注目する点（極大・極小・零点・切片・鞍点） | 済み（`flags` のビット 4 = `PLOT_POI`。止まっているときだけ求める） | `critical.rs` の `points_of_interest`、`lib.rs` の `last_poi`、`plot.worker.ts` 499 行 |
| 停留点の分類 `\operatorname{crit} f` | 済み（`sin(x) cos(y)` の極値が全部見つかるテストあり。N4 の完了の目安は満たしている） | `critical.rs` の `find`、`equation.rs` の `critical_line`、`core/tests/learning.rs` |
| グラフの上で点をドラッグする（動く点・`A=(a,b)` の点・常微分方程式の初期値・測地線の始点） | **無い**。ドラッグは全部、視点の操作になる | — |

**残り**（この文書の N4 ステップ）: 陰関数・媒介変数で押した点を式の上へ戻すことと、点のドラッグ。

### 1.4 つなぐ所（実装の土台の元。行番号は 2026-11-25 の目安）

| 部品 | 場所 | 使うステップ |
|---|---|---|
| 行の種類の振り分け（特別な行を順に試す） | `equation.rs` の `compile_with` の中、665〜700 行（`quantified` → `perp_line` → `@cobweb` → `@orbit` → `sequence_line` → `ode_line` → `critical_line` → `matrix_line`） | L2・L6（`locus_line`・`envelope_line` をここに足す） |
| 記法の展開（演算子ごとの分岐） | `expand.rs` の 1369〜1440 行（`Expr::Op { op, .. } if op == "intset"`・`"taylor"` など）、ベクトルの演算は 1509 行の `"norm"` の辺り | L1・Z1・Z3 |
| 定義の値 | `expand.rs` の `Def::Value`（66 行の `enum Def`、1191 行で展開） | L1・L2・N4 |
| 線積分・面積分 | `expand.rs` の `region_integral`（3371 行。`; {d_…}` の印を取り出す）→ `parametric_integral`（2767 行） | Z1 |
| 1 重の積分 | `expand.rs` の `integral`（2176 行）→ 実数なら `closed_integral_value`（2452 行。`breakpoints::find` と `quad::integrate_split`）、そうでなければ `integral_checked` | Z1・Z2 |
| 多項式・グレブナー基底 | `groebner.rs`（`Rat` は 20 行の `i128` の分数、`Mono = [u16; 3]` は 109 行、`solve` は 407 行、`polynomial_integral` は 391 行） | L4 |
| 記号微分 | `diff.rs` の `diff`（292 行）・`diff_n`（403 行） | L6・Z1 |
| べき級数（複素数の中心も可） | `series.rs` の `taylor(e, var, a: C, n, value)`（133 行） | Z3 |
| 面へ寄せるニュートン法 | `system.rs` の `newton_onto`・`newton_onto_in`（91 行） | N4 |
| 量化子 | `equation.rs` の `quantified`（1732 行） | L6 |
| 集合の書き方 | `latex.ts` 1252 行（`{点 | 条件}` → `⦃点∣条件⦄`）、`logic.rs` の `parse_formula` | L3 |
| 回転の残像 | `Viewer.ts` の `rotationTrails`（190 行）・`recordRotationTrail`（1748 行）・`RotationSurfaceTrail.ts` | L2・Z4 |
| 押した点 | `Viewer.ts` の `setupPick`（470 行）・`pickAtScreen`（500 行）・`grabPlane`・`rayAt`（788 行） | N4 |
| スライダーの値を変える入口 | `App.svelte` の `setParam(item, re, im)`（1762 行。刻み・条件の行で止める処理も入っている）、動かしている間の粗い描画は `isCoarse`（880 行） | N4 |
| 積分の分類・図 | `latex.ts` の `integralKinds`（2194 行）・`integralDisplay`（2390 行）、`\oint` は 1041 行 | Z1・Z4 |
| ⚙ の項目を足す道（手本は曲率の色分け N2） | `types.ts` のビットの定数 → `plot.worker.ts` 137 行の `req.flags & (…)`（**キャッシュの鍵。新しいビットはここに足す**）→ `lib.rs` の `plot` の中、`build` の直後 → `MeshResult` のフィールドと getter → `ExpressionItem.svelte` | L2・L4 |
| 使っているビット | `flags` のビット 0〜7 が使用中。8（256）・9（512）は文書 1、10（1024）は文書 2、11（2048）は文書 3 が予約 | この文書は **12（4096）** を L4・L5 に使う |

## 2. 全体の決めごと

### 2.1 軌跡は `orbit` に混ぜず、`\operatorname{locus}` を新しく作る

`orbit` は「写像を $N$ 回繰り返す」離散的な軌道で、引数は開始項と回数。軌跡は「媒介変数を連続的に動かす」もので、引数は点と媒介変数と範囲。
1 つの命令に入れると引数の意味が場合で変わるので、分ける。`orbit` は今のまま（README の説明に「連続な軌跡は `locus`」と 1 行足す）。

### 2.2 軌跡の答えは「図」「方程式」「除く所」の 3 つ

高校の軌跡の問題の答えは「方程式」と「ただし … を除く」。図だけでなく、式の下に次の 2 つを出す（L4・L5）:

- **軌跡の方程式**: 媒介変数を消した多項式の等式（多項式で書けるときだけ。書けなければ「方程式は求められません（図だけ）」）
- **除く所**: 方程式の図形のうち、媒介変数の範囲では届かない点・部分（点なら「ただし点 $(-1, 0)$ を除く」、部分なら「ただし $x \ge 0$ の部分」）

方程式は「媒介変数がすべての複素数を動いたとき」の図形（ザリスキ閉包）なので、範囲の制限から生まれる除外は、消去だけでは出ない。
L5 はこの差を数値で調べる。**これを自動で出せることが、この計画の売り**。

### 2.3 複素積分は「経路を媒介変数にして、1 変数の積分に落とす」

$$\oint_C f(z)\,dz = \int_a^b f(\gamma(t))\,\gamma'(t)\,dt$$

$\gamma'$ は記号微分（M3）で作る。求積は今の 1 変数の積分（値が複素数でも動くことは 1.2 で確かめた）。
積分の中の $z$ は**積分の変数**（束縛変数）として扱い、3 次元の座標 $z$ とは別にする（`\int … dx` の $x$ と同じ扱い）。

### 2.4 経路の上の特異点は、エラーにする

経路の上に極があると積分は発散する。今は $\pi i$（主値）を黙って返している。**エラー「経路の上に特異点があります（z = 1 の近く）」にする**。
主値がほしい場合は、あとで `\operatorname{PV}` を足す（この計画ではやらない）。

### 2.5 ドラッグは「点 → スライダーの値」に直して、今の入口から動かす

グラフの上で点をつかんで動かしても、**式の文字は変えない**。動かした結果は、その点を決めているスライダー（パラメータ）の値として
`App.svelte` の `setParam` に渡す（刻みへの丸め・条件の行で止める処理・共有リンクへの保存が、今のまま効く）。
どの点が、どのスライダーで、どう動くかは、計算エンジンが「ドラッグの取っ手」（2.6 の `Handle`）として返す。画面側は式を読まない。

### 2.6 実装の土台（全体の作り）

**新しいファイル**

| ファイル | 中身 |
|---|---|
| `core/src/expand/geometry.rs` | 図形の関数（L1） |
| `core/src/locus.rs` | `locus` の行（L2）、作図の線の組、ドラッグの取っ手（N4） |
| `core/src/bigint.rs` | 自前の多倍長の整数と分数（D1。L4） |
| `core/src/eliminate.rs` | 媒介変数の消去（L4）、除く所（L5）、包絡線（L6） |
| `core/src/contour.rs` | 複素平面の経路（Z1）、経路の近くの特異点（Z2）、回転数（Z3） |
| `core/src/residue.rs` | 留数（Z3） |
| `core/src/pick.rs` | 押した点を式の上へ戻す（N4） |
| `web/src/lib/dragPoints.ts`・`dragPoints.test.ts` | 取っ手の一覧と、カーソル → スライダーの値（N4） |
| `core/tests/locus.rs`・`core/tests/complex_integral.rs`・`core/tests/pick.rs` | 物差し |

**データの形（下書き）**

```rust
// locus.rs
pub struct Locus {
    pub point: Vec<Expr>,          // M の成分（定義を展開した後。媒介変数 var を含む）
    pub var: String,               // 媒介変数の名前（t）
    pub range: (f64, f64),         // 範囲
    pub open: (bool, bool),        // 端を含まないか（L5 で使う）
    pub named: Vec<(String, Vec<Expr>)>,  // 作図の点（A, P …）の成分
    pub segments: Vec<(usize, usize)>,    // named の番号の組（結ぶ線分）
}
/// ドラッグの取っ手（N4）。画面の点の位置と、動かしたときに変えるスライダー
pub struct Handle {
    pub at: [f64; 3],              // 今の位置（表示中の座標）
    pub kind: HandleKind,
}
pub enum HandleKind {
    /// 1 つのパラメータで決まる点: その値を変えると曲線の上を動く。curve は (値, 点) を細かく取った列
    Along { param: String, curve: Vec<(f64, [f64; 3])> },
    /// 2 つ（3D は 3 つ）のパラメータの 1 次式で決まる点: p = p0 + J (a − a0)。J を解いて a を出す
    Free { params: Vec<String>, jacobian: Vec<f64> /* 3 × k */ },
    /// 欄の値（常微分方程式の初期値・測地線の始点）。画面側が欄を書き換える
    Field { which: String },
}

// eliminate.rs
pub struct Equation { pub poly: BigPoly, pub latex: String, pub completed: Option<String> /* 平方完成した形 */ }
pub enum Excluded { Point([f64; 2]), Part { pts: Vec<[f64; 2]>, text: Option<String> } }

// contour.rs
pub struct Piece { pub gamma: Expr, pub dgamma: Expr, pub var: String, pub a: f64, pub b: f64 }
pub struct Contour { pub pieces: Vec<Piece>, pub closed: bool, pub label: String }
```

**画面までの道**

1. `MeshResult` に足すもの（どれも getter を付ける。無いときは空）:
   - `locus_now: Vec<f64>`（今の点 3 つの座標）、`locus_ts: Vec<f32>`（曲線の頂点ごとの媒介変数の値。N4 のドラッグで使う）、`construction: Vec<f32>`（作図の線分の端の組。線分ごとに 6 個）
   - `locus_equation: String`（LaTeX。L4）、`locus_excluded: Vec<f64>`（`[種類, 点の数 n, x₀, y₀, …]` を続けて。種類 0: 孤立した点、1: 部分）・`locus_excluded_text: String`（L5）
   - `handles: Vec<f64>`（`[x, y, z, 種類, 名前の番号, …]`）と `handle_names: String`（N4）
   - `contour: Vec<f64>`（経路の点の列・向き・特異点と留数。Z4）
2. ビット 12（4096）`PLOT_LOCUS_EQUATION`: 方程式と除く所（L4・L5。1 秒かかることがある）を、**止まっているとき・式の ⚙ の「方程式を求める」がオンのときだけ**求める。`plot.worker.ts` 137 行のキャッシュの鍵にも足す
3. 取っ手（N4）は軽いので、ビットを使わずいつも返す
4. 複素数の値の表示は、文書 2 の V0（`core/src/value.rs` の `Values`）・V1（`valueText.ts`）を使う（D7）

## 3. 決めたこと（2026-11-25。全部おすすめの案で決まった）

| # | 内容 | 決まったこと |
|---|---|---|
| D1 | 消去法の係数の大きさ（`i128` ではあふれやすい） | **自前の小さな多倍長整数**（加減乗除と最大公約数だけ。300 行ほど）。依存を増やさない |
| D2 | 軌跡の命令の名前 | `\operatorname{locus}(M, t)`（範囲は媒介変数の範囲の欄。`\operatorname{locus}(M, t, 0, \pi)` でも書ける）。キーの名前は「軌跡」 |
| D3 | 除く所の出し方 | 図では**白抜きの丸（点）と点線（部分）**、式の下に文章 |
| D4 | 経路の上の特異点 | エラー（2.4） |
| D5 | ドラッグできる点（文書 6 で足した。おすすめの案） | (1) 1 つのパラメータで決まる点（`locus` の今の点、`P=(\cos a, \sin a)`）は、その曲線に沿って動く。(2) 2 つ（3D は 3 つ）のパラメータの 1 次式で決まる点（`A=(a,b)`・`(a+1, 2b)`）は自由に動く。(3) 常微分方程式の初期値・測地線の始点の欄が数だけなら、その欄を書き換える。それ以外の点は動かせない（押すと札に「動かせません（a と b の両方を使う曲がった式のため）」） |
| D6 | ドラッグの有効・無効（文書 6 で足した。おすすめの案） | 「グラフを押したら値を出す」の設定と関係なく、いつも有効。動かせる点は白い縁の輪で示し、輪の 12 画素以内で押し始めたときだけドラッグ（それ以外は今の視点の操作） |
| D7 | 複素積分の値の表示（文書 6 で足した。おすすめの案） | 文書 2 の V0・V1（複素数の値と書式）を先に入れ、Z1 はその上に乗せる。V0 が無いまま Z1 をやるなら、`integralValues` に虚部を足す最小限だけを Z1 に入れ、0 章に書く |

D5〜D7 は、`plan-roadmap.md` の「文書 6 は決めることを全部おすすめで決定済み」に従って、おすすめの案で決めた。違う案にしたいときは、この表を書き換えてから実装する。

## 4. ステップの一覧

| # | 内容 | 前提 | 大きさ | 担当 |
|---|---|---|---|---|
| L0 | 軌跡の物差し（テストの表） | — | 小 | C |
| L1 | 図形の関数（中点・内分・垂線の足・対称点・回転・2 直線の交点・重心など） | — | 小〜中 | C |
| L2 | `\operatorname{locus}`: 軌跡の曲線・今の点・残像・作図の線 | L1 | 中 | B |
| N4 | 点を押す・ドラッグする（押した点を式の上へ戻す、動く点・スライダーの点・初期値のドラッグ） | L2（ドラッグの (2)(3) と押した点は L2 の前でもよい） | 中 | B |
| L3 | 条件で決まる軌跡（`\{P \mid |P-A| = 2|P-B|\}`） | L1 | 小 | C |
| L4 | 媒介変数を消して、軌跡の方程式を出す | L2 | 大 | A |
| L5 | 除く所の判定 | L4 | 大 | A |
| L6 | 包絡線と、通過領域の境目を正確に | L4, M3 | 中〜大 | B |
| L7 | 交点で決まる点（値が複数ある点）の枝の追跡 | 文書 1 の R5 | 大 | A |
| Z0 | 複素積分の物差し | — | 小 | C |
| Z1 | $dz$ と複素平面の経路の書き方 | 文書 2 の V0（D7） | 中 | B |
| Z2 | 経路の近く・上の特異点 | Z1 | 中 | A |
| Z3 | 留数と回転数 | Z1 | 中 | B |
| Z4 | 描き方（経路の向き・特異点と留数・途中までの積分の値の軌跡） | Z1, Z3 | 中 | B |
| Z5 | 多価関数を経路に沿って続ける（$\oint \sqrt z\,dz$ を枝の切れ目で飛ばさない） | 文書 1 の R5 | 中 | A |

担当の理由は、各ステップの「担当」の行に 1 行で書いた。

**おすすめの順番**: L0 → L1 → L2 → N4 → L3（ここまでで「動く点の軌跡」が使えて、点をドラッグできる）→ Z0 →（文書 2 の V0）→ Z1 → Z2 → Z3 → Z4 → L4 → L5 → L6 →（文書 1 の R0〜R5）→ L7・Z5。

Z を L4 より先にするのは、Z1〜Z4 が今の求積の上に乗る中くらいの大きさで、すぐ使えるようになるから。L4・L5 は多倍長の土台（D1）が要る大きい仕事。
L7 と Z5 は、N8 の「となりの点に一番近い枝を選ぶ」追跡と同じ仕組みなので、文書 1（`plan-riemann-topology.md`）の R5 で公開される `branch::continue_along` を使う。R0 の試作で N8 を止めると決まったら、L7・Z5 もやらない。

## 5. ステップごとの指示書

共通の決まり（`plan-math-notation.md` の 2.7 と同じ）: コメントと README は日本語。`cargo test --release`（`core/`）と `npm test`・`npm run check`（`web/`）が通ること。
新しい記法は `latex.ts` が構造を写すだけで、意味は計算エンジン（`expand.rs`・`equation.rs`）で決める。README の機能の一覧に、使い方と例を足す。
テストのエラーの中身は WASM 以外では読めない（`JsError` を作るとパニックする）ので、計算エンジンの内側の関数（`Result<_, String>` を返すもの。
`equation::compile`・`expand::expand`・`eval::Program`）をテストから呼ぶか、`std::panic::catch_unwind` で成功・失敗を見る（`core/tests/learning.rs` の `run`・`error_of` が手本）。

### L0: 軌跡の物差し

- **担当**: C（理由: 正解は手で決まっている表を、テストの形に写すだけ）
- **目的**: 以降のステップの完了の条件を、数で確かめられるようにする
- **触るファイル**: `core/tests/locus.rs`（新規）
- **やること**: 次の表を、正解と一緒にテストにする（初めは `#[ignore]` を付け、各ステップで外す）

  | 問題 | 正解（方程式） | 除く所 |
  |---|---|---|
  | $A(2,0)$、$P$ が単位円の上、$AP$ の中点 $M$ | $(x-1)^2 + y^2 = \frac14$ | なし |
  | 同じ。ただし $0 < t < \pi$ | 同じ | $y \le 0$ の部分（端の 2 点 $(\frac12, 0)$・$(\frac32, 0)$ を含む） |
  | $\left(\frac{1-s^2}{1+s^2}, \frac{2s}{1+s^2}\right)$、$s \in \mathbb R$ | $x^2 + y^2 = 1$ | 点 $(-1, 0)$ |
  | $(t^2, t^4)$ | $y = x^2$ | $x < 0$ の部分 |
  | 放物線 $y = x^2 + 2ax + a$ の頂点（$a$ が動く） | $y = -x^2 - x$ | なし |
  | $PA : PB = 2 : 1$（$A(0,0)$、$B(3,0)$） | $(x-4)^2 + y^2 = 4$ | なし |
  | 直線 $y = 2ax - a^2$ の包絡線 | $y = x^2$ | なし |
  | 同じ直線の、$0 \le a \le 1$ での通過領域 | 境目は $y = x^2$（$0 \le x \le 1$）、$y = 0$、$y = 2x - 1$ | — |

  曲線の比べ方: 描いた点の列のどの点も、正解の曲線から $10^{-9}$（表示範囲の幅に対する比）以内。正解の曲線を細かく取った点から見ても同じ（見落としが無い）。
- **実装の土台**

  ```rust
  // core/tests/locus.rs
  struct Case { src: &'static str, defs: &'static str, equation: &'static str /* 正解の多項式（計算エンジンの文字列） */,
                truth: fn(f64, f64) -> f64 /* 正解の陰関数 */, excluded: &'static [Excl] }
  enum Excl { Point(f64, f64), Part(fn(f64, f64) -> bool /* 除く部分の条件 */) }
  /// 描いた頂点が正解の曲線の上にあるか: |truth(p)| / |∇truth(p)| ≤ 1e-9 × 表示範囲の幅（勾配は中心差分）
  fn on_curve(pos: &[f32], truth: fn(f64, f64) -> f64, width: f64) -> f64 /* 最大のずれ */;
  /// 見落とし: 正解の曲線を 2000 点に取り（媒介変数で書ける正解は媒介変数で）、各点から描いた折れ線までの距離の最大
  fn coverage(pos: &[f32], truth_pts: &[[f64; 2]]) -> f64;
  /// 方程式の比べ方（L4 で外す）: 両方を有理数の係数の多項式にし、一番大きい次数の係数で割って全部の係数が一致
  fn same_poly(got_latex: &str, want: &str) -> bool;
  ```

  `run` は `core/tests/learning.rs` の `run` を写す（`plot` の 25 個の引数は位置で渡す）。範囲の欄は `plot` の `ranges` 引数（今の `&[0.0, 5.0]` の所）で渡す
- **やらないこと**: 実装
- **完了の条件**: 表がテストになっていて、今の状態で描けるもの（1 行目の曲線だけ。定義の行 `M=(A+P)/2` を `M` で描く）は通る
- **README**: 変えない

### L1: 図形の関数

- **担当**: C（理由: どの関数も教科書の式が決まっていて、手で書いた式と比べるだけで確かめられる）
- **目的**: 作図の言葉で点を定義できるようにする
- **触るファイル**: `core/src/expand/geometry.rs`（新規）、`core/src/expand.rs`（関数の名前の分岐）、`web/src/lib/latex.ts`、`web/src/lib/names.ts`、数式キーボードの定義
- **やること**: 展開の段で、ふつうの式（成分の組）に落とす関数を足す。すべて 2 次元と 3 次元の点で動く（回転と垂線の足は 2 次元だけでもよい）
  - `\operatorname{mid}(A,B)`、内分 `\operatorname{divide}(A,B,m,n)` $= \frac{nA + mB}{m+n}$（外分は $n$ を負に）、重心 `\operatorname{centroid}(A,B,C)`
  - `\operatorname{dist}(A,B)` と `|A-B|`（今の `abs` が組の長さになる。1.1）
  - 垂線の足 `\operatorname{foot}(P,A,B)`（直線 $AB$ へ）、対称点 `\operatorname{reflect}(P,A,B)`（直線 $AB$ について）・`\operatorname{reflect}(P,A)`（点 $A$ について）
  - 回転 `\operatorname{rotate}(P,A,\theta)`（$A$ の周りに $\theta$。2 次元）
  - 2 直線の交点 `\operatorname{meet}(A,B,C,D)`（直線 $AB$ と $CD$。平行なら値が無い点）
  - 外心・内心・垂心 `\operatorname{circumcenter}`・`\operatorname{incenter}`・`\operatorname{orthocenter}`
- **実装の土台**

  ```rust
  // core/src/expand/geometry.rs。どれも成分の式の Vec<Expr>（2 か 3 個）を受け取り、成分の式を返す（数値にしない。媒介変数 t を含んだまま）
  pub fn mid(a: &[Expr], b: &[Expr]) -> Vec<Expr>;                   // (a + b) / 2
  pub fn divide(a: &[Expr], b: &[Expr], m: &Expr, n: &Expr) -> Vec<Expr>; // (n a + m b) / (m + n)
  pub fn foot(p: &[Expr], a: &[Expr], b: &[Expr]) -> Vec<Expr>;      // a + ((p−a)·(b−a) / |b−a|²)(b−a)
  pub fn reflect_line(p: &[Expr], a: &[Expr], b: &[Expr]) -> Vec<Expr>; // 2 foot − p
  pub fn rotate(p: &[Expr], a: &[Expr], th: &Expr) -> Result<Vec<Expr>, String>; // 2 次元だけ
  pub fn meet(a: &[Expr], b: &[Expr], c: &[Expr], d: &[Expr]) -> Result<Vec<Expr>, String>;
      // 2 次元: クラメルの公式。分母 det = (b−a)×(d−c) が 0 になる所は、式の値が NaN（割り算のまま残せば自然にそうなる）
  pub fn circumcenter(a: &[Expr], b: &[Expr], c: &[Expr]) -> Vec<Expr>; // 3 次元は a,b,c の平面の中で
  pub fn incenter(..) -> Vec<Expr>;    // (|bc| a + |ca| b + |ab| c) / (|bc| + |ca| + |ab|)
  pub fn orthocenter(..) -> Vec<Expr>; // a + b + c − 2 × 外心（オイラー線）
  ```

  つなぐ所: `expand.rs` の、関数の名前で分ける所（1509 行の `"norm"` と同じ並び）に `"mid" | "divide" | …` を足し、引数を `self.mat(&args[k], &what)` で成分の列にして、上の関数を呼んで組（`Expr::Tuple`）で返す。
  **利用者の定義が先**: `expand.rs` で定義（`Def`）を引く所が先に当たるようにし、`mid(x) = …` を定義したら、そちらを使う（テストを 1 つ）。
  LaTeX は `\operatorname{mid}` を `@mid` ではなく関数の呼び出し `mid(…)` に写す（今の `\operatorname{tr}` などと同じ扱いか、`latex.ts` の演算子の表を見て合わせる）
- **やらないこと**: 円と直線の交点（値が 2 つある。L7）
- **完了の条件**: それぞれの関数の値が、手で書いた式と一致（$10^{-12}$。点を数で入れて比べる）。`M = \operatorname{mid}(A,P)` で L0 の 1 行目が描ける
- **壊してはいけないもの**: 同じ名前の利用者の定義（`mid(x) = …` を定義したら、そちらを優先する）
- **README**: 機能の一覧に「図形の関数」の段落

### L2: `\operatorname{locus}`

- **担当**: B（理由: 新しい行の種類を、計算エンジンの判定から画面の残像・作図の線まで通す。今の仕組み（定義・回転の残像）に乗る）
- **目的**: 動く点の軌跡を「曲線・今の点・残像・作図の線」として見せる
- **触るファイル**: `core/src/locus.rs`（新規）、`core/src/equation.rs`（行の種類の判定）、`core/src/lib.rs`、`web/src/lib/Viewer.ts`（残像。`rotationTrails` の仕組みを、スライダーの値が変わったときにも使えるようにする）、`web/src/lib/ExpressionItem.svelte`
- **やること**
  1. `\operatorname{locus}(M, t)`・`\operatorname{locus}(M, t, a, b)`: $t$ の範囲（省略時は媒介変数の範囲の欄）で $M$ を曲線にする
  2. $t$ がスライダーでもあるとき（同じ名前のパラメータがある）、スライダーの値の所の $M$ を点で描き、▶ で動かすと残像を残す（⚙ に「残像」のチェックと「消す」）
  3. ⚙ の「作図の線」: $M$ の定義に出てくる名前付きの点（$A$・$P$）を点で描き、定義の式に出てくる 2 点の組を線分で結ぶ（例: $AP$）。どの組を結ぶかは、L1 の関数の引数から決める（`mid(A,P)` なら $AP$）
  4. 2D 表示と 3D 表示の両方
- **実装の土台**

  ```rust
  // equation.rs の 665〜700 行の並び（critical_line の前）に:
  //   if let Some(p) = crate::locus::locus_line(source, params, space)? { return Ok(p); }
  // Plot に足す: Locus { curve: Box<Plot> /* Plot::Curve */, now: Option<[f64; 3]>, named: Vec<[f64; 3]>, segments: Vec<(usize, usize)>, ts: Vec<f64> }

  pub fn locus_line(source: &str, params: &Params, space: &Space) -> Result<Option<Plot>, String> {
      // 1. `@locus(` で始まる行だけ。引数は (点, 変数) か (点, 変数, a, b)
      // 2. 点が名前（M）なら、定義を引いて「定義の式の木」を残す（作図の線のため）。expand は木の写しにかけ、展開した成分を Locus.point に
      // 3. 作図の線: 定義の式の木をたどり、L1 の関数の引数と、2 つの名前付きの点の差（P − A）に出てくる名前の組を segments に（重複は除く）。
      //    名前付きの点は、その定義を展開した成分を named に。媒介変数を含んでよい（t の今の値で計算する）
      // 4. 曲線: 今の媒介変数の曲線と同じ道（Plot::Curve）。範囲は引数か、範囲の欄
      // 5. 今の点: params に var と同じ名前があれば、その値で point を計算（無ければ None）
      // 6. ts: 曲線の頂点ごとの媒介変数の値（N4 のドラッグで使う）。曲線を作る所で、頂点と同じ順に記録する
  }
  ```

  画面側:
  - `MeshResult` の `locus_now`・`construction`・`locus_ts`（2.6）を `types.ts` の `MeshData` に足し、`Viewer.ts` で今の点（太い点）と作図の線（細い灰色の線分）を描く
  - 残像: `recordRotationTrail` と同じ形の `recordParamTrail(id, value)` を足す。`App.svelte` で、`locus` の媒介変数と同じ名前のスライダーが変わったら（`paramsChanged` の所）、その式の `locus_now` を記録する。
    残像は点の列（古いものほど薄く）。⚙ の「残像」がオフなら記録しない、「消す」で `clearParamTrail(id)`
- **やらないこと**: 方程式（L4）、ドラッグ（N4）
- **完了の条件**: L0 の 1・2 行目の曲線が正しい（除く所は無視）。▶ で 1 周させたときに、残像と曲線が重なる（残像の点が、曲線の折れ線から 1 画素（表示範囲の幅の $\frac{1}{1000}$）以内）。作図の線の組が `mid(A,P)` で $\{AP\}$、`meet(A,B,C,D)` で $\{AB, CD\}$
- **壊してはいけないもの**: `orbit`・回転の残像
- **README**: 「軌跡」の段落。`orbit` の説明に「連続な媒介変数の軌跡は `locus`」

### N4: 点を押す・ドラッグする

- **担当**: B（理由: 画面の操作（視点の操作との区別）と計算エンジンの両方を触るが、どれも今ある押す仕組み・スライダーの入口に乗る）
- **目的**: `plan-math-notation.md` 7.3 の N4 の残り。押した点を、陰関数・媒介変数でも式の上の正確な点にする。動く点をグラフの上でつかんで動かせるようにする（Desmos の動かせる点）
- **前提**: 押した点（やること 1）とドラッグの (2)(3)（D5）は、L2 の前でもできる。`locus` の今の点のドラッグ（D5 の (1)）は L2 の後
- **触るファイル**: `core/src/pick.rs`（新規）、`core/src/locus.rs`（取っ手）、`core/src/lib.rs`（`pick_onto`・`handles`）、`web/src/lib/dragPoints.ts`・`dragPoints.test.ts`（新規）、`web/src/lib/Viewer.ts`、`web/src/App.svelte`、`web/src/lib/evalExpr.ts`、`core/tests/pick.rs`（新規）
- **やること**
  1. **押した点を式の上へ戻す**（今は陽関数だけ。1.3）:
     - 陰関数（2D の曲線 $F(x, y) = 0$・3D の面 $F(x, y, z) = 0$）: 押した点から `system::newton_onto` で面へ寄せる。寄せた点が押した点から 3 画素より遠ければ、元の点のまま（別の枝へ飛んだ）
     - 媒介変数の曲線 $\mathbf r(t)$: $\lvert \mathbf r(t) - \mathbf p\rvert^2$ を最小にする $t$ を、粗い格子（256 点）で一番近い所から始めてニュートン法で詰める。札に「$t = 1.0472$」も出す
     - 媒介変数の曲面 $\mathbf r(u, v)$: 同じことを 2 変数で（格子 32 × 32 → ガウス・ニュートン法）。札に $(u, v)$
     - 計算は 1 点なので、画面のスレッドの WASM（`evalExpr.ts` が使っているもの）で `pick_onto` を呼ぶ（計算担当のスレッドへは送らない）
  2. **ドラッグの取っ手**（D5）を、計算エンジンが図形と一緒に返す（`MeshResult` の `handles`）:
     - 点の行・点の定義の行（`A=(a,b)`）で、成分が使うパラメータを調べる。1 つなら「曲線に沿って」（そのパラメータをスライダーの最小〜最大で 256 点動かした点の列を `curve` に）、
       2〜3 個で成分がパラメータの 1 次式なら「自由」（ヤコビ行列を記号微分で作り、定数か確かめる）、それ以外は取っ手なし
     - `locus` の今の点: 「曲線に沿って」。`curve` は L2 の曲線の頂点と `locus_ts`
     - 常微分方程式の初期値・測地線の始点: 欄の式が数だけのとき「欄」
  3. **ドラッグの操作**（D6）: `Viewer.ts` に取っ手の一覧を持たせ、白い縁の輪で描く。`pointerdown` が輪の 12 画素以内なら、視点の操作（OrbitControls）を止めてドラッグにする。
     `pointermove` ごとに、カーソルの下の点（2D は表示の平面、3D は `grabPlane` と同じ面）を求め、`onDragHandle(id, k, point)` で画面側に知らせる
  4. 画面側（`dragPoints.ts`）で、点 → スライダーの値に直して `setParam` に渡す:
     - 曲線に沿って: `curve` の折れ線のうち、**今の値の近くから順に**探して一番近い所の値（自分と交わる曲線で、交点の向こうの枝へ飛ばないため。今の値の前後 $\frac{1}{8}$ の範囲で 2 画素以内の点があればそこを使い、無ければ全体から）。線分の中は直線で補間
     - 自由: $\mathbf a = \mathbf a_0 + J^{+}(\mathbf p - \mathbf p_0)$（$J^{+}$ は擬似逆行列。2D で 2 つのパラメータなら $J$ の逆行列）
     - 欄: 欄の文字を、刻み（表示範囲の幅の $10^{-3}$ に丸めた数）で書き換える
  5. ドラッグしている間は粗い描画（`isCoarse` に `draggingHandle` を足す。`PLOT_NO_REFINE`・注目する点を求めない）、離したら細かく描き直す
- **実装の土台**

  ```rust
  // core/src/pick.rs
  pub enum Onto { Implicit, Curve { t: f64 }, Surface { u: f64, v: f64 } }
  /// 押した点 p（表示中の座標）を、source の図形の上へ戻す。戻せなければ None（画面側は元の点のまま）
  pub fn onto(source: &str, params: &Params, space: &Space, p: [f64; 3], tol: f64) -> Result<Option<([f64; 3], Onto)>, String> {
      // 1. equation::compile_with で Plot を作り、種類で分ける
      // 2. Implicit { prog, .. }: system::newton_onto_in(&[&prog], &[p], h, dims)。|q − p| > tol なら None
      // 3. Curve { sets, .. }: 各成分の Program で t の格子 256 点の距離 → 最小の所から
      //    t ← t − g'(t)/g''(t)（g = |r(t) − p|²。微分は中心差分で十分。1 点だけなので）を 30 回まで。範囲の外へ出たら端に止める
      // 4. Surface: 同じことを (u, v) で
  }
  #[wasm_bindgen] pub fn pick_onto(source: &str, param_names: &str, param_values: &[f64], p: &[f64]) -> Vec<f64>;
      // [x, y, z, 種類, t または u, v]。戻せなければ空

  // core/src/locus.rs（取っ手）
  pub fn handles_of(plot: &Plot, defs: &Defs, params: &Params, sliders: &[(String, f64, f64)]) -> Vec<Handle>;
  ```

  ```ts
  // web/src/lib/dragPoints.ts（画面に依らない純粋な関数。テストしやすくする）
  export type Handle =
    | { kind: 'along'; param: string; curve: Float64Array /* [値, x, y, z, …] */ }
    | { kind: 'free'; params: string[]; at: [number, number, number]; jacobian: number[] /* 3 × k */; values: number[] }
    | { kind: 'field'; which: string }
  /** カーソルの下の点 p から、新しいパラメータの値。動かせなければ null */
  export function valuesFor(h: Handle, p: [number, number, number], now: Record<string, number>, pixel: number): Record<string, number> | null
  ```

  つなぐ所:
  - `Viewer.ts`: `poiData` と同じ形の `handleData = new Map<number, Float64Array>()`、`setupPick` の `pointerdown` の前に取っ手の当たり判定を足す（当たれば `controls.enabled = false` にして、`pointerup` で戻す）。当たり判定は `pickAtScreen` の注目する点の探し方（画面の距離）を使う
  - `App.svelte`: `viewer.onDragHandle = (id, k, p) => { const v = valuesFor(…); for (…) setParam(item, v, 0) }`。`handlePick` の後半（陽関数で計算し直す所）の前に、陰関数・媒介変数なら `pick_onto` を呼ぶ
- **やらないこと**: 2 つの式の交点を注目する点にすること（別の計画）。陰関数・媒介変数の曲線の注目する点（接線が水平・垂直の点）。点の行の式そのものを書き換えるドラッグ（`A=(1,2)` の数を書き換える。式の文字を変えない、2.5）
- **完了の条件**（`core/tests/pick.rs` と `dragPoints.test.ts`）
  - 陰関数 $x^2 + y^2 = 4$ の線の近く（$(1.43, 1.41)$）を押すと、戻した点で $\lvert x^2 + y^2 - 4\rvert < 10^{-12}$、押した点からのずれが表示範囲の幅の $\frac{1}{200}$ 以内
  - 球 $x^2 + y^2 + z^2 = 4$ の面の点でも同じ
  - 媒介変数の曲線 $(\cos t, \sin 2t)$ で $(\cos 1, \sin 2)$ の近くを押すと $t = 1$（$10^{-10}$）
  - ドラッグ（`valuesFor` の単体テスト）: `A=(a,b)` の取っ手を $(1.5, -0.5)$ に動かすと $a = 1.5$、$b = -0.5$。`(a+1, 2b)` なら $a = 0.5$、$b = -0.25$。
    `P=(\cos a, \sin a)`（$a \in [0, 2\pi]$）を $(-1, 0)$ に動かすと $a = \pi$（$10^{-2}$。折れ線の補間）
  - 自分と交わる曲線 $(\sin 2a, \sin 3a)$ の取っ手を、交点（原点）を通るようにまっすぐ動かしても、$a$ が連続に変わる（1 回の動きで $\frac{1}{8}$ 周より大きく飛ばない）
  - ドラッグの 1 回の更新（`pointermove` から描き直しまで）が、L0 の 1 行目の軌跡で 50 ms 以内（デスクトップ。粗い描画）
- **壊してはいけないもの**: 視点の操作（取っ手の輪の外で押したとき）、「グラフを押したら値を出す」がオフのときの動き（取っ手のドラッグは D6 でいつも有効だが、印と札は出さない）、陽関数の注目する点
- **README**: 「点を押すと値が出る」の段落に、陰関数・媒介変数の値と、「点をドラッグする」の段落（動かせる点の 3 種類と例）
- **終わったら**: `plan-math-notation.md` の 0 章に N4 の行を足し、7.3 の N4 を「済み」にする

### L3: 条件で決まる軌跡

- **担当**: C（理由: 今の集合の書き方の読み替えを 1 つ足すだけで、正解の円が決まっている）
- **目的**: 「〜をみたす点 $P$ の軌跡」を、点の名前のまま書けるようにする
- **触るファイル**: `core/src/expand.rs`、`core/src/logic.rs`、`web/src/lib/latex.ts`（1252 行の集合の変換）
- **やること**: 集合の書き方 `\{P \mid |P-A| = 2|P-B|\}` で、$P$ を表示中の座標の点 $(x, y)$（3D なら $(x, y, z)$）に読み替える（今は `\{(x,y) \mid …\}` の形。点の名前 1 文字だけの形を足す）。`|P-A|` は L1 の長さ。角の条件 `\angle APB = \frac{\pi}{2}` は、内積で `(A-P)\cdot(B-P) = 0` に落とす（ただし $P \ne A, B$ を除く所として出す。L5 の後）
- **実装の土台**

  ```rust
  // logic.rs の parse_formula（⦃点∣条件⦄ を読む所）で、点が 1 つの名前（Var）で、その名前が定義にも座標にも無いときだけ:
  //   名前 P を、表示中の座標の組 (x, y)（3D は (x, y, z)）に置き換えた条件を作る（Expr::substitute で Var("P") → Tuple(座標)）。
  //   P が定義済みの点の名前なら、今までどおり（定義の点が条件をみたすか）にし、置き換えない
  // ∠APB は latex.ts で @angle(A, P, B) に写し、expand.rs で acos((A−P)·(B−P) / (|A−P||B−P|)) にする。
  //   「= π/2」の等式だけは、割り算の無い (A−P)·(B−P) = 0 に直す（P = A・B で 0/0 にならないように）
  ```

- **完了の条件**: L0 の 6 行目（アポロニウスの円）が描ける。`\{P \mid \angle APB = \frac{\pi}{2}\}`（$A(-1,0)$、$B(1,0)$）が単位円になる
- **README**: 「軌跡」の段落に足す

### L4: 媒介変数を消して、軌跡の方程式を出す

- **担当**: A（理由: 消去・因数分解・余分な因子の捨て方を間違えても、それらしい式が出てしまい、見た目では気づきにくい）
- **目的**: 軌跡の方程式を、正確な多項式で出す
- **触るファイル**: `core/src/bigint.rs`（新規。D1）、`core/src/groebner.rs`（多倍長の係数・変数の数を 4 まで）、`core/src/eliminate.rs`（新規）、`core/src/lib.rs`（ビット 4096）、`web/src/lib/types.ts`・`plot.worker.ts`・`ExpressionItem.svelte`（式の下の表示と ⚙ の「方程式を求める」）
- **やること**
  1. D1 の多倍長の有理数を入れ、`Rat` を置き換える（今の交点の計算も同じものを使う。あふれて諦める場合が減る）
  2. $x - X(t) = 0$、$y - Y(t) = 0$（3D なら $z$ も）を、有理数係数の多項式にする。三角関数は $s = \tan\frac t2$ で $\cos t = \frac{1-s^2}{1+s^2}$、$\sin t = \frac{2s}{1+s^2}$ に置き換えて分母を払う（$\cos 2t$ などは倍角の公式で先に $\cos t$・$\sin t$ の多項式にする）。$e^t$ などが残る式は諦める
  3. $t$（または $s$）を先頭にした辞書式順序で基底を求め、$t$ を含まない元を取り出す（消去定理）。2 つ以上あれば、平面の曲線では最大公約数、空間の曲線では全部を連立にする
  4. 結果を因数分解（有理数の範囲。重根の部分を除く）し、軌跡に乗らない因子（曲線の点を代入して 0 にならない因子）を捨てる
  5. スライダーの値は、そのときの値（有限桁の小数）を分数にして使う。式の下に「軌跡の方程式: $4x^2 - 8x + 4y^2 + 3 = 0$」と、平方完成できる 2 次式なら「$(x-1)^2 + y^2 = \frac14$」も出す
  6. ⚙ の「方程式を求める」（既定はオン）。ビット 4096 は、これがオンで、止まっているときだけ立てる（動かしている間は求めない。`PLOT_POI` と同じ考え）
- **実装の土台**

  ```rust
  // core/src/bigint.rs（D1。依存を増やさない）
  #[derive(Clone, PartialEq, Eq)] pub struct BigInt { neg: bool, mag: Vec<u32> /* 下の桁から。0 は空 */ }
  impl BigInt { pub fn from_i128(v: i128) -> Self; pub fn add(&self, o: &Self) -> Self; pub fn sub(..); pub fn mul(..);
               pub fn divrem(&self, o: &Self) -> (Self, Self) /* 筆算（クヌースの D） */; pub fn gcd(..) /* ユークリッド */; pub fn to_f64(&self) -> f64; }
  #[derive(Clone, PartialEq, Eq)] pub struct BigRat { n: BigInt, d: BigInt /* d > 0、既約 */ }
  // groebner.rs: Rat の演算（add・mul・div・is_zero・from_f64 の連分数）を BigRat で同じ名前に実装し、型だけ置き換える。
  //   Mono を [u16; 4] にし（t, x, y, z）、lex を「消したい変数が先頭」の順にする。今の solve は 4 つ目を 0 のまま使う
  //   計算の打ち切り: 係数の桁数（mag の長さ）の合計と、経った時間（timing の時計）で、1 秒を超えたら None

  // core/src/eliminate.rs
  pub fn eliminate(coords: &[Expr], var: &str, params: &Params, budget_ms: f64) -> Result<Option<Equation>, String> {
      // 1. trig_to_rational: 式の木の中の cos(k t)・sin(k t) を、倍角の公式で cos t・sin t の多項式にし、
      //    さらに cos t → (1−s²)/(1+s²)、sin t → 2s/(1+s²)。t がほかの形（e^t・t そのものと三角関数の混在）で残れば Ok(None)
      //    （t そのものだけ・多項式だけなら置き換えない）
      // 2. 各成分 X_k(s) = P_k(s)/Q_k(s) にして、多項式 Q_k(s)·x_k − P_k(s) と、分母が 0 にならない条件 w·Π Q_k − 1（ラビノビッチの変数 w。
      //    変数が 5 つになるときは、分母が全部同じなら 1 つだけ）を作る
      // 3. groebner(lex: w > s > x > y (> z)) → s・w を含まない元を集める。平面なら最大公約数を 1 つの多項式に
      // 4. 因数分解: 有理数の範囲で、無平方分解（f / gcd(f, ∂f)）と、1 変数に代入した因数を持ち上げる簡単な方法。
      //    曲線の点（t の格子 64 点で計算した (x, y)）を代入して |値| が 1e−8（正規化した係数で）を超える因子を捨てる
      // 5. latex: 項を次数の高い順、x の次数の高い順に。2 次式なら平方完成を試す（x²・y² の係数が等しい・xy の項が無い）
  }
  ```

  つなぐ所: `lib.rs` の `plot` の中、`build` の直後で `if flags & FLAG_LOCUS_EQUATION != 0 { if let Plot::Locus { .. } = &plot { … } }`。結果は `MeshResult` の `locus_equation`。
  `bench.rs` に、L0 の 1 行目と 5 行目の消去の時間を 1 行ずつ足す
- **やらないこと**: 媒介変数が 2 つ以上（曲面の軌跡）。消去に 1 秒以上かかる式（打ち切って「方程式は求められません」）
- **完了の条件**: L0 の方程式の列が、L3 の条件の行を除いて全部一致（定数倍を除いて係数が同じ）。どの式も 200 ms 以内（`bench.rs` に 1 行）
- **壊してはいけないもの**: 今の交点の計算（`groebner::solve`）の結果と時間（`bench.rs` の交点の行が 1.2 倍を超えて遅くならない）
- **README**: 「軌跡」の段落に「方程式」。多項式に直せる式の範囲を書く

### L5: 除く所の判定

- **担当**: A（理由: 数値で点ごとに調べるので、短い除外部分・端の点の見落としが起こりやすく、正しいかを見た目で判断できない）
- **目的**: 方程式の図形のうち、範囲の制限で届かない所を出す
- **触るファイル**: `core/src/eliminate.rs`、`core/src/lib.rs`（`locus_excluded`）、描画（`Viewer.ts` の白抜きの丸・点線）
- **やること**
  1. L4 の方程式の曲線を、今の 2D の陰関数の道で点の列にする
  2. 曲線の点 $\mathbf p$ ごとに、$X(t) = p_x$、$Y(t) = p_y$ をみたす実数の $t$ が範囲の中にあるかを調べる（L4 の基底で $t$ の多項式にし、`groebner.rs` の実根の求め方で根を出す）
  3. 届く・届かないが変わる所を二分法で詰め、届かない部分を「部分」（点線）と「孤立した点」（白抜きの丸）に分ける。$s = \tan\frac t2$ で抜ける $t = \pi$ の点は、別に代入して調べる
  4. 文章にする: 孤立した点は「ただし点 $(-1, 0)$ を除く」、部分は、座標 1 つの不等式で書けるなら「ただし $x \ge 0$ の部分」、書けなければ「点線の部分を除く」
- **実装の土台**

  ```rust
  pub fn excluded(eq: &Equation, coords: &[Program] /* X(t), Y(t) */, var_basis: &[BigPoly] /* L4 の基底（t を含む元） */,
                  range: (f64, f64), open: (bool, bool), dom: Domain) -> Vec<Excluded> {
      // 1. 曲線の点: eq.poly を Program にして、今の 2D の陰関数の道（mesh の 2D 曲線）で折れ線にする（点の間隔は表示範囲の 1/400）
      // 2. reach(p) -> bool: 基底の元に (x, y) = p を代入して t（または s）の 1 変数の多項式にし、実根を出す。
      //    s のときは t = 2 atan s に戻す。根のどれかが範囲の中（open なら端を除く）にあれば true。
      //    s = ∞（t = π）は別に: X(π), Y(π) が p と一致し、π が範囲の中なら true
      // 3. 折れ線の点ごとに reach。true と false が入れ替わる線分で二分法（40 回）
      // 4. false の連続した区間 → Part（点の数が 1 つだけなら Point）。孤立した点の候補は別にも出す:
      //    t → ±∞・s → ∞ の極限の点（X, Y の分子と分母の次数から）と、L4 で捨てなかった因子の孤立した実点。それぞれ reach で確かめる
      // 5. 文章: Part の点が全部 x ≥ c（または ≤・y について）をみたし、残りがみたさないなら「x ≥ c の部分」。c は端の点の座標
  }
  ```

  開区間: 範囲の欄は `\le` だけなので、`\operatorname{locus}(M, t, 0, \pi)` の後ろに `open` の印を書ける形（`\operatorname{locus}(M, t, 0 < t < \pi)`）を足す
- **完了の条件**: L0 の「除く所」の列が全部一致
- **危ない所**: 端の点が含まれるかどうか（開区間・閉区間）は、範囲の欄の `<` と `\le` から決める。範囲の欄が `\le` しか書けないなら、`locus` の引数で開区間を書けるようにする
- **README**: 「軌跡」の段落に「除く所」

### L6: 包絡線と、通過領域の境目

- **担当**: B（理由: 消去（L4）と今の $\exists$ の上に乗る組み合わせで、新しいアルゴリズムは境目の和だけ）
- **目的**: 直線や曲線の族が動いてできる包絡線と通過領域を、正確な境目で描く
- **触るファイル**: `core/src/equation.rs`（`quantified`・`envelope_line`）、`core/src/eliminate.rs`
- **やること**
  1. `\operatorname{envelope}(F, a)`（$F(x, y, a) = 0$ の族）: $F = 0$ と $\partial_a F = 0$（M3 の記号微分）から $a$ を消す（L4）。消せない式は、2 つの等式の連立として $(x, y, a)$ の空間の曲線を描いて、$(x, y)$ に射影する
  2. `\exists a \in [a_0, a_1],\ F(x,y,a) = 0` の領域（通過領域）の境目は、包絡線のうち $a$ が範囲に入る部分と、端の曲線 $F(x, y, a_0) = 0$・$F(x, y, a_1) = 0$ の和に含まれる。境目をこの和から作り、塗る所は今の近似（32 等分）を、境目で分けた小さな領域ごとに 1 点で判定し直す
- **実装の土台**

  ```rust
  // equation.rs の行の振り分けに envelope_line（@envelope(F, a) だけの行）を足す
  fn envelope_line(source: &str, params: &Params, space: &Space) -> Result<Option<Plot>, String> {
      // F と Fa = diff::diff(F, a) を作り、eliminate::eliminate_system(&[F, Fa], a, params, 1000.0) で a を消す
      //   （L4 の eliminate の「x − X(t)」の代わりに、多項式の等式をそのまま渡す版）
      // 消せたら Plot::Implicit（2D の曲線）。消せなければ (x, y, a) の 2 つの等式の連立（今の連立の道）を描き、a の座標を落として射影
  }
  // quantified（1732 行）の、中身が等式で範囲が [a0, a1] の場合に:
  //   boundary = 包絡線の a が範囲に入る部分（L5 の reach と同じく、点ごとに a を解いて範囲を見る）∪ F(x, y, a0) = 0 ∪ F(x, y, a1) = 0
  //   境目の曲線を描き、塗りは、境目で区切られた画素のかたまり（2D の格子の連結成分）ごとに代表の 1 点で ∃ を判定し直す
  ```

- **完了の条件**: L0 の 7・8 行目。通過領域の境目が、$a$ を $10^4$ 等分して重ねた図の境目と $10^{-6}$ 以内
- **README**: 「$\exists$」の段落の「近似が混ざる」を、多項式の族では正確になると書き直す

### L7: 交点で決まる点の枝の追跡

- **担当**: A（理由: 枝の取り違え・合流点の扱いを間違えると、つながり方の違う曲線が見た目にはそれらしく描けてしまう）
- **目的**: 「$P$ を通る直線が放物線と交わる点 $Q$」のように、値が 2 つ以上ある点の軌跡を、枝ごとにつながった曲線で描く
- **前提**: 文書 1 の R0 で N8 を続けると決まり、R5 の `branch::continue_along` が使えること
- **やること**: `\operatorname{meet}` に円・2 次曲線を足し、$t$ を細かく動かしながら、前の $t$ の値に一番近い解を選んで枝をつなぐ。枝が合流する所（重解）では分けて描く
- **実装の土台**

  ```rust
  // 円と直線の交点は、2 次方程式の解の集合 {q₊(t), q₋(t)}。これを branch.rs の ValueSet として渡す:
  struct MeetSet { a: Vec<Program>, b: Vec<Program>, c: Vec<Program> /* 2 次方程式の係数（t の式） */ }
  impl ValueSet for MeetSet { fn values(&self, t: C) -> Vec<C>; } // 判別式が負の所は空（実の交点が無い）
  // 曲線は、実の t の格子の上で continue_along を呼び、値の集合の大きさが 2 → 0 に変わる所（判別式の零点）で枝を閉じる。
  // 判別式の零点では 2 つの枝が出会うので、そこで 2 本の折れ線を 1 本につなぐ（円の「t = ±1」の所）
  ```

- **完了の条件**: 単位円と直線 $y = t$ の交点 2 つが、$t = \pm 1$ でつながった 1 つの円として描ける

### Z0: 複素積分の物差し

- **担当**: C（理由: 正解は教科書で決まっている表を、テストの形に写すだけ）
- **触るファイル**: `core/tests/complex_integral.rs`（新規）
- **やること**: 次の表をテストにする（初めは `#[ignore]`）

  | 積分 | 正解 |
  |---|---|
  | $\oint_{\lvert z\rvert=1} \frac{dz}{z}$ | $2\pi i$ |
  | $\oint_{\lvert z\rvert=2} \frac{e^z}{z-1}\,dz$ | $2\pi i e$ |
  | $\oint_{\lvert z\rvert=1} \bar z\,dz$ | $2\pi i$（正則でない例） |
  | $\int_0^{1+i} z^2\,dz$（線分） | $\frac{(1+i)^3}{3}$ |
  | 折れ線 $0 \to 1 \to 1+i$ の $\int z^2\,dz$ | 同じ（経路によらない） |
  | $\oint_{\lvert z\rvert=1} \frac{dz}{z-0.999}$ | $2\pi i$（極が経路に近い） |
  | $\oint_{\lvert z\rvert=1} \frac{dz}{z-1}$ | エラー（経路の上の極） |
  | $\oint_{\lvert z\rvert=4} \frac{dz}{\sin z}$ | $-2\pi i$（中の極 $0, \pm\pi$ の留数 $1, -1, -1$ の和が $-1$） |
  | $\operatorname{Res}_{z=i} \frac{1}{1+z^2}$ | $-\frac i2$ |
  | $\operatorname{Res}_{z=0} e^{1/z}$ | $1$（真性特異点） |

  誤差の目安は $10^{-10}$（経路に近い極は $10^{-8}$）
- **実装の土台**

  ```rust
  // 値は、展開してから Program で計算した複素数で比べる（1.2 の確かめと同じ道。エラーの中身も Result<_, String> で読める）
  fn cval(src: &str, defs: &str) -> Result<C, String> {
      let _d = graph_core::expand::install(graph_core::expand::Defs::parse(defs, &Default::default()));
      let e = graph_core::expand::expand(&graph_core::parser::parse(src).map_err(|e| e.to_string())?, &Default::default())?;
      let p = graph_core::eval::Program::compile_with(&e, &[], &Default::default())?;
      let mut o = [C::new(0.0, 0.0)];
      p.eval_batch(&[], &mut o);
      Ok(o[0])
  }
  // 表の行は、LaTeX の形（`\oint_{|z|=1}\frac{1}{z}\,dz`）を latex.ts が写した計算エンジンの文字列で書く。
  // 写し方が決まるのは Z1 なので、L0 と同じく #[ignore] を付けて置き、Z1 で文字列を確かめてから外す。
  // 今でも書ける「手で媒介変数にした形」（@int(s,0,2*pi)(…)）の行を、各行の隣に 1 つずつ置く（今の値を守るため。1・2 行目は今も通る）
  ```

- **完了の条件**: 表がテストになっている。手で媒介変数にした形の 1・2 行目は、今の状態で通る

### Z1: $dz$ と複素平面の経路の書き方

- **担当**: B（理由: 今の `@intset` の展開と記号微分に乗せる。新しいのは経路の読み取りと、$z$ を束縛変数にする判定）
- **目的**: `\oint_C f(z)\,dz` を、そのまま書いて計算できるようにする
- **前提**: 文書 2 の V0・V1（複素数の値の表示。D7）
- **触るファイル**: `core/src/contour.rs`（新規）、`core/src/expand.rs`（`region_integral` の振り分け）、`web/src/lib/latex.ts`（`integralKinds` の分類の表示）、`web/src/App.svelte`（`integralValues` を V0 の値にする）
- **やること**
  1. `@intset(…; {d_z})` で、次のどれかなら複素積分にする:
     - 経路が複素数の式 1 つと媒介変数の範囲（`\{e^{it} \mid 0 \le t \le 2\pi\}`）→ $\gamma(t)$
     - 経路が円の条件 `|z-a| = r` → $\gamma(t) = a + r e^{it}$、$0 \le t \le 2\pi$（反時計回り）
     - 定積分の端が複素数 `\int_{a}^{b} f(z)\,dz` → 線分 $\gamma(t) = a + t(b - a)$
     - 折れ線 `\int_{[a, b, c]}`（点の並び）→ 線分の和
     - 名前で定義した経路 `\gamma(t) = …` を `\oint_{\gamma}` で使う
  2. 展開: $\int f(\gamma(t))\,\gamma'(t)\,dt$。$\gamma'$ は M3 の記号微分。被積分関数の $z$ を $\gamma(t)$ に置き換える（座標の $z$ ではない。`\int … dx` の $x$ と同じく束縛変数）。`\bar z`・`\operatorname{Re} z`・`|z|` もそのまま置き換える
  3. `|dz|`（弧長）は $\lvert\gamma'(t)\rvert\,dt$
  4. 分類の表示: 「複素積分（経路: 円 $\lvert z\rvert = 1$、反時計回り）」
- **実装の土台**

  ```rust
  // core/src/contour.rs
  pub fn read_path(vars_e: &Expr, chains: &[Expr], params: &Params) -> Result<Option<Contour>, String> {
      // 振り分け（どれにも当たらなければ Ok(None) で今までの道）:
      //   (a) vars_e が Var("z") で、chains が 1 つの等式 abs(z − a) = r → Circle。a・r は定数かパラメータの式（Program で値を確かめ、r > 0）
      //   (b) vars_e が z 以外の式（e^(i t)）で、chains が媒介変数の範囲 1 つ → Param
      //   (c) 点の並び（Expr::List）→ 折れ線
      //   (d) 名前 → 定義の関数 γ(t) を引く。範囲は定義の範囲か、媒介変数の範囲の欄
      // 各片で dgamma = diff::diff(&gamma, var)
  }
  pub fn integrand(c: &Contour, body: &Expr, arc: bool /* |dz| */) -> Vec<(Expr /* f(γ(t))·γ'(t) */, String, f64, f64)> {
      // body の Var("z") → gamma、conj(z)・re(z)・abs(z) はそのまま置き換えれば式の意味が合う。
      // arc なら γ' の代わりに abs(γ')
  }
  ```

  つなぐ所: `expand.rs` の `region_integral`（3371 行）の先頭、`diffs` に `"z"` があり（`{d_z}`）、`contour::read_path` が `Some` を返したら、
  片ごとに `Expr::Binder { op: "int", var: t, args: [a, b], body: integrand }` を作って足し、それを `self.integral` に渡す（今の 1 重の積分の道）。
  `\int_a^b f(z)\,dz` の端が複素数のときは、`integral`（2176 行）の中で端の値が実数でないことを見て、線分の経路にする。
  **座標の $z$ との区別**: 経路が上の (a)〜(d) のどれかに当たったときだけ、$z$ を束縛変数にする。`\iiint … dz` の今の形は、`diffs` に `x`・`y` も並ぶので当たらない
- **やらないこと**: 経路を等式の曲線（`x^2+y^2=1`）で書く形の複素数版（円は上の `|z-a|=r` で足りる。ほかの形は需要を見て）
- **完了の条件**: Z0 の表のうち、極が経路に近い・上にある行と留数の行を除いて通る。値が画面に複素数で出る（V1 の書式）
- **壊してはいけないもの**: 3 次元の座標 $z$ を使う今の積分（`\iiint … dz` など。`d_z` の意味は、経路が複素数のときだけ変える）。`core/tests/notation.rs` の積分の行
- **README**: 積分の段落に「複素積分」

### Z2: 経路の近く・上の特異点

- **担当**: A（理由: 特異点の探し方と、経路の上か近くかの判定の閾値を間違えると、正しくない値を黙って返す）
- **目的**: 極が経路に近くても正しく、経路の上ならエラーにする
- **触るファイル**: `core/src/contour.rs`、`core/src/quad.rs`（複素数の版）、`core/src/expand.rs`（`closed_integral_value` の複素数の道）
- **やること**
  1. まず NaN の原因を確かめる（1.2 の調べでは、値が複素数の中身は `as_real` で NaN になり、適応型の求積を通らずに決まった点の数の `integral_checked` に落ちている。そのとおりか確かめる）
  2. 値が複素数の 1 重の定積分にも、適応型の求積の道を作る: 実部と虚部を同時に積む `quad::integrate_split_c`（分割は 2 つの誤差の大きい方で決める）
  3. 被積分関数の特異点を、複素平面で探す（下の土台）。経路からの距離が小さければ、その点に一番近い $t_0$ で区間を分け（`integrate_split_c` の `cuts`）、端に点を集める
  4. 距離がほぼ 0（経路の上）ならエラー「経路の上に特異点があります（$z \approx 1$）」
  5. 特異点を探せない式（特殊関数・場合分け）は、今の適応型の求積のまま。値が有限でなければエラー「経路の近くに特異点がある可能性があります」
- **実装の土台**

  ```rust
  // quad.rs
  pub fn integrate_split_c(f: &mut dyn FnMut(&[f64], &mut [C]), a: f64, b: f64, cuts: &[f64], tol: Tol, budget: &Budget) -> Result<C, Trouble>;
  // integrate_split と同じ形。区間ごとの誤差は max(|K−G| の実部, 虚部)。今の integrate_split を、値の型だけ違う共通の中身にしてもよい

  // contour.rs
  /// f（z の式）の特異点のうち、経路の近く（表示範囲の幅の 1/10 以内）にあるもの
  pub fn near_singularities(f: &Expr, c: &Contour, params: &Params) -> Vec<C> {
      // 1. z を x + i·y に置き換えた式の 1/f を Program（変数 x, y）にする
      // 2. 種: 経路の上の 512 点のうち |f| が両隣より大きい点と、そこから法線の向きに ±(経路の長さの 1/64) ずらした点
      // 3. 各種から、g = 1/f の零点を複素数のニュートン法で: z ← z − g(z) / g'(z)、g' は (g(z + h) − g(z)) / h（h = 1e−7 |z| + 1e−9。
      //    正則なら向きによらない）。30 回まで。|g| < 1e−12 になったら採用
      // 4. 近い点（1e−9）はまとめる
  }
  /// 点 s に一番近い経路の点の (片の番号, t, 距離)
  pub fn closest(c: &Contour, s: C) -> (usize, f64, f64);
  // 距離 < 1e−10 × 経路の長さ → エラー（経路の上）。それ以外 → その t を cuts に入れる
  ```

  つなぐ所: `closed_integral_value`（2452 行）の中、`Program::compile_with(&g, …)` の後で中身が複素数になる（`as_real` が NaN の点がある）とき、
  `integrate_split_c` の道へ分ける。Z1 から来た積分なら、`contour::near_singularities` の結果の $t$ を `found.points` に足す
- **完了の条件**: Z0 の 6・7 行目。極を $10^{-1}$ から $10^{-6}$ まで経路に近づけて（`1/(z - (1 - 10^{-k}))`）、どれも $2\pi i$ と $10^{-8}$ 以内
- **壊してはいけないもの**: 実数の定積分の値と時間（`bench.rs`。中身が実数なら今の `integrate_split` のまま）

### Z3: 留数と回転数

- **担当**: B（理由: 留数は円周の台形則という確実な方法があり、Z2 の特異点探しを使うだけ。正確な値の道は確かめに使う）
- **触るファイル**: `core/src/residue.rs`（新規）、`core/src/contour.rs`（回転数）、`core/src/expand.rs`、`latex.ts`
- **やること**
  1. `\operatorname{Res}_{z=a} f`: $\frac{1}{2\pi i}\oint_{\lvert z-a\rvert=\rho} f\,dz$。$\rho$ は、ほかの特異点までの距離の半分より小さく取る（Z2 の零点探しで近くの特異点を探す。見つからなければ $\rho = 10^{-2}$）。有理関数なら、極の位数を数えて正確な値を出す（ローラン展開の係数。`series.rs` を使えるか確かめる）
  2. 回転数 `\operatorname{ind}_{\gamma}(a)`（$\frac{1}{2\pi i}\oint_\gamma \frac{dz}{z-a}$）
  3. 閉じた経路の複素積分の下に、経路の中の特異点・留数・回転数の表と、留数定理の値 $2\pi i \sum_k \operatorname{ind}_\gamma(a_k) \operatorname{Res}_{z=a_k} f$ を出す（積分の値と並べる）
- **実装の土台**

  ```rust
  // core/src/residue.rs
  pub fn residue(f: &Expr, a: C, params: &Params) -> Result<C, String> {
      // 1. ρ: contour::near_singularities を a の周り（半径 1 の円を経路にして）で探し、a 以外で一番近いものまでの距離の 1/2（無ければ 1e−2）
      // 2. 円周の台形則: Res ≈ (ρ/N) Σ_k f(a + ρ e^{iθ_k}) e^{iθ_k}、θ_k = 2πk/N。周期関数なので指数的に収束する。
      //    N = 64 から倍にしていき、続く 2 回の差が 1e−12 (1 + |値|) 以下で止める（N は 2^16 まで。届かなければ Err）
      //    真性特異点（e^{1/z}）もこの方法で同じく求まる
      // 3. 確かめ（有理関数のとき）: 分母の a での零点の位数 m を数え、(1/(m−1)!) d^{m−1}/dz^{m−1}[(z−a)^m f] の a での値を
      //    series::taylor（中心が複素数でよい）で求める。2 と 1e−10 以内で合わなければ、2 の値を採って説明に「数値の値」と書く
  }
  // contour.rs
  pub fn winding(c: &Contour, a: C) -> Result<i64, String> {
      // 経路を細かく取った点 γ_k について arg(γ_{k+1} − a) − arg(γ_k − a) を (−π, π] に直して足し、2π で割って丸める。
      // 丸める前の値が整数から 1e−6 より離れれば、点を倍にしてやり直す。a が経路の上（距離 1e−10 以下）なら Err
  }
  ```

  つなぐ所: `expand.rs` の演算子の分岐に `"res"`（`@res(z, a)(f)`）と `"ind"`（`@ind(γ, a)`）。値は複素数の数（`Expr` の複素数の定数）にして返す。
  積分の下の表は、Z1 の積分の行の `MeshResult` に `residues: Vec<f64>`（`[Re a, Im a, Re Res, Im Res, ind, …]`）を足し、`ExpressionItem.svelte` で表にする
- **完了の条件**: Z0 の留数の行と 8 行目
- **README**: 「留数」の段落

### Z4: 描き方

- **担当**: B（理由: 今の複素数の色分け・曲線・L2 の残像を組み合わせる画面側の仕事）
- **触るファイル**: `web/src/lib/Viewer.ts`、`latex.ts` の `integralDisplay`、`web/src/lib/samples.ts`、`core/src/lib.rs`（`contour`）
- **やること**
  1. 経路を、複素平面（横が実部・縦が虚部）に向きの矢印付きで描く。下に被積分関数の定義域の色分け（今の複素数の色分け）を薄く敷く
  2. 特異点に印を付け、留数を文字で出す
  3. 積分の進み具合のスライダー（今の ▶）で、途中までの積分 $\int_a^s f(\gamma(t))\,\gamma'(t)\,dt$ の値を、複素平面の点として動かし、その軌跡を描く（L2 の残像の仕組みを使う）。閉じた経路なら、終点が $2\pi i \sum \operatorname{Res}$ に着くのが見える
  4. サンプル: 経路を半径のスライダーで広げ、極をまたぐ所で値が飛ぶ（コーシーの定理）。実積分 $\int_{-\infty}^{\infty} \frac{dx}{1+x^2} = \pi$ を半円の経路で求める例
- **実装の土台**
  - `integralDisplay`（2390 行）の今の「進み具合 `progress`」を、複素積分では「経路の何割まで」に使う。途中までの積分の値は、経路を 256 個に分けた区間ごとの積分を足していった列を、計算エンジンで一度に作る
    （`MeshResult` の `contour` に、経路の点の列・向きの矢印の位置・特異点と留数・途中までの値の列 `[Re, Im] × 257` をまとめる）
  - 途中までの値の点は、L2 の `recordParamTrail` で残像にする（▶ のときだけ）
  - 色分けの下敷きは、被積分関数を `z = x + iy` の陽関数として、今の複素数の色分け（`complex_mode` の 4: arg）で薄く描く（別の式の行を作らず、同じ結果の中の 2 つ目の図形）
- **完了の条件**: サンプル 2 つが描ける。途中までの積分の値の終点が、積分の値と一致（$10^{-8}$）
- **README**: 「複素積分」の段落に描き方

### Z5: 多価関数を経路に沿って続ける

- **担当**: A（理由: 解析接続の枝の取り違えは、値がそれらしいまま間違う）
- **前提**: 文書 1 の R5（`branch::continue_along`）
- **やること**: 被積分関数に $\sqrt{\ }$・$\log$・累乗がある複素積分では、主値で計算すると枝の切れ目で値が飛ぶ。経路に沿って、となりの点の値に一番近い枝を選ぶ（解析接続）。$\oint_{\lvert z\rvert=1} \sqrt z\,dz$ は、出発の枝を決めれば $-\frac43$（1 周で別の枝に移る）
- **実装の土台**

  ```rust
  // 1. 被積分関数の木に多価の関数があるか（文書 1 の R1 の branch::enumerate で枝が 2 つ以上になるか）。無ければ今の道
  // 2. 経路を細かく（4096 点。求積の点もこの中から取れるように、ガウス・ルジャンドルの点を区間ごとに）取り、
  //    出発点の値は主値、そこから continue_along(&枝の集合, &経路の点, 主値) で続けた値の列を作る
  // 3. 求積は、続けた値の列を点の値として使う（式を Program にして毎回計算するのではなく、点の値の表から和を作る）。
  //    説明に「√ を経路に沿って続けました（出発は主値）」
  ```

- **完了の条件**: 上の値が $10^{-8}$ 以内。経路が枝の切れ目を通らない式の結果は、今と同じ

## 6. 危ない所

- **L4 の消去の時間と大きさ**: 辞書式順序のグレブナー基底は、式によって係数と項の数が爆発する。1 秒で打ち切り、図だけ出す。三角関数の置き換えで次数が 2 倍になるので、$\cos 3t$ のような式は特に重い
- **L4 の `Rat` の置き換えで交点の計算が遅くなる**: 多倍長は `i128` より遅い。係数が小さいうちは `i128` のまま計算し、あふれたときだけ多倍長に移す形（列挙型で 2 つを持つ）も考える。`bench.rs` の交点の行で決める
- **L5 の判定の正しさ**: 数値で点ごとに調べるので、とても短い除外部分は見落とす。孤立した点は、L4 の因数と $t = \pm\infty$・$s = \infty$ の極限から候補を出して、別に確かめる
- **Z1 の $z$ の意味**: 座標の $z$ と積分の変数の $z$ が同じ文字。経路が複素数のときだけ束縛変数にする判定を間違えると、今の 3 重積分が壊れる。テスト（`notation.rs` の積分の行）を必ず通す
- **Z2 の零点探し**: 分母が初等関数でない（特殊関数・場合分け）と零点が探せない。そのときは今の適応型の求積のまま、値が有限でなければエラー「経路の近くに特異点がある可能性があります」
- **N4 のドラッグと視点の操作の取り合い**: 取っ手の輪の当たり判定が広すぎると、グラフを回そうとして点を動かしてしまう。12 画素（タッチは 20 画素）で始め、スマートフォンで試してから決める。2 本指の操作は、取っ手の上で始まっても視点の操作にする
- **N4 の GPU で描いた曲線**: 画素ごと（G4・G5）で描いた 2D の陰関数の曲線・3D の面は、メッシュの線・三角形が隠れているか無いことがある。押した点の探し方（`pickAtScreen` は `curvePts`・三角形を見る）で拾えるかを確かめ、拾えなければ、`pick_onto` を押した画面の点の近く（視線の上）から始める道を足す
- **L7・Z5 は N8 次第**: 枝の追跡が破綻するなら（文書 1 の R0 の判断）、どちらもやらない

## 7. ほかの計画との関係

| 計画 | 関係 |
|---|---|
| N8（リーマン面）＝ 文書 1（`plan-riemann-topology.md`） | L7・Z5 と同じ「枝の追跡」。R0 の試作の結果で、この 2 つをやるか決める。やるなら R5 の `branch::continue_along`・`ValueSet` を使う |
| N4（点を押すと値が出る、`plan-math-notation.md` 7.3） | 前半（押すと値・陽関数の注目する点・`\operatorname{crit}`）は済み（1.3）。残りはこの文書の N4 ステップ（L2 の今の点のドラッグと一緒） |
| N3（測地線）・M9（常微分方程式） | N4 のドラッグの「欄」（D5 の (3)）で、始点・初期値をグラフの上で動かす |
| 文書 2（`plan-analysis.md`）の V0・V1 | Z1・Z3 の複素数の値と書式（D7） |
| 文書 5（`plan-performance.md`）の Q4 | カーソルの下の点を GPU の深度で求める案。N4 の押した点・取っ手の当たり判定が重くなったら、同じ道を使う |
| 数値から閉じた式を推定する案（未計画） | L4 は推定ではなく正確な消去。推定を入れるなら、Z3 の留数の値の表示に使える |
| M3（記号微分） | Z1 の $\gamma'$、L6 の $\partial_a F$、N4 の取っ手のヤコビ行列 |
| 12 章（画素ごとの GPU） | Z4 の定義域の色分けは、今の色分けを使う。N4 の押した点は、GPU で描いた図形でも拾えるか確かめる（6 章） |
| `flags` のビット | この文書は 12（4096。L4・L5 の方程式と除く所）を使う。N4 は今のビット 4（`PLOT_POI`）の考え方（止まっているときだけ）に合わせ、取っ手はビットを使わない |

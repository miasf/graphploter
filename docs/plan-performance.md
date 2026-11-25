# 計画: 性能の改善（測って決める）

文書 5（`docs/plan-roadmap.md`）。`plan-render-algorithms.md` 0.2 の大化け候補のうち、性能に関わる Q1〜Q7・Q10 と、描き方の「自動」の判定（`plan-auto-render.md`）、
特殊関数の精度の検証 N10（`plan-math-notation.md` 7.3）を、1 ステップずつ（1 ステップ = 1 PR）の指示書にする。
ステップの名前は、Q の番号をそのまま使い（**Q1〜Q7・Q10**）、Q に無い追加のステップは **O0〜O5**、精度の検証は **N10** のまま。
各ステップの「担当」は `docs/agent-tiers.md` の段階。各ステップの「実装の土台」は、関数の形・データの形・処理の流れの下書き。
**土台は出発点で、命令ではない**。コードを読んで合わない所があれば直してよく、そのときは 0 章の表に「計画との違い」を書く。

**この文書の芯**: どのステップも `plan-render-algorithms.md` 0.2 の「小さく試して測り、基準を超えたら採用する」形。書く前に測った結果（1.2）で、次の 2 つが分かった。

- **ふつうの式（初等関数だけ）の描画は、式の計算が全体の 6〜30 %**。残りはメッシュを作る段（1.2 の表 3）。命令列を速くする Q6（SIMD）・Q5（JIT）は、
  式の計算を何倍にしても、全体は最大でその割合しか縮まない。だから Q6 は「ほぼ手間なし」（ビルドの設定だけで 1.5 倍前後。1.2 の表 1）なのでやり、
  Q5 は全体の時間で判断する（D5）
- **特殊関数の式は、式の計算が全体の 79〜97 %**（`hypU` の曲面で 533 ms / 546 ms）。1 回の評価が 1〜13 µs と重い（表 2）。ここは Q1（代理）が効く。
  区間ごとのチェビシェフ補間（次数 16）で、誤差は値の最大の $10^{-14}$ 程度・評価は数十倍〜数百倍速い見込み（表 4）

## 0. 進み具合（実装の記録）

| ステップ | 状態 | メモ（計画との違い） |
|---|---|---|
| （まだ無い） | | |

## 1. 調べた結果（2026-11-25）

### 1.1 今ある部品

| 部品 | 場所 | この計画での使い道 |
|---|---|---|
| 命令列の評価（256 点ずつ。まず実数だけで計算し、だめな CHUNK だけ複素数で計算し直す） | `core/src/eval.rs` の `Code::run_inner`（235 行）・`chunk_real`（585 行）・`chunk_complex`（284 行）。`real_fn1`（147 行）が実数で計算できる関数の一覧。1 引数の重い関数は `map1_memo`（直前の 64 個の引数を覚える） | Q6（SIMD）・Q5（JIT）が速くする所。Q1 の代理を新しい命令として入れる所 |
| **特殊関数はいつも複素数の道** | `real_fn1`・`real_fn2` に特殊関数が無いので、`erf(x)` も複素数の級数で計算する（実数の $x$ で 1 回 1.4 µs） | Q1 の代理は実数の道に入れる（実数の値になる代理は `chunk_real` で計算できる） |
| 区間演算 | `Code::interval_with`（351 行）。**特殊関数は `None`（分からない）** | Q1 の代理の引数の範囲を、引数の部分の区間演算で求める（引数に特殊関数が入れ子なら諦める） |
| 特殊関数 | `core/src/special/`（`mod.rs`・`bessel.rs`・`hyper.rs`・`elliptic.rs`・`expint.rs`・`gamma_ext.rs`・`ortho.rs`）、呼び出しは `functions.rs` の `call`（372 行あたり） | Q1 の代理を作るときの「正しい値」 |
| 特殊関数の参照値のテスト | `core/tests/special_values.rs`（約 190 項目。P0-1）、`core/tests/special_accuracy.rs` と `special_accuracy_table.inc`（極端な引数の約 240 項目。mpmath 40 桁） | N10 は、これに「関数ごとに複素平面の 200 点・誤差の最大の表」を足す |
| 時間の物差し | `cargo test --release --test bench -- --ignored --nocapture`（`bench_report`。式の計算・メッシュ・粗い版・全体、陰関数の部分ごとの時間）、`?bench=1`（`web/src/lib/bench.ts`。実際の画面のコマの時間）、`?bench=1&set=samples`（サンプル全部） | O0 で WASM の時間・結果の指紋を足す |
| 描き方の自動の判定 | `web/src/lib/renderMode.ts` の `gpuByDefault`（`pointer: coarse` ならメッシュ）・`wantGpu`。`Viewer.ts` の `syncGpu3d`（2058 行）・`abortRatio`（2141 行）・`checkAborts`（2212 行。打ち切り 15 % でメッシュに）・`syncGpu2d`（2005 行）。⚙ の文は `ExpressionItem.svelte` 322 行 | O1・O2 |
| 動いている間の質 | `web/src/lib/motionQuality.ts`（段階 0〜7。画素密度と粗い版）、光線の回数の上限 `RAY_STILL`（256 回・4096）・`RAY_MOVING`（`raycastMaterial.ts` 292 行）、`Viewer.ts` の `applyRayBudget`（998 行） | Q10・O5 |
| 不等式の領域の霧 | `web/src/lib/volumeMaterial.ts` の `uRegion` の分かれ（テクスチャの値は境目までの距離の見積もり $G / \lVert\nabla G\rVert$）。歩数は格子の 1 辺の 2.5 倍（96〜256）・決まった歩幅。`Viewer.ts` 2668 行が歩数と、領域を囲む箱に縮める処理 | Q7 |
| 光線の基準実装（Rust） | `core/src/pixel.rs` の `ray_hit`（105 行。区間で二分）・`classify_rect` | Q2 の試作 |
| カーソルの下の点 | `Viewer.ts` の `rayAt`（788 行。three.js の `Raycaster`、三角形の総当たり） | Q4 |
| GPU の確かめの道具 | `tools/gpu-g0/g0.html`（光線の速さ）、`tools/gpu-g3/check.cjs`（ヘッドレスの Chromium で GLSL と Rust を比べる。`npm run test:gpu`） | O2・Q7 の SwiftShader での確かめ |
| WASM の読み込み | `web/package.json` の `wasm`（`wasm-pack build ../core --target web --out-dir ../web/src/wasm-core`）、`plot.worker.ts` と `evalExpr.ts` の `import init from '../wasm-core/graph_core.js'` | Q6 の 2 つのビルドの選び方 |

### 1.2 計算エンジンで確かめた今の動き

`core/tests/` に一時的なテストを置き（終わったので消した）、ネイティブの `--release` と、同じ Rust を `wasm32-wasip1` にして Node 22（V8。Chrome と同じ）の WASI で動かして測った。
点は乱数（格子の `map1_memo` が効かない、いちばん重い場合）。WASM の行は、先に同じ式を 1 回ずつ回して温めた後の、3 回の最小。

**表 1: 命令列の 1 点の時間（ns）と、融合した場合（JIT の上限の目安）**

| 式 | 命令 | ネイティブ | WASM | WASM ＋ `simd128`（自動ベクトル化だけ） | 融合（Rust で直接書いた関数を WASM に） |
|---|---|---|---|---|---|
| `x^2+y^2+z^2-4` | 11 | 4.9 | 7.1 | 4.9 | 1.9 |
| トーラス `(sqrt(x^2+y^2)-3)^2+z^2-0.04` | 15 | 7.0 | 11.2 | 7.4 | 2.2 |
| ローマ曲面 `4(x^2 y^2 + y^2 z^2 + z^2 x^2) - 4 x y z` | 18 | 7.4 | 12.7 | 7.9 | 2.4 |
| ジャイロイド | 14 | 131 | 164 | 162 | 117 |

- **`-C target-feature=+simd128` を付けてビルドし直すだけ（コードは同じ）で、多項式の式は 1.45〜1.6 倍**。ただし温めないで測った回は差が出ない・逆転した回もあり、ぶれが大きい（O0 で測り方を固める）
- 融合した関数は、命令列の解釈より 3.7〜5.3 倍速い（SIMD ありの解釈より 2.6〜3.3 倍）。**ジャイロイドは 1.4 倍だけ**（6 回の `sin`・`cos` が重さのほとんど。JIT では縮まない）
- WASM の大きさ（`wasm32-unknown-unknown`、wasm-bindgen の前）: 3,463,106 バイト、**gzip 1,162,871 バイト**（`plan-render-algorithms.md` 5 章の「約 814 KB」は古い）。`simd128` ありでも gzip 1,163,133 バイトで、**大きさはほぼ変わらない**

**表 2: 特殊関数の 1 回の評価（µs。乱数の点）**

| 式（範囲） | ネイティブ | WASM |
|---|---|---|
| `gamma(x)`（$0.1 \sim 10$） | 0.06 | 0.04 |
| `erf(x)`（$-5 \sim 5$） | 1.47 | 1.46 |
| `besselJ(0, x)`・`besselY(1, x)`・`besselK(1, x)`（$0.1 \sim 30$） | 2.9・7.0・1.2 | 3.4・9.0・1.6 |
| `abs(hankelH1(0.5, x))` | 9.7 | 10.2 |
| `airyAi(x)`（$-10 \sim 10$） | 3.1 | 3.3 |
| `hypU(1, 2.5, x)`（$0.1 \sim 20$） | 11.7 | 13.3 |
| `hyp1F1(0.5, 1.5, x)`・`hyp2F1(0.5, 1, 1.5, x)` | 0.8・1.1 | 0.8・1.1 |
| `ellipticK(x)`・`lambertW(x)`・`zeta(x)` | 0.65・0.24・0.60 | 0.53・0.24・0.60 |
| `abs(zeta(x + i y))`・`abs(gamma(x + i y))`・`abs(besselJ(1, x + i y))` | 1.8・0.20・4.8 | 2.5・0.22・5.4 |

**表 3: 描画全体のうち、式の計算の割合**（`bench_report`、ネイティブ、ms）

| 式 | 式の計算 | 全体 | 割合 |
|---|---|---|---|
| 陽関数 `z = sin(x) cos(y)`（150） | 1.2 | 10.2 | 12 % |
| 陽関数 `z = x^2 - y^2`（150） | 0.5 | 9.2 | 6 % |
| 陰関数 球（80）・球（200） | 2.2・17.1 | 34.9・290.3 | 6 % |
| 陰関数 ジャイロイド（80）・（200） | 28.4・133.2 | 89.3・600.2 | 32 %・22 % |
| 陰関数 トーラス（200） | 12.6 | 137.3 | 9 % |
| 陽関数 `z = abs(zeta(x + i y))`（150） | 88.4 | 101.5 | 87 % |
| 陰関数 `abs(gamma(x + i y)) - z = 0`（80） | 164.0 | 207.4 | 79 % |
| 陽関数 `z = hypU(1, 2.5, x^2+y^2+0.1)`（150） | 533.0 | 546.4 | 97 % |
| 陽関数 `z = erf(x) erf(y)`（150） | 137.0 | 148.2 | 92 % |
| 2D `y = besselY(1, x)` | 136.7 | 149.9 | 91 % |
| 陽関数 `z = airyAi(x) y`（150） | 56.3 | 67.4 | 84 % |
| 陽関数 `z = besselJ(0, sqrt(x^2+y^2))`（150） | 14.8 | 28.7 | 52 % |

`bench_report` の「陰関数の部分ごとの時間」は、小さな球の 3 行（作り直しの道）のほかは 0 のまま出ている（記録されていない）。メッシュの段の内訳は今は分からない（O0 で埋める）。

**表 4: 代理（近似の関数）の試算**（Python・numpy。参照は mpmath 25 桁。誤差は「値の最大に対する」もの）

| 関数（範囲） | 区間ごとのチェビシェフ補間（次数 16） | 同（次数 32） | AAA（有理関数。目標 $10^{-10}$） |
|---|---|---|---|
| `hypU(1, 2.5, x)`（$0.1 \sim 20.1$） | 10 区間・$6.0 \times 10^{-15}$ | 7 区間・$4.7 \times 10^{-15}$ | 次数 8・$1.3 \times 10^{-10}$（点ごとの相対 $1.8 \times 10^{-8}$） |
| `besselY(1, x)`（$0.1 \sim 30.1$） | 14 区間・$5.0 \times 10^{-15}$ | 7 区間・$3.1 \times 10^{-13}$ | 22・$8.6 \times 10^{-11}$（点ごと $2.1 \times 10^{-6}$） |
| `besselJ(0, x)`（$0.1 \sim 30.1$） | 14・$3.0 \times 10^{-15}$ | 2・$3.6 \times 10^{-15}$ | 19・$8.3 \times 10^{-12}$ |
| `airyAi(x)`（$-10 \sim 10$） | 15・$4.8 \times 10^{-15}$ | 3・$2.9 \times 10^{-15}$ | 22・$9.8 \times 10^{-11}$（$x = 10$ 付近の点ごとの相対 0.16） |
| `erf(x)`（$-5 \sim 5$） | 10・$2.8 \times 10^{-15}$ | 2・$2.3 \times 10^{-14}$ | 18・$2.5 \times 10^{-11}$ |
| `zeta(0.5 + i y)`（$0 \sim 40$） | — | — | 19・$4.1 \times 10^{-11}$ |

- チェビシェフ補間は、作るのに区間の数 × 17 回の評価（`hypU` で 170 回 ≈ 2 ms）。評価は次数 16 のクレンショーの漸化式（掛け算・足し算 32 回ほど）で、1 回 10〜20 ns の見込み。
  **`hypU` で 500 倍前後、`erf` でも 70 倍前後**
- AAA は次数が小さいが、値の小さい所（減衰する裾・零点の近く）で点ごとの相対誤差が大きく、作るのに特異値分解が要る。実数の 1 変数ではチェビシェフ補間のほうが向く（D6）

**表 5: 特殊関数の精度の試し（N10 の下見）**: 23 の関数 × 40 点（15 点は実数 $[-10, 10]$、25 点は $\lvert z\rvert \le 10$ の複素数。mpmath 40 桁）。相対誤差の最大

| 関数 | 最大 | 関数 | 最大 |
|---|---|---|---|
| `gamma`・`loggamma`・`digamma` | $5.1 \times 10^{-14}$・$1.0 \times 10^{-14}$・$5.6 \times 10^{-14}$ | `besselY(1, z)`（整数次） | **$9.5 \times 10^{-10}$**（$z = 8.91$） |
| `zeta` | $7.4 \times 10^{-15}$ | `besselK(1, z)` | **$2.8 \times 10^{-10}$**（$z = 0.42 - 9.58i$） |
| `erf`・`erfc` | $4.7 \times 10^{-14}$・$1.2 \times 10^{-12}$ | `besselJ`・`besselI`・`besselY(2.3, z)` | $1.2 \times 10^{-13}$ 以下 |
| `airyAi`・`airyBi` | $4.2 \times 10^{-12}$・$6.0 \times 10^{-14}$ | `hyp1F1`・`hyp2F1`・`hypU` | $3.3 \times 10^{-13}$ 以下 |
| `ellipticK`・`ellipticE`・`lambertW`・`polylog(2, z)` | $9.1 \times 10^{-15}$ 以下 | `expIntegralEi`・`fresnelS` | $4.0 \times 10^{-14}$・$2.8 \times 10^{-12}$ |

計算できなかった点は 0。整数次の `besselY`・`besselK` が $3 \times 10^{-11}$ ほど、という P0-1 の記録より悪い点がある（N10 で表にする）。

### 1.3 すでに試して決めたもの（`plan-render-algorithms.md` 7 章の記録）

| Q | 結果 | この文書での扱い |
|---|---|---|
| Q2（アフィン演算） | ジャイロイドの二分の回数は区間演算と理想の差が 1.07 倍だけ。ローマ曲面だけ 2.5 倍 | Q2 は「同じ変数が何度も出る多項式」に絞った条件つきのステップ |
| Q3（タイルごとの式の刈り込み） | 不等式 8 本で、タイル 32×32 のとき残る式が 18 %（基準 1/3 以下を満たす）。ただし 2D の領域が遅い例が無い | 発動の条件（O2 の測定で遅い 2D の式が出たら）つきのステップ |
| Q4（BVH） | three.js の総当たりで 100 万三角形 41 ms。ふだんの大きさ（13.5 万個まで）は 6 ms。BVH は作り直しの時間で得にならない | 発動の条件つき。やるなら BVH ではなく GPU の深度を 1 画素読む方法 |

### 1.4 無いもの

- WASM での時間の物差し（今の時間は全部ネイティブ。WASM は「1.5〜2 倍」と書いてあるだけ）と、結果の指紋（速くした前後で、出力がビットまで同じか）
- 端末の速さで決める自動の判定（今は `pointer: coarse` だけ）
- SIMD の WASM と、それを読み込むときに選ぶ仕組み
- 特殊関数の代理、関数ごとの精度の表
- 霧の歩幅可変、動いている間の質の差の記録

## 2. 全体の決めごと

### 2.1 測って決める形

どのステップも次の形にする（`plan-render-algorithms.md` 5 章の守ること 1〜7 を引き継ぐ）。

1. **先に物差し**: 何をどの道具で測るか（O0 の WASM ベンチ・`bench_report`・`?bench=1`）を決め、変える前の数を 0 章の表に書く
2. **採用の基準を数で**: 各ステップの「完了の条件」に、採用の分かれ目の数（倍率・誤差・時間）を書いてある。満たさなければ**採らない**（採らなかった理由と数を 0 章と `plan-render-algorithms.md` 7 章に残す。それも完了）
3. **標準は、比べてから切り替える**: 新しい道は URL の引数（`?simd=0`・`?proxy=0`・`?jit=0` など）で切れるようにし、同じ式で前後を比べられるようにする
4. **全体の時間で判断する**: 式の計算だけ速くなっても、全体（1.2 の表 3）で効かなければ採らない（Q5）

### 2.2 結果がビットまで同じか

- **同じでなければならないもの**: Q6（SIMD）・Q5（JIT）。どちらも計算の順序を変えない（WASM の `simd128` には積和の融合 FMA が無いので、四則は 1 つずつ同じ丸めになる）。
  `sin`・`exp` などは、命令列の解釈と同じ Rust の関数（WASM の中の libm）を呼ぶ。**JavaScript の `Math.sin` を使うと値が変わる**ので使わない。O0 の指紋で確かめる
- **変わってよいもの**: Q1（代理）・Q7（霧の歩幅）・Q2（アフィン演算で区間が狭くなる）。前後の差を表にし、基準（各ステップ）に入ることを確かめる。代理は「近似」と説明に出す（D8）

### 2.3 実装の土台（全体の作り）

**新しいファイル**

| ファイル | ステップ | 中身 |
|---|---|---|
| `tools/wasm-bench/`（Rust の小さなクレート ＋ `run.mjs`） | O0 | 命令列と描画の時間を WASM（Node の WASI）とネイティブで測り、結果の指紋を出す |
| `web/src/lib/renderMode.ts`（書き換え）・`renderMode.test.ts`（新規） | O1 | 自動の判定の関数（three.js に依存しない） |
| `web/src/lib/gpuProbe.ts`（新規） | O2 | 小さな描画先で時間と打ち切りを測る（`Viewer.ts` の `abortRatio` をここへ移す） |
| `web/src/lib/wasmLoad.ts`（新規） | Q6 | SIMD が使えるかを調べて、読み込む `.wasm` を選ぶ |
| `core/src/jit.rs`（新規）・`core/js/jit.js`（新規） | Q5・O3 | 命令列から WASM のバイト列を作る・JS で組み立てて呼ぶ |
| `tools/special-ref/make_n10.py`（新規）・`core/tests/data/special_n10.inc`・`core/tests/special_n10.rs`（新規） | N10 | 参照値の表と、関数ごとの誤差の表 |
| `core/src/proxy.rs`（新規） | Q1・O4 | 区間ごとのチェビシェフ補間 |
| `core/src/affine.rs`（新規。採用なら） | Q2 | アフィン演算の試作 |

**データの形（Rust）**

```rust
// core/src/eval.rs の Op に足す（Q1）。引数 a の値を代理で計算する。範囲の外・代理が作れなかった区間は、元の関数（orig）で計算する
enum Op { /* 今のもの */ Proxy(std::sync::Arc<crate::proxy::Proxy>, usize, Box<Op>) }

// core/src/proxy.rs（Q1）
pub struct ChebPiece { pub lo: f64, pub hi: f64, pub re: [f64; 17], pub im: Option<[f64; 17]>, pub exact: bool }
pub struct Proxy { pub pieces: Vec<ChebPiece>, pub lo: f64, pub hi: f64, pub max_abs: f64, pub est_err: f64 }

// 性能の設定（plot の前に呼ぶ。plot の 25 個の引数は増やさない。geodesic_spec と同じ形）
#[wasm_bindgen] pub fn perf_options(proxy: bool, jit: bool);
```

**データの形（TypeScript）**

```ts
// renderMode.ts（O1）
export type GpuProbe = {
  webgl2: boolean
  renderer: string | null      // WEBGL_debug_renderer_info の UNMASKED_RENDERER_WEBGL（取れなければ null）
  perPixelMs: number | null    // 1 画素あたりの時間 c（測れなければ null）
  pixels: number               // 画面の画素数 N（画素密度込み）
  abortRatio: number | null    // 3D だけ。2D は null
}
export type AutoChoice = { gpu: boolean; reason: string; frameMs: number | null }
export function decideAuto(p: GpuProbe, kind: '2d' | '3d'): AutoChoice
```

**画面までの道**

- 自動の判定（O2）: `syncGpu2d`・`syncGpu3d` の `compileAsync(...).then(show)` の `show` の中で、**シーンに足す前に** `gpuProbe.ts` で測る → `decideAuto` → メッシュなら `gpuRejected` に理由を入れて層を捨て、`onGpuFallback` で ⚙ に理由を出す。画素ごとなら、その GLSL を `gpuAccepted` に覚えて今までどおり足す
- SIMD（Q6）: `npm run wasm` が 2 つのビルドを作る → `wasmLoad.ts` が `WebAssembly.validate` で選ぶ → `plot.worker.ts`・`evalExpr.ts` の `init(url)` に渡す
- 代理（Q1）: `lib.rs` の `plot` の中、`build` の直後に `perf_options` の値を見て、形を作る `Program` だけ `with_proxies` で差し替える。区間演算・数値の表示（`eval_constant`・文書 2 の近似値）は元のまま

## 3. 決めたこと（2026-11-25。全部おすすめの案で決まった）

D1〜D3（描き方の「自動」）は書く前に利用者に聞き、D4〜D12 は書いた後に「全部おすすめで」と決まった。

| # | 内容 | 決まったこと |
|---|---|---|
| D1 | 描き方の「自動」の端末の決まり（`plan-auto-render.md` 3.1） | **案 A**: `pointer: coarse` の決まりをやめる。どの端末でも「WebGL2 が無い・ソフトウェアの描画 → メッシュ」→「GLSL ごとに画面に出す前に時間を測り、見積もり $T$ が目安を超えたらメッシュ」→「打ち切り 15 % 超でメッシュ（3D だけ）」→「どれでもなければ画素ごと」 |
| D2 | 時間の目安（同 3.2） | $T = c \cdot N \le 50\,\mathrm{ms}$（止まっているときの 1 コマ。G0 と同じ目安） |
| D3 | 測った結果を覚えるか（同 3.3） | **覚えない**。GLSL ごとに毎回測る（`localStorage` は使わない） |
| D4 | SIMD の WASM の配り方 | 2 つ作って読み込むときに選ぶ。SIMD 版（標準）と SIMD 無し版（Safari 16.4 より前など）。大きさはほぼ同じ（1.2）なので、利用者が読み込む量は増えない。増えるのはビルドの時間だけなので、`npm run dev` は SIMD 版だけ、`npm run build` で両方（採らなかった案: SIMD 版だけ（古いブラウザでは動かない。2026 年 11 月の時点で、Safari 16.4 は 3 年半前）） |
| D5 | JIT（Q5）の採否の基準 | 全体の時間で決める: Q5 の試作で、`bench_report` の陰関数の合計（WASM）が Q6 の後より **1.3 倍以上**速くなり、多項式 4 式の 1 点の時間が **2 倍以上**なら O3（本体）に進む。1.2 の表 1・3 からは届かない見込みが強い（式の計算が全体の 6〜30 %）（採らなかった案: 1 点の時間だけで決める（`plan-render-algorithms.md` 0.2 の元の基準。ジャイロイドで 2 倍。表 1 から 1.4 倍で届かない）） |
| D6 | 代理の作り方（Q1） | 区間ごとのチェビシェフ補間（次数 16）。作るのが簡単（特異値分解が要らない）、誤差の見積もりが係数の末尾から出る、試算で値の最大の $10^{-14}$（表 4）（採らなかった案: AAA（有理関数。極を持つ関数に強いが、作るのに特異値分解が要り、裾で点ごとの誤差が大きい）。複素数の引数（O4）で改めて比べる） |
| D7 | 代理を使う所 | 形を作る点の計算だけ（陽関数・陰関数・媒介変数の格子の値と、その後の二分法の詰め・色の値）。区間演算（もともと特殊関数は分からない扱い）・定数の値・文書 2 の近似値の表示・積分の中の評価には使わない（採らなかった案: 全部で使う（近似値の表示が近似の近似になる）） |
| D8 | 代理を使ったことの見せ方 | 式の説明に 1 行「特殊関数は速い近似で描いています（ずれは値の大きさの $10^{-12}$ 以下の見込み）」。`?proxy=0` で切って比べられる。⚙ の項目は作らない（採らなかった案: ⚙ に「近似で速く」の印（既定は入）。項目が増える） |
| D9 | 霧（Q7）が速くなったら標準にするか | しない。不等式の領域の標準は今の境目の面とふた（くっきりして軽い）のまま。霧は ⚙ の選択肢として速くなるだけ（採らなかった案: 3D の不等式の標準を霧にする） |
| D10 | 動いている間の再利用（Q10） | まず差を見せて決める: Q10 で「動いている間に質がどれだけ下がるか」を測り、止まったときとの並べた絵を利用者に見せる。「実害あり」なら O5（光線の層の再利用の試作）（採らなかった案: 測らずにやらない（今の段階的に下げる仕組みで足りる、とみなす）） |
| D11 | 条件つきのもの（Q2・Q3・Q4） | 発動の条件を書いて保留。Q2 は Rust の数え上げ（小さい）だけ先にやり、GLSL に移すのは条件つき。Q3・Q4 は条件が満たされるまで手を付けない（採らなかった案: 今やる） |
| D12 | N10 の表をテストにするか | する。関数ごとに今の誤差の最大の 10 倍（下限 $10^{-13}$）を上限としてテストに書き、悪くなったら落ちる（直すのは別のステップ）（採らなかった案: 表を出すだけ（悪くなっても気づかない）） |

## 4. ステップの一覧

| # | 内容 | 前提 | 大きさ | 担当 |
|---|---|---|---|---|
| O0 | 測る土台（WASM の時間・結果の指紋・陰関数の部分ごとの時間） | — | 小〜中 | C |
| O1 | 自動の判定の関数と表のテスト（`renderMode.ts`） | — | 小 | C |
| O2 | 自動の判定を画面に（画面に出す前に測る。2D・3D。⚙ の理由の文） | O1 | 中 | B |
| Q6 | WASM SIMD（ビルドの設定 → 足りなければ `chunk_real` の四則を手で `f64x2`）と、2 つのビルドの選び方 | O0 | 中 | B |
| Q5 | JIT の試作と判断（命令列 → WASM のバイト列。実数の道だけ） | O0, Q6 | 中 | B |
| O3 | JIT の本体（D5 の基準を満たしたときだけ） | Q5 の採用 | 大 | A |
| N10 | 特殊関数の精度の表（mpmath の参照値・関数ごとの誤差の最大） | — | 小〜中 | C |
| Q1 | 特殊関数の代理（実数の 1 変数。区間ごとのチェビシェフ補間） | O0, N10 | 中 | A |
| O4 | 複素数の引数の代理（`zeta(x+iy)`・`gamma(x+iy)` の色分け。条件つき） | Q1 | 中〜大 | A |
| Q7 | 霧の歩幅可変（境目から遠い所・中身の深い所を大きく進む） | — | 中 | B |
| Q10 | 動いている間の質の差を測り、利用者に見せる（判断） | — | 小 | B |
| O5 | 光線の層の時間方向の再利用（Q10 で「実害あり」のときだけ） | Q10 の採用 | 大 | A |
| Q2 | アフィン演算（Rust で二分の回数を数える → 条件つきで GLSL） | — | 小（数える）・中（GLSL） | B |
| Q3 | 2D の領域のタイルごとの刈り込み（発動の条件つき） | O2 | 中 | B |
| Q4 | カーソルの下の点を GPU の深度で（発動の条件つき） | — | 中 | B |

**おすすめの順番**: O0 → O1 → O2（使う人に見える効果が大きく、決めることは済んでいる）→ Q6 → N10 → Q1 → Q5（D5 で判断）→ Q7 → Q10（判断）→ Q2（数えるだけ）。
O3・O4・O5・Q3・Q4 は、前のステップの判断・発動の条件しだい。

- O0 は全部の物差しなので最初。O1・O2 は計算エンジンを触らないので、Q6 と並行してよい
- N10 は Q1 の精度の物差し（代理の「正しい値」が正しいかを先に知る）。単独でも価値がある
- Q5 は Q6 の後に測る（Q6 の後の解釈型と比べないと、JIT の得が分からない）

## 5. ステップごとの指示書

共通の決まり: コメントと README は日本語。`core/` で `cargo test --release`、`web/` で `npm test` と `npm run check` が通ること。
テストのエラーの中身は WASM 以外では読めない（`JsError` を作るとパニックする）ので、計算エンジンの内側の関数を呼ぶか、`std::panic::catch_unwind` で成功・失敗を見る。
時間は 3 回（WASM は温めた後 5 回）の最小か中央値で比べ、どちらを使ったかを書く。採らなかったステップも、測った数と理由を 0 章の表に書けば完了。

### O0: 測る土台

- **担当**: C（理由: 測る道具を作るだけで、正しい答えが決まっている。1.2 で試した形をそのまま道具にする）
- **平たい説明**: 今の時間は全部ネイティブ（PC で直接動かした Rust）で測っている。実際の画面は WASM（ブラウザの中）なので、WASM で同じ式の時間を測る道具を作る。
  あわせて、速くした前後で計算の結果が 1 ビットも変わっていないかを確かめる「指紋」（結果のビットから作った短い数）を出す
- **触るファイル**: `tools/wasm-bench/Cargo.toml`・`src/main.rs`・`run.mjs`・`README.md`（新規）、`core/tests/bench.rs`（部分ごとの時間）、`core/src/mesh.rs` か `lib.rs`（部分ごとの時間の記録が抜けている道）、`docs/agent-tiers.md` の 4 章（準備に `rustup target add wasm32-wasip1`）
- **やること**
  1. `tools/wasm-bench/`: `graph-core` をパスで使う小さなクレート。式の一覧（1.2 の表 1・2 の式と、`bench_report` の陽関数・陰関数の行）を、(a) 命令列の 1 点の時間（乱数の点と格子の点の両方）、(b) `plot` の全体と内訳、で測る。
     式ごとに出力のビットの FNV-1a の指紋も出す
  2. `run.mjs`: `cargo build --release --target wasm32-wasip1` を、ふつうと `RUSTFLAGS="-C target-feature=+simd128"` の 2 通りで作り、Node の `node:wasi` で動かして表を並べる。ネイティブも同じクレートで動かす。
     **温め**: 表を取る前に、全部の式を 1 回ずつ回す（V8 は最初の数回を軽い翻訳で動かすので、温めないと 1.2 のように順番でぶれる）
  3. `bench_report` の「陰関数の部分ごとの時間」が 0 の行を埋める（今は小さな球の作り直しの道だけが記録している。ほかの道でも `mesh.stats.phases` に入れる）。
     メッシュの段のどこが重いかを、この表で初めて見られるようにする（速くするのは別の計画。7 章）
  4. 測った今の数（ネイティブ・WASM・WASM＋SIMD）を、この文書の 0 章と `plan-render-algorithms.md` 7 章に書く。WASM の大きさ（`wasm-pack` の後の `.wasm` と gzip）も書き、`plan-render-algorithms.md` 5 章の 3 の「約 814 KB」を直す
- **実装の土台**

  ```rust
  // tools/wasm-bench/src/main.rs
  struct Row { src: &'static str, vars: &'static [&'static str], n: usize, lo: f64, hi: f64 }
  fn per_point(r: &Row, grid: bool) -> (f64 /* ns */, u64 /* 指紋 */);
  fn plot_time(src: &str, res: u32, ires: u32) -> ([f64; 4] /* 式・メッシュ・粗い版・全体 */, u64);
  fn fnv1a(values: &[f64]) -> u64; // to_bits を 8 バイトずつ
  fn main() { /* 温め → 表を Markdown で出す */ }
  ```

  ```js
  // tools/wasm-bench/run.mjs: node tools/wasm-bench/run.mjs [--only 式の一部]
  // 1. 2 通りのビルド  2. new WASI({ version: 'preview1' }) で start  3. ネイティブの結果と並べ、指紋が違えば「違う」と出す
  ```

- **やらないこと**: 速くすること自体。ブラウザでの測定（`?bench=1` はそのまま）
- **完了の条件**
  - `node tools/wasm-bench/run.mjs` が、1.2 の表 1・2 の式と `bench_report` の行について、3 列（ネイティブ・WASM・WASM＋SIMD）の時間と指紋を出す。同じビルドで 2 回回したとき、多項式 4 式の時間のぶれが ±15 % 以内
  - ネイティブと WASM の指紋が、初等関数だけの式で一致する（違えば、その式と理由を 0 章に書く。`sin` などの libm の違いなら、それが正しい結果）
  - `bench_report` の部分ごとの時間が、球（80）・ジャイロイド（80）でも 0 でない
- **壊してはいけないもの**: `bench_report` の今の列（足すのはよいが、今の列の意味を変えない）
- **README**: 書かない（`tools/wasm-bench/README.md` に使い方）

### O1: 自動の判定の関数と表のテスト

- **担当**: C（理由: 決めごと D1〜D3 が全部決まっていて、three.js に依存しない関数と表のテストだけ）
- **平たい説明**: 「この端末・この式は、画素ごと（GPU）で描くかメッシュで描くか」を決める部分だけを、画面から切り離した関数にする。
  入力は測った数（GPU の名前・1 画素の時間・画面の画素数・打ち切りの割合）、出力は「どちらで描くか」と「理由の文」
- **触るファイル**: `web/src/lib/renderMode.ts`、`web/src/lib/renderMode.test.ts`（新規）、`web/package.json`（`npm test` に足す）
- **やること**
  1. `isSoftwareRenderer(name)`: `/swiftshader|llvmpipe|softpipe|basic render|microsoft basic|software rasterizer/i`。`null`（名前が取れない）は「分からない」で、ソフトウェア扱いにしない
  2. `perPixelCost(small, large)`: 小さい描画（横 96 画素）と大きい描画（横 192 画素）を 2 回ずつ測った時間から、$c = \dfrac{\min t_{\mathrm{large}} - \min t_{\mathrm{small}}}{N_{\mathrm{large}} - N_{\mathrm{small}}}$。
     差が 0 以下（測りの粒度より速い）なら、$c = \min t_{\mathrm{large}} / N_{\mathrm{large}}$（上から抑えた値）。時間が全部 0 なら $c = 0$
  3. `decideAuto(p, kind)`: 3.1 の D1 の順。理由の文（メッシュにしたときだけ）:
     - WebGL2 が無い: 「この端末では WebGL2 が使えないので、メッシュで描いています」
     - ソフトウェア: 「この端末は GPU を使わずに描いているので（{名前}）、メッシュで描いています（画素ごと（GPU）を選ぶと使えます）」
     - 遅い: 「この端末では画素ごとの描き方が重いので（見積もり 1 コマ {T} ms）、メッシュで描いています（画素ごと（GPU）を選ぶと使えます）」
     - 打ち切り: 今の文のまま（`checkAborts` の文）
  4. `gpuByDefault` を消す（`ExpressionItem.svelte` の 322 行は O2 で直す。このステップでは、まだ呼ばれているなら残してよい）
- **実装の土台**

  ```ts
  export const FRAME_BUDGET_MS = 50   // D2
  export const ABORT_LIMIT = 0.15
  export function isSoftwareRenderer(name: string | null): boolean
  export function perPixelCost(small: { px: number; ms: number[] }, large: { px: number; ms: number[] }): number | null
  export function decideAuto(p: GpuProbe, kind: '2d' | '3d'): AutoChoice {
    // if (!p.webgl2) → mesh
    // if (isSoftwareRenderer(p.renderer)) → mesh
    // const T = p.perPixelMs === null ? null : p.perPixelMs * p.pixels
    // if (T !== null && T > FRAME_BUDGET_MS) → mesh（frameMs: T）
    // if (kind === '3d' && p.abortRatio !== null && p.abortRatio > ABORT_LIMIT) → mesh
    // → gpu
  }
  ```

- **やらないこと**: 実際に測ること（O2）、`localStorage`（D3）
- **完了の条件**（`renderMode.test.ts`。`motionQuality.test.ts` と同じ表の形）
  - 指の端末でも速ければ画素ごと（**古い決まりでは失敗する行**: 古い `gpuByDefault` を `pointer: coarse` 真で呼ぶとメッシュになることを、同じファイルで確かめる）
  - `ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero)))`・`llvmpipe (LLVM 15.0.7, 256 bits)`・`Microsoft Basic Render Driver` → メッシュ、`ANGLE (Apple, Apple M2)`・`null` → 時間で決まる
  - $c = 2 \times 10^{-5}\,\mathrm{ms}$・$N = 1920 \times 1080 \times 2^2$ → $T = 165.9\,\mathrm{ms}$ でメッシュ、理由の文に「166 ms」。$N = 1920 \times 1080$ → $T = 41.5\,\mathrm{ms}$ で画素ごと
  - `perPixelCost`: 小 $[2.0, 1.8]\,\mathrm{ms}$・9,216 画素、大 $[5.1, 5.3]\,\mathrm{ms}$・36,864 画素 → $c = (5.1 - 1.8) / 27{,}648 = 1.194 \times 10^{-4}$。差が負の例で上から抑えた値
  - 2D は打ち切りを見ない（`abortRatio` が 0.9 でも時間が速ければ画素ごと）
- **壊してはいけないもの**: `wantGpu` の `?gpu2d=0`・`?gpu3d=0` と、⚙ で「メッシュ」「画素ごと」を選んだときの動き（今の `motionQuality.test.ts` などのテスト）
- **README**: まだ書かない

### O2: 自動の判定を画面に

- **担当**: B（理由: 描画の手順（コンパイル → 測る → 出すか捨てる）の順番を変える。three.js の描画先・色空間の落とし穴（`abortRatio` のコメント）を踏まないようにする必要がある）
- **平たい説明**: 画素ごとの描き方の準備（シェーダーのコンパイル）が終わったら、画面に出す**前に**、左下の小さな範囲に何回か描いて時間を測る。
  遅ければ出さずにメッシュのまま。今は「出した次のコマで打ち切りを数える」ので、遅い端末で重い 1 コマを描いてしまう（`plan-auto-render.md` の困ること 4）
- **触るファイル**: `web/src/lib/gpuProbe.ts`（新規）、`web/src/lib/Viewer.ts`（`syncGpu2d`・`syncGpu3d`・`abortRatio`・`checkAborts`）、`web/src/lib/ExpressionItem.svelte`（322 行）、`web/src/App.svelte`（`onGpuFallback` の受け取り）、README
- **やること**
  1. `gpuProbe.ts` に `abortRatio` の中身（ほかの物を隠す・左下だけに描く・元に戻す）を移し、時間も測れるようにする: 光ありのふつうの描画（`uProbe = 0`）で、横 96 画素と 192 画素を 2 回ずつ描き、
     それぞれ 1 画素の `readPixels` で描き終わりを待って `performance.now()` の差を取る。3D は打ち切りの数え（`uProbe = 1`、横 192 画素の 1 回。今と同じ）も同じ関数で行う（描画は 5 回）。
     `plan-auto-render.md` 3.2 の「打ち切りと同じ描画で時間も測る（3 回）」から変えた（光を当てない描画では時間が軽く出る。6 章の 8）。光線の回数は止まっているときの上限（`RAY_STILL`）で測る（`camMoving` によらず）
  2. `syncGpu3d`・`syncGpu2d` の `show` で、`scene.add` の前に測る（測る間だけシーンに足し、ほかを隠す）。`decideAuto` がメッシュなら、層を捨てて `gpuRejected` に理由を入れ、`onGpuFallback` を呼ぶ。
     画素ごとなら `gpuAccepted`（GLSL の集合）に入れて今までどおり出す。**同じ GLSL では測り直さない**（D3。画面の回転で $N$ が変わっても測り直さない）
  3. 自動でないとき（⚙ で「画素ごと」・`forced`）は測らない。`?gpu2d=0`・`?gpu3d=0` は今のまま
  4. WebGL2 の有無・描画の名前は、`Viewer` を作るときに 1 回だけ読む（`gl.getExtension('WEBGL_debug_renderer_info')`。無ければ `null`）
  5. `ExpressionItem.svelte` の「この端末では、自動のときはメッシュで描きます」の文を消し、`onGpuFallback` の理由を出す（3D は今もそうしている。2D にも同じ道を通す）
  6. `checkAborts`（出した後の打ち切りの数え）は 1 の中に入ったので消す
- **実装の土台**

  ```ts
  // gpuProbe.ts
  export type ProbeTarget = { mesh: THREE.Mesh; ray?: RaycastUniforms }   // 2D は ray が無い
  export function probeLayer(renderer: THREE.WebGLRenderer, scene: THREE.Scene, camera: THREE.Camera,
                             t: ProbeTarget, view: { w: number; h: number }): { small: Timing; large: Timing; abortRatio: number | null }
  type Timing = { px: number; ms: number[] }

  // Viewer.ts の show の中（3D）
  // const m = probeLayer(...)
  // const choice = decideAuto({ webgl2: true, renderer: this.rendererName, perPixelMs: perPixelCost(m.small, m.large),
  //                             pixels: canvas.width * canvas.height, abortRatio: m.abortRatio }, '3d')
  // if (!choice.gpu) { this.gpuRejected.set(id, { glsl, reason: choice.reason }); this.removeGpu3d(id); this.onGpuFallback?.(id, choice.reason); return }
  // this.gpuAccepted.add(glsl); layer.ready = true; this.scene.add(layer.mesh) …
  ```

- **やらないこと**: 判定の決まりを変えること（O1）、`localStorage`、測った値を ⚙ に数字で出すこと（理由の文の見積もりだけ）
- **完了の条件**
  - ヘッドレスの Chromium（SwiftShader）で、自動の式が全部メッシュになり、⚙ に「GPU を使わずに描いているので（…SwiftShader…）」が出る。`?bench=1&set=samples` の掃引が通る
  - 同じ式でスライダーを動かして GLSL が変わらない間は、測り直さない（測った回数を数える一時的な記録で確かめる）
  - 判定の前に光線の箱・2D の平面が画面に出ない（測る前の 1 コマを、`renderer.info.render.calls` の記録か、測る間の `scene` の子の一覧で確かめる）
  - 依頼者の実機（デスクトップの GPU とスマートフォン）で、ジャイロイド・`x y z = 1` の ⚙ の表示を見てもらい、結果を 0 章に書く（速い端末では画素ごと、遅ければ理由の文と見積もりの ms）
- **壊してはいけないもの**: 打ち切りでメッシュに回す動き（`mod(x y, z) = 0` が今と同じくメッシュになり、同じ理由の文）、Windows の Chrome で固まらないこと（`abortRatio` のコメントのとおり、描く先と光源の数を変えない）
- **README**: 描き方の ⚙ の段落の「自動」の説明を、「端末とその式の描く速さで決める（目安は止まっているときの 1 コマ 50 ms）」に直す。`plan-auto-render.md` の冒頭に「指示書は `plan-performance.md` の O1・O2」と 1 行

### Q6: WASM SIMD

- **担当**: B（理由: 計算の中身はほぼ変えないが、ビルドの設定・2 つの `.wasm` の読み込み・結果がビットまで同じかの確かめと、触る所が広い）
- **平たい説明**: SIMD は、1 つの命令で 2 つの数（`f64x2`）をまとめて計算する仕組み。命令列の計算は「同じ命令を 256 点に」なので、そのまま 2 点ずつにできる。
  1.2 では、ビルドの設定を変えるだけ（コンパイラが自動で 2 点ずつにする）で、多項式の式が 1.5 倍前後速くなった
- **触るファイル**: `web/package.json`（`wasm`・`build`）、`web/src/lib/wasmLoad.ts`（新規）、`web/src/lib/plot.worker.ts`・`evalExpr.ts`（`init` の呼び方）、`core/src/eval.rs`（2 段目だけ）、`.cargo/config.toml` は作らない（RUSTFLAGS は `package.json` のスクリプトで渡す）、README
- **やること**
  1. **1 段目（コードを変えない）**: `npm run wasm` を、`RUSTFLAGS="-C target-feature=+simd128"` で `src/wasm-core/` に作るようにする。`npm run build` では、SIMD 無し版も `src/wasm-core-nosimd/` に作る（D4）
  2. 2 つのビルドの JS の接着コード（`graph_core.js`）が同じかを `diff` で確かめる。**同じなら**、接着コードは SIMD 版のものだけを使い、`init(url)` に渡す `.wasm` の URL だけを選ぶ。違えば、`wasmLoad.ts` で接着コードごと動的に `import()` する
  3. `wasmLoad.ts`: SIMD の命令を 1 つ含む最小の WASM（`v128.const` を返す関数。20 バイトほど）を `WebAssembly.validate` で調べ、結果で URL を選ぶ。`?simd=0` で SIMD 無し版を強制（比べる用）
  4. O0 の道具で、多項式 4 式・`bench_report` の行の時間（WASM と WASM＋SIMD）と指紋を測る。**指紋が全部一致すること**
  5. **2 段目（1 段目が 1.3 倍に届かないときだけ）**: `chunk_real` の `Bin(Add | Sub | Mul | Div)`・`Neg`・`Min`・`Max` を、`core::arch::wasm32` の `f64x2_*` で 2 点ずつ書く（`#[cfg(target_feature = "simd128")]` のときだけ。ネイティブ・SIMD 無しは今のループ）
- **実装の土台**

  ```ts
  // wasmLoad.ts
  const SIMD_PROBE = new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,123,3,2,1,0,10,22,1,20,0,253,12,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,11])
  export function wasmUrl(): URL {
    const off = new URLSearchParams(location.search).get('simd') === '0'
    const simd = !off && WebAssembly.validate(SIMD_PROBE)
    return new URL(simd ? '../wasm-core/graph_core_bg.wasm' : '../wasm-core-nosimd/graph_core_bg.wasm', import.meta.url)
  }
  // plot.worker.ts: await init({ module_or_path: wasmUrl() })
  ```

  ```rust
  // 2 段目（必要なときだけ）。eval.rs の chunk_real の中
  #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
  fn add_f64x2(dst: &mut [f64], a: &[f64], b: &[f64]) {
      use core::arch::wasm32::*;
      // 2 つずつ v128_load → f64x2_add → v128_store。端の 1 つは今のループ
  }
  ```

  （`SIMD_PROBE` は `(func (result v128) v128.const i64x2 0 0)` だけのモジュール。Node 22 で `WebAssembly.validate` が真になることを確かめた。SIMD の無いブラウザでは偽になる）
- **やらないこと**: 区間演算・複素数の道の SIMD、`relaxed-simd`（FMA が入り、結果が変わる）、JIT（Q5）
- **完了の条件**
  - O0 の道具で、多項式 4 式の 1 点の時間（WASM）が **1.3 倍以上**速い（中央値）。届かなければ 2 段目をして測り直し、それでも届かなければ採らない（1 段目も戻す）
  - 指紋が、O0 の全部の式で SIMD 無しと一致する
  - `bench_report` の行の全体の時間（WASM）を前後で表にする（表 3 から、全体では 0〜20 % の見込み）
  - Chrome・Firefox で SIMD 版、`?simd=0` で SIMD 無し版が読み込まれる（開発者ツールの通信の記録で確かめる）
- **壊してはいけないもの**: `npm run dev` の手順（1 回のビルドで動く）、ネイティブのテスト
- **README**: 動く環境の段落に「SIMD の使えるブラウザ（Chrome 91・Firefox 89・Safari 16.4 から）では速い版を使う」と 1 行

### Q5: JIT の試作と判断

- **担当**: B（理由: 試作と測定まで。命令の種類を絞った小さな版で、採否の数を出す）
- **平たい説明**: 今は「命令を 1 つ読む → 256 点に計算する → 次の命令を読む」で、点ごとの途中の値を毎回メモリに書いて読む。
  JIT は、式ごとに「1 点の計算を全部まとめた関数」を WASM の命令として作り、ブラウザに翻訳させる。1.2 の融合の列が上限の目安で、多項式で 3〜5 倍、ジャイロイドで 1.4 倍
- **触るファイル**: `core/src/jit.rs`（新規。バイト列を作るだけ）、`core/js/jit.js`（新規）、`tools/wasm-bench/`（JIT の列を足す）
- **やること**
  1. `jit::emit(&Program) -> Option<Vec<u8>>`: 実数の道で扱える命令（四則・`Neg`・`Min`・`Max`・整数乗・`sqrt`・`abs`・`floor`）だけの式を、1 つの関数 `run(inputs_ptr, n, out_ptr) -> i32` の WASM モジュールにする。
     点のループの中で命令を順に計算し、途中の値は WASM の局所変数（`f64`）に置く。`sin` などの初等関数は、**計算エンジンから書き出した関数を import して呼ぶ**（2.2。`Math.sin` は使わない）。
     途中で有限でない値・実数に収まらない所（`sqrt` の中が負）が出たら、`-1` を返して止める（呼び出し側が、その CHUNK を今の道で計算し直す。`chunk_real` が false を返すのと同じ約束）
  2. `jit.js`: バイト列から `new WebAssembly.Module` → 計算エンジンの `memory` と関数を import にして `Instance` を作り、番号で覚える。Rust からは `#[wasm_bindgen(module = "/js/jit.js")]` の関数で呼ぶ（`wasm32` のときだけ。ネイティブでは JIT 無し）
  3. O0 の道具に「JIT」の列を足す（多項式 4 式・ジャイロイド・`bench_report` の陰関数）。WASI ではなく Node の `wasm-bindgen` の `--target nodejs` の版で測ってよい
  4. D5 の基準で判断し、数を 0 章に書く
- **実装の土台**

  ```rust
  // core/src/jit.rs
  pub struct JitModule { pub bytes: Vec<u8>, pub inputs: usize }
  pub fn emit(p: &Program) -> Option<JitModule>;      // 扱えない命令があれば None
  // バイト列の形: type section（(i32, i32, i32) -> i32 と、import する (f64) -> f64）/ import section（"env" "memory"、"m" "sin" …）/
  //   function section / code section（loop { 各命令 → local.set; 出力を f64.store; 有限か調べる }）
  #[no_mangle] pub extern "C" fn jit_sin(x: f64) -> f64 { x.sin() }   // 解釈と同じ libm を使うために書き出す
  ```

  ```js
  // core/js/jit.js
  export function jit_compile(bytes, memory, exports) { /* Module → Instance。番号を返す */ }
  export function jit_run(id, inputsPtr, n, outPtr) { return instances[id].exports.run(inputsPtr, n, outPtr) }
  ```

- **やらないこと**: 複素数の道・区間演算の JIT、特殊関数、本体への組み込み（O3）
- **完了の条件**: D5 の 2 つの数（多項式 4 式の 1 点の時間の倍率、`bench_report` の陰関数の合計の倍率。どちらも Q6 の後の WASM と比べる）と、JIT の指紋が解釈と一致すること（ジャイロイドも）を 0 章に書く。
  基準を満たせば O3 へ、満たさなければ「採らない」と `plan-render-algorithms.md` 7 章に書いて終わり（`jit.rs` は消す）
- **壊してはいけないもの**: 何も変えない（試作は O0 の道具の中で使うだけ。`plot` からは呼ばない）
- **README**: 書かない

### O3: JIT の本体（Q5 で採用のときだけ）

- **担当**: A（理由: 式ごとに WASM を作って差し替えるので、間違えると一部の CHUNK だけ値が違う、のように見た目で気づきにくい。作り直し・捨てる時期・Worker の再起動の扱いも多い）
- **平たい説明**: Q5 の試作を、描画の道に入れる
- **触るファイル**: `core/src/jit.rs`、`core/src/eval.rs`（`Code` に JIT の番号）、`core/src/lib.rs`（`perf_options` の `jit`）、`web/src/lib/plot.worker.ts`、README
- **やること**
  1. `Program` を作ったとき（`compile_with` の後）、命令が 8 個以上・点の数が多い使い方（格子）のときだけ `emit` → `jit_compile`。番号を `Code` に持つ。式が変わって捨てた `Program` の番号は、JS 側で外す（`Drop`）
  2. `run_inner` で、JIT があれば `chunk_real` の代わりに `jit_run`。`-1` ならその CHUNK を今の道で計算し直す
  3. `?jit=0` で切れる。作るのに 1 ms を超えた式は、0 章に記録して以後作らない（`emit` の上限）
- **完了の条件**: O0 の全部の式で指紋が一致。`bench_report` の陰関数の合計（WASM）が Q5 で測った倍率の 8 割以上。`?bench=1&set=samples` の掃引が通る。Worker を作り直しても動く
- **壊してはいけないもの**: ネイティブのテスト（JIT 無し）、`?jit=0` の結果
- **README**: 速さの段落に 1 行

### N10: 特殊関数の精度の表

- **担当**: C（理由: 参照値の表を作って比べるだけ。正しい答えは mpmath が決める）
- **平たい説明**: 特殊関数（ベッセル関数・超幾何関数など）は、近似の式を組み合わせて計算している。複素平面のいろいろな所で、40 桁で計算した正しい値と比べ、
  関数ごとに「いちばん悪い所で何桁合っているか」を表にする。直すのは表を見てから（別のステップ）
- **触るファイル**: `tools/special-ref/make_n10.py`（新規）、`core/tests/data/special_n10.inc`（新規。作ったもの）、`core/tests/special_n10.rs`（新規）、`plan-math-notation.md` 7.1 の候補 K の行
- **やること**
  1. `make_n10.py`（mpmath。`pip install mpmath`）: `functions.rs` の名前の表（`(Func::…, &["名前"], &[引数の数])`）にある特殊関数の全部について、関数ごとに 200 点: 実数 50 点（$[-20, 20]$。定義域が正の関数は $(0, 40]$）・
     複素数 150 点（$\lvert z\rvert \le 20$ の円板に一様。そのうち 30 点は実軸・虚軸の近く $\lvert\operatorname{Im} z\rvert < 0.1$）。種は固定。パラメータを取る関数は、1.2 の表 5 の値（整数次と非整数次の両方）。
     mpmath の定義が計算エンジンと違う関数（楕円積分の引数 $m$ と $k$、フレネル積分の正規化など）は、`special_values.rs` の値で合わせてから作る
  2. 表の形は `special_accuracy_table.inc` と同じ（式の文字列・実部・虚部）に、関数の名前を足す。参照値が有限でない点・$\lvert f\rvert < 10^{-280}$ の点は入れない
  3. `special_n10.rs`: 関数ごとに、点の数・相対誤差の最大・そのときの引数・計算できなかった点の数を Markdown の表で出す（`--nocapture`）。D12 の上限を超えた関数があれば落ちる
  4. 表を PR の説明に貼る。$10^{-10}$ より悪い関数を、`plan-render-algorithms.md` 7 章の P0-1 の行の後に「直す候補」として書く
- **実装の土台**

  ```rust
  const TABLE: &[(&str /* 関数 */, &str /* 式 */, f64, f64)] = include!("data/special_n10.inc");
  const BOUND: &[(&str, f64)] = &[("besselY", 1e-8), /* 関数ごとの上限（今の最大の 10 倍、下限 1e-13） */];
  #[test] fn special_function_accuracy_table() { /* 関数ごとに集めて表を出し、BOUND と比べる */ }
  ```

- **やらないこと**: 精度を直すこと、区間演算の保証、時間を測ること（O0）
- **完了の条件**: 全部の特殊関数（`functions.rs` の表の、ガンマ・ゼータ・誤差関数・指数積分・ベッセル・エアリー・直交多項式・超幾何・楕円・ランベルトの仲間）の表が PR に貼ってある。
  1.2 の表 5 の 2 つ（`besselY(1, 8.91)`・`besselK(1, 0.42 − 9.58i)`）が表に出ている。`cargo test --release --test special_n10` が通る
- **壊してはいけないもの**: `special_values.rs`・`special_accuracy.rs`（別のテストとして残す）
- **README**: 特殊関数の段落に「精度の表は `core/tests/special_n10.rs`」と 1 行

### Q1: 特殊関数の代理（実数の 1 変数）

- **担当**: A（理由: 代理は誤差の保証が無く、区間の端・特異点・極の近くで外れても、絵では少しずれるだけで気づきにくい。どこで使ってどこで使わないか（D7）の線引きも誤りやすい）
- **平たい説明**: `hypU(1, 2.5, x)` のような式は、1 回の計算が 10 µs ほどかかり、曲面全体で 0.5 秒になる（1.2 の表 3）。描く範囲が決まれば、その範囲だけで十分正確な
  「速い多項式の継ぎ合わせ」（チェビシェフ補間）を最初に 1 回作り、点ごとにはそれを使う。作るのに数 ms、点ごとには 1 回 20 ns ほど
- **触るファイル**: `core/src/proxy.rs`（新規）、`core/src/eval.rs`（`Op::Proxy`・`Program::with_proxies`・`chunk_real` と `chunk_complex`）、`core/src/lib.rs`（`perf_options`・`plot` の `build` の後）、`web/src/lib/plot.worker.ts`（`?proxy=0`）、`core/tests/proxy.rs`（新規）、README
- **やること**
  1. `proxy::build(f, lo, hi, tol, budget)`: 区間 $[a, b]$ でチェビシェフ点 $x_k = \tfrac{a+b}{2} + \tfrac{b-a}{2}\cos\tfrac{(k + 1/2)\pi}{17}$（$k = 0, \dots, 16$）の値から次数 16 の係数を出し、
     末尾 4 つの係数の最大が $\mathrm{tol} \cdot \max\lvert f\rvert$ 以下なら採る。超えたら半分に割る。割る深さは 12 まで・評価の回数は `budget` まで。
     深さが尽きた区間（極・端の特異点の近く）は `exact: true`（その区間は元の関数で計算する）。値に有限でないものが出た区間も `exact`
  2. 値が複素数の関数（`hankelH1` など）は、実部と虚部を別々に補間する（`im: Some`）。全部の標本で虚部が 0 なら `im: None` にし、**実数の道**（`chunk_real`）で計算できる命令にする
  3. `Program::with_proxies(&self, boxes, budget)`: 特殊関数の命令（`Call1`・`Call2`・`CallN`）のうち、**引数が 1 つだけ変数に依り、残りが定数**のものを探す。
     その引数の命令の区間を `interval_with` と同じ計算で求め（区間が出なければ諦める）、`build` で代理を作って `Op::Proxy` に替える。
     **作る価値の見積もり**: 格子の点の数 ≥ 作るのに使う評価の回数 × 20 のときだけ作る（小さい図形・2D の数点の式で、作るほうが遅くならないように）
  4. `plot` の中、`build` の直後に、形を作る `Program`（陽関数・陰関数・媒介変数・2D の曲線）だけを差し替える。D7 のとおり、`eval_constant`・積分の中・区間演算では使わない
  5. 式の説明に D8 の 1 行。`?proxy=0`（`perf_options(false, …)`）で切れる。キャッシュの鍵（`plot.worker.ts` の `shapeKey`）に入れる
- **実装の土台**

  ```rust
  // core/src/proxy.rs
  pub fn build(f: &dyn Fn(f64) -> C, lo: f64, hi: f64, tol: f64, budget: usize) -> Option<Proxy> {
      // 区間の積み（深さ, lo, hi）。取り出して 17 点を評価 → 係数（DCT の式で直接。17 点なので O(n²) でよい）
      // 末尾 4 つ ≤ tol·scale なら piece、超えて深さ < 12 なら 2 つに割って積む、そうでなければ exact の piece
      // scale は全区間の標本の |f| の最大（最初に 33 点で見積もり、piece ごとに更新）
  }
  impl Proxy {
      #[inline] pub fn eval(&self, x: f64) -> Option<C> {
          // 区間を二分探索（等幅に割っていないので）。exact の区間・範囲の外は None
          // t = (2x − lo − hi) / (hi − lo)、クレンショー: b_k = c_k + 2t b_{k+1} − b_{k+2}
      }
  }
  // core/src/eval.rs
  impl Program { pub fn with_proxies(&self, boxes: &[Interval], points: usize) -> Program; }
  ```

  クレンショーの漸化式: $p(t) = \sum_{k=0}^{16} c_k T_k(t)$ を、$b_{17} = b_{18} = 0$、$b_k = c_k + 2t\,b_{k+1} - b_{k+2}$、$p(t) = c_0 + t\,b_1 - b_2$ で求める（$T_k$ はチェビシェフ多項式）。
- **やらないこと**: 複素数の引数（O4）、2 つ以上の引数が変わる式（`besselJ(n, x)` の $n$ もスライダーでなく変数のとき）、特殊関数の入れ子の内側（引数の区間が出ない）、区間演算への組み込み
- **完了の条件**（`core/tests/proxy.rs` とネイティブの時間）
  - 誤差: 1.2 の表 4 の 5 つの関数・範囲で、代理と元の関数の差が、値の最大の $10^{-12}$ 以下（2 万点）。`besselY(1, x)` の $x \in [10^{-6}, 30]$（端の対数の特異点）で、`exact` の区間が端だけにでき、差が同じ基準以下
  - 極を含む範囲: `gamma(x)`（$[-5, 5]$）で、極の近くが `exact` になり、極の上の点が元の関数と同じ値（NaN・無限大）
  - 時間: 1.2 の表 3 の `z = hypU(1, 2.5, x^2+y^2+0.1)`（546 ms）が **1/10 以下**、`z = erf(x) erf(y)`（148 ms）が **1/5 以下**、`y = besselY(1, x)`（150 ms）が 1/5 以下。代理の命令 1 回が 50 ns 以下（O0 の道具、WASM）
  - 形: 代理あり・なしで、頂点の座標の差の最大が表示範囲の $10^{-9}$ 以下（`shape_report` の欠け・余計が増えない）
  - 作る価値の見積もり: 2D の数点だけの呼び出し（`classify_rect` など）で代理を作らない
- **壊してはいけないもの**: 定数の値（`eval_constant` の答えは 1 ビットも変わらない）、`special_values.rs`・`special_accuracy.rs`・N10 の表、区間演算、`?proxy=0` の結果が今と同じ指紋
- **README**: 「近似が混ざる」の欄に D8 の文と、`?proxy=0` の使い方

### O4: 複素数の引数の代理（条件つき）

- **担当**: A（理由: 2 次元の補間で、極（$\Gamma$ の負の整数・$\zeta$ の 1）と分枝の切れ目をまたぐと大きく外れる）
- **発動の条件**: Q1 の後で、`z = abs(zeta(x + i y))`（今 101 ms）・`abs(gamma(x + i y)) - z = 0`（207 ms）の類が「遅い」と利用者から報告があるか、O0 の表で複素数の特殊関数の式がサンプルの遅い順の上位 5 つに入る
- **平たい説明**: 複素数の引数 $z = x + iy$ の関数を、画面の長方形をタイルに分けて、タイルごとに 2 次元のチェビシェフ補間（$17 \times 17$ 点）で置き換える
- **やること（要点）**: タイルの 4 隅と中の標本で $\lvert f\rvert$ の比が大きいタイル（極の近く）は `exact`。偏角が飛ぶ関数（`loggamma` の切れ目）は補間しない一覧に入れる。D6 の比べ直し（AAA と 2 次元チェビシェフ）を最初に行う
- **完了の条件**: `z = abs(zeta(x + i y))` が 1/5 以下の時間で、差が値の大きさの $10^{-10}$ 以下（極の近くの `exact` のタイルを除く）
- **README**: Q1 の文に複素数を足す

### Q7: 霧の歩幅可変

- **担当**: B（理由: シェーダーの 1 か所の歩き方を変えるだけだが、見た目が変わらないことを数で確かめる必要がある）
- **平たい説明**: 不等式の領域の霧は、視線に沿って同じ歩幅で 96〜256 歩進み、毎歩テクスチャを読む。霧の濃さは「境目の近く」でだけ変わり、
  境目から遠い外側は 0、深い内側は一定（1.1）。だから、境目から遠い所では大きく跳んでよい。テクスチャの値は境目までの距離の見積もりなので、跳べる距離もそこから分かる
- **触るファイル**: `web/src/lib/volumeMaterial.ts`（`uRegion` の分かれ）、`web/src/lib/Viewer.ts`（歩数の数え・`uAdaptive`）、`tools/fog-check/`（新規。SwiftShader で比べる）
- **やること**
  1. 歩幅: 今の点の値 $x$（格子の 1.5 マスを 1 とした距離）から、外側で $x > 4.5$ なら、次の点まで $(x - 4.5) \cdot u \cdot 0.8$（$u$ は 1 の長さ。0.8 は見積もりの余裕）だけ進む。
     内側で $x < -4.5$ なら、濃さ $0.35$ が一定なので、同じだけ進み、その区間の不透明度を $1 - e^{-k \cdot 0.35 \cdot \Delta t}$ で一度に足す。
     どちらも 1 歩は今の歩幅の 8 倍まで（距離の見積もりは下限ではないので、細い所を跳び越さないように）。境目の近くは今の歩幅のまま
  2. 歩数の上限 `MAX_STEPS` はそのまま（跳ぶ分だけ早く終わる）。`uAdaptive = 0` で今の歩き方（比べる用。`?fogstep=0`）
  3. 比べる道具 `tools/fog-check/`: ヘッドレスの Chromium で、領域の霧のサンプル（`x^2+y^2+z^2<4`・トーラスの内側・不等式 3 本の共通部分・薄い殻 `3.9<x^2+y^2+z^2<4`）を、今と歩幅可変で描き、
     (a) 画素ごとのテクスチャの読みの回数の平均（`uProbe` と同じ形で、色に回数を書いて読む）、(b) 2 枚の絵の差、を出す
- **実装の土台**

  ```glsl
  // volumeMaterial.ts の uRegion の分かれ（下書き）
  float t = tn + jitter * dt;
  for (int i = 0; i < MAX_STEPS; i++) {
    if (t >= tf) break;
    float x = sampleDistance(t);                   // 今の v / (1.5 · 1 マス)
    float step = dt;
    if (uAdaptive > 0.5 && abs(x) > 4.5) step = clamp((abs(x) - 4.5) * unit * 0.8, dt, 8.0 * dt);
    step = min(step, tf - t);
    // 濃さ dens(x) は今の式。外側の跳ぶ区間は dens ≈ 0、内側は 0.35 とみなして一度に足す
    accumulate(dens, step);
    t += step;
  }
  ```

- **やらないこと**: 値の色付け・伝達関数のボリューム（`uRegion = 0`）の歩き方、霧を標準にすること（D9）
- **完了の条件**
  - (a) テクスチャの読みの回数の平均が、4 つのサンプルの平均で **1/3 以下**（`plan-render-algorithms.md` 0.2 の「3 倍以上」）
  - (b) 2 枚の絵の差: 色の成分の差が 3/255 を超える画素が 1 % 以下、6/255 を超える画素が 0.1 % 以下。薄い殻が欠けない（殻の画素の数が今の 99 % 以上）
  - 実機（依頼者のデスクトップ）で `?bench=1&only=…` のコマの時間を前後で表にする
- **壊してはいけないもの**: 視点が箱の中に入ったときの裏の面の描き方、切り口、`uRegion = 0` のボリューム
- **README**: 霧の説明の「重い」の文を、測った時間に合わせて直す

### Q10: 動いている間の質の差を測り、利用者に見せる

- **担当**: B（理由: 測って並べるだけだが、どの場面が「質の低下」として目立つかを選ぶ判断が要る）
- **平たい説明**: 回している間は、間に合わないと画素密度を下げたり（ぼやける）、三角形の少ない版にしたり、光線の回数を減らしたりする（`motionQuality.ts`・`RAY_MOVING`）。
  それが実際に困るほどかを、数と絵で利用者に見せて決めてもらう（D10）
- **触るファイル**: `web/src/lib/bench.ts`（段階の記録）、`docs/visual/motion/`（絵の置き場）
- **やること**
  1. `?bench=1&set=samples` で、回している間の段階（0〜7）の分布と、光線の層の有無を、サンプルごとに記録する（何 % のコマが段階 0 か）。依頼者のデスクトップとスマートフォンで回してもらう
  2. 段階がいちばん下がったサンプル 3 つについて、止まったときと、動いている間の最も低い段階の絵（同じ視点）を並べた画像を作る
  3. 利用者に見せて「実害あり・なし」を決めてもらい、0 章に書く
- **完了の条件**: 記録の表と並べた絵が `docs/visual/motion/` にあり、利用者の判断が 0 章に書いてある
- **README**: 書かない

### O5: 光線の層の時間方向の再利用（Q10 で「実害あり」のときだけ）

- **担当**: A（理由: 前のコマの結果を今のコマへ写す（再投影）とき、隠れていた所が見え始める画素・面の縁・半透明で、古い色が残る誤りが出やすい）
- **平たい説明**: 回している間、前のコマで求めた「光線が面に当たった深さ」を今のコマの視点に写し、そこから光線を始める。光線の回数の上限を下げずに済む
- **やること（要点）**: 光線の層だけ（メッシュの面は今のまま）。深さを描画先のテクスチャ（1 チャンネルの浮動小数）に残し、次のコマの初めの $t$ の候補にする。
  候補の少し手前から進め、当たらなければ今の進め方に戻す（結果は今と同じ面。速くなるだけ）。止まったら使わない
- **完了の条件**: 回している間、光線の回数の上限を `RAY_STILL` のままにしても、コマの時間が今の `RAY_MOVING` 以下（依頼者の実機）。止まったときの絵が今と同じ（指紋。`npm run test:gpu` の形）
- **README**: 動いている間の段落に 1 行

### Q2: アフィン演算（数える → 条件つきで GLSL）

- **担当**: B（理由: Rust の基準実装に試作して数えるところまでが本体。GLSL は条件つき）
- **平たい説明**: 区間演算は、同じ変数が何度も出ると範囲が広がりすぎる（$x - x$ の範囲が 0 でなく $[-2, 2]$ になる、のような）。アフィン演算は「どの変数から来た幅か」を覚えるので打ち消せる。
  1.3 のとおり、ジャイロイドには効かず、ローマ曲面のような多項式で 2.5 倍の差が見込まれる
- **触るファイル**: `core/src/affine.rs`（新規）、`core/src/pixel.rs`（`ray_hit` に区間の代わりのアフィンの道）、`core/tests/pixel.rs`
- **やること**
  1. `Affine { c: f64, d: [f64; 3], e: f64 }`（中心・3 つの座標の係数・それ以外の幅）で、四則・整数乗・`sqrt`・`sin`・`cos`（1 次の近似 + 誤差の幅）。外向きの丸めは区間演算と同じ扱い
  2. `ray_hit` の箱の判定を、区間とアフィンの両方で数える（光線の区間の二分の回数・48×48 本の平均。P6-2 と同じ数え方）
  3. 式: ローマ曲面・スタイナー曲面・クンマー曲面・ジャイロイド・球・トーラス
- **完了の条件**: 6 つの式の二分の回数の表（区間 / アフィン）を 0 章に書く。**回数が半分以下の式があり、かつ** O2 の測定でその式の GPU の見積もり $T$ が依頼者のデスクトップで 50 ms を超える（＝遅くて困っている）なら、GLSL に移すステップを足す。そうでなければ採らない
- **README**: 書かない

### Q3: 2D の領域のタイルごとの刈り込み（発動の条件つき）

- **担当**: B
- **発動の条件**: O2 の後、依頼者の実機で 2D の領域（不等式の本数が多い式）の GPU の見積もり $T$ が 16 ms を超える例が出る。出なければ手を付けない（1.3）
- **やること（要点）**: `plan-render-algorithms.md` 7 章の P6-3 の記録のとおり、タイル（32×32）ごとに「効く $G_i$」のビットを区間演算で求めてテクスチャで渡し、GLSL で効かない項を飛ばす
- **完了の条件**: その例のコマの時間が 1/2 以下、絵が同じ（`npm run test:gpu` の形で画素ごとに比べる）

### Q4: カーソルの下の点を GPU の深度で（発動の条件つき）

- **担当**: B
- **発動の条件**: 三角形 100 万個を超える図形が、サンプル・利用者の報告で普通に出る（今は最大 13.5 万個・6 ms。1.3）
- **やること（要点）**: BVH ではなく、カーソルの 1 画素だけを深度のテクスチャに描いて読み、深度から座標を出す（作り直しが要らない）。光線の層（GPU の陰関数）にも同じ道が使える
- **完了の条件**: 三角形 100 万個で、カーソルの応答が 41 ms から 5 ms 以下。座標の差が今の交差と 1 画素以内

## 6. 危ない所

1. **温めない WASM の時間は信用できない**（1.2: 同じビルドで、最初に測った式だけ 1.5 倍ぶれた）。O0 の温めと、5 回の中央値を必ず使う
2. **全体の時間で判断する**: 式の計算が全体の 6〜30 % の式では、Q6・Q5 で式の計算を何倍にしても全体は少ししか縮まない（表 3）。1 点の時間だけで「採用」にしない
3. **JIT・SIMD で値が変わる罠**: `Math.sin` などの JavaScript の関数、FMA（`relaxed-simd`）、足し算の順序の入れ替え（コンパイラの `fast-math` に当たる最適化）は、結果を変える。指紋で必ず確かめる
4. **代理の誤差は保証が無い**: チェビシェフ補間の「末尾の係数が小さい」は見積もりで、区間の中の細い山を見落とすことがある。代理は形を作る点の計算だけ（D7）、区間演算・数値の表示には使わない。
   引数の範囲は区間演算で求めるので、表示範囲の外の値で代理を使うことは無い（範囲の外は `None` で元の関数）
5. **代理を作る時間が、点の計算より長くなる**: 小さい図形・2D の数点では、作るのに使う評価（数百回）のほうが重い。作る価値の見積もり（Q1 のやること 3）を必ず入れる
6. **自動の判定の時間は SwiftShader では測れない**: こちらのヘッドレスの Chromium はソフトウェアなので、O2 で確かめられるのは「ソフトウェア → メッシュ」の道だけ。速い GPU の道は依頼者の実機で見てもらう
7. **測るための描画で固まる**: 描く先（画面 / 描画先のテクスチャ）・光源の数を変えると、three.js がシェーダーを作り直して画面のスレッドが止まる（`abortRatio` のコメント。Windows の Chrome で 2〜4 秒）。O2 の測定は今の `abortRatio` と同じく、画面の左下・同じシーンで行う
8. **打ち切りの数えは光を当てない描画**（`uProbe = 1`）なので、時間の見積もりは光の計算の分だけ軽く出る。O2 では光あり（`uProbe = 0`）で時間を測り、打ち切りは別の 1 回で数える（描画は 5 回）。それでも重すぎる端末が出たら、時間の 2 回を 1 回に減らす
9. **2 つの WASM のビルドで接着コードが違うことがある**: wasm-bindgen の版・機能で、import の名前が変わると、片方の `.wasm` をもう片方の接着コードで読めない。Q6 のやること 2 で `diff` を必ず取る
10. **霧の距離の見積もりは下限ではない**: $G / \lVert\nabla G\rVert$ は、$G$ が急に曲がる所で実際の距離より大きくなる。Q7 は余裕 0.8 と、1 歩 8 倍までの上限で跳び越しを防ぐ。薄い殻のサンプルで必ず確かめる

## 7. ほかの計画との関係

- **`plan-render-algorithms.md`**: 0.2 の Q1〜Q7・Q10 の指示書がこの文書。Q8（LIC・FTLE）は文書 4（`plan-visual-extras.md` の B2〜B4）、Q9（`max_t` の分枝限定）は**この文書に入れない**（P1-7 で定数になる場合は済み。残りは精度の話で性能ではない）。
  各ステップの結果は、この文書の 0 章と、`plan-render-algorithms.md` 7 章の両方に書く。5 章の守ること 3（WASM の大きさ）の数は O0 で直す
- **`plan-auto-render.md`**: 3 章の決めることは D1〜D3 で決まった。指示書は O1・O2
- **`plan-math-notation.md`**: N10 はこの文書。**N1（勾配を記号微分で）・N5（射影した輪郭）・N6（端のふた）・N7（正しい半透明）は入れない**（性能ではなく形・見た目の話。7.3 のまま）
- **文書 2（`plan-analysis.md`）**: 近似値の表示（V）は `eval_constant` を使うので、Q1 の代理の影響を受けない（D7）
- **文書 3（`plan-fractal-hyperbolic.md`）**: 脱出時間の図は GPU（12 章の G 系）で、この文書の SIMD・JIT とは別の道。深い拡大の摂動法（K）も別
- **文書 4（`plan-visual-extras.md`）**: D6「吸引域・FTLE を GPU にするかは文書 5 で測ってから」は、B1（点をまとめて流す解法）ができた後、その時間を O0 の道具で測って決める。この文書では GPU の版は作らない
- **将来の別計画**: メッシュの段（表 3 の残りの 70〜94 %）を速くすること。O0 で部分ごとの時間を埋めてから、重い部分を見て計画を立てる。厳密な誤差つきの特殊関数（ball 演算。A21）も別計画

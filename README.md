# graphploter

ブラウザで動く 3D グラフソフト。目標は「Desmos くらい使いやすく、多次元の媒介変数表示・陰関数・特殊関数・複素数を 3 次元空間に自由に描ける」こと。

現在できること（第1段階）:

- `z = f(x, y)` の曲面を、滑らかな陰影付きで表示
- 計算はすべて複素数で行う（例: `z = abs(gamma(x + iy))`）
- 特殊関数: ガンマ関数 `gamma`
- 式の追加・削除・色変更・表示切り替え、表示範囲と細かさの変更

## 構成

```
graphploter/
├── core/   … Rust で書いた計算エンジン（WebAssembly に変換してブラウザで動かす）
│   └── src/
│       ├── parser.rs     数式の文字列 → 構文木
│       ├── eval.rs       構文木 → 命令列、たくさんの点でまとめて計算（複素数）
│       ├── functions.rs  組み込み関数の一覧
│       ├── special.rs    特殊関数（ガンマ関数など）
│       ├── mesh.rs       曲面のメッシュ（三角形の集まり）を作る
│       └── lib.rs        JavaScript から呼ぶ入口
└── web/    … 画面（Svelte + TypeScript + Three.js）
    └── src/
        ├── App.svelte               画面全体
        └── lib/
            ├── ExpressionItem.svelte  式の一覧の1行
            ├── Viewer.ts              3D 表示（Three.js）
            ├── plot.worker.ts         計算担当（別スレッドで WASM を呼ぶ）
            ├── plotClient.ts          計算担当への依頼窓口
            └── types.ts               データの型
```

処理の流れ:

```
式を入力 → (Web Worker) Rust/WASM が解析・計算・メッシュ生成 → Three.js が描画
```

## 動かし方

### 1. 必要なものを入れる（最初の1回だけ）

| ツール | 用途 | 入れ方 |
|---|---|---|
| Node.js (v20 以上) | 画面側の開発 | https://nodejs.org から LTS 版 |
| Rust | 計算エンジン | https://rustup.rs の手順どおり |
| WASM 用の部品 | Rust → WebAssembly | 下のコマンド |

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

### 2. 起動する

```sh
cd web
npm install      # 最初の1回だけ（部品のダウンロード）
npm run dev      # Rust のビルド → 開発用サーバーの起動
```

表示された URL（たいてい http://localhost:5173 ）をブラウザで開く。

- **画面側（`web/src`）を書き換えたとき**: 保存するだけでブラウザに自動で反映される
- **Rust 側（`core/src`）を書き換えたとき**: `npm run dev` を止めて（Ctrl + C）もう一度実行する

### よく使うコマンド

| 場所 | コマンド | 内容 |
|---|---|---|
| `core/` | `cargo test` | Rust 側のテストを実行 |
| `web/` | `npm run dev` | 開発用サーバーを起動 |
| `web/` | `npm run check` | 画面側の型チェック |
| `web/` | `npm run build` | 公開用のファイルを `web/dist/` に作る |

## 数式の書き方

- `z = x^2 - y^2`（`z =` は省略可）
- 掛け算の `*` は省略可: `2xy`, `x sin(y)`, `(x+1)(x-1)`
- 括弧なしの関数: `sin x`, `sin^2 x`（= `(sin x)^2`）
- 定数: `pi`（`π`）, `e`, `i`（虚数単位）
- 関数: `sin cos tan arcsin arccos arctan sinh cosh tanh exp ln log sqrt abs Re Im conj arg gamma`
- 結果が実数にならない点（例: `x < 0` での `sqrt(x)`）は描かれない

## 今後の予定

1. ~~土台: `z = f(x, y)` の表示~~ ✅
2. 陰関数 `F(x, y, z) = 0`（区間演算 + マーチングキューブ法）
3. 媒介変数表示の曲線・曲面
4. 4 次元以上: 変数を軸・色に割り当てる / 定義域の範囲で立体を作る
5. 複素関数の表示方法（定義域の色分けなど）と特殊関数の追加
6. 2D 表示への切り替え、数式入力を MathLive にする

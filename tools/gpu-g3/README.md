# G3: 命令列から作った GLSL の答え合わせ（計画 12 章）

`check.cjs` は、計算エンジンの `Program::to_glsl`（`core/src/eval/glsl.rs`）が作る 3 つの関数を、ヘッドレスの Chromium の WebGL2 で計算し、Rust の結果と比べる。

```text
cd web && npm run test:gpu
```

1. `cargo run --release --example glsl_fixture` が、65 式の GLSL と、乱数の点・箱（1000 個ずつ）での Rust の結果を JSON に書き出す
2. 式ごとに、点を浮動小数点数のテクスチャで渡し、`F`（値）・`Fd`（二重数）・`Fi`（飾り付きの区間）の結果を浮動小数点数の描画先から読む
3. 値は相対誤差 1e-4、勾配（記号微分の式と比べる）は 1e-3 以内か、区間は箱の中の点の Rust の値を全部含むか、飾りが Def 以上なら箱の中に定義されない点が無いかを数え、表にする

「（参考）」の式は 32 ビットで誤差が大きくなりやすい式（計画 12.6 の 2）で、表に出すだけで失敗にはしない。

GPU とブラウザが要るので CI では回さない。この環境（GPU なし）では SwiftShader で動く。実機の GPU で回すときは、Playwright の Chromium を `headless: false` にするなどして、ハードウェアの WebGL を使う。

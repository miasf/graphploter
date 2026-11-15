# G5: 3D の陰関数を光線で描く GLSL の検査（計画 12 章）

`check.mjs` は、光線の計算（`web/src/lib/raycastGlsl.ts` の `RAY_GLSL`）と計算エンジンが作った式の GLSL を、ブラウザの WebGL2 で動かし、
計算エンジンの基準の実装（`core/src/pixel.rs` の `ray_hit`）と画素ごとに突き合わせる。

```text
cd web && npm run test:gpu                                   （G3・G4 の検査の後に回る）
node --experimental-strip-types tools/gpu-g5/check.mjs --speed  （G0 の 6 式を 1280 × 720・1920 × 1080 で描く時間も測る）
```

1. `cargo run --release --example ray_fixture` が、G2 の物差し（`pixel_report`）と同じカメラで 160 × 120 本の光線を基準の実装で飛ばした結果と、25 式の GLSL を書き出す
2. 同じ光線を GPU で飛ばし、当たる・当たらないが違う画素の割合（完了の条件 1 % 以下）、当たった深さの差の 95 パーセンタイル（画素）、GPU が打ち切った画素の割合を出す

環境変数: `G5_MAXITER`（繰り返しの上限。既定 512）、`CHROME_CHANNEL=chrome`（入っている Chrome で、本物の GPU を使う）、
`PLAYWRIGHT_MODULE`・`PLAYWRIGHT_FROM`（playwright の代わりに playwright-core など）、`CARGO`（例: `mise exec -- cargo`）。

Windows の Chrome（Direct3D に変換される）で見つかったこと: 分かれ道の中で配列の変数の番号に書いた値が残らないことがある。
光線のスタックへの書き込みは、ループの最後の 1 か所にまとめてある（`raycastGlsl.ts` の説明）。

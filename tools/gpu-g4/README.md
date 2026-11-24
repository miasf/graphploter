# G4: 2D 表示の画素ごとの描き方の画像の検査（計画 12 章）

`check.mjs` は、計算エンジンが作った 2D の GLSL（`core/src/gpu2d.rs`。`cargo run --release --example gpu2d_fixture`）と、
画面の材質の判定と塗り（`web/src/lib/gpu2dMaterial.ts` の `GPU2D_FRAGMENT`）を、ヘッドレスの Chromium の WebGL2 で 400 × 400 画素に描き、
次のことを画素で確かめる。

- `1/x = y`: 極に縦の線が出ない
- `floor(x) = y`: 段差の縦の線が出ない
- `(x² + y² − 1)² = 0`: 符号の変わらない零点の円が、ふつうの線の太さで出る
- `sin(1/x) = y`: 原点の近くで落ちない
- 不等式の領域（1 本・2 本）と、不等式で切った曲線
- 厳密な不等式で空集合になる領域（`x > 0, x <= 0`・`x^2+y^2 < 0`・`y > x^2, y < x^2`・`x > 0.3, x <= 0.3`）は何も塗らない。対照の `x >= 0, x <= 0` は直線

```text
cd web && npm run test:gpu      （G3 の答え合わせの後に回る）
```

表の時間は SwiftShader（CPU で動く WebGL）の参考値で、実機の GPU の時間ではない。

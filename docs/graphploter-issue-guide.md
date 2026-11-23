# graphploter の Issue 修正ガイド

2026年11月23日確認。全99件（未解決97件、クローズ済み2件）。分類は、Issue の再現例と期待動作に基づく委任の目安。着手時に状態と最新コメントを確認する。 [GitHub Issues](https://github.com/miasf/graphploter/issues)

| 任せ方 | 件数 |
| --- | ---: |
| 直接修正を依頼 | 12 |
| 短い計画を立ててそのまま実装 | 26 |
| 計画と回帰条件をレビューしてから実装 | 40 |
| 期待する仕様を決めてから実装 | 11 |
| Issue 内の項目を分けて委任 | 6 |
| 進捗管理用のまとめ | 2 |
| クローズ済み | 2 |

## AIへの依頼文

**直接修正**

> Issue #番号を修正して。記載の再現例を修正前後で実行し、正常な対照例も確認して。結果と未検証の範囲を報告して。

**短い計画のあとに実装**

> Issue #番号を再現し、原因・変更範囲・回帰条件を短く示してから、そのまま実装して。通常の操作経路でも結果を確認して。

**レビューを挟む場合**

> Issue #番号を再現し、原因、修正方針、守るべき既存動作、回帰条件を示して。今回は計画まで。

数値や描画の修正では、失敗例と同じ変更で壊れうる正常例を対にする。たとえば広義積分では発散する例と収束する特異積分、接点描画では真の接点と近接しても交わらない例を比較する。 [#109](https://github.com/miasf/graphploter/issues/109) · [#119](https://github.com/miasf/graphploter/issues/119)

## 直接修正を依頼（12件）

記載の再現例を修正前後で確認し、正常な対照例も検証する。

- [#125 数列の極限の見出しに入力した添字付きの式を表示する](https://github.com/miasf/graphploter/issues/125)
- [#132 初期値で右辺が未定義の常微分方程式を画面で知らせる](https://github.com/miasf/graphploter/issues/132)
- [#157 旧版のグラフ移行で一覧への書き込みに失敗すると元データが消える](https://github.com/miasf/graphploter/issues/157)
- [#173 フルヴィッツゼータの負整数でベルヌーイ多項式との値が一致しない](https://github.com/miasf/graphploter/issues/173)
- [#181 LaTeX から貼り付けた直立体の微分記号で積分を読めるようにする](https://github.com/miasf/graphploter/issues/181)
- [#187 LaTeX の数式スタイル指定があると、同じ式でも計算用に変換できない](https://github.com/miasf/graphploter/issues/187)
- [#188 指数の直後で Backspace を1回押しても数字が消えない](https://github.com/miasf/graphploter/issues/188)
- [#193 微小区間の定積分を途中まで表示すると、上限がゼロに丸められる](https://github.com/miasf/graphploter/issues/193)
- [#195 同じ定積分でも LaTeX の積分記号によって近似を選べない](https://github.com/miasf/graphploter/issues/195)
- [#197 累次積分の内側を括弧で囲むだけで積分図を生成できなくなる](https://github.com/miasf/graphploter/issues/197)
- [#201 条件式を編集中、再生中のスライダーが未完成の条件を読む](https://github.com/miasf/graphploter/issues/201)
- [#202 フォルダの一括非表示後に積分図を閉じると元の式が再表示される](https://github.com/miasf/graphploter/issues/202)

## 短い計画を立ててそのまま実装（26件）

原因、変更範囲、回帰条件を短く示してから実装する。

- [#114 重積分の領域で ∧・∨ が拒否される](https://github.com/miasf/graphploter/issues/114)
- [#118 WebGL の初期化失敗で数式計算と共有グラフの読み込みまで止まる](https://github.com/miasf/graphploter/issues/118)
- [#130 線積分の普通の括弧で dx + dy の第2項が落ちる](https://github.com/miasf/graphploter/issues/130)
- [#133 リーマン和・シンプソン則の設定がグラフを開き直すと消える](https://github.com/miasf/graphploter/issues/133)
- [#144 球座標の φ=π で負の z 軸の半直線が消える](https://github.com/miasf/graphploter/issues/144)
- [#146 整数の集合で開区間の端の整数が含まれる](https://github.com/miasf/graphploter/issues/146)
- [#147 整数点を x 軸に並べる集合が空になる](https://github.com/miasf/graphploter/issues/147)
- [#148 ベータ関数と正則化不完全ベータが通常の正の整数で計算できない](https://github.com/miasf/graphploter/issues/148)
- [#149 ゼータ関数の自明な零点が −40 で 7.52、−60 で約 3.67×10¹⁹ になる](https://github.com/miasf/graphploter/issues/149)
- [#160 注目点を有効にすると √x/√x の描画計算が長時間終わらない](https://github.com/miasf/graphploter/issues/160)
- [#162 z=0 と重なる図形が 2D の「切り口」で消える](https://github.com/miasf/graphploter/issues/162)
- [#163 総和・総積の添字で教科書に多い i を使う](https://github.com/miasf/graphploter/issues/163)
- [#164 逆三角関数と底付き対数の教科書表記を入力できるようにする](https://github.com/miasf/graphploter/issues/164)
- [#171 ルジャンドル陪関数と球面調和関数が北極の値を返さない](https://github.com/miasf/graphploter/issues/171)
- [#177 小さいスライダー値を保存して開き直すと別の数に変わる](https://github.com/miasf/graphploter/issues/177)
- [#179 上下限を逆にした定積分で値は正しいが近似図が空になる](https://github.com/miasf/graphploter/issues/179)
- [#180 直書きの積分範囲と数列の項の表の表示を保存できるようにする](https://github.com/miasf/graphploter/issues/180)
- [#186 保存容量が残り少ないと、別のグラフの名前変更が一覧にだけ反映される](https://github.com/miasf/graphploter/issues/186)
- [#189 積分図を表示した状態を復元すると、図を閉じても元の式が再表示されない](https://github.com/miasf/graphploter/issues/189)
- [#190 被積分関数の定義を編集すると、積分図の数値が古いままになる](https://github.com/miasf/graphploter/issues/190)
- [#191 ユーザー定義関数をスライダーの範囲と刻みにも使えるようにする](https://github.com/miasf/graphploter/issues/191)
- [#192 スライダーの不等式制約が、式を評価できない点への移動を許す](https://github.com/miasf/graphploter/issues/192)
- [#196 重積分の領域を逆向きの不等号で書くと進み具合を操作できない](https://github.com/miasf/graphploter/issues/196)
- [#199 積分の上限が添字付きパラメータだと座標と誤判定して図を拒否する](https://github.com/miasf/graphploter/issues/199)
- [#200 場合分けを含むスライダー条件で、内側の不等号まで比較の区切りになる](https://github.com/miasf/graphploter/issues/200)
- [#208 関数定義が媒介変数・高次元断面・カスタム座標の範囲に渡らない](https://github.com/miasf/graphploter/issues/208)

## 計画と回帰条件をレビューしてから実装（40件）

アルゴリズムや境界条件への影響を確認するため、計画段階でレビューする。

- [#109 有限区間の広義積分で発散を有限値として描く](https://github.com/miasf/graphploter/issues/109)
- [#110 有限区間の内部にある可積分特異点で積分値が約10%ずれる](https://github.com/miasf/graphploter/issues/110)
- [#111 無限区間の振動積分で sin と cos の発散を有限値として扱う](https://github.com/miasf/graphploter/issues/111)
- [#112 教科書の Dirichlet 積分 sin(t)/t が端点の穴で収束エラーになる](https://github.com/miasf/graphploter/issues/112)
- [#113 変数上限の積分グラフが固定上限の計算と大きく食い違う](https://github.com/miasf/graphploter/issues/113)
- [#115 小振幅の振動する関数・数列に有限極限を表示する](https://github.com/miasf/graphploter/issues/115)
- [#116 Taylor が |x| の原点で存在しない多項式を表示する](https://github.com/miasf/graphploter/issues/116)
- [#117 crit が x⁶・x⁸ の原点を「極値でない」と誤分類する](https://github.com/miasf/graphploter/issues/117)
- [#119 連立方程式で接しているだけの交点を表示する](https://github.com/miasf/graphploter/issues/119)
- [#120 陰関数の積で曲面と線・曲線と点が混在する解を描く](https://github.com/miasf/graphploter/issues/120)
- [#121 集合の和で領域と異なる次元の成分が消える](https://github.com/miasf/graphploter/issues/121)
- [#122 複素陰関数で実部と虚部が従属すると球面が消える](https://github.com/miasf/graphploter/issues/122)
- [#123 桁落ちする整数行列の行列式が −1 から 0 になる](https://github.com/miasf/graphploter/issues/123)
- [#124 ほぼ特異な行列で零空間の次元を誤表示する](https://github.com/miasf/graphploter/issues/124)
- [#131 常微分方程式の初期条件が表示範囲を移動するだけで変わる](https://github.com/miasf/graphploter/issues/131)
- [#134 4次元の超球が断面に一点で接するとメッシュから点が消える](https://github.com/miasf/graphploter/issues/134)
- [#138 別タブで削除したグラフが古いタブの自動保存で復活する](https://github.com/miasf/graphploter/issues/138)
- [#139 2D の厳密不等式が空集合でも偽の境界を描く](https://github.com/miasf/graphploter/issues/139)
- [#140 2D の陰関数で絶対値と平方根を使うと同じ円が欠ける](https://github.com/miasf/graphploter/issues/140)
- [#141 ほぼ接する円と直線の連立で 2 個ある交点が消える](https://github.com/miasf/graphploter/issues/141)
- [#142 2D で画面の格子より小さい円を見失う](https://github.com/miasf/graphploter/issues/142)
- [#143 薄い球殻のメッシュから外側の境界が落ちる](https://github.com/miasf/graphploter/issues/143)
- [#145 量化記号 ∃・∀ で普通の区間括弧と論理結合を受け付ける](https://github.com/miasf/graphploter/issues/145)
- [#150 ゼータ関数が臨界線の虚部 200 で有限値を返さない](https://github.com/miasf/graphploter/issues/150)
- [#151 複素エアリー関数 Ai・Bi が虚軸付近で桁違いの値を返す](https://github.com/miasf/graphploter/issues/151)
- [#152 高い次数のベッセル関数 J・I の小さい値が丸め誤差に埋もれる](https://github.com/miasf/graphploter/issues/152)
- [#153 複素引数で高次数のベッセル関数 Y・K が極端に発散する](https://github.com/miasf/graphploter/issues/153)
- [#154 負の引数の不完全ガンマ関数で指数関数との恒等式が壊れる](https://github.com/miasf/graphploter/issues/154)
- [#155 一般化指数積分 E₂₀(100) が約 18 万倍になる](https://github.com/miasf/graphploter/issues/155)
- [#156 複素母数を使うヤコビ sn の値が参照値から大きく外れる](https://github.com/miasf/graphploter/issues/156)
- [#158 場合分けで1点だけ定義した値が格子に乗らないと消える](https://github.com/miasf/graphploter/issues/158)
- [#159 幅 0.01 の矩形パルスの定積分が 0 になる](https://github.com/miasf/graphploter/issues/159)
- [#161 絶対値や床関数の導関数が存在しない点を描かないようにする](https://github.com/miasf/graphploter/issues/161)
- [#169 2D GPU で陰関数を小さい非零定数倍にすると円の外まで全面が塗られる](https://github.com/miasf/graphploter/issues/169)
- [#170 ヤコビ多項式が次数100でルジャンドル多項式との恒等式を破る](https://github.com/miasf/graphploter/issues/170)
- [#172 高次の球面調和関数が正規化の途中で 0 や未定義になる](https://github.com/miasf/graphploter/issues/172)
- [#174 複素振幅の第3種不完全楕円積分 Π が同じ経路の積分と合わない](https://github.com/miasf/graphploter/issues/174)
- [#175 床関数を使った整数条件で境界にだけある正しい解が消える](https://github.com/miasf/graphploter/issues/175)
- [#178 リーマン和とシンプソン則が利用者の変数名を内部の添字に置き換える](https://github.com/miasf/graphploter/issues/178)
- [#194 変数上限の積分を描くと、自由な上限が積分変数に束縛される](https://github.com/miasf/graphploter/issues/194)

## 期待する仕様を決めてから実装（11件）

受け付ける入力、表示する結果、既存動作との互換性を決める。

- [#126 座標を含む無限級数で収束域と部分和を区別して可視化する](https://github.com/miasf/graphploter/issues/126)
- [#127 多変数の極限を経路比較と ε–δ の図で学べるようにする](https://github.com/miasf/graphploter/issues/127)
- [#128 開集合と閉集合の境界を2Dで見分ける表示を追加する](https://github.com/miasf/graphploter/issues/128)
- [#129 変数変換の積分で領域の面積とパラメータの被覆回数を見分ける](https://github.com/miasf/graphploter/issues/129)
- [#135 複素数の学習で i を追記せずに負の実数の平方根や対数を描く](https://github.com/miasf/graphploter/issues/135)
- [#136 4次元の立体を3次元へ射影して外側の境界だけを描く](https://github.com/miasf/graphploter/issues/136)
- [#165 奇数根を負の実数まで描ける実数の立方根を用意する](https://github.com/miasf/graphploter/issues/165)
- [#166 曲線積分と面積分を並べてグリーン・ストークスの定理を検証できる教材を追加する](https://github.com/miasf/graphploter/issues/166)
- [#167 定積分の上端を動かしたときの面積と積分関数を同時に見る](https://github.com/miasf/graphploter/issues/167)
- [#168 常微分方程式の初期条件・境界条件を教科書の式から入力する](https://github.com/miasf/graphploter/issues/168)
- [#198 1行に複数の積分があるとき、描画・近似の対象を選べるようにする](https://github.com/miasf/graphploter/issues/198)

## Issue 内の項目を分けて委任（6件）

Issue を進捗の記録に使い、実装は原因や回帰条件ごとに分ける。

- 関数ごとで分ける。 [#203 特殊関数の極端な引数で、対称性・実数値・相対精度が崩れる](https://github.com/miasf/graphploter/issues/203)
- 総和・積分・ノルム・数値ランクなど演算ごとで分ける。 [#204 総和・内積・積分で桁落ちし、ノルムや総積で中間値だけがオーバーフローする](https://github.com/miasf/graphploter/issues/204)
- 微分・極限・差分で分ける。 [#205 微分・極限・差分を合成すると、同値な式や尺度で結果が変わる](https://github.com/miasf/graphploter/issues/205)
- 偽の零点・切り取りの接点・従属な連立で分ける。 [#206 陰関数のメッシュで解のない輪が出たり、接点・従属な連立の解が消えたりする](https://github.com/miasf/graphploter/issues/206)
- 垂直な帯・線積分の構文・積分変数のスコープで分ける。 [#207 積分の値は正しくても、線積分・累次積分・近似の図が生成できない](https://github.com/miasf/graphploter/issues/207)
- 行列・スライダー・共有復元で分ける。 [#209 行列の貼り付け、極小刻み、部分的に壊れた共有データを扱うと状態が変わる](https://github.com/miasf/graphploter/issues/209)

## 進捗管理用のまとめ（2件）

一覧内の個別 Issue を担当単位にする。

- [#137 大学数学の可視化を検証した結果と改善項目](https://github.com/miasf/graphploter/issues/137)
- [#176 大学数学の可視化を再検証した結果と追加の改善項目](https://github.com/miasf/graphploter/issues/176)

## クローズ済み（2件）

修正依頼の対象外。

- [#182 極限の lim が手入力で変換されず、数式キーボードの「微積」にもない](https://github.com/miasf/graphploter/issues/182)
- [#184 2D の移動や 3D の回転で目盛りの数字が点滅する](https://github.com/miasf/graphploter/issues/184)


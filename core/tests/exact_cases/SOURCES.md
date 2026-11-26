# 例題の表の出典とライセンス

`*.tsv` は `tools/exact/cases.py` が作る（手で直さない）。答えは SymPy 1.13.3 と mpmath で 30 桁まで確かめてある
（不定積分は、答えを微分して中身と 3 点で一致することを確かめる。「存在しない」の行は確かめない）。

| 出典 | 意味 | ライセンス |
|---|---|---|
| 自作 | この表のために作った問題（教科書によくある型の数を変えたものを含む） | このリポジトリと同じ |

外から写した行を足すときは、出典の列にその名前を書き、下にライセンスの文を残す。

- SymPy のテスト（`sympy/*/tests/`）: BSD-3（Copyright (c) 2006-2023 SymPy Development Team）
- Rubi の `MathematicaSyntaxTestSuite`: MIT

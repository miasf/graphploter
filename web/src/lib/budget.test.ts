// budget.ts のテスト。実行: npm test
import { BUDGET_MS, Budget } from './budget.ts'

let failures = 0
function check(name: string, got: unknown, want: unknown) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    failures++
    console.log(`NG  ${name}\n    結果: ${JSON.stringify(got)}\n    期待: ${JSON.stringify(want)}`)
  }
}

// まだ計算していない式は 0.5 から
check('未計算は 0.5', new Budget().scaleFor(1, { res: 150, ires: 150 }), 0.5)

// (a) 細かい計算が 3 ms の式は倍率 1
{
  const b = new Budget()
  b.record(1, 3, { res: 150, ires: 150 }, 'surface', false)
  check('(a) 3 ms の曲面は 1', b.scaleFor(1, { res: 150, ires: 150 }), 1)
}

// (b) 陰関数で 90 ms なら 0.5（90 × 0.5³ = 11.25 ≤ 12）
{
  const b = new Budget()
  b.record(1, 90, { res: 80, ires: 80 }, 'volume', false)
  check('(b) 90 ms の陰関数は 0.5', b.scaleFor(1, { res: 80, ires: 80 }), 0.5)
  // 半分の細かさで測った時間からも同じ見積もり
  const b2 = new Budget()
  b2.record(1, 90 / 8, { res: 40, ires: 40 }, 'volume', false)
  check('(b) 半分の細かさで測っても 0.5', b2.scaleFor(1, { res: 80, ires: 80 }), 0.5)
  check('(b) 細かさ 200 の見積もり', Math.round(b2.estimate(1, { res: 200, ires: 200 })!), Math.round(90 * 2.5 ** 3))
}

// (c) 時間が ±15% 揺れても、倍率が毎回は変わらない（再生中の流れをまねる）
{
  const b = new Budget()
  const n = 80
  b.record(1, 90, { res: n, ires: n }, 'volume', false)
  let s = b.scaleFor(1, { res: n, ires: n })
  let changes = 0
  for (let k = 0; k < 40; k++) {
    const full = 90 * (1 + (k % 2 === 0 ? 0.15 : -0.15) * ((k * 7) % 3 === 0 ? 1 : 0.6))
    const used = Math.round(n * s)
    b.record(1, full * (used / n) ** 3, { res: used, ires: used }, 'volume', false)
    const next = b.scaleFor(1, { res: n, ires: n })
    if (next !== s) changes++
    s = next
  }
  check('(c) 40 回のうち倍率が変わったのは 2 回以下', changes <= 2, true)
  check('(c) 持ち時間に収まる倍率のまま', s, 0.5)
}

// (d) 射影し直しだけの返事は、見積もりを変えない
{
  const b = new Budget()
  b.record(1, 90, { res: 80, ires: 80 }, 'volume', false)
  b.record(1, 0.5, { res: 80, ires: 80 }, 'volume', true)
  check('(d) reprojected は数えない', Math.round(b.estimate(1, { res: 80, ires: 80 })!), 90)
}

// 上げるのは、1つ上の倍率でも 0.8B に収まるときだけ
{
  const b = new Budget()
  b.record(1, 400, { res: 80, ires: 80 }, 'volume', false)
  check('重い式は小さい倍率', b.scaleFor(1, { res: 80, ires: 80 }), 0.25)
  b.record(1, 30, { res: 80, ires: 80 }, 'volume', false) // 2倍以上変わったので置き換え
  check('軽くなったら1段ずつ上げる', b.scaleFor(1, { res: 80, ires: 80 }), 0.35)
  check('もう1段', b.scaleFor(1, { res: 80, ires: 80 }), 0.5)
  check('0.75 は 30 × 0.42 > 0.8B なので上げない', 30 * 0.75 ** 3 > 0.8 * BUDGET_MS && b.scaleFor(1, { res: 80, ires: 80 }) === 0.5, true)
}

// 2段階の表示: 細かい計算が 120 ms を超えそうなら、40 ms 前後の倍率で先に出す
{
  const b = new Budget()
  check('未計算は1回で', b.previewScale(1, { res: 80, ires: 80 }), null)
  b.record(1, 60, { res: 80, ires: 80 }, 'volume', false)
  check('60 ms は1回で', b.previewScale(1, { res: 80, ires: 80 }), null)
  b.record(1, 60 / 8, { res: 40, ires: 40 }, 'volume', false)
  check('細かさ 200（見積もり 940 ms）は先に粗い版', b.previewScale(1, { res: 200, ires: 200 }), 0.35)
  b.record(2, 500, { res: 150, ires: 150 }, 'curve', false)
  check('曲線は1回で', b.previewScale(2, { res: 150, ires: 150 }), null)
}

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('budget.ts: すべて成功')

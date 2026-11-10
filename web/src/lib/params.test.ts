// params.ts のテスト。実行: npm test
import { fitRotation, formatNumber, pairs } from './params.ts'

let failures = 0
function check(name: string, got: unknown, want: unknown) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    failures++
    console.log(`NG  ${name}\n    結果: ${JSON.stringify(got)}\n    期待: ${JSON.stringify(want)}`)
  }
}

// スライダーの値を式に書く形
check('刻み 0.25 の 0.25', formatNumber(0.25, 0.25), '0.25')
check('刻み 0.25 の 0.75', formatNumber(0.75, 0.25), '0.75')
check('最小値 0.05・刻み 0.1 の 0.15', formatNumber(0.15, 0.1, 0.05), '0.15')
check('丸め誤差を落とす', formatNumber(0.30000000000000004, 0.1), '0.3')
check('最小値 2.5・刻み 5 の 7.5', formatNumber(7.5, 5, 2.5), '7.5')
check('刻みより細かい値は刻みの桁まで', formatNumber(Math.PI, 0.01), '3.14')
check('-0 は 0', formatNumber(-0.0001, 0.1), '0')

// 保存データの回転角を、今の並び（5次元の 10 個）にそろえる
const want10 = pairs(5).map(([i, j]) => (i === 0 && j === 2 ? 30 : i === 1 && j === 2 ? 45 : 0))
check('古い3次元の保存データ（3個）', fitRotation([0, 30, 45], [1, 2, 3], 3), { rotation: want10, center: [1, 2, 3, 0, 0] })
check('今の形はそのまま', fitRotation(want10, [1, 2, 3, 0, 0], 3), { rotation: want10, center: [1, 2, 3, 0, 0] })
check('壊れた値は 0', fitRotation(null, ['a'], 4).rotation.length, 10)
check('6次元', fitRotation([], [], 6).rotation.length, 15)

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('params.ts: すべて成功')

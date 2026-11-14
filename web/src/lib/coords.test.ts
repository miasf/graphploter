// coords.ts のテスト。実行: npm test
import { mixPositions, vertexNormals, weldGroups, weldNormals } from './coords.ts'

let failures = 0
function check(name: string, got: unknown, want: unknown) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    failures++
    console.log(`NG  ${name}\n    結果: ${JSON.stringify(got)}\n    期待: ${JSON.stringify(want)}`)
  }
}
/** 小数を丸めて比べる（法線の √2 など） */
const round = (v: ArrayLike<number>) => Array.from(v, (x) => Math.round(x * 1e6) / 1e6)

// mixPositions: s = 0 で q、s = 1 で target、途中は 1 次式。NaN の区切りを保つ
{
  const q = new Float32Array([0, 0, 0, NaN, NaN, NaN, 1, 2, 3])
  const t = new Float32Array([2, 4, 6, NaN, NaN, NaN, 3, 2, 1])
  check('s = 0 は q そのもの', mixPositions(q, t, 0) === q, true)
  check('s = 1 は target そのもの', mixPositions(q, t, 1) === t, true)
  const m = mixPositions(q, t, 0.5)
  check('s = 0.5 の 1 点目', Array.from(m.subarray(0, 3)), [1, 2, 3])
  check('NaN の区切りを保つ', Array.from(m.subarray(3, 6)).every((v) => Number.isNaN(v)), true)
  check('s = 0.5 の 3 点目', Array.from(m.subarray(6, 9)), [2, 2, 2])
  check('target が無い（座標系なし）なら q', mixPositions(q, new Float32Array(0), 0.5) === q, true)
}

// vertexNormals: xy 平面の正方形（2 つの三角形）は、全部の頂点で (0, 0, 1)
{
  const pos = new Float32Array([0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0])
  const n = vertexNormals(pos, new Uint32Array([0, 1, 2, 0, 2, 3]))
  check('平面の法線', round(n), [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1])
}

// weldGroups / weldNormals: 同じ位置の 2 頂点（継ぎ目）の法線を、足し合わせてそろえる
{
  const target = new Float32Array([0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 0, 0, 2, 0, 1])
  const g = weldGroups(target, new Uint32Array([0, 1, 2, 3, 4, 2]))
  check('継ぎ目の組', Array.from(g), [1, 3, -1])
  const normals = new Float32Array([0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 1])
  weldNormals(normals, g)
  const s = Math.round(Math.SQRT1_2 * 1e6) / 1e6
  check('そろえた法線', round(normals), [0, 0, 1, s, s, 0, 0, 0, 1, s, s, 0, 0, 0, 1])
}

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('coords.ts: すべて成功')

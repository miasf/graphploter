// coords.ts のテスト。実行: npm test
import { autoCoord, autoRadius, defaultAngleRange, mixPositions, vertexNormals, weldGroups, weldNormals } from './coords.ts'
import { latexToText } from './latex.ts'

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

// autoCoord: θ・φ・ρ を使う式は、自動で極座標・円柱座標・球座標
{
  const kind = (latex: string, flat = false, defined: string[] = []) => autoCoord(latexToText(latex), flat, new Set(defined))?.kind ?? null
  check('2D の r = θ は極座標', kind('r=\\theta', true), 'polar')
  check('3D の r = θ は円柱座標', kind('r=\\theta'), 'cylinder')
  check('3D の z = r cos θ は円柱座標', kind('z=r\\cos\\theta'), 'cylinder')
  check('φ があれば球座標', kind('r=1+\\frac{\\sin(3\\theta)\\sin(4\\phi)}{4}'), 'sphere')
  check('ρ だけでも球座標', kind('\\rho=2'), 'sphere')
  check('ρ のときの名前', autoCoord(latexToText('\\rho=\\phi'), false, new Set())?.names, ['rho', 'theta', 'phi'])
  check('2D の φ は対象外', kind('r=\\phi', true), null)
  check('θ が無ければ対象外', kind('z=x+y'), null)
  check('r だけの式も', kind('z=\\sin r'), 'cylinder')
  check('2D の r = 2 は円', kind('r=2', true), 'polar')
  check('x と一緒の r はスライダーかもしれない', kind('z=rx'), null)
  check('r がパラメータなら対象外', kind('z=\\sin r', false, ['r']), null)
  check('θ がパラメータなら対象外', kind('r=\\theta', false, ['theta']), null)
  check('関数の定義は対象外', kind('f(\\theta)=\\sin\\theta'), null)
  check('組は対象外', kind('(\\cos\\theta,\\sin\\theta)'), null)
  check('不等式も', kind('r\\le 2\\cos\\theta', true), 'polar')
}

// defaultAngleRange: θ が三角関数の中だけなら 0〜2π、外にもあれば 0〜12π
{
  const r = (latex: string) => defaultAngleRange(latexToText(latex), 'theta')[1]
  check('らせん', r('r=\\theta'), '12\\pi')
  check('等角らせん', r('r=e^{\\frac{\\theta}{5}}'), '12\\pi')
  check('バラ曲線', r('r=\\cos(2\\theta)'), '2\\pi')
  check('括弧なしの三角関数', r('r=1+\\cos\\theta'), '2\\pi')
  check('入れ子の括弧', r('r=\\sin(2(\\theta+1))'), '2\\pi')
  check('三角関数の外にもある', r('r=\\theta\\sin\\theta'), '12\\pi')
  check('φ は 0〜π', defaultAngleRange('r = phi', 'phi'), ['0', '\\pi'])
}

// autoRadius: 表示範囲の箱が全部入る半径
{
  check('負の半径も', autoRadius('polar', [[-3, 3], [-4, 4], [-1, 1]], true), ['-5', '5'])
  check('負の半径は曲線だけ', autoRadius('polar', [[-3, 3], [-4, 4], [-1, 1]]), ['0', '5'])
  const signed = (latex: string, flat = true) => autoCoord(latexToText(latex), flat, new Set())?.signedRadius
  check('リマソンは負の半径も', signed('r=1+2\\cos\\theta'), true)
  check('不等式は負の半径なし', signed('r\\le 2+\\cos\\theta'), false)
  check('z = f(r, θ) は負の半径なし', signed('z=r\\cos\\theta', false), false)
  check('円柱座標は xy 平面', autoRadius('cylinder', [[-3, 3], [-4, 4], [-9, 9]]), ['0', '5'])
  check('球座標は 3 つの座標', autoRadius('sphere', [[-2, 2], [-3, 3], [-6, 6]]), ['0', '7'])
}

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('coords.ts: すべて成功')

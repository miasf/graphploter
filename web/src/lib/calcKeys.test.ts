// calcKeys.ts のテスト。実行: npm test
// 「∑∫∈」のタブの全部のキーの雛形が、空欄を埋めると計算エンジン用の式に変換できること（「まだ使えません」にならないこと）
import { CALC_PAGES, MATRIX_ROWS } from './calcKeys.ts'
import { latexToText, setRegions } from './latex.ts'

// 名前で書いた積分の範囲（∬_D など）の、範囲の欄の中身
setRegions({
  D: '\\left\\{\\left(x,y\\right)\\mid x^2+y^2<1\\right\\}',
  V: '\\left\\{\\left(x,y,z\\right)\\mid x^2+y^2+z^2<1\\right\\}',
  C: '\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}',
  S: '\\left\\{\\left(u,v,0\\right)\\mid0\\le u\\le1,0\\le v\\le1\\right\\}',
})

let failures = 0
for (const page of [...CALC_PAGES, { title: '行列', rows: MATRIX_ROWS }]) {
  for (const row of page.rows) {
    // 長押しで出る別のキー（∮・∯ など）も
    for (const [k, insert] of row.flatMap((k) => [k.insert, ...(k.variants ?? [])].map((ins) => [k, ins] as const))) {
      // 空欄と、選んでいた部分（#0・#@）を埋める
      let latex = insert.replace(/#\?/g, ' x').replace(/#0/g, ' x').replace(/#@/g, 'f')
      // 単独では式にならない記号（関係・∃・≡ (mod n)）は、使う形の中で試す
      if (/^\\(perp|parallel|bmod|equiv)$/.test(latex)) latex = `x${latex} 1`
      if (/^\\(exists|forall)/.test(latex)) latex = `${latex}x<1`
      if (/^\\pmod/.test(latex)) latex = `x\\equiv1${latex}`
      try {
        latexToText(latex)
      } catch (e) {
        failures++
        console.log(`NG  ${page.title} の ${k.label}: ${latex}\n    ${e instanceof Error ? e.message : e}`)
      }
    }
  }
}
if (failures > 0) throw new Error(`${failures} 件失敗`)

const equationKeys = CALC_PAGES.find((page) => page.title === '方程式')?.rows.flat() ?? []
for (const [label, args, expected] of [
  ['cobweb', ['a_0', '12'], '@cobweb (a_0,12)'],
  ['orbit₂', ['a_0', 'b_0', 'N'], '@orbit (a_0,b_0,N)'],
  ['orbit₃', ['a_0', 'b_0', 'c_0', 'N'], '@orbit (a_0,b_0,c_0,N)'],
] as const) {
  const template = equationKeys.find((key) => key.label === label)
  if (!template || template.insert.match(/#\?/g)?.length !== args.length) {
    throw new Error(`方程式の ${label} に ${args.length} 個の入力欄が必要です`)
  }
  let index = 0
  const actual = latexToText(template.insert.replace(/#\?/g, () => args[index++]))
  if (actual !== expected) throw new Error(`${label}: ${actual} ≠ ${expected}`)
}

console.log('calcKeys.ts: すべて成功')

// calcKeys.ts のテスト。実行: npm test
// 「∑∫∈」のタブの全部のキーの雛形が、空欄を埋めると計算エンジン用の式に変換できること（「まだ使えません」にならないこと）
import { CALC_PAGES } from './calcKeys.ts'
import { latexToText } from './latex.ts'

let failures = 0
for (const page of CALC_PAGES) {
  for (const row of page.rows) {
    for (const k of row) {
      // 空欄と、選んでいた部分（#0・#@）を埋める
      const latex = k.insert.replace(/#\?/g, ' x').replace(/#0/g, ' x').replace(/#@/g, 'f')
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
console.log('calcKeys.ts: すべて成功')

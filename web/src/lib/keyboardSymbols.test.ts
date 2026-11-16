// keyboardSymbols.ts のテスト（計画 docs/plan-math-notation.md の M14）。実行: npm test
// 数式キーボードの全部のキーが、エラーにならずに計算エンジン用の式へ変換できること（空欄には x・1 を入れる）
import { KEEP_WITH_EXAMPLE, SYMBOL_PAGES, convertible, keyInsert } from './keyboardSymbols.ts'
import { latexToText } from './latex.ts'

let failures = 0
for (const page of SYMBOL_PAGES) {
  const keys = page.rows ? page.rows.flat() : (page.keys ?? [])
  if (keys.length === 0) {
    failures++
    console.log(`NG  空のページ: ${page.title}`)
  }
  if (!page.rows && keys.length > 40) {
    failures++
    console.log(`NG  ${page.title}: ${keys.length} 個（40 個まで）`)
  }
  for (const k of keys) {
    if (!convertible(k)) {
      failures++
      console.log(`NG  ${page.title}: ${keyInsert(k)}`)
    }
  }
}
// 決まった形でだけ意味を持つ記号は、その形で変換できる
for (const [cmd, example] of Object.entries(KEEP_WITH_EXAMPLE)) {
  try {
    latexToText(example)
  } catch (e) {
    failures++
    console.log(`NG  ${cmd} の例 ${example}: ${e}`)
  }
}
// 除外した記号の例（3.7 の (c)）が、キーボードに無い
const all = new Set(SYMBOL_PAGES.flatMap((p) => (p.rows ? p.rows.flat() : (p.keys ?? []))).map((k) => (typeof k === 'string' ? k : k.latex)))
for (const gone of ['\\approx', '\\subset', '\\otimes', '\\to', '\\therefore', '\\vdots']) {
  if (all.has(gone)) {
    failures++
    console.log(`NG  ${gone} がキーボードに残っている`)
  }
}
if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('keyboardSymbols.ts: すべて成功')

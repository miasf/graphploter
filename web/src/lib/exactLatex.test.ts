// 厳密値の道の答えの LaTeX（例題の表 core/tests/exact_cases/*.tsv の「期待する答え」）が、latex.ts で読めることを確かめる
// （計画 docs/plan-exact-core.md の EX4。「行に追加」した行が、そのまま計算エンジンの式になる）。実行: npm test
// @ts-expect-error node の組み込み（このリポジトリは node の型の定義を入れていない）
import { readdirSync, readFileSync } from 'node:fs'
import { latexToText } from './latex.ts'

const dir = new URL('../../../core/tests/exact_cases/', import.meta.url)
let failures = 0
let count = 0
for (const name of (readdirSync(dir) as string[]).filter((f: string) => f.endsWith('.tsv'))) {
  for (const line of readFileSync(new URL(name, dir), 'utf8').split('\n')) {
    if (!line || line.startsWith('#')) continue
    const latex = line.split('\t')[1]
    if (!latex) continue
    // 不定積分の積分定数 C・行列は、行の式としては別の決まり（それぞれのステップで確かめる）
    if (latex.includes('+C') || latex.includes('pmatrix')) continue
    count++
    try {
      const text = latexToText(latex)
      if (!text.trim()) throw new Error('空')
    } catch (e) {
      failures++
      console.log(`NG  ${name}: ${latex}\n    ${(e as Error).message}`)
    }
  }
}
// 書き方の例: 根号・分数・π・i・対数
const spot: [string, string][] = [
  ['\\frac{\\sqrt{6}-\\sqrt{2}}{4}', 'sqrt'],
  ['\\frac{1}{2}+\\frac{\\sqrt{3}}{2}i', 'i'],
  ['\\pi i', 'pi'],
  ['2\\ln 2-1', 'ln'],
]
for (const [latex, piece] of spot) {
  const text = latexToText(latex)
  if (!text.includes(piece)) {
    failures++
    console.log(`NG  ${latex} → ${text}（${piece} が無い）`)
  }
}
if (failures > 0) throw new Error(`exactLatex.test.ts: ${failures} 件の失敗（${count} 行）`)
console.log(`exactLatex.test.ts: ${count} 行の答えの LaTeX が読めた`)

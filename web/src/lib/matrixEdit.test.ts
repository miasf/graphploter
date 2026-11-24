// matrixEdit.ts のテスト。実行: npm test
import { BLANK, blankMatrix, colBlank, editWithMemory, formatMatrix, insertCol, insertRow, parseMatrix, removeCol, removeRow, resize, resizeMatrix, rowBlank, type Matrix } from './matrixEdit.ts'
import { latexToText } from './latex.ts'

let failures = 0
function check(name: string, got: unknown, want: unknown) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    failures++
    console.log(`NG  ${name}\n    結果: ${JSON.stringify(got)}\n    期待: ${JSON.stringify(want)}`)
  }
}

const src = 'A=\\begin{pmatrix}1 & \\frac{1}{2}\\\\ \\left(a,b\\right) & \\begin{bmatrix}5 & 6\\\\ 7 & 8\\end{bmatrix}\\end{pmatrix}+x'
const at = src.indexOf('\\frac')
const inner = '\\begin{bmatrix}5 & 6\\\\ 7 & 8\\end{bmatrix}'

// 成分に \frac・\left( , \right)・入れ子の行列があっても、成分に分けられる
const m = parseMatrix(src.slice(2, src.length - 2))
check('成分に分ける', m?.cells, [['1', '\\frac{1}{2}'], ['\\left(a,b\\right)', inner]])
check('括弧の形', m?.env, 'pmatrix')

// 2×2 → 3×3 → 2×3 → 1×1: 残る成分の中身が変わらず、増えた成分は空欄
const s33 = resizeMatrix(src, at, 3, 3)
check('2×2 → 3×3', s33, `A=\\begin{pmatrix}1 & \\frac{1}{2} & ${BLANK}\\\\ \\left(a,b\\right) & ${inner} & ${BLANK}\\\\ ${BLANK} & ${BLANK} & ${BLANK}\\end{pmatrix}+x`)
const s23 = resizeMatrix(s33, at, 2, 3)
check('3×3 → 2×3', s23, `A=\\begin{pmatrix}1 & \\frac{1}{2} & ${BLANK}\\\\ \\left(a,b\\right) & ${inner} & ${BLANK}\\end{pmatrix}+x`)
check('2×3 → 1×1', resizeMatrix(s23, at, 1, 1), 'A=\\begin{pmatrix}1\\end{pmatrix}+x')
// 6×6 を超えない
const big = parseMatrix(resizeMatrix(src, at, 9, 9).slice(2, -2))
check('6×6 まで', [big?.cells.length, big?.cells[0].length], [6, 6])
// 入れ子の行列の中の位置なら、内側の行列だけが変わる（括弧の形も変わらない）
const innerAt = src.indexOf('5 & 6')
check('内側の行列', resizeMatrix(src, innerAt, 1, 2), src.replace(inner, '\\begin{bmatrix}5 & 6\\end{bmatrix}'))
// 行列の外の位置では何もしない
check('行列の外', resizeMatrix(src, 0, 3, 3), src)

// 行・列を足す・消す
const base = parseMatrix('\\begin{vmatrix}a & b\\\\ c & d\\end{vmatrix}')!
check('行を下に足す', formatMatrix(insertRow(base, 1)), `\\begin{vmatrix}a & b\\\\ ${BLANK} & ${BLANK}\\\\ c & d\\end{vmatrix}`)
check('列を右に足す', formatMatrix(insertCol(base, 2)), `\\begin{vmatrix}a & b & ${BLANK}\\\\ c & d & ${BLANK}\\end{vmatrix}`)
check('行を消す', formatMatrix(removeRow(base, 0)), '\\begin{vmatrix}c & d\\end{vmatrix}')
check('列を消す', formatMatrix(removeCol(base, 1)), '\\begin{vmatrix}a\\\\ c\\end{vmatrix}')
check('最後の 1 行は消せない', formatMatrix(removeRow(removeRow(base, 0), 0)), '\\begin{vmatrix}c & d\\end{vmatrix}')
let grown = base
for (let k = 0; k < 10; k++) grown = insertRow(insertCol(grown, 0), 0)
check('足しても 6×6 まで', [grown.cells.length, grown.cells[0].length], [6, 6])
const withBlank = insertRow(insertCol(base, 1), 2)
check('空の行・列', [rowBlank(withBlank, 2), rowBlank(withBlank, 0), colBlank(withBlank, 1), colBlank(withBlank, 0)], [true, false, true, false])
check('大きさを変えても中身が残る', resize(resize(base, 3, 3), 2, 2).cells, base.cells)

// 空欄は 0 として読める（大きさを変えている途中でもエラーにならない）
check('空欄のある行列を読む', latexToText(resizeMatrix('\\begin{pmatrix}1 & 2\\\\ 3 & 4\\end{pmatrix}', 5, 3, 3)), '⟦1,2,0;3,4,0;0,0,0⟧')
check('新しい行列', latexToText(blankMatrix(2, 2)), '⟦0,0;0,0⟧')

// 小さくしてから大きくすると、元の成分が戻る（覚え）
{
  const m33: Matrix = { env: 'pmatrix', cells: [['1', '2', '3'], ['4', '5', '6'], ['7', '8', '9']] }
  let g: Matrix | null = null
  let cur = m33
  const step = (e: Parameters<typeof editWithMemory>[2]) => {
    const r = editWithMemory(cur, g, e)
    cur = r.next
    g = r.ghost
    return cur.cells
  }
  check('3×3 → 2×2', step({ op: 'size', rows: 2, cols: 2 }), [['1', '2'], ['4', '5']])
  check('2×2 → 3×3 で元に戻る', step({ op: 'size', rows: 3, cols: 3 }), m33.cells)
  check('3×3 → 1×3', step({ op: 'size', rows: 1, cols: 3 }), [['1', '2', '3']])
  // 小さくしている間に書き換えた成分は、書き換えたほうが残る
  cur = { env: 'pmatrix', cells: [['1', 'a', '3']] }
  check('最後に行を足すと戻る', step({ op: 'row+', at: 1 }), [['1', 'a', '3'], ['4', '5', '6']])
  check('最後の行を消して足すと戻る', (step({ op: 'row-', at: 1 }), step({ op: 'row+', at: 1 })), [['1', 'a', '3'], ['4', '5', '6']])
  check('最後の列を消して足すと戻る', (step({ op: 'col-', at: 2 }), step({ op: 'col+', at: 2 })), [['1', 'a', '3'], ['4', '5', '6']])
  check('大きさを戻すと 3 行目も戻る', step({ op: 'size', rows: 3, cols: 3 }), [['1', 'a', '3'], ['4', '5', '6'], ['7', '8', '9']])
  // 途中に行を足すと、覚えも一緒にずれる（増えた行は空欄）
  check('途中に行を足す', step({ op: 'row+', at: 1 }), [['1', 'a', '3'], ['', '', ''], ['4', '5', '6'], ['7', '8', '9']])
  // 途中の行を消すと、覚えからも消える
  check('途中の行を消す', (step({ op: 'size', rows: 2, cols: 3 }), step({ op: 'row-', at: 0 })), [['', '', '']])
  check('消した行は戻らない', step({ op: 'size', rows: 3, cols: 3 }), [['', '', ''], ['4', '5', '6'], ['7', '8', '9']])
}

// 行の間の空き・最後の \\・pmatrix*・smallmatrix（#209）。計算のほうと同じ大きさで読む
{
  const det = (latex: string) => latexToText(latex).replace(/\s+/g, '')
  check('行の間の空き: 成分', parseMatrix('\\begin{pmatrix}1&2\\\\[2pt]3&4\\end{pmatrix}')?.cells, [['1', '2'], ['3', '4']])
  check('行の間の空き: 計算', det('\\begin{vmatrix}1&2\\\\[2pt]3&4\\end{vmatrix}'), det('\\begin{vmatrix}1&2\\\\3&4\\end{vmatrix}'))
  check('最後の \\\\: 成分', parseMatrix('\\begin{pmatrix}1&2\\\\3&4\\\\\\end{pmatrix}')?.cells, [['1', '2'], ['3', '4']])
  check('最後の \\\\: 計算', det('\\begin{vmatrix}1&2\\\\3&4\\\\\\end{vmatrix}'), det('\\begin{vmatrix}1&2\\\\3&4\\end{vmatrix}'))
  check('pmatrix*', parseMatrix('\\begin{pmatrix*}1&2\\\\3&4\\end{pmatrix*}')?.cells, [['1', '2'], ['3', '4']])
  const star = parseMatrix('\\begin{pmatrix*}[r]1&-2\\\\3&4\\end{pmatrix*}')!
  check('pmatrix*[r]: 成分', star.cells, [['1', '-2'], ['3', '4']])
  check('pmatrix*[r]: 大きさを変えてもそろえ方を残す', formatMatrix(resize(star, 2, 3)), `\\begin{pmatrix*}[r]1 & -2 & ${BLANK}\\\\ 3 & 4 & ${BLANK}\\end{pmatrix*}`)
  check('pmatrix*[r]: 計算', det('\\begin{pmatrix*}[r]1&-2\\\\3&4\\end{pmatrix*}'), det('\\begin{pmatrix}1&-2\\\\3&4\\end{pmatrix}'))
  check('smallmatrix', formatMatrix(insertRow(parseMatrix('\\begin{smallmatrix}1&2\\\\3&4\\end{smallmatrix}')!, 2)), `\\begin{smallmatrix}1 & 2\\\\ 3 & 4\\\\ ${BLANK} & ${BLANK}\\end{smallmatrix}`)
  // 区間のような [ ] で始まる成分は消さない
  check('[ で始まる成分は残す', parseMatrix('\\begin{pmatrix}1\\\\[a]\\end{pmatrix}')?.cells, [['1'], ['[a]']])
}

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('matrixEdit.ts: すべて成功')

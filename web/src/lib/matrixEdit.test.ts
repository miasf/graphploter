// matrixEdit.ts のテスト。実行: npm test
import { BLANK, blankMatrix, colBlank, formatMatrix, insertCol, insertRow, parseMatrix, removeCol, removeRow, resize, resizeMatrix, rowBlank } from './matrixEdit.ts'
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

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('matrixEdit.ts: すべて成功')

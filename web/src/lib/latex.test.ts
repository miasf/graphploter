// latex.ts のテスト。実行: npm test
import { joinTuple, latexToText, splitTuple } from './latex.ts'

let failures = 0
function check(name: string, got: unknown, want: unknown) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    failures++
    console.log(`NG  ${name}\n    結果: ${JSON.stringify(got)}\n    期待: ${JSON.stringify(want)}`)
  }
}

// LaTeX → 計算エンジン用の式
const cases: [string, string][] = [
  ['\\frac{x^{2}+1}{y}', '((x^(2)+1)/(y))'],
  ['\\sin\\left(x\\right)\\cdot y', 'sin (x)*y'],
  ['\\operatorname{nCr}\\left(5,2\\right)', 'nCr (5,2)'],
  ['\\left|x\\right|', 'abs(x)'],
  ['|x|+|y|', 'abs(x)+ abs(y)'],
  ['\\sqrt{x+1}', 'sqrt(x+1)'],
  ['\\sqrt[3]{x}', '((x)^(1/(3)))'],
  ['2\\pi', '2 pi'],
  ['e^{-x^2}', 'e^(-x^(2))'],
  ['\\sin^{2}x', 'sin ^(2)x'],
  ['x!', 'x!'],
  ['\\Gamma\\left(x+\\imaginaryI y\\right)', 'gamma (x+iy)'],
  ['floor\\left(x\\right)', 'floor(x)'],
  ['\\lfloor x\\rfloor', 'floor(x)'],
  ['z=\\exponentialE^{x}', 'z=e^(x)'],
  ['x^2+y^2+z^2=4', 'x^(2)+y^(2)+z^(2)=4'],
  // 数式キーボードの関数キーで入れると、名前の中に \mathrm が入れ子になる
  ['z=\\operatorname{\\mathrm{Im}}\\left(xyi\\right)', 'z= Im (xyi)'],
  ['\\operatorname{\\mathrm{nCr}}\\left(5,2\\right)', 'nCr (5,2)'],
  ['\\operatorname{\\mathrm{floor}}\\left(x\\right)', 'floor (x)'],
  ['\\operatorname{\\mathrm I m}\\left(x\\right)', 'Im (x)'],
  // 添字（4次元目以降の座標 w₂ や、媒介変数 t₁）
  ['w_{2}+w_3', 'w_2+w_3'],
  ['\\left(t_1,t_{2}\\right)', '(t_1,t_2)'],
]
for (const [input, want] of cases) check(input, latexToText(input), want)

// 変換できないものはエラーになる
for (const input of ['x_{a}', '2_{1}', '\\alpha', '\\frac{1}']) {
  let threw = false
  try {
    latexToText(input)
  } catch {
    threw = true
  }
  check(`${input} はエラー`, threw, true)
}

// 組の分割・結合
check('split', splitTuple('\\left(\\cos t,\\sin t,\\frac{t}{4}\\right)'), ['\\cos t', '\\sin t', '\\frac{t}{4}'])
check('split 括弧の掛け算は組ではない', splitTuple('\\left(x+1\\right)\\left(y+1\\right)'), null)
check('split 入れ子のカンマ', splitTuple('\\left(\\operatorname{max}\\left(t,1\\right),2\\right)'), [
  '\\operatorname{max}\\left(t,1\\right)',
  '2',
])
check('join', joinTuple(['a', 'b']), '\\left(a,b\\right)')

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('latex.ts: すべて成功')

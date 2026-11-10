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
  // 添字（文字の後ろの数字。入力欄では w2 と打つと w_2 になる）: 多変数の媒介変数表示
  ['\\left(t_1,t_2,t_3,t_{10}\\right)', '(t_1,t_2,t_3,t_10)'],
  ['w_2^2+a_1', 'w_2^(2)+a_1'],
  // 名前に数字を含む関数・長い関数名
  ['\\operatorname{hyp2F1}\\left(1,1,2,x\\right)', 'hyp2F1 (1,1,2,x)'],
  ['x\\operatorname{\\mathrm{sinc}}\\left(x\\right)', 'x sinc (x)'],
  // 数式キーボードの関数キーで入れると、名前の中に \mathrm が入れ子になる
  ['z=\\operatorname{\\mathrm{Im}}\\left(xyi\\right)', 'z= Im (xyi)'],
  ['\\operatorname{\\mathrm{nCr}}\\left(5,2\\right)', 'nCr (5,2)'],
  ['\\operatorname{\\mathrm{floor}}\\left(x\\right)', 'floor (x)'],
  ['\\operatorname{\\mathrm I m}\\left(x\\right)', 'Im (x)'],
  // 特殊関数
  ['\\zeta\\left(s\\right)', 'zeta (s)'],
  ['\\psi\\left(x\\right)+\\cot x', 'psi (x)+ cot x'],
  // リスト
  ['\\left(t,\\left[-1,1\\right],0\\right)', '(t,[-1,1],0)'],
  ['2[1,3]', '2[1,3]'],
  // ふつうの LaTeX の関数命令
  ['\\max\\left(\\left|x\\right|,\\left|w\\right|\\right)=1', 'max ( abs(x), abs(w))=1'],
  ['\\min\\left(x,y\\right)+\\Re\\left(z\\right)', 'min (x,y)+ Re (z)'],
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

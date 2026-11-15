// latex.ts のテスト。実行: npm test
import { fenceBareDelimiters, joinTuple, latexToText, splitTuple } from './latex.ts'

let failures = 0
function check(name: string, got: unknown, want: unknown) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    failures++
    console.log(`NG  ${name}\n    結果: ${JSON.stringify(got)}\n    期待: ${JSON.stringify(want)}`)
  }
}

// LaTeX → 計算エンジン用の式
const cases: [string, string][] = [
  // 数学の記法（計画 docs/plan-math-notation.md の M1・M4）
  ['\\sum_{n=1}^{10}n^2', '@sum(n,1,10) n^(2)'],
  ['\\sum_{k=1}^{N}\\frac{\\sin\\left(kx\\right)}{k}+1', '@sum(k,1,N) ((sin (kx))/(k))+1'],
  ['\\prod\\limits_{k=1}^{5}\\left(x-k\\right)', '@prod(k,1,5) (x-k)'],
  ['y=\\pm\\sqrt{x}', 'y=± sqrt(x)'],
  ['x\\mp y', 'x∓y'],
  ['\\left[1,3,\\ldots,9\\right]', '[1,3,…,9]'],
  ['\\left(f\\circ g\\right)\\left(x\\right)', '(f∘g)(x)'],
  ['f\\colon x\\mapsto x^2', 'f(x)=x^(2)'],
  ['30^{\\circ}', '30*(pi/180)'],
  // 場合分け（M2）
  ['f\\left(x\\right)=\\begin{cases}x^2 & x<0\\\\ x & \\text{otherwise}\\end{cases}', 'f(x)= @cases(x^(2), x<0; x)'],
  ['\\begin{cases}1 & \\text{if } 0\\le x<1\\\\ 0 & x\\ge1\\\\ -1\\end{cases}', '@cases(1, 0<=x<1; 0, x>=1; -1)'],
  // 差分（前進 Δ・後退 ∇・中心 δ）
  ['y=\\Delta_{x}x^2', 'y= @fdiff(x) x^(2)'],
  ['y=\\nabla_{x,0.5}x^2', 'y= @bdiff(x,0.5) x^(2)'],
  ['y=\\delta_{x}^{2}x^3', 'y= @cdiff(x,1,2) x^(3)'],
  ['y=\\Delta_{x,\\frac{x}{10}}x^2', 'y= @fdiff(x,((x)/(10))) x^(2)'],
  ['y=\\Delta^{2}_{x,h}f\\left(x\\right)', 'y= @fdiff(x,h,2) f(x)'],
  // 定積分（M5）
  ['y=\\int_{0}^{x}e^{-t^{2}}\\,dt', 'y= @int(t,0,x)(e^(-t^(2)))'],
  ['\\int_0^1t\\,dt+1', '@int(t,0,1)(t) +1'],
  ['\\int_{0}^{\\infty}e^{-t}\\mathrm{d}t', '@int(t,0,∞)(e^(-t))'],
  ['\\int_{-\\infty}^{\\infty}\\frac{1}{1+u^2}du', '@int(u,-∞,∞)(((1)/(1+u^(2))))'],
  // 不定積分・重積分
  ['y=\\int x^2\\,dx', 'y= @int(x)(x^(2))'],
  ['\\int_0^1\\int_0^x xy\\,dy\\,dx', '@int(x,0,1)(@int(y,0,x)(xy))'],
  ['\\iint_{\\left\\{\\left(x,y\\right)\\mid0\\le x\\le1,0\\le y\\le x\\right\\}}xy\\,dA', '@intset((x,y),0<=x<=1,0<=y<=x)(xy)'],
  ['\\iint_{\\left[0,1\\right]\\times\\left[0,2\\right]}xy\\,dx\\,dy', '@intset((x,y),0<=x<=1,0<=y<=2)(xy)'],
  ['\\iiint_{\\left\\{\\left(x,y,z\\right)\\mid0\\le x\\le1,0\\le y\\le1,0\\le z\\le1\\right\\}}1\\,dV', '@intset((x,y,z),0<=x<=1,0<=y<=1,0<=z<=1)(1)'],
  // ベクトル場の演算子・外積・線積分・面積分
  ['z=\\nabla\\cdot\\left(x^2,y^2,0\\right)', 'z= @div (x^(2),y^(2),0)'],
  ['z=\\nabla\\times\\left(-y,x\\right)', 'z= @rot (-y,x)'],
  ['z=\\nabla^{2}x^2y', 'z= @lap x^(2)y'],
  ['\\nabla f', '@grad f'],
  ['z=\\Delta\\left(x^2y\\right)', 'z= @lap (x^(2)y)'],
  ['\\operatorname{div}F', '@div F'],
  ['\\left(1,2,3\\right)\\times\\left(4,5,6\\right)', '(1,2,3)×(4,5,6)'],
  ['\\oint_{\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}}\\left(-y,x\\right)\\cdot d\\mathbf{r}', '@intset(( cos t, sin t),0<=t<=2 pi; {vec})((-y,x))'],
  ['\\int_{\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}}x^2\\,ds', '@intset(( cos t, sin t),0<=t<=2 pi)(x^(2))'],
  // 微分（M3）
  ['\\frac{d}{dx}\\sin\\left(x\\right)', '@diff(x) sin (x)'],
  ['z=\\frac{\\partial}{\\partial x}\\sin\\left(xy\\right)', 'z= @diff(x) sin (xy)'],
  ['\\frac{d^{2}}{dx^{2}}e^{-x^2}', '@diff(x,2) e^(-x^(2))'],
  ['\\frac{\\partial^{2}}{\\partial x\\partial y}xy', '@diff(x) @diff(y) xy'],
  ['\\frac{\\mathrm{d}f}{\\mathrm{d}x}', '@diff(x) f'],
  ["y=f^{\\prime}\\left(x\\right)", "y=f'(x)"],
  ["y=f''\\left(x\\right)", "y=f''(x)"],
  ['\\frac{d}{2}', '((d)/(2))'],
  // 座標の名前（座標系。計画 S38）
  ['r=\\cos\\left(2\\theta\\right)', 'r= cos (2 theta )'],
  ['\\rho\\sin\\varphi+\\vartheta', 'rho sin phi + theta'],
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
  // キーボードで [ を打ったときの書き方
  ['z=\\left\\lbrack1,2\\right\\rbrack x', 'z=[1,2]x'],
  ['\\left(t,\\left\\lbrack-1,1\\right\\rbrack,0\\right)', '(t,[-1,1],0)'],
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

// 連立方程式・不等式（実際の出力の空白は、計算エンジンの読み方に影響しない）
const squash = (t: string) => t.replace(/\s+/g, '')
for (const [input, want] of [
  ['x\\le y\\le z', 'x<=y<=z'],
  ['x^2+y^2+z^2<4', 'x^(2)+y^(2)+z^(2)<4'],
  ['y=x,z=0', 'y=x,z=0'],
  ['\\left(\\cos t,\\sin t,t\\right),z\\ge0', '(cost,sint,t),z>=0'],
  ['0\\leq z\\lt\\sin x', '0<=z<sinx'],
  ['x\\gt y\\geqslant 1', 'x>y>=1'],
  ['x\\leqq y\\geqq z', 'x<=y>=z'],
]) {
  check(input, squash(latexToText(input)), want)
}

// 組の分割・結合
check('split', splitTuple('\\left(\\cos t,\\sin t,\\frac{t}{4}\\right)'), ['\\cos t', '\\sin t', '\\frac{t}{4}'])
check('split 括弧の掛け算は組ではない', splitTuple('\\left(x+1\\right)\\left(y+1\\right)'), null)
check('split 入れ子のカンマ', splitTuple('\\left(\\operatorname{max}\\left(t,1\\right),2\\right)'), [
  '\\operatorname{max}\\left(t,1\\right)',
  '2',
])
// 裸の床・天井の括弧を \left … \right の形に直す
check('fence floor', fenceBareDelimiters('z=\\lfloor x\\rfloor'), 'z=\\left\\lfloor x\\right\\rfloor')
check('fence 入れ子', fenceBareDelimiters('\\lceil \\lfloor x\\rfloor /2\\rceil'), '\\left\\lceil \\left\\lfloor x\\right\\rfloor /2\\right\\rceil')
check('fence 分数の中', fenceBareDelimiters('z=\\frac{\\lfloor x\\rfloor}{2}'), 'z=\\frac{\\left\\lfloor x\\right\\rfloor}{2}')
check('fence 済みはそのまま', fenceBareDelimiters('\\left\\lfloor x\\right\\rfloor'), '\\left\\lfloor x\\right\\rfloor')
check('fence 対にならない', fenceBareDelimiters('\\lfloor x\\rceil'), '\\lfloor x\\rceil')
check('fence { } をまたがない', fenceBareDelimiters('\\frac{\\lfloor x}{y\\rfloor}'), '\\frac{\\lfloor x}{y\\rfloor}')
check('fence 変換結果は同じ', latexToText(fenceBareDelimiters('z=\\lfloor x\\rfloor+1')), latexToText('z=\\lfloor x\\rfloor+1'))
check('join', joinTuple(['a', 'b']), '\\left(a,b\\right)')

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('latex.ts: すべて成功')

// latex.ts のテスト。実行: npm test
import { fenceBareDelimiters, joinTuple, latexToText, regionDisplayLatex, regionsUsed, setRegions, splitTuple, integralDisplay } from './latex.ts'

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
  // ベクトルと行列（M6）
  ['A=\\begin{pmatrix}1 & 2\\\\ 3 & 4\\end{pmatrix}', 'A= ⟦1,2;3,4⟧'],
  ['\\begin{bmatrix}\\cos a & -\\sin a\\\\ \\sin a & \\cos a\\end{bmatrix}\\left(\\cos t,2\\sin t\\right)', '⟦cos a,- sin a;sin a,cos a⟧ ( cos t,2 sin t)'],
  ['z=\\begin{vmatrix}x & y\\\\ y & x\\end{vmatrix}', 'z= @det(⟦x,y;y,x⟧)'],
  // 空欄は 0 として読む（大きさを変えている途中）
  ['\\begin{pmatrix}1 & \\placeholder{}\\\\ \\placeholder{} & 1\\end{pmatrix}', '⟦1,0;0,1⟧'],
  ['\\begin{pmatrix}1 & 2\\\\ \\placeholder{} & \\placeholder{}\\end{pmatrix}', '⟦1,2;0,0⟧'],
  ['\\det A+\\operatorname{tr}\\left(A\\right)', '@det A+ @tr (A)'],
  ['A^{T}B^{\\top}C^{\\dagger}', 'A^(T)B^(T)C^(†)'],
  ['\\left\\langle a,b\\right\\rangle+\\left\\Vert a\\right\\Vert+\\lVert b\\rVert', '@dot(a,b) + @norm(a) + @norm(b)'],
  ['\\vec{b}+t\\vec{v}', '{vec_b} +t {vec_v}'],
  ['\\vec{a}\\times\\vec{b}', '{vec_a} × {vec_b}'],
  ['A_{1,2}+\\vec{v}_{2}+w_{2}', 'A_(1,2)+ {vec_v}_2+w_2'],
  ['x_{a}', 'x_(a)'],
  ['\\dim\\ker A', '@dim @ker A'],
  // 常微分方程式（M9）
  ["x'=-y,\\ y'=x", "x'=-y, y'=x"],
  ['\\dot{x}=10\\left(y-x\\right)', "x'=10(y-x)"],
  ['\\ddot{x}+x=0', "x''+x=0"],
  ['\\frac{dx}{dt}=-y', '@diff(t) x =-y'],
  ['\\frac{d^{2}y}{dx^{2}}+y=0', '@diff(x,2) y +y=0'],
  ['y^{\\prime}=x-y', "y'=x-y"],
  // 名前に使える文字と、小さな記号（M13）
  ['z=\\alpha x', 'z= {alpha} x'],
  ['\\alpha=1', '{alpha} =1'],
  ['z=\\hat{x}+\\tilde{\\alpha}+\\mathcal{A}+\\mathbb{K}+\\ell', 'z= {hat_x} + {tilde_alpha} + {cal_A} + {vec_K} + {ell}'],
  ['z=\\theta+\\beta+\\delta', 'z= theta + {beta} + {delta}'],
  ['x\\equiv1\\pmod{3}', 'mod((x)-(1),3)=0'],
  ['x\\equiv y', 'x=y'],
  ['z=x\\bmod 3', 'z=x%%3'],
  ['3\\divides x', '3∣x'],
  ['\\left(x,y,z\\right)\\perp\\left(1,1,1\\right)', '(x,y,z)⟂(1,1,1)'],
  ['\\left(x,y,z\\right)\\parallel\\left(1,1,1\\right)', '(x,y,z)∥(1,1,1)'],
  ['y\\nless x', 'y>=x'],
  ['y\\ngeq x', 'y<x'],
  ['y\\lneq x', 'y<x'],
  ['y=x\\degree', 'y=x*(pi/180)'],
  ['y=50\\%', 'y=50/100'],
  ['z=w^{\\ast}', 'z=w^(∗)'],
  ['z=\\overline{x+iy}+\\bar{y}', 'z= conj(x+iy) + conj(y)'],
  ['z=\\binom{5}{2}', 'z= nCr(5,2)'],
  ['z=\\cfrac{1}{x}', 'z=((1)/(x))'],
  ['z=\\gcd\\left(x,y\\right)+\\operatorname{lcm}\\left(x,y\\right)', 'z= gcd (x,y)+ lcm (x,y)'],
  ['z=\\angle\\left(u,v\\right)', 'z= @angle (u,v)'],
  ['z=\\boxed{x}+\\underbrace{y}_{a}+\\overbrace{x}^{b}', 'z=(x)+(y)+(x)'],
  ['z=x\\star y\\bullet x\\centerdot y', 'z=x*y*x*y'],
  ['z=\\Re x+\\Im y', 'z= Re x+ Im y'],
  // ∃・∀・添字つきの max（M12）
  ['\\exists a\\in\\left[0,1\\right],x^2+y^2+\\left(z-a\\right)^2=1', '@exists(a,0,1) x^(2)+y^(2)+(z-a)^(2)=1'],
  ['\\forall a\\in\\left[-1,1\\right],z\\ge ax', '@forall(a,-1,1) z>=ax'],
  ['z=\\max_{t\\in\\left[0,1\\right]}\\sin\\left(xt\\right)y', 'z= @max(t,0,1) sin (xt)y'],
  ['z=\\inf_{t\\in\\left[0,1\\right]}tx', 'z= @min(t,0,1) tx'],
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
  // 集合と論理（M11）
  ['\\left\\{\\left(x,y,z\\right)\\mid x^2+y^2<1,0\\le z\\le2\\right\\}', '⦃(x,y,z)∣x^(2)+y^(2)<1,0<=z<=2⦄'],
  ['x^2+y^2+z^2\\le4\\land\\neg\\left(z>1\\right)', 'x^(2)+y^(2)+z^(2)<=4∧¬(z>1)'],
  ['A\\cup B\\setminus C', 'A∪B∖C'],
  ['A\\triangle B', 'A△B'],
  ['A^{c}\\cap B^{\\complement}', 'A^(c)∩Bᶜ'],
  ['x<0\\oplus y<0', 'x<0⊕y<0'],
  ['x<0\\veebar y<0', 'x<0⊕y<0'],
  ['x<0\\barwedge y<0', 'x<0⊼y<0'],
  ['x<0\\downarrow y<0', 'x<0⊽y<0'],
  ['x\\in\\left[0,1\\right)', 'x∈[0,1)'],
  ['x\\in\\left(0,1\\right]', 'x∈(0,1]'],
  ['\\left(x,y,z\\right)\\in\\left[0,1\\right]\\times\\left[0,2\\right]\\times\\left[0,1\\right]', '(x,y,z)∈[0,1]×[0,2]×[0,1]'],
  ['x\\in\\mathbb{Z}', 'x∈ℤ'],
  ['x\\notin\\mathbb{N}', 'x∉ℕ'],
  ['x\\ne0', 'x≠0'],
  ['\\emptyset\\lor\\top', '∅∨⊤'],
  ['S=\\left\\{\\left(x,y\\right)\\mid x^2+y^2<1\\right\\}', 'S= ⦃(x,y)∣x^(2)+y^(2)<1⦄'],
  // 縦線を消して | で打ち直したもの（\mid ではなく裸の |）も、区切りとして読む。絶対値の組はそのまま
  ['\\left\\{\\left(x,y\\right)|x<1\\right\\}', '⦃(x,y)∣x<1⦄'],
  ['\\left\\{\\left(x,y\\right)|\\left|x\\right|<1\\right\\}', '⦃(x,y)∣abs(x)<1⦄'],
  ['\\left\\{\\left(x,y\\right)\\mid\\left|x\\right|+\\left|y\\right|<1\\right\\}', '⦃(x,y)∣abs(x)+ abs(y)<1⦄'],
  ['\\left\\{x+1\\right\\}', '(x+1)'],
  // 集合の上の総和・総積、有限集合、⋃・⋂
  ['\\sum_{n\\in\\left\\{1,4,9\\right\\}}n', '@sumset(n; n∈⦃1,4,9⦄) n'],
  ['\\sum_{1\\le n\\le10,\\ \\operatorname{mod}\\left(n,2\\right)=1}n', '@sumset(n; 1<=n<=10, mod (n,2)=1) n'],
  ['\\sum_{d\\mid12}d', '@sumset(d; d∣12) d'],
  ['\\prod_{\\left(m,n\\right)\\in\\left[1,3\\right]\\times\\left[1,3\\right]}mn', '@prodset((m,n); (m,n)∈[1,3]×[1,3]) mn'],
  ['\\bigcup_{k=1}^{3}\\left\\{\\left(x,y\\right)\\mid x<k\\right\\}', '@bigcup(k; 1<=k<=3) ⦃(x,y)∣x<k⦄'],
  ['\\bigcap_{k\\in\\left\\{0,1\\right\\}}A', '@bigcap(k; k∈⦃0,1⦄) A'],
  ['x\\in\\left\\{1,2,3\\right\\}', 'x∈⦃1,2,3⦄'],
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
// 数字でない添字 x_{a} は、計算エンジンへ x_(a) の形で渡す（行列の成分・数列の項。M6・M10）
for (const input of ['2_{1}', '\\frac{1}']) {
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

// 名前で書いた積分の範囲（∬_D の D。範囲は別の欄に書く）
setRegions({
  D: '\\left\\{\\left(x,y\\right)\\mid0\\le x\\le1,0\\le y\\le x\\right\\}',
  C: '\\left\\{\\left(\\cos t,\\sin t\\right)|0\\le t\\le2\\pi\\right\\}',
  '\\Omega': '\\left[0,1\\right]\\times\\left[0,2\\right]',
  E: '',
})
check('∬_D', latexToText('\\iint_{D}xy\\,dA'), '@intset((x,y),0<=x<=1,0<=y<=x)(xy)')
check('∬_D（括弧なし）', latexToText('\\iint_Dxy\\,dA'), '@intset((x,y),0<=x<=1,0<=y<=x)(xy)')
check('∮_C F·dr', latexToText('\\oint_{C}\\left(-y,x\\right)\\cdot d\\mathbf{r}'), '@intset(( cos t, sin t),0<=t<=2 pi; {vec})((-y,x))')
check('∫_Ω dx dy', latexToText('\\int_{\\Omega}xy\\,dx\\,dy'), '@intset((x,y),0<=x<=1,0<=y<=2)(xy)')
check('定積分はそのまま', latexToText('\\int_{a}^{b}t\\,dt'), '@int(t,a,b)(t)')
for (const name of ['E', 'G']) {
  let message = ''
  try {
    latexToText(`\\iint_{${name}}xy\\,dA`)
  } catch (e) {
    message = e instanceof Error ? e.message : String(e)
  }
  check(`範囲 ${name} が空`, message.startsWith(`${name} の範囲を、下の欄に`), true)
}
check(
  '使っている範囲',
  regionsUsed('\\iint_{D}x\\,dA+\\int_{C}x\\,ds+\\iiint_{V}1\\,dV+\\iint_{S}F\\cdot d\\mathbf{S}+\\int_{a}^{b}t\\,dt+\\iint_{D}y\\,dA').map((r) => `${r.name}=${r.template}`),
  [
    'D=\\left\\{\\left(x,y\\right)\\mid\\placeholder{}\\right\\}',
    'C=\\left\\{\\left(\\placeholder{}\\right)\\mid\\placeholder{}\\right\\}',
    'V=\\left\\{\\left(x,y,z\\right)\\mid\\placeholder{}\\right\\}',
    'S=\\left\\{\\left(\\placeholder{}\\right)\\mid\\placeholder{}\\right\\}',
  ],
)
setRegions({})

// 積分の範囲を描くための式
const shown: [string, string | null][] = [
  ['\\left\\{\\left(x,y\\right)\\mid-1\\le x+y\\le1,0\\le x-y\\le1\\right\\}', '⦃(x,y,z)∣-1<=x+y<=1,0<=x-y<=1,z=0⦄'],
  ['\\left\\{\\left(x,y,z\\right)\\mid x^2+y^2+z^2<1\\right\\}', '⦃(x,y,z)∣x^(2)+y^(2)+z^(2)<1⦄'],
  ['\\left[0,1\\right]\\times\\left[0,2\\pi\\right]', '⦃(x,y,z)∣0<=x<=1,0<=y<=2 pi ,z=0⦄'],
  ['\\left\\{\\left(r\\cos\\theta,r\\sin\\theta\\right)\\mid0\\le r\\le1,0\\le\\theta\\le2\\pi\\right\\}', '⦃(u cos v,u sin v)∣0<=u<=1,0<=v<=2 pi⦄'],
  ['\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}', '⦃( cos t, sin t)∣0<=t<=2 pi⦄'],
  ['\\left\\{\\left(x,y\\right)\\mid\\placeholder{}\\right\\}', null],
  ['', null],
]
for (const [def, want] of shown) {
  const latex = regionDisplayLatex(def)
  check(`範囲を描く式 ${def}`, latex === null ? null : latexToText(latex), want)
}

// 条件の並びの省略形 a ≤ x, y, z ≤ b
check('省略形（集合）', latexToText('\\left\\{\\left(x,y,z\\right)\\mid-1\\le x,y,z\\le1\\right\\}'), '⦃(x,y,z)∣-1<=x<=1,-1<=y<=1,-1<=z<=1⦄')
check('省略形（積分）', latexToText('\\iiint_{\\left\\{\\left(x,y,z\\right)\\mid0\\le x,y,z\\le1,x+y\\le z\\right\\}}1\\,dV'), '@intset((x,y,z),0<=x<=1,0<=y<=1,0<=z<=1,x+y<=z)(1)')
check('省略形でない', latexToText('\\left\\{\\left(x,y\\right)\\mid0\\le x,y\\le1\\right\\}'), '⦃(x,y)∣0<=x,y<=1⦄')

// 積分を描くための式（被積分関数を、積分する範囲の上に描く）
setRegions({
  D: '\\left\\{\\left(x,y\\right)\\mid1\\le x\\le y,y\\le3\\right\\}',
  C: '\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}',
  P: '\\left\\{\\left(r\\cos\\theta,r\\sin\\theta\\right)\\mid0\\le r\\le1,0\\le\\theta\\le2\\pi\\right\\}',
  S: '\\left\\{\\left(\\sin u\\cos v,\\sin u\\sin v,\\cos u\\right)\\mid0\\le u\\le\\pi,0\\le v\\le2\\pi\\right\\}',
  V: '\\left\\{\\left(a,b,c\\right)\\mid0\\le a\\le1,0\\le b\\le a,0\\le c\\le2\\right\\}',
  H: '\\left\\{\\left(\\cos t,\\sin t,t\\right)\\mid0\\le t\\le2\\pi\\right\\}',
  E: '',
})
/** 描くための式（計算エンジン用の文字列）。描けなければ「error: 理由の先頭」、積分が無ければ null */
const drawn = (latex: string) => {
  const v = integralDisplay(latex)
  return v === null ? null : 'error' in v ? `error: ${v.error.slice(0, 12)}` : `${v.field ? '[色] ' : ''}${latexToText(v.latex)}`
}
const integrals: [string, string | null][] = [
  // 範囲が変数の式（1 ≤ x ≤ y）でも、条件をそのまま不等式として描く
  ['\\iint_{D}xy\\,dA', '⦃(x,y,z)∣1<=x<=y,y<=3, min (0,xy)<=z<= max (0,xy)⦄'],
  ['z=\\iint_{D}x-y\\,dA', '⦃(x,y,z)∣1<=x<=y,y<=3, min (0,x-y)<=z<= max (0,x-y)⦄'],
  ['\\iint_{\\left[0,1\\right]\\times\\left[0,2\\right]}xy\\,dx\\,dy', '⦃(x,y,z)∣0<=x<=1,0<=y<=2, min (0,xy)<=z<= max (0,xy)⦄'],
  // 累次積分は、外側の変数から順に範囲を条件にする
  ['\\int_0^1\\int_0^xxy\\,dy\\,dx', '⦃(x,y,z)∣0<=x<=1,0<=y<=x, min (0,xy)<=z<= max (0,xy)⦄'],
  ['\\int_{0}^{1}\\int_{0}^{x}\\int_{0}^{y}xyz\\,dz\\,dy\\,dx', '[色] @cases(xyz, 0<=x<=1,0<=y<=x,0<=z<=y) =0'],
  // 1 変数: 曲線と軸の間（高さ 0 の面）
  ['\\int_{0}^{\\pi}\\sin x\\,dx', '⦃(x,y,z)∣0<=x<= pi , min (0, sin x)<=y<= max (0, sin x),z=0⦄'],
  ['\\int_{0}^{2}t^2\\,dt', '⦃(t,y,z)∣0<=t<=2, min (0,t^(2))<=y<= max (0,t^(2)),z=0⦄'],
  ['\\int_{0}^{x}e^{-t^2}\\,dt', 'error: 範囲か中身に、積分の変数'],
  // 3 重積分: 変数を x, y, z に直して、値を色で塗る
  ['\\iiint_{V}abc\\,dV', '[色] @cases(xyz, 0<=x<=1,0<=y<=x,0<=z<=2) =0'],
  // 被積分関数が定数なら、領域そのもの
  ['\\iiint_{V}2\\,dV', '⦃(a,b,c)∣0<=a<=1,0<=b<=a,0<=c<=2⦄'],
  // 線積分: 曲線の上の幕（媒介変数は u、高さの媒介変数は v）
  ['\\int_{C}x^2\\,ds', '⦃( cos u, sin u,v(( cos u)^(2)))∣0<=u<=2 pi ,0<=v<=1⦄'],
  ['\\oint_{C}\\left(-y,x\\right)\\cdot d\\mathbf{r}', '⦃( cos u, sin u,v((((-( sin u),( cos u)))*( @diff(u) ( cos u, sin u)))/(abs(( @diff(u) ( cos u, sin u))))))∣0<=u<=2 pi ,0<=v<=1⦄'],
  // 空間の曲線: 曲線に垂直で上を向いた向きに広げた帯
  ['\\int_{H}1\\,ds', '⦃( cos u, sin u,u)+v((1))((((0,0,1)-((( @diff(u) ( cos u, sin u,u))*(0,0,1))/(( @diff(u) ( cos u, sin u,u))*( @diff(u) ( cos u, sin u,u))))( @diff(u) ( cos u, sin u,u))))/(abs(((0,0,1)-((( @diff(u) ( cos u, sin u,u))*(0,0,1))/(( @diff(u) ( cos u, sin u,u))*( @diff(u) ( cos u, sin u,u))))( @diff(u) ( cos u, sin u,u))))))∣0<=u<=2 pi ,0<=v<=1⦄'],
  // 点を式で書いた平面の領域（極座標）: 高さが被積分関数の立体
  ['\\iint_{P}x^2+y^2\\,dA', '⦃(u cos v,u sin v,t((u cos v)^(2)+(u sin v)^(2)))∣0<=u<=1,0<=v<=2 pi ,0<=t<=1⦄'],
  ['\\iint_{E}xy\\,dA', 'error: E の範囲を、下の欄に書'],
  ['\\int x^2\\,dx', 'error: 不定積分は描けません（範'],
  ['z=x^2', null],
]
for (const [latex, want] of integrals) check(`積分を描く式 ${latex}`, drawn(latex), want)
// 面積分: 曲面を、法線の向きに被積分関数の分だけ厚くする
check('積分を描く式（面積分）', drawn('\\iint_{S}z^2\\,dS')?.startsWith('⦃( sin u cos v, sin u sin v, cos u)+t(( cos u)^(2))((( @diff(u) ( sin u cos v, sin u sin v, cos u))×( @diff(v) '), true)
check('積分を描く式（流束）', drawn('\\iint_{S}\\left(x,y,z\\right)\\cdot d\\mathbf{S}')?.startsWith('⦃( sin u cos v, sin u sin v, cos u)+t(((( sin u cos v),( sin u sin v),( cos u)))*((('), true)
// 進み具合: 最初の媒介変数の範囲を途中までにし、途中までの積分の式と、今の点・矢印を添える
{
  const v = integralDisplay('\\oint_{H}\\vec{F}\\cdot d\\mathbf{r}', 0.5, 2)
  const ok = v !== null && !('error' in v) ? v : null
  check('進み具合の変数', ok?.sweep, 't')
  check('途中までの積分', ok && latexToText(ok.partial!), '@intset(( cos t, sin t,t),0<=t<=3.141593; {vec})({vec_F})')
  check('添えるもの', ok?.extras?.map((e) => e.role), ['point', 'tangent', 'vector', 'component'])
  check('今の点', ok && latexToText(ok.extras![0].latex), '( cos (3.141593), sin (3.141593),(3.141593))')
  // 被積分関数が媒介変数そのもの（(t + 1, t²)）: 場の矢印は添えず、F の矢印には今の t を入れる
  setRegions({ C: '\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}' })
  const q = integralDisplay('\\oint_{C}\\left(t+1,t^2\\right)\\cdot d\\mathbf{r}', 0.25, 2)
  const okq = q !== null && !('error' in q) ? q : null
  check('媒介変数の被積分関数: 添えるもの', okq?.extras?.map((e) => e.role), ['point', 'tangent', 'vector', 'component'])
  check('媒介変数の被積分関数: F の矢印', okq && /\bt\+1/.test(latexToText(okq.extras![2].latex)), false)
  // ∇ · F⃗ の体積分（媒介変数 3 つ）: 立体を描く
  setRegions({ W: '\\left\\{\\left(r\\cos v,r\\sin v,h\\right)\\mid0\\le r\\le2,0\\le v\\le2\\pi,0\\le h\\le1\\right\\}' })
  const w = integralDisplay('\\iiint_{W}\\nabla\\cdot\\left(x,y,z\\right)\\,dV', 0.5)
  const okw = w !== null && !('error' in w) ? w : null
  check('体積分の立体', okw && latexToText(okw.latex), '⦃(u cos v,u sin v,t)∣0<=u<=1,0<=v<=2 pi ,0<=t<=1⦄')
  check('体積分の場の矢印', okw?.extras?.map((e) => e.role), ['field'])
}
// 太字の 1 文字は、ベクトルの名前（\vec と同じ名前）
check('太字 \\mathbf{F}', latexToText('\\mathbf{F}\\left(x,y,z\\right)=\\left(-y,x,0\\right)'), latexToText('\\vec{F}\\left(x,y,z\\right)=\\left(-y,x,0\\right)'))
check('太字 \\boldsymbol{F}', latexToText('\\boldsymbol{F}'), '{vec_F}')
check('黒板太字 \\mathbb{F}', latexToText('\\oint_{\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le\\pi\\right\\}}\\mathbb{F}\\cdot d\\mathbb{r}'), '@intset(( cos t, sin t),0<=t<= pi; {vec})({vec_F})')
check('黒板太字の集合はそのまま', latexToText('x\\in\\mathbb{Z}'), 'x∈ℤ')
check('太字の名前（2 文字以上）はそのまま', latexToText('\\mathbf{Im}'), 'Im')
setRegions({})

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('latex.ts: すべて成功')

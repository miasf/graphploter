// latex.ts のテスト。実行: npm test
import { conditionHolds, paramConditions, riemannDisplay, fenceBareDelimiters, inlineRegions, integralKinds, joinTuple, latexToText, regionDisplayLatex, regionLabel, regionsUsed, setRegions, splitTuple, integralDisplay } from './latex.ts'

let failures = 0
function check(name: string, got: unknown, want: unknown) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    failures++
    console.log(`NG  ${name}\n    結果: ${JSON.stringify(got)}\n    期待: ${JSON.stringify(want)}`)
  }
}

// LaTeX → 計算エンジン用の式
const cases: [string, string][] = [
  ['\\lim_{x\\to0}\\frac{\\sin x}{x}', '@lim(x,0) ((sin x)/(x))'],
  ['\\lim\\limits_{x\\rightarrow(-1)^{+}}x', '@lim(x,(-1),1) x'],
  ['\\lim_{\\alpha\\to a^{-}}f(\\alpha)', '@lim({alpha},a,-1) f( {alpha} )'],
  ['\\lim_{t\\longrightarrow-\\infty}e^{t}', '@lim(t,-∞) e^(t)'],
  ['\\lim_{k\\to\\infty,\\,k\\in\\mathbb{N}}a_k', '@seqlim(k,∞) a_(k)'],
  ['\\lim_{x\\to0}f\\prime(x+a)', "@lim(x,0) f'(x+a)"],
  // 数学の記法（計画 docs/plan-math-notation.md の M1・M4）
  ['\\sum_{n=1}^{10}n^2', '@sum(n,1,10) n^(2)'],
  // 添字を並べた多重の和: \sum_{x,y,z=0}^{n} は x・y・z それぞれ 0〜n
  ['\\sum_{x,y,z=0}^{2}xyz', '@sum(x,0,2) @sum(y,0,2) @sum(z,0,2) xyz'],
  ['\\prod_{m,n=1}^{3}\\left(m+n\\right)', '@prod(m,1,3) @prod(n,1,3) (m+n)'],
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
  ['\\operatorname{Taylor}\\left(\\sin x,x,0,5\\right)', '@taylor ( sin x,x,0,5)'],
  ['\\operatorname{factor}\\left(x^4-1\\right)', '@factor (x^(4)-1)'],
  ['\\operatorname{jordan}\\left(A\\right)', '@jordan (A)'],
  ['\\operatorname{diagonalize}A', '@diagonalize A'],
  ['\\operatorname{gramschmidt}\\left(\\left(1,1,0\\right),\\left(1,0,1\\right)\\right)', '@gramschmidt ((1,1,0),(1,0,1))'],
  ['\\operatorname{crit}\\left(x^3-3x+y^2\\right)', '@crit (x^(3)-3x+y^(2))'],
  ['\\operatorname{cobweb}\\left(a_1,12\\right)', '@cobweb (a_1,12)'],
  ['\\operatorname{orbit}\\left(a_0,b_0,N\\right)', '@orbit (a_0,b_0,N)'],
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
  ['\\iint_{\\left\\{\\left(x,y\\right)\\mid0\\le x\\le1,0\\le y\\le x\\right\\}}xy\\,dA', '@intset((x,y),0<=x<=1,0<=y<=x; {d_A})(xy)'],
  ['\\iint_{\\left[0,1\\right]\\times\\left[0,2\\right]}xy\\,dx\\,dy', '@intset((x,y),0<=x<=1,0<=y<=2; {d_x},{d_y})(xy)'],
  ['\\iiint_{\\left\\{\\left(x,y,z\\right)\\mid0\\le x\\le1,0\\le y\\le1,0\\le z\\le1\\right\\}}1\\,dV', '@intset((x,y,z),0<=x<=1,0<=y<=1,0<=z<=1; {d_V})(1)'],
  // ベクトル場の演算子・外積・線積分・面積分
  ['z=\\nabla\\cdot\\left(x^2,y^2,0\\right)', 'z= @div (x^(2),y^(2),0)'],
  ['z=\\nabla\\times\\left(-y,x\\right)', 'z= @rot (-y,x)'],
  ['z=\\nabla^{2}x^2y', 'z= @lap x^(2)y'],
  ['\\nabla f', '@grad f'],
  ['z=\\Delta\\left(x^2y\\right)', 'z= @lap (x^(2)y)'],
  ['\\operatorname{div}F', '@div F'],
  ['\\left(1,2,3\\right)\\times\\left(4,5,6\\right)', '(1,2,3)×(4,5,6)'],
  ['\\oint_{\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}}\\left(-y,x\\right)\\cdot d\\mathbf{r}', '@intset(( cos t, sin t),0<=t<=2 pi; {vec},{d_r})((-y,x))'],
  ['\\int_{\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}}x^2\\,ds', '@intset(( cos t, sin t),0<=t<=2 pi; {d_s})(x^(2))'],
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
check('∬_D', latexToText('\\iint_{D}xy\\,dA'), '@intset((x,y),0<=x<=1,0<=y<=x; {d_A})(xy)')
check('∬_D（括弧なし）', latexToText('\\iint_Dxy\\,dA'), '@intset((x,y),0<=x<=1,0<=y<=x; {d_A})(xy)')
check('∮_C F·dr', latexToText('\\oint_{C}\\left(-y,x\\right)\\cdot d\\mathbf{r}'), '@intset(( cos t, sin t),0<=t<=2 pi; {vec},{d_r})((-y,x))')
check('∫_Ω dx dy', latexToText('\\int_{\\Omega}xy\\,dx\\,dy'), '@intset((x,y),0<=x<=1,0<=y<=2; {d_x},{d_y})(xy)')
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
check('省略形（積分）', latexToText('\\iiint_{\\left\\{\\left(x,y,z\\right)\\mid0\\le x,y,z\\le1,x+y\\le z\\right\\}}1\\,dV'), '@intset((x,y,z),0<=x<=1,0<=y<=1,0<=z<=1,x+y<=z; {d_V})(1)')
check('省略形（2 つ）', latexToText('\\left\\{\\left(x,y\\right)\\mid0\\le x,y\\le1\\right\\}'), '⦃(x,y)∣0<=x<=1,0<=y<=1⦄')
check('省略形（集合でない連立）', latexToText('-1\\le x,y\\le1'), latexToText('-1\\le x\\le1,-1\\le y\\le1'))
check('省略形（≦・パラメータの端）', latexToText('\\left\\{\\left(x,y\\right)\\mid-a\\leqq x,y\\leqq a\\right\\}'), '⦃(x,y)∣-a<=x<=a,-a<=y<=a⦄')
check('省略形（範囲を描く式）', regionDisplayLatex('\\left\\{\\left(x,y\\right)\\mid-1\\le x,y\\le1\\right\\}'), regionDisplayLatex('\\left\\{\\left(x,y\\right)\\mid-1\\le x\\le1,-1\\le y\\le1\\right\\}'))
// 省略形にしない: 端に座標がある・向きが違う・同じ文字
check('省略形でない（端に座標）', latexToText('\\left\\{\\left(x,y,z\\right)\\mid x\\le y,z\\le1\\right\\}'), '⦃(x,y,z)∣x<=y,z<=1⦄')
check('省略形でない（向きが違う）', latexToText('0\\le x,y\\ge1'), '0<=x,y>=1')
check('省略形でない（同じ文字）', latexToText('0\\le x,x\\le1'), '0<=x,x<=1')

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
check('積分を描く式（ベクトル場の面積分）', drawn('\\iint_{S}\\left(x,y,z\\right)\\cdot d\\mathbf{S}')?.startsWith('⦃( sin u cos v, sin u sin v, cos u)+t(((( sin u cos v),( sin u sin v),( cos u)))*((('), true)
// 進み具合: 最初の媒介変数の範囲を途中までにし、途中までの積分の式と、今の点・矢印を添える
{
  const v = integralDisplay('\\oint_{H}\\vec{F}\\cdot d\\mathbf{r}', 0.5, 2)
  const ok = v !== null && !('error' in v) ? v : null
  check('進み具合の変数', ok?.sweep, 't')
  check('途中までの積分', ok && latexToText(ok.partial!), '@intset(( cos t, sin t,t),0<=t<=3.14159265359; {vec},{d_r})({vec_F})')
  check('添えるもの', ok?.extras?.map((e) => e.role), ['path', 'point', 'tangent', 'vector', 'component'])
  check('今の点', ok && latexToText(ok.extras![1].latex), '( cos (3.14159265359), sin (3.14159265359),(3.14159265359))')
  // 被積分関数が媒介変数そのもの（(t + 1, t²)）: 場の矢印は添えず、F の矢印には今の t を入れる
  setRegions({ C: '\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}' })
  const q = integralDisplay('\\oint_{C}\\left(t+1,t^2\\right)\\cdot d\\mathbf{r}', 0.25, 2)
  const okq = q !== null && !('error' in q) ? q : null
  check('媒介変数の被積分関数: 添えるもの', okq?.extras?.map((e) => e.role), ['path', 'point', 'tangent', 'vector', 'component'])
  check('媒介変数の被積分関数: F の矢印', okq && /\bt\+1/.test(latexToText(okq.extras![3].latex)), false)
  // 経路は、積分記号の下に直接書いた範囲でも、範囲全体を描く（進み具合に関係なく）
  const inl = integralDisplay('\\oint_{\\left\\{\\left(2\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}}\\left(-y,x\\right)\\cdot d\\mathbf{r}', 0.3)
  const oki = inl !== null && !('error' in inl) ? inl : null
  check('直書きの経路', oki && latexToText(oki.extras!.find((e) => e.role === 'path')!.latex), '⦃(2 cos t, sin t)∣0<=t<=2 pi⦄')
  // 3D 表示の 1 変数の定積分: 軸の上に立てた高さ f（z）の幕（線積分と同じ向き）
  check('1 変数の 3D', (() => { const v = integralDisplay('\\int_{0}^{2}t^2\\,dt', 1, 1, false); return v && !('error' in v) ? latexToText(v.latex) : null })(), '⦃(t,y,z)∣0<=t<=2,y=0, min (0,t^(2))<=z<= max (0,t^(2))⦄')
  check('1 変数の区間', (() => { const v = integralDisplay('\\int_{0}^{2}t^2\\,dt'); return v && !('error' in v) ? latexToText(v.extras![0].latex) : null })(), '⦃(x,y,z)∣0<=x<=2,y=0,z=0⦄')
  // ∇ · 𝐅 の体積分（媒介変数 3 つ）: 立体を描く
  setRegions({ W: '\\left\\{\\left(r\\cos v,r\\sin v,h\\right)\\mid0\\le r\\le2,0\\le v\\le2\\pi,0\\le h\\le1\\right\\}' })
  const w = integralDisplay('\\iiint_{W}\\nabla\\cdot\\left(x,y,z\\right)\\,dV', 0.5)
  const okw = w !== null && !('error' in w) ? w : null
  check('体積分の立体', okw && latexToText(okw.latex), '⦃(u cos v,u sin v,t)∣0<=u<=1,0<=v<=2 pi ,0<=t<=1⦄')
  check('体積分の場の矢印（立体は重ねない）', okw?.extras?.map((e) => e.role), ['field'])
}
// 太字の 1 文字は、ベクトルの名前（\vec と同じ名前）
check('太字 \\mathbf{F}', latexToText('\\mathbf{F}\\left(x,y,z\\right)=\\left(-y,x,0\\right)'), latexToText('\\vec{F}\\left(x,y,z\\right)=\\left(-y,x,0\\right)'))
check('太字 \\boldsymbol{F}', latexToText('\\boldsymbol{F}'), '{vec_F}')
check('黒板太字 \\mathbb{F}', latexToText('\\oint_{\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le\\pi\\right\\}}\\mathbb{F}\\cdot d\\mathbb{r}'), '@intset(( cos t, sin t),0<=t<= pi; {vec},{d_r})({vec_F})')
check('黒板太字の集合はそのまま', latexToText('x\\in\\mathbb{Z}'), 'x∈ℤ')
check('太字の名前（2 文字以上）はそのまま', latexToText('\\mathbf{Im}'), 'Im')
setRegions({})

// MathLive のキーボードで打った集合: { は \left\lbrace … \right\rbrace、集合の中の | は \,\middle\vert\,（\middle{\vert}・\middle|）
const typed: [string, string][] = [
  ['\\left\\lbrace\\left(x,x^2\\right)\\,\\middle{\\vert}\\,0\\le x\\le1\\right\\rbrace', '⦃(x,x^(2))∣0<=x<=1⦄'],
  ['\\left\\lbrace\\left(t,\\sin t\\right)\\middle\\vert0\\le t\\le2\\pi\\right\\rbrace', '⦃(t, sin t)∣0<=t<=2 pi⦄'],
  ['\\left\\lbrace\\left(x,y\\right)\\middle|x^2+y^2<1\\right\\rbrace', '⦃(x,y)∣x^(2)+y^(2)<1⦄'],
  ['\\lbrace\\left(x,y,z\\right)\\mid x=y=z,0\\le x\\le1\\rbrace', '⦃(x,y,z)∣x=y=z,0<=x<=1⦄'],
  ['x\\in\\left\\lbrace1,2,3\\right\\rbrace', 'x∈⦃1,2,3⦄'],
  ['\\int_{\\left\\lbrace\\left(\\cos t,\\sin t\\right)\\,\\middle\\vert\\,0\\le t\\le2\\pi\\right\\rbrace}x^2\\,ds', '@intset(( cos t, sin t),0<=t<=2 pi; {d_s})(x^(2))'],
]
for (const [latex, want] of typed) check(latex, latexToText(latex), want)
check('像集合（教科書の LaTeX）', squash(latexToText('$$ D = \\left\\{(2x,y) \\;\\middle\\vert{}\\; x^2+y^2\\leq\\dfrac{\\sqrt{3}}{2}\\right\\} $$')),
  'D=⦃(2x,y)∣x^(2)+y^(2)<=((sqrt(3))/(2))⦄')
check('平方根を z 座標にした像集合', squash(latexToText('D=\\left\\{\\left(x,y,\\sqrt{1-x^2-y^2}\\right)\\middle|x^2+y^2\\le\\frac{\\sqrt{3}}{2}\\right\\}')),
  'D=⦃(x,y,sqrt(1-x^(2)-y^(2)))∣x^(2)+y^(2)<=((sqrt(3))/(2))⦄')
check('値域とサイズ付き区切り', squash(latexToText('\\bigl\\{\\sqrt{1-x^2-y^2}\\bigm|x^2+y^2\\leq1\\bigr\\}')),
  '⦃sqrt(1-x^(2)-y^(2))∣x^(2)+y^(2)<=1⦄')
check('プレーンテキストの集合と絶対値', squash(latexToText('{|x| | |x|<2}')), '⦃abs(x)∣abs(x)<2⦄')
check('文章の区切り', squash(latexToText('\\{(x^2,y)\\text{ such that }x>0\\text{ and }y<1\\}')),
  '⦃(x^(2),y)∣x>0∧y<1⦄')
check('所属付きの集合', squash(latexToText('\\{(x,y)\\in[0,1]^2:x<y\\}')),
  '⦃(x,y)∣((x,y)∈[0,1]×[0,1])∧(x<y)⦄')
check('添字と上付きの入力順', latexToText('a^{2}_{n+1}=a_n+1'), latexToText('a_{n+1}^{2}=a_n+1'))
check('数列の項のべき乗', latexToText('a_{n+1}^{2}=a_n+1'), 'a_(n+1)^(2)=a_(n)+1')
check('括弧を並べた数列の積', latexToText('\\left(a_{n+1}\\right)\\left(a_n\\right)=6'), '(a_(n+1))(a_(n))=6')
check('分数の左辺と括弧のない正弦', latexToText('\\frac{a_{n+1}}{2}=\\sin a_n+1'), '((a_(n+1))/(2))= sin a_(n)+1')
for (const [latex, want] of [
  ['sin^2{x}', 'sin^(2)(x)'],
  ['\\sin^{2}\\left\\lbrace x\\right\\rbrace', 'sin^(2)(x)'],
  ['\\operatorname{erf}^{-3}\\lbrace x+1\\rbrace', 'erf^(-3)(x+1)'],
  ['f^{p+1}{x,y}', 'f^(p+1)(x,y)'],
  ['f^{p+1}\\{x,g^2\\{y,z\\}\\}', 'f^(p+1)(x,g^(2)(y,z))'],
]) check(latex, squash(latexToText(latex)), want)
// 範囲の欄に打った集合も同じ
setRegions({ C: '\\left\\lbrace\\left(x,y,z\\right)\\,\\middle\\vert\\,x=y=z,0\\le x\\le1\\right\\rbrace' })
check('範囲の欄の \\lbrace・\\middle\\vert', latexToText('\\int_C x\\,ds'), '@intset((x,y,z),x=y=z,0<=x<=1; {d_s})(x)')
check('範囲の欄の \\lbrace を描く式', latexToText(regionDisplayLatex('\\left\\lbrace\\left(t,t^2\\right)\\middle\\vert0\\le t\\le1\\right\\rbrace') ?? ''), '⦃(t,t^(2))∣0<=t<=1⦄')

// 積分の d の後の文字（計算エンジンが範囲の次元と合うかを確かめる）と、曲線の上の P dx + Q dy
setRegions({ C: '\\left\\{\\left(\\cos t,\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}' })
const marks: [string, string][] = [
  ['\\oint_C-y\\,dx+x\\,dy', '( @intset(( cos t, sin t),0<=t<=2 pi; {d_x})(-y) + @intset(( cos t, sin t),0<=t<=2 pi; {d_y})(x) )'],
  ['\\oint_C y^2\\,dx-x\\,dy+1', '( @intset(( cos t, sin t),0<=t<=2 pi; {d_x})(y^(2)) - @intset(( cos t, sin t),0<=t<=2 pi; {d_y})(x) ) +1'],
  // 区間の直積の上の dx（続く + … dy は足さない）
  ['\\int_{\\left[0,1\\right]}x\\,dx+1', '@intset(x,0<=x<=1; {d_x})(x) +1'],
  ['\\iint_{\\left[0,1\\right]\\times\\left[0,2\\right]}xy\\,dA', '@intset((x,y),0<=x<=1,0<=y<=2; {d_A})(xy)'],
  ['\\iint_{\\left\\{\\left(r\\cos\\theta,r\\sin\\theta\\right)\\mid0\\le r\\le1,0\\le\\theta\\le2\\pi\\right\\}}r\\,dr\\,d\\theta', '@intset((r cos theta ,r sin theta ),0<=r<=1,0<= theta <=2 pi; {d_r},{d_theta})(r)'],
  ['\\int_{0}^{\\pi}\\sin\\theta\\,d\\theta', '@int(theta,0,pi)(sin theta)'],
  ['\\iint_{\\left\\{\\left(u,v,uv\\right)\\mid0\\le u\\le1,0\\le v\\le1\\right\\}}1\\,d\\sigma', '@intset((u,v,uv),0<=u<=1,0<=v<=1; {d_sigma})(1)'],
]
for (const [latex, want] of marks) check(latex, latexToText(latex), want)
// 途中までの積分（と 100% の値）は、P dx + Q dy の続く項も含める
{
  const v = integralDisplay('\\oint_C-y\\,dx+x\\,dy', 1)
  const ok = v !== null && !('error' in v) ? v : null
  check('P dx + Q dy の値の式', ok?.partial && latexToText(ok.partial), latexToText('\\oint_C-y\\,dx+x\\,dy').replace(/^\s*\(\s*|\s*\)\s*$/g, ''))
}
setRegions({})
// 積分記号の下に直接書いた範囲（範囲を描くボタン）。名前で書いた範囲・上下の端のある積分は入れない
check(
  '直書きの範囲',
  inlineRegions('\\int_C x\\,ds+\\iint_{\\left[0,1\\right]\\times\\left[0,2\\right]}1\\,dA+\\int_0^1x\\,dx+\\int_{\\left\\lbrace\\left(t,t^2\\right)\\middle\\vert0\\le t\\le1\\right\\rbrace}1\\,ds').map((r) => latexToText(regionDisplayLatex(r) ?? '')),
  ['⦃(x,y,z)∣0<=x<=1,0<=y<=2,z=0⦄', '⦃(t,t^(2))∣0<=t<=1⦄'],
)


// ---- 精査で足した書き方 ----
{
  const S = (p: string, c: string) => `\\left\\{\\left(${p}\\right)\\mid ${c}\\right\\}`
  const circ = S('\\cos t,\\sin t', '0\\le t\\le2\\pi')
  const tri = `${S('t,0', '0\\le t\\le1')}\\cup${S('1-t,t', '0\\le t\\le1')}\\cup${S('0,1-t', '0\\le t\\le1')}`
  const more: [string, string][] = [
    // 片側の省略形 x, y, z ≥ 0・0 ≤ x, y
    [S('x,y,z', 'x+y+z=1,x,y,z\\ge0'), '⦃(x,y,z)∣x+y+z=1,x>=0,y>=0,z>=0⦄'],
    [S('x,y', '0\\le x,y,x+y\\le1'), '⦃(x,y)∣0<=x,0<=y,x+y<=1⦄'],
    // 区間・実数全体の累乗は直積（リストの累乗はそのまま）
    ['\\left(x,y\\right)\\in\\left[0,1\\right]^{2}', '(x,y)∈[0,1]×[0,1]'],
    ['\\iiint_{\\left[0,1\\right]^3}1\\,dV', '@intset((x,y,z),0<=x<=1,0<=y<=1,0<=z<=1; {d_V})(1)'],
    ['\\iint_{\\mathbb{R}^2}e^{-x^2-y^2}\\,dA', '@intset((x,y),-∞<=x<=∞,-∞<=y<=∞; {d_A})(e^(-x^(2)-y^(2)))'],
    ['\\left[1,2\\right]^2', '[1,2]^(2)'],
    // 集合の補集合
    [`${S('x,y', 'x^2+y^2<1')}^{c}`, '⦃(x,y)∣x^(2)+y^(2)<1⦄ᶜ'],
    // \mathrm{d}\mathbf{r}・単位接線 T ds・単位法線 n dS
    [`\\oint_{${circ}}\\left(-y,x\\right)\\cdot\\mathrm{d}\\mathbf{r}`, '@intset(( cos t, sin t),0<=t<=2 pi; {vec},{d_r})((-y,x))'],
    [`\\int_{${circ}}\\left(-y,x\\right)\\cdot\\mathbf{T}\\,ds`, '@intset(( cos t, sin t),0<=t<=2 pi; {vec},{d_r})((-y,x))'],
    [`\\iint_{${S('u,v,0', '0\\le u,v\\le1')}}\\left(0,0,1\\right)\\cdot\\hat{\\mathbf{n}}\\,dS`, '@intset((u,v,0),0<=u<=1,0<=v<=1; {vec},{d_S})((0,0,1))'],
    // 括弧で囲んだ微分形式（括弧の外に d があれば、今までどおり）
    [`\\oint_{${circ}}\\left(-y\\,dx+x\\,dy\\right)`, '( @intset(( cos t, sin t),0<=t<=2 pi; {d_x})(-y) + @intset(( cos t, sin t),0<=t<=2 pi; {d_y})(x) )'],
    ['\\int_0^1\\left(x+1\\right)\\,dx', '@int(x,0,1)((x+1))'],
    // 積分記号の下に条件だけ（点は d の文字か、積分記号の数）
    ['\\iint_{x^2+y^2\\le1}1\\,dx\\,dy', '@intset((x,y),x^(2)+y^(2)<=1; {d_x},{d_y})(1)'],
    ['\\iint_{x^2+y^2\\le1}1\\,dA', '@intset((x,y),x^(2)+y^(2)<=1; {d_A})(1)'],
    ['\\iiint_{x^2+y^2+z^2\\le1}1\\,dV', '@intset((x,y,z),x^(2)+y^(2)+z^(2)<=1; {d_V})(1)'],
    // 集合どうしの ∩（点が同じ変数の組）と、曲線どうしの ∪（範囲ごとの積分の和）
    [`\\iint_{${S('x,y', 'x^2+y^2\\le1')}\\cap${S('x,y', 'y\\ge0')}}1\\,dA`, '@intset((x,y),(x,y)∈⦃(x,y)∣x^(2)+y^(2)<=1⦄ ∩ ⦃(x,y)∣y>=0⦄; {d_A})(1)'],
    [`\\oint_{${tri}}1\\,ds`, '( @intset((t,0),0<=t<=1; {d_s})(1) + @intset((1-t,t),0<=t<=1; {d_s})(1) + @intset((0,1-t),0<=t<=1; {d_s})(1) )'],
  ]
  for (const [latex, want] of more) check(latex, latexToText(latex), want)
  // 添字の付いた範囲の名前 C_1・D_{12}
  check('添字の名前', regionsUsed('\\int_{C_1}x\\,ds+\\iint_{D_{12}}1\\,dA').map((r) => r.name), ['C_1', 'D_12'])
  check('添字の名前の表示', [regionLabel('C_1'), regionLabel('\\Omega_2')], ['C₁', 'Ω₂'])
  setRegions({ C_1: circ })
  check('添字の名前の範囲', latexToText('\\int_{C_1}x^2\\,ds'), '@intset(( cos t, sin t),0<=t<=2 pi; {d_s})(x^(2))')
  setRegions({})
  // 範囲を表示: 条件だけの範囲・集合の演算
  check('条件だけの範囲を表示', inlineRegions('\\iint_{x^2+y^2\\le1}x\\,dA').map((r) => latexToText(regionDisplayLatex(r) ?? '')), ['⦃(x,y,z)∣x^(2)+y^(2)<=1,z=0⦄'])
  check('∩ の範囲を表示', latexToText(regionDisplayLatex(`${S('x,y', 'x^2+y^2\\le1')}\\cap${S('x,y', 'y\\ge0')}`) ?? ''), '⦃(x,y,z)∣x^(2)+y^(2)<=1,z=0⦄ ∩ ⦃(x,y,z)∣y>=0,z=0⦄')
  check('∪ の経路を表示', latexToText(regionDisplayLatex(tri) ?? ''), '⦃(t,0)∣0<=t<=1⦄ ∪ ⦃(1-t,t)∣0<=t<=1⦄ ∪ ⦃(0,1-t)∣0<=t<=1⦄')
  // 積分の図: x + y + z = 1 は z について解いて曲面にする
  const plane = integralDisplay(`\\iint_{${S('x,y,z', 'x+y+z=1,x\\ge0,y\\ge0,z\\ge0')}}1\\,dS`, 1)
  check('x + y + z = 1 の図', plane !== null && !('error' in plane), true)
}

// 入力の途中で崩れた集合の括弧（} を消して打ち直す・積分記号の下で { が添字の括弧になる）は、そろえた形と同じに読む
{
  const fixed: [string, string][] = [
    ['\\left\\lbrace\\left(x,y\\right)\\,\\middle{\\vert}\\,x^2<1\\right.\\rbrace', '\\left\\lbrace\\left(x,y\\right)\\,\\middle{\\vert}\\,x^2<1\\right\\rbrace'],
    ['\\left\\lbrace\\left(x,y\\right)\\,\\middle{\\vert}\\,x^2<1\\right.,y>0\\rbrace', '\\left\\lbrace\\left(x,y\\right)\\,\\middle{\\vert}\\,x^2<1,y>0\\right\\rbrace'],
    ['\\iint_{(x,y)|x^2+y^2<1\\rbrace}1\\,dA', '\\iint_{\\left\\{(x,y)\\mid x^2+y^2<1\\right\\}}1\\,dA'],
    ['\\left(x,y\\right)\\mid x^2<1\\rbrace', '\\left\\{\\left(x,y\\right)\\mid x^2<1\\right\\}'],
    ['A=\\left(x,y\\right)\\mid x^2<1\\rbrace', 'A=\\left\\{\\left(x,y\\right)\\mid x^2<1\\right\\}'],
    ['\\left\\lbrace\\left(x,y\\right)\\,\\middle{\\vert}\\,x^2<1\\right.', '\\left\\lbrace\\left(x,y\\right)\\,\\middle{\\vert}\\,x^2<1\\right\\rbrace'],
  ]
  for (const [broken, good] of fixed) check(`崩れた括弧 ${broken}`, latexToText(broken), latexToText(good))
}

// 定積分の近似（リーマン和・台形則・シンプソン則）
{
  const ok = (m: 'left' | 'right' | 'mid' | 'trap' | 'simpson', n: number) => {
    const r = riemannDisplay('\\int_0^2x^2\\,dx', m, n)
    if (!r || 'error' in r) throw new Error(`近似が作れない: ${m}`)
    return r
  }
  check('左端の和', latexToText(ok('left', 4).sum), '(((2)-(0))/(4)) @sum(k,0,3) @at((x^(2)),x,(0)+(k+0)(((2)-(0))/(4)))')
  check('左端の説明', ok('left', 4).label, '左端のリーマン和（n = 4）')
  check('シンプソンは偶数', ok('simpson', 5).n, 6)
  check('近似の図は集合', latexToText(ok('mid', 3).latex).startsWith('⦃(x,y,z)∣(0)<=x<=(2),'), true)
  check('正確な積分', latexToText(ok('trap', 3).exact), '@int(x,0,2)(x^(2))')
  // 中身に k がある式では、和の添字を別の文字にする
  const rk = riemannDisplay('\\int_0^1kx\\,dx', 'left', 2)
  check('添字の文字', rk !== null && !('error' in rk) && latexToText(rk.sum).includes('@sum(j,'), true)
  // 描けない形
  const err = (l: string) => {
    const r = riemannDisplay(l, 'left', 4)
    return r !== null && 'error' in r
  }
  check('2 重積分は近似しない', err('\\iint_{x^2+y^2\\le1}1\\,dA'), true)
  check('端に座標がある', err('\\int_0^xt\\,dt'), true)
  check('不定積分', err('\\int x\\,dx'), true)
}

// 積分の分類（何の積分か）: 積分記号は自由に書き、範囲の次元と被積分関数の形から決める
{
  const regions = {
    C: '\\left\\{\\left(x,y\\right)\\mid x^2+y^2=1\\right\\}',
    P: '\\left\\{\\left(\\cos t,\\sin t,t\\right)\\mid 0\\le t\\le 2\\pi\\right\\}',
    S: '\\left\\{\\left(x,y,z\\right)\\mid x^2+y^2+z^2=1\\right\\}',
    T: '\\left\\{\\left(\\sin u\\cos v,\\sin u\\sin v,\\cos u\\right)\\mid 0\\le u\\le\\pi,0\\le v\\le 2\\pi\\right\\}',
    D: '\\left\\{\\left(r\\cos\\theta,r\\sin\\theta\\right)\\mid 0\\le r\\le1,0\\le\\theta\\le2\\pi\\right\\}',
    V: '\\left\\{\\left(x,y,z\\right)\\mid x^2+y^2+z^2\\le1\\right\\}',
  }
  const kind = (l: string) => integralKinds(l, regions).map((k) => k.kind)
  check('定積分', kind('\\int_{0}^{1}x\\,dx'), ['定積分'])
  check('累次積分', kind('\\int_{0}^{1}\\int_{0}^{x}xy\\,dy\\,dx'), ['累次積分（2 重）'])
  check('スカラー場の線積分（等式の曲線）', kind('\\int_{C}x^2\\,ds'), ['スカラー場の線積分（弧長についての線積分）'])
  check('ベクトル場の線積分（∮）', kind('\\oint_{C}\\left(-y,x\\right)\\cdot d\\mathbf{r}'), ['ベクトル場の線積分'])
  check('座標についての線積分', kind('\\int_{C}-y\\,dx+x\\,dy'), ['座標についての線積分（P dx + Q dy の形）'])
  check('空間の曲線', integralKinds('\\int_{P}z\\,ds', regions)[0]?.region, '媒介変数表示の空間の曲線（媒介変数 t）')
  check('スカラー場の面積分（等式の曲面）', integralKinds('\\iint_{S}1\\,dS', regions)[0]?.region, '等式で書いた曲面（集合の書き方。媒介変数を使わない）')
  check('ベクトル場の面積分', kind('\\oiint_{T}\\left(x,y,z\\right)\\cdot d\\mathbf{S}'), ['ベクトル場の面積分'])
  check('極座標の 2 重積分', kind('\\iint_{D}1\\,dA'), ['2 重積分（平面の領域の上の積分）'])
  check('媒介変数についての積分', kind('\\iint_{D}r\\,dr\\,d\\theta'), ['媒介変数についての積分'])
  check('発散の体積分', kind('\\iiint_{V}\\nabla\\cdot\\left(x,y,z\\right)\\,dV'), ['3 重積分（体積分）: 発散 ∇ · F の体積分'])
  // ∬ で書いても、範囲が曲線なら線積分（記号ではなく範囲で決める）
  check('記号に依らない', kind('\\iint_{C}1\\,ds'), ['スカラー場の線積分（弧長についての線積分）'])
  check('2 つの積分', kind('\\int_{C}1\\,ds+\\iiint_{V}1\\,dV').length, 2)
  check('範囲が空', kind('\\iint_{Q}1\\,dA'), ['範囲を書くと決まります'])
}

// 等式で書いた曲線・曲面（媒介変数なし）の上の積分の図: 幕・殻を、集合の条件のまま描く
{
  setRegions({ C: '\\left\\{\\left(x,y\\right)\\mid x^2+y^2=1\\right\\}', S: '\\left\\{\\left(x,y,z\\right)\\mid x^2+y^2+z^2=1\\right\\}' })
  const drawn = (l: string) => {
    const d = integralDisplay(l)
    return d === null || 'error' in d ? d : latexToText(d.latex)
  }
  check('等式の曲線の上の幕', drawn('\\int_{C}x^2\\,ds'), '⦃(x,y,z)∣x^(2)+y^(2)=1, min (0,x^(2))<=z<= max (0,x^(2))⦄')
  check('等式の曲面の殻', String(drawn('\\iint_{S}1\\,dS')).startsWith('⦃(x,y,z)∣min (0,1)<=(((x^(2)+y^(2)+z^(2))-(1))/(abs(( @diff(x)'), true)
  check('P dx + Q dy の幕', String(drawn('\\int_{C}-y\\,dx+x\\,dy')).includes('(-y)(- @diff(y)'), true)
  setRegions({})
}

// スライダーの値の条件（不等式だけの行）
{
  check('ab > 1', JSON.stringify(paramConditions('ab>1')), JSON.stringify([{ parts: ['ab', '1'], rels: ['>'] }]))
  check('連鎖とカンマ', JSON.stringify(paramConditions('0<a<1,\\ b\\ge1')), JSON.stringify([{ parts: ['0', 'a', '1'], rels: ['<', '<'] }, { parts: ['b', '1'], rels: ['>='] }]))
  check('かつ', paramConditions('a<b\\land b<2')?.length, 2)
  check('等式は条件でない', paramConditions('a=1'), null)
  check('集合は条件でない', paramConditions('\\left\\{(x,y)\\mid x<a\\right\\}'), null)
  check('成り立つ', conditionHolds(['<', '<'], [0, 0.5, 1]), true)
  check('境目（<）', conditionHolds(['<'], [1, 1]), false)
  check('境目（≤）', conditionHolds(['<='], [1, 1]), true)
  check('値が無い', conditionHolds(['>'], [NaN, 1]), false)
}

// 第2段階: 積分の図と近似（#193 #195 #197 #179 #196 #199 #130 #178 #194 #207）
{
  const view = (latex: string, progress = 1, flat = true) => integralDisplay(latex, progress, 1, flat)
  const err = (v: ReturnType<typeof integralDisplay> | ReturnType<typeof riemannDisplay>) => (v && 'error' in v ? v.error : null)
  // #193: 微小区間の途中を 0 に丸めない
  const tiny = view('\\int_0^{0.0000004}1\\,dt', 0.5)
  check('#193 微小区間の途中', tiny && !('error' in tiny) && latexToText(tiny.partial!), latexToText('\\int_{\\left\\{\\left(t\\right)\\mid 0\\le t\\le 0.0000002\\right\\}}1\\,dt'))
  const far = view('\\int_{1000}^{1000.000004}1\\,dt', 0.5)
  check('#193 大きい基準値の近く', far && !('error' in far) && far.partial!.includes('1000.000002'), true)
  // #195: \intop・\smallint でも近似を作れる
  for (const sym of ['\\intop', '\\smallint']) {
    const r = riemannDisplay(`${sym}_0^1 t^2\\,dt`, 'simpson', 8)
    check(`#195 ${sym} の近似`, r && !('error' in r) && latexToText(r.sum), latexToText((riemannDisplay('\\int_0^1 t^2\\,dt', 'simpson', 8) as { sum: string }).sum))
  }
  // #197: 括弧で囲んだ内側の積分も、同じ領域
  const plain = view('\\int_0^1\\int_0^x y\\,dy\\,dx')
  for (const wrapped of ['\\int_0^1\\left(\\int_0^x y\\,dy\\right)dx', '\\int_0^1(\\int_0^x y\\,dy)dx']) {
    const w = view(wrapped)
    check(`#197 ${wrapped}`, w && !('error' in w) && w.latex, plain && !('error' in plain) && plain.latex)
  }
  // #196: 逆向きの不等号・片側ずつの条件でも、進み具合を動かせる
  const sq = (c: string) => view(`\\iint_{\\left\\{\\left(x,y\\right)\\mid ${c}\\right\\}}1\\,dA`, 0.5)
  const base = sq('0\\le x\\le1,0\\le y\\le1')
  for (const c of ['1\\ge x\\ge0,1\\ge y\\ge0', '0\\le x,x\\le1,0\\le y,y\\le1', 'x\\ge0,1\\ge x,y\\ge0,y\\le1']) {
    const v = sq(c)
    check(`#196 ${c}: 掃く変数`, v && !('error' in v) && v.sweep, 'x')
    check(`#196 ${c}: 途中の図`, v && !('error' in v) && latexToText(v.latex).includes('x<=0.5'), true)
  }
  check('#196 ふつうの書き方は今までどおり', base && !('error' in base) && base.sweep, 'x')
  // #199: 添字の付いた x_1 は座標の x ではない
  check('#199 積分の図', err(view('\\int_0^{x_1}t^2\\,dt')), null)
  check('#199 近似', err(riemannDisplay('\\int_0^{x_1}t^2\\,dt', 'simpson', 8)), null)
  check('#199 本当の座標 x は今までどおり描けない', err(view('\\int_0^{x}t^2\\,dt'))?.includes('積分の変数でない x'), true)
  // #194: 上の端と中身が同じ名前でも、上の端の x は座標（どちらの書き方も同じ扱い）
  check('#194 ∫_0^x x² dx', err(view('\\int_0^x x^2\\,dx')), err(view('\\int_0^x t^2\\,dt')))
  check('#194 近似も同じ', err(riemannDisplay('\\int_0^x x^2\\,dx', 'left', 4)), err(riemannDisplay('\\int_0^x t^2\\,dt', 'left', 4)))
  // #207: 内側と外側が同じ名前の累次積分は、内側を付け替えて三角形の領域
  const same = view('\\int_0^1\\int_0^x x\\,dx\\,dx')
  check('#207 同じ名前の累次積分', err(same), null)
  check('#207 同じ名前の累次積分: 2 重', same && !('error' in same) && /<=x<=1/.test(latexToText(same.latex)), true)
  // #207・#130: 括弧で囲んだ P dx + Q dy は、括弧なしと同じ
  const circle = '\\{(\\cos t,\\sin t)\\mid 0\\le t\\le2\\pi\\}'
  const bare = latexToText(`\\oint_{${circle}}-y\\,dx+x\\,dy`)
  for (const form of [`(-y\\,dx+x\\,dy)`, `\\left(-y\\,dx+x\\,dy\\right)`]) {
    check(`#130 ${form} の計算`, latexToText(`\\oint_{${circle}}${form}`), bare)
    const v = view(`\\oint_{${circle}}${form}`)
    check(`#207 ${form} の図`, err(v), null)
  }
  check('#130 ふつうの括弧の被積分関数は外さない', latexToText('\\int_0^1(x+1)\\,dx'), '@int(x,0,1)((x+1))')
  // #207: z 軸に平行な線でも帯を作る（基準を e_x にする）
  check('#207 縦の線の帯', view('\\int_{\\{(0,0,t)\\mid 0\\le t\\le1\\}}1\\,ds', 1, false) && latexToText((view('\\int_{\\{(0,0,t)\\mid 0\\le t\\le1\\}}1\\,ds', 1, false) as { latex: string }).latex).includes('(1,0,0)'), true)
  check('#207 ふつうの曲線は上向きのまま', latexToText((view('\\int_{\\{(\\cos t,\\sin t,t)\\mid 0\\le t\\le1\\}}1\\,ds', 1, false) as { latex: string }).latex).includes('(0,0,1)'), true)
  // #178: 近似の Σ の添字は、利用者の k, j, m, p, q と重ねない。中身の Σ_{t=1}^3 の t は置き換えない
  const used = riemannDisplay('\\int_0^1(k+j+m+p+q+t)\\,dt', 'simpson', 8)
  const idx = used && !('error' in used) ? /\\sum_\{(\w)=/.exec(used.sum)?.[1] : null
  check('#178 添字は使っていない名前', idx !== null && !'kjmpqt'.includes(idx!), true)
  const inner = riemannDisplay('\\int_0^1\\sum_{t=1}^3 t\\,dt', 'simpson', 8)
  check('#178 中の Σ をこわさない', inner && !('error' in inner) && latexToText(inner.sum).includes('@at('), true)
  // #207: 添字の付いた t_1 は、積分の変数 t として置き換えない
  const sub = riemannDisplay('\\int_0^1 t_1 t\\,dt', 'simpson', 8)
  check('#207 t_1 を残す', sub && !('error' in sub) && latexToText(sub.sum).includes('t_1'), true)
  // #179: 上下の端が逆でも、図は小さいほうから大きいほう。等しければ図なし
  const rev = riemannDisplay('\\int_1^0 t^2\\,dt', 'simpson', 8)
  check('#179 逆向きの図の区間', rev && !('error' in rev) && latexToText(rev.latex).includes('(0)<=t<=(1)'), true)
  check('#179 逆向きの説明', rev && !('error' in rev) && rev.label.includes('符号を逆'), true)
  const eq = riemannDisplay('\\int_1^1 t^2\\,dt', 'left', 4)
  check('#179 等しい端', eq && !('error' in eq) && [eq.latex, eq.sum, eq.exact], ['', '0', '0'])
  const sym = riemannDisplay('\\int_a^b t^2\\,dt', 'left', 4)
  check('#179 文字の端は min・max', sym && !('error' in sym) && latexToText(sym.latex).includes('min'), true)
}

// 第2段階 2b: コーシーの主値 PV（ふつうの積分では主値を出さず、PV と書いたときだけ）
{
  for (const [input, want] of [
    ['\\operatorname{PV}\\int_{-1}^{1}\\frac{1}{t}\\,dt', '@pvint(t,-1,1)(((1)/(t)))'],
    ['\\mathrm{PV}\\int_{-1}^{1}\\frac{1}{t}\\,dt', '@pvint(t,-1,1)(((1)/(t)))'],
    ['\\mathrm{p.v.}\\int_{-1}^{2}\\frac{1}{t}\\,dt', '@pvint(t,-1,2)(((1)/(t)))'],
    ['PV\\int_{-1}^{1}\\frac{1}{t}\\,dt', '@pvint(t,-1,1)(((1)/(t)))'],
    ['\\operatorname{PV}\\left(\\int_{-1}^{1}\\frac{1}{t}\\,dt\\right)', '( @pvint(t,-1,1)(((1)/(t))) )'],
    ['\\int_{-1}^{1}\\frac{1}{t}\\,dt', '@int(t,-1,1)(((1)/(t)))'],
  ] as const) check(`PV ${input}`, latexToText(input), want)
  // 積分の前でなければ、P と V のまま
  check('PV の名前', latexToText('P V x'), 'PVx')
  for (const bad of ['\\operatorname{PV}\\int\\frac1t\\,dt', '\\operatorname{PV}\\iint_{[0,1]\\times[0,1]}x\\,dA']) {
    let msg = ''
    try {
      latexToText(bad)
    } catch (e) {
      msg = (e as Error).message
    }
    check(`PV は 1 重の定積分だけ ${bad}`, msg.includes('1 重の定積分'), true)
  }
  const v = integralDisplay('\\operatorname{PV}\\int_{-1}^{2}\\frac{1}{t}\\,dt', 0.5, 1, true)
  check('PV の途中の積分', v && !('error' in v) && latexToText(v.partial!), '@pvint(t,-1,0.5)(((1)/(t)))')
  const r = riemannDisplay('\\operatorname{PV}\\int_{-1}^{2}\\frac{1}{t}\\,dt', 'left', 4)
  check('PV の近似は描かない', r !== null && 'error' in r, true)
}

// 第3段階: 場合分けの中の不等号で、スライダーの条件を区切らない（#200）
check('#200 場合分けの条件', paramConditions('\\begin{cases}1&a<0\\\\2&a\\ge0\\end{cases}<3')?.map((c) => [c.parts.length, c.rels]), [[2, ['<']]])
check('#200 連鎖はそのまま', paramConditions('0<a<3')?.map((c) => c.rels), [['<', '<']])
check('#200 場合分けと連鎖', paramConditions('\\begin{cases}1&a<0\\\\2&a\\ge0\\end{cases}<b<3')?.map((c) => c.rels), [['<', '<']])
check('#200 括弧の中の不等号も数えない', paramConditions('\\left(a+1\\right)<2')?.map((c) => c.rels), [['<']])

// 第4段階 4a: 教科書・貼り付けた LaTeX の書き方
// #187 数式スタイルの指定は読み飛ばす
for (const style of ['\\displaystyle', '\\textstyle', '\\scriptstyle', '\\scriptscriptstyle']) {
  check(`#187 ${style}`, latexToText(`y=${style} \\frac{1}{2}x`), latexToText('y=\\frac{1}{2}x'))
}
// #181 直立体の微分記号
for (const d of ['\\mathrm{d}t', '\\operatorname{d}t', '\\mathrm{dt}', '\\text{d}t', '\\operatorname{dt}', '\\mathit{d}t']) {
  check(`#181 ${d}`, latexToText(`\\int_0^1 t^2\\,${d}`), '@int(t,0,1)(t^(2))')
}
check('#181 重積分', latexToText('\\int_0^1\\int_0^1 xy\\,\\mathrm{dx}\\,\\mathrm{dy}'), latexToText('\\int_0^1\\int_0^1 xy\\,dx\\,dy'))
check('#181 名前の中の d', latexToText('\\int_0^1 d^2\\,\\mathrm{dd}'), '@int(d,0,1)(d^(2))')
check('#181 d で始まる名前は変えない', latexToText('\\mathrm{dim}'), '@dim')
// #163 添字・積分の変数の i（計算エンジンが中身の i を変数にする）
check('#163 Σ の i', latexToText('a=\\sum_{i=1}^5 i'), 'a= @sum(i,1,5) i')
check('#163 ∫ di', latexToText('a=\\int_0^1 i\\,di'), 'a= @int(i,0,1)(i)')
// #164 逆三角関数・底のある対数
check('#164 sin⁻¹', latexToText('a=\\sin^{-1}\\left(\\frac{1}{2}\\right)'), 'a= arcsin (((1)/(2)))')
check('#164 cos⁻¹', latexToText('a=\\cos^{-1}(1/2)'), 'a= arccos (1/2)')
check('#164 tan^-1', latexToText('y=\\tan^-1x'), 'y= arctan x')
check('#164 tanh⁻¹', latexToText('y=\\tanh^{-1}x'), 'y= arctanh x')
check('#164 sin² は 2 乗のまま', latexToText('y=\\sin^2 x'), 'y= sin ^(2)x')
check('#164 逆数は括弧で', latexToText('a=\\left(\\sin x\\right)^{-1}'), 'a=( sin x)^(-1)')
check('#164 log₂', latexToText('a=\\log_{2}8'), 'a= @logbase(2) 8')
check('#164 log₂(1/8)', latexToText('a=\\log_2\\left(\\frac{1}{8}\\right)'), 'a= @logbase(2) (((1)/(8)))')
check('#164 底のない log は常用対数のまま', latexToText('y=\\log x'), 'y= log x')
let secError = ''
try {
  latexToText('a=\\sec^{-1}2')
} catch (e) {
  secError = (e as Error).message
}
check('#164 sec⁻¹ は書き方を案内', secError.includes('\\arccos\\left(\\frac{1}{x}\\right)'), true)

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('latex.ts: すべて成功')

// 数式キーボードの「∑∫∈」のタブのキー（微積・ベクトル・集合と論理の雛形）。
// MathLive を読み込まない（テスト calcKeys.test.ts から、全部のキーが変換できることを確かめるため）。

/** キー 1 つ（MathLive の VirtualKeyboardKeycap の一部） */
export interface CalcKey {
  label: string
  insert: string
  tooltip: string
  class?: string
  width?: 2
  /** 長押しで出る別のキー（入れる LaTeX） */
  variants?: string[]
}

/** 矢印のベクトルの名前のキーの表示（矢印を重ねる文字 U+20D7 は字体によって □ になるので、→ を文字の上に重ねる） */
export const vecLabel = (c: string) =>
  `<span style="display:inline-flex;flex-direction:column;align-items:center;line-height:0.75"><span style="font-size:0.7em">→</span><i>${c}</i></span>`

/** 雛形のキー（label はキーの表示、insert は入れる LaTeX（#? は空欄、#0 は選んでいた部分）、ja は長押しで出る名前） */
function key(label: string, insert: string, ja: string, small = false): CalcKey {
  // 全角の文字は、欧文の文字のおよそ 2 文字分として幅を決める。
  const length = [...label.replace(/<[^>]*>/g, '').replace(/[\u3000-\u9fff]/g, 'aa')].length
  const withSubscripts = label.replace(/_([A-Za-z])/g, '<sub>$1</sub>')
  return {
    label: withSubscripts === label ? label : `<span>${withSubscripts}</span>`,
    insert,
    tooltip: ja,
    ...(small || length > 2 ? { class: 'small' } : {}),
    ...(small || length >= 4 ? { width: 2 } : {}),
  }
}

/**
 * 「∑∫∈」のタブ: 数学の記法（微積・ベクトル・集合と論理）の雛形。スマホで探さずに入れられるように、abc の奥ではなくここにまとめる。
 * 長いラベルにはキー 2 個分の幅を使う。ページは下の段の ◀ ▶ でめくる
 */
export const CALC_PAGES: { title: string; rows: CalcKey[][] }[] = [
  {
    title: '微積',
    rows: [
      [
        key('Σ', '\\sum_{n=1}^{#?}#?', '総和 Σ（中身は直後の項）'),
        key('Π', '\\prod_{n=1}^{#?}#?', '総積 Π'),
        key('∫ₐᵇ', '\\int_{#?}^{#?}#?\\,dx', '定積分'),
        key('∫dx', '\\int #?\\,dx', '不定積分（積分定数を省く）'),
        key('d/dx', '\\frac{d}{dx}#?', '微分'),
        key('∂/∂x', '\\frac{\\partial}{\\partial x}#?', '偏微分'),
        key('d²/dx²', '\\frac{d^{2}}{dx^{2}}#?', '2 階の微分'),
        key("f′", "#@^{\\prime}", '導関数 f′（定義した関数の後ろに）'),
      ],
      [
        key('Δₓ', '\\Delta_{x}#?', '前進差分 f(x+h) − f(x)（幅は Δ_{x, h}）'),
        key('∇ₓ', '\\nabla_{x}#?', '後退差分 f(x) − f(x−h)'),
        key('δₓ', '\\delta_{x}#?', '中心差分 f(x+h/2) − f(x−h/2)'),
        key('∬_D', '\\iint_{D}#?\\,dA', '平面の領域 D の上の 2 重積分（D の範囲は式の下の欄に書く。線積分・面積分は「積分」のページ）'),
        key('∭_V', '\\iiint_{V}#?\\,dV', '空間の領域 V の上の 3 重積分（V の範囲は式の下の欄に書く）'),
      ],
      [
        key('{⋮', '\\begin{cases}#? & #?\\\\ #? & \\text{otherwise}\\end{cases}', '場合分け（値 & 条件）'),
        key('±', '\\pm', 'プラスマイナス（両方の符号を描く）'),
        key('∞', '\\infty', '無限大（積分の端・区間の端）'),
        key('…', '\\ldots', 'リストの省略 [1, 2, …, 10]'),
        key('x°', '#@^{\\circ}', '度'),
        key('f∘g', '\\circ', '関数の合成'),
        key('Σ∈', '\\sum_{n\\in#?}#?', '集合の上の総和 Σ_{n ∈ S}（S は {1, 4, 9}・1 ≤ n ≤ 10, …・d ∣ 12 など）'),
        key('f(x)=', 'f\\left(x\\right)=#?', '関数の定義'),
        key('Tₙ', '\\operatorname{Taylor}\\left(#?,x,0,#?\\right)', 'テイラー多項式 Taylor(f, x, a, n)（中心 a・n 次。多変数は Taylor(f, (x, y), (a, b), n)。説明に多項式が出る。n をスライダーにすると次数を動かせる）'),
        key('crit', '\\operatorname{crit}#?', '停留点 crit f（∇f = 0 の点を探し、ヘッセ行列で極大・極小・鞍点に分ける。1 行にそれだけ。説明に一覧、図に点）'),
      ],
    ],
  },
  {
    // 積分を、範囲の次元で並べる: 1 段目は 1 次元（区間 → 曲線）、2 段目は 2 次元（平面の領域 → 曲面）、3 段目は 3 次元（空間の領域）。
    // 線積分・面積分は、定積分・2 重積分の範囲（まっすぐな区間・平らな領域）を、曲がった曲線・曲面に広げたもの。
    // どの積分も、被積分関数がスカラー場 f（長さ・面積・体積で足す）か、ベクトル場 F（接線・法線の成分を足す。向きに依る）かの 2 通り。
    // 範囲 C・S・D・V は式の下の欄に、集合 {(x, y) | x² + y² = 1} でも、媒介変数表示 {(cos t, sin t) | 0 ≤ t ≤ 2π} でも書ける
    title: '積分',
    rows: [
      [
        key('∫ₐᵇ', '\\int_{#?}^{#?}#?\\,dx', '定積分 ∫_a^b f dx（区間 [a, b] の上の積分。1 次元の範囲）'),
        key('∫dx', '\\int #?\\,dx', '不定積分（積分定数を省く）'),
        {
          ...key('∫_C f ds', '\\int_{C}#?\\,ds', 'スカラー場の線積分 ∫_C f ds = ∫_a^b f(r(t)) |r′(t)| dt（弧長についての積分。f = 1 なら C の長さ。C は式の下の欄に書く。長押しで ∮）'),
          variants: ['\\oint_{C}#?\\,ds'],
        },
        {
          ...key('∫_C F·dr', '\\int_{C}#?\\cdot d\\mathbf{r}', 'ベクトル場の線積分 ∫_C F · dr = ∫_a^b F(r(t)) · r′(t) dt（接線成分の積分。C の向きで符号が変わる。F は (P, Q, R) か、定義したベクトル場 F⃗。長押しで ∮）'),
          variants: ['\\oint_{C}#?\\cdot d\\mathbf{r}'],
        },
        key('∮_C F·dr', '\\oint_{C}#?\\cdot d\\mathbf{r}', '閉じた曲線の上のベクトル場の線積分 ∮_C F · dr（計算は ∫ と同じ。平面ならグリーンの定理で ∬_D (Q_x − P_y) dA と等しい）'),
        key('∫dx+dy', '\\int_{C}#?\\,dx+#?\\,dy', '座標についての線積分 ∫_C P dx + Q dy = ∫_a^b (P x′(t) + Q y′(t)) dt（(P, Q) · dr と同じ。向きに依る）'),
      ],
      [
        key('∬_D f dA', '\\iint_{D}#?\\,dA', '2 重積分 ∬_D f dA（平面の領域 D の上の積分。D を媒介変数で書くと変数変換 |det J| du dv になる）'),
        {
          ...key('∬_S f dS', '\\iint_{S}#?\\,dS', 'スカラー場の面積分 ∬_S f dS = ∬ f(r(u, v)) |r_u × r_v| du dv（面積についての積分。f = 1 なら S の面積。S は式の下の欄に書く。長押しで ∯）'),
          variants: ['\\oiint_{S}#?\\,dS'],
        },
        {
          ...key('∬_S F·dS', '\\iint_{S}#?\\cdot d\\mathbf{S}', 'ベクトル場の面積分 ∬_S F · dS = ∬ F(r(u, v)) · (r_u × r_v) du dv（法線成分の積分。法線の向きで符号が変わる。長押しで ∯）'),
          variants: ['\\oiint_{S}#?\\cdot d\\mathbf{S}'],
        },
        key('∯_S F·dS', '\\oiint_{S}#?\\cdot d\\mathbf{S}', '閉じた曲面の上のベクトル場の面積分 ∯_S F · dS（計算は ∬ と同じ。外向きの法線なら、ガウスの発散定理で ∭_V ∇ · F dV と等しい）'),
        key('∬ f dx dy', '\\iint_{S}#?\\,dx\\,dy', '座標についての面積分 ∬_S f dx dy = ∬ f (r_u × r_v)_z du dv（曲面を xy 平面に映した、符号付きの面積で積分する。向きに依る）'),
      ],
      [
        key('∭_V f dV', '\\iiint_{V}#?\\,dV', '3 重積分（体積分）∭_V f dV（空間の領域 V の上の積分。f = 1 なら V の体積。V を媒介変数で書くと |det J| du dv dw）'),
        key('∭∇·F dV', '\\iiint_{V}\\nabla\\cdot #?\\,dV', '発散の体積分 ∭_V ∇ · F dV（ガウスの発散定理で、V の境界の面の外向きの面積分 ∯ F · dS と等しい）'),
        key('f=', 'f\\left(x,y,z\\right)=#?', 'スカラー場の定義 f(x, y, z) = …（積分の中で f と書ける）'),
        key(`${vecLabel('F')}=`, '\\vec{F}\\left(x,y,z\\right)=\\left(#?,#?,#?\\right)', 'ベクトル場の定義 F⃗(x, y, z) = (P, Q, R)（積分の中で F⃗ と書ける。ふつうの f・F とは別の名前）'),
        key('∇f', '\\nabla #?', '勾配 ∇f（スカラー場からベクトル場。∫_C ∇f · dr = f(終点) − f(始点)）'),
        key('∇·F', '\\nabla\\cdot #?', '発散 ∇ · F（ベクトル場からスカラー場）'),
        key('∇×F', '\\nabla\\times #?', '回転 ∇ × F（ストークスの定理: ∬_S (∇ × F) · dS = ∮_{∂S} F · dr）'),
      ],
    ],
  },
  {
    // 積分の範囲（式の下の欄）の書き方: 集合の書き方（等式・不等式）と、媒介変数表示。どの積分にも、どちらでも書ける
    title: '範囲',
    rows: [
      [
        key('{(x,y)|}', '\\left\\{\\left(x,y\\right)\\mid #?\\right\\}', '平面の集合 {(x, y) | 条件}（不等式なら領域、等式 x² + y² = 1 なら曲線）'),
        key('{(x,y,z)|}', '\\left\\{\\left(x,y,z\\right)\\mid #?\\right\\}', '空間の集合 {(x, y, z) | 条件}（不等式なら立体、等式 1 つなら曲面、等式 2 つなら曲線）'),
        key('F=0', '\\left\\{\\left(x,y\\right)\\mid #?=#?\\right\\}', '等式で書いた平面の曲線 {(x, y) | F = G}（線積分の向きは、F < G の側を左に見る向き）'),
        key('F=0₃', '\\left\\{\\left(x,y,z\\right)\\mid #?=#?\\right\\}', '等式で書いた曲面 {(x, y, z) | F = G}（法線は、F − G が増える向き。球面 x² + y² + z² = 1 なら外向き）'),
        key('[a,b]×[c,d]', '\\left[#?,#?\\right]\\times\\left[#?,#?\\right]', '区間の直積（長方形）'),
      ],
      [
        key('r(t)', '\\left\\{\\left(#?,#?\\right)\\mid #?\\le t\\le #?\\right\\}', '媒介変数表示の平面の曲線 {(x(t), y(t)) | a ≤ t ≤ b}（向きは t が増える向き）'),
        key('r(t)₃', '\\left\\{\\left(#?,#?,#?\\right)\\mid #?\\le t\\le #?\\right\\}', '媒介変数表示の空間の曲線 {(x(t), y(t), z(t)) | a ≤ t ≤ b}'),
        key('r(u,v)', '\\left\\{\\left(#?,#?,#?\\right)\\mid #?\\le u\\le #?,#?\\le v\\le #?\\right\\}', '媒介変数表示の曲面 {r(u, v) | 範囲}（法線の向きは r_u × r_v）'),
        key('r(r,θ)', '\\left\\{\\left(r\\cos\\theta,r\\sin\\theta\\right)\\mid 0\\le r\\le #?,0\\le\\theta\\le 2\\pi\\right\\}', '極座標で書いた平面の領域（2 重積分の変数変換。dA = r dr dθ）'),
        key('r(u,v,w)', '\\left\\{\\left(#?,#?,#?\\right)\\mid #?\\le u\\le #?,#?\\le v\\le #?,#?\\le w\\le #?\\right\\}', '媒介変数表示の立体 {r(u, v, w) | 範囲}（3 重積分の変数変換。dV = |det J| du dv dw）'),
      ],
      [
        key('円', '\\left\\{\\left(\\cos t,\\sin t\\right)\\mid 0\\le t\\le 2\\pi\\right\\}', '単位円（媒介変数表示。反時計回り）'),
        key('円=', '\\left\\{\\left(x,y\\right)\\mid x^{2}+y^{2}=1\\right\\}', '単位円（等式。反時計回り）'),
        key('球面', '\\left\\{\\left(\\sin u\\cos v,\\sin u\\sin v,\\cos u\\right)\\mid 0\\le u\\le\\pi,0\\le v\\le 2\\pi\\right\\}', '単位球面（媒介変数表示。法線は外向き）'),
        key('球面=', '\\left\\{\\left(x,y,z\\right)\\mid x^{2}+y^{2}+z^{2}=1\\right\\}', '単位球面（等式。法線は外向き）'),
        key('球', '\\left\\{\\left(x,y,z\\right)\\mid x^{2}+y^{2}+z^{2}\\le 1\\right\\}', '単位球（不等式で書いた立体）'),
        key('円板', '\\left\\{\\left(x,y\\right)\\mid x^{2}+y^{2}\\le 1\\right\\}', '単位円板（不等式で書いた平面の領域）'),
      ],
    ],
  },
  {
    title: 'ベクトル',
    rows: [
      [
        key('(a,b)', '\\left(#?,#?\\right)', '2 成分のベクトル'),
        key('(a,b,c)', '\\left(#?,#?,#?\\right)', '3 成分のベクトル'),
        key('a·b', '\\cdot', '内積（スカラーなら掛け算）'),
        key('a×b', '\\times', '外積（スカラーなら掛け算）'),
        key('|a|', '\\left|#0\\right|', 'ベクトルの大きさ（スカラーなら絶対値）'),
        key('∠(u,v)', '\\angle\\left(#?,#?\\right)', '2 つのベクトルのなす角'),
      ],
      [
        key('∇f', '\\nabla #?', '勾配 grad'),
        key('∇·F', '\\nabla\\cdot #?', '発散 div'),
        key('∇×F', '\\nabla\\times #?', '回転 rot'),
        key('∇²f', '\\nabla^{2}#?', 'ラプラシアン'),
        key(`${vecLabel('F')}=`, '\\vec{F}\\left(x,y,z\\right)=\\left(#?,#?,#?\\right)', 'ベクトル場の定義 \\vec{F}(x, y, z) = (P, Q, R)（ベクトルの名前 F は、ふつうの f・F とは別の名前）'),
        key('∭∇·F', '\\iiint_{V}\\nabla\\cdot #?\\,dV', '発散の体積分 ∭_V ∇ · F dV（V は式の下の欄に書く。ガウスの発散定理で、V の境界の面の外向きの面積分 ∯ F · dS と等しい）'),
        key('⊥', '\\perp', '直交 u ⊥ v（u · v = 0）'),
        key('∥', '\\parallel', '平行 u ∥ v（u × v = 0）'),
      ],
      [
        key('(⋮⋮)', '\\begin{pmatrix}#? & #?\\\\ #? & #?\\end{pmatrix}', '2×2 の行列（大きさは、行列の中で出る帯・Enter・カンマで変える）'),
        key('(⋮⋮⋮)', '\\begin{pmatrix}#? & #? & #?\\\\ #? & #? & #?\\\\ #? & #? & #?\\end{pmatrix}', '3×3 の行列'),
        key('|⋮⋮|', '\\begin{vmatrix}#? & #?\\\\ #? & #?\\end{vmatrix}', '行列式（1 行にそれだけなら、列ベクトルが張る平行四辺形・平行六面体）'),
        key('det', '\\det', '行列式'),
        key('Aᵀ', '#@^{T}', '転置'),
        key('A⁻¹', '#@^{-1}', '逆行列'),
        key('ker', '\\ker', '零空間 ker A（1 行にそれだけなら、点・直線・平面）'),
        key('Im', '\\operatorname{Im}', '像 Im A（列ベクトルが張る空間）'),
        key(vecLabel('v'), '\\vec{#@}', '直前の文字をベクトルの名前にする（v の後に押すと \\vec{v}。v とは別の名前）'),
      ],
    ],
  },
  {
    title: '方程式',
    rows: [
      [
        key("x′=", "x'=#?", '常微分方程式 x′ = …（行ごとに , で並べると連立。独立変数は t）'),
        key('ẋ', '\\dot{#0}', '時間の導関数 ẋ'),
        key('ẍ', '\\ddot{#0}', '2 階の時間の導関数 ẍ'),
        key('dx/dt', '\\frac{dx}{dt}', '導関数 dx/dt'),
        key("y″", "y''", '2 階の導関数（y″ + y = 0 は 1 本の常微分方程式）'),
        key('aₙ₊₁', 'a_{n+1}=#?', '数列の漸化式（初項は a_1 = … の行）'),
        key('aₙ', 'a_{#?}', '数列の項'),
        key('mod', '\\bmod', 'a mod b（余り）'),
      ],
      [
        key('∃', '\\exists a\\in\\left[#?,#?\\right],', '存在する（a を範囲で動かした和。近似が混ざる）'),
        key('∀', '\\forall a\\in\\left[#?,#?\\right],', '任意の（不等式だけ。近似）'),
        key('max_t', '\\max_{t\\in\\left[#?,#?\\right]}#?', '範囲での最大（64 等分の点での近似）'),
        key('min_t', '\\min_{t\\in\\left[#?,#?\\right]}#?', '範囲での最小（近似）'),
        key('≡', '\\equiv', '合同 a ≡ b（\\pmod{n} を後ろに付ける）'),
        key('(mod n)', '\\pmod{#?}', '合同の法'),
        key('gcd', '\\gcd\\left(#?,#?\\right)', '最大公約数'),
        key('C(n,k)', '\\binom{#?}{#?}', '二項係数'),
      ],
      [
        key('cobweb', '\\operatorname{cobweb}\\left(#?,#?\\right)', '漸化式の階段図 cobweb(a₀, N)（開始項・反復回数）'),
        key('orbit₂', '\\operatorname{orbit}\\left(#?,#?,#?\\right)', '2D の軌道 orbit(a₀, b₀, N)（2 個の開始項・反復回数）'),
        key('orbit₃', '\\operatorname{orbit}\\left(#?,#?,#?,#?\\right)', '3D の軌道 orbit(a₀, b₀, c₀, N)（3 個の開始項・反復回数）'),
        key('f|ₓ', '\\left.#?\\right|_{x=#?}', '点での値（x に指定した値を代入する）'),
      ],
    ],
  },
  {
    title: '集合',
    rows: [
      [
        key('{xy|}', '\\left\\{\\left(x,y\\right)\\mid#?\\right\\}', '集合 {(x, y) | 条件}（平面）'),
        key('{xyz|}', '\\left\\{\\left(x,y,z\\right)\\mid#?\\right\\}', '集合 {(x, y, z) | 条件}（空間）'),
        { ...key('∈', '\\in', '属する（長押しで ∉ 属さない）'), variants: ['\\notin'] },
        key('∣', '\\mid', '集合の区切りの縦線 {点 ∣ 条件}（消してしまったときに。約数 d ∣ 12 にも）'),
        key('∩', '\\cap', '共通部分'),
        key('∪', '\\cup', '和集合'),
        key('∖', '\\setminus', '差集合'),
        key('Aᶜ', '#@^{c}', '補集合'),
      ],
      [
        key('∧', '\\land', 'かつ'),
        key('∨', '\\lor', 'または'),
        key('¬', '\\neg', 'でない'),
        key('⇒', '\\Rightarrow', 'ならば'),
        key('⇔', '\\Leftrightarrow', '同値'),
        key('XOR', '\\oplus', '排他的論理和（どちらか一方だけ）', true),
        key('NAND', '\\barwedge', '否定論理積（両方ではない）', true),
        key('NOR', '\\downarrow', '否定論理和（どちらでもない）', true),
      ],
      [
        key('ℝ', '\\mathbb{R}', '実数'),
        key('ℤ', '\\mathbb{Z}', '整数'),
        key('ℕ', '\\mathbb{N}', '自然数（1 以上）'),
        key('[a,b]', '\\left[#?,#?\\right]', '閉区間'),
        key('[a,b)', '\\left[#?,#?\\right)', '半開区間'),
        key('△', '\\triangle', '対称差（集合の XOR）'),
        key('⋃', '\\bigcup_{k=1}^{#?}#?', '添字で並べた和集合 ⋃_{k=1}^{n}（⋃_{k ∈ S} も）'),
        key('⋂', '\\bigcap_{k=1}^{#?}#?', '添字で並べた共通部分 ⋂_{k=1}^{n}'),
      ],
    ],
  },
]

const pm = (rows: number, cols: number, env = 'pmatrix') =>
  `\\begin{${env}}${Array.from({ length: rows }, () => Array.from({ length: cols }, () => '#?').join(' & ')).join('\\\\ ')}\\end{${env}}`

/**
 * 「行列」のタブ: 行列・ベクトルの雛形と演算（∑∫∈ の奥まで行かなくても、1 回で開けるように）。
 * tr・det・rank は、名前の前に付けると値になる（その行だけなら、説明に値が出る）
 */
export const MATRIX_ROWS: CalcKey[][] = [
  [
    key('(⋮⋮)', pm(2, 2), '2×2 の行列（大きさは、行列の中で出る帯・Enter・カンマで変える）'),
    key('(⋮⋮⋮)', pm(3, 3), '3×3 の行列'),
    key('(⋮)₂', pm(2, 1), '2 成分の列ベクトル'),
    key('(⋮)₃', pm(3, 1), '3 成分の列ベクトル'),
    key('[⋮⋮]', pm(2, 2, 'bmatrix'), '2×2 の行列（角括弧）'),
    key('|⋮⋮|', pm(2, 2, 'vmatrix'), '2×2 の行列式（1 行にそれだけなら、列ベクトルが張る平行四辺形）'),
    key('|⋮⋮⋮|', pm(3, 3, 'vmatrix'), '3×3 の行列式（平行六面体）'),
    key('A=', 'A=' + pm(2, 2), '行列の定義 A = …（ほかの式で A と書ける）', true),
  ],
  [
    key('det', '\\det #?', '行列式 det A（その行だけなら値が出る）'),
    key('tr', '\\operatorname{tr}#?', 'トレース tr A（対角成分の和。その行だけなら値が出る）'),
    key('rank', '\\operatorname{rank}#?', '階数 rank A'),
    key('Aᵀ', '#@^{T}', '転置（直前の行列に付ける）'),
    key('A⁻¹', '#@^{-1}', '逆行列'),
    key('Aⁿ', '#@^{#?}', '行列のべき（20 までの整数）'),
    key('ker', '\\ker #?', '零空間 ker A（1 行にそれだけなら、点・直線・平面。説明に式も出る。固有値 λ の固有空間は ker(A − λI)、I・E は単位行列）'),
    key('Im', '\\operatorname{Im}#?', '像 Im A（列ベクトルが張る空間。1 行にそれだけなら、点・直線・平面。説明に式も出る）'),
    key('eig', '\\operatorname{eig}#?', '固有値と固有空間（1 行にそれだけ。説明に固有値ごとの固有空間の式、図に固有ベクトル）'),
  ],

  [
    {
      ...key(vecLabel('v'), '\\vec{#@}', '直前の文字をベクトルの名前にする（F の後に押すと \\vec{F}。F とは別の名前で、定義して積分などに使える。長押しで太字 \\mathbf{F}・黒板太字 \\mathbb{F}。この 3 つは同じ名前）'),
      variants: ['\\mathbf{#@}', '\\mathbb{#@}'],
    },
    {
      ...key(`${vecLabel('F')}=`, '\\vec{F}\\left(x,y,z\\right)=\\left(#?,#?,#?\\right)', 'ベクトル場の定義 \\vec{F}(x, y, z) = (P, Q, R)（長押しで太字 \\mathbf{F}・黒板太字 \\mathbb{F} の形）'),
      variants: ['\\mathbf{F}\\left(x,y,z\\right)=\\left(#?,#?,#?\\right)', '\\mathbb{F}\\left(x,y,z\\right)=\\left(#?,#?,#?\\right)'],
    },
    key('a·b', '\\cdot', '内積・行列の積'),
    key('a×b', '\\times', '外積'),
    key('⟨a,b⟩', '\\left\\langle#?,#?\\right\\rangle', '内積 ⟨a, b⟩'),
    key('‖a‖', '\\left\\Vert#0\\right\\Vert', 'ノルム（大きさ）', true),
    key('Aᵢⱼ', '#@_{#?,#?}', '成分 A_{i,j}'),
    key('A†', '#@^{\\dagger}', '共役転置'),
    key('dim', '\\dim #?', '次元 dim ker A など'),
  ],
]


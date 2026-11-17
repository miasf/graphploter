// 数式キーボードの「∑∫∈」のタブのキー（微積・ベクトル・集合と論理の雛形）。
// MathLive を読み込まない（テスト calcKeys.test.ts から、全部のキーが変換できることを確かめるため）。

/** キー 1 つ（MathLive の VirtualKeyboardKeycap の一部） */
export interface CalcKey {
  label: string
  insert: string
  tooltip: string
  class?: string
  /** 長押しで出る別のキー（入れる LaTeX） */
  variants?: string[]
}

/** 矢印のベクトルの名前のキーの表示（矢印を重ねる文字 U+20D7 は字体によって □ になるので、→ を文字の上に重ねる） */
export const vecLabel = (c: string) =>
  `<span style="display:inline-flex;flex-direction:column;align-items:center;line-height:0.75"><span style="font-size:0.7em">→</span><i>${c}</i></span>`

/** 雛形のキー（label はキーの表示、insert は入れる LaTeX（#? は空欄、#0 は選んでいた部分）、ja は長押しで出る名前） */
function key(label: string, insert: string, ja: string, small = false): CalcKey {
  return { label, insert, tooltip: ja, ...(small || [...label].length > 2 ? { class: 'small' } : {}) }
}

/**
 * 「∑∫∈」のタブ: 数学の記法（微積・ベクトル・集合と論理）の雛形。スマホで探さずに入れられるように、abc の奥ではなくここにまとめる。
 * 1 段 8 個まで（幅 360px のスマホでも押しやすい大きさ）。ページは下の段の ◀ ▶ でめくる
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
        key('∬_D', '\\iint_{D}#?\\,dA', '領域 D の上の 2 重積分（D の範囲は式の下の欄に書く。文字は自由）'),
        key('∭_V', '\\iiint_{V}#?\\,dV', '領域 V の上の 3 重積分（V の範囲は式の下の欄に書く）'),
        key('∫_C', '\\int_{C}#?\\,ds', '線積分（曲線 C は式の下の欄に {(x(t), y(t)) | a ≤ t ≤ b} で書く）'),
        key('∮F·dr', '\\oint_{C}#?\\cdot d\\mathbf{r}', '仕事（線積分 F · dr。曲線 C は式の下の欄に書く）'),
        key('∬F·dS', '\\iint_{S}#?\\cdot d\\mathbf{S}', '流束（面積分 F · dS。曲面 S は式の下の欄に書く）'),
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
      ],
      [
        key('∇f', '\\nabla #?', '勾配 grad'),
        key('∇·F', '\\nabla\\cdot #?', '発散 div'),
        key('∇×F', '\\nabla\\times #?', '回転 rot'),
        key('∇²f', '\\nabla^{2}#?', 'ラプラシアン'),
        key(`${vecLabel('F')}=`, '\\vec{F}\\left(x,y,z\\right)=\\left(#?,#?,#?\\right)', 'ベクトル場の定義 𝐅(x, y, z) = (P, Q, R)（𝐅 は f とは別の名前）'),
        key('∭∇·F', '\\iiint_{V}\\nabla\\cdot #?\\,dV', '体積分 ∭_V ∇ · F dV（立体 V は式の下の欄に {(x(r,u,v), …) | 範囲} で書く。値は V の表面を通る流束と同じ）'),
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
    key('A=', 'A=' + pm(2, 2), '行列の定義 A = …（ほかの式で A と書ける）'),
  ],
  [
    key('det', '\\det #?', '行列式 det A（その行だけなら値が出る）'),
    key('tr', '\\operatorname{tr}#?', 'トレース tr A（対角成分の和。その行だけなら値が出る）'),
    key('rank', '\\operatorname{rank}#?', '階数 rank A'),
    key('Aᵀ', '#@^{T}', '転置（直前の行列に付ける）'),
    key('A⁻¹', '#@^{-1}', '逆行列'),
    key('Aⁿ', '#@^{#?}', '行列のべき（20 までの整数）'),
    key('A†', '#@^{\\dagger}', '共役転置'),
    key('ker', '\\ker #?', '零空間 ker A（1 行にそれだけなら、点・直線・平面）'),
  ],
  [
    {
      ...key(vecLabel('v'), '\\vec{#@}', '直前の文字をベクトルの名前にする（F の後に押すと \\vec{F}。F とは別の名前で、定義して積分などに使える。長押しで太字 \\mathbf{F}・黒板太字 \\mathbb{F}。この 3 つは同じ名前）'),
      variants: ['\\mathbf{#@}', '\\mathbb{#@}'],
    },
    {
      ...key(`${vecLabel('F')}=`, '\\vec{F}\\left(x,y,z\\right)=\\left(#?,#?,#?\\right)', 'ベクトル場の定義 𝐅(x, y, z) = (P, Q, R)（長押しで太字の 𝐅・黒板太字の 𝔽）'),
      variants: ['\\mathbf{F}\\left(x,y,z\\right)=\\left(#?,#?,#?\\right)', '\\mathbb{F}\\left(x,y,z\\right)=\\left(#?,#?,#?\\right)'],
    },
    key('a·b', '\\cdot', '内積・行列の積'),
    key('a×b', '\\times', '外積'),
    key('⟨a,b⟩', '\\left\\langle#?,#?\\right\\rangle', '内積 ⟨a, b⟩'),
    key('‖a‖', '\\left\\Vert#0\\right\\Vert', 'ノルム（大きさ）'),
    key('Aᵢⱼ', '#@_{#?,#?}', '成分 A_{i,j}'),
    key('dim', '\\dim #?', '次元 dim ker A など'),
  ],
]


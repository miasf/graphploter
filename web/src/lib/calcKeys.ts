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
        key('F=', 'F\\left(x,y,z\\right)=\\left(#?,#?,#?\\right)', 'ベクトル場の定義 F(x, y, z) = (P, Q, R)'),
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


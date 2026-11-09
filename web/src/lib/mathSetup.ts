// 数式入力欄（MathLive）の共通設定: フォントの場所、自動変換の一覧、画面上の数式キーボード。

import { MathfieldElement, type VirtualKeyboardKeycap } from 'mathlive'

// フォントは public/mathlive-fonts に置いてある（npm run fonts でコピーされる）
// 相対パスだとスクリプトの場所が基準になってしまうので、ページの URL を基準にした絶対 URL にする
MathfieldElement.fontsDirectory = new URL(`${import.meta.env.BASE_URL}mathlive-fonts`, document.baseURI).href
MathfieldElement.soundsDirectory = null // キーを押したときの音を鳴らさない

/**
 * 入力中の自動変換。例: "pi" と打つと π、"sqrt" と打つと √ になる。
 * MathLive の標準の変換表は、"in" が ∈ になるなどこのアプリに合わないので、必要なものだけにしている。
 * ここに無い関数名（floor, mod, nCr など）は文字のまま残り、計算エンジン側で関数として読まれる。
 */
export const SHORTCUTS: Record<string, string> = {
  // 下の短い変換（sin、log、gamma など）を中に含む長い関数名は、そのままの名前で残す
  // （例: "sinc" が「\sin c」に、"digamma" が「di Γ」にならないように）
  ...Object.fromEntries(
    [
      'sinc', 'gammaP', 'gammaQ', 'loggamma', 'lngamma', 'digamma', 'polygamma', 'polylog',
      'expIntegralEi', 'expIntegralE', 'logIntegral', 'sinIntegral', 'cosIntegral', 'sinhIntegral', 'coshIntegral',
    ].map((f) => [f, `\\operatorname{${f}}`]),
  ),
  pi: '\\pi',
  sqrt: '\\sqrt{#?}',
  gamma: '\\Gamma',
  // w2 → w₂、t1 → t₁ のように、w と t の後ろの数字は添字にする（4次元目以降の座標・媒介変数）
  ...Object.fromEntries(
    ['w', 't'].flatMap((c) => Array.from({ length: 9 }, (_, k) => [`${c}${k + 1}`, `${c}_{${k + 1}}`])),
  ),
  ...Object.fromEntries(
    ['sin', 'cos', 'tan', 'sec', 'csc', 'cot', 'sinh', 'cosh', 'tanh', 'arcsin', 'arccos', 'arctan', 'exp', 'ln', 'log'].map((f) => [
      f,
      `\\${f}`,
    ]),
  ),
}

/** 関数のキー（押すと「関数名(□)」が入る） */
function fn(name: string, opts: { operator?: boolean; args?: number } = {}) {
  const head = opts.operator ? `\\operatorname{${name}}` : `\\${name}`
  const args = Array.from({ length: opts.args ?? 1 }, () => '#?').join(',')
  return { label: name, latex: head, insert: `${head}\\left(${args}\\right)`, class: 'small' }
}

/** 特殊関数のキー。label はキーの表示、name は計算エンジンでの関数名、args は引数の数 */
function sp(label: string, name: string, args: number, head?: string) {
  const h = head ?? `\\operatorname{${name}}`
  const a = Array.from({ length: args }, () => '#?').join(',')
  return {
    label,
    insert: `${h}\\left(${a}\\right)`,
    class: 'small',
    width: 2 as const, // 名前が長いので2個分の幅にする
    tooltip: `${name}(${'□, '.repeat(args - 1)}□)`,
  }
}

/**
 * 特殊関数のページ。計算エンジンで使えるものをすべて入れる（書き方のヒントの一覧と同じ）。
 * 1ページは5個 × 3段まで（キーは2個分の幅。幅の狭いスマホでもはみ出さない数）
 */
const SPECIAL_PAGES: { title: string; keys: ReturnType<typeof sp>[] }[] = [
  {
    title: 'ガンマ',
    keys: [
      sp('Γ(z)', 'gamma', 1, '\\Gamma'),
      sp('Γ(a,z)', 'gamma', 2, '\\Gamma'),
      sp('P(a,z)', 'gammaP', 2),
      sp('Q(a,z)', 'gammaQ', 2),
      sp('lnΓ(z)', 'loggamma', 1),
      sp('ψ(z)', 'digamma', 1, '\\psi'),
      sp('ψ⁽ⁿ⁾(z)', 'polygamma', 2),
      sp('B(a,b)', 'beta', 2),
      sp('B(z;a,b)', 'beta', 3),
      sp('I(z;a,b)', 'betaRegularized', 3),
      sp('(a)ₙ', 'pochhammer', 2),
      sp('(n k)', 'binomial', 2),
    ],
  },
  {
    title: 'ゼータ・erf',
    keys: [
      sp('ζ(s)', 'zeta', 1, '\\zeta'),
      sp('ζ(s,a)', 'zeta', 2, '\\zeta'),
      sp('Liₛ(z)', 'polylog', 2),
      sp('η(s)', 'dirichletEta', 1),
      sp('erf', 'erf', 1),
      sp('erfc', 'erfc', 1),
      sp('erfi', 'erfi', 1),
      sp('dawson', 'dawson', 1),
      sp('S(z)', 'fresnelS', 1),
      sp('C(z)', 'fresnelC', 1),
    ],
  },
  {
    title: '積分',
    keys: [
      sp('Ei(z)', 'Ei', 1),
      sp('Eₙ(z)', 'En', 2),
      sp('li(z)', 'li', 1),
      sp('Si(z)', 'Si', 1),
      sp('Ci(z)', 'Ci', 1),
      sp('Shi(z)', 'Shi', 1),
      sp('Chi(z)', 'Chi', 1),
      sp('sinc', 'sinc', 1),
    ],
  },
  {
    title: 'ベッセル',
    keys: [
      sp('J_ν(z)', 'besselJ', 2),
      sp('Y_ν(z)', 'besselY', 2),
      sp('I_ν(z)', 'besselI', 2),
      sp('K_ν(z)', 'besselK', 2),
      sp('H⁽¹⁾_ν(z)', 'hankelH1', 2),
      sp('H⁽²⁾_ν(z)', 'hankelH2', 2),
      sp('jₙ(z)', 'sphericalJ', 2),
      sp('yₙ(z)', 'sphericalY', 2),
      sp('Ai(z)', 'airyAi', 1),
      sp('Bi(z)', 'airyBi', 1),
      sp("Ai'(z)", 'airyAiPrime', 1),
      sp("Bi'(z)", 'airyBiPrime', 1),
    ],
  },
  {
    title: '多項式',
    keys: [
      sp('Pₙ(x)', 'legendreP', 2),
      sp('Pₙᵐ(x)', 'legendreP', 3),
      sp('Hₙ(x)', 'hermiteH', 2),
      sp('Lₙ(x)', 'laguerreL', 2),
      sp('Lₙᵃ(x)', 'laguerreL', 3),
      sp('Tₙ(x)', 'chebyshevT', 2),
      sp('Uₙ(x)', 'chebyshevU', 2),
      sp('Cₙᵃ(x)', 'gegenbauerC', 3),
      sp('Pₙ⁽ᵃᵇ⁾(x)', 'jacobiP', 4),
      sp('Yₗᵐ(θ,φ)', 'sphericalHarmonicY', 4),
    ],
  },
  {
    title: '超幾何・楕円',
    keys: [
      sp('₀F₁', 'hyp0F1', 2),
      sp('₁F₁', 'hyp1F1', 3),
      sp('₂F₁', 'hyp2F1', 4),
      sp('U(a,b,z)', 'hypU', 3),
      sp('K(m)', 'ellipticK', 1),
      sp('E(m)', 'ellipticE', 1),
      sp('E(φ,m)', 'ellipticE', 2),
      sp('F(φ,m)', 'ellipticF', 2),
      sp('Π(n,m)', 'ellipticPi', 2),
      sp('Π(n,φ,m)', 'ellipticPi', 3),
      sp('sn(u,m)', 'jacobiSN', 2),
      sp('cn(u,m)', 'jacobiCN', 2),
      sp('dn(u,m)', 'jacobiDN', 2),
    ],
  },
  {
    title: 'その他',
    keys: [
      sp('W(z)', 'lambertW', 1),
      sp('W_k(z)', 'lambertW', 2),
      sp('sec', 'sec', 1, '\\sec'),
      sp('csc', 'csc', 1, '\\csc'),
      sp('cot', 'cot', 1, '\\cot'),
      sp('arcsinh', 'arcsinh', 1),
      sp('arccosh', 'arccosh', 1),
      sp('arctanh', 'arctanh', 1),
    ],
  },
]

/** 特殊関数のタブ: ページごとに層（layer）を分け、下の段の ◀ ▶ でページをめくる */
const SPECIAL_LAYOUT = {
  label: '特殊',
  tooltip: '特殊関数（◀ ▶ でページをめくる）',
  layers: SPECIAL_PAGES.map((page, k) => {
    const n = SPECIAL_PAGES.length
    const id = (j: number) => `special-${(j + n) % n}`
    const rows: Partial<VirtualKeyboardKeycap>[][] = []
    for (let r = 0; r < page.keys.length; r += 5) rows.push(page.keys.slice(r, r + 5))
    while (rows.length < 3) rows.push([]) // ページによって高さが変わらないように
    rows.push([
      { label: '◀', class: 'action', width: 1, command: ['switchKeyboardLayer', id(k - 1)], tooltip: '前のページ' },
      { label: `${page.title} ${k + 1}/${n}`, class: 'action small', width: 5, command: ['switchKeyboardLayer', id(k + 1)], tooltip: '次のページ' },
      { label: '▶', class: 'action', width: 1, command: ['switchKeyboardLayer', id(k + 1)], tooltip: '次のページ' },
      { label: '[left]', width: 1 },
      { label: '[right]', width: 1 },
      { label: '[backspace]', width: 1 },
    ])
    return { id: id(k), rows }
  }),
}

/** 画面上の数式キーボード（スマホでは入力欄を押すと自動で出る） */
export function setupKeyboard() {
  const kb = window.mathVirtualKeyboard
  kb.layouts = [
    {
      label: '123',
      tooltip: '数字と記号',
      rows: [
        ['x', 'y', 'z', 'w', 't', 'u', 'v', '[separator-5]', '7', '8', '9', { latex: '\\frac{#@}{#?}', label: '÷' }],
        [
          '(',
          ')',
          ',',
          '\\pi',
          { latex: '\\imaginaryI', label: 'i' },
          'e',
          { latex: '#@_{#?}', label: 'x₂', tooltip: '添字（w₂ など）' },
          '[separator-5]',
          '4',
          '5',
          '6',
          { latex: '\\cdot', label: '×' },
        ],
        [
          { latex: '#@^{2}', label: 'x²' },
          { latex: '#@^{#?}', label: 'xⁿ' },
          { latex: '\\sqrt{#0}', label: '√' },
          { latex: '\\left|#0\\right|', label: '|x|' },
          '!',
          '=',
          '[separator-5]',
          '1',
          '2',
          '3',
          '-',
        ],
        // 一文字消すボタン（⌫）は、ふつうのキーボードと同じく右下に置く（⏎ は左下の矢印の隣）
        [
          { label: '[left]', width: 1 },
          { label: '[right]', width: 1 },
          '[hide-keyboard]',
          { label: '[return]', width: 1 },
          '[separator-5]',
          '0',
          '.',
          '+',
          { label: '[backspace]', width: 1 },
        ],
      ],
    },
    {
      label: 'f(x)',
      tooltip: '関数',
      rows: [
        [fn('sin'), fn('cos'), fn('tan'), fn('sinh'), fn('cosh'), fn('tanh')],
        [fn('arcsin'), fn('arccos'), fn('arctan'), fn('exp'), fn('ln'), fn('log')],
        [
          fn('floor', { operator: true }),
          fn('ceil', { operator: true }),
          fn('round', { operator: true }),
          fn('sign', { operator: true }),
          { latex: '\\Gamma\\left(#0\\right)', label: 'Γ' },
          '!',
          { latex: '\\left[#0\\right]', label: '[ ]', tooltip: 'リスト（[−1, 1] など）' },
        ],
        [
          fn('mod', { operator: true, args: 2 }),
          fn('min', { operator: true, args: 2 }),
          fn('max', { operator: true, args: 2 }),
          fn('nCr', { operator: true, args: 2 }),
          fn('nPr', { operator: true, args: 2 }),
        ],
        [
          fn('Re', { operator: true }),
          fn('Im', { operator: true }),
          fn('conj', { operator: true }),
          fn('arg', { operator: true }),
          { label: '[left]', width: 1 },
          { label: '[right]', width: 1 },
          '[backspace]',
        ],
      ],
    },
    SPECIAL_LAYOUT,
    'alphabetic',
  ]
}

// ---- PC でも画面上の数式キーボードを出す ----
// MathLive の自動（auto）では、キーボードが出るのはスマホなど指で操作する画面だけ。
// PC では手動（manual）にして、入力欄に入ったときにこちらで出す。
// キーボードの「隠す」ボタンで閉じたら、その設定を覚えて、次からは自動で出さない
// （入力欄のキーボードのボタンで開き直すと、また自動で出す）。

/** 指で操作する画面か（MathLive に任せる） */
const touch = window.matchMedia('(pointer: coarse)').matches
const PC_KEYBOARD_KEY = 'graphploter:pcKeyboard'

function loadAutoShow(): boolean {
  try {
    return localStorage.getItem(PC_KEYBOARD_KEY) !== 'off'
  } catch {
    return true
  }
}

let autoShow = loadAutoShow()
let watching = false

const mathFieldFocused = () => document.activeElement?.tagName === 'MATH-FIELD'

/** 入力欄に設定するキーボードの出し方 */
export function keyboardPolicy(): 'auto' | 'manual' {
  return touch ? 'auto' : 'manual'
}

/** 入力欄に入ったとき（PC では、閉じられていなければキーボードを出す） */
export function keyboardOnFocus() {
  if (touch) return
  const kb = window.mathVirtualKeyboard
  if (!watching) {
    watching = true
    // 入力欄に入ったまま開閉された = 利用者がボタンで開閉した。その状態を覚える
    kb.addEventListener('virtual-keyboard-toggle', () => {
      if (!mathFieldFocused()) return // 入力欄から出たので閉じた
      autoShow = kb.visible
      try {
        localStorage.setItem(PC_KEYBOARD_KEY, autoShow ? 'on' : 'off')
      } catch {
        // 保存できない環境では、このページを開いている間だけ覚える
      }
    })
  }
  if (autoShow && !kb.visible) kb.show()
}

/** 入力欄から出たとき（PC では、ほかの入力欄に移ったのでなければキーボードを閉じる） */
export function keyboardOnBlur() {
  if (touch) return
  setTimeout(() => {
    const kb = window.mathVirtualKeyboard
    if (!mathFieldFocused() && kb.visible) kb.hide()
  }, 0)
}

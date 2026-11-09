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

/** 関数のキー（押すと「関数名(□)」が入る）。ja は長押し・カーソルを当てたときに出す名前 */
function fn(name: string, ja: string, opts: { operator?: boolean; args?: number } = {}) {
  const head = opts.operator ? `\\operatorname{${name}}` : `\\${name}`
  const n = opts.args ?? 1
  const args = Array.from({ length: n }, () => '#?').join(',')
  const tooltip = `${ja}\n${name}(${Array.from({ length: n }, () => '□').join(', ')})`
  return { label: name, latex: head, insert: `${head}\\left(${args}\\right)`, class: 'small', tooltip }
}

/**
 * 特殊関数のキー。label はキーの表示、name は計算エンジンでの関数名、args は引数の名前（例: "ν, z"）、
 * ja は長押し・カーソルを当てたときに出す名前
 */
function sp(label: string, name: string, args: string, ja: string, head?: string) {
  const h = head ?? `\\operatorname{${name}}`
  const a = args.split(',').map(() => '#?').join(',')
  return {
    label: `<span>${label}</span>`, // 添字（<sub>）を含むので1つにまとめる（キーの中は縦に並ぶため）
    insert: `${h}\\left(${a}\\right)`,
    class: 'small',
    width: 2 as const, // 名前が長いので2個分の幅にする
    tooltip: `${ja}\n${name}(${args})`,
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
      sp('Γ(z)', 'gamma', 'z', 'ガンマ関数', '\\Gamma'),
      sp('Γ(a,z)', 'gamma', 'a, z', '上側不完全ガンマ関数', '\\Gamma'),
      sp('P(a,z)', 'gammaP', 'a, z', '正則化された下側不完全ガンマ関数'),
      sp('Q(a,z)', 'gammaQ', 'a, z', '正則化された上側不完全ガンマ関数'),
      sp('lnΓ(z)', 'loggamma', 'z', '対数ガンマ関数'),
      sp('ψ(z)', 'digamma', 'z', 'ディガンマ関数', '\\psi'),
      sp('ψ⁽ⁿ⁾(z)', 'polygamma', 'n, z', 'ポリガンマ関数'),
      sp('B(a,b)', 'beta', 'a, b', 'ベータ関数'),
      sp('B(z;a,b)', 'beta', 'z, a, b', '不完全ベータ関数'),
      sp('I(z;a,b)', 'betaRegularized', 'z, a, b', '正則化された不完全ベータ関数'),
      sp('(a)ₙ', 'pochhammer', 'a, n', 'ポッホハマー記号（上昇階乗）'),
      sp('(n k)', 'binomial', 'n, k', '二項係数'),
    ],
  },
  {
    title: 'ゼータ・erf',
    keys: [
      sp('ζ(s)', 'zeta', 's', 'リーマンのゼータ関数', '\\zeta'),
      sp('ζ(s,a)', 'zeta', 's, a', 'フルヴィッツのゼータ関数', '\\zeta'),
      sp('Liₛ(z)', 'polylog', 's, z', '多重対数関数'),
      sp('η(s)', 'dirichletEta', 's', 'ディリクレのイータ関数'),
      sp('erf', 'erf', 'z', '誤差関数'),
      sp('erfc', 'erfc', 'z', '相補誤差関数'),
      sp('erfi', 'erfi', 'z', '虚数誤差関数'),
      sp('dawson', 'dawson', 'z', 'ドーソン積分'),
      sp('S(z)', 'fresnelS', 'z', 'フレネル積分 S'),
      sp('C(z)', 'fresnelC', 'z', 'フレネル積分 C'),
    ],
  },
  {
    title: '積分',
    keys: [
      sp('Ei(z)', 'Ei', 'z', '指数積分 Ei'),
      sp('Eₙ(z)', 'En', 'n, z', '一般化指数積分 Eₙ'),
      sp('li(z)', 'li', 'z', '対数積分'),
      sp('Si(z)', 'Si', 'z', '正弦積分'),
      sp('Ci(z)', 'Ci', 'z', '余弦積分'),
      sp('Shi(z)', 'Shi', 'z', '双曲線正弦積分'),
      sp('Chi(z)', 'Chi', 'z', '双曲線余弦積分'),
      sp('sinc', 'sinc', 'z', 'sinc 関数（sin z / z）'),
    ],
  },
  {
    title: 'ベッセル',
    keys: [
      sp('J<sub>ν</sub>(z)', 'besselJ', 'ν, z', '第1種ベッセル関数'),
      sp('Y<sub>ν</sub>(z)', 'besselY', 'ν, z', '第2種ベッセル関数'),
      sp('I<sub>ν</sub>(z)', 'besselI', 'ν, z', '第1種変形ベッセル関数'),
      sp('K<sub>ν</sub>(z)', 'besselK', 'ν, z', '第2種変形ベッセル関数'),
      sp('H<sup>(1)</sup><sub>ν</sub>(z)', 'hankelH1', 'ν, z', '第1種ハンケル関数'),
      sp('H<sup>(2)</sup><sub>ν</sub>(z)', 'hankelH2', 'ν, z', '第2種ハンケル関数'),
      sp('jₙ(z)', 'sphericalJ', 'n, z', '第1種球ベッセル関数'),
      sp('yₙ(z)', 'sphericalY', 'n, z', '第2種球ベッセル関数'),
      sp('Ai(z)', 'airyAi', 'z', 'エアリー関数 Ai'),
      sp('Bi(z)', 'airyBi', 'z', 'エアリー関数 Bi'),
      sp("Ai'(z)", 'airyAiPrime', 'z', 'エアリー関数 Ai の導関数'),
      sp("Bi'(z)", 'airyBiPrime', 'z', 'エアリー関数 Bi の導関数'),
    ],
  },
  {
    title: '多項式',
    keys: [
      sp('Pₙ(x)', 'legendreP', 'n, x', 'ルジャンドル多項式'),
      sp('Pₙᵐ(x)', 'legendreP', 'n, m, x', 'ルジャンドル陪関数'),
      sp('Hₙ(x)', 'hermiteH', 'n, x', 'エルミート多項式'),
      sp('Lₙ(x)', 'laguerreL', 'n, x', 'ラゲール多項式'),
      sp('Lₙᵃ(x)', 'laguerreL', 'n, a, x', 'ラゲール陪多項式'),
      sp('Tₙ(x)', 'chebyshevT', 'n, x', '第1種チェビシェフ多項式'),
      sp('Uₙ(x)', 'chebyshevU', 'n, x', '第2種チェビシェフ多項式'),
      sp('Cₙᵃ(x)', 'gegenbauerC', 'n, a, x', 'ゲーゲンバウアー多項式'),
      sp('Pₙ⁽ᵃᵇ⁾(x)', 'jacobiP', 'n, a, b, x', 'ヤコビ多項式'),
      sp('Yₗᵐ(θ,φ)', 'sphericalHarmonicY', 'l, m, θ, φ', '球面調和関数'),
    ],
  },
  {
    title: '超幾何・楕円',
    keys: [
      sp('₀F₁', 'hyp0F1', 'b, z', '合流型超幾何極限関数 ₀F₁'),
      sp('₁F₁', 'hyp1F1', 'a, b, z', 'クンマーの合流型超幾何関数 ₁F₁'),
      sp('₂F₁', 'hyp2F1', 'a, b, c, z', 'ガウスの超幾何関数 ₂F₁'),
      sp('U(a,b,z)', 'hypU', 'a, b, z', 'トリコミの合流型超幾何関数 U'),
      sp('K(m)', 'ellipticK', 'm', '第1種完全楕円積分（m = k²）'),
      sp('E(m)', 'ellipticE', 'm', '第2種完全楕円積分（m = k²）'),
      sp('E(φ,m)', 'ellipticE', 'φ, m', '第2種不完全楕円積分'),
      sp('F(φ,m)', 'ellipticF', 'φ, m', '第1種不完全楕円積分'),
      sp('Π(n,m)', 'ellipticPi', 'n, m', '第3種完全楕円積分'),
      sp('Π(n,φ,m)', 'ellipticPi', 'n, φ, m', '第3種不完全楕円積分'),
      sp('sn(u,m)', 'jacobiSN', 'u, m', 'ヤコビの楕円関数 sn'),
      sp('cn(u,m)', 'jacobiCN', 'u, m', 'ヤコビの楕円関数 cn'),
      sp('dn(u,m)', 'jacobiDN', 'u, m', 'ヤコビの楕円関数 dn'),
    ],
  },
  {
    title: 'その他',
    keys: [
      sp('W(z)', 'lambertW', 'z', 'ランベルトの W 関数（主枝）'),
      sp('W<sub>k</sub>(z)', 'lambertW', 'k, z', 'ランベルトの W 関数（第 k 枝）'),
      sp('sec', 'sec', 'x', '正割（1 / cos x）', '\\sec'),
      sp('csc', 'csc', 'x', '余割（1 / sin x）', '\\csc'),
      sp('cot', 'cot', 'x', '余接（1 / tan x）', '\\cot'),
      sp('arcsinh', 'arcsinh', 'x', '逆双曲線正弦'),
      sp('arccosh', 'arccosh', 'x', '逆双曲線余弦'),
      sp('arctanh', 'arctanh', 'x', '逆双曲線正接'),
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

// ---- キーの名前の表示 ----
// PC: カーソルを少し当てていると名前が出る。スマホ: 長押しで名前が出る（名前が出てから指を離したときは、
// そのキーを入力しない）。名前はキーの tooltip（MathLive が data-tooltip に入れる）。
// MathLive 自身の吹き出しは、スマホではキーの外が切り取られて見えないので、ページの一番上に別の吹き出しを出す。

/** 絵だけのキー（MathLive が用意しているもの）の名前。絵の id → 名前 */
const ICON_NAMES: Record<string, string> = {
  '#svg-delete-backward': '一文字消す',
  '#svg-arrow-left': 'カーソルを左へ',
  '#svg-arrow-right': 'カーソルを右へ',
  '#svg-commit': '改行（下に新しい式を足す）',
  '#svg-keyboard-down': 'キーボードを閉じる',
  '#svg-shift': 'シフト（大文字）',
}

/** キーボードの中のキーと、その名前 */
function keyAt(target: EventTarget | null): { key: HTMLElement; name: string } | null {
  const key = target instanceof Element ? target.closest<HTMLElement>('.MLK__row > *') : null
  if (!key || !key.closest('.ML__keyboard')) return null
  let name = key.dataset.tooltip
  if (!name) {
    // 絵だけのキーには名前が付いていないので、絵の種類から決める
    const use = key.querySelector('use')
    const icon = use?.getAttribute('xlink:href') ?? use?.getAttribute('href')
    name = icon ? ICON_NAMES[icon] : undefined
  }
  return name ? { key, name } : null
}

/** カーソルを当ててから、名前を出すまでの時間（ミリ秒） */
const HOVER_MS = 600
/** 長押しとみなすまでの時間（ミリ秒） */
const LONG_PRESS_MS = 450

function setupKeyTips() {
  const bubble = document.createElement('div')
  bubble.className = 'key-tip'
  bubble.setAttribute('role', 'tooltip')
  bubble.hidden = true
  document.body.append(bubble)

  /** キーの上に名前を出す（画面からはみ出さないように左右を寄せる） */
  function show(key: HTMLElement, name: string) {
    bubble.textContent = name
    bubble.hidden = false
    const r = key.getBoundingClientRect()
    const w = bubble.offsetWidth
    const left = Math.min(Math.max(r.left + r.width / 2 - w / 2, 4), window.innerWidth - w - 4)
    bubble.style.left = `${left}px`
    bubble.style.top = `${Math.max(r.top - bubble.offsetHeight - 6, 4)}px`
  }
  const hide = () => {
    bubble.hidden = true
  }

  let timer: ReturnType<typeof setTimeout> | undefined

  // PC: カーソルを当てる
  let hovered: HTMLElement | null = null
  document.addEventListener('pointerover', (e) => {
    if (e.pointerType !== 'mouse') return
    const hit = keyAt(e.target)
    if (hit?.key === hovered) return
    clearTimeout(timer)
    hide()
    hovered = hit?.key ?? null
    if (hit) timer = setTimeout(() => show(hit.key, hit.name), HOVER_MS)
  })
  document.addEventListener('pointerdown', (e) => {
    if (e.pointerType === 'mouse') {
      clearTimeout(timer) // 押したら出さない
      hide()
    }
  })

  // スマホ: 長押し
  let start: { x: number; y: number } | null = null
  let shown = false
  document.addEventListener(
    'pointerdown',
    (e) => {
      if (e.pointerType === 'mouse') return
      clearTimeout(timer)
      hide()
      shown = false
      const hit = keyAt(e.target)
      start = hit ? { x: e.clientX, y: e.clientY } : null
      if (hit) {
        timer = setTimeout(() => {
          shown = true
          show(hit.key, hit.name)
        }, LONG_PRESS_MS)
      }
    },
    true,
  )
  document.addEventListener(
    'pointermove',
    (e) => {
      if (e.pointerType === 'mouse' || !start || shown) return
      if (Math.hypot(e.clientX - start.x, e.clientY - start.y) > 10) {
        clearTimeout(timer)
        start = null
      }
    },
    true,
  )
  const release = (e: PointerEvent) => {
    if (e.pointerType === 'mouse') return
    clearTimeout(timer)
    start = null
    if (!shown) return
    shown = false
    hide()
    // 名前を見るための長押しなので、キーは入力しない（MathLive に指を離したことを知らせない）
    e.stopPropagation()
    e.preventDefault()
    // 押した見た目だけ元に戻す
    document.querySelectorAll('.ML__keyboard .is-pressed').forEach((k) => k.classList.remove('is-pressed'))
  }
  document.addEventListener('pointerup', release, true)
  document.addEventListener('pointercancel', release, true)
}

/** 画面上の数式キーボード（スマホでは入力欄を押すと自動で出る） */
export function setupKeyboard() {
  setupKeyTips()
  const kb = window.mathVirtualKeyboard
  kb.layouts = [
    {
      label: '123',
      tooltip: '数字と記号',
      rows: [
        [
          { latex: 'x', tooltip: '座標 x' },
          { latex: 'y', tooltip: '座標 y' },
          { latex: 'z', tooltip: '座標 z' },
          { latex: 'w', tooltip: '4次元目の座標 w' },
          { latex: 't', tooltip: '媒介変数 t（曲線）' },
          { latex: 'u', tooltip: '媒介変数 u（曲面）' },
          { latex: 'v', tooltip: '媒介変数 v（曲面）' },
          '[separator-5]',
          '7',
          '8',
          '9',
          { latex: '\\frac{#@}{#?}', label: '÷', tooltip: '割り算（分数）' },
        ],
        [
          { latex: '(', tooltip: '左かっこ' },
          { latex: ')', tooltip: '右かっこ' },
          { latex: ',', tooltip: 'カンマ（引数・成分の区切り）' },
          { latex: '\\pi', tooltip: '円周率 π' },
          { latex: '\\imaginaryI', label: 'i', tooltip: '虚数単位 i' },
          { latex: 'e', tooltip: 'ネイピア数 e' },
          { latex: '#@_{#?}', label: 'x₂', tooltip: '添字（w₂ など）' },
          '[separator-5]',
          '4',
          '5',
          '6',
          { latex: '\\cdot', label: '×', tooltip: '掛け算' },
        ],
        [
          { latex: '#@^{2}', label: 'x²', tooltip: '2乗' },
          { latex: '#@^{#?}', label: 'xⁿ', tooltip: 'べき乗' },
          { latex: '\\sqrt{#0}', label: '√', tooltip: '平方根' },
          { latex: '\\left|#0\\right|', label: '|x|', tooltip: '絶対値' },
          { latex: '!', tooltip: '階乗' },
          { latex: '=', tooltip: '等号（陰関数・パラメータの定義）' },
          '[separator-5]',
          '1',
          '2',
          '3',
          { latex: '-', tooltip: '引き算・マイナス' },
        ],
        // 一文字消すボタン（⌫）は、ふつうのキーボードと同じく右下に置く（⏎ は左下の矢印の隣）
        [
          { label: '[left]', width: 1 },
          { label: '[right]', width: 1 },
          '[hide-keyboard]',
          { label: '[return]', width: 1 },
          '[separator-5]',
          '0',
          { latex: '.', tooltip: '小数点' },
          { latex: '+', tooltip: '足し算' },
          { label: '[backspace]', width: 1 },
        ],
      ],
    },
    {
      label: 'f(x)',
      tooltip: '関数',
      rows: [
        [fn('sin', '正弦'), fn('cos', '余弦'), fn('tan', '正接'), fn('sinh', '双曲線正弦'), fn('cosh', '双曲線余弦'), fn('tanh', '双曲線正接')],
        [fn('arcsin', '逆正弦'), fn('arccos', '逆余弦'), fn('arctan', '逆正接'), fn('exp', '指数関数 eˣ'), fn('ln', '自然対数（底 e）'), fn('log', '常用対数（底 10）')],
        [
          fn('floor', '床関数（小数点以下を切り下げ）', { operator: true }),
          fn('ceil', '天井関数（小数点以下を切り上げ）', { operator: true }),
          fn('round', '四捨五入', { operator: true }),
          fn('sign', '符号（正なら 1、負なら −1）', { operator: true }),
          { latex: '\\Gamma\\left(#0\\right)', label: 'Γ', tooltip: 'ガンマ関数\nΓ(z)' },
          '!',
          { latex: '\\left[#0\\right]', label: '[ ]', tooltip: 'リスト（[−1, 1] など）' },
        ],
        [
          fn('mod', '割り算の余り', { operator: true, args: 2 }),
          fn('min', '小さいほう', { operator: true, args: 2 }),
          fn('max', '大きいほう', { operator: true, args: 2 }),
          fn('nCr', '組み合わせの数', { operator: true, args: 2 }),
          fn('nPr', '順列の数', { operator: true, args: 2 }),
        ],
        [
          fn('Re', '実部', { operator: true }),
          fn('Im', '虚部', { operator: true }),
          fn('conj', '共役複素数', { operator: true }),
          fn('arg', '偏角', { operator: true }),
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

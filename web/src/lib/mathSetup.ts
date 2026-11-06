// 数式入力欄（MathLive）の共通設定: フォントの場所、自動変換の一覧、画面上の数式キーボード。

import { MathfieldElement } from 'mathlive'

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
  pi: '\\pi',
  sqrt: '\\sqrt{#?}',
  gamma: '\\Gamma',
  // w2 → w₂、t1 → t₁ のように、w と t の後ろの数字は添字にする（4次元目以降の座標・媒介変数）
  ...Object.fromEntries(
    ['w', 't'].flatMap((c) => Array.from({ length: 9 }, (_, k) => [`${c}${k + 1}`, `${c}_{${k + 1}}`])),
  ),
  ...Object.fromEntries(
    ['sin', 'cos', 'tan', 'sinh', 'cosh', 'tanh', 'arcsin', 'arccos', 'arctan', 'exp', 'ln', 'log'].map((f) => [
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
        ['[left]', '[right]', '[backspace]', { label: '[hide-keyboard]', width: 1.5 }, '[separator-5]', '0', '.', '+'],
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
          '[left]',
          '[right]',
          '[backspace]',
        ],
      ],
    },
    'alphabetic',
  ]
}

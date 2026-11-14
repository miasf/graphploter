// サンプル（タップすると式が一覧に追加される例）

import type { PlotItem } from './types'

export interface SampleItem {
  /** 数式欄の LaTeX */
  latex: string
  /** 見た目などの設定 */
  options?: Partial<PlotItem>
  /** パラメータの定義（a = 1 など）なら、その名前。同じ名前がすでにあれば追加しない */
  param?: string
}

export interface Sample {
  label: string
  /** 必要な次元（足りなければ自動で増やす） */
  dims?: number
  items: SampleItem[]
  /** 式を追加するだけでなく、次元・回転・表示範囲も設定する例（今ある式は非表示にする） */
  view?: SampleView
  /** 追加したら、このパラメータを再生する（値を自動で動かす） */
  play?: string
}

/** サンプルと一緒に設定する見え方 */
export interface SampleView {
  /** 次元 */
  dims: number
  /** 回転角（度）。座標の組の番号（(0,1), (0,2), (0,3), (0,4), (1,2), … の順）→ 角度 */
  rotation?: Record<number, number>
  /** 4次元の遠近感（0〜100） */
  perspective?: number
  /** 表示範囲（−half 〜 half） */
  half: number
  /** 再生する回転（座標の組の番号） */
  play?: number
}

// 回転の組の番号（5次元までの回転。params.ts の pairs と同じ並び）
const XW = 2
const YZ = 4
const YW2 = 6
const ZW = 7

export interface SampleGroup {
  title: string
  samples: Sample[]
}

const TWO_PI = ['0', '2\\pi'] as [string, string]

/** n 次元の立方体 (t₁, …, tₙ)（各 −1〜1）。媒介変数が n 個なので、面をまとめて描く */
function cube(n: number, color: string, opacity: number): SampleItem {
  const vars = Array.from({ length: n }, (_, k) => k + 1)
  return {
    latex: `\\left(${vars.map((k) => `t_${k}`).join(',')}\\right)`,
    options: {
      color,
      opacity,
      solidSeen: true,
      grid: { enabled: true, x: 1, y: 1, z: 0, u: 1, v: 1, w: 1, width: 1.5 },
      ranges: Object.fromEntries(vars.map((k) => [`t${k}`, ['-1', '1'] as [string, string]])),
    },
  }
}

export const SAMPLE_GROUPS: SampleGroup[] = [
  {
    title: '曲面・曲線',
    samples: [
      {
        label: 'ガウス曲面（格子のみ・a で高さ）',
        items: [
          { latex: 'a=1', param: 'a' },
          {
            latex: 'z=2ae^{-\\frac{x^2+y^2}{4}}-1',
            options: {
              color: '#3fb6c8',
              opacity: 0,
              grid: { enabled: true, x: 0.25, y: 0.25, z: 0, u: 24, v: 24, width: 1 },
            },
          },
        ],
      },
      { label: '鞍形 z = (x² − y²)/4', items: [{ latex: 'z=\\frac{x^2-y^2}{4}' }] },
      {
        label: '階段 z = ⌊x⌋/2 + ⌊y⌋/4（段差の壁つき）',
        items: [
          {
            latex: 'z=\\frac{\\left\\lfloor x\\right\\rfloor}{2}+\\frac{\\left\\lfloor y\\right\\rfloor}{4}',
            options: { color: '#1a9ea0', walls: true },
          },
        ],
      },
      {
        label: 'らせん（曲線）',
        items: [
          {
            latex: '\\left(4\\cos t,4\\sin t,\\frac{t}{5}\\right)',
            options: { color: '#c74440', ranges: { t: ['-4\\pi', '4\\pi'], u: TWO_PI, v: TWO_PI } },
          },
        ],
      },
      {
        label: '伸びるらせん（0 ≤ t ≤ b の b を再生）',
        play: 'b',
        items: [
          { latex: 'b=0.1', param: 'b', options: { slider: { min: 0, max: 24, complex: false, stack: false } } },
          {
            latex: '\\left(3\\cos t,3\\sin t,\\frac{t}{4}-3\\right)',
            options: { color: '#c74440', lineWidth: 4, ranges: { t: ['0', 'b'], u: TWO_PI, v: TWO_PI } },
          },
        ],
      },
      {
        label: '広がるトーラス（0 ≤ u ≤ c の c を再生）',
        play: 'c',
        items: [
          { latex: 'c=0.1', param: 'c', options: { slider: { min: 0, max: 6.3, complex: false, stack: false } } },
          {
            latex: '\\left(\\left(3+\\cos v\\right)\\cos u,\\left(3+\\cos v\\right)\\sin u,\\sin v\\right)',
            options: { color: '#6042a6', ranges: { t: TWO_PI, u: ['0', 'c'], v: TWO_PI } },
          },
        ],
      },
      {
        label: '中身の詰まったトーラス（媒介変数3つの立体）',
        items: [
          {
            latex:
              '\\left(\\left(3+t\\cos v\\right)\\cos u,\\left(3+t\\cos v\\right)\\sin u,t\\sin v\\right)',
            options: {
              color: '#fa7e19',
              ranges: { t: ['0', '1.5'], u: ['\\frac{\\pi}{6}', '\\frac{5\\pi}{3}'], v: TWO_PI },
              grid: { enabled: true, x: 1, y: 1, z: 0, u: 24, v: 12, w: 3, width: 1 },
            },
          },
        ],
      },
      {
        label: 'トーラス（媒介変数の曲面）',
        items: [
          {
            latex: '\\left(\\left(3+\\cos v\\right)\\cos u,\\left(3+\\cos v\\right)\\sin u,\\sin v\\right)',
            options: { color: '#6042a6' },
          },
        ],
      },
    ],
  },
  {
    title: '陰関数',
    samples: [
      { label: '球 x² + y² + z² = 4', items: [{ latex: 'x^2+y^2+z^2=4' }] },
      {
        label: 'リストで3つの球 x² + y² + z² = [1, 4, 9]',
        items: [
          {
            latex: 'x^2+y^2+z^2=\\left[1,4,9\\right]',
            options: { color: '#2d70b3', opacity: 0.45 },
          },
        ],
      },
      {
        label: '接するだけ (x² + y² − 4)²(z − 1)² = 0',
        items: [{ latex: '\\left(x^2+y^2-4\\right)^2\\left(z-1\\right)^2=0', options: { color: '#6042a6' } }],
      },
      {
        label: 'ジャイロイド',
        items: [{ latex: '\\sin x\\cos y+\\sin y\\cos z+\\sin z\\cos x=0', options: { color: '#c74440' } }],
      },
    ],
  },
  {
    title: '連立方程式・不等式',
    samples: [
      {
        label: 'ヴィヴィアーニの曲線（球と円柱の交わり）',
        view: { dims: 3, half: 3 },
        items: [
          { latex: 'x^2+y^2+z^2=4', options: { color: '#2d70b3', opacity: 0.3 } },
          { latex: 'x^2+y^2=2x', options: { color: '#388c46', opacity: 0.3 } },
          { latex: 'x^2+y^2+z^2=4,x^2+y^2=2x', options: { color: '#c74440', lineWidth: 6 } },
        ],
      },
      {
        label: 'sin x = cos y = tan z（交わる線）',
        items: [{ latex: '\\sin x=\\cos y=\\tan z', options: { color: '#6042a6', lineWidth: 4 } }],
      },
      { label: '領域 x ≤ y ≤ z', items: [{ latex: 'x\\le y\\le z', options: { color: '#c74440' } }] },
      {
        label: 'シュタインメッツの立体（2 本の円柱の共通部分）',
        items: [{ latex: 'x^2+y^2\\le9,x^2+z^2\\le9', options: { color: '#fa7e19' } }],
      },
      {
        label: '領域 0 ≤ z ≤ sin x cos y',
        items: [{ latex: '0\\le z\\le\\sin x\\cos y', options: { color: '#2d70b3' } }],
      },
      { label: '半球 x² + y² + z² = 4, z ≥ 0', items: [{ latex: 'x^2+y^2+z^2=4,z\\ge0', options: { color: '#388c46' } }] },
      {
        label: '線分 y = x, z = 0, 0 ≤ x ≤ 1 と、点 x = y = z = 1',
        items: [
          { latex: 'y=x,z=0,0\\le x\\le1', options: { color: '#c74440', lineWidth: 5 } },
          { latex: 'x=y=z=1', options: { color: '#2d70b3', lineWidth: 6 } },
        ],
      },
    ],
  },
  {
    title: '複素数・特殊関数',
    samples: [
      {
        label: '複素平面の eⁱᵗ（単位円）と、t で持ち上げたらせん',
        items: [
          { latex: 'e^{it}', options: { color: '#c74440', lineWidth: 4, ranges: { t: TWO_PI, u: TWO_PI, v: TWO_PI } } },
          {
            latex: 'e^{it}',
            options: { color: '#2d70b3', lineWidth: 3, complexMode: 'param', ranges: { t: ['0', '4\\pi'], u: TWO_PI, v: TWO_PI } },
          },
        ],
      },
      {
        label: '複素平面の点 c, c², 1/c（c をドラッグ）と単位円',
        items: [
          { latex: 'c=0.8+0.6i', param: 'c' },
          { latex: 'c', options: { color: '#c74440', lineWidth: 4 } },
          { latex: 'c^2', options: { color: '#2d70b3', lineWidth: 4 } },
          { latex: '\\frac{1}{c}', options: { color: '#388c46', lineWidth: 4 } },
          { latex: 'e^{it}', options: { color: '#000000', lineWidth: 1.5, ranges: { t: TWO_PI, u: TWO_PI, v: TWO_PI } } },
        ],
      },
      {
        label: '複素平面の等角写像 (u + iv)²（格子線の像）',
        items: [
          {
            latex: '\\left(u+iv\\right)^2',
            options: { color: '#388c46', opacity: 0.35, ranges: { t: TWO_PI, u: ['0', '1.5'], v: ['-1.5', '1.5'] } },
          },
        ],
      },
      {
        label: '複素平面の Γ(1 + it)（縦は |Γ|）',
        items: [
          {
            latex: '\\Gamma\\left(1+it\\right)',
            options: { color: '#6042a6', lineWidth: 4, complexMode: 'abs', ranges: { t: ['-4', '4'], u: TWO_PI, v: TWO_PI } },
          },
        ],
      },
      {
        label: '複素数の陰関数 e^{ix} = y + iz（らせん）と (x + iy)² = z（放物線 2 本）',
        view: { dims: 3, half: 3 },
        items: [
          { latex: 'e^{ix}=y+iz', options: { color: '#c74440', lineWidth: 4 } },
          { latex: '\\left(x+iy\\right)^2-z=0', options: { color: '#2d70b3', lineWidth: 4 } },
        ],
      },
      { label: '|Γ(x + iy)|', items: [{ latex: 'z=\\left|\\Gamma\\left(x+iy\\right)\\right|', options: { color: '#fa7e19' } }] },
      {
        label: 'Γ(x + iy) の色分け（高さ |Γ|、色は偏角）',
        items: [{ latex: 'z=\\Gamma\\left(x+iy\\right)', options: { complexMode: 'abs' } }],
      },
      { label: '|ζ(x + iy)|', items: [{ latex: 'z=\\left|\\zeta\\left(x+iy\\right)\\right|', options: { color: '#2d70b3' } }] },
      {
        label: 'ベッセル関数 J₀ の膜',
        items: [{ latex: 'z=2\\operatorname{besselJ}\\left(0,2\\sqrt{x^2+y^2}\\right)', options: { color: '#c74440' } }],
      },
      {
        label: '球面調和関数 Y₃² の形',
        items: [
          {
            latex:
              '\\left(5\\left|\\operatorname{Re}\\left(\\operatorname{sphericalHarmonicY}\\left(3,2,u,v\\right)\\right)\\right|\\sin u\\cos v,5\\left|\\operatorname{Re}\\left(\\operatorname{sphericalHarmonicY}\\left(3,2,u,v\\right)\\right)\\right|\\sin u\\sin v,5\\left|\\operatorname{Re}\\left(\\operatorname{sphericalHarmonicY}\\left(3,2,u,v\\right)\\right)\\right|\\cos u\\right)',
            options: { color: '#388c46', ranges: { t: TWO_PI, u: ['0', '\\pi'], v: TWO_PI } },
          },
        ],
      },
      {
        label: 'エアリー関数 Ai と Bi',
        items: [
          { latex: '\\left(t,\\operatorname{airyAi}\\left(t\\right),0\\right)', options: { color: '#c74440', lineWidth: 4, ranges: { t: ['-5', '3'], u: TWO_PI, v: TWO_PI } } },
          { latex: '\\left(t,\\operatorname{airyBi}\\left(t\\right),0\\right)', options: { color: '#2d70b3', lineWidth: 4, ranges: { t: ['-5', '2'], u: TWO_PI, v: TWO_PI } } },
        ],
      },
    ],
  },
  {
    title: '4次元',
    samples: [
      {
        label: '超球の断面（w を動かす）',
        dims: 4,
        items: [{ latex: 'x^2+y^2+z^2+w^2=4', options: { color: '#2d70b3' } }],
      },
      {
        label: 'w = x² + y² + z² を等値面で（w の値ごとに 6 枚）',
        view: { dims: 4, half: 3 },
        items: [
          {
            latex: 'w=x^2+y^2+z^2',
            options: {
              opacity: 0.35,
              hidden: { mode: 'layers', count: 6, colormap: 'viridis', opacity: 'flat', density: 0.5 },
              ranges: { w: ['0.5', '8'] },
            },
          },
        ],
      },
      {
        label: 'w = sin x sin y sin z を伝達関数で（等値面風）',
        view: { dims: 4, half: 3 },
        items: [
          {
            latex: 'w=\\sin x\\sin y\\sin z',
            options: {
              hidden: { mode: 'volume', count: 4, colormap: 'turbo', opacity: 'bands', density: 0.5 },
              ranges: { w: ['-1', '1'] },
            },
          },
        ],
      },
      {
        label: 'w = sin x + cos y + z の断面の塗り分け',
        view: { dims: 4, half: 3 },
        items: [
          {
            latex: 'w=\\sin x+\\cos y+z',
            options: {
              hidden: { mode: 'planes', count: 2, colormap: 'turbo', opacity: 'flat', density: 0.5 },
              ranges: { w: ['-3', '3'] },
            },
          },
        ],
      },
      {
        label: 'クリフォードトーラス',
        dims: 4,
        items: [
          {
            latex: '\\left(2\\cos u,2\\sin u,\\cos v,\\sin v\\right)',
            options: { color: '#6042a6', opacity: 0.6 },
          },
        ],
      },
      {
        label: '正八胞体（4次元の立方体）の回転',
        // 辺 32 本: (t₁, t₂, t₃, t₄) を「辺だけ描く」で、1つの媒介変数だけを動かした線（残りは ±1）にする
        items: [
          {
            latex: '\\left(t_1,t_2,t_3,t_4\\right)',
            options: {
              color: '#2d70b3',
              lineWidth: 2.5,
              edges: true,
              ranges: Object.fromEntries([1, 2, 3, 4].map((k) => [`t${k}`, ['-1', '1'] as [string, string]])),
            },
          },
        ],
        view: { dims: 4, rotation: { [XW]: 20, [YZ]: 20 }, perspective: 40, half: 2.5, play: XW },
      },
    ],
  },
  {
    title: '多次元の図形・関数',
    samples: [
      {
        label: 'クラインの壺（4次元では自分と交わらない）',
        items: [
          {
            latex:
              '\\left(\\left(2+\\cos v\\right)\\cos u,\\left(2+\\cos v\\right)\\sin u,\\sin v\\cos\\frac{u}{2},\\sin v\\sin\\frac{u}{2}\\right)',
            options: { color: '#1a9ea0', opacity: 0.75, grid: { enabled: true, x: 1, y: 1, z: 0, u: 24, v: 12, w: 12, width: 1 } },
          },
        ],
        view: { dims: 4, rotation: { [ZW]: 0, [YZ]: 25 }, perspective: 30, half: 4, play: ZW },
      },
      {
        label: 'ホップのトーラス（3次元球面 S³ の中の入れ子のトーラス）',
        // S³ の点 (cos a·e^{iu}, sin a·e^{iv})。a を変えると、S³ を埋めつくすトーラスの族になる
        items: [
          ['0.35', '#c74440'],
          ['0.785', '#388c46'],
          ['1.2', '#2d70b3'],
        ].map(([a, color]) => ({
          latex: `\\left(\\cos\\left(${a}\\right)\\cos u,\\cos\\left(${a}\\right)\\sin u,\\sin\\left(${a}\\right)\\cos v,\\sin\\left(${a}\\right)\\sin v\\right)`,
          options: { color, opacity: 0.6, grid: { enabled: true, x: 1, y: 1, z: 0, u: 16, v: 16, w: 16, width: 1 } },
        })),
        view: { dims: 4, rotation: { [XW]: 0, [YZ]: 20 }, perspective: 55, half: 2.2, play: XW },
      },
      {
        label: 'リーマン面 √z（複素関数 z = w² のグラフ。4次元の曲面）',
        // w = u + iv として (Re w², Im w², Re w, Im w)。3次元では自分と交わって見えるが、4次元では交わらない
        items: [
          {
            latex: '\\left(u^2-v^2,2uv,u,v\\right)',
            options: {
              color: '#6042a6',
              opacity: 0.8,
              grid: { enabled: true, x: 1, y: 1, z: 0, u: 12, v: 12, w: 12, width: 1 },
              ranges: { u: ['-1.5', '1.5'], v: ['-1.5', '1.5'] },
            },
          },
        ],
        view: { dims: 4, rotation: { [ZW]: 0, [YZ]: 15 }, perspective: 0, half: 2.5, play: ZW },
      },
      {
        label: 'フェルマー曲面 z₁³ + z₂³ = 1（カラビ・ヤウ多様体の絵でおなじみの形。9 枚）',
        // z₁ = e^{2πik₁/3} cos(u + iv)^{2/3}, z₂ = e^{2πik₂/3} sin(u + iv)^{2/3} の (Re z₁, Re z₂, Im z₁, Im z₂)。
        // k₁, k₂ = 0, 1, 2 の 9 枚で1つの曲面になる
        items: [0, 1, 2].flatMap((k1) =>
          [0, 1, 2].map((k2) => {
            const phase = (k: number) => (k === 0 ? '' : `e^{\\frac{${2 * k}\\pi i}{3}}`)
            const z1 = `${phase(k1)}\\left(\\cos\\left(u+iv\\right)\\right)^{\\frac{2}{3}}`
            const z2 = `${phase(k2)}\\left(\\sin\\left(u+iv\\right)\\right)^{\\frac{2}{3}}`
            const re = (z: string) => `\\operatorname{Re}\\left(${z}\\right)`
            const im = (z: string) => `\\operatorname{Im}\\left(${z}\\right)`
            return {
              latex: `\\left(${re(z1)},${re(z2)},${im(z1)},${im(z2)}\\right)`,
              options: {
                color: ['#c74440', '#fa7e19', '#388c46', '#1a9ea0', '#2d70b3', '#6042a6', '#c74440', '#fa7e19', '#388c46'][(k1 * 3 + k2 * 4) % 9],
                grid: { enabled: true, x: 1, y: 1, z: 0, u: 8, v: 8, w: 8, width: 1 },
                ranges: { u: ['0', '\\frac{\\pi}{2}'], v: ['-1', '1'] } as Record<string, [string, string]>,
              },
            }
          }),
        ),
        view: { dims: 4, rotation: { [ZW]: 35, [YZ]: 20 }, perspective: 0, half: 2, play: ZW },
      },
      {
        label: '3変数の関数 w = sin x + sin y + sin z の等値面（w を動かす）',
        dims: 4,
        items: [{ latex: 'w=\\sin x+\\sin y+\\sin z', options: { color: '#fa7e19' } }],
      },
      {
        label: '5次元の超球の断面（w と w₂ を動かす）',
        dims: 5,
        items: [{ latex: 'x^2+y^2+z^2+w^2+w_2^2=9', options: { color: '#2d70b3' } }],
      },
    ],
  },
  {
    title: '多変数（媒介変数4つ以上）',
    samples: [
      {
        label: '4次元の立方体の面 24 枚 (t₁, t₂, t₃, t₄) の回転',
        items: [cube(4, '#2d70b3', 0.3)],
        view: { dims: 4, rotation: { [XW]: 20, [YZ]: 20 }, perspective: 40, half: 2.5, play: XW },
      },
      {
        label: '5次元の立方体の面 80 枚 (t₁, …, t₅) の回転',
        items: [cube(5, '#6042a6', 0.2)],
        view: { dims: 5, rotation: { [XW]: 20, [YZ]: 20, [YW2]: 35 }, perspective: 40, half: 3.5, play: XW },
      },
      {
        label: 'デュオシリンダー（円板 × 円板。半径2つと角度2つ）',
        // (t₃ e^{i t₁}, t₄ e^{i t₂}): 2つの円板の直積。表面は2つの「中身の詰まったトーラス」でできている
        items: [
          {
            latex: '\\left(t_3\\cos t_1,t_3\\sin t_1,t_4\\cos t_2,t_4\\sin t_2\\right)',
            options: {
              color: '#fa7e19',
              opacity: 0.35,
              solidSeen: true,
              grid: { enabled: true, x: 1, y: 1, z: 0, u: 8, v: 8, w: 8, width: 1 },
              ranges: { t1: TWO_PI, t2: TWO_PI, t3: ['0', '1'], t4: ['0', '1'] },
            },
          },
        ],
        view: { dims: 4, rotation: { [XW]: 30, [YZ]: 25 }, perspective: 50, half: 2, play: XW },
      },
      {
        label: '4次元の球の中身（半径 t₁ と3つの角度）',
        dims: 4,
        items: [
          {
            latex:
              '\\left(t_1\\sin t_2\\sin t_3\\cos t_4,t_1\\sin t_2\\sin t_3\\sin t_4,t_1\\sin t_2\\cos t_3,t_1\\cos t_2\\right)',
            options: {
              color: '#388c46',
              opacity: 0.4,
              layers: 2,
              grid: { enabled: true, x: 1, y: 1, z: 0, u: 6, v: 6, w: 6, width: 1 },
              ranges: { t1: ['0', '2'], t2: ['0', '\\pi'], t3: ['0', '\\pi'], t4: TWO_PI },
            },
          },
        ],
      },
    ],
  },
]

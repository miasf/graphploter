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
  /** 式を追加するだけでなく、表示範囲や回転も設定する特別な例 */
  special?: 'tesseract'
}

export interface SampleGroup {
  title: string
  samples: Sample[]
}

const TWO_PI = ['0', '2\\pi'] as [string, string]

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
        label: 'らせん（曲線）',
        items: [
          {
            latex: '\\left(4\\cos t,4\\sin t,\\frac{t}{5}\\right)',
            options: { color: '#c74440', ranges: { t: ['-4\\pi', '4\\pi'], u: TWO_PI, v: TWO_PI } },
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
        label: 'ジャイロイド',
        items: [{ latex: '\\sin x\\cos y+\\sin y\\cos z+\\sin z\\cos x=0', options: { color: '#c74440' } }],
      },
    ],
  },
  {
    title: '複素数・特殊関数',
    samples: [
      { label: '|Γ(x + iy)|', items: [{ latex: 'z=\\left|\\Gamma\\left(x+iy\\right)\\right|', options: { color: '#fa7e19' } }] },
      { label: '|ζ(x + iy)|', items: [{ latex: 'z=\\left|\\zeta\\left(x+iy\\right)\\right|', options: { color: '#2d70b3' } }] },
      {
        label: 'ベッセル関数 J₀ の膜',
        items: [{ latex: 'z=2\\operatorname{besselJ}\\left(0,2\\sqrt{x^2+y^2}\\right)', options: { color: '#c74440' } }],
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
        label: 'クリフォードトーラス',
        dims: 4,
        items: [
          {
            latex: '\\left(2\\cos u,2\\sin u,\\cos v,\\sin v\\right)',
            options: { color: '#6042a6', opacity: 0.6 },
          },
        ],
      },
      { label: '正八胞体（4次元の立方体）の回転', items: [], special: 'tesseract' },
    ],
  },
]

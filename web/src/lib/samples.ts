// サンプル（タップすると式が一覧に追加される例）

import type { CoordPreset, PlotItem } from './types'

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
  /** 2D 表示にする */
  flat?: boolean
  /**
   * 座標系で描く例（計画 9 章）。追加する式を、この座標系（用意した座標系。同じものがあれば使い回す）で描く。
   * morph は移行のスライダーの値、play なら移行を再生する、flat なら 2D 表示にする
   */
  /** 追加したら、積分のある式の「∫ 積分を描く」を押して、進み具合をこの値（0〜1）にする */
  integral?: number
  coord?: { preset: CoordPreset; morph: number; play?: boolean; flat?: boolean; ranges?: [[string, string], [string, string], [string, string]] }
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
      // 画素ごとの描き方（GPU の光線。計画 12 章）でよく描ける式。メッシュでは崩れる・消える
      {
        label: 'tan x = z y（極に偽の壁が出ない）',
        items: [{ latex: '\\tan\\left(x\\right)=zy', options: { color: '#388c46' } }],
      },
      {
        label: '細い輪（管の太さ 0.02。メッシュでは消える）',
        items: [{ latex: '\\left(\\sqrt{x^2+y^2}-3\\right)^2+z^2=0.01^2', options: { color: '#fa7e19' } }],
      },
      {
        label: '⌊x⌋ + ⌊y⌋ + ⌊z⌋ = 0（解が体積を持つ）',
        items: [{ latex: '\\left\\lfloor x\\right\\rfloor+\\left\\lfloor y\\right\\rfloor+\\left\\lfloor z\\right\\rfloor=0', options: { color: '#2d70b3' } }],
      },
    ],
  },
  {
    title: '関数・Σ',
    samples: [
      {
        label: '関数の定義と合成 f(x) = x²、z = f(x) − f(y)',
        items: [
          { latex: 'f\\left(x\\right)=\\frac{x^2}{4}', options: { color: '#c74440', visible: false } },
          { latex: 'z=f\\left(x\\right)-f\\left(y\\right)', options: { color: '#2d70b3' } },
        ],
      },
      {
        label: '矩形波のフーリエ級数（N を再生）',
        play: 'N',
        items: [
          { latex: 'N=1', param: 'N', options: { slider: { min: 1, max: 30, complex: false, stack: false, stepExpr: '1', step: 1 } } },
          {
            latex: 'y=\\frac{4}{\\pi}\\sum_{n=1}^{N}\\frac{\\sin\\left(\\left(2n-1\\right)x\\right)}{2n-1}',
            options: { color: '#388c46' },
          },
        ],
      },
    ],
  },
  {
    title: '数学の記法',
    samples: [
      {
        label: '場合分け',
        items: [{ latex: 'z=\\begin{cases}x^2-y^2 & x<0\\\\ \\sin\\left(xy\\right) & \\text{otherwise}\\end{cases}', options: { color: '#c74440' } }],
      },
      {
        label: '定積分で定義した関数 ∫₀ˣ e^(−t²) dt',
        items: [{ latex: 'y=\\int_{0}^{x}e^{-t^{2}}\\,dt', options: { color: '#2d70b3' } }],
      },
      {
        label: '線積分 ∮ F⃗ · dr（らせん。スライダーで t を進める）',
        items: [
          { latex: '\\vec{F}\\left(x,y,z\\right)=\\left(-y,x,1\\right)', options: { color: '#6042a6', opacity: 0.8 } },
          { latex: '\\oint_{\\left\\{\\left(3\\cos t,3\\sin t,t\\right)\\mid0\\le t\\le2\\pi\\right\\}}\\vec{F}\\cdot d\\mathbf{r}', options: { color: '#388c46' } },
        ],
        integral: 0.3,
      },
      {
        label: '流束 ∬ F⃗ · dS とガウスの定理 ∭ ∇ · F⃗ dV（球）',
        items: [
          { latex: '\\vec{G}\\left(x,y,z\\right)=\\left(x,y,z\\right)', options: { color: '#6042a6', opacity: 0.8 } },
          {
            latex: '\\iint_{\\left\\{\\left(3\\sin u\\cos v,3\\sin u\\sin v,3\\cos u\\right)\\mid0\\le u\\le\\pi,0\\le v\\le2\\pi\\right\\}}\\vec{G}\\cdot d\\mathbf{S}',
            options: { color: '#388c46' },
          },
        ],
        integral: 0.5,
      },
      {
        label: 'スカラー場の線積分 ∫ f ds（平面の曲線の上の幕）',
        items: [
          {
            latex: '\\int_{\\left\\{\\left(4\\cos t,2\\sin t\\right)\\mid0\\le t\\le2\\pi\\right\\}}1+\\frac{x^2}{8}\\,ds',
            options: { color: '#fa7e19' },
          },
        ],
        integral: 0.4,
      },
      {
        label: '集合 {(x, y, z) | x² + y² ≤ 4, 0 ≤ z ≤ x}',
        items: [{ latex: '\\left\\{\\left(x,y,z\\right)\\mid x^2+y^2\\le4,0\\le z\\le x\\right\\}', options: { color: '#388c46', opacity: 0.6 } }],
      },
      {
        label: '球が通る領域 ∃a ∈ [0, 3]',
        items: [{ latex: '\\exists a\\in\\left[0,3\\right],x^2+y^2+\\left(z-a\\right)^2=1', options: { color: '#6042a6', opacity: 0.6 } }],
      },
      {
        label: '最大公約数 z = gcd(x, y)/3',
        items: [{ latex: 'z=\\frac{\\gcd\\left(x,y\\right)}{3}', options: { color: '#fa7e19', walls: true } }],
      },
    ],
  },
  {
    title: 'ベクトル・行列',
    samples: [
      { label: '回転する場 (−y, x, 0)', items: [{ latex: '\\left(-y,x,0\\right)', options: { color: '#2d70b3' } }] },
      {
        label: '勾配の場 ∇(x² + y² − z) と等位面',
        items: [
          { latex: '\\nabla\\left(x^2+y^2-z\\right)', options: { color: '#c74440' } },
          { latex: 'x^2+y^2-z=0', options: { color: '#2d70b3', opacity: 0.4 } },
        ],
      },
      {
        label: '行列のせん断で円を変形（a を再生）',
        play: 'a',
        items: [
          { latex: 'a=1', param: 'a', options: { slider: { min: -2, max: 2, complex: false, stack: false } } },
          { latex: 'A=\\begin{pmatrix}1 & a\\\\ 0 & 1\\end{pmatrix}', options: { color: '#c74440' } },
          { latex: 'A\\left(\\cos t,\\sin t\\right)', options: { color: '#2d70b3' } },
        ],
      },
      {
        label: '回転する場と流線',
        items: [{ latex: '\\left(-y,x,0.3\\right)', options: { color: '#2d70b3', field: { stream: 3 } } }],
      },
      { label: '零空間 ker (1 1 1)（平面）', items: [{ latex: '\\ker\\begin{pmatrix}1 & 1 & 1\\end{pmatrix}', options: { color: '#6042a6' } }] },
    ],
  },
  {
    title: '数列',
    samples: [
      {
        label: '収束する数列 aₙ₊₁ = aₙ/2 + 1（a₁ を動かす）',
        view: { dims: 3, half: 12 },
        items: [
          { latex: 'a_1=8', param: 'a1', options: { slider: { min: -10, max: 10, complex: false, stack: false } } },
          { latex: 'a_{n+1}=\\frac{a_n}{2}+1', options: { color: '#c74440', lineWidth: 6 } },
        ],
      },
      {
        label: 'マンデルブロ集合の縁 |z₂₀| = 2（2D）',
        flat: true,
        view: { dims: 3, half: 2 },
        items: [
          { latex: 'z_0=0', param: 'z0' },
          { latex: 'z_{n+1}=z_n^2+x+iy', options: { color: '#888888' } },
          { latex: '\\left|z_{20}\\right|=2', options: { color: '#2d70b3' } },
        ],
      },
    ],
  },
  {
    title: '常微分方程式',
    samples: [
      {
        label: 'ローレンツ・アトラクター',
        view: { dims: 3, half: 30 },
        items: [
          {
            latex: "x'=10\\left(y-x\\right),\\ y'=x\\left(28-z\\right)-y,\\ z'=xy-\\frac{8z}{3}",
            options: { color: '#c74440', ranges: { t: ['0', '40'] } },
          },
        ],
      },
      {
        label: '減衰振動 x″ = −x − x′/5（初期値を 3 通り）',
        items: [
          {
            latex: "x''=-x-\\frac{x'}{5},\\ y'=1",
            options: { color: '#2d70b3', ranges: { t: ['0', '20'] }, ode: { init: ['\\left[1,2,3\\right]', '0', '-10'] } },
          },
        ],
      },
      {
        label: "傾きの場 y′ = x − y と解曲線",
        items: [{ latex: "y'=x-y", options: { color: '#388c46', ode: { init: ['\\left[-4,-2,0,2,4\\right]'] } } }],
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
    title: '座標系',
    samples: [
      {
        label: '球座標の (t, t, t)（まっすぐな線がらせんに）',
        items: [{ latex: '\\left(t,t,t\\right)', options: { color: '#c74440', lineWidth: 4, ranges: { t: ['0', '5'] } } }],
        coord: { preset: 'sphere', morph: 0, play: true },
      },
      {
        label: '球座標の x = 2（平面が球に）',
        items: [{ latex: 'x=2', options: { color: '#2d70b3' } }],
        coord: { preset: 'sphere', morph: 1 },
      },
      {
        label: '円柱座標の x = 1 + cos 3y',
        items: [{ latex: 'x=1+\\cos\\left(3y\\right)', options: { color: '#388c46' } }],
        coord: { preset: 'cylinder', morph: 1 },
      },
      {
        label: '極座標のバラ曲線 x = cos 2y（2D 表示）',
        items: [{ latex: 'x=\\cos\\left(2y\\right)', options: { color: '#6042a6', lineWidth: 4 } }],
        // 半径の負の側も描く（r < 0 の所が残りの 2 枚の花びら）
        coord: { preset: 'cylinder', morph: 0, play: true, flat: true, ranges: [['-5', '5'], ['0', '2\\pi'], ['-5', '5']] },
      },
      {
        label: 'トーラス座標の τ = 1（入れ子のトーラスの 1 枚）',
        items: [{ latex: '\\tau=1', options: { color: '#fa7e19' } }],
        coord: { preset: 'toroidal', morph: 0, play: true },
      },
      {
        label: '楕円座標の μ = 1 と、曲がった方眼（2D 表示）',
        items: [{ latex: '\\mu=1', options: { color: '#c74440', lineWidth: 4 } }],
        coord: { preset: 'ellipticCylinder', morph: 0, play: true, flat: true },
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
        label: 'w = sin x sin y sin z + cos(2x + y)/3 の最大値投影',
        view: { dims: 4, half: 3 },
        items: [
          {
            latex: 'w=\\sin x\\sin y\\sin z+\\frac{1}{3}\\cos(2x+y)',
            options: {
              hidden: { mode: 'volume', count: 4, colormap: 'turbo', opacity: 'high', density: 0.5, blend: 'max' },
              ranges: { w: ['-1.3', '1.3'] },
            },
          },
        ],
      },
      {
        label: 'ジャイロイドの F をぼやけた面で（陰関数の F の値の伝達関数）',
        view: { dims: 3, half: 3 },
        items: [
          {
            latex: '\\sin x\\cos y+\\sin y\\cos z+\\sin z\\cos x=0',
            options: {
              hidden: { mode: 'field', count: 3, colormap: 'shade', opacity: 'zero', density: 0.6 },
            },
          },
        ],
      },
      {
        label: '(x + iy)³ − z² = 1/2 の F = 0 を管で（複素数の値の伝達関数。色は偏角）',
        view: { dims: 3, half: 2 },
        items: [
          {
            latex: '\\left(x+iy\\right)^3-z^2=\\frac{1}{2}',
            options: {
              hidden: { mode: 'field', count: 3, colormap: 'shade', opacity: 'zero', density: 0.6 },
            },
          },
        ],
      },
      {
        label: 'w = sin(x + iy) + z の最小値投影（|w| が小さい所。色は偏角）',
        view: { dims: 4, half: 3 },
        items: [
          {
            latex: 'w=\\sin\\left(x+iy\\right)+z',
            options: {
              hidden: { mode: 'volume', count: 3, colormap: 'turbo', opacity: 'low', density: 0.5, blend: 'min' },
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
              opacity: 0.6,
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

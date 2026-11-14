// ベンチマーク（?bench=1 で開いたときだけ動く。計画 S0 の 5・6）。
//
// 式の一覧（core/tests/bench.rs と同じ式）を順に描き、式ごとに次を測る:
// - 計算の内訳（計算担当の中の eval / mesh / lod / copy / total と、受け渡し・描画側への登録）
// - カメラを 1 周 frames コマで回したときのコマの時間。描き方は (そのままの版 / 粗い版) × (画素密度: 基準 / 1 / 0.5) の 6 通り
// - 同じ視点で「止まった状態の絵」と比べた D（色の差が 24/255 を超える画素の割合）
// - 回転の再生のある式（4 次元）は、再生中の「射影し直しの往復」（計算担当 → 画面 → 登録）の時間（S14 をやるかの判断用）
//
// 結果は画面の <pre> と console.log(JSON) に出す。
// URL の引数: frames（1 周のコマ数。既定 240）、only（式の名前にこの文字を含むものだけ）、play（再生を測る秒数。既定 4）

import type { Viewer } from './Viewer'
import type { SampleItem } from './samples'
import type { Stage } from './motionQuality'
import { breakdownListeners, lastBreakdown, type PlotBreakdown } from './timings'

/** 測る式 1 つ分 */
export type BenchCase = {
  label: string
  items: SampleItem[]
  /** 次元 */
  dims: number
  /** 表示範囲（−half 〜 half） */
  half: number
  /** 陽関数・媒介変数の細かさ */
  res?: number
  /** 陰関数の細かさ */
  ires?: number
  /** 回転角（度）。座標の組の番号 → 角度 */
  rotation?: Record<number, number>
  /** 4 次元の遠近感（0〜100） */
  perspective?: number
  /** 再生中の往復を測る回転（座標の組の番号） */
  play?: number
}

/** 画面側（App.svelte）が用意する操作 */
export type BenchHost = {
  viewer: Viewer
  /** 今の式を全部消して、この式を描き、計算が全部終わるまで待つ */
  load(c: BenchCase): Promise<void>
  /** 回転 k を再生する（null で止める） */
  play(k: number | null): void
  /** 計算が全部終わるまで待つ */
  idle(): Promise<void>
}

// 4 次元の回転の組の番号（params.ts の pairs と同じ並び: (0,1), (0,2), (0,3), (1,2), …）
const XW = 2
const YZ = 3

const TWO_PI: [string, string] = ['0', '2\\pi']
const cube4: SampleItem = {
  latex: '\\left(t_1,t_2,t_3,t_4\\right)',
  options: {
    opacity: 0.3,
    solidSeen: true,
    grid: { enabled: true, x: 1, y: 1, z: 0, u: 1, v: 1, w: 1, width: 1.5 },
    ranges: Object.fromEntries([1, 2, 3, 4].map((k) => [`t${k}`, ['-1', '1'] as [string, string]])),
  },
}

const explicit = (label: string, latex: string): BenchCase => ({ label, items: [{ latex }], dims: 3, half: 5, res: 150 })
const implicit = (label: string, latex: string, ires = 80, half = 5): BenchCase => ({
  label: `${label}（${ires}）`,
  items: [{ latex }],
  dims: 3,
  half,
  ires,
})
const GYROID = '\\sin x\\cos y+\\sin y\\cos z+\\sin z\\cos x=0'
const torus4 = (res: number): BenchCase => ({
  label: `4 次元のトーラス（${res}）`,
  items: [{ latex: '\\left(\\cos u,\\sin u,\\cos v,\\sin v\\right)', options: { opacity: 0.6, ranges: { u: TWO_PI, v: TWO_PI } } }],
  dims: 4,
  half: 1.5,
  res,
  rotation: { [YZ]: 20 },
  perspective: 40,
  play: XW,
})

/** 測る式の一覧（core/tests/bench.rs の bench_report と同じ式） */
export const BENCH_CASES: BenchCase[] = [
  explicit('z = sin(x) cos(y)', 'z=\\sin x\\cos y'),
  explicit('z = x^2 - y^2', 'z=x^2-y^2'),
  explicit('z = 1/(x^2+y^2)', 'z=\\frac{1}{x^2+y^2}'),
  explicit('z = tan(x) y', 'z=\\tan\\left(x\\right)y'),
  explicit('z = floor(x) + floor(y)', 'z=\\lfloor x\\rfloor+\\lfloor y\\rfloor'),
  explicit('z = abs(gamma(x + i y))', 'z=\\left|\\Gamma\\left(x+iy\\right)\\right|'),
  explicit('z = abs(zeta(x + i y))', 'z=\\left|\\zeta\\left(x+iy\\right)\\right|'),
  explicit('z = sqrt(4 - x^2 - y^2)', 'z=\\sqrt{4-x^2-y^2}'),
  implicit('球 4', 'x^2+y^2+z^2=4'),
  implicit('球 4.01', 'x^2+y^2+z^2=4.01'),
  implicit('球 4.01', 'x^2+y^2+z^2=4.01', 200),
  implicit('ジャイロイド', GYROID),
  implicit('ジャイロイド', GYROID, 200),
  implicit('z (x^2+y^2) = 1', 'z\\left(x^2+y^2\\right)=1'),
  implicit('abs(gamma(x + i y)) = z', '\\left|\\Gamma\\left(x+iy\\right)\\right|=z'),
  implicit('tan(x) = z', '\\tan x=z'),
  implicit('細いトーラス 0.04', '\\left(\\sqrt{x^2+y^2}-3\\right)^2+z^2=0.04', 200),
  implicit('細いトーラス 0.0025', '\\left(\\sqrt{x^2+y^2}-3\\right)^2+z^2=0.0025'),
  ...[0.1, 0.2, 0.3].map((r) =>
    implicit(`小さい球 ${r}`, `\\left(x-0.12\\right)^2+\\left(y-0.12\\right)^2+\\left(z-0.12\\right)^2=${(r * r).toFixed(2)}`, 80, 10),
  ),
  torus4(150),
  torus4(300),
  {
    label: '媒介変数の立体 (u cos v, u sin v, t)（150）',
    items: [{ latex: '\\left(u\\cos v,u\\sin v,t\\right)', options: { opacity: 0.6, ranges: { u: ['0', '2'], v: TWO_PI, t: ['0', '2'] } } }],
    dims: 3,
    half: 3,
    res: 150,
  },
  {
    label: '多変数 4 次元の立方体の面 k=4（150）',
    items: [cube4],
    dims: 4,
    half: 2.5,
    res: 150,
    rotation: { [XW]: 20, [YZ]: 20 },
    perspective: 40,
    play: XW,
  },
]

type FrameStats = ReturnType<Viewer['frameStats']>

/** 1 つの描き方で回したときの結果 */
type StageResult = { name: string } & FrameStats & { D: number | null; Dmax: number | null }

/** 回転の再生中の、射影し直しの往復の時間（ms） */
type PlayResult = {
  /** 届いた返事の数 */
  replies: number
  /** そのうち、点を使い回して射影だけし直したもの / 回転だけで形はそのまま（陰関数）のもの */
  reprojected: number
  transformOnly: number
  /** 往復（計算担当の total + 受け渡し + 登録）の中央値・95% 点 */
  roundMedian: number | null
  roundP95: number | null
  /** 計算担当の中（total）・受け渡し・登録それぞれの中央値 */
  totalMedian: number | null
  transferMedian: number | null
  setPlotMedian: number | null
  /** 1 秒あたりの返事の数 */
  perSecond: number
  /** 再生中のコマの時間 */
  frames: FrameStats
  triangles: number
}

type CaseResult = {
  label: string
  /** 止まった状態で最後に描いた計算の内訳（式ごと） */
  breakdown: (PlotBreakdown & { sumRatio: number })[]
  stages: StageResult[]
  play: PlayResult | null
}

const frame = () => new Promise<number>((r) => requestAnimationFrame(r))
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
const quantile = (xs: number[], p: number) => {
  if (xs.length === 0) return null
  const s = [...xs].sort((a, b) => a - b)
  return s[Math.min(s.length - 1, Math.floor(p * s.length))]
}

/** 2 枚の絵で、色の差（RGB のどれか）が 24/255 を超える画素の割合 */
function pictureDiff(a: Uint8ClampedArray, b: Uint8ClampedArray): number {
  let n = 0
  const total = a.length / 4
  for (let k = 0; k < a.length; k += 4) {
    if (Math.abs(a[k] - b[k]) > 24 || Math.abs(a[k + 1] - b[k + 1]) > 24 || Math.abs(a[k + 2] - b[k + 2]) > 24) n++
  }
  return n / total
}

/** 6 通りの描き方。null は「そのままの版・基準の画素密度」 */
function stages(base: number): { name: string; stage: Stage }[] {
  const list: { name: string; stage: Stage }[] = []
  for (const lod of [false, true]) {
    for (const [rname, ratio] of [
      [`基準 ${base}`, base],
      ['1', 1],
      ['0.5', 0.5],
    ] as const) {
      list.push({ name: `${lod ? '粗い版' : 'そのまま'}・${rname}`, stage: { lod, ratio } })
    }
  }
  return list
}

async function measureStage(viewer: Viewer, stage: Stage, frames: number): Promise<FrameStats> {
  // 描き方を変えた直後の数コマ（画素密度を変えたときの作り直し）は数えない
  viewer.benchMeasure(stage)
  for (let k = 0; k < 6; k++) {
    viewer.benchOrbitTo((2 * Math.PI * k) / frames)
    await frame()
  }
  viewer.benchMeasure(stage)
  for (let k = 0; k <= frames; k++) {
    viewer.benchOrbitTo((2 * Math.PI * k) / frames)
    await frame()
  }
  return viewer.frameStats()
}

/** 4 つの向きで、止まった状態の絵と、動かしている間の絵の差を測る */
function measureDiff(viewer: Viewer, stage: Stage, w: number, h: number): { D: number; Dmax: number } {
  const ds: number[] = []
  for (const a of [0.3, 1.9, 3.4, 5.0]) {
    viewer.benchOrbitTo(a)
    const still = viewer.benchCapture(null, w, h)
    const moving = viewer.benchCapture(stage, w, h)
    ds.push(pictureDiff(still, moving))
  }
  return { D: ds.reduce((x, y) => x + y, 0) / ds.length, Dmax: Math.max(...ds) }
}

async function measurePlay(host: BenchHost, k: number, seconds: number): Promise<PlayResult> {
  const got: PlotBreakdown[] = []
  const listen = (_: number, b: PlotBreakdown) => got.push(b)
  host.play(k)
  // 再生を始めた直後（細かさの見積もり・作り直し）は数えない
  await sleep(800)
  breakdownListeners.add(listen)
  host.viewer.benchMeasure(null)
  const t0 = performance.now()
  await sleep(seconds * 1000)
  const elapsed = (performance.now() - t0) / 1000
  breakdownListeners.delete(listen)
  const frames = host.viewer.frameStats()
  host.play(null)
  host.viewer.benchEnd()
  await host.idle()
  const med = (f: (b: PlotBreakdown) => number) => quantile(got.map(f), 0.5)
  const round = got.map((b) => b.total + b.transfer + b.setPlot)
  return {
    replies: got.length,
    reprojected: got.filter((b) => b.reprojected).length,
    transformOnly: got.filter((b) => b.transformOnly).length,
    roundMedian: quantile(round, 0.5),
    roundP95: quantile(round, 0.95),
    totalMedian: med((b) => b.total),
    transferMedian: med((b) => b.transfer),
    setPlotMedian: med((b) => b.setPlot),
    perSecond: got.length / elapsed,
    frames,
    triangles: got.length ? Math.max(...got.map((b) => b.triangles)) : 0,
  }
}

const f1 = (v: number | null) => (v === null ? '—' : v.toFixed(1))
const f2 = (v: number | null) => (v === null ? '—' : v.toFixed(2))
const pct = (v: number | null) => (v === null ? '—' : `${(v * 100).toFixed(2)}%`)

/** 結果を表（Markdown）にする */
export function formatResults(results: CaseResult[], env: Record<string, unknown>): string {
  const out: string[] = []
  out.push(`# ベンチマーク（?bench=1）`, '', '```', JSON.stringify(env), '```', '')
  out.push('## 計算の内訳（止まった状態の最後の計算。ms）', '')
  out.push('| 式 | 種類 | eval | mesh | lod | copy | total | (合計)/total | 受け渡し | 登録 | 三角形 |')
  out.push('|---|---|---|---|---|---|---|---|---|---|---|')
  for (const r of results) {
    for (const b of r.breakdown) {
      out.push(
        `| ${r.label} | ${b.kind} | ${f2(b.eval)} | ${f2(b.mesh)} | ${f2(b.lod)} | ${f2(b.copy)} | ${f2(b.total)} | ${b.sumRatio.toFixed(3)} | ${f2(b.transfer)} | ${f2(b.setPlot)} | ${b.triangles.toLocaleString()} |`,
      )
    }
  }
  out.push('', '## 視点を回している間のコマの時間（ms）と、止まった状態の絵との差 D', '')
  out.push('間隔は requestAnimationFrame の間隔（画面の更新間隔で刻まれる）、render は render の呼び出し、GPU は GPU の時間（測れなければ —）', '')
  out.push('| 式 | 描き方 | 間隔 中央値 | 間隔 95% | render 中央値 | render 95% | GPU 中央値 | GPU 95% | 三角形 | D 平均 | D 最大 |')
  out.push('|---|---|---|---|---|---|---|---|---|---|---|')
  for (const r of results) {
    for (const s of r.stages) {
      out.push(
        `| ${r.label} | ${s.name}${s.lod ? '' : s.name.startsWith('粗い版') ? '（粗い版なし）' : ''} | ${f1(s.median)} | ${f1(s.p95)} | ${f2(s.cpuMedian)} | ${f2(s.cpuP95)} | ${f2(s.gpuMedian)} | ${f2(s.gpuP95)} | ${s.triangles.toLocaleString()} | ${pct(s.D)} | ${pct(s.Dmax)} |`,
      )
    }
  }
  const plays = results.filter((r) => r.play)
  if (plays.length) {
    out.push('', '## 回転の再生中の、射影し直しの往復（ms。S14 をやるかの判断用）', '')
    out.push('往復 = 計算担当の total + 受け渡し + 登録（viewer.setPlot）', '')
    out.push('| 式 | 返事 | 使い回し / 向きだけ | 返事/秒 | 往復 中央値 | 往復 95% | total | 受け渡し | 登録 | コマ間隔 中央値 | コマ間隔 95% | 三角形 |')
    out.push('|---|---|---|---|---|---|---|---|---|---|---|---|')
    for (const r of plays) {
      const p = r.play!
      out.push(
        `| ${r.label} | ${p.replies} | ${p.reprojected} / ${p.transformOnly} | ${p.perSecond.toFixed(1)} | ${f1(p.roundMedian)} | ${f1(p.roundP95)} | ${f1(p.totalMedian)} | ${f1(p.transferMedian)} | ${f1(p.setPlotMedian)} | ${f1(p.frames.median)} | ${f1(p.frames.p95)} | ${p.triangles.toLocaleString()} |`,
      )
    }
  }
  return out.join('\n')
}

/** ベンチマークを回す。進み具合と結果は show に渡す */
export async function runBench(host: BenchHost, show: (text: string) => void): Promise<CaseResult[]> {
  const q = new URLSearchParams(location.search)
  const frames = Math.max(30, Number(q.get('frames')) || 240)
  const only = q.get('only')
  const playSeconds = Math.max(1, Number(q.get('play')) || 4)
  const cases = BENCH_CASES.filter((c) => !only || c.label.includes(only))
  const viewer = host.viewer
  const env = {
    userAgent: navigator.userAgent,
    devicePixelRatio: window.devicePixelRatio,
    baseRatio: viewer.baseRatio,
    width: window.innerWidth,
    height: window.innerHeight,
    frames,
    playSeconds,
  }
  const results: CaseResult[] = []
  const w = Math.max(1, Math.floor(window.innerWidth / 2))
  const h = Math.max(1, Math.floor(window.innerHeight / 2))
  for (const [n, c] of cases.entries()) {
    show(`${n + 1} / ${cases.length}: ${c.label} を計算中…\n\n${formatResults(results, env)}`)
    await host.load(c)
    const breakdown = [...lastBreakdown.values()].map((b) => ({ ...b, sumRatio: (b.eval + b.mesh + b.lod + b.copy) / b.total }))
    const res: CaseResult = { label: c.label, breakdown, stages: [], play: null }
    for (const { name, stage } of stages(viewer.baseRatio)) {
      show(`${n + 1} / ${cases.length}: ${c.label}・${name} を測定中…\n\n${formatResults(results, env)}`)
      const stats = await measureStage(viewer, stage, frames)
      const d = measureDiff(viewer, stage, w, h)
      res.stages.push({ name, ...stats, ...d })
      viewer.benchEnd()
      await frame()
    }
    if (c.play !== undefined) {
      show(`${n + 1} / ${cases.length}: ${c.label}・回転の再生を測定中…\n\n${formatResults(results, env)}`)
      res.play = await measurePlay(host, c.play, playSeconds)
    }
    results.push(res)
  }
  const text = formatResults(results, env)
  show(`終わりました（${cases.length} 個の式）\n\n${text}`)
  console.log(JSON.stringify({ env, results }))
  ;(globalThis as { __benchResults?: unknown }).__benchResults = { env, results, text }
  return results
}

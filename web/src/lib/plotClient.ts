// Web Worker に計算を頼むための窓口。

import type {
  FieldStageResponse,
  CapRequest,
  CapResponse,
  CoordGridRequest,
  CoordGridResponse,
  ComplexMode,
  CutRequest,
  Domain,
  ParamRanges,
  ParamValue,
  PlotRequest,
  PlotResponse,
  Space,
} from './types'
import PlotWorker from './plot.worker.ts?worker'
import { PLOT_LOCAL, PLOT_NO_REFINE } from './types'

/**
 * 計算担当の Worker を CPU のコア数に合わせて最大4つ用意し、式ごとに担当を決めて並列に計算する。
 * 同じ式はいつも同じ Worker が担当する（媒介変数表示の点を Worker の中で覚えて使い回すため）
 */
const WORKERS = Math.max(1, Math.min(4, (navigator.hardwareConcurrency || 2) - 1))
const workers = Array.from({ length: WORKERS }, () => new PlotWorker())
let nextId = 1
const pending = new Map<number, (r: any) => void>()

for (const w of workers) {
  w.onmessage = (ev: MessageEvent<PlotResponse | CapResponse>) => {
    const resolve = pending.get(ev.data.requestId)
    pending.delete(ev.data.requestId)
    resolve?.(ev.data)
  }
}

/**
 * 4次元の遠近法の強さ（0〜100）→ 視点までの距離。強さ 100% で、表示範囲の半分の 0.6 倍の距離
 * （表示範囲 −5〜5 なら 3）。距離が近いほど、奥（w の小さい側）の部分が小さく描かれる
 */
function perspectiveDistance(strength: number, d: Domain): number {
  if (strength <= 0) return 0
  const half = Math.max(d.xmax - d.xmin, d.ymax - d.ymin, d.zmax - d.zmin) / 2
  return (half * 0.6) / (strength / 100)
}

/** 曲面の計算を頼む。結果は Promise で返る。 */
export function requestPlot(
  itemId: number,
  source: string,
  domain: Domain,
  resolution: number,
  implicitResolution: number,
  ranges: ParamRanges,
  params: Record<string, ParamValue>,
  space: Space,
  stacks: Record<string, [number, number]>,
  cut: CutRequest,
  complexMode: ComplexMode = 'off',
  flat = false,
  walls = false,
  layers = 1,
  flatSlice = false,
  flags = 0,
  hidden: { mode: number; count: number } | null = null,
  complexCoords = false,
  coord: PlotRequest['coord'] = null,
): Promise<PlotResponse> {
  const requestId = nextId++
  const req: PlotRequest = {
    requestId,
    itemId,
    source,
    domain: { ...domain },
    resolution,
    implicitResolution,
    // Svelte の $state（Proxy）は Worker に送れないので、ふつうの配列にコピーする
    ranges: Object.fromEntries(Object.entries(ranges).map(([k, [a, b]]) => [k, [a, b]])),
    params: Object.fromEntries(Object.entries(params).map(([k, p]) => [k, { re: p.re, im: p.im, complex: p.complex }])),
    space: {
      dims: space.dims,
      axes: [...space.axes],
      slice: [...space.slice],
      ranges: space.ranges.map((r) => (r ? [r[0], r[1]] : null)),
      samples: space.samples,
      rotation: space.rotation.map((deg) => (deg * Math.PI) / 180),
      center: [...space.center],
      perspective: perspectiveDistance(space.perspective, domain),
    },
    stacks: Object.fromEntries(Object.entries(stacks).map(([k, [a, b]]) => [k, [a, b]])),
    cut: cut && { ...cut },
    complexMode,
    flat,
    walls,
    layers,
    flatSlice,
    flags,
    hidden: hidden && { ...hidden },
    complexCoords,
    coord: coord && { map: coord.map, ranges: coord.ranges.map(([a, b]) => [a, b] as [string, string]), names: coord.names },
  }
  return planned(req)
}

// ---- 重い陰関数の F の計算を、全部の Worker に分ける ----
// 前回の計算で、式の計算（F）に 30 ms 以上かかった陰関数・範囲で重ねた立体は、
// 担当の Worker に計画（F を計算する点の「かたまり」の数）を頼み、かたまりを Worker の数で等分して全部の Worker に計算を頼む。
// 返ってきた値をつなげて担当の Worker に渡し、メッシュを作ってもらう（メッシュを作るのは担当だけ。点・ふたの使い回しも担当だけ）。
// かたまりの切れ目はいつも同じなので、分けても 1 回で計算したときと同じ形になる。
// 手伝いの依頼は、各 Worker の自分の依頼の後ろに並ぶ（Worker は届いた順に 1 つずつ処理する）

/** 式ごとの、前回の F の計算の時間（分けたときは、各 Worker の時間の合計）。30 ms 以上なら分ける */
const fieldCost = new Map<number, number>()
/** 式ごとの、いちばん新しい依頼の番号（古い依頼の途中の段階は、続けずにやめる） */
const latestOf = new Map<number, number>()
const SPLIT_EVAL_MS = 30

function send<T>(worker: number, req: PlotRequest): Promise<T> {
  return new Promise((resolve) => {
    pending.set(req.requestId, resolve)
    workers[worker].postMessage(req, req.field?.mode === 'given' ? [req.field.values.buffer as ArrayBuffer] : [])
  })
}

/** 式ごとの、前の依頼の「回転と切り口以外」の入力（回転だけが変わったかを見る） */
const lastInputs = new Map<number, string>()

/**
 * 回転だけが変わった、細かく作る依頼（前の依頼と、回転・切り口以外が同じ）なら、印（PLOT_LOCAL）を付ける。
 * 計算担当は、覚えている回す前の形（陰関数）があれば作り直さず、無ければ面を返した後すぐに作る。
 * 印の無い細かく作る依頼（式・スライダーが変わった）でも、計算担当は少し待ってから回す前の形を作る（初めて回すときから、回すだけで描く）。
 * F の計算を分けるときは、全部の Worker が同じ印で計算する必要があるので、ここで決める
 */
function markRotationOnly(req: PlotRequest) {
  const { requestId: _r, cut: _c, field: _f, space, ...rest } = req
  const { rotation: _rot, ...spaceRest } = space
  // 細かさ（陽関数・陰関数とも）・細かくしない印は、使い回す側（Worker）が見るので比べない（再生中・2 段階の表示で細かさが変わっても、
  // 回転だけが変わったとみなす）
  const inputs = JSON.stringify({ ...rest, resolution: 0, implicitResolution: 0, flags: rest.flags & ~PLOT_NO_REFINE, space: spaceRest })
  // 再生・ドラッグ中（PLOT_NO_REFINE）は作らない。回す前の形は表示範囲より広く細かく作るので、再生中の軽い計算の 8 倍ほどかかり、
  // 回し始めの 1 コマがカクッと止まる。回し始めは今までどおり軽く作り直し、止まったとき（細かく作る依頼）に作っておけば、
  // 次に回すときは Worker が覚えた形を回すだけで描ける（細かさが違っても、再生中なら使い回す）
  const moving = (req.flags & PLOT_NO_REFINE) !== 0
  // 回転が 0 に戻ったときも同じ（回す前の形は回転によらないので、覚えている形を使い続ける）
  if (lastInputs.get(req.itemId) === inputs && !moving) req.flags |= PLOT_LOCAL
  lastInputs.set(req.itemId, inputs)
}

async function planned(req: PlotRequest): Promise<PlotResponse> {
  markRotationOnly(req)
  const { itemId } = req
  const owner = itemId % WORKERS
  latestOf.set(itemId, req.requestId)
  const split = WORKERS >= 2 && !req.flat && (fieldCost.get(itemId) ?? 0) >= SPLIT_EVAL_MS
  if (split) {
    const t0 = performance.now()
    const res = await splitPlot(req, owner)
    if (res) {
      if (res.ok) res.ms = performance.now() - t0 // 待った時間（細かさの見積もりに使う）
      return res
    }
    // 分けられない式だった・途中で失敗した: ふつうに計算する
    fieldCost.delete(itemId)
  }
  const res = await send<PlotResponse>(owner, req)
  if (res.ok) {
    if (res.cost === 'volume' && !res.reprojected) fieldCost.set(itemId, res.timing.eval)
    else fieldCost.delete(itemId)
  }
  return res
}

/** F の計算を分けて計算する。分けられなければ null */
async function splitPlot(req: PlotRequest, owner: number): Promise<PlotResponse | null> {
  const stage = (field: NonNullable<PlotRequest['field']>, w: number) =>
    send<FieldStageResponse>(w, { ...req, requestId: nextId++, field })
  const superseded = (): PlotResponse => ({ requestId: req.requestId, ok: false, error: '新しい依頼があるので中止しました' })
  const plan = await stage({ mode: 'plan' }, owner)
  if (!plan.ok || plan.chunks < 2) return null
  if (latestOf.get(req.itemId) !== req.requestId) return superseded()
  // かたまりを Worker の数で等分する（担当の Worker も手伝う）
  const c = plan.chunks
  const parts = Array.from({ length: Math.min(WORKERS, c) }, (_, k) => {
    const from = Math.floor((c * k) / Math.min(WORKERS, c))
    const to = Math.floor((c * (k + 1)) / Math.min(WORKERS, c))
    return stage({ mode: 'chunks', from, to }, (owner + k) % WORKERS)
  })
  const got = await Promise.all(parts)
  if (got.some((g) => !g.ok || !g.values)) return null
  if (latestOf.get(req.itemId) !== req.requestId) return superseded()
  const vals = got.map((g) => (g.ok && g.values) || new Float32Array())
  const values = new Float32Array(vals.reduce((n, v) => n + v.length, 0))
  let at = 0
  for (const v of vals) {
    values.set(v, at)
    at += v.length
  }
  const evalMs = got.reduce((t, g) => t + (g.ok ? g.ms : 0), 0)
  const res = await send<PlotResponse>(owner, { ...req, field: { mode: 'given', values } })
  if (res.ok) {
    fieldCost.set(req.itemId, evalMs)
    res.timing = { ...res.timing, eval: res.timing.eval + evalMs }
  }
  return res
}

/**
 * 切り口の位置だけが変わったときに、陰関数・範囲で重ねた立体の切り口のふただけを作り直す。
 * その式をまだ計算していなければ null が返る
 */
/** 座標系の格子（計画 S39）を頼む。どの式の担当でもないので、1 つ目の Worker に頼む */
export function requestCoordGrid(req: Omit<CoordGridRequest, 'type' | 'requestId'>): Promise<CoordGridResponse> {
  const requestId = nextId++
  const full: CoordGridRequest = { ...req, type: 'grid', requestId }
  return new Promise((resolve) => {
    pending.set(requestId, resolve)
    workers[0].postMessage(full)
  })
}

export function requestCap(itemId: number, cut: CutRequest, resolution: number): Promise<CapResponse> {
  const requestId = nextId++
  const req: CapRequest = { type: 'cap', requestId, itemId, cut: cut && { ...cut }, resolution }
  return new Promise((resolve) => {
    pending.set(requestId, resolve)
    workers[itemId % WORKERS].postMessage(req)
  })
}

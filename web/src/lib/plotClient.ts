// Web Worker に計算を頼むための窓口。

import type {
  CapRequest,
  CapResponse,
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
    params: Object.fromEntries(Object.entries(params).map(([k, p]) => [k, { re: p.re, im: p.im }])),
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
  }
  return new Promise((resolve) => {
    pending.set(requestId, resolve)
    workers[itemId % WORKERS].postMessage(req)
  })
}

/**
 * 切り口の位置だけが変わったときに、陰関数・範囲で重ねた立体の切り口のふただけを作り直す。
 * その式をまだ計算していなければ null が返る
 */
export function requestCap(itemId: number, cut: CutRequest, resolution: number): Promise<CapResponse> {
  const requestId = nextId++
  const req: CapRequest = { type: 'cap', requestId, itemId, cut: cut && { ...cut }, resolution }
  return new Promise((resolve) => {
    pending.set(requestId, resolve)
    workers[itemId % WORKERS].postMessage(req)
  })
}

// Web Worker に計算を頼むための窓口。

import type { Domain, ParamRanges, ParamValue, PlotRequest, PlotResponse } from './types'
import PlotWorker from './plot.worker.ts?worker'

const worker = new PlotWorker()
let nextId = 1
const pending = new Map<number, (r: PlotResponse) => void>()

worker.onmessage = (ev: MessageEvent<PlotResponse>) => {
  const resolve = pending.get(ev.data.requestId)
  pending.delete(ev.data.requestId)
  resolve?.(ev.data)
}

/** 曲面の計算を頼む。結果は Promise で返る。 */
export function requestPlot(
  source: string,
  domain: Domain,
  resolution: number,
  implicitResolution: number,
  ranges: ParamRanges,
  params: Record<string, ParamValue>,
): Promise<PlotResponse> {
  const requestId = nextId++
  const req: PlotRequest = {
    requestId,
    source,
    domain: { ...domain },
    resolution,
    implicitResolution,
    // Svelte の $state（Proxy）は Worker に送れないので、ふつうの配列にコピーする
    ranges: { t: [...ranges.t], u: [...ranges.u], v: [...ranges.v] },
    params: Object.fromEntries(Object.entries(params).map(([k, p]) => [k, { re: p.re, im: p.im }])),
  }
  return new Promise((resolve) => {
    pending.set(requestId, resolve)
    worker.postMessage(req)
  })
}

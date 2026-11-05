// Web Worker に計算を頼むための窓口。

import type { Domain, PlotRequest, PlotResponse } from './types'
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
): Promise<PlotResponse> {
  const requestId = nextId++
  const req: PlotRequest = { requestId, source, domain: { ...domain }, resolution, implicitResolution }
  return new Promise((resolve) => {
    pending.set(requestId, resolve)
    worker.postMessage(req)
  })
}

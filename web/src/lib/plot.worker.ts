// 計算担当の Web Worker。
// 重い計算を画面の処理とは別のスレッドで行うので、計算中も画面が固まらない。

import init, { plot } from '../wasm-core/graph_core.js'
import type { PlotRequest, PlotResponse } from './types'

// WASM の読み込み（最初の1回だけ）
const ready = init()

self.onmessage = async (ev: MessageEvent<PlotRequest>) => {
  const { requestId, source, domain: d, resolution, implicitResolution } = ev.data
  await ready

  let res: PlotResponse
  const t0 = performance.now()
  try {
    const r = plot(source, d.xmin, d.xmax, d.ymin, d.ymax, d.zmin, d.zmax, resolution, implicitResolution)
    const mesh = { positions: r.positions, normals: r.normals, indices: r.indices }
    const kind = r.kind
    r.free() // Rust 側のメモリを解放
    res = { requestId, ok: true, mesh, kind, ms: performance.now() - t0 }
  } catch (e) {
    res = { requestId, ok: false, error: e instanceof Error ? e.message : String(e) }
  }

  // 配列はコピーせずに「移動」して渡す（速い）
  const transfer = res.ok
    ? [res.mesh.positions.buffer, res.mesh.normals.buffer, res.mesh.indices.buffer]
    : []
  self.postMessage(res, { transfer })
}

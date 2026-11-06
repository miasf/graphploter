// 計算担当の Web Worker。
// 重い計算を画面の処理とは別のスレッドで行うので、計算中も画面が固まらない。

import init, { eval_constant, plot } from '../wasm-core/graph_core.js'
import type { PlotRequest, PlotResponse } from './types'

// WASM の読み込み（最初の1回だけ）
const ready = init()

/** 範囲の文字列（"2pi" など）を数値にする。誤りがあれば説明付きで投げる */
function range(name: string, [lo, hi]: [string, string]): [number, number] {
  try {
    const a = eval_constant(lo)
    const b = eval_constant(hi)
    if (!(a < b)) throw new Error('左の値を右の値より小さくしてください')
    return [a, b]
  } catch (e) {
    throw new Error(`${name} の範囲: ${e instanceof Error ? e.message : e}`)
  }
}

self.onmessage = async (ev: MessageEvent<PlotRequest>) => {
  const { requestId, source, domain: d, resolution, implicitResolution, ranges, params, space } = ev.data
  await ready

  let res: PlotResponse
  const t0 = performance.now()
  try {
    const rangeNames = Object.keys(ranges)
    const rangeValues = rangeNames.map((n) => range(n, ranges[n]))
    const r = plot(
      source,
      new Float64Array([d.xmin, d.xmax, d.ymin, d.ymax, d.zmin, d.zmax]),
      resolution,
      implicitResolution,
      rangeNames.join(','),
      new Float64Array(rangeValues.flat()),
      Object.keys(params).join(','),
      new Float64Array(Object.values(params).flatMap((p) => [p.re, p.im])),
      space.dims,
      new Uint32Array(space.axes),
      new Float64Array(space.slice),
    )
    const vars = r.params.split(',').filter((s) => s !== '')
    // 曲面の格子線を引くための、媒介変数の範囲の長さ
    const span = (k: number) => {
      const v = vars[k]
      if (v === undefined) return 2 * Math.PI
      const rg = v in ranges ? rangeValues[rangeNames.indexOf(v)] : [0, 2 * Math.PI]
      return rg[1] - rg[0]
    }
    const mesh = {
      positions: r.positions,
      normals: r.normals,
      indices: r.indices,
      gridCoords: r.grid_coords,
      curve: r.curve,
      params: vars,
      paramSpan: { u: span(0), v: span(1) },
    }
    const kind = r.kind
    const usedParams = r.used_params.split(',').filter((s) => s !== '')
    const pv = r.param_value
    const param = r.param_name ? { name: r.param_name, re: pv[0], im: pv[1] } : null
    r.free() // Rust 側のメモリを解放
    res = { requestId, ok: true, mesh, kind, ms: performance.now() - t0, usedParams, param }
  } catch (e) {
    res = { requestId, ok: false, error: e instanceof Error ? e.message : String(e) }
  }

  // 配列はコピーせずに「移動」して渡す（速い）
  const transfer = res.ok
    ? [res.mesh.positions.buffer, res.mesh.normals.buffer, res.mesh.indices.buffer, res.mesh.gridCoords.buffer]
    : []
  self.postMessage(res, { transfer })
}

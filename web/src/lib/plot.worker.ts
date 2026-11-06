// 計算担当の Web Worker。
// 重い計算を画面の処理とは別のスレッドで行うので、計算中も画面が固まらない。

import init, { eval_range, last_used_params, plot, type MeshResult, type Shape } from '../wasm-core/graph_core.js'
import type { PlotRequest, PlotResponse } from './types'

// WASM の読み込み（最初の1回だけ）
const ready = init()

/**
 * 範囲の文字列（"2pi" や、パラメータを使った "b" など）を数値にする。誤りがあれば説明付きで投げる。
 * 使ったパラメータの名前は `used` に足す（パラメータが変わったら計算し直すため）
 */
function range(
  name: string,
  [lo, hi]: [string, string],
  params: PlotRequest['params'],
  used: Set<string>,
): [number, number] {
  const names = Object.keys(params).join(',')
  const values = new Float64Array(Object.values(params).flatMap((p) => [p.re, p.im]))
  const value = (src: string) => {
    const v = eval_range(src, names, values)
    for (const n of last_used_params().split(',')) if (n) used.add(n)
    return v
  }
  try {
    const a = value(lo)
    const b = value(hi)
    if (!(a < b)) throw new Error('左の値を右の値より小さくしてください')
    return [a, b]
  } catch (e) {
    throw new Error(`${name} の範囲: ${e instanceof Error ? e.message : e}`)
  }
}

/**
 * 媒介変数表示の、回転・射影する前の形（n 次元の点）を式ごとに覚えておく。
 * 式・範囲・パラメータなど「点の計算に関わる入力」が同じなら、回転・表示する軸が変わっても
 * 式を計算し直さず、射影し直すだけで済む（回転のスライダーが軽く動く）。
 */
const shapes = new Map<number, { key: string; shape: Shape }>()

/**
 * 点の計算に関わる入力（回転は含めない）。
 * 回転した陽関数から作った点（view が true）は、表示する軸・断面の値・回転の中心にもよる
 */
function shapeKey(req: PlotRequest, view: boolean): string {
  const { source, domain, resolution, ranges, params, stacks, space } = req
  const base = [source, domain, resolution, ranges, params, stacks, space.dims]
  return JSON.stringify(view ? [...base, space.axes, space.slice, space.center] : base)
}

function forget(itemId: number) {
  shapes.get(itemId)?.shape.free()
  shapes.delete(itemId)
}

self.onmessage = async (ev: MessageEvent<PlotRequest>) => {
  const req = ev.data
  const { requestId, itemId, source, domain: d, resolution, implicitResolution, ranges, params, space, stacks } = req
  await ready

  let res: PlotResponse
  const t0 = performance.now()
  const domainArr = new Float64Array([d.xmin, d.xmax, d.ymin, d.ymax, d.zmin, d.zmax])
  try {
    const rangeNames = Object.keys(ranges)
    const rangeUsed = new Set<string>()
    const rangeValues = rangeNames.map((n) => range(n, ranges[n], params, rangeUsed))
    const cached = shapes.get(itemId)
    let r: MeshResult | null = null
    if (cached && cached.key === shapeKey(req, cached.shape.view_dependent)) {
      // 点は計算済み: 回転・射影だけし直す
      try {
        r = cached.shape.project(
          domainArr,
          space.dims,
          new Uint32Array(space.axes),
          new Float64Array(space.rotation),
          new Float64Array(space.center),
          space.perspective,
        )
      } catch {
        r = null // 回転した陽関数で、回転が条件を外れた（陰関数として計算し直す）
      }
    }
    if (!r) {
      r = plot(
        source,
        domainArr,
        resolution,
        implicitResolution,
        rangeNames.join(','),
        new Float64Array(rangeValues.flat()),
        Object.keys(params).join(','),
        new Float64Array(Object.values(params).flatMap((p) => [p.re, p.im])),
        space.dims,
        new Uint32Array(space.axes),
        new Float64Array(space.slice),
        new Float64Array(space.ranges),
        Object.keys(stacks).join(','),
        new Float64Array(Object.values(stacks).flat()),
        space.samples,
        new Float64Array(space.rotation),
        new Float64Array(space.center),
        space.perspective,
      )
      forget(itemId)
      const shape = r.take_shape()
      if (shape) shapes.set(itemId, { key: shapeKey(req, shape.view_dependent), shape })
    }
    const vars = r.params.split(',').filter((s) => s !== '')
    // 曲面の格子線を引くための、媒介変数の範囲の長さ
    const span = (k: number) => {
      const v = vars[k]
      if (v === undefined) return 2 * Math.PI
      const rg = v in ranges ? rangeValues[rangeNames.indexOf(v)] : (stacks[v] ?? [0, 2 * Math.PI])
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
    const usedParams = [...new Set([...r.used_params.split(',').filter((s) => s !== ''), ...rangeUsed])]
    const pv = r.param_value
    const param = r.param_name ? { name: r.param_name, re: pv[0], im: pv[1] } : null
    r.free() // Rust 側のメモリを解放
    res = { requestId, ok: true, mesh, kind, ms: performance.now() - t0, usedParams, param }
  } catch (e) {
    forget(itemId)
    res = { requestId, ok: false, error: e instanceof Error ? e.message : String(e) }
  }

  // 配列はコピーせずに「移動」して渡す（速い）
  const transfer = res.ok
    ? [res.mesh.positions.buffer, res.mesh.normals.buffer, res.mesh.indices.buffer, res.mesh.gridCoords.buffer]
    : []
  self.postMessage(res, { transfer })
}

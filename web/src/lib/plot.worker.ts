// 計算担当の Web Worker。
// 重い計算を画面の処理とは別のスレッドで行うので、計算中も画面が固まらない。

import init, {
  eval_range,
  last_used_params,
  plot,
  type CapSource,
  type MeshResult,
  type Shape,
} from '../wasm-core/graph_core.js'
import { coordName, isReserved, prettyName } from './params'
import type { CapData, CapRequest, CapResponse, CutRequest, PlotRequest, PlotResponse } from './types'

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
  /** 最小 > 最大でもよい（呼び出し側で入れ替える） */
  anyOrder = false,
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
    if (!Number.isFinite(a) || !Number.isFinite(b)) throw new Error('値が有限ではありません')
    // 両端が同じ値はよい（長さ 0: 何も描かない）。0 ≤ u ≤ c の c を再生すると、最小値の 0 に戻るたびにこうなる
    if (!anyOrder && !(a <= b)) throw new Error('左の値を右の値以下にしてください')
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
function shapeKey(req: PlotRequest, view: boolean, slice: number[], coordRanges: number[]): string {
  const { source, domain, resolution, ranges, params, stacks, space } = req
  const base = [source, domain, resolution, ranges, params, stacks, space.dims, req.complexMode, req.flat, req.flatSlice, req.walls, req.layers]
  return JSON.stringify(view ? [...base, space.axes, slice, coordRanges, space.center] : base)
}

/**
 * 座標の「範囲で重ねる」範囲の式（0 〜 a など）を数値にする。
 * 返すのは計算エンジンに渡す [x の最小, x の最大, y の最小, …]（重ねない座標は NaN）と、断面の値。
 * 最小と最大が逆なら入れ替え、同じ値なら、その値での断面（固定）にする
 */
function coordRanges(
  space: PlotRequest['space'],
  params: PlotRequest['params'],
  used: Set<string>,
): { ranges: number[]; slice: number[] } {
  const slice = [...space.slice]
  const ranges: number[] = []
  space.ranges.forEach((r, k) => {
    if (!r || space.axes.includes(k)) {
      ranges.push(NaN, NaN)
      return
    }
    const [a, b] = range(prettyName(coordName(k)), r, params, used, true)
    if (a === b) {
      slice[k] = a
      ranges.push(NaN, NaN)
    } else ranges.push(Math.min(a, b), Math.max(a, b))
  })
  return { ranges, slice }
}

/** パラメータの定義（a = 1 など）かもしれない式か。左辺が座標でない名前1つ */
function looksLikeDefinition(source: string): boolean {
  const m = source.match(/^\s*([A-Za-z]\w*)\s*=/)
  return !!m && !isReserved(m[1])
}

function forget(itemId: number) {
  shapes.get(itemId)?.shape.free()
  shapes.delete(itemId)
  fields.get(itemId)?.source.free()
  fields.delete(itemId)
}

/** 陰関数・範囲で重ねた立体の中身（切り口の位置だけが変わったとき、ふただけを作り直す）と、ふたの細かさ */
const fields = new Map<number, { source: CapSource; resolution: number }>()

function makeCap(source: CapSource, cut: CutRequest, resolution: number): CapData | null {
  if (!cut) return null
  const m = source.cap(cut.axis, cut.pos, cut.keepBelow, resolution)
  const cap = { positions: m.positions, normals: m.normals, indices: m.indices }
  m.free()
  return cap
}

function capTransfer(cap: CapData | null): ArrayBuffer[] {
  return cap ? [cap.positions.buffer, cap.normals.buffer, cap.indices.buffer] as ArrayBuffer[] : []
}

self.onmessage = async (ev: MessageEvent<PlotRequest | CapRequest>) => {
  if ('type' in ev.data) {
    const { requestId, itemId, cut, resolution } = ev.data
    await ready
    const f = fields.get(itemId)
    const cap = f ? makeCap(f.source, cut, resolution) : null
    const res: CapResponse = { requestId, cap }
    self.postMessage(res, { transfer: capTransfer(cap) })
    return
  }
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
    // 座標の範囲（0 〜 a など）が計算できなくても、パラメータの定義（a = 1）は計算する
    // （そうしないと、範囲で使う a を定義する行まで同じエラーになり、a がいつまでも定義されない）
    let coord: { ranges: number[]; slice: number[] }
    let coordError: unknown = null
    try {
      coord = coordRanges(space, params, rangeUsed)
    } catch (e) {
      if (!looksLikeDefinition(source)) throw e
      coordError = e
      coord = { ranges: space.ranges.flatMap(() => [NaN, NaN]), slice: [...space.slice] }
    }
    const key = (view: boolean) => shapeKey(req, view, coord.slice, coord.ranges)
    const cached = shapes.get(itemId)
    let r: MeshResult | null = null
    if (cached && cached.key === key(cached.shape.view_dependent)) {
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
        new Float64Array(coord.slice),
        new Float64Array(coord.ranges),
        Object.keys(stacks).join(','),
        new Float64Array(Object.values(stacks).flat()),
        space.samples,
        new Float64Array(space.rotation),
        new Float64Array(space.center),
        space.perspective,
        ['off', 'abs', 're', 'im'].indexOf(req.complexMode),
        req.walls,
        req.flat,
        req.layers,
        req.flatSlice,
      )
      if (coordError && !r.param_name) {
        r.free()
        throw coordError // パラメータの定義ではなかった
      }
      forget(itemId)
      const shape = r.take_shape()
      if (shape) shapes.set(itemId, { key: key(shape.view_dependent), shape })
      const capSource = r.take_field()
      if (capSource) fields.set(itemId, { source: capSource, resolution: implicitResolution * 2 })
    }
    const f = fields.get(itemId)
    const cap = f ? makeCap(f.source, req.cut, f.resolution) : null
    const vars = r.params.split(',').filter((s) => s !== '')
    // 曲面の格子線を引くための、媒介変数の範囲の長さ
    const span = (k: number) => {
      const v = vars[k]
      if (v === undefined) return 2 * Math.PI
      const rg = stacks[v] ?? (v in ranges ? rangeValues[rangeNames.indexOf(v)] : [0, 2 * Math.PI])
      return rg[1] - rg[0]
    }
    const mesh = {
      positions: r.positions,
      normals: r.normals,
      indices: r.indices,
      lodIndices: r.lod_indices,
      gridCoords: r.grid_coords,
      complexValues: r.complex_values,
      curve: r.curve,
      solid: r.solid,
      heat: r.kind.includes('の値で塗り分け'),
      params: vars,
      // 媒介変数が4つ以上の図形は、格子線用の値を面ごとに 0〜1 にそろえてある
      paramSpan:
        vars.length >= 4
          ? { u: 1, v: 1, w: 0 }
          : vars.length === 3
            ? { u: span(0), v: span(1), w: span(2) }
            : { u: span(0), v: span(1) },
    }
    const kind = r.kind
    const usedParams = [...new Set([...r.used_params.split(',').filter((s) => s !== ''), ...rangeUsed])]
    const pv = r.param_value
    const param = r.param_name ? { name: r.param_name, re: pv[0], im: pv[1] } : null
    r.free() // Rust 側のメモリを解放
    res = { requestId, ok: true, mesh, kind, ms: performance.now() - t0, usedParams, param, cap }
  } catch (e) {
    forget(itemId)
    res = { requestId, ok: false, error: e instanceof Error ? e.message : String(e) }
  }

  // 配列はコピーせずに「移動」して渡す（速い）
  const transfer = res.ok
    ? [
        res.mesh.positions.buffer,
        res.mesh.normals.buffer,
        res.mesh.indices.buffer,
        res.mesh.lodIndices.buffer,
        res.mesh.gridCoords.buffer,
        res.mesh.complexValues.buffer,
        ...capTransfer(res.cap),
      ]
    : []
  self.postMessage(res, { transfer })
}

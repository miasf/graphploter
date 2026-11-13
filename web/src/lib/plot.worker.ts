// 計算担当の Web Worker。
// 重い計算を画面の処理とは別のスレッドで行うので、計算中も画面が固まらない。

import init, {
  eval_range,
  field_chunks,
  field_mode_chunks,
  field_mode_given,
  field_mode_plan,
  field_mode_reset,
  last_used_params,
  plot,
  take_field_values,
  type CapSource,
  type MeshResult,
  type Shape,
} from '../wasm-core/graph_core.js'
import { coordName, isReserved, prettyName } from './params'
import type { Cost } from './budget'
import type { CapData, CapRequest, CapResponse, CutRequest, FieldStageResponse, PlotRequest, PlotResponse, PlotTiming } from './types'
import { COMPLEX_MODE_ORDER, PLOT_EDGES, PLOT_LOCAL, PLOT_NO_REFINE, PLOT_THOROUGH } from './types'

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
const shapes = new Map<
  number,
  {
    key: string
    shape: Shape
    refined: boolean
    ires: number
    thorough: boolean
    /** 回す前の形で作った陰関数（rigid）で、そのメッシュを画面側に送ってあるか（送ってあれば、回転が変わっても 1 次式だけ送る） */
    localShown: boolean
  }
>()

/**
 * 表示する面を返した後に作る、回す前の形（陰関数）の予約。式ごとに、覚える形の鍵と、作る関数。
 * 止まったとき（細かく作る依頼）は、表示する面を先に返して、回す前の形はその後で作る
 * （回す前の形は表示範囲より広く作るので、表示する面の 0.5〜2 倍かかる。止まってから面が出るまでを今までどおりにする）。
 * 回転だけが変わった依頼ならすぐに、式・スライダーが変わった依頼なら少し待ってから作る（打っている途中・動かしている途中に作らない）。
 * 作る前に同じ式の新しい依頼が来たら、作らない（その依頼が今の形を決める）。ただし、その依頼が同じ形を回し始める依頼（再生・ドラッグ中）なら、
 * 先に作る（回し始めから、回す前の形を回すだけで描く。粗く作り直すと細い所が欠ける）
 */
const localJobs = new Map<number, { key: string; run: () => void }>()

/** 回転以外が変わった依頼のあと、回す前の形を作り始めるまでの待ち（ms）。この間に次の依頼が来たら作らない */
const LOCAL_IDLE_MS = 400

/**
 * 点の計算に関わる入力（回転は含めない）。
 * 回転した陽関数から作った点（view が true）は、表示する軸・断面の値・回転の中心にもよる
 */
function shapeKey(req: PlotRequest, view: boolean, slice: number[], coordRanges: number[], rigid = false): string {
  const { source, domain, ranges, params, stacks, space } = req
  // 回す前の形で作った陰関数（rigid）は、陽関数・媒介変数表示の細かさによらない（再生中は細かさが下がるので、入れると使い回せない）
  const resolution = rigid ? 0 : req.resolution
  const base = [source, domain, resolution, ranges, params, stacks, space.dims, req.complexMode, req.flat, req.flatSlice, req.walls, req.layers, req.flags & PLOT_EDGES]
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

/**
 * 計算時間が細かさの何乗に比例するか: 陰関数・範囲で重ねた立体は3乗、曲線（細かさによらず曲がり方で点を足す）はほぼ無関係、
 * ほか（陽関数・媒介変数表示の曲面、2D 表示の陰関数の曲線・切り口の線）は2乗。
 * w などが絡む回転をした陽関数は、傾いた座標系の陽関数（説明が「z = f(x, y)（回したもの）」）なら曲面なので2乗、
 * 陰関数として計算し直したもの（「陰関数（… を回したもの）」）なら3乗
 */
function costOf(kind: string, curve: boolean, flat: boolean): Cost {
  if (!flat && !curve && /^(陰関数|範囲で重ねた立体)/.test(kind)) return 'volume'
  if (curve && !/陰関数|切り口/.test(kind)) return 'curve'
  return 'surface'
}

function forget(itemId: number) {
  shapes.get(itemId)?.shape.free()
  shapes.delete(itemId)
  fields.get(itemId)?.source.free()
  fields.delete(itemId)
}

/** 陰関数・範囲で重ねた立体の中身（切り口の位置だけが変わったとき、ふただけを作り直す）と、ふたの細かさ */
const fields = new Map<number, { source: CapSource; resolution: number }>()

/** 時間の内訳を数える入れ物（`PlotTiming` の eval, mesh, lod, copy） */
type Tally = { eval: number; mesh: number; lod: number; copy: number }

/**
 * 計算エンジンの内訳 [式の計算, メッシュ, 粗い版, 点の数] を足す。`wall` は計算エンジンを呼んだ時間全体で、
 * 計算エンジンの中で測れない分（引数・戻り値の受け渡し）はメッシュの時間に入れる
 */
function addStats(t: Tally, r: MeshResult, wall: number) {
  const [e, , l] = r.stats
  t.eval += e
  t.mesh += Math.max(0, wall - e - l)
  t.lod += l
}

/**
 * 切り口のふたと、範囲で重ねた立体の表示範囲の端のふた（切り口が無くても作る。細かさは切り口のふたの半分）。
 * どちらも無ければ null
 */
function makeCap(source: CapSource, cut: CutRequest, resolution: number, t?: Tally): CapData | null {
  const w0 = performance.now()
  const boxN = Math.max(8, Math.round(resolution / 2))
  const m = cut
    ? source.caps(cut.axis, cut.pos, cut.keepBelow, resolution, boxN)
    : source.caps(-1, 0, false, resolution, boxN)
  const c0 = performance.now()
  // 配列は取り出して渡す（コピーは1回）
  const cap = { positions: m.take_positions(), normals: m.take_normals(), indices: m.take_indices() }
  if (!cut && cap.indices.length === 0) {
    m.free()
    return null
  }
  if (t) {
    t.copy += performance.now() - c0
    addStats(t, m, c0 - w0)
  }
  m.free()
  return cap
}

function capTransfer(cap: CapData | null): ArrayBuffer[] {
  return cap ? [cap.positions.buffer, cap.normals.buffer, cap.indices.buffer] as ArrayBuffer[] : []
}

/**
 * 覚えている形を、この依頼に使えるか。回す前の形で作った陰関数（implicit）は、陰関数の細かさ・念入りに探すかでも形が変わる。
 * ただし再生・ドラッグ中（細かくしない依頼）なら、細かさが違っても使う（回転のたびに F を計算し直さない。止まったら細かく作り直す）
 */
function reusable(cached: NonNullable<ReturnType<typeof shapes.get>>, key: string, req: PlotRequest): boolean {
  const refine = (req.flags & PLOT_NO_REFINE) === 0
  if (cached.key !== key || !(cached.refined || !refine)) return false
  // 回す前の形で作った陰関数は、動いている間だけ使う（止まったときは、表示範囲だけを細かく作った面を表示する）
  if (cached.shape.rigid) return !refine
  if (!cached.shape.implicit || !refine) return true
  return cached.ires === req.implicitResolution && cached.thorough === ((req.flags & PLOT_THOROUGH) !== 0)
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
  const pendingLocal = localJobs.get(req.itemId)
  localJobs.delete(req.itemId)
  const { requestId, itemId, source, domain: d, resolution, implicitResolution, ranges, params, space, stacks } = req
  await ready

  let res: PlotResponse
  const t0 = performance.now()
  const tally: Tally = { eval: 0, mesh: 0, lod: 0, copy: 0 }
  let localDelay = 0 // 回す前の形を作り始めるまでの待ち（ms）
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
    // 回す前の形だけを作る印（PLOT_LOCAL）は、ふつうの呼び出しでは外す（回す前の形は、面を返した後に別に作る）
    const callPlot = (flags = req.flags & ~PLOT_LOCAL) =>
      plot(
        source,
        domainArr,
        resolution,
        implicitResolution,
        rangeNames.join(','),
        new Float64Array(rangeValues.flat()),
        Object.keys(params).join(','),
        new Float64Array(Object.values(params).flatMap((p) => [p.re, p.im])),
        Object.entries(params)
          .filter(([, p]) => p.complex)
          .map(([k]) => k)
          .join(','),
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
        Math.max(0, COMPLEX_MODE_ORDER.indexOf(req.complexMode)),
        req.walls,
        req.flat,
        req.layers,
        req.flatSlice,
        flags,
      )
    const key = (view: boolean, rigid = false) => shapeKey(req, view, coord.slice, coord.ranges, rigid)
    // 回し始め（再生・ドラッグ中の依頼）で、同じ形の回す前の形を作る予約があれば、先に作る（この 1 コマは待つが、粗く作り直さない）
    if (pendingLocal && (req.flags & PLOT_NO_REFINE) !== 0 && pendingLocal.key === key(true, true)) pendingLocal.run()
    const cached = shapes.get(itemId)
    // 覚えている形を回すだけで描けるなら、F の計算を分けない（計画の段階で「分けられない」と返す）
    if (req.field?.mode === 'plan' && cached && reusable(cached, key(cached.shape.view_dependent, cached.shape.rigid), req)) {
      const stage: FieldStageResponse = { requestId, ok: true, chunks: 0, values: null, ms: 0 }
      self.postMessage(stage)
      return
    }
    // 重い陰関数の F の計算を分ける途中の段階（計画・一部の計算）: メッシュは作らず、結果だけ返す
    if (req.field && req.field.mode !== 'given') {
      const w0 = performance.now()
      let stage: FieldStageResponse
      try {
        if (req.field.mode === 'plan') field_mode_plan()
        else field_mode_chunks(req.field.from, req.field.to)
        callPlot().free()
        stage =
          req.field.mode === 'plan'
            ? { requestId, ok: true, chunks: field_chunks(), values: null, ms: performance.now() - w0 }
            : { requestId, ok: true, chunks: 0, values: take_field_values(), ms: performance.now() - w0 }
      } catch (e) {
        field_mode_reset()
        stage = { requestId, ok: false, error: e instanceof Error ? e.message : String(e) }
      }
      self.postMessage(stage, { transfer: stage.ok && stage.values ? [stage.values.buffer as ArrayBuffer] : [] })
      return
    }
    let r: MeshResult | null = null
    /** メッシュの配列を取り出す結果（回す前の形のメッシュを返すときは、r とは別） */
    let geom: MeshResult | null = null
    let localTransform: Float64Array | null = null
    let wall = 0 // 計算エンジンを呼んだ時間
    let reprojected = false // n 次元の点を使い回して、射影だけし直したか
    // 細かくしない依頼（再生・ドラッグ中）なら、細かくした点もそのまま使う（回転の再生で計算し直さない）。
    // 細かくする依頼なら、細かくしないで計算した点は使わない（止まったときに細かく計算し直す）
    const refine = (req.flags & PLOT_NO_REFINE) === 0
    // 回す前の形で作った陰関数は、メッシュを射影し直さず、描画側で 3D オブジェクトに掛ける 1 次式だけを返す
    let transformOnly = false
    const transformOf = (shape: Shape) =>
      shape.transform(domainArr, space.dims, new Uint32Array(space.axes), new Float64Array(space.rotation), new Float64Array(space.center))
    if (cached && reusable(cached, key(cached.shape.view_dependent, cached.shape.rigid), req)) {
      // 点は計算済み: 回転・射影だけし直す
      const w0 = performance.now()
      try {
        if (cached.shape.rigid) {
          r = transformOf(cached.shape)
          if (cached.localShown) transformOnly = true
          else {
            // 画面側には、止まったときの面（表示範囲だけを細かく作ったもの）がある。回す前の形のメッシュを 1 回だけ送る
            geom = cached.shape.local_mesh() ?? null
            localTransform = new Float64Array(r.transform)
            cached.localShown = true
          }
        } else {
          r = cached.shape.project(
            domainArr,
            space.dims,
            new Uint32Array(space.axes),
            new Float64Array(space.rotation),
            new Float64Array(space.center),
            space.perspective,
          )
        }
      } catch {
        r = null // 回転した陽関数で、回転が条件を外れた（傾いた座標系の陽関数・陰関数として計算し直す）
      }
      if (r) {
        wall = performance.now() - w0
        reprojected = true
        // 回した陰関数は、切り口のふたの元もその回転のものに替える
        const capSource = r.take_field()
        if (capSource) {
          fields.get(itemId)?.source.free()
          fields.set(itemId, { source: capSource, resolution: implicitResolution * 2 })
        }
      }
    }
    if (!r) {
      const w0 = performance.now()
      // ほかの Worker が計算した F の値を使ってメッシュを作る（計算し直すときだけ。使い回すときに入れると、次の計算に残る）
      if (req.field?.mode === 'given') field_mode_given(req.field.values)
      // 止まったとき: 覚えている回す前の形（回転の再生用）がそのまま使えるなら、作り直さない（表示範囲の面だけ作る）
      const thorough = (req.flags & PLOT_THOROUGH) !== 0
      const keepLocal =
        cached !== undefined &&
        cached.shape.rigid &&
        (req.flags & PLOT_LOCAL) !== 0 &&
        cached.key === key(true, true) &&
        cached.ires === implicitResolution &&
        cached.thorough === thorough
      r = callPlot()
      wall = performance.now() - w0
      if (coordError && !r.param_name) {
        r.free()
        throw coordError // パラメータの定義ではなかった
      }
      if (keepLocal && cached) {
        // 画面側は表示範囲の面に戻るので、次に動かすときは回す前の形のメッシュを送り直す
        cached.localShown = false
        fields.get(itemId)?.source.free()
        fields.delete(itemId)
      } else forget(itemId)
      const shape = r.take_shape()
      if (shape && !keepLocal) {
        shapes.set(itemId, { key: key(shape.view_dependent, shape.rigid), shape, refined: refine, ires: implicitResolution, thorough, localShown: false })
      }
      // 細かく作る依頼: 面を返した後で、回す前の形を作っておく（次に回すときは、回すだけで描く。回転が 0 のときも作る）
      if (!keepLocal && refine && r.local_available) {
        localJobs.set(itemId, {
          key: key(true, true),
          run: () => {
            let lr: MeshResult | null = null
            try {
              field_mode_reset()
              lr = callPlot(req.flags | PLOT_LOCAL)
              const local = lr.take_shape()
              if (local?.rigid) {
                shapes.get(itemId)?.shape.free()
                shapes.set(itemId, { key: key(true, true), shape: local, refined: true, ires: implicitResolution, thorough, localShown: false })
              } else local?.free()
            } catch {
              // 作れなくても、次に回すときに今までどおり作り直すだけ
            } finally {
              lr?.free()
            }
          },
        })
        localDelay = (req.flags & PLOT_LOCAL) !== 0 ? 0 : LOCAL_IDLE_MS
      }
      const capSource = r.take_field()
      if (capSource) fields.set(itemId, { source: capSource, resolution: implicitResolution * 2 })
    }
    addStats(tally, r, wall)
    const f = fields.get(itemId)
    const cap = f ? makeCap(f.source, req.cut, f.resolution, tally) : null
    const vars = r.params.split(',').filter((s) => s !== '')
    // 曲面の格子線を引くための、媒介変数の範囲の長さ
    const span = (k: number) => {
      const v = vars[k]
      if (v === undefined) return 2 * Math.PI
      const rg = stacks[v] ?? (v in ranges ? rangeValues[rangeNames.indexOf(v)] : [0, 2 * Math.PI])
      return rg[1] - rg[0]
    }
    const c0 = performance.now()
    // 配列は取り出して渡す（getter だと Rust 側の clone と JavaScript への写しで2回コピーになる）
    const g = geom ?? r
    const transform = transformOnly ? new Float64Array(r.transform) : localTransform
    const mesh = {
      positions: g.take_positions(),
      normals: g.take_normals(),
      indices: g.take_indices(),
      lodIndices: g.take_lod_indices(),
      lod: g.has_lod_mesh
        ? { positions: g.take_lod_positions(), normals: g.take_lod_normals(), indices: g.take_lod_mesh_indices() }
        : null,
      gridCoords: g.take_grid_coords(),
      complexValues: g.take_complex_values(),
      transform,
      transformOnly,
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
    tally.copy += performance.now() - c0
    const kind = r.kind
    const usedParams = [...new Set([...r.used_params.split(',').filter((s) => s !== ''), ...rangeUsed])]
    const pv = r.param_value
    const param = r.param_name ? { name: r.param_name, re: pv[0], im: pv[1] } : null
    r.free() // Rust 側のメモリを解放
    geom?.free()
    const total = performance.now() - t0
    const timing: PlotTiming = { ...tally, total, sentAt: performance.timeOrigin + performance.now() }
    res = { requestId, ok: true, mesh, kind, ms: total, usedParams, param, cap, timing, reprojected, cost: costOf(kind, mesh.curve, req.flat) }
  } catch (e) {
    field_mode_reset()
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
        ...capTransfer(res.mesh.lod),
        ...capTransfer(res.cap),
      ]
    : []
  self.postMessage(res, { transfer })
  // 返した後で、回す前の形を作る（届いている次の依頼を先に処理する。その依頼が同じ式なら、予約は消える）
  const job = localJobs.get(req.itemId)
  if (job) {
    setTimeout(() => {
      // 待っている間に次の依頼が来ていれば、予約は消えているか、別の予約に替わっている
      if (localJobs.get(req.itemId) !== job) return
      localJobs.delete(req.itemId)
      job.run()
    }, localDelay)
  }
}

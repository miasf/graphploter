<script lang="ts">
  // アプリの画面全体。左に式の一覧、右に3D表示。
  import { onMount, tick } from 'svelte'
  import ExpressionItem from './lib/ExpressionItem.svelte'
  import { conditionHolds, paramConditions, hasIntegral, inlineRegions, integralDisplay, riemannDisplay, type ParamCondition, type RiemannMethod, latexToText, regionDisplayLatex, regionsUsed, setRegions } from './lib/latex'
  import { blankMatrix, matrixDefinition, MAX_SIZE as MATRIX_MAX } from './lib/matrixEdit'
  import { evalBounds, evalValue, exprError } from './lib/evalExpr'
  import NumberInput from './lib/NumberInput.svelte'
  import EditableValue from './lib/EditableValue.svelte'
  import { loadInitial, shareUrl, type SavedState } from './lib/persist'
  import {
    UNTITLED, deleteGraph, duplicateGraph, isEmptyState, lastGraphId, listGraphs, markNew, renameGraph, saveGraph, setCurrent, setThumb, type GraphMeta,
  } from './lib/library'
  import GraphsDialog from './lib/GraphsDialog.svelte'
  import { calledAsFunction, complexLatex, coordName, fitRotation, latexName, pairs, prettyName, resizeRotation, resizeSpace, rotDims, sequenceDefinitionName, sequenceInitialLatex, sliderStep, snapToStep } from './lib/params'
  import { keyboardOcclusion, setKeyboardDims } from './lib/mathSetup'
  import { SAMPLE_GROUPS, type Sample, type SampleView } from './lib/samples'
  import RotationPanel from './lib/RotationPanel.svelte'
  import CoordPanel from './lib/CoordPanel.svelte'
  import { AUTO_COORD_NAMES, COORD_PRESETS, GREEK_NAMES, autoCoord, autoRadius, defaultAngleRange, effectiveRanges, engineName, presetFollow, type AutoCoord } from './lib/coords'
  import SpacePanel from './lib/SpacePanel.svelte'
  import { requestCap, requestCoordGrid, requestPlot } from './lib/plotClient'
  import { lastBreakdown, recordBreakdown } from './lib/timings'
  import { Budget } from './lib/budget'
  import type {
    ComplexMode,
    CoordSystem,
    IntegralViewInfo,
    CutRequest,
    CutStyle,
    Domain,
    Folder,
    GridStyle,
    HiddenStyle,
    ParamRanges,
    ParamValue,
    PlotItem,
    RotationGroup,
    SliderStyle,
    Space,
    RenderMode,
  } from './lib/types'
  import { DEFAULT_HIDDEN, GLOBAL_GROUP, HIDDEN_MODE_ORDER, OWN_GROUP, PLOT_EDGES, PLOT_NO_REFINE, PLOT_POI, PLOT_SEQUENCE_TABLE, PLOT_THOROUGH } from './lib/types'
  import { Viewer, type PickResult } from './lib/Viewer'
  import type { MotionMode } from './lib/motionQuality'

  const PALETTE = ['#2d70b3', '#c74440', '#388c46', '#6042a6', '#fa7e19', '#1a9ea0']

  let nextId = 1
  /** 新しい式。latex は数式入力欄の書き方（LaTeX） */
  function newItem(latex: string, options: Partial<PlotItem> = {}): PlotItem {
    const id = nextId++
    return {
      id,
      latex,
      color: PALETTE[(id - 1) % PALETTE.length],
      visible: true,
      opacity: 1,
      grid: { enabled: true, x: 1, y: 1, z: 0, u: 24, v: 24, w: 24, width: 1 },
      lineWidth: 3,
      showSettings: false,
      vertical: true,
      ranges: { t: ['0', '2\\pi'], u: ['0', '2\\pi'], v: ['0', '2\\pi'] },
      params: [],
      paramSpan: { u: 2 * Math.PI, v: 2 * Math.PI },
      slider: { min: -10, max: 10, complex: false, stack: false },
      usedParams: [],
      ...options,
    }
  }

  // ---- 画面の状態 ----
  // 最初は空の式が1つだけ。例は「サンプル」からタップして追加する
  let items = $state<PlotItem[]>([newItem('')])
  let domain = $state<Domain>({ xmin: -10, xmax: 10, ymin: -10, ymax: 10, zmin: -6, zmax: 6 })
  let resolution = $state(150)
  let implicitResolution = $state(80)
  /** 陰関数で、格子より小さい図形を念入りに探す（⚙ の設定。遅くなる） */
  let findSmall = $state(false)
  let domainError = $state('')
  /** 遠近感（0: 平行投影 〜 100: 強い透視投影） */
  let perspective = $state(41)
  /** 2D 表示（縦の座標を 0 にして、横・奥の平面に描く） */
  let flat = $state(false)
  /** 2D 表示の描き方: true なら切り口（縦の座標 = 0 の平面との交わりの線）、false なら投影（真上から押しつぶした形） */
  let flatSlice = $state(false)
  /** 立体の切り口 */
  let cut = $state<CutStyle>({ enabled: false, axis: 0, pos: 0, flip: false })
  /** 計算担当・描画に渡す切り口の指定 */
  function cutRequest(): CutRequest {
    return cut.enabled && !flat ? { axis: cut.axis, pos: cut.pos, keepBelow: !cut.flip } : null
  }
  /** マウスの操作モード（A: 標準 / B: Google Earth に近い操作） */
  let controlMode = $state<'A' | 'B'>('A')
  $effect(() => {
    viewer?.setControlMode(controlMode)
  })
  /** 暗い配色にするか（最初は OS の設定に合わせる） */
  let dark = $state(window.matchMedia('(prefers-color-scheme: dark)').matches)

  // 配色を画面全体（CSS）と 3D 表示の両方に反映
  $effect(() => {
    document.documentElement.dataset.theme = dark ? 'dark' : 'light'
    viewer?.setDark(dark)
  })

  let viewerEl: HTMLElement
  let viewer = $state.raw<Viewer | null>(null)

  // 視点を動かしている間の画質。端末（GPU）ごとに向きが違うので、共有リンクには入れず、このブラウザにだけ覚える
  const MOTION_KEY = 'graphploter.motionMode'
  const MOTION_MODES: { mode: MotionMode; label: string; note: string }[] = [
    { mode: 'auto', label: '自動', note: '遅いときだけ、重い図形を粗くし、それでも遅ければ画面の解像度を下げる（下げても速くならなければ戻す）' },
    { mode: 'sharp', label: 'くっきり', note: '画面の解像度は下げない。重い図形だけ粗くする' },
    { mode: 'full', label: '常にそのまま', note: '何も下げない（重い式では動きがカクつく）' },
  ]
  function loadMotionMode(): MotionMode {
    try {
      const v = localStorage.getItem(MOTION_KEY)
      if (v === 'auto' || v === 'sharp' || v === 'full') return v
    } catch {
      // 保存できない環境（プライベートモードなど）では自動
    }
    return 'auto'
  }
  let motionMode = $state<MotionMode>(loadMotionMode())
  function setMotionMode(mode: MotionMode) {
    motionMode = mode
    viewer?.setMotionMode(mode)
    try {
      localStorage.setItem(MOTION_KEY, mode)
    } catch {
      // 覚えられなくても、この画面の間は効く
    }
  }

  onMount(() => {
    viewer = new Viewer(viewerEl, () => narrow.matches)
    viewer.onPick = handlePick
    // 光線の描き方で打ち切りが多く、自動でメッシュに回したとき: 式の ⚙ に理由を出す
    viewer.onGpuFallback = (id, reason) => {
      const it = items.find((x) => x.id === id)
      if (it && it.render !== 'gpu') {
        it.gpuAvailable = false
        it.gpuReason = reason
      }
    }
    viewer.setMotionMode(motionMode)
    if (BENCH) {
      // ベンチマーク: 保存した状態は読まず、自動保存もしない（loaded を false のままにする）
      viewer.setDark(dark)
      viewer.setPerspective(perspective / 100)
      startBench(viewer)
      return () => viewer?.dispose()
    }
    ;(async () => {
      // 共有リンクで開いたならその状態、そうでなければ最後に開いたグラフに戻す（どちらも無ければ新しいグラフ）
      blankState = snapshot()
      const init = await loadInitial()
      graphId = init.id
      const saved = init.state
      if (saved) {
        // 読み込めない保存データ（別の版のものなど）で止まると、開き直しても同じ所で止まり続ける。最初の状態に戻して続ける
        const fresh = snapshot()
        try {
          restore(saved)
        } catch (e) {
          console.error('保存した状態を読み込めませんでした', e)
          restore(fresh)
        }
      }
      viewer?.setDark(dark)
      viewer?.setPerspective(flat ? 0 : perspective / 100)
      viewer?.setFlat(flat)
      viewer?.setDomain(domain, true)
      viewer?.setCut(cutRequest())
      loaded = true
      updateSpaceRangeErrors()
      items.forEach((it) => replot(it))
    })()
    return () => viewer?.dispose()
  })

  // ---- 点を押すと値が出る（学習用）----

  /**
   * グラフを押したら値を出すか（標準はオフ。タッチで操作すると印が出て見づらいため）。端末ごとの好みなので、共有リンクには入れず、このブラウザにだけ覚える
   */
  const PICK_KEY = 'graphploter.pickValues'
  let pickValues = $state(
    (() => {
      try {
        return localStorage.getItem(PICK_KEY) === '1'
      } catch {
        return false
      }
    })(),
  )
  function setPickValues(on: boolean) {
    pickValues = on
    try {
      localStorage.setItem(PICK_KEY, on ? '1' : '0')
    } catch {
      // 保存できない環境では、この画面の間だけ
    }
    if (!on) clearPick()
    // 注目する点（極値・零点）は、オンのときだけ求めるので、オンにしたら計算し直す
    else for (const it of items) if (it.visible && it.latex.trim() !== '') schedule(it, 0)
  }

  /** 印を出している式の id（無ければ null） */
  let pickedId: number | null = null
  /** 押した順の番号（古い計算の結果を捨てる） */
  let pickSeq = 0

  const POI_NAMES = ['極大', '極小', '鞍点', '零点', '', '停留点']

  function clearPick() {
    pickedId = null
    pickSeq++
    viewer?.showPick(null)
  }

  /** 札に出す数（有効 6 桁。ごく小さい数は 0） */
  function pickNum(v: number, span: number) {
    if (Math.abs(v) < 1e-9 * Math.max(1, span)) return '0'
    return Number(v.toPrecision(6)).toString().replace('-', '−')
  }

  /**
   * グラフを押したとき: 押した所の式の点に印を付け、座標を札に出す。
   * 陽関数の行（y = f(x)・z = f(x, y)）は、押した横・奥の座標で式の値を計算し直して、縦の座標を正確な値にする
   * （画面の線・三角形は折れ線なので、そのままでは少しずれる）。注目する点（極値・零点など）は計算エンジンが求めた正確な点
   */
  function handlePick(r: PickResult | null) {
    if (!pickValues) return
    if (!r) {
      clearPick()
      return
    }
    const item = items.find((it) => it.id === r.id)
    if (!item) {
      clearPick()
      return
    }
    const seq = ++pickSeq
    pickedId = item.id
    const names = axisNames
    const shownAxes = flat ? [0, 1] : [0, 1, 2]
    const spans = [domain.xmax - domain.xmin, domain.ymax - domain.ymin, domain.zmax - domain.zmin]
    const pt: [number, number, number] = [...r.point]
    const label = () => {
      const kindName = (k: number) => (k === 4 ? `${prettyName(names[flat ? 1 : 2])} 切片` : (POI_NAMES[k] ?? ''))
      const head = r.poi === undefined ? '' : `${(r.kinds ?? [r.poi]).map(kindName).join('・')}\n`
      const coords = `(${shownAxes.map((k) => pickNum(pt[k], spans[k])).join(', ')})`
      return head + coords
    }
    viewer?.showPick(pt, label(), item.color, item.id)
    if (r.poi !== undefined) return
    // 陽関数の行なら、縦の座標を式から計算し直す
    const m = /^\s*([a-z])\s*=(.*)$/s.exec(item.latex)
    if (!m || item.integralOf !== undefined || item.regionName !== undefined) return
    const lhs = m[1]
    const axis = names.indexOf(lhs)
    const rhs = m[2]
    if (axis < 0 || (flat && axis === 2) || new RegExp(`(^|[^a-zA-Z\\\\])${lhs}(?![a-zA-Z])`).test(rhs) || /,/.test(rhs.replace(/\\left\(.*?\\right\)/g, ''))) return
    const others = shownAxes.filter((k) => k !== axis)
    const at = `\\left.${rhs}\\right|_{${others.map((k) => `${names[k]}=${pt[k].toFixed(12)}`).join(',')}}`
    const run = () => {
      if (seq !== pickSeq) return
      const v = evalValue(at, currentParams(), run, currentDefinitions(item))
      if (!v || !('value' in v)) return
      // 押した点から遠すぎる値（式の読み違い）は使わない
      if (Math.abs(v.value - pt[axis]) > 0.08 * Math.max(spans[axis], 1e-9)) return
      pt[axis] = v.value
      viewer?.showPick(pt, label(), item.color, item.id)
    }
    run()
  }

  // ---- 保存・共有 ----

  /** 保存した状態を読み込み終わったか（それまでは自動保存しない） */
  let loaded = $state(false)

  // ---- グラフの一覧（lib/library.ts。このブラウザに、グラフごとに保存する）----

  /** 開いているグラフの id と名前 */
  let graphId = $state('')
  let graphTitle = $state(UNTITLED)
  /** 何も書いていない最初の状態（「新しいグラフ」「最初の状態に戻す」に使う） */
  let blankState: SavedState | null = null
  /** 一覧の画面を開いているか・一覧の中身 */
  let graphsOpen = $state(false)
  let graphList = $state<GraphMeta[]>([])
  /** 最後の保存に失敗したか（容量がいっぱいなど） */
  let saveFailed = $state(false)
  /** 最後に小さな絵を撮った時刻 */
  let lastThumb = 0

  $effect(() => {
    document.title = graphTitle && graphTitle !== UNTITLED ? `${graphTitle} – graphploter` : 'graphploter'
  })

  /** 今のグラフの小さな絵を撮って一覧に付ける */
  function takeThumb() {
    if (!graphId || !viewer) return
    const t = viewer.thumbnail()
    if (t) setThumb(graphId, t)
    lastThumb = Date.now()
  }

  function refreshGraphs() {
    graphList = listGraphs()
  }

  function openGraphs() {
    flushSave()
    takeThumb()
    refreshGraphs()
    graphsOpen = true
  }

  /** 今のグラフが空（式が 1 つも無い）か */
  function currentEmpty() {
    return isEmptyState(snapshot())
  }

  /** 読み込み直して、次のグラフを開く（式・計算・再生の状態を確実に入れ替えるため、ページごと読み込み直す） */
  function reloadInto(prepare: () => void) {
    flushSave()
    takeThumb()
    // 何も書いていない今のグラフは、一覧に残さない
    if (currentEmpty()) deleteGraph(graphId)
    loaded = false
    unsaved = null
    clearTimeout(saveTimer)
    prepare()
    location.reload()
  }

  function openGraph(id: string) {
    if (id === graphId) {
      graphsOpen = false
      return
    }
    reloadInto(() => setCurrent(id))
  }

  function newGraph() {
    if (currentEmpty()) {
      graphsOpen = false
      return
    }
    reloadInto(markNew)
  }

  function removeGraph(id: string) {
    const title = graphList.find((m) => m.id === id)?.title ?? UNTITLED
    if (!confirm(`「${title}」を削除します。元に戻せません。よろしいですか？`)) return
    if (id !== graphId) {
      deleteGraph(id)
      refreshGraphs()
      return
    }
    // 開いているグラフを消したら、次に新しいものを開く（無ければ新しいグラフ）
    loaded = false
    unsaved = null
    clearTimeout(saveTimer)
    deleteGraph(id)
    const next = lastGraphId()
    if (next) setCurrent(next)
    else markNew()
    location.reload()
  }

  function copyGraph(id: string) {
    flushSave()
    const nid = duplicateGraph(id)
    if (nid) reloadInto(() => setCurrent(nid))
  }

  function renameAny(id: string, title: string) {
    if (id === graphId) {
      graphTitle = title.trim() || UNTITLED
      flushSave()
    } else {
      renameGraph(id, title)
    }
    refreshGraphs()
  }

  /** 式 1 つの、保存する分の設定 */
  function savedItem(it: PlotItem) {
    return {
      latex: it.latex,
      color: it.color,
      visible: it.visible,
      opacity: it.opacity,
      grid: it.grid,
      lineWidth: it.lineWidth,
      complexMode: it.complexMode,
      // 断面のままでも、霧の濃さ・陰影などを変えていれば残す
      hidden: it.hidden && JSON.stringify(it.hidden) !== JSON.stringify(DEFAULT_HIDDEN) ? it.hidden : undefined,
      complexCoords: it.complexCoords || undefined,
      slice: it.slice && Object.keys(it.slice).length > 0 ? it.slice : undefined,
      walls: it.walls,
      field: it.field && (it.field.count !== undefined || it.field.unit || it.field.stream) ? it.field : undefined,
      ode: it.ode && it.ode.init.some((s) => s.trim() !== '') ? it.ode : undefined,
      layers: it.layers,
      edges: it.edges || undefined,
      render: it.render && it.render !== 'auto' ? it.render : undefined,
      ranges: it.ranges,
      vertical: it.vertical,
      slider: it.slider,
      rotGroup: it.rotGroup,
      rotation: it.rotGroup === undefined ? it.rotation : undefined,
      center: it.rotGroup === undefined ? it.center : undefined,
      folder: it.folder,
      coordSys: coordOf(it) ? it.coordSys : undefined,
      angleRanges: it.angleRanges && Object.keys(it.angleRanges).length > 0 ? it.angleRanges : undefined,
    }
  }

  /** 今の式と設定（保存する分だけ） */
  function snapshot(): SavedState {
    return $state.snapshot({
      version: 1 as const,
      title: graphTitle,
      items: listed().map(savedItem),
      groups,
      coordSystems: coordSystems.length > 0 ? coordSystems : undefined,
      folders,
      domain,
      resolution,
      implicitResolution,
      findSmall: findSmall || undefined,
      perspective,
      dark,
      space,
      cut,
      flat,
      flatSlice,
      controlMode,
      // 今の式で使っている範囲だけを残す
      regionsShown: (() => {
        // 直書きの範囲（#式の id:番号）は式の id が保存で変わるので残さない
        const shown = items.flatMap((it) => (it.regionName && !it.regionName.startsWith('#') ? [it.regionName] : []))
        return shown.length > 0 ? shown : undefined
      })(),
      integralsShown: (() => {
        const rows = listed()
        const shown = items.flatMap((it) => (it.integralOf === undefined ? [] : [rows.findIndex((x) => x.id === it.integralOf)])).filter((k) => k >= 0)
        return shown.length > 0 ? shown : undefined
      })(),
      regions: (() => {
        const used = new Set(items.flatMap((it) => regionsUsed(it.latex).map((r) => r.name)))
        const kept = Object.entries(regions).filter(([name, v]) => used.has(name) && v.trim() !== '')
        return kept.length > 0 ? Object.fromEntries(kept) : undefined
      })(),
      rotOwn: true,
    })
  }

  function restore(s: SavedState) {
    nextId = 1
    graphTitle = typeof s.title === 'string' && s.title.trim() ? s.title : UNTITLED
    regions = { ...(s.regions ?? {}) }
    setRegions(regions)
    // 古い保存データでは、グループに入っていない式は全体の回転で描いていた
    items = s.items.map((si) => newItem(si.latex, { ...si, rotGroup: si.rotGroup ?? (s.rotOwn ? undefined : GLOBAL_GROUP) }))
    domain = s.domain
    // 数でない値（壊れた・別の版の保存データ）は、今の値のままにする
    const num = (v: unknown, now: number) => (typeof v === 'number' && Number.isFinite(v) ? v : now)
    resolution = num(s.resolution, resolution)
    implicitResolution = num(s.implicitResolution, implicitResolution)
    findSmall = s.findSmall ?? false
    perspective = num(s.perspective, perspective)
    dark = typeof s.dark === 'boolean' ? s.dark : dark
    // 古い保存データ・壊れた共有リンクでも動くよう、次元に合わせて配列の長さをそろえる
    const dims = Math.max(3, Math.min(10, Math.round(Number(s.space.dims)) || 3))
    space = {
      ...s.space,
      dims,
      axes: s.space.axes?.length === 3 && s.space.axes.every((a) => a < dims) ? s.space.axes : [0, 1, 2],
      slice: Array.from({ length: dims }, (_, k) => s.space.slice?.[k] ?? 0),
      ranges: Array.from({ length: dims }, (_, k) => s.space.ranges?.[k] ?? null),
      perspective: s.space.perspective ?? 0,
      ...fitRotation(s.space.rotation, s.space.center, dims),
    }
    for (const it of items) {
      if (it.rotation) Object.assign(it, fitRotation(it.rotation, it.center, dims))
    }
    if (s.cut) cut = s.cut
    flat = s.flat ?? false
    flatSlice = s.flatSlice ?? false
    groups = (s.groups ?? []).map((g) => ({ ...g, ...fitRotation(g.rotation, g.center, dims) }))
    folders = s.folders ?? []
    nextFolderId = Math.max(0, ...folders.map((f) => f.id)) + 1
    normalizeFolders()
    if (s.controlMode) controlMode = s.controlMode
    nextGroupId = Math.max(0, ...groups.map((g) => g.id)) + 1
    coordSystems = (s.coordSystems ?? []).map((c) => ({ ...c, morph: Math.min(1, Math.max(0, Number(c.morph) || 0)) }))
    nextCoordId = Math.max(0, ...coordSystems.map((c) => c.id)) + 1
    playingMorph = []
    const sources = (s.integralsShown ?? []).flatMap((k) => (items[k] ? [items[k]] : []))
    for (const name of s.regionsShown ?? []) showRegion(name)
    for (const it of sources) showIntegral(it)
  }

  // 変わったら（0.5 秒ほどまとめてから）ブラウザに保存する。
  // 「変わるたびに待ち直す」形にすると、回転・パラメータの再生中は毎コマ変わるので、いつまでも保存されない
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  /** まだ保存していない最新の状態 */
  let unsaved: SavedState | null = null
  function flushSave() {
    clearTimeout(saveTimer)
    saveTimer = undefined
    if (unsaved && graphId) {
      // 開いただけの空のグラフは一覧に入れない（式を書いたら入る）
      if (!isEmptyState(unsaved) || listGraphs().some((m) => m.id === graphId)) {
        saveFailed = !saveGraph(graphId, unsaved)
        // 小さな絵は、変わってから 10 秒ごとまで（描き直しの手間を減らす）
        if (!saveFailed && Date.now() - lastThumb > 10000) takeThumb()
      }
    }
    unsaved = null
  }
  $effect(() => {
    if (!loaded) return
    unsaved = snapshot() // ここで読んだ値が変わると、この処理がもう一度動く
    saveTimer ??= setTimeout(flushSave, 500)
  })
  // タブを閉じる・別のアプリに切り替えるときは、待たずに保存する
  window.addEventListener('pagehide', flushSave)
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'hidden') flushSave()
  })

  let shareMessage = $state('')
  async function copyShareLink() {
    const url = await shareUrl(snapshot())
    try {
      await navigator.clipboard.writeText(url)
      shareMessage = 'リンクをコピーしました'
    } catch {
      // コピーできない環境では、アドレス欄に入れて見せる
      history.replaceState(null, '', url)
      shareMessage = 'アドレス欄のリンクを共有してください'
    }
    setTimeout(() => (shareMessage = ''), 3000)
  }

  function resetAll() {
    if (!confirm('このグラフの式と設定をすべて最初の状態に戻します（名前は残します）。よろしいですか？')) return
    // 読み込み直す前（ページを閉じるとき）に今の状態が保存し直されないようにする
    loaded = false
    unsaved = null
    clearTimeout(saveTimer)
    if (blankState) saveGraph(graphId, { ...blankState, title: graphTitle })
    location.reload()
  }

  // ---- スマホ用: 下から引き出すシート ----
  //
  // 幅の狭い画面では、式の一覧をグラフの上に重なるシートにする。
  // 上端のつまみをドラッグすると高さが変わり、指を離すと 3 段階（たたむ / 半分 / 全体）のどれかに収まる。
  // つまみをタップすると「たたむ ⇔ 半分」を切り替える。

  // ---- PC 用: 式の一覧の幅 ----
  // 長い式が欄に収まらないとき、右の端をドラッグして広げられる。幅はこのブラウザに覚える
  const PANEL_WIDTH_KEY = 'graphploter:panelWidth'
  let panelWidth = $state<number | null>(
    (() => {
      try {
        const w = Number(localStorage.getItem(PANEL_WIDTH_KEY))
        return w >= 300 ? w : null
      } catch {
        return null
      }
    })(),
  )
  let widthDrag = $state<{ startX: number; startW: number } | null>(null)

  function setPanelWidth(w: number | null) {
    panelWidth = w === null ? null : Math.round(Math.min(Math.max(w, 300), window.innerWidth * 0.7))
    try {
      if (panelWidth === null) localStorage.removeItem(PANEL_WIDTH_KEY)
      else localStorage.setItem(PANEL_WIDTH_KEY, String(panelWidth))
    } catch {
      // 保存できない環境では、このページを開いている間だけ覚える
    }
  }
  function widthDown(e: PointerEvent) {
    const panel = (e.currentTarget as HTMLElement).parentElement!
    widthDrag = { startX: e.clientX, startW: panel.getBoundingClientRect().width }
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    e.preventDefault()
  }
  function widthMove(e: PointerEvent) {
    if (widthDrag) setPanelWidth(widthDrag.startW + e.clientX - widthDrag.startX)
  }
  function widthUp() {
    widthDrag = null
  }

  let preferredSheetExtent = $state<number | null>(null)
  let expandedSheetExtent: number | null = null
  let windowHeight = $state(window.innerHeight)
  // キーボードの高さは入力欄の見える範囲だけに使い、グラフの視点には反映しない
  let keyboardHeight = $state(0)
  onMount(() => {
    const kb = window.mathVirtualKeyboard
    const measure = () => (keyboardHeight = keyboardOcclusion())
    kb.addEventListener('geometrychange', measure)
    measure()
    return () => kb.removeEventListener('geometrychange', measure)
  })
  const defaultSheetExtent = $derived(Math.max(64, Math.round(windowHeight * 0.36)))
  const maxSheetExtent = $derived(Math.max(64, windowHeight - 120))
  const minSheetExtent = $derived(Math.min(maxSheetExtent, keyboardHeight > 0 ? keyboardHeight + 176 : 64))
  const sheetExtent = $derived(clampSheetExtent(preferredSheetExtent ?? defaultSheetExtent))
  const sheetHeight = $derived(Math.max(0, sheetExtent - keyboardHeight))
  const settingsHeight = $derived(Math.max(0, Math.min(windowHeight * 0.45, maxSheetExtent - keyboardHeight)))

  const narrow = window.matchMedia('(max-width: 560px), (max-width: 720px) and (min-height: 501px)')
  /** 設定を開いているか */
  let settingsOpen = $state(false)
  const graphBottom = $derived(settingsOpen ? Math.max(sheetExtent, settingsHeight + keyboardHeight) : sheetExtent)

  function clampSheetExtent(extent: number) {
    return Math.max(minSheetExtent, Math.min(maxSheetExtent, extent))
  }

  function toggleSheet() {
    if (keyboardHeight > 0) window.mathVirtualKeyboard.hide()
    if ((preferredSheetExtent ?? defaultSheetExtent) <= 64) {
      preferredSheetExtent = expandedSheetExtent
    } else {
      expandedSheetExtent = preferredSheetExtent
      preferredSheetExtent = 64
    }
  }

  let drag: { pointerId: number; startY: number; startExtent: number; moved: boolean } | null = null

  function sheetDown(e: PointerEvent) {
    if (!narrow.matches || e.button !== 0 || drag) return
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    // つまみを触っても編集中の数式からフォーカスを外さず、キーボードを開いたまま動かす。
    e.preventDefault()
    drag = { pointerId: e.pointerId, startY: e.clientY, startExtent: sheetExtent, moved: false }
  }

  function sheetMove(e: PointerEvent) {
    if (drag?.pointerId !== e.pointerId) return
    const dy = e.clientY - drag.startY
    if (Math.abs(dy) > 4) drag.moved = true
    if (!drag.moved) return
    preferredSheetExtent = clampSheetExtent(drag.startExtent - dy)
  }

  function sheetUp(e: PointerEvent) {
    if (drag?.pointerId !== e.pointerId) return
    if (!drag.moved && e.type === 'pointerup') toggleSheet()
    drag = null
  }

  function sheetKey(e: KeyboardEvent) {
    if (!narrow.matches || e.target !== e.currentTarget) return
    if (e.key === 'Enter' || e.key === ' ') toggleSheet()
    else if (e.key === 'ArrowUp') preferredSheetExtent = clampSheetExtent(sheetExtent + 32)
    else if (e.key === 'ArrowDown') preferredSheetExtent = clampSheetExtent(sheetExtent - 32)
    else return
    e.preventDefault()
  }

  // ---- 設定（ボタンで項目の一覧を出し、選んだ項目だけを表示する）----

  type SettingsCat = 'dims' | 'rotate' | 'coords' | 'range' | 'detail' | 'look' | 'control' | 'cut' | 'save' | 'help'
  const SETTINGS_CATS: { id: SettingsCat; icon: string; label: string; desc: string }[] = [
    { id: 'rotate', icon: '⟳', label: '回転', desc: '全体・グループごとの回転と再生' },
    { id: 'dims', icon: '⧉', label: '次元', desc: '4次元以上、表示する座標と断面' },
    { id: 'coords', icon: '◍', label: '座標系', desc: '極座標・球座標・自作の座標系と、直交座標からの移行' },
    { id: 'range', icon: '⬚', label: '表示範囲', desc: 'x, y, z の最小値・最大値' },
    { id: 'detail', icon: '▦', label: '細かさ', desc: '曲面・陰関数の細かさ、重ねる枚数' },
    { id: 'look', icon: '◎', label: '見え方', desc: '動かしている間の画質、遠近感、視点のリセット、背景' },
    { id: 'control', icon: '✥', label: '操作', desc: 'マウスの操作モード A / B、グラフを押したら値を出す' },
    { id: 'cut', icon: '✂', label: '切り口', desc: '立体を平面で切る' },
    { id: 'save', icon: '⇪', label: '保存・共有', desc: 'グラフの一覧、共有リンク、最初の状態に戻す' },
    { id: 'help', icon: '?', label: '書き方のヒント', desc: '式の書き方と操作' },
  ]
  let settingsCat = $state<SettingsCat | null>(null)
  /** 回転の設定で表示しているグループ */
  let rotTab = $state(GLOBAL_GROUP)

  function closeSettings() {
    settingsOpen = false
    settingsCat = null
  }

  /** 式の ⚙ の「回転」から、そのグループの回転の設定を開く（この式だけの回転なら全体のタブ） */
  function openRotation(gid: number) {
    rotTab = gid < 0 ? GLOBAL_GROUP : gid
    settingsCat = 'rotate'
    settingsOpen = true
  }

  // ---- 計算の依頼 ----

  // 式ごとの「最新の依頼番号」。古い依頼の結果が後から届いても無視するため。
  const latest = new Map<number, number>()
  // 入力中に毎回計算しないよう、少し待ってから計算する（デバウンス）
  const timers = new Map<number, ReturnType<typeof setTimeout>>()
  // 計算中の式。計算中にもう一度頼まれたら、終わってから最新の状態でもう1回だけ計算する
  // （スライダーを動かし続けても、依頼が溜まって遅れないように）
  const running = new Set<number>()
  const again = new Set<number>()
  /** 最後に計算を頼んだ・計算が終わった時刻（ベンチマークが、計算が全部終わるのを待つのに使う） */
  let lastPlotActivity = 0

  function schedule(item: PlotItem, delay = 200) {
    lastPlotActivity = performance.now()
    clearTimeout(timers.get(item.id))
    timers.set(item.id, setTimeout(() => replot(item), delay))
  }

  // ---- 操作中は粗く計算する ----
  // スライダーや複素平面を動かしている間・回転を再生している間は、式ごとに測った計算時間から、
  // 1回の計算が持ち時間（12 ms）に収まる細かさで計算する（lib/budget.ts。軽い式は細かいまま）。
  // 指を離したら、粗く計算した式だけを細かく計算し直す。
  let pointerOnControl = false
  const coarseItems = new Set<number>()
  /** 式ごとの計算時間の見積もり */
  const budget = new Budget()
  /** 式ごとの、最後の計算で細かさに掛けた倍率（切り口のふたも同じ倍率にする） */
  const lastScale = new Map<number, number>()
  /** 2段階の表示: 粗い版を出した後、続けて細かく計算する式 */
  const fineNext = new Set<number>()
  /**
   * 前の返事が「点を使い回して射影だけし直した」ものだった式（回転の再生・回転角のドラッグ中の、媒介変数表示・回した陽関数）。
   * こうした式は、点を一度細かく計算すれば、あとは回しても計算し直さない。操作中でも細かさを下げずに計算する
   * （下げると、操作が終わるまで粗い形のまま回すことになる）
   */
  const reusable = new Set<number>()
  /** reusable のために、操作中でも細かく計算し直した式（その返事は使い回しではないが、次からは使い回せる） */
  const upgrading = new Set<number>()
  /**
   * 操作中に細かいまま頼んだのに、使い回しでなく作り直しになった式（1 回目）。2 回続いたら reusable から外す。
   * 3 次元の回転を回すだけで描いている途中に 4 次元の回転（表示していない座標が絡む回転）を始めると、
   * 毎コマ作り直しになるのに、細かさを下げずに頼み続けて激重になっていた
   */
  const upgradeMissed = new Set<number>()
  const isCoarse = () => pointerOnControl || anyRotPlaying() || playingParams.length > 0 || playingSlices.length > 0

  function refine() {
    viewer?.setBusy(isCoarse())
    if (isCoarse()) return
    for (const it of items) if (coarseItems.has(it.id)) schedule(it, 0)
  }

  window.addEventListener('pointerdown', (e) => {
    const t = e.target as Element | null
    if (t?.closest('input[type=range], svg.pad')) {
      pointerOnControl = true
      viewer?.setBusy(true)
    }
  })
  for (const ev of ['pointerup', 'pointercancel'] as const) {
    window.addEventListener(ev, () => {
      if (!pointerOnControl) return
      pointerOnControl = false
      refine()
    })
  }

  /** `full` なら、2段階の表示をせずに細かく計算する（2段階の表示の2回目） */
  async function replot(item: PlotItem, full = false) {
    // 非表示の式は計算しない（表示したときに計算する）。パラメータの行は、ほかの式が使うので計算する
    if (!item.visible && !item.param) {
      item.stale = true
      item.sequenceTable = undefined
      const previous = sourceDefinitionName(lastDefSource.get(item.id) ?? '')
      // 計算はしないが、式の書き方の誤りだけは今の式で調べ直す（範囲の欄を直したのに、前の誤りが残って見えないように）。
      // 計算の誤りは分からないので、表示したときに計算し直すまで出さない
      try {
        const source = latexToText(item.latex)
        const definition = sourceDefinitionName(source)
        if (definition || previous) noteDefinition(item, definition, source)
        item.error = undefined
      } catch (e) {
        if (previous) noteDefinition(item, null, '')
        item.error = e instanceof Error ? e.message : String(e)
      }
      return
    }
    if (running.has(item.id)) {
      again.add(item.id)
      return
    }
    running.add(item.id)
    lastPlotActivity = performance.now()
    try {
      await replotNow(item, full)
    } finally {
      running.delete(item.id)
      lastPlotActivity = performance.now()
      // 粗い版の計算中に新しい依頼が来ていたら、細かい計算はせずに、新しい依頼を（必要なら2段階で）計算する
      if (again.delete(item.id) && items.includes(item)) replot(item)
      else if (fineNext.delete(item.id) && items.includes(item)) replot(item, true)
    }
  }

  /** いま定義されているパラメータ（同じ名前が複数あれば上の行を優先） */
  function currentParams(): Record<string, ParamValue> {
    const out: Record<string, ParamValue> = {}
    for (const it of items) {
      if (it.param && !it.error && !(it.param.name in out)) {
        out[it.param.name] = { re: it.param.re, im: it.param.im, complex: it.slider.complex }
      }
    }
    return out
  }

  /** 計算エンジン用の式の、関数の定義の左辺 `f(x, y) =` */
  const DEF_HEAD = /^\s*([A-Za-z]\d*|\{\w+\})\s*\(\s*[A-Za-z]\w*(\s*,\s*[A-Za-z]\w*)*\s*\)\s*=/
  /** 計算エンジン用の式の、集合の定義 `S = {…}`・`S = A ∪ B`（右辺に集合の記号がある） */
  const SET_DEF_HEAD = /^\s*([A-Za-z]\d*|\{\w+\})\s*=.*[⦃∪∩∖△ᶜ∁]/

  function sourceDefinitionName(source: string): string | null {
    return (source.match(DEF_HEAD) ?? source.match(SET_DEF_HEAD))?.[1] ?? sequenceDefinitionName(source)
  }

  /** 行が定義の行か（計算エンジンが定義の行と答えた行と、関数の定義の左辺の形の行）。定義の名前か null */
  function defNameOf(it: PlotItem): string | null {
    if (it.def) return it.def
    try {
      return sourceDefinitionName(latexToText(it.latex))
    } catch {
      return null
    }
  }

  /** ほかの行の定義（関数 f(x) = … など）。計算エンジン用の式を改行でつなぐ（計画 docs/plan-math-notation.md の M1） */
  function currentDefinitions(except: PlotItem): string {
    const out: string[] = []
    for (const it of items) {
      if (it === except || !defNameOf(it)) continue
      try {
        out.push(latexToText(it.latex))
      } catch {
        // 変換できない行は使わない
      }
    }
    return out.join('\n')
  }

  /** 定義 names が変わった・増えた・消えたとき、それを使う式と、エラーの式（未定義だった関数が定義されたかもしれない）を計算し直す */
  function defsChanged(names: string[], except?: PlotItem) {
    for (const it of items) {
      if (it === except) continue
      if (it.usedDefs?.some((n) => names.includes(n)) || it.error || running.has(it.id)) schedule(it, 0)
    }
  }

  /** 式ごとの、最後に計算した定義の行の式（中身が変わったら、それを使う式を計算し直す） */
  const lastDefSource = new Map<number, string>()

  /** 計算の結果から、この行が定義の行かを覚え、定義が変わったら使う式を計算し直す */
  function noteDefinition(item: PlotItem, name: string | null, source: string) {
    const prev = item.def ?? null
    item.def = name ?? undefined
    const changed = prev !== name || (name !== null && lastDefSource.get(item.id) !== source)
    if (name) lastDefSource.set(item.id, source)
    else lastDefSource.delete(item.id)
    if (changed) defsChanged([prev, name].filter((n): n is string => n !== null), item)
  }

  /** 範囲で重ねるパラメータ → [最小, 最大] */
  function currentStacks(): Record<string, [number, number]> {
    const out: Record<string, [number, number]> = {}
    for (const it of items) {
      if (it.param && !it.error && it.slider.stack && !(it.param.name in out)) {
        out[it.param.name] = [it.slider.min, it.slider.max]
      }
    }
    return out
  }

  /**
   * パラメータ names が変わった・増えた・消えたとき、それを使う式を計算し直す。
   * エラーの式（未定義だった文字が定義されたかもしれない）と、計算中の式（古い値で計算している）も含める
   */
  function paramsChanged(names: string[], except?: PlotItem) {
    updateSliderBounds()
    updateSpaceRangeErrors()
    for (const it of items) {
      if (it === except) continue
      const uses = it.usedParams.some((n) => names.includes(n)) || it.params.some((n) => names.includes(n))
      if (uses || it.error || running.has(it.id)) schedule(it, 0)
    }
    // 積分の図: 途中までの積分の値は画面側で計算しているので、スライダーが変わったら計算し直す
    // （矢印・今の点も、範囲の端がスライダーの式なら動く）
    for (const it of items) {
      if (it !== except && !isAux(it) && integralItem(it)) updateIntegral(it, 60)
    }
  }

  /**
   * スライダーの範囲を式（0 ≤ c ≤ b など）で決めている行の、範囲を計算し直す。
   * 範囲で重ねているパラメータの範囲が変わったら、それを使う式も計算し直す
   */
  function updateSliderBounds() {
    const all = currentParams()
    const changed: string[] = []
    for (const it of items) {
      // 自分自身は使えない（c の範囲を c で決めることはできない）
      const params = { ...all }
      if (it.param) delete params[it.param.name]
      // 刻み（Step）
      const stepExpr = it.slider.stepExpr?.trim() ?? ''
      let stepError: string | undefined
      if (stepExpr === '') it.slider.step = undefined
      else {
        const s = evalValue(stepExpr, params, updateSliderBounds)
        if (s && 'error' in s) stepError = `刻み: ${s.error}`
        else if (s && !(s.value > 0)) stepError = '刻み: 0 より大きい値にしてください'
        else if (s) it.slider.step = s.value
      }
      if (!it.slider.bounds) {
        it.sliderError = stepError
        continue
      }
      const r = evalBounds(it.slider.bounds, params, updateSliderBounds)
      if (!r) continue // 計算エンジンの準備待ち
      if ('error' in r) {
        it.sliderError = r.error
        continue
      }
      it.sliderError = stepError
      if (r.min !== it.slider.min || r.max !== it.slider.max) {
        it.slider.min = r.min
        it.slider.max = r.max
        if (it.slider.stack && it.param) changed.push(it.param.name)
      }
    }
    if (changed.length > 0) paramsChanged(changed)
  }

  /** 座標の「範囲で重ねる」範囲の式の誤り（座標の番号 → 説明）。次元の設定の欄に出す */
  let spaceRangeErrors = $state<Record<number, string>>({})

  function updateSpaceRangeErrors() {
    const params = currentParams()
    const out: Record<number, string> = {}
    space.ranges.forEach((r, k) => {
      if (!r || space.axes.includes(k)) return
      for (const side of r) {
        const e = exprError(side, params, updateSpaceRangeErrors)
        if (e === undefined) return // 計算エンジンの準備待ち
        if (e) {
          out[k] = e
          return
        }
      }
    })
    spaceRangeErrors = out
  }

  /** 「a のスライダーを追加」（次元の設定の欄から）: 一覧の一番上に a = 1 の行を足す */
  function addSliderTop(name: string) {
    items.unshift(newItem(`${latexName(name)}=1`))
    schedule(items[0], 0)
  }

  /** この行がパラメータでなくなったときの後始末 */
  function dropParam(item: PlotItem) {
    if (!item.param) return
    const name = item.param.name
    item.param = undefined
    paramsChanged([name], item)
  }

  async function replotNow(item: PlotItem, full = false) {
    if (!viewer) return
    // スライダーの値の条件の行（ab > 1）: 図形は描かず、説明だけ出す
    item.condition = isCondition(item)
    if (item.condition) {
      viewer.removePlot(item.id)
      item.error = undefined
      item.limit = undefined
      refreshConditions()
      return
    }
    const token = (latest.get(item.id) ?? 0) + 1
    latest.set(item.id, token)
    item.stale = false
    fineNext.delete(item.id)
    // 細かさに掛ける倍率。再生・ドラッグ中は持ち時間に収まる倍率。止まっているときは 1 だが、
    // 細かい計算が長くかかりそうな式は、先に粗い版（preview）を出してから細かく計算し直す
    const sizes = { res: resolution, ires: implicitResolution }
    const coarse = isCoarse()
    const preview = !coarse && !full ? budget.previewScale(item.id, sizes) : null
    const keepFine = coarse && reusable.has(item.id)
    if (keepFine) upgrading.add(item.id)
    const scale = keepFine ? 1 : coarse ? budget.scaleFor(item.id, sizes) : (preview ?? 1)
    // 止まったときに細かく計算し直す式（2段階の表示の粗い版は、続けて細かく計算するので入れない）。
    // 操作中は、形が急に変わる所を細かくしない（PLOT_NO_REFINE）ので、細かさを下げなかった式も計算し直す
    if (coarse) coarseItems.add(item.id)
    else coarseItems.delete(item.id)
    lastScale.set(item.id, scale)
    // 粗くするときも、細かすぎない下限（陽関数など 20、陰関数 16）は保つ
    const scaled = (n: number, min: number) => (scale === 1 ? n : Math.min(n, Math.max(min, Math.round(n * scale))))
    const used = { res: scaled(resolution, 20), ires: scaled(implicitResolution, 16) }

    // 数式入力欄の LaTeX を、計算エンジン用の式に変換する
    let source: string
    let ranges: ParamRanges
    let spaceRanges: Space['ranges']
    let coord: ReturnType<typeof coordRequest>
    const inputLatex = item.latex
    const initialLatex = sequenceInitialLatex(inputLatex, (name) => items.some((other) => {
      if (other === item) return false
      try {
        return sequenceDefinitionName(latexToText(other.latex)) === name
      } catch {
        return false
      }
    }))
    try {
      source = latexToText(initialLatex)
      ranges = Object.fromEntries(
        Object.entries(item.ranges).map(([k, [a, b]]) => [k, [latexToText(a), latexToText(b)] as [string, string]]),
      )
      // 座標の「範囲で重ねる」範囲（0 〜 a など）も同じように変換する
      spaceRanges = space.ranges.map((r) => (r ? [latexToText(r[0]), latexToText(r[1])] : null))
      coord = coordRequest(item)
    } catch (e) {
      viewer.removePlot(item.id)
      item.error = e instanceof Error ? e.message : String(e)
      item.info = undefined
      dropParam(item)
      return
    }

    if (source === '') {
      viewer.removePlot(item.id)
      item.error = undefined
      item.limit = undefined
      item.info = undefined
      item.usedParams = []
      dropParam(item)
      noteDefinition(item, null, '')
      return
    }

    const rotationGroup = groupIdOf(item)
    const pose = rotationIn(rotationGroup)
    const sequenceTableFlag = item.showSequenceTable ? PLOT_SEQUENCE_TABLE : 0
    const res = await requestPlot(
      item.id,
      source,
      domain,
      used.res,
      used.ires,
      ranges,
      currentParams(),
      { ...space, slice: sliceFor(item), ranges: spaceRanges, ...pose },
      currentStacks(),
      cutRequest(),
      item.complexMode ?? 'off',
      flat,
      item.walls ?? false,
      item.layers ?? 1,
      flatSlice,
      (coarse ? PLOT_NO_REFINE : findSmall ? PLOT_THOROUGH : 0) | (item.edges ? PLOT_EDGES : 0) | (coarse || !pickValues ? 0 : PLOT_POI) | sequenceTableFlag,
      hiddenRequest(item),
      item.complexCoords ?? false,
      coord,
      currentDefinitions(item),
      { count: item.field?.count ?? 0, unit: item.field?.unit ?? false, stream: item.field?.stream ?? 0 },
      odeInitFor(item),
    )
    // 計算担当が返事を送ってから受け取るまで（受け渡し）
    const transfer = res.ok ? performance.timeOrigin + performance.now() - res.timing.sentAt : 0
    if (latest.get(item.id) !== token || !items.includes(item)) return // もっと新しい依頼がある / 削除済み

    if (!res.ok) {
      viewer.removePlot(item.id)
      item.sequenceTable = undefined
      item.limit = undefined
      // 「（8文字目）」のような位置は、変換後の式での位置なので入力欄とは合わない。表示しない
      item.error = res.error.replace(/（\d+文字目）/g, '')
      item.info = undefined
      dropParam(item)
      // 誤りのある定義の行も定義の行のまま（左辺の形で決める）。定義が消えた・変わったら、使う式を計算し直す
      noteDefinition(item, source.match(DEF_HEAD)?.[1] ?? sequenceDefinitionName(source), source)
      return
    }

    item.error = undefined
    item.limit = item.latex === inputLatex ? res.limit : undefined
    if (!res.defName || !/（数列の(?:点|軌道)/.test(res.kind)) item.showSequenceTable = false
    if (item.showSequenceTable && item.latex === inputLatex) item.sequenceTable = res.sequenceTable
    item.usedParams = res.usedParams
    item.usedDefs = res.usedDefs
    item.odeStates = res.odeStates && res.odeStates.length > 0 ? res.odeStates : undefined
    noteDefinition(item, res.defName, source)
    budget.record(item.id, res.ms, used, res.cost, res.reprojected)
    if (res.reprojected) {
      reusable.add(item.id)
      upgradeMissed.delete(item.id)
    } else if (!upgrading.has(item.id) || upgradeMissed.has(item.id)) {
      reusable.delete(item.id)
      upgradeMissed.delete(item.id)
    } else upgradeMissed.add(item.id)
    upgrading.delete(item.id)

    if (res.param) {
      // パラメータの定義（a = 1 など）: グラフは描かず、値を覚えてスライダーを出す
      viewer.removePlot(item.id)
      // 同じ名前を上の行ですでに定義していたら、この行はエラーにする（使われない値のスライダーを出さない）
      const name = res.param.name
      const k = items.indexOf(item)
      if (items.slice(0, k).some((it) => it.param?.name === name && !it.error)) {
        item.error = `${prettyName(name)} は上の行ですでに定義されています`
        item.info = undefined
        dropParam(item)
        return
      }
      // 下の行に同じ名前の定義があれば、そちらをエラーにするため計算し直す（先に計算が終わっていたかもしれない）
      for (const it of items.slice(k + 1)) if (it.param?.name === name) schedule(it, 0)
      const prev = item.param
      item.param = res.param
      if (initialLatex !== inputLatex && item.latex === inputLatex) item.latex = initialLatex
      if (res.param.im !== 0) item.slider.complex = true
      // 値がスライダーの範囲の外なら、範囲を広げる（範囲の中なら、そのまま）
      const { re, im } = res.param
      const outside = (v: number) => v < item.slider.min || v > item.slider.max
      // 範囲を式で決めているときは広げない
      if (!item.slider.bounds && (outside(re) || (im !== 0 && outside(im)))) {
        const r = Math.max(1, 2 ** Math.ceil(Math.log2(Math.max(Math.abs(re), Math.abs(im)))))
        item.slider.min = Math.min(item.slider.min, -r)
        item.slider.max = Math.max(item.slider.max, r)
      }
      if (!prev || prev.name !== res.param.name || prev.re !== res.param.re || prev.im !== res.param.im) {
        paramsChanged(prev && prev.name !== res.param.name ? [prev.name, res.param.name] : [res.param.name], item)
      }
      return
    }

    dropParam(item)
    // 範囲で重ねた立体は、内側の境界面（穴など）が外から見えないので、最初は半透明にする
    // 媒介変数が4つ以上の図形も、面が何枚も重なるので同じようにする
    // 不等式の領域の霧は、中身を薄く塗るので不透明のまま
    if (/^(範囲で重ねた立体|媒介変数表示の多変数|不等式の領域)/.test(res.kind) && !/霧/.test(res.kind) && !item.solidSeen) {
      item.solidSeen = true
      if (item.opacity === 1) item.opacity = 0.45
    }
    item.params = res.mesh.params
    // 初めて使う媒介変数（t₁ など）の範囲の入力欄を用意する（計算エンジン側も 0〜2π を使っている）
    // 範囲で重ねるパラメータは、スライダーの範囲を使うので入力欄を作らない
    const stacked = currentStacks()
    for (const n of item.params) if (!(n in item.ranges) && !(n in stacked)) item.ranges[n] = ['0', '2\\pi']
    item.paramSpan = res.mesh.paramSpan
    item.kind = res.kind
    if (!res.mesh.transformOnly) {
      item.gpuAvailable = !!(res.mesh.gpu3d || res.mesh.gpu2d)
      item.gpuReason = res.mesh.gpuReason ?? ''
      // 打ち切りが多くてメッシュに回した式（Viewer が決めた）
      const fb = viewer.gpuFallbackReason(item.id, res.mesh.gpu3d?.glsl)
      if (fb && item.render !== 'gpu') {
        item.gpuAvailable = false
        item.gpuReason = fb
      }
    }
    // 見せ方が断面でない式で、新しく表示していない座標を使うようになったら、範囲を用意して計算し直す
    if (item.hidden && usesHiddenRanges(item.hidden.mode) && ensureHiddenRanges(item)) schedule(item, 0)
    const s0 = performance.now()
    // 回す前の形で作った陰関数で、回転だけが変わった: 形はそのままで、3D オブジェクトの向きだけを変える
    viewer.setMorph(item.id, coordOf(item)?.morph ?? (item.autoCoord ? 1 : 0))
    if (!res.mesh.transformOnly) viewer.setPlot(item.id, res.mesh, item)
    else viewer.dropPixelLayers(item.id) // 回した後の式で描いている光線は、向きだけが変わったらメッシュに戻す（回す前の式なら、setPlotTransform で回す）
    viewer.setPlotTransform(item.id, res.mesh.transform)
    viewer.setCap(item.id, res.cap, item)
    if (!isAux(item) && trailGroups.includes(rotationGroup) && groupIdOf(item) === rotationGroup) {
      viewer.recordRotationTrail(item.id, pose.rotation, pose.center)
    }
    recordBreakdown(item.id, { ...res.timing, transfer, setPlot: performance.now() - s0, source, kind: res.kind, reprojected: !!res.reprojected, transformOnly: !!res.mesh.transformOnly, triangles: res.mesh.indices.length / 3 })
    // 複素平面の点は値を書く（回転・表示する座標の入れ替えで、画面の位置が値と違うときは書かない）
    const plain = space.axes.every((a, k) => a === k) && space.rotation.every((a) => a === 0)
    // 漸化式の図は種類の欄で反復数や点・辺の数を示す。描画用の補間点と区切りは数えない。
    const size = /^@(cobweb|orbit|orbitpoints|recurrence|bifurcation)\s*\(/.test(source)
      ? ''
      : /^複素平面の点/.test(res.kind) && plain
      ? `(${complexPointText(res.mesh.positions)})`
      : /^複素平面の点/.test(res.kind)
      ? '点'
      : res.mesh.curve
      ? `${(res.mesh.positions.length / (res.kind.includes('（数列の点') ? 9 : 3)).toLocaleString()} 個の点`
      : `${(res.mesh.indices.length / 3).toLocaleString()} 個の三角形`
    // 回転だけを変えた（形はそのまま）なら、三角形の数は前の表示のまま
    if (res.mesh.transformOnly) {
      if (item.info) item.info = item.info.replace(/ · [\d.]+ ms/, ` · ${res.ms.toFixed(0)} ms`)
    } else item.info = [res.kind, size, `${res.ms.toFixed(0)} ms`].filter(Boolean).join(' · ')
    if (preview !== null) {
      // 2段階の表示: 粗い版を出したので、続けて細かく計算する（終わったら1回だけ差し替える）
      item.info += ' · 計算中'
      fineNext.add(item.id)
    }
    // 陽関数の高さ（頂点の縦の座標）。定数（積分の値など）ならその値を書き、表示範囲の外なら知らせる
    // 2D 表示（高さは色）・回した式・座標系の式は、頂点の縦の座標が値でないので調べない
    const upright = plain && !flat && !coordOf(item) && rotationOf(item).rotation.every((a: number) => a === 0)
    const heights = / = f\(/.test(res.kind) && !res.mesh.curve && upright ? finiteHeights(res.mesh.positions) : null
    if (heights && heights.min === heights.max) item.info += ` · 値 ${formatValue(heights.min)}`
    if (heights && res.mesh.indices.length === 0) {
      const [lo, hi] = [domain.zmin, domain.zmax]
      if (heights.max < lo || heights.min > hi) {
        item.info += ` · 値が表示範囲（${lo} 〜 ${hi}）の外なので、見えません`
        return
      }
    }
    // 陽関数の値がどこでも実数にならない（複素数の関数）なら、色分けで描けることを知らせる
    if (!res.mesh.curve && res.mesh.indices.length === 0 && !heights && / = f\(/.test(res.kind) && (item.complexMode ?? 'off') === 'off') {
      item.info +=
        ' · 値が実数になりません。⚙ の「複素数の色分け」で |f| と偏角の色で描けます。' +
        '値を複素平面の点として描くなら、x, y の代わりに媒介変数 t を使ってください（例: e^{it} → 単位円）'
    }
  }

  /** 頂点の縦の座標（3 つ目）のうち有限なものの最小・最大（無ければ null） */
  function finiteHeights(pts: Float32Array): { min: number; max: number } | null {
    let min = Infinity
    let max = -Infinity
    for (let k = 2; k < pts.length; k += 3) {
      const v = pts[k]
      if (!Number.isFinite(v)) continue
      if (v < min) min = v
      if (v > max) max = v
    }
    return min <= max ? { min, max } : null
  }

  /** 値を 6 桁で書く（負の符号は −） */
  const formatValue = (v: number) => String(Number(v.toPrecision(6))).replace('-', '−')

  /** 複素平面の点の位置を「0.28 + 0.96i」のように書く（リストで複数あれば、ちがう点を並べる） */
  function complexPointText(pts: Float32Array): string {
    const seen: string[] = []
    for (let k = 0; k + 2 < pts.length && seen.length < 4; k += 3) {
      const [re, im] = [pts[k], pts[k + 1]]
      if (!Number.isFinite(re) || !Number.isFinite(im)) continue
      const r = (v: number) => String(Number(v.toPrecision(4))).replace('-', '−')
      const s = im === 0 ? r(re) : `${r(re)} ${im < 0 ? '−' : '+'} ${r(Math.abs(im))}i`
      if (!seen.includes(s)) seen.push(s)
    }
    return seen.join(', ') || '表示範囲の外か、値が計算できません'
  }

  function replotAll(delay = 200) {
    items.forEach((it) => schedule(it, delay))
  }

  // ---- 式の一覧の操作 ----

  function addItem() {
    pushItem(newItem(''))
  }

  /** 「＋ 行列」のマス目を開いているか、指しているマス */
  let matrixPicker = $state(false)
  let matrixHover = $state<[number, number] | null>(null)

  /** 行列の定義の行 `A = (…)` を足す（名前は、使っていない大文字の最初のもの）。計画 M6 */
  async function addMatrix(rows: number, cols: number) {
    matrixPicker = false
    matrixHover = null
    const used = new Set(items.map((it) => /^\s*([A-Z])\s*=/.exec(it.latex)?.[1]).filter(Boolean))
    const name = 'ABCDEFGHIJKLMNOPQRSUVWXYZ'.split('').find((c) => !used.has(c)) ?? 'A'
    const item = newItem(`${name}=${blankMatrix(rows, cols)}`, { autofocus: true })
    pushItem(item)
    await tick()
    const added = items.find((it) => it.id === item.id)
    if (added) added.autofocus = false
  }

  /**
   * 式を複製して、すぐ下に置く（式・見た目・範囲・回転・フォルダなど、保存する設定を全部写す）。
   * 色は、元の式と見分けられるように次の色にする
   */
  function duplicateItem(item: PlotItem) {
    const { color: _color, ...saved } = $state.snapshot(savedItem(item)) as ReturnType<typeof savedItem>
    const copy = newItem(saved.latex, structuredClone(saved))
    const k = items.indexOf(item) + 1
    items.splice(k, 0, copy)
    schedule(items[k], 0)
  }

  /** エンター: その式のすぐ下に空の式を足して、そこに入力を始める */
  async function insertAfter(item: PlotItem) {
    const k = items.indexOf(item) + 1
    items.splice(k, 0, newItem('', { autofocus: true, folder: item.folder }))
    await tick()
    items[k].autofocus = false // 入力欄が作られて入力が始まったら、もう要らない
  }

  // ---- 並べ替え（左端の番号をつまんで上下にドラッグ）----

  let listEl: HTMLElement
  let dragId = $state<number | null>(null)

  function dragItem(item: PlotItem, phase: 'start' | 'move' | 'end', e: PointerEvent) {
    if (phase === 'start') {
      dragId = item.id
      return
    }
    if (dragId !== item.id) return
    if (phase === 'end') {
      dragId = null
      return
    }
    // 指（マウス）の高さが、ほかの式の真ん中より上なら、その式の前に入れる（下まで行ったら最後に入れる）。
    // 一覧には、フォルダの見出しや、折りたたまれて出ていない式もあるので、行に付けた式の番号で探す
    const from = items.indexOf(item)
    let to = items.length
    for (const row of listEl.querySelectorAll<HTMLElement>('li[data-item-id]')) {
      const r = row.getBoundingClientRect()
      if (e.clientY < r.top + r.height / 2) {
        to = items.findIndex((it) => it.id === Number(row.dataset.itemId))
        break
      }
    }
    if (to > from) to-- // 自分を抜いた後の位置
    if (to < 0 || to === from) return
    items.splice(from, 1)
    items.splice(to, 0, item)
    // フォルダ: 同じフォルダの式の隣にいる間は、そのまま。2つの式が同じフォルダなら、その間に入るとそのフォルダに入る。
    // それ以外はフォルダの外（こうすると、同じフォルダの式はいつも続けて並ぶ）
    const prev = items[to - 1]?.folder
    const next = items[to + 1]?.folder
    if (!(item.folder && (prev === item.folder || next === item.folder))) {
      item.folder = prev && prev === next ? prev : undefined
    }
  }

  // ---- フォルダ（式をまとめて折りたたむ）----

  let folders = $state<Folder[]>([])
  let nextFolderId = 1

  /** 一覧の1行: フォルダの見出しか、式 */
  type Row =
    | { kind: 'folder'; folder: Folder; members: PlotItem[] }
    | { kind: 'item'; item: PlotItem; index: number }

  /**
   * 一覧に並べる行。フォルダの見出しは、そのフォルダの最初の式の前に出す。
   * 折りたたんだフォルダの式は出さない（グラフには描く）。空のフォルダは最後に出す
   */
  const rows = $derived.by(() => {
    const out: Row[] = []
    const seen = new Set<number>()
    items.forEach((item, index) => {
      if (isAux(item)) return // 積分の範囲・積分を描くための行は、一覧に出さない
      const f = item.folder ? folders.find((f) => f.id === item.folder) : undefined
      if (f && !seen.has(f.id)) {
        seen.add(f.id)
        out.push({ kind: 'folder', folder: f, members: items.filter((it) => it.folder === f.id) })
      }
      if (!f || !f.collapsed) out.push({ kind: 'item', item, index })
    })
    for (const f of folders) if (!seen.has(f.id)) out.push({ kind: 'folder', folder: f, members: [] })
    return out
  })

  function addFolder(name = `フォルダ ${folders.length + 1}`, collapsed = false): Folder {
    folders.push({ id: nextFolderId++, name, collapsed })
    return folders[folders.length - 1]
  }

  /** 同じフォルダの式が続けて並ぶようにする（保存データを読み込んだとき）。無いフォルダの番号は消す */
  function normalizeFolders() {
    const ids = new Set(folders.map((f) => f.id))
    for (const it of items) if (it.folder && !ids.has(it.folder)) it.folder = undefined
    const out: PlotItem[] = []
    const done = new Set<number>()
    for (const it of items) {
      if (!it.folder) out.push(it)
      else if (!done.has(it.folder)) {
        done.add(it.folder)
        out.push(...items.filter((x) => x.folder === it.folder))
      }
    }
    items = out
  }

  /** 式をフォルダ fid に入れる（0 なら外に出す、'new' なら新しいフォルダを作って入れる） */
  function setItemFolder(item: PlotItem, fid: number | 'new') {
    const target = fid === 'new' ? addFolder().id : fid
    if ((item.folder ?? 0) === target) return
    const old = item.folder
    let k = items.indexOf(item)
    items.splice(k, 1)
    // 入れ先のフォルダの最後の式の後ろへ。入れ先が空（または外に出す）なら、元のフォルダの最後の式の後ろへ
    const lastOf = (f: number | undefined) => (f ? items.findLastIndex((it) => it.folder === f) : -1)
    const after = lastOf(target) >= 0 ? lastOf(target) : lastOf(old)
    if (after >= 0) k = after + 1
    items.splice(k, 0, item)
    items[k].folder = target || undefined
  }

  /** フォルダを消す。withItems なら中の式も消す。そうでなければ式は残す（フォルダの解除） */
  function removeFolder(folder: Folder, withItems: boolean) {
    const members = items.filter((it) => it.folder === folder.id)
    if (withItems) {
      if (members.length > 0 && !confirm(`フォルダ「${folder.name}」と、中の ${members.length} 個の式を削除します。よろしいですか？`)) return
      for (const it of members) removeItem(it)
    } else {
      for (const it of members) it.folder = undefined
    }
    folders.splice(folders.indexOf(folder), 1)
  }

  /** フォルダの中の式をまとめて表示・非表示にする（1つでも表示中なら、全部を非表示にする） */
  function toggleFolderVisible(members: PlotItem[]) {
    const plots = members.filter((it) => !it.param)
    const show = !plots.some((it) => it.visible)
    for (const it of plots) if (it.visible !== show) toggle(it)
  }

  function removeItem(item: PlotItem) {
    if (!items.includes(item)) return // すでに消してある（範囲を描く行は、使う式が消えると一緒に消える）
    viewer?.removePlot(item.id)
    lastBreakdown.delete(item.id)
    budget.forget(item.id)
    reusable.delete(item.id)
    upgrading.delete(item.id)
    upgradeMissed.delete(item.id)
    lastScale.delete(item.id)
    fineNext.delete(item.id)
    delete playing[-item.id]
    if (trailGroups.includes(-item.id)) trailGroups = trailGroups.filter((id) => id !== -item.id)
    items.splice(items.indexOf(item), 1)
    if (item.param) paramsChanged([item.param.name])
    if (item.def) defsChanged([item.def])
    lastDefSource.delete(item.id)
    // 積分の図を消したら、図を出すときに非表示にした元の式（積分だけの式）を表示に戻す
    if (item.integralOf !== undefined && item.integralRole === undefined) {
      const source = items.find((it) => it.id === item.integralOf)
      // 図を出すときに非表示にした式だけを表示に戻す（もとから非表示だった式は、非表示のまま）
      if (source && hiddenForIntegral.has(source.id) && !source.visible) toggle(source)
      if (source) hiddenForIntegral.delete(source.id)
      // 添えて描いていた矢印・点も消す（元の式がもう一覧から消えていても、番号で探す）
      for (const x of items.filter((it) => it.integralOf === item.integralOf && it.integralRole !== undefined)) removeItem(x)
      playingIntegral = playingIntegral.filter((x) => x !== item.integralOf)
    }
    if (!isAux(item)) {
      pruneRegionItems()
      // この式の積分を描いていたら、図と、添えて描いていた矢印・点を全部消す
      for (const x of items.filter((it) => it.integralOf === item.id)) removeItem(x)
    }
    // 念のため: 元の式が無くなった積分の図・矢印・点が残っていれば消す（どの道で消えても、グラフに残さない）
    for (const x of items.filter((it) => it.integralOf !== undefined && !items.some((s) => s.id === it.integralOf))) {
      removeItem(x)
    }
  }

  /** 「a のスライダーを追加」: その式のすぐ上に a = 1 の行を足す */
  function addSlider(item: PlotItem, name: string) {
    const k = items.indexOf(item)
    // f(…) の形で使われていれば、スライダーではなく関数の定義 f(x) = x を足す
    const line = calledAsFunction(item.latex, name) ? `${latexName(name)}\\left(x\\right)=x` : `${latexName(name)}=1`
    items.splice(k, 0, newItem(line, { folder: item.folder }))
    schedule(items[k], 0) // items[k] は Svelte の状態に入ったもの（元のオブジェクトではない）
  }

  /** スライダー・複素平面でパラメータの値を変えたとき */
  // ---- スライダーの値の条件（ab > 1 のような、パラメータだけの不等式の行）----
  //
  // 不等式だけで、値が数になる（座標を使わない）行は、図形を描かずに「条件」にする。
  // スライダーを動かして、いま成り立っている条件が成り立たなくなる値にしようとしたら、境目の手前で止める
  // （前の値と新しい値の間を二分法で詰める。どんな不等式でも使える）。非表示にした条件の行は使わない

  /** 式（LaTeX）→ 条件の並び（不等式だけの行でなければ null） */
  const conditionCache = new Map<string, ParamCondition[] | null>()
  function parsedConditions(latex: string): ParamCondition[] | null {
    if (!conditionCache.has(latex)) {
      if (conditionCache.size > 500) conditionCache.clear()
      conditionCache.set(latex, paramConditions(latex))
    }
    return conditionCache.get(latex)!
  }

  /**
   * 条件の並びの成り立ち（params の値で）。true / false、計算できない（座標を使う・準備中など）なら null
   */
  function conditionValue(conds: ParamCondition[], params: Record<string, ParamValue>, item?: PlotItem, onReady: () => void = () => {}): boolean | null {
    const defs = item ? currentDefinitions(item) : ''
    let all = true
    for (const c of conds) {
      const vals: number[] = []
      for (const part of c.parts) {
        const v = evalValue(part, params, onReady, defs)
        if (!v || !('value' in v)) return null
        vals.push(v.value)
      }
      if (!conditionHolds(c.rels, vals)) all = false
    }
    return all
  }

  /** 条件の行か（不等式だけで、今のパラメータで値が計算できる） */
  function isCondition(item: PlotItem): boolean {
    if (item.param || isAux(item)) return false
    const conds = parsedConditions(item.latex)
    // 計算エンジンの準備がまだなら、準備ができてから読み直す
    return conds !== null && conditionValue(conds, currentParams(), item, () => schedule(item, 0)) !== null
  }

  /** 使う条件の行（表示中のもの）と、その条件 */
  function activeConditions(): { item: PlotItem; conds: ParamCondition[] }[] {
    return items.flatMap((it) => (it.visible && it.condition ? [{ item: it, conds: parsedConditions(it.latex)! }] : []))
  }

  /**
   * パラメータ item を値 re にしたいとき、条件で止めた値。いま成り立っている条件だけを守る
   * （いま成り立っていない条件は、動かすのを止めない。止めると動かせなくなる）
   */
  function constrained(item: PlotItem, re: number): { value: number; clamped: boolean } {
    const name = item.param?.name
    const free = { value: re, clamped: false }
    if (!name || item.slider.complex) return free
    const conds = activeConditions()
    if (conds.length === 0) return free
    const base = currentParams()
    const at = (v: number) => ({ ...base, [name]: { re: v, im: 0, complex: false } })
    const holding = conds.filter((c) => conditionValue(c.conds, base, c.item) === true)
    if (holding.length === 0) return free
    const ok = (v: number) => holding.every((c) => conditionValue(c.conds, at(v), c.item) !== false)
    if (ok(re)) return free
    // 前の値（成り立つ）と新しい値の間を詰める
    let good = item.param!.re
    let bad = re
    if (!ok(good)) return free
    for (let k = 0; k < 50; k++) {
      const mid = (good + bad) / 2
      if (ok(mid)) good = mid
      else bad = mid
    }
    // 刻みにそろえる（そろえて成り立たなくなるなら、前の値の側の刻みにする）
    const snapped = snapToStep(good, item.slider)
    if (snapped !== good && ok(snapped)) return { value: snapped, clamped: true }
    const step = sliderStep(item.slider)
    const toward = snapped + Math.sign(item.param!.re - snapped) * step
    if (snapped !== good && ok(toward)) return { value: toward, clamped: true }
    // 刻みが自動なら、書いたときに成り立つ、いちばん短い桁数の値にする（丸めて 1 になると a < 1 が成り立たない）
    for (let d = 2; d <= 12; d++) {
      const f = 10 ** d
      const t = (good > bad ? Math.ceil(good * f) : Math.floor(good * f)) / f
      if (ok(t)) return { value: t, clamped: true }
    }
    return { value: good, clamped: true }
  }

  /** 条件の行の説明（成り立っているか）を今の値で更新する */
  function refreshConditions() {
    const ps = currentParams()
    for (const it of items) {
      if (!it.condition) continue
      const v = conditionValue(parsedConditions(it.latex)!, ps, it)
      it.kind = v === false ? '条件（今の値では成り立っていません。動かしても止めません）' : '条件（スライダーの値を、この不等式が成り立つ範囲に制限）'
      it.info = it.kind
    }
  }

  /** パラメータの行ごとの、スライダーの範囲の点（48 等分）でその値にできるか（条件の行が無ければ無い） */
  function allowedMask(item: PlotItem): boolean[] | undefined {
    const name = item.param?.name
    if (!name || item.slider.complex) return undefined
    const conds = activeConditions()
    if (conds.length === 0) return undefined
    const base = currentParams()
    const holding = conds.filter((c) => conditionValue(c.conds, base, c.item) === true)
    if (holding.length === 0) return undefined
    const n = 48
    const { min, max } = item.slider
    return Array.from({ length: n }, (_, k) => {
      const v = min + ((k + 0.5) / n) * (max - min)
      const ps = { ...base, [name]: { re: v, im: 0, complex: false } }
      return holding.every((c) => conditionValue(c.conds, ps, c.item) !== false)
    })
  }

  function setParam(item: PlotItem, re: number, im: number, fromPlay = false) {
    if (!item.param) return
    if (!fromPlay) playPos.delete(item.id) // 手で動かしたら、再生はその値から続ける
    const step = sliderStep(item.slider)
    // 刻みを決めているなら、最小値から刻みの整数倍の所にそろえる（Desmos の Step と同じ）
    re = snapToStep(re, item.slider)
    // 条件の行（ab > 1 など）が成り立たなくなる値なら、境目の手前で止める
    const limit = im === 0 ? constrained(item, re) : { value: re, clamped: false }
    re = limit.value
    if (im !== 0) im = snapToStep(im, item.slider) // 実数のときの虚部 0 はそのまま
    item.param.re = re
    item.param.im = im
    // 条件で止めた値は、丸めずに書く（丸めると境目を越えて、条件が成り立たなくなる）
    item.latex = `${latexName(item.param.name)}=${limit.clamped ? String(re) : complexLatex(re, im, step, item.slider.min)}`
    paramsChanged([item.param.name], item)
    refreshConditions()
    // 光線で描く式は、計算し直しを待たずに uniform を変える（計画 G7）
    const ps = currentParams()
    viewer?.setGpuParams(Object.fromEntries(Object.entries(ps).map(([k, v]) => [k, v.re])))
  }

  function setLatex(item: PlotItem, latex: string) {
    // 押した点の印を出している式が書き換わったら、印を消す（値が古くなる）
    if (pickedId === item.id) clearPick()
    viewer?.clearRotationTrail(item.id)
    item.latex = latex
    item.sequenceTable = undefined
    item.limit = undefined
    // 初めて出てきた範囲の名前（∬_D の D）には、範囲の欄に雛形を入れておく
    for (const r of regionsUsed(latex)) {
      if (!(r.name in regions)) regions[r.name] = r.template
    }
    setRegions(regions)
    pruneRegionItems()
    refreshInlineRegions(item)
    updateIntegral(item)
    schedule(item)
  }

  // ---- 名前で書いた積分の範囲（∬_D f dA の D）----
  /** 範囲の名前 → 範囲（LaTeX）。同じ名前は、どの式でも同じ範囲 */
  let regions = $state<Record<string, string>>({})

  /** 積分の範囲・積分を描くための行（式の一覧に出さず、保存もしない。一覧の最後に置く） */
  const isAux = (it: PlotItem) => it.regionName !== undefined || it.integralOf !== undefined

  /** 式の一覧に出す行（積分の範囲・積分を描くための行を除く） */
  const listed = () => items.filter((it) => !isAux(it))

  /** 式を一覧の最後に足す（積分の範囲を描くための行は、いつも最後に置く） */
  function pushItem(it: PlotItem) {
    const k = items.findIndex(isAux)
    if (k < 0) items.push(it)
    else items.splice(k, 0, it)
  }

  /** 範囲 name を描くための行（描いていなければ undefined） */
  const regionItem = (name: string) => items.find((it) => it.regionName === name)

  /** 積分記号の下に直接書いた範囲の、描くための行の名前（式の id と、式の中で何番目の直書きの範囲か） */
  const inlineKey = (id: number, k: number) => `#${id}:${k}`
  /** 式 item の直書きの範囲の名前（出てきた順） */
  const inlineKeys = (item: PlotItem) => inlineRegions(item.latex).map((_, k) => inlineKey(item.id, k))

  /** 範囲 name の中身（LaTeX）。名前で書いた範囲は範囲の欄、直書きの範囲は式の中の積分記号の下 */
  function regionSource(name: string): string {
    const m = /^#(\d+):(\d+)$/.exec(name)
    if (!m) return regions[name] ?? ''
    const it = items.find((x) => x.id === Number(m[1]))
    return it ? (inlineRegions(it.latex)[Number(m[2])] ?? '') : ''
  }

  /** 範囲 name をグラフに描く（半透明の面・線） */
  function showRegion(name: string) {
    if (regionItem(name)) return
    items.push(newItem(regionDisplayLatex(regionSource(name)) ?? '', { regionName: name, opacity: 0.45, vertical: false }))
    schedule(items[items.length - 1], 0)
  }

  /** 範囲の欄の「表示」ボタン: 範囲を描く・消す */
  function toggleRegion(name: string) {
    const it = regionItem(name)
    if (it) removeItem(it)
    else showRegion(name)
  }

  /** どの式も使わなくなった範囲は、描くのをやめる */
  function pruneRegionItems() {
    const used = new Set(listed().flatMap((it) => [...regionsUsed(it.latex).map((r) => r.name), ...inlineKeys(it)]))
    for (const it of items.filter((x) => x.regionName && !used.has(x.regionName))) removeItem(it)
  }

  /** 式 item の直書きの範囲を描いている行を、今の式に合わせて描き直す */
  function refreshInlineRegions(item: PlotItem) {
    for (const key of inlineKeys(item)) {
      const shown = regionItem(key)
      if (!shown) continue
      const latex = regionDisplayLatex(regionSource(key)) ?? ''
      if (latex === shown.latex) continue
      shown.latex = latex
      schedule(shown, 300)
    }
  }

  /** 範囲の欄に出す、描いている範囲の色と誤り（描いていない範囲は入れない） */
  const regionViews = $derived(
    Object.fromEntries(items.flatMap((it) => (it.regionName ? [[it.regionName, { color: it.color, error: it.error ?? (it.latex === '' ? 'この形の範囲は描けません' : undefined) }]] : []))),
  )

  /** 範囲の欄が書き換えられたとき: その範囲を使う式を全部計算し直す */
  function setRegion(name: string, latex: string) {
    regions[name] = latex
    setRegions(regions)
    const shown = regionItem(name)
    if (shown) {
      shown.latex = regionDisplayLatex(latex) ?? ''
      schedule(shown, 300)
    }
    for (const it of items) {
      if (!regionsUsed(it.latex).some((r) => r.name === name)) continue
      schedule(it, 300)
      updateIntegral(it, 300)
    }
  }

  // ---- 積分を描く（被積分関数を、積分する範囲の上に描く）----
  /** 式 item の積分を描くための行（描いていなければ undefined） */
  const integralItem = (item: PlotItem) => items.find((it) => it.integralOf === item.id && it.integralRole === undefined)
  /** 式 item の積分の図に添えて描いている行（矢印・点） */
  const integralExtras = (item: PlotItem) => items.filter((it) => it.integralOf === item.id && it.integralRole !== undefined)

  /** 添えて描くものの色 */
  const EXTRA_COLORS: Record<NonNullable<PlotItem['integralRole']>, string> = {
    path: '#3a3a40',
    field: '#6042a6',
    point: '#222222',
    tangent: '#2d70b3',
    normal: '#2d70b3',
    vector: '#c74440',
    component: '#fa7e19',
    riemann: '#2d70b3',
  }

  /** 矢印の長さ（表示範囲の一番短い辺の 1/6） */
  const arrowScale = () => Math.min(domain.xmax - domain.xmin, domain.ymax - domain.ymin, flat ? Infinity : domain.zmax - domain.zmin) / 6

  /** 積分の図の、進み具合までの積分の値（式の id → 値・誤り） */
  let integralValues = $state<Record<number, { partial?: string; value?: number; error?: string }>>({})
  /** 式ごとの、値の計算の予定・最後に計算した時刻・最後の計算にかかった時間 */
  const integralValueJobs = new Map<number, { timer?: ReturnType<typeof setTimeout>; last: number; cost: number }>()

  /**
   * 進み具合までの積分の値を計算し直す。スライダーを動かし続けている間（ドラッグ・▶ の再生）も、間をあけて計算する
   * （呼ばれるたびに予定を取り消して待ち直すと、動かしている間は一度も計算されない）。間は 80 ms か、
   * 前の計算にかかった時間の 2 倍の長いほう（重い積分で画面が止まらないように）。動かし終わった後の値も、必ず 1 回計算する
   */
  function updateIntegralValue(item: PlotItem, partial: string | undefined) {
    const job = integralValueJobs.get(item.id) ?? { last: -Infinity, cost: 0 }
    integralValueJobs.set(item.id, job)
    clearTimeout(job.timer)
    if (!partial) {
      integralValues[item.id] = {}
      return
    }
    const run = () => {
      const t0 = performance.now()
      const r = evalValue(partial, currentParams(), run, currentDefinitions(item))
      job.cost = performance.now() - t0
      if (r === null) return
      integralValues[item.id] = 'error' in r ? { partial, error: r.error } : { partial, value: r.value }
    }
    const gap = Math.max(80, 2 * job.cost)
    const wait = Math.max(0, job.last + gap - performance.now())
    job.timer = setTimeout(() => {
      job.last = performance.now()
      run()
    }, wait)
  }

  /** 積分を描くための行 view の中身を、元の式 item から作り直す（添えて描く行も、足す・消す・作り直す） */
  function fillIntegral(view: PlotItem, item: PlotItem) {
    const d = integralDisplay(item.latex, item.integralProgress ?? 1, arrowScale(), flat)
    const failed = d === null ? '積分がありません' : 'error' in d ? d.error : undefined
    const ok = d !== null && !('error' in d) ? d : null
    const field = ok?.field ?? false
    const wasField = view.hidden?.mode === 'field'
    view.latex = ok?.latex ?? ''
    view.auxNote = ok?.note
    view.auxError = failed
    // 3 重積分は、領域の中を値の色で塗る（陰関数の F の値の伝達関数）。それ以外は半透明の面
    if (field !== wasField || view.hidden === undefined) {
      view.hidden = field ? { ...DEFAULT_HIDDEN, mode: 'field', opacity: 'abs', colormap: 'turbo', density: 0.6, lit: true } : { ...DEFAULT_HIDDEN }
      view.opacity = field ? 1 : 0.55
    }
    view.auxSweep = ok?.sweep
    updateIntegralValue(item, ok?.partial)
    // 添えて描くもの（定積分の近似の図も、ここに足す）
    const want = [...(ok?.extras ?? [])]
    const rv = ok && item.riemann ? riemannDisplay(item.latex, item.riemann.method, item.riemann.n) : null
    if (rv && !('error' in rv)) want.push({ role: 'riemann', latex: rv.latex, label: rv.label })
    updateRiemannValues(item, rv)
    for (const old of integralExtras(item)) {
      if (!want.some((e) => e.role === old.integralRole)) removeItem(old)
    }
    for (const e of want) {
      let x = integralExtras(item).find((it) => it.integralRole === e.role)
      const fresh = !x
      if (!x) {
        x = newItem('', {
          integralOf: item.id,
          integralRole: e.role,
          vertical: false,
          color: EXTRA_COLORS[e.role],
          lineWidth: e.role === 'point' ? 7 : e.role === 'field' || e.role === 'path' || e.role === 'riemann' ? 3 : 5,
          // 経路・領域は、積分の図の下に薄く（曲線はくっきり、面は透かす）。近似の長方形は、積分の図と重なっても見分けられる濃さ
          opacity: e.role === 'field' ? 0.8 : e.role === 'path' ? 0.45 : e.role === 'riemann' ? 0.6 : 1,
          ranges: { t: ['0', '1'], u: ['0', '1'], v: ['0', '1'] },
        })
        items.push(x)
        x = items[items.length - 1]
      }
      x.auxNote = e.label
      if (x.latex !== e.latex || fresh) {
        x.latex = e.latex
        schedule(x, fresh ? 0 : 60)
      }
    }
  }

  /** 定積分の近似の値（式の id → 近似の値・正確な値・誤り） */
  let riemannValues = $state<Record<number, { approx?: number; exact?: number; error?: string }>>({})
  const riemannTimers = new Map<number, ReturnType<typeof setTimeout>>()

  /** 近似の値と正確な積分の値を計算し直す（n を動かし続けても固まらないよう、少し待ってから） */
  function updateRiemannValues(item: PlotItem, rv: ReturnType<typeof riemannDisplay>) {
    clearTimeout(riemannTimers.get(item.id))
    if (!rv) {
      delete riemannValues[item.id]
      return
    }
    if ('error' in rv) {
      riemannValues[item.id] = { error: rv.error }
      return
    }
    const run = () => {
      const defs = currentDefinitions(item)
      const a = evalValue(rv.sum, currentParams(), run, defs)
      const e = evalValue(rv.exact, currentParams(), run, defs)
      if (a === null || e === null) return
      const err = 'error' in a ? a.error : 'error' in e ? e.error : undefined
      riemannValues[item.id] = { approx: 'value' in a ? a.value : undefined, exact: 'value' in e ? e.value : undefined, error: err }
    }
    riemannTimers.set(item.id, setTimeout(run, 60))
  }

  /** 定積分の近似の方法と分割数を変える（method が null なら近似を消す） */
  function setRiemann(item: PlotItem, method: RiemannMethod | null, n: number) {
    item.riemann = method ? { method, n: Math.max(1, Math.min(400, Math.round(n))) } : undefined
    updateIntegral(item, 30)
  }

  /** 積分の図の進み具合を変える（スライダー・再生） */
  function setIntegralProgress(item: PlotItem, v: number) {
    item.integralProgress = Math.min(1, Math.max(0, v))
    updateIntegral(item, 30)
  }

  /** 進み具合を再生している式の id */
  let playingIntegral = $state<number[]>([])
  function togglePlayIntegral(item: PlotItem) {
    if (playingIntegral.includes(item.id)) {
      playingIntegral = playingIntegral.filter((x) => x !== item.id)
      return
    }
    if ((item.integralProgress ?? 1) >= 1) setIntegralProgress(item, 0)
    playingIntegral = [...playingIntegral, item.id]
    let last = performance.now()
    let acc = 0
    const frame = (now: number) => {
      if (!playingIntegral.includes(item.id) || !items.includes(item)) return
      const dt = Math.min(0.1, (now - last) / 1000)
      last = now
      acc += dt
      // 計算し直すのは 0.08 秒ごと（毎コマだと追いつかない）
      if (acc >= 0.08) {
        const v = Math.min(1, (item.integralProgress ?? 1) + acc / INTEGRAL_PLAY_SECONDS)
        acc = 0
        setIntegralProgress(item, v)
        if (v >= 1) {
          playingIntegral = playingIntegral.filter((x) => x !== item.id)
          return
        }
      }
      requestAnimationFrame(frame)
    }
    requestAnimationFrame(frame)
  }

  /** 式 item の積分をグラフに描く */
  function showIntegral(item: PlotItem) {
    if (integralItem(item) || isAux(item)) return
    items.push(newItem('', { integralOf: item.id, vertical: false }))
    // 添えて描く行は fillIntegral が後ろに足すので、図の行は足した直後に取っておく（最後の行ではない）
    const view = items[items.length - 1]
    fillIntegral(view, item)
    schedule(view, 0)
  }

  /** 「積分を描く」ボタン: 描く・消す */
  function toggleIntegral(item: PlotItem) {
    const view = integralItem(item)
    if (view) {
      removeItem(view)
      return
    }
    item.integralProgress = 1
    showIntegral(item)
    // 積分だけの式（値が 1 つの数）は、高さがその値の平面として描かれていて、積分の図を隠す（∫_0^{2π} sin x dx = 0 の平面は、
    // 面積の図とちょうど重なる）。図を出している間は、平面のほうを非表示にする（● でいつでも戻せる。図を消すと表示に戻す）
    if (item.visible && onlyIntegral(item)) {
      toggle(item)
      hiddenForIntegral.add(item.id)
    }
  }

  /** 積分の図を出したときに、図のほうで非表示にした元の式の id（図を消したら、これだけを表示に戻す） */
  const hiddenForIntegral = new Set<number>()

  /** 式が積分だけ（積分記号で始まる）か */
  const onlyIntegral = (item: PlotItem) => /^\s*\\(?:o?i{1,3}nt|intop|smallint)(?![A-Za-z])/.test(item.latex)

  /** 元の式・範囲の欄が書き換えられたとき: 描いている積分を作り直す（積分が無くなったら消す） */
  function updateIntegral(item: PlotItem, delay = 200) {
    const view = integralItem(item)
    if (!view) return
    if (!hasIntegral(item.latex)) {
      removeItem(view)
      return
    }
    fillIntegral(view, item)
    if (view.latex === '') viewer?.removePlot(view.id)
    schedule(view, delay)
  }

  /** 式の id → 描いている積分の色・説明・誤り（描いていない式は入れない） */
  const integralViews = $derived(
    Object.fromEntries(
      items.flatMap((it) => {
        if (it.integralOf === undefined || it.integralRole !== undefined) return []
        const source = items.find((x) => x.id === it.integralOf)
        const extras = source ? integralExtras(source) : []
        const val = source ? integralValues[source.id] : undefined
        return [
          [
            it.integralOf,
            {
              color: it.color,
              note: it.auxNote,
              error: it.auxError ?? it.error,
              sweep: it.auxSweep,
              progress: source?.integralProgress ?? 1,
              playing: source ? playingIntegral.includes(source.id) : false,
              value: val?.value,
              valueError: val?.error,
              legend: extras.map((x) => ({ color: x.color, label: x.auxNote ?? '', error: x.error })),
              riemann: (() => {
                // 1 変数の定積分だけ、近似の操作を出す
                const probe = source ? riemannDisplay(source.latex, 'left', 4) : null
                if (!probe || 'error' in probe) return undefined
                const rv = riemannValues[source!.id]
                return { method: source!.riemann?.method ?? null, n: source!.riemann?.n ?? 8, ...(source!.riemann ? rv : {}) }
              })(),
            },
          ],
        ]
      }),
    ) as Record<number, IntegralViewInfo>,
  )

  function setColor(item: PlotItem, color: string) {
    item.color = color
    viewer?.setStyle(item.id, item)
  }

  function setOpacity(item: PlotItem, opacity: number) {
    item.opacity = opacity
    viewer?.setStyle(item.id, item)
  }

  function setLineWidth(item: PlotItem, width: number) {
    item.lineWidth = width
    viewer?.setStyle(item.id, item)
  }

  function setRange(item: PlotItem, name: string, side: 0 | 1, value: string) {
    item.ranges[name][side] = value
    schedule(item, 300)
  }

  function setLayers(item: PlotItem, layers: number) {
    item.layers = Math.max(1, Math.round(layers))
    schedule(item, 0)
  }

  /** 描画の方法を変えたとき: 計算し直して、届いた結果で描き方を決め直す */
  function setRender(item: PlotItem, mode: RenderMode) {
    item.render = mode
    schedule(item, 0)
  }

  function setEdges(item: PlotItem, on: boolean) {
    item.edges = on
    schedule(item, 0)
  }

  /** 常微分方程式の初期値の欄を、計算エンジン用の式にする（変換できない欄は空 = 既定の値。誤りは計算エンジンに任せない） */
  function odeInitFor(item: PlotItem): string[] {
    return (item.ode?.init ?? []).map((s) => (s.trim() === '' ? '' : latexToText(s)))
  }

  /** 常微分方程式の初期値の欄を変えた（計画 M9） */
  function setOdeInit(item: PlotItem, k: number, latex: string) {
    const init = [...(item.ode?.init ?? [])]
    while (init.length <= k) init.push('')
    init[k] = latex
    item.ode = { init }
    schedule(item, 0)
  }

  /** ベクトル場の矢印の数・長さをそろえるか（計画 M8） */
  function setField(item: PlotItem, patch: { count?: number; unit?: boolean; stream?: number }) {
    item.field = { ...item.field, ...patch }
    schedule(item, 0)
  }

  function setWalls(item: PlotItem, on: boolean) {
    item.walls = on
    schedule(item, 0)
  }

  /** 表示していない座標の見せ方を変える。見せ方・枚数が変わったら計算し直し、色・不透明度だけなら描き方だけ変える */
  function setHidden(item: PlotItem, patch: Partial<HiddenStyle>) {
    const prev = item.hidden ?? DEFAULT_HIDDEN
    const next = { ...prev, ...patch }
    // 陰関数の F の値に切り替えたら、F = 0 の面がぼやけて見える形にする。色は式の色の明暗（ぼやけた面は F ≈ 0 の所だけなので、
    // viridis・turbo だと真ん中の 1 色になる）
    if (next.mode === 'field' && prev.mode !== 'field' && !patch.opacity) {
      next.opacity = 'zero'
      next.count = 4
      if (next.colormap === 'viridis') next.colormap = 'shade'
    }
    item.hidden = next
    if (usesHiddenRanges(next.mode)) ensureHiddenRanges(item)
    // 見せ方を切り替えたら、面の濃さをその見せ方の既定値にする（自分で動かした値は変えない）。
    // 等値面・塗り分けは重なった面が見えるように薄く、伝達関数は面の濃さを全体の不透明度に掛けるので 100%
    if (next.mode !== prev.mode && (item.opacity === 1 || item.opacity === HIDDEN_OPACITY[prev.mode])) {
      item.opacity = HIDDEN_OPACITY[next.mode]
    }
    // 不等式の領域: 境目の面は最初から半透明（45%）にしているので、霧にするときは不透明に、境目の面に戻すときは 45% に戻す
    if (/^不等式の領域/.test(item.kind ?? '')) {
      if (next.mode === 'fog' && prev.mode !== 'fog' && item.opacity === 0.45) item.opacity = 1
      if (prev.mode === 'fog' && next.mode === 'slice' && item.opacity === 1) item.opacity = 0.45
    }
    const recompute = next.mode !== prev.mode || (next.count !== prev.count && next.mode !== 'volume' && next.mode !== 'field')
    if (recompute) schedule(item, next.mode === prev.mode ? 150 : 0)
    else viewer?.setStyle(item.id, item)
  }

  /** この式の断面の値（式だけの値があればそれ、無ければ次元の設定の値） */
  function sliceFor(item: PlotItem): number[] {
    return space.slice.map((v, k) => item.slice?.[coordName(k)] ?? v)
  }

  /** この式だけの断面の値を決める（null なら、次元の設定の値に戻す） */
  function setItemSlice(item: PlotItem, name: string, v: number | null) {
    const next = { ...(item.slice ?? {}) }
    if (v === null) delete next[name]
    else next[name] = v
    item.slice = next
    schedule(item, v === null ? 0 : 30)
  }

  /** 座標を複素数にする・やめる。する場合、次元が 6 未満なら 6 にする（虚部に w, w₂, w₃ を使う） */
  function setComplexCoords(item: PlotItem, on: boolean) {
    item.complexCoords = on
    if (on && space.dims < 6) setLayout(resizeSpace(space, 6))
    schedule(item, 0)
  }

  /** 見せ方ごとの面の濃さの既定値 */
  const HIDDEN_OPACITY: Record<HiddenStyle['mode'], number> = { slice: 1, layers: 0.35, planes: 0.6, volume: 1, stack: 0.45, field: 1, fog: 1 }

  /** 表示していない座標の範囲の欄を使う見せ方か（断面・陰関数の F の値・不等式の領域の霧は、断面の値を使う） */
  const usesHiddenRanges = (mode: HiddenStyle['mode']) => mode !== 'slice' && mode !== 'field' && mode !== 'fog'

  /** 見せ方が断面でないとき、式が使っている表示していない座標の範囲の入力欄を用意する（断面のスライダーの範囲）。用意したら true */
  function ensureHiddenRanges(item: PlotItem): boolean {
    let added = false
    // 1 つ目の座標だけ断面のスライダーの範囲にし、ほかは今の断面の値に固定する（両端が同じ）。全部を動かすと、組み合わせの数だけ
    // 図形が増えて重く、見たい形も分からなくなる（6 次元の (x + iw)² + (y + iw₂)² + (z + iw₃)² = 1 で z・w₂・w₃ の 64 通り）
    let first = !Object.keys(item.ranges).some((n) => {
      const k = Array.from({ length: space.dims }, (_, j) => j).find((j) => coordName(j) === n)
      return k !== undefined && !space.axes.includes(k)
    })
    for (let k = 0; k < space.dims; k++) {
      const name = coordName(k)
      if (space.axes.includes(k) || !item.usedParams.includes(name) || name in item.ranges) continue
      if (first) item.ranges[name] = [String(space.sliceMin), String(space.sliceMax)]
      else item.ranges[name] = [String(space.slice[k] ?? 0), String(space.slice[k] ?? 0)]
      first = false
      added = true
    }
    return added
  }

  /** 計算担当に渡す見せ方（断面なら null） */
  function hiddenRequest(item: PlotItem): { mode: number; count: number } | null {
    const h = item.hidden
    if (!h || h.mode === 'slice') return null
    return { mode: HIDDEN_MODE_ORDER.indexOf(h.mode), count: h.count }
  }

  function setComplexMode(item: PlotItem, mode: ComplexMode) {
    item.complexMode = mode
    schedule(item, 0)
  }

  function setGrid(item: PlotItem, grid: GridStyle) {
    item.grid = grid
    viewer?.setStyle(item.id, item)
  }

  function toggle(item: PlotItem) {
    item.visible = !item.visible
    viewer?.setStyle(item.id, item)
    // 非表示の間に変わっていたら、表示したときに計算し直す
    if (item.visible && item.stale) schedule(item, 0)
  }

  // ---- 表示範囲 ----

  const axes = ['x', 'y', 'z'] as const

  // ---- 次元（4次元以上）----

  /** 4次元の遠近感の標準（%） */
  const PERSPECTIVE_4D = 40
  let space = $state<Space>({
    dims: 3,
    axes: [0, 1, 2],
    slice: [0, 0, 0],
    sliceMin: -5,
    sliceMax: 5,
    ranges: [null, null, null],
    samples: 24,
    rotation: Array(10).fill(0), // 5次元の回転（5C2 = 10 通り）
    center: Array(5).fill(0),
    perspective: PERSPECTIVE_4D,
  })
  /** 画面の横・奥・縦の軸に表示している座標の名前 */
  const axisNames = $derived(space.axes.map(coordName))

  // 数式キーボードの「変数」のタブに、今の次元の座標を並べる
  $effect(() => setKeyboardDims(space.dims))

  $effect(() => {
    viewer?.setAxisNames(axisNames.map(prettyName))
  })

  function setLayout(next: Space) {
    if (next.dims !== space.dims || next.axes.some((axis, i) => axis !== space.axes[i])) viewer?.clearRotationTrails()
    // 次元が変わったら、グループの回転も同じ座標の組の角度を引き継いで作り直す
    if (next.dims !== space.dims) {
      resizeRotations(space.dims, next.dims)
    }
    space = next
    updateSpaceRangeErrors()
    replotAll(0)
  }

  // ---- 回転（式ごとの回転・複数の式の同期）----
  //
  // 標準では、式ごとに自分だけの回転（item.rotation, item.center）で描く。
  // 「全体と同じ」にした式は全体の回転（space.rotation, space.center）で描く。
  // 同じグループの式は、そのグループの回転で描く（角度を変えると、まとめて回る）。
  // 回転の番号（gid）: 0 は全体、正の数はグループ、負の数 −id は式 id だけの回転

  let groups = $state<RotationGroup[]>([])
  let nextGroupId = 1

  /** 式が使う回転の番号（この式だけなら −id。入っていたグループが無くなっていれば全体） */
  function groupIdOf(item: PlotItem): number {
    const g = item.rotGroup
    if (g === undefined) return -item.id
    return g && groups.some((x) => x.id === g) ? g : GLOBAL_GROUP
  }

  /** 回転角が全部 0、中心が原点 */
  function zeroRotation(dims = space.dims) {
    return { rotation: pairs(rotDims(dims)).map(() => 0), center: Array(rotDims(dims)).fill(0) }
  }

  /** 回転 gid の回転角と中心 */
  function rotationIn(gid: number): { rotation: number[]; center: number[] } {
    if (gid < 0) {
      const it = items.find((x) => x.id === -gid)
      const zero = zeroRotation()
      return { rotation: it?.rotation ?? zero.rotation, center: it?.center ?? zero.center }
    }
    const g = groups.find((x) => x.id === gid)
    return g ? { rotation: g.rotation, center: g.center } : { rotation: space.rotation, center: space.center }
  }

  /** 次元が変わったとき、グループと式ごとの回転を、同じ座標の組の角度を引き継いで作り直す */
  function resizeRotations(from: number, to: number) {
    groups = groups.map((g) => ({ ...g, ...resizeRotation(g.rotation, g.center, from, to) }))
    for (const it of items) {
      if (!it.rotation) continue
      const r = resizeRotation(it.rotation, it.center ?? [], from, to)
      it.rotation = r.rotation
      it.center = r.center
    }
  }

  /** 式を描くときの回転角と中心 */
  function rotationOf(item: PlotItem) {
    return rotationIn(groupIdOf(item))
  }

  /** 回転 gid を変える。その回転で描く式だけを計算し直す */
  function setRotation(gid: number, rotation: number[], center: number[]) {
    if (gid < 0) {
      const it = items.find((x) => x.id === -gid)
      if (!it) return
      it.rotation = rotation
      it.center = center
      if (!it.param) schedule(it, 0)
      return
    }
    const g = groups.find((x) => x.id === gid)
    if (g) {
      g.rotation = rotation
      g.center = center
    } else {
      space.rotation = rotation
      space.center = center
    }
    for (const it of items) if (!it.param && groupIdOf(it) === (g ? gid : GLOBAL_GROUP)) schedule(it, 0)
  }

  /** 新しいグループを作る。回転は from（グループの番号）の今の回転を引き継ぐ */
  function createGroup(from: number): number {
    const { rotation, center } = rotationIn(from)
    const used = new Set(groups.map((g) => g.name))
    let n = 1
    while (used.has(`グループ ${n}`)) n++
    const g: RotationGroup = { id: nextGroupId++, name: `グループ ${n}`, rotation: [...rotation], center: [...center] }
    groups.push(g)
    return g.id
  }

  /**
   * 式をグループ gid に入れる（'new' なら新しいグループを作って入れる、OWN_GROUP ならこの式だけの回転にする）。
   * 新しいグループ・この式だけの回転は、今の回転を引き継ぐ
   */
  function setItemGroup(item: PlotItem, gid: number | 'new') {
    const from = groupIdOf(item)
    if (gid === OWN_GROUP) {
      if (from < 0) return
      const { rotation, center } = rotationIn(from)
      item.rotation = [...rotation]
      item.center = [...center]
      item.rotGroup = undefined
    } else {
      const to = gid === 'new' ? createGroup(from) : gid
      if (to === from) return
      item.rotGroup = to
    }
    if (from < 0) delete playing[from] // この式だけの回転の再生は、ほかの回転に移ったら止める
    if (from < 0) trailGroups = trailGroups.filter((id) => id !== from)
    viewer?.clearRotationTrail(item.id)
    schedule(item, 0)
  }

  function renameGroup(gid: number, name: string) {
    const g = groups.find((x) => x.id === gid)
    if (g && name.trim()) g.name = name.trim()
  }

  /** グループを解散する（入っていた式は、グループの今の回転を引き継いで、その式だけの回転になる） */
  function removeGroup(gid: number) {
    for (const it of items) {
      if (it.rotGroup === gid) setItemGroup(it, OWN_GROUP)
    }
    groups = groups.filter((g) => g.id !== gid)
    delete playing[gid]
    trailGroups = trailGroups.filter((id) => id !== gid)
    if (rotTab === gid) rotTab = GLOBAL_GROUP
    animationChanged()
  }

  /** グループごとの式の数 */
  const groupSizes = $derived.by(() => {
    const out: Record<number, number> = {}
    for (const it of items) if (it.rotGroup) out[it.rotGroup] = (out[it.rotGroup] ?? 0) + 1
    return out
  })

  /** グラフを描く式（パラメータの行でないもの） */
  const plotItems = $derived(items.filter((it) => !it.param && !isAux(it)))

  // ---- 座標系（計画 9 章の S37） ----

  let coordSystems = $state<CoordSystem[]>([])
  let nextCoordId = 1
  /** 座標系の設定で表示している座標系の番号 */
  let coordTab = $state(0)
  /** 移行を再生している座標系の番号 */
  let playingMorph = $state<number[]>([])
  /** 再生中の移行の向き（+1 で 1 へ、−1 で 0 へ） */
  const morphDir = new Map<number, number>()

  /** 式が使う座標系（選んでいない・消えていれば undefined） */
  function coordOf(item: PlotItem): CoordSystem | undefined {
    return item.coordSys === undefined ? undefined : coordSystems.find((c) => c.id === item.coordSys)
  }

  /**
   * 座標系を選んでいない式が θ・φ・ρ を使っていれば、自動で使う座標系（2D は極座標、3D は円柱座標・球座標）。
   * 同じ名前のパラメータ・定義があれば使わない
   */
  function autoCoordOf(item: PlotItem): AutoCoord | null {
    if (item.coordSys !== undefined && coordOf(item)) return null
    if (item.param) return null
    let text: string
    try {
      text = latexToText(item.latex)
    } catch {
      return null
    }
    const defined = new Set<string>(Object.keys(currentParams()))
    for (const it of items) {
      if (it === item) continue
      const d = defNameOf(it)
      if (d) defined.add(d)
      if (it.param) defined.add(it.param.name)
    }
    return autoCoord(text, flat, defined)
  }

  /** 自動の座標系の角度（theta・phi）の範囲（LaTeX。書き換えていなければ、式から決めた初期値） */
  function angleRangeOf(item: PlotItem, angle: string): [string, string] {
    const own = item.angleRanges?.[angle]
    if (own) return own
    let text = ''
    try {
      text = latexToText(item.latex)
    } catch {
      // 初期値は 0〜2π
    }
    return defaultAngleRange(text, angle)
  }

  /** 自動の座標系の角度の範囲を書き換えた */
  function setAngleRange(item: PlotItem, angle: string, side: 0 | 1, value: string) {
    const r = [...angleRangeOf(item, angle)] as [string, string]
    r[side] = value
    item.angleRanges = { ...(item.angleRanges ?? {}), [angle]: r }
    schedule(item, 300)
  }

  /** 計算担当に渡す座標系（計算エンジン用の式に変換する。LaTeX の誤りは投げる） */
  function coordRequest(item: PlotItem): { map: string; ranges: [string, string][]; names: string } | null {
    const c = coordOf(item)
    if (!c) {
      const a = autoCoordOf(item)
      item.autoCoord = a?.kind
      if (!a) return null
      const axes: [number, number][] = [
        [domain.xmin, domain.xmax],
        [domain.ymin, domain.ymax],
        [domain.zmin, domain.zmax],
      ]
      const angle = (name: string): [string, string] => {
        const [lo, hi] = angleRangeOf(item, name)
        const t = (v: string, side: string) => {
          const s = latexToText(v)
          if (s.trim() === '') throw new Error(`${GREEK_NAMES[name] ?? name} の${side}が空です`)
          return s
        }
        return [t(lo, '最小値'), t(hi, '最大値')]
      }
      const third: [string, string] = a.kind === 'sphere' ? angle('phi') : [String(domain.zmin), String(domain.zmax)]
      return { map: a.map, ranges: [autoRadius(a.kind, axes, a.signedRadius), angle('theta'), third], names: a.names.join(',') }
    }
    item.autoCoord = undefined
    const text = (latex: string, what: string) => {
      const t = latexToText(latex)
      if (t.trim() === '') throw new Error(`座標系「${c.name}」の${what}が空です`)
      return t
    }
    return {
      map: `(${c.map.map((m, k) => text(m, ` ${'XYZ'[k]} の式`)).join(', ')})`,
      ranges: coordRanges(c).map(([a, b], k) => [text(a, ` ${'xyz'[k]} の範囲`), text(b, ` ${'xyz'[k]} の範囲`)] as [string, string]),
      names: coordNames(c),
    }
  }

  /** 座標系の、描くときに使う範囲（表示範囲に合わせる座標は、今の表示範囲） */
  function coordRanges(c: CoordSystem): CoordSystem['ranges'] {
    return effectiveRanges(c, [
      [domain.xmin, domain.xmax],
      [domain.ymin, domain.ymax],
      [domain.zmin, domain.zmax],
    ])
  }

  /**
   * 座標の範囲を表示範囲に合わせるかを変える。合わせている間も、書いた範囲（c.ranges）は書き換えないので、
   * やめたときは合わせる前に書いた範囲に戻る（前は、やめたときの表示範囲で上書きして、元の範囲に戻せなかった）
   */
  function setCoordFollow(cid: number, k: number, on: boolean) {
    const c = coordSystems.find((x) => x.id === cid)
    if (!c) return
    const follow = [...(c.follow ?? [false, false, false])] as [boolean, boolean, boolean]
    follow[k] = on
    c.follow = follow
    for (const it of items) if (it.coordSys === cid) schedule(it)
  }

  /**
   * 計算担当に渡す座標の名前（カンマ区切り。計画 S38）。名前が無い・x, y, z のままなら空。
   * 空の名前の座標は x, y, z の名前のまま。同じ名前が 2 つあればエラー
   */
  function coordNames(c: CoordSystem): string {
    const n = (c.names ?? ['x', 'y', 'z']).map((s, k) => engineName(s) ?? 'xyz'[k])
    if (n.join(',') === 'x,y,z') return ''
    if (new Set(n).size < 3) throw new Error(`座標系「${c.name}」の座標の名前が重なっています`)
    // 計算エンジンは英字の並びを 1 文字ずつの変数に分けるので、名前は英字 1 文字か、1 つの名前として読むギリシャ文字（e と i は定数）
    const ok = (s: string) => s in GREEK_NAMES || (/^[A-Za-z]$/.test(s) && s !== 'e' && s !== 'i')
    if (!n.every(ok)) throw new Error(`座標系「${c.name}」の座標の名前は、英字 1 文字（e・i 以外）か θ・φ・ρ・σ・τ・μ・ν にしてください`)
    return n.join(',')
  }

  /** 座標の名前を変えた: その座標系の式を計算し直す */
  function setCoordNames(cid: number, names: [string, string, string]) {
    const c = coordSystems.find((x) => x.id === cid)
    if (!c) return
    c.names = names
    for (const it of items) if (it.coordSys === cid) schedule(it)
  }

  /** 座標系を作る（用意した座標系から。名前は重ならないようにする）。番号を返す */
  function createCoord(preset: CoordSystem['preset']): number {
    const p = COORD_PRESETS[preset]
    const used = new Set(coordSystems.map((c) => c.name))
    let name = p.name
    for (let k = 2; used.has(name); k++) name = `${p.name} ${k}`
    const c: CoordSystem = {
      id: nextCoordId++,
      name,
      preset,
      map: [...p.map],
      names: [...p.names],
      ranges: p.ranges.map((r) => [...r]) as CoordSystem['ranges'],
      // 長さの座標（直交・半径・高さ）は、最初は表示範囲に合わせる（角度などは用意した範囲のまま）
      follow: presetFollow(preset),
      grid: { on: true, faces: 4 },
      morph: 1,
    }
    coordSystems.push(c)
    return c.id
  }

  /**
   * 行列の定義の行（2×2・3×3）を、座標系 T(q) = A q として使う（計画 M7 の線形変換）。同じ行列の座標系がもうあれば、それを開く。
   * 成分は A_{i,j} の形で書くので、行列の成分（スライダーの文字を含む）を変えると座標系も動く
   */
  function useMatrixAsCoord(item: PlotItem) {
    const m = matrixDefinition(item.latex)
    if (!m) return
    const { name, size } = m
    const row = (i: number) => Array.from({ length: size }, (_, j) => `${name}_{${i + 1},${j + 1}}${'xyz'[j]}`).join('+')
    const map: CoordSystem['map'] = [row(0), row(1), size === 3 ? row(2) : 'z']
    const label = `行列 ${latexToText(name).replace(/[{}]/g, '').replace(/^vec_/, '')}`
    let c = coordSystems.find((x) => x.preset === 'custom' && x.map.join('|') === map.join('|'))
    if (!c) {
      c = {
        id: nextCoordId++,
        name: label,
        preset: 'custom',
        map,
        names: ['x', 'y', 'z'],
        ranges: [['-5', '5'], ['-5', '5'], ['-5', '5']],
        follow: [true, true, true],
        grid: { on: true, faces: 4 },
        morph: 1,
      }
      coordSystems.push(c)
    }
    openCoord(c.id)
  }

  /** 座標系の式・範囲を変えた: その座標系の式を計算し直す */
  function setCoordShape(cid: number, map: CoordSystem['map'], ranges: CoordSystem['ranges']) {
    const c = coordSystems.find((x) => x.id === cid)
    if (!c) return
    c.map = map
    c.ranges = ranges
    for (const it of items) if (it.coordSys === cid) schedule(it)
  }

  /** 移行のスライダー: 位置を混ぜ直すだけ（計算し直さない） */
  function setMorph(cid: number, s: number) {
    const c = coordSystems.find((x) => x.id === cid)
    if (!c) return
    c.morph = Math.min(1, Math.max(0, s))
    for (const it of items) if (it.coordSys === cid) viewer?.setMorph(it.id, c.morph)
    viewer?.setCoordGridMorph(cid, c.morph)
  }

  function renameCoord(cid: number, name: string) {
    const c = coordSystems.find((x) => x.id === cid)
    if (c && name.trim() !== '') c.name = name.trim()
  }

  /** 座標系を消す: 使っていた式は直交座標に戻す */
  function removeCoord(cid: number) {
    coordSystems = coordSystems.filter((c) => c.id !== cid)
    playingMorph = playingMorph.filter((x) => x !== cid)
    for (const it of items) {
      if (it.coordSys !== cid) continue
      it.coordSys = undefined
      schedule(it, 0)
    }
  }

  /** 式の座標系を選ぶ（undefined なら直交座標。'new' なら球座標を作って選ぶ） */
  function setItemCoord(item: PlotItem, cid: number | 'new' | undefined) {
    const id = cid === 'new' ? createCoord('sphere') : cid
    if (item.coordSys === id) return
    item.coordSys = id
    schedule(item, 0)
  }

  // ---- 座標系の格子（座標曲線。計画 S39） ----

  /** 格子を描いている座標系の番号と、最新の依頼の番号（古い返事を捨てる） */
  const gridShown = new Set<number>()
  const gridToken = new Map<number, number>()
  let gridTimer: ReturnType<typeof setTimeout> | undefined

  /** 格子の設定を変える */
  function setCoordGrid(cid: number, grid: { on: boolean; faces: number }) {
    const c = coordSystems.find((x) => x.id === cid)
    if (c) c.grid = grid
  }

  /** 座標系を使う、表示中の式がある座標系の格子を頼み直す（それ以外の座標系の格子は消す） */
  function refreshCoordGrids() {
    if (!viewer) return
    const params = currentParams()
    const wanted = new Set<number>()
    for (const c of coordSystems) {
      const grid = c.grid ?? { on: true, faces: 4 }
      if (!grid.on || !items.some((it) => it.visible && !it.param && it.coordSys === c.id)) continue
      let req: ReturnType<typeof coordRequest>
      try {
        req = coordRequest({ coordSys: c.id } as PlotItem)
      } catch {
        continue // 式の誤りは、式の行に出る
      }
      if (!req) continue
      wanted.add(c.id)
      const token = (gridToken.get(c.id) ?? 0) + 1
      gridToken.set(c.id, token)
      requestCoordGrid({ ...req, params, lines: 8, faces: grid.faces, flat, definitions: currentDefinitions({} as PlotItem) }).then((res) => {
        if (gridToken.get(c.id) !== token) return
        const now = coordSystems.find((x) => x.id === c.id)
        if (res.ok && now) {
          viewer?.setCoordGrid(c.id, res.positions, res.target, now.morph)
          gridShown.add(c.id)
        } else {
          viewer?.setCoordGrid(c.id, null, null, 0)
          gridShown.delete(c.id)
        }
      })
    }
    for (const id of [...gridShown]) {
      if (wanted.has(id)) continue
      gridToken.set(id, (gridToken.get(id) ?? 0) + 1)
      viewer?.setCoordGrid(id, null, null, 0)
      gridShown.delete(id)
    }
  }

  // 座標系（移行の値を除く）・式の座標系と表示・パラメータ・2D 表示が変わったら、少し待ってから格子を頼み直す
  $effect(() => {
    const key = JSON.stringify([
      coordSystems.map((c) => [c.id, c.map, coordRanges(c), c.names, c.grid]),
      items.map((it) => [it.coordSys, it.visible, !!it.param, it.param?.name, it.param?.re, it.param?.im, it.def ? it.latex : '']),
      flat,
    ])
    void key
    clearTimeout(gridTimer)
    gridTimer = setTimeout(refreshCoordGrids, 150)
  })

  /** 式の ⚙ の「座標系」から、その座標系の設定を開く */
  function openCoord(cid: number | undefined) {
    if (cid !== undefined) coordTab = cid
    settingsCat = 'coords'
    settingsOpen = true
  }

  /**
   * 移行の再生: 1 まで動かして止まる（1 にいるときは 0 まで。端から端まで PARAM_PLAY_SECONDS 秒）。
   * 計算し直さないので、操作中の粗い計算にはしない
   */
  function togglePlayMorph(cid: number) {
    if (playingMorph.includes(cid)) {
      playingMorph = playingMorph.filter((x) => x !== cid)
      return
    }
    const c = coordSystems.find((x) => x.id === cid)
    morphDir.set(cid, c && c.morph >= 1 ? -1 : 1)
    playingMorph = [...playingMorph, cid]
    if (playingMorph.length === 1) {
      let last = performance.now()
      const frame = (now: number) => {
        if (playingMorph.length === 0) return
        const dt = Math.min(0.1, (now - last) / 1000)
        last = now
        for (const id of playingMorph) {
          const c = coordSystems.find((x) => x.id === id)
          if (!c) continue
          const dir = morphDir.get(id) ?? 1
          const v = Math.min(1, Math.max(0, c.morph + (dir * dt) / PARAM_PLAY_SECONDS))
          setMorph(id, v)
          // 端（最大値・最小値）まで来たら止まる
          if ((dir > 0 && v >= 1) || (dir < 0 && v <= 0)) playingMorph = playingMorph.filter((x) => x !== id)
        }
        if (playingMorph.length > 0) requestAnimationFrame(frame)
      }
      requestAnimationFrame(frame)
    }
  }

  // ---- 回転の再生（角度を自動で動かす）----

  /** 再生中の回転（グループの番号 → 回転角の番号） */
  let playing = $state<Record<number, number[]>>({})
  let trailGroups = $state<number[]>([])

  function clearTrail(gid: number) {
    for (const item of items) if (groupIdOf(item) === gid) viewer?.clearRotationTrail(item.id)
  }

  function setTrail(gid: number, enabled: boolean) {
    if (enabled) {
      if (trailGroups.includes(gid)) return
      trailGroups = [...trailGroups, gid]
      const { rotation, center } = rotationIn(gid)
      for (const item of items) {
        if (!isAux(item) && groupIdOf(item) === gid) viewer?.recordRotationTrail(item.id, rotation, center)
      }
    } else {
      trailGroups = trailGroups.filter((id) => id !== gid)
      clearTrail(gid)
    }
  }
  /** 再生の速さ（度 / 秒） */
  const PLAY_SPEED = 30
  let lastFrame = 0

  const anyRotPlaying = () => Object.values(playing).some((ks) => ks.length > 0)

  function togglePlay(gid: number, k: number) {
    const cur = playing[gid] ?? []
    playing[gid] = cur.includes(k) ? cur.filter((j) => j !== k) : [...cur, k]
    animationChanged()
  }

  /** 回転の再生を止める（リセットしたとき） */
  function stopPlay(gid: number, ks: number[]) {
    playing[gid] = (playing[gid] ?? []).filter((j) => !ks.includes(j))
    animationChanged()
  }

  /** そのグループの回転で描いている、表示中のグラフがあるか（無ければ回転の再生は意味が無いので止める） */
  function groupHasVisible(gid: number): boolean {
    return items.some((it) => !it.param && it.visible && it.latex.trim() !== '' && groupIdOf(it) === gid)
  }

  // ---- 次元の断面の値の再生（w, w₂ などのスライダーを自動で動かす）----

  /** 断面の値を自動で動かしている座標の番号 */
  let playingSlices = $state<number[]>([])
  /** 再生中の断面の値（刻みにそろえる前の値） */
  const slicePos = new Map<number, number>()

  function togglePlaySlice(k: number) {
    slicePos.delete(k)
    playingSlices = playingSlices.includes(k) ? playingSlices.filter((j) => j !== k) : [...playingSlices, k]
    animationChanged()
  }

  function stopSlice(k: number) {
    if (!playingSlices.includes(k)) return
    playingSlices = playingSlices.filter((j) => j !== k)
    animationChanged()
  }

  /** 座標 k が今も「固定」（断面）か。軸や範囲にしたり、次元を減らしたりしたら再生をやめる */
  const isFixedCoord = (k: number) => k < space.dims && !space.axes.includes(k) && !space.ranges[k]

  // ---- パラメータの再生（スライダーの値を自動で動かす）----

  /** 再生中のパラメータの行（式の id） */
  let playingParams = $state<number[]>([])
  /** パラメータの再生: 最小値から最大値まで 4 秒で動かし、最大値に来たら最小値に戻る */
  const PARAM_PLAY_SECONDS = 4
  /** 積分の図の進み具合を、0 から 1 まで再生する秒数 */
  const INTEGRAL_PLAY_SECONDS = 6

  /** 再生中のパラメータの、刻みにそろえる前の値（行の id → 値） */
  const playPos = new Map<number, number>()

  function togglePlayParam(item: PlotItem) {
    playPos.delete(item.id)
    playingParams = playingParams.includes(item.id)
      ? playingParams.filter((id) => id !== item.id)
      : [...playingParams, item.id]
    animationChanged()
  }

  let animating = false
  function animationChanged() {
    const any = anyRotPlaying() || playingParams.length > 0 || playingSlices.length > 0
    if (any && !animating) {
      animating = true
      viewer?.setBusy(true)
      lastFrame = performance.now()
      requestAnimationFrame(playFrame)
    } else if (!any) {
      refine() // 再生を止めたら、細かく計算し直す
    }
  }

  function playFrame(now: number) {
    // グラフを消した・非表示にした回転のグループは、再生を止める
    for (const [key, ks] of Object.entries(playing)) {
      if (ks.length > 0 && !groupHasVisible(Number(key))) playing[Number(key)] = []
    }
    playingSlices = playingSlices.filter(isFixedCoord)
    if (!anyRotPlaying() && playingParams.length === 0 && playingSlices.length === 0) {
      animating = false
      refine() // 止まったら、細かく計算し直す
      return
    }
    const dt = Math.min(0.1, (now - lastFrame) / 1000)
    lastFrame = now
    for (const [key, ks] of Object.entries(playing)) {
      if (ks.length === 0) continue
      const gid = Number(key)
      const cur = rotationIn(gid)
      const rotation = [...cur.rotation]
      for (const k of ks) {
        // −180〜180 の範囲で回り続ける（小数第1位まで）
        const a = rotation[k] + PLAY_SPEED * dt
        rotation[k] = Math.round((((a + 180) % 360) - 180) * 10) / 10
      }
      setRotation(gid, rotation, cur.center)
    }
    // 断面の値: 最小値から最大値まで 4 秒で動かし、最大値に来たら最小値に戻る
    for (const k of playingSlices) {
      const { sliceMin: min, sliceMax: max } = space
      if (!(min < max)) continue
      let v = (slicePos.get(k) ?? space.slice[k]) + ((max - min) / PARAM_PLAY_SECONDS) * dt
      if (v > max || v < min) v = min
      slicePos.set(k, v)
      setSlice(k, Number(v.toPrecision(6)))
    }
    for (const id of playingParams) {
      const it = items.find((x) => x.id === id)
      if (!it || it.error) {
        playingParams = playingParams.filter((x) => x !== id) // 消された・パラメータでなくなった
        continue
      }
      if (!it.param) continue // まだ計算が終わっていない
      const { min, max } = it.slider
      if (!(min < max)) continue
      // 刻みにそろえる前の値で進める（刻みが粗いと、そろえた値からでは1コマで次の刻みまで進めない）
      let v = (playPos.get(id) ?? it.param.re) + ((max - min) / PARAM_PLAY_SECONDS) * dt
      if (v > max) v = min // 最後まで行ったら最初から
      playPos.set(id, v)
      setParam(it, v, 0, true)
    }
    requestAnimationFrame(playFrame)
  }

  // ---- ベンチマーク（?bench=1。計画 S0 の 6。lib/bench.ts） ----

  const BENCH = new URLSearchParams(location.search).get('bench') === '1'
  /** ベンチマークの進み具合と結果（画面の左上に出す） */
  let benchText = $state('')

  /** 計算が全部終わり、0.6 秒たっても次の依頼が無くなるまで待つ */
  async function benchIdle() {
    const until = performance.now() + 180_000
    while (performance.now() < until) {
      await new Promise((r) => setTimeout(r, 100))
      const busy = running.size > 0 || again.size > 0 || fineNext.size > 0 || animating
      if (!busy && performance.now() - lastPlotActivity > 600) return
    }
  }

  async function startBench(v: Viewer) {
    // ?bench=1&set=samples: サンプルを全部、ふつうに追加したときと同じ道で描いて測る
    if (new URLSearchParams(location.search).get('set') === 'samples') {
      const { runSampleSweep } = await import('./lib/bench')
      const stopAll = () => {
        for (const key of Object.keys(playing)) playing[Number(key)] = []
        playingParams = []
        playingSlices = []
        playingMorph = []
        animationChanged()
      }
      await runSampleSweep(
        {
          viewer: v,
          idle: benchIdle,
          stopAll,
          animating: () => anyRotPlaying() || playingParams.length > 0 || playingSlices.length > 0 || playingMorph.length > 0,
          errors: () => items.filter((it) => it.error).map((it) => ({ latex: it.latex, error: it.error! })),
          async load(s) {
            stopAll()
            for (const it of [...items]) removeItem(it)
            folders = []
            for (const c of [...coordSystems]) removeCoord(c.id)
            if (flat) setFlat(false)
            resolution = 150
            implicitResolution = 80
            applyView({ dims: 3, half: 5 })
            await benchIdle()
            pushItem(newItem(''))
            await addSample(s)
          },
        },
        SAMPLE_GROUPS,
        (text) => (benchText = text),
      )
      return
    }
    const { runBench } = await import('./lib/bench')
    await runBench(
      {
        viewer: v,
        idle: benchIdle,
        play(k) {
          playing[GLOBAL_GROUP] = k === null ? [] : [k]
          animationChanged()
        },
        async load(c) {
          for (const key of Object.keys(playing)) playing[Number(key)] = []
          playingParams = []
          playingSlices = []
          animationChanged()
          for (const it of [...items]) removeItem(it)
          folders = []
          resolution = c.res ?? 150
          implicitResolution = c.ires ?? 80
          pushItem(newItem(''))
          await addSample({
            label: c.label,
            items: c.items,
            view: { dims: c.dims, half: c.half, rotation: c.rotation, perspective: c.perspective },
          })
          await benchIdle()
        },
      },
      (text) => (benchText = text),
    )
  }

  // ---- 例の読み込み ----

  /** サンプルを一覧の最後に追加する（今ある式は消さない） */
  async function addSample(sample: Sample) {
    const view = sample.view
    if (view) {
      // 次元・回転・表示範囲を変える例: 今ある式は非表示にする（見え方が変わってしまうため）
      for (const it of items) {
        if (!it.visible) continue
        it.visible = false
        viewer?.setStyle(it.id, it)
      }
    }
    // 4次元の例などは、次元が足りなければ増やす
    if (!view && sample.dims && space.dims < sample.dims) setLayout(resizeSpace(space, sample.dims))
    // 空の式が1つだけなら、それはサンプルで置き換える
    // （一覧に出ない行（積分の範囲・積分の図）が前にあることもあるので、一覧の行で調べる）
    const only = listed()
    if (only.length === 1 && only[0].latex === '') removeItem(only[0])
    const defined = new Set(items.map((it) => it.param?.name ?? latexParamName(it.latex)))
    const adding = sample.items.filter((si) => !(si.param && defined.has(si.param))) // 同じ名前のパラメータがすでにあれば足さない
    // 式が3つ以上の例は、フォルダにまとめる（4つ以上なら折りたたんでおく）
    const folder = adding.length >= 3 ? addFolder(sample.label.replace(/（.*$/, ''), adding.length >= 4 && !sample.openFolder).id : undefined
    // 座標系で描く例: 用意した座標系（式・範囲が初期値のまま）があれば使い回し、無ければ作る
    let coordSys: number | undefined
    if (sample.coord) {
      const { preset, morph, play, flat: toFlat } = sample.coord
      const p = COORD_PRESETS[preset]
      const ranges = sample.coord.ranges ?? p.ranges
      const same = coordSystems.find(
        (c) => c.preset === preset && JSON.stringify([c.map, c.ranges]) === JSON.stringify([p.map, ranges]),
      )
      coordSys = same?.id ?? createCoord(preset)
      const made = coordSystems.find((c) => c.id === coordSys)
      if (made && !same && sample.coord.ranges) {
        // 範囲を指定した例（極座標で負の半径も描くなど）は、表示範囲に合わせない
        made.ranges = ranges.map((r) => [...r]) as CoordSystem['ranges']
        made.follow = [false, false, false]
      }
      playingMorph = playingMorph.filter((x) => x !== coordSys)
      setMorph(coordSys, morph)
      if (play) togglePlayMorph(coordSys)
      if (toFlat && !flat) setFlat(true)
    }
    if (sample.flat !== undefined && sample.flat !== flat) setFlat(sample.flat)
    for (const si of adding) {
      // 次元・回転を変える例は、全体の回転で描く（例の回転・再生は全体の回転に入れる）
      pushItem(newItem(si.latex, { ...si.options, folder, ...(view ? { rotGroup: GLOBAL_GROUP } : {}), ...(coordSys !== undefined ? { coordSys } : {}) }))
      if (!view) schedule(items[items.length - 1], 0)
    }
    if (view) applyView(view)
    // 積分の図を出す例（学習用。進み具合を途中にしておく）
    if (sample.integral !== undefined) {
      for (const it of items.slice(items.length - adding.length)) {
        if (!hasIntegral(it.latex) || integralItem(it)) continue
        toggleIntegral(it)
        setIntegralProgress(it, sample.integral)
        if (sample.riemann) setRiemann(it, sample.riemann.method, sample.riemann.n)
      }
    }
    // パラメータを再生する例
    if (sample.play) {
      const p = items.find((it) => (it.param?.name ?? latexParamName(it.latex)) === sample.play)
      if (p && !playingParams.includes(p.id)) togglePlayParam(p)
    }
    // 追加した式が見えるところまで一覧を動かす
    await tick()
    listEl.lastElementChild?.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
  }

  /** 「a=1」のような式なら、そのパラメータの名前（まだ計算していない式のため、文字の形だけで調べる） */
  function latexParamName(latex: string): string | undefined {
    const one = /^([a-zA-Z])\s*=/.exec(latex)?.[1]
    if (one) return one
    // 名前に使える文字（\alpha=1・\hat{x}=1。計画 M13）
    try {
      return /^\s*\{(\w+)\}\s*=/.exec(latexToText(latex))?.[1]
    } catch {
      return undefined
    }
  }

  /** サンプルの見え方（次元・回転・4次元の遠近感・表示範囲・回転の再生）にして、すべて計算し直す */
  function applyView(view: SampleView) {
    const dims = view.dims
    // グループの回転も、次元に合わせて作り直す
    if (dims !== space.dims) {
      resizeRotations(space.dims, dims)
    }
    const rotation = pairs(rotDims(dims)).map((_, k) => view.rotation?.[k] ?? 0)
    space = {
      ...space,
      dims,
      axes: [0, 1, 2],
      slice: Array(dims).fill(0),
      ranges: Array(dims).fill(null),
      rotation,
      center: Array(rotDims(dims)).fill(0),
      perspective: view.perspective ?? PERSPECTIVE_4D,
    }
    // 図形が大きく見えるよう、表示範囲を合わせて視点もリセットする
    const h = view.half
    const [xmin, xmax, ymin, ymax] = view.xy ?? [-h, h, -h, h]
    domain = { xmin, xmax, ymin, ymax, zmin: -h, zmax: h }
    domainError = ''
    viewer?.setDomain(domain, true)
    updateSpaceRangeErrors()
    replotAll(0)
    // 全体の回転の再生は、この例のものだけにする
    for (const k of [...(playing[GLOBAL_GROUP] ?? [])]) if (k !== view.play) togglePlay(GLOBAL_GROUP, k)
    if (view.play !== undefined && !(playing[GLOBAL_GROUP] ?? []).includes(view.play)) togglePlay(GLOBAL_GROUP, view.play)
  }

  /** パラメータのスライダーの設定を変えたとき。範囲で重ねる設定が変われば、それを使う式を計算し直す */
  function setSlider(item: PlotItem, next: SliderStyle) {
    const prev = item.slider
    item.slider = next
    // 範囲・刻みの式が変わったなら計算し直す（範囲で重ねているなら、それを使う式も計算し直す）
    if (next.bounds || next.stepExpr !== undefined) updateSliderBounds()
    else item.sliderError = undefined
    next = item.slider
    const stackChanged = prev.stack !== next.stack || (next.stack && (prev.min !== next.min || prev.max !== next.max))
    // 複素数のスライダーにした・戻したときも、それを使う式を計算し直す（定数の式が点になる・平面に戻る）
    if (item.param && (stackChanged || prev.complex !== next.complex)) paramsChanged([item.param.name], item)
  }

  /** 表示しない座標の断面の値を変えたとき: その座標を使う式だけを計算し直す */
  function setSlice(k: number, value: number) {
    space.slice[k] = value
    paramsChanged([coordName(k)])
  }

  // ---- 2D 表示 ----

  function setFlat(on: boolean) {
    flat = on
    viewer?.clearRotationTrails()
    viewer?.setFlat(on)
    viewer?.setCut(cutRequest())
    // 2D は真上からほぼ平行に見る。3D に戻したら元の遠近感にする
    viewer?.setPerspective(on ? 0 : perspective / 100)
    viewer?.resetCamera()
    replotAll(0)
  }

  // ---- 立体の切り口 ----

  /** 陰関数・範囲で重ねた立体・不等式の領域の式（切り口のふたを計算担当が作る） */
  const capKinds = ['陰関数', '範囲で重ねた立体', '不等式の領域']
  /** 切り口のふたの細かさ（陰関数の格子の2倍。再生・ドラッグ中は、その式の計算と同じ倍率を掛ける） */
  const capResolution = (id: number) =>
    Math.max(16, Math.round(implicitResolution * 2 * (isCoarse() ? (lastScale.get(id) ?? 0.5) : 1)))
  /** ふたの依頼の番号（古い結果を捨てるため） */
  const capToken = new Map<number, number>()

  /** 切り口を変えたとき: 描画側の切り落としを変え、陰関数などのふただけを作り直す（式は計算し直さない） */
  async function setCut(next: Partial<CutStyle>) {
    Object.assign(cut, next)
    const req = cutRequest()
    viewer?.setCut(req)
    await Promise.all(
      items
        .filter((it) => it.visible && !it.error && it.info && capKinds.some((k) => it.info!.startsWith(k)))
        .map(async (it) => {
          const token = (capToken.get(it.id) ?? 0) + 1
          capToken.set(it.id, token)
          const res = await requestCap(it.id, req, capResolution(it.id))
          if (capToken.get(it.id) === token) viewer?.setCap(it.id, res.cap, it)
        }),
    )
  }

  /** 切り口の軸の範囲（表示範囲） */
  const cutRange = $derived(
    [
      [domain.xmin, domain.xmax],
      [domain.ymin, domain.ymax],
      [domain.zmin, domain.zmax],
    ][cut.axis],
  )

  function setBound(key: keyof Domain, value: number) {
    if (!Number.isFinite(value)) return
    domain[key] = value
    if (domain.xmin >= domain.xmax || domain.ymin >= domain.ymax || domain.zmin >= domain.zmax) {
      domainError = '最小値は最大値より小さくしてください'
      return
    }
    domainError = ''
    viewer?.setDomain(domain)
    viewer?.fitDomainIfNeeded()
    replotAll(300)
    // 積分の図の矢印の長さは表示範囲で決めるので、作り直す
    for (const it of items) if (!isAux(it) && integralItem(it)) updateIntegral(it, 300)
  }
</script>

<svelte:window
  bind:innerHeight={windowHeight}
  onkeydown={(e) => e.key === 'Escape' && settingsOpen && closeSettings()}
/>

{#if graphsOpen}
  <GraphsDialog
    graphs={graphList}
    currentId={graphId}
    {saveFailed}
    {shareMessage}
    onClose={() => (graphsOpen = false)}
    onOpen={openGraph}
    onNew={newGraph}
    onDelete={removeGraph}
    onDuplicate={copyGraph}
    onRename={renameAny}
    onShare={copyShareLink}
  />
{/if}

<div
  class="app"
  style:--keyboard-h="{keyboardHeight}px"
  style:--graph-bottom="{graphBottom}px"
  style:--panel-w={panelWidth === null ? null : `${panelWidth}px`}
>
  <aside
    class="panel"
    style:--sheet-h="{sheetHeight}px"
  >
    <!-- スマホではここがシートのつまみになる -->
    <header
      class="title"
      role="button"
      tabindex="0"
      aria-label="式の一覧を出し入れする"
      onpointerdown={sheetDown}
      onpointermove={sheetMove}
      onpointerup={sheetUp}
      onpointercancel={sheetUp}
      onkeydown={sheetKey}
    >
      <span class="grip"></span>
      <h1>graphploter</h1>
      <!-- グラフの名前とグラフの一覧。シートのつまみのドラッグにならないよう、押した操作をここで止める -->
      <input
        class="graph-title"
        value={graphTitle}
        aria-label="グラフの名前"
        title="グラフの名前（押して変える）"
        onpointerdown={(e) => e.stopPropagation()}
        onkeydown={(e) => {
          e.stopPropagation()
          if (e.key === 'Enter') e.currentTarget.blur()
        }}
        onchange={(e) => (graphTitle = e.currentTarget.value.trim() || UNTITLED)}
      />
      <button
        class="graphs-btn"
        onpointerdown={(e) => e.stopPropagation()}
        onclick={openGraphs}
        title="グラフの一覧（新しいグラフ・開く・共有）"
        aria-label="グラフの一覧"
      >☰ グラフ</button>
    </header>

    <div class="panel-body">

      <ul class="list" bind:this={listEl}>
        {#each rows as row (row.kind === 'folder' ? `f${row.folder.id}` : row.item.id)}
          {#if row.kind === 'folder'}
            {@const f = row.folder}
            {@const errors = row.members.filter((it) => it.error).length}
            {@const shown = row.members.some((it) => !it.param && it.visible)}
            <li class="folder" class:collapsed={f.collapsed}>
              <button class="fold" aria-expanded={!f.collapsed} aria-label={f.collapsed ? 'フォルダを開く' : 'フォルダを折りたたむ'}
                title={f.collapsed ? '開く' : '折りたたむ'} onclick={() => (f.collapsed = !f.collapsed)}>{f.collapsed ? '▸' : '▾'}</button>
              <input class="folder-name" value={f.name} aria-label="フォルダの名前"
                onchange={(e) => (f.name = e.currentTarget.value.trim() || f.name)} />
              <span class="folder-count" class:has-error={errors > 0}
                >{row.members.length} 個{errors > 0 ? `（エラー ${errors}）` : ''}</span>
              <button class="folder-btn" title={shown ? '中の式をまとめて非表示にする' : '中の式をまとめて表示する'}
                aria-label={shown ? '中の式をまとめて非表示にする' : '中の式をまとめて表示する'}
                onclick={() => toggleFolderVisible(row.members)}>{shown ? '●' : '○'}</button>
              <button class="folder-btn" title="フォルダを解除（式は残す）" aria-label="フォルダを解除（式は残す）"
                onclick={() => removeFolder(f, false)}>⇱</button>
              <button class="folder-btn" title="フォルダごと削除" aria-label="フォルダごと削除"
                onclick={() => removeFolder(f, true)}>×</button>
            </li>
          {:else}
            {@const item = row.item}
            {@const i = row.index}
          <ExpressionItem
            {item}
            onToggleSequenceTable={() => {
              item.showSequenceTable = !item.showSequenceTable
              if (item.showSequenceTable) schedule(item, 0)
              else item.sequenceTable = undefined
            }}
            index={i}
            {domain}
            {axisNames}
            onInput={(s) => setLatex(item, s)}
            {regions}
            onRegion={setRegion}
            {regionViews}
            onRegionShow={toggleRegion}
            integralView={integralViews[item.id]}
            onIntegralShow={() => toggleIntegral(item)}
            onIntegralProgress={(v) => {
              playingIntegral = playingIntegral.filter((x) => x !== item.id)
              setIntegralProgress(item, v)
            }}
            onIntegralPlay={() => togglePlayIntegral(item)}
            onRiemann={(m, n) => setRiemann(item, m, n)}
            paramAllowed={item.param ? allowedMask(item) : undefined}
            onColor={(c) => setColor(item, c)}
            onGrid={(g) => setGrid(item, g)}
            onOpacity={(o) => setOpacity(item, o)}
            onComplexMode={(m) => setComplexMode(item, m)}
            onHidden={(patch) => setHidden(item, patch)}
            onComplexCoords={(on) => setComplexCoords(item, on)}
            globalSlice={space.slice}
            sliceMin={space.sliceMin}
            sliceMax={space.sliceMax}
            onSlice={(name, v) => setItemSlice(item, name, v)}
            onWalls={(on) => setWalls(item, on)}
            onField={(patch) => setField(item, patch)}
            onOdeInit={(k, s) => setOdeInit(item, k, s)}
            flatView={flat}
            onLayers={(n) => setLayers(item, n)}
            onEdges={(on) => setEdges(item, on)}
            onRender={(mode) => setRender(item, mode)}
            onSetDims={(n) => { if (n > space.dims) setLayout(resizeSpace(space, n)) }}
          onLineWidth={(w) => setLineWidth(item, w)}
          onRange={(name, side, v) => setRange(item, name, side, v)}
          autoCoordName={item.autoCoord && AUTO_COORD_NAMES[item.autoCoord]}
          autoAngles={item.autoCoord ? (item.autoCoord === 'sphere' ? ['theta', 'phi'] : ['theta']).map((name) => ({ name, range: angleRangeOf(item, name) })) : []}
          onAngleRange={(name, side, v) => setAngleRange(item, name, side, v)}
            onToggle={() => {
              // 自分で ● を押したら、その表示・非表示に従う（積分の図を消しても戻さない）
              hiddenForIntegral.delete(item.id)
              toggle(item)
            }}
            onToggleSettings={() => (item.showSettings = !item.showSettings)}
          onToggleVertical={() => (item.vertical = !item.vertical)}
            onRemove={() => removeItem(item)}
            onDuplicate={() => duplicateItem(item)}
            onParam={(re, im) => setParam(item, re, im)}
            onSlider={(sl) => setSlider(item, sl)}
            paramPlaying={playingParams.includes(item.id)}
            onPlayParam={() => togglePlayParam(item)}
            onAddSlider={(name) => addSlider(item, name)}
            onEnter={() => insertAfter(item)}
            onDrag={(phase, e) => dragItem(item, phase, e)}
            dragging={dragId === item.id}
            dims={space.dims}
            viewAxes={space.axes}
            {groups}
            groupId={groupIdOf(item)}
            {groupSizes}
            rotation={rotationOf(item).rotation}
            center={rotationOf(item).center}
            rotPlaying={playing[groupIdOf(item)] ?? []}
            onRotate={(r, c) => setRotation(groupIdOf(item), r, c)}
            onPlayRot={(k) => togglePlay(groupIdOf(item), k)}
            onStopRot={(ks) => stopPlay(groupIdOf(item), ks)}
            trailEnabled={trailGroups.includes(groupIdOf(item))}
            onTrail={(enabled) => setTrail(groupIdOf(item), enabled)}
            onClearTrail={() => clearTrail(groupIdOf(item))}
            onGroup={(g) => setItemGroup(item, g)}
            {coordSystems}
            onCoord={(c) => setItemCoord(item, c)}
            onOpenCoord={() => openCoord(item.coordSys)}
            onOpenRotation={() => openRotation(groupIdOf(item))}
            onMatrixCoord={() => useMatrixAsCoord(item)}
            {folders}
            onFolder={(fid) => setItemFolder(item, fid)}
          />
          {/if}
        {/each}
      </ul>
      <div class="add-row">
        <button class="add" onclick={addItem}>＋ 式を追加</button>
        <button class="add" onclick={() => addFolder()}>＋ フォルダ</button>
        <button class="add" aria-expanded={matrixPicker} onclick={() => (matrixPicker = !matrixPicker)}>＋ 行列</button>
        {#if matrixPicker}
          <!-- 行列の大きさを選ぶマス目（押したマスの大きさの行列の定義の行ができる） -->
          <div class="matrix-grid" role="group" aria-label="行列の大きさ" onpointerleave={() => (matrixHover = null)}>
            {#each Array(MATRIX_MAX) as _, r}
              {#each Array(MATRIX_MAX) as _, c}
                <button
                  class="cell"
                  class:on={matrixHover ? r <= matrixHover[0] && c <= matrixHover[1] : false}
                  aria-label="{r + 1} × {c + 1}"
                  onpointerenter={() => (matrixHover = [r, c])}
                  onclick={() => addMatrix(r + 1, c + 1)}
                ></button>
              {/each}
            {/each}
            <span class="label">{matrixHover ? `${matrixHover[0] + 1} × ${matrixHover[1] + 1}` : '大きさを選ぶ'}</span>
          </div>
        {/if}
      </div>
      <details class="samples" open>
        <summary>サンプル（タップで追加）</summary>
        {#each SAMPLE_GROUPS as group (group.title)}
          <div class="sample-group">
            <span class="sample-title">{group.title}</span>
            <div class="chips">
              {#each group.samples as sample (sample.label)}
                <button class="chip" onclick={() => addSample(sample)}>{sample.label}</button>
              {/each}
            </div>
          </div>
        {/each}
      </details>


    </div>
    <!-- PC: 式の一覧の幅を変えるつまみ（右の端をドラッグ。ダブルクリックで元の幅） -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="panel-resizer"
      class:active={widthDrag !== null}
      title="ドラッグで式の欄の幅を変える（ダブルクリックで元に戻す）"
      onpointerdown={widthDown}
      onpointermove={widthMove}
      onpointerup={widthUp}
      onpointercancel={widthUp}
      ondblclick={() => setPanelWidth(null)}
    ></div>
  </aside>

  <main class="viewer" bind:this={viewerEl}></main>
  {#if BENCH}
    <pre class="bench-report">{benchText}</pre>
  {/if}

  <!-- 設定: ボタンで項目の一覧を出し、選んだ項目だけを表示する（下までスクロールしなくてよいように） -->
  <div
    class="settings-anchor"
    style:--settings-h="{settingsHeight}px"
  >
    <div class="tool-row">
      <button class="settings-toggle" onclick={() => viewer?.resetCamera()} title="視点を最初の位置に戻す"
        aria-label="視点を最初の位置に戻す">⟲ 視点</button>
      <button
        class="settings-toggle"
        class:on={settingsOpen}
        onclick={() => (settingsOpen ? closeSettings() : (settingsOpen = true))}
        aria-expanded={settingsOpen}
        title="設定"
      >⚙ 設定</button>
    </div>
    {#if settingsOpen}
      <div class="settings-card" role="dialog" aria-label="設定">
        <div class="card-head">
          {#if settingsCat}
            <button class="back" onclick={() => (settingsCat = null)} aria-label="項目の一覧に戻る">‹</button>
            <span class="card-title">{SETTINGS_CATS.find((c) => c.id === settingsCat)?.label}</span>
          {:else}
            <span class="card-title">設定</span>
          {/if}
          <button class="close" onclick={closeSettings} aria-label="設定を閉じる">×</button>
        </div>
        <div class="card-body">
          {#if !settingsCat}
            <div class="cat-list">
              {#each SETTINGS_CATS as c (c.id)}
                <button class="cat" onclick={() => (settingsCat = c.id)}>
                  <span class="cat-icon">{c.icon}</span>
                  <span class="cat-text">
                    <span class="cat-label">{c.label}</span>
                    <span class="cat-desc">{c.desc}</span>
                  </span>
                </button>
              {/each}
            </div>
          {:else if settingsCat === 'dims'}
            <SpacePanel
              {space}
              rangeErrors={spaceRangeErrors}
              onLayout={setLayout}
              onSlice={setSlice}
              onAddSlider={addSliderTop}
              playing={playingSlices}
              onPlaySlice={togglePlaySlice}
              onResetSlice={(k) => {
                stopSlice(k)
                setSlice(k, 0)
              }}
            />

          {:else if settingsCat === 'rotate'}
            <div class="group-tabs" role="tablist" aria-label="回転のグループ">
              <button role="tab" class="chip" class:selected={rotTab === GLOBAL_GROUP} aria-selected={rotTab === GLOBAL_GROUP}
                onclick={() => (rotTab = GLOBAL_GROUP)}>全体</button>
              {#each groups as g (g.id)}
                <button role="tab" class="chip" class:selected={rotTab === g.id} aria-selected={rotTab === g.id}
                  onclick={() => (rotTab = g.id)}>{g.name}</button>
              {/each}
              <button class="chip" onclick={() => (rotTab = createGroup(rotTab))} title="新しいグループを作る（今のタブの回転を引き継ぐ）">＋ グループ</button>
            </div>
            {@const tabGroup = groups.find((g) => g.id === rotTab)}
            {#if tabGroup}
              <div class="group-head">
                <input class="group-name" value={tabGroup.name} aria-label="グループの名前"
                  onchange={(e) => renameGroup(tabGroup.id, e.currentTarget.value)} />
                <button class="secondary small" onclick={() => { removeGroup(tabGroup.id); rotTab = GLOBAL_GROUP }}>解散</button>
              </div>
            {/if}
            <p class="note">
              {rotTab === GLOBAL_GROUP
                ? '式は標準では、その式だけの回転で描きます（式の ⚙ →「回転」）。チェックした式は、この全体の回転で一緒に動きます。'
                : '同じグループの式は同じ回転で動きます（同期）。チェックした式がこのグループに入ります。'}
            </p>
            <div class="members">
              {#each plotItems as it (it.id)}
                {@const k = items.indexOf(it)}
                {@const gid = groupIdOf(it)}
                <label class="member">
                  <input type="checkbox" checked={gid === rotTab}
                    onchange={(e) => setItemGroup(it, e.currentTarget.checked ? rotTab : OWN_GROUP)} />
                  <span class="dot" style:background={it.color}></span>
                  <span class="member-name">式 {k + 1}</span>
                  {#if gid !== rotTab}
                    <span class="member-group">（{gid < 0 ? 'この式だけ' : gid === GLOBAL_GROUP ? '全体' : groups.find((g) => g.id === gid)?.name}）</span>
                  {/if}
                </label>
              {/each}
            </div>
            {@const rot = rotationIn(rotTab)}
            <RotationPanel
              dims={space.dims}
              axes={space.axes}
              rotation={rot.rotation}
              center={rot.center}
              onRotate={(r, c) => setRotation(rotTab, r, c)}
              playing={playing[rotTab] ?? []}
              onPlay={(k) => togglePlay(rotTab, k)}
              onStop={(ks) => stopPlay(rotTab, ks)}
              trailEnabled={trailGroups.includes(rotTab)}
              onTrail={(enabled) => setTrail(rotTab, enabled)}
              onClearTrail={() => clearTrail(rotTab)}
            />
          {:else if settingsCat === 'coords'}
            <div class="group-tabs" role="tablist" aria-label="座標系">
              {#each coordSystems as c (c.id)}
                <button role="tab" class="chip" class:selected={coordTab === c.id} aria-selected={coordTab === c.id}
                  onclick={() => (coordTab = c.id)}>{c.name}</button>
              {/each}
              <select class="chip add-coord" value="" aria-label="座標系を足す"
                onchange={(e) => {
                  const v = e.currentTarget.value as CoordSystem['preset'] | ''
                  e.currentTarget.value = ''
                  if (v) coordTab = createCoord(v)
                }}>
                <option value="">＋ 座標系を足す</option>
                {#each Object.entries(COORD_PRESETS) as [key, p] (key)}
                  <option value={key}>{p.name}</option>
                {/each}
              </select>
            </div>
            {@const tabCoord = coordSystems.find((c) => c.id === coordTab) ?? coordSystems[0]}
            {#if tabCoord}
              <div class="group-head">
                <input class="group-name" value={tabCoord.name} aria-label="座標系の名前"
                  onchange={(e) => renameCoord(tabCoord.id, e.currentTarget.value)} />
                <button class="secondary small" onclick={() => removeCoord(tabCoord.id)}>削除</button>
              </div>
              <p class="note">チェックした式を、この座標系で描きます（式の ⚙ →「座標系」でも選べます）。</p>
              <div class="members">
                {#each plotItems as it (it.id)}
                  {@const k = items.indexOf(it)}
                  {@const other = coordOf(it)}
                  <label class="member">
                    <input type="checkbox" checked={it.coordSys === tabCoord.id}
                      onchange={(e) => setItemCoord(it, e.currentTarget.checked ? tabCoord.id : undefined)} />
                    <span class="dot" style:background={it.color}></span>
                    <span class="member-name">式 {k + 1}</span>
                    {#if other && other.id !== tabCoord.id}
                      <span class="member-group">（{other.name}）</span>
                    {/if}
                  </label>
                {/each}
              </div>
              <CoordPanel
                sys={tabCoord}
                effective={coordRanges(tabCoord)}
                onFollow={(k, on) => setCoordFollow(tabCoord.id, k, on)}
                onShape={(m, r) => setCoordShape(tabCoord.id, m, r)}
                onNames={(n) => setCoordNames(tabCoord.id, n)}
                onGrid={(g) => setCoordGrid(tabCoord.id, g)}
                onMorph={(v) => setMorph(tabCoord.id, v)}
                playing={playingMorph.includes(tabCoord.id)}
                onPlay={() => togglePlayMorph(tabCoord.id)}
              />
            {:else}
              <p class="note">
                式を極座標・球座標・自作の座標系で描きます。式は x, y, z で書き、それを座標の値（球座標なら半径・z 軸からの角・xy 平面での角）として読みます。
                「移行」のスライダーで、直交座標の見え方となめらかに行き来できます。上のボタンで座標系を足してください。
              </p>
            {/if}
          {:else if settingsCat === 'range'}
            <div class="ranges">
              {#each axes as a (a)}
                <div class="range-row">
                  <NumberInput
                    value={domain[`${a}min`]}
                    label="{axisNames[axes.indexOf(a)]} の最小値"
                    onChange={(v) => setBound(`${a}min`, v)}
                  />
                  <span class="axis">≤ {prettyName(axisNames[axes.indexOf(a)])} ≤</span>
                  <NumberInput
                    value={domain[`${a}max`]}
                    label="{axisNames[axes.indexOf(a)]} の最大値"
                    onChange={(v) => setBound(`${a}max`, v)}
                  />
                </div>
              {/each}
            </div>
            {#if domainError}<p class="error">{domainError}</p>{/if}


          {:else if settingsCat === 'detail'}
            <div class="slider">
              <span>
                曲面の細かさ:
                <EditableValue value={resolution} label="曲面の細かさ" suffix=" × {resolution}" min={10} max={600} integer
                  onChange={(v) => { resolution = v; replotAll(0) }} />
              </span>
              <input
                type="range"
                aria-label="曲面の細かさ"
                min="20"
                max="400"
                step="10"
                bind:value={resolution}
                onchange={() => replotAll(0)}
              />
            </div>
            <div class="slider">
              <span>
                範囲で重ねる枚数:
                <EditableValue value={space.samples} label="範囲で重ねる枚数" min={2} max={200} integer
                  onChange={(v) => { space.samples = v; replotAll(0) }} />
              </span>
              <input
                type="range"
                aria-label="範囲で重ねる枚数"
                min="4"
                max="64"
                step="2"
                bind:value={space.samples}
                onchange={() => replotAll(0)}
              />
            </div>
            <div class="slider">
              <span>
                陰関数・立体の細かさ:
                <EditableValue value={implicitResolution} label="陰関数・立体の細かさ" suffix="³（重いので注意）" min={10} max={300} integer
                  onChange={(v) => { implicitResolution = v; replotAll(0) }} />
              </span>
              <input
                type="range"
                aria-label="陰関数・立体の細かさ"
                min="20"
                max="200"
                step="10"
                bind:value={implicitResolution}
                onchange={() => replotAll(0)}
              />
            </div>
            <label class="check-row">
              <input type="checkbox" checked={findSmall} onchange={(e) => { findSmall = e.currentTarget.checked; replotAll(0) }} />
              陰関数の細かい図形を念入りに探す（遅くなる）
            </label>
            <p class="note">格子より小さい図形（小さい球・細い輪など）は、オフでもある程度探して細かく描きます。オンにすると、探す手間と細かさの上限を上げます。</p>


          {:else if settingsCat === 'look'}
            <div class="slider">
              <span>視点を動かしている間の画質:</span>
              {#each MOTION_MODES as m (m.mode)}
                <label class="check-row">
                  <input type="radio" name="motion-mode" checked={motionMode === m.mode} onchange={() => setMotionMode(m.mode)} />
                  {m.label}
                </label>
              {/each}
              <span class="note">{MOTION_MODES.find((m) => m.mode === motionMode)?.note}</span>
            </div>
            <div class="slider">
              <span>
                4次元の遠近感:
                <EditableValue value={space.perspective} label="4次元の遠近感" suffix={space.perspective === 0 ? '%（なし: 平行な射影）' : '%'}
                  min={0} max={100} onChange={(v) => { space.perspective = v; replotAll(0) }} />
              </span>
              <input
                type="range"
                aria-label="4次元の遠近感"
                min="0"
                max="100"
                step="1"
                bind:value={space.perspective}
                oninput={() => replotAll(0)}
              />
              <span class="note">w などの奥にある部分ほど小さく描く（媒介変数表示・回した陽関数に効く）</span>
            </div>
            <div class="slider">
              <span>
                遠近感:
                <EditableValue value={perspective} label="遠近感" suffix={perspective === 0 ? '%（平行投影）' : '%'} min={0} max={100}
                  onChange={(v) => { perspective = v; if (!flat) viewer?.setPerspective(v / 100) }} />
              </span>
              <input
                type="range"
                aria-label="遠近感"
                min="0"
                max="100"
                step="1"
                bind:value={perspective}
                oninput={() => !flat && viewer?.setPerspective(perspective / 100)}
              />
            </div>
            <div class="buttons">
              <button class="secondary" onclick={() => viewer?.resetCamera()}>視点をリセット</button>
              <button class="secondary" onclick={() => (dark = !dark)}>{dark ? '明るい背景' : '暗い背景'}</button>
            </div>

          {:else if settingsCat === 'control'}
            {#snippet modeCard(mode: 'A' | 'B', title: string, rows: [string, string][])}
              <label class="mode-card" class:selected={controlMode === mode}>
                <span class="mode-head">
                  <input type="radio" name="control-mode" checked={controlMode === mode} onchange={() => (controlMode = mode)} />
                  {title}
                </span>
                <span class="mode-rows">
                  {#each rows as [k, v] (k)}
                    <span class="mode-key">{k}</span><span>{v}</span>
                  {/each}
                </span>
              </label>
            {/snippet}
            {@render modeCard('A', '操作 A（標準）', [
              ['左ドラッグ', '回転'],
              ['右ドラッグ', '平行移動'],
              ['Shift + 左ドラッグ', '平行移動'],
              ['ホイール・中ドラッグ', '拡大縮小（B と同じ動き。中ドラッグは押した位置に向かう）'],
            ])}
            {@render modeCard('B', '操作 B（Google Earth 風）', [
              ['左ドラッグ', '平行移動'],
              ['右ドラッグ', '押した位置を中心に、上下で縮小・拡大、左右で回転'],
              ['左を2回押してドラッグ', '押した位置を中心に回転（視点は動かさない）'],
              ['Shift + 左ドラッグ', '押した位置を中心に回転（視点は動かさない）'],
              ['Ctrl（⌘）+ 左ドラッグ', 'その場から見回す'],
              ['ホイール', '拡大縮小'],
              ['中ドラッグ', '平行移動'],
            ])}
            <p class="note">パソコンのマウスの操作だけが変わります（スマホの指の操作は同じ）。</p>
            <label class="check-row">
              <input type="checkbox" checked={pickValues} onchange={(e) => setPickValues(e.currentTarget.checked)} />
              グラフを押したら値を出す
            </label>
            <p class="note">
              オンにすると、曲線・曲面を押した所に印と座標が出ます。y = f(x)・z = f(x, y) の式では、極大・極小・零点などの点も灰色の印で出て、押すと種類と座標が出ます。
              タッチで操作すると印が出て見づらいので、標準はオフです（このブラウザに覚えます）。
            </p>
          {:else if settingsCat === 'cut'}
            {#if flat}<p class="note">2D 表示では使えません</p>{/if}
            <label class="check-row" class:disabled={flat}>
              <input type="checkbox" checked={cut.enabled} onchange={(e) => setCut({ enabled: e.currentTarget.checked })} />
              立体を平面で切って、切り口を見る
            </label>
            {#if cut.enabled}
              <div class="cut-axes" role="group" aria-label="切る向き">
                {#each [0, 1, 2] as const as k (k)}
                  <button
                    class="secondary"
                    class:selected={cut.axis === k}
                    onclick={() => {
                      const [lo, hi] = [
                        [domain.xmin, domain.xmax],
                        [domain.ymin, domain.ymax],
                        [domain.zmin, domain.zmax],
                      ][k]
                      setCut({ axis: k, pos: Math.min(hi, Math.max(lo, cut.pos)) })
                    }}>{prettyName(axisNames[k])} = 一定</button
                  >
                {/each}
              </div>
              <div class="slider">
                <span>
                  位置: {prettyName(axisNames[cut.axis])}
                  <EditableValue
                    value={cut.pos}
                    prefix="= "
                    format={(v) => String(Number(v.toPrecision(4)))}
                    label="切り口の位置"
                    onChange={(v) => setCut({ pos: v })}
                  />
                </span>
                <input
                  type="range"
                  min={cutRange[0]}
                  max={cutRange[1]}
                  step={(cutRange[1] - cutRange[0]) / 200}
                  value={cut.pos}
                  aria-label="切り口の位置"
                  oninput={(e) => setCut({ pos: e.currentTarget.valueAsNumber })}
                />
              </div>
              <div class="buttons">
                <button class="secondary" onclick={() => setCut({ flip: !cut.flip })}>
                  {cut.flip ? `${prettyName(axisNames[cut.axis])} ≥ ${Number(cut.pos.toPrecision(4))} を表示中` : `${prettyName(axisNames[cut.axis])} ≤ ${Number(cut.pos.toPrecision(4))} を表示中`}（反対側にする）
                </button>
              </div>
              <p class="note">
                立体（陰関数・範囲で重ねた立体・媒介変数3つの立体）の切り口はふたをして塗ります。陰関数は、式の両側のうち表示範囲の中で狭いほうを中身とします
              </p>
            {/if}


          {:else if settingsCat === 'save'}
            <p class="note">
              式と設定は、グラフごとにこのブラウザに自動で保存されます。次にサイトを開くと、最後に開いたグラフが開きます。
              共有リンクには、グラフの中身がまるごと入ります（サーバーには送りません）。リンクを開いた人の一覧には、新しいグラフとして入ります。
            </p>
            <div class="buttons">
              <button class="secondary" onclick={() => { closeSettings(); openGraphs() }}>グラフの一覧</button>
              <button class="secondary" onclick={copyShareLink}>共有リンクをコピー</button>
              <button class="secondary" onclick={resetAll}>最初の状態に戻す</button>
            </div>
            {#if shareMessage}<p class="note">{shareMessage}</p>{/if}


          {:else if settingsCat === 'help'}
            <div class="help">
              <ul>
                <li>陽関数: <code>z = x^2 - y^2</code>（<code>z =</code> は省略可）、<code>y = sin(x) z</code> なども可</li>
                <li>陰関数: <code>x^2 + y^2 + z^2 = 4</code>、<code>sin x + sin y + sin z = 0</code></li>
                <li>曲線: <code>(cos t, sin t, t/4)</code>（t を使う。範囲は式の下で指定）</li>
                <li>
                  複素平面: <code>e^(it)</code> のように、座標を使わず t（または u, v）だけの式は、値を複素平面（横 = 実部、奥 = 虚部）に描く（単位円）。
                  ⚙ の「縦の軸」で t・|f|・arg f を縦にとれる（らせん）。<code>1 + 2i</code> や複素数のパラメータ <code>c</code>、<code>c^2</code> は複素平面の点になる
                </li>
                <li>曲面: <code>(u cos v, u sin v, u)</code>（u と v を使う）</li>
                <li>掛け算の <code>*</code> は省略可: <code>2xy</code>, <code>x sin(y)</code></li>
                <li>定数: <code>pi</code>, <code>e</code>, <code>i</code>（虚数単位）</li>
                <li>
                  関数: sin cos tan sec csc cot arcsin arccos arctan sinh cosh tanh arcsinh arccosh arctanh exp ln log sqrt abs
                floor ceil round sign Re Im conj arg、2つの引数: mod min max nCr nPr、階乗: <code>n!</code>
              </li>
              <li>
                特殊関数（すべて複素数で使えます。数式キーボードの「特殊」タブに全部あり、◀ ▶ でページをめくれます）
                <ul>
                  <li>ガンマ系: gamma(z)（Γ）, gamma(a, z)（上側不完全）, gammaP(a, z), gammaQ(a, z), loggamma, beta(a, b), beta(z, a, b), betaRegularized(z, a, b), digamma（psi, ψ）, polygamma(n, z), pochhammer(a, n), binomial(n, k)</li>
                  <li>ゼータ系: zeta(s)（ζ）, zeta(s, a)（フルヴィッツ）, polylog(s, z)（Li）, dirichletEta(s)</li>
                  <li>誤差関数・積分: erf, erfc, erfi, dawson, fresnelS, fresnelC, Ei, En(n, z), li, Si, Ci, Shi, Chi, sinc</li>
                  <li>ベッセル系: besselJ(ν, z), besselY, besselI, besselK, hankelH1, hankelH2, sphericalJ(n, z), sphericalY, airyAi(z), airyBi, airyAiPrime, airyBiPrime</li>
                  <li>直交多項式: legendreP(n, x), legendreP(n, m, x), hermiteH(n, x), laguerreL(n, x), laguerreL(n, a, x), chebyshevT(n, x), chebyshevU, gegenbauerC(n, a, x), jacobiP(n, a, b, x), sphericalHarmonicY(l, m, θ, φ)</li>
                  <li>超幾何関数: hyp0F1(b, z), hyp1F1(a, b, z), hyp2F1(a, b, c, z), hypU(a, b, z)</li>
                  <li>楕円系（m = k²）: ellipticK(m), ellipticE(m), ellipticE(φ, m), ellipticF(φ, m), ellipticPi(n, m), ellipticPi(n, φ, m), jacobiSN(u, m), jacobiCN, jacobiDN</li>
                  <li>その他: lambertW(z)（productLog）, lambertW(k, z)（第 k 枝）</li>
                </ul>
              </li>
                <li>複素数の結果が実数でない点は描かれません</li>
                <li>
                入力欄は数式の見た目のまま編集できます。<code>/</code> で分数、<code>^</code> でべき乗、<code>pi</code> →
                π、<code>sqrt</code> → √ に自動で変わります
              </li>
              <li>入力欄に入ると数式用のキーボードが出ます（123 / f(x) / 特殊 / abc のタブで切り替え）。キーにカーソルを当てる（スマホでは長押しする）と、そのキーの名前が出ます</li>
              <li>
                4次元以上: 「次元」を増やすと座標 w、w₂、w₃… が使えます（4次元目が w、5次元目が w₂。w₁ と書くと w と同じ）。
                表示する3つの座標を選び、残りはスライダーで固定した断面を描きます
              </li>
              <li>
                添字: 文字の後ろに数字を打つと添字になります（<code>w2</code> → w₂、<code>t10</code> → t₁₀）。
                数式キーボードの「変数」のタブには、今の次元で使える座標と媒介変数 t₁, t₂, … が並んでいます
              </li>
              <li>
                多変数: 媒介変数は t₁, t₂, t₃, t₄, … といくつでも使えます（8個まで）。4つ以上のときは、
                媒介変数を2つずつ動かした面（残りは範囲の両端に固定）をたくさん描いて形を表します。
                例: <code>(t1, t2, t3, t4)</code> は4次元の立方体の面 24 枚。⚙ の「中の面」で、中を通る面も描けます
              </li>
              <li>
                範囲で重ねる: 座標を「範囲」にするか、パラメータの「範囲で重ねる」を押すと、その範囲で動かしたときに
                図形が通る領域（曲面 → 立体、曲線 → 曲面）を描きます
              </li>
              <li>
                回転: 中心の点と、座標の組（xy, xw など）ごとの角度で図形を回せます。▶ で回し続けます。
                「4次元の遠近感」を上げると、w の奥にある部分ほど小さく描きます
              </li>
              <li>
                リスト: <code>(t, [-1,1], [-1,1], [-1,1])</code> や <code>x^2 + y^2 + z^2 = [1, 4, 9]</code>
                のように書くと、組み合わせの数だけまとめて描きます
              </li>
              <li>立体: <code>((3 + t cos v) cos u, (3 + t cos v) sin u, t sin v)</code>（u, v, t の3つを使う）</li>
              <li>2D 表示: グラフ右上の「2D」で、真上から見た平面に描きます（z = f(x, y) は値の塗り分け）</li>
              <li>媒介変数は t（曲線）、u と v（曲面）、または t₁ と t₂。組 (…) の外では t, u, v もパラメータにできます</li>
              <li>
                パラメータ: <code>a=1</code> と書くとスライダーになり、ほかの式で <code>a</code> を使えます。
                「複素数」を押すと、複素平面の上で点を動かして値を決められます
              </li>
              <li>
                数学の記法（数式キーボードの「∑∫∈」のタブに、雛形があります。◀ ▶ で「微積」「積分」「範囲」「ベクトル」「方程式」「集合」のページ）
                <ul>
                  <li>関数: <code>f(x) = x^2</code> と定義して <code>z = f(x) - f(y)</code>。合成 <code>(f∘g)(x)</code>、導関数 <code>f′(x)</code>、逆関数 <code>y = f^(-1)(x)</code></li>
                  <li>微積: <code>Σ</code>・<code>Π</code>（中身は直後の項）、<code>d/dx</code>・<code>∂/∂x</code>、差分 <code>Δₓ</code>、場合分け</li>
                  <li>積分: 範囲の次元で並べてあります。1 次元は定積分 <code>∫ₐᵇ f dx</code>（区間）と線積分（曲線）、2 次元は 2 重積分 <code>∬_D f dA</code>（平面の領域）と面積分（曲面）、3 次元は 3 重積分 <code>∭_V f dV</code>。線積分・面積分は、定積分・2 重積分の範囲を曲がった曲線・曲面に広げたもので、それぞれスカラー場 <code>f ds</code>・<code>f dS</code>（長さ・面積で足す）とベクトル場 <code>F · dr</code>・<code>F · dS</code>（接線・法線の成分を足す。向きで符号が変わる）があります。範囲は「範囲」のページの雛形で、集合 <code>{'{(x, y) | x² + y² = 1}'}</code> でも媒介変数表示 <code>{'{(cos t, sin t) | 0 ≤ t ≤ 2π}'}</code> でも書けます。積分の記号はどれで書いてもよく、式の下に、範囲と被積分関数から判定した「何の積分か」が出ます</li>
                  <li>ベクトルと行列: 組は列ベクトル。行列の積・<code>det</code>・逆行列・転置・<code>ker A</code>・<code>Im A</code>（像）。行列の中にカーソルを置くと、行・列を足す・消す帯が出ます（Enter で行、<code>,</code> で列、空の行で ⌫ で消す）。行列の定義の行の ⚙ →「座標系」で、線形変換の座標系にできます</li>
                  <li>ベクトル場: <code>(-y, x, 0)</code>・<code>∇f</code>・<code>∇×F</code> は矢印で描きます（⚙ で矢印の数・流線）</li>
                  <li>常微分方程式: <code>x′ = 10(y - x), y′ = x(28 - z) - y, z′ = xy - 8z/3</code>（独立変数 t）、<code>y′ = x - y</code>（傾きの場も）、<code>y″ + y = 0</code>。初期値は式の下の欄（リストで何本も）</li>
                  <li>数列: <code>a_(n+1) = a_n/2 + 1</code> と初項 <code>a_1 = 8</code>。式の中では <code>a_(10)</code>。<code>|z_(20)| = 2</code>（z_(n+1) = z_n² + x + iy）でマンデルブロ集合の縁</li>
                  <li>集合と論理: <code>&#123;(x, y, z) | x^2 + y^2 ≤ 1, z ≥ 0&#125;</code>、<code>∧ ∨ ¬ ⇒ ∩ ∪ ∖ ∈</code>、ℤ・ℕ、<code>⋃</code>・<code>⋂</code></li>
                  <li>∃・∀・max（近似が混ざる）: <code>∃a ∈ [0, 1], x^2 + y^2 + (z - a)^2 = 1</code>、<code>z = max_(t ∈ [0, 1]) sin(xt) y</code></li>
                  <li>名前: ギリシャ文字 <code>α</code>・アクセント <code>x̂</code>・書体 <code>𝒜</code> もスライダーの名前にできます。小さな記号: <code>≡ (mod n)</code>・<code>mod</code>・<code>⊥</code>・<code>∥</code>・共役 <code>z^∗</code>・<code>gcd</code>・<code>lcm</code></li>
                </ul>
              </li>
              <li>式の欄でエンター（⏎）を押すと、すぐ下に新しい式を足します。左端の番号をつまんで上下にドラッグすると並べ替えられます</li>
              <li>式の ⚙ ボタンで、色・格子線・回転を式ごとに変えられます。回転は標準では式ごとに別々で、「全体と同じ」やグループを選ぶと、同じものを選んだ式が一緒に回ります（同期）</li>
                <li>マウスの操作は「⚙ 設定」→「マウス操作」で、操作 A（標準: 左ドラッグで回転、右ドラッグで移動）と操作 B（Google Earth 風）を切り替えられます。右上の「⟲ 視点」で最初の視点に戻ります</li>
              </ul>

            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
  {#if flat}
    <div class="flat-mode" role="radiogroup" aria-label="2D 表示の描き方">
      <button role="radio" aria-checked={!flatSlice} class:selected={!flatSlice}
        title="3次元の図形を、真上から平面に押しつぶした形（影）で描く。z = f(x, y) は値で塗り分ける"
        onclick={() => { if (flatSlice) { flatSlice = false; replotAll(0) } }}>投影</button>
      <button role="radio" aria-checked={flatSlice} class:selected={flatSlice}
        title="縦の座標 = 0 の平面で切って、図形と交わる線を描く（z = f(x, y) なら f = 0 の曲線）"
        onclick={() => { if (!flatSlice) { flatSlice = true; replotAll(0) } }}>切り口</button>
    </div>
  {/if}
  <button
    class="mode-toggle"
    onclick={() => setFlat(!flat)}
    title={flat ? '3D 表示に切り替える' : '2D 表示に切り替える（真上から見た平面に描く）'}
  >
    {flat ? '3D' : '2D'}
  </button>
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: clamp(300px, var(--panel-w, 380px), 70vw) 1fr;
    height: 100dvh;
  }
  .panel {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--border);
    background: var(--panel);
  }
  .panel-resizer {
    position: absolute;
    top: 0;
    right: -4px;
    bottom: 0;
    z-index: 5;
    width: 8px;
    cursor: col-resize;
    touch-action: none;
  }
  .panel-resizer:hover,
  .panel-resizer.active {
    background: color-mix(in srgb, var(--accent, #2f6fb5) 35%, transparent);
  }
  .panel-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    /* PC: 画面上の数式キーボードに隠れる分、下まで送れるようにする */
    padding-bottom: var(--keyboard-h);
  }
  .title {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .grip {
    display: none;
  }
  h1 {
    margin: 0;
    font-size: 18px;
  }
  .graph-title {
    flex: 1;
    min-width: 0;
    font: inherit;
    font-size: 14px;
    color: var(--text);
    background: transparent;
    border: 1px solid transparent;
    border-radius: 6px;
    padding: 3px 6px;
    text-overflow: ellipsis;
  }
  .graph-title:hover,
  .graph-title:focus {
    border-color: var(--border);
    outline: none;
  }
  .graphs-btn {
    flex-shrink: 0;
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    padding: 4px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--panel);
    color: var(--text);
    cursor: pointer;
  }
  .graphs-btn:hover {
    background: var(--hover);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .add-row {
    position: relative;
    flex-wrap: wrap;
    display: flex;
    gap: 8px;
    margin: 8px 16px;
  }
  .add-row .add:not(:first-child) {
    flex: 0 0 auto;
    padding: 8px 14px;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 6px 4px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--gutter);
  }
  .fold,
  .folder-btn {
    flex: none;
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--muted);
    font-size: 15px;
    cursor: pointer;
  }
  .fold:hover,
  .folder-btn:hover {
    background: var(--hover);
    color: var(--text);
  }
  .folder-name {
    flex: 1;
    min-width: 0;
    padding: 3px 6px;
    border: 1px solid transparent;
    border-radius: 6px;
    background: none;
    color: var(--text);
    font-size: 14px;
    font-weight: 600;
  }
  .folder-name:hover,
  .folder-name:focus {
    border-color: var(--border-strong);
    background: var(--bg);
    outline: none;
  }
  .folder-count {
    flex: none;
    font-size: 12px;
    color: var(--muted);
  }
  .folder-count.has-error {
    color: var(--error);
  }
  .matrix-grid {
    flex-basis: 100%;
    display: grid;
    grid-template-columns: repeat(6, 28px);
    gap: 3px;
    justify-content: end;
    padding: 4px 0;
  }
  .matrix-grid .cell {
    width: 28px;
    height: 28px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 3px;
    background: var(--bg);
    cursor: pointer;
  }
  .matrix-grid .cell.on {
    background: var(--accent);
    border-color: var(--accent);
  }
  .matrix-grid .label {
    grid-column: 1 / -1;
    text-align: center;
    font-size: 12px;
    color: var(--muted);
  }
  .add {
    display: block;
    flex: 1;
    padding: 8px;
    border: 1px dashed var(--border-strong);
    border-radius: 8px;
    background: none;
    color: var(--text);
    font-size: 14px;
    cursor: pointer;
  }
  .add:hover {
    background: var(--hover);
  }
  /* 設定（グラフの右上の ⚙ から開くカード） */
  .settings-anchor {
    position: fixed;
    top: 12px;
    right: 68px;
    z-index: 20;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
  }
  .tool-row {
    display: flex;
    gap: 8px;
  }
  .settings-toggle {
    height: 36px;
    padding: 0 12px;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--panel);
    color: var(--text);
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.12);
  }
  /* 開いているとき: 塗りつぶす（式の ⚙ と同じ見せ方） */
  .settings-toggle.on {
    border-color: var(--accent);
    background: var(--accent);
    color: var(--on-accent);
  }
  .settings-card {
    position: absolute;
    top: 44px;
    right: -56px;
    width: min(380px, calc(100vw - 24px));
    max-height: calc(100dvh - 68px - var(--keyboard-h));
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--panel);
    box-shadow: 0 6px 28px rgb(0 0 0 / 0.2);
    overflow: hidden;
  }
  .card-head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 8px 8px 14px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .card-title {
    flex: 1;
    font-weight: 600;
    font-size: 14px;
  }
  .back,
  .close {
    width: 30px;
    height: 30px;
    border: none;
    border-radius: 6px;
    background: none;
    color: var(--muted);
    font-size: 20px;
    line-height: 1;
    cursor: pointer;
  }
  .back {
    margin-left: -8px;
  }
  .back:hover,
  .close:hover {
    background: var(--hover);
  }
  .card-body {
    overflow-y: auto;
    padding: 10px 14px 14px;
  }
  .cat-list {
    display: grid;
    gap: 4px;
  }
  .cat {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px;
    border: none;
    border-radius: 8px;
    background: none;
    color: var(--text);
    text-align: left;
    cursor: pointer;
  }
  .cat:hover {
    background: var(--hover);
  }
  .cat-icon {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    background: var(--hover);
    font-size: 15px;
    flex-shrink: 0;
  }
  .cat-text {
    display: grid;
  }
  .cat-label {
    font-size: 14px;
    font-weight: 600;
  }
  .cat-desc {
    font-size: 11px;
    color: var(--muted);
  }
  .group-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 8px;
  }
  .chip.selected {
    border-color: #2d70b3;
    background: var(--hover);
    font-weight: 600;
  }
  .group-head {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .group-name {
    flex: 1;
    min-width: 0;
    padding: 4px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font-size: 13px;
  }
  .secondary.small {
    padding: 4px 10px;
    font-size: 12px;
  }
  .mode-card {
    display: grid;
    gap: 6px;
    padding: 10px 12px;
    margin-bottom: 8px;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    cursor: pointer;
  }
  .mode-card.selected {
    border-color: #2d70b3;
    background: var(--hover);
  }
  .mode-head {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
    font-size: 14px;
  }
  .mode-rows {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2px 10px;
    font-size: 12px;
    color: var(--muted);
  }
  .mode-key {
    color: var(--text);
    white-space: nowrap;
  }
  .members {
    display: grid;
    gap: 2px;
    margin: 4px 0 10px;
    max-height: 160px;
    overflow-y: auto;
  }
  .member {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .member-group {
    font-size: 11px;
    color: var(--muted);
  }
  .ranges {
    display: grid;
    gap: 6px;
  }
  .range-row {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 8px;
  }
  .axis {
    font: 14px var(--mono);
    color: var(--muted);
  }
  .slider {
    display: grid;
    gap: 4px;
    margin: 14px 0;
    font-size: 13px;
    color: var(--muted);
  }
  .samples {
    margin: 0 16px 8px;
    font-size: 13px;
  }
  .samples summary {
    cursor: pointer;
    color: var(--muted);
    font-weight: 600;
    padding: 4px 0;
  }
  .sample-group {
    margin-top: 6px;
  }
  .sample-title {
    font-size: 11px;
    color: var(--muted);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
  }
  .chip {
    padding: 5px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: var(--bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
  .chip:hover {
    background: var(--hover);
  }
  .note {
    font-size: 11px;
  }
  .buttons {
    display: flex;
    gap: 8px;
  }
  .secondary {
    padding: 6px 12px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    cursor: pointer;
  }
  .error {
    color: var(--error);
    font-size: 12px;
    margin: 6px 0 0;
  }
  .check-row {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    margin: 8px 0;
  }
  .cut-axes {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }
  .secondary.selected {
    border-color: var(--accent, #2d70b3);
    background: var(--hover);
    font-weight: 600;
  }
  .help {
    font-size: 13px;
    color: var(--muted);
  }
  .help ul {
    padding-left: 18px;
    margin: 0;
    line-height: 1.7;
  }
  code {
    font-family: var(--mono);
    background: var(--hover);
    padding: 0 4px;
    border-radius: 4px;
  }
  .viewer {
    position: relative;
    overflow: hidden;
    min-height: 0;
  }
  /* 2D / 3D の切り替え（グラフの右上） */
  .flat-mode {
    position: fixed;
    top: 56px;
    right: 12px;
    display: flex;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    overflow: hidden;
    background: var(--panel);
    z-index: 5;
  }
  .flat-mode button {
    padding: 8px 12px;
    border: none;
    background: none;
    color: var(--muted);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .flat-mode button.selected {
    background: var(--hover);
    color: var(--text);
  }
  .mode-toggle {
    position: fixed;
    top: 12px;
    right: 12px;
    z-index: 5;
    min-width: 44px;
    height: 36px;
    padding: 0 12px;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--panel);
    color: var(--text);
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.12);
  }

  /* 幅が狭いときは下に式の一覧を置く。横向きで高さが限られるときは、式とグラフを横に並べる。 */
  @media (max-width: 560px), (max-width: 720px) and (min-height: 501px) {
    /* スマホ: 設定はグラフを隠さないよう、下から出るシートにする（式の一覧の上に重ねる） */
    .settings-card {
      position: fixed;
      top: auto;
      left: 0;
      right: 0;
      bottom: var(--keyboard-h);
      width: auto;
      height: var(--settings-h);
      max-height: none;
      border-radius: 14px 14px 0 0;
      box-shadow: 0 -4px 24px rgb(0 0 0 / 0.18);
    }
    .app {
      display: block;
    }
    .panel-resizer {
      display: none;
    }
    .viewer {
      position: fixed;
      inset: 0 0 var(--graph-bottom) 0;
    }
    .panel {
      position: fixed;
      left: 0;
      right: 0;
      bottom: var(--keyboard-h);
      z-index: 10;
      height: var(--sheet-h);
      border-right: none;
      border-radius: 14px 14px 0 0;
      box-shadow: 0 -4px 24px rgb(0 0 0 / 0.18);
      /* キーボードを閉じる瞬間に bottom と height が別々にアニメーションして跳ねないようにする。 */
      transition: none;
      overflow: hidden;
    }
    .panel-body {
      padding-bottom: 0; /* シートごとキーボードの上に乗せている */
    }
    .title {
      position: relative;
      padding: 22px 16px 12px;
      touch-action: none; /* ドラッグ中に画面がスクロールしないように */
      cursor: grab;
      user-select: none;
    }
    .grip {
      display: block;
      position: absolute;
      top: 8px;
      left: 50%;
      width: 40px;
      height: 5px;
      margin-left: -20px;
      border-radius: 3px;
      background: var(--border-strong);
    }
  }
  /* ベンチマーク（?bench=1）の結果。左上に重ねる */
  .bench-report {
    position: fixed;
    top: 8px;
    left: 8px;
    z-index: 1000;
    max-width: min(900px, calc(100vw - 16px));
    max-height: 45vh;
    overflow: auto;
    margin: 0;
    padding: 8px 10px;
    font-size: 11px;
    line-height: 1.35;
    background: rgba(255, 255, 255, 0.92);
    color: #1f1f24;
    border: 1px solid #ccc;
    border-radius: 6px;
    white-space: pre;
  }
</style>

<script lang="ts">
  // アプリの画面全体。左に式の一覧、右に3D表示。
  import { onMount, tick } from 'svelte'
  import ExpressionItem from './lib/ExpressionItem.svelte'
  import { latexToText } from './lib/latex'
  import { evalBounds, evalValue, exprError } from './lib/evalExpr'
  import NumberInput from './lib/NumberInput.svelte'
  import EditableValue from './lib/EditableValue.svelte'
  import { clearLocal, loadInitial, saveLocal, shareUrl, type SavedState } from './lib/persist'
  import { complexLatex, coordName, latexName, pairs, prettyName, resizeRotation, resizeSpace, rotDims, sliderStep, snapToStep } from './lib/params'
  import { setKeyboardDims } from './lib/mathSetup'
  import { SAMPLE_GROUPS, type Sample, type SampleView } from './lib/samples'
  import RotationPanel from './lib/RotationPanel.svelte'
  import SpacePanel from './lib/SpacePanel.svelte'
  import { requestCap, requestPlot } from './lib/plotClient'
  import type {
    ComplexMode,
    CutRequest,
    CutStyle,
    Domain,
    Folder,
    GridStyle,
    ParamRanges,
    ParamValue,
    PlotItem,
    RotationGroup,
    SliderStyle,
    Space,
  } from './lib/types'
  import { GLOBAL_GROUP } from './lib/types'
  import { Viewer } from './lib/Viewer'

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
  let controlMode = $state<'A' | 'B'>('B')
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

  onMount(() => {
    viewer = new Viewer(viewerEl)
    ;(async () => {
      // 共有リンクで開いた、またはブラウザに保存してあるなら、その状態に戻す
      const saved = await loadInitial()
      if (saved) restore(saved)
      viewer?.setDark(dark)
      viewer?.setPerspective(flat ? 0 : perspective / 100)
      viewer?.setFlat(flat)
      viewer?.setDomain(domain, true)
      viewer?.setCut(cutRequest())
      loaded = true
      updateSpaceRangeErrors()
      items.forEach(replot)
    })()
    return () => viewer?.dispose()
  })

  // ---- 保存・共有 ----

  /** 保存した状態を読み込み終わったか（それまでは自動保存しない） */
  let loaded = $state(false)

  /** 今の式と設定（保存する分だけ） */
  function snapshot(): SavedState {
    return $state.snapshot({
      version: 1 as const,
      items: items.map((it) => ({
        latex: it.latex,
        color: it.color,
        visible: it.visible,
        opacity: it.opacity,
        grid: it.grid,
        lineWidth: it.lineWidth,
        complexMode: it.complexMode,
        walls: it.walls,
        layers: it.layers,
        ranges: it.ranges,
        vertical: it.vertical,
        slider: it.slider,
        rotGroup: it.rotGroup,
        folder: it.folder,
      })),
      groups,
      folders,
      domain,
      resolution,
      implicitResolution,
      perspective,
      dark,
      space,
      cut,
      flat,
      flatSlice,
      controlMode,
    })
  }

  function restore(s: SavedState) {
    nextId = 1
    items = s.items.map((si) => newItem(si.latex, { ...si }))
    domain = s.domain
    resolution = s.resolution
    implicitResolution = s.implicitResolution
    perspective = s.perspective
    dark = s.dark
    space = s.space
    if (s.cut) cut = s.cut
    flat = s.flat ?? false
    flatSlice = s.flatSlice ?? false
    groups = s.groups ?? []
    folders = s.folders ?? []
    nextFolderId = Math.max(0, ...folders.map((f) => f.id)) + 1
    normalizeFolders()
    if (s.controlMode) controlMode = s.controlMode
    nextGroupId = Math.max(0, ...groups.map((g) => g.id)) + 1
  }

  // 変わるたびに（少し待ってから）ブラウザに保存する
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  $effect(() => {
    if (!loaded) return
    const state = snapshot() // ここで読んだ値が変わると、この処理がもう一度動く
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => saveLocal(state), 500)
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
    if (!confirm('式と設定をすべて最初の状態に戻します。よろしいですか？')) return
    clearLocal()
    location.reload()
  }

  // ---- スマホ用: 下から引き出すシート ----
  //
  // 幅の狭い画面では、式の一覧をグラフの上に重なるシートにする。
  // 上端のつまみをドラッグすると高さが変わり、指を離すと 3 段階（たたむ / 半分 / 全体）のどれかに収まる。
  // つまみをタップすると「たたむ ⇔ 半分」を切り替える。

  type SheetState = 'peek' | 'half' | 'full'
  let sheetState = $state<SheetState>('half')
  let dragHeight = $state<number | null>(null) // ドラッグ中の高さ
  let windowHeight = $state(window.innerHeight)
  // 画面上の数式キーボードの高さ（出ていなければ 0）。シートはキーボードの上に乗せる
  let keyboardHeight = $state(0)
  const kb = window.mathVirtualKeyboard
  kb.addEventListener('geometrychange', () => {
    keyboardHeight = kb.visible ? kb.boundingRect.height : 0
  })
  const snapHeights = $derived({
    peek: 64,
    half: Math.round((windowHeight - keyboardHeight) * 0.45),
    full: Math.max(64, windowHeight - keyboardHeight - 48),
  })
  const sheetHeight = $derived(dragHeight ?? snapHeights[sheetState])

  // シートが重なっている分だけ、グラフの表示を上にずらす（全体まで引き出したときは半分の位置で止める）
  const narrow = window.matchMedia('(max-width: 720px)')
  let isNarrow = $state(narrow.matches)
  narrow.addEventListener('change', () => (isNarrow = narrow.matches))
  /** 設定を開いているか */
  let settingsOpen = $state(false)
  // スマホで設定を開いている間は、設定が下半分を覆うので、グラフはその上に収める
  const bottomInset = $derived(
    isNarrow
      ? (settingsOpen ? snapHeights.half : Math.min(sheetHeight, snapHeights.half)) + keyboardHeight
      : keyboardHeight,
  )
  $effect(() => {
    viewer?.setBottomInset(bottomInset)
  })

  let drag: { startY: number; startH: number; lastY: number; lastT: number; v: number; moved: boolean } | null =
    null

  function sheetDown(e: PointerEvent) {
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    drag = { startY: e.clientY, startH: sheetHeight, lastY: e.clientY, lastT: e.timeStamp, v: 0, moved: false }
  }

  function sheetMove(e: PointerEvent) {
    if (!drag) return
    const dy = e.clientY - drag.startY
    if (Math.abs(dy) > 4) drag.moved = true
    if (!drag.moved) return
    const dt = e.timeStamp - drag.lastT
    if (dt > 0) drag.v = (e.clientY - drag.lastY) / dt // 指の速さ（下向きが正、px/ms）
    drag.lastY = e.clientY
    drag.lastT = e.timeStamp
    dragHeight = Math.min(snapHeights.full, Math.max(snapHeights.peek, drag.startH - dy))
  }

  function sheetUp() {
    if (!drag) return
    if (!drag.moved) {
      sheetState = sheetState === 'peek' ? 'half' : 'peek' // タップ
    } else {
      const h = dragHeight ?? sheetHeight
      const order: SheetState[] = ['peek', 'half', 'full']
      if (drag.v < -0.5) {
        // 上向きにはじいた → 1段上へ
        sheetState = order.find((st) => snapHeights[st] > h + 1) ?? 'full'
      } else if (drag.v > 0.5) {
        // 下向きにはじいた → 1段下へ
        sheetState = [...order].reverse().find((st) => snapHeights[st] < h - 1) ?? 'peek'
      } else {
        // 一番近い段へ
        sheetState = order.reduce((a, b) => (Math.abs(snapHeights[b] - h) < Math.abs(snapHeights[a] - h) ? b : a))
      }
    }
    drag = null
    dragHeight = null
  }

  // ---- 設定（ボタンで項目の一覧を出し、選んだ項目だけを表示する）----

  type SettingsCat = 'dims' | 'rotate' | 'range' | 'detail' | 'look' | 'control' | 'cut' | 'save' | 'help'
  const SETTINGS_CATS: { id: SettingsCat; icon: string; label: string; desc: string }[] = [
    { id: 'rotate', icon: '⟳', label: '回転', desc: '全体・グループごとの回転と再生' },
    { id: 'dims', icon: '⧉', label: '次元', desc: '4次元以上、表示する座標と断面' },
    { id: 'range', icon: '⬚', label: '表示範囲', desc: 'x, y, z の最小値・最大値' },
    { id: 'detail', icon: '▦', label: '細かさ', desc: '曲面・陰関数の細かさ、重ねる枚数' },
    { id: 'look', icon: '◎', label: '見え方', desc: '遠近感、視点のリセット、背景' },
    { id: 'control', icon: '✥', label: 'マウス操作', desc: '操作モード A / B の切り替え' },
    { id: 'cut', icon: '✂', label: '切り口', desc: '立体を平面で切る' },
    { id: 'save', icon: '⇪', label: '保存・共有', desc: '共有リンク、最初の状態に戻す' },
    { id: 'help', icon: '?', label: '書き方のヒント', desc: '式の書き方と操作' },
  ]
  let settingsCat = $state<SettingsCat | null>(null)
  /** 回転の設定で表示しているグループ */
  let rotTab = $state(GLOBAL_GROUP)

  function closeSettings() {
    settingsOpen = false
    settingsCat = null
  }

  /** 式の ⚙ の「回転」から、そのグループの回転の設定を開く */
  function openRotation(gid: number) {
    rotTab = gid
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

  function schedule(item: PlotItem, delay = 200) {
    clearTimeout(timers.get(item.id))
    timers.set(item.id, setTimeout(() => replot(item), delay))
  }

  // ---- 操作中は粗く計算する ----
  // スライダーや複素平面を動かしている間・回転を再生している間は、細かさを半分にして計算する
  // （陰関数は 1/8 の計算量）。指を離したら、粗く計算した式だけを細かく計算し直す。
  let pointerOnControl = false
  const coarseItems = new Set<number>()
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

  async function replot(item: PlotItem) {
    // 非表示の式は計算しない（表示したときに計算する）。パラメータの行は、ほかの式が使うので計算する
    if (!item.visible && !item.param) {
      item.stale = true
      return
    }
    if (running.has(item.id)) {
      again.add(item.id)
      return
    }
    running.add(item.id)
    try {
      await replotNow(item)
    } finally {
      running.delete(item.id)
      if (again.delete(item.id) && items.includes(item)) replot(item)
    }
  }

  /** いま定義されているパラメータ（同じ名前が複数あれば上の行を優先） */
  function currentParams(): Record<string, ParamValue> {
    const out: Record<string, ParamValue> = {}
    for (const it of items) {
      if (it.param && !it.error && !(it.param.name in out)) out[it.param.name] = { re: it.param.re, im: it.param.im }
    }
    return out
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
    items.unshift(newItem(`${name}=1`))
    schedule(items[0], 0)
  }

  /** この行がパラメータでなくなったときの後始末 */
  function dropParam(item: PlotItem) {
    if (!item.param) return
    const name = item.param.name
    item.param = undefined
    paramsChanged([name], item)
  }

  async function replotNow(item: PlotItem) {
    if (!viewer) return
    const token = (latest.get(item.id) ?? 0) + 1
    latest.set(item.id, token)
    item.stale = false
    const coarse = isCoarse()
    if (coarse) coarseItems.add(item.id)
    else coarseItems.delete(item.id)

    // 数式入力欄の LaTeX を、計算エンジン用の式に変換する
    let source: string
    let ranges: ParamRanges
    let spaceRanges: Space['ranges']
    try {
      source = latexToText(item.latex)
      ranges = Object.fromEntries(
        Object.entries(item.ranges).map(([k, [a, b]]) => [k, [latexToText(a), latexToText(b)] as [string, string]]),
      )
      // 座標の「範囲で重ねる」範囲（0 〜 a など）も同じように変換する
      spaceRanges = space.ranges.map((r) => (r ? [latexToText(r[0]), latexToText(r[1])] : null))
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
      item.info = undefined
      item.usedParams = []
      dropParam(item)
      return
    }

    const res = await requestPlot(
      item.id,
      source,
      domain,
      coarse ? Math.max(20, Math.round(resolution / 2)) : resolution,
      coarse ? Math.max(16, Math.round(implicitResolution / 2)) : implicitResolution,
      ranges,
      currentParams(),
      { ...space, ranges: spaceRanges, ...rotationOf(item) },
      currentStacks(),
      cutRequest(),
      item.complexMode ?? 'off',
      flat,
      item.walls ?? false,
      item.layers ?? 1,
      flatSlice,
    )
    if (latest.get(item.id) !== token || !items.includes(item)) return // もっと新しい依頼がある / 削除済み

    if (!res.ok) {
      viewer.removePlot(item.id)
      // 「（8文字目）」のような位置は、変換後の式での位置なので入力欄とは合わない。表示しない
      item.error = res.error.replace(/（\d+文字目）/g, '')
      item.info = undefined
      dropParam(item)
      return
    }

    item.error = undefined
    item.usedParams = res.usedParams

    if (res.param) {
      // パラメータの定義（a = 1 など）: グラフは描かず、値を覚えてスライダーを出す
      viewer.removePlot(item.id)
      const prev = item.param
      item.param = res.param
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
    if (/^(範囲で重ねた立体|媒介変数表示の多変数)/.test(res.kind) && !item.solidSeen) {
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
    viewer.setPlot(item.id, res.mesh, item)
    viewer.setCap(item.id, res.cap, item)
    const size = res.mesh.curve
      ? `${(res.mesh.positions.length / 3).toLocaleString()} 個の点`
      : `${(res.mesh.indices.length / 3).toLocaleString()} 個の三角形`
    item.info = `${res.kind} · ${size} · ${res.ms.toFixed(0)} ms`
    // 陽関数の値がどこでも実数にならない（複素数の関数）なら、色分けで描けることを知らせる
    if (!res.mesh.curve && res.mesh.indices.length === 0 && / = f\(/.test(res.kind) && (item.complexMode ?? 'off') === 'off') {
      item.info += ' · 値が実数になりません。⚙ の「複素数の色分け」で |f| と偏角の色で描けます'
    }
  }

  function replotAll(delay = 200) {
    items.forEach((it) => schedule(it, delay))
  }

  // ---- 式の一覧の操作 ----

  function addItem() {
    const it = newItem('')
    items.push(it)
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
    viewer?.removePlot(item.id)
    items.splice(items.indexOf(item), 1)
    if (item.param) paramsChanged([item.param.name])
  }

  /** 「a のスライダーを追加」: その式のすぐ上に a = 1 の行を足す */
  function addSlider(item: PlotItem, name: string) {
    const k = items.indexOf(item)
    items.splice(k, 0, newItem(`${name}=1`, { folder: item.folder }))
    schedule(items[k], 0) // items[k] は Svelte の状態に入ったもの（元のオブジェクトではない）
  }

  /** スライダー・複素平面でパラメータの値を変えたとき */
  function setParam(item: PlotItem, re: number, im: number, fromPlay = false) {
    if (!item.param) return
    if (!fromPlay) playPos.delete(item.id) // 手で動かしたら、再生はその値から続ける
    const step = sliderStep(item.slider)
    // 刻みを決めているなら、最小値から刻みの整数倍の所にそろえる（Desmos の Step と同じ）
    re = snapToStep(re, item.slider)
    if (im !== 0) im = snapToStep(im, item.slider) // 実数のときの虚部 0 はそのまま
    item.param.re = re
    item.param.im = im
    item.latex = `${latexName(item.param.name)}=${complexLatex(re, im, step)}`
    paramsChanged([item.param.name], item)
  }

  function setLatex(item: PlotItem, latex: string) {
    item.latex = latex
    schedule(item)
  }

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

  function setWalls(item: PlotItem, on: boolean) {
    item.walls = on
    schedule(item, 0)
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
    perspective: 0,
  })
  /** 画面の横・奥・縦の軸に表示している座標の名前 */
  const axisNames = $derived(space.axes.map(coordName))

  // 数式キーボードの「変数」のタブに、今の次元の座標を並べる
  $effect(() => setKeyboardDims(space.dims))

  $effect(() => {
    viewer?.setAxisNames(axisNames.map(prettyName))
  })

  function setLayout(next: Space) {
    // 次元が変わったら、グループの回転も同じ座標の組の角度を引き継いで作り直す
    if (next.dims !== space.dims) {
      groups = groups.map((g) => ({ ...g, ...resizeRotation(g.rotation, g.center, space.dims, next.dims) }))
    }
    space = next
    updateSpaceRangeErrors()
    replotAll(0)
  }

  // ---- 回転のグループ（式ごとの回転・複数の式の同期）----
  //
  // グループに入っていない式は全体の回転（space.rotation, space.center）で描く。
  // 同じグループの式は、そのグループの回転で描く（角度を変えると、まとめて回る）

  let groups = $state<RotationGroup[]>([])
  let nextGroupId = 1

  /** 式が使うグループの番号（グループが無くなっていれば全体） */
  function groupIdOf(item: PlotItem): number {
    const g = item.rotGroup
    return g && groups.some((x) => x.id === g) ? g : GLOBAL_GROUP
  }

  /** グループ gid の回転角と中心 */
  function rotationIn(gid: number): { rotation: number[]; center: number[] } {
    const g = groups.find((x) => x.id === gid)
    return g ? { rotation: g.rotation, center: g.center } : { rotation: space.rotation, center: space.center }
  }

  /** 式を描くときの回転角と中心 */
  function rotationOf(item: PlotItem) {
    return rotationIn(groupIdOf(item))
  }

  /** グループ gid の回転を変える。そのグループの式だけを計算し直す */
  function setRotation(gid: number, rotation: number[], center: number[]) {
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

  /** 式をグループ gid に入れる（'new' なら、この式だけの新しいグループを作る） */
  function setItemGroup(item: PlotItem, gid: number | 'new') {
    const from = groupIdOf(item)
    const to = gid === 'new' ? createGroup(from) : gid
    if (to === from) return
    item.rotGroup = to === GLOBAL_GROUP ? undefined : to
    schedule(item, 0)
  }

  function renameGroup(gid: number, name: string) {
    const g = groups.find((x) => x.id === gid)
    if (g && name.trim()) g.name = name.trim()
  }

  /** グループを解散する（入っていた式は全体の回転に戻る） */
  function removeGroup(gid: number) {
    for (const it of items) {
      if (it.rotGroup === gid) {
        it.rotGroup = undefined
        schedule(it, 0)
      }
    }
    groups = groups.filter((g) => g.id !== gid)
    delete playing[gid]
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
  const plotItems = $derived(items.filter((it) => !it.param))

  // ---- 回転の再生（角度を自動で動かす）----

  /** 再生中の回転（グループの番号 → 回転角の番号） */
  let playing = $state<Record<number, number[]>>({})
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
    if (items.length === 1 && items[0].latex === '') {
      viewer?.removePlot(items[0].id)
      items.splice(0, 1)
    }
    const defined = new Set(items.map((it) => it.param?.name ?? latexParamName(it.latex)))
    const adding = sample.items.filter((si) => !(si.param && defined.has(si.param))) // 同じ名前のパラメータがすでにあれば足さない
    // 式が3つ以上の例は、フォルダにまとめる（4つ以上なら折りたたんでおく）
    const folder = adding.length >= 3 ? addFolder(sample.label.replace(/（.*$/, ''), adding.length >= 4).id : undefined
    for (const si of adding) {
      items.push(newItem(si.latex, { ...si.options, folder }))
      if (!view) schedule(items[items.length - 1], 0)
    }
    if (view) applyView(view)
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
    return /^([a-zA-Z])\s*=/.exec(latex)?.[1]
  }

  /** サンプルの見え方（次元・回転・4次元の遠近感・表示範囲・回転の再生）にして、すべて計算し直す */
  function applyView(view: SampleView) {
    const dims = view.dims
    // グループの回転も、次元に合わせて作り直す
    if (dims !== space.dims) {
      groups = groups.map((g) => ({ ...g, ...resizeRotation(g.rotation, g.center, space.dims, dims) }))
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
      perspective: view.perspective ?? 0,
    }
    // 図形が大きく見えるよう、表示範囲を合わせて視点もリセットする
    const h = view.half
    domain = { xmin: -h, xmax: h, ymin: -h, ymax: h, zmin: -h, zmax: h }
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
    if (item.param && stackChanged) paramsChanged([item.param.name], item)
  }

  /** 表示しない座標の断面の値を変えたとき: その座標を使う式だけを計算し直す */
  function setSlice(k: number, value: number) {
    space.slice[k] = value
    paramsChanged([coordName(k)])
  }

  // ---- 2D 表示 ----

  function setFlat(on: boolean) {
    flat = on
    viewer?.setFlat(on)
    viewer?.setCut(cutRequest())
    // 2D は真上からほぼ平行に見る。3D に戻したら元の遠近感にする
    viewer?.setPerspective(on ? 0 : perspective / 100)
    viewer?.resetCamera()
    replotAll(0)
  }

  // ---- 立体の切り口 ----

  /** 陰関数・範囲で重ねた立体の式（切り口のふたを計算担当が作る） */
  const capKinds = ['陰関数', '範囲で重ねた立体']
  /** 切り口のふたの細かさ（陰関数の格子の2倍） */
  const capResolution = () => (isCoarse() ? implicitResolution : implicitResolution * 2)
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
          const res = await requestCap(it.id, req, capResolution())
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
    replotAll(300)
  }
</script>

<svelte:window
  bind:innerHeight={windowHeight}
  onkeydown={(e) => e.key === 'Escape' && settingsOpen && closeSettings()}
/>

<div class="app">
  <aside
    class="panel"
    class:dragging={dragHeight !== null}
    style:--sheet-h="{sheetHeight}px"
    style:--keyboard-h="{keyboardHeight}px"
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
      onkeydown={(e) => e.key === 'Enter' && (sheetState = sheetState === 'peek' ? 'half' : 'peek')}
    >
      <span class="grip"></span>
      <h1>graphploter</h1>
      <span class="subtitle">3D グラフ</span>
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
            index={i}
            {domain}
            {axisNames}
            onInput={(s) => setLatex(item, s)}
            onColor={(c) => setColor(item, c)}
            onGrid={(g) => setGrid(item, g)}
            onOpacity={(o) => setOpacity(item, o)}
            onComplexMode={(m) => setComplexMode(item, m)}
            onWalls={(on) => setWalls(item, on)}
            onLayers={(n) => setLayers(item, n)}
            onSetDims={(n) => { if (n > space.dims) setLayout(resizeSpace(space, n)) }}
          onLineWidth={(w) => setLineWidth(item, w)}
          onRange={(name, side, v) => setRange(item, name, side, v)}
            onToggle={() => toggle(item)}
            onToggleSettings={() => (item.showSettings = !item.showSettings)}
          onToggleVertical={() => (item.vertical = !item.vertical)}
            onRemove={() => removeItem(item)}
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
            onGroup={(g) => setItemGroup(item, g)}
            onOpenRotation={() => openRotation(groupIdOf(item))}
            {folders}
            onFolder={(fid) => setItemFolder(item, fid)}
          />
          {/if}
        {/each}
      </ul>
      <div class="add-row">
        <button class="add" onclick={addItem}>＋ 式を追加</button>
        <button class="add" onclick={() => addFolder()}>＋ フォルダ</button>
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
  </aside>

  <main class="viewer" bind:this={viewerEl}></main>

  <!-- 設定: ボタンで項目の一覧を出し、選んだ項目だけを表示する（下までスクロールしなくてよいように） -->
  <div
    class="settings-anchor"
    style:--inset="{bottomInset}px"
    style:--half="{snapHeights.half}px"
    style:--keyboard-h="{keyboardHeight}px"
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
                ? 'グループに入っていない式は、この回転で描きます。'
                : '同じグループの式は同じ回転で動きます（同期）。チェックした式がこのグループに入ります。'}
            </p>
            {#if tabGroup || groups.length > 0}
              <div class="members">
                {#each plotItems as it (it.id)}
                  {@const k = items.indexOf(it)}
                  <label class="member">
                    <input type="checkbox" checked={groupIdOf(it) === rotTab}
                      disabled={rotTab === GLOBAL_GROUP && groupIdOf(it) === GLOBAL_GROUP}
                      onchange={(e) => setItemGroup(it, e.currentTarget.checked ? rotTab : GLOBAL_GROUP)} />
                    <span class="dot" style:background={it.color}></span>
                    <span class="member-name">式 {k + 1}</span>
                    {#if groupIdOf(it) !== rotTab}
                      <span class="member-group">（{groupIdOf(it) === GLOBAL_GROUP ? '全体' : groups.find((g) => g.id === groupIdOf(it))?.name}）</span>
                    {/if}
                  </label>
                {/each}
              </div>
            {/if}
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
            />
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


          {:else if settingsCat === 'look'}
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
              ['ホイール・中ドラッグ', '拡大縮小'],
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
            <p class="note">式と設定はこのブラウザに自動で保存され、次に開いたときに元に戻ります。</p>
            <div class="buttons">
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
              <li>式の欄でエンター（⏎）を押すと、すぐ下に新しい式を足します。左端の番号をつまんで上下にドラッグすると並べ替えられます</li>
              <li>式の ⚙ ボタンで、色・格子線・回転を式ごとに変えられます。回転は「グループ」を選ぶと、同じグループの式が一緒に回ります（同期）</li>
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
    grid-template-columns: minmax(300px, 380px) 1fr;
    height: 100dvh;
  }
  .panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--border);
    background: var(--panel);
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
    align-items: baseline;
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
  .subtitle {
    color: var(--muted);
    font-size: 13px;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .add-row {
    display: flex;
    gap: 8px;
    margin: 8px 16px;
  }
  .add-row .add:last-child {
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
  .settings-toggle.on {
    border-color: #2d70b3;
    color: #2d70b3;
  }
  .settings-card {
    position: absolute;
    top: 44px;
    right: -56px;
    width: min(380px, calc(100vw - 24px));
    max-height: calc(100dvh - 68px - var(--inset));
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

  /* スマホなど幅が狭いときは、グラフを全画面にして、式の一覧を下から引き出すシートにする */
  @media (max-width: 720px) {
    /* スマホ: 設定はグラフを隠さないよう、下から出るシートにする（式の一覧の上に重ねる） */
    .settings-card {
      position: fixed;
      top: auto;
      left: 0;
      right: 0;
      bottom: var(--keyboard-h);
      width: auto;
      height: var(--half);
      max-height: none;
      border-radius: 14px 14px 0 0;
      box-shadow: 0 -4px 24px rgb(0 0 0 / 0.18);
    }
    .app {
      display: block;
    }
    .viewer {
      position: fixed;
      inset: 0;
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
      transition:
        height 0.25s ease,
        bottom 0.25s ease;
      overflow: hidden;
    }
.panel-body {
      padding-bottom: 0; /* シートごとキーボードの上に乗せている */
    }
    .panel.dragging {
      transition: none;
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
</style>

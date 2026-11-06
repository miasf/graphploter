<script lang="ts">
  // アプリの画面全体。左に式の一覧、右に3D表示。
  import { onMount } from 'svelte'
  import ExpressionItem from './lib/ExpressionItem.svelte'
  import { latexToText } from './lib/latex'
  import { complexLatex, coordName, latexName, niceStep, prettyName } from './lib/params'
  import SpacePanel from './lib/SpacePanel.svelte'
  import { requestPlot } from './lib/plotClient'
  import type { Domain, GridStyle, ParamRanges, ParamValue, PlotItem, Space } from './lib/types'
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
      grid: { enabled: true, x: 1, y: 1, z: 0, u: 24, v: 24, width: 1 },
      lineWidth: 3,
      showSettings: false,
      vertical: true,
      ranges: { t: ['0', '2\\pi'], u: ['0', '2\\pi'], v: ['0', '2\\pi'] },
      params: [],
      paramSpan: { u: 2 * Math.PI, v: 2 * Math.PI },
      slider: { min: -10, max: 10, complex: false },
      usedParams: [],
      ...options,
    }
  }

  // ---- 画面の状態 ----
  let items = $state<PlotItem[]>([
    // パラメータ（スライダー）。下の式の a を動かせる
    newItem('a=1'),
    // 格子線だけの曲面（面の不透明度 0）
    newItem('z=2ae^{-\\frac{x^2+y^2}{4}}-1', {
      color: '#3fb6c8',
      opacity: 0,
      grid: { enabled: true, x: 0.25, y: 0.25, z: 0, u: 24, v: 24, width: 1 },
    }),
    // 媒介変数表示の曲線（らせん）
    newItem('\\left(4\\cos t,4\\sin t,\\frac{t}{5}\\right)', {
      color: '#c74440',
      ranges: { t: ['-4\\pi', '4\\pi'], u: ['0', '2\\pi'], v: ['0', '2\\pi'] },
    }),
    // 媒介変数表示の曲面（トーラス）
    newItem('\\left(\\left(3+\\cos v\\right)\\cos u,\\left(3+\\cos v\\right)\\sin u,\\sin v\\right)', {
      visible: false,
      color: '#6042a6',
    }),
    newItem('x^2+y^2+z^2=4', { visible: false }),
    newItem('z=\\left|\\Gamma\\left(x+iy\\right)\\right|', { visible: false }),
  ])
  let domain = $state<Domain>({ xmin: -5, xmax: 5, ymin: -5, ymax: 5, zmin: -3, zmax: 3 })
  let resolution = $state(150)
  let implicitResolution = $state(80)
  let domainError = $state('')
  /** 遠近感（0: 平行投影 〜 100: 強い透視投影） */
  let perspective = $state(41)
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
    viewer.setDark(dark)
    viewer.setPerspective(perspective / 100)
    viewer.setDomain(domain)
    items.forEach(replot)
    return () => viewer?.dispose()
  })

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
  $effect(() => {
    viewer?.setBottomInset(isNarrow ? Math.min(sheetHeight, snapHeights.half) + keyboardHeight : 0)
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

  async function replot(item: PlotItem) {
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

  /**
   * パラメータ names が変わった・増えた・消えたとき、それを使う式を計算し直す。
   * エラーの式（未定義だった文字が定義されたかもしれない）と、計算中の式（古い値で計算している）も含める
   */
  function paramsChanged(names: string[], except?: PlotItem) {
    for (const it of items) {
      if (it === except) continue
      if (it.usedParams.some((n) => names.includes(n)) || it.error || running.has(it.id)) schedule(it, 0)
    }
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

    // 数式入力欄の LaTeX を、計算エンジン用の式に変換する
    let source: string
    let ranges: ParamRanges
    try {
      source = latexToText(item.latex)
      ranges = Object.fromEntries(
        Object.entries(item.ranges).map(([k, [a, b]]) => [k, [latexToText(a), latexToText(b)] as [string, string]]),
      )
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

    const res = await requestPlot(source, domain, resolution, implicitResolution, ranges, currentParams(), space)
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
      // 値がスライダーの範囲の外なら、範囲を広げる
      const m = Math.max(Math.abs(res.param.re), Math.abs(res.param.im))
      if (m > item.slider.max || -m < item.slider.min) {
        const r = Math.max(1, 2 ** Math.ceil(Math.log2(m)))
        item.slider.min = -r
        item.slider.max = r
      }
      if (!prev || prev.name !== res.param.name || prev.re !== res.param.re || prev.im !== res.param.im) {
        paramsChanged(prev && prev.name !== res.param.name ? [prev.name, res.param.name] : [res.param.name], item)
      }
      return
    }

    dropParam(item)
    item.params = res.mesh.params
    // 初めて使う媒介変数（t₁ など）の範囲の入力欄を用意する（計算エンジン側も 0〜2π を使っている）
    for (const n of item.params) if (!(n in item.ranges)) item.ranges[n] = ['0', '2\\pi']
    item.paramSpan = res.mesh.paramSpan
    viewer.setPlot(item.id, res.mesh, item)
    const size = res.mesh.curve
      ? `${(res.mesh.positions.length / 3).toLocaleString()} 個の点`
      : `${(res.mesh.indices.length / 3).toLocaleString()} 個の三角形`
    item.info = `${res.kind} · ${size} · ${res.ms.toFixed(0)} ms`
  }

  function replotAll(delay = 200) {
    items.forEach((it) => schedule(it, delay))
  }

  // ---- 式の一覧の操作 ----

  function addItem() {
    const it = newItem('')
    items.push(it)
  }

  function removeItem(item: PlotItem) {
    viewer?.removePlot(item.id)
    items.splice(items.indexOf(item), 1)
    if (item.param) paramsChanged([item.param.name])
  }

  /** 「a のスライダーを追加」: その式のすぐ上に a = 1 の行を足す */
  function addSlider(item: PlotItem, name: string) {
    const k = items.indexOf(item)
    items.splice(k, 0, newItem(`${name}=1`))
    schedule(items[k], 0) // items[k] は Svelte の状態に入ったもの（元のオブジェクトではない）
  }

  /** スライダー・複素平面でパラメータの値を変えたとき */
  function setParam(item: PlotItem, re: number, im: number) {
    if (!item.param) return
    const step = niceStep(item.slider.max - item.slider.min, 200)
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

  function setGrid(item: PlotItem, grid: GridStyle) {
    item.grid = grid
    viewer?.setStyle(item.id, item)
  }

  function toggle(item: PlotItem) {
    item.visible = !item.visible
    viewer?.setStyle(item.id, item)
  }

  // ---- 表示範囲 ----

  const axes = ['x', 'y', 'z'] as const

  // ---- 次元（4次元以上）----

  let space = $state<Space>({ dims: 3, axes: [0, 1, 2], slice: [0, 0, 0], sliceMin: -5, sliceMax: 5 })
  /** 画面の横・奥・縦の軸に表示している座標の名前 */
  const axisNames = $derived(space.axes.map(coordName))

  $effect(() => {
    viewer?.setAxisNames(axisNames.map(prettyName))
  })

  function setLayout(next: Space) {
    space = next
    replotAll(0)
  }

  /** 表示しない座標の断面の値を変えたとき: その座標を使う式だけを計算し直す */
  function setSlice(k: number, value: number) {
    space.slice[k] = value
    paramsChanged([coordName(k)])
  }

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

<svelte:window bind:innerHeight={windowHeight} />

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

      <ul class="list">
        {#each items as item, i (item.id)}
          <ExpressionItem
            {item}
            index={i}
            {domain}
            {axisNames}
            onInput={(s) => setLatex(item, s)}
            onColor={(c) => setColor(item, c)}
            onGrid={(g) => setGrid(item, g)}
            onOpacity={(o) => setOpacity(item, o)}
          onLineWidth={(w) => setLineWidth(item, w)}
          onRange={(name, side, v) => setRange(item, name, side, v)}
            onToggle={() => toggle(item)}
            onToggleSettings={() => (item.showSettings = !item.showSettings)}
          onToggleVertical={() => (item.vertical = !item.vertical)}
            onRemove={() => removeItem(item)}
            onParam={(re, im) => setParam(item, re, im)}
            onSlider={(sl) => (item.slider = sl)}
            onAddSlider={(name) => addSlider(item, name)}
          />
        {/each}
      </ul>
      <button class="add" onclick={addItem}>＋ 式を追加</button>

      <section class="settings">
        <h2>次元</h2>
        <SpacePanel {space} onLayout={setLayout} onSlice={setSlice} />

        <h2>表示範囲</h2>
        <div class="ranges">
          {#each axes as a (a)}
            <label class="range-row">
              <input
                type="number"
                step="any"
                value={domain[`${a}min`]}
                onchange={(e) => setBound(`${a}min`, e.currentTarget.valueAsNumber)}
              />
              <span class="axis">≤ {prettyName(axisNames[axes.indexOf(a)])} ≤</span>
              <input
                type="number"
                step="any"
                value={domain[`${a}max`]}
                onchange={(e) => setBound(`${a}max`, e.currentTarget.valueAsNumber)}
              />
            </label>
          {/each}
        </div>
        {#if domainError}<p class="error">{domainError}</p>{/if}

        <label class="slider">
          <span>曲面の細かさ: {resolution} × {resolution}</span>
          <input
            type="range"
            min="20"
            max="400"
            step="10"
            bind:value={resolution}
            onchange={() => replotAll(0)}
          />
        </label>
        <label class="slider">
          <span>陰関数の細かさ: {implicitResolution}³（重いので注意）</span>
          <input
            type="range"
            min="20"
            max="200"
            step="10"
            bind:value={implicitResolution}
            onchange={() => replotAll(0)}
          />
        </label>

        <h2>見え方</h2>
        <label class="slider">
          <span>遠近感: {perspective === 0 ? '平行投影' : `${perspective}%`}</span>
          <input
            type="range"
            min="0"
            max="100"
            step="1"
            bind:value={perspective}
            oninput={() => viewer?.setPerspective(perspective / 100)}
          />
        </label>
        <div class="buttons">
          <button class="secondary" onclick={() => viewer?.resetCamera()}>視点をリセット</button>
          <button class="secondary" onclick={() => (dark = !dark)}>{dark ? '明るい背景' : '暗い背景'}</button>
        </div>
      </section>

      <details class="help">
        <summary>書き方のヒント</summary>
        <ul>
          <li>陽関数: <code>z = x^2 - y^2</code>（<code>z =</code> は省略可）、<code>y = sin(x) z</code> なども可</li>
          <li>陰関数: <code>x^2 + y^2 + z^2 = 4</code>、<code>sin x + sin y + sin z = 0</code></li>
          <li>曲線: <code>(cos t, sin t, t/4)</code>（t を使う。範囲は式の下で指定）</li>
          <li>曲面: <code>(u cos v, u sin v, u)</code>（u と v を使う）</li>
          <li>掛け算の <code>*</code> は省略可: <code>2xy</code>, <code>x sin(y)</code></li>
          <li>定数: <code>pi</code>, <code>e</code>, <code>i</code>（虚数単位）</li>
          <li>
            関数: sin cos tan arcsin arccos arctan sinh cosh tanh exp ln log sqrt abs floor ceil round sign
          Re Im conj arg gamma、2つの引数: mod(a, b) min max nCr(n, r) nPr(n, r)、階乗: <code>n!</code>
        </li>
          <li>複素数の結果が実数でない点は描かれません</li>
          <li>
          入力欄は数式の見た目のまま編集できます。<code>/</code> で分数、<code>^</code> でべき乗、<code>pi</code> →
          π、<code>sqrt</code> → √ に自動で変わります
        </li>
        <li>スマホでは数式用のキーボードが出ます（123 / f(x) / abc のタブで切り替え）</li>
        <li>
          4次元以上: 「次元」を増やすと座標 w、w₂、w₃… が使えます（<code>w2</code> と打つと w₂）。
          表示する3つの座標を選び、残りはスライダーで固定した断面を描きます
        </li>
        <li>媒介変数は t（曲線）、u と v（曲面）、または t₁ と t₂。組 (…) の外では t, u, v もパラメータにできます</li>
        <li>
          パラメータ: <code>a=1</code> と書くとスライダーになり、ほかの式で <code>a</code> を使えます。
          「複素数」を押すと、複素平面の上で点を動かして値を決められます
        </li>
        <li>⚙ ボタンで色と格子線（方向ごとの間隔）を変えられます</li>
          <li>操作: 左ドラッグで回転、右ドラッグで移動、ホイールで拡大縮小</li>
        </ul>
      </details>
    </div>
  </aside>

  <main class="viewer" bind:this={viewerEl}></main>
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
  .add {
    display: block;
    width: calc(100% - 32px);
    margin: 8px 16px;
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
  .settings {
    padding: 8px 16px 16px;
    border-top: 1px solid var(--border);
    margin-top: 8px;
  }
  h2 {
    font-size: 13px;
    color: var(--muted);
    font-weight: 600;
    margin: 8px 0;
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
  .range-row input {
    width: 100%;
    min-width: 0;
    padding: 5px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: 14px var(--mono);
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
  .help {
    padding: 0 16px 16px;
    font-size: 13px;
    color: var(--muted);
  }
  .help summary {
    cursor: pointer;
    padding: 8px 0;
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

  /* スマホなど幅が狭いときは、グラフを全画面にして、式の一覧を下から引き出すシートにする */
  @media (max-width: 720px) {
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

<script lang="ts">
  // アプリの画面全体。左に式の一覧、右に3D表示。
  import { onMount } from 'svelte'
  import ExpressionItem from './lib/ExpressionItem.svelte'
  import { requestPlot } from './lib/plotClient'
  import type { Domain, GridStyle, PlotItem } from './lib/types'
  import { Viewer } from './lib/Viewer'

  const PALETTE = ['#2d70b3', '#c74440', '#388c46', '#6042a6', '#fa7e19', '#1a9ea0']

  let nextId = 1
  function newItem(source: string, options: Partial<PlotItem> = {}): PlotItem {
    const id = nextId++
    return {
      id,
      source,
      color: PALETTE[(id - 1) % PALETTE.length],
      visible: true,
      opacity: 1,
      grid: { enabled: true, x: 1, y: 1, z: 0, width: 1 },
      showSettings: false,
      ...options,
    }
  }

  // ---- 画面の状態 ----
  let items = $state<PlotItem[]>([
    // 格子線だけの曲面（面の不透明度 0）
    newItem('z = 2exp(-(x^2 + y^2)/4) - 1', {
      color: '#3fb6c8',
      opacity: 0,
      grid: { enabled: true, x: 0.25, y: 0.25, z: 0, width: 1 },
    }),
    newItem('x^2 + y^2 + z^2 = 4', { visible: false }),
    newItem('z = abs(gamma(x + iy))', { visible: false }),
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
  const snapHeights = $derived({
    peek: 64,
    half: Math.round(windowHeight * 0.45),
    full: windowHeight - 48,
  })
  const sheetHeight = $derived(dragHeight ?? snapHeights[sheetState])

  // シートが重なっている分だけ、グラフの表示を上にずらす（全体まで引き出したときは半分の位置で止める）
  const narrow = window.matchMedia('(max-width: 720px)')
  let isNarrow = $state(narrow.matches)
  narrow.addEventListener('change', () => (isNarrow = narrow.matches))
  $effect(() => {
    viewer?.setBottomInset(isNarrow ? Math.min(sheetHeight, snapHeights.half) : 0)
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

  function schedule(item: PlotItem, delay = 200) {
    clearTimeout(timers.get(item.id))
    timers.set(item.id, setTimeout(() => replot(item), delay))
  }

  async function replot(item: PlotItem) {
    if (!viewer) return
    const token = (latest.get(item.id) ?? 0) + 1
    latest.set(item.id, token)

    if (item.source.trim() === '') {
      viewer.removePlot(item.id)
      item.error = undefined
      item.info = undefined
      return
    }

    const res = await requestPlot(item.source, domain, resolution, implicitResolution)
    if (latest.get(item.id) !== token || !items.includes(item)) return // もっと新しい依頼がある / 削除済み

    if (res.ok) {
      viewer.setPlot(item.id, res.mesh, item)
      item.error = undefined
      const tris = res.mesh.indices.length / 3
      item.info = `${res.kind} · ${tris.toLocaleString()} 個の三角形 · ${res.ms.toFixed(0)} ms`
    } else {
      viewer.removePlot(item.id)
      item.error = res.error
      item.info = undefined
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

  function removeItem(item: PlotItem) {
    viewer?.removePlot(item.id)
    items.splice(items.indexOf(item), 1)
  }

  function setSource(item: PlotItem, source: string) {
    item.source = source
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
  <aside class="panel" class:dragging={dragHeight !== null} style:--sheet-h="{sheetHeight}px">
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
            onInput={(s) => setSource(item, s)}
            onColor={(c) => setColor(item, c)}
            onGrid={(g) => setGrid(item, g)}
            onOpacity={(o) => setOpacity(item, o)}
            onToggle={() => toggle(item)}
            onToggleSettings={() => (item.showSettings = !item.showSettings)}
            onRemove={() => removeItem(item)}
          />
        {/each}
      </ul>
      <button class="add" onclick={addItem}>＋ 式を追加</button>

      <section class="settings">
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
              <span class="axis">≤ {a} ≤</span>
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
          <li>掛け算の <code>*</code> は省略可: <code>2xy</code>, <code>x sin(y)</code></li>
          <li>定数: <code>pi</code>, <code>e</code>, <code>i</code>（虚数単位）</li>
          <li>
            関数: sin cos tan arcsin arccos arctan sinh cosh tanh exp ln log sqrt abs Re Im conj arg
            gamma
          </li>
          <li>複素数の結果が実数でない点は描かれません</li>
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
      bottom: 0;
      z-index: 10;
      height: var(--sheet-h);
      border-right: none;
      border-radius: 14px 14px 0 0;
      box-shadow: 0 -4px 24px rgb(0 0 0 / 0.18);
      transition: height 0.25s ease;
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

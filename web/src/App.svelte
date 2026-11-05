<script lang="ts">
  // アプリの画面全体。左に式の一覧、右に3D表示。
  import { onMount } from 'svelte'
  import ExpressionItem from './lib/ExpressionItem.svelte'
  import { requestPlot } from './lib/plotClient'
  import type { Domain, PlotItem } from './lib/types'
  import { Viewer } from './lib/Viewer'

  const PALETTE = ['#2d70b3', '#c74440', '#388c46', '#6042a6', '#fa7e19', '#1a9ea0']

  let nextId = 1
  function newItem(source: string, visible = true): PlotItem {
    const id = nextId++
    return { id, source, color: PALETTE[(id - 1) % PALETTE.length], visible }
  }

  // ---- 画面の状態 ----
  let items = $state<PlotItem[]>([
    newItem('z = sin(x) cos(y)'),
    newItem('z = abs(gamma(x + iy))', false),
  ])
  let domain = $state<Domain>({ xmin: -5, xmax: 5, ymin: -5, ymax: 5, zmin: -3, zmax: 3 })
  let resolution = $state(150)
  let domainError = $state('')

  let viewerEl: HTMLElement
  let viewer: Viewer | null = null

  onMount(() => {
    viewer = new Viewer(viewerEl)
    viewer.setDomain(domain)
    items.forEach(replot)
    return () => viewer?.dispose()
  })

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

    const res = await requestPlot(item.source, domain, resolution)
    if (latest.get(item.id) !== token || !items.includes(item)) return // もっと新しい依頼がある / 削除済み

    if (res.ok) {
      viewer.setPlot(item.id, res.mesh, item.color, item.visible)
      item.error = undefined
      const tris = res.mesh.indices.length / 3
      item.info = `${tris.toLocaleString()} 個の三角形 · ${res.ms.toFixed(0)} ms`
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
    viewer?.setStyle(item.id, item.color, item.visible)
  }

  function toggle(item: PlotItem) {
    item.visible = !item.visible
    viewer?.setStyle(item.id, item.color, item.visible)
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

<div class="app">
  <aside class="panel">
    <header class="title">
      <h1>graphploter</h1>
      <span class="subtitle">3D グラフ</span>
    </header>

    <ul class="list">
      {#each items as item, i (item.id)}
        <ExpressionItem
          {item}
          index={i}
          onInput={(s) => setSource(item, s)}
          onColor={(c) => setColor(item, c)}
          onToggle={() => toggle(item)}
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
        <span>細かさ: {resolution} × {resolution}</span>
        <input
          type="range"
          min="20"
          max="400"
          step="10"
          bind:value={resolution}
          onchange={() => replotAll(0)}
        />
      </label>

      <button class="secondary" onclick={() => viewer?.resetCamera()}>視点をリセット</button>
    </section>

    <details class="help">
      <summary>書き方のヒント</summary>
      <ul>
        <li><code>z = x^2 - y^2</code>（<code>z =</code> は省略可）</li>
        <li>掛け算の <code>*</code> は省略可: <code>2xy</code>, <code>x sin(y)</code></li>
        <li>定数: <code>pi</code>, <code>e</code>, <code>i</code>（虚数単位）</li>
        <li>
          関数: sin cos tan arcsin arccos arctan sinh cosh tanh exp ln log sqrt abs Re Im conj arg
          gamma
        </li>
        <li>複素数の結果が実数でない点は描かれません</li>
        <li>操作: 左ドラッグで回転、右ドラッグで移動、ホイールで拡大縮小</li>
      </ul>
    </details>
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
    overflow-y: auto;
    border-right: 1px solid var(--border);
    background: var(--panel);
  }
  .title {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
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

  /* スマホなど幅が狭いときは上下に並べる */
  @media (max-width: 720px) {
    .app {
      grid-template-columns: 1fr;
      grid-template-rows: 55dvh auto;
      height: auto;
    }
    .viewer {
      order: -1;
      height: 55dvh;
    }
    .panel {
      border-right: none;
    }
  }
</style>

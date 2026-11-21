<script lang="ts">
  import type { LimitResult } from './types'

  let { result, variable }: { result: LimitResult; variable: string } = $props()
  let open = $state(false)
  let epsilon = $state<number | undefined>(0.1)
  let delta = $state<number | undefined>(0.5)
  let threshold = $state<number | undefined>(10)
  let candidate = $state<number | undefined>()
  let canvas = $state<HTMLCanvasElement>()
  let width = $state(0)

  const points = $derived.by(() => {
    const points: [number, number][] = []
    for (let i = 0; i + 1 < result.samples.length; i += 2) {
      points.push([result.samples[i], result.samples[i + 1]])
    }
    return points
  })
  const last = $derived(points.at(-1)?.[0] ?? 1)
  const limit = $derived(candidate ?? (result.value !== undefined && Number.isFinite(result.value) ? result.value : (result.defined ?? 0)))
  const valid = $derived(
    epsilon !== undefined && Number.isFinite(epsilon) && epsilon > 0 && Number.isFinite(limit)
      && (result.sequence ? threshold !== undefined && Number.isInteger(threshold) && threshold >= 1
        : delta !== undefined && Number.isFinite(delta) && delta > 0),
  )
  const selects = (x: number) => {
    if (result.sequence) return x >= threshold!
    const d = x - result.target
    return Math.abs(d) < delta! && d !== 0 && (result.side === 0 || Math.sign(d) === result.side)
  }
  const stats = $derived.by(() => {
    let checked = 0
    let skipped = 0
    let outside = 0
    let example: [number, number] | undefined
    if (valid) {
      for (const p of points) {
        if (!selects(p[0])) continue
        if (!Number.isFinite(p[1])) { skipped++; continue }
        checked++
        if (Math.abs(p[1] - limit) >= epsilon!) {
          outside++
          example ??= p
        }
      }
    }
    return { checked, skipped, outside, example }
  })

  const fmt = (v: number) => Number(v.toPrecision(6)).toString().replaceAll('-', '−')

  $effect(() => {
    if (!canvas || !open) return
    const el = canvas
    const observer = new ResizeObserver(() => { width = el.clientWidth })
    observer.observe(el)
    return () => observer.disconnect()
  })

  $effect(() => {
    if (open && canvas && width > 0 && valid) {
      draw(canvas, width, epsilon!, result.sequence ? threshold! : delta!)
    }
  })

  function draw(el: HTMLCanvasElement, w: number, eps: number, bound: number) {
    const ctx = el.getContext('2d')
    if (!ctx) return
    const h = 154
    const dpr = Math.min(2, devicePixelRatio)
    el.width = Math.round(w * dpr)
    el.height = Math.round(h * dpr)
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    const pad = 12
    const bottom = h - 22
    const a = result.target
    // 関数では差 x−a を先に取って、小さい δ でも中央の軸を見失わない。
    const lo = result.sequence ? Math.max(0, bound >= last / 2 ? Math.min(last - 1, bound / 2) : 0) : -1.45
    const hi = result.sequence ? Math.max(lo + 1, Math.min(last, Math.max(32, 2 * bound))) : 1.45
    const xAt = (x: number) => pad + ((x - lo) / (hi - lo)) * (w - 2 * pad)
    const yAt = (offset: number) => (pad + bottom) / 2 - Math.max(-2.1, Math.min(2.1, offset)) * (bottom - pad) / 4.2
    const middle = result.sequence ? bound : 0
    const start = result.sequence ? bound : result.side > 0 ? 0 : -1
    const end = result.sequence ? hi : result.side < 0 ? 0 : 1
    const topBand = yAt(1)
    const bottomBand = yAt(-1)

    ctx.fillStyle = 'rgba(52, 130, 88, 0.10)'
    if (start <= end) ctx.fillRect(xAt(Math.max(lo, start)), topBand, Math.max(0, xAt(end) - xAt(Math.max(lo, start))), bottomBand - topBand)
    ctx.lineWidth = 1
    ctx.strokeStyle = '#6a8778'
    ctx.setLineDash([4, 3])
    ctx.beginPath()
    for (const y of [topBand, bottomBand]) {
      ctx.moveTo(pad, y)
      ctx.lineTo(w - pad, y)
    }
    for (const x of result.sequence ? [middle] : [start, middle, end]) {
      if (x < lo || x > hi) continue
      ctx.moveTo(xAt(x), pad)
      ctx.lineTo(xAt(x), bottom)
    }
    ctx.stroke()
    ctx.setLineDash([])
    for (const [x, y] of points) {
      const p = result.sequence ? x : (x - a) / bound
      if (p < lo || p > hi || !Number.isFinite(y)) continue
      const active = selects(x)
      ctx.fillStyle = active ? (Math.abs(y - limit) < eps ? '#258558' : '#cf4c50') : '#8994a0'
      ctx.beginPath()
      ctx.arc(xAt(p), yAt((y - limit) / eps), active ? 2.6 : 1.5, 0, 2 * Math.PI)
      ctx.fill()
    }
    ctx.fillStyle = getComputedStyle(el).color
    ctx.font = '11px sans-serif'
    ctx.fillText('L + ε', pad + 2, topBand - 3)
    ctx.fillText('L − ε', pad + 2, bottomBand + 11)
    ctx.textAlign = 'center'
    if (middle >= lo && middle <= hi) ctx.fillText(result.sequence ? 'N' : 'a', xAt(middle), h - 5)
    if (!result.sequence) {
      if (start < 0) ctx.fillText('a−δ', xAt(start), h - 5)
      if (end > 0) ctx.fillText('a+δ', xAt(end), h - 5)
    }
  }
</script>

<details class="lesson" bind:open>
  <summary>{result.sequence ? 'ε–N 論法を試す' : 'ε–δ 論法を試す'}</summary>
  {#if open}
    <p>
      {#if result.sequence}
        任意の ε &gt; 0 に対し、すべての {variable} ≥ N で |a<sub>{variable}</sub> − L| &lt; ε となる N を選びます。
      {:else}
        任意の ε &gt; 0 に対し、{result.side > 0 ? '0 < ' + variable + ' − a < δ' : result.side < 0 ? '0 < a − ' + variable + ' < δ' : '0 < |' + variable + ' − a| < δ'}
        を満たす定義域の点で |f({variable}) − L| &lt; ε となる δ を選びます。
      {/if}
    </p>
    <div class="controls">
      <label>ε <input type="number" inputmode="decimal" step="any" bind:value={epsilon} /></label>
      {#if result.sequence}
        <label>N <input type="number" inputmode="numeric" min="1" step="1" bind:value={threshold} /></label>
      {:else}
        <label>δ <input type="number" inputmode="decimal" step="any" bind:value={delta} /></label>
      {/if}
      <label>L <input type="number" inputmode="decimal" step="any" value={limit} oninput={(event) => {
        const v = event.currentTarget.valueAsNumber
        candidate = Number.isFinite(v) ? v : undefined
      }} /></label>
    </div>
    {#if valid}
      <canvas bind:this={canvas} aria-label={result.sequence ? '数列の項と ε の帯' : '関数の標本と ε、δ の範囲'}></canvas>
      <p aria-live="polite">
        {#if stats.checked === 0}
          この条件を調べられる標本がありません。
        {:else}
          {stats.checked} 点を確認。許容範囲の外は {stats.outside} 点。
          {#if stats.example}<br />例　{variable} = {fmt(stats.example[0])}、値 ≈ {fmt(stats.example[1])}{/if}
        {/if}
        {#if stats.skipped > 0}<br />値を取得できない標本 {stats.skipped} 点。{/if}
      </p>
    {:else}
      <p>{result.sequence ? '正の ε と 1 以上の整数 N' : '正の ε と δ'}を入力してください。</p>
    {/if}
    <p class="hint">緑は帯の内側、赤は条件を満たさない点です。証明では条件内のすべての{result.sequence ? '項' : '点'}について示します。</p>
  {/if}
</details>

<style>
  .lesson { margin-top: 6px; font-size: 12px; }
  summary { cursor: pointer; }
  p { margin: 5px 0; }
  .controls { display: flex; flex-wrap: wrap; gap: 4px 10px; }
  label { display: flex; align-items: center; gap: 4px; }
  input {
    width: 5.6em;
    min-width: 0;
    box-sizing: border-box;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    padding: 2px 3px;
    font: inherit;
  }
  canvas { display: block; width: 100%; height: 154px; color: var(--text); }
  .hint { color: var(--muted); }
</style>

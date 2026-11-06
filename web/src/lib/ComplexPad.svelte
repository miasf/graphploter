<script lang="ts">
  // 複素平面の上で点をドラッグして、複素数の値を決める
  import { niceStep } from './params'

  interface Props {
    re: number
    im: number
    /** 実部・虚部とも、この範囲を表示する */
    min: number
    max: number
    /** 値を丸める刻み */
    step: number
    onChange: (re: number, im: number) => void
  }
  let { re, im, min, max, step, onChange }: Props = $props()

  const SIZE = 200 // SVG の座標の大きさ
  const toX = (v: number) => ((v - min) / (max - min)) * SIZE
  const toY = (v: number) => SIZE - ((v - min) / (max - min)) * SIZE

  // 目盛りの線
  const gridStep = $derived(niceStep(max - min, 8))
  const gridLines = $derived.by(() => {
    const out: number[] = []
    for (let v = Math.ceil(min / gridStep) * gridStep; v <= max + 1e-9; v += gridStep) out.push(v)
    return out
  })

  let svg: SVGSVGElement
  let dragging = false

  function update(e: PointerEvent) {
    const r = svg.getBoundingClientRect()
    const fx = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width))
    const fy = Math.min(1, Math.max(0, (e.clientY - r.top) / r.height))
    const snap = (v: number) => Math.round(v / step) * step
    onChange(snap(min + fx * (max - min)), snap(max - fy * (max - min)))
  }

  function down(e: PointerEvent) {
    svg.setPointerCapture(e.pointerId)
    dragging = true
    update(e)
  }
  function move(e: PointerEvent) {
    if (dragging) update(e)
  }
  function up() {
    dragging = false
  }

  // 点が範囲の外にあるときは、端に寄せて描く
  const clampView = (v: number) => Math.min(max, Math.max(min, v))
</script>

<svg
  bind:this={svg}
  class="pad"
  viewBox="0 0 {SIZE} {SIZE}"
  role="slider"
  aria-label="複素数の値（横が実部、縦が虚部）"
  aria-valuenow={re}
  tabindex="0"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
>
  {#each gridLines as v (v)}
    <line class="grid" x1={toX(v)} y1="0" x2={toX(v)} y2={SIZE} />
    <line class="grid" x1="0" y1={toY(v)} x2={SIZE} y2={toY(v)} />
  {/each}
  {#if min <= 0 && 0 <= max}
    <line class="axis" x1={toX(0)} y1="0" x2={toX(0)} y2={SIZE} />
    <line class="axis" x1="0" y1={toY(0)} x2={SIZE} y2={toY(0)} />
    <text class="label" x={SIZE - 4} y={toY(0) - 4} text-anchor="end">Re</text>
    <text class="label" x={toX(0) + 4} y="12">Im</text>
  {/if}
  <!-- 原点から点までの線（ベクトル） -->
  <line class="arm" x1={toX(clampView(0))} y1={toY(clampView(0))} x2={toX(clampView(re))} y2={toY(clampView(im))} />
  <circle class="point" cx={toX(clampView(re))} cy={toY(clampView(im))} r="7" />
</svg>

<style>
  .pad {
    display: block;
    width: 100%;
    max-width: 240px;
    aspect-ratio: 1;
    border: 1px solid var(--border-strong);
    border-radius: 8px;
    background: var(--bg);
    touch-action: none; /* ドラッグ中に画面がスクロールしないように */
    cursor: crosshair;
  }
  .grid {
    stroke: var(--border);
    stroke-width: 1;
  }
  .axis {
    stroke: var(--muted);
    stroke-width: 1;
  }
  .label {
    fill: var(--muted);
    font: italic 11px serif;
  }
  .arm {
    stroke: var(--accent, #2d70b3);
    stroke-width: 1.5;
    stroke-dasharray: 3 3;
  }
  .point {
    fill: var(--accent, #2d70b3);
    stroke: var(--bg);
    stroke-width: 2;
  }
</style>

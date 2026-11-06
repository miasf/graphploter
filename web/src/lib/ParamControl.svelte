<script lang="ts">
  // パラメータ（a = 1 など）の値を決める部品。
  // 実数モード: スライダー / 複素数モード: 複素平面の上で点を動かす
  import ComplexPad from './ComplexPad.svelte'
  import { formatNumber, niceStep } from './params'
  import type { ParamValue, SliderStyle } from './types'

  interface Props {
    name: string
    value: ParamValue
    slider: SliderStyle
    onChange: (re: number, im: number) => void
    onSlider: (slider: SliderStyle) => void
  }
  let { name, value, slider, onChange, onSlider }: Props = $props()

  /** 値を丸める刻み（範囲のおよそ 1/200） */
  const step = $derived(niceStep(slider.max - slider.min, 200))

  function setBound(key: 'min' | 'max', v: number) {
    if (!Number.isFinite(v)) return
    const next = { ...slider, [key]: v }
    if (next.min < next.max) onSlider(next)
  }

  function toggleComplex() {
    const complex = !slider.complex
    onSlider({ ...slider, complex })
    if (!complex && value.im !== 0) onChange(value.re, 0) // 実数モードに戻すときは虚部を 0 に
  }
</script>

<div class="param">
  {#if slider.complex}
    <ComplexPad re={value.re} im={value.im} min={slider.min} max={slider.max} {step} {onChange} />
    <p class="value">
      {name} = {formatNumber(value.re, step)}
      {value.im < 0 ? '−' : '+'}
      {formatNumber(Math.abs(value.im), step)}<i>i</i>
    </p>
  {:else}
    <input
      class="slider"
      type="range"
      min={slider.min}
      max={slider.max}
      {step}
      value={value.re}
      aria-label="{name} の値"
      oninput={(e) => onChange(e.currentTarget.valueAsNumber, 0)}
    />
  {/if}

  <div class="bounds">
    <input
      type="number"
      step="any"
      value={slider.min}
      aria-label="{name} の最小値"
      onchange={(e) => setBound('min', e.currentTarget.valueAsNumber)}
    />
    <span class="dots">{slider.complex ? '≤ Re, Im ≤' : `≤ ${name} ≤`}</span>
    <input
      type="number"
      step="any"
      value={slider.max}
      aria-label="{name} の最大値"
      onchange={(e) => setBound('max', e.currentTarget.valueAsNumber)}
    />
    <button class="mode" class:on={slider.complex} onclick={toggleComplex} title="複素数の値にする">
      複素数
    </button>
  </div>
</div>

<style>
  .param {
    display: grid;
    gap: 6px;
    margin-top: 6px;
  }
  .slider {
    width: 100%;
  }
  .value {
    margin: 0;
    font: 14px var(--mono);
    color: var(--text);
  }
  .bounds {
    display: grid;
    grid-template-columns: 1fr auto 1fr auto;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }
  .bounds input {
    width: 100%;
    min-width: 0;
    padding: 3px 6px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: 13px var(--mono);
  }
  .dots {
    font-family: var(--mono);
  }
  .mode {
    padding: 3px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: var(--bg);
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
  }
  .mode.on {
    background: #2d70b3;
    border-color: #2d70b3;
    color: #fff;
  }
</style>

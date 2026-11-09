<script lang="ts">
  // パラメータ（a = 1 など）の値を決める部品。
  // 実数モード: スライダー / 複素数モード: 複素平面の上で点を動かす
  import ComplexPad from './ComplexPad.svelte'
  import MathInput from './MathInput.svelte'
  import { formatNumber, missingParam, niceStep, prettyName } from './params'
  import type { ParamValue, SliderStyle } from './types'

  interface Props {
    name: string
    value: ParamValue
    slider: SliderStyle
    /** 範囲の式の誤りの説明 */
    error?: string
    /** 「a のスライダーを追加」を押したとき（範囲の式に未定義の文字があるとき） */
    onAddSlider?: (name: string) => void
    onChange: (re: number, im: number) => void
    onSlider: (slider: SliderStyle) => void
    /** 値を自動で動かしているか */
    playing: boolean
    onPlay: () => void
  }
  let { name, value, slider, error, onAddSlider, onChange, onSlider, playing, onPlay }: Props = $props()

  /** 値を丸める刻み（範囲のおよそ 1/200） */
  const step = $derived(slider.max > slider.min ? niceStep(slider.max - slider.min, 200) : 0.01)

  /** 範囲の式（入力欄の中身）。式が無い古い設定なら、今の数値 */
  const bounds: [string, string] = $derived(slider.bounds ?? [String(slider.min), String(slider.max)])

  /** 範囲の端を式で決める。数値でも、ほかのパラメータを使った式（b, 2b, a+1 など）でもよい */
  function setBound(side: 0 | 1, latex: string) {
    if (bounds[side] === latex) return
    const next: [string, string] = [bounds[0], bounds[1]]
    next[side] = latex
    onSlider({ ...slider, bounds: next })
  }

  function toggleComplex() {
    const complex = !slider.complex
    onSlider({ ...slider, complex, stack: complex ? false : slider.stack })
    if (!complex && value.im !== 0) onChange(value.re, 0) // 実数モードに戻すときは虚部を 0 に
  }

  /** 範囲で重ねる: 最小値〜最大値で動かしたときに図形が通る領域を描く（実数だけ） */
  function toggleStack() {
    const stack = !slider.stack
    onSlider({ ...slider, stack, complex: stack ? false : slider.complex })
    if (stack && value.im !== 0) onChange(value.re, 0)
  }
</script>

<div class="param">
  {#if slider.stack}
    <p class="value">{name} を {formatNumber(slider.min, step)} 〜 {formatNumber(slider.max, step)} で動かして重ねています</p>
  {:else if slider.complex}
    <ComplexPad re={value.re} im={value.im} min={slider.min} max={slider.max} {step} {onChange} />
    <p class="value">
      {name} = {formatNumber(value.re, step)}
      {value.im < 0 ? '−' : '+'}
      {formatNumber(Math.abs(value.im), step)}<i>i</i>
    </p>
  {:else}
    <div class="slider-row">
    <button
      class="play"
      class:on={playing}
      onclick={onPlay}
      aria-label="{name} を{playing ? '止める' : '自動で動かす'}"
      title={playing ? '止める' : '最小値から最大値まで自動で動かす'}>{playing ? '❚❚' : '▶'}</button
    >
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
    </div>
  {/if}

  <div class="bounds">
    <MathInput small value={bounds[0]} label="{name} の最小値" onInput={(v) => setBound(0, v)} />
    <span class="dots">{slider.complex ? '≤ Re, Im ≤' : `≤ ${prettyName(name)} ≤`}</span>
    <MathInput small value={bounds[1]} label="{name} の最大値" onInput={(v) => setBound(1, v)} />
    <button class="mode" class:on={slider.complex} onclick={toggleComplex} title="複素数の値にする">
      複素数
    </button>
  </div>
  {#if error}
    {@const missing = missingParam(error)}
    <p class="error">
      {error}
      {#if missing && onAddSlider}
        <button class="mode" onclick={() => onAddSlider(missing)}>{prettyName(missing)} のスライダーを追加</button>
      {/if}
    </p>
  {/if}
  <button class="mode stack" class:on={slider.stack} onclick={toggleStack} title="範囲で動かしたときに図形が通る領域を描く">
    範囲で重ねる
  </button>
</div>

<style>
  .param {
    display: grid;
    gap: 6px;
    margin-top: 6px;
  }
  .slider-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .slider {
    flex: 1;
    min-width: 0;
  }
  .play {
    width: 28px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--muted);
    font-size: 11px;
    cursor: pointer;
  }
  .play.on {
    background: #2d70b3;
    border-color: #2d70b3;
    color: #fff;
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
  .error {
    margin: 0;
    font-size: 12px;
    color: #c74440;
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
  .stack {
    justify-self: start;
  }
  .mode.on {
    background: #2d70b3;
    border-color: #2d70b3;
    color: #fff;
  }
</style>

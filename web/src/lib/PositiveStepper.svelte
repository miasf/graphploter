<script lang="ts">
  import { onDestroy } from 'svelte'

  let { label, value = $bindable(), integer = false }: { label: string; value: number; integer?: boolean } = $props()
  let draft = $state<string | null>(null)
  let timer: ReturnType<typeof setTimeout> | undefined
  const minimum = $derived(integer ? 1 : Number.MIN_VALUE)
  const maximum = $derived(integer ? Number.MAX_SAFE_INTEGER : Number.MAX_VALUE)
  const factor = 1.25
  const directions = [1, -1] as const

  function step(direction: 1 | -1) {
    draft = null
    const next = integer || value > 1 || (value === 1 && direction === 1)
      ? Math.max(1, value + direction)
      : Math.min(1, direction === 1 ? value * factor : value / factor)
    // 小数の誤差で 0.7999999999999999 のように表示されるのを避ける。
    const rounded = integer || next >= 1e14 ? next : Number(next.toPrecision(15))
    value = Math.max(minimum, Math.min(maximum, Number.isFinite(rounded) ? rounded : next))
  }

  function stop() {
    clearTimeout(timer)
    timer = undefined
  }

  function press(event: PointerEvent, direction: 1 | -1) {
    if (!event.isPrimary || event.button !== 0) return
    stop()
    const button = event.currentTarget as HTMLButtonElement
    button.setPointerCapture(event.pointerId)
    step(direction)
    function repeat() {
      step(direction)
      timer = setTimeout(repeat, 65)
    }
    timer = setTimeout(repeat, 330)
  }

  function edit(input: HTMLInputElement) {
    const text = input.value
    // 0.5 や 1e-6 の入力途中でも、学習グラフには最後の正の値を使う。
    const pattern = integer ? /^\d*$/ : /^(?:\d*\.?\d*)(?:[eE][+-]?\d*)?$/
    if (!pattern.test(text)) {
      input.value = draft ?? String(value)
      return
    }
    draft = text
    const next = Number(text)
    if (next >= minimum && next <= maximum && (!integer || Number.isSafeInteger(next))) value = next
  }

  onDestroy(stop)
</script>

<svelte:window onblur={stop} />

<div class="stepper">
  <span>{label}</span>
  <div class="field">
    <input
      type="text"
      inputmode={integer ? 'numeric' : 'decimal'}
      role="spinbutton"
      aria-label={label}
      aria-valuemin={minimum}
      aria-valuenow={value}
      value={draft ?? String(value)}
      oninput={(event) => edit(event.currentTarget)}
      onblur={() => (draft = null)}
      onkeydown={(event) => {
        if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
          event.preventDefault()
          step(event.key === 'ArrowUp' ? 1 : -1)
        } else if (event.key === 'Enter') {
          draft = null
        }
      }}
    />
    <div class="buttons">
      {#each directions as direction}
        <button
          type="button"
          aria-label={label + (direction === 1 ? ' を増やす' : ' を減らす')}
          onpointerdown={(event) => press(event, direction)}
          onpointerup={stop}
          onpointercancel={stop}
          onlostpointercapture={stop}
          onclick={(event) => { if (event.detail === 0) step(direction) }}
        >{direction === 1 ? '▴' : '▾'}</button>
      {/each}
    </div>
  </div>
</div>

<style>
  .stepper, .field { display: flex; align-items: center; }
  .stepper { gap: 4px; }
  .field {
    width: 5.6em;
    box-sizing: border-box;
    background: var(--bg);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
  }
  .field:focus-within { outline: 1px solid var(--text); }
  input {
    flex: 1;
    width: 0;
    min-width: 0;
    padding: 2px 3px;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
  }
  .buttons { display: grid; align-self: stretch; grid-template-rows: 1fr 1fr; }
  button {
    width: 17px;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text);
    font: 11px/1 sans-serif;
    cursor: pointer;
    touch-action: none;
    user-select: none;
  }
  button:hover, button:active { background: var(--hover); }
</style>

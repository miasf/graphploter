<script lang="ts">
  // 色の選択。ブラウザの色選択（<input type="color">）は、スマホでは「カスタム」を開くと今の色から始まらないことがあるので、
  // アプリの中で作る。よく使う色と、今の色から始まる「カスタム」（彩度・明るさの面、色相の帯、16 進の入力）

  let { value, onChange }: { value: string; onChange: (color: string) => void } = $props()

  const PRESETS = [
    '#c74440', '#fa7e19', '#e6b800', '#388c46', '#1a9ea0', '#2d70b3',
    '#6042a6', '#b5338a', '#7a5230', '#000000', '#6b6b75', '#ffffff',
  ]

  let open = $state(false)
  let custom = $state(false)
  /** カスタムの色（色相 0〜360、彩度・明るさ 0〜1）。開いたときに今の色から決める */
  let h = $state(0)
  let s = $state(0)
  let v = $state(0)
  let hexText = $state('')

  function toHsv(hex: string): [number, number, number] {
    const n = parseInt(hex.slice(1), 16)
    const [r, g, b] = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map((c) => c / 255)
    const max = Math.max(r, g, b)
    const d = max - Math.min(r, g, b)
    let hue = 0
    if (d > 0) {
      if (max === r) hue = ((g - b) / d) % 6
      else if (max === g) hue = (b - r) / d + 2
      else hue = (r - g) / d + 4
    }
    return [(hue * 60 + 360) % 360, max > 0 ? d / max : 0, max]
  }

  function toHex(hue: number, sat: number, val: number): string {
    const f = (k: number) => {
      const x = (k + hue / 60) % 6
      return val - val * sat * Math.max(0, Math.min(x, 4 - x, 1))
    }
    return '#' + [f(5), f(3), f(1)].map((c) => Math.round(c * 255).toString(16).padStart(2, '0')).join('')
  }

  const valid = (hex: string) => /^#[0-9a-f]{6}$/i.test(hex)

  /** 今の色からカスタムの値を決め直す */
  function syncFrom(hex: string) {
    if (!valid(hex)) return
    ;[h, s, v] = toHsv(hex.toLowerCase())
    hexText = hex.toLowerCase()
  }

  function toggle() {
    open = !open
    if (open) syncFrom(value)
  }

  function openCustom() {
    custom = !custom
    if (custom) syncFrom(value) // 今設定している色から始める
  }

  function pick(hex: string) {
    syncFrom(hex)
    onChange(hex)
  }

  function emit() {
    const hex = toHex(h, s, v)
    hexText = hex
    onChange(hex)
  }

  /** 面・帯をドラッグしている間、指・カーソルの位置から値を決める */
  function drag(e: PointerEvent, apply: (x: number, y: number) => void) {
    const el = e.currentTarget as HTMLElement
    el.setPointerCapture(e.pointerId)
    const at = (ev: PointerEvent) => {
      const r = el.getBoundingClientRect()
      const x = Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width))
      const y = Math.min(1, Math.max(0, (ev.clientY - r.top) / r.height))
      apply(x, y)
      emit()
    }
    at(e)
    const move = (ev: PointerEvent) => at(ev)
    const up = () => {
      el.removeEventListener('pointermove', move)
      el.removeEventListener('pointerup', up)
      el.removeEventListener('pointercancel', up)
    }
    el.addEventListener('pointermove', move)
    el.addEventListener('pointerup', up)
    el.addEventListener('pointercancel', up)
  }
</script>

<button class="swatch current" style:background={value} aria-label="色を選ぶ" aria-expanded={open} onclick={toggle}></button>
{#if open}
  <div class="picker">
    <div class="presets">
      {#each PRESETS as c (c)}
        <button
          class="swatch"
          class:selected={c === value.toLowerCase()}
          style:background={c}
          aria-label={c}
          onclick={() => pick(c)}
        ></button>
      {/each}
      <button class="custom-btn" class:on={custom} onclick={openCustom}>カスタム</button>
    </div>
    {#if custom}
      <div
        class="sv"
        style:background-color={toHex(h, 1, 1)}
        role="slider"
        tabindex="0"
        aria-label="彩度と明るさ"
        aria-valuenow={Math.round(s * 100)}
        onpointerdown={(e) => drag(e, (x, y) => ((s = x), (v = 1 - y)))}
      >
        <div class="knob" style:left="{s * 100}%" style:top="{(1 - v) * 100}%" style:background={toHex(h, s, v)}></div>
      </div>
      <div
        class="hue"
        role="slider"
        tabindex="0"
        aria-label="色相"
        aria-valuenow={Math.round(h)}
        onpointerdown={(e) => drag(e, (x) => (h = Math.min(359.9, x * 360)))}
      >
        <div class="knob" style:left="{(h / 360) * 100}%" style:top="50%" style:background={toHex(h, 1, 1)}></div>
      </div>
      <label class="hex">
        <span>16 進</span>
        <input
          type="text"
          value={hexText}
          maxlength="7"
          spellcheck="false"
          oninput={(e) => {
            let t = e.currentTarget.value.trim()
            if (!t.startsWith('#')) t = '#' + t
            if (valid(t)) pick(t.toLowerCase())
          }}
        />
      </label>
    {/if}
  </div>
{/if}

<style>
  .swatch {
    width: 24px;
    height: 24px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 50%;
    cursor: pointer;
  }
  .swatch.current {
    width: 32px;
    height: 22px;
    border-radius: 4px;
  }
  .swatch.selected {
    outline: 2px solid var(--text);
    outline-offset: 2px;
  }
  .picker {
    flex-basis: 100%;
    display: grid;
    gap: 8px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
  }
  .presets {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }
  .custom-btn {
    font: inherit;
    font-size: 0.9em;
    padding: 2px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 12px;
    background: var(--bg);
    color: var(--text);
    cursor: pointer;
  }
  .custom-btn.on {
    border-color: var(--text);
  }
  .sv {
    position: relative;
    height: 120px;
    border-radius: 6px;
    background-image: linear-gradient(to top, #000, transparent), linear-gradient(to right, #fff, transparent);
    touch-action: none;
    cursor: crosshair;
  }
  .hue {
    position: relative;
    height: 16px;
    border-radius: 8px;
    background: linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00);
    touch-action: none;
    cursor: pointer;
  }
  .knob {
    position: absolute;
    width: 14px;
    height: 14px;
    border: 2px solid #fff;
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.5);
    transform: translate(-50%, -50%);
    pointer-events: none;
  }
  .hex {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--muted);
  }
  .hex input {
    width: 7em;
    font: inherit;
    font-family: ui-monospace, monospace;
    padding: 2px 6px;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
  }
</style>

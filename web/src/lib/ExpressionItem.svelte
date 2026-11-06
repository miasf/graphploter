<script lang="ts">
  // 式の一覧の1行
  import { indexToSpacing, spacingOptions, spacingToIndex } from './gridSpacing'
  import { joinTuple, splitTuple } from './latex'
  import MathInput from './MathInput.svelte'
  import ParamControl from './ParamControl.svelte'
  import { missingParam, prettyName } from './params'
  import type { Domain, GridStyle, PlotItem, SliderStyle } from './types'

  interface Props {
    item: PlotItem
    index: number
    /** 表示範囲（格子線の間隔の選択肢を決めるのに使う） */
    domain: Domain
    /** 画面の横・奥・縦の軸に表示している座標の名前（ふつうは x, y, z） */
    axisNames: string[]
    /** 式（LaTeX）が書き換えられたとき */
    onInput: (latex: string) => void
    onColor: (color: string) => void
    onGrid: (grid: GridStyle) => void
    onOpacity: (opacity: number) => void
    onLineWidth: (width: number) => void
    onRange: (name: string, side: 0 | 1, value: string) => void
    onToggle: () => void
    onToggleSettings: () => void
    onToggleVertical: () => void
    onRemove: () => void
    /** パラメータの値を変えたとき */
    onParam: (re: number, im: number) => void
    onSlider: (slider: SliderStyle) => void
    /** 「a のスライダーを追加」を押したとき */
    onAddSlider: (name: string) => void
  }
  let {
    item,
    index,
    domain,
    axisNames,
    onInput,
    onColor,
    onGrid,
    onOpacity,
    onLineWidth,
    onRange,
    onToggle,
    onToggleSettings,
    onToggleVertical,
    onRemove,
    onParam,
    onSlider,
    onAddSlider,
  }: Props = $props()

  /** 未定義の文字を使っていてエラーになっているなら、その文字（スライダーを追加できる） */
  const missing = $derived(missingParam(item.error))

  // 1行の入力欄を編集している間は、縦の表示に切り替えない（入力中に欄が入れ替わらないように）
  let editingLine = $state(false)
  /** 式が組 (a, b, c) なら成分の配列 */
  const parts = $derived(splitTuple(item.latex))
  const showVertical = $derived(item.vertical && parts !== null && !editingLine)
  const COMPONENT_NAMES = ['x', 'y', 'z']

  function setPart(k: number, value: string) {
    if (!parts) return
    const next = [...parts]
    next[k] = value
    onInput(joinTuple(next))
  }

  /** 媒介変数表示の曲面で選べる分割数（0 は線なし） */
  const DIVISIONS = [0, 1, 2, 3, 4, 6, 8, 10, 12, 16, 20, 24, 30, 36, 48, 60, 72, 96, 120]
  const divIndex = (n: number) =>
    DIVISIONS.reduce((best, d, k) => (Math.abs(d - n) < Math.abs(DIVISIONS[best] - n) ? k : best), 0)

  /** 範囲の入力欄を出す媒介変数（媒介変数表示のときだけ） */
  const paramNames = $derived(item.params.filter((n) => n in item.ranges))
  const isCurve = $derived(item.params.length === 1)
  const isSurface = $derived(item.params.length === 2)

  // 格子線の方向（grid.x, grid.y, grid.z は、画面の横・奥・縦の軸の方向）
  const gridAxes = $derived([
    { axis: 'x', label: `${prettyName(axisNames[0])} 方向` },
    { axis: 'y', label: `${prettyName(axisNames[1])} 方向` },
    { axis: 'z', label: `${prettyName(axisNames[2])}（等高線）` },
  ] as const)

  const ranges = $derived({
    x: domain.xmax - domain.xmin,
    y: domain.ymax - domain.ymin,
    z: domain.zmax - domain.zmin,
  })
</script>

<li class="item" class:has-error={!!item.error}>
  <div class="gutter">
    <span class="index">{index + 1}</span>
    {#if !item.param}
      <button
      class="swatch"
      class:off={!item.visible}
      style:--c={item.color}
      title={item.visible ? '非表示にする' : '表示する'}
      aria-label={item.visible ? '非表示にする' : '表示する'}
      onclick={onToggle}
    ></button>
    {/if}
  </div>

  <div class="body">
    {#if showVertical && parts}
      <!-- 媒介変数表示: 成分ごとに縦に並べる -->
      <div class="components">
        {#each parts as part, k (k)}
          <div class="component">
            <span class="name">{COMPONENT_NAMES[k] ?? '?'} =</span>
            <MathInput
              value={part}
              label="{COMPONENT_NAMES[k] ?? '?'} 成分"
              keyboardToggle={k === 0}
              onInput={(v) => setPart(k, v)}
            />
          </div>
        {/each}
      </div>
      <button class="link" onclick={onToggleVertical}>1行で編集</button>
    {:else}
      <MathInput
        value={item.latex}
        label="式 {index + 1}"
        placeholder="z=\sin x\cos y"
        onInput={(v) => onInput(v)}
        onFocusChange={(f) => (editingLine = f)}
      />
      {#if parts && !item.vertical}
        <button class="link" onclick={onToggleVertical}>縦に並べる</button>
      {/if}
    {/if}
    {#each paramNames as name (name)}
      <div class="param-range">
        <MathInput
          small
          value={item.ranges[name][0]}
          label="{name} の最小値"
          onInput={(v) => onRange(name, 0, v)}
        />
        <span>≤ {prettyName(name)} ≤</span>
        <MathInput
          small
          value={item.ranges[name][1]}
          label="{name} の最大値"
          onInput={(v) => onRange(name, 1, v)}
        />
      </div>
    {/each}
    {#if item.param && !item.error}
      <ParamControl name={item.param.name} value={item.param} slider={item.slider} onChange={onParam} {onSlider} />
    {/if}
    {#if item.error}
      <p class="error">{item.error}</p>
      {#if missing}
        <button class="add-slider" onclick={() => onAddSlider(missing)}>＋ {missing} のスライダーを追加</button>
      {/if}
    {:else if item.info && !item.param}
      <p class="info">{item.info}</p>
    {/if}

    {#if item.showSettings && !item.param}
      <div class="settings">
        <label class="row">
          <span class="label">色</span>
          <input
            class="color"
            type="color"
            value={item.color}
            oninput={(e) => onColor(e.currentTarget.value)}
          />
        </label>
        {#if isCurve}
          <div class="sliders">
            <span class="label">線の太さ</span>
            <input
              type="range"
              min="1"
              max="10"
              step="0.5"
              value={item.lineWidth}
              oninput={(e) => onLineWidth(e.currentTarget.valueAsNumber)}
            />
            <span class="value">{item.lineWidth} px</span>
          </div>
        {:else}
          <div class="sliders">
            <span class="label">面の濃さ</span>
            <input
              type="range"
              min="0"
              max="1"
              step="0.05"
              value={item.opacity}
              oninput={(e) => onOpacity(e.currentTarget.valueAsNumber)}
            />
            <span class="value">{item.opacity === 0 ? '格子のみ' : `${Math.round(item.opacity * 100)}%`}</span>
          </div>

          <label class="check">
            <input
              type="checkbox"
              checked={item.grid.enabled}
              onchange={(e) => onGrid({ ...item.grid, enabled: e.currentTarget.checked })}
            />
            格子線
          </label>
          <div class="sliders" class:disabled={!item.grid.enabled}>
            {#if isSurface}
              <!-- 媒介変数表示の曲面: 2つの媒介変数（u, v など）の範囲を何等分するか -->
              {#each ['u', 'v'] as const as axis, n (axis)}
                {@const k = divIndex(item.grid[axis])}
                <span class="label">{prettyName(item.params[n])} 方向</span>
                <input
                  type="range"
                  min="0"
                  max={DIVISIONS.length - 1}
                  step="1"
                  value={k}
                  disabled={!item.grid.enabled}
                  oninput={(e) => onGrid({ ...item.grid, [axis]: DIVISIONS[e.currentTarget.valueAsNumber] })}
                />
                <span class="value">{k === 0 ? 'なし' : `${DIVISIONS[k]} 等分`}</span>
              {/each}
            {/if}
            {#each isSurface ? gridAxes.slice(2) : gridAxes as { axis, label } (axis)}
              {@const options = spacingOptions(ranges[axis])}
              {@const index = spacingToIndex(item.grid[axis], options)}
              <span class="label">{label}</span>
              <input
                type="range"
                min="0"
                max={options.length}
                step="1"
                value={index}
                disabled={!item.grid.enabled}
                oninput={(e) =>
                  onGrid({ ...item.grid, [axis]: indexToSpacing(e.currentTarget.valueAsNumber, options) })}
              />
              <span class="value">{index === 0 ? 'なし' : `間隔 ${indexToSpacing(index, options)}`}</span>
            {/each}
            <span class="label">線の太さ</span>
            <input
              type="range"
              min="0.5"
              max="4"
              step="0.25"
              value={item.grid.width}
              disabled={!item.grid.enabled}
              oninput={(e) => onGrid({ ...item.grid, width: e.currentTarget.valueAsNumber })}
            />
            <span class="value">{item.grid.width} px</span>
          </div>
          <p class="hint">スライダーを右にするほど線が細かくなります。左端で線なし。</p>
        {/if}
      </div>
    {/if}
  </div>

  <div class="actions">
    {#if !item.param}
      <button
      class="icon"
      class:active={item.showSettings}
      title="見た目の設定"
      aria-label="見た目の設定"
      aria-expanded={item.showSettings}
      onclick={onToggleSettings}>⚙</button
    >
    {/if}
    <button class="icon" title="削除" aria-label="削除" onclick={onRemove}>×</button>
  </div>
</li>

<style>
  .item {
    display: grid;
    grid-template-columns: 44px 1fr auto;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .gutter {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 8px 0;
    background: var(--gutter);
    border-left: 3px solid transparent;
  }
  .has-error .gutter {
    border-left-color: var(--error);
  }
  .index {
    font-size: 11px;
    color: var(--muted);
  }
  .swatch {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2px solid var(--c);
    background: var(--c);
    cursor: pointer;
    padding: 0;
  }
  .swatch.off {
    background: transparent;
  }
  .body {
    padding: 10px 8px;
    min-width: 0;
  }
  .error,
  .info {
    margin: 4px 0 0;
    font-size: 12px;
  }
  .error {
    color: var(--error);
  }
  .info {
    color: var(--muted);
  }
  .add-slider {
    margin-top: 6px;
    padding: 4px 10px;
    border: 1px solid #2d70b3;
    border-radius: 999px;
    background: none;
    color: #2d70b3;
    font-size: 13px;
    cursor: pointer;
  }
  .settings {
    margin-top: 10px;
    padding: 10px;
    border-radius: 8px;
    background: var(--gutter);
    font-size: 13px;
    display: grid;
    gap: 8px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .label {
    color: var(--muted);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text);
    cursor: pointer;
  }
  .color {
    width: 32px;
    height: 22px;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
  }
  .components {
    display: grid;
    gap: 2px;
  }
  .component {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: baseline;
    gap: 6px;
  }
  .name {
    font: italic 15px 'Times New Roman', serif;
    color: var(--muted);
    min-width: 1.6em;
  }
  .link {
    margin-top: 2px;
    padding: 0;
    border: none;
    background: none;
    color: var(--muted);
    font-size: 12px;
    text-decoration: underline;
    cursor: pointer;
  }
  .param-range {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font: 13px var(--mono);
    color: var(--muted);
  }
  .sliders {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 6px 10px;
  }
  .sliders.disabled {
    opacity: 0.5;
  }
  .sliders input {
    width: 100%;
    min-width: 0;
  }
  .value {
    min-width: 5.5em;
    font: 12px var(--mono);
    color: var(--muted);
  }
  .hint {
    margin: 0;
    font-size: 11px;
    color: var(--muted);
  }
  .actions {
    display: flex;
    align-items: flex-start;
    gap: 2px;
    padding: 8px 6px 0 0;
  }
  .icon {
    width: 28px;
    height: 28px;
    border: none;
    background: none;
    color: var(--muted);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
    border-radius: 6px;
  }
  .icon:hover,
  .icon.active {
    background: var(--hover);
    color: var(--text);
  }
</style>

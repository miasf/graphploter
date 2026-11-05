<script lang="ts">
  // 式の一覧の1行
  import { indexToSpacing, spacingOptions, spacingToIndex } from './gridSpacing'
  import type { Domain, GridStyle, PlotItem } from './types'

  interface Props {
    item: PlotItem
    index: number
    /** 表示範囲（格子線の間隔の選択肢を決めるのに使う） */
    domain: Domain
    onInput: (source: string) => void
    onColor: (color: string) => void
    onGrid: (grid: GridStyle) => void
    onOpacity: (opacity: number) => void
    onToggle: () => void
    onToggleSettings: () => void
    onRemove: () => void
  }
  let { item, index, domain, onInput, onColor, onGrid, onOpacity, onToggle, onToggleSettings, onRemove }: Props =
    $props()

  const gridAxes = [
    { axis: 'x', label: 'x 方向' },
    { axis: 'y', label: 'y 方向' },
    { axis: 'z', label: 'z（等高線）' },
  ] as const

  const ranges = $derived({
    x: domain.xmax - domain.xmin,
    y: domain.ymax - domain.ymin,
    z: domain.zmax - domain.zmin,
  })
</script>

<li class="item" class:has-error={!!item.error}>
  <div class="gutter">
    <span class="index">{index + 1}</span>
    <button
      class="swatch"
      class:off={!item.visible}
      style:--c={item.color}
      title={item.visible ? '非表示にする' : '表示する'}
      aria-label={item.visible ? '非表示にする' : '表示する'}
      onclick={onToggle}
    ></button>
  </div>

  <div class="body">
    <input
      class="source"
      type="text"
      value={item.source}
      placeholder="例: z = sin(x) cos(y)、x^2 + y^2 + z^2 = 9"
      spellcheck="false"
      autocomplete="off"
      oninput={(e) => onInput(e.currentTarget.value)}
    />
    {#if item.error}
      <p class="error">{item.error}</p>
    {:else if item.info}
      <p class="info">{item.info}</p>
    {/if}

    {#if item.showSettings}
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
          {#each gridAxes as { axis, label } (axis)}
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
      </div>
    {/if}
  </div>

  <div class="actions">
    <button
      class="icon"
      class:active={item.showSettings}
      title="見た目の設定"
      aria-label="見た目の設定"
      aria-expanded={item.showSettings}
      onclick={onToggleSettings}>⚙</button
    >
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
  .source {
    width: 100%;
    border: none;
    outline: none;
    background: transparent;
    color: var(--text);
    font: 17px/1.4 var(--mono);
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

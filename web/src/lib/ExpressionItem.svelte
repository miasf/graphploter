<script lang="ts">
  // 式の一覧の1行
  import type { PlotItem } from './types'

  interface Props {
    item: PlotItem
    index: number
    onInput: (source: string) => void
    onColor: (color: string) => void
    onToggle: () => void
    onRemove: () => void
  }
  let { item, index, onInput, onColor, onToggle, onRemove }: Props = $props()
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
      placeholder="例: z = sin(x) cos(y)"
      spellcheck="false"
      autocomplete="off"
      oninput={(e) => onInput(e.currentTarget.value)}
    />
    {#if item.error}
      <p class="error">{item.error}</p>
    {:else if item.info}
      <p class="info">{item.info}</p>
    {/if}
  </div>

  <div class="actions">
    <input
      class="color"
      type="color"
      value={item.color}
      title="色を変える"
      aria-label="色を変える"
      oninput={(e) => onColor(e.currentTarget.value)}
    />
    <button class="remove" title="削除" aria-label="削除" onclick={onRemove}>×</button>
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
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
    padding-right: 6px;
  }
  .color {
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
  }
  .remove {
    width: 28px;
    height: 28px;
    border: none;
    background: none;
    color: var(--muted);
    font-size: 20px;
    cursor: pointer;
    border-radius: 6px;
  }
  .remove:hover {
    background: var(--hover);
    color: var(--text);
  }
</style>

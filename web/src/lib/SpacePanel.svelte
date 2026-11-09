<script lang="ts">
  // 次元の設定: 次元の数、画面の横・奥・縦に表示する座標、表示しない座標の断面の値
  import EditableValue from './EditableValue.svelte'
  import MathInput from './MathInput.svelte'
  import { coordName, formatNumber, missingParam, niceStep, prettyName, resizeSpace } from './params'
  import type { Space } from './types'

  interface Props {
    space: Space
    /** 次元や、表示する座標の割り当てが変わったとき（すべて計算し直す） */
    onLayout: (space: Space) => void
    /** 表示しない座標 k の断面の値が変わったとき */
    onSlice: (k: number, value: number) => void
    /** 「範囲」の式の誤り（座標の番号 → 説明） */
    rangeErrors: Record<number, string>
    /** 「a のスライダーを追加」を押したとき */
    onAddSlider: (name: string) => void
  }
  let { space, onLayout, onSlice, rangeErrors, onAddSlider }: Props = $props()

  const ROLES = ['横', '奥', '縦'] as const
  const step = $derived(niceStep(space.sliceMax - space.sliceMin, 200))

  /** 役割の番号: 0〜2 は画面の軸、FIXED は固定（断面）、RANGE は範囲で重ねる */
  const FIXED = -1
  const RANGE = -2

  /** 座標 k の役割 */
  const roleOf = (k: number) => {
    const a = space.axes.indexOf(k)
    if (a >= 0) return a
    return space.ranges[k] ? RANGE : FIXED
  }

  /** 次元を変える。回転角は、同じ座標の組のものを引き継ぐ */
  function setDims(n: number) {
    onLayout(resizeSpace(space, n))
  }

  /** 範囲の端を式（LaTeX）で決める。0 〜 a のようにパラメータも使える（a のスライダーで範囲が動く） */
  function setRange(k: number, side: 0 | 1, v: string) {
    const cur = space.ranges[k]
    if (!cur || cur[side] === v) return
    const next: [string, string] = [...cur]
    next[side] = v
    const ranges = [...space.ranges]
    ranges[k] = next
    onLayout({ ...space, ranges })
  }

  function setRole(k: number, role: number) {
    const axes = [...space.axes] as [number, number, number]
    const ranges = [...space.ranges]
    const old = roleOf(k)
    if (role === old) return
    if (role >= 0) {
      const prev = axes[role]
      axes[role] = k
      // k が別の軸にあったなら、入れ替える（そうでなければ prev は固定になる）
      if (old >= 0) axes[old] = prev
      ranges[k] = null
    } else {
      if (old >= 0) {
        // 軸から外す: 代わりに、軸でない座標の中で一番若いものを入れる
        const free = Array.from({ length: space.dims }, (_, j) => j).find((j) => !axes.includes(j))
        if (free === undefined) return // 3次元では外せない
        axes[old] = free
        ranges[free] = null
      }
      ranges[k] = role === RANGE ? ['-1', '1'] : null
    }
    onLayout({ ...space, axes, ranges })
  }
</script>

<div class="space">
  <div class="dims">
    <span class="label">次元</span>
    <button onclick={() => setDims(space.dims - 1)} disabled={space.dims <= 3} aria-label="次元を減らす">−</button>
    <span class="n">{space.dims}</span>
    <button onclick={() => setDims(space.dims + 1)} disabled={space.dims >= 10} aria-label="次元を増やす">＋</button>
  </div>

  {#each Array.from({ length: space.dims }, (_, k) => k) as k (k)}
    {@const role = roleOf(k)}
    <div class="coord">
      <span class="name">{prettyName(coordName(k))}</span>
      <select value={role} onchange={(e) => setRole(k, Number(e.currentTarget.value))} aria-label="{coordName(k)} の表示">
        {#each ROLES as r, j (r)}
          <option value={j}>{r}の軸</option>
        {/each}
        {#if space.dims > 3}
          <option value={FIXED}>固定</option>
          <option value={RANGE}>範囲</option>
        {/if}
      </select>
      {#if role === RANGE && space.ranges[k]}
        {@const rg = space.ranges[k]!}
        <div class="range">
          <MathInput small value={rg[0]} label="{coordName(k)} の最小値" onInput={(v) => setRange(k, 0, v)} />
          <span>〜</span>
          <MathInput small value={rg[1]} label="{coordName(k)} の最大値" onInput={(v) => setRange(k, 1, v)} />
        </div>
        <span class="value">で重ねる</span>
        {#if rangeErrors[k]}
          {@const missing = missingParam(rangeErrors[k])}
          <div class="range-error">
            <span>{prettyName(coordName(k))} の範囲: {rangeErrors[k]}</span>
            {#if missing}
              <button onclick={() => onAddSlider(missing)}>{prettyName(missing)} のスライダーを追加</button>
            {/if}
          </div>
        {/if}
      {:else if role === FIXED}
        <input
          type="range"
          min={space.sliceMin}
          max={space.sliceMax}
          {step}
          value={space.slice[k]}
          aria-label="{coordName(k)} の値"
          oninput={(e) => onSlice(k, e.currentTarget.valueAsNumber)}
        />
        <span class="value">
          <EditableValue
            value={space.slice[k]}
            prefix="= "
            chars={6}
            format={(v) => formatNumber(v, step)}
            label="{coordName(k)} の値"
            onChange={(v) => onSlice(k, v)}
          />
        </span>
      {/if}
    </div>
  {/each}
  {#if space.dims > 3}
    <p class="hint">
      「固定」はスライダーの値での断面、「範囲」はその範囲で動かしたときに図形が通る領域（立体）を描きます。
      範囲には式やパラメータも書けます（例: 0 〜 a。a のスライダーで範囲を動かせる。両端が同じ値ならその値での断面）。
      媒介変数表示は、軸に選んだ3つの成分を描きます。
    </p>
  {/if}
</div>

<style>
  .space {
    display: grid;
    gap: 6px;
  }
  .dims {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .dims button {
    width: 28px;
    height: 28px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    cursor: pointer;
  }
  .dims button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .n {
    min-width: 1.5em;
    text-align: center;
    font: 15px var(--mono);
  }
  .label {
    font-size: 13px;
    color: var(--muted);
    margin-right: 4px;
  }
  .coord {
    display: grid;
    grid-template-columns: 2.2em auto 1fr auto;
    align-items: center;
    gap: 8px;
  }
  .name {
    font: italic 16px 'Times New Roman', serif;
  }
  select {
    padding: 3px 4px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font-size: 13px;
  }
  input[type='range'] {
    width: 100%;
    min-width: 0;
  }
  .range {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 4px;
    font-size: 12px;
    color: var(--muted);
  }
  .value {
    font: 12px var(--mono);
    color: var(--muted);
    min-width: 4em;
  }
  .range-error {
    grid-column: 1 / -1;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--error);
  }
  .range-error button {
    padding: 2px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: var(--bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
  .hint {
    margin: 2px 0 0;
    font-size: 11px;
    color: var(--muted);
  }
</style>

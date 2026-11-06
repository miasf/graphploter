<script lang="ts">
  // 次元の設定: 次元の数、画面の横・奥・縦に表示する座標、表示しない座標の断面の値
  import { coordName, formatNumber, niceStep, prettyName } from './params'
  import type { Space } from './types'

  interface Props {
    space: Space
    /** 次元や、表示する座標の割り当てが変わったとき（すべて計算し直す） */
    onLayout: (space: Space) => void
    /** 表示しない座標 k の断面の値が変わったとき */
    onSlice: (k: number, value: number) => void
  }
  let { space, onLayout, onSlice }: Props = $props()

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
    n = Math.max(3, Math.min(10, n))
    const slice = Array.from({ length: n }, (_, k) => space.slice[k] ?? 0)
    const ranges = Array.from({ length: n }, (_, k) => space.ranges[k] ?? null)
    const center = Array.from({ length: n }, (_, k) => space.center[k] ?? 0)
    const oldAngle = new Map<string, number>()
    pairs(space.dims).forEach(([i, j], k) => oldAngle.set(`${i},${j}`, space.rotation[k] ?? 0))
    const rotation = pairs(n).map(([i, j]) => oldAngle.get(`${i},${j}`) ?? 0)
    // 表示中の座標が無くなる場合は、x, y, z の表示に戻す
    const axes = space.axes.every((a) => a < n) ? space.axes : ([0, 1, 2] as [number, number, number])
    onLayout({ ...space, dims: n, axes: [...axes] as [number, number, number], slice, ranges, center, rotation })
  }

  /** 座標の組 (i, j)（i < j）を、回転角と同じ順に並べたもの */
  function pairs(n: number): [number, number][] {
    const out: [number, number][] = []
    for (let i = 0; i < n; i++) for (let j = i + 1; j < n; j++) out.push([i, j])
    return out
  }

  function setRange(k: number, side: 0 | 1, v: number) {
    const cur = space.ranges[k]
    if (!cur || !Number.isFinite(v)) return
    const next: [number, number] = [...cur]
    next[side] = v
    if (!(next[0] < next[1])) return
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
      ranges[k] = role === RANGE ? [-1, 1] : null
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
          <input type="number" step="any" value={rg[0]} aria-label="{coordName(k)} の最小値"
            onchange={(e) => setRange(k, 0, e.currentTarget.valueAsNumber)} />
          <span>〜</span>
          <input type="number" step="any" value={rg[1]} aria-label="{coordName(k)} の最大値"
            onchange={(e) => setRange(k, 1, e.currentTarget.valueAsNumber)} />
        </div>
        <span class="value">で重ねる</span>
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
        <span class="value">= {formatNumber(space.slice[k], step)}</span>
      {/if}
    </div>
  {/each}
  {#if space.dims > 3}
    <p class="hint">
      「固定」はスライダーの値での断面、「範囲」はその範囲で動かしたときに図形が通る領域（立体）を描きます。
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
  .range input {
    width: 100%;
    min-width: 0;
    padding: 2px 4px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: 12px var(--mono);
  }
  .value {
    font: 12px var(--mono);
    color: var(--muted);
    min-width: 4em;
  }
  .hint {
    margin: 2px 0 0;
    font-size: 11px;
    color: var(--muted);
  }
</style>

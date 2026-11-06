<script lang="ts">
  // 回転の設定: 中心の点と、座標の組 (i, j) ごとの回転角（n 次元なら nC2 個）
  import { coordName, prettyName } from './params'
  import type { Space } from './types'

  interface Props {
    space: Space
    onRotate: (rotation: number[], center: number[]) => void
  }
  let { space, onRotate }: Props = $props()

  /** 最初から見せる回転の数。これより多い分は折りたたむ */
  const SHOW = 6

  /** 回転角と同じ順の座標の組。表示中の軸どうしの組 → 軸と軸でない座標の組 → それ以外 の順に並べ替える */
  const planes = $derived.by(() => {
    const out: { i: number; j: number; k: number; order: number }[] = []
    let k = 0
    for (let i = 0; i < space.dims; i++) {
      for (let j = i + 1; j < space.dims; j++) {
        const shown = Number(space.axes.includes(i)) + Number(space.axes.includes(j))
        out.push({ i, j, k: k++, order: 2 - shown })
      }
    }
    return out.sort((a, b) => a.order - b.order || a.k - b.k)
  })
  const nonzero = $derived(space.rotation.filter((a) => a !== 0).length)

  function setAngle(k: number, deg: number) {
    const rotation = [...space.rotation]
    rotation[k] = deg
    onRotate(rotation, space.center)
  }

  function setCenter(k: number, v: number) {
    if (!Number.isFinite(v)) return
    const center = [...space.center]
    center[k] = v
    onRotate(space.rotation, center)
  }

  function reset() {
    onRotate(
      space.rotation.map(() => 0),
      space.center.map(() => 0),
    )
  }
</script>

{#snippet plane(p: { i: number; j: number; k: number })}
  <span class="name">{prettyName(coordName(p.i))}{prettyName(coordName(p.j))}</span>
  <input
    type="range"
    min="-180"
    max="180"
    step="1"
    value={space.rotation[p.k]}
    aria-label="{coordName(p.i)}{coordName(p.j)} 平面の回転角"
    oninput={(e) => setAngle(p.k, e.currentTarget.valueAsNumber)}
  />
  <span class="value">{space.rotation[p.k]}°</span>
{/snippet}

<details class="rotation">
  <summary>
    回転（{planes.length} 通り{nonzero > 0 ? `、${nonzero} 個を回転中` : ''}）
  </summary>

  <div class="center">
    <span class="label">中心</span>
    {#each space.center as c, k (k)}
      <label>
        <span class="cname">{prettyName(coordName(k))}</span>
        <input type="number" step="any" value={c} onchange={(e) => setCenter(k, e.currentTarget.valueAsNumber)} />
      </label>
    {/each}
  </div>

  <div class="planes">
    {#each planes.slice(0, SHOW) as p (p.k)}
      {@render plane(p)}
    {/each}
  </div>
  {#if planes.length > SHOW}
    <details class="more">
      <summary>ほかの {planes.length - SHOW} 通り</summary>
      <div class="planes">
        {#each planes.slice(SHOW) as p (p.k)}
          {@render plane(p)}
        {/each}
      </div>
    </details>
  {/if}
  <p class="hint">
    「xy」は x と y の平面での回転（x 軸から y 軸の向きが正）。中心の点の周りに、xy, xz, …, yz, … の
    座標の番号順に回します（回転の順番で結果が変わるため）。
  </p>
  <button class="reset" onclick={reset}>回転をリセット</button>
</details>

<style>
  .rotation {
    margin-top: 10px;
  }
  summary {
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    color: var(--muted);
    padding: 4px 0;
  }
  .center {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 6px 0;
  }
  .center label {
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .center input {
    width: 4em;
    padding: 2px 4px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font: 12px var(--mono);
  }
  .label {
    font-size: 12px;
    color: var(--muted);
  }
  .cname,
  .name {
    font: italic 15px 'Times New Roman', serif;
  }
  .planes {
    display: grid;
    grid-template-columns: 2.6em 1fr 3.5em;
    align-items: center;
    gap: 4px 8px;
  }
  .planes input {
    width: 100%;
    min-width: 0;
  }
  .value {
    font: 12px var(--mono);
    color: var(--muted);
    text-align: right;
  }
  .more {
    margin-top: 6px;
  }
  .more summary {
    font-weight: normal;
    font-size: 12px;
  }
  .hint {
    margin: 6px 0;
    font-size: 11px;
    color: var(--muted);
  }
  .reset {
    padding: 4px 10px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font-size: 12px;
    cursor: pointer;
  }
</style>

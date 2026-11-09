<script lang="ts">
  // 回転の設定: 中心の点と、座標の組 (i, j) ごとの回転角（m = max(次元, 5) として mC2 個）
  // 次元が5未満でも、w, w₂ を使った5次元の回転ができる（次元より先の座標は回転のときだけ使う）
  import EditableValue from './EditableValue.svelte'
  import NumberInput from './NumberInput.svelte'
  import { coordName, prettyName, rotDims } from './params'

  interface Props {
    /** 次元 */
    dims: number
    /** 画面の横・奥・縦の軸に表示している座標の番号 */
    axes: number[]
    /** 回転角（度） */
    rotation: number[]
    /** 回転の中心 */
    center: number[]
    onRotate: (rotation: number[], center: number[]) => void
    /** 再生中（角度を自動で動かしている）回転の番号 */
    playing: number[]
    onPlay: (k: number) => void
  }
  let { dims, axes, rotation, center, onRotate, playing, onPlay }: Props = $props()

  /** 最初から見せる回転の数。これより多い分は折りたたむ */
  const SHOW = 6

  /** 回転角と同じ順の座標の組。表示中の軸どうしの組 → 軸と軸でない座標の組 → それ以外 の順に並べ替える */
  const planes = $derived.by(() => {
    const out: { i: number; j: number; k: number; order: number }[] = []
    let k = 0
    const n = rotDims(dims)
    for (let i = 0; i < n; i++) {
      for (let j = i + 1; j < n; j++) {
        const shown = Number(axes.includes(i)) + Number(axes.includes(j))
        out.push({ i, j, k: k++, order: 2 - shown })
      }
    }
    return out.sort((a, b) => a.order - b.order || a.k - b.k)
  })
  const nonzero = $derived(rotation.filter((a) => a !== 0).length)

  function setAngle(k: number, deg: number) {
    const next = [...rotation]
    next[k] = deg
    onRotate(next, center)
  }

  function setCenter(k: number, v: number) {
    if (!Number.isFinite(v)) return
    const next = [...center]
    next[k] = v
    onRotate(rotation, next)
  }

  function reset() {
    onRotate(
      rotation.map(() => 0),
      center.map(() => 0),
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
    value={rotation[p.k]}
    aria-label="{coordName(p.i)}{coordName(p.j)} 平面の回転角"
    oninput={(e) => setAngle(p.k, e.currentTarget.valueAsNumber)}
  />
  <span class="value">
    <EditableValue
      value={rotation[p.k]}
      suffix="°"
      chars={6}
      format={(v) => String(Number(v.toFixed(1)))}
      label="{coordName(p.i)}{coordName(p.j)} 平面の回転角（度）"
      onChange={(v) => setAngle(p.k, v)}
    />
  </span>
  <button
    class="play"
    class:on={playing.includes(p.k)}
    onclick={() => onPlay(p.k)}
    aria-label="{coordName(p.i)}{coordName(p.j)} の回転を{playing.includes(p.k) ? '止める' : '再生する'}"
    title={playing.includes(p.k) ? '止める' : '回し続ける'}>{playing.includes(p.k) ? '❚❚' : '▶'}</button
  >
{/snippet}

<div class="rotation">
  <p class="status">{planes.length} 通りの回転{nonzero > 0 ? `（${nonzero} 個を回転中）` : ''}</p>

  <div class="center">
    <span class="label">中心</span>
    {#each center as c, k (k)}
      <div class="c" class:extra={k >= dims}>
        <span class="cname">{prettyName(coordName(k))}</span>
        <NumberInput value={c} label="回転の中心の {coordName(k)} 座標" onChange={(v) => setCenter(k, v)} />
      </div>
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
  {#if dims < 5}
    <p class="hint">
      {dims === 3 ? 'w と w₂' : 'w₂'} は回転のときだけ使う座標です。
      陰関数・陽関数はその方向にまっすぐ伸びた形、媒介変数表示はその成分が 0 の形として回します。
    </p>
  {/if}
  <p class="hint">
    「xy」は x と y の平面での回転（x 軸から y 軸の向きが正）。中心の点の周りに、xy, xz, …, yz, … の
    座標の番号順に回します（回転の順番で結果が変わるため）。
  </p>
  <button class="reset" onclick={reset}>回転をリセット</button>
</div>

<style>
  .status {
    margin: 0 0 4px;
    font-size: 12px;
    color: var(--muted);
  }
  .more summary {
    cursor: pointer;
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
  .center .c {
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .center .c :global(.num) {
    width: 4.5em;
  }
  .extra {
    opacity: 0.6;
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
    grid-template-columns: 2.6em 1fr auto 1.8em;
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
  .play {
    width: 24px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 5px;
    background: var(--bg);
    color: var(--muted);
    font-size: 10px;
    cursor: pointer;
  }
  .play.on {
    background: #2d70b3;
    border-color: #2d70b3;
    color: #fff;
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

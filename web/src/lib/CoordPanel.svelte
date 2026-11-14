<script lang="ts">
  // 座標系の設定（計画 9 章の S37）: 3 つの式 T、作る範囲、移行のスライダー（直交座標の見え方 ↔ 座標系で移した見え方）
  import EditableValue from './EditableValue.svelte'
  import MathInput from './MathInput.svelte'
  import { COORD_PRESETS } from './coords'
  import type { CoordSystem } from './types'

  interface Props {
    sys: CoordSystem
    /** 式・範囲を変えた（計算し直す） */
    onShape: (map: CoordSystem['map'], ranges: CoordSystem['ranges']) => void
    /** 移行のスライダーを動かした（計算し直さない） */
    onMorph: (s: number) => void
    /** 移行を再生しているか（0 → 1 → 0 と往復する） */
    playing: boolean
    onPlay: () => void
  }
  let { sys, onShape, onMorph, playing, onPlay }: Props = $props()

  const NAMES = ['x', 'y', 'z'] as const
  const COMPONENTS = ['X', 'Y', 'Z'] as const

  function setMap(k: number, latex: string) {
    const map = [...sys.map] as CoordSystem['map']
    map[k] = latex
    onShape(map, sys.ranges)
  }

  function setRange(k: number, side: 0 | 1, latex: string) {
    const ranges = sys.ranges.map((r) => [...r]) as CoordSystem['ranges']
    ranges[k][side] = latex
    onShape(sys.map, ranges)
  }
</script>

<div class="coord">
  <p class="hint">{COORD_PRESETS[sys.preset].note}</p>

  <div class="morph">
    <span class="label">移行</span>
    <input
      type="range"
      min="0"
      max="1"
      step="0.01"
      value={sys.morph}
      aria-label="直交座標から座標系への移行"
      oninput={(e) => onMorph(e.currentTarget.valueAsNumber)}
    />
    <span class="value">
      <EditableValue value={sys.morph} chars={4} min={0} max={1} format={(v) => v.toFixed(2)} label="移行の値（0〜1）" onChange={(v) => onMorph(v)} />
    </span>
    <button
      class="play"
      class:on={playing}
      onclick={onPlay}
      aria-label={playing ? '移行を止める' : '移行を再生する'}
      title={playing ? '止める' : '0 → 1 → 0 と往復する'}>{playing ? '❚❚' : '▶'}</button
    >
  </div>
  <p class="hint">0 で直交座標（座標の値をそのまま x, y, z とみなした絵）、1 で座標系で移した絵です。動かしても計算し直しません。</p>

  <div class="maps">
    {#each COMPONENTS as c, k (c)}
      <span class="name">{c} =</span>
      <MathInput small keyboardToggle={false} value={sys.map[k]} label="座標系の {c} 成分の式" onInput={(v) => setMap(k, v)} />
    {/each}
  </div>

  <p class="label">作る範囲（座標の値の範囲）</p>
  <div class="ranges">
    {#each NAMES as n, k (n)}
      <MathInput small keyboardToggle={false} value={sys.ranges[k][0]} label="座標系の {n} の最小値" onInput={(v) => setRange(k, 0, v)} />
      <span class="name">≤ {n} ≤</span>
      <MathInput small keyboardToggle={false} value={sys.ranges[k][1]} label="座標系の {n} の最大値" onInput={(v) => setRange(k, 1, v)} />
    {/each}
  </div>
  <p class="hint">
    式は今までどおり x, y, z で書き、それを座標の値として読みます。回転は座標の値の空間でかかります（移した後の図形は、視点を回して見てください）。
    切り口のふたは作りません。曲線は (t, t, t) のように書きます（(u, u, u) は曲面になります）。
  </p>
</div>

<style>
  .coord {
    display: grid;
    gap: 6px;
  }
  .label {
    margin: 4px 0 0;
    font-size: 12px;
    color: var(--muted);
  }
  .name {
    font: italic 15px 'Times New Roman', serif;
    white-space: nowrap;
  }
  .morph {
    display: grid;
    grid-template-columns: auto 1fr auto 1.8em;
    align-items: center;
    gap: 8px;
  }
  .morph input {
    width: 100%;
    min-width: 0;
  }
  .value {
    font: 12px var(--mono);
    color: var(--muted);
    text-align: right;
  }
  .maps {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: center;
    gap: 4px 8px;
  }
  .ranges {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: center;
    gap: 4px 8px;
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
  .hint {
    margin: 0;
    font-size: 11px;
    color: var(--muted);
  }
</style>

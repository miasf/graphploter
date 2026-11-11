<script lang="ts">
  // 式の一覧の1行
  import { indexToSpacing, spacingOptions, spacingToIndex } from './gridSpacing'
  import { joinTuple, splitTuple } from './latex'
  import ColorPicker from './ColorPicker.svelte'
  import EditableValue from './EditableValue.svelte'
  import MathInput from './MathInput.svelte'
  import ParamControl from './ParamControl.svelte'
  import RotationPanel from './RotationPanel.svelte'
  import { coordName, missingParam, neededDims, prettyName } from './params'
  import { GLOBAL_GROUP, OWN_GROUP, type ComplexMode, type Domain, type Folder, type GridStyle, type PlotItem, type RotationGroup, type SliderStyle } from './types'

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
    onComplexMode: (mode: ComplexMode) => void
    onWalls: (on: boolean) => void
    /** 媒介変数が4つ以上の図形の、面を描く所の分割数を変えたとき */
    onLayers: (layers: number) => void
    onEdges: (on: boolean) => void
    /** 次元を n にする（「次元を 5 にする」ボタン・成分を足したとき） */
    onSetDims: (n: number) => void
    onLineWidth: (width: number) => void
    onRange: (name: string, side: 0 | 1, value: string) => void
    onToggle: () => void
    onToggleSettings: () => void
    onToggleVertical: () => void
    onRemove: () => void
    /** パラメータの値を変えたとき */
    onParam: (re: number, im: number) => void
    onSlider: (slider: SliderStyle) => void
    /** パラメータを再生中か（値を自動で動かしている） */
    paramPlaying: boolean
    onPlayParam: () => void
    /** 「a のスライダーを追加」を押したとき */
    onAddSlider: (name: string) => void
    /** 式の欄でエンターが押されたとき（この下に新しい式を足す） */
    onEnter: () => void
    /** 並べ替えのつまみをドラッグしたとき */
    onDrag: (phase: 'start' | 'move' | 'end', e: PointerEvent) => void
    /** ドラッグして動かしている最中か */
    dragging: boolean
    /** 回転: 次元と表示中の座標 */
    dims: number
    viewAxes: number[]
    /** 回転のグループの一覧と、この式の回転の番号（負ならこの式だけ、GLOBAL_GROUP なら全体） */
    groups: RotationGroup[]
    groupId: number
    /** グループごとの式の数（選択肢に出す） */
    groupSizes: Record<number, number>
    /** この式を描くときの回転角・中心 */
    rotation: number[]
    center: number[]
    /** この式のグループで再生中の回転 */
    rotPlaying: number[]
    onRotate: (rotation: number[], center: number[]) => void
    onPlayRot: (k: number) => void
    /** 回転の再生を止める（リセットしたとき） */
    onStopRot: (ks: number[]) => void
    /** グループを変える（'new' ならこの式だけの新しいグループ） */
    onGroup: (gid: number | 'new') => void
    /** 設定の「回転」でグループを開く */
    onOpenRotation: () => void
    /** フォルダの一覧と、この式をフォルダに入れる・出す（0 なら外へ、'new' なら新しいフォルダへ） */
    folders: Folder[]
    onFolder: (fid: number | 'new') => void
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
    onComplexMode,
    onWalls,
    onLayers,
    onEdges,
    onSetDims,
    onLineWidth,
    onRange,
    onToggle,
    onToggleSettings,
    onToggleVertical,
    onRemove,
    onParam,
    onSlider,
    paramPlaying,
    onPlayParam,
    onAddSlider,
    onEnter,
    onDrag,
    dragging,
    dims,
    viewAxes,
    groups,
    groupId,
    groupSizes,
    rotation,
    center,
    rotPlaying,
    onRotate,
    onPlayRot,
    onStopRot,
    onGroup,
    onOpenRotation,
    folders,
    onFolder,
  }: Props = $props()

  /** フォルダを選ぶ欄を開いているか */
  let choosingFolder = $state(false)


  /** 未定義の文字を使っていてエラーになっているなら、その文字（スライダーを追加できる） */
  const missing = $derived(missingParam(item.error))
  /** 次元が足りなくてエラーになっているなら、必要な次元（その次元にするボタンを出す） */
  const moreDims = $derived(neededDims(item.error))
  /** 次元の上限（SpacePanel と同じ） */
  const MAX_DIMS = 10

  // 1行の入力欄を編集している間は、縦の表示に切り替えない（入力中に欄が入れ替わらないように）
  let editingLine = $state(false)
  /** 式が組 (a, b, c) なら成分の配列 */
  const parts = $derived(splitTuple(item.latex))
  const showVertical = $derived(item.vertical && parts !== null && !editingLine)

  function setPart(k: number, value: string) {
    if (!parts) return
    const next = [...parts]
    next[k] = value
    onInput(joinTuple(next))
  }

  /** 成分を1つ足す（次の座標の成分。値は 0）。次元が足りなければ増やす */
  function addPart() {
    if (!parts || parts.length >= MAX_DIMS) return
    if (parts.length + 1 > dims) onSetDims(parts.length + 1)
    onInput(joinTuple([...parts, '0']))
  }

  /** 最後の成分を消す（次元はそのまま） */
  function removePart() {
    if (!parts || parts.length <= 2) return
    onInput(joinTuple(parts.slice(0, -1)))
  }

  /** 媒介変数表示の曲面で選べる分割数（0 は線なし） */
  const DIVISIONS = [0, 1, 2, 3, 4, 6, 8, 10, 12, 16, 20, 24, 30, 36, 48, 60, 72, 96, 120]
  const divIndex = (n: number) =>
    DIVISIONS.reduce((best, d, k) => (Math.abs(d - n) < Math.abs(DIVISIONS[best] - n) ? k : best), 0)

  /** 範囲の入力欄を出す媒介変数（媒介変数表示のときだけ） */
  const paramNames = $derived(item.params.filter((n) => n in item.ranges))
  /** 複素平面の点（1 + 2i、複素数のパラメータ c など）。曲線と同じく線の太さ（点の大きさ）を選べる */
  const isComplexPoint = $derived(/^複素平面の点/.test(item.kind ?? ''))
  /** 媒介変数が3つ以上の図形を、辺だけで描いている（線として描く） */
  const isEdges = $derived(item.params.length >= 3 && (item.edges ?? false))
  const isCurve = $derived(item.params.length === 1 || isComplexPoint || isEdges)
  /** 媒介変数表示の曲面・立体（格子線を媒介変数に沿って引く） */
  const isSurface = $derived(item.params.length >= 2 && !isEdges)
  /** 媒介変数が4つ以上の図形（媒介変数を2つずつ動かした面の集まり） */
  const isSheets = $derived(item.params.length >= 4)
  /** 媒介変数ごとの格子線の分割数の設定の名前（立体なら3つ目は w。面の集まりは、どの面も同じ分割にする） */
  const divAxes = $derived(
    isSheets ? (['u'] as const) : item.params.length === 3 ? (['u', 'v', 'w'] as const) : (['u', 'v'] as const),
  )
  /** 面を描く所の分割数の上限（面は kC2 · (分割 + 1)^(k − 2) 枚。多すぎると計算エンジンが減らす） */
  const MAX_LAYERS = 6

  /** 陽関数（z = f(x, y) など）か。複素数の色分けができる */
  // 回して陰関数として計算し直した陽関数（「陰関数（z = f(x, y) を回したもの）」）も含む。
  // 色分けをしている間は、計算できなかったときも選べるようにしておく（元に戻せるように）
  /** 複素平面の図形（e^{it} など。横 Re f・奥 Im f）。縦の軸に描くものを選べる */
  const isComplexPlane = $derived(/^複素平面/.test(item.kind ?? ''))
  const isExplicit = $derived(
    !isComplexPlane && (/ = f\(|の値で塗り分け/.test(item.kind ?? '') || (item.complexMode ?? 'off') !== 'off'),
  )
  /** 段差の壁を描けるか（陽関数をそのまま描くときだけ） */
  const canWall = $derived(isExplicit && /^[^陰]* = f\(/.test(item.kind ?? '') && !/塗り分け/.test(item.kind ?? ''))
  const COMPLEX_MODES: { mode: ComplexMode; label: string }[] = [
    { mode: 'off', label: 'しない' },
    { mode: 'abs', label: '高さ |f|' },
    { mode: 're', label: '高さ Re f' },
    { mode: 'im', label: '高さ Im f' },
    { mode: 'arg', label: '高さ arg f' },
  ]
  /** 複素平面の図形の縦の軸（媒介変数は曲線のときだけ） */
  const LIFT_MODES = $derived<{ mode: ComplexMode; label: string }[]>([
    { mode: 'off', label: '0（平面の上）' },
    ...(isCurve && !isComplexPoint ? [{ mode: 'param' as const, label: `媒介変数 ${prettyName(item.params[0] ?? 't')}` }] : []),
    { mode: 'abs', label: '|f|' },
    { mode: 'arg', label: 'arg f' },
    { mode: 're', label: 'Re f' },
    { mode: 'im', label: 'Im f' },
  ])

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

  /** 設定欄のタブ */
  type Tab = 'look' | 'grid' | 'rotate'
  let tab = $state<Tab>('look')
  const tabs = $derived(
    [
      { id: 'look' as const, label: '見た目' },
      ...(isCurve ? [] : [{ id: 'grid' as const, label: '格子線' }]),
      { id: 'rotate' as const, label: groupId > 0 ? '回転 ●' : '回転' },
    ],
  )
  // 曲線になって格子線のタブが無くなったら、見た目のタブに戻す
  $effect(() => {
    if (!tabs.some((t) => t.id === tab)) tab = 'look'
  })
  const groupName = $derived(groups.find((g) => g.id === groupId)?.name)

  /** 媒介変数に沿った格子線の分割数を変える。面の集まり（媒介変数4つ以上）は、面の2方向を同じ分割にする */
  function setDivision(axis: 'u' | 'v' | 'w', n: number) {
    onGrid(isSheets ? { ...item.grid, u: n, v: n } : { ...item.grid, [axis]: n })
  }
</script>

<li class="item" class:has-error={!!item.error} class:dragging class:in-folder={!!item.folder} data-item-id={item.id}>
  <div class="gutter">
    <!-- 並べ替えのつまみ（上下にドラッグして式の順番を変える） -->
    <span
      class="index"
      role="button"
      tabindex="-1"
      aria-label="式 {index + 1} を動かす（上下にドラッグ）"
      title="上下にドラッグして並べ替え"
      onpointerdown={(e) => {
        e.currentTarget.setPointerCapture(e.pointerId)
        onDrag('start', e)
      }}
      onpointermove={(e) => onDrag('move', e)}
      onpointerup={(e) => onDrag('end', e)}
      onpointercancel={(e) => onDrag('end', e)}
    >
      <span class="grip">⋮⋮</span>
      {index + 1}
    </span>
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
            <span class="name">{prettyName(coordName(k))} =</span>
            <MathInput
              value={part}
              label="{prettyName(coordName(k))} 成分"
              keyboardToggle={k === 0}
              autofocus={item.autofocus && k === 0}
              onInput={(v) => setPart(k, v)}
              {onEnter}
            />
          </div>
        {/each}
      </div>
      <div class="part-actions">
        {#if parts.length < MAX_DIMS}
          <button class="link" onclick={addPart} title="次の座標の成分を足す（次元が足りなければ増やす）"
            >＋ {prettyName(coordName(parts.length))} 成分を追加</button>
        {/if}
        {#if parts.length > 2}
          <button class="link" onclick={removePart}>− {prettyName(coordName(parts.length - 1))} 成分を消す</button>
        {/if}
        <button class="link" onclick={onToggleVertical}>1行で編集</button>
      </div>
    {:else}
      <MathInput
        value={item.latex}
        label="式 {index + 1}"
        placeholder="z=\sin x\cos y"
        autofocus={item.autofocus}
        onInput={(v) => onInput(v)}
        onFocusChange={(f) => (editingLine = f)}
        {onEnter}
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
      <ParamControl
        name={item.param.name}
        value={item.param}
        slider={item.slider}
        error={item.sliderError}
        {onAddSlider}
        onChange={onParam}
        {onSlider}
        playing={paramPlaying}
        onPlay={onPlayParam}
      />
    {/if}
    {#if item.error}
      <p class="error">{item.error}</p>
      {#if missing}
        <!-- 押した瞬間に入力欄からフォーカスが外れると、縦の表示に切り替わってボタンが動き、押せなくなる。
             フォーカスを動かさないようにする -->
        <button class="add-slider" onmousedown={(e) => e.preventDefault()} onclick={() => onAddSlider(missing)}
          >＋ {prettyName(missing)} のスライダーを追加</button>
      {/if}
      {#if moreDims && moreDims <= MAX_DIMS}
        <button class="add-slider" onmousedown={(e) => e.preventDefault()} onclick={() => onSetDims(moreDims)}
          >次元を {moreDims} にする</button>
      {/if}
    {:else if item.info && !item.param}
      <!-- 計算時間（〇〇 ms）は桁数が変わっても幅が変わらないようにする。
           そうしないと、1桁 ⇔ 2桁で改行の位置が変わり、下の行が上下にカクカク動く -->
      {@const m = /^(.*?)(\d+) ms(.*)$/s.exec(item.info)}
      {#if m}
        <p class="info">{m[1]}<span class="ms">{m[2]} ms</span>{m[3]}</p>
      {:else}
        <p class="info">{item.info}</p>
      {/if}
    {/if}

    {#if choosingFolder}
      <div class="folder-row">
        <span class="label">フォルダ</span>
        <select
          value={item.folder ?? 0}
          aria-label="この式を入れるフォルダ"
          onchange={(e) => {
            const v = e.currentTarget.value
            onFolder(v === 'new' ? 'new' : Number(v))
            choosingFolder = false
          }}
        >
          <option value={0}>なし（フォルダの外）</option>
          {#each folders as f (f.id)}
            <option value={f.id}>{f.name}</option>
          {/each}
          <option value="new">＋ 新しいフォルダ</option>
        </select>
      </div>
    {/if}
    {#if item.showSettings && !item.param}
      <div class="settings">
        <div class="tabs" role="tablist" aria-label="設定の項目">
          {#each tabs as t (t.id)}
            <button role="tab" class="tab" class:selected={tab === t.id} aria-selected={tab === t.id}
              onclick={() => (tab = t.id)}>{t.label}</button>
          {/each}
        </div>
        {#if tab === 'look'}
        <div class="row wrap">
          <span class="label">色</span>
          <ColorPicker value={item.color} onChange={onColor} />
        </div>
          {#if isCurve}
          <div class="sliders">
            <span class="label">{isComplexPoint ? "点の大きさ" : "線の太さ"}</span>
            <input
              type="range"
              min="1"
              max="10"
              step="0.5"
              value={item.lineWidth}
              aria-label="線の太さ"
              oninput={(e) => onLineWidth(e.currentTarget.valueAsNumber)}
            />
            <span class="value">
              <EditableValue
                chars={5} value={item.lineWidth} label="線の太さ" suffix=" px" min={0.5} max={20} onChange={onLineWidth} />
            </span>
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
              aria-label="面の濃さ"
              oninput={(e) => onOpacity(e.currentTarget.valueAsNumber)}
            />
            <span class="value">
              <EditableValue
                chars={5}
                value={item.opacity * 100}
                label="面の濃さ（%）"
                suffix="%"
                format={(v) => String(Math.round(v))}
                min={0}
                max={100}
                onChange={(v) => onOpacity(v / 100)}
              />
            </span>
          </div>

          {#if item.params.length >= 3}
            <label class="check">
              <input type="checkbox" checked={item.edges ?? false} onchange={(e) => onEdges(e.currentTarget.checked)} />
              辺だけ描く（媒介変数を1つだけ動かした線。範囲の直方体の辺の像）
            </label>
          {/if}
          {#if isSheets && !isEdges}
            <div class="sliders">
              <span class="label">中の面</span>
              <input
                type="range"
                min="1"
                max={MAX_LAYERS}
                step="1"
                value={item.layers ?? 1}
                aria-label="中の面（固定する媒介変数の分割数）"
                oninput={(e) => onLayers(e.currentTarget.valueAsNumber)}
              />
              <span class="value">
                <EditableValue
                  chars={5} value={item.layers ?? 1} label="固定する媒介変数の分割数" suffix=" 等分" min={1} max={MAX_LAYERS}
                  integer onChange={onLayers} />
              </span>
            </div>
            <p class="hint">
              媒介変数を2つずつ動かした面を描いています（残りは固定）。1 等分なら、固定する値は範囲の両端だけ。
              増やすと、範囲の途中の値に固定した面（中を通る面）も描きます。
            </p>
          {/if}
          {#if canWall}
            <label class="check">
              <input type="checkbox" checked={item.walls ?? false} onchange={(e) => onWalls(e.currentTarget.checked)} />
              値が飛ぶ所（floor の段差など）に壁を描く
            </label>
          {/if}
          {#if isExplicit}
            <div class="complex-mode">
              <span class="label">複素数の色分け</span>
              <select
                value={item.complexMode ?? 'off'}
                onchange={(e) => onComplexMode(e.currentTarget.value as ComplexMode)}
              >
                {#each COMPLEX_MODES as { mode, label } (mode)}
                  <option value={mode}>{label}</option>
                {/each}
              </select>
            </div>
            {#if (item.complexMode ?? 'off') !== 'off'}
              <p class="hint">
                色は偏角 arg f（赤: 正の実数、水色: 負の実数、黄緑: +i の向き、紫: −i の向き）。|f| が 2 倍になるごとに明るさの縞が1本
              </p>
            {/if}
          {/if}

          {/if}
          {#if isComplexPlane}
            <div class="complex-mode">
              <span class="label">縦の軸</span>
              <select
                value={item.complexMode ?? 'off'}
                onchange={(e) => onComplexMode(e.currentTarget.value as ComplexMode)}
              >
                {#each LIFT_MODES as { mode, label } (mode)}
                  <option value={mode}>{label}</option>
                {/each}
              </select>
            </div>
            <p class="hint">
              値 f を複素平面（横 = 実部 Re f、奥 = 虚部 Im f）の点として描いています。縦の軸に媒介変数や |f| を選ぶと、
              平面から持ち上げた形になります（e<sup>it</sup> なら、らせん）
            </p>
          {/if}
        {:else if tab === 'grid'}
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
              <!-- 媒介変数表示の曲面・立体: 媒介変数（u, v など）の範囲を何等分するか -->
              {#each divAxes as axis, n (axis)}
                {@const k = divIndex(item.grid[axis] ?? item.grid.v)}
                <span class="label">{isSheets ? '面の分割' : `${prettyName(item.params[n])} 方向`}</span>
                <input
                  type="range"
                  min="0"
                  max={DIVISIONS.length - 1}
                  step="1"
                  value={k}
                  disabled={!item.grid.enabled}
                  aria-label="{isSheets ? '面' : `${item.params[n]} 方向`}の分割数"
                  oninput={(e) => setDivision(axis, DIVISIONS[e.currentTarget.valueAsNumber])}
                />
                <span class="value">
                  <EditableValue
                chars={5}
                    value={item.grid[axis] ?? item.grid.v}
                    label="{isSheets ? '面' : `${item.params[n]} 方向`}の分割数"
                    suffix=" 等分"
                    min={0}
                    max={200}
                    integer
                    onChange={(v) => setDivision(axis, v)}
                  />
                </span>
              {/each}
            {/if}
            {#each item.params.length >= 3 ? [] : isSurface ? gridAxes.slice(2) : gridAxes as { axis, label } (axis)}
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
                aria-label="{label}の格子線"
                oninput={(e) =>
                  onGrid({ ...item.grid, [axis]: indexToSpacing(e.currentTarget.valueAsNumber, options) })}
              />
              <span class="value">
                間隔
                <EditableValue
                chars={5}
                  value={item.grid[axis]}
                  label="{label}の格子線の間隔"
                  min={0}
                  onChange={(v) =>
                    // 0 以下は線なし。細かすぎると線が多くなりすぎるので、範囲の 1/400 までにする
                    onGrid({ ...item.grid, [axis]: v <= 0 ? 0 : Math.max(v, ranges[axis] / 400) })}
                />
              </span>
            {/each}
            <span class="label">線の太さ</span>
            <input
              type="range"
              min="0.5"
              max="4"
              step="0.25"
              value={item.grid.width}
              disabled={!item.grid.enabled}
              aria-label="格子線の太さ"
              oninput={(e) => onGrid({ ...item.grid, width: e.currentTarget.valueAsNumber })}
            />
            <span class="value">
              <EditableValue
                chars={5} value={item.grid.width} label="格子線の太さ" suffix=" px" min={0.25} max={10}
                onChange={(v) => onGrid({ ...item.grid, width: v })} />
            </span>
          </div>
          <p class="hint">スライダーを右にするほど線が細かくなります。左端（間隔・分割数 0）で線なし。面の濃さ 0% で格子線だけになります。</p>
        {:else if tab === 'rotate'}
          <div class="group-row">
            <span class="label">回転</span>
            <select
              value={groupId < 0 ? OWN_GROUP : groupId}
              aria-label="この式の回転"
              onchange={(e) => {
                const v = e.currentTarget.value
                onGroup(v === 'new' ? 'new' : Number(v))
              }}
            >
              <option value={OWN_GROUP}>この式だけ</option>
              <option value={GLOBAL_GROUP}>全体と同じ</option>
              {#each groups as g (g.id)}
                <option value={g.id}>{g.name}（{groupSizes[g.id] ?? 0} 個の式）</option>
              {/each}
              <option value="new">＋ 新しいグループを作る</option>
            </select>
          </div>
          <p class="hint">
            {groupId < 0
              ? 'この式だけの回転です。ほかの式と一緒に回したいときは「全体と同じ」かグループを選びます。'
              : groupId === GLOBAL_GROUP
                ? '全体の回転で描いています。ここで角度を変えると、全体と同じ式がまとめて回ります。'
                : `「${groupName}」の回転で描いています。同じグループを選んだ式は一緒に回ります（同期）。`}
            <button class="link" onclick={onOpenRotation}>まとめて設定</button>
          </p>
          <RotationPanel {dims} axes={viewAxes} {rotation} {center} {onRotate} playing={rotPlaying} onPlay={onPlayRot} onStop={onStopRot} />
        {/if}
      </div>
    {/if}
  </div>

  <div class="actions">
    {#if !item.param}
      <button
      class="icon"
      class:active={item.showSettings}
      title="式の設定（見た目・格子線・回転）"
      aria-label="式の設定"
      aria-expanded={item.showSettings}
      onclick={onToggleSettings}>⚙</button
    >
    {/if}
    <button
      class="icon folder-icon"
      class:active={choosingFolder}
      title="フォルダに入れる・出す"
      aria-label="フォルダに入れる・出す"
      aria-expanded={choosingFolder}
      onclick={() => (choosingFolder = !choosingFolder)}>▤</button
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
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 2px 6px;
    font-size: 11px;
    line-height: 1.1;
    color: var(--muted);
    cursor: grab;
    touch-action: none; /* ドラッグ中に画面がスクロールしないように */
    user-select: none;
    border-radius: 4px;
  }
  .index:hover {
    background: var(--hover);
  }
  .grip {
    font-size: 9px;
    letter-spacing: -2px;
    opacity: 0.6;
  }
  .item.dragging {
    position: relative;
    z-index: 2;
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.2);
    opacity: 0.95;
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
    font-variant-numeric: tabular-nums; /* 数字の幅をそろえる（三角形の数が変わっても幅が揺れない） */
  }
  .ms {
    display: inline-block;
    min-width: 7ch; /* 「9999 ms」まで同じ幅 */
    white-space: nowrap;
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
  .tabs {
    display: flex;
    gap: 4px;
    border-bottom: 1px solid var(--border);
    margin: -2px 0 2px;
  }
  .tab {
    padding: 4px 10px 6px;
    border: none;
    border-bottom: 2px solid transparent;
    background: none;
    color: var(--muted);
    font-size: 13px;
    cursor: pointer;
  }
  .tab.selected {
    color: var(--text);
    border-bottom-color: #2d70b3;
    font-weight: 600;
  }
  .group-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .group-row select {
    flex: 1;
    min-width: 0;
    padding: 3px 4px;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font-size: 13px;
  }
  .link {
    padding: 0;
    border: none;
    background: none;
    color: #2d70b3;
    font-size: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .row.wrap {
    flex-wrap: wrap;
  }
  .label {
    color: var(--muted);
  }
  .complex-mode {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .complex-mode select {
    font: inherit;
    padding: 2px 4px;
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text);
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
  /* フォルダの中の式は、左に帯を付けて少し下げる */
  .item.in-folder {
    margin-left: 10px;
    border-left: 1px solid var(--border);
  }
  .folder-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 6px;
    font-size: 13px;
    color: var(--muted);
  }
  .folder-row select {
    flex: 1;
    min-width: 0;
  }
  .folder-icon {
    font-size: 13px;
  }
  .part-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 14px;
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

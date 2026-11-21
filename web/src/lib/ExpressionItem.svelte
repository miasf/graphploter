<script lang="ts">
  // 式の一覧の1行
  import { indexToSpacing, spacingOptions, spacingToIndex } from './gridSpacing'
  import { RIEMANN_METHODS, hasIntegral, inlineRegions, integralKinds, joinTuple, regionLabel, regionsUsed, splitTuple, type RiemannMethod } from './latex'
  import { matrixDefinition } from './matrixEdit'
  import ColorPicker from './ColorPicker.svelte'
  import EditableValue from './EditableValue.svelte'
  import MathInput from './MathInput.svelte'
  import LimitLesson from './LimitLesson.svelte'
  import ParamControl from './ParamControl.svelte'
  import RotationPanel from './RotationPanel.svelte'
  import SequenceTable from './SequenceTable.svelte'
  import { calledAsFunction, coordName, missingParam, neededDims, prettyName } from './params'
  import { prettyCoordName } from './coords'
  import { DEFAULT_HIDDEN, GLOBAL_GROUP, OWN_GROUP, type ComplexMode, type CoordSystem, type Domain, type HiddenStyle, type Folder, type GridStyle, type IntegralViewInfo, type LimitBehavior, type PlotItem, type RenderMode, type RotationGroup, type SliderStyle } from './types'
  import { gpuByDefault } from './renderMode'

  interface Props {
    item: PlotItem
    onToggleSequenceTable: () => void
    index: number
    /** 表示範囲（格子線の間隔の選択肢を決めるのに使う） */
    domain: Domain
    /** 画面の横・奥・縦の軸に表示している座標の名前（ふつうは x, y, z） */
    axisNames: string[]
    /** 式（LaTeX）が書き換えられたとき */
    onInput: (latex: string) => void
    /** 名前で書いた積分の範囲（∬_D の D → 範囲の LaTeX）。全部の式で共通 */
    regions: Record<string, string>
    /** 範囲の欄が書き換えられたとき */
    onRegion: (name: string, latex: string) => void
    /** グラフに描いている範囲 → 色と、描けないときの説明 */
    regionViews: Record<string, { color: string; error?: string }>
    /** 範囲の欄の「表示」ボタン（描く・消すを切り替える） */
    onRegionShow: (name: string) => void
    /** この式の積分を描いていれば、その色・説明・誤り（描いていなければ undefined） */
    integralView?: IntegralViewInfo
    /** 「積分を描く」ボタン（描く・消すを切り替える） */
    onIntegralShow: () => void
    /** 積分の図の進み具合（0〜1）を変えた */
    onIntegralProgress: (v: number) => void
    /** 進み具合の再生・停止 */
    onIntegralPlay: () => void
    /** 定積分の近似（リーマン和など）の方法と分割数を変えた（方法が null なら近似を消す） */
    onRiemann?: (method: RiemannMethod | null, n: number) => void
    /** パラメータの行: 条件の行があるとき、スライダーの範囲の点ごとに、その値にできるか */
    paramAllowed?: boolean[]
    onColor: (color: string) => void
    onGrid: (grid: GridStyle) => void
    onOpacity: (opacity: number) => void
    onComplexMode: (mode: ComplexMode) => void
    /** 表示していない座標の見せ方を変えたとき（変えた所だけ） */
    onHidden: (patch: Partial<HiddenStyle>) => void
    /** 座標を複素数にする・やめる */
    onComplexCoords: (on: boolean) => void
    /** 次元の設定の断面の値（座標の番号順）と、スライダーの範囲 */
    globalSlice: number[]
    sliceMin: number
    sliceMax: number
    /** この式だけの断面の値を決める（null なら全体の値に戻す） */
    onSlice: (name: string, value: number | null) => void
    onWalls: (on: boolean) => void
    /** ベクトル場の矢印の数・長さをそろえるか */
    onField?: (patch: { count?: number; unit?: boolean; stream?: number }) => void
    /** 常微分方程式の初期値の欄を変えた（成分の番号と LaTeX） */
    onOdeInit?: (k: number, latex: string) => void
    /** 2D 表示か（矢印の数の既定が変わる） */
    flatView?: boolean
    /** 媒介変数が4つ以上の図形の、面を描く所の分割数を変えたとき */
    onLayers: (layers: number) => void
    onEdges: (on: boolean) => void
    /** 描画の方法を変えたとき（計画 12 章の G7） */
    onRender: (mode: RenderMode) => void
    /** 次元を n にする（「次元を 5 にする」ボタン・成分を足したとき） */
    onSetDims: (n: number) => void
    onLineWidth: (width: number) => void
    onRange: (name: string, side: 0 | 1, value: string) => void
    /** θ・φ を使うので自動で使っている座標系の名前（極座標など。使っていなければ undefined） */
    autoCoordName?: string
    /** 自動の座標系の角度と、その範囲（LaTeX） */
    autoAngles?: { name: string; range: [string, string] }[]
    onAngleRange?: (name: string, side: 0 | 1, value: string) => void
    onToggle: () => void
    onToggleSettings: () => void
    onToggleVertical: () => void
    onRemove: () => void
    /** 式を複製して、すぐ下に置く */
    onDuplicate: () => void
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
    trailEnabled: boolean
    onTrail: (enabled: boolean) => void
    onClearTrail: () => void
    /** 回転の再生を止める（リセットしたとき） */
    onStopRot: (ks: number[]) => void
    /** グループを変える（'new' ならこの式だけの新しいグループ） */
    onGroup: (gid: number | 'new') => void
    /** 座標系の一覧（計画 9 章） */
    coordSystems: CoordSystem[]
    /** この式の座標系を選ぶ（undefined なら直交座標、'new' なら球座標を作って選ぶ） */
    onCoord: (cid: number | 'new' | undefined) => void
    /** 座標系の設定を開く */
    onOpenCoord: () => void
    /** 設定の「回転」でグループを開く */
    onOpenRotation: () => void
    /** この行（行列の定義）を、座標系 T(q) = A q として使う */
    onMatrixCoord?: () => void
    /** フォルダの一覧と、この式をフォルダに入れる・出す（0 なら外へ、'new' なら新しいフォルダへ） */
    folders: Folder[]
    onFolder: (fid: number | 'new') => void
  }
  let {
    item,
    onToggleSequenceTable,
    index,
    domain,
    axisNames,
    onInput,
    regions,
    onRegion,
    regionViews,
    onRegionShow,
    integralView,
    onIntegralShow,
    onIntegralProgress,
    onIntegralPlay,
    onRiemann,
    paramAllowed,
    onColor,
    onGrid,
    onOpacity,
    onComplexMode,
    onHidden,
    onComplexCoords,
    globalSlice,
    sliceMin,
    sliceMax,
    onSlice,
    onWalls,
    onField,
    onOdeInit,
    flatView = false,
    onLayers,
    onEdges,
    onRender,
    onSetDims,
    onLineWidth,
    onRange,
    autoCoordName,
    autoAngles = [],
    onAngleRange,
    onToggle,
    onToggleSettings,
    onToggleVertical,
    onRemove,
    onDuplicate,
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
    trailEnabled,
    onTrail,
    onClearTrail,
    onStopRot,
    onGroup,
    coordSystems,
    onCoord,
    onOpenCoord,
    onOpenRotation,
    onMatrixCoord,
    folders,
    onFolder,
  }: Props = $props()

  /** フォルダを選ぶ欄を開いているか */
  let choosingFolder = $state(false)


  /** 未定義の文字を使っていてエラーになっているなら、その文字（スライダーを追加できる） */
  const missing = $derived(missingParam(item.error))
  const hasSequenceTable = $derived(item.visible && !item.error && !item.param && !!item.def && /（数列の(?:点|軌道)/.test(item.kind ?? ''))
  /**
   * 積分の範囲の欄・積分の図にだけ出ている、未定義の文字（式そのものは、積分の図を出すと非表示になって計算し直さないことがある）。
   * 式のエラーで出しているときは出さない
   */
  const auxMissing = $derived.by(() => {
    if (missing) return null
    const errors = [integralView?.error, integralView?.valueError, ...[...regionNames, ...inlineNames].map((n) => regionViews[n]?.error)]
    for (const e of errors) {
      const m = missingParam(e)
      if (m) return m
    }
    return null
  })
  /** 未定義の文字が、関数の呼び出しの形 f(…) で使われているか（そうなら、スライダーの代わりに関数の定義を足す） */
  const missingCall = $derived(missing !== null && calledAsFunction(item.latex, missing))
  /** 次元が足りなくてエラーになっているなら、必要な次元（その次元にするボタンを出す） */
  const moreDims = $derived(neededDims(item.error))
  /** 次元の上限（SpacePanel と同じ） */
  const MAX_DIMS = 10

  // 1行の入力欄を編集している間は、縦の表示に切り替えない（入力中に欄が入れ替わらないように）
  let editingLine = $state(false)
  /** 式が組 (a, b, c) なら成分の配列 */
  const parts = $derived(splitTuple(item.latex))
  /** この式で使っている、名前で書いた積分の範囲（∬_D の D） */
  const regionNames = $derived(regionsUsed(item.latex).map((r) => r.name))
  /** 積分記号の下に直接書いた範囲を描くための名前（App の inlineKey と同じ形） */
  const inlineNames = $derived(inlineRegions(item.latex).map((_, k) => `#${item.id}:${k}`))
  /** 式の中の積分の分類（何の積分か。範囲の書き方・次元と被積分関数の形から決める。範囲の欄が変われば変わる） */
  const kinds = $derived(hasIntegral(item.latex) ? integralKinds(item.latex, regions) : [])
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
  /** 初期値の欄の見出し: y' → y′(0) =（初期時刻は、範囲に 0 があれば 0、無ければ範囲の始まり） */
  function prettyOde(st: string): string {
    const name = st.replace(/'+$/, '')
    return `${prettyName(name)}${'′'.repeat(st.length - name.length)}(0) =`
  }

  const paramNames = $derived(item.params.filter((n) => n in item.ranges))
  /** 複素平面の点（1 + 2i、複素数のパラメータ c など）。曲線と同じく線の太さ（点の大きさ）を選べる */
  const isComplexPoint = $derived(/^複素平面の点/.test(item.kind ?? ''))
  /** 描画の方法の説明: 選んだ方法で描けない理由・自動で選んだもの（計画 12 章の G7） */
  const renderNote = $derived.by(() => {
    const mode = item.render ?? 'auto'
    if (mode === 'mesh') return ''
    if (!item.gpuAvailable) return `画素ごとの描き方は使えないので、メッシュで描いています（${item.gpuReason || 'この式は対象外です'}）`
    if (mode === 'auto' && !gpuByDefault()) return 'この端末では、自動のときはメッシュで描きます（画素ごと（GPU）を選ぶと使えます）'
    return ''
  })
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
  // 回した陽関数（傾いた座標系の陽関数「z = f(x, y)（回したもの）」と、陰関数として計算し直した
  // 「陰関数（z = f(x, y) を回したもの）」）も含む。
  // 色分けをしている間は、計算できなかったときも選べるようにしておく（元に戻せるように）
  /** 複素平面の図形（e^{it} など。横 Re f・奥 Im f）。縦の軸に描くものを選べる */
  const isComplexPlane = $derived(/^複素平面/.test(item.kind ?? ''))
  /** 複素数の陰関数（値が複素数の F = 0）。曲線か、色分けした曲面かを選べる */
  const isComplexImplicit = $derived(/^複素数の陰関数/.test(item.kind ?? ''))
  /** 複素数の陰関数の描き方（色分けの選択肢を、陰関数の意味で読み替える） */
  const IMPLICIT_COMPLEX_MODES: { mode: ComplexMode; label: string }[] = [
    { mode: 'off', label: 'しない（Re F = Im F = 0 の曲線）' },
    { mode: 'abs', label: '|F| = 1 の曲面（色は arg F）' },
    { mode: 're', label: 'Re F = 0 の曲面（色は F）' },
    { mode: 'im', label: 'Im F = 0 の曲面（色は F）' },
  ]
  /** 式が使っている、表示していない座標の名前（w, w2, …）。媒介変数表示には使わない（成分が座標そのもの） */
  const hiddenCoords = $derived(
    item.params.length > 0 || item.param
      ? []
      : Array.from({ length: dims }, (_, k) => k)
          .filter((k) => !viewAxes.includes(k))
          .map(coordName)
          .filter((n) => item.usedParams.includes(n)),
  )
  const hidden = $derived(item.hidden ?? DEFAULT_HIDDEN)
  const HIDDEN_MODES: { mode: HiddenStyle['mode']; label: string }[] = [
    { mode: 'slice', label: '断面（スライダーの値 1 つ）' },
    { mode: 'layers', label: '等値面を何枚も' },
    { mode: 'planes', label: '断面の塗り分け' },
    { mode: 'volume', label: '伝達関数（色と透明度）' },
    { mode: 'stack', label: '範囲で重ねる（通る領域）' },
  ]
  const COLORMAPS: { map: HiddenStyle['colormap']; label: string }[] = [
    { map: 'viridis', label: 'viridis（紫 → 緑 → 黄）' },
    { map: 'turbo', label: 'turbo（青 → 赤）' },
    { map: 'shade', label: '式の色の明暗' },
  ]
  const OPACITY_SHAPES: { shape: HiddenStyle['opacity']; label: string }[] = [
    { shape: 'flat', label: '一様' },
    { shape: 'high', label: '値が大きいほど濃い' },
    { shape: 'low', label: '値が小さいほど濃い' },
    { shape: 'bands', label: '何枚かの値の所だけ濃い（等値面風）' },
    { shape: 'zero', label: '0 の近くだけ濃い（ぼやけた面）' },
    { shape: 'abs', label: '|値| が大きいほど濃い（正も負も）' },
  ]
  const BLENDS: { blend: NonNullable<HiddenStyle['blend']>; label: string }[] = [
    { blend: 'over', label: '手前から重ね塗り' },
    { blend: 'max', label: '視線上の最大値' },
    { blend: 'min', label: '視線上の最小値' },
    { blend: 'mean', label: '視線上の平均値' },
  ]
  /** 陰関数 F(x, y, z) = 0（値が複素数のものも含む。不等式で切り取ったものは除く）。F の値を伝達関数で描ける */
  const canField = $derived(/^(複素数の)?陰関数(?!（不等式)/.test(item.kind ?? ''))
  /** 伝達関数で描いている値が複素数（色は偏角。色の対応表は使わない） */
  const complexVolume = $derived(/の伝達関数（.*色は偏角/.test(item.kind ?? ''))
  /** 3D の不等式の領域（標準は境目の面とふた。⚙ で霧に切り替えられる） */
  const isRegion = $derived(/^不等式の領域(?!（2D）)/.test(item.kind ?? '') && !/の投影/.test(item.kind ?? ''))
  /** 不等式の領域を霧で描いている */
  const regionFog = $derived(isRegion && /霧/.test(item.kind ?? ''))
  /** 選べる見せ方。表示していない座標を使わない陰関数は、曲面か F の値の伝達関数か。不等式の領域は、霧か境目の面か */
  const hiddenModes = $derived(
    isRegion || hidden.mode === 'fog'
      ? hiddenCoords.length > 0
        ? [
            { mode: 'slice' as const, label: '断面（境目の面とふた）' },
            { mode: 'fog' as const, label: '断面（霧。重い）' },
            ...HIDDEN_MODES.filter((m) => m.mode !== 'slice'),
          ]
        : [
            { mode: 'slice' as const, label: '境目の面とふた' },
            { mode: 'fog' as const, label: '霧（中身を薄く塗る。回すと重い）' },
          ]
      : hiddenCoords.length > 0
        ? [...HIDDEN_MODES, ...(canField || hidden.mode === 'field' ? [{ mode: 'field' as const, label: '陰関数の F の値を伝達関数で' }] : [])]
        : [
            { mode: 'slice' as const, label: '曲面（F = 0）' },
            { mode: 'field' as const, label: 'F の値を伝達関数で（色と透明度）' },
          ],
  )
  /** 伝達関数で描く見せ方か */
  const isVolume = $derived(hidden.mode === 'volume' || hidden.mode === 'field')
  const isExplicit = $derived(
    !isComplexPlane && (/ = f\(|の値で塗り分け/.test(item.kind ?? '') || (item.complexMode ?? 'off') !== 'off'),
  )
  /** 段差の壁を描けるか（陽関数をそのまま描くときだけ） */
  /** ベクトル場の行か（計画 M8） */
  const isField = $derived(/ベクトル場/.test(item.kind ?? ''))
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

  /** この式の座標系（選んでいない・消えていれば undefined） */
  /** 行列の定義の行（2×2・3×3）なら、その名前と大きさ */
  const matrixDef = $derived(matrixDefinition(item.latex))
  const coordSys = $derived(item.coordSys === undefined ? undefined : coordSystems.find((c) => c.id === item.coordSys))
  /** 設定欄のタブ */
  type Tab = 'look' | 'grid' | 'rotate' | 'coord'
  let tab = $state<Tab>('look')
  const tabs = $derived(
    [
      { id: 'look' as const, label: '見た目' },
      ...(isCurve ? [] : [{ id: 'grid' as const, label: '格子線' }]),
      { id: 'rotate' as const, label: groupId > 0 ? '回転 ●' : '回転' },
      ...(item.param ? [] : [{ id: 'coord' as const, label: coordSys ? '座標系 ●' : '座標系' }]),
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

  /** 進み具合で動かす変数の名前（\\theta などの LaTeX の名前も、θ のように出す） */
  const sweepName = (n: string) => prettyName(n.replace(/^\\(var)?/, ''))

  /** 積分の値を短く出す（有効 6 桁） */
  const formatValue = (v: number) => (Math.abs(v) < 1e-12 ? '0' : Number(v.toPrecision(6)).toString().replace('-', '−'))
  const limitNumber = (v: number) => Number.isFinite(v)
    ? Number(v.toPrecision(10)).toString().replaceAll('-', '−')
    : v < 0 ? '−∞' : '+∞'
  /** 誤差（小さい数は指数表記） */
  const formatError = (v: number) => {
    const a = Math.abs(v)
    if (a === 0) return '0'
    if (a >= 1e-3 && a < 1e5) return formatValue(v)
    const [m, e] = v.toExponential(2).split('e')
    return `${m.replace('-', '−')} × 10^${Number(e)}`.replace(/\^(-?\d+)/, (_, x) => [...String(x)].map((c) => '⁻⁰¹²³⁴⁵⁶⁷⁸⁹'['-0123456789'.indexOf(c)]).join(''))
  }

  // ---- 定積分の近似: n を 1 から増やしていく再生 ----
  let riemannPlaying = $state(false)
  let riemannTimer: ReturnType<typeof setInterval> | undefined
  function toggleRiemannPlay() {
    const r = integralView?.riemann
    if (!r?.method) return
    if (riemannPlaying) {
      riemannPlaying = false
      clearInterval(riemannTimer)
      return
    }
    riemannPlaying = true
    let n = r.n >= 64 ? 1 : r.n
    onRiemann?.(r.method, n)
    riemannTimer = setInterval(() => {
      const m = integralView?.riemann?.method
      // 増やし方は、小さい n はゆっくり、大きい n は速く（1, 2, 3, …, 10, 12, 14, …）
      n = n < 10 ? n + 1 : n < 32 ? n + 2 : n + 4
      if (!m || n > 64) {
        riemannPlaying = false
        clearInterval(riemannTimer)
        return
      }
      onRiemann?.(m, n)
    }, 450)
  }
  $effect(() => () => clearInterval(riemannTimer))
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
    {#if !item.error && item.limit}
      {@const result = item.limit}
      {@const variable = prettyName(result.variable)}
      {@const target = limitNumber(result.target)}
      {@const finite = Number.isFinite(result.target)}
      {#snippet limitSide(behavior: LimitBehavior, side: number)}
        <dt>
          {finite ? (side > 0 ? '右極限' : '左極限') : result.sequence ? '数列の極限' : '極限'}
          <span class="limit-formula">lim<sub>{variable} → {target}{finite ? (side > 0 ? '⁺' : '⁻') : ''}</sub>
            {#if result.sequence}a<sub>{variable}</sub>{:else}f({variable}){/if}</span>
        </dt>
        <dd>
          {#if behavior.kind === 'finite'}
            <span>≈ {limitNumber(behavior.value)}</span>
            <span class="limit-state">収束</span>
          {:else if behavior.kind === 'infinite'}
            {limitNumber(behavior.value)} に発散
          {:else if behavior.kind === 'oscillating'}
            振動
          {:else}
            判定できません
          {/if}
        </dd>
      {/snippet}
      <div class="limit-result" aria-label="極限の数値判定" aria-live="polite">
        <dl>
          {#if finite ? result.side >= 0 : result.target > 0}{@render limitSide(result.right, 1)}{/if}
          {#if finite ? result.side <= 0 : result.target < 0}{@render limitSide(result.left, -1)}{/if}
          {#if finite}
            <dt>f({target})</dt>
            <dd>{result.defined === undefined ? '未定義' : limitNumber(result.defined)}</dd>
          {/if}
        </dl>
        {#if finite && result.side === 0 && result.value === undefined}
          {#if 'value' in result.left && 'value' in result.right}
            <p>極限なし（左右で値が異なります）</p>
          {:else if result.left.kind === 'oscillating' || result.right.kind === 'oscillating'}
            <p>極限なし（振動）</p>
          {/if}
        {/if}
        <p class="hint">数値判定</p>
      </div>
      {#if result.samples.length > 0}
        <LimitLesson {result} {variable} />
      {/if}
    {/if}
    <!-- 積分の範囲（∬_D の D）。式には名前だけを書き、範囲はここに書く。同じ名前は、どの式でも同じ範囲 -->
    {#each regionNames as name (name)}
      <div class="region">
        <span class="name">{regionLabel(name)} =</span>
        <MathInput
          small
          keyboardToggle={false}
          value={regions[name] ?? ''}
          label="積分の範囲 {regionLabel(name)}"
          placeholder={'\\left\\{\\left(x,y\\right)\\mid x^2+y^2<1\\right\\}'}
          onInput={(v) => onRegion(name, v)}
        />
        <button
          class="region-show"
          class:on={name in regionViews}
          style:--region-color={regionViews[name]?.color}
          aria-pressed={name in regionViews}
          title={name in regionViews ? 'この範囲をグラフから消す' : 'この範囲をグラフに描く'}
          onclick={() => onRegionShow(name)}>{name in regionViews ? '表示中' : '表示'}</button>
      </div>
      {#if regionViews[name]?.error}
        <div class="region-error">{regionLabel(name)} を描けません: {regionViews[name].error}</div>
      {/if}
    {/each}
    <!-- 何の積分か（書いた範囲と被積分関数から判定した分類と、媒介変数で書いたときの式） -->
    {#if kinds.length > 0}
      <div class="integral-kinds" aria-label="積分の分類">
        {#each kinds as k, j (j)}
          <div class="integral-kind">
            <span class="kind-name">{k.kind}</span>
            <span class="kind-region">範囲: {k.region}</span>
            <span class="kind-formula">{k.formula}</span>
          </div>
        {/each}
      </div>
    {/if}
    <!-- 積分を描く: 被積分関数を、積分する範囲の上に描く（面積・体積が積分の値になる図形） -->
    {#if hasIntegral(item.latex) || integralView}
      <div class="integral">
        <button
          class="region-show"
          class:on={!!integralView}
          style:--region-color={integralView?.color}
          aria-pressed={!!integralView}
          title={integralView ? '積分の図をグラフから消す' : '被積分関数を、積分する範囲の上に描く（面積・体積が積分の値になる図形）'}
          onclick={onIntegralShow}>{integralView ? '∫ 表示中' : '∫ 積分を描く'}</button>
        <!-- 積分記号の下に直接書いた範囲も、名前で書いた範囲と同じように描ける -->
        {#each inlineNames as name, k (name)}
          <button
            class="region-show"
            class:on={name in regionViews}
            style:--region-color={regionViews[name]?.color}
            aria-pressed={name in regionViews}
            title={name in regionViews ? '積分の範囲をグラフから消す' : '積分記号の下に書いた範囲をグラフに描く'}
            onclick={() => onRegionShow(name)}
            >{name in regionViews ? '範囲 表示中' : '範囲を表示'}{inlineNames.length > 1 ? ` ${k + 1}` : ''}</button
          >
        {/each}
        {#each inlineNames as name (name)}
          {#if regionViews[name]?.error}
            <span class="region-error">範囲を描けません: {regionViews[name].error}</span>
          {/if}
        {/each}
        {#if integralView?.error}
          <span class="region-error">描けません: {integralView.error}</span>
        {:else if integralView?.note}
          <span class="integral-note">{integralView.note}</span>
        {/if}
      </div>
      {#if integralView && !integralView.error && integralView.sweep}
        <!-- 進み具合: 積分の範囲を、下の端から途中までにする（学習用。値は途中までの積分） -->
        <div class="integral-sweep">
          <span class="sweep-name" title="積分の範囲の、この変数を下の端から動かす">{sweepName(integralView.sweep)}</span>
          <input
            type="range"
            min="0"
            max="1"
            step="0.005"
            value={integralView.progress}
            aria-label="積分の図の進み具合（{integralView.sweep} の範囲の何割まで）"
            oninput={(e) => onIntegralProgress(e.currentTarget.valueAsNumber)}
          />
          <span class="sweep-pct">{Math.round(integralView.progress * 100)}%</span>
          <button
            class="sweep-play"
            class:on={integralView.playing}
            onclick={onIntegralPlay}
            aria-label={integralView.playing ? '止める' : '0 から再生する'}
            title={integralView.playing ? '止める' : '0 から 1 まで動かす'}>{integralView.playing ? '❚❚' : '▶'}</button
          >
        </div>
        <div class="integral-value">
          {#if integralView.progress >= 1}
            {#if integralView.valueError}
              積分の値: 計算できません（{integralView.valueError}）
            {:else if integralView.value !== undefined}
              範囲全体の積分 = <b>{formatValue(integralView.value)}</b>
            {:else}
              積分の値を計算中…
            {/if}
            <span class="sweep-hint">（{sweepName(integralView.sweep)} を動かすと、途中までの積分の値と図が出ます）</span>
          {:else if integralView.valueError}
            途中までの値: 計算できません（{integralView.valueError}）
          {:else if integralView.value !== undefined}
            途中までの積分 = <b>{formatValue(integralView.value)}</b>
          {:else}
            途中までの積分を計算中…
          {/if}
        </div>
      {/if}
      {#if integralView && !integralView.error && integralView.riemann && onRiemann}
        {@const r = integralView.riemann}
        <!-- 定積分の近似（リーマン和・台形則・シンプソン則）: 図と、近似の値・正確な値・誤差 -->
        <div class="riemann">
          <div class="riemann-methods" role="group" aria-label="定積分の近似の方法">
            <span class="riemann-title">近似</span>
            <button class="riemann-chip" class:on={r.method === null} aria-pressed={r.method === null} onclick={() => onRiemann(null, r.n)}>なし</button>
            {#each RIEMANN_METHODS as m (m.id)}
              <button
                class="riemann-chip"
                class:on={r.method === m.id}
                aria-pressed={r.method === m.id}
                title="{m.label}{m.id === 'trap' || m.id === 'simpson' ? '' : 'のリーマン和'}（誤差は {m.order}）"
                onclick={() => onRiemann(m.id, r.n)}>{m.label}</button
              >
            {/each}
          </div>
          {#if r.method}
            <div class="integral-sweep">
              <span class="sweep-name" title="区間を n 等分する">n</span>
              <input
                type="range"
                min="1"
                max="64"
                step="1"
                value={Math.min(64, r.n)}
                aria-label="分割数 n"
                oninput={(e) => onRiemann(r.method, e.currentTarget.valueAsNumber)}
              />
              <EditableValue chars={4} value={r.n} label="分割数 n" onChange={(v) => onRiemann(r.method, v)} />
              <button
                class="sweep-play"
                class:on={riemannPlaying}
                onclick={toggleRiemannPlay}
                aria-label={riemannPlaying ? '止める' : 'n を増やしていく'}
                title={riemannPlaying ? '止める' : 'n を 1 から 64 まで増やしていく'}>{riemannPlaying ? '❚❚' : '▶'}</button
              >
            </div>
            <div class="integral-value riemann-values">
              {#if r.error}
                近似の値: 計算できません（{r.error}）
              {:else if r.approx !== undefined && r.exact !== undefined}
                <span>近似 = <b>{formatValue(r.approx)}</b></span>
                <span>正確な値 = <b>{formatValue(r.exact)}</b></span>
                <span>誤差 = <b>{Math.abs(r.approx - r.exact) <= 1e-12 * Math.max(1, Math.abs(r.exact)) ? '0（丸め誤差の範囲）' : formatError(r.approx - r.exact)}</b></span>
                <span class="sweep-hint">
                  {RIEMANN_METHODS.find((m) => m.id === r.method)?.label}{r.method === 'trap' || r.method === 'simpson' ? '則' : 'のリーマン和'}の誤差は
                  {RIEMANN_METHODS.find((m) => m.id === r.method)?.order}（n を 2 倍にすると、{r.method === 'simpson' ? 'およそ 1/16' : r.method === 'mid' || r.method === 'trap' ? 'およそ 1/4' : 'およそ 1/2'} になる）。{r.method === 'simpson' ? 'n は偶数にそろえる。' : ''}
                </span>
              {:else}
                近似の値を計算中…
              {/if}
            </div>
          {/if}
        </div>
      {/if}
      {#if integralView && integralView.legend.length > 0}
        <div class="integral-legend">
          {#each integralView.legend as l (l.label)}
            <span class="legend-item" title={l.error ?? l.label}><i style:background={l.color}></i>{l.label}{l.error ? '（描けません）' : ''}</span>
          {/each}
        </div>
      {/if}
    {/if}
    {#if hidden.mode === 'slice' || hidden.mode === 'field' || hidden.mode === 'fog'}
      <!-- この式だけの断面の値（決めた座標だけ。× で次元の設定の値に戻す） -->
      {#each hiddenCoords.filter((n) => item.slice && n in item.slice) as name (name)}
        <div class="own-slice">
          <span>{prettyName(name)} =</span>
          <input
            type="range"
            min={Math.min(sliceMin, item.slice![name])}
            max={Math.max(sliceMax, item.slice![name])}
            step={(sliceMax - sliceMin) / 200}
            value={item.slice![name]}
            aria-label="この式の {name} の値"
            oninput={(e) => onSlice(name, e.currentTarget.valueAsNumber)}
          />
          <EditableValue chars={6} value={item.slice![name]} label="この式の {name} の値" onChange={(v) => onSlice(name, v)} />
          <button class="link" title="次元の設定の値に戻す" aria-label="{name} を全体の値に戻す" onclick={() => onSlice(name, null)}>×</button>
        </div>
      {/each}
    {:else}
      {#each hiddenCoords.filter((n) => n in item.ranges) as name (name)}
        <div class="param-range">
          <MathInput small value={item.ranges[name][0]} label="{name} の最小値" onInput={(v) => onRange(name, 0, v)} />
          <span>≤ {prettyName(name)} ≤</span>
          <MathInput small value={item.ranges[name][1]} label="{name} の最大値" onInput={(v) => onRange(name, 1, v)} />
        </div>
      {/each}
    {/if}
    {#if autoCoordName}
      <div class="auto-coord">{autoCoordName}で描いています（r は表示範囲に合わせます）</div>
    {/if}
    {#each autoAngles as a (a.name)}
      <!-- θ・φ を使う式は、自動で極座標・円柱座標・球座標で描く。角度の範囲を変えられる（r は表示範囲に合わせる） -->
      <div class="param-range" title="{autoCoordName}で描いています（r は表示範囲に合わせます）">
        <MathInput small value={a.range[0]} label="{prettyCoordName(a.name)} の最小値" onInput={(v) => onAngleRange?.(a.name, 0, v)} />
        <span>≤ {prettyCoordName(a.name)} ≤</span>
        <MathInput small value={a.range[1]} label="{prettyCoordName(a.name)} の最大値" onInput={(v) => onAngleRange?.(a.name, 1, v)} />
      </div>
    {/each}
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
    {#if item.odeStates && onOdeInit}
      <!-- 常微分方程式の初期値（計画 M9）。空欄は既定の値（状態は 1、導関数は 0）。リスト [0, 1, 2] なら何本も描く -->
      {#each item.odeStates as st, k (st)}
        <div class="param-range">
          <span>{prettyOde(st)}</span>
          <MathInput
            small
            value={item.ode?.init[k] ?? ''}
            placeholder={st.endsWith("'") ? '0' : '1'}
            label="{st} の初期値"
            onInput={(v) => onOdeInit(k, v)}
          />
        </div>
      {/each}
    {/if}
    {#if auxMissing}
      <button class="add-slider" onmousedown={(e) => e.preventDefault()} onclick={() => onAddSlider(auxMissing)}
        >＋ {prettyName(auxMissing)} のスライダーを追加</button
      >
    {/if}
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
        allowed={paramAllowed}
      />
    {/if}
    {#if item.error}
      <p class="error">{item.error}</p>
      {#if missing}
        <!-- 押した瞬間に入力欄からフォーカスが外れると、縦の表示に切り替わってボタンが動き、押せなくなる。
             フォーカスを動かさないようにする -->
        <button class="add-slider" onmousedown={(e) => e.preventDefault()} onclick={() => onAddSlider(missing)}
          >＋ {missingCall ? `${prettyName(missing)}(x) の定義を追加` : `${prettyName(missing)} のスライダーを追加`}</button>
      {/if}
      {#if moreDims && moreDims <= MAX_DIMS}
        <button class="add-slider" onmousedown={(e) => e.preventDefault()} onclick={() => onSetDims(moreDims)}
          >次元を {moreDims} にする</button>
      {/if}
    {:else if item.info && !item.param}
      <!-- 計算時間（〇〇 ms）は桁数が変わっても幅が変わらないようにする。
           そうしないと、1桁 ⇔ 2桁で改行の位置が変わり、下の行が上下にカクカク動く -->
      {@const m = /^(.*?)(\d+) ms(.*)$/s.exec(item.info)}
      <p class="info">
        {#if m}{m[1]}<span class="ms">{m[2]} ms</span>{m[3]}{:else}{item.info}{/if}
        {#if hasSequenceTable}
          <button type="button" class="sequence-table-toggle" aria-expanded={!!item.showSequenceTable}
            aria-label={item.showSequenceTable ? '項の表を閉じる' : '項の表を開く'} onclick={onToggleSequenceTable}>項の表</button>
        {/if}
      </p>
    {/if}

    {#if hasSequenceTable && item.showSequenceTable}
      <SequenceTable values={item.sequenceTable} />
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
          {#if isField && onField}
            <!-- ベクトル場の矢印（計画 M8）。色は大きさ（色の対応表は「見せ方」と同じ） -->
            <div class="sliders">
              <span class="label">矢印の数</span>
              <input
                type="range"
                min="4"
                max="24"
                step="1"
                value={item.field?.count ?? (flatView ? 18 : 9)}
                aria-label="各軸の矢印の数"
                onchange={(e) => onField({ count: e.currentTarget.valueAsNumber })}
              />
              <span class="value">{item.field?.count ?? (flatView ? 18 : 9)}</span>
            </div>
            <label class="check">
              <input type="checkbox" checked={item.field?.unit ?? false} onchange={(e) => onField({ unit: e.currentTarget.checked })} />
              矢印の長さをそろえる（向きだけを見る。色は大きさ）
            </label>
            <label class="check">
              <input
                type="checkbox"
                checked={(item.field?.stream ?? 0) > 0}
                onchange={(e) => onField({ stream: e.currentTarget.checked ? 4 : 0 })}
              />
              流線を描く（格子の点から、場に沿って進む曲線）
            </label>
            {#if (item.field?.stream ?? 0) > 0}
              <div class="sliders">
                <span class="label">流線の始点</span>
                <input
                  type="range"
                  min="1"
                  max="8"
                  step="1"
                  value={item.field?.stream ?? 4}
                  aria-label="流線の始点の各軸の数"
                  onchange={(e) => onField({ stream: e.currentTarget.valueAsNumber })}
                />
                <span class="value">{item.field?.stream}</span>
              </div>
            {/if}
          {/if}
          {#if canWall}
            <label class="check">
              <input type="checkbox" checked={item.walls ?? false} onchange={(e) => onWalls(e.currentTarget.checked)} />
              値が飛ぶ所（floor の段差など）に壁を描く
            </label>
          {/if}
          {#if item.gpuAvailable || item.gpuReason}
            <div class="complex-mode">
              <span class="label">描画の方法</span>
              <select value={item.render ?? 'auto'} aria-label="描画の方法" onchange={(e) => onRender(e.currentTarget.value as RenderMode)}>
                <option value="auto">自動</option>
                <option value="mesh">メッシュ（三角形）</option>
                <option value="gpu">画素ごと（GPU）</option>
              </select>
            </div>
            {#if renderNote}
              <p class="hint">{renderNote}</p>
            {/if}
          {/if}
          {#if (item.params.length === 0 && !item.param) || item.complexCoords}
            <label class="check">
              <input type="checkbox" checked={item.complexCoords ?? false} onchange={(e) => onComplexCoords(e.currentTarget.checked)} />
              座標を複素数にする（x → x + i w、y → y + i w₂、z → z + i w₃。6 次元で見る）
            </label>
            {#if item.complexCoords}
              <p class="hint">
                x, y, z の虚部を w, w₂, w₃ に割り当てています。表示する座標を (x, w, y) のように実部と虚部を混ぜて選ぶと、
                実数でない所の形が見えます（x, y, z のままなら実数の部分）。式が複素数になる断面は、Re = Im = 0 の線で描きます
              </p>
            {/if}
          {/if}
          {#if hiddenCoords.length > 0 || canField || hidden.mode === 'field' || isRegion || hidden.mode === 'fog'}
            <div class="complex-mode">
              <span class="label">{hiddenCoords.length > 0 ? `${hiddenCoords.map(prettyName).join('・')} の見せ方` : '描き方'}</span>
              <select value={hidden.mode} onchange={(e) => onHidden({ mode: e.currentTarget.value as HiddenStyle['mode'] })}>
                {#each hiddenModes as { mode, label } (mode)}
                  <option value={mode}>{label}</option>
                {/each}
              </select>
            </div>
            {#if hidden.mode === 'slice' || hidden.mode === 'field' || hidden.mode === 'fog'}
              {#each hiddenCoords.filter((n) => !(item.slice && n in item.slice)) as name (name)}
                <button
                  class="link"
                  onclick={() => onSlice(name, globalSlice[Array.from({ length: dims }, (_, k) => k).find((k) => coordName(k) === name) ?? 0] ?? 0)}
                  >{prettyName(name)} の値をこの式だけで決める</button
                >
              {/each}
            {/if}
            {#if regionFog}
              <label class="check">
                <input type="checkbox" checked={hidden.lit ?? true} onchange={(e) => onHidden({ lit: e.currentTarget.checked })} />
                境目に陰影を付ける
              </label>
              <div class="sliders">
                <span class="label">濃さ</span>
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={hidden.density}
                  aria-label="霧の濃さ"
                  oninput={(e) => onHidden({ density: e.currentTarget.valueAsNumber })}
                />
                <span class="value">{Math.round(hidden.density * 100)}%</span>
              </div>
              <p class="hint">
                領域の中身を式の色の霧で薄く塗り、境目を濃くしています（表示範囲の箱の中を光線で重ね塗り）。
                光線を表示範囲の箱の中で何百歩も進めるので、回している間は境目の面よりかなり重くなります。
              </p>
            {/if}
            {#if hidden.mode !== 'slice' && hidden.mode !== 'fog'}
              {#if hidden.mode === 'layers' || hidden.mode === 'planes' || (isVolume && ((hidden.opacity === 'bands' && !complexVolume) || hidden.opacity === 'zero'))}
                <div class="sliders">
                  <span class="label">{hidden.mode === 'layers' ? '枚数' : hidden.mode === 'planes' ? '各軸の枚数' : hidden.opacity === 'zero' ? '面の幅' : '濃い所の数'}</span>
                  <input
                    type="range"
                    min={hidden.mode === 'layers' ? 2 : 1}
                    max="16"
                    step="1"
                    value={hidden.count}
                    aria-label="枚数"
                    oninput={(e) => onHidden({ count: e.currentTarget.valueAsNumber })}
                  />
                  <span class="value">{hidden.count}</span>
                </div>
              {/if}
              {#if hidden.mode !== 'stack' && !(isVolume && complexVolume)}
              <div class="complex-mode">
                <span class="label">色</span>
                <select value={hidden.colormap} onchange={(e) => onHidden({ colormap: e.currentTarget.value as HiddenStyle['colormap'] })}>
                  {#each COLORMAPS as { map, label } (map)}
                    <option value={map}>{label}</option>
                  {/each}
                </select>
              </div>
              {/if}
              {#if hidden.mode === 'planes' || hidden.mode === 'layers'}
                <label class="check">
                  <input type="checkbox" checked={hidden.log ?? false} onchange={(e) => onHidden({ log: e.currentTarget.checked })} />
                  対数の目盛り（値の差が何桁もあるとき、小さな値の違いも色で見分けられる）
                </label>
              {/if}
              {#if isVolume}
                <div class="complex-mode">
                  <span class="label">重ね方</span>
                  <select value={hidden.blend ?? 'over'} onchange={(e) => onHidden({ blend: e.currentTarget.value as HiddenStyle['blend'] })}>
                    {#each BLENDS as { blend, label } (blend)}
                      <option value={blend}>{label}</option>
                    {/each}
                  </select>
                </div>
                {#if (hidden.blend ?? 'over') === 'over'}
                  <label class="check">
                    <input type="checkbox" checked={hidden.lit ?? true} onchange={(e) => onHidden({ lit: e.currentTarget.checked })} />
                    勾配で陰影を付ける（値の変わる向きを面の向きとみなして光を当てる）
                  </label>
                {/if}
                <div class="complex-mode">
                  <span class="label">不透明度</span>
                  <select value={hidden.opacity} onchange={(e) => onHidden({ opacity: e.currentTarget.value as HiddenStyle['opacity'] })}>
                    {#each OPACITY_SHAPES as { shape, label } (shape)}
                      <option value={shape}>{label}</option>
                    {/each}
                  </select>
                </div>
                <div class="sliders">
                  <span class="label">濃さ</span>
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.01"
                    value={hidden.density}
                    aria-label="伝達関数の濃さ"
                    oninput={(e) => onHidden({ density: e.currentTarget.valueAsNumber })}
                  />
                  <span class="value">{Math.round(hidden.density * 100)}%</span>
                </div>
              {/if}
              <p class="hint">
                {#if hidden.mode === 'layers'}
                  範囲を等分した値ごとの図形を重ね、値で色を付けています（{hiddenCoords.length >= 2 ? '色は 1 つ目の座標の値。' : ''}範囲は式の下の欄）。
                {:else if hidden.mode === 'stack'}
                  範囲で動かしたときに図形が通る領域を、立体で描いています（範囲は式の下の欄。何等分して調べるかは右上の ⚙ の「範囲で重ねる枚数」）。
                {:else if hidden.mode === 'planes'}
                  表示範囲の中の平面（各軸に等間隔）を、{prettyName(hiddenCoords[0])} = f の値で塗っています（{prettyName(hiddenCoords[0])} = f(x, y, z) の形の式だけ）。
                {:else if isVolume && complexVolume}
                  値が複素数なので、色は偏角（赤: 正の実数、水色: 負の実数、黄緑: +i の向き、紫: −i の向き。|f| が 2 倍になるごとに明るさの縞が 1 本）、
                  不透明度の形は |f| で決めます（|f| の範囲は 0 〜 表示範囲の中の |f| の 9 割が入る値。「何枚かの値の所だけ濃い」は |f| = 2ᵏ の所、
                  「0 の近くだけ濃い」は f = 0 の所。3 次元では f = 0 はふつう線なので、線のまわりの管になります）。
                {:else if hidden.mode === 'field'}
                  陰関数 F(x, y, z) = 0 の F（左辺 − 右辺）の値に色と不透明度を割り当て、表示範囲の箱の中を光線で重ね塗りしています。
                  色の範囲は、表示範囲の中の |F| の 9 割が入る値 q での −q 〜 q（外の値は端の色）。「0 の近くだけ濃い」は、F = 0 の面からの距離
                  |F| / |∇F| でぼかすので、式の大きさによらず同じ厚みになります。{hiddenCoords.length > 0 ? '表示していない座標は断面の値です。' : ''}
                {:else}
                  {prettyName(hiddenCoords[0])} の値に色と不透明度を割り当て、表示範囲の箱の中を光線で重ね塗りしています（{prettyName(hiddenCoords[0])} = f(x, y, z) の形の式だけ。範囲の外の値は描きません）。
                {/if}
              </p>
            {/if}
          {/if}
          {#if isComplexImplicit}
            <div class="complex-mode">
              <span class="label">複素数の色分け</span>
              <select value={item.complexMode ?? 'off'} onchange={(e) => onComplexMode(e.currentTarget.value as ComplexMode)}>
                {#each IMPLICIT_COMPLEX_MODES as { mode, label } (mode)}
                  <option value={mode}>{label}</option>
                {/each}
              </select>
            </div>
            <p class="hint">
              {#if (item.complexMode ?? 'off') === 'off'}
                値が複素数の F = 0 は、Re F = 0 と Im F = 0 の交わる線になります（複素数の等式 1 本 = 実数の等式 2 本）。
              {:else}
                色は偏角 arg F（赤: 正の実数、水色: 負の実数、黄緑: +i の向き、紫: −i の向き）。|F| が 2 倍になるごとに明るさの縞が1本。
                Re F = Im F = 0 の線は、縞が集まる所に見えます
              {/if}
            </p>
          {:else if isExplicit}
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
          <RotationPanel {dims} axes={viewAxes} {rotation} {center} {onRotate} playing={rotPlaying} onPlay={onPlayRot} onStop={onStopRot} {trailEnabled} {onTrail} {onClearTrail} />
        {:else if tab === 'coord'}
          <div class="group-row">
            <span class="label">座標系</span>
            <select
              value={coordSys ? String(coordSys.id) : ''}
              aria-label="この式の座標系"
              onchange={(e) => {
                const v = e.currentTarget.value
                onCoord(v === '' ? undefined : v === 'new' ? 'new' : Number(v))
              }}
            >
              <option value="">直交座標</option>
              {#each coordSystems as c (c.id)}
                <option value={String(c.id)}>{c.name}</option>
              {/each}
              <option value="new">＋ 球座標を作る</option>
            </select>
          </div>
          <p class="hint">
            {coordSys
              ? `式の x, y, z を「${coordSys.name}」の座標の値として読み、移した形を描きます（移行 ${coordSys.morph.toFixed(2)}）。`
              : '式の x, y, z を、そのまま直交座標として描いています。'}
            <button class="link" onclick={onOpenCoord}>座標系の設定・移行のスライダー</button>
          </p>
          {#if matrixDef && onMatrixCoord}
            <!-- 行列の定義の行: 線形変換 T(q) = A q の座標系を作る（計画 M7） -->
            <p class="hint">
              <button class="link" onclick={onMatrixCoord}>この行列を座標系として使う</button>
              （{matrixDef.size}×{matrixDef.size} の行列の線形変換 q ↦ A q。ほかの式の ⚙「座標系」で選ぶと、その式が変換されます。
              移行のスライダー 0 で元の形）
            </p>
          {/if}
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
    <button class="icon" title="複製（すぐ下に同じ式を足す）" aria-label="複製" onclick={onDuplicate}>⧉</button>
    <button class="icon" title="削除" aria-label="削除" onclick={onRemove}>×</button>
  </div>
</li>

<style>
  .sequence-table-toggle {
    min-height: 28px;
    padding: 2px 6px;
    border: 0;
    border-radius: 6px;
    color: var(--text);
    background: transparent;
    font-size: 12px;
    text-decoration: underline;
    cursor: pointer;
  }
  .sequence-table-toggle:hover { background: var(--hover); }

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
    /* 中のスライダーの並べ方を、この枠の幅で決める（下の @container） */
    container-type: inline-size;
    margin-top: 10px;
    padding: 10px;
    border-radius: 8px;
    background: var(--gutter);
    font-size: 13px;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 8px;
  }
  .tabs {
    display: flex;
    flex-wrap: wrap;
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
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
  }
  /* 見出しは折り返さない（選択欄の文字が長いと、見出しが 1 文字ずつ縦に並んでいた）。入りきらなければ選択欄が次の行へ */
  .complex-mode .label {
    white-space: nowrap;
  }
  .complex-mode select {
    min-width: 0;
    max-width: 100%;
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
  .region-show {
    padding: 3px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: none;
    color: var(--muted);
    font-size: 12px;
    white-space: nowrap;
    cursor: pointer;
  }
  .region-show.on {
    border-color: var(--region-color);
    background: color-mix(in srgb, var(--region-color) 18%, transparent);
    color: var(--text);
  }
  .region-error {
    margin-top: 2px;
    color: var(--error, #c0392b);
    font-size: 12px;
  }
  .integral {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 8px;
    margin-top: 4px;
  }
  .integral .region-error {
    margin-top: 0;
  }
  .integral-sweep {
    display: grid;
    grid-template-columns: auto 1fr 3em auto;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
  }
  .integral-sweep input {
    width: 100%;
    min-width: 0;
  }
  .sweep-name {
    font: italic 15px 'Times New Roman', serif;
  }
  .sweep-pct {
    font: 12px var(--mono);
    color: var(--muted);
    text-align: right;
  }
  .sweep-play {
    width: 26px;
    height: 22px;
    padding: 0;
    border: 1px solid var(--border-strong);
    border-radius: 5px;
    background: var(--bg);
    color: var(--muted);
    font-size: 10px;
    cursor: pointer;
  }
  .sweep-play.on {
    background: #2d70b3;
    border-color: #2d70b3;
    color: #fff;
  }
  .integral-value {
    font-size: 12px;
    color: var(--muted);
  }
  .riemann {
    display: grid;
    gap: 4px;
    margin-top: 4px;
  }
  .riemann-methods {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px;
  }
  .riemann-title {
    font-size: 12px;
    color: var(--muted);
    margin-right: 2px;
  }
  .riemann-chip {
    font: inherit;
    font-size: 12px;
    padding: 1px 8px;
    border: 1px solid var(--border-strong);
    border-radius: 999px;
    background: var(--bg);
    color: var(--text);
    cursor: pointer;
  }
  .riemann-chip.on {
    background: #2d70b3;
    border-color: #2d70b3;
    color: #fff;
  }
  .riemann-values {
    display: flex;
    flex-wrap: wrap;
    column-gap: 12px;
  }
  .riemann-values .sweep-hint {
    flex-basis: 100%;
  }
  .integral-value .sweep-hint {
    display: block;
    font-size: 11px;
  }
  .integral-value b {
    color: var(--text);
    font: 13px var(--mono);
  }
  .integral-legend {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 10px;
    font-size: 11px;
    color: var(--muted);
  }
  .legend-item i {
    display: inline-block;
    width: 10px;
    height: 3px;
    margin-right: 4px;
    vertical-align: middle;
    border-radius: 2px;
  }
  .integral-note {
    color: var(--muted);
    font-size: 12px;
  }
  .integral-kinds {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: 4px;
  }
  .integral-kind {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 8px;
    font-size: 12px;
    line-height: 1.5;
  }
  .kind-name {
    font-weight: 600;
  }
  .kind-region {
    color: var(--muted);
  }
  .kind-formula {
    flex-basis: 100%;
    color: var(--muted);
    overflow-wrap: anywhere;
  }
  .region {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 6px;
    margin-top: 4px;
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
  .own-slice span {
    white-space: nowrap;
  }
  .own-slice {
    display: grid;
    grid-template-columns: auto 1fr auto auto;
    align-items: center;
    gap: 6px;
    margin-top: 6px;
    font: 13px var(--mono);
    color: var(--muted);
  }
  .auto-coord {
    margin-top: 6px;
    font-size: 12px;
    color: var(--muted);
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
  /* 式の欄が狭いとき（スマートフォン・左の欄の幅 380px）: ラベル・スライダー・値を 1 行に並べると、
     ラベル（「z（等高線）」）と値（「間隔 1」）に幅を取られて、スライダーが 30px ほどに潰れていた。
     ラベルを上の行に出し、下の行にスライダーと値を並べる */
  @container (max-width: 300px) {
    .sliders {
      grid-template-columns: minmax(0, 1fr) auto;
      row-gap: 2px;
    }
    .sliders > .label {
      grid-column: 1 / -1;
      margin-top: 4px;
    }
    .sliders > input[type='range'] {
      width: 100%;
      min-width: 0;
    }
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
  .limit-result {
    margin-top: 4px;
    font-size: 12px;
  }
  .limit-result dl {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 3px 8px;
    margin: 0;
  }
  .limit-result dd,
  .limit-result p {
    margin: 0;
  }
  .limit-result dd {
    display: flex;
    flex-wrap: wrap;
    column-gap: 6px;
  }
  .limit-state {
    color: var(--muted);
    white-space: nowrap;
  }
  .limit-formula {
    font-family: var(--mono);
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
  /* カーソルを乗せたときの薄い色は、マウスのある端末だけ（スマホでは押した後も残り、開いているように見えるため） */
  @media (hover: hover) {
    .icon:hover {
      background: var(--hover);
      color: var(--text);
    }
  }
  /* 開いているとき: 塗りつぶして、閉じているとき・乗せたときとはっきり見分けられるようにする */
  .icon.active,
  .icon.active:hover {
    background: var(--accent);
    color: var(--on-accent);
    border-radius: 6px;
  }
</style>

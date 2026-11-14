// アプリ全体で使うデータの形

import type { Cost } from './budget'

/** 表示範囲（直方体） */
export interface Domain {
  xmin: number
  xmax: number
  ymin: number
  ymax: number
  zmin: number
  zmax: number
}

/** 曲面に描く格子線の設定 */
export interface GridStyle {
  enabled: boolean
  /** 各方向の線の間隔（0 なら描かない）。z 方向の線は z = f(x, y) では等高線になる */
  x: number
  y: number
  z: number
  /** 媒介変数表示の曲面で、u, v の範囲をそれぞれ何等分する線を引くか（0 なら描かない） */
  u: number
  v: number
  /** 媒介変数表示の立体で、3つ目の媒介変数の範囲を何等分するか（古い保存データには無い） */
  w?: number
  /** 線の太さ（画素） */
  width: number
}

/**
 * 複素数の値をとる陽関数の表し方（色は偏角 arg f）。
 * off: 色分けしない（実数にならない点は描かない）/ abs: 高さ |f| / re: 高さ Re f / im: 高さ Im f / arg: 高さ arg f。
 * 複素平面の図形（e^{it} など。横 Re f・奥 Im f）では、縦の軸に描くもの（off: 0 / param: 媒介変数 / abs: |f| / arg: arg f …）
 */
/**
 * 表示していない座標（w 以降）を使う式の見せ方。slice: 断面（スライダーの値 1 つ）、layers: 等値面を何枚も（範囲を等分した値ごとの図形を重ね、値で色を付ける）、
 * planes: 断面の塗り分け（表示範囲の中の平面を w = f の値で塗る）、volume: 伝達関数（値に色と不透明度を割り当てて重ね塗りする）、
 * stack: 範囲で重ねる（範囲で動かしたとき通る領域を立体で描く。次元の設定の「範囲で重ねる」を、この式だけの範囲で使う）、
 * field: 陰関数の F の値の伝達関数（F(x, y, z) = 0 の F の値を、volume と同じ描き方で塗る。表示していない座標は断面の値）、
 * fog: 霧（不等式の領域の中身を、境目の面とふたではなく霧で塗る。表示していない座標は断面の値）。
 * slice 以外では、次元の設定の「範囲で重ねる」より、式の設定を優先する
 */
export type HiddenMode = 'slice' | 'layers' | 'planes' | 'volume' | 'stack' | 'field' | 'fog'
/** 計算エンジンに渡す HiddenMode の番号の順 */
export const HIDDEN_MODE_ORDER: HiddenMode[] = ['slice', 'layers', 'planes', 'volume', 'stack', 'field', 'fog']
/** 値の色の対応表。shade は式の色の明暗 */
export type ColorMap = 'viridis' | 'turbo' | 'shade'
/**
 * 伝達関数の不透明度の形。flat: 一様、high: 値が大きいほど濃い、low: 小さいほど濃い、bands: 何枚かの値の所だけ濃い（等値面風）、
 * zero: 値 0 の近くだけ濃い（ぼやけた面。幅は 0 の面からの距離 |v| / |∇v| で決めるので、式の大きさによらず一定の厚み）
 */
export type OpacityShape = 'flat' | 'high' | 'low' | 'bands' | 'zero'
/**
 * 伝達関数の重ね方。over: 手前から重ね塗り、max: 視線上の最大値（最大値投影）、min: 最小値、mean: 平均値。
 * max・min・mean では、視線ごとに 1 つの値の色を出し、不透明度はその値で決める
 */
export type VolumeBlend = 'over' | 'max' | 'min' | 'mean'
export interface HiddenStyle {
  mode: HiddenMode
  /** 等値面の枚数（layers）・各軸の平面の枚数（planes）・濃い所の数（volume の bands）・ぼやけた面の幅（zero。格子の 0.5 マス単位） */
  count: number
  colormap: ColorMap
  opacity: OpacityShape
  /** 伝達関数の濃さ（0〜1） */
  density: number
  /** 伝達関数の重ね方（古い保存データには無い。無ければ over） */
  blend?: VolumeBlend
  /** 伝達関数で、値の勾配を面の法線とみなして光を当てるか（重ね塗りのとき。無ければ当てる） */
  lit?: boolean
}
export const DEFAULT_HIDDEN: HiddenStyle = { mode: 'slice', count: 5, colormap: 'viridis', opacity: 'flat', density: 0.5 }

export type ComplexMode = 'off' | 'abs' | 're' | 'im' | 'arg' | 'param'
/** 計算エンジンに渡す ComplexMode の番号の順 */
export const COMPLEX_MODE_ORDER: ComplexMode[] = ['off', 'abs', 're', 'im', 'arg', 'param']

/** 曲面の見た目 */
export interface PlotStyle {
  color: string
  visible: boolean
  /** 面の不透明度（0〜1）。0 にすると格子線だけの曲面になる */
  opacity: number
  grid: GridStyle
  /** 曲線の太さ（画素） */
  lineWidth: number
  /** 表示していない座標の見せ方（値の色の対応表・伝達関数の不透明度は、計算し直さずに変える） */
  hidden?: HiddenStyle
}

/** 媒介変数の範囲。媒介変数の名前（t, u, v, t1, …）→ [最小, 最大]（入力欄の LaTeX のまま。2π などの式も書ける） */
export type ParamRanges = Record<string, [string, string]>

/** 媒介変数の範囲の長さ。u, v は1つ目・2つ目、w は3つ目（立体のときだけ）。
 *  媒介変数が4つ以上の図形では、面ごとに 0〜1 にそろえてあるので u = v = 1、w = 0（3つ目の線は引かない） */
export interface ParamSpan {
  u: number
  v: number
  w?: number
}

/** 座標空間の設定（4次元以上のとき、どの座標を表示し、残りをどの値で固定するか） */
export interface Space {
  /** 次元（3 以上） */
  dims: number
  /** 画面の横・奥・縦の軸に表示する座標の番号（0: x, 1: y, 2: z, 3: w, 4: w₂, …） */
  axes: [number, number, number]
  /** 各座標の断面の値（表示していない座標だけが使われる） */
  slice: number[]
  /** 断面のスライダーの範囲 */
  sliceMin: number
  sliceMax: number
  /** 各座標を「範囲で重ねる」なら [最小, 最大]（LaTeX の式。パラメータも使える）、断面なら null
   *  （表示していない座標だけが使われる） */
  ranges: ([string, string] | null)[]
  /** 範囲を何等分して重ねるか */
  samples: number
  /** 回転角（度）。座標の組 (0,1), (0,2), …, (0,n−1), (1,2), … の順に nC2 個 */
  rotation: number[]
  /** 回転の中心 */
  center: number[]
  /** 4次元の遠近法の強さ（0〜100。0 なら平行な射影）。射影で描く図形（媒介変数表示・回した陽関数）だけに効く */
  perspective: number
}

/**
 * 回転のグループ。同じグループの式は、同じ回転（角度・中心）で描く（同期する）。
 * グループに入っていない式は、全体の回転（Space の rotation, center）を使う
 */
export interface RotationGroup {
  id: number
  name: string
  /** 回転角（度）。Space の rotation と同じ並び */
  rotation: number[]
  /** 回転の中心 */
  center: number[]
}

/**
 * 座標系（計画 9 章）。座標の値 q（式の x, y, z）を、空間の点 T(q) へ移す。式ごとに選ぶ（選ばなければ直交座標）。
 * 移行のスライダー morph = s で、(1 − s) q + s T(q) を描く（s = 0 は今の絵、s = 1 は座標系で移した絵）
 */
export interface CoordSystem {
  id: number
  name: string
  /** 用意した座標系か、自作か（式・範囲の初期値を決めるだけ。描くときは map と ranges を使う） */
  preset: 'cylinder' | 'sphere' | 'custom'
  /** T の 3 つの成分（LaTeX。変数は x, y, z。パラメータも使える） */
  map: [string, string, string]
  /**
   * 座標の名前（計画 S38。計算エンジンの名前: r, theta, phi など。無い古い保存データは x, y, z）。
   * 座標系を選んだ式と、この座標系の式・範囲では、この名前で 3 つの座標を書ける
   */
  names?: [string, string, string]
  /** 作る範囲（座標の値の範囲。LaTeX。2\pi なども書ける）。x, y, z の順 */
  ranges: [[string, string], [string, string], [string, string]]
  /** 移行のスライダー（0〜1） */
  morph: number
  /**
   * 座標系の格子（座標曲線。計画 S39）。on なら、この座標系を使う式があるときに描く（無い古い保存データは on）。
   * faces は線を引く面（ビット a: 座標 a が範囲の最小の面。既定は 4 = 3 つ目の座標の面）
   */
  grid?: { on: boolean; faces: number }
}

/** フォルダ: 式をまとめて、一覧で折りたためるようにする（Desmos のフォルダと同じ） */
export interface Folder {
  id: number
  name: string
  /** 折りたたんでいるか（中の式は一覧に出さないが、グラフには描く） */
  collapsed: boolean
}

/** 全体の回転を表すグループの番号 */
export const GLOBAL_GROUP = 0

/** 「この式だけの回転」を選ぶときの番号（式の rotGroup は無しになる。式を新しく作ったときはこれ） */
export const OWN_GROUP = -1

/** パラメータ（スライダーで値を決める文字）の値。複素数も扱えるよう実部と虚部を持つ */
export interface ParamValue {
  re: number
  im: number
  /** 複素数のスライダー（複素平面）にしたパラメータか。これを使った定数の式は複素平面の点になる */
  complex?: boolean
}

/** パラメータのスライダーの設定 */
export interface SliderStyle {
  min: number
  max: number
  /** 複素数モード（複素平面の上で点を動かして値を決める） */
  complex: boolean
  /** 範囲で重ねる（min〜max で動かしたときに図形が通る領域を描く） */
  stack: boolean
  /** 範囲を式（LaTeX）で決めるときの [最小, 最大]。ほかのパラメータも使える（a ≤ c ≤ 2b など）。
   *  あれば min, max はこの式の値（古い保存データや、数値だけのときは無い） */
  bounds?: [string, string]
  /** 刻み（Desmos の Step）の式（LaTeX）。空または無ければ自動（範囲のおよそ 1/200 のきりの良い値） */
  stepExpr?: string
  /** stepExpr の値。値はこの刻みで最小値からそろえる */
  step?: number
}

/** 式の一覧の1行 */
export interface PlotItem extends PlotStyle {
  id: number
  /** 入力された式（LaTeX）。計算エンジンに渡す前に latexToText で変換する */
  latex: string
  /** 設定欄を開いているか */
  showSettings: boolean
  /** 媒介変数表示 (a, b, c) を、成分ごとに縦に並べた入力欄で編集するか */
  vertical: boolean
  /** 式の誤りの説明（無ければ undefined） */
  error?: string
  /** 計算にかかった時間などの情報 */
  info?: string
  /** 媒介変数の範囲 */
  ranges: ParamRanges
  /** 最後に描けたときの媒介変数の名前（範囲の入力欄を出すのに使う）。例: ['t'], ['u', 'v'] */
  params: string[]
  /** 媒介変数 u, v（立体なら3つ目の w も）の範囲の長さ（格子線の分割に使う） */
  paramSpan: ParamSpan
  /** この行がパラメータの定義（a = 1 など）なら、その名前と値 */
  param?: { name: string } & ParamValue
  slider: SliderStyle
  /** スライダーの範囲の式の誤りの説明（無ければ undefined） */
  sliderError?: string
  /** この式で使われているパラメータの名前（値が変わったら計算し直す） */
  usedParams: string[]
  /** 非表示の間に計算し直す必要が生じた（表示したときに計算する） */
  stale?: boolean
  /** 表示されたらすぐ入力を始める（エンターで新しく足した式） */
  autofocus?: boolean
  /** 範囲で重ねた立体として描いたことがあるか（最初の1回だけ半透明にするため） */
  solidSeen?: boolean
  /** 複素数の色分け（陽関数だけ。古い保存データには無い） */
  complexMode?: ComplexMode
  /** 表示していない座標（w 以降）の見せ方（無ければ断面）。範囲は ranges に座標の名前（w, w2, …）で入れる */
  hidden?: HiddenStyle
  /** 座標を複素数にする（x, y, z を x + i w, y + i w₂, z + i w₃ に置き換える。6 次元以上） */
  complexCoords?: boolean
  /** この式だけの断面の値（座標の名前 → 値）。無い座標は次元の設定の断面の値を使う */
  slice?: Record<string, number>
  /** 陽関数の値が飛ぶ所（floor の段差など）に縦の壁を描くか（古い保存データには無い） */
  walls?: boolean
  /** 回転のグループの番号（無ければこの式だけの回転 rotation, center、0 なら全体の回転を使う） */
  rotGroup?: number
  /** この式だけの回転角（rotGroup が無いとき使う。無ければ全部 0） */
  rotation?: number[]
  /** この式だけの回転の中心（rotGroup が無いとき使う。無ければ原点） */
  center?: number[]
  /** 媒介変数が4つ以上の図形で、固定する媒介変数の範囲を何等分した所に面を描くか（無い・1 なら両端だけ） */
  layers?: number
  /** 媒介変数が3つ以上の図形を、面ではなく辺だけで描くか（無ければ面） */
  edges?: boolean
  /** 入っているフォルダの番号（無ければフォルダの外）。同じフォルダの式は、一覧で続けて並ぶ */
  folder?: number
  /** 最後に描けたときのグラフの種類（例: "z = f(x, y)", "陰関数"） */
  kind?: string
  /** 座標系の番号（CoordSystem.id。無ければ直交座標） */
  coordSys?: number
}

/** 立体の切り口: 画面の軸 axis（0: 横, 1: 奥, 2: 縦）に垂直な平面で切り、片側だけを表示する */
export interface CutStyle {
  enabled: boolean
  axis: 0 | 1 | 2
  /** 切る位置（その軸の座標） */
  pos: number
  /** false なら座標が pos 以下の側を残す。true なら pos 以上の側を残す */
  flip: boolean
}

/** 計算担当に渡す切り口の指定（切らないなら null） */
export type CutRequest = { axis: number; pos: number; keepBelow: boolean } | null

/** 切り口のふた（陰関数・範囲で重ねた立体の中身を、切り口の平面で切った部分） */
export interface CapData {
  positions: Float32Array
  normals: Float32Array
  indices: Uint32Array
  /** 頂点ごとの色分けの値（粗い版で、本体が値で色を付けるとき。無ければ省略） */
  complexValues?: Float32Array
  /** 座標系で移した先 T(q)（頂点ごとに 3 つ。粗い版で、座標系を使うとき。無ければ省略） */
  target?: Float32Array
}

/** Rust 側で作ったメッシュ */
export interface MeshData {
  positions: Float32Array
  normals: Float32Array
  indices: Uint32Array
  /** 粗い版の三角形の頂点番号（視点を動かしている間に描く。空なら indices をそのまま描く） */
  lodIndices: Uint32Array
  /** 自分の頂点を持つ粗い版（陰関数。F の格子を間引いて作り直したもの）。無ければ null */
  lod: CapData | null
  /** 格子線用の座標（空なら positions を使う） */
  gridCoords: Float32Array
  /** 複素数の色分け: 頂点ごとの (Re w / |w|, Im w / |w|, log₂|w|)。空なら単色 */
  complexValues: Float32Array
  /** 曲線なら true（positions は点の列で、途切れる所に NaN） */
  curve: boolean
  /** 媒介変数表示の立体（閉じた面で外向き）なら true。切り口のふたは描画側で作る */
  solid: boolean
  /** 2D 表示の z = f(x, y) の塗り分け（格子線用の座標の3つ目が値。値で色の濃さを変える） */
  heat: boolean
  /** 媒介変数の名前 */
  params: string[]
  /** 媒介変数の範囲の長さ */
  paramSpan: ParamSpan
  /**
   * 頂点に掛ける 1 次式 q = M p + t（[M の 1 行目, t₀, 2 行目, t₁, 3 行目, t₂] の 12 個）。回す前の形で作った陰関数だけ。
   * 描画側は 3D オブジェクトにこれを掛ける（回転だけが変わったときは、メッシュを作り直さず、これだけが届く）。無ければ null
   */
  transform: Float64Array | null
  /** true なら形は前と同じで、transform だけが新しい（メッシュの配列は空） */
  transformOnly: boolean
  /** 表示していない座標の値で色を付けるなら [最小, 最大]（値は complexValues の 1 つ目）。付けないなら null */
  valueRange: [number, number] | null
  /** 伝達関数で描くなら、格子（1 辺 n 点。x が最も速く変わる）での値。無ければ省略 */
  volume?: { n: number; values: Float32Array }
  /** 2D 表示の不等式の領域の境目の線（実線。点の列で、途切れる所に NaN）。無ければ省略 */
  outline?: Float32Array
  /** 同じく、厳密な不等式（`<`・`>`）の境目の線（点線で描く） */
  outlineDashed?: Float32Array
  /** 座標系で移した先 T(q)（頂点ごとに 3 つ。座標系を使う式だけ。無ければ省略）。描画側が移行のスライダーで positions と混ぜる */
  target?: Float32Array
  /** outline・outlineDashed を座標系で移した先 */
  outlineTarget?: Float32Array
  outlineDashedTarget?: Float32Array
}

// ---- Web Worker とのやりとり ----

export interface PlotRequest {
  requestId: number
  /** どの式の依頼か（媒介変数表示の計算結果を、式ごとに使い回すため） */
  itemId: number
  source: string
  domain: Domain
  /** 陽関数の細かさ（1辺のマス数） */
  resolution: number
  /** 陰関数の細かさ（立方体の1辺のマス数） */
  implicitResolution: number
  ranges: ParamRanges
  /** ほかの行で定義されているパラメータ */
  params: Record<string, ParamValue>
  space: {
    dims: number
    axes: number[]
    slice: number[]
    /** 各座標の範囲の式 [最小, 最大]（計算エンジン用の式。重ねない座標は null） */
    ranges: ([string, string] | null)[]
    samples: number
    /** 回転角（ラジアン） */
    rotation: number[]
    center: number[]
    /** 4次元の遠近法の視点までの距離（0 なら遠近法なし） */
    perspective: number
  }
  /** 範囲で重ねるパラメータ → [最小, 最大] */
  stacks: Record<string, [number, number]>
  /** 切り口（陰関数・範囲で重ねた立体のふたを作る） */
  cut: CutRequest
  /** 陽関数の複素数の色分け */
  complexMode: ComplexMode
  /** 表示していない座標の見せ方（番号は HIDDEN_MODE_ORDER。0 か null なら断面） */
  hidden: { mode: number; count: number } | null
  /** 座標を複素数にする */
  complexCoords: boolean
  /**
   * 座標系（計算エンジン用の式。無ければ直交座標）。map は組 (X, Y, Z) の式、ranges は作る範囲 [最小, 最大] × 3（式）、
   * names は座標の名前 3 つ（カンマ区切り。r,theta,phi など。空なら x, y, z だけ）。
   * 座標系があると、domain の代わりに作る範囲で図形を作る（画面の箱は domain のまま）
   */
  coord: { map: string; ranges: [string, string][]; names: string } | null
  /** 2D 表示（縦の座標を 0 にして平面に描く） */
  flat: boolean
  /** 2D 表示の描き方。true なら切り口（縦の座標 = 0 の平面との交わりの線）、false なら投影 */
  flatSlice: boolean
  /** 陽関数の段差の壁 */
  walls: boolean
  /** 媒介変数が4つ以上の図形の、面を描く所の分割数（1 なら範囲の両端だけ） */
  layers: number
  /**
   * そのほかの設定（ビットごと）。ビット 0（PLOT_NO_REFINE）: 形が急に変わる所を細かくしない（再生・ドラッグ中）。
   * ビット 1（PLOT_THOROUGH）: 陰関数の小さい図形を念入りに探す。ビット 2（PLOT_EDGES）: 辺だけで描く。
   * ビット 3（PLOT_LOCAL）: 回転だけが変わった依頼（plotClient が付ける）。計算担当が、面を返した後で陰関数の回す前の形を作る
   */
  flags: number
  /**
   * 重い陰関数の F の計算を、いくつかの Worker に分けるときの段階（無ければふつうに計算する）。
   * plan: かたまりの数だけ返す / chunks: かたまり from..to の値だけ返す / given: 渡した値でメッシュを作る
   */
  field?: { mode: 'plan' } | { mode: 'chunks'; from: number; to: number } | { mode: 'given'; values: Float32Array }
}

/** F の計算を分ける途中の段階（計画・一部の計算）の返事 */
export type FieldStageResponse =
  | { requestId: number; ok: true; chunks: number; values: Float32Array | null; ms: number }
  | { requestId: number; ok: false; error: string }

/** PlotRequest.flags のビット 0: 形が急に変わる所を細かくしない（再生・ドラッグ中。止まったら細かく計算し直す） */
export const PLOT_NO_REFINE = 1
/**
 * PlotRequest.flags のビット 3: 回転だけが変わった、細かく作る依頼（plotClient が付ける）。計算担当は表示する面を返した後で、
 * 回転が表示中の座標どうしだけの陰関数を回す前の形で（表示範囲より広く）作っておき、次に回すときは回すだけで描く。
 * 計算エンジン（plot）に渡すときは「回す前の形だけを作る」の意味になるので、計算担当はふつうの呼び出しではこの印を外す
 */
export const PLOT_LOCAL = 8
/** PlotRequest.flags のビット 1: 陰関数で、格子より小さい図形を念入りに探す（遅くなる。⚙ の設定） */
export const PLOT_THOROUGH = 2
/** PlotRequest.flags のビット 2: 媒介変数が3つ以上の図形を、辺だけで描く */
export const PLOT_EDGES = 4

/** 切り口の位置だけが変わったときの依頼: 覚えておいた中身から、ふただけを作り直す */
export interface CapRequest {
  type: 'cap'
  requestId: number
  itemId: number
  cut: CutRequest
  /** ふたの格子の1辺のマス数 */
  resolution: number
}

export type CapResponse = { requestId: number; cap: CapData | null }

/** 座標系の格子（座標曲線。計画 S39）の依頼。map・ranges・names は PlotRequest.coord と同じ（計算エンジン用の式） */
export interface CoordGridRequest {
  type: 'grid'
  requestId: number
  map: string
  ranges: [string, string][]
  names: string
  params: Record<string, ParamValue>
  /** 範囲を何等分した所に線を引くか */
  lines: number
  /** 線を引く面（ビット a: 座標 a が範囲の最小の面） */
  faces: number
  flat: boolean
}

/** 座標系の格子: 元の位置（座標の値）と移した先（点の列。線の間は NaN）。式に誤りがあれば error */
export type CoordGridResponse =
  | { requestId: number; ok: true; positions: Float32Array; target: Float32Array }
  | { requestId: number; ok: false; error: string }

export type PlotResponse =
  | {
      requestId: number
      ok: true
      mesh: MeshData
      kind: string
      ms: number
      usedParams: string[]
      /** パラメータの定義だったときの名前と値 */
      param: ({ name: string } & ParamValue) | null
      /** 切り口のふた（陰関数・範囲で重ねた立体を切っているときだけ） */
      cap: CapData | null
      /** 計算担当の中での時間の内訳（ベンチマーク用） */
      timing: PlotTiming
      /** 媒介変数表示の n 次元の点を使い回して、射影だけし直したか（細かさと関係ない時間なので、見積もりに数えない） */
      reprojected: boolean
      /** 計算時間が細かさの何乗に比例するか（budget.ts を参照） */
      cost: Cost
    }
  | { requestId: number; ok: false; error: string }

/**
 * 計算担当の中での時間の内訳（ミリ秒）。eval + mesh + lod + copy ≈ total
 * （残りは範囲の式の計算や、計算エンジンへの引数の受け渡し）
 */
export type PlotTiming = {
  /** 式の計算（切り口のふたの分を含む） */
  eval: number
  /** 式の計算と粗い版を除いた、メッシュを作る時間（射影し直しだけのときは射影の時間） */
  mesh: number
  /** 粗い版を作る時間 */
  lod: number
  /** 計算エンジンから配列を取り出す時間（WASM → JavaScript のコピー） */
  copy: number
  /** 依頼を受け取ってから返事を作り終えるまで */
  total: number
  /** 返事を送った時刻（performance.timeOrigin + performance.now()。画面側で受け渡しの時間を出すのに使う） */
  sentAt: number
}

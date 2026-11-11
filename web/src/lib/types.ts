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
  /** 入っているフォルダの番号（無ければフォルダの外）。同じフォルダの式は、一覧で続けて並ぶ */
  folder?: number
  /** 最後に描けたときのグラフの種類（例: "z = f(x, y)", "陰関数"） */
  kind?: string
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
}

/** Rust 側で作ったメッシュ */
export interface MeshData {
  positions: Float32Array
  normals: Float32Array
  indices: Uint32Array
  /** 粗い版の三角形の頂点番号（視点を動かしている間に描く。空なら indices をそのまま描く） */
  lodIndices: Uint32Array
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
  /** 2D 表示（縦の座標を 0 にして平面に描く） */
  flat: boolean
  /** 2D 表示の描き方。true なら切り口（縦の座標 = 0 の平面との交わりの線）、false なら投影 */
  flatSlice: boolean
  /** 陽関数の段差の壁 */
  walls: boolean
  /** 媒介変数が4つ以上の図形の、面を描く所の分割数（1 なら範囲の両端だけ） */
  layers: number
}

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

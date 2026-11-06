// アプリ全体で使うデータの形

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
  /** 線の太さ（画素） */
  width: number
}

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
  /** 各座標を「範囲で重ねる」なら [最小, 最大]、断面なら null（表示していない座標だけが使われる） */
  ranges: ([number, number] | null)[]
  /** 範囲を何等分して重ねるか */
  samples: number
  /** 回転角（度）。座標の組 (0,1), (0,2), …, (0,n−1), (1,2), … の順に nC2 個 */
  rotation: number[]
  /** 回転の中心 */
  center: number[]
  /** 4次元の遠近法の強さ（0〜100。0 なら平行な射影）。射影で描く図形（媒介変数表示・回した陽関数）だけに効く */
  perspective: number
}

/** パラメータ（スライダーで値を決める文字）の値。複素数も扱えるよう実部と虚部を持つ */
export interface ParamValue {
  re: number
  im: number
}

/** パラメータのスライダーの設定 */
export interface SliderStyle {
  min: number
  max: number
  /** 複素数モード（複素平面の上で点を動かして値を決める） */
  complex: boolean
  /** 範囲で重ねる（min〜max で動かしたときに図形が通る領域を描く） */
  stack: boolean
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
  /** 媒介変数 u, v の範囲の長さ（格子線の分割に使う） */
  paramSpan: { u: number; v: number }
  /** この行がパラメータの定義（a = 1 など）なら、その名前と値 */
  param?: { name: string } & ParamValue
  slider: SliderStyle
  /** この式で使われているパラメータの名前（値が変わったら計算し直す） */
  usedParams: string[]
  /** 表示されたらすぐ入力を始める（エンターで新しく足した式） */
  autofocus?: boolean
  /** 範囲で重ねた立体として描いたことがあるか（最初の1回だけ半透明にするため） */
  solidSeen?: boolean
}

/** Rust 側で作ったメッシュ */
export interface MeshData {
  positions: Float32Array
  normals: Float32Array
  indices: Uint32Array
  /** 格子線用の座標（空なら positions を使う） */
  gridCoords: Float32Array
  /** 曲線なら true（positions は点の列で、途切れる所に NaN） */
  curve: boolean
  /** 媒介変数の名前 */
  params: string[]
  /** 媒介変数 u, v の範囲の長さ */
  paramSpan: { u: number; v: number }
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
    /** 各座標の範囲 [最小, 最大, …]（重ねない座標は NaN） */
    ranges: number[]
    samples: number
    /** 回転角（ラジアン） */
    rotation: number[]
    center: number[]
    /** 4次元の遠近法の視点までの距離（0 なら遠近法なし） */
    perspective: number
  }
  /** 範囲で重ねるパラメータ → [最小, 最大] */
  stacks: Record<string, [number, number]>
}

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
    }
  | { requestId: number; ok: false; error: string }

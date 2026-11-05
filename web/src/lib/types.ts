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

/** 媒介変数の名前: 't'（曲線）, 'uv'（曲面）, ''（媒介変数表示でない） */
export type ParamKind = 't' | 'uv' | ''

/** 媒介変数の範囲（入力された文字列のまま。"2pi" などの式も書ける） */
export interface ParamRanges {
  t: [string, string]
  u: [string, string]
  v: [string, string]
}

/** 式の一覧の1行 */
export interface PlotItem extends PlotStyle {
  id: number
  source: string
  /** 設定欄を開いているか */
  showSettings: boolean
  /** 式の誤りの説明（無ければ undefined） */
  error?: string
  /** 計算にかかった時間などの情報 */
  info?: string
  /** 媒介変数の範囲 */
  ranges: ParamRanges
  /** 最後に描けたときの媒介変数の種類（範囲の入力欄を出すのに使う） */
  params: ParamKind
  /** 媒介変数 u, v の範囲の長さ（格子線の分割に使う） */
  paramSpan: { u: number; v: number }
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
  params: ParamKind
  /** 媒介変数 u, v の範囲の長さ */
  paramSpan: { u: number; v: number }
}

// ---- Web Worker とのやりとり ----

export interface PlotRequest {
  requestId: number
  source: string
  domain: Domain
  /** 陽関数の細かさ（1辺のマス数） */
  resolution: number
  /** 陰関数の細かさ（立方体の1辺のマス数） */
  implicitResolution: number
  ranges: ParamRanges
}

export type PlotResponse =
  | { requestId: number; ok: true; mesh: MeshData; kind: string; ms: number }
  | { requestId: number; ok: false; error: string }

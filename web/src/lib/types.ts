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

/** 曲面に描く格子線の設定。各方向の線の間隔（0 なら描かない） */
export interface GridStyle {
  enabled: boolean
  x: number
  y: number
  z: number
}

/** 曲面の見た目 */
export interface PlotStyle {
  color: string
  visible: boolean
  /** 面の不透明度（0〜1）。0 にすると格子線だけの曲面になる */
  opacity: number
  grid: GridStyle
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
}

/** Rust 側で作ったメッシュ */
export interface MeshData {
  positions: Float32Array
  normals: Float32Array
  indices: Uint32Array
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
}

export type PlotResponse =
  | { requestId: number; ok: true; mesh: MeshData; kind: string; ms: number }
  | { requestId: number; ok: false; error: string }

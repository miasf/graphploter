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

/** 式の一覧の1行 */
export interface PlotItem {
  id: number
  source: string
  color: string
  visible: boolean
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
  resolution: number
}

export type PlotResponse =
  | { requestId: number; ok: true; mesh: MeshData; ms: number }
  | { requestId: number; ok: false; error: string }

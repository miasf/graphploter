// 式ごとの、最後の計算の時間の内訳（ベンチマーク用。ふつうの画面には出さない）

import type { PlotTiming } from './types'

/** 計算担当の中の内訳に、画面側で測った時間を足したもの（ミリ秒） */
export type PlotBreakdown = PlotTiming & {
  /** 計算担当が返事を送ってから、画面側が受け取るまで（受け渡し） */
  transfer: number
  /** 描画側に図形を登録する時間（viewer.setPlot と setCap） */
  setPlot: number
  /** 式（どの式の内訳か） */
  source: string
  /** 図形の種類（計算担当の返事の kind） */
  kind: string
  /** 媒介変数表示の点を使い回して、射影だけし直したか */
  reprojected: boolean
  /** 回転だけが変わり、形はそのままで向きだけを変えたか */
  transformOnly: boolean
  /** 三角形の数 */
  triangles: number
}

/** 式の行の番号 → 最後の計算の内訳 */
export const lastBreakdown = new Map<number, PlotBreakdown>()

// ?bench=1 のときだけ、ブラウザの開発者ツールやベンチマークから読めるようにする
if (typeof location !== 'undefined' && new URLSearchParams(location.search).get('bench') === '1') {
  ;(globalThis as { __plotTimings?: typeof lastBreakdown }).__plotTimings = lastBreakdown
}

/** 内訳が届くたびに呼ぶ関数（ベンチマークが、回転の再生中の往復の時間を集めるのに使う） */
export const breakdownListeners = new Set<(id: number, b: PlotBreakdown) => void>()

/** 式の行 id の最後の計算の内訳を覚え、待っている関数に知らせる */
export function recordBreakdown(id: number, b: PlotBreakdown) {
  lastBreakdown.set(id, b)
  for (const f of breakdownListeners) f(id, b)
}

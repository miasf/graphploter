/**
 * 描画の方法（計画 docs/plan-hard-tasks.md 12 章の G7）: 式の ⚙ の「自動・メッシュ・画素ごと（GPU）」から、
 * GPU の描き方（2D の画素ごとの曲線・領域、3D の光線）を使うかを決める。
 *
 * 自動: G0 の判断のとおり、デスクトップ・ノート PC では GPU の描き方、スマートフォン（タッチが主の端末）ではメッシュ。
 * スマートフォンで GPU の描き方を既定にするかは、実機で G5 の材質を測ってから決める。
 * URL の ?gpu2d=0・?gpu3d=0 は、式の設定によらず使わない（比べるため）
 */
import type { RenderMode } from './types'

// 見た目を設定するたびに呼ばれる（再生中は毎コマ）ので、メディアクエリと URL の読み取りは 1 回だけにする。
// MediaQueryList の matches は、端末の状態が変わればその時の値を返す
const coarse = typeof matchMedia === 'undefined' ? null : matchMedia('(pointer: coarse)')
const urlOff = (() => {
  if (typeof location === 'undefined') return { '2d': false, '3d': false }
  const q = new URLSearchParams(location.search)
  return { '2d': q.get('gpu2d') === '0', '3d': q.get('gpu3d') === '0' }
})()

/** この端末で、自動のときに GPU の描き方を使うか */
export function gpuByDefault(): boolean {
  return coarse === null || !coarse.matches
}

/** URL で GPU の描き方を止めているか（kind は '2d' か '3d'） */
function disabledByUrl(kind: '2d' | '3d'): boolean {
  return urlOff[kind]
}

/** 式の描画の方法 mode で、GPU の描き方を使うか（式が GPU に移せるかは、呼び出し側が別に確かめる） */
export function wantGpu(mode: RenderMode | undefined, kind: '2d' | '3d'): boolean {
  if (disabledByUrl(kind)) return false
  const m = mode ?? 'auto'
  if (m === 'mesh') return false
  if (m === 'gpu') return true
  return gpuByDefault()
}

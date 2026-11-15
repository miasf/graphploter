/**
 * 描画の方法（計画 docs/plan-hard-tasks.md 12 章の G7）: 式の ⚙ の「自動・メッシュ・画素ごと（GPU）」から、
 * GPU の描き方（2D の画素ごとの曲線・領域、3D の光線）を使うかを決める。
 *
 * 自動: G0 の判断のとおり、デスクトップ・ノート PC では GPU の描き方、スマートフォン（タッチが主の端末）ではメッシュ。
 * スマートフォンで GPU の描き方を既定にするかは、実機で G5 の材質を測ってから決める。
 * URL の ?gpu2d=0・?gpu3d=0 は、式の設定によらず使わない（比べるため）
 */
import type { RenderMode } from './types'

/** この端末で、自動のときに GPU の描き方を使うか */
export function gpuByDefault(): boolean {
  if (typeof matchMedia === 'undefined') return true
  return !matchMedia('(pointer: coarse)').matches
}

/** URL で GPU の描き方を止めているか（kind は '2d' か '3d'） */
function disabledByUrl(kind: '2d' | '3d'): boolean {
  if (typeof location === 'undefined') return false
  return new URLSearchParams(location.search).get(kind === '2d' ? 'gpu2d' : 'gpu3d') === '0'
}

/** 式の描画の方法 mode で、GPU の描き方を使うか（式が GPU に移せるかは、呼び出し側が別に確かめる） */
export function wantGpu(mode: RenderMode | undefined, kind: '2d' | '3d'): boolean {
  if (disabledByUrl(kind)) return false
  const m = mode ?? 'auto'
  if (m === 'mesh') return false
  if (m === 'gpu') return true
  return gpuByDefault()
}

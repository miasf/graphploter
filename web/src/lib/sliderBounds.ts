// スライダーの最小値・最大値を式で決める（Desmos のように 0 ≤ c ≤ b、a ≤ c ≤ 2b など）。
// 式の値は画面側で計算する。ただの数値なら JS で読み、式なら WASM の計算エンジンを使う
// （WASM は式が初めて出てきたときに読み込む）。

import init, { eval_range } from '../wasm-core/graph_core.js'
import { latexToText } from './latex'
import type { ParamValue } from './types'

let wasm: 'none' | 'loading' | 'ready' = 'none'

/** 式の値。WASM の準備がまだなら null（準備ができたら onReady を呼ぶ） */
function evaluate(latex: string, params: Record<string, ParamValue>, onReady: () => void): number | null {
  const text = latexToText(latex).trim()
  if (text === '') throw new Error('値を入れてください')
  if (/^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(text)) return Number(text)
  if (wasm !== 'ready') {
    if (wasm === 'none') {
      wasm = 'loading'
      init().then(() => {
        wasm = 'ready'
        onReady()
      })
    }
    return null
  }
  const names = Object.keys(params).join(',')
  const values = new Float64Array(Object.values(params).flatMap((p) => [p.re, p.im]))
  return eval_range(text, names, values)
}

/**
 * スライダーの範囲の式 [最小, 最大] を数値にする。params には自分自身を入れない（自分を使う式はエラーになる）。
 * 結果: 数値の範囲 / 誤りの説明 / WASM の準備待ち（null）
 */
export function evalBounds(
  exprs: [string, string],
  params: Record<string, ParamValue>,
  onReady: () => void,
): { min: number; max: number } | { error: string } | null {
  try {
    const min = evaluate(exprs[0], params, onReady)
    const max = evaluate(exprs[1], params, onReady)
    if (min === null || max === null) return null
    if (!Number.isFinite(min) || !Number.isFinite(max)) return { error: 'スライダーの範囲: 値が有限ではありません' }
    if (!(min < max)) return { error: 'スライダーの範囲: 最小値を最大値より小さくしてください' }
    return { min, max }
  } catch (e) {
    return { error: `スライダーの範囲: ${e instanceof Error ? e.message : String(e)}` }
  }
}

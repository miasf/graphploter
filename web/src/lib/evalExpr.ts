// 画面側で式の値を計算する: スライダーの最小値・最大値（Desmos のように 0 ≤ c ≤ b、a ≤ c ≤ 2b など）や、
// 表示範囲・回転の中心の数値の欄（2π なども書ける）。ただの数値なら JS で読み、式なら WASM の計算エンジンを使う
// （WASM は式が初めて出てきたときに読み込む）。

import init, { definitions, eval_range } from '../wasm-core/graph_core.js'
import { latexToText } from './latex'
import type { ParamValue } from './types'

let wasm: 'none' | 'loading' | 'ready' = 'none'
/** 読み込み中に値を頼んだ所（読み込み終わったら全部呼ぶ。前は最初の 1 つだけを呼んでいたので、ほかの欄は計算中のままになった） */
const waiting = new Set<() => void>()

/** 式の値。WASM の準備がまだなら null（準備ができたら onReady を呼ぶ） */
function evaluate(latex: string, params: Record<string, ParamValue>, onReady: () => void, defs = ''): number | null {
  const text = latexToText(latex).trim()
  if (text === '') throw new Error('値を入れてください')
  if (/^[-+]?(\d+\.?\d*|\.\d+)(e[-+]?\d+)?$/i.test(text)) return Number(text)
  if (wasm !== 'ready') {
    waiting.add(onReady)
    if (wasm === 'none') {
      wasm = 'loading'
      init().then(() => {
        wasm = 'ready'
        const all = [...waiting]
        waiting.clear()
        for (const f of all) f()
      })
    }
    return null
  }
  const names = Object.keys(params).join(',')
  const values = new Float64Array(Object.values(params).flatMap((p) => [p.re, p.im]))
  if (defs !== '') definitions(defs)
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

/**
 * 数値の欄に入れた式（パラメータは使わない。2π, 1/3 など）の値。
 * 結果: 数値 / 誤りの説明 / WASM の準備待ち（null）
 */
export function evalNumber(latex: string, onReady: () => void): { value: number } | { error: string } | null {
  try {
    const value = evaluate(latex, {}, onReady)
    if (value === null) return null
    if (!Number.isFinite(value)) return { error: '値が有限ではありません' }
    return { value }
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) }
  }
}

/**
 * 式（パラメータを使ってよい）が計算できるか。誤りの説明 / 計算できるなら null / WASM の準備待ちなら undefined
 */
export function exprError(latex: string, params: Record<string, ParamValue>, onReady: () => void): string | null | undefined {
  try {
    const v = evaluate(latex, params, onReady)
    if (v === null) return undefined
    return Number.isFinite(v) ? null : '値が有限ではありません'
  } catch (e) {
    return e instanceof Error ? e.message : String(e)
  }
}

/** パラメータを使ってよい式の値。結果: 数値 / 誤りの説明 / WASM の準備待ち（null） */
export function evalValue(
  latex: string,
  params: Record<string, ParamValue>,
  onReady: () => void,
  /** 定義の行（計算エンジン用の文字列を改行でつないだもの。関数・ベクトル場の定義を使う式のため） */
  defs = '',
): { value: number } | { error: string } | null {
  try {
    const value = evaluate(latex, params, onReady, defs)
    if (value === null) return null
    if (!Number.isFinite(value)) return { error: '値が有限ではありません' }
    return { value }
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) }
  }
}

// パラメータ（スライダー）・次元まわりの小さな関数

import type { Space } from './types'

/** 座標（x, y, z, w, w2, w3, …）と定数（e, i）の名前か。これらはパラメータにできない。
 *  t, u, v は組 (…, …) の外ではパラメータにできる */
export function isReserved(name: string): boolean {
  return /^([xyzei]|w\d*)$/.test(name)
}

/** k 番目の座標の名前（0: x, 1: y, 2: z, 3: w, 4: w2, …） */
export function coordName(k: number): string {
  return k < 4 ? 'xyzw'[k] : `w${k - 2}`
}

/** 座標の組 (i, j)（i < j）を、回転角と同じ順に並べたもの */
export function pairs(n: number): [number, number][] {
  const out: [number, number][] = []
  for (let i = 0; i < n; i++) for (let j = i + 1; j < n; j++) out.push([i, j])
  return out
}

/** 次元を n に変えた空間の設定。断面の値・範囲・回転角は、同じ座標（の組）のものを引き継ぐ */
export function resizeSpace(space: Space, n: number): Space {
  n = Math.max(3, Math.min(10, n))
  const slice = Array.from({ length: n }, (_, k) => space.slice[k] ?? 0)
  const ranges = Array.from({ length: n }, (_, k) => space.ranges[k] ?? null)
  // 回転は max(次元, 5) 次元で行う
  const { rotation, center } = resizeRotation(space.rotation, space.center, space.dims, n)
  // 表示中の座標が無くなる場合は、x, y, z の表示に戻す
  const axes = space.axes.every((a) => a < n) ? space.axes : ([0, 1, 2] as [number, number, number])
  return { ...space, dims: n, axes: [...axes] as [number, number, number], slice, ranges, center, rotation }
}

/** 次元を from から to に変えたときの回転角と中心。同じ座標（の組）のものを引き継ぐ */
export function resizeRotation(
  rotation: number[],
  center: number[],
  from: number,
  to: number,
): { rotation: number[]; center: number[] } {
  const oldAngle = new Map<string, number>()
  pairs(rotDims(from)).forEach(([i, j], k) => oldAngle.set(`${i},${j}`, rotation[k] ?? 0))
  return {
    rotation: pairs(rotDims(to)).map(([i, j]) => oldAngle.get(`${i},${j}`) ?? 0),
    center: Array.from({ length: rotDims(to) }, (_, k) => center[k] ?? 0),
  }
}

/**
 * 保存データ・共有リンクの回転角と中心を、今の並び（max(次元, 5) 次元の座標の組 nC2 個）にそろえる。
 * 5次元の回転ができる前の保存データは、次元 dims の nC2 個（3次元なら3個）だったので、
 * 同じ座標の組の角度を引き継いで並べ直す（そのまま使うと角度が別の組にずれ、足りない分が NaN になる）
 */
export function fitRotation(rotation: unknown, center: unknown, dims: number): { rotation: number[]; center: number[] } {
  const n = rotDims(dims)
  const num = (v: unknown) => (typeof v === 'number' && Number.isFinite(v) ? v : 0)
  const rot = Array.isArray(rotation) ? rotation.map(num) : []
  const cen = Array.isArray(center) ? center.map(num) : []
  const from = rot.length === pairs(dims).length ? dims : n
  const old = new Map<string, number>()
  pairs(from).forEach(([i, j], k) => old.set(`${i},${j}`, rot[k] ?? 0))
  return {
    rotation: pairs(n).map(([i, j]) => old.get(`${i},${j}`) ?? 0),
    center: Array.from({ length: n }, (_, k) => cen[k] ?? 0),
  }
}

/** 回転に使う次元。次元が5未満でも、w, w₂ を使った5次元の回転ができる */
export function rotDims(dims: number): number {
  return Math.max(dims, 5)
}

/** 画面に出すときの名前（w2 → w₂） */
export function prettyName(name: string): string {
  return name.replace(/\d/g, (d) => '₀₁₂₃₄₅₆₇₈₉'[Number(d)])
}

/** 数式入力欄（LaTeX）での名前（t1 → t_{1}） */
export function latexName(name: string): string {
  return name.replace(/^([a-zA-Z])(\d+)$/, '$1_{$2}')
}

/** 範囲 range をおよそ n 個に区切る「きりの良い」刻み（1, 2, 5 × 10^k） */
export function niceStep(range: number, n: number): number {
  const raw = range / n
  const mag = 10 ** Math.floor(Math.log10(raw))
  const r = raw / mag
  return (r < 1.5 ? 1 : r < 3 ? 2 : r < 7 ? 5 : 10) * mag
}

/** x を表すのに要る小数の桁数（0.25 → 2、0.1 → 1、5 → 0）。12 桁まで */
function decimals(x: number): number {
  x = Math.abs(x)
  if (!Number.isFinite(x) || x === 0) return 0
  let d = Math.max(0, -Math.floor(Math.log10(x)))
  while (d < 12 && Math.abs(x * 10 ** d - Math.round(x * 10 ** d)) > 1e-9 * x * 10 ** d) d++
  return d
}

/**
 * 刻み step（と、刻みの基準の値 base。スライダーの値は 最小値 + 刻みの整数倍）で表せる桁まで丸め、
 * 余計な桁を落とした文字列にする（0.30000000000000004 → "0.3"）。
 * 刻みが 0.25 なら小数第2位まで書く（第1位までにすると 0.25 が "0.3" になる）
 */
export function formatNumber(v: number, step: number, base = 0): string {
  const digits = Math.max(decimals(step), decimals(base))
  return String(Number(v.toFixed(digits)))
}

/** 複素数 re + im·i を、数式入力欄に入れる形（LaTeX）にする。例: 1.5−0.5i → "1.5-0.5i" */
export function complexLatex(re: number, im: number, step: number, base = 0): string {
  const a = formatNumber(re, step, base)
  const b = formatNumber(im, step, base)
  if (b === '0') return a
  const imPart = b === '1' ? 'i' : b === '-1' ? '-i' : `${b}i`
  if (a === '0') return imPart
  return imPart.startsWith('-') ? `${a}${imPart}` : `${a}+${imPart}`
}

/**
 * 計算エンジンのエラー「変数 'a' はここでは使えません」から、パラメータにできる文字を取り出す。
 * パラメータにできない文字（x など）なら null。
 */
export function missingParam(error: string | undefined): string | null {
  const m = error?.match(/変数 '([a-zA-Z]\d*)' はここでは使えません/)
  return m && !isReserved(m[1]) ? m[1] : null
}

/**
 * 計算エンジンのエラー「次元を 5 にしてください」「次元を 5 以上にしてください」から、必要な次元を取り出す。
 * 次元のエラーでなければ null
 */
export function neededDims(error: string | undefined): number | null {
  const m = error?.match(/次元を (\d+) (?:以上)?にしてください/)
  return m ? Number(m[1]) : null
}

/** スライダーの刻み。刻み（Step）を決めていれば、その値。無ければ範囲のおよそ 1/200 のきりの良い値 */
export function sliderStep(slider: { min: number; max: number; step?: number }): number {
  if (slider.step && slider.step > 0) return slider.step
  return slider.max > slider.min ? niceStep(slider.max - slider.min, 200) : 0.01
}

/** 刻みを決めたスライダーの値を、最小値から刻みの整数倍の所にそろえる（刻みが自動ならそのまま） */
export function snapToStep(v: number, slider: { min: number; step?: number }): number {
  if (!slider.step || !(slider.step > 0)) return v
  const k = Math.round((v - slider.min) / slider.step)
  return Number((slider.min + k * slider.step).toPrecision(12))
}

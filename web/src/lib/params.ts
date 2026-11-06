// パラメータ（スライダー）まわりの小さな関数

/** 座標・媒介変数・定数の名前。パラメータには使えない */
export const RESERVED = new Set(['x', 'y', 'z', 't', 'u', 'v', 'e', 'i'])

/** 範囲 range をおよそ n 個に区切る「きりの良い」刻み（1, 2, 5 × 10^k） */
export function niceStep(range: number, n: number): number {
  const raw = range / n
  const mag = 10 ** Math.floor(Math.log10(raw))
  const r = raw / mag
  return (r < 1.5 ? 1 : r < 3 ? 2 : r < 7 ? 5 : 10) * mag
}

/** 刻み step に合わせて丸め、余計な桁を落とした文字列にする（0.30000000000000004 → "0.3"） */
export function formatNumber(v: number, step: number): string {
  const digits = Math.max(0, -Math.floor(Math.log10(step)))
  const r = Math.round(v / step) * step
  return String(Number(r.toFixed(digits)))
}

/** 複素数 re + im·i を、数式入力欄に入れる形（LaTeX）にする。例: 1.5−0.5i → "1.5-0.5i" */
export function complexLatex(re: number, im: number, step: number): string {
  const a = formatNumber(re, step)
  const b = formatNumber(im, step)
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
  const m = error?.match(/変数 '([a-zA-Z])' はここでは使えません/)
  return m && !RESERVED.has(m[1]) ? m[1] : null
}

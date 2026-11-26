// 値の書式（計画 docs/plan-analysis.md の V1 の一部。厳密値の近似値の表示で先に使う）

const SUP: Record<string, string> = { '-': '⁻', '0': '⁰', '1': '¹', '2': '²', '3': '³', '4': '⁴', '5': '⁵', '6': '⁶', '7': '⁷', '8': '⁸', '9': '⁹' }

/** 数 1 つ: 有効 `digits` 桁。末尾の 0 は消す。とても大きい・小さい数は「仮数 × 10ⁿ」。負の符号は − */
export function formatNumber(v: number, digits = 10): string {
  if (Number.isNaN(v)) return '定義されません'
  if (!Number.isFinite(v)) return v > 0 ? '∞' : '−∞'
  if (v === 0) return '0'
  const a = Math.abs(v)
  let s: string
  if (a >= 1e-4 && a < 1e10) {
    s = v.toPrecision(digits)
    if (s.includes('.')) s = s.replace(/0+$/, '').replace(/\.$/, '')
  } else {
    const [m, e] = v.toExponential(digits - 1).split('e')
    const mant = m.includes('.') ? m.replace(/0+$/, '').replace(/\.$/, '') : m
    const exp = String(Number(e)).split('').map((c) => SUP[c] ?? c).join('')
    s = `${mant} × 10${exp}`
  }
  return s.replace('-', '−')
}

/** 複素数: a + bi・a − bi（虚部が 0 なら実数だけ、実部が 0 なら bi） */
export function formatApprox(re: number, im: number, digits = 10): string {
  if (!Number.isFinite(im) || im === 0) return formatNumber(re, digits)
  const b = Math.abs(im)
  const bt = b === 1 ? '' : formatNumber(b, digits)
  if (re === 0) return `${im < 0 ? '−' : ''}${bt}i`
  return `${formatNumber(re, digits)} ${im < 0 ? '−' : '+'} ${bt}i`
}

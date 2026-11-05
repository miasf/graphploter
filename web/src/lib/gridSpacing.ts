// 格子線の間隔を「スライダーの位置」と対応させる。
// 間隔はきりの良い値（1, 2, 2.5, 5 × 10^n）だけにして、線が整数などの区切りの良い位置に来るようにする。

/** 範囲の長さ range に対して選べる間隔の一覧（粗い → 細かい）。線がおよそ 2〜400 本になる範囲。 */
export function spacingOptions(range: number): number[] {
  const out: number[] = []
  const lo = range / 400
  const hi = range / 2
  for (let p = Math.ceil(Math.log10(hi)); p >= Math.floor(Math.log10(lo)) - 1; p--) {
    for (const m of [5, 2.5, 2, 1]) {
      const v = Number((m * 10 ** p).toPrecision(6))
      if (v >= lo && v <= hi) out.push(v)
    }
  }
  return out
}

/** 間隔 → スライダーの位置（0 は線なし、大きいほど細かい） */
export function spacingToIndex(spacing: number, options: number[]): number {
  if (!(spacing > 0) || options.length === 0) return 0
  let best = 0
  for (let k = 1; k < options.length; k++) {
    if (Math.abs(Math.log(options[k] / spacing)) < Math.abs(Math.log(options[best] / spacing))) best = k
  }
  return best + 1
}

/** スライダーの位置 → 間隔 */
export function indexToSpacing(index: number, options: number[]): number {
  return index <= 0 ? 0 : options[Math.min(index, options.length) - 1]
}

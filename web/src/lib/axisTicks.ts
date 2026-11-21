/** 各軸をおよそ10分割し、方眼は 1・2・5 × 10ⁿ の位置にそろえる。 */
export function axisTicks(min: number, max: number): { step: number; grid: number[]; labels: { value: number; edge: boolean }[] } {
  const raw = (max - min) / 10
  const mag = 10 ** Math.floor(Math.log10(raw))
  const norm = raw / mag
  const step = Number.isFinite(norm) ? (norm < 1.5 ? 1 : norm < 3 ? 2 : norm < 7 ? 5 : 10) * mag : max - min
  const grid: number[] = []
  const labels = [{ value: min, edge: true }, { value: max, edge: true }]
  if (!(step > 0) || !Number.isFinite(step)) return { step: 0, grid, labels }

  // 端にほぼ重なる方眼は枠線と重複する。刻みの整数倍を使い、加算誤差もためない。
  const epsilon = step * 1e-8
  const first = Math.ceil(min / step)
  for (let i = 0; i < 24; i++) {
    const value = (first + i) * step
    if (value >= max - epsilon) break
    if (value <= min + epsilon || (grid.length > 0 && value <= grid[grid.length - 1])) continue
    grid.push(value)
    labels.push({ value, edge: false })
  }
  return { step, grid, labels }
}

export function axisTickText(value: number, step: number): string {
  const precision = value && step > 0 ? Math.ceil(Math.log10(Math.abs(value)) - Math.log10(step)) + 3 : 10
  return String(Number(value.toPrecision(Math.min(17, Math.max(10, precision)))))
}

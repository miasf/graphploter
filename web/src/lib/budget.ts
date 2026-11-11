// 再生・ドラッグ中の細かさを、式ごとに測った計算時間から決める。
//
// 計算時間は細かさ n の p 乗に比例するとみなす（t = c · n^p）。p は図形の種類で決まる:
// 曲面（陽関数・媒介変数表示）は 2、陰関数・範囲で重ねた立体は 3、曲線は細かさにほぼよらない。
// 式ごとに、最後の計算の時間から係数 c を覚えておき、細かい計算の時間を見積もる。

/** 計算時間が細かさの何乗に比例するか */
export type Cost = 'surface' | 'volume' | 'curve'

/** 再生・ドラッグ中に使う倍率（細かさに掛ける） */
export const SCALES = [1, 0.75, 0.5, 0.35, 0.25]
/** 再生・ドラッグ中の1回の計算の持ち時間（ミリ秒） */
export const BUDGET_MS = 12
/** 止まっているときの計算がこれより長くなりそうなら、先に粗い版を出す（ミリ秒） */
export const TWO_STAGE_MS = 120
/** 2段階の表示で、先に出す粗い版の計算時間の目安（ミリ秒） */
export const PREVIEW_MS = 40

const power = (cost: Cost) => (cost === 'volume' ? 3 : cost === 'surface' ? 2 : 0)

/** 細かさ: 陽関数・媒介変数表示の細かさ res と、陰関数の細かさ ires（どちらを使うかは図形の種類で決まる） */
export type Sizes = { res: number; ires: number }
const sizeOf = (s: Sizes, cost: Cost) => (cost === 'volume' ? s.ires : s.res)

/** 倍率を SCALES のどれかに切り下げる（0.25 より小さければ 0.25） */
function snapDown(s: number): number {
  return SCALES.find((x) => x <= s + 1e-9) ?? SCALES[SCALES.length - 1]
}

type Entry = {
  /** t = c · n^p の c */
  c: number
  cost: Cost
  /** 再生・ドラッグ中に最後に使った倍率 */
  scale: number
}

export class Budget {
  private entries = new Map<number, Entry>()

  /**
   * 計算が終わったとき。`ms` はかかった時間、`sizes` は使った細かさ。
   * 点を使い回して射影だけした返事（reprojected）は、細かさと関係ないので数えない
   */
  record(id: number, ms: number, sizes: Sizes, cost: Cost, reprojected: boolean) {
    const n = sizeOf(sizes, cost)
    if (reprojected || !(ms >= 0) || !(n > 0)) return
    const p = power(cost)
    const c = ms / n ** p
    const old = this.entries.get(id)
    // 時間は毎回 ±10〜20% ほど揺れるので、ならして覚える（最後の1回だけで決めると、境目の式で倍率が毎回変わる）。
    // 2倍以上変わったとき（式を書き換えたなど）は、ならさずに置き換える
    const smooth = old && old.cost === cost && c < 2 * old.c && 2 * c > old.c
    this.entries.set(id, { c: smooth ? 0.6 * old.c + 0.4 * c : c, cost, scale: old?.scale ?? 1 })
  }

  /** 細かさ sizes で計算したときの時間の見積もり（まだ計算していない式は null） */
  estimate(id: number, sizes: Sizes): number | null {
    const e = this.entries.get(id)
    return e ? e.c * sizeOf(sizes, e.cost) ** power(e.cost) : null
  }

  /**
   * 再生・ドラッグ中の倍率。細かさ n（止まっているとき）での時間 t_full から、
   * s = clamp((B / t_full)^(1/p), 0.25, 1) を SCALES に切り下げる。
   * 上げるのは1段ずつで、1つ上の倍率でも 0.8B に収まるときだけ（毎コマ細かさが変わって格子がちらつかないように）。
   * まだ計算していない式は 0.5
   */
  scaleFor(id: number, sizes: Sizes): number {
    const e = this.entries.get(id)
    if (!e) return 0.5
    const p = power(e.cost)
    if (p === 0) return (e.scale = 1)
    const full = e.c * sizeOf(sizes, e.cost) ** p
    const target = snapDown(Math.min(1, Math.max(0.25, (BUDGET_MS / full) ** (1 / p))))
    if (target < e.scale) {
      e.scale = target
    } else if (target > e.scale) {
      const up = SCALES[SCALES.indexOf(e.scale) - 1]
      if (up !== undefined && full * up ** p <= 0.8 * BUDGET_MS) e.scale = up
    }
    return e.scale
  }

  /**
   * 止まっているときの計算で、先に粗い版を出すなら、その倍率（時間が PREVIEW_MS 前後になるもの）。
   * 細かい計算が TWO_STAGE_MS 以下と見積もった式・まだ計算していない式・曲線は null（1回で計算する）
   */
  previewScale(id: number, sizes: Sizes): number | null {
    const e = this.entries.get(id)
    if (!e) return null
    const p = power(e.cost)
    const full = e.c * sizeOf(sizes, e.cost) ** p
    if (p === 0 || full <= TWO_STAGE_MS) return null
    // 時間が PREVIEW_MS にいちばん近い倍率（比で比べる）
    const off = (x: number) => Math.abs(Math.log((full * x ** p) / PREVIEW_MS))
    const s = SCALES.reduce((best, x) => (off(x) < off(best) ? x : best))
    return s < 1 ? s : null
  }

  forget(id: number) {
    this.entries.delete(id)
  }
}

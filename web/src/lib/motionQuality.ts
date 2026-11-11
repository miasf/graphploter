// 視点を動かしている間の描画の質（画素密度・粗い版）を、1コマの時間から段階的に決める。
// Three.js に依存しない（テストできるように）。Viewer が1コマごとに時間を渡し、今の段階を受け取る。
//
// 60 コマ/秒を保てる間は、止まっているときと同じ絵を出す（段階 0）。遅いときだけ1段ずつ下げ、
// 余裕が続けば1段ずつ戻す。操作が終わったときの段階を覚えておき、次の操作はそこから始める。

/** 1つの段階の描き方 */
export type Stage = {
  /** 粗い版（三角形の少ない版）を描くか */
  lod: boolean
  /** 画素密度 */
  ratio: number
}

/** 場面の三角形の合計がこれ以上なら、「粗い版にする」を段階 1 に繰り上げる（画素より三角形が効くとみなす） */
export const MANY_TRIANGLES = 300_000
/** 粗い版にした後、画素密度をさらに何倍にするか（段階 4〜7） */
const LOW_SCALES = [0.75, 0.55, 0.4, 0.3]
/** 1コマの時間の目安の上限（60 コマ/秒） */
const FRAME_60 = 16.7
/** これより遅ければ1段下げる（目安の何倍か） */
const SLOW = 1.3
/** これより速いコマが RAISE_FRAMES 続けば1段戻してみる（目安の何倍か） */
const FAST = 1.1
const RAISE_FRAMES = 45
/** 段階を変えた後、ならした時間が落ち着くまで判定しないコマの数 */
const SETTLE_FRAMES = 4
/** 画面の更新間隔を見積もるのに使う、直近のコマの数 */
const RECENT = 120

/**
 * 段階の一覧（上ほど質が高い）。基準の画素密度を r として:
 * 0: そのままの版・r / 1: そのままの版・max(1, 0.75r) / 2: そのままの版・min(r, 1) /
 * 3: 粗い版・min(r, 1) / 4〜7: 粗い版・min(r, 1) × (0.75, 0.55, 0.4, 0.3)。
 * 三角形が MANY_TRIANGLES 以上なら、段階 1 から粗い版にする（画素密度の並びは同じ）。
 * 粗い版が無い場面では粗い版の段階は意味がないので、同じ描き方になる段階はまとめる
 * （r ≤ 1 の端末では 0〜2 が同じなど。そうしないと、下げても何も変わらない段階で時間を使う）
 */
export function stageList(base: number, triangles: number, hasLod: boolean): Stage[] {
  const low = Math.min(base, 1)
  const ratios = [base, Math.max(1, 0.75 * base), low, low, ...LOW_SCALES.map((s) => low * s)]
  const lodFrom = triangles >= MANY_TRIANGLES ? 1 : 3
  const list: Stage[] = []
  ratios.forEach((ratio, k) => {
    const s = { lod: hasLod && k >= lodFrom, ratio: Math.min(ratio, base) }
    const prev = list[list.length - 1]
    if (!prev || prev.lod !== s.lod || prev.ratio !== s.ratio) list.push(s)
  })
  return list
}

export class MotionQuality {
  /** 今の段階（stages の番号） */
  level = 0
  private stages: Stage[] = [{ lod: false, ratio: 1 }]
  /** 操作の最中か（begin 〜 end） */
  private active = false
  /** 前の操作が終わったときの段階と、そのときの三角形の合計 */
  private saved = 0
  private savedTriangles = 0
  private triangles = 0
  /** ならした1コマの時間（0.8 / 0.2 の指数平均）と、段階を変えてから数えたコマの数 */
  private avg = 0
  private count = 0
  /** 速いコマが続いた数 */
  private fast = 0
  /** 直前に1段戻したところか / 戻したら遅くなったので、この操作の間はもう戻さない */
  private raised = false
  private raiseFailed = false
  /** 直近のコマの時間（画面の更新間隔の見積もり用） */
  private recent: number[] = []

  /** 今の段階の描き方 */
  get stage(): Stage {
    return this.stages[this.level]
  }

  /** 場面（基準の画素密度・三角形の合計・粗い版があるか）が変わったとき */
  setScene(base: number, triangles: number, hasLod: boolean) {
    this.stages = stageList(base, triangles, hasLod)
    this.triangles = triangles
    this.level = Math.min(this.level, this.stages.length - 1)
  }

  /**
   * 操作（視点の移動・スライダー・再生）を始める。前の操作が終わった段階から始める。
   * 三角形の合計が2倍以上変わっていたら（図形が差し替わった）、覚えた段階を1つ上げてから始める
   */
  begin() {
    if (this.active) return
    this.active = true
    let level = this.saved
    const [a, b] = [this.triangles, this.savedTriangles]
    if (b > 0 && (a >= 2 * b || 2 * a <= b)) level = Math.max(0, level - 1)
    this.level = Math.min(level, this.stages.length - 1)
    this.avg = this.count = this.fast = 0
    this.raised = this.raiseFailed = false
  }

  /** 操作を終える（今の段階を覚えておく） */
  end() {
    if (!this.active) return
    this.active = false
    this.saved = this.level
    this.savedTriangles = this.triangles
  }

  /**
   * 画面の更新間隔 T の見積もり: 直近のコマの時間の速いほうから 20% の中央値（6.9〜16.7 ms）。
   * 数えたコマが少ないうちは 16.7 ms
   */
  interval(): number {
    if (this.recent.length < 10) return FRAME_60
    const sorted = [...this.recent].sort((x, y) => x - y)
    const fastest = sorted.slice(0, Math.max(1, Math.round(sorted.length * 0.2)))
    return Math.min(FRAME_60, Math.max(6.9, fastest[Math.floor(fastest.length / 2)]))
  }

  /**
   * 操作の最中の1コマの時間 dt（ミリ秒）を数える。段階が変わったら true。
   * カメラが動いていないコマ（計算待ちで描いていないだけ）は、呼び出し側で数えない
   */
  frame(dt: number): boolean {
    if (!this.active) return false
    this.recent.push(dt)
    if (this.recent.length > RECENT) this.recent.shift()
    this.avg = this.count === 0 ? dt : this.avg * 0.8 + dt * 0.2
    if (++this.count < SETTLE_FRAMES) return false
    const budget = Math.max(this.interval(), FRAME_60)
    const last = this.stages.length - 1
    if (this.avg > SLOW * budget) {
      this.fast = 0
      if (this.raised) {
        // 戻した直後に遅くなったなら、戻したのが原因。元の段階に下げて、この操作の間はもう戻さない
        this.raised = false
        this.raiseFailed = true
        return this.set(Math.min(this.level + 1, last))
      }
      // とても遅いときは、2段まとめて下げる
      return this.set(Math.min(this.level + (this.avg > 3 * budget ? 2 : 1), last))
    }
    if (this.avg < FAST * budget) {
      if (++this.fast < RAISE_FRAMES) return false
      this.fast = 0
      this.raised = false // 戻した後も速いままなら、戻して正解
      if (this.level > 0 && !this.raiseFailed) {
        this.raised = true
        return this.set(this.level - 1)
      }
      return false
    }
    this.fast = 0
    return false
  }

  private set(level: number): boolean {
    if (level === this.level) return false
    this.level = level
    this.count = 0
    return true
  }
}

// motionQuality.ts のテスト。実行: npm test
import { KEEP_FULL_TRIANGLES, MotionQuality, pickLod, stageList } from './motionQuality.ts'

let failures = 0
function check(name: string, got: unknown, want: unknown) {
  if (JSON.stringify(got) !== JSON.stringify(want)) {
    failures++
    console.log(`NG  ${name}\n    結果: ${JSON.stringify(got)}\n    期待: ${JSON.stringify(want)}`)
  }
}

/** 基準の画素密度 2、三角形 10 万個（粗い版あり）の場面で操作を始めたもの */
function start(): MotionQuality {
  const q = new MotionQuality()
  q.setScene(2, 100_000, true)
  q.begin()
  return q
}
/** dt の1コマを n 回数え、各コマの後の段階を返す（dt が関数なら、今の段階から1コマの時間を決める） */
function run(q: MotionQuality, dt: number | ((level: number) => number), n: number): number[] {
  const levels: number[] = []
  for (let k = 0; k < n; k++) {
    q.frame(typeof dt === 'number' ? dt : dt(q.level))
    levels.push(q.level)
  }
  return levels
}
/** 段階を 1 つ下げるごとに 1 コマの時間が縮む端末（画素密度で重い場面） */
const helps = (level: number) => 60 / (1 + level)

// 段階の一覧
// 重い面の粗い版を、画素密度より先にする
check('基準 2 の段階', stageList(2, true).map((s) => [s.lod, s.ratio]), [
  [false, 2], [true, 2], [true, 1.5], [true, 1], [true, 0.75], [true, 0.55], [true, 0.4], [true, 0.3],
])
check('基準 1 では同じ描き方の段階をまとめる', stageList(1, true).length, 6)
check('粗い版が無ければ画素密度だけ', stageList(1, false).map((s) => s.ratio), [1, 0.75, 0.55, 0.4, 0.3])
check('くっきり: 画素密度は下げない', stageList(2, true, 'sharp').map((s) => [s.lod, s.ratio]), [[false, 2], [true, 2]])
check('常にそのまま: 段階 0 だけ', stageList(2, true, 'full').map((s) => [s.lod, s.ratio]), [[false, 2]])

// (a) 16.7 ms が続けば、段階 0 のまま
{
  const q = start()
  check('(a) 60 コマ/秒なら段階 0', Math.max(...run(q, 16.7, 300)), 0)
}

// (b) 下げるたびに速くなる端末では、8 コマ以内に1段下がり、60 コマ/秒になるまで下がり続ける
{
  const q = start()
  const levels = run(q, helps, 120)
  check('(b) 8 コマ以内に下がる', levels.slice(0, 8).some((l) => l >= 1), true)
  check('(b) 間に合う段階まで下がる', helps(levels[levels.length - 1]) <= 1.3 * 16.7, true)
}

// (b2) 下げても速くならない（40 ms のまま）なら、元に戻して、その操作の間はもう下げない（ぼやけさせても得が無い）
{
  const q = start()
  const levels = run(q, 40, 120)
  check('(b2) 一度は下げてみる', levels.slice(0, 8).some((l) => l >= 1), true)
  check('(b2) 戻して、もう下げない', levels.slice(30).every((l) => l === 0), true)
  // 次の操作では、また試す
  q.end()
  q.begin()
  check('(b2) 次の操作でまた試す', run(q, 40, 8).some((l) => l >= 1), true)
}

// (b3) 画質の設定: くっきりなら画素密度を下げない、常にそのままなら何も下げない
{
  const q = new MotionQuality()
  q.setScene(2, 1_000_000, true)
  q.setMode('sharp')
  q.begin()
  run(q, helps, 120)
  check('(b3) くっきり', q.stage, { lod: true, ratio: 2 })
  q.end()
  q.setMode('full')
  q.begin()
  run(q, helps, 120)
  check('(b3) 常にそのまま', q.stage, { lod: false, ratio: 2 })
}

// (c) 下げた後に 16.7 ms が 45 コマ続くと、1段戻る
// （遅いコマが終わった直後は、ならした時間がまだ遅いので、もう1段下がることがある。下げて効いたかを確かめる間も含めて、落ち着くまで 20 コマ待つ）
function lowered(): MotionQuality {
  const q = start()
  run(q, 40, 6)
  run(q, 16.7, 20)
  return q
}
{
  const q = lowered()
  const low = q.level
  check('(c) 下がっている', low >= 1, true)
  const levels = run(q, 16.7, 45)
  check('(c) 30 コマ目までは戻らない', levels.slice(0, 30).every((l) => l === low), true)
  check('(c) 1段戻る', levels[levels.length - 1], low - 1)
}

// (d) 戻した直後に 40 ms になると元に戻り、その操作の間はもう戻さない
{
  const q = lowered()
  const low = q.level
  run(q, 16.7, 45)
  check('(d) 戻した', q.level, low - 1)
  run(q, 40, 4)
  check('(d) 元の段階に下げる', q.level, low)
  check('(d) もう戻さない', Math.min(...run(q, 16.7, 300)), low)
}

// (e) 次の操作は、前の操作が終わった段階から始まる
{
  const q = start()
  run(q, 40, 6)
  const low = q.level
  q.end()
  q.begin()
  check('(e) 前の段階から始まる', q.level, low)
  // 図形が差し替わって三角形が2倍以上変わったら、1つ上げてから始める
  q.end()
  q.setScene(2, 250_000, true)
  q.begin()
  check('(e) 図形が変わったら1つ上げる', q.level, low - 1)
}

// 粗い版にする面: 重い面だけ（細かいままの合計が KEEP_FULL_TRIANGLES 以下になるまで、多い順に）
check('(f) 1 つだけ重い式', pickLod([1_500_000, 13_000, 40_000], [true, true, false]), [true, false, false])
check('(f) 重い式が 2 つ', pickLod([400_000, 13_000, 300_000], [true, true, true]), [true, false, true])
check('(f) 軽い場面は粗くしない', pickLod([KEEP_FULL_TRIANGLES - 1, 0], [true, true]), [false, false])
check('(f) 粗い版の無い面は飛ばす', pickLod([900_000, 200_000], [false, true]), [false, true])

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('motionQuality.ts: すべて成功')

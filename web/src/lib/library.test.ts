// library.ts のテスト。実行: npm test
import { deleteGraph, duplicateGraph, graphMeta, formatDate, importGraph, isEmptyState, lastGraphId, listGraphs, loadGraph, markNew, migrate, renameGraph, saveGraph, setCurrent, setThumb, useStore, UNTITLED } from './library.ts'
import type { SavedState } from './persist.ts'

const mem = new Map<string, string>()
let full = false
/** このキーへの書き込みだけを失敗させる（容量がいっぱいになった所を真似る） */
let failKey: ((k: string) => boolean) | null = null
useStore({
  getItem: (k) => mem.get(k) ?? null,
  setItem: (k, v) => {
    if (full || failKey?.(k)) throw new Error('QuotaExceededError')
    mem.set(k, v)
  },
  removeItem: (k) => void mem.delete(k),
})

let failures = 0
function check(name: string, ok: boolean, detail = '') {
  if (!ok) {
    failures++
    console.log(`NG  ${name} ${detail}`)
  }
}

const state = (latex: string, title?: string) => ({ version: 1, title, items: [{ latex }], domain: {}, space: {} }) as unknown as SavedState

// 前の版の自動保存は、一覧の最初のグラフになる
mem.set('graphploter:state', JSON.stringify(state('x^2')))
migrate(1000)
check('移行: 一覧に 1 つ', listGraphs().length === 1)
check('移行: 古いキーは消える', !mem.has('graphploter:state'))
const first = listGraphs()[0].id
check('移行: 名前は無題', listGraphs()[0].title === UNTITLED)
check('移行: 最後に開いたグラフ', lastGraphId() === first)
migrate(2000)
check('移行は 1 回だけ', listGraphs().length === 1)

// 保存すると、最後に開いたグラフになり、一覧の先頭に来る
saveGraph('b', state('y', '放物面'), 3000)
check('保存: 一覧に 2 つ', listGraphs().length === 2)
check('保存: 最後に変えたものが先', listGraphs()[0].id === 'b')
check('保存: 名前', listGraphs()[0].title === '放物面')
check('保存: 次に開くのは b', lastGraphId() === 'b')
// 中身が同じなら日時を変えない
saveGraph('b', state('y', '放物面'), 9000)
check('同じ中身では日時を変えない', listGraphs()[0].updated === 3000)

// 開いたグラフを覚える
setCurrent(first)
check('開いたグラフを覚える', lastGraphId() === first)
// 新しいグラフの印
markNew()
check('新しいグラフの印', lastGraphId() === null)

// 名前の変更は、保存した中身にも入る
renameGraph('b', '  鞍点  ')
check('名前の変更', listGraphs().find((m) => m.id === 'b')?.title === '鞍点')
check('名前の変更が中身にも', (loadGraph('b') as SavedState).title === '鞍点')

// 複製
const c = duplicateGraph('b', 4000)
check('複製', c !== null && listGraphs().find((m) => m.id === c)?.title === '鞍点 のコピー')

// 共有リンクの取り込み
const d = importGraph(state('z'), 5000)
check('取り込み: 名前', d !== null && listGraphs().find((m) => m.id === d)?.title === '共有されたグラフ')

// 小さな絵
setThumb('b', 'data:image/jpeg;base64,AAAA')
check('小さな絵', listGraphs().find((m) => m.id === 'b')?.thumb === 'data:image/jpeg;base64,AAAA')

// 削除
setCurrent('b')
deleteGraph('b')
check('削除', !listGraphs().some((m) => m.id === 'b') && loadGraph('b') === null)
check('削除したら次は最後に変えたもの', lastGraphId() === d)

// 容量がいっぱいなら false
full = true
check('容量がいっぱい', saveGraph('e', state('w'), 6000) === false)
full = false

// 一覧と本体は、片方だけが変わらない（#157, #186）
const INDEX = 'graphploter:graphs'
const isBody = (k: string) => k.startsWith('graphploter:graph:')
const snapshot = () => JSON.stringify([...mem.entries()].sort())

// 移行: 一覧が書けなければ、古い保存を消さず、何も残さない。次に開いたときに移行できる
{
  const saved = new Map(mem)
  mem.clear()
  mem.set('graphploter:state', JSON.stringify(state('x^3')))
  failKey = (k) => k === INDEX
  migrate(7000)
  failKey = null
  check('移行の失敗: 古い保存が残る', mem.has('graphploter:state'))
  check('移行の失敗: 本体だけが残らない', ![...mem.keys()].some(isBody), [...mem.keys()].join(','))
  check('移行の失敗: 一覧は空', listGraphs().length === 0)
  migrate(7100)
  check('移行の再試行', listGraphs().length === 1 && !mem.has('graphploter:state') && lastGraphId() === listGraphs()[0].id)
  check('移行の再試行: 中身', JSON.stringify(loadGraph(listGraphs()[0].id)) === JSON.stringify(state('x^3')))
  // 最後に開いたグラフの印だけが書けなくても、移行はする
  mem.clear()
  mem.set('graphploter:state', JSON.stringify(state('x^4')))
  failKey = (k) => k === 'graphploter:current'
  migrate(7200)
  failKey = null
  check('移行: 印が書けなくても一覧の先頭を開く', listGraphs().length === 1 && lastGraphId() === listGraphs()[0].id && !mem.has('graphploter:state'))
  mem.clear()
  for (const [k, v] of saved) mem.set(k, v)
}

// 保存: 一覧が書けなければ、本体も前のまま
{
  const before = snapshot()
  failKey = (k) => k === INDEX
  check('保存の失敗: false', saveGraph(d!, state('z^2', '新しい中身'), 8000) === false)
  check('保存の失敗（新しいグラフ）: false', saveGraph('fresh', state('q'), 8000) === false)
  failKey = null
  check('保存の失敗: 何も変わらない', snapshot() === before)
}

// 名前の変更: 本体が書けなければ、一覧の名前も前のまま（#186）
{
  saveGraph('r', state('y=x', '旧名'), 8100)
  const before = snapshot()
  failKey = isBody
  check('名前の変更の失敗: false', renameGraph('r', '新しい名前') === false)
  failKey = (k) => k === INDEX
  check('名前の変更の失敗（一覧）: false', renameGraph('r', '新しい名前') === false)
  failKey = null
  check('名前の変更の失敗: 一覧も中身も旧名', graphMeta('r')?.title === '旧名' && (loadGraph('r') as SavedState).title === '旧名')
  check('名前の変更の失敗: 何も変わらない', snapshot() === before)
  check('名前の変更: 成功なら true', renameGraph('r', '新しい名前') === true && graphMeta('r')?.title === '新しい名前' && (loadGraph('r') as SavedState).title === '新しい名前')
}

// 削除: 一覧から外せなければ、本体も消さない
{
  const before = snapshot()
  failKey = (k) => k === INDEX
  check('削除の失敗: false', deleteGraph('r') === false)
  failKey = null
  check('削除の失敗: 何も変わらない', snapshot() === before)
  check('削除: 成功なら true', deleteGraph('r') === true && !graphMeta('r') && loadGraph('r') === null)
}

// 空の状態
check('空の状態', isEmptyState(state('  ')) && !isEmptyState(state('x')))

// 日時の表示
const t = new Date(2026, 2, 2, 14, 5).getTime()
check('今日', formatDate(t, t + 1000) === '今日 14:05', formatDate(t, t + 1000))
check('今年', formatDate(t, new Date(2026, 5, 1).getTime()) === '3 月 2 日')
check('去年', formatDate(t, new Date(2027, 5, 1).getTime()) === '2026/3/2')

if (failures > 0) throw new Error(`${failures} 件失敗`)
console.log('library.ts: すべて成功')

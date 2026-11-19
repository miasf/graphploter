// グラフの一覧（Desmos の「マイグラフ」と同じ考え方）。サーバーは使わず、このブラウザの localStorage に保存する。
// - グラフごとに状態を別のキーに保存し、一覧（名前・作った日時・最後に変えた日時・小さな絵）を 1 つのキーにまとめる
// - 最後に開いた（保存した）グラフの id を覚えておき、次にサイトを開いたらそのグラフを開く
// - 前の版の「1 つだけの自動保存」（graphploter:state）は、最初に読むときに一覧の 1 つ目のグラフにする

import type { SavedState } from './persist'

export interface GraphMeta {
  id: string
  title: string
  created: number
  updated: number
  /** 小さな絵（JPEG の data URL）。無ければ無し */
  thumb?: string
}

export const UNTITLED = '無題のグラフ'
const INDEX_KEY = 'graphploter:graphs'
const CURRENT_KEY = 'graphploter:current'
const OLD_KEY = 'graphploter:state'
const graphKey = (id: string) => `graphploter:graph:${id}`

/** 保存の入れ物（テストでは差し替える） */
export interface Store {
  getItem(k: string): string | null
  setItem(k: string, v: string): void
  removeItem(k: string): void
}

let store: Store | null = null
function st(): Store | null {
  if (store) return store
  try {
    return localStorage
  } catch {
    return null
  }
}

/** テスト用: 保存の入れ物を差し替える */
export function useStore(s: Store) {
  store = s
}

function get(k: string): string | null {
  try {
    return st()?.getItem(k) ?? null
  } catch {
    return null
  }
}

/** 書けたか（容量がいっぱい・プライベートモードなどでは false） */
function set(k: string, v: string): boolean {
  try {
    const s = st()
    if (!s) return false
    s.setItem(k, v)
    return true
  } catch {
    return false
  }
}

function remove(k: string) {
  try {
    st()?.removeItem(k)
  } catch {
    // 何もしない
  }
}

function readIndex(): GraphMeta[] {
  try {
    const raw = get(INDEX_KEY)
    const list = raw ? JSON.parse(raw) : []
    if (!Array.isArray(list)) return []
    return list.filter((m) => m && typeof m.id === 'string' && typeof m.title === 'string' && typeof m.updated === 'number')
  } catch {
    return []
  }
}

function writeIndex(list: GraphMeta[]): boolean {
  return set(INDEX_KEY, JSON.stringify(list))
}

/** 新しい id（時刻と乱数。グラフの数は多くないので、これで重ならない） */
export function newId(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 8)
}

/** 前の版の自動保存があれば、一覧の最初のグラフにする（1 回だけ） */
export function migrate(now = Date.now()) {
  const old = get(OLD_KEY)
  if (!old) return
  if (readIndex().length === 0) {
    const id = newId()
    if (set(graphKey(id), old)) {
      writeIndex([{ id, title: UNTITLED, created: now, updated: now }])
      set(CURRENT_KEY, id)
    } else {
      return
    }
  }
  remove(OLD_KEY)
}

/** グラフの一覧（最後に変えたものが先） */
export function listGraphs(): GraphMeta[] {
  return readIndex().sort((a, b) => b.updated - a.updated)
}

export function graphMeta(id: string): GraphMeta | undefined {
  return readIndex().find((m) => m.id === id)
}

/** 「新しいグラフ」を押した後、読み込み直したときに新しいグラフを開くための印 */
const NEW_MARK = '#new'

/** 次に開くグラフの id: 最後に開いたグラフ（一覧に無ければ、最後に変えたグラフ）。何も無い・新しいグラフを開くなら null */
export function lastGraphId(): string | null {
  const list = listGraphs()
  const cur = get(CURRENT_KEY)
  if (cur === NEW_MARK) return null
  if (cur && list.some((m) => m.id === cur)) return cur
  return list[0]?.id ?? null
}

/** 開いているグラフを覚える（次にサイトを開いたら、これを開く） */
export function setCurrent(id: string) {
  set(CURRENT_KEY, id)
}

/** 次に開くときは新しいグラフにする（「新しいグラフ」で読み込み直す前に呼ぶ） */
export function markNew() {
  set(CURRENT_KEY, NEW_MARK)
}

/** 保存した状態（JSON を読んだだけのもの。中身の確かめは呼ぶ側） */
export function loadGraph(id: string): unknown {
  try {
    const raw = get(graphKey(id))
    return raw ? JSON.parse(raw) : null
  } catch {
    return null
  }
}

/** 状態に式が 1 つも無いか（新しいグラフを開いたまま何もしていない） */
export function isEmptyState(s: SavedState): boolean {
  return s.items.every((it) => it.latex.trim() === '')
}

/**
 * グラフを保存する（一覧の名前・日時も更新する）。中身が前と同じなら日時は変えない（開いただけで「最近」にならないように）。
 * 保存できなければ false
 */
export function saveGraph(id: string, state: SavedState, now = Date.now()): boolean {
  const text = JSON.stringify(state)
  const list = readIndex()
  let meta = list.find((m) => m.id === id)
  const title = (state.title ?? '').trim() || UNTITLED
  if (meta && get(graphKey(id)) === text && meta.title === title) return true
  if (!set(graphKey(id), text)) return false
  if (!meta) {
    meta = { id, title, created: now, updated: now }
    list.push(meta)
  }
  meta.title = title
  meta.updated = now
  set(CURRENT_KEY, id)
  return writeIndex(list)
}

/** 小さな絵を付ける（入らなければ付けない） */
export function setThumb(id: string, thumb: string) {
  const list = readIndex()
  const meta = list.find((m) => m.id === id)
  if (!meta || meta.thumb === thumb) return
  const before = meta.thumb
  meta.thumb = thumb
  if (!writeIndex(list)) {
    meta.thumb = before
  }
}

/** 名前を変える（保存した状態の中の名前も） */
export function renameGraph(id: string, title: string) {
  const t = title.trim() || UNTITLED
  const list = readIndex()
  const meta = list.find((m) => m.id === id)
  if (!meta) return
  meta.title = t
  writeIndex(list)
  const s = loadGraph(id) as SavedState | null
  if (s && typeof s === 'object') set(graphKey(id), JSON.stringify({ ...s, title: t }))
}

/** グラフを消す */
export function deleteGraph(id: string) {
  writeIndex(readIndex().filter((m) => m.id !== id))
  remove(graphKey(id))
  if (get(CURRENT_KEY) === id) remove(CURRENT_KEY)
}

/** 複製する（名前は「〜 のコピー」）。新しい id。できなければ null */
export function duplicateGraph(id: string, now = Date.now()): string | null {
  const meta = graphMeta(id)
  const s = loadGraph(id) as SavedState | null
  if (!meta || !s) return null
  const nid = newId()
  const title = `${meta.title} のコピー`
  if (!saveGraph(nid, { ...s, title }, now)) return null
  if (meta.thumb) setThumb(nid, meta.thumb)
  return nid
}

/** 外から来た状態（共有リンク）を、新しいグラフとして一覧に入れる。新しい id。保存できなければ null */
export function importGraph(state: SavedState, now = Date.now()): string | null {
  const id = newId()
  return saveGraph(id, { ...state, title: (state.title ?? '').trim() || '共有されたグラフ' }, now) ? id : null
}

/** 日時を「今日 14:05」「3 月 2 日」「2025/3/2」の形で */
export function formatDate(t: number, now = Date.now()): string {
  const d = new Date(t)
  const n = new Date(now)
  const pad = (v: number) => String(v).padStart(2, '0')
  if (d.toDateString() === n.toDateString()) return `今日 ${d.getHours()}:${pad(d.getMinutes())}`
  if (d.getFullYear() === n.getFullYear()) return `${d.getMonth() + 1} 月 ${d.getDate()} 日`
  return `${d.getFullYear()}/${d.getMonth() + 1}/${d.getDate()}`
}

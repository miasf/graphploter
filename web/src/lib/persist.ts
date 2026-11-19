// 式と設定の保存・共有。
// - ブラウザ（localStorage）のグラフの一覧（lib/library.ts）に自動で保存し、次に開いたときは最後に開いたグラフに戻す
// - 「共有リンク」: 状態を圧縮して URL の # の後ろに入れる（サーバーは使わない）。開くと、一覧に新しいグラフとして入る

import { importGraph, lastGraphId, loadGraph, migrate, newId, setCurrent } from './library'
import type { ComplexMode, CoordSystem, CutStyle, Domain, Folder, GridStyle, HiddenStyle, ParamRanges, RotationGroup, SliderStyle, Space } from './types'

/** 保存する式1つ分（計算結果などは保存しない） */
export interface SavedItem {
  latex: string
  color: string
  visible: boolean
  opacity: number
  grid: GridStyle
  lineWidth: number
  ranges: ParamRanges
  vertical: boolean
  slider: SliderStyle
  complexMode?: ComplexMode
  /** 表示していない座標の見せ方（古い保存データには無い。無ければ断面） */
  hidden?: HiddenStyle
  /** 座標を複素数にするか（古い保存データには無い） */
  complexCoords?: boolean
  /** この式だけの断面の値（古い保存データには無い） */
  slice?: Record<string, number>
  walls?: boolean
  /** ベクトル場の矢印の設定（古い保存データには無い） */
  field?: { count?: number; unit?: boolean }
  /** 回転のグループの番号（無ければこの式だけの回転。ただし rotOwn の無い古い保存データでは全体の回転） */
  rotGroup?: number
  /** この式だけの回転角と中心（古い保存データには無い） */
  rotation?: number[]
  center?: number[]
  /** 媒介変数が4つ以上の図形の、面を描く所の分割数（古い保存データには無い） */
  layers?: number
  /** 媒介変数が3つ以上の図形を辺だけで描くか（古い保存データには無い。無ければ面） */
  edges?: boolean
  /** 描画の方法（古い保存データには無い。無ければ自動） */
  render?: 'auto' | 'mesh' | 'gpu'
  /** 入っているフォルダの番号（古い保存データには無い） */
  folder?: number
  /** 座標系の番号（古い保存データには無い。無ければ直交座標） */
  coordSys?: number
  /** 自動の座標系（極座標など）の角度の範囲（書き換えたものだけ） */
  angleRanges?: Record<string, [string, string]>
}

/** 保存する状態全体 */
export interface SavedState {
  version: 1
  /** グラフの名前（古い保存データには無い。無ければ「無題のグラフ」） */
  title?: string
  items: SavedItem[]
  domain: Domain
  resolution: number
  implicitResolution: number
  /** 陰関数で、格子より小さい図形を念入りに探すか（古い保存データには無い。無ければ探さない） */
  findSmall?: boolean
  /** 3次元の遠近感（0〜100） */
  perspective: number
  dark: boolean
  space: Space
  /** 立体の切り口（古い保存データには無い） */
  cut?: CutStyle
  /** 2D 表示（古い保存データには無い） */
  flat?: boolean
  /** 回転のグループ（古い保存データには無い） */
  groups?: RotationGroup[]
  /** 座標系（古い保存データには無い） */
  coordSystems?: CoordSystem[]
  /** 2D 表示を切り口で描くか（古い保存データには無い。無ければ投影） */
  flatSlice?: boolean
  /** フォルダ（古い保存データには無い） */
  folders?: Folder[]
  /** 名前で書いた積分の範囲（∬_D の D → 範囲の LaTeX。古い保存データには無い） */
  regions?: Record<string, string>
  /** グラフに描いている積分の範囲の名前（古い保存データには無い） */
  regionsShown?: string[]
  /** 積分を描いている式の番号（items の中の位置。古い保存データには無い） */
  integralsShown?: number[]
  /** マウスの操作モード（古い保存データには無い） */
  controlMode?: 'A' | 'B'
  /** 式の回転の標準が「この式だけ」になった後の保存データか（古いものは、グループに入っていない式が全体の回転） */
  rotOwn?: boolean
}

const HASH_PREFIX = '#s='

/** バイト列 → URL に入れられる base64（+ / = を使わない形） */
function toBase64Url(bytes: Uint8Array): string {
  let s = ''
  for (const b of bytes) s += String.fromCharCode(b)
  return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '')
}

function fromBase64Url(text: string): Uint8Array {
  const s = atob(text.replace(/-/g, '+').replace(/_/g, '/'))
  return Uint8Array.from(s, (c) => c.charCodeAt(0))
}

/** 文字列を圧縮する（ブラウザ標準の deflate） */
async function compress(text: string): Promise<Uint8Array> {
  const stream = new Blob([text]).stream().pipeThrough(new CompressionStream('deflate-raw'))
  return new Uint8Array(await new Response(stream).arrayBuffer())
}

async function decompress(bytes: Uint8Array): Promise<string> {
  const stream = new Blob([bytes as BlobPart]).stream().pipeThrough(new DecompressionStream('deflate-raw'))
  return await new Response(stream).text()
}

export function isState(x: unknown): x is SavedState {
  const s = x as SavedState
  const ok = !!s && typeof s === 'object' && s.version === 1 && Array.isArray(s.items) && validDomain(s.domain) && !!s.space && typeof s.space === 'object'
  if (!ok) return false
  // 壊れた・別の版の保存データ（共有リンクは外から来る）でも、画面が壊れたままにならないようにする:
  // LaTeX の無い式は捨て、null の項目は「無い」にする（null のままだと、式の既定の設定を上書きしてしまう）
  s.items = s.items
    .filter((it) => !!it && typeof it === 'object' && typeof it.latex === 'string')
    .map((it) => Object.fromEntries(Object.entries(it).filter(([, v]) => v !== null)) as unknown as SavedItem)
  // 古い保存データでは、座標の範囲が数値 [最小, 最大] だった。式（文字列）にそろえる
  if (Array.isArray(s.space.ranges)) {
    s.space.ranges = s.space.ranges.map((r) => (Array.isArray(r) ? [String(r[0]), String(r[1])] : null))
  }
  return true
}

/** 表示範囲が数値で、最小 < 最大か */
function validDomain(d: unknown): d is Domain {
  if (!d || typeof d !== 'object') return false
  const v = d as Record<string, unknown>
  const num = (k: string) => typeof v[k] === 'number' && Number.isFinite(v[k])
  return ['x', 'y', 'z'].every((a) => num(a + 'min') && num(a + 'max') && (v[a + 'min'] as number) < (v[a + 'max'] as number))
}

/** 共有リンク（今のページの URL の # の後ろに状態を入れたもの）を作る */
export async function shareUrl(state: SavedState): Promise<string> {
  const data = toBase64Url(await compress(JSON.stringify(state)))
  return `${location.origin}${location.pathname}${HASH_PREFIX}${data}`
}

/** 最初に開くグラフ: id と状態（新しいグラフなら状態は null）。`shared` は共有リンクから開いたか */
export interface Initial {
  id: string
  state: SavedState | null
  shared: boolean
}

/**
 * 最初に読み込むグラフ。共有リンクで開いたなら、その状態を新しいグラフとして一覧に入れて開く。
 * そうでなければ、最後に開いたグラフ。どれも無い（または壊れている）なら、新しいグラフ（状態は null）
 */
export async function loadInitial(): Promise<Initial> {
  migrate()
  if (location.hash.startsWith(HASH_PREFIX)) {
    try {
      const s = JSON.parse(await decompress(fromBase64Url(location.hash.slice(HASH_PREFIX.length))))
      // 共有リンクの状態は一度読み込んだら URL から消す（以後は一覧のグラフとして保存を続ける）
      history.replaceState(null, '', location.pathname + location.search)
      if (isState(s)) {
        const title = (s.title ?? '').trim() || '共有されたグラフ'
        const id = importGraph({ ...s, title }) ?? newId()
        setCurrent(id)
        return { id, state: { ...s, title }, shared: true }
      }
    } catch {
      // 壊れたリンクは無視する
    }
  }
  const id = lastGraphId()
  if (id) {
    const s = loadGraph(id)
    if (isState(s)) return { id, state: s, shared: false }
  }
  return { id: newId(), state: null, shared: false }
}

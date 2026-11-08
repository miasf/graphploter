// 式と設定の保存・共有。
// - ブラウザ（localStorage）に自動で保存し、次に開いたときに元に戻す
// - 「共有リンク」: 状態を圧縮して URL の # の後ろに入れる（サーバーは使わない）

import type { ComplexMode, CutStyle, Domain, GridStyle, ParamRanges, SliderStyle, Space } from './types'

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
  walls?: boolean
}

/** 保存する状態全体 */
export interface SavedState {
  version: 1
  items: SavedItem[]
  domain: Domain
  resolution: number
  implicitResolution: number
  /** 3次元の遠近感（0〜100） */
  perspective: number
  dark: boolean
  space: Space
  /** 立体の切り口（古い保存データには無い） */
  cut?: CutStyle
  /** 2D 表示（古い保存データには無い） */
  flat?: boolean
}

const STORAGE_KEY = 'graphploter:state'
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

function isState(x: unknown): x is SavedState {
  const s = x as SavedState
  return !!s && s.version === 1 && Array.isArray(s.items) && !!s.domain && !!s.space
}

/** ブラウザに保存する */
export function saveLocal(state: SavedState) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state))
  } catch {
    // 保存できない環境（プライベートモードなど）では何もしない
  }
}

/** ブラウザに保存した状態を消す */
export function clearLocal() {
  try {
    localStorage.removeItem(STORAGE_KEY)
  } catch {
    // 何もしない
  }
}

/** 共有リンク（今のページの URL の # の後ろに状態を入れたもの）を作る */
export async function shareUrl(state: SavedState): Promise<string> {
  const data = toBase64Url(await compress(JSON.stringify(state)))
  return `${location.origin}${location.pathname}${HASH_PREFIX}${data}`
}

/**
 * 最初に読み込む状態。共有リンクで開いたならその状態、そうでなければブラウザに保存した状態。
 * どちらも無い（または壊れている）なら null
 */
export async function loadInitial(): Promise<SavedState | null> {
  if (location.hash.startsWith(HASH_PREFIX)) {
    try {
      const s = JSON.parse(await decompress(fromBase64Url(location.hash.slice(HASH_PREFIX.length))))
      // 共有リンクの状態は一度読み込んだら URL から消す（以後はブラウザへの保存で続ける）
      history.replaceState(null, '', location.pathname + location.search)
      if (isState(s)) return s
    } catch {
      // 壊れたリンクは無視する
    }
  }
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const s = JSON.parse(raw)
      if (isState(s)) return s
    }
  } catch {
    // 何もしない
  }
  return null
}

// 厳密値の Worker の窓口（計画 docs/plan-exact-core.md の EX5）。
// - Worker は 1 つ。依頼は 1 つずつ。同じ行の新しい依頼が来たら、まだ始まっていない古い依頼は捨てる（null で返す）
// - 8 秒で終わらなければ Worker を作り直し、その依頼は「時間切れ」にする（エンジンの中の予算で止まらないときの最後の手段）
// - 結果を覚える（同じ式・同じ定義・同じスライダーの値なら計算し直さない。200 個まで）

import ExactWorker from './exact.worker.ts?worker'
import type { ExactJob, ExactReply, ExactState } from './types'

/** エンジンの中の仕事の量の上限（ふつうの PC の WASM で約 2 秒。計画の D7。0 ならエンジンの既定） */
const BUDGET = 0
const HARD_TIMEOUT_MS = 8000
const CACHE_SIZE = 200

interface Pending {
  itemId: number
  key: string
  job: ExactJob
  resolve: (s: ExactState | null) => void
}

let worker: Worker | null = null
let nextId = 1
let running: (Pending & { id: number; timer: ReturnType<typeof setTimeout> }) | null = null
const queue: Pending[] = []
const cache = new Map<string, ExactState>()

function ensureWorker(): Worker {
  if (!worker) {
    worker = new ExactWorker()
    worker.onmessage = (ev: MessageEvent<ExactReply>) => finish(ev.data)
  }
  return worker
}

function toState(r: ExactReply): ExactState {
  const approx: [number, number] = r.approx.length === 2 ? [r.approx[0], r.approx[1]] : [NaN, NaN]
  switch (r.status) {
    case 1:
      return { kind: 'found', latex: r.latex, inputLatex: r.inputLatex, engine: r.engine, approx, digits: r.digits, addable: /[a-z]/.test(r.engine.replace(/\b(pi|eulergamma|catalan|sqrt|ln|log|sin|cos|tan|arcsin|arccos|arctan|sinh|cosh|tanh|exp|gamma|zeta|digamma|abs|e|i)\b/g, '')) }
    case 2:
      return { kind: 'guess', latex: r.latex, inputLatex: r.inputLatex, engine: r.engine, approx }
    case 3:
      return { kind: 'notExist', reason: r.reason }
    default:
      return { kind: 'notFound', reason: r.reason }
  }
}

function finish(r: ExactReply) {
  if (!running || running.id !== r.id) return
  const cur = running
  clearTimeout(cur.timer)
  running = null
  const state = toState(r)
  remember(cur.key, state)
  // 走っている間に同じ行の新しい依頼が来ていたら、この結果は古い
  cur.resolve(queue.some((q) => q.itemId === cur.itemId) ? null : state)
  pump()
}

function remember(key: string, state: ExactState) {
  cache.delete(key)
  cache.set(key, state)
  while (cache.size > CACHE_SIZE) cache.delete(cache.keys().next().value as string)
}

function pump() {
  if (running || queue.length === 0) return
  const p = queue.shift()!
  const id = nextId++
  const timer = setTimeout(() => {
    // 止まらない: Worker ごと作り直す
    worker?.terminate()
    worker = null
    finish({ id, status: 0, latex: '', inputLatex: '', engine: '', approx: [], digits: '', reason: '時間切れ', work: 0 })
  }, HARD_TIMEOUT_MS)
  running = { ...p, id, timer }
  ensureWorker().postMessage({ id, job: p.job, budget: BUDGET })
}

/** 行の厳密値を頼む。同じ行の新しい依頼で置き換えられたら null */
export function requestExact(itemId: number, job: ExactJob): Promise<ExactState | null> {
  const key = JSON.stringify([job.source, job.defs, job.names, job.values])
  const hit = cache.get(key)
  if (hit) return Promise.resolve(hit)
  // まだ始まっていない同じ行の依頼は捨てる
  for (let k = queue.length - 1; k >= 0; k--) {
    if (queue[k].itemId === itemId) {
      queue[k].resolve(null)
      queue.splice(k, 1)
    }
  }
  return new Promise((resolve) => {
    queue.push({ itemId, key, job, resolve })
    pump()
  })
}

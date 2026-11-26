// 厳密値の Worker（計画 docs/plan-exact-core.md の EX5）。描画の Worker とは別に、同じ WASM を読み込んで厳密値だけを計算する。
// 描画を止めないように別にしてある。止まらない計算は、窓口（exactClient.ts）がこの Worker ごと作り直して止める

import init, { exact_eval } from '../wasm-core/graph_core.js'
import type { ExactJob, ExactReply } from './types'

const ready = init()

self.onmessage = async (ev: MessageEvent<{ id: number; job: ExactJob; budget: number }>) => {
  await ready
  const { id, job, budget } = ev.data
  let reply: ExactReply
  try {
    const r = exact_eval(job.source, job.defs, job.names.join(','), new Float64Array(job.values), budget)
    reply = {
      id,
      status: r.status(),
      latex: r.latex(),
      inputLatex: r.input_latex(),
      engine: r.engine(),
      approx: Array.from(r.approx()),
      digits: r.digits(),
      reason: r.reason(),
      work: r.work(),
    }
    r.free()
  } catch (e) {
    reply = { id, status: 0, latex: '', inputLatex: '', engine: '', approx: [], digits: '', reason: e instanceof Error ? e.message : String(e), work: 0 }
  }
  ;(self as unknown as Worker).postMessage(reply)
}

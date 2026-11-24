// 計画 12 章の G4: 2D 表示の画素ごとの描き方の画像の検査。計算エンジンが作った GLSL（cargo run --example gpu2d_fixture）と、
// 画面の材質の判定と塗り（web/src/lib/gpu2dMaterial.ts の GPU2D_FRAGMENT）を、ヘッドレスの Chromium の WebGL2 で描いて調べる。
//
//   cd web && npm run test:gpu      （G3 の答え合わせの後に回る）
//
// 表示範囲 [−5, 5]²、400 × 400 画素（1 単位 40 画素）、線の太さ 3 画素。確かめること:
// - 1/x = y: 極（x = 0）に縦の線が出ない。双曲線は出る
// - floor(x) = y: 段差の縦の線が出ない。段は出る
// - (x² + y² − 1)² = 0: 符号の変わらない零点の円が出る（太さはふつうの線と同じくらい）
// - sin(1/x) = y: 落ちずに描ける。原点から離れた所の曲線は出る
// - 不等式の領域: 中は塗り、外は塗らない。≤ の境目は実線、< の境目は描かない（点線はメッシュの線のまま）
import { execFileSync } from 'child_process'
import path from 'path'
import { fileURLToPath } from 'url'
import { createRequire } from 'module'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '../..')
const require = createRequire(path.join(root, 'web/package.json'))
const { chromium } = require('playwright')
const { GPU2D_FRAGMENT } = await import(path.join(root, 'web/src/lib/gpu2dMaterial.ts'))

const json = execFileSync('cargo', ['run', '--release', '-q', '--example', 'gpu2d_fixture'], { cwd: path.join(root, 'core'), encoding: 'utf8' })
const cases = JSON.parse(json)

const N = 400
const DOM = [-5, -5, 5, 5]

/** ページの中で 1 つの図形を描き、画素ごとの不透明度（0〜255、下の行から）と時間を返す */
function render({ glsl, frag, n, dom, width, affine }) {
  const canvas = document.createElement('canvas')
  canvas.width = n
  canvas.height = n
  const gl = canvas.getContext('webgl2', { premultipliedAlpha: false, antialias: false })
  const vs = `#version 300 es
in vec2 aPos;
void main() { gl_Position = vec4(aPos, 0.0, 1.0); }`
  const fs = `#version 300 es
precision highp float; precision highp int;
layout(location = 0) out highp vec4 pc_fragColor;
#define gl_FragColor pc_fragColor
${glsl}
${frag.replace('#include <colorspace_fragment>', '')}`
  const sh = (type, src) => {
    const s = gl.createShader(type)
    gl.shaderSource(s, src)
    gl.compileShader(s)
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s))
    return s
  }
  const prog = gl.createProgram()
  try {
    gl.attachShader(prog, sh(gl.VERTEX_SHADER, vs))
    gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, fs))
  } catch (e) {
    return { error: String(e.message || e) }
  }
  gl.linkProgram(prog)
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return { error: gl.getProgramInfoLog(prog) }
  gl.useProgram(prog)
  const u = (name) => gl.getUniformLocation(prog, name)
  // 画素 → 座標: 画面の中央の画素が表示範囲の中心
  gl.uniform2f(u('uQ0'), (dom[0] + dom[2]) / 2, (dom[1] + dom[3]) / 2)
  gl.uniform2f(u('uC'), n / 2, n / 2)
  gl.uniform2f(u('uDx'), (dom[2] - dom[0]) / n, 0)
  gl.uniform2f(u('uDy'), 0, (dom[3] - dom[1]) / n)
  gl.uniform1i(u('uIters'), 400)
  // 1 次式で受け取る形: 式の座標は画素の座標そのまま（P = (q.x, q.y, 0)）
  if (affine) {
    gl.uniform3fv(u('uPA'), [1, 0, 0, 0, 1, 0, 0, 0, 1])
    gl.uniform1fv(u('uPb'), [0, 0, 0])
  }
  gl.uniform3f(u('uColor'), 1, 1, 1)
  gl.uniform1f(u('uWidth'), width)
  gl.uniform1f(u('uRatio'), 1)
  gl.uniform1f(u('uFill'), 0.5)
  gl.uniform2f(u('uGridSpacing'), 0, 0)
  gl.uniform1f(u('uGridWidth'), 1)
  const buf = gl.createBuffer()
  gl.bindBuffer(gl.ARRAY_BUFFER, buf)
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW)
  const loc = gl.getAttribLocation(prog, 'aPos')
  gl.enableVertexAttribArray(loc)
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0)
  gl.viewport(0, 0, n, n)
  gl.clearColor(0, 0, 0, 0)
  const px = new Uint8Array(n * n * 4)
  const t0 = performance.now()
  gl.clear(gl.COLOR_BUFFER_BIT)
  gl.drawArrays(gl.TRIANGLES, 0, 3)
  gl.readPixels(0, 0, n, n, gl.RGBA, gl.UNSIGNED_BYTE, px)
  const ms = performance.now() - t0
  const alpha = []
  for (let i = 0; i < n * n; i++) alpha.push(px[i * 4 + 3])
  return { alpha, ms }
}

/** 座標 (x, y) の画素の不透明度（0〜1） */
const at = (alpha, x, y) => {
  const i = Math.round(((x - DOM[0]) / (DOM[2] - DOM[0])) * N - 0.5)
  const j = Math.round(((y - DOM[1]) / (DOM[3] - DOM[1])) * N - 0.5)
  if (i < 0 || j < 0 || i >= N || j >= N) return 0
  return alpha[j * N + i] / 255
}

/** 長方形 [x0, x1] × [y0, y1] の中で、塗った（不透明度 0.5 以上の）画素の数 */
const painted = (alpha, x0, x1, y0, y1) => {
  let c = 0
  for (let j = 0; j < N; j++) {
    const y = DOM[1] + ((j + 0.5) / N) * (DOM[3] - DOM[1])
    if (y < y0 || y > y1) continue
    for (let i = 0; i < N; i++) {
      const x = DOM[0] + ((i + 0.5) / N) * (DOM[2] - DOM[0])
      if (x >= x0 && x <= x1 && alpha[j * N + i] >= 128) c++
    }
  }
  return c
}

/** 曲線が通る所の近く（±2 画素）で、いちばん濃い画素 */
const near = (alpha, x, y) => {
  let m = 0
  const h = (DOM[2] - DOM[0]) / N
  for (let dx = -2; dx <= 2; dx++) for (let dy = -2; dy <= 2; dy++) m = Math.max(m, at(alpha, x + dx * h, y + dy * h))
  return m
}

/** 式ごとの確かめ。[説明, 合格か] の列 */
const checks = {
  '1/x = y': (a) => [
    ['極 x = 0 の列（|x| < 0.05）に線が無い', painted(a, -0.05, 0.05, -4.5, 4.5) === 0],
    ['双曲線 (2, 0.5)・(−0.5, −2) が出る', near(a, 2, 0.5) > 0.5 && near(a, -0.5, -2) > 0.5],
  ],
  'floor(x) = y': (a) => {
    let wall = 0
    for (let k = -4; k <= 4; k++) wall += painted(a, k - 0.03, k + 0.03, k - 0.85, k - 0.15)
    return [
      ['段差の縦の線（x = k、k − 0.85 < y < k − 0.15）が無い', wall === 0],
      ['段 (k + 0.5, k) が出る', [-3, -1, 0, 2, 4].every((k) => near(a, k + 0.5, k) > 0.5)],
    ]
  },
  '(x^2+y^2-1)^2 = 0': (a) => {
    const on = [0, 1, 2, 3, 4, 5].every((k) => near(a, Math.cos(k), Math.sin(k)) > 0.5)
    // 太さ: 線の太さ 3 画素 + 1 画素くらいまで（半径 1 ± 0.1 の外は塗らない）
    let thick = 0
    for (let k = 0; k < 6; k++) thick += at(a, 0.88 * Math.cos(k), 0.88 * Math.sin(k)) + at(a, 1.12 * Math.cos(k), 1.12 * Math.sin(k))
    return [
      ['円が出る', on],
      ['太すぎない（半径 0.88・1.12 は塗らない）', thick === 0],
    ]
  },
  'sin(1/x) = y': (a) => [['原点から離れた所 (3, sin(1/3))・(−2, sin(−1/2)) が出る', near(a, 3, Math.sin(1 / 3)) > 0.5 && near(a, -2, Math.sin(-0.5)) > 0.5]],
  // 定数倍しても零点の集合（単位円）は同じ（#169。10⁻²⁰ 倍で画面全体を塗っていた）
  '10^(-20)*(x^2+y^2-1) = 0': (a) => [
    ['円が出る', [0, 1, 2, 3, 4, 5].every((k) => near(a, Math.cos(k), Math.sin(k)) > 0.5)],
    ['中心と画面の端は塗らない', at(a, 0, 0) === 0 && at(a, 4.5, 4.5) === 0 && painted(a, -5, 5, -5, 5) < 3000],
  ],
  '10^20*(x^2+y^2-1) = 0': (a) => [
    ['円が出る', [0, 1, 2, 3, 4, 5].every((k) => near(a, Math.cos(k), Math.sin(k)) > 0.5)],
    ['中心と画面の端は塗らない', at(a, 0, 0) === 0 && at(a, 4.5, 4.5) === 0 && painted(a, -5, 5, -5, 5) < 3000],
  ],
  'x^2+y^2 = 4': (a) => [
    ['円が出る', [0, 1, 2, 3, 4, 5].every((k) => near(a, 2 * Math.cos(k), 2 * Math.sin(k)) > 0.5)],
    ['中と外は塗らない', at(a, 0, 0) === 0 && at(a, 3, 3) === 0],
  ],
  'x^2+y^2 <= 4': (a) => [
    ['中を塗る', at(a, 0, 0) > 0.3 && at(a, 1, 1) > 0.3],
    ['外は塗らない', at(a, 3, 3) === 0 && at(a, -2.5, 0) === 0],
    ['≤ の境目は実線', near(a, 2, 0) > 0.9],
  ],
  'x^2+y^2 <= 9, y > x': (a) => [
    ['両方を満たす所 (−1, 1) を塗る', at(a, -1, 1) > 0.3],
    ['片方だけの所 (1, −1)・(−4, 4) は塗らない', at(a, 1, -1) === 0 && at(a, -4, 4) === 0],
    ['≤ の境目（y > x の側）は実線', near(a, -3, 0) > 0.9],
    ['< の境目 y = x は GPU では引かない（中の塗りと同じ濃さ）', at(a, -1, -1) < 0.6],
  ],
  // 空集合は、境目の線も塗りも出さない（#139。x = 0 の 1 列の 800 画素を塗っていた）
  'x > 0, x <= 0': (a) => [['何も塗らない', painted(a, -5, 5, -5, 5) === 0]],
  'x^2+y^2 < 0': (a) => [['何も塗らない', painted(a, -5, 5, -5, 5) === 0]],
  'y > x^2, y < x^2': (a) => [['何も塗らない', painted(a, -5, 5, -5, 5) === 0]],
  'x > 0.3, x <= 0.3': (a) => [['何も塗らない', painted(a, -5, 5, -5, 5) === 0]],
  // 対照: ≤ どうしは直線 x = 0
  'x >= 0, x <= 0': (a) => [
    ['直線 x = 0 が出る', [-3, -1, 0, 2, 4].every((y) => near(a, 0, y) > 0.5)],
    ['離れた所は塗らない', at(a, 1, 0) === 0 && at(a, -1, 0) === 0],
  ],
  'x^2+y^2 = 9, y >= 0': (a) => [
    ['y ≥ 0 の側の円が出る', near(a, 0, 3) > 0.5 && near(a, 3 * Math.cos(1), 3 * Math.sin(1)) > 0.5],
    ['y < 0 の側は出ない', near(a, 0, -3) === 0],
  ],
}

const browser = await chromium.launch()
const page = await browser.newPage()
let failed = 0
console.log('| 式 | 確かめたこと | 結果 | 時間 ms（SwiftShader、参考） |')
console.log('|---|---|---|---|')
for (const c of cases) {
  const r = await page.evaluate(render, { glsl: c.glsl, frag: GPU2D_FRAGMENT, n: N, dom: DOM, width: 3, affine: c.affine })
  if (r.error) {
    console.log(`| \`${c.src}\` | コンパイル | ✗ ${r.error.split('\n')[0]} | |`)
    failed++
    continue
  }
  for (const [what, ok] of (checks[c.src.replace('（1 次式）', '')] ?? (() => []))(r.alpha)) {
    if (!ok) failed++
    console.log(`| \`${c.src}\` | ${what} | ${ok ? '○' : '✗'} | ${r.ms.toFixed(0)} |`)
  }
}
await browser.close()
console.log(failed ? `\n${failed} 項目が失敗` : '\n全部の項目で合格')
process.exit(failed ? 1 : 0)

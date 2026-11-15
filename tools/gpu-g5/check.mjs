// 計画 12 章の G5: 3D の陰関数を光線で描く GLSL（web/src/lib/raycastGlsl.ts の RAY_GLSL。材質は raycastMaterial.ts）の検査。
//
//   cd web && npm run test:gpu      （G3・G4 の検査の後に回る）
//   node --experimental-strip-types tools/gpu-g5/check.mjs [--speed]
//
// 1. G2 の物差し（core/tests/pixel.rs の pixel_report）と同じカメラで 160 × 120 本の光線を GPU で飛ばし、
//    計算エンジンの基準の実装（ray_hit、上限 512 回・1 画素。cargo run --example ray_fixture）と比べる:
//    当たる・当たらないが違う画素の割合（基準が打ち切った画素は数えない）、当たった深さの差の 95 パーセンタイル（画素の大きさの何倍か）、
//    GPU が打ち切った画素の割合。完了の条件（計画 G5）: 違う画素 1 % 以下
// 2. --speed を付けると、G0 の 6 式を 1280 × 720・繰り返しの上限 256 で描く GPU の時間（timer query があれば）も測る
import { execFileSync } from 'child_process'
import path from 'path'
import { fileURLToPath } from 'url'
import { createRequire } from 'module'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = process.env.GRAPHPLOTER_ROOT ?? path.resolve(here, '../..')
const require = createRequire(process.env.PLAYWRIGHT_FROM ?? path.join(root, 'web/package.json'))
const { chromium } = require(process.env.PLAYWRIGHT_MODULE ?? 'playwright')
const { RAY_GLSL } = await import(new URL('file:///' + path.join(root, 'web/src/lib/raycastGlsl.ts').replace(/\\/g, '/')).href)

const cargo = process.env.CARGO ? process.env.CARGO.split(' ') : ['cargo']
const json = execFileSync(cargo[0], [...cargo.slice(1), 'run', '--release', '-q', '--example', 'ray_fixture'], { cwd: path.join(root, 'core'), encoding: 'utf8', maxBuffer: 1 << 28 })
const fx = JSON.parse(json)
const speed = process.argv.includes('--speed')

/** ページの中で: 1 つの式の光線を全部の画素で飛ばし、(t, 結果) を返す。timing なら 1280 × 720 で描く時間 */
function run({ glsl, ray, cam, w, h, maxIter, timing }) {
  const canvas = document.createElement('canvas')
  canvas.width = w
  canvas.height = h
  const gl = canvas.getContext('webgl2', { antialias: false })
  if (!gl.getExtension('EXT_color_buffer_float')) return { error: 'EXT_color_buffer_float が無い' }
  const vs = `#version 300 es
in vec2 aPos; void main() { gl_Position = vec4(aPos, 0.0, 1.0); }`
  const fs = `#version 300 es
precision highp float; precision highp int;
#define varying in
out vec4 oColor;
#define G_N 3
uniform vec3 uEye; uniform vec3 uFwd; uniform vec3 uRight; uniform vec3 uUp; uniform float uTanHalf; uniform vec2 uSize;
${glsl}
${ray}
void main() {
  vec2 ij = vec2(floor(gl_FragCoord.x), uSize.y - 1.0 - floor(gl_FragCoord.y));
  float aspect = uSize.x / uSize.y;
  float x = ((ij.x + 0.5) / uSize.x * 2.0 - 1.0) * uTanHalf * aspect;
  float y = (1.0 - (ij.y + 0.5) / uSize.y * 2.0) * uTanHalf;
  vec3 d = normalize(uFwd + x * uRight + y * uUp);
  vec2 tr = gRayBox(uEye, d);
  if (!(tr.x < tr.y)) { oColor = vec4(0.0, 0.0, 0.0, 1.0); return; }
  float t;
  int res = gRay(uEye, d, tr.x, tr.y, t);
  oColor = vec4(t, float(res), 0.0, 1.0);
}`
  const sh = (type, src) => {
    const s = gl.createShader(type)
    gl.shaderSource(s, src)
    gl.compileShader(s)
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s))
    return s
  }
  const prog = gl.createProgram()
  const t0 = performance.now()
  try {
    gl.attachShader(prog, sh(gl.VERTEX_SHADER, vs))
    gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, fs))
  } catch (e) {
    return { error: String(e.message || e).slice(0, 500) }
  }
  gl.linkProgram(prog)
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return { error: gl.getProgramInfoLog(prog) }
  const compile = performance.now() - t0
  gl.useProgram(prog)
  const u = (n) => gl.getUniformLocation(prog, n)
  gl.uniform3fv(u('uEye'), cam.eye)
  gl.uniform3fv(u('uFwd'), cam.fwd)
  gl.uniform3fv(u('uRight'), cam.right)
  gl.uniform3fv(u('uUp'), cam.up)
  gl.uniform1f(u('uTanHalf'), cam.tanHalf)
  gl.uniform2f(u('uSize'), w, h)
  gl.uniform3fv(u('uBoxLo'), cam.lo)
  gl.uniform3fv(u('uBoxHi'), cam.hi)
  gl.uniform1f(u('uPixel'), (2 * cam.tanHalf) / h)
  gl.uniform1i(u('uMaxIter'), maxIter)
  gl.uniform1i(u('uIters'), 4096)
  gl.uniform1i(u('uWork'), 100000)
  // 式の座標は表示中の座標そのまま（P = q）
  gl.uniform3fv(u('uPA'), [1, 0, 0, 0, 1, 0, 0, 0, 1])
  gl.uniform1fv(u('uPb'), [0, 0, 0])
  gl.uniform1f(u('uCutOn'), 0)
  const buf = gl.createBuffer()
  gl.bindBuffer(gl.ARRAY_BUFFER, buf)
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW)
  const loc = gl.getAttribLocation(prog, 'aPos')
  gl.enableVertexAttribArray(loc)
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0)
  const tex = gl.createTexture()
  gl.bindTexture(gl.TEXTURE_2D, tex)
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32F, w, h, 0, gl.RGBA, gl.FLOAT, null)
  const fb = gl.createFramebuffer()
  gl.bindFramebuffer(gl.FRAMEBUFFER, fb)
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, tex, 0)
  gl.viewport(0, 0, w, h)
  if (timing) {
    const tq = gl.getExtension('EXT_disjoint_timer_query_webgl2')
    return (async () => {
      const times = []
      for (let k = 0; k < 9; k++) {
        if (tq) {
          const q = gl.createQuery()
          gl.beginQuery(tq.TIME_ELAPSED_EXT, q)
          gl.drawArrays(gl.TRIANGLES, 0, 3)
          gl.endQuery(tq.TIME_ELAPSED_EXT)
          while (!gl.getQueryParameter(q, gl.QUERY_RESULT_AVAILABLE)) await new Promise((r) => setTimeout(r, 1))
          times.push(gl.getQueryParameter(q, gl.QUERY_RESULT) / 1e6)
        } else {
          const s = performance.now()
          gl.drawArrays(gl.TRIANGLES, 0, 3)
          gl.readPixels(0, 0, 1, 1, gl.RGBA, gl.FLOAT, new Float32Array(4))
          times.push(performance.now() - s)
        }
      }
      times.sort((a, b) => a - b)
      return { ms: times[4], gpuTimer: !!tq, compile }
    })()
  }
  gl.drawArrays(gl.TRIANGLES, 0, 3)
  const px = new Float32Array(w * h * 4)
  gl.readPixels(0, 0, w, h, gl.RGBA, gl.FLOAT, px)
  // 下の行から並んでいるので、上の行からに並べ直す（Rust と同じ順）
  const out = []
  for (let j = 0; j < h; j++) {
    const row = h - 1 - j
    for (let i = 0; i < w; i++) {
      const k = (row * w + i) * 4
      out.push([px[k], px[k + 1]])
    }
  }
  return { out, compile }
}

const browser = await chromium.launch(process.env.CHROME_CHANNEL ? { channel: process.env.CHROME_CHANNEL, args: ['--use-angle=d3d11', '--enable-gpu'] } : {})
const page = await browser.newPage()
await page.goto('about:blank')
let failed = 0
const cam = { eye: fx.eye, fwd: fx.fwd, right: fx.right, up: fx.up, tanHalf: fx.tanHalf, lo: fx.lo, hi: fx.hi }
console.log('| 式 | 違う画素 % | 深さの差 p95（画素） | GPU の打ち切り % | 基準の打ち切り % | コンパイル ms |')
console.log('|---|---|---|---|---|---|')
for (const c of fx.cases) {
  const r = await page.evaluate(run, { glsl: c.glsl, ray: RAY_GLSL, cam, w: fx.w, h: fx.h, maxIter: +(process.env.G5_MAXITER ?? 512), timing: false })
  if (r.error) {
    console.log(`| \`${c.src}\` | ✗ ${r.error.split('\n')[0]} | | | | |`)
    failed++
    continue
  }
  let n = 0, wrong = 0, gpuAbort = 0, cpuAbort = 0
  const diffs = []
  for (let k = 0; k < c.rays.length; k++) {
    const truth = c.rays[k]
    const [t, res] = r.out[k]
    if (truth === 'A') {
      cpuAbort++
      continue
    }
    n++
    if (res === 2) {
      gpuAbort++
      continue
    }
    const hit = res === 1
    if (hit !== (truth !== null)) wrong++
    else if (hit) diffs.push(Math.abs(t - truth) / (fx.pixel * truth))
  }
  diffs.sort((a, b) => a - b)
  const p95 = diffs.length ? diffs[Math.min(diffs.length - 1, Math.floor(diffs.length * 0.95))] : 0
  const wrongPct = (100 * wrong) / Math.max(1, n)
  const ok = wrongPct <= 1
  if (!ok) failed++
  const pct = (x) => ((100 * x) / Math.max(1, n)).toFixed(2)
  console.log(`| ${ok ? '' : '✗ '}\`${c.src}\` | ${wrongPct.toFixed(2)} | ${p95.toFixed(2)} | ${pct(gpuAbort)} | ${((100 * cpuAbort) / c.rays.length).toFixed(2)} | ${r.compile.toFixed(0)} |`)
}
if (speed) {
  const g0 = ['x^2+y^2+z^2 = 4', 'sin(x) = cos(y) z', 'tan(x) = z y', 'floor(x y) = z floor(z)', 'mod(x y, z) = 1', 'sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x) = 0']
  console.log('\n| G0 の式 | 1280 × 720・上限 256 の時間 ms | 1920 × 1080 ms |')
  console.log('|---|---|---|')
  for (const src of g0) {
    const c = fx.cases.find((x) => x.src === src)
    const a = await page.evaluate(run, { glsl: c.glsl, ray: RAY_GLSL, cam, w: 1280, h: 720, maxIter: 256, timing: true })
    const b = await page.evaluate(run, { glsl: c.glsl, ray: RAY_GLSL, cam, w: 1920, h: 1080, maxIter: 256, timing: true })
    console.log(`| \`${src}\` | ${a.ms?.toFixed(2) ?? a.error} | ${b.ms?.toFixed(2) ?? b.error} |${a.gpuTimer ? '' : '（GPU の時間を測れないので、描き終わりまでの時間）'}`)
  }
}
await browser.close()
console.log(failed ? `\n${failed} 式が失敗` : '\n全部の式で合格')
process.exit(failed ? 1 : 0)
